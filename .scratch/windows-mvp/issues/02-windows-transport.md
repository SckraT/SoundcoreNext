# 02 — Windows transport

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Implement Windows paired-device enumeration and an RFCOMM transport that can be handed to the session layer.

## Files

- `desktop/Cargo.toml` (add `windows` 0.62 with `Win32_Foundation`, `Win32_Devices_Bluetooth`, `Win32_Networking_WinSock`; add a path dependency on the session crate only if needed for the trait)
- `desktop/src/transport_windows.rs`
- `desktop/src/main.rs` only for `mod transport_windows;` (no UI wiring; ticket 03 owns it)

## Requirements

1. `list_paired_devices() -> Result<Vec<PairedDevice>, BluetoothError>` using `BluetoothFindFirstDevice`/`BluetoothFindNextDevice` with authenticated/remembered/connected flags, no inquiry.
2. Each `PairedDevice` exposes name and the numeric address; format the MAC string only at the UI boundary.
3. `installed_services(address) -> Result<Vec<Uuid>, BluetoothError>` via `BluetoothEnumerateInstalledServices` (handle the `ERROR_MORE_DATA` count query).
4. `vendor_service(model_code) -> Uuid` for `0CF12D31-FAC3-4553-BD80-D6832E7Bxxxx`; `pick_service(services, model_code)` prefers the vendor UUID and falls back to SPP `00001101-0000-1000-8000-00805F9B34FB`.
5. `WindowsRfcommTransport`:
   - connect with `WSASocketW(AF_BTH, SOCK_STREAM, BTHPROTO_RFCOMM)` and `connect` to `SOCKADDR_BTH { btAddr, serviceClassId, port: 0 }`;
   - set `SO_RCVTIMEO`/`SO_SNDTIMEO` (e.g. 5 s);
   - implement the core `Transport` trait (write all, partial read);
   - map Winsock errors to a small `BluetoothError` enum (`NotPaired`, `NotFound`, `Timeout`, `ConnectionFailed`, `BluetoothOff`, `Other`) with `Display`;
   - drop closes the socket and cleans WSA.
6. Unit tests for MAC formatting/parsing, vendor UUID derivation, service preference and error mapping. Hardware enumeration may be a manual check (log names; mask last 4 hex in reports).
7. No UI edits and no writes to any device.

## Acceptance

- [ ] `cargo build`, `cargo test`, `fmt --check`, `clippy -D warnings` pass in `desktop`.
- [ ] Manual: enumerate the machine's paired devices, see the three Soundcore devices and their vendor service UUIDs (mask addresses in the report).
- [ ] No `.slint` file changed.
