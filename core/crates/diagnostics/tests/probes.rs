use soundcore_diagnostics::{DiagnosticSession, PROBES, ProbeId, ProbeResult, ProbeStatus, hex};
use soundcore_protocol::{Direction, OUTBOUND_PREFIX};

#[test]
fn every_probe_is_read_only() {
    assert_eq!(PROBES.len(), 3);
    for probe in PROBES {
        assert!(probe.read_only, "{:?} must be read-only", probe.id);
    }
    assert_eq!(PROBES[0].id, ProbeId::State);
    assert_eq!(PROBES[1].id, ProbeId::Battery);
    assert_eq!(PROBES[2].id, ProbeId::Charging);
}

#[test]
fn probe_requests_are_stable() {
    assert_eq!(PROBES[0].request_hex(), "08 EE 00 00 00 01 01 0A 00 02");
    assert_eq!(PROBES[1].request_hex(), "08 EE 00 00 00 01 03 0A 00 04");
    assert_eq!(PROBES[2].request_hex(), "08 EE 00 00 00 01 04 0A 00 05");
}

#[test]
fn manifest_counts_probes_and_frames_without_panicking_on_bad_frames() {
    let mut session = DiagnosticSession {
        notes: "user toggled ANC".to_owned(),
        ..DiagnosticSession::default()
    };
    session.record_probe(ProbeResult {
        id: ProbeId::State,
        request_hex: PROBES[0].request_hex(),
        response_hex: None,
        status: ProbeStatus::Timeout,
        elapsed_ms: 4_001,
    });

    let valid = soundcore_protocol::encode(
        Direction::Inbound,
        soundcore_protocol::Command::STATE,
        &[1, 1, 5, 4],
    );
    session.record_raw_frame(10, Direction::Inbound, &valid);
    session.record_raw_frame(20, Direction::Inbound, &[0x09, 0xFF, 0x00]);

    let manifest = session.manifest_json();
    assert!(manifest.contains("\"probeCount\": 1"));
    assert!(manifest.contains("\"frameCount\": 2"));
    assert!(manifest.contains("\"checksumOk\": false"));
    assert!(manifest.contains("user toggled ANC"));
}

#[test]
fn hex_is_uppercase_and_space_separated() {
    assert_eq!(hex(&[0x08, 0xEE, 0x00]), "08 EE 00");
    assert_eq!(OUTBOUND_PREFIX[0], 0x08);
}
