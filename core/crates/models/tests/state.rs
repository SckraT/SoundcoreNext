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
    assert_eq!(state.battery.left, fixture.expected.left);
    assert_eq!(state.battery.right, fixture.expected.right);
    assert_eq!(state.battery.scale.maximum(), fixture.expected.scale);
    assert!(!state.battery.left_charging);
    assert!(!state.battery.right_charging);
    assert_eq!(
        state.firmware.left.as_deref(),
        Some(fixture.expected.firmware_left.as_str())
    );
    assert_eq!(
        state.firmware.right.as_deref(),
        Some(fixture.expected.firmware_right.as_str())
    );
    assert_eq!(state.serial, fixture.expected.serial);
}

#[test]
fn keeps_hundred_step_values_for_a3954() {
    let body = synthetic_body(0, 1, 100, 75, [0, 1], *b"01.02", *b"03.04", [0; 16]);
    let state = parse_state_body(A3954, &body).expect("parse");
    assert_eq!(state.battery.scale, BatteryScale::Hundred);
    assert_eq!((state.battery.left, state.battery.right), (100, 75));
    assert!(state.battery.right_charging);
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
fn single_battery_layout_is_not_invented() {
    let model = model_by_code("A3035").expect("A3035");
    assert_eq!(model.layout, Layout::SingleCmm2);
    assert!(matches!(
        parse_state_body(model, &[0; 64]),
        Err(StateError::UnsupportedLayout(Layout::SingleCmm2))
    ));
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
