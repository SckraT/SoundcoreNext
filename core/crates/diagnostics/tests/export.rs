use std::io::{Cursor, Read};

use soundcore_diagnostics::{
    DiagnosticSession, ProbeId, ProbeResult, ProbeStatus, export::export_zip,
};
use soundcore_protocol::{Command, Direction, encode, parse};

fn a3949_frame() -> Vec<u8> {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    value["frame_hex"]
        .as_str()
        .expect("hex")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("hex"))
        .collect()
}

fn read_zip(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut file = archive.by_name(name).expect("entry");
    let mut text = String::new();
    file.read_to_string(&mut text).expect("read");
    text
}

#[test]
fn exports_manifest_frames_and_readme() {
    let model = soundcore_models::model_by_code("A3949").expect("model");
    let mut session = DiagnosticSession::new(model);
    session.record_probe(ProbeResult {
        id: ProbeId::State,
        request_hex: "08 EE 00 00 00 01 01 0A 00 02".to_owned(),
        response_hex: None,
        status: ProbeStatus::Ok,
        elapsed_ms: 12,
    });
    session.record_raw_frame(12, Direction::Inbound, &a3949_frame());
    session.notes = "user toggled ANC".to_owned();

    let bytes = export_zip(&session).expect("export");
    assert!(bytes.len() > 100);

    let manifest = read_zip(&bytes, "manifest.json");
    assert!(manifest.contains("\"schema\": 1"));
    assert!(manifest.contains("\"model\": \"A3949\""));
    assert!(manifest.contains("\"probeCount\": 1"));
    assert!(manifest.contains("\"frameCount\": 1"));

    let frames = read_zip(&bytes, "frames.jsonl");
    let line: serde_json::Value = serde_json::from_str(frames.trim()).expect("frame json");
    let hex = line["hex"].as_str().expect("hex");
    let raw: Vec<u8> = hex
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("hex"))
        .collect();
    let frame = parse(&raw).expect("valid frame");
    assert_eq!(frame.command, Command::STATE);
    assert!(frame.body[16..33].iter().all(|byte| *byte == 0));
    assert!(!frames.contains("0123456789ABCDEF"));

    let readme = read_zip(&bytes, "README.txt");
    assert!(readme.contains("redacted"));
}

#[test]
fn export_does_not_change_the_session() {
    let model = soundcore_models::model_by_code("A3949").expect("model");
    let mut session = DiagnosticSession::new(model);
    session.record_raw_frame(0, Direction::Inbound, &a3949_frame());
    let before = session.frames.clone();
    let _ = export_zip(&session).expect("export");
    assert_eq!(session.frames, before);
}

#[test]
fn malformed_frames_are_kept_as_errors() {
    let model = soundcore_models::model_by_code("A3949").expect("model");
    let mut session = DiagnosticSession::new(model);
    let bad = encode(Direction::Inbound, Command::BATTERY, &[5, 4]);
    let mut truncated = bad;
    truncated.truncate(8);
    session.record_raw_frame(1, Direction::Inbound, &truncated);
    let manifest = session.manifest_json();
    assert!(manifest.contains("\"checksumOk\": false"));
}

#[test]
fn redacts_the_a3062_serial_before_export() {
    let model = soundcore_models::model_by_code("A3062").expect("model");
    let mut body = vec![0u8; 95];
    body[..7].copy_from_slice(&[2, 0xFF, b'0', b'4', b'.', b'3', b'9']);
    body[7..23].copy_from_slice(b"0123456789ABCDEF");
    let mut session = DiagnosticSession::new(model);
    session.record_raw_frame(
        10,
        Direction::Inbound,
        &encode(Direction::Inbound, Command::STATE, &body),
    );

    let bytes = export_zip(&session).expect("export");
    let frames = read_zip(&bytes, "frames.jsonl");
    assert!(!frames.contains("0123456789ABCDEF"), "the serial is gone");
    let line: serde_json::Value = serde_json::from_str(frames.trim()).expect("frame json");
    let raw: Vec<u8> = line["hex"]
        .as_str()
        .expect("hex")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("hex"))
        .collect();
    let frame = parse(&raw).expect("valid frame");
    assert!(frame.body[7..23].iter().all(|byte| *byte == 0));
}
