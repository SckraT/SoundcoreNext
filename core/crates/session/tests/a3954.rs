//! A3954 (Liberty 4 Pro) session behavior against a scripted fake transport.
//!
//! The state and command fixtures are the real Liberty 4 Pro capture and the
//! Soundcore app's own writes, sanitized (serial bytes masked, no address,
//! checksums recalculated) and committed in `core/data`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

use soundcore_models::{
    A3954AirplaneMode, A3954AmbientSoundMode, A3954AutoPowerOff, A3954AutoPowerOffDuration,
    A3954Button, A3954ButtonAction, A3954ButtonActionByte, A3954ButtonActionValue,
    A3954CaseFeatures, A3954CaseLanguage, A3954EasyChat, A3954EasyChatWaitTime,
    A3954SettingCommand, A3954SoundMode, A3954SoundModeSlider, A3954SpatialAudio,
    A3954SpatialAudioMode, A3954SpatialAudioMusicMode, AmbientSoundModeCycle, DbLimit,
    DecibelReadingRefreshRate, VISIBLE_EQ_BANDS, a3954_equalizer_preset, model_by_code,
    parse_state_frame,
};
use soundcore_protocol::{Command, Direction, encode, request};
use soundcore_session::{DeviceSession, SessionError, Transport};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

#[derive(serde::Deserialize)]
struct StateFixture {
    frame_hex: String,
}

fn a3954_frame() -> Vec<u8> {
    let fixture: StateFixture =
        serde_json::from_str(include_str!("../../../data/a3954-state.json")).expect("fixture");
    hex(&fixture.frame_hex)
}

#[derive(serde::Deserialize)]
struct SoundModesFixture {
    writes: Vec<SoundModeWrite>,
}

#[derive(serde::Deserialize)]
struct SoundModeWrite {
    mode_byte: u8,
    position: u8,
    payload_hex: String,
    frame_hex: String,
}

fn sound_mode_writes() -> SoundModesFixture {
    serde_json::from_str(include_str!("../../../data/a3954-sound-modes.json")).expect("fixture")
}

#[derive(serde::Deserialize)]
struct EqualizerFixture {
    frames: Vec<EqualizerFrame>,
}

#[derive(serde::Deserialize)]
struct EqualizerFrame {
    id: u16,
    body_hex: String,
}

fn equalizer_fixture() -> EqualizerFixture {
    serde_json::from_str(include_str!("../../../data/a3954-eq-presets.json")).expect("fixture")
}

fn model() -> soundcore_models::DeviceModel {
    model_by_code("A3954").expect("A3954")
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
fn polls_the_real_a3954_state() {
    let transport = FakeTransport::chunks([a3954_frame()]);
    let mut session = DeviceSession::new(model(), transport.clone());

    let state = session.poll_state().expect("state");
    let battery = state.battery.as_ref().expect("A3954 is a TWS layout");
    assert_eq!((battery.left, battery.right), (96, 98));
    assert_eq!(
        state.firmware.left.as_deref(),
        Some("04.29"),
        "the shared TWS prefix parses"
    );
    assert_eq!(state.serial, None, "the fixture masks the serial");

    let a3954 = state.a3954.expect("the A3954 block");
    assert_eq!(a3954.case_firmware.as_deref(), Some("01.58"));
    assert_eq!(a3954.case_battery_level, Some(9));
    assert_eq!(a3954.case_serial, None);
    assert_eq!(
        a3954
            .sound_mode
            .expect("the measured sound-mode block")
            .slider
            .value(),
        11
    );
    assert_eq!(
        a3954
            .sound_mode
            .expect("the measured sound-mode block")
            .slider
            .mode(),
        A3954AmbientSoundMode::NoiseCanceling
    );
    assert!(a3954.ldac);
    assert!(a3954.dual_connections);
    assert_eq!(
        a3954.auto_power_off,
        A3954AutoPowerOff {
            enabled: true,
            duration: A3954AutoPowerOffDuration::ThirtyMinutes,
        }
    );
    assert_eq!(transport.writes(), vec![request(Command::STATE)]);
}

#[test]
fn writes_every_captured_a3954_sound_mode_frame_once() {
    let fixture = sound_mode_writes();
    assert_eq!(fixture.writes.len(), 11);

    for write in &fixture.writes {
        // No reads are scripted: the write must not try to read the ack.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model(), transport.clone());

        let mode = A3954AmbientSoundMode::from_value(write.mode_byte).expect("a measured mode");
        let slider = A3954SoundModeSlider::new(write.position).expect("a measured position");
        session
            .set_a3954_sound_mode(A3954SoundMode {
                mode,
                slider,
                airplane_mode: A3954AirplaneMode::ManualUpdate,
                wind_noise_suppression: true,
                wind_noise_detected: false,
            })
            .expect("write the scale");
        assert_eq!(
            transport.writes(),
            vec![hex(&write.frame_hex)],
            "position {}",
            write.position
        );
        // The captured body is the model's scale body.
        let body = hex(&write.payload_hex);
        assert_eq!(
            body,
            model()
                .a3954_sound_mode_body(A3954SoundMode {
                    mode,
                    slider,
                    airplane_mode: A3954AirplaneMode::ManualUpdate,
                    wind_noise_suppression: true,
                    wind_noise_detected: false,
                })
                .expect("the scale body")
                .to_vec()
        );
    }
}

/// The HearID block and the two channel curves of a captured app `03 87` body,
/// turned back into the 165-byte `01 01` body the device would have reported
/// before the write. It lets the session tests compare a write against the
/// app's own captured body byte for byte.
fn a3954_body_for_app_frame(app_body: &[u8]) -> Vec<u8> {
    let mut body = vec![0u8; soundcore_models::A3954_STATE_BODY_LEN];
    body[1] = 1;
    body[2] = 50;
    body[3] = 50;
    body[44..46].copy_from_slice(&app_body[0..2]);
    body[46..66].copy_from_slice(&app_body[4..24]);
    let hear_id = [
        app_body[26],
        app_body[27],
        app_body[28],
        app_body[29],
        app_body[30],
        app_body[31],
        app_body[32],
        app_body[33],
        app_body[34],
        app_body[35],
        app_body[36],
        app_body[37],
        app_body[38],
        app_body[39],
        app_body[40],
        app_body[41],
        app_body[42],
        app_body[43],
        app_body[44],
        app_body[45],
        app_body[46],
        app_body[47],
        app_body[48],
        app_body[49],
        app_body[50],
        app_body[51],
        app_body[52],
        app_body[53],
        app_body[54],
        app_body[55],
        app_body[56],
        app_body[57],
        app_body[58],
        app_body[59],
        app_body[60],
        app_body[61],
        app_body[62],
        app_body[63],
        app_body[64],
        app_body[65],
        app_body[66],
        app_body[67],
        app_body[68],
        app_body[69],
        app_body[70],
        app_body[71],
        app_body[2],
        app_body[3],
    ];
    body[67..115].copy_from_slice(&hear_id);
    // normal mode on the 1..=11 scale, 75 dB limit, first auto-off duration
    body[125] = A3954AmbientSoundMode::Normal.value();
    body[126] = A3954SoundModeSlider::NORMAL;
    body[146] = 75;
    body
}

fn app_derived_state(app_body: &[u8]) -> soundcore_models::A3954State {
    parse_state_frame(model(), &{
        let body = a3954_body_for_app_frame(app_body);
        encode(Direction::Inbound, Command::STATE, &body)
    })
    .expect("the app-derived state parses")
    .a3954
    .expect("the A3954 block")
}

#[test]
fn writes_the_exact_app_measured_a3954_equalizer_preset_frame_once() {
    let fixture = equalizer_fixture();
    let rock = fixture
        .frames
        .iter()
        .find(|frame| frame.id == 17)
        .expect("the app's Rock frame");
    let app_body = hex(&rock.body_hex);
    let current = app_derived_state(&app_body);
    let preset = a3954_equalizer_preset(17).expect("Rock");

    // No reads are scripted: the write must not try to read the ack.
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model(), transport.clone());
    session
        .set_a3954_equalizer_preset(&current.equalizer, preset)
        .expect("write the preset");

    assert_eq!(
        transport.writes(),
        vec![encode(Direction::Outbound, Command::EQUALIZER, &app_body)],
        "exactly the app's measured 114-byte 03 87 frame"
    );
}

#[test]
fn writes_the_exact_a3954_custom_equalizer_frame_once() {
    let fixture = equalizer_fixture();
    let rock = fixture
        .frames
        .iter()
        .find(|frame| frame.id == 17)
        .expect("the app's Rock frame");
    let app_body = hex(&rock.body_hex);
    let current = app_derived_state(&app_body);

    // +3.0, -3.0, +6.0, 0, -6.0, +2.0, 0, -2.0 dB in raw dB×10 units, then the
    // two invisible bands fixed at 0 dB.
    let curve = [30, -30, 60, 0, -60, 20, 0, -20];
    let mut expected = app_body.clone();
    expected[0..2].copy_from_slice(&soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_le_bytes());
    let mut band_bytes: Vec<u8> = curve.iter().map(|value| (value + 120) as u8).collect();
    band_bytes.extend_from_slice(&[120, 120]);
    expected[4..14].copy_from_slice(&band_bytes);
    expected[14..24].copy_from_slice(&band_bytes);
    for (index, byte) in band_bytes.iter().enumerate() {
        expected[72 + index * 2] = *byte;
        expected[92 + index * 2] = *byte;
    }

    // No reads are scripted: the write must not try to read the ack.
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model(), transport.clone());
    session
        .set_a3954_custom_equalizer(&current.equalizer, curve)
        .expect("write the custom curve");

    assert_eq!(
        transport.writes(),
        vec![encode(Direction::Outbound, Command::EQUALIZER, &expected)],
        "exactly one 114-byte custom 03 87 frame; only the curves and the preset id differ from the app's preset body"
    );
    let written = &transport.writes()[0];
    assert_eq!(written.len(), 124);
    assert_eq!(
        u16::from_le_bytes([written[9], written[10]]),
        soundcore_models::CUSTOM_EQUALIZER_PRESET_ID
    );
}

#[test]
fn writes_the_exact_a3954_setting_frames_once() {
    let cases: &[(A3954SettingCommand, &str)] = &[
        (
            A3954SettingCommand::Ldac(true),
            "08 EE 00 00 00 01 FF 0B 00 01 02",
        ),
        (
            A3954SettingCommand::DualConnections(true),
            "08 EE 00 00 00 0B 84 0B 00 01 91",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: true,
                duration: A3954AutoPowerOffDuration::TwentyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 01 8B",
        ),
        (
            A3954SettingCommand::LimitHighVolume {
                enabled: false,
                db_limit: DbLimit::new(90).expect("a measured dB step"),
            },
            "08 EE 00 00 00 20 82 0C 00 00 5A FE",
        ),
        (
            A3954SettingCommand::LimitHighVolumeRefreshRate(DecibelReadingRefreshRate::RealTime),
            "08 EE 00 00 00 20 81 0B 00 00 A2",
        ),
        (
            A3954SettingCommand::LowBatteryPrompt(true),
            "08 EE 00 00 00 10 82 0B 00 01 94",
        ),
        (
            A3954SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: false,
            }),
            "08 EE 00 00 00 06 82 0B 00 03 8C",
        ),
        (
            A3954SettingCommand::WearingDetection(true),
            "08 EE 00 00 00 01 81 0B 00 01 84",
        ),
        (
            A3954SettingCommand::SoundLeakCompensation(true),
            "08 EE 00 00 00 10 9A 0B 00 01 AC",
        ),
        (
            A3954SettingCommand::SpatialAudio(A3954SpatialAudio {
                enabled: true,
                music_mode: A3954SpatialAudioMusicMode::Fixed,
                mode: A3954SpatialAudioMode::Music,
            }),
            "08 EE 00 00 00 10 81 0D 00 01 01 00 96",
        ),
        (
            A3954SettingCommand::EasyChat(A3954EasyChat {
                enabled: false,
                wait_time: A3954EasyChatWaitTime::FifteenSeconds,
            }),
            "08 EE 00 00 00 10 9D 0C 00 00 02 B1",
        ),
        (
            A3954SettingCommand::CaseFeatures(A3954CaseFeatures {
                atmospheric: false,
                remote_camera: false,
                find_device: true,
                case_spatial_audio: false,
            }),
            "08 EE 00 00 00 07 87 0E 00 00 01 00 00 93",
        ),
        (
            A3954SettingCommand::CaseLanguage(A3954CaseLanguage::Chinese),
            "08 EE 00 00 00 07 8A 0B 00 02 94",
        ),
    ];

    for (setting, expected) in cases {
        // No reads are scripted: the write must not try to read the ack.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model(), transport.clone());
        session
            .set_a3954_setting(*setting)
            .expect("write the setting");
        assert_eq!(transport.writes(), vec![hex(expected)], "{setting:?}");
    }
}

#[test]
fn writes_the_exact_a3954_button_frames_once() {
    // The action bytes are the real fixture's, so each frame pins the measured
    // side and button id as well as the one-gesture body.
    let cases: &[(A3954Button, u8, &str)] = &[
        (
            A3954Button::LeftSingle,
            0x66,
            "08 EE 00 00 00 04 81 0D 00 00 02 66 F0",
        ),
        (
            A3954Button::RightSingle,
            0x66,
            "08 EE 00 00 00 04 81 0D 00 01 02 66 F1",
        ),
        (
            A3954Button::LeftDouble,
            0x32,
            "08 EE 00 00 00 04 81 0D 00 00 00 32 BA",
        ),
        (
            A3954Button::RightDouble,
            0x33,
            "08 EE 00 00 00 04 81 0D 00 01 00 33 BC",
        ),
        (
            A3954Button::LeftTriple,
            0xFF,
            "08 EE 00 00 00 04 81 0D 00 00 05 FF 8C",
        ),
        (
            A3954Button::RightTriple,
            0xFF,
            "08 EE 00 00 00 04 81 0D 00 01 05 FF 8D",
        ),
        (
            A3954Button::LeftLong,
            0x44,
            "08 EE 00 00 00 04 81 0D 00 00 01 44 CD",
        ),
        (
            A3954Button::RightLong,
            0x44,
            "08 EE 00 00 00 04 81 0D 00 01 01 44 CE",
        ),
        (
            A3954Button::LeftSlideUp,
            0x00,
            "08 EE 00 00 00 04 81 0D 00 00 06 00 8E",
        ),
        (
            A3954Button::RightSlideUp,
            0x00,
            "08 EE 00 00 00 04 81 0D 00 01 06 00 8F",
        ),
        (
            A3954Button::LeftSlideDown,
            0x11,
            "08 EE 00 00 00 04 81 0D 00 00 07 11 A0",
        ),
        (
            A3954Button::RightSlideDown,
            0x11,
            "08 EE 00 00 00 04 81 0D 00 01 07 11 A1",
        ),
        (
            A3954Button::LeftSingle,
            0x7A,
            "08 EE 00 00 00 04 81 0D 00 00 02 7A 04",
        ),
    ];

    for (button, action, expected) in cases {
        // No reads are scripted: the write must not try to read the ack.
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model(), transport.clone());
        session
            .set_a3954_button(*button, A3954ButtonActionByte::from_byte(*action))
            .expect("write the button frame");
        assert_eq!(transport.writes(), vec![hex(expected)], "{button:?}");
    }

    // The app's own captured writes: right double press, connected action 1
    // and 2 (the high nibble 1 is the disconnected-state action).
    for (action, expected) in [
        (0x11, "08 EE 00 00 00 04 81 0D 00 01 00 11 9A"),
        (0x12, "08 EE 00 00 00 04 81 0D 00 01 00 12 9B"),
    ] {
        let transport = FakeTransport::default();
        let mut session = DeviceSession::new(model(), transport.clone());
        session
            .set_a3954_button(
                A3954Button::RightDouble,
                A3954ButtonActionByte::from_byte(action),
            )
            .expect("write the button frame");
        assert_eq!(transport.writes(), vec![hex(expected)]);
    }
}

#[test]
fn writes_the_parsed_a3954_button_actions_and_case_settings_back() {
    // The read-then-write path a controller uses: decode the real state, edit
    // one gesture, write the exact frame and the parsed case flags/language.
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model(), transport.clone());

    let state = parse_state_frame(model(), &a3954_frame()).expect("the real A3954 state");
    let a3954 = state.a3954.expect("the A3954 block");
    let left_double = a3954.button_action_byte(A3954Button::LeftDouble);
    let edited = left_double.with_tws(A3954ButtonActionValue::Action(A3954ButtonAction::PlayPause));
    session
        .set_a3954_button(A3954Button::LeftDouble, edited)
        .expect("write the edited button");
    session
        .set_a3954_setting(A3954SettingCommand::CaseFeatures(a3954.case_features))
        .expect("write the parsed case features");
    session
        .set_a3954_setting(A3954SettingCommand::CaseLanguage(a3954.case_language))
        .expect("write the parsed case language");

    assert_eq!(
        transport.writes(),
        vec![
            // Left double press, button id 0, action (nonTws=3, tws=6).
            hex("08 EE 00 00 00 04 81 0D 00 00 00 36 BE"),
            // The fixture's case flags: only find device on; the `07 87` body
            // is in the app's order, so find device is the second byte.
            hex("08 EE 00 00 00 07 87 0E 00 00 01 00 00 93"),
            // The fixture's language: English.
            hex("08 EE 00 00 00 07 8A 0B 00 01 93"),
        ],
        "one frame per write and no read"
    );
}

#[test]
fn a_non_a3954_model_writes_nothing_through_the_a3954_apis() {
    let a3949 = model_by_code("A3949").expect("A3949");
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(a3949, transport.clone());

    let mut sound_mode = parse_state_frame(model(), &a3954_frame())
        .expect("the real A3954 state")
        .a3954
        .expect("the A3954 block")
        .sound_mode
        .expect("the measured sound-mode block");
    sound_mode.wind_noise_suppression = false;
    let error = session
        .set_a3954_sound_mode(sound_mode)
        .expect_err("the P20i has no A3954 scale");
    assert!(
        matches!(error, SessionError::UnsupportedSoundMode),
        "{error}"
    );

    let a3954 = parse_state_frame(model(), &a3954_frame())
        .expect("the real A3954 state")
        .a3954
        .expect("the A3954 block");
    let preset = a3954_equalizer_preset(17).expect("Rock");
    for error in [
        session
            .set_a3954_equalizer_preset(&a3954.equalizer, preset)
            .expect_err("the P20i has no A3954 equalizer"),
        session
            .set_a3954_custom_equalizer(&a3954.equalizer, [0; VISIBLE_EQ_BANDS])
            .expect_err("the P20i has no A3954 custom equalizer"),
    ] {
        assert!(
            matches!(error, SessionError::UnsupportedEqualizer),
            "{error}"
        );
    }

    let error = session
        .set_a3954_setting(A3954SettingCommand::Ldac(true))
        .expect_err("the P20i has no A3954 settings");
    assert!(matches!(error, SessionError::UnsupportedSetting), "{error}");
    let error = session
        .set_a3954_button(
            A3954Button::LeftSingle,
            A3954ButtonActionByte::from_byte(0x66),
        )
        .expect_err("the P20i has no A3954 buttons");
    assert!(
        matches!(error, SessionError::UnsupportedButtonConfiguration),
        "{error}"
    );
    assert!(transport.writes().is_empty(), "nothing may be written");
}

#[test]
fn skips_the_a3954_acks_and_a_notification_before_the_next_state() {
    // The device answers a scale write with the empty inbound `06 81` frame and
    // may push a battery notification before the `01 01` answer.
    let mode_ack = encode(Direction::Inbound, Command::ACK_AMBIENT_SOUND_MODE, &[]);
    let notification = encode(Direction::Inbound, Command::BATTERY, &[0x62]);
    let mut scripted = mode_ack;
    scripted.extend_from_slice(&notification);
    scripted.extend_from_slice(&a3954_frame());

    let transport = FakeTransport::chunks([scripted]);
    let mut session = DeviceSession::new(model(), transport.clone());

    session
        .set_a3954_sound_mode(A3954SoundMode {
            mode: A3954AmbientSoundMode::Normal,
            slider: A3954SoundModeSlider::normal(),
            airplane_mode: A3954AirplaneMode::ManualUpdate,
            wind_noise_suppression: true,
            wind_noise_detected: false,
        })
        .expect("write the scale");
    let state = session.poll_state().expect("state after the acks");
    assert_eq!(
        state
            .a3954
            .expect("the A3954 block")
            .sound_mode
            .expect("the measured sound-mode block")
            .slider
            .value(),
        11
    );
    assert_eq!(
        transport.writes(),
        vec![
            encode(
                Direction::Outbound,
                Command::ACK_AMBIENT_SOUND_MODE,
                &[2, 6, 0, 1, 0xFF],
            ),
            request(Command::STATE),
        ],
        "one scale frame, then one state request"
    );
}

#[test]
fn reports_a_failed_a3954_write_as_an_io_error() {
    for setting in [
        A3954SettingCommand::Ldac(true),
        A3954SettingCommand::SoundLeakCompensation(false),
    ] {
        let transport = FakeTransport::failing_writes(io::ErrorKind::ConnectionReset);
        let mut session = DeviceSession::new(model(), transport);
        let error = session
            .set_a3954_setting(setting)
            .expect_err("write failure");
        assert!(
            matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::ConnectionReset),
            "unexpected error: {error}"
        );
    }

    let a3954 = parse_state_frame(model(), &a3954_frame())
        .expect("the real A3954 state")
        .a3954
        .expect("the A3954 block");
    let transport = FakeTransport::failing_writes(io::ErrorKind::BrokenPipe);
    let mut session = DeviceSession::new(model(), transport);
    let error = session
        .set_a3954_sound_mode(a3954.sound_mode.expect("the measured sound-mode block"))
        .expect_err("write failure");
    assert!(
        matches!(error, SessionError::Io(ref io_error) if io_error.kind() == io::ErrorKind::BrokenPipe),
        "unexpected error: {error}"
    );
}

#[test]
fn the_a3954_uses_the_existing_dual_connections_helpers() {
    // The A3954 reuses the measured `0B 01` list and the `0B 82`/`0B 81`/`0B 83`
    // writes through the same helpers as the A3062; the reply frames are the
    // committed real A3062 ones, which are protocol-generic.
    #[derive(serde::Deserialize)]
    struct DualConnectionsFixture {
        frames: Vec<DualConnectionsFrameFixture>,
    }
    #[derive(serde::Deserialize)]
    struct DualConnectionsFrameFixture {
        frame_hex: String,
    }

    let fixture: DualConnectionsFixture =
        serde_json::from_str(include_str!("../../../data/a3062-dual-connections.json"))
            .expect("fixture");
    let mut chunk = Vec::new();
    for frame in &fixture.frames {
        chunk.extend_from_slice(&hex(&frame.frame_hex));
    }
    let transport = FakeTransport::chunks([chunk]);
    let mut session = DeviceSession::new(model(), transport.clone());

    let devices = session.read_dual_connections_devices().expect("the list");
    assert!(!devices.is_empty());
    assert_eq!(
        transport.writes(),
        vec![soundcore_models::dual_connections_request()]
    );

    // The writes are fire-and-forget and reuse the same frame builders.
    let address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let transport = FakeTransport::default();
    let mut session = DeviceSession::new(model(), transport.clone());
    session
        .connect_dual_connection(address)
        .expect("connect frame");
    session
        .disconnect_dual_connection(address)
        .expect("disconnect frame");
    session
        .forget_dual_connection(address)
        .expect("forget frame");
    assert_eq!(
        transport.writes(),
        vec![
            soundcore_models::dual_connections_connect_frame(address),
            soundcore_models::dual_connections_disconnect_frame(address),
            soundcore_models::dual_connections_forget_frame(address),
        ]
    );
}
