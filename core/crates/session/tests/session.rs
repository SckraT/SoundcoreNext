//! Session behavior against a scripted fake transport.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

use soundcore_models::BatteryScale;
use soundcore_models::{SoundMode, model_by_code};
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

#[derive(serde::Deserialize)]
struct SoundModeFixture {
    modes: Vec<SoundModeFrame>,
}

#[derive(serde::Deserialize)]
struct SoundModeFrame {
    mode: String,
    frame_hex: String,
}

/// The real sanitized `01 01` frame captured right after writing `mode`.
fn a3062_mode_frame(mode: &str) -> Vec<u8> {
    let fixture: SoundModeFixture =
        serde_json::from_str(include_str!("../../../data/a3062-sound-modes.json"))
            .expect("fixture");
    fixture
        .modes
        .iter()
        .find(|entry| entry.mode == mode)
        .map(|entry| hex(&entry.frame_hex))
        .unwrap_or_else(|| panic!("the fixture records {mode}"))
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
    let battery = state.battery.as_ref().expect("A3949 is a TWS layout");
    assert_eq!((battery.left, battery.right), (5, 4));
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
    let battery = state.battery.as_ref().expect("A3949 is a TWS layout");
    assert_eq!((battery.left, battery.right), (5, 4));
}

#[test]
fn polls_the_real_a3949_state() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::chunks([a3949_frame()]);
    let mut session = DeviceSession::new(model, transport.clone());

    let state = session.poll_state().expect("state");

    let battery = state.battery.as_ref().expect("A3949 is a TWS layout");
    assert_eq!(battery.left, 5);
    assert_eq!(battery.right, 4);
    assert_eq!(battery.scale, BatteryScale::Five);
    assert!(!battery.left_charging);
    assert!(!battery.right_charging);
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

#[test]
fn writes_the_exact_measured_a3062_sound_mode_frames() {
    let model = model_by_code("A3062").expect("A3062");
    for (mode, expected_frame) in [
        (
            SoundMode::NoiseCanceling,
            "08 EE 00 00 00 06 81 10 00 00 51 01 01 00 05 E5",
        ),
        (
            SoundMode::Transparency,
            "08 EE 00 00 00 06 81 10 00 01 51 01 01 00 05 E6",
        ),
        (
            SoundMode::Normal,
            "08 EE 00 00 00 06 81 10 00 02 51 01 01 00 05 E7",
        ),
    ] {
        // No reads are scripted: the write must not try to read the ack.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model, transport.clone());

        session.set_sound_mode(mode).expect("write the mode");
        assert_eq!(transport.writes(), vec![hex(expected_frame)], "{mode:?}");
    }
}

#[test]
fn a_model_without_a_measured_sound_mode_writes_nothing() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    let error = session
        .set_sound_mode(SoundMode::NoiseCanceling)
        .expect_err("the P20i has no mode");
    assert!(
        matches!(error, SessionError::UnsupportedSoundMode),
        "unexpected error: {error}"
    );
    assert_eq!(
        error.to_string(),
        "sound modes were not measured for this model"
    );
    assert!(transport.writes().is_empty(), "nothing may be written");
}

#[test]
fn skips_the_mode_ack_and_a_notification_before_the_next_state() {
    let model = model_by_code("A3062").expect("A3062");
    // The measured ack is the empty inbound `06 81` frame; a battery
    // notification may arrive before the state answer, too.
    let ack = encode(Direction::Inbound, Command::ACK_AMBIENT_SOUND_MODE, &[]);
    assert_eq!(ack, hex("09 FF 00 00 01 06 81 0A 00 9A"));
    let notification = encode(Direction::Inbound, Command::BATTERY, &[5]);
    let state_frame = a3062_mode_frame("Transparency");
    let mut scripted = ack;
    scripted.extend_from_slice(&notification);
    scripted.extend_from_slice(&state_frame);
    let transport = FakeTransport::chunks([scripted]);
    let mut session = DeviceSession::new(model, transport.clone());

    session
        .set_sound_mode(SoundMode::Transparency)
        .expect("write the mode");
    let state = session.poll_state().expect("state after the ack");
    assert_eq!(state.sound_mode, Some(SoundMode::Transparency));
    assert_eq!(
        transport.writes(),
        vec![
            encode(
                Direction::Outbound,
                Command::ACK_AMBIENT_SOUND_MODE,
                &[1, 0x51, 0x01, 0x01, 0x00, 0x05],
            ),
            request(Command::STATE),
        ],
        "one mode frame, then one state request"
    );
}

#[test]
fn reports_a_failed_mode_write_as_an_io_error() {
    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::failing_writes(io::ErrorKind::ConnectionReset);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .set_sound_mode(SoundMode::Normal)
        .expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::ConnectionReset),
        "unexpected error: {error}"
    );
}
