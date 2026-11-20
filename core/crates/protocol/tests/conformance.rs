//! Golden corpus from the sanitized research artifacts.
//!
//! The JSON contains 503 full frames collected from application captures and
//! live A3062 traffic. It has no device identifiers; if parsing fails here the
//! framing implementation is wrong.

use serde::Deserialize;
use soundcore_protocol::{Command, Direction, checksum, parse};

#[derive(Debug, Deserialize)]
struct Corpus {
    count: usize,
    frames: Vec<FrameJson>,
}

#[derive(Debug, Deserialize)]
struct FrameJson {
    label: String,
    #[allow(dead_code)]
    source: String,
    frame_hex: String,
}

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex"))
        .collect()
}

#[test]
fn parses_the_entire_corpus() {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../../data/conformance.json")).expect("corpus json");
    assert_eq!(corpus.frames.len(), 503);
    assert_eq!(corpus.count, 503);

    for fixture in &corpus.frames {
        let raw = hex(&fixture.frame_hex);
        let frame = parse(&raw)
            .unwrap_or_else(|error| panic!("{}: {error} ({})", fixture.label, fixture.frame_hex));
        assert_eq!(frame.raw, raw, "{}", fixture.label);
        assert_eq!(
            frame.direction,
            Direction::Outbound,
            "application builders produce outbound frames: {}",
            fixture.label
        );
        assert!(
            !fixture.label.is_empty(),
            "every frame carries a label for triage"
        );
        assert_eq!(
            frame.raw.len(),
            usize::from(u16::from_le_bytes([frame.raw[7], frame.raw[8]])),
            "{}",
            fixture.label
        );
        assert_eq!(
            checksum(&frame.raw[..frame.raw.len() - 1]),
            frame.raw[frame.raw.len() - 1],
            "{}",
            fixture.label
        );
    }
}

#[test]
fn corpus_covers_the_documented_groups() {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../../data/conformance.json")).expect("corpus json");
    let mut groups = std::collections::BTreeSet::new();
    for fixture in &corpus.frames {
        let raw = hex(&fixture.frame_hex);
        let frame = parse(&raw).expect("parse");
        groups.insert(frame.command.group);
    }
    for group in [0x01, 0x02, 0x04, 0x06, 0x0B, 0x10, 0x20, 0x41] {
        assert!(groups.contains(&group), "group {group:02X} missing");
    }
    assert_eq!(
        Command::STATE.bytes(),
        [0x01, 0x01],
        "command constants must stay stable"
    );
}
