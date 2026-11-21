//! Tests for the saved equalizer profile model, set operations and JSON
//! schema v1.

use soundcore_models::{EQ_BAND_MAX, EQ_BAND_MIN, VISIBLE_EQ_BANDS};
use soundcore_profiles::{
    EqualizerProfile, MAX_PROFILE_NAME_LEN, ProfileError, ProfileJsonError, ProfileSet,
};

/// A distinctive curve within the measured A3062 band range.
const WARM_BANDS: [i16; VISIBLE_EQ_BANDS] = [30, -30, 60, 0, -60, 20, 0, -20];
/// The neutral curve.
const FLAT_BANDS: [i16; VISIBLE_EQ_BANDS] = [0; VISIBLE_EQ_BANDS];

fn profile(name: &str, bands: [i16; VISIBLE_EQ_BANDS]) -> EqualizerProfile {
    EqualizerProfile::new(name, bands).expect("a valid test profile")
}

#[test]
fn roundtrips_profiles_through_json_schema_v1() {
    let mut set = ProfileSet::new();
    set.add(profile("Warm", WARM_BANDS)).expect("add Warm");
    set.add(profile("Flat", FLAT_BANDS)).expect("add Flat");

    let json = set.to_json();
    assert_eq!(
        json,
        concat!(
            r#"[{"name":"Warm","bands":[30,-30,60,0,-60,20,0,-20]},"#,
            r#"{"name":"Flat","bands":[0,0,0,0,0,0,0,0]}]"#,
        )
    );

    let parsed = ProfileSet::from_json(&json).expect("the written JSON parses");
    assert_eq!(parsed, set);
}

#[test]
fn missing_empty_and_whitespace_documents_are_an_empty_set() {
    for text in ["", "   \n\t ", "[]"] {
        let set = ProfileSet::from_json(text).expect("an empty document");
        assert!(set.is_empty(), "{text:?} must parse as the empty set");
        assert_eq!(set.len(), 0);
    }

    let empty = ProfileSet::new();
    assert_eq!(empty.to_json(), "[]");
    assert_eq!(empty, ProfileSet::default());
    assert_eq!(
        ProfileSet::from_json(&empty.to_json()).expect("empty"),
        empty
    );
}

#[test]
fn ignores_unknown_fields_and_never_writes_them() {
    let json = concat!(
        r#"[{"name":"Warm","bands":[30,-30,60,0,-60,20,0,-20],"#,
        r#""curve":"M 0 0 L 8 0","enabled":true},"#,
        r#"{"name":"Flat","bands":[0,0,0,0,0,0,0,0],"bands2":[1],"note":null}]"#,
    );

    let set = ProfileSet::from_json(json).expect("unknown object fields are ignored");
    assert_eq!(set.len(), 2);
    assert_eq!(set.get("Warm").expect("Warm").bands, WARM_BANDS);

    let written = set.to_json();
    assert_eq!(written, set.to_json(), "serialization is deterministic");
    for unknown in ["curve", "enabled", "bands2", "note"] {
        assert!(
            !written.contains(unknown),
            "the unknown field {unknown:?} must not be written back"
        );
    }
}

#[test]
fn rejects_duplicate_names_and_replace_updates_the_existing_profile() {
    let mut set = ProfileSet::new();
    set.add(profile("Warm", WARM_BANDS)).expect("add Warm");

    let duplicate = set.add(profile("Warm", FLAT_BANDS)).expect_err("duplicate");
    assert_eq!(
        duplicate,
        ProfileError::DuplicateName {
            name: "Warm".to_owned()
        }
    );
    assert_eq!(
        set.get("Warm").expect("Warm stays").bands,
        WARM_BANDS,
        "a rejected duplicate must not change the saved curve"
    );
    assert_eq!(set.len(), 1);

    set.add(profile("Flat", FLAT_BANDS)).expect("add Flat");
    set.replace(profile(" Warm ", FLAT_BANDS))
        .expect("replace Warm");
    assert_eq!(set.len(), 2);
    assert_eq!(set.get("Warm").expect("Warm").bands, FLAT_BANDS);
    let names: Vec<&str> = set.profiles().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["Warm", "Flat"],
        "replace keeps the replaced profile's position"
    );

    let missing = set
        .replace(profile("Missing", FLAT_BANDS))
        .expect_err("replace needs an existing profile");
    assert_eq!(
        missing,
        ProfileError::NotFound {
            name: "Missing".to_owned()
        }
    );
}

#[test]
fn trims_names_for_storage_lookup_and_duplicate_detection() {
    let mut set = ProfileSet::new();
    set.add(profile("  Warm  ", WARM_BANDS)).expect("add Warm");

    assert_eq!(set.get("Warm").expect("Warm").name, "Warm");
    assert!(set.contains("  Warm  "));
    assert!(set.get("\tWarm\r\n").is_some());

    let duplicate = set
        .add(profile(" Warm", FLAT_BANDS))
        .expect_err("trimmed duplicate");
    assert_eq!(
        duplicate,
        ProfileError::DuplicateName {
            name: "Warm".to_owned()
        }
    );

    assert_eq!(set.remove(" Warm ").expect("remove Warm").bands, WARM_BANDS);
    assert!(set.get("Warm").is_none());
    assert!(set.remove("Warm").is_none(), "removing twice is a no-op");
    assert!(set.is_empty());
}

#[test]
fn rejects_empty_and_whitespace_names() {
    assert_eq!(
        EqualizerProfile::new("", WARM_BANDS),
        Err(ProfileError::EmptyName)
    );
    assert_eq!(
        EqualizerProfile::new(" \t\r\n ", WARM_BANDS),
        Err(ProfileError::EmptyName)
    );

    // The fields are public, so the set validates profiles it is handed too.
    let mut set = ProfileSet::new();
    let hand_built = EqualizerProfile {
        name: "  ".to_owned(),
        bands: WARM_BANDS,
    };
    assert_eq!(set.add(hand_built), Err(ProfileError::EmptyName));
    assert!(set.is_empty());
}

#[test]
fn normalizes_profiles_supplied_through_the_public_fields() {
    let mut set = ProfileSet::new();
    let hand_built = EqualizerProfile {
        name: "  Mercury  ".to_owned(),
        bands: WARM_BANDS,
    };
    set.add(hand_built).expect("the set trims the name");
    assert_eq!(set.get("Mercury").expect("Mercury").name, "Mercury");
    assert_eq!(set.len(), 1);
}

#[test]
fn accepts_a_name_of_the_maximum_length_and_rejects_longer_ones() {
    let longest = "x".repeat(MAX_PROFILE_NAME_LEN);
    let accepted = EqualizerProfile::new(longest.clone(), WARM_BANDS).expect("max length");
    assert_eq!(accepted.name, longest);
    assert_eq!(
        accepted.name.chars().count(),
        MAX_PROFILE_NAME_LEN,
        "the limit counts characters, not bytes"
    );

    let multibyte = "Ж".repeat(MAX_PROFILE_NAME_LEN);
    let accepted = EqualizerProfile::new(multibyte, WARM_BANDS).expect("60 multibyte characters");
    assert_eq!(accepted.name.chars().count(), MAX_PROFILE_NAME_LEN);

    let too_long = "y".repeat(MAX_PROFILE_NAME_LEN + 1);
    assert_eq!(
        EqualizerProfile::new(too_long, WARM_BANDS),
        Err(ProfileError::NameTooLong {
            actual: MAX_PROFILE_NAME_LEN + 1,
            max: MAX_PROFILE_NAME_LEN,
        })
    );
}

#[test]
fn rejects_bands_outside_the_measured_range() {
    let cases = [
        (0, EQ_BAND_MIN - 1),
        (3, EQ_BAND_MAX + 1),
        (7, i16::MAX),
        (2, i16::MIN),
    ];
    for (index, value) in cases {
        let mut bands = WARM_BANDS;
        bands[index] = value;
        assert_eq!(
            EqualizerProfile::new("Warm", bands),
            Err(ProfileError::BandOutOfRange {
                index,
                value,
                min: EQ_BAND_MIN,
                max: EQ_BAND_MAX,
            }),
            "band {index} = {value} must be rejected"
        );
    }

    let mut edges = WARM_BANDS;
    edges[0] = EQ_BAND_MIN;
    edges[1] = EQ_BAND_MAX;
    let accepted = EqualizerProfile::new("Edges", edges).expect("both range ends are valid");
    assert_eq!(accepted.bands[0], EQ_BAND_MIN);
    assert_eq!(accepted.bands[1], EQ_BAND_MAX);
}

#[test]
fn reports_typed_errors_for_malformed_and_invalid_documents() {
    assert!(
        matches!(
            ProfileSet::from_json("{"),
            Err(ProfileJsonError::InvalidJson(_))
        ),
        "malformed JSON is a typed error"
    );
    assert!(
        matches!(
            ProfileSet::from_json(r#"{"profiles":[]}"#),
            Err(ProfileJsonError::InvalidJson(_))
        ),
        "schema v1 is an array, not an object"
    );

    let short = r#"[{"name":"Short","bands":[0,0,0,0,0,0,0]}]"#;
    assert!(matches!(
        ProfileSet::from_json(short),
        Err(ProfileJsonError::InvalidProfile {
            index: 0,
            source: ProfileError::WrongBandCount {
                actual: 7,
                expected: VISIBLE_EQ_BANDS,
            },
        })
    ));

    let loud = r#"[{"name":"Loud","bands":[0,0,0,0,0,0,0,135]}]"#;
    assert!(matches!(
        ProfileSet::from_json(loud),
        Err(ProfileJsonError::InvalidProfile {
            index: 0,
            source: ProfileError::BandOutOfRange {
                index: 7,
                value: 135,
                ..
            },
        })
    ));

    let unnamed = r#"[{"name":"  ","bands":[0,0,0,0,0,0,0,0]}]"#;
    assert!(matches!(
        ProfileSet::from_json(unnamed),
        Err(ProfileJsonError::InvalidProfile {
            index: 0,
            source: ProfileError::EmptyName,
        })
    ));

    let repeated = concat!(
        r#"[{"name":"Same","bands":[0,0,0,0,0,0,0,0]},"#,
        r#"{"name":"Same","bands":[1,1,1,1,1,1,1,1]}]"#,
    );
    assert!(
        matches!(
            ProfileSet::from_json(repeated),
            Err(ProfileJsonError::DuplicateName { .. })
        ),
        "a document with duplicate names is not schema v1"
    );
}

#[test]
fn keeps_insertion_order_for_deterministic_output() {
    let mut first = ProfileSet::new();
    first.add(profile("Zulu", WARM_BANDS)).expect("Zulu");
    first.add(profile("Alpha", FLAT_BANDS)).expect("Alpha");

    let mut second = ProfileSet::new();
    second.add(profile("Zulu", WARM_BANDS)).expect("Zulu");
    second.add(profile("Alpha", FLAT_BANDS)).expect("Alpha");

    assert_eq!(first.to_json(), second.to_json());

    let names: Vec<&str> = first.profiles().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Zulu", "Alpha"], "profiles stay in insertion order");

    let reloaded = ProfileSet::from_json(&first.to_json()).expect("reload");
    assert_eq!(
        reloaded.to_json(),
        first.to_json(),
        "a load-and-save roundtrip does not reorder profiles"
    );
    assert_eq!(reloaded.iter().count(), 2);
}
