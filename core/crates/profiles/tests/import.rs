//! Tests for previewing and merging OpenSCL4 transfer-profile files.

use soundcore_models::VISIBLE_EQ_BANDS;
use soundcore_profiles::EqualizerProfile;
use soundcore_profiles::{
    ImportError, ImportItem, ImportPreview, ImportStatus, MAX_PROFILE_NAME_LEN, ProfileError,
    ProfileSet, preview_import,
};

/// A neutral curve for the saved profiles these tests start from.
fn flat() -> [i16; VISIBLE_EQ_BANDS] {
    [0; VISIBLE_EQ_BANDS]
}

/// A saved-profile set built from the given profiles, in order.
fn saved_profiles(profiles: &[(&str, [i16; VISIBLE_EQ_BANDS])]) -> ProfileSet {
    let mut set = ProfileSet::new();
    for &(name, bands) in profiles {
        set.add(EqualizerProfile::new(name, bands).expect("a valid saved profile"))
            .expect("a unique saved profile name");
    }
    set
}

/// One transfer profile, with the values written out as the file would have them.
fn one_profile(name: &str, values: &str) -> String {
    format!(r#"[{{"name":{name:?},"volumeAdjustments":[{values}]}}]"#)
}

/// The statuses in a preview, in file order.
fn statuses(preview: &ImportPreview) -> Vec<ImportStatus> {
    preview.items().iter().map(|item| item.status).collect()
}

/// The single item of a one-profile preview.
fn only_item(preview: &ImportPreview) -> &ImportItem {
    assert_eq!(preview.len(), 1, "expected exactly one preview item");
    &preview.items()[0]
}

#[test]
fn parses_the_openscl4_transfer_format_and_marks_new_profiles() {
    let text = r#"[
        {"name": "Warm", "volumeAdjustments": [0.4, -0.3, 0.5, 0.0, -6.0, 2.0, 0.0, -2.0]}
    ]"#;

    let preview = preview_import(text, &ProfileSet::new()).expect("a transfer-profile file");
    assert_eq!(preview.len(), 1);
    assert!(!preview.is_empty());
    assert!(!preview.is_blocked());

    let item = only_item(&preview);
    assert_eq!(item.name, "Warm");
    assert_eq!(item.band_count, VISIBLE_EQ_BANDS);
    assert_eq!(item.status, ImportStatus::New);
    assert_eq!(item.bands, Some([4, -3, 5, 0, -60, 20, 0, -20]));
}

#[test]
fn accepts_integer_values_and_ignores_unknown_fields() {
    let text = r#"[
        {"name": "Flat", "volumeAdjustments": [0, -3, 5, 0, 0, 0, 0, 0],
         "enabled": true, "curve": "M 0 0 L 8 0", "note": null}
    ]"#;

    let preview = preview_import(text, &ProfileSet::new()).expect("a newer export still reads");
    let item = only_item(&preview);
    assert_eq!(item.status, ImportStatus::New);
    assert_eq!(item.bands, Some([0, -30, 50, 0, 0, 0, 0, 0]));
}

#[test]
fn saved_names_make_replacements_after_trimming() {
    let mut saved = saved_profiles(&[("Warm", flat()), ("Flat", flat())]);
    let text = r#"[
        {"name": "  Warm  ", "volumeAdjustments": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]},
        {"name": "New one", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]},
        {"name": "warm", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]}
    ]"#;

    let preview = preview_import(text, &saved).expect("a transfer-profile file");
    assert_eq!(
        statuses(&preview),
        [
            ImportStatus::ReplacesExisting,
            ImportStatus::New,
            ImportStatus::New,
        ],
        "a trimmed name matches the saved profile; matching is case-sensitive"
    );
    assert_eq!(preview.items()[0].name, "Warm", "names are trimmed");
    assert!(!preview.is_blocked());

    preview
        .merge_into(&mut saved)
        .expect("an importable preview merges");
    let names: Vec<&str> = saved
        .profiles()
        .iter()
        .map(|profile| profile.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Warm", "Flat", "New one", "warm"],
        "replace keeps the saved position; new profiles are appended in file order"
    );
    assert_eq!(
        saved.get("Warm").expect("Warm").bands,
        [10, 20, 30, 40, 50, 60, 70, 80]
    );
}

#[test]
fn rounds_decibels_to_tenths_with_halves_away_from_zero() {
    let text = one_profile(
        "Rounded",
        "0.04, 0.05, -0.05, 0.15, -0.15, 0.25, -0.25, 0.0",
    );

    let preview = preview_import(&text, &ProfileSet::new()).expect("valid values");
    let item = only_item(&preview);
    assert_eq!(item.status, ImportStatus::New);
    assert_eq!(
        item.bands,
        Some([0, 1, -1, 2, -2, 3, -3, 0]),
        "dB × 10 rounded to the nearest dB×10, halves away from zero"
    );
}

#[test]
fn accepts_values_that_round_inside_the_measured_range() {
    for (value, expected) in [
        ("13.4", 134),
        ("13.44", 134),
        ("-12.0", -120),
        ("-12.04", -120),
    ] {
        let text = one_profile("Edges", &format!("0, 0, 0, 0, 0, 0, 0, {value}"));
        let preview = preview_import(&text, &ProfileSet::new()).expect("valid values");
        let item = only_item(&preview);
        assert_eq!(
            item.status,
            ImportStatus::New,
            "{value} dB is inside the range"
        );
        assert_eq!(
            item.bands.expect("an importable item has bands")[7],
            expected,
            "{value} dB rounds to {expected} dB×10"
        );
    }
}

#[test]
fn out_of_range_values_are_blocked_after_rounding() {
    let mut saved = ProfileSet::new();
    for (value, rounded) in [
        ("13.5", 135),
        ("-12.1", -121),
        ("13.45", 135),
        ("-12.05", -121),
        ("1000000", 10000000),
    ] {
        let text = one_profile("Loud", &format!("0, 0, 0, 0, 0, 0, 0, {value}"));
        let preview = preview_import(&text, &saved).expect("numeric values parse");
        let item = only_item(&preview);
        assert_eq!(
            item.status,
            ImportStatus::OutOfRange,
            "{value} dB rounds to {rounded} dB×10"
        );
        assert_eq!(item.band_count, VISIBLE_EQ_BANDS);
        assert_eq!(item.bands, None, "a blocked item carries no bands to merge");
        assert!(preview.is_blocked());
        assert!(
            matches!(preview.merge_into(&mut saved), Err(ImportError::Blocked)),
            "an out-of-range preview must not merge"
        );
        assert!(saved.is_empty(), "a blocked preview must not write");
    }
}

#[test]
fn wrong_band_counts_are_blocked_and_listed_with_their_count() {
    let mut saved = saved_profiles(&[("Warm", flat())]);
    let before = saved.clone();
    for (values, count) in [
        ("0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0", 7),
        ("0, 0, 0, 0, 0, 0, 0, 0, 0", 9),
        ("", 0),
    ] {
        let text = one_profile("Warm", values);
        let preview =
            preview_import(&text, &saved).expect("a wrong count is still a transfer profile");
        let item = only_item(&preview);
        assert_eq!(item.band_count, count, "{values:?}");
        assert_eq!(item.status, ImportStatus::WrongBandCount, "{values:?}");
        assert_eq!(item.bands, None);
        assert!(preview.is_blocked());
        assert!(matches!(
            preview.merge_into(&mut saved),
            Err(ImportError::Blocked)
        ));
        assert_eq!(saved, before, "a blocked preview must not change the set");
    }
}

#[test]
fn the_band_count_is_checked_before_the_values() {
    let text = one_profile("Short and loud", "0, 0, 0, 13.5, 0, 0, 0");

    let preview = preview_import(&text, &ProfileSet::new()).expect("a transfer profile");
    assert_eq!(
        only_item(&preview).status,
        ImportStatus::WrongBandCount,
        "OpenSCL4 checks the count before the values"
    );
}

#[test]
fn a_blocked_import_never_applies_its_valid_profiles() {
    let mut saved = saved_profiles(&[("Warm", flat()), ("Flat", flat())]);
    let before = saved.clone();
    let text = r#"[
        {"name": "Warm", "volumeAdjustments": [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]},
        {"name": "New one", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]},
        {"name": "Short", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0]}
    ]"#;

    let preview = preview_import(text, &saved).expect("names and values parse");
    assert_eq!(
        statuses(&preview),
        [
            ImportStatus::ReplacesExisting,
            ImportStatus::New,
            ImportStatus::WrongBandCount,
        ]
    );
    assert!(preview.is_blocked());
    assert!(matches!(
        preview.merge_into(&mut saved),
        Err(ImportError::Blocked)
    ));
    assert_eq!(
        saved, before,
        "the valid profiles of a blocked import are not applied either"
    );
}

#[test]
fn an_empty_array_previews_and_merges_nothing() {
    let mut saved = saved_profiles(&[("Warm", flat())]);
    let before = saved.clone();

    for text in ["[]", " \n\t [] \n"] {
        let preview = preview_import(text, &saved).expect("an empty profiles array");
        assert!(preview.is_empty());
        assert_eq!(preview.len(), 0);
        assert!(!preview.is_blocked());
        preview
            .merge_into(&mut saved)
            .expect("merging nothing succeeds");
        assert_eq!(saved, before);
    }
}

#[test]
fn text_that_is_not_a_profile_array_is_not_profiles() {
    let saved = ProfileSet::new();
    for text in [
        "",
        "   \n\t ",
        "hello",
        "{}",
        r#"{"profiles": []}"#,
        "null",
        "42",
        "[1]",
        r#"[{"name": "No values"}]"#,
        r#"[{"volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]}]"#,
        r#"[{"name": "Strings", "volumeAdjustments": ["0", "0", "0", "0", "0", "0", "0", "0"]}]"#,
        r#"[{"name": 7, "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]}]"#,
        r#"[{"name": "Null bands", "volumeAdjustments": null}]"#,
        r#"[{"name": "Not an object"}]"#,
        r#"[{"name": "Mismatched", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]}"#,
    ] {
        assert!(
            matches!(
                preview_import(text, &saved),
                Err(ImportError::NotProfiles(_))
            ),
            "{text:?} must not parse as a profiles file"
        );
    }
}

#[test]
fn repeated_names_are_rejected() {
    let text = r#"[
        {"name": "Same", "volumeAdjustments": [0, 0, 0, 0, 0, 0, 0, 0]},
        {"name": " Same ", "volumeAdjustments": [1, 1, 1, 1, 1, 1, 1, 1]}
    ]"#;

    match preview_import(text, &ProfileSet::new()) {
        Err(ImportError::DuplicateName { name }) => assert_eq!(name, "Same"),
        other => panic!("expected a duplicate-name error, got {other:?}"),
    }
}

#[test]
fn invalid_names_are_rejected() {
    for name in [
        String::new(),
        " \t\r\n ".to_owned(),
        "x".repeat(MAX_PROFILE_NAME_LEN + 1),
    ] {
        let text = one_profile(&name, "0, 0, 0, 0, 0, 0, 0, 0");
        match preview_import(&text, &ProfileSet::new()) {
            Err(ImportError::InvalidName { index: 0, source }) => match source {
                ProfileError::EmptyName => assert!(name.trim().is_empty(), "{name:?}"),
                ProfileError::NameTooLong { actual, max } => {
                    assert_eq!(actual, MAX_PROFILE_NAME_LEN + 1);
                    assert_eq!(max, MAX_PROFILE_NAME_LEN);
                }
                other => panic!("unexpected name error for {name:?}: {other:?}"),
            },
            other => panic!("expected an invalid-name error for {name:?}, got {other:?}"),
        }
    }
}

#[test]
fn accepts_a_name_of_the_saved_profile_maximum_length() {
    let name = "y".repeat(MAX_PROFILE_NAME_LEN);
    let text = one_profile(&name, "0, 0, 0, 0, 0, 0, 0, 0");

    let preview = preview_import(&text, &ProfileSet::new()).expect("the longest allowed name");
    let item = only_item(&preview);
    assert_eq!(item.name, name);
    assert_eq!(item.status, ImportStatus::New);
}

#[test]
fn status_codes_are_the_import_dialog_tags() {
    assert_eq!(ImportStatus::New as u8, 0);
    assert_eq!(ImportStatus::ReplacesExisting as u8, 1);
    assert_eq!(ImportStatus::WrongBandCount as u8, 2);
    assert_eq!(ImportStatus::OutOfRange as u8, 3);

    assert!(ImportStatus::New.is_importable());
    assert!(ImportStatus::ReplacesExisting.is_importable());
    assert!(ImportStatus::WrongBandCount.is_blocking());
    assert!(ImportStatus::OutOfRange.is_blocking());
}
