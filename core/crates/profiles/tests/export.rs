//! Tests for exporting saved profiles in the OpenSCL4 transfer format.

use soundcore_models::{EQ_BAND_MAX, EQ_BAND_MIN, VISIBLE_EQ_BANDS};
use soundcore_profiles::{EqualizerProfile, ImportStatus, ProfileSet, export_json, preview_import};

/// A distinctive curve within the measured A3062 band range.
const WARM_BANDS: [i16; VISIBLE_EQ_BANDS] = [30, -30, 60, 0, -60, 20, 0, -20];
/// The neutral curve.
const FLAT_BANDS: [i16; VISIBLE_EQ_BANDS] = [0; VISIBLE_EQ_BANDS];

fn profile(name: &str, bands: [i16; VISIBLE_EQ_BANDS]) -> EqualizerProfile {
    EqualizerProfile::new(name, bands).expect("a valid test profile")
}

/// The one-decimal dB text for a raw dB×10 band, built without floating point.
/// This is the oracle for the exporter's exact-one-decimal contract.
fn one_decimal(band: i16) -> String {
    let sign = if band < 0 { "-" } else { "" };
    let magnitude = band.unsigned_abs();
    format!("{sign}{}.{}", magnitude / 10, magnitude % 10)
}

/// One transfer-profile file carrying a single profile with the given values.
fn single_profile_file(name: &str, values: &str) -> String {
    format!(r#"[{{"name":{name:?},"volumeAdjustments":[{values}]}}]"#)
}

#[test]
fn exports_the_openscl4_transfer_format() {
    let profiles = [profile("Warm", WARM_BANDS), profile("Flat", FLAT_BANDS)];

    let json = export_json(&profiles);
    assert_eq!(
        json,
        concat!(
            r#"[{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]},"#,
            r#"{"name":"Flat","volumeAdjustments":[0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]}]"#,
        ),
        "one compact array, the selection order, camelCase fields and exactly one decimal"
    );
    assert_eq!(
        export_json(&profiles),
        json,
        "serialization is deterministic"
    );
}

#[test]
fn an_empty_selection_exports_an_empty_array() {
    let json = export_json(&[]);
    assert_eq!(json, "[]");

    let preview = preview_import(&json, &ProfileSet::new()).expect("an empty profiles array");
    assert!(
        preview.is_empty(),
        "the empty export roundtrips as an empty file"
    );
    assert!(!preview.is_blocked());
}

#[test]
fn exports_negative_flat_and_extreme_values_with_exactly_one_decimal() {
    let edges = [EQ_BAND_MIN, EQ_BAND_MAX, -1, 1, -119, 119, 0, -100];

    let json = export_json(&[profile("Edges", edges)]);
    assert_eq!(
        json,
        concat!(
            r#"[{"name":"Edges","volumeAdjustments":"#,
            r#"[-12.0,13.4,-0.1,0.1,-11.9,11.9,0.0,-10.0]}]"#,
        )
    );
}

#[test]
fn every_measured_band_value_exports_with_exactly_one_decimal() {
    for band in EQ_BAND_MIN..=EQ_BAND_MAX {
        let mut bands = FLAT_BANDS;
        bands[0] = band;
        let json = export_json(&[profile("Band", bands)]);
        let value = one_decimal(band);
        assert_eq!(
            json,
            single_profile_file("Band", &format!("{value},0.0,0.0,0.0,0.0,0.0,0.0,0.0")),
            "{band} dB×10 must export as {value} dB"
        );
    }
}

#[test]
fn exported_json_roundtrips_through_preview_import_as_new_profiles() {
    let edges = [EQ_BAND_MIN, EQ_BAND_MAX, -1, 1, -119, 119, 0, -100];
    let profiles = [
        profile("Warm", WARM_BANDS),
        profile("Flat", FLAT_BANDS),
        profile("Edges", edges),
    ];

    let json = export_json(&profiles);
    let preview =
        preview_import(&json, &ProfileSet::new()).expect("the export is a transfer-profile file");
    assert!(!preview.is_blocked());
    assert_eq!(preview.len(), profiles.len());
    for (item, profile) in preview.items().iter().zip(&profiles) {
        assert_eq!(item.name, profile.name);
        assert_eq!(item.status, ImportStatus::New);
        assert_eq!(item.band_count, VISIBLE_EQ_BANDS);
        assert_eq!(
            item.bands,
            Some(profile.bands),
            "bands survive the roundtrip"
        );
    }

    let mut saved = ProfileSet::new();
    preview
        .merge_into(&mut saved)
        .expect("an importable preview merges");
    let names: Vec<&str> = saved.profiles().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Warm", "Flat", "Edges"]);
    for profile in &profiles {
        assert_eq!(
            saved
                .get(&profile.name)
                .expect("roundtripped profile")
                .bands,
            profile.bands
        );
    }
}

#[test]
fn exported_json_roundtrips_through_preview_import_as_replacements() {
    let mut saved = ProfileSet::new();
    saved.add(profile("Warm", FLAT_BANDS)).expect("Warm");
    saved.add(profile("Keep", FLAT_BANDS)).expect("Keep");

    let json = export_json(&[profile("Warm", WARM_BANDS)]);
    let preview = preview_import(&json, &saved).expect("exported profiles");
    assert_eq!(preview.items()[0].status, ImportStatus::ReplacesExisting);
    preview
        .merge_into(&mut saved)
        .expect("a replacement import merges");
    assert_eq!(
        saved.get("Warm").expect("Warm").bands,
        WARM_BANDS,
        "the exported bands replace the saved ones"
    );
    assert_eq!(
        saved.get("Keep").expect("Keep").bands,
        FLAT_BANDS,
        "profiles that were not exported keep their bands"
    );

    let all = saved.profiles().to_vec();
    let json = export_json(&all);
    let preview = preview_import(&json, &saved).expect("the saved set exports");
    assert!(
        preview
            .items()
            .iter()
            .all(|item| item.status == ImportStatus::ReplacesExisting),
        "exporting a saved set and previewing it against itself replaces every profile"
    );
    preview.merge_into(&mut saved).expect("a stable replace");
    assert_eq!(
        export_json(saved.profiles()),
        json,
        "export-preview-merge-export does not change the profiles or their order"
    );
}

#[test]
fn escapes_names_and_roundtrips_them_through_the_parser() {
    let name = "Bass \"boost\"\nЖ\\";
    let json = export_json(&[profile(name, WARM_BANDS)]);

    assert!(
        !json.contains('\n'),
        "a name newline is escaped so the export stays one compact line"
    );
    let preview = preview_import(&json, &ProfileSet::new()).expect("valid escaped JSON");
    let item = &preview.items()[0];
    assert_eq!(item.name, name);
    assert_eq!(item.bands, Some(WARM_BANDS));
}
