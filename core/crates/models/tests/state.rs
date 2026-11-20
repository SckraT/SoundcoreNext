//! State parser tests against real and synthetic frames.

use soundcore_models::{
    A3954, BatteryScale, HostDevice, Layout, StateError, model_by_code, parse_state_body,
    parse_state_frame,
};

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
    // The state frame has no charging byte; charging is a separate `01 04`
    // notification and is not part of this poll.
    assert_eq!(single.charging, None);
    assert_eq!(
        single.firmware.as_deref(),
        Some(fixture.expected.firmware.as_str())
    );
    assert_eq!(state.serial, fixture.expected.serial);
    assert_eq!(state.battery, None);
}

#[test]
fn a_single_battery_at_five_of_ten_is_fifty_percent() {
    let model = model_by_code("A3062").expect("A3062");
    let state = parse_state_body(model, &single_battery_body(5)).expect("parse");
    let single = state.single_battery.expect("single battery");
    assert_eq!(single.value, 5);
    assert_eq!(single.scale, BatteryScale::Ten);
    assert_eq!(single.scale.maximum(), 10);
    assert_eq!(single.firmware.as_deref(), Some("04.39"));
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
    // their frames were not captured on this branch.
    let a3035 = model_by_code("A3035").expect("A3035");
    assert_eq!(a3035.layout, Layout::SingleCmm2);
    assert!(matches!(
        parse_state_body(a3035, &[0; 64]),
        Err(StateError::UnsupportedLayout(Layout::SingleCmm2))
    ));

    let a3062 = model_by_code("A3062").expect("A3062");
    assert!(parse_state_body(a3062, &single_battery_body(5)).is_ok());
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

/// The measured A3062 prefix: battery, reserved `0xFF`, single firmware, serial.
fn single_battery_body(battery: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(23);
    body.extend_from_slice(&[battery, 0xFF]);
    body.extend_from_slice(b"04.39");
    body.extend_from_slice(&[0; 16]);
    debug_assert_eq!(body.len(), 23);
    body
}
