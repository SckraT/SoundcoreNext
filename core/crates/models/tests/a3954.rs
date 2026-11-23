//! A3954 (Liberty 4 Pro) state, sound-mode scale, equalizer and settings tests.
//!
//! The state fixture is a real `01 01` answer of a Liberty 4 Pro captured from
//! the app's own btsnoop record; the earbud serial is masked to ASCII zeroes,
//! the case serial is zeroed, the checksum is recalculated and no Bluetooth
//! address is present. The sound-mode and equalizer command vectors come from
//! the app's own writes in the same capture and from
//! `analysis/capabilities/a3954-eq-presets.json`.

use soundcore_models::{
    A3954_EQ_BANDS, A3954_EQUALIZER_COMMAND_BODY_LEN, A3954_EQUALIZER_PRESETS,
    A3954_SOUND_MODE_COMMAND_BODY_LEN, A3954_STATE_BODY_LEN, A3954AirplaneMode,
    A3954AmbientSoundMode, A3954AutoPowerOff, A3954AutoPowerOffDuration, A3954CaseFeatures,
    A3954CaseLanguage, A3954EasyChat, A3954EasyChatWaitTime, A3954EqualizerState,
    A3954SettingCommand, A3954SoundMode, A3954SoundModeSlider, A3954SpatialAudio,
    A3954SpatialAudioMode, A3954SpatialAudioMusicMode, A3954State, AmbientSoundModeCycle,
    CUSTOM_EQUALIZER_PRESET_ID, DbLimit, DecibelReadingRefreshRate, EqualizerPreset,
    LimitHighVolume, SoundMode, StateError, VISIBLE_EQ_BANDS, a3954_equalizer_preset,
    model_by_code, parse_state_body, parse_state_frame,
};
use soundcore_protocol::{Command, Direction, encode, request};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

fn frame_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn model() -> soundcore_models::DeviceModel {
    model_by_code("A3954").expect("A3954")
}

#[derive(serde::Deserialize)]
struct StateFixture {
    frame_hex: String,
    body_length: usize,
    expected: ExpectedState,
}

#[derive(serde::Deserialize)]
struct ExpectedState {
    host_device: String,
    tws_connected: bool,
    left: u8,
    right: u8,
    scale: u8,
    left_charging: bool,
    right_charging: bool,
    firmware_left: String,
    firmware_right: String,
    case_firmware: String,
    case_battery_level: u8,
    equalizer_preset_id: u16,
    equalizer_left_bands: Vec<i16>,
    equalizer_right_bands: Option<Vec<i16>>,
    ambient_sound_mode_cycle: ExpectedCycle,
    sound_mode: String,
    slider_position: u8,
    airplane_mode: String,
    wind_noise_suppression: bool,
    wind_noise_detected: bool,
    case_features: ExpectedCaseFeatures,
    air_pressure: u8,
    low_battery_prompt: bool,
    ldac: bool,
    dual_connections: bool,
    auto_power_off: ExpectedAutoPowerOff,
    limit_high_volume: ExpectedLimitHighVolume,
    spatial_audio: ExpectedSpatialAudio,
    easy_chat: ExpectedEasyChat,
    sound_leak_compensation: bool,
    case_language: String,
    wearing_detection: bool,
    main_buttons_hex: String,
    slide_buttons_hex: String,
}

#[derive(serde::Deserialize)]
struct ExpectedCycle {
    noise_canceling: bool,
    transparency: bool,
    normal: bool,
}

#[derive(serde::Deserialize)]
struct ExpectedCaseFeatures {
    atmospheric: bool,
    remote_camera: bool,
    find_device: bool,
    case_spatial_audio: bool,
}

#[derive(serde::Deserialize)]
struct ExpectedAutoPowerOff {
    enabled: bool,
    minutes: u16,
}

#[derive(serde::Deserialize)]
struct ExpectedLimitHighVolume {
    enabled: bool,
    db_limit: u8,
    refresh_rate: String,
}

#[derive(serde::Deserialize)]
struct ExpectedSpatialAudio {
    enabled: bool,
    music_mode: String,
    mode: String,
}

#[derive(serde::Deserialize)]
struct ExpectedEasyChat {
    enabled: bool,
    wait_time_seconds: u16,
}

fn fixture() -> StateFixture {
    serde_json::from_str(include_str!("../../../data/a3954-state.json")).expect("fixture")
}

fn a3954_frame() -> Vec<u8> {
    hex(&fixture().frame_hex)
}

/// The captured frame's body.
fn a3954_captured_body() -> Vec<u8> {
    let frame = a3954_frame();
    frame[9..frame.len() - 1].to_vec()
}

/// A minimal synthetic A3954 body that parses: the enum values are the ones
/// the parser validates, and every other byte stays zero, which the parser
/// accepts as flags off, zero bands and default modes.
fn synthetic_body() -> Vec<u8> {
    let mut body = vec![0u8; A3954_STATE_BODY_LEN];
    body[0] = 0;
    body[1] = 1;
    body[2] = 50;
    body[3] = 50;
    // normal mode on the 1..=11 scale, manual airplane mode
    body[125] = A3954AmbientSoundMode::Normal.value();
    body[126] = A3954SoundModeSlider::NORMAL;
    body[127] = A3954AirplaneMode::ManualUpdate.value();
    // a valid auto power off index and dB limit
    body[144] = 0;
    body[146] = 75;
    body
}

fn parsed_state(body: &[u8]) -> A3954State {
    parse_state_body(model(), body)
        .expect("the A3954 body parses")
        .a3954
        .expect("the A3954 state")
}

fn parsed_equalizer(body: &[u8]) -> A3954EqualizerState {
    parsed_state(body).equalizer
}

fn expected_mode(name: &str) -> A3954AmbientSoundMode {
    match name {
        "NoiseCanceling" => A3954AmbientSoundMode::NoiseCanceling,
        "Transparency" => A3954AmbientSoundMode::Transparency,
        "Normal" => A3954AmbientSoundMode::Normal,
        "AirplaneMode" => A3954AmbientSoundMode::AirplaneMode,
        other => panic!("the fixture names an unknown mode {other:?}"),
    }
}

fn expected_airplane_mode(name: &str) -> A3954AirplaneMode {
    match name {
        "ManualUpdate" => A3954AirplaneMode::ManualUpdate,
        "AutomaticUpdate" => A3954AirplaneMode::AutomaticUpdate,
        other => panic!("the fixture names an unknown airplane mode {other:?}"),
    }
}

fn expected_refresh_rate(name: &str) -> DecibelReadingRefreshRate {
    match name {
        "RealTime" => DecibelReadingRefreshRate::RealTime,
        "TenSeconds" => DecibelReadingRefreshRate::TenSeconds,
        "OneMinute" => DecibelReadingRefreshRate::OneMinute,
        other => panic!("the fixture names an unknown refresh rate {other:?}"),
    }
}

fn expected_spatial_music_mode(name: &str) -> A3954SpatialAudioMusicMode {
    match name {
        "Fixed" => A3954SpatialAudioMusicMode::Fixed,
        "HeadTracking" => A3954SpatialAudioMusicMode::HeadTracking,
        other => panic!("the fixture names an unknown music mode {other:?}"),
    }
}

fn expected_spatial_mode(name: &str) -> A3954SpatialAudioMode {
    match name {
        "Music" => A3954SpatialAudioMode::Music,
        "Podcast" => A3954SpatialAudioMode::Podcast,
        "Movie" => A3954SpatialAudioMode::Movie,
        "Gaming" => A3954SpatialAudioMode::Gaming,
        other => panic!("the fixture names an unknown spatial mode {other:?}"),
    }
}

fn expected_case_language(name: &str) -> A3954CaseLanguage {
    match name {
        "Japanese" => A3954CaseLanguage::Japanese,
        "English" => A3954CaseLanguage::English,
        "Chinese" => A3954CaseLanguage::Chinese,
        other => panic!("the fixture names an unknown case language {other:?}"),
    }
}

#[test]
fn parses_the_real_a3954_state_at_the_measured_offsets() {
    let fixture = fixture();
    let frame = hex(&fixture.frame_hex);
    let body = &frame[9..frame.len() - 1];
    assert_eq!(body.len(), fixture.body_length);
    assert_eq!(body.len(), A3954_STATE_BODY_LEN);

    // The measured block starts at body 32. Assert the raw bytes at every
    // measured offset so a parser change that moves an offset fails here first.
    assert_eq!(&body[32..37], b"01.58");
    assert_eq!(body[37], 9, "raw case battery level");
    assert_eq!(&body[38..44], &[0; 6], "the fixture masks the case serial");
    assert_eq!(&body[44..46], &[0xFE, 0xFE], "custom equalizer preset id");
    assert_eq!(
        &body[46..56],
        &[0x8A, 0x80, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78],
        "left channel band bytes"
    );
    assert_eq!(&body[56..66], &[0xFF; 10], "absent right channel");
    assert_eq!(&body[67..115], &[0; 48], "the HearID block");
    assert_eq!(
        &body[116..124],
        &[0x66, 0x66, 0x32, 0x33, 0xFF, 0xFF, 0x44, 0x44],
        "main button bytes"
    );
    assert_eq!(body[124], 0x33, "ambient cycle byte");
    assert_eq!(&body[125..129], &[0, 11, 0, 1], "sound-mode block");
    assert_eq!(&body[132..136], &[0, 0, 1, 0], "case features");
    assert_eq!(body[136], 98, "air pressure");
    assert_eq!(body[140], 1, "low battery prompt");
    assert_eq!(body[141], 1, "LDAC");
    assert_eq!(body[142], 1, "dual connections");
    assert_eq!(&body[143..145], &[1, 2], "auto power off");
    assert_eq!(&body[145..148], &[0, 90, 0], "limit high volume");
    assert_eq!(&body[148..151], &[0, 0, 0], "spatial audio");
    assert_eq!(body[151], 0, "easy chat enabled");
    assert_eq!(body[153], 0, "sound leak compensation");
    assert_eq!(body[157], 1, "case language");
    assert_eq!(body[158], 0, "easy chat wait time");
    assert_eq!(body[159], 0, "wearing detection");
    assert_eq!(&body[161..165], &[0, 0, 0x11, 0x11], "slide buttons");

    let state = parse_state_frame(model(), &frame).expect("parse the real state");
    // The base TWS fields are the shared parser's.
    assert_eq!(
        state.host_device,
        if fixture.expected.host_device == "Right" {
            soundcore_models::HostDevice::Right
        } else {
            soundcore_models::HostDevice::Left
        }
    );
    assert_eq!(state.tws_connected, fixture.expected.tws_connected);
    let battery = state.battery.as_ref().expect("A3954 is a TWS layout");
    assert_eq!(
        (battery.left, battery.right),
        (fixture.expected.left, fixture.expected.right)
    );
    assert_eq!(battery.scale.maximum(), fixture.expected.scale);
    assert_eq!(battery.left_charging, fixture.expected.left_charging);
    assert_eq!(battery.right_charging, fixture.expected.right_charging);
    assert_eq!(
        state.firmware.left.as_deref(),
        Some(fixture.expected.firmware_left.as_str())
    );
    assert_eq!(
        state.firmware.right.as_deref(),
        Some(fixture.expected.firmware_right.as_str())
    );
    // The fixture masks the earbud serial to ASCII zeroes, which the parser
    // reports as absent; no real serial is committed.
    assert_eq!(state.serial, None);
    assert_eq!(state.single_battery, None);
    assert_eq!(
        state.equalizer, None,
        "the A3062 equalizer shape does not apply"
    );
    assert_eq!(
        state.settings, None,
        "the A3062 settings shape does not apply"
    );
    assert_eq!(
        state.sound_mode, None,
        "the A3062 three-mode shape does not apply"
    );

    let a3954 = state.a3954.expect("the A3954 block");
    assert_eq!(
        a3954.case_firmware.as_deref(),
        Some(fixture.expected.case_firmware.as_str())
    );
    assert_eq!(
        a3954.case_battery_level,
        Some(fixture.expected.case_battery_level)
    );
    assert_eq!(a3954.case_serial, None, "the fixture masks the case serial");

    let equalizer = &a3954.equalizer;
    assert_eq!(equalizer.preset_id, fixture.expected.equalizer_preset_id);
    assert!(equalizer.is_custom());
    assert_eq!(equalizer.preset(), None, "a custom curve is not a preset");
    let expected_left: [i16; A3954_EQ_BANDS] = fixture
        .expected
        .equalizer_left_bands
        .clone()
        .try_into()
        .expect("ten bands");
    let expected_right: Option<[i16; A3954_EQ_BANDS]> = fixture
        .expected
        .equalizer_right_bands
        .clone()
        .map(|bands| bands.try_into().expect("ten bands"));
    assert_eq!(equalizer.left, Some(expected_left));
    assert_eq!(equalizer.right, expected_right);
    let mut expected_visible = [0i16; VISIBLE_EQ_BANDS];
    expected_visible.copy_from_slice(&expected_left[..VISIBLE_EQ_BANDS]);
    assert_eq!(equalizer.visible_bands(), Some(expected_visible));
    let main_buttons: [u8; 8] = hex(&fixture.expected.main_buttons_hex)
        .try_into()
        .expect("eight bytes");
    assert_eq!(a3954.main_buttons, main_buttons);
    let cycle = &fixture.expected.ambient_sound_mode_cycle;
    assert_eq!(
        a3954.ambient_sound_mode_cycle,
        AmbientSoundModeCycle {
            noise_canceling: cycle.noise_canceling,
            transparency: cycle.transparency,
            normal: cycle.normal,
        }
    );
    assert_eq!(
        a3954.sound_mode.mode,
        expected_mode(&fixture.expected.sound_mode)
    );
    assert_eq!(
        a3954.sound_mode.slider.value(),
        fixture.expected.slider_position
    );
    assert_eq!(
        a3954.sound_mode.slider.mode(),
        expected_mode(&fixture.expected.sound_mode),
        "the measured mode agrees with the slider's half of the scale"
    );
    assert_eq!(
        a3954.sound_mode.airplane_mode,
        expected_airplane_mode(&fixture.expected.airplane_mode)
    );
    assert_eq!(
        a3954.sound_mode.wind_noise_suppression,
        fixture.expected.wind_noise_suppression
    );
    assert_eq!(
        a3954.sound_mode.wind_noise_detected,
        fixture.expected.wind_noise_detected
    );
    assert_eq!(
        a3954.case_features,
        A3954CaseFeatures {
            atmospheric: fixture.expected.case_features.atmospheric,
            remote_camera: fixture.expected.case_features.remote_camera,
            find_device: fixture.expected.case_features.find_device,
            case_spatial_audio: fixture.expected.case_features.case_spatial_audio,
        }
    );
    assert_eq!(a3954.air_pressure, fixture.expected.air_pressure);
    assert_eq!(
        a3954.low_battery_prompt,
        fixture.expected.low_battery_prompt
    );
    assert_eq!(a3954.ldac, fixture.expected.ldac);
    assert_eq!(a3954.dual_connections, fixture.expected.dual_connections);
    assert_eq!(
        a3954.auto_power_off,
        A3954AutoPowerOff {
            enabled: fixture.expected.auto_power_off.enabled,
            duration: A3954AutoPowerOffDuration::from_index(2).expect("30 minutes"),
        }
    );
    assert_eq!(
        a3954.auto_power_off.duration.minutes(),
        fixture.expected.auto_power_off.minutes
    );
    assert_eq!(
        a3954.limit_high_volume,
        LimitHighVolume {
            enabled: fixture.expected.limit_high_volume.enabled,
            db_limit: DbLimit::new(fixture.expected.limit_high_volume.db_limit)
                .expect("a measured dB limit"),
            refresh_rate: expected_refresh_rate(&fixture.expected.limit_high_volume.refresh_rate),
        }
    );
    assert_eq!(
        a3954.spatial_audio,
        A3954SpatialAudio {
            enabled: fixture.expected.spatial_audio.enabled,
            music_mode: expected_spatial_music_mode(&fixture.expected.spatial_audio.music_mode),
            mode: expected_spatial_mode(&fixture.expected.spatial_audio.mode),
        }
    );
    assert_eq!(
        a3954.easy_chat,
        A3954EasyChat {
            enabled: fixture.expected.easy_chat.enabled,
            wait_time: A3954EasyChatWaitTime::FiveSeconds,
        }
    );
    assert_eq!(
        a3954.easy_chat.wait_time.seconds(),
        fixture.expected.easy_chat.wait_time_seconds
    );
    assert_eq!(
        a3954.sound_leak_compensation,
        fixture.expected.sound_leak_compensation
    );
    assert_eq!(
        a3954.case_language,
        expected_case_language(&fixture.expected.case_language)
    );
    assert_eq!(a3954.wearing_detection, fixture.expected.wearing_detection);
    let slide_buttons: [u8; 4] = hex(&fixture.expected.slide_buttons_hex)
        .try_into()
        .expect("four bytes");
    assert_eq!(a3954.slide_buttons, slide_buttons);

    // The capability records the two measured block offsets.
    let capability = model().a3954.expect("the A3954 capability");
    assert_eq!(capability.equalizer_offset, 44);
    assert_eq!(capability.sound_mode_offset, 125);
}

#[test]
fn rejects_an_a3954_body_shorter_than_the_measured_165_bytes() {
    for length in [33, 164] {
        assert!(
            matches!(
                parse_state_body(model(), &synthetic_body()[..length]),
                Err(StateError::ShortBody {
                    actual,
                    minimum: A3954_STATE_BODY_LEN
                }) if actual == length
            ),
            "a {length}-byte body must be short"
        );
    }
}

#[test]
fn only_the_a3954_has_an_a3954_state() {
    for code in ["A3949", "A3947", "A3035", "A3040", "A3062"] {
        let model = model_by_code(code).expect("the catalog lists the model");
        assert!(model.a3954.is_none(), "{code}");
    }
    assert!(model().a3954.is_some());

    // The real A3949 capture parses to its shared TWS fields with no A3954
    // block, even though both use the same body prefix.
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    let frame = hex(fixture["frame_hex"].as_str().expect("frame_hex"));
    let state = parse_state_frame(model_by_code("A3949").expect("A3949"), &frame).expect("parse");
    assert_eq!(state.a3954, None);
}

#[test]
fn rejects_unknown_values_of_the_closed_a3954_enums() {
    let mutate = |offset: usize, value: u8| {
        let mut body = a3954_captured_body();
        body[offset] = value;
        parse_state_body(model(), &body)
    };

    assert!(matches!(
        mutate(125, 4),
        Err(StateError::InvalidA3954SoundMode(4))
    ));
    for position in [0, 12] {
        assert!(
            matches!(
                mutate(126, position),
                Err(StateError::InvalidA3954SoundModeSlider(value)) if value == position
            ),
            "slider {position}"
        );
    }
    assert!(matches!(
        mutate(127, 2),
        Err(StateError::InvalidA3954AirplaneMode(2))
    ));
    assert!(matches!(
        mutate(144, 4),
        Err(StateError::InvalidA3954AutoPowerOffIndex(4))
    ));
    assert!(matches!(
        mutate(146, 78),
        Err(StateError::InvalidLimitHighVolumeDbLimit(78))
    ));
    assert!(matches!(
        mutate(147, 3),
        Err(StateError::InvalidDecibelReadingRefreshRate(3))
    ));
    assert!(matches!(
        mutate(149, 3),
        Err(StateError::InvalidA3954SpatialAudioMusicMode(3))
    ));
    assert!(matches!(
        mutate(150, 4),
        Err(StateError::InvalidA3954SpatialAudioMode(4))
    ));
    assert!(matches!(
        mutate(158, 3),
        Err(StateError::InvalidA3954EasyChatWaitTime(3))
    ));
    assert!(matches!(
        mutate(157, 3),
        Err(StateError::InvalidA3954CaseLanguage(3))
    ));

    // A flag byte other than 1 reads as off instead of failing the poll.
    let mut body = a3954_captured_body();
    body[140] = 2;
    let state = parsed_state(&body);
    assert!(!state.low_battery_prompt);
}

#[test]
fn the_a3954_state_separates_an_absent_case_battery_and_a_present_case_serial() {
    let mut body = synthetic_body();
    body[37] = 255;
    body[38..44].copy_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
    let state = parsed_state(&body);
    assert_eq!(state.case_battery_level, None, "255 is unavailable");
    assert_eq!(
        state.case_serial.as_deref(),
        Some("3954123456789ABC"),
        "the case serial is the 3954 prefix plus the six bytes as uppercase hex"
    );

    // The masked fixture reports both as absent, so no real identity is
    // committed or logged.
    let captured = parsed_state(&a3954_captured_body());
    assert_eq!(captured.case_battery_level, Some(9));
    assert_eq!(captured.case_serial, None);
}

#[test]
fn maps_the_a3954_sound_mode_scale() {
    assert_eq!(
        A3954SoundModeSlider::new(0),
        None,
        "position 0 is outside the scale"
    );
    for position in 1..=11 {
        assert_eq!(
            A3954SoundModeSlider::new(position).map(A3954SoundModeSlider::value),
            Some(position)
        );
    }
    assert_eq!(A3954SoundModeSlider::new(12), None);
    assert_eq!(
        (A3954SoundModeSlider::MIN, A3954SoundModeSlider::MAX),
        (1, 11)
    );

    // Positions 1..=5 are transparency, 6 is normal and 7..=11 is noise
    // canceling; position 5 is the strongest transparency and 11 the strongest
    // noise canceling.
    for position in 1..=11u8 {
        let slider = A3954SoundModeSlider::new(position).expect("a measured position");
        let expected = match position {
            1..=5 => A3954AmbientSoundMode::Transparency,
            6 => A3954AmbientSoundMode::Normal,
            7..=11 => A3954AmbientSoundMode::NoiseCanceling,
            _ => unreachable!(),
        };
        assert_eq!(slider.mode(), expected, "position {position}");
    }
    assert_eq!(A3954SoundModeSlider::new(1).expect("1").strength(), Some(5));
    assert_eq!(A3954SoundModeSlider::new(5).expect("5").strength(), Some(1));
    assert_eq!(A3954SoundModeSlider::new(6).expect("6").strength(), None);
    assert_eq!(A3954SoundModeSlider::new(7).expect("7").strength(), Some(1));
    assert_eq!(
        A3954SoundModeSlider::new(11).expect("11").strength(),
        Some(5)
    );

    for strength in 1..=5u8 {
        let transparency = A3954SoundModeSlider::from_mode_and_strength(
            A3954AmbientSoundMode::Transparency,
            strength,
        )
        .expect("a measured transparency strength");
        assert_eq!(transparency.value(), 6 - strength);
        assert_eq!(transparency.mode(), A3954AmbientSoundMode::Transparency);
        assert_eq!(transparency.strength(), Some(strength));

        let canceling = A3954SoundModeSlider::from_mode_and_strength(
            A3954AmbientSoundMode::NoiseCanceling,
            strength,
        )
        .expect("a measured noise-canceling strength");
        assert_eq!(canceling.value(), 6 + strength);
        assert_eq!(canceling.mode(), A3954AmbientSoundMode::NoiseCanceling);
        assert_eq!(canceling.strength(), Some(strength));
    }
    assert_eq!(A3954SoundModeSlider::normal().value(), 6);
    assert_eq!(
        A3954SoundModeSlider::from_mode_and_strength(A3954AmbientSoundMode::Normal, 0)
            .map(A3954SoundModeSlider::value),
        Some(6),
        "normal ignores the strength and stays on position 6"
    );
    for strength in [0, 6] {
        assert_eq!(
            A3954SoundModeSlider::from_mode_and_strength(
                A3954AmbientSoundMode::NoiseCanceling,
                strength
            ),
            None,
            "strength {strength}"
        );
    }
    assert_eq!(
        A3954SoundModeSlider::from_mode_and_strength(A3954AmbientSoundMode::AirplaneMode, 3),
        None,
        "airplane mode has no slider position"
    );
}

#[test]
fn builds_every_captured_a3954_sound_mode_body_and_frame() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        writes: Vec<Write>,
    }
    #[derive(serde::Deserialize)]
    struct Write {
        mode_byte: u8,
        position: u8,
        payload_hex: String,
        frame_hex: String,
    }

    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3954-sound-modes.json"))
            .expect("fixture");
    assert_eq!(fixture.writes.len(), 11, "the app's captured scale writes");

    let mut positions = Vec::new();
    for write in &fixture.writes {
        let mode = A3954AmbientSoundMode::from_value(write.mode_byte)
            .expect("the capture writes a measured mode");
        let slider = A3954SoundModeSlider::new(write.position).expect("a measured position");
        assert_eq!(
            slider.mode(),
            mode,
            "the captured mode matches the slider's half of the scale"
        );
        positions.push(write.position);
        let sound_mode = A3954SoundMode {
            mode,
            slider,
            airplane_mode: A3954AirplaneMode::ManualUpdate,
            // Every captured write carries wind 01: suppression on, not detected.
            wind_noise_suppression: true,
            wind_noise_detected: false,
        };
        let body = model()
            .a3954_sound_mode_body(sound_mode)
            .expect("the A3954 builds a scale body");
        assert_eq!(body.len(), A3954_SOUND_MODE_COMMAND_BODY_LEN);
        assert_eq!(frame_hex(&body), write.payload_hex);
        assert_eq!(
            frame_hex(&encode(
                Direction::Outbound,
                Command::ACK_AMBIENT_SOUND_MODE,
                &body
            )),
            write.frame_hex
        );
    }
    assert_eq!(
        positions,
        [6, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11],
        "the capture walks normal then both halves of the scale"
    );
}

#[test]
fn the_a3954_preset_catalog_is_the_openscl4_type_2_table() {
    assert_eq!(A3954_EQUALIZER_PRESETS.len(), 22);
    let ids: Vec<u16> = A3954_EQUALIZER_PRESETS
        .iter()
        .map(|preset| preset.id)
        .collect();
    assert_eq!(ids, (0..=21).collect::<Vec<u16>>());
    for preset in A3954_EQUALIZER_PRESETS {
        assert!(!preset.name.is_empty());
        // The catalog is the same ten-band table as the A3062 one: the presets
        // fix the last band at -120 raw.
        assert_eq!(preset.bands.len(), A3954_EQ_BANDS);
        assert_eq!(preset.bands[9], -120, "{}", preset.name);
    }
    // Unlike the A3062 catalog, Bass Booster is id 2 and there is no 0x7E7E.
    assert_eq!(
        a3954_equalizer_preset(2).expect("Bass Booster"),
        &EqualizerPreset {
            id: 2,
            name: "Bass Booster",
            bands: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
        }
    );
    assert!(a3954_equalizer_preset(0x7E7E).is_none());
    // Spot checks transcribed from the reference table; Rock was also measured
    // on the real device.
    assert_eq!(
        a3954_equalizer_preset(0).expect("Signature").bands,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, -120]
    );
    assert_eq!(
        a3954_equalizer_preset(17).expect("Rock").bands,
        [30, 20, -10, -10, 10, 30, 40, 50, 0, -120]
    );
    assert_eq!(a3954_equalizer_preset(17).expect("Rock").name, "Rock");
    assert_eq!(
        a3954_equalizer_preset(21).expect("Treble Reducer").bands,
        [0, 0, 0, -20, -30, -40, -40, -60, 0, -120]
    );
    assert!(a3954_equalizer_preset(CUSTOM_EQUALIZER_PRESET_ID).is_none());
    assert!(a3954_equalizer_preset(22).is_none());

    // The model exposes its own catalog and keeps the A3062-shaped one empty.
    assert_eq!(model().a3954_equalizer_presets(), A3954_EQUALIZER_PRESETS);
    let a3062 = model_by_code("A3062").expect("A3062");
    assert_eq!(a3062.a3954_equalizer_presets(), &[]);
    assert!(model().equalizer_presets().is_empty());
}

#[test]
fn builds_the_exact_app_measured_a3954_equalizer_preset_bodies() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        frames: Vec<Frame>,
    }
    #[derive(serde::Deserialize)]
    struct Frame {
        id: u16,
        name: String,
        bands: Vec<i16>,
        body_hex: String,
    }

    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3954-eq-presets.json")).expect("fixture");
    assert_eq!(fixture.frames.len(), 2);

    for entry in &fixture.frames {
        let app_body = hex(&entry.body_hex);
        assert_eq!(app_body.len(), A3954_EQUALIZER_COMMAND_BODY_LEN);
        assert_eq!(
            u16::from_le_bytes([app_body[0], app_body[1]]),
            entry.id,
            "{}",
            entry.name
        );

        // Rebuild the state the app wrote from: the same preset and bands, and
        // the HearID fields the app echoed in its own body.
        let mut body = synthetic_body();
        body[44..46].copy_from_slice(&entry.id.to_le_bytes());
        let bands: [i16; A3954_EQ_BANDS] = entry
            .bands
            .clone()
            .try_into()
            .expect("the fixture records ten bands");
        let band_bytes: Vec<u8> = bands.iter().map(|value| (value + 120) as u8).collect();
        body[46..56].copy_from_slice(&band_bytes);
        body[56..66].copy_from_slice(&band_bytes);
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

        let current = parsed_equalizer(&body);
        let preset = a3954_equalizer_preset(entry.id).expect("a catalog preset");
        assert_eq!(preset.bands, bands);
        let built = model()
            .a3954_equalizer_preset_body(&current, preset)
            .expect("the A3954 preset body");
        assert_eq!(
            frame_hex(&built),
            entry.body_hex,
            "the built body is the app's own measured {} body",
            entry.name
        );
        assert_eq!(built[26], 0, "HearID is disabled for an equalizer write");
        assert_eq!(&built[4..24], &app_body[4..24]);
        assert_eq!(&built[72..112], &app_body[72..112]);
    }
}

#[test]
fn builds_the_exact_a3954_custom_equalizer_body() {
    let current = parsed_equalizer(&a3954_captured_body());
    // +3.0, -3.0, +6.0, 0, -6.0, +2.0, 0, -2.0 dB in raw dB×10 units.
    let curve = [30, -30, 60, 0, -60, 20, 0, -20];
    let body = model()
        .a3954_custom_equalizer_body(&current, curve)
        .expect("the A3954 custom body");
    assert_eq!(
        frame_hex(&body),
        "FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 78 96 5A \
        B4 78 3C 8C 78 64 78 78 FF FF 00 00 00 00 00 00 \
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
        00 00 00 00 00 00 00 00 96 00 5A 00 B4 00 78 00 \
        3C 00 8C 00 78 00 64 00 78 00 78 00 96 00 5A 00 \
        B4 00 78 00 3C 00 8C 00 78 00 64 00 78 00 78 00 \
        00 00",
        "the exact 114-byte custom equalizer body"
    );
    assert_eq!(body.len(), A3954_EQUALIZER_COMMAND_BODY_LEN);
    assert_eq!(
        u16::from_le_bytes([body[0], body[1]]),
        CUSTOM_EQUALIZER_PRESET_ID
    );
    // The curve is applied to both channels and the two invisible bands are
    // the measured fixed 0 dB, not the state's absent right channel.
    assert_eq!(
        &body[4..14],
        &[0x96, 0x5A, 0xB4, 0x78, 0x3C, 0x8C, 0x78, 0x64, 0x78, 0x78]
    );
    assert_eq!(&body[14..24], &body[4..14]);
    assert_eq!(
        &body[72..92],
        &[
            0x96, 0, 0x5A, 0, 0xB4, 0, 0x78, 0, 0x3C, 0, 0x8C, 0, 0x78, 0, 0x64, 0, 0x78, 0, 0x78,
            0,
        ]
    );
    assert_eq!(&body[92..112], &body[72..92]);
    assert_eq!(&body[112..114], &[0, 0]);

    let frame = encode(Direction::Outbound, Command::EQUALIZER, &body);
    assert_eq!(
        frame_hex(&frame),
        "08 EE 00 00 00 03 87 7C 00 FE FE 00 00 96 5A B4 \
        78 3C 8C 78 64 78 78 96 5A B4 78 3C 8C 78 64 78 \
        78 FF FF 00 00 00 00 00 00 00 00 00 00 00 00 00 \
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
        00 96 00 5A 00 B4 00 78 00 3C 00 8C 00 78 00 64 \
        00 78 00 78 00 96 00 5A 00 B4 00 78 00 3C 00 8C \
        00 78 00 64 00 78 00 78 00 00 00 B6",
        "the exact 124-byte 03 87 frame"
    );
    assert_eq!(frame.len(), 124);
}

#[test]
fn clamps_a3954_custom_bands_to_the_measured_range() {
    let current = parsed_equalizer(&a3954_captured_body());
    let body = model()
        .a3954_custom_equalizer_body(&current, [135, i16::MAX, -121, i16::MIN, 134, -120, 0, -10])
        .expect("the A3954 custom body");
    assert_eq!(
        &body[4..14],
        &[0xFE, 0xFE, 0x00, 0x00, 0xFE, 0x00, 0x78, 0x6E, 0x78, 0x78],
        "out-of-range bands clamp to -120..=134 and then add 120"
    );
    assert_eq!(&body[14..24], &body[4..14]);
}

#[test]
fn keeps_the_state_hear_id_block_verbatim_in_a3954_writes() {
    // A distinctive HearID block: enabled, two channels of volume
    // adjustments, time, type, custom adjustments and a music genre.
    let mut body = synthetic_body();
    let hear_id: [u8; 48] = std::array::from_fn(|index| 0x40 + index as u8);
    body[67..115].copy_from_slice(&hear_id);
    let current = parsed_equalizer(&body);

    let preset = a3954_equalizer_preset(3).expect("Bass Reducer");
    let body = model()
        .a3954_equalizer_preset_body(&current, preset)
        .expect("the A3954 preset body");
    assert_eq!(&body[2..4], &hear_id[46..48]);
    assert_eq!(&body[27..37], &hear_id[1..11]);
    assert_eq!(&body[37..47], &hear_id[11..21]);
    assert_eq!(&body[47..51], &hear_id[21..25]);
    assert_eq!(body[51], hear_id[25]);
    assert_eq!(&body[52..62], &hear_id[26..36]);
    assert_eq!(&body[62..72], &hear_id[36..46]);
    assert_eq!(body[26], 0, "HearID is disabled for an equalizer write");
}

#[test]
fn a3954_equalizer_channels_parse_as_present_and_absent() {
    let mut body = synthetic_body();
    let left: [u8; 10] = [0, 60, 120, 180, 254, 253, 1, 2, 3, 4];
    let right: [u8; 10] = [120, 120, 120, 120, 120, 120, 120, 120, 120, 0];
    body[46..56].copy_from_slice(&left);
    body[56..66].copy_from_slice(&right);
    let equalizer = parsed_equalizer(&body);
    assert_eq!(
        equalizer.left,
        Some([-120, -60, 0, 60, 134, 133, -119, -118, -117, -116])
    );
    assert_eq!(equalizer.right, Some([0, 0, 0, 0, 0, 0, 0, 0, 0, -120]));
    assert_eq!(
        equalizer.visible_bands(),
        Some([-120, -60, 0, 60, 134, 133, -119, -118])
    );

    // Only a leading 0xFF marks a channel absent; the real fixture reports the
    // left channel present and the right channel absent.
    let mut body = synthetic_body();
    body[46..56].copy_from_slice(&[0xFF; 10]);
    let equalizer = parsed_equalizer(&body);
    assert_eq!(equalizer.left, None);
    assert_eq!(equalizer.right, Some([-120; 10]));
    assert_eq!(
        equalizer.visible_bands(),
        Some([-120; VISIBLE_EQ_BANDS]),
        "the visible bands fall back to the right channel"
    );
}

#[test]
fn builds_the_exact_a3954_setting_frames() {
    let cases: &[(A3954SettingCommand, &str)] = &[
        (
            A3954SettingCommand::Ldac(true),
            "08 EE 00 00 00 01 FF 0B 00 01 02",
        ),
        (
            A3954SettingCommand::Ldac(false),
            "08 EE 00 00 00 01 FF 0B 00 00 01",
        ),
        (
            A3954SettingCommand::DualConnections(true),
            "08 EE 00 00 00 0B 84 0B 00 01 91",
        ),
        (
            A3954SettingCommand::DualConnections(false),
            "08 EE 00 00 00 0B 84 0B 00 00 90",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: true,
                duration: A3954AutoPowerOffDuration::TenMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 00 8A",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: true,
                duration: A3954AutoPowerOffDuration::TwentyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 01 8B",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: true,
                duration: A3954AutoPowerOffDuration::ThirtyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 02 8C",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: true,
                duration: A3954AutoPowerOffDuration::SixtyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 01 03 8D",
        ),
        (
            A3954SettingCommand::AutoPowerOff(A3954AutoPowerOff {
                enabled: false,
                duration: A3954AutoPowerOffDuration::ThirtyMinutes,
            }),
            "08 EE 00 00 00 01 86 0C 00 00 02 8B",
        ),
        (
            A3954SettingCommand::LimitHighVolume {
                enabled: false,
                db_limit: DbLimit::new(90).expect("a measured dB step"),
            },
            "08 EE 00 00 00 20 82 0C 00 00 5A FE",
        ),
        (
            A3954SettingCommand::LimitHighVolume {
                enabled: true,
                db_limit: DbLimit::new(75).expect("a measured dB step"),
            },
            "08 EE 00 00 00 20 82 0C 00 01 4B F0",
        ),
        (
            A3954SettingCommand::LimitHighVolume {
                enabled: true,
                db_limit: DbLimit::new(100).expect("a measured dB step"),
            },
            "08 EE 00 00 00 20 82 0C 00 01 64 09",
        ),
        (
            A3954SettingCommand::LimitHighVolumeRefreshRate(DecibelReadingRefreshRate::RealTime),
            "08 EE 00 00 00 20 81 0B 00 00 A2",
        ),
        (
            A3954SettingCommand::LimitHighVolumeRefreshRate(DecibelReadingRefreshRate::TenSeconds),
            "08 EE 00 00 00 20 81 0B 00 01 A3",
        ),
        (
            A3954SettingCommand::LimitHighVolumeRefreshRate(DecibelReadingRefreshRate::OneMinute),
            "08 EE 00 00 00 20 81 0B 00 02 A4",
        ),
        (
            A3954SettingCommand::LowBatteryPrompt(true),
            "08 EE 00 00 00 10 82 0B 00 01 94",
        ),
        (
            A3954SettingCommand::LowBatteryPrompt(false),
            "08 EE 00 00 00 10 82 0B 00 00 93",
        ),
        (
            A3954SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: true,
            }),
            "08 EE 00 00 00 06 82 0B 00 07 90",
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
            A3954SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: false,
                transparency: false,
                normal: false,
            }),
            "08 EE 00 00 00 06 82 0B 00 00 89",
        ),
        (
            A3954SettingCommand::WearingDetection(true),
            "08 EE 00 00 00 01 81 0B 00 01 84",
        ),
        (
            A3954SettingCommand::WearingDetection(false),
            "08 EE 00 00 00 01 81 0B 00 00 83",
        ),
        (
            A3954SettingCommand::SoundLeakCompensation(true),
            "08 EE 00 00 00 10 9A 0B 00 01 AC",
        ),
        (
            A3954SettingCommand::SoundLeakCompensation(false),
            "08 EE 00 00 00 10 9A 0B 00 00 AB",
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
            A3954SettingCommand::SpatialAudio(A3954SpatialAudio {
                enabled: false,
                music_mode: A3954SpatialAudioMusicMode::Fixed,
                mode: A3954SpatialAudioMode::Music,
            }),
            "08 EE 00 00 00 10 81 0D 00 00 01 00 95",
        ),
        (
            A3954SettingCommand::SpatialAudio(A3954SpatialAudio {
                enabled: true,
                music_mode: A3954SpatialAudioMusicMode::HeadTracking,
                mode: A3954SpatialAudioMode::Movie,
            }),
            "08 EE 00 00 00 10 81 0D 00 01 02 02 99",
        ),
        (
            A3954SettingCommand::EasyChat(A3954EasyChat {
                enabled: true,
                wait_time: A3954EasyChatWaitTime::FiveSeconds,
            }),
            "08 EE 00 00 00 10 9D 0C 00 01 00 B0",
        ),
        (
            A3954SettingCommand::EasyChat(A3954EasyChat {
                enabled: false,
                wait_time: A3954EasyChatWaitTime::FifteenSeconds,
            }),
            "08 EE 00 00 00 10 9D 0C 00 00 02 B1",
        ),
        (
            A3954SettingCommand::CaseLanguage(A3954CaseLanguage::Japanese),
            "08 EE 00 00 00 07 8A 0B 00 00 92",
        ),
        (
            A3954SettingCommand::CaseLanguage(A3954CaseLanguage::English),
            "08 EE 00 00 00 07 8A 0B 00 01 93",
        ),
        (
            A3954SettingCommand::CaseLanguage(A3954CaseLanguage::Chinese),
            "08 EE 00 00 00 07 8A 0B 00 02 94",
        ),
    ];

    for (setting, expected) in cases {
        let frame = model()
            .a3954_setting_frame(*setting)
            .expect("the A3954 builds a setting frame");
        assert_eq!(frame_hex(&frame), *expected, "{setting:?}");
    }
}

#[test]
fn maps_the_a3954_auto_power_off_durations() {
    assert_eq!(
        A3954AutoPowerOffDuration::ALL.map(A3954AutoPowerOffDuration::index),
        [0, 1, 2, 3]
    );
    for (index, minutes) in [(0, 10), (1, 20), (2, 30), (3, 60)] {
        let duration = A3954AutoPowerOffDuration::from_index(index).expect("a measured index");
        assert_eq!(duration.minutes(), minutes);
    }
    assert_eq!(A3954AutoPowerOffDuration::from_index(4), None);
    assert_eq!(A3954AutoPowerOffDuration::from_index(255), None);
}

#[test]
fn maps_the_a3954_setting_value_enums() {
    assert_eq!(
        A3954AmbientSoundMode::ALL.map(A3954AmbientSoundMode::value),
        [0, 1, 2, 3]
    );
    for mode in A3954AmbientSoundMode::ALL {
        assert_eq!(A3954AmbientSoundMode::from_value(mode.value()), Some(mode));
    }
    assert_eq!(A3954AmbientSoundMode::from_value(4), None);

    for mode in [
        A3954AirplaneMode::ManualUpdate,
        A3954AirplaneMode::AutomaticUpdate,
    ] {
        assert_eq!(A3954AirplaneMode::from_value(mode.value()), Some(mode));
    }
    assert_eq!(A3954AirplaneMode::from_value(2), None);

    assert_eq!(
        A3954SpatialAudioMode::ALL.map(A3954SpatialAudioMode::value),
        [0, 1, 2, 3]
    );
    for mode in A3954SpatialAudioMode::ALL {
        assert_eq!(A3954SpatialAudioMode::from_value(mode.value()), Some(mode));
    }
    assert_eq!(A3954SpatialAudioMode::from_value(4), None);

    // The state reports 0 for fixed music mode; every command writes 1.
    assert_eq!(
        A3954SpatialAudioMusicMode::from_state_value(0),
        Some(A3954SpatialAudioMusicMode::Fixed)
    );
    assert_eq!(
        A3954SpatialAudioMusicMode::from_state_value(1),
        Some(A3954SpatialAudioMusicMode::Fixed)
    );
    assert_eq!(
        A3954SpatialAudioMusicMode::from_state_value(2),
        Some(A3954SpatialAudioMusicMode::HeadTracking)
    );
    assert_eq!(A3954SpatialAudioMusicMode::from_state_value(3), None);
    assert_eq!(A3954SpatialAudioMusicMode::Fixed.value(), 1);
    assert_eq!(A3954SpatialAudioMusicMode::HeadTracking.value(), 2);

    assert_eq!(
        A3954EasyChatWaitTime::ALL.map(A3954EasyChatWaitTime::value),
        [0, 1, 2]
    );
    for wait_time in A3954EasyChatWaitTime::ALL {
        assert_eq!(
            A3954EasyChatWaitTime::from_value(wait_time.value()),
            Some(wait_time)
        );
    }
    assert_eq!(
        A3954EasyChatWaitTime::ALL.map(A3954EasyChatWaitTime::seconds),
        [5, 10, 15]
    );
    assert_eq!(A3954EasyChatWaitTime::from_value(3), None);

    assert_eq!(
        A3954CaseLanguage::ALL.map(A3954CaseLanguage::value),
        [0, 1, 2]
    );
    for language in A3954CaseLanguage::ALL {
        assert_eq!(
            A3954CaseLanguage::from_value(language.value()),
            Some(language)
        );
    }
    assert_eq!(A3954CaseLanguage::from_value(3), None);
}

#[test]
fn non_a3954_models_build_no_a3954_frames() {
    let sound_mode = A3954SoundMode {
        mode: A3954AmbientSoundMode::Normal,
        slider: A3954SoundModeSlider::normal(),
        airplane_mode: A3954AirplaneMode::ManualUpdate,
        wind_noise_suppression: false,
        wind_noise_detected: false,
    };
    let equalizer = parsed_equalizer(&a3954_captured_body());
    let preset = a3954_equalizer_preset(0).expect("Signature");
    for code in ["A3949", "A3947", "A3035", "A3040", "A3062"] {
        let model = model_by_code(code).expect("the catalog lists the model");
        assert_eq!(model.a3954_sound_mode_body(sound_mode), None, "{code}");
        assert!(model.a3954_equalizer_presets().is_empty(), "{code}");
        assert!(
            model
                .a3954_equalizer_preset_body(&equalizer, preset)
                .is_none(),
            "{code}"
        );
        assert!(
            model
                .a3954_custom_equalizer_body(&equalizer, [0; VISIBLE_EQ_BANDS])
                .is_none(),
            "{code} builds no custom body"
        );
        assert!(
            model
                .a3954_setting_frame(A3954SettingCommand::Ldac(true))
                .is_none(),
            "{code} builds no A3954 setting frame"
        );
    }
}

#[test]
fn the_a3954_scale_write_is_not_the_a3062_three_mode_write() {
    // The A3954 has no three-mode control: the A3062-shaped API stays empty and
    // the model has no A3062 sound-mode capability.
    assert_eq!(model().sound_mode, None);
    assert_eq!(model().sound_mode_body(SoundMode::Normal), None);
    assert_eq!(
        model()
            .a3954_sound_mode_body(A3954SoundMode {
                mode: A3954AmbientSoundMode::Normal,
                slider: A3954SoundModeSlider::normal(),
                airplane_mode: A3954AirplaneMode::ManualUpdate,
                wind_noise_suppression: false,
                wind_noise_detected: false,
            })
            .expect("the A3954 scale body"),
        [2, 6, 0, 0, 0xFF]
    );
    assert_eq!(
        request(Command::STATE),
        hex("08 EE 00 00 00 01 01 0A 00 02"),
        "the state request is shared"
    );
}
