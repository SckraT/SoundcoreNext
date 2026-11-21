//! Equalizer state, preset catalog and command-body tests.
//!
//! The fixture frames are real `01 01` answers of a Space One Pro captured over
//! the Windows RFCOMM transport after this app's `03 87` preset writes; the
//! serial bytes are zero and the checksums are recomputed.

use soundcore_models::{
    A3062_EQUALIZER_PRESETS, CUSTOM_EQUALIZER_PRESET_ID, EQ_BANDS, EQUALIZER_COMMAND_BODY_LEN,
    EqualizerPreset, EqualizerState, SoundMode, StateError, VISIBLE_EQ_BANDS,
    a3062_equalizer_preset, model_by_code, parse_state_body, parse_state_frame,
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
        let frame = encode(Direction::Outbound, Command::EQUALIZER, &body);
        assert_eq!(frame_hex(&frame), expected_frame, "preset {id}");
    }
}

#[test]
fn builds_the_exact_a3062_custom_body_and_frame() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    // Every custom write starts from the state the device reported, whose
    // HearID block is copied into the body.
    let current = a3062_state(&fixture.presets[0])
        .equalizer
        .expect("the parsed equalizer");
    // The distinctive measured curve encoded as raw dB×10 units:
    // +3.0, -3.0, +6.0, 0, -6.0, +2.0, 0, -2.0 dB.
    let curve = [30, -30, 60, 0, -60, 20, 0, -20];

    let body = model
        .custom_equalizer_body(&current, curve)
        .expect("the A3062 custom body");
    assert_eq!(body.len(), EQUALIZER_COMMAND_BODY_LEN);
    assert_eq!(
        frame_hex(&body),
        "FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 \
         00 00 00 00 FF FF FF FF FF FF FF FF FF FF 96 5A B4 78 3C 8C 78 64 78 00 00",
        "the 53-byte custom equalizer body"
    );
    assert_eq!(
        u16::from_le_bytes([body[0], body[1]]),
        CUSTOM_EQUALIZER_PRESET_ID,
        "a custom write reports preset id 0xFEFE"
    );
    // Band bytes are value + 120. The last two bands are the measured fixed
    // ones, [0, -120] raw, and not whatever the current state reports.
    assert_eq!(
        &body[4..14],
        &[0x96, 0x5A, 0xB4, 0x78, 0x3C, 0x8C, 0x78, 0x64, 0x78, 0x00]
    );
    assert_eq!(&body[42..52], &body[4..14], "the active curve is repeated");

    let frame = encode(Direction::Outbound, Command::EQUALIZER, &body);
    assert_eq!(
        frame_hex(&frame),
        "08 EE 00 00 00 03 87 3F 00 FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 00 00 00 00 \
         FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF \
         96 5A B4 78 3C 8C 78 64 78 00 00 17",
        "the exact 03 87 frame"
    );
    assert_eq!(frame.len(), 63);
}

#[test]
fn clamps_custom_bands_to_the_measured_range() {
    let fixture = fixture();
    let model = model_by_code("A3062").expect("A3062");
    let current = a3062_state(&fixture.presets[0])
        .equalizer
        .expect("the parsed equalizer");

    // Values at and beyond both ends of the measured -120..=134 range.
    let body = model
        .custom_equalizer_body(&current, [135, i16::MAX, -121, i16::MIN, 134, -120, 0, -10])
        .expect("the A3062 custom body");
    assert_eq!(
        &body[4..14],
        &[254, 254, 0, 0, 254, 0, 120, 110, 120, 0],
        "out-of-range bands clamp to -120..=134 and then add 120"
    );
    assert_eq!(&body[42..52], &body[4..14]);
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

    // A custom write preserves the same HearID fields and clears the flag.
    let custom = model
        .custom_equalizer_body(&current, [10, 20, 30, 40, 50, 60, 70, 80])
        .expect("the A3062 custom body");
    assert_eq!(&custom[0..2], &CUSTOM_EQUALIZER_PRESET_ID.to_le_bytes());
    assert_eq!(&custom[2..4], &body[2..4]);
    assert_eq!(&custom[17..27], &body[17..27]);
    assert_eq!(&custom[27..31], &body[27..31]);
    assert_eq!(custom[31], 2);
    assert_eq!(&custom[32..42], &body[32..42]);
    assert_eq!(custom[16], 0, "HearID is disabled for a custom curve too");
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
    assert_eq!(
        equalizer.visible_bands(),
        [-120, -110, -100, -90, -80, -70, -60, -50],
        "only the first eight of the ten parsed bands are visible"
    );

    // Rebuilding the custom body from this state keeps its visible curve but
    // writes the measured fixed invisible bands, not the -40/-30 the crafted
    // state carries.
    let body = model
        .custom_equalizer_body(&equalizer, equalizer.visible_bands())
        .expect("the A3062 custom body");
    assert_eq!(
        &body[4..14],
        &[0, 10, 20, 30, 40, 50, 60, 70, 120, 0],
        "invisible bands are fixed at 0 dB and -12.0 dB"
    );
}

#[derive(serde::Deserialize)]
struct CustomFixture {
    preset_offset: usize,
    bands_offset: usize,
    custom_preset_id: u16,
    visible_bands: Vec<i16>,
    bands: Vec<i16>,
    body_hex: String,
    frame_hex: String,
    before_frame_hex: String,
    custom_frame_hex: String,
    restored_frame_hex: String,
}

fn custom_fixture() -> CustomFixture {
    serde_json::from_str(include_str!("../../../data/a3062-eq-custom.json"))
        .expect("custom equalizer fixture")
}

fn fixture_state(frame_hex: &str) -> soundcore_models::DeviceState {
    let model = model_by_code("A3062").expect("A3062");
    parse_state_frame(model, &hex(frame_hex)).expect("parse the real state")
}

#[test]
fn parses_the_real_a3062_custom_equalizer_fixture() {
    let fixture = custom_fixture();
    let model = model_by_code("A3062").expect("A3062");
    let capability = model.equalizer.expect("A3062 has a measured equalizer");
    assert_eq!(capability.state_offset, fixture.preset_offset);
    assert_eq!(capability.state_offset + 2, fixture.bands_offset);

    // The state found on connect was a built-in preset.
    let before_state = fixture_state(&fixture.before_frame_hex);
    let before = before_state.equalizer.expect("the found A3062 equalizer");
    assert!(!before.is_custom());
    assert_eq!(before.preset().map(|preset| preset.id), Some(0));
    assert_eq!(before.visible_bands(), [0; VISIBLE_EQ_BANDS]);

    // The body and frame built from that state are exactly the bytes the device
    // accepted.
    let visible: [i16; VISIBLE_EQ_BANDS] = fixture
        .visible_bands
        .clone()
        .try_into()
        .expect("the fixture records eight visible bands");
    let body = model
        .custom_equalizer_body(&before, visible)
        .expect("the A3062 custom body");
    assert_eq!(frame_hex(&body), fixture.body_hex);
    let frame = encode(Direction::Outbound, Command::EQUALIZER, &body);
    assert_eq!(frame_hex(&frame), fixture.frame_hex);

    // The re-polled state reports the custom preset and exactly the written
    // curve; the two invisible bands are the measured fixed ones.
    let custom_state = fixture_state(&fixture.custom_frame_hex);
    assert_eq!(custom_state.serial, None, "the fixture serial is masked");
    assert_eq!(custom_state.sound_mode, Some(SoundMode::Normal));
    let custom = custom_state.equalizer.expect("the custom A3062 equalizer");
    assert_eq!(custom.preset_id, fixture.custom_preset_id);
    assert_eq!(custom.preset_id, CUSTOM_EQUALIZER_PRESET_ID);
    assert!(custom.is_custom());
    assert!(custom.preset().is_none());
    assert_eq!(custom.bands.to_vec(), fixture.bands);
    assert_eq!(custom.visible_bands().to_vec(), fixture.visible_bands);

    // The restored state is the state found on connect, so the device was left
    // as found.
    let restored_state = fixture_state(&fixture.restored_frame_hex);
    assert_eq!(restored_state, before_state);
}

#[test]
fn a3949_equalizer_is_explicitly_deferred() {
    // A3949 equalizer support is deliberately out of this slice: no capability,
    // no catalog and no write body, so the page stays hidden and no `03 87` is
    // sent. This is a limitation, not evidence that the model has no equalizer.
    let model = model_by_code("A3949").expect("A3949");
    assert!(model.equalizer.is_none());
    assert!(model.equalizer_presets().is_empty());

    // The committed real A3949 capture does hold an equalizer block: right
    // after the 16-byte serial, body 32..34 is the preset id and body 34..44 is
    // the same ten-band, value-minus-120 curve A3062 uses (nine 0 dB bands and
    // a -12.0 dB last band on this capture). OpenSCL4's A3949 state parser reads
    // the same block; only the write body is unverified.
    let a3949_fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/a3949-state.json")).expect("fixture");
    let frame = hex(a3949_fixture["frame_hex"].as_str().expect("frame_hex"));
    let body = &frame[9..frame.len() - 1];
    assert_eq!(
        u16::from_le_bytes([body[32], body[33]]),
        0,
        "the A3949 capture reports preset id 0"
    );
    assert_eq!(
        &body[34..44],
        &[0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00],
        "the A3949 capture's ten band bytes are the flat curve"
    );

    // Because the capability is deliberately absent, the parser does not invent
    // A3949 offsets and reports no equalizer for the same capture.
    let state = parse_state_frame(model, &frame).expect("parse the P20i state");
    assert!(
        state.equalizer.is_none(),
        "A3949 equalizer parsing is deferred"
    );

    // The write body also refuses a deferred model, even when handed a parsed
    // A3062 state.
    let eq_fixture = fixture();
    let current: EqualizerState = a3062_state(&eq_fixture.presets[0])
        .equalizer
        .expect("the parsed A3062 equalizer");
    let preset: &EqualizerPreset = a3062_equalizer_preset(3).expect("a built-in preset");
    assert!(model.equalizer_preset_body(&current, preset).is_none());
    assert!(
        model
            .custom_equalizer_body(&current, [0; VISIBLE_EQ_BANDS])
            .is_none(),
        "the deferred model has no custom write body either"
    );
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
