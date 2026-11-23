//! Typed message keys for the sentences the Rust model sends to the UI.
//!
//! The window never receives a ready-made English sentence from Rust: a model
//! field carries a [`UiMessage`], which names a branch of the `Messages` catalog
//! in `ui/messages.slint` and carries up to two arguments for that sentence. The
//! catalog resolves the key through `@tr`, so the `.po` documents translate
//! runtime copy exactly like static Slint text.
//!
//! Keys are internal identifiers (for example `quick-toggle.one-earbud`), never
//! UI copy. When a model field needs a sentence, add a key here, add the matching
//! branch in `ui/messages.slint` with its English source, and add that source
//! string to the translation documents.

use crate::UiMessage;

/// The LDAC quick toggle while only one earbud of a pair is connected.
pub const QUICK_TOGGLE_ONE_EARBUD: &str = "quick-toggle.one-earbud";
/// The spatial-audio choices while spatial audio itself is off.
pub const SETTING_TURN_ON_SPATIAL_AUDIO: &str = "setting.turn-on-spatial-audio";

/// A message with a key and its two arguments; pass `""` for an unused one.
pub fn with_args(key: &str, arg0: &str, arg1: &str) -> UiMessage {
    UiMessage {
        key: key.into(),
        arg0: arg0.into(),
        arg1: arg1.into(),
    }
}

/// The empty message: the model has nothing to explain.
pub fn empty() -> UiMessage {
    with_args("", "", "")
}

/// The reason of the LDAC quick toggle while only one earbud is connected.
pub fn quick_toggle_one_earbud() -> UiMessage {
    with_args(QUICK_TOGGLE_ONE_EARBUD, "", "")
}

/// The reason of the spatial-audio choices while spatial audio itself is off.
pub fn turn_on_spatial_audio_first() -> UiMessage {
    with_args(SETTING_TURN_ON_SPATIAL_AUDIO, "", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_carries_its_key_and_both_arguments() {
        let message = with_args("example.key", "first", "second");
        assert_eq!(message.key.as_str(), "example.key");
        assert_eq!(message.arg0.as_str(), "first");
        assert_eq!(message.arg1.as_str(), "second");
    }

    #[test]
    fn the_empty_message_has_no_key_or_arguments() {
        assert_eq!(empty(), UiMessage::default());
        assert!(empty().key.is_empty());
    }

    #[test]
    fn reason_helpers_name_a_catalog_key_and_carry_no_sentence() {
        assert_eq!(
            quick_toggle_one_earbud(),
            with_args(QUICK_TOGGLE_ONE_EARBUD, "", "")
        );
        assert_eq!(
            turn_on_spatial_audio_first(),
            with_args(SETTING_TURN_ON_SPATIAL_AUDIO, "", "")
        );
        for message in [quick_toggle_one_earbud(), turn_on_spatial_audio_first()] {
            // A key names a catalog branch; it is not an English sentence.
            assert!(!message.key.is_empty());
            assert!(!message.key.contains(' '));
            assert!(message.arg0.is_empty());
            assert!(message.arg1.is_empty());
        }
        let catalog = include_str!("../ui/messages.slint");
        for key in [QUICK_TOGGLE_ONE_EARBUD, SETTING_TURN_ON_SPATIAL_AUDIO] {
            assert!(
                catalog.contains(&format!("message.key == \"{key}\"")),
                "ui/messages.slint has no Messages branch for {key}"
            );
        }
    }

    #[test]
    fn english_reason_sources_live_in_the_slint_catalog_not_in_rust() {
        // The guard for the migrated reasons: the model modules carry keys, the
        // English source sits in the Slint catalog, and the translation document
        // owns the Russian copy.
        let catalog = include_str!("../ui/messages.slint");
        let russian = include_str!("../translations/ru/LC_MESSAGES/soundcore-desktop.po");
        // Assembled from parts so this test's own text does not match the scan.
        let sources = [
            concat!("Changing is available", " when both earbuds are connected"),
            concat!("Turn on spatial", " audio first"),
        ];
        for source in sources {
            assert!(
                catalog.contains(source),
                "the English source belongs in ui/messages.slint: {source}"
            );
            assert!(
                russian.contains(&format!("msgid \"{source}\"")),
                "the Russian catalog has no msgid for: {source}"
            );
        }
        for rust_source in [include_str!("overview.rs"), include_str!("controller.rs")] {
            for source in sources {
                assert!(
                    !rust_source.contains(source),
                    "Rust carries a ready-made UI sentence: {source}"
                );
            }
        }
    }
}
