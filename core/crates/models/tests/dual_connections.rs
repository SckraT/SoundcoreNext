//! Dual-connections list parser and write-frame tests.
//!
//! The committed fixture is a real `0B 01` answer of a Space One Pro captured
//! over the Windows RFCOMM transport on 2026-11-21. Its device names are
//! replaced by same-length placeholders and its addresses by neutral bytes;
//! flags, name lengths, device order and frame order are unchanged.

use soundcore_models::{
    DualConnectionDevice, DualConnectionsError, DualConnectionsFrame, DualConnectionsList,
    dual_connections_connect_frame, dual_connections_disconnect_frame,
    dual_connections_forget_frame, dual_connections_request, model_by_code,
    parse_dual_connections_frame,
};
use soundcore_protocol::{Command, Direction, encode};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn mac(text: &str) -> [u8; 6] {
    let bytes: Vec<u8> = text
        .split(':')
        .map(|octet| u8::from_str_radix(octet, 16).expect("hex octet"))
        .collect();
    bytes.try_into().expect("six address bytes")
}

/// One 40-byte device record: length `0x28`, flag, six address bytes and the
/// 32-byte name field right-padded with NULs.
fn padded_device(name: &[u8], address: [u8; 6], connected: bool) -> Vec<u8> {
    device_with_name_field(name, address, connected, 32)
}

/// One device record whose name field is exactly `field_length` bytes; the
/// declared length byte always matches the field.
fn device_with_name_field(
    name: &[u8],
    address: [u8; 6],
    connected: bool,
    field_length: usize,
) -> Vec<u8> {
    assert!(name.len() <= field_length);
    let length = u8::try_from(8 + field_length).expect("device record length");
    let mut record = vec![length, u8::from(connected)];
    record.extend_from_slice(&address);
    record.extend_from_slice(name);
    record.resize(8 + field_length, 0);
    record
}

/// A reply-frame body: `[total, current, devices...]`.
fn body(total: u8, current: u8, records: &[Vec<u8>]) -> Vec<u8> {
    let mut body = vec![total, current];
    for record in records {
        body.extend_from_slice(record);
    }
    body
}

/// A whole inbound `0B 01` frame.
fn frame(total: u8, current: u8, records: &[Vec<u8>]) -> Vec<u8> {
    encode(
        Direction::Inbound,
        soundcore_models::DUAL_CONNECTIONS_LIST,
        &body(total, current, records),
    )
}

#[derive(serde::Deserialize)]
struct Fixture {
    frames: Vec<FrameFixture>,
    expected: Expected,
}

#[derive(serde::Deserialize)]
struct FrameFixture {
    frame_hex: String,
    total: u8,
    current: u8,
}

#[derive(serde::Deserialize)]
struct Expected {
    devices: Vec<DeviceExpected>,
}

#[derive(serde::Deserialize)]
struct DeviceExpected {
    name: String,
    mac: String,
    connected: bool,
}

#[test]
fn parses_the_real_a3062_dual_connections_list() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3062-dual-connections.json"))
            .expect("fixture");
    let mut list = DualConnectionsList::new();
    for (index, entry) in fixture.frames.iter().enumerate() {
        let raw = hex(&entry.frame_hex);
        let frame = parse_dual_connections_frame(&raw).expect("a real reply frame parses");
        assert_eq!(frame.total, entry.total, "frame {index} total");
        assert_eq!(frame.current, entry.current, "frame {index} current");
        let complete = list.push_frame(frame);
        assert_eq!(
            complete,
            index + 1 == fixture.frames.len(),
            "frame {index} terminates the reply"
        );
    }

    let expected: Vec<DualConnectionDevice> = fixture
        .expected
        .devices
        .iter()
        .map(|device| DualConnectionDevice {
            name: device.name.clone(),
            mac: mac(&device.mac),
            connected: device.connected,
        })
        .collect();
    assert_eq!(list.into_devices(), expected);
}

#[test]
fn parses_several_devices_from_one_frame() {
    let phone = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let laptop = [0x02, 0x00, 0x00, 0x00, 0x00, 0x02];
    let raw = frame(
        1,
        1,
        &[
            padded_device(b"Phone", phone, true),
            padded_device(b"Laptop", laptop, false),
        ],
    );

    let frame = parse_dual_connections_frame(&raw).expect("parse");

    assert_eq!((frame.total, frame.current), (1, 1));
    assert_eq!(
        frame.devices,
        vec![
            DualConnectionDevice {
                name: "Phone".to_owned(),
                mac: phone,
                connected: true,
            },
            DualConnectionDevice {
                name: "Laptop".to_owned(),
                mac: laptop,
                connected: false,
            },
        ]
    );
}

#[test]
fn keeps_the_valid_utf8_prefix_of_a_name_cut_in_half() {
    // The device pads the name field to 32 bytes and can cut a multibyte
    // character in half. "Device " is seven bytes and "пользователь" is 24, so
    // the field ends after the first byte of the next two-byte character.
    let mut name_field = b"Device ".to_vec();
    name_field.extend_from_slice("пользователь".as_bytes());
    assert_eq!(name_field.len(), 31);
    name_field.push(0xD0);
    let raw = frame(
        1,
        1,
        &[device_with_name_field(
            &name_field,
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x03],
            false,
            32,
        )],
    );

    let frame = parse_dual_connections_frame(&raw).expect("a cut character must not drop the list");

    assert_eq!(frame.devices.len(), 1);
    assert_eq!(frame.devices[0].name, "Device пользователь");
    assert!(!frame.devices[0].connected);
}

#[test]
fn keeps_a_name_that_fills_the_field_without_nul_termination() {
    let name = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
    assert_eq!(name.len(), 32);
    let raw = frame(
        1,
        1,
        &[device_with_name_field(
            name,
            [0x02, 0x00, 0x00, 0x00, 0x00, 0x04],
            true,
            32,
        )],
    );

    let frame = parse_dual_connections_frame(&raw).expect("no terminator is fine");

    assert_eq!(frame.devices[0].name, "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345");
}

#[test]
fn a_shorter_declared_name_field_is_clamped_to_the_bytes_present() {
    // A real device was observed sending a lone final record padded a few bytes
    // short of its own length byte; the record is still parsed.
    let mut raw = encode(
        Direction::Inbound,
        soundcore_models::DUAL_CONNECTIONS_LIST,
        &body(
            1,
            1,
            &[device_with_name_field(
                b"Tablet",
                [0x02, 0x00, 0x00, 0x00, 0x00, 0x05],
                true,
                32,
            )],
        ),
    );
    // Drop the final ten NUL padding bytes, so the declared 32-byte name field
    // has only 22 bytes on the wire.
    let checksum = raw.pop().expect("checksum");
    raw.truncate(raw.len() - 10);
    let total = u16::try_from(raw.len() + 1).expect("frame length");
    raw[7..9].copy_from_slice(&total.to_le_bytes());
    raw.push(checksum);
    let checksum = soundcore_protocol::checksum(&raw[..raw.len() - 1]);
    let last = raw.len() - 1;
    raw[last] = checksum;

    let frame = parse_dual_connections_frame(&raw).expect("the clamped record parses");

    assert_eq!(frame.devices.len(), 1);
    assert_eq!(frame.devices[0].name, "Tablet");
}

#[test]
fn rejects_a_bad_frame_count() {
    for (total, current) in [(0, 1), (2, 0), (2, 3)] {
        let raw = frame(total, current, &[]);
        let error = parse_dual_connections_frame(&raw).expect_err("bad count");
        assert_eq!(
            error,
            DualConnectionsError::BadFrameCount { total, current }
        );
    }
}

#[test]
fn rejects_a_body_shorter_than_the_frame_count() {
    for body in [&[][..], &[1][..]] {
        let raw = encode(
            Direction::Inbound,
            soundcore_models::DUAL_CONNECTIONS_LIST,
            body,
        );
        let error = parse_dual_connections_frame(&raw).expect_err("short body");
        assert!(
            matches!(
                error,
                DualConnectionsError::ShortBody {
                    actual,
                    minimum: 2
                } if actual == body.len()
            ),
            "{error}"
        );
    }
}

#[test]
fn rejects_a_device_record_below_the_eight_fixed_bytes() {
    let raw = frame(1, 1, &[vec![7, 1, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01]]);
    let error = parse_dual_connections_frame(&raw).expect_err("bad device length");
    assert_eq!(
        error,
        DualConnectionsError::BadDeviceLength {
            offset: 2,
            length: 7
        }
    );
}

#[test]
fn rejects_a_device_record_cut_off_in_its_fixed_bytes() {
    let raw = encode(
        Direction::Inbound,
        soundcore_models::DUAL_CONNECTIONS_LIST,
        &[1, 1, 0x28, 0x01, 0x02],
    );
    let error = parse_dual_connections_frame(&raw).expect_err("cut device");
    assert_eq!(
        error,
        DualConnectionsError::ShortDevice {
            offset: 2,
            remaining: 3
        }
    );
}

#[test]
fn rejects_a_frame_that_is_not_a_dual_connections_reply() {
    let state = encode(Direction::Inbound, Command::STATE, &[1, 1, 5, 4]);
    let error = parse_dual_connections_frame(&state).expect_err("not 0B 01");
    assert_eq!(error, DualConnectionsError::NotADualConnectionsFrame);
}

#[test]
fn accumulates_out_of_order_frames_and_restarts_on_current_one() {
    let first = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let second = [0x02, 0x00, 0x00, 0x00, 0x00, 0x02];
    let third = [0x02, 0x00, 0x00, 0x00, 0x00, 0x03];
    let mut list = DualConnectionsList::new();

    // The second frame arrives first and appends out of order.
    let parsed =
        parse_dual_connections_frame(&frame(3, 2, &[padded_device(b"Second", second, true)]))
            .expect("second");
    assert!(!list.push_frame(parsed));
    assert_eq!(
        list.devices()
            .iter()
            .map(|device| device.name.as_str())
            .collect::<Vec<_>>(),
        ["Second"]
    );

    // A late first frame starts a new list, dropping the out-of-order devices.
    let parsed =
        parse_dual_connections_frame(&frame(3, 1, &[padded_device(b"First", first, false)]))
            .expect("first");
    assert!(!list.push_frame(parsed));
    assert_eq!(
        list.devices()
            .iter()
            .map(|device| device.name.as_str())
            .collect::<Vec<_>>(),
        ["First"]
    );

    // The last frame appends and terminates the reply.
    let parsed =
        parse_dual_connections_frame(&frame(3, 3, &[padded_device(b"Third", third, false)]))
            .expect("third");
    assert!(list.push_frame(parsed));
    assert_eq!(
        list.into_devices()
            .iter()
            .map(|device| device.name.as_str())
            .collect::<Vec<_>>(),
        ["First", "Third"]
    );
}

#[test]
fn builds_the_exact_read_connect_disconnect_and_forget_frames() {
    assert_eq!(
        hex_line(&dual_connections_request()),
        "08 EE 00 00 00 0B 01 0A 00 0C",
        "0B 01 asks for the list with an empty body"
    );

    // The address bytes travel in reply order: no byte reversal.
    let address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let address_hex = "02 00 00 00 00 01";
    assert_eq!(
        hex_line(&dual_connections_connect_frame(address)),
        format!("08 EE 00 00 00 0B 82 10 00 {address_hex} 96")
    );
    assert_eq!(
        hex_line(&dual_connections_disconnect_frame(address)),
        format!("08 EE 00 00 00 0B 81 10 00 {address_hex} 95")
    );
    assert_eq!(
        hex_line(&dual_connections_forget_frame(address)),
        format!("08 EE 00 00 00 0B 83 10 00 {address_hex} 97")
    );
}

#[test]
fn every_write_frame_carries_its_address_bytes_unchanged_and_in_order() {
    // A neutral, locally administered fixture address: the bytes travel in reply
    // order and are never reversed or re-encoded on the way into a frame.
    let address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x06];
    for frame in [
        dual_connections_connect_frame(address),
        dual_connections_disconnect_frame(address),
        dual_connections_forget_frame(address),
    ] {
        assert_eq!(&frame[9..15], &address);
    }
}

#[test]
fn only_the_measured_a3062_has_dual_connections() {
    assert!(model_by_code("A3062").expect("A3062").dual_connections);
    for code in ["A3035", "A3040", "A3947", "A3949", "A3954"] {
        let model = model_by_code(code).expect("catalog model");
        assert!(!model.dual_connections, "{code} must stay deferred");
    }
}

#[test]
fn an_empty_last_frame_terminates_an_empty_list() {
    let raw = frame(1, 1, &[]);
    let mut list = DualConnectionsList::new();
    assert!(list.push_frame(parse_dual_connections_frame(&raw).expect("parse")));
    assert!(list.into_devices().is_empty());
}

#[test]
fn a_frame_type_is_the_parsed_reply_shape() {
    // `DualConnectionsFrame` is the per-frame parser result; a one-frame reply
    // of two devices is the shape the session accumulates.
    let raw = frame(
        1,
        1,
        &[
            padded_device(b"Phone", [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], true),
            padded_device(b"Tablet", [0x02, 0x00, 0x00, 0x00, 0x00, 0x02], false),
        ],
    );
    let parsed: DualConnectionsFrame = parse_dual_connections_frame(&raw).expect("parse");
    assert_eq!(parsed.devices.len(), 2);
}
