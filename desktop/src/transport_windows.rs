//! Windows Bluetooth transport: paired classic devices, their cached SDP
//! services, and an RFCOMM socket the session layer can read and write.
//!
//! The transport keeps Bluetooth addresses numeric (`u64`) everywhere; the UI
//! formats them with [`format_mac`] only at its own boundary.
//!
//! It implements [`soundcore_session::Transport`]: blocking `write_all` and a
//! partial `read`, both returning [`std::io::Result`], with a read deadline of
//! [`IO_TIMEOUT`] reported as [`io::ErrorKind::TimedOut`]. Ticket 03 owns
//! creating the `DeviceSession` and driving it from a worker thread; `connect`
//! itself blocks while Windows pages the device (tens of seconds), never on a
//! UI thread.

#![allow(dead_code)] // wired into the desktop controller by ticket 03

use std::fmt;
use std::io;
use std::mem::size_of;
use std::time::Duration;

use windows::Win32::Devices::Bluetooth::{
    AF_BTH, BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_FIND_RADIO_PARAMS,
    BTHPROTO_RFCOMM, BluetoothEnumerateInstalledServices, BluetoothFindFirstDevice,
    BluetoothFindFirstRadio, BluetoothFindNextDevice, SOCKADDR_BTH,
};
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_DEVICE_NOT_CONNECTED, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA,
    ERROR_NO_MORE_ITEMS, ERROR_NOT_FOUND, ERROR_SERVICE_NOT_ACTIVE, ERROR_SUCCESS, HANDLE,
    WIN32_ERROR,
};
use windows::Win32::Networking::WinSock::{
    SEND_RECV_FLAGS, SO_RCVTIMEO, SO_SNDTIMEO, SOCK_STREAM, SOCKADDR, SOCKET, SOCKET_ERROR,
    SOL_SOCKET, WSA_ERROR, WSACleanup, WSADATA, WSAEACCES, WSAEADDRNOTAVAIL, WSAECONNREFUSED,
    WSAECONNRESET, WSAEDISCON, WSAEHOSTDOWN, WSAEINVAL, WSAENETDOWN, WSAENETUNREACH, WSAETIMEDOUT,
    WSAGetLastError, WSAHOST_NOT_FOUND, WSANO_DATA, WSASocketW, WSAStartup, connect, recv, send,
    setsockopt,
};
use windows::core::{GUID, Owned};

/// The standard Bluetooth SPP service class UUID, the fallback for models that
/// do not advertise a vendor service.
pub const SPP_SERVICE: GUID = GUID::from_u128(0x0000_1101_0000_1000_8000_0080_5F9B_34FB);

/// The Soundcore vendor service class UUID up to its product-code half; the
/// low 16 bits are the model's product code (see [`vendor_service`]).
pub const VENDOR_SERVICE_PREFIX: u128 = 0x0CF1_2D31_FAC3_4553_BD80_D683_2E7B_0000;

/// The receive and send timeout set on the RFCOMM socket.
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// One classic-Bluetooth device paired with Windows, from the OS pairing cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    /// The numeric Bluetooth address (`BLUETOOTH_ADDRESS.ullLong`).
    pub address: u64,
    /// The device name Windows shows in its own Bluetooth settings.
    pub name: String,
}

/// Everything that can go wrong on the Windows transport, already reduced to
/// the handful of cases the UI distinguishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluetoothError {
    /// The device is not paired, or Windows rejected the link key.
    NotPaired,
    /// No device with that address is paired, or the service is not cached.
    NotFound,
    /// The device did not answer in the I/O or connect window.
    Timeout,
    /// The RFCOMM connection could not be opened or was dropped.
    ConnectionFailed,
    /// Windows reports no Bluetooth radio, i.e. Bluetooth is off.
    BluetoothOff,
    /// Anything else, with the OS error text for diagnostics.
    Other(String),
}

impl BluetoothError {
    /// Maps a Winsock error code (`WSAGetLastError`) to the transport error.
    pub fn from_wsa(code: i32) -> Self {
        match WSA_ERROR(code) {
            WSAEACCES => Self::NotPaired,
            WSAETIMEDOUT => Self::Timeout,
            WSAENETDOWN => Self::BluetoothOff,
            WSAEADDRNOTAVAIL | WSAHOST_NOT_FOUND | WSANO_DATA => Self::NotFound,
            WSAENETUNREACH | WSAEHOSTDOWN | WSAECONNRESET | WSAEDISCON | WSAEINVAL
            | WSAECONNREFUSED => Self::ConnectionFailed,
            _ => Self::Other(format!("Winsock error {code}")),
        }
    }

    /// Maps a Win32 error code (`GetLastError`) to the transport error.
    pub fn from_win32(code: u32) -> Self {
        match WIN32_ERROR(code) {
            ERROR_ACCESS_DENIED => Self::NotPaired,
            ERROR_FILE_NOT_FOUND | ERROR_NOT_FOUND | ERROR_NO_MORE_ITEMS => Self::NotFound,
            ERROR_SERVICE_NOT_ACTIVE | ERROR_DEVICE_NOT_CONNECTED => Self::BluetoothOff,
            _ => Self::Other(format!("Windows error {code}")),
        }
    }
}

impl fmt::Display for BluetoothError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPaired => f.write_str("the device is not paired with Windows"),
            Self::NotFound => f.write_str("no paired Bluetooth device was found"),
            Self::Timeout => f.write_str("the Bluetooth device did not answer in time"),
            Self::ConnectionFailed => {
                f.write_str("could not open a Bluetooth connection to the device")
            }
            Self::BluetoothOff => f.write_str("Bluetooth is turned off"),
            Self::Other(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for BluetoothError {}

impl From<BluetoothError> for io::Error {
    fn from(error: BluetoothError) -> Self {
        match error {
            // The session treats a timed-out read as a retryable wait.
            BluetoothError::Timeout => Self::new(io::ErrorKind::TimedOut, error.to_string()),
            error => Self::other(error),
        }
    }
}

/// The UI string for a Bluetooth address, `AA:BB:CC:DD:EE:FF`.
///
/// Only the lower 48 bits are the address; the transport keeps the numeric
/// form and the UI calls this at its boundary.
pub fn format_mac(address: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (address >> 40) & 0xFF,
        (address >> 32) & 0xFF,
        (address >> 24) & 0xFF,
        (address >> 16) & 0xFF,
        (address >> 8) & 0xFF,
        address & 0xFF,
    )
}

/// Parses the UI's MAC string back to the numeric address; `None` when the
/// string is not six colon-separated hex octets.
pub fn parse_mac(mac: &str) -> Option<u64> {
    let mut address = 0u64;
    let mut octets = mac.trim().split(':');
    for _ in 0..6 {
        let octet = octets.next()?;
        if octet.len() != 2 || !octet.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        address = (address << 8) | u64::from(u8::from_str_radix(octet, 16).ok()?);
    }
    octets.next().is_none().then_some(address)
}

/// The vendor RFCOMM service class of a catalog model code, e.g. `A3949` ->
/// `0CF12D31-FAC3-4553-BD80-D6832E7B3949`; `None` when the code is not hex.
///
/// The catalog code is the hexadecimal product code; the GUID carries only its
/// low 16 bits (the wiki table lists the last four digits of the code).
pub fn vendor_service(model_code: &str) -> Option<GUID> {
    let product_code = u32::from_str_radix(model_code, 16).ok()?;
    Some(GUID::from_u128(
        VENDOR_SERVICE_PREFIX | u128::from(product_code & 0xFFFF),
    ))
}

/// The service to connect to: the model's vendor service when the device
/// advertises it, otherwise the standard SPP service, otherwise `None`.
pub fn pick_service(services: &[GUID], model_code: &str) -> Option<GUID> {
    if let Some(vendor) = vendor_service(model_code)
        && services.contains(&vendor)
    {
        return Some(vendor);
    }
    services.contains(&SPP_SERVICE).then_some(SPP_SERVICE)
}

/// Lists the classic devices paired with Windows (authenticated, remembered or
/// connected) without running an inquiry.
pub fn list_paired_devices() -> Result<Vec<PairedDevice>, BluetoothError> {
    let infos = paired_device_infos()?;
    let mut devices: Vec<PairedDevice> = Vec::with_capacity(infos.len());
    for info in &infos {
        let name = device_name(info);
        let address = device_address(info);
        // The same link key can be cached by more than one local radio; the UI
        // lists a device once.
        if devices.iter().any(|device| device.address == address) {
            continue;
        }
        devices.push(PairedDevice { address, name });
    }
    Ok(devices)
}

/// The RFCOMM service class UUIDs the device advertises, from Windows' cached
/// SDP records.
pub fn installed_services(address: u64) -> Result<Vec<GUID>, BluetoothError> {
    let infos = paired_device_infos()?;
    let info = infos
        .iter()
        .find(|info| device_address(info) == address)
        .ok_or(BluetoothError::NotFound)?;
    installed_services_for(info)
}

/// A connected RFCOMM socket to a paired device.
pub struct WindowsRfcommTransport {
    socket: Owned<SOCKET>,
    // Declared after `socket` so the socket closes before Winsock shuts down.
    _wsa: Wsa,
}

impl WindowsRfcommTransport {
    /// Opens an RFCOMM connection to one service of a paired device.
    ///
    /// The socket is `AF_BTH`/`SOCK_STREAM`/`BTHPROTO_RFCOMM` and connects with
    /// `port: 0`, so Winsock resolves the remote RFCOMM channel from the SDP
    /// service class itself. This blocks for the radio page timeout.
    pub fn connect(address: u64, service: GUID) -> Result<Self, BluetoothError> {
        let wsa = Wsa::start()?;
        // SAFETY: Winsock was just started and RFCOMM needs no protocol info.
        let socket = match unsafe {
            WSASocketW(
                AF_BTH as i32,
                SOCK_STREAM.0,
                BTHPROTO_RFCOMM as i32,
                None,
                0,
                0,
            )
        } {
            Ok(socket) => socket,
            Err(_) => return Err(BluetoothError::from_wsa(unsafe { WSAGetLastError().0 })),
        };
        // SAFETY: the socket was just created and is owned by this value.
        let socket = unsafe { Owned::new(socket) };
        set_io_timeout(*socket, SO_RCVTIMEO)?;
        set_io_timeout(*socket, SO_SNDTIMEO)?;
        let bth = SOCKADDR_BTH {
            addressFamily: AF_BTH,
            btAddr: address,
            serviceClassId: service,
            // Port 0 is what makes Winsock run the SDP lookup for us.
            port: 0,
        };
        // SAFETY: `bth` is fully initialised and its length matches the packed
        // SOCKADDR_BTH layout the Winsock call expects.
        let result = unsafe {
            connect(
                *socket,
                &bth as *const SOCKADDR_BTH as *const SOCKADDR,
                size_of::<SOCKADDR_BTH>() as i32,
            )
        };
        if result == SOCKET_ERROR {
            return Err(BluetoothError::from_wsa(unsafe { WSAGetLastError().0 }));
        }
        Ok(Self { socket, _wsa: wsa })
    }

    /// Writes the whole buffer, retrying short sends. The session layer passes
    /// complete frames; this method never returns after a partial write.
    pub fn write_all(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            // SAFETY: the socket is connected and `bytes` is a valid buffer.
            let sent = unsafe { send(*self.socket, bytes, SEND_RECV_FLAGS(0)) };
            if sent < 0 {
                let code = unsafe { WSAGetLastError() };
                return Err(BluetoothError::from_wsa(code.0).into());
            }
            let sent = sent as usize;
            if sent == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "the Bluetooth connection closed while writing",
                ));
            }
            bytes = &bytes[sent..];
        }
        Ok(())
    }

    /// Reads whatever arrived into `buffer`; `Ok(0)` means the peer closed the
    /// connection. Frames are reassembled by the session layer, not here.
    pub fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // SAFETY: the socket is connected and `buffer` is a valid destination.
        let received = unsafe { recv(*self.socket, buffer, SEND_RECV_FLAGS(0)) };
        if received < 0 {
            let code = unsafe { WSAGetLastError() };
            return Err(BluetoothError::from_wsa(code.0).into());
        }
        Ok(received as usize)
    }
}

/// The Windows socket is the blocking byte transport the session layer expects:
/// a timed-out read is an [`io::ErrorKind::TimedOut`] `io::Error`, and every
/// short send is retried by [`WindowsRfcommTransport::write_all`].
impl soundcore_session::Transport for WindowsRfcommTransport {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        WindowsRfcommTransport::write_all(self, bytes)
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        WindowsRfcommTransport::read(self, buffer)
    }
}

/// Sets a socket timeout; Winsock expects a DWORD of milliseconds.
fn set_io_timeout(socket: SOCKET, option: i32) -> Result<(), BluetoothError> {
    let millis = u32::try_from(IO_TIMEOUT.as_millis()).unwrap_or(u32::MAX);
    let value = millis.to_ne_bytes();
    // SAFETY: the socket is valid and `value` is a DWORD holding milliseconds.
    let result = unsafe { setsockopt(socket, SOL_SOCKET, option, Some(&value)) };
    if result == SOCKET_ERROR {
        return Err(BluetoothError::from_wsa(unsafe { WSAGetLastError().0 }));
    }
    Ok(())
}

/// One `WSAStartup` reference, released by a matching `WSACleanup`.
struct Wsa;

impl Wsa {
    fn start() -> Result<Self, BluetoothError> {
        let mut data = WSADATA::default();
        // SAFETY: `data` is a valid out-parameter for the requested Winsock
        // version, and a successful call is matched in `Drop`.
        let code = unsafe { WSAStartup(0x0202, &mut data) };
        if code == 0 {
            Ok(Self)
        } else {
            Err(BluetoothError::from_wsa(code))
        }
    }
}

impl Drop for Wsa {
    fn drop(&mut self) {
        // SAFETY: every `Wsa` comes from exactly one successful `WSAStartup`.
        unsafe {
            WSACleanup();
        }
    }
}

/// Whether Windows has a Bluetooth radio to talk to. Enumeration comes back
/// empty both when nothing is paired and when Bluetooth is off, so this probe
/// is what tells the two apart.
fn bluetooth_radio_available() -> Result<bool, BluetoothError> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut radio = HANDLE::default();
    // SAFETY: the params carry their size, `radio` is a valid out-parameter,
    // and both returned handles are owned by `Owned` and closed on return.
    unsafe {
        match BluetoothFindFirstRadio(&params, &mut radio) {
            Ok(find) => {
                let _find = Owned::new(find);
                let _radio = Owned::new(radio);
                Ok(true)
            }
            Err(error) => {
                let code = win32_code(&error);
                if is_nothing(code) {
                    Ok(false)
                } else {
                    Err(BluetoothError::from_win32(code))
                }
            }
        }
    }
}

/// The Windows device records for every paired classic device, without
/// issuing an inquiry.
fn paired_device_infos() -> Result<Vec<BLUETOOTH_DEVICE_INFO>, BluetoothError> {
    if !bluetooth_radio_available()? {
        return Err(BluetoothError::BluetoothOff);
    }
    // SAFETY: both structs are plain C structures with their `dwSize` set, and
    // the find handle is owned by `Owned` until enumeration finishes.
    unsafe {
        let search = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: true.into(),
            fReturnRemembered: true.into(),
            fReturnUnknown: false.into(),
            fReturnConnected: true.into(),
            // No inquiry: only devices already in the Windows pairing cache.
            fIssueInquiry: false.into(),
            cTimeoutMultiplier: 0,
            // A null radio searches every local Bluetooth radio.
            hRadio: HANDLE::default(),
        };
        let mut info = BLUETOOTH_DEVICE_INFO {
            dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };
        let find = match BluetoothFindFirstDevice(&search, &mut info) {
            Ok(find) => Owned::new(find),
            Err(error) => {
                let code = win32_code(&error);
                if is_nothing(code) {
                    return Ok(Vec::new());
                }
                return Err(BluetoothError::from_win32(code));
            }
        };
        let mut infos = vec![info];
        while BluetoothFindNextDevice(*find, &mut info).is_ok() {
            infos.push(info);
        }
        Ok(infos)
    }
}

/// The services the device advertises, from Windows' cached SDP records. The
/// first call with a null buffer only asks how many GUIDs there are.
fn installed_services_for(info: &BLUETOOTH_DEVICE_INFO) -> Result<Vec<GUID>, BluetoothError> {
    let mut count = 0u32;
    // SAFETY: `info` is a filled device record and a null buffer is the
    // documented way to ask for the GUID count.
    let code = unsafe { BluetoothEnumerateInstalledServices(None, info, &mut count, None) };
    if !is_service_query_success(code) {
        return Err(BluetoothError::from_win32(code));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut services = vec![GUID::zeroed(); count as usize];
    let mut written = count;
    // SAFETY: the buffer holds `count` GUIDs, which is what the count query
    // reported; `written` reports how many were actually filled.
    let code = unsafe {
        BluetoothEnumerateInstalledServices(None, info, &mut written, Some(services.as_mut_ptr()))
    };
    if !is_service_query_success(code) {
        return Err(BluetoothError::from_win32(code));
    }
    services.truncate(written.min(count) as usize);
    Ok(services)
}

/// `BluetoothEnumerateInstalledServices` reports success and, for the count
/// query, `ERROR_MORE_DATA`; both mean the query itself worked.
fn is_service_query_success(code: u32) -> bool {
    matches!(WIN32_ERROR(code), ERROR_SUCCESS | ERROR_MORE_DATA)
}

/// The numeric Bluetooth address of a device record; `ullLong` is exactly the
/// form `SOCKADDR_BTH.btAddr` expects, with no byte swap.
fn device_address(info: &BLUETOOTH_DEVICE_INFO) -> u64 {
    // SAFETY: `Address` is a union the OS filled; `ullLong` is the u64 view of
    // the same six address bytes, and any u64 is a valid read.
    unsafe { info.Address.Anonymous.ullLong }
}

/// The device name from the fixed-size UTF-16 buffer Windows fills.
fn device_name(info: &BLUETOOTH_DEVICE_INFO) -> String {
    let units: Vec<u16> = info
        .szName
        .iter()
        .copied()
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// The Win32 code that windows-rs wrapped into the error's HRESULT.
fn win32_code(error: &windows::core::Error) -> u32 {
    // `Error::from_thread` is HRESULT_FROM_WIN32, whose low 16 bits are the
    // original code.
    (error.code().0 as u32) & 0xFFFF
}

/// The codes Windows uses when a Bluetooth query simply found nothing.
fn is_nothing(code: u32) -> bool {
    matches!(
        WIN32_ERROR(code),
        ERROR_FILE_NOT_FOUND | ERROR_NOT_FOUND | ERROR_NO_MORE_ITEMS
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guid(value: u128) -> GUID {
        GUID::from_u128(value)
    }

    #[test]
    fn formats_an_address_as_six_ui_octets() {
        assert_eq!(format_mac(0x0000_A4C1_3992_1122), "A4:C1:39:92:11:22");
        assert_eq!(format_mac(0x0000_0000_0000_0001), "00:00:00:00:00:01");
        assert_eq!(format_mac(0x0000_F49D_8AA2_ABCD), "F4:9D:8A:A2:AB:CD");
    }

    #[test]
    fn formatting_ignores_bits_above_the_48_bit_address() {
        assert_eq!(
            format_mac(0xFFFF_F49D_8AA2_1122),
            format_mac(0xF49D_8AA2_1122)
        );
    }

    #[test]
    fn a_formatted_mac_parses_back_to_the_address() {
        for address in [
            0x0000_0000_0000_0000,
            0x0000_A4C1_3992_1122,
            0x0000_FFFF_FFFF_FFFF,
        ] {
            assert_eq!(parse_mac(&format_mac(address)), Some(address));
        }
    }

    #[test]
    fn parsing_accepts_lower_case_hex() {
        assert_eq!(parse_mac("a4:c1:39:92:11:22"), Some(0x0000_A4C1_3992_1122));
    }

    #[test]
    fn parsing_rejects_malformed_macs() {
        for mac in [
            "",
            "A4:C1:39:92:11",
            "A4:C1:39:92:11:22:33",
            "A4-C1-39-92-11-22",
            "G4:C1:39:92:11:22",
            "A4:C1:39:92:11:2",
        ] {
            assert_eq!(parse_mac(mac), None, "{mac:?} parsed");
        }
    }

    #[test]
    fn parses_a_mac_after_trimming_outer_whitespace() {
        assert_eq!(
            parse_mac(" A4:C1:39:92:11:22 "),
            Some(0x0000_A4C1_3992_1122)
        );
    }

    #[test]
    fn vendor_service_appends_the_product_code() {
        for (code, expected) in [
            ("A3949", 0x0CF1_2D31_FAC3_4553_BD80_D683_2E7B_3949),
            ("A3954", 0x0CF1_2D31_FAC3_4553_BD80_D683_2E7B_3954),
            ("A3035", 0x0CF1_2D31_FAC3_4553_BD80_D683_2E7B_3035),
            ("A3062", 0x0CF1_2D31_FAC3_4553_BD80_D683_2E7B_3062),
        ] {
            assert_eq!(vendor_service(code), Some(guid(expected)), "{code}");
        }
    }

    #[test]
    fn a_non_hex_model_code_has_no_vendor_service() {
        assert_eq!(vendor_service(""), None);
        assert_eq!(vendor_service("not-a-code"), None);
    }

    #[test]
    fn the_spp_service_is_the_standard_class() {
        assert_eq!(SPP_SERVICE, guid(0x0000_1101_0000_1000_8000_0080_5F9B_34FB));
    }

    #[test]
    fn the_vendor_service_wins_over_spp() {
        let vendor = vendor_service("A3949").expect("A3949 has a vendor service");
        let services = [guid(0x1), SPP_SERVICE, vendor];
        assert_eq!(pick_service(&services, "A3949"), Some(vendor));
    }

    #[test]
    fn the_standard_spp_service_is_the_fallback() {
        let services = [guid(0x1), SPP_SERVICE];
        assert_eq!(pick_service(&services, "A3949"), Some(SPP_SERVICE));
        // A code outside the vendor scheme still falls back to SPP.
        assert_eq!(pick_service(&services, "not-a-code"), Some(SPP_SERVICE));
    }

    #[test]
    fn no_known_service_means_no_choice() {
        let services = [guid(0x1), guid(0x2)];
        assert_eq!(pick_service(&services, "A3949"), None);
        assert_eq!(pick_service(&[], "A3949"), None);
    }

    #[test]
    fn winsock_errors_map_to_transport_errors() {
        assert_eq!(BluetoothError::from_wsa(10013), BluetoothError::NotPaired);
        assert_eq!(BluetoothError::from_wsa(10060), BluetoothError::Timeout);
        assert_eq!(
            BluetoothError::from_wsa(10051),
            BluetoothError::ConnectionFailed
        );
        assert_eq!(
            BluetoothError::from_wsa(10064),
            BluetoothError::ConnectionFailed
        );
        assert_eq!(
            BluetoothError::from_wsa(10054),
            BluetoothError::ConnectionFailed
        );
        assert_eq!(BluetoothError::from_wsa(10049), BluetoothError::NotFound);
        assert_eq!(
            BluetoothError::from_wsa(10050),
            BluetoothError::BluetoothOff
        );
    }

    #[test]
    fn win32_errors_map_to_transport_errors() {
        assert_eq!(BluetoothError::from_win32(5), BluetoothError::NotPaired);
        assert_eq!(BluetoothError::from_win32(259), BluetoothError::NotFound);
        assert_eq!(BluetoothError::from_win32(2), BluetoothError::NotFound);
        assert_eq!(BluetoothError::from_win32(1168), BluetoothError::NotFound);
        assert_eq!(
            BluetoothError::from_win32(1062),
            BluetoothError::BluetoothOff
        );
    }

    #[test]
    fn unknown_os_errors_keep_their_code_for_diagnostics() {
        let unknown = BluetoothError::from_wsa(12345);
        assert!(
            matches!(&unknown, BluetoothError::Other(detail) if detail.contains("12345")),
            "{unknown:?}"
        );
        let unknown = BluetoothError::from_win32(12345);
        assert!(
            matches!(&unknown, BluetoothError::Other(detail) if detail.contains("12345")),
            "{unknown:?}"
        );
    }

    #[test]
    fn every_error_has_a_readable_message() {
        for (error, expected) in [
            (
                BluetoothError::NotPaired,
                "the device is not paired with Windows",
            ),
            (
                BluetoothError::NotFound,
                "no paired Bluetooth device was found",
            ),
            (
                BluetoothError::Timeout,
                "the Bluetooth device did not answer in time",
            ),
            (
                BluetoothError::ConnectionFailed,
                "could not open a Bluetooth connection to the device",
            ),
            (BluetoothError::BluetoothOff, "Bluetooth is turned off"),
        ] {
            assert_eq!(error.to_string(), expected);
        }
        assert_eq!(
            BluetoothError::Other("the socket closed".into()).to_string(),
            "the socket closed"
        );
    }

    #[test]
    fn a_timeout_read_becomes_a_timed_out_io_error() {
        let error: io::Error = BluetoothError::Timeout.into();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn other_transport_errors_become_other_io_errors() {
        let error: io::Error = BluetoothError::ConnectionFailed.into();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("Bluetooth connection"));
    }

    #[test]
    fn the_socket_is_a_session_transport() {
        // Compile-time check that ticket 03 can hand this socket to
        // `DeviceSession::new`; the hardware is not needed for it.
        fn assert_transport<T: soundcore_session::Transport>() {}
        assert_transport::<WindowsRfcommTransport>();
    }

    /// Manual hardware check, not part of the automated suite:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_paired_enumeration -- --ignored --nocapture
    /// ```
    ///
    /// It prints paired devices and their vendor/SPP services with the last two
    /// address octets masked, so the output is safe to paste into a report.
    #[test]
    #[ignore = "manual hardware check: needs paired Bluetooth devices"]
    fn manual_paired_enumeration() {
        let devices = list_paired_devices().expect("paired devices");
        println!("{} paired classic device(s)", devices.len());
        for device in &devices {
            let mac = format_mac(device.address);
            println!("- {} ({})", device.name, masked_mac(&mac));
            match installed_services(device.address) {
                Ok(services) => {
                    println!("    {} installed service(s)", services.len());
                    for code in ["A3949", "A3954", "A3035", "A3062"] {
                        let Some(vendor) = vendor_service(code) else {
                            continue;
                        };
                        if services.contains(&vendor) {
                            println!("    vendor {code}: {}", guid_string(vendor));
                        }
                    }
                    if services.contains(&SPP_SERVICE) {
                        println!("    standard SPP: {}", guid_string(SPP_SERVICE));
                    }
                }
                Err(error) => println!("    services unavailable: {error}"),
            }
        }
    }

    fn masked_mac(mac: &str) -> String {
        format!("{} __:__", &mac[..12])
    }

    fn guid_string(guid: GUID) -> String {
        let value = guid.to_u128();
        format!(
            "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
            value >> 96,
            (value >> 80) & 0xFFFF,
            (value >> 64) & 0xFFFF,
            (value >> 48) & 0xFFFF,
            value & 0x0000_FFFF_FFFF_FFFF
        )
    }
}
