//! Redaction of diagnostic data.
//!
//! Captures must never leave the device with a serial number or a Bluetooth
//! address. The state parser knows where the serial lives, so state frames are
//! masked at the byte level and the checksum is recalculated. Free text is
//! additionally scanned for MAC- and serial-shaped strings.

use soundcore_models::{DeviceModel, Layout};
use soundcore_protocol::{Command, encode, parse};

/// Zero the serial field of a TWS Cmm2 state frame and recalculate checksum.
///
/// Returns `None` when the input is not a valid frame. Unknown commands and
/// layouts are returned unchanged; callers should still run `redact_text` over
/// any accompanying metadata.
pub fn redact_state_frame(raw: &[u8], model: DeviceModel) -> Option<Vec<u8>> {
    let frame = parse(raw).ok()?;
    if frame.command != Command::STATE || model.layout != Layout::TwsCmm2 {
        return Some(raw.to_vec());
    }
    let mut body = frame.body;
    if body.len() >= 33 {
        // 16 ASCII bytes of serial followed by NUL; zero the whole field so a
        // parser sees "no serial" and a human sees no identifier.
        body[16..33].fill(0);
    }
    Some(encode(frame.direction, frame.command, &body))
}

/// Replace MAC- and serial-shaped substrings in free text.
pub fn redact_text(input: &str) -> String {
    redact_serial_like(&redact_mac_like(input))
}

fn redact_mac_like(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + 17 <= bytes.len() && is_mac_at(bytes, index) {
            out.push_str("<redacted-mac>");
            index += 17;
        } else {
            out.push(bytes[index] as char);
            index += 1;
        }
    }
    out
}

fn is_mac_at(bytes: &[u8], start: usize) -> bool {
    for offset in 0..17 {
        let position = start + offset;
        let byte = bytes[position];
        let is_separator = matches!(offset, 2 | 5 | 8 | 11 | 14);
        if is_separator {
            if byte != b':' && byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn redact_serial_like(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        let boundary = index == 0 || !bytes[index - 1].is_ascii_hexdigit();
        if boundary && index + 16 <= bytes.len() && is_hex_run(&bytes[index..index + 16]) {
            let after = bytes.get(index + 16).copied();
            if after.map(|byte| !byte.is_ascii_hexdigit()).unwrap_or(true) {
                out.push_str("<redacted-serial>");
                index += 16;
                continue;
            }
        }
        out.push(bytes[index] as char);
        index += 1;
    }
    out
}

fn is_hex_run(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_mac_addresses() {
        assert_eq!(
            redact_text("device A4:C1:39:92:2B:F1 connected"),
            "device <redacted-mac> connected"
        );
        assert_eq!(
            redact_text("device A4-C1-39-92-2B-F1 connected"),
            "device <redacted-mac> connected"
        );
    }

    #[test]
    fn redacts_sixteen_hex_digits() {
        assert_eq!(
            redact_text("serial 0123456789ABCDEF seen"),
            "serial <redacted-serial> seen"
        );
        assert_eq!(
            redact_text("serial 0123456789ABCDEF01"),
            "serial 0123456789ABCDEF01",
            "a longer hex run is not a serial field"
        );
    }

    #[test]
    fn leaves_ordinary_text_and_frames_alone() {
        let text = "frame 09 FF 00 00 01 01 03 0D 00 05 04 FF 22";
        assert_eq!(redact_text(text), text);
    }

    #[test]
    fn masks_the_serial_in_a_state_frame() {
        let fixture = include_str!("../../../data/a3949-state.json");
        let value: serde_json::Value = serde_json::from_str(fixture).expect("fixture");
        let raw: Vec<u8> = value["frame_hex"]
            .as_str()
            .expect("hex")
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("hex"))
            .collect();
        let model = soundcore_models::model_by_code("A3949").expect("model");
        let redacted = redact_state_frame(&raw, model).expect("redact");
        let frame = parse(&redacted).expect("valid redacted frame");
        assert!(frame.body[16..33].iter().all(|byte| *byte == 0));
        assert_eq!(&frame.raw[..5], &soundcore_protocol::INBOUND_PREFIX);
    }
}
