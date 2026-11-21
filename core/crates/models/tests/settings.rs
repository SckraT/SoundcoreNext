//! A3062 remaining-settings state and command tests.
//!
//! The committed captures are real `01 01` answers of a Space One Pro captured
//! over the Windows RFCOMM transport; every one carries the same measured
//! settings block, and the frames are sanitized (zero serial bytes, no address,
//! checksums recalculated).

use soundcore_models::{
    A3062Settings, AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffDuration, ButtonAction,
    DbLimit, DecibelReadingRefreshRate, LimitHighVolume, NO_BUTTON_ACTION, SettingCommand,
    SettingsCapability, StateError, model_by_code, parse_state_body, parse_state_frame,
};

/// The measured settings bytes before the sound-mode block: body 67..69.
///
/// The sound-mode bytes at body 69..76 differ between the first committed
/// capture and the later ones (the app had rewritten the sound mode tail), so
/// they are not part of this block's settings assertion.
const MEASURED_SETTINGS_PREFIX: [u8; 2] = [0x0F, 0x03];

/// The measured settings block after the sound-mode block: body 76..87.
///
/// Low battery prompt on, Dolby off, LDAC off, dual connections on, auto power
/// off enabled at 60 minutes, limit high volume off at 90 dB with real-time
/// refresh, side tone on and voice prompt on.
const MEASURED_SETTINGS_SUFFIX: [u8; 11] = [
    0x01, 0x00, 0x00, 0x01, 0x01, 0x01, 0x00, 0x5A, 0x00, 0x01, 0x01,
];

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

#[derive(serde::Deserialize)]
struct StateFixture {
    frame_hex: String,
}

#[derive(serde::Deserialize)]
struct NamedFrame {
    frame_hex: String,
}

#[derive(serde::Deserialize)]
struct SoundModesFixture {
    modes: Vec<NamedFrame>,
}

#[derive(serde::Deserialize)]
struct EqualizerFixture {
    presets: Vec<NamedFrame>,
    restored_frame_hex: String,
}

#[derive(serde::Deserialize)]
struct CustomEqualizerFixture {
    before_frame_hex: String,
    custom_frame_hex: String,
    restored_frame_hex: String,
}

/// Every committed real A3062 `01 01` frame: the initial state capture, the
/// three sound-mode captures, the three preset captures plus the restored one,
/// and the three custom-curve captures.
fn committed_a3062_frames() -> Vec<Vec<u8>> {
    let state: StateFixture =
        serde_json::from_str(include_str!("../../../data/a3062-state.json")).expect("fixture");
    let sound_modes: SoundModesFixture =
        serde_json::from_str(include_str!("../../../data/a3062-sound-modes.json"))
            .expect("fixture");
    let presets: EqualizerFixture =
        serde_json::from_str(include_str!("../../../data/a3062-eq-presets.json")).expect("fixture");
    let custom: CustomEqualizerFixture =
        serde_json::from_str(include_str!("../../../data/a3062-eq-custom.json")).expect("fixture");

    let mut frames = vec![hex(&state.frame_hex)];
    frames.extend(sound_modes.modes.iter().map(|entry| hex(&entry.frame_hex)));
    frames.extend(presets.presets.iter().map(|entry| hex(&entry.frame_hex)));
    frames.push(hex(&presets.restored_frame_hex));
    frames.push(hex(&custom.before_frame_hex));
    frames.push(hex(&custom.custom_frame_hex));
    frames.push(hex(&custom.restored_frame_hex));
    frames
}

/// The measured settings block of the real captures as parsed values.
fn measured_settings() -> A3062Settings {
    A3062Settings {
        button_double_press: None,
        ambient_sound_mode_cycle: AmbientSoundModeCycle {
            noise_canceling: true,
            transparency: true,
            normal: false,
        },
        low_battery_prompt: true,
        dolby_audio: false,
        ldac: false,
        dual_connections: true,
        auto_power_off: AutoPowerOff {
            enabled: true,
            duration: AutoPowerOffDuration::SixtyMinutes,
        },
        limit_high_volume: LimitHighVolume {
            enabled: false,
            db_limit: DbLimit::new(90).expect("90 dB is a measured step"),
            refresh_rate: DecibelReadingRefreshRate::RealTime,
        },
        side_tone: true,
        voice_prompt: true,
    }
}

/// The measured A3062 body, body 95 bytes, with every block parseable.
fn captured_a3062_body() -> Vec<u8> {
    let fixture: StateFixture =
        serde_json::from_str(include_str!("../../../data/a3062-state.json")).expect("fixture");
    let frame = hex(&fixture.frame_hex);
    frame[9..frame.len() - 1].to_vec()
}

/// The measured body with one offset replaced.
fn body_with(offset: usize, value: u8) -> Vec<u8> {
    let mut body = captured_a3062_body();
    body[offset] = value;
    body
}

#[test]
fn parses_the_measured_settings_from_every_committed_a3062_capture() {
    let model = model_by_code("A3062").expect("A3062");
    assert_eq!(
        model.settings,
        Some(SettingsCapability { state_offset: 67 }),
        "the settings block starts at the measured body byte 67"
    );

    let frames = committed_a3062_frames();
    assert_eq!(frames.len(), 11, "the committed A3062 captures");
    for frame in frames {
        assert_eq!(frame.len(), 105, "the measured state frame length");
        let body = &frame[9..frame.len() - 1];
        assert_eq!(
            &body[67..69],
            &MEASURED_SETTINGS_PREFIX,
            "every committed capture carries the same double press and cycle"
        );
        assert_eq!(
            &body[76..87],
            &MEASURED_SETTINGS_SUFFIX,
            "every committed capture carries the same settings after the sound modes"
        );
        let state = parse_state_frame(model, &frame).expect("parse the real state");
        assert_eq!(state.settings, Some(measured_settings()));
    }
}

#[test]
fn a_model_without_a_measured_settings_block_has_none() {
    let model = model_by_code("A3949").expect("A3949");
    let fixture: StateFixture =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    let state = parse_state_frame(model, &hex(&fixture.frame_hex)).expect("parse the real state");
    assert_eq!(state.settings, None, "the P20i has no measured settings");

    for code in ["A3949", "A3947", "A3954", "A3035", "A3040"] {
        let model = model_by_code(code).expect("the catalog lists the model");
        assert_eq!(model.settings, None, "{code}");
        assert_eq!(
            model.setting_frame(SettingCommand::Ldac(true)),
            None,
            "{code} must not build a settings frame"
        );
    }
}

#[test]
fn builds_the_exact_frame_for_every_setting() {
    let model = model_by_code("A3062").expect("A3062");
    let cycle = |noise_canceling, transparency, normal| {
        SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
            noise_canceling,
            transparency,
            normal,
        })
    };
    let auto_power_off =
        |enabled, duration| SettingCommand::AutoPowerOff(AutoPowerOff { enabled, duration });
    let limit = |enabled, db_limit| SettingCommand::LimitHighVolume {
        enabled,
        db_limit: DbLimit::new(db_limit).expect("a measured dB limit"),
    };
    let refresh = SettingCommand::LimitHighVolumeRefreshRate;

    let cases: &[(SettingCommand, &str)] = &[
        (
            SettingCommand::ButtonDoublePress(Some(ButtonAction::BassUp)),
            "08 EE 00 00 00 04 81 0D 00 00 00 07 8F",
        ),
        (
            SettingCommand::ButtonDoublePress(None),
            "08 EE 00 00 00 04 81 0D 00 00 00 0F 97",
        ),
        (cycle(true, true, true), "08 EE 00 00 00 06 82 0B 00 07 90"),
        (cycle(true, true, false), "08 EE 00 00 00 06 82 0B 00 03 8C"),
        (
            cycle(false, false, false),
            "08 EE 00 00 00 06 82 0B 00 00 89",
        ),
        (
            SettingCommand::LowBatteryPrompt(true),
            "08 EE 00 00 00 10 82 0B 00 01 94",
        ),
        (
            SettingCommand::LowBatteryPrompt(false),
            "08 EE 00 00 00 10 82 0B 00 00 93",
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
            SettingCommand::Ldac(false),
            "08 EE 00 00 00 01 FF 0B 00 00 01",
        ),
        (
            SettingCommand::DualConnections(true),
            "08 EE 00 00 00 0B 84 0B 00 01 91",
        ),
        (
            SettingCommand::DualConnections(false),
            "08 EE 00 00 00 0B 84 0B 00 00 90",
        ),
        (
            auto_power_off(true, AutoPowerOffDuration::ThirtyMinutes),
            "08 EE 00 00 00 01 86 0C 00 01 00 8A",
        ),
        (
            auto_power_off(true, AutoPowerOffDuration::SixtyMinutes),
            "08 EE 00 00 00 01 86 0C 00 01 01 8B",
        ),
        (
            auto_power_off(true, AutoPowerOffDuration::NinetyMinutes),
            "08 EE 00 00 00 01 86 0C 00 01 02 8C",
        ),
        (
            auto_power_off(true, AutoPowerOffDuration::OneHundredTwentyMinutes),
            "08 EE 00 00 00 01 86 0C 00 01 03 8D",
        ),
        (
            auto_power_off(false, AutoPowerOffDuration::SixtyMinutes),
            "08 EE 00 00 00 01 86 0C 00 00 01 8A",
        ),
        (limit(false, 90), "08 EE 00 00 00 20 82 0C 00 00 5A FE"),
        (limit(true, 75), "08 EE 00 00 00 20 82 0C 00 01 4B F0"),
        (limit(true, 100), "08 EE 00 00 00 20 82 0C 00 01 64 09"),
        (
            refresh(DecibelReadingRefreshRate::RealTime),
            "08 EE 00 00 00 20 81 0B 00 00 A2",
        ),
        (
            refresh(DecibelReadingRefreshRate::TenSeconds),
            "08 EE 00 00 00 20 81 0B 00 01 A3",
        ),
        (
            refresh(DecibelReadingRefreshRate::OneMinute),
            "08 EE 00 00 00 20 81 0B 00 02 A4",
        ),
        (
            SettingCommand::SideTone(true),
            "08 EE 00 00 00 01 84 0B 00 01 87",
        ),
        (
            SettingCommand::SideTone(false),
            "08 EE 00 00 00 01 84 0B 00 00 86",
        ),
        (
            SettingCommand::VoicePrompt(true),
            "08 EE 00 00 00 01 AE 0B 00 01 B1",
        ),
        (
            SettingCommand::VoicePrompt(false),
            "08 EE 00 00 00 01 AE 0B 00 00 B0",
        ),
    ];

    for (setting, expected) in cases {
        let frame = model
            .setting_frame(*setting)
            .expect("the A3062 has a settings block");
        assert_eq!(frame, hex(expected), "{setting:?}");
    }
}

#[test]
fn validates_the_limit_high_volume_db_limit() {
    for value in [75, 80, 85, 90, 95, 100] {
        assert_eq!(
            DbLimit::new(value).map(DbLimit::value),
            Some(value),
            "{value} dB is a measured step"
        );
    }
    for value in [0, 70, 74, 76, 78, 79, 101, 105, 255] {
        assert_eq!(
            DbLimit::new(value),
            None,
            "{value} dB is not a measured limit"
        );
    }
    assert_eq!((DbLimit::MIN, DbLimit::MAX, DbLimit::STEP), (75, 100, 5));
}

#[test]
fn maps_auto_power_off_indexes_and_minutes() {
    for (index, duration, minutes) in [
        (0, AutoPowerOffDuration::ThirtyMinutes, 30),
        (1, AutoPowerOffDuration::SixtyMinutes, 60),
        (2, AutoPowerOffDuration::NinetyMinutes, 90),
        (3, AutoPowerOffDuration::OneHundredTwentyMinutes, 120),
    ] {
        assert_eq!(AutoPowerOffDuration::from_index(index), Some(duration));
        assert_eq!(duration.index(), index);
        assert_eq!(duration.minutes(), minutes);
    }
    assert_eq!(AutoPowerOffDuration::from_index(4), None);
    assert_eq!(AutoPowerOffDuration::from_index(255), None);
    assert_eq!(
        AutoPowerOffDuration::ALL.map(AutoPowerOffDuration::index),
        [0, 1, 2, 3]
    );
}

#[test]
fn maps_the_decibel_reading_refresh_rate() {
    assert_eq!(
        DecibelReadingRefreshRate::ALL.map(DecibelReadingRefreshRate::value),
        [0, 1, 2]
    );
    for rate in DecibelReadingRefreshRate::ALL {
        assert_eq!(
            DecibelReadingRefreshRate::from_value(rate.value()),
            Some(rate)
        );
    }
    assert_eq!(DecibelReadingRefreshRate::from_value(3), None);
    assert_eq!(DecibelReadingRefreshRate::from_value(255), None);
}

#[test]
fn maps_the_button_action() {
    assert_eq!(ButtonAction::from_value(7), Some(ButtonAction::BassUp));
    assert_eq!(ButtonAction::BassUp.value(), 7);
    for value in [0, 1, 6, 8, 0x0F, 0xFF] {
        assert_eq!(ButtonAction::from_value(value), None, "{value}");
    }
    assert_eq!(NO_BUTTON_ACTION, 0x0F);
}

#[test]
fn maps_the_ambient_sound_mode_cycle_bits() {
    for (byte, (noise_canceling, transparency, normal)) in [
        (0x00, (false, false, false)),
        (0x01, (true, false, false)),
        (0x02, (false, true, false)),
        (0x03, (true, true, false)),
        (0x04, (false, false, true)),
        (0x05, (true, false, true)),
        (0x06, (false, true, true)),
        (0x07, (true, true, true)),
    ] {
        let cycle = AmbientSoundModeCycle::from_byte(byte);
        assert_eq!(
            (cycle.noise_canceling, cycle.transparency, cycle.normal),
            (noise_canceling, transparency, normal),
            "{byte:02X}"
        );
        assert_eq!(cycle.byte(), byte, "{byte:02X}");
    }
    // Bits 3..7 are not part of the cycle: they are ignored on read and never
    // written, as in OpenSCL4's parser and serializer.
    assert_eq!(AmbientSoundModeCycle::from_byte(0xF8).byte(), 0x00);
    assert_eq!(AmbientSoundModeCycle::from_byte(0xFF).byte(), 0x07);
}

#[test]
fn rejects_unknown_values_of_the_closed_settings_enums() {
    let model = model_by_code("A3062").expect("A3062");

    assert!(matches!(
        parse_state_body(model, &body_with(67, 0x03)),
        Err(StateError::InvalidButtonAction(3))
    ));
    assert!(matches!(
        parse_state_body(model, &body_with(81, 4)),
        Err(StateError::InvalidAutoPowerOffIndex(4))
    ));
    assert!(matches!(
        parse_state_body(model, &body_with(83, 78)),
        Err(StateError::InvalidLimitHighVolumeDbLimit(78))
    ));
    assert!(matches!(
        parse_state_body(model, &body_with(84, 3)),
        Err(StateError::InvalidDecibelReadingRefreshRate(3))
    ));
}

#[test]
fn reads_an_unexpected_flag_byte_as_off() {
    // OpenSCL4 reads a flag as on only when the byte is 1; any other byte is
    // off. Following it keeps an unknown flag value from failing the whole
    // state poll.
    let model = model_by_code("A3062").expect("A3062");
    let state = parse_state_body(model, &body_with(76, 0x02)).expect("parse");
    let settings = state.settings.expect("the A3062 settings");
    assert!(!settings.low_battery_prompt);
}

#[test]
fn a_body_truncated_inside_the_settings_block_is_short() {
    let model = model_by_code("A3062").expect("A3062");
    let body = captured_a3062_body();
    assert!(matches!(
        parse_state_body(model, &body[..86]),
        Err(StateError::ShortBody {
            actual: 86,
            minimum: 87
        })
    ));
}
