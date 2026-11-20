//! Adapter from the clean-room model catalog (`soundcore_models::catalog`) to
//! the data the vendored Add Device dialog expects.
//!
//! The dialog's chips are used as indexes: All, Earbuds, Over-ear, Open-ear,
//! Speakers. `CatalogEntry` is the pure layer; `filter_models` only converts it
//! to the Slint `ModelOption`, so filtering is tested without a window.

// Ticket 03 wires this module to the dialog's callbacks. Until then nothing in
// the binary calls the adapter, so its items would be dead code warnings.
#![allow(dead_code)]

use std::cell::RefCell;

use soundcore_models::catalog::{CATALOG, FormFactor, grouped_families};

use crate::ModelOption;

/// The All chip, which does not filter.
pub const KIND_ALL: i32 = 0;
/// The Earbuds chip. The vendored UI has no Sleep chip, so sleep buds share it.
pub const KIND_EARBUDS: i32 = 1;
/// The Over-ear chip.
pub const KIND_OVER_EAR: i32 = 2;
/// The Open-ear chip.
pub const KIND_OPEN_EAR: i32 = 3;
/// The Speakers chip.
pub const KIND_SPEAKERS: i32 = 4;

/// The vendored chip the form factor is shown under.
pub fn category(form_factor: FormFactor) -> i32 {
    match form_factor {
        FormFactor::TwsEarbuds | FormFactor::SleepBuds => KIND_EARBUDS,
        FormFactor::Headphones => KIND_OVER_EAR,
        FormFactor::OpenEar => KIND_OPEN_EAR,
        FormFactor::Speaker => KIND_SPEAKERS,
    }
}

/// One model of the catalog, as the Add Device dialog needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub code: &'static str,
    pub name: &'static str,
    pub kind: i32,
    pub art: slint::Image,
}

/// Every catalog model once, in the vendored list order: form factor, then
/// family, then how the catalog lists the model in its family.
pub fn entries() -> Vec<CatalogEntry> {
    let mut entries = Vec::with_capacity(CATALOG.len());
    for (form_factor, families) in grouped_families() {
        let kind = category(form_factor);
        for family in families {
            for model in CATALOG.iter().filter(|model| model.family == family.id) {
                entries.push(CatalogEntry {
                    code: model.code,
                    name: model.name,
                    kind,
                    art: art(form_factor),
                });
            }
        }
    }
    entries
}

/// The vendored device art, embedded as SVG bytes at compile time.
///
/// The vendored tree ships silver and black pairs; silver is the colour the
/// OpenSCL4 shell picks for its light theme, and a baked image cannot follow a
/// theme change.
fn art_svg(form_factor: FormFactor) -> &'static [u8] {
    match form_factor {
        FormFactor::TwsEarbuds => include_bytes!("../ui/art/sound_art_stem_premium_silver.svg"),
        FormFactor::Headphones => include_bytes!("../ui/art/sound_art_headphones_silver.svg"),
        FormFactor::OpenEar => include_bytes!("../ui/art/sound_art_open_clip_silver.svg"),
        FormFactor::Speaker => include_bytes!("../ui/art/sound_art_motion_plus_silver.svg"),
        FormFactor::SleepBuds => include_bytes!("../ui/art/sound_art_buds_compact_silver.svg"),
    }
}

thread_local! {
    static ART: RefCell<[Option<slint::Image>; 5]> =
        const { RefCell::new([None, None, None, None, None]) };
}

/// The vendored illustration of the form factor, decoded once per thread and
/// shared by every model of that shape.
pub fn art(form_factor: FormFactor) -> slint::Image {
    let index = FormFactor::ALL
        .iter()
        .position(|candidate| *candidate == form_factor)
        .expect("every form factor is listed in FormFactor::ALL");
    ART.with(|cache| {
        cache.borrow_mut()[index]
            .get_or_insert_with(|| {
                slint::Image::load_from_svg_data(art_svg(form_factor))
                    .expect("the vendored device art is a valid SVG")
            })
            .clone()
    })
}

/// The entries the dialog lists for a query and a chip.
pub fn filter_entries(query: &str, kind: i32) -> Vec<CatalogEntry> {
    let query = query.to_lowercase();
    entries()
        .into_iter()
        .filter(|entry| kind == KIND_ALL || entry.kind == kind)
        .filter(|entry| {
            query.is_empty()
                || entry.code.to_lowercase().contains(&query)
                || entry.name.to_lowercase().contains(&query)
        })
        .collect()
}

/// The options the Add Device dialog's model list binds to.
pub fn filter_models(query: &str, kind: i32) -> Vec<ModelOption> {
    filter_entries(query, kind)
        .into_iter()
        .map(|entry| ModelOption {
            code: entry.code.into(),
            name: entry.name.into(),
            art: entry.art,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_factors_map_to_the_vendored_chips() {
        assert_eq!(category(FormFactor::TwsEarbuds), 1);
        assert_eq!(category(FormFactor::Headphones), 2);
        assert_eq!(category(FormFactor::OpenEar), 3);
        assert_eq!(category(FormFactor::Speaker), 4);
        // The vendored UI has no Sleep chip, so sleep buds are earbuds.
        assert_eq!(category(FormFactor::SleepBuds), 1);
    }

    #[test]
    fn the_catalog_is_listed_once_in_the_vendored_order() {
        let entries = entries();
        let codes: Vec<&str> = entries.iter().map(|entry| entry.code).collect();
        assert_eq!(
            codes,
            [
                // TWS: liberty flagship, liberty NC, P/A budget, Life legacy, Space, sport.
                "A3909", "A3930", "A3951", "A3952", "A3953", "A3954", "A3957", "A3947", "A3948",
                "A3949", "A3955", "A3959", "D1202", "A3031", "A3033", "A3926", "A3931", "A3933",
                "A3935", "A3939", "A3944", "A3945", "A3936", "A3968",
                // Headphones: Space, then Q series and Life Tune.
                "A3035", "A3040", "A3062", "A3004", "A3005", "A3027", "A3028", "A3029", "A3030",
                // Open-ear, speaker, sleep buds last.
                "A3330", "A3388", "A3876", "D1101", "A3116", "D1301",
            ]
        );
        let count = |kind: i32| entries.iter().filter(|entry| entry.kind == kind).count();
        // The catalog has 24 TWS models; the sleep buds join them under Earbuds.
        assert_eq!(count(KIND_EARBUDS), 25);
        assert_eq!(count(KIND_OVER_EAR), 9);
        assert_eq!(count(KIND_OPEN_EAR), 4);
        assert_eq!(count(KIND_SPEAKERS), 1);
    }

    #[test]
    fn search_by_code_finds_the_model() {
        let found = filter_entries("A3954", KIND_ALL);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "A3954");
        assert_eq!(found[0].name, "Liberty 4 Pro");
        assert_eq!(filter_entries("a3954", KIND_ALL).len(), 1);
    }

    #[test]
    fn search_by_name_is_case_insensitive() {
        let found = filter_entries("p20", KIND_ALL);
        let codes: Vec<&str> = found.iter().map(|entry| entry.code).collect();
        assert_eq!(codes, ["A3949"]);
        assert_eq!(filter_entries("P20", KIND_ALL).len(), 1);
        assert_eq!(filter_entries("liberty 4 pro", KIND_ALL).len(), 1);
    }

    #[test]
    fn an_unknown_query_finds_nothing() {
        assert!(filter_entries("no such model", KIND_ALL).is_empty());
    }

    #[test]
    fn an_empty_query_returns_the_whole_catalog() {
        assert_eq!(filter_entries("", KIND_ALL).len(), 39);
    }

    #[test]
    fn the_chips_filter_by_kind() {
        let earbuds = filter_entries("", KIND_EARBUDS);
        assert_eq!(earbuds.len(), 25);
        assert!(earbuds.iter().all(|entry| entry.kind == KIND_EARBUDS));
        // The sleep buds are listed under Earbuds: the vendored UI has no Sleep chip.
        assert!(earbuds.iter().any(|entry| entry.code == "D1301"));
        assert_eq!(filter_entries("", KIND_OVER_EAR).len(), 9);
        assert_eq!(filter_entries("", KIND_OPEN_EAR).len(), 4);
        assert_eq!(filter_entries("", KIND_SPEAKERS).len(), 1);
        assert!(filter_entries("", 99).is_empty());
    }

    #[test]
    fn search_and_kind_narrow_each_other() {
        let space = filter_entries("space", KIND_ALL);
        assert_eq!(space.len(), 4);
        let space_earbuds = filter_entries("space", KIND_EARBUDS);
        let codes: Vec<&str> = space_earbuds.iter().map(|entry| entry.code).collect();
        assert_eq!(codes, ["A3936"]);
        assert_eq!(filter_entries("space", KIND_OVER_EAR).len(), 3);
        assert_eq!(filter_entries("A3954", KIND_OVER_EAR).len(), 0);
    }

    #[test]
    fn every_form_factor_has_its_own_embedded_art() {
        for form_factor in FormFactor::ALL {
            assert_ne!(
                art(form_factor),
                slint::Image::default(),
                "{form_factor:?} has no art"
            );
        }
        // The art is loaded once per form factor and shared, not redrawn per model.
        assert_eq!(art(FormFactor::TwsEarbuds), art(FormFactor::TwsEarbuds));
        assert_ne!(art(FormFactor::TwsEarbuds), art(FormFactor::Headphones));
        assert_ne!(art(FormFactor::OpenEar), art(FormFactor::Speaker));
        assert_ne!(art(FormFactor::SleepBuds), art(FormFactor::OpenEar));
    }

    #[test]
    fn an_entry_carries_the_art_of_its_form_factor() {
        let entries = entries();
        let art_of = |code: &str| {
            entries
                .iter()
                .find(|entry| entry.code == code)
                .expect("the code is in the catalog")
                .art
                .clone()
        };
        assert_eq!(art_of("A3954"), art(FormFactor::TwsEarbuds));
        assert_eq!(art_of("A3035"), art(FormFactor::Headphones));
        assert_eq!(art_of("A3330"), art(FormFactor::OpenEar));
        assert_eq!(art_of("A3116"), art(FormFactor::Speaker));
        assert_eq!(art_of("D1301"), art(FormFactor::SleepBuds));
    }

    #[test]
    fn filter_models_returns_the_slint_options() {
        let options = filter_models("A3954", KIND_ALL);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].code.as_str(), "A3954");
        assert_eq!(options[0].name.as_str(), "Liberty 4 Pro");
        assert_ne!(options[0].art, slint::Image::default());
    }

    #[test]
    fn filter_models_passes_search_and_kind_through() {
        assert_eq!(filter_models("", KIND_SPEAKERS).len(), 1);
        assert_eq!(filter_models("p20", KIND_ALL)[0].code.as_str(), "A3949");
        assert_eq!(filter_models("A3954", KIND_OVER_EAR).len(), 0);
    }
}
