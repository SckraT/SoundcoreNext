//! State parser tests against real and synthetic frames.

use soundcore_models::{
    A3954, AmbientSoundModeCycle, BatteryScale, DbLimit, HostDevice, Layout, NO_BUTTON_ACTION,
    SoundMode, StateError, model_by_code, parse_state_body, parse_state_frame,
};
use soundcore_protocol::{Command, Direction, encode};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

#[derive(serde::Deserialize)]
struct Fixture {
    frame_hex: String,
    expected: Expected,
}

#[derive(serde::Deserialize)]
struct Expected {
    host_device: String,
    tws_connected: bool,
    left: u8,
    right: u8,
    scale: u8,
    firmware_left: String,
    firmware_right: String,
    serial: Option<String>,
}

#[derive(serde::Deserialize)]
struct SingleFixture {
    frame_hex: String,
    expected: SingleExpected,
}

#[derive(serde::Deserialize)]
struct SingleExpected {
    battery: u8,
    scale: u8,
    firmware: String,
    serial: Option<String>,
}

#[derive(serde::Deserialize)]
struct SoundModeFixture {
    state_offset: usize,
    modes: Vec<SoundModeFrame>,
}

#[derive(serde::Deserialize)]
struct SoundModeFrame {
    mode: String,
    frame_hex: String,
}

fn named_sound_mode(name: &str) -> SoundMode {
    match name {
        "NoiseCanceling" => SoundMode::NoiseCanceling,
        "Transparency" => SoundMode::Transparency,
        "Normal" => SoundMode::Normal,
        other => panic!("the fixture names an unknown mode {other:?}"),
    }
}

#[test]
fn parses_the_real_a3949_state() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    let model = model_by_code("A3949").expect("A3949");
    let state = parse_state_frame(model, &hex(&fixture.frame_hex)).expect("parse");

    assert_eq!(
        state.host_device,
        if fixture.expected.host_device == "Right" {
            HostDevice::Right
        } else {
            HostDevice::Left
        }
    );
    assert_eq!(state.tws_connected, fixture.expected.tws_connected);
    let battery = state.battery.as_ref().expect("A3949 is a TWS layout");
    assert_eq!(battery.left, fixture.expected.left);
    assert_eq!(battery.right, fixture.expected.right);
    assert_eq!(battery.scale.maximum(), fixture.expected.scale);
    assert!(!battery.left_charging);
    assert!(!battery.right_charging);
    assert_eq!(
        state.firmware.left.as_deref(),
        Some(fixture.expected.firmware_left.as_str())
    );
    assert_eq!(
        state.firmware.right.as_deref(),
        Some(fixture.expected.firmware_right.as_str())
    );
    assert_eq!(state.serial, fixture.expected.serial);
    assert_eq!(state.single_battery, None);
    // The P20i has no ANC modes, and this parser does not invent any.
    assert_eq!(state.sound_mode, None);
}

#[test]
fn parses_the_real_a3062_single_battery_state() {
    let fixture: SingleFixture =
        serde_json::from_str(include_str!("../../../data/a3062-state.json")).expect("fixture");
    let model = model_by_code("A3062").expect("A3062");
    let state = parse_state_frame(model, &hex(&fixture.frame_hex)).expect("parse");

    let single = state
        .single_battery
        .as_ref()
        .expect("A3062 is a single-battery layout");
    assert_eq!(single.value, fixture.expected.battery);
    assert_eq!(single.scale, BatteryScale::Ten);
    assert_eq!(single.scale.maximum(), fixture.expected.scale);
    // Body byte 1 is the charging flag: the capture carries `0xFF`, which
    // OpenSCL4's A3062 parser reads as not charging. A `01 04` notification may
    // later report a newer value, but this poll does not request it.
    assert_eq!(single.charging, Some(false));
    assert_eq!(
        single.firmware.as_deref(),
        Some(fixture.expected.firmware.as_str())
    );
    assert_eq!(state.serial, fixture.expected.serial);
    assert_eq!(state.battery, None);
    // The fixture was captured while the device was in normal mode; the mode
    // byte at body offset 69 is `02`.
    assert_eq!(state.sound_mode, Some(SoundMode::Normal));
}

#[test]
fn parses_each_measured_a3062_sound_mode() {
    let fixture: SoundModeFixture =
        serde_json::from_str(include_str!("../../../data/a3062-sound-modes.json"))
            .expect("fixture");
    let model = model_by_code("A3062").expect("A3062");
    let capability = model.sound_mode.expect("A3062 has a measured sound mode");
    assert_eq!(capability.state_offset, fixture.state_offset);
    assert_eq!(fixture.modes.len(), SoundMode::ALL.len());

    let mut frames = Vec::new();
    for (entry, expected) in fixture.modes.iter().zip(SoundMode::ALL) {
        assert_eq!(named_sound_mode(&entry.mode), expected);
        let frame = hex(&entry.frame_hex);
        // The measured mode byte is at the fixture's declared body offset.
        assert_eq!(frame[9 + fixture.state_offset], expected.value());
        let state = parse_state_frame(model, &frame).expect("parse");
        assert_eq!(state.sound_mode, Some(expected), "{}", entry.mode);
        // The mode write does not disturb the measured battery and firmware.
        let single = state.single_battery.expect("the single-battery layout");
        assert_eq!(single.firmware.as_deref(), Some("04.39"));
        frames.push(frame);
    }

    // The three real captures differ only in the mode byte and the checksum.
    for window in frames.windows(2) {
        let (before, after) = (&window[0], &window[1]);
        let changed: Vec<usize> = before
            .iter()
            .zip(after)
            .enumerate()
            .filter(|(_, (before, after))| before != after)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            changed,
            [9 + fixture.state_offset, before.len() - 1],
            "only the mode byte and the checksum may differ"
        );
    }
}

#[test]
fn the_a3062_sound_mode_command_is_the_measured_six_byte_body() {
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
        let body = model.sound_mode_body(mode).expect("A3062 has a mode body");
        assert_eq!(body, [mode.value(), 0x51, 0x01, 0x01, 0x00, 0x05]);
        assert_eq!(
            encode(Direction::Outbound, Command::ACK_AMBIENT_SOUND_MODE, &body),
            hex(expected_frame),
            "{mode:?}"
        );
    }
}

#[test]
fn models_without_a_measured_sound_mode_have_no_command_body() {
    for code in ["A3949", "A3947", "A3954", "A3035", "A3040"] {
        let model = model_by_code(code).expect("the catalog lists the model");
        assert!(model.sound_mode.is_none(), "{code}");
        assert_eq!(model.sound_mode_body(SoundMode::Normal), None, "{code}");
    }
}

#[test]
fn sound_mode_values_are_the_measured_ones() {
    assert_eq!(SoundMode::ALL.map(SoundMode::value), [0, 1, 2]);
    for mode in SoundMode::ALL {
        assert_eq!(SoundMode::from_value(mode.value()), Some(mode));
    }
    assert_eq!(SoundMode::from_value(3), None);
}

#[test]
fn rejects_an_unknown_sound_mode_byte() {
    let model = model_by_code("A3062").expect("A3062");
    let mut body = single_battery_body(5);
    body[69] = 3;
    assert!(matches!(
        parse_state_body(model, &body),
        Err(StateError::InvalidSoundMode(3))
    ));
}

#[test]
fn a_truncated_a3062_body_is_short_of_the_equalizer_block() {
    let model = model_by_code("A3062").expect("A3062");
    let body = single_battery_body(5);
    // The equalizer block starts at body offset 23 and runs to the end of the
    // HearID block at offset 65, before the sound-mode byte at 69.
    assert!(matches!(
        parse_state_body(model, &body[..23]),
        Err(StateError::ShortBody {
            actual: 23,
            minimum: 65
        })
    ));
}

#[test]
fn a_single_battery_at_five_of_ten_is_fifty_percent() {
    let model = model_by_code("A3062").expect("A3062");
    let state = parse_state_body(model, &single_battery_body(5)).expect("parse");
    let single = state.single_battery.expect("single battery");
    assert_eq!(single.value, 5);
    assert_eq!(single.scale, BatteryScale::Ten);
    assert_eq!(single.scale.maximum(), 10);
    assert_eq!(single.charging, Some(false));
    assert_eq!(single.firmware.as_deref(), Some("04.39"));
}

#[test]
fn the_a3062_charging_byte_is_one_only_when_charging() {
    let model = model_by_code("A3062").expect("A3062");
    for (byte, expected) in [(1, true), (0, false), (0xFF, false), (2, false)] {
        let mut body = single_battery_body(5);
        body[1] = byte;
        let state = parse_state_body(model, &body).expect("parse");
        assert_eq!(
            state.single_battery.expect("single battery").charging,
            Some(expected),
            "charging byte {byte:#04X}"
        );
    }
}

#[test]
fn rejects_a_single_battery_above_the_ten_step_scale() {
    let model = model_by_code("A3062").expect("A3062");
    assert!(matches!(
        parse_state_body(model, &single_battery_body(11)),
        Err(StateError::BatteryAboveScale {
            value: 11,
            maximum: 10
        })
    ));
}

#[test]
fn rejects_a_short_single_battery_body() {
    let model = model_by_code("A3062").expect("A3062");
    assert!(matches!(
        parse_state_body(model, &[2, 0xFF, b'0']),
        Err(StateError::ShortBody {
            actual: 3,
            minimum: 23
        })
    ));
}

#[test]
fn keeps_hundred_step_values_for_a3954() {
    let body = synthetic_body(0, 1, 100, 75, [0, 1], *b"01.02", *b"03.04", [0; 16]);
    let state = parse_state_body(A3954, &body).expect("parse");
    let battery = state.battery.as_ref().expect("A3954 is a TWS layout");
    assert_eq!(battery.scale, BatteryScale::Hundred);
    assert_eq!((battery.left, battery.right), (100, 75));
    assert!(battery.right_charging);
}

#[test]
fn rejects_a_battery_above_the_scale() {
    let body = synthetic_body(0, 1, 6, 4, [0, 0], *b"01.02", *b"03.04", [0; 16]);
    let model = model_by_code("A3949").expect("A3949");
    assert!(matches!(
        parse_state_body(model, &body),
        Err(StateError::BatteryAboveScale {
            value: 6,
            maximum: 5
        })
    ));
}

#[test]
fn rejects_a_short_body() {
    let model = model_by_code("A3949").expect("A3949");
    assert!(matches!(
        parse_state_body(model, &[0, 1, 5, 4]),
        Err(StateError::ShortBody { .. })
    ));
}

#[test]
fn single_battery_layout_is_only_implemented_for_the_measured_a3062() {
    // The layout was measured on A3062 only; A3035/A3040 share the enum but
    // their frames were not captured on this branch. They are rejected before
    // parsing, so they keep the earlier behaviour: no single-battery state and
    // no charging flag.
    let a3062_body = single_battery_body(5);
    for code in ["A3035", "A3040"] {
        let model = model_by_code(code).expect("the catalog lists the model");
        assert_eq!(model.layout, Layout::SingleCmm2, "{code}");
        assert!(
            matches!(
                parse_state_body(model, &a3062_body),
                Err(StateError::UnsupportedLayout(Layout::SingleCmm2))
            ),
            "{code} has no measured single-battery frame"
        );
    }

    let a3062 = model_by_code("A3062").expect("A3062");
    let state = parse_state_body(a3062, &a3062_body).expect("parse");
    assert!(state.single_battery.is_some());
    assert_eq!(
        state.single_battery.expect("single battery").charging,
        Some(false)
    );
}

#[test]
fn model_registry_is_explicit() {
    assert!(model_by_code("A3949").is_some());
    assert!(model_by_code("A3954").is_some());
    assert!(model_by_code("D1203").is_none());
    assert_eq!(A3954.battery_scale.maximum(), 100);
}

#[allow(clippy::too_many_arguments)]
fn synthetic_body(
    host: u8,
    tws: u8,
    left: u8,
    right: u8,
    charging: [u8; 2],
    fw_left: [u8; 5],
    fw_right: [u8; 5],
    serial: [u8; 16],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(33);
    body.extend_from_slice(&[host, tws, left, right, charging[0], charging[1]]);
    body.extend_from_slice(&fw_left);
    body.extend_from_slice(&fw_right);
    body.extend_from_slice(&serial);
    body.push(0);
    debug_assert_eq!(body.len(), 33);
    body
}

/// The measured A3062 prefix, mode byte and a valid settings block: battery,
/// the charging byte (`0xFF`, not charging), single firmware, serial, the mode
/// at the capability's measured offset, and the remaining settings through
/// body 86.
fn single_battery_body(battery: u8) -> Vec<u8> {
    let mode = SoundMode::NoiseCanceling;
    let model = model_by_code("A3062").expect("A3062");
    let offset = model
        .sound_mode
        .expect("A3062 has a measured sound mode")
        .state_offset;
    // The settings block (body 67..87) must parse: no double press, all three
    // cycle modes, auto power off disabled with the 30 minute index, limit high
    // volume off at the lowest dB limit with real-time refresh, all flags off.
    // The measured values are covered in settings.rs.
    let mut body = vec![0; 87];
    body[0] = battery;
    body[1] = 0xFF;
    body[2..7].copy_from_slice(b"04.39");
    body[offset] = mode.value();
    body[67] = NO_BUTTON_ACTION;
    body[68] = AmbientSoundModeCycle {
        noise_canceling: true,
        transparency: true,
        normal: true,
    }
    .byte();
    body[83] = DbLimit::MIN;
    body
}
