//! Catalog invariants: every supported model has exactly one family, the
//! physical groups are complete, and inferred assignments stay visible.

use std::collections::{BTreeMap, BTreeSet};

use soundcore_models::catalog::{
    BatteryTopology, CATALOG, Confidence, FAMILIES, FormFactor, Tier, family, grouped_families,
    model,
};

#[test]
fn every_family_has_a_model_and_every_model_has_a_family() {
    assert_eq!(CATALOG.len(), 39);
    let codes: BTreeSet<_> = CATALOG.iter().map(|model| model.code).collect();
    assert_eq!(codes.len(), CATALOG.len(), "codes must be unique");

    let family_ids: BTreeSet<_> = FAMILIES.iter().map(|family| family.id).collect();
    assert_eq!(
        family_ids.len(),
        FAMILIES.len(),
        "family ids must be unique"
    );
    for model in CATALOG {
        assert!(
            family_ids.contains(model.family),
            "{} references missing family {}",
            model.code,
            model.family
        );
    }
    for family in FAMILIES {
        assert!(
            family.models().next().is_some(),
            "{} has no members",
            family.id
        );
    }
}

#[test]
fn form_factor_totals_match_the_supported_list() {
    let mut counts: BTreeMap<FormFactor, usize> = BTreeMap::new();
    for model in CATALOG {
        let family = family(model.family).expect("family");
        *counts.entry(family.form_factor).or_default() += 1;
    }
    assert_eq!(counts[&FormFactor::TwsEarbuds], 24);
    assert_eq!(counts[&FormFactor::Headphones], 9);
    assert_eq!(counts[&FormFactor::OpenEar], 4);
    assert_eq!(counts[&FormFactor::Speaker], 1);
    assert_eq!(counts[&FormFactor::SleepBuds], 1);
    assert_eq!(counts.values().sum::<usize>(), 39);
}

#[test]
fn grouped_families_have_stable_order() {
    let groups = grouped_families();
    assert_eq!(groups.len(), 5);
    assert_eq!(groups[0].0, FormFactor::TwsEarbuds);
    assert!(groups[0].1.len() >= 5);
    assert_eq!(groups[1].0, FormFactor::Headphones);
    assert_eq!(groups[1].1.len(), 2);
    assert_eq!(groups[2].0, FormFactor::OpenEar);
    assert_eq!(groups[2].1.len(), 1);
    assert_eq!(groups[3].0, FormFactor::Speaker);
    assert_eq!(groups[3].1.len(), 1);
    assert_eq!(groups[4].0, FormFactor::SleepBuds);
    assert_eq!(groups[4].1.len(), 1);
}

#[test]
fn key_models_land_in_the_expected_subgroup() {
    let cases = [
        ("A3954", "liberty-flagship", Tier::Flagship),
        ("A3957", "liberty-flagship", Tier::Flagship),
        ("A3947", "liberty-nc", Tier::Midrange),
        ("A3949", "p-series", Tier::Budget),
        ("A3936", "space-tws", Tier::Midrange),
        ("A3968", "sport", Tier::Sport),
        ("A3035", "space-headphones", Tier::Flagship),
        ("A3004", "q-headphones", Tier::Midrange),
        ("A3388", "open-ear", Tier::OpenEar),
        ("A3116", "speaker", Tier::Speaker),
        ("D1301", "sleep", Tier::Sleep),
    ];
    for (code, family_id, tier) in cases {
        let entry = model(code).unwrap_or_else(|| panic!("{code} missing"));
        let family = family(entry.family).expect("family");
        assert_eq!(entry.family, family_id, "{code}");
        assert_eq!(family.tier, tier, "{code}");
    }
}

#[test]
fn inferred_assignments_are_visible() {
    for code in ["A3005", "A3031", "A3033", "A3876"] {
        assert_eq!(
            model(code).expect("model").confidence,
            Confidence::Inferred,
            "{code} needs a product-page check"
        );
    }
    assert_eq!(
        model("A3954").expect("model").confidence,
        Confidence::Confirmed
    );
}

#[test]
fn battery_topology_is_separate_from_form_factor() {
    assert_eq!(
        model("A3949").expect("model").battery,
        BatteryTopology::Dual
    );
    assert_eq!(
        model("A3954").expect("model").battery,
        BatteryTopology::DualWithCase
    );
    assert_eq!(
        model("A3062").expect("model").battery,
        BatteryTopology::Single
    );
    assert_eq!(
        model("A3033").expect("model").battery,
        BatteryTopology::Single
    );
}

#[test]
fn unsupported_codes_are_not_in_the_catalog() {
    assert!(model("D1203").is_none());
    assert!(model("A3956").is_none());
    assert!(model("A6611").is_none());
}
