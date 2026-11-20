//! Session behavior against a scripted fake transport.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

use soundcore_models::BatteryScale;
use soundcore_models::model_by_code;
use soundcore_protocol::{Command, Direction, ParseError, encode, request};
use soundcore_session::{DeviceSession, ProtocolError, SessionError, Transport};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

#[derive(serde::Deserialize)]
struct Fixture {
    frame_hex: String,
}

fn a3949_frame() -> Vec<u8> {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    hex(&fixture.frame_hex)
}

/// Scripted transport: `reads` are handed out one `read` call at a time.
#[derive(Clone, Default)]
struct FakeTransport {
    state: Rc<RefCell<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    reads: VecDeque<io::Result<Vec<u8>>>,
    writes: Vec<Vec<u8>>,
    write_error: Option<io::ErrorKind>,
}

impl FakeTransport {
    fn scripted(results: impl IntoIterator<Item = io::Result<Vec<u8>>>) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeState {
                reads: results.into_iter().collect(),
                writes: Vec::new(),
                write_error: None,
            })),
        }
    }

    fn chunks(chunks: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self::scripted(chunks.into_iter().map(Ok))
    }

    fn failing_writes(kind: io::ErrorKind) -> Self {
        let transport = Self::default();
        transport.state.borrow_mut().write_error = Some(kind);
        transport
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.state.borrow().writes.clone()
    }
}

impl Transport for FakeTransport {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        if let Some(kind) = state.write_error {
            return Err(io::Error::new(kind, "scripted write failure"));
        }
        state.writes.push(bytes.to_vec());
        Ok(())
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut state = self.state.borrow_mut();
        match state.reads.pop_front() {
            Some(Ok(bytes)) => {
                assert!(
                    bytes.len() <= buffer.len(),
                    "scripted chunk is bigger than the read buffer"
                );
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Some(Err(error)) => Err(error),
            None => panic!("transport read past the scripted data"),
        }
    }
}

#[test]
fn assembles_a_frame_split_across_reads() {
    let model = model_by_code("A3949").expect("A3949");
    let frame = a3949_frame();
    // Split inside the prefix, inside the header and inside the body.
    let cuts = [1, 4, 5, 9, 33, frame.len() - 1];
    let mut chunks = Vec::new();
    let mut previous = 0;
    for cut in cuts {
        chunks.push(frame[previous..cut].to_vec());
        previous = cut;
    }
    chunks.push(frame[previous..].to_vec());

    let transport = FakeTransport::chunks(chunks);
    let mut session = DeviceSession::new(model, transport);

    let state = session.poll_state().expect("state");
    assert_eq!((state.battery.left, state.battery.right), (5, 4));
}

#[test]
fn ignores_leading_garbage() {
    let model = model_by_code("A3949").expect("A3949");
    let frame = a3949_frame();
    // Noise, an outbound-looking prefix and a partial inbound prefix.
    let garbage = hex("00 13 7F 08 EE 00 00 00 09 FF");
    let mut chunk = garbage;
    chunk.extend_from_slice(&frame);

    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport);

    let state = session.poll_state().expect("state");
    assert_eq!(
        state.firmware.left.as_deref(),
        Some("14.43"),
        "garbage must not shift the frame"
    );
}

#[test]
fn returns_one_frame_and_keeps_the_next_for_a_later_poll() {
    let model = model_by_code("A3949").expect("A3949");
    let frame = a3949_frame();
    let mut chunk = frame.clone();
    chunk.extend_from_slice(&frame);

    // A single scripted read: the second `poll_state` must reuse the buffer.
    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport.clone());

    assert!(session.poll_state().is_ok());
    assert!(session.poll_state().is_ok());
    assert_eq!(transport.writes().len(), 2, "each poll sends one request");
}

#[test]
fn skips_frames_that_are_not_state_responses() {
    let model = model_by_code("A3949").expect("A3949");
    // Empty inbound ACK for a write, then the state response.
    let ack = hex("09 FF 00 00 01 01 83 0A 00 97");
    let mut chunk = ack;
    chunk.extend_from_slice(&a3949_frame());

    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport);

    let state = session.poll_state().expect("state after the ack");
    assert!(state.tws_connected);
}

#[test]
fn resynchronises_after_a_prefix_with_an_impossible_length() {
    let model = model_by_code("A3949").expect("A3949");
    // An inbound-looking prefix followed by a declared length below the
    // minimum frame length can only be noise, not a frame.
    let mut chunk = hex("09 FF 00 00 01 00 00 03 00");
    chunk.extend_from_slice(&a3949_frame());

    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport);

    let state = session.poll_state().expect("state after resynchronising");
    assert_eq!((state.battery.left, state.battery.right), (5, 4));
}

#[test]
fn polls_the_real_a3949_state() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::chunks([a3949_frame()]);
    let mut session = DeviceSession::new(model, transport.clone());

    let state = session.poll_state().expect("state");

    assert_eq!(state.battery.left, 5);
    assert_eq!(state.battery.right, 4);
    assert_eq!(state.battery.scale, BatteryScale::Five);
    assert!(!state.battery.left_charging);
    assert!(!state.battery.right_charging);
    assert_eq!(state.firmware.left.as_deref(), Some("14.43"));
    assert_eq!(state.firmware.right.as_deref(), Some("14.43"));
    assert_eq!(state.serial, None);
    assert!(state.tws_connected);
    assert_eq!(transport.writes(), vec![request(Command::STATE)]);
}

#[test]
fn reports_a_bad_checksum_as_a_protocol_error() {
    let model = model_by_code("A3949").expect("A3949");
    let mut frame = a3949_frame();
    let checksum = frame.len() - 1;
    frame[checksum] ^= 0xFF;

    let transport = FakeTransport::chunks([frame]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("bad checksum");
    assert!(
        matches!(
            error,
            SessionError::Protocol(ProtocolError::Frame(ParseError::ChecksumMismatch { .. }))
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_a_bad_state_body_as_a_protocol_error() {
    let model = model_by_code("A3949").expect("A3949");
    let frame = encode(Direction::Inbound, Command::STATE, &[0, 1, 5]);

    let transport = FakeTransport::chunks([frame]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("short body");
    assert!(
        matches!(
            error,
            SessionError::Protocol(ProtocolError::State(
                soundcore_models::StateError::ShortBody { .. }
            ))
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn maps_an_expired_read_deadline_to_timeout() {
    for kind in [io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock] {
        let model = model_by_code("A3949").expect("A3949");
        let transport =
            FakeTransport::scripted([Err(io::Error::new(kind, "read deadline expired"))]);
        let mut session = DeviceSession::new(model, transport);

        let error = session.poll_state().expect_err("deadline");
        assert!(
            matches!(error, SessionError::Timeout),
            "{kind:?} must map to Timeout, got {error}"
        );
    }
}

#[test]
fn reports_unsupported_layout_for_a_single_battery_model() {
    let model = model_by_code("A3035").expect("A3035");
    let transport = FakeTransport::chunks([a3949_frame()]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("single battery layout");
    assert!(
        matches!(error, SessionError::UnsupportedLayout),
        "unexpected error: {error}"
    );
}

#[test]
fn passes_transport_io_failures_through() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::scripted([Err(io::Error::new(
        io::ErrorKind::ConnectionReset,
        "connection reset by peer",
    ))]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("reset");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::ConnectionReset),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_end_of_stream_as_an_io_error() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::scripted([Ok(Vec::new())]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("eof");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::UnexpectedEof),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_a_failed_request_write_as_an_io_error() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::failing_writes(io::ErrorKind::BrokenPipe);
    let mut session = DeviceSession::new(model, transport);

    let error = session.poll_state().expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::BrokenPipe),
        "unexpected error: {error}"
    );
}
