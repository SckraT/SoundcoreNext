//! Session behavior against a scripted fake transport.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

use soundcore_models::{
    AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffDuration, BatteryScale, ButtonAction, DbLimit,
    DecibelReadingRefreshRate, DualConnectionDevice, SettingCommand, SoundMode,
    dual_connections_request, model_by_code,
};
use soundcore_protocol::{Command, Direction, ParseError, encode, request};
use soundcore_session::{
    DUAL_CONNECTIONS_MAX_FRAMES, DeviceSession, ProtocolError, SessionError, Transport,
};

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

#[derive(serde::Deserialize)]
struct EqualizerFixture {
    presets: Vec<EqualizerPresetFrame>,
}

#[derive(serde::Deserialize)]
struct EqualizerPresetFrame {
    name: String,
    frame_hex: String,
}

/// The real sanitized A3062 state captured after writing `name`.
fn a3062_equalizer_frame(name: &str) -> Vec<u8> {
    let fixture: EqualizerFixture =
        serde_json::from_str(include_str!("../../../data/a3062-eq-presets.json")).expect("fixture");
    fixture
        .presets
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| hex(&entry.frame_hex))
        .unwrap_or_else(|| panic!("the fixture records {name}"))
}

fn a3062_equalizer_state(name: &str) -> soundcore_models::EqualizerState {
    let model = model_by_code("A3062").expect("A3062");
    soundcore_models::parse_state_frame(model, &a3062_equalizer_frame(name))
        .expect("the real state")
        .equalizer
        .expect("the A3062 equalizer")
}

#[test]
fn writes_the_exact_a3062_equalizer_preset_frame_once() {
    let model = model_by_code("A3062").expect("A3062");
    let current = a3062_equalizer_state("Soundcore Signature");
    let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
    // No reads are scripted: the write must not try to read the ack.
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    session
        .set_equalizer_preset(&current, preset)
        .expect("write the preset");

    assert_eq!(
        transport.writes(),
        vec![hex(
            "08 EE 00 00 00 03 87 3F 00 03 00 00 00 50 5A 6E 78 78 78 78 78 78 00 00 \
             00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF \
             FF FF 50 5A 6E 78 78 78 78 78 78 00 00 7E"
        )],
        "exactly one measured 03 87 frame"
    );
}

#[test]
fn writes_the_exact_a3062_custom_equalizer_frame_once() {
    let model = model_by_code("A3062").expect("A3062");
    let current = a3062_equalizer_state("Soundcore Signature");
    // +3.0, -3.0, +6.0, 0, -6.0, +2.0, 0, -2.0 dB in raw dB×10 units.
    let curve = [30, -30, 60, 0, -60, 20, 0, -20];
    // No reads are scripted: the write must not try to read the ack.
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    session
        .set_custom_equalizer(&current, curve)
        .expect("write the custom curve");

    assert_eq!(
        transport.writes(),
        vec![hex(
            "08 EE 00 00 00 03 87 3F 00 FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 00 00 \
             00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF \
             FF FF 96 5A B4 78 3C 8C 78 64 78 00 00 17"
        )],
        "exactly one custom 03 87 frame"
    );
}

#[test]
fn a_deferred_equalizer_model_writes_nothing() {
    // The A3949 equalizer is deliberately deferred: its state block is known
    // but its write body is unverified, so the session refuses the write rather
    // than sending an invented `03 87` frame.
    let model = model_by_code("A3949").expect("A3949");
    let current = a3062_equalizer_state("Soundcore Signature");
    let preset = soundcore_models::a3062_equalizer_preset(5).expect("Podcast");
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    let error = session
        .set_equalizer_preset(&current, preset)
        .expect_err("the A3949 equalizer is deferred");
    assert!(
        matches!(error, SessionError::UnsupportedEqualizer),
        "unexpected error: {error}"
    );
    assert_eq!(
        error.to_string(),
        "equalizer writes are not supported for this model"
    );

    let error = session
        .set_custom_equalizer(&current, [0; 8])
        .expect_err("the A3949 custom equalizer is deferred");
    assert!(
        matches!(error, SessionError::UnsupportedEqualizer),
        "unexpected error: {error}"
    );
    assert!(transport.writes().is_empty(), "nothing may be written");
}

#[test]
fn skips_the_equalizer_ack_and_a_notification_before_the_next_state() {
    let model = model_by_code("A3062").expect("A3062");
    // The device answers a preset write with an empty inbound `03 87` frame and
    // may push a battery notification before the `01 01` answer.
    let ack = encode(Direction::Inbound, Command::EQUALIZER, &[]);
    assert_eq!(ack, hex("09 FF 00 00 01 03 87 0A 00 9D"));
    let notification = encode(Direction::Inbound, Command::BATTERY, &[2]);
    let state_frame = a3062_equalizer_frame("Bass Reducer");
    let mut scripted = ack;
    scripted.extend_from_slice(&notification);
    scripted.extend_from_slice(&state_frame);
    let transport = FakeTransport::chunks([scripted]);
    let mut session = DeviceSession::new(model, transport.clone());

    let current = a3062_equalizer_state("Soundcore Signature");
    let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
    session
        .set_equalizer_preset(&current, preset)
        .expect("write the preset");
    let state = session.poll_state().expect("state after the ack");
    let equalizer = state.equalizer.expect("the equalizer");
    assert_eq!(equalizer.preset_id, 3);
    assert_eq!(equalizer.bands, preset.bands);

    assert_eq!(
        transport.writes().len(),
        2,
        "one preset frame, then one state request"
    );
    assert_eq!(
        transport.writes()[1],
        request(Command::STATE),
        "the poll sends the state request after the write"
    );
}

#[test]
fn reports_a_failed_equalizer_write_as_an_io_error() {
    let model = model_by_code("A3062").expect("A3062");
    let current = a3062_equalizer_state("Soundcore Signature");
    let preset = soundcore_models::a3062_equalizer_preset(5).expect("Podcast");
    let transport = FakeTransport::failing_writes(io::ErrorKind::BrokenPipe);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .set_equalizer_preset(&current, preset)
        .expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::BrokenPipe),
        "unexpected error: {error}"
    );
}

/// The real sanitized A3062 `01 01` frame captured on connect.
fn a3062_state_frame() -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct Fixture {
        frame_hex: String,
    }
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3062-state.json")).expect("fixture");
    hex(&fixture.frame_hex)
}

#[test]
fn writes_the_exact_a3062_setting_frame_once() {
    let model = model_by_code("A3062").expect("A3062");
    let cases: &[(SettingCommand, &str)] = &[
        (
            SettingCommand::ButtonDoublePress(Some(ButtonAction::BassUp)),
            "08 EE 00 00 00 04 81 0D 00 00 00 07 8F",
        ),
        (
            SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: false,
            }),
            "08 EE 00 00 00 06 82 0B 00 03 8C",
        ),
        (
            SettingCommand::LowBatteryPrompt(true),
            "08 EE 00 00 00 10 82 0B 00 01 94",
        ),
        (
            SettingCommand::DolbyAudio(true),
            "08 EE 00 00 00 02 86 0B 00 01 8A",
        ),
        (
            SettingCommand::Ldac(true),
            "08 EE 00 00 00 01 FF 0B 00 01 02",
        ),
        (
            SettingCommand::DualConnections(true),
            "08 EE 00 00 00 0B 84 0B 00 01 91",
        ),
        (
            SettingCommand::AutoPowerOff(AutoPowerOff {
                enabled: true,
                duration: AutoPowerOffDuration::SixtyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 01 8B",
        ),
        (
            SettingCommand::LimitHighVolume {
                enabled: false,
                db_limit: DbLimit::new(90).expect("90 dB is a measured step"),
            },
            "08 EE 00 00 00 20 82 0C 00 00 5A FE",
        ),
        (
            SettingCommand::LimitHighVolumeRefreshRate(DecibelReadingRefreshRate::RealTime),
            "08 EE 00 00 00 20 81 0B 00 00 A2",
        ),
        (
            SettingCommand::SideTone(true),
            "08 EE 00 00 00 01 84 0B 00 01 87",
        ),
        (
            SettingCommand::VoicePrompt(true),
            "08 EE 00 00 00 01 AE 0B 00 01 B1",
        ),
    ];

    for (setting, expected) in cases {
        // No reads are scripted: the write must not try to read the ack.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model, transport.clone());

        session.set_setting(*setting).expect("write the setting");
        assert_eq!(transport.writes(), vec![hex(expected)], "{setting:?}");
    }
}

#[test]
fn a_model_without_a_measured_settings_block_writes_nothing() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    let error = session
        .set_setting(SettingCommand::Ldac(true))
        .expect_err("the P20i has no measured settings");
    assert!(
        matches!(error, SessionError::UnsupportedSetting),
        "unexpected error: {error}"
    );
    assert_eq!(
        error.to_string(),
        "settings are not supported for this model"
    );
    assert!(transport.writes().is_empty(), "nothing may be written");
}

#[test]
fn skips_the_setting_ack_and_a_notification_before_the_next_state() {
    let model = model_by_code("A3062").expect("A3062");
    // The device answers a setting write with an empty inbound frame of the
    // same command and may push a battery notification before the `01 01`
    // answer.
    let ack = encode(Direction::Inbound, Command::new(0x10, 0x82), &[]);
    let notification = encode(Direction::Inbound, Command::BATTERY, &[2]);
    let mut scripted = ack;
    scripted.extend_from_slice(&notification);
    scripted.extend_from_slice(&a3062_state_frame());
    let transport = FakeTransport::chunks([scripted]);
    let mut session = DeviceSession::new(model, transport.clone());

    session
        .set_setting(SettingCommand::LowBatteryPrompt(true))
        .expect("write the setting");
    let state = session.poll_state().expect("state after the ack");
    let settings = state.settings.expect("the A3062 settings");
    assert!(settings.low_battery_prompt);
    assert!(!settings.dolby_audio);
    assert_eq!(
        transport.writes(),
        vec![
            encode(Direction::Outbound, Command::new(0x10, 0x82), &[1],),
            request(Command::STATE),
        ],
        "one setting frame, then one state request"
    );
}

#[test]
fn reports_a_failed_setting_write_as_an_io_error() {
    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::failing_writes(io::ErrorKind::ConnectionReset);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .set_setting(SettingCommand::VoicePrompt(true))
        .expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::ConnectionReset),
        "unexpected error: {error}"
    );
}

#[derive(serde::Deserialize)]
struct DualConnectionsFixture {
    frames: Vec<DualConnectionsFrameFixture>,
    expected: DualConnectionsExpected,
}

#[derive(serde::Deserialize)]
struct DualConnectionsFrameFixture {
    frame_hex: String,
}

#[derive(serde::Deserialize)]
struct DualConnectionsExpected {
    devices: Vec<DualConnectionExpected>,
}

#[derive(serde::Deserialize)]
struct DualConnectionExpected {
    name: String,
    mac: String,
    connected: bool,
}

/// The committed real A3062 `0B 01` reply (sanitized names and addresses).
fn a3062_dual_connections_fixture() -> DualConnectionsFixture {
    serde_json::from_str(include_str!("../../../data/a3062-dual-connections.json"))
        .expect("fixture")
}

/// Every frame of the real reply as raw bytes.
fn fixture_frames(fixture: &DualConnectionsFixture) -> Vec<Vec<u8>> {
    fixture
        .frames
        .iter()
        .map(|entry| hex(&entry.frame_hex))
        .collect()
}

fn fixture_expected_devices(fixture: &DualConnectionsFixture) -> Vec<DualConnectionDevice> {
    fixture
        .expected
        .devices
        .iter()
        .map(|device| DualConnectionDevice {
            name: device.name.clone(),
            mac: colon_mac(&device.mac),
            connected: device.connected,
        })
        .collect()
}

fn colon_mac(text: &str) -> [u8; 6] {
    let bytes: Vec<u8> = text
        .split(':')
        .map(|octet| u8::from_str_radix(octet, 16).expect("hex octet"))
        .collect();
    bytes.try_into().expect("six address bytes")
}

#[test]
fn reads_the_real_a3062_dual_connections_list() {
    let fixture = a3062_dual_connections_fixture();
    let mut chunk = Vec::new();
    for frame in fixture_frames(&fixture) {
        chunk.extend_from_slice(&frame);
    }
    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport.clone());

    let devices = session
        .read_dual_connections_devices()
        .expect("the real list");

    assert_eq!(devices, fixture_expected_devices(&fixture));
    assert_eq!(
        transport.writes(),
        vec![dual_connections_request()],
        "one empty 0B 01 frame"
    );
}

#[test]
fn assembles_the_dual_connections_reply_split_across_reads() {
    let fixture = a3062_dual_connections_fixture();
    let mut all = Vec::new();
    let mut cuts = vec![1, 4, 5, 9];
    for frame in fixture_frames(&fixture) {
        all.extend_from_slice(&frame);
        cuts.push(all.len() - 1);
        cuts.push(all.len());
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.retain(|cut| *cut > 0 && *cut < all.len());

    let mut chunks = Vec::new();
    let mut previous = 0;
    for cut in cuts {
        chunks.push(all[previous..cut].to_vec());
        previous = cut;
    }
    chunks.push(all[previous..].to_vec());

    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::chunks(chunks);
    let mut session = DeviceSession::new(model, transport);

    let devices = session
        .read_dual_connections_devices()
        .expect("the split real list");

    assert_eq!(devices, fixture_expected_devices(&fixture));
}

#[test]
fn skips_other_frames_before_the_dual_connections_reply() {
    let fixture = a3062_dual_connections_fixture();
    // An empty same-command acknowledgement and a battery notification may
    // arrive before the list frames.
    let ack = encode(Direction::Inbound, Command::new(0x0B, 0x84), &[]);
    let notification = encode(Direction::Inbound, Command::BATTERY, &[5]);
    let mut chunk = ack;
    chunk.extend_from_slice(&notification);
    for frame in fixture_frames(&fixture) {
        chunk.extend_from_slice(&frame);
    }

    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport);

    let devices = session
        .read_dual_connections_devices()
        .expect("the list after unrelated frames");

    assert_eq!(devices, fixture_expected_devices(&fixture));
}

#[test]
fn a_dual_connections_read_times_out_on_silence() {
    for kind in [io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock] {
        let model = model_by_code("A3062").expect("A3062");
        let transport =
            FakeTransport::scripted([Err(io::Error::new(kind, "read deadline expired"))]);
        let mut session = DeviceSession::new(model, transport.clone());

        let error = session
            .read_dual_connections_devices()
            .expect_err("silence");
        assert!(
            matches!(error, SessionError::Timeout),
            "unexpected error: {error}"
        );
        assert_eq!(transport.writes(), vec![dual_connections_request()]);
    }
}

#[test]
fn a_dual_connections_reply_that_never_ends_times_out() {
    let model = model_by_code("A3062").expect("A3062");
    // The only frame arrives, says two frames are coming, and the next read
    // expires: the partial list must not be returned.
    let first = encode(Direction::Inbound, Command::new(0x0B, 0x01), &[2, 1]);
    let transport = FakeTransport::scripted([
        Ok(first),
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "read deadline expired",
        )),
    ]);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .read_dual_connections_devices()
        .expect_err("the reply never ends");
    assert!(
        matches!(error, SessionError::Timeout),
        "unexpected error: {error}"
    );
}

#[test]
fn the_dual_connections_frame_budget_stops_an_unterminated_reply() {
    let model = model_by_code("A3062").expect("A3062");
    // One more frame than the budget, and none of them claims to be the last.
    let mut chunk = Vec::new();
    for current in 1..=u8::try_from(DUAL_CONNECTIONS_MAX_FRAMES + 1).expect("budget") {
        chunk.extend_from_slice(&encode(
            Direction::Inbound,
            Command::new(0x0B, 0x01),
            &[0xFF, current],
        ));
    }
    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .read_dual_connections_devices()
        .expect_err("the frame budget");
    assert!(
        matches!(error, SessionError::Timeout),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_a_malformed_dual_connections_reply_as_a_protocol_error() {
    use soundcore_models::DualConnectionsError;

    let model = model_by_code("A3062").expect("A3062");
    let bad_count = encode(Direction::Inbound, Command::new(0x0B, 0x01), &[0, 1]);
    let transport = FakeTransport::chunks([bad_count]);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .read_dual_connections_devices()
        .expect_err("bad frame count");
    assert!(
        matches!(
            error,
            SessionError::Protocol(ProtocolError::DualConnections(
                DualConnectionsError::BadFrameCount {
                    total: 0,
                    current: 1
                }
            ))
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn writes_the_exact_dual_connections_frames() {
    let model = model_by_code("A3062").expect("A3062");
    // The address bytes are the neutral ones of the committed fixture and must
    // travel unchanged, in reply order.
    let address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let cases: &[(&str, &str)] = &[
        ("connect", "08 EE 00 00 00 0B 82 10 00 02 00 00 00 00 01 96"),
        (
            "disconnect",
            "08 EE 00 00 00 0B 81 10 00 02 00 00 00 00 01 95",
        ),
        ("forget", "08 EE 00 00 00 0B 83 10 00 02 00 00 00 00 01 97"),
    ];
    for (kind, expected) in cases {
        // No reads are scripted: every dual-connections write is fire-and-forget.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model, transport.clone());
        match *kind {
            "connect" => session.connect_dual_connection(address),
            "disconnect" => session.disconnect_dual_connection(address),
            "forget" => session.forget_dual_connection(address),
            other => panic!("unknown dual-connections write {other}"),
        }
        .expect("write the dual-connections frame");
        assert_eq!(transport.writes(), vec![hex(expected)], "{kind}");
    }
}

#[test]
fn a_model_without_dual_connections_reads_and_writes_nothing() {
    let model = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model, transport.clone());

    let error = session
        .read_dual_connections_devices()
        .expect_err("the P20i has no measured dual connections");
    assert!(
        matches!(error, SessionError::UnsupportedDualConnections),
        "unexpected error: {error}"
    );
    assert_eq!(
        error.to_string(),
        "dual connections are not supported for this model"
    );

    let address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    for write in [
        session.connect_dual_connection(address),
        session.disconnect_dual_connection(address),
        session.forget_dual_connection(address),
    ] {
        let error = write.expect_err("the P20i has no dual-connections writes");
        assert!(
            matches!(error, SessionError::UnsupportedDualConnections),
            "unexpected error: {error}"
        );
    }
    assert!(transport.writes().is_empty(), "nothing may be written");
}

#[test]
fn reports_a_failed_dual_connections_write_as_an_io_error() {
    let model = model_by_code("A3062").expect("A3062");
    let transport = FakeTransport::failing_writes(io::ErrorKind::BrokenPipe);
    let mut session = DeviceSession::new(model, transport);

    let error = session
        .connect_dual_connection([0x02, 0x00, 0x00, 0x00, 0x00, 0x01])
        .expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::BrokenPipe),
        "unexpected error: {error}"
    );
}

#[test]
fn probe_frame_writes_one_read_only_request_and_skips_other_frames() {
    let model = model_by_code("A3062").expect("A3062");
    let ack = encode(Direction::Inbound, Command::STATE, &[]);
    let reply = encode(Direction::Inbound, Command::BATTERY, &[3]);
    let transport = FakeTransport::scripted([Ok([ack, reply.clone()].concat())]);
    let mut session = DeviceSession::new(model, transport.clone());

    let raw = session
        .probe_frame(Command::BATTERY)
        .expect("the battery reply");
    assert_eq!(raw, reply);
    assert_eq!(transport.writes(), vec![request(Command::BATTERY)]);
}

#[test]
fn probe_frame_passes_a_silent_transport_timeout_through() {
    let model = model_by_code("A3062").expect("A3062");
    let transport =
        FakeTransport::scripted([Err(io::Error::new(io::ErrorKind::TimedOut, "no reply"))]);
    let mut session = DeviceSession::new(model, transport);

    let error = session.probe_frame(Command::CHARGING).expect_err("timeout");
    assert!(
        matches!(error, SessionError::Timeout),
        "unexpected: {error}"
    );
}
