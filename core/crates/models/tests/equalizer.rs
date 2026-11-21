//! Equalizer state, preset catalog and command-body tests.
//!
//! The fixture frames are real `01 01` answers of a Space One Pro captured over
//! the Windows RFCOMM transport after this app's `03 87` preset writes; the
//! serial bytes are zero and the checksums are recomputed.

use soundcore_models::{
    A3062_EQUALIZER_PRESETS, CUSTOM_EQUALIZER_PRESET_ID, EQ_BANDS, EQUALIZER_COMMAND_BODY_LEN,
    EqualizerPreset, EqualizerState, SoundMode, StateError, a3062_equalizer_preset, model_by_code,
    parse_state_body, parse_state_frame,
};
use soundcore_protocol::{Command, Direction, checksum, encode};

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

#[derive(serde::Deserialize)]
struct Fixture {
    preset_offset: usize,
    bands_offset: usize,
    presets: Vec<PresetFrame>,
    restored_frame_hex: String,
}

#[derive(serde::Deserialize)]
struct PresetFrame {
    name: String,
    id: u16,
    bands: Vec<i16>,
    frame_hex: String,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../../../data/a3062-eq-presets.json"))
        .expect("equalizer fixture")
}

impl PresetFrame {
    fn bands(&self) -> [i16; EQ_BANDS] {
        self.bands
            .clone()
            .try_into()
            .expect("the fixture records ten bands")
    }

    fn frame(&self) -> Vec<u8> {
        hex(&self.frame_hex)
    }
}

fn a3062_state(entry: &PresetFrame) -> soundcore_models::DeviceState {
    let model = model_by_code("A3062").expect("A3062");
    parse_state_frame(model, &entry.frame()).expect("parse the real state")
}

#[test]
fn parses_the_real_a3062_equalizer_state() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    let capability = model.equalizer.expect("A3062 has a measured equalizer");

    // The measured offsets are body offsets, not frame offsets.
    assert_eq!(capability.state_offset, fixture.preset_offset);
    assert_eq!(capability.state_offset + 2, fixture.bands_offset);
    assert_eq!(capability.band_count, EQ_BANDS);
    assert_eq!(capability.custom_preset_id, CUSTOM_EQUALIZER_PRESET_ID);
    assert_eq!(fixture.preset_offset, 23);
    assert_eq!(fixture.bands_offset, 25);

    for entry in &fixture.presets {
        let frame = entry.frame();
        assert_eq!(frame.len(), 105, "the measured state frame length");

        // The bytes at the fixture's declared offsets are the preset id and
        // the ten band bytes; the parsed values are those bytes minus 120.
        let body = &frame[9..frame.len() - 1];
        assert_eq!(
            u16::from_le_bytes([body[fixture.preset_offset], body[fixture.preset_offset + 1]]),
            entry.id,
            "{}",
            entry.name
        );
        let expected_bands: Vec<u8> = entry
            .bands()
            .iter()
            .map(|value| u8::try_from(value + 120).expect("a measured band byte"))
            .collect();
        assert_eq!(
            &body[fixture.bands_offset..fixture.bands_offset + EQ_BANDS],
            expected_bands.as_slice(),
            "{}",
            entry.name
        );

        let state = a3062_state(entry);
        let equalizer = state.equalizer.expect("the parsed equalizer");
        assert_eq!(equalizer.preset_id, entry.id, "{}", entry.name);
        assert_eq!(equalizer.bands, entry.bands(), "{}", entry.name);
        assert_eq!(
            equalizer.preset().map(|preset| preset.name),
            Some(entry.name.as_str()),
            "a measured built-in preset is in the catalog"
        );
        assert!(!equalizer.is_custom());

        // The fixture is sanitized: the serial field is sixteen ASCII zeros
        // and the parser reports it as absent.
        assert_eq!(state.serial, None);
        assert_eq!(
            state
                .single_battery
                .as_ref()
                .and_then(|battery| battery.firmware.as_deref()),
            Some("04.39")
        );
        // The captures were taken while the device was in normal mode.
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
    }
}

#[test]
fn the_restored_frame_is_the_original_signature_preset() {
    let fixture = fixture();
    let restored = hex(&fixture.restored_frame_hex);
    let model = model_by_code("A3062").expect("A3062");
    let restored = parse_state_frame(model, &restored).expect("parse the restored state");
    let equalizer = restored.equalizer.expect("the restored equalizer");
    let original = &fixture.presets[0];
    assert_eq!(equalizer.preset_id, original.id);
    assert_eq!(equalizer.bands, original.bands());
    assert_eq!(
        equalizer.preset().map(|preset| preset.name),
        Some(original.name.as_str())
    );
}

#[test]
fn the_a3062_catalog_is_the_openscl4_preset_list() {
    assert_eq!(A3062_EQUALIZER_PRESETS.len(), 22);
    let ids: Vec<u16> = A3062_EQUALIZER_PRESETS
        .iter()
        .map(|preset| preset.id)
        .collect();
    assert_eq!(
        ids,
        [
            0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 0x7E7E,
        ],
        "OpenSCL4's A3062 list skips id 2 and puts Bass Booster at 0x7E7E"
    );
    for preset in A3062_EQUALIZER_PRESETS {
        assert!(!preset.name.is_empty());
        // The last two bands are invisible on this model: OpenSCL4 fixes them
        // at 0 and -120 raw units for every preset.
        assert_eq!(preset.bands[8], 0, "{}", preset.name);
        assert_eq!(preset.bands[9], -120, "{}", preset.name);
    }
    // Spot checks transcribed from OpenSCL4's A3062 list.
    assert_eq!(
        a3062_equalizer_preset(0).expect("signature").bands,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, -120]
    );
    assert_eq!(
        a3062_equalizer_preset(3).expect("bass reducer").bands,
        [-40, -30, -10, 0, 0, 0, 0, 0, 0, -120]
    );
    assert_eq!(
        a3062_equalizer_preset(5).expect("podcast").bands,
        [-30, 20, 40, 40, 30, 20, 0, -20, 0, -120]
    );
    assert_eq!(
        a3062_equalizer_preset(0x7E7E).expect("bass booser").name,
        "Bass Booster"
    );
    assert_eq!(
        a3062_equalizer_preset(0).expect("signature").name,
        "Soundcore Signature"
    );

    // Id 2 is not an A3062 preset and the custom preset id is not selectable.
    assert!(a3062_equalizer_preset(2).is_none());
    assert!(a3062_equalizer_preset(CUSTOM_EQUALIZER_PRESET_ID).is_none());
    assert!(
        A3062_EQUALIZER_PRESETS
            .iter()
            .all(|preset| preset.id != CUSTOM_EQUALIZER_PRESET_ID)
    );

    // The catalog is exposed on the model, and only for A3062.
    let model = model_by_code("A3062").expect("A3062");
    assert_eq!(model.equalizer_presets(), A3062_EQUALIZER_PRESETS);
    let a3949 = model_by_code("A3949").expect("A3949");
    assert!(a3949.equalizer_presets().is_empty());
}

#[test]
fn builds_the_exact_a3062_preset_body_and_frame() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    // Every measured capture was taken from the same Signature starting state;
    // its HearID block is what a preset write carries over.
    let current = a3062_state(&fixture.presets[0])
        .equalizer
        .expect("the parsed equalizer");

    let cases = [
        (
            0,
            "00 00 00 00 78 78 78 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 78 78 78 78 78 78 78 78 78 00 00",
            "08 EE 00 00 00 03 87 3F 00 00 00 00 00 78 78 78 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 78 78 78 78 78 78 78 78 78 00 00 1B",
        ),
        (
            3,
            "03 00 00 00 50 5A 6E 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 50 5A 6E 78 78 78 78 78 78 00 00",
            "08 EE 00 00 00 03 87 3F 00 03 00 00 00 50 5A 6E 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 50 5A 6E 78 78 78 78 78 78 00 00 7E",
        ),
        (
            5,
            "05 00 00 00 5A 8C A0 A0 96 8C 78 64 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 5A 8C A0 A0 96 8C 78 64 78 00 00",
            "08 EE 00 00 00 03 87 3F 00 05 00 00 00 5A 8C A0 A0 96 8C 78 64 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 5A 8C A0 A0 96 8C 78 64 78 00 00 E8",
        ),
    ];

    for (id, expected_body, expected_frame) in cases {
        let preset = a3062_equalizer_preset(id).expect("a built-in preset");
        let body = model
            .equalizer_preset_body(&current, preset)
            .expect("the A3062 preset body");
        assert_eq!(body.len(), EQUALIZER_COMMAND_BODY_LEN);
        assert_eq!(frame_hex(&body), expected_body, "preset {id}");
        let frame = encode(Direction::Outbound, Command::ACK_EQUALIZER, &body);
        assert_eq!(frame_hex(&frame), expected_frame, "preset {id}");
    }
}

#[test]
fn keeps_the_state_hear_id_fields_but_clears_the_enabled_flag() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    let frame = fixture.presets[0].frame();
    // Build a state whose HearID block differs from the defaults, with the
    // enabled flag set: genre 0x1234, time 0x01020304, type 2, volume values.
    let mut custom = frame.clone();
    let body_end = custom.len() - 1;
    let body = &mut custom[9..body_end];
    body[37] = 1; // enabled
    for (offset, value) in (38..48).zip([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]) {
        body[offset] = 200 + value;
    }
    body[48..52].copy_from_slice(&0x0102_0304u32.to_be_bytes());
    body[52] = 2;
    body[53..63].fill(100);
    body[63..65].copy_from_slice(&0x1234u16.to_le_bytes());
    let checksum_at = custom.len() - 1;
    custom[checksum_at] = checksum(&custom[..checksum_at]);

    let state = parse_state_frame(model, &custom).expect("parse the crafted state");
    let current = state.equalizer.expect("equalizer");
    let preset = a3062_equalizer_preset(3).expect("bass reducer");
    let body = model
        .equalizer_preset_body(&current, preset)
        .expect("the A3062 preset body");
    // The favorite music genre, volume adjustments, time, type and custom
    // adjustments are copied from the state...
    assert_eq!(&body[2..4], &[0x34, 0x12]);
    assert_eq!(
        &body[17..27],
        &[200, 201, 202, 203, 204, 205, 206, 207, 208, 209]
    );
    assert_eq!(&body[27..31], &[0x01, 0x02, 0x03, 0x04]);
    assert_eq!(body[31], 2);
    assert_eq!(&body[32..42], &[100; 10]);
    // ...but the enabled flag is cleared so the preset curve is in effect.
    assert_eq!(body[16], 0);
}

#[test]
fn custom_preset_id_is_not_a_built_in_preset() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    let mut frame = fixture.presets[0].frame();
    let body_end = frame.len() - 1;
    let body = &mut frame[9..body_end];
    body[23..25].copy_from_slice(&CUSTOM_EQUALIZER_PRESET_ID.to_le_bytes());
    // A custom curve: distinct from every built-in preset.
    for (offset, byte) in (25..35).zip([0, 10, 20, 30, 40, 50, 60, 70, 80, 90]) {
        body[offset] = byte;
    }
    let checksum_at = frame.len() - 1;
    frame[checksum_at] = checksum(&frame[..checksum_at]);

    let state = parse_state_frame(model, &frame).expect("parse the custom state");
    let equalizer = state.equalizer.expect("equalizer");
    assert_eq!(equalizer.preset_id, CUSTOM_EQUALIZER_PRESET_ID);
    assert!(equalizer.is_custom());
    assert!(equalizer.preset().is_none());
    assert_eq!(
        equalizer.bands,
        [-120, -110, -100, -90, -80, -70, -60, -50, -40, -30]
    );
}

#[test]
fn a3949_has_no_equalizer() {
    let model = model_by_code("A3949").expect("A3949");
    assert!(model.equalizer.is_none());
    assert!(model.equalizer_presets().is_empty());

    let a3949_fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    let frame = hex(a3949_fixture["frame_hex"].as_str().expect("frame_hex"));
    let state = parse_state_frame(model, &frame).expect("parse the P20i state");
    assert!(state.equalizer.is_none());

    // The write body also refuses an unmeasured model, even when handed a
    // parsed A3062 state.
    let eq_fixture = fixture();
    let current: EqualizerState = a3062_state(&eq_fixture.presets[0])
        .equalizer
        .expect("the parsed A3062 equalizer");
    let preset: &EqualizerPreset = a3062_equalizer_preset(3).expect("a built-in preset");
    assert!(model.equalizer_preset_body(&current, preset).is_none());
}

#[test]
fn a_truncated_a3062_body_stops_at_the_end_of_the_equalizer_block() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    let frame = fixture.presets[0].frame();
    let body = &frame[9..frame.len() - 1];
    // The measured preset and bands live in body 23..35, but a preset write
    // also needs the HearID block through body offset 65.
    assert!(matches!(
        parse_state_body(model, &body[..64]),
        Err(StateError::ShortBody {
            actual: 64,
            minimum: 65
        })
    ));
    assert!(matches!(
        parse_state_body(model, &body[..23]),
        Err(StateError::ShortBody {
            actual: 23,
            minimum: 65
        })
    ));
}

#[test]
fn a_synthetic_body_parses_band_bytes_as_value_minus_120() {
    let model = model_by_code("A3062").expect("A3062");
    let mut body = vec![0u8; 95];
    body[0] = 2;
    body[1] = 0xFF;
    body[2..7].copy_from_slice(b"04.39");
    body[23..25].copy_from_slice(&0x7E7Eu16.to_le_bytes());
    for (offset, byte) in (25..35).zip([0, 60, 120, 180, 254, 253, 1, 2, 3, 4]) {
        body[offset] = byte;
    }
    body[69] = 2;

    let state = parse_state_body(model, &body).expect("parse");
    let equalizer = state.equalizer.expect("equalizer");
    assert_eq!(equalizer.preset_id, 0x7E7E);
    assert_eq!(
        equalizer.bands,
        [-120, -60, 0, 60, 134, 133, -119, -118, -117, -116]
    );
}
