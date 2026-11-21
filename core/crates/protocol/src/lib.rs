//! Soundcore SPP framing.
//!
//! Every packet is a fixed prefix, a two-byte command, a little-endian total
//! length (checksum included), the body, and a one-byte additive checksum.
//!
//! This crate deliberately contains no model knowledge: it only frames and
//! validates bytes. Model-specific state lives in `soundcore-models`.

use serde::Serialize;
use std::fmt;

pub const OUTBOUND_PREFIX: [u8; 5] = [0x08, 0xEE, 0x00, 0x00, 0x00];
pub const INBOUND_PREFIX: [u8; 5] = [0x09, 0xFF, 0x00, 0x00, 0x01];

/// Minimum frame: prefix (5), command (2), length (2), checksum (1).
pub const MIN_FRAME_LEN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Command {
    pub group: u8,
    pub opcode: u8,
}

impl Command {
    pub const STATE: Self = Self::new(0x01, 0x01);
    pub const BATTERY: Self = Self::new(0x01, 0x03);
    pub const CHARGING: Self = Self::new(0x01, 0x04);
    /// The ambient sound mode command: an outbound `06 81` carries the
    /// six-byte mode body, and the device acknowledges it with an empty inbound
    /// `06 81`.
    pub const ACK_AMBIENT_SOUND_MODE: Self = Self::new(0x06, 0x81);
    /// The equalizer command: an outbound `03 87` carries the A3062 preset
    /// body, and the device acknowledges it with an empty inbound `03 87`.
    pub const ACK_EQUALIZER: Self = Self::new(0x03, 0x87);

    pub const fn new(group: u8, opcode: u8) -> Self {
        Self { group, opcode }
    }

    pub const fn bytes(self) -> [u8; 2] {
        [self.group, self.opcode]
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X} {:02X}", self.group, self.opcode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Outbound,
    Inbound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub direction: Direction,
    pub command: Command,
    pub body: Vec<u8>,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TooShort { actual: usize },
    UnknownPrefix([u8; 5]),
    LengthMismatch { declared: usize, actual: usize },
    ChecksumMismatch { expected: u8, actual: u8 },
    MissingChecksum,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { actual } => {
                write!(f, "frame is {actual} bytes, need at least {MIN_FRAME_LEN}")
            }
            Self::UnknownPrefix(prefix) => write!(f, "unknown prefix {prefix:02X?}"),
            Self::LengthMismatch { declared, actual } => {
                write!(f, "declared length {declared}, actual {actual}")
            }
            Self::ChecksumMismatch { expected, actual } => {
                write!(f, "checksum {actual:02X}, expected {expected:02X}")
            }
            Self::MissingChecksum => write!(f, "frame has no checksum byte"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Additive checksum over every byte before the checksum.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

pub fn direction_of(prefix: [u8; 5]) -> Option<Direction> {
    if prefix == OUTBOUND_PREFIX {
        Some(Direction::Outbound)
    } else if prefix == INBOUND_PREFIX {
        Some(Direction::Inbound)
    } else {
        None
    }
}

pub fn parse(raw: &[u8]) -> Result<Frame, ParseError> {
    if raw.len() < MIN_FRAME_LEN {
        return Err(ParseError::TooShort { actual: raw.len() });
    }
    let prefix: [u8; 5] = raw[..5]
        .try_into()
        .map_err(|_| ParseError::UnknownPrefix([0; 5]))?;
    let direction = direction_of(prefix).ok_or(ParseError::UnknownPrefix(prefix))?;
    let command = Command::new(raw[5], raw[6]);
    let declared = usize::from(u16::from_le_bytes([raw[7], raw[8]]));
    if declared != raw.len() {
        return Err(ParseError::LengthMismatch {
            declared,
            actual: raw.len(),
        });
    }
    let (with_checksum, checksum_bytes) = raw.split_at(raw.len() - 1);
    let actual = checksum_bytes[0];
    let expected = checksum(with_checksum);
    if actual != expected {
        return Err(ParseError::ChecksumMismatch { expected, actual });
    }
    Ok(Frame {
        direction,
        command,
        body: raw[9..raw.len() - 1].to_vec(),
        raw: raw.to_vec(),
    })
}

/// Build a frame. `body` is copied as-is; the checksum is appended.
pub fn encode(direction: Direction, command: Command, body: &[u8]) -> Vec<u8> {
    let prefix = match direction {
        Direction::Outbound => OUTBOUND_PREFIX,
        Direction::Inbound => INBOUND_PREFIX,
    };
    let total = prefix.len() + 2 + 2 + body.len() + 1;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&prefix);
    out.extend_from_slice(&command.bytes());
    out.extend_from_slice(&(total as u16).to_le_bytes());
    out.extend_from_slice(body);
    let sum = checksum(&out);
    out.push(sum);
    out
}

/// An empty outbound request, such as `01 01` for state.
pub fn request(command: Command) -> Vec<u8> {
    encode(Direction::Outbound, command, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &str) -> Vec<u8> {
        bytes
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("hex"))
            .collect()
    }

    #[test]
    fn state_request_is_stable() {
        assert_eq!(
            request(Command::STATE),
            hex("08 EE 00 00 00 01 01 0A 00 02")
        );
    }

    #[test]
    fn parses_an_ack() {
        let frame = parse(&hex("09 FF 00 00 01 01 83 0A 00 97")).expect("ack");
        assert_eq!(frame.direction, Direction::Inbound);
        assert_eq!(frame.command, Command::new(0x01, 0x83));
        assert!(frame.body.is_empty());
    }

    #[test]
    fn rejects_bad_checksum() {
        let error = parse(&hex("09 FF 00 00 01 01 03 0A 00 00")).expect_err("bad checksum");
        assert!(matches!(error, ParseError::ChecksumMismatch { .. }));
    }

    #[test]
    fn rejects_length_mismatch() {
        let mut frame = request(Command::STATE);
        frame[7] = 0x0B;
        assert!(matches!(
            parse(&frame),
            Err(ParseError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn rejects_unknown_prefix() {
        let mut frame = request(Command::STATE);
        frame[0] = 0x07;
        assert!(matches!(parse(&frame), Err(ParseError::UnknownPrefix(_))));
    }

    #[test]
    fn round_trips_arbitrary_body() {
        let body = [0x00, 0x01, 0xFE, 0xFF, 0x7F];
        let encoded = encode(Direction::Inbound, Command::new(0x10, 0x9A), &body);
        let parsed = parse(&encoded).expect("parse");
        assert_eq!(parsed.direction, Direction::Inbound);
        assert_eq!(parsed.command, Command::new(0x10, 0x9A));
        assert_eq!(parsed.body, body);
        assert_eq!(parsed.raw, encoded);
    }
}
