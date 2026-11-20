//! Adapter from the clean-room model catalog (`soundcore_models::catalog`) to
//! the data the vendored Add Device dialog expects.
//!
//! The dialog's chips are the five categories below, in the order the vendored
//! UI lists them. `CatalogEntry` is the pure layer; `filter_models` only converts
//! it to the Slint `ModelOption`, so filtering is tested without a window.

use std::cell::RefCell;

use soundcore_models::catalog::{self, FormFactor};

use crate::ModelOption;

/// A chip on the vendored Add Device dialog, in the order the dialog lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    All,
    Earbuds,
    OverEar,
    OpenEar,
    Speakers,
}

impl Chip {
    /// The chip's position in the vendored row; the UI stores that index.
    pub const fn index(self) -> i32 {
        match self {
            Self::All => 0,
            Self::Earbuds => 1,
            Self::OverEar => 2,
            Self::OpenEar => 3,
            Self::Speakers => 4,
        }
    }

    /// The chip at a vendored row index, or `None` when the index is not a chip.
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::All),
            1 => Some(Self::Earbuds),
            2 => Some(Self::OverEar),
            3 => Some(Self::OpenEar),
            4 => Some(Self::Speakers),
            _ => None,
        }
    }
}

/// The vendored chip a form factor is shown under.
pub fn category(form_factor: FormFactor) -> Chip {
    match form_factor {
        FormFactor::TwsEarbuds | FormFactor::SleepBuds => Chip::Earbuds,
        FormFactor::Headphones => Chip::OverEar,
        FormFactor::OpenEar => Chip::OpenEar,
        FormFactor::Speaker => Chip::Speakers,
    }
}

/// One model of the catalog, as the Add Device dialog needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub code: &'static str,
    pub name: &'static str,
    pub chip: Chip,
    pub art: slint::Image,
}

/// Every catalog model once, in the vendored list order: form factor, then
/// family, then how the catalog lists the model in its family.
pub fn entries() -> Vec<CatalogEntry> {
    let mut entries = Vec::with_capacity(catalog::CATALOG.len());
    for (form_factor, families) in catalog::grouped_families() {
        let chip = category(form_factor);
        for family in families {
            for model in family.models() {
                entries.push(CatalogEntry {
                    code: model.code,
                    name: model.name,
                    chip,
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
pub fn filter_entries(query: &str, chip: Chip) -> Vec<CatalogEntry> {
    let query = query.to_lowercase();
    entries()
        .into_iter()
        .filter(|entry| chip == Chip::All || entry.chip == chip)
        .filter(|entry| {
            query.is_empty()
                || entry.code.to_lowercase().contains(&query)
                || entry.name.to_lowercase().contains(&query)
        })
        .collect()
}

/// What choosing a model card puts into the Add Device dialog's confirmation
/// step: the fields it shows, and whether it words the dialog for a speaker.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub name: &'static str,
    pub code: &'static str,
    pub art: slint::Image,
    pub is_speaker: bool,
}

/// The confirmation step for a model code, or `None` when the catalog lists no
/// model of that exact code.
pub fn selection(code: &str) -> Option<Selection> {
    let model = catalog::model(code)?;
    let form_factor = catalog::family(model.family)
        .expect("every catalog model lists a family of the catalog")
        .form_factor;
    Some(Selection {
        name: model.name,
        code: model.code,
        art: art(form_factor),
        is_speaker: category(form_factor) == Chip::Speakers,
    })
}

/// The options the Add Device dialog's model list binds to.
pub fn filter_models(query: &str, chip: Chip) -> Vec<ModelOption> {
    filter_entries(query, chip)
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
    fn the_chip_indexes_match_the_vendored_row() {
        for (index, chip) in [
            Chip::All,
            Chip::Earbuds,
            Chip::OverEar,
            Chip::OpenEar,
            Chip::Speakers,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(chip.index(), index as i32);
            assert_eq!(Chip::from_index(chip.index()), Some(chip));
        }
        // The vendored row lists five chips; no other index is a chip.
        assert_eq!(Chip::from_index(-1), None);
        assert_eq!(Chip::from_index(5), None);
    }

    #[test]
    fn form_factors_map_to_the_vendored_chips() {
        assert_eq!(category(FormFactor::TwsEarbuds), Chip::Earbuds);
        assert_eq!(category(FormFactor::Headphones), Chip::OverEar);
        assert_eq!(category(FormFactor::OpenEar), Chip::OpenEar);
        assert_eq!(category(FormFactor::Speaker), Chip::Speakers);
        // The vendored UI has no Sleep chip, so sleep buds are earbuds.
        assert_eq!(category(FormFactor::SleepBuds), Chip::Earbuds);
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
        let count = |chip: Chip| entries.iter().filter(|entry| entry.chip == chip).count();
        // The catalog has 24 TWS models; the sleep buds join them under Earbuds.
        assert_eq!(count(Chip::Earbuds), 25);
        assert_eq!(count(Chip::OverEar), 9);
        assert_eq!(count(Chip::OpenEar), 4);
        assert_eq!(count(Chip::Speakers), 1);
    }

    #[test]
    fn every_catalog_model_belongs_to_a_family() {
        // `entries` walks families, and `selection` resolves a model's family for
        // its form factor, so a model with a dangling family would drop out.
        for model in catalog::CATALOG {
            assert!(
                catalog::family(model.family).is_some(),
                "{} lists unknown family {:?}",
                model.code,
                model.family
            );
        }
    }

    #[test]
    fn search_by_code_finds_the_model() {
        let found = filter_entries("A3954", Chip::All);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "A3954");
        assert_eq!(found[0].name, "Liberty 4 Pro");
        assert_eq!(filter_entries("a3954", Chip::All).len(), 1);
    }

    #[test]
    fn search_by_name_is_case_insensitive() {
        let found = filter_entries("p20", Chip::All);
        let codes: Vec<&str> = found.iter().map(|entry| entry.code).collect();
        assert_eq!(codes, ["A3949"]);
        assert_eq!(filter_entries("P20", Chip::All).len(), 1);
        assert_eq!(filter_entries("liberty 4 pro", Chip::All).len(), 1);
    }

    #[test]
    fn an_unknown_query_finds_nothing() {
        assert!(filter_entries("no such model", Chip::All).is_empty());
    }

    #[test]
    fn an_empty_query_returns_the_whole_catalog() {
        assert_eq!(filter_entries("", Chip::All).len(), 39);
    }

    #[test]
    fn the_chips_filter_by_kind() {
        let earbuds = filter_entries("", Chip::Earbuds);
        assert_eq!(earbuds.len(), 25);
        assert!(earbuds.iter().all(|entry| entry.chip == Chip::Earbuds));
        // The sleep buds are listed under Earbuds: the vendored UI has no Sleep chip.
        assert!(earbuds.iter().any(|entry| entry.code == "D1301"));
        assert_eq!(filter_entries("", Chip::OverEar).len(), 9);
        assert_eq!(filter_entries("", Chip::OpenEar).len(), 4);
        assert_eq!(filter_entries("", Chip::Speakers).len(), 1);
    }

    #[test]
    fn search_and_kind_narrow_each_other() {
        let space = filter_entries("space", Chip::All);
        assert_eq!(space.len(), 4);
        let space_earbuds = filter_entries("space", Chip::Earbuds);
        let codes: Vec<&str> = space_earbuds.iter().map(|entry| entry.code).collect();
        assert_eq!(codes, ["A3936"]);
        assert_eq!(filter_entries("space", Chip::OverEar).len(), 3);
        assert_eq!(filter_entries("A3954", Chip::OverEar).len(), 0);
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
        let options = filter_models("A3954", Chip::All);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].code.as_str(), "A3954");
        assert_eq!(options[0].name.as_str(), "Liberty 4 Pro");
        assert_ne!(options[0].art, slint::Image::default());
    }

    #[test]
    fn filter_models_passes_search_and_kind_through() {
        assert_eq!(filter_models("", Chip::Speakers).len(), 1);
        assert_eq!(filter_models("p20", Chip::All)[0].code.as_str(), "A3949");
        assert_eq!(filter_models("A3954", Chip::OverEar).len(), 0);
    }

    #[test]
    fn choosing_a_model_carries_the_confirmation_fields() {
        let selection = selection("A3954").expect("A3954 is in the catalog");
        assert_eq!(selection.name, "Liberty 4 Pro");
        assert_eq!(selection.code, "A3954");
        assert_eq!(selection.art, art(FormFactor::TwsEarbuds));
        assert!(!selection.is_speaker);
    }

    #[test]
    fn choosing_a_speaker_marks_it_as_one() {
        let selection = selection("A3116").expect("A3116 is in the catalog");
        assert!(selection.is_speaker);
    }

    #[test]
    fn choosing_a_code_outside_the_catalog_yields_nothing() {
        assert!(selection("not-a-model").is_none());
    }
}
