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
//!
//! A sentence that quotes a translatable word (`Could not change “LDAC (no
//! answer)”`) takes that word as its argument *as another key*: [`SUBJECT_*`]
//! names the setting or action, [`REASON_*`] says why a write did not take, and
//! [`PART_UNKNOWN`] is the word for a missing value. `ui/messages.slint` resolves
//! those argument keys through its `MessageParts` global, so no English text is
//! composed on the Rust side.

use crate::UiMessage;

/// The LDAC quick toggle while only one earbud of a pair is connected.
pub const QUICK_TOGGLE_ONE_EARBUD: &str = "quick-toggle.one-earbud";
/// The spatial-audio choices while spatial audio itself is off.
pub const SETTING_TURN_ON_SPATIAL_AUDIO: &str = "setting.turn-on-spatial-audio";

/// A device write the device did not take. `arg0` is a [`SUBJECT_*`] key and
/// `arg1` a [`REASON_*`] key.
pub const ERROR_SETTING_WRITE: &str = "error.setting-write";
/// A failed write of the saved equalizer-profile document.
pub const ERROR_PROFILE_WRITE: &str = "error.profile-write";

/// Why a connection attempt failed, one key per [`crate::controller::ConnectFailure`]
/// case; `connect.transport` takes the technical OS detail as its argument.
pub const CONNECT_NOT_PAIRED: &str = "connect.not-paired";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_NOT_FOUND: &str = "connect.not-found";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_TIMED_OUT: &str = "connect.timed-out";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_BLUETOOTH_OFF: &str = "connect.bluetooth-off";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_CONNECTION_FAILED: &str = "connect.connection-failed";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_NO_SERVICE: &str = "connect.no-service";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_NO_PARSER: &str = "connect.no-parser";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_UNSUPPORTED_LAYOUT: &str = "connect.unsupported-layout";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_PROTOCOL: &str = "connect.protocol";
/// See [`CONNECT_NOT_PAIRED`].
pub const CONNECT_TRANSPORT: &str = "connect.transport";
/// The Bluetooth worker thread could not be started.
pub const CONNECT_WORKER_START: &str = "connect.worker-start";
/// The demo worker thread could not be started.
pub const CONNECT_DEMO_WORKER_START: &str = "connect.demo-worker-start";
/// A saved device card's address cannot be parsed any more.
pub const CONNECT_SAVED_ADDRESS: &str = "connect.saved-address";

/// The Add Device dialog was asked to add a device without a chosen model.
pub const ADD_CHOOSE_MODEL: &str = "add.choose-model";
/// The Add Device row's address cannot be read.
pub const ADD_UNREADABLE_PAIRED: &str = "add.unreadable-paired";
/// The paired-device enumerator thread could not be started.
pub const ADD_PAIRED_WORKER_START: &str = "add.paired-worker-start";
/// Demo mode was switched on for a model the committed capture does not describe.
pub const DEMO_UNAVAILABLE: &str = "demo.unavailable";

/// Demo mode cannot collect diagnostics.
pub const DIAGNOSTICS_DEMO_UNAVAILABLE: &str = "diagnostics.demo-unavailable";
/// A capture was started while the device worker was gone.
pub const DIAGNOSTICS_WORKER_STOPPED: &str = "diagnostics.worker-stopped";
/// Save ZIP was clicked before a capture finished.
pub const DIAGNOSTICS_NO_CAPTURE: &str = "diagnostics.no-capture";
/// The ZIP could not be built in memory; the technical error is the argument.
pub const DIAGNOSTICS_BUILD_FAILED: &str = "diagnostics.build-failed";
/// The Windows save dialog could not be started.
pub const DIAGNOSTICS_SAVE_DIALOG_FAILED: &str = "diagnostics.save-dialog-failed";
/// The ZIP could not be written to the chosen file; the technical error is the argument.
pub const DIAGNOSTICS_WRITE_FAILED: &str = "diagnostics.write-failed";
/// Open GitHub issue was clicked before a capture built the URL.
pub const DIAGNOSTICS_ISSUE_NOT_READY: &str = "diagnostics.issue-not-ready";
/// The device card's status tag: the connected model is supported.
pub const DIAGNOSTICS_SUPPORTED: &str = "diagnostics.supported";
/// The device card's firmware/battery line; both values are arguments.
pub const DIAGNOSTICS_META: &str = "diagnostics.meta";
/// A probe answered.
pub const DIAGNOSTICS_PROBE_DONE: &str = "diagnostics.probe.done";
/// A probe timed out.
pub const DIAGNOSTICS_PROBE_NO_ANSWER: &str = "diagnostics.probe.no-answer";
/// A probe answered with an empty reply.
pub const DIAGNOSTICS_PROBE_EMPTY_REPLY: &str = "diagnostics.probe.empty-reply";
/// A probe answered with a reply for another command.
pub const DIAGNOSTICS_PROBE_UNEXPECTED: &str = "diagnostics.probe.unexpected";
/// A probe's reply had a bad checksum.
pub const DIAGNOSTICS_PROBE_BAD_CHECKSUM: &str = "diagnostics.probe.bad-checksum";
/// A probe has not run yet.
pub const DIAGNOSTICS_PROBE_WAITING: &str = "diagnostics.probe.waiting";

/// The subject of a failed sound-mode write.
pub const SUBJECT_SOUND_MODE: &str = "subject.sound-mode";
/// The subject of a failed equalizer-preset write.
pub const SUBJECT_EQUALIZER_PROFILE: &str = "subject.equalizer-profile";
/// The subject of a failed custom-curve write.
pub const SUBJECT_EQUALIZER_BANDS: &str = "subject.equalizer-bands";
/// The subject of a failed A3062 double-press write.
pub const SUBJECT_DOUBLE_PRESS: &str = "subject.double-press";
/// The subject of a failed ambient-mode-cycle write.
pub const SUBJECT_MODE_CYCLE: &str = "subject.mode-cycle";
/// The subject of a failed low-battery-prompt write.
pub const SUBJECT_LOW_BATTERY_PROMPT: &str = "subject.low-battery-prompt";
/// The subject of a failed Dolby-audio write.
pub const SUBJECT_DOLBY_AUDIO: &str = "subject.dolby-audio";
/// The subject of a failed LDAC write.
pub const SUBJECT_LDAC: &str = "subject.ldac";
/// The subject of a failed dual-connections write.
pub const SUBJECT_DUAL_CONNECTIONS: &str = "subject.dual-connections";
/// The subject of a failed auto-power-off write.
pub const SUBJECT_AUTO_POWER_OFF: &str = "subject.auto-power-off";
/// The subject of a failed high-volume-limit write.
pub const SUBJECT_LIMIT_HIGH_VOLUME: &str = "subject.limit-high-volume";
/// The subject of a failed dB-limit refresh-rate write.
pub const SUBJECT_REFRESH_RATE: &str = "subject.refresh-rate";
/// The subject of a failed side-tone write.
pub const SUBJECT_SIDE_TONE: &str = "subject.side-tone";
/// The subject of a failed voice-prompt write.
pub const SUBJECT_VOICE_PROMPT: &str = "subject.voice-prompt";
/// The subject of a failed wearing-detection write.
pub const SUBJECT_WEARING_DETECTION: &str = "subject.wearing-detection";
/// The subject of a failed sound-leak-compensation write.
pub const SUBJECT_SOUND_LEAK_COMPENSATION: &str = "subject.sound-leak-compensation";
/// The subject of a failed spatial-audio write.
pub const SUBJECT_SPATIAL_AUDIO: &str = "subject.spatial-audio";
/// The subject of a failed easy-chat write.
pub const SUBJECT_EASY_CHAT: &str = "subject.easy-chat";
/// The subject of a failed case-screen write whose row is not specific.
pub const SUBJECT_CASE_SCREEN: &str = "subject.case-screen";
/// The subject of a failed case-language write.
pub const SUBJECT_CASE_LANGUAGE: &str = "subject.case-language";
/// The subject of a failed case-screen atmospheric toggle.
pub const SUBJECT_CASE_ATMOSPHERIC: &str = "subject.case-atmospheric";
/// The subject of a failed case-screen remote-camera toggle.
pub const SUBJECT_CASE_REMOTE_CAMERA: &str = "subject.case-remote-camera";
/// The subject of a failed case-screen find-device toggle.
pub const SUBJECT_CASE_FIND_DEVICE: &str = "subject.case-find-device";
/// The subject of a failed case-screen spatial-audio toggle.
pub const SUBJECT_CASE_SPATIAL_AUDIO: &str = "subject.case-spatial-audio";
/// The subject of a failed left-single-press write.
pub const SUBJECT_LEFT_SINGLE_PRESS: &str = "subject.left-single-press";
/// The subject of a failed right-single-press write.
pub const SUBJECT_RIGHT_SINGLE_PRESS: &str = "subject.right-single-press";
/// The subject of a failed left-double-press write.
pub const SUBJECT_LEFT_DOUBLE_PRESS: &str = "subject.left-double-press";
/// The subject of a failed right-double-press write.
pub const SUBJECT_RIGHT_DOUBLE_PRESS: &str = "subject.right-double-press";
/// The subject of a failed left-triple-press write.
pub const SUBJECT_LEFT_TRIPLE_PRESS: &str = "subject.left-triple-press";
/// The subject of a failed right-triple-press write.
pub const SUBJECT_RIGHT_TRIPLE_PRESS: &str = "subject.right-triple-press";
/// The subject of a failed left-long-press write.
pub const SUBJECT_LEFT_LONG_PRESS: &str = "subject.left-long-press";
/// The subject of a failed right-long-press write.
pub const SUBJECT_RIGHT_LONG_PRESS: &str = "subject.right-long-press";
/// The subject of a failed left-slide-up write.
pub const SUBJECT_LEFT_SLIDE_UP: &str = "subject.left-slide-up";
/// The subject of a failed right-slide-up write.
pub const SUBJECT_RIGHT_SLIDE_UP: &str = "subject.right-slide-up";
/// The subject of a failed left-slide-down write.
pub const SUBJECT_LEFT_SLIDE_DOWN: &str = "subject.left-slide-down";
/// The subject of a failed right-slide-down write.
pub const SUBJECT_RIGHT_SLIDE_DOWN: &str = "subject.right-slide-down";
/// The subject of a failed dual-connections list read.
pub const SUBJECT_DUAL_CONNECTIONS_LIST: &str = "subject.dual-connections-list";
/// The subject of a failed list-row connect.
pub const SUBJECT_CONNECT_DEVICE: &str = "subject.connect-device";
/// The subject of a failed list-row disconnect.
pub const SUBJECT_DISCONNECT_DEVICE: &str = "subject.disconnect-device";
/// The subject of a failed list-row forget.
pub const SUBJECT_FORGET_DEVICE: &str = "subject.forget-device";

/// The device did not answer a write or the state re-read.
pub const REASON_NO_ANSWER: &str = "reason.no-answer";
/// The Bluetooth link dropped before the write was confirmed.
pub const REASON_CONNECTION_LOST: &str = "reason.connection-lost";
/// The device answered with something this app cannot read.
pub const REASON_UNREADABLE_ANSWER: &str = "reason.unreadable-answer";

/// The word for a value the sentence cannot read; any other value argument is a
/// technical reading and passes through unchanged.
pub const PART_UNKNOWN: &str = "part.unknown";

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

/// The banner of a failed device write: the setting the user changed and why.
pub fn setting_write_failure(subject: &str, reason: &str) -> UiMessage {
    with_args(ERROR_SETTING_WRITE, subject, reason)
}

/// The banner of a failed saved-profile document write.
pub fn profile_write_failure() -> UiMessage {
    with_args(ERROR_PROFILE_WRITE, "", "")
}

/// A connection failure the window words itself; `payload` is a technical OS
/// detail for [`connect_transport`] and `""` otherwise.
pub fn connect_failure(key: &str, payload: &str) -> UiMessage {
    with_args(key, payload, "")
}

/// A transport failure with its technical OS detail as the argument.
pub fn connect_transport(detail: &str) -> UiMessage {
    connect_failure(CONNECT_TRANSPORT, detail)
}

/// The demo-mode refusal in the Add Device dialog.
pub fn demo_unavailable() -> UiMessage {
    with_args(DEMO_UNAVAILABLE, "", "")
}

/// The Add Device dialog's error for a paired device whose row cannot be read.
pub fn add_unreadable_paired() -> UiMessage {
    with_args(ADD_UNREADABLE_PAIRED, "", "")
}

/// The Add Device dialog's error for an add click with no chosen model.
pub fn add_choose_model() -> UiMessage {
    with_args(ADD_CHOOSE_MODEL, "", "")
}

/// The Add Device dialog's error for a failed paired-device enumeration worker.
pub fn add_paired_worker_start() -> UiMessage {
    with_args(ADD_PAIRED_WORKER_START, "", "")
}

/// A diagnostics error that is a complete sentence.
pub fn diagnostics_error(key: &str) -> UiMessage {
    with_args(key, "", "")
}

/// A diagnostics error that quotes a technical error string.
pub fn diagnostics_error_with_detail(key: &str, detail: &str) -> UiMessage {
    with_args(key, detail, "")
}

/// The demo-mode diagnostics explanation.
pub fn diagnostics_demo_unavailable() -> UiMessage {
    diagnostics_error(DIAGNOSTICS_DEMO_UNAVAILABLE)
}

/// The device card's status tag for a supported model.
pub fn diagnostics_supported() -> UiMessage {
    diagnostics_error(DIAGNOSTICS_SUPPORTED)
}

/// The device card's firmware/battery line; a missing reading is [`PART_UNKNOWN`].
pub fn diagnostics_meta(firmware: &str, battery: &str) -> UiMessage {
    with_args(DIAGNOSTICS_META, firmware, battery)
}

/// A probe row's state word.
pub fn probe_status(key: &str) -> UiMessage {
    with_args(key, "", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key the Rust model can put into a [`UiMessage`], so the catalog
    /// tests cannot miss one that was added without a branch.
    const ALL_KEYS: &[&str] = &[
        QUICK_TOGGLE_ONE_EARBUD,
        SETTING_TURN_ON_SPATIAL_AUDIO,
        ERROR_SETTING_WRITE,
        ERROR_PROFILE_WRITE,
        CONNECT_NOT_PAIRED,
        CONNECT_NOT_FOUND,
        CONNECT_TIMED_OUT,
        CONNECT_BLUETOOTH_OFF,
        CONNECT_CONNECTION_FAILED,
        CONNECT_NO_SERVICE,
        CONNECT_NO_PARSER,
        CONNECT_UNSUPPORTED_LAYOUT,
        CONNECT_PROTOCOL,
        CONNECT_TRANSPORT,
        CONNECT_WORKER_START,
        CONNECT_DEMO_WORKER_START,
        CONNECT_SAVED_ADDRESS,
        ADD_CHOOSE_MODEL,
        ADD_UNREADABLE_PAIRED,
        ADD_PAIRED_WORKER_START,
        DEMO_UNAVAILABLE,
        DIAGNOSTICS_DEMO_UNAVAILABLE,
        DIAGNOSTICS_WORKER_STOPPED,
        DIAGNOSTICS_NO_CAPTURE,
        DIAGNOSTICS_BUILD_FAILED,
        DIAGNOSTICS_SAVE_DIALOG_FAILED,
        DIAGNOSTICS_WRITE_FAILED,
        DIAGNOSTICS_ISSUE_NOT_READY,
        DIAGNOSTICS_SUPPORTED,
        DIAGNOSTICS_META,
        DIAGNOSTICS_PROBE_DONE,
        DIAGNOSTICS_PROBE_NO_ANSWER,
        DIAGNOSTICS_PROBE_EMPTY_REPLY,
        DIAGNOSTICS_PROBE_UNEXPECTED,
        DIAGNOSTICS_PROBE_BAD_CHECKSUM,
        DIAGNOSTICS_PROBE_WAITING,
        SUBJECT_SOUND_MODE,
        SUBJECT_EQUALIZER_PROFILE,
        SUBJECT_EQUALIZER_BANDS,
        SUBJECT_DOUBLE_PRESS,
        SUBJECT_MODE_CYCLE,
        SUBJECT_LOW_BATTERY_PROMPT,
        SUBJECT_DOLBY_AUDIO,
        SUBJECT_LDAC,
        SUBJECT_DUAL_CONNECTIONS,
        SUBJECT_AUTO_POWER_OFF,
        SUBJECT_LIMIT_HIGH_VOLUME,
        SUBJECT_REFRESH_RATE,
        SUBJECT_SIDE_TONE,
        SUBJECT_VOICE_PROMPT,
        SUBJECT_WEARING_DETECTION,
        SUBJECT_SOUND_LEAK_COMPENSATION,
        SUBJECT_SPATIAL_AUDIO,
        SUBJECT_EASY_CHAT,
        SUBJECT_CASE_SCREEN,
        SUBJECT_CASE_LANGUAGE,
        SUBJECT_CASE_ATMOSPHERIC,
        SUBJECT_CASE_REMOTE_CAMERA,
        SUBJECT_CASE_FIND_DEVICE,
        SUBJECT_CASE_SPATIAL_AUDIO,
        SUBJECT_LEFT_SINGLE_PRESS,
        SUBJECT_RIGHT_SINGLE_PRESS,
        SUBJECT_LEFT_DOUBLE_PRESS,
        SUBJECT_RIGHT_DOUBLE_PRESS,
        SUBJECT_LEFT_TRIPLE_PRESS,
        SUBJECT_RIGHT_TRIPLE_PRESS,
        SUBJECT_LEFT_LONG_PRESS,
        SUBJECT_RIGHT_LONG_PRESS,
        SUBJECT_LEFT_SLIDE_UP,
        SUBJECT_RIGHT_SLIDE_UP,
        SUBJECT_LEFT_SLIDE_DOWN,
        SUBJECT_RIGHT_SLIDE_DOWN,
        SUBJECT_DUAL_CONNECTIONS_LIST,
        SUBJECT_CONNECT_DEVICE,
        SUBJECT_DISCONNECT_DEVICE,
        SUBJECT_FORGET_DEVICE,
        REASON_NO_ANSWER,
        REASON_CONNECTION_LOST,
        REASON_UNREADABLE_ANSWER,
        PART_UNKNOWN,
    ];

    #[test]
    fn russian_messages_resolve_through_the_slint_catalog() {
        // The data-level tests above prove the catalog and `.po` agree; this
        // test proves the compiled window resolves a key end-to-end, including a
        // composed sentence whose subject/reason come from MessageParts.
        crate::test_support::with_ui_window(|ui| {
            slint::select_bundled_translation("ru").expect("Russian is a bundled translation");
            slint::update_all_translations();
            use slint::ComponentHandle as _;
            let catalog = ui.global::<crate::Messages>();
            assert_eq!(
                catalog.invoke_text(quick_toggle_one_earbud()).as_str(),
                "Изменение доступно, когда подключены оба наушника"
            );
            assert_eq!(
                catalog
                    .invoke_text(setting_write_failure(SUBJECT_LDAC, REASON_NO_ANSWER))
                    .as_str(),
                "Не удалось изменить «LDAC (нет ответа)». На устройстве осталось прежнее значение."
            );
            assert_eq!(catalog.invoke_text(empty()).as_str(), "");
            assert_eq!(
                catalog
                    .invoke_text(with_args("unknown.key", "", ""))
                    .as_str(),
                ""
            );
            // The translation selection is process-wide, so leave the other UI
            // tests in the source language.
            slint::select_bundled_translation("en").expect("English is the source language");
            slint::update_all_translations();
        });
    }

    #[test]
    fn the_migrated_sentence_setters_take_messages_not_literals() {
        // A second guard in addition to the field types: no call may hand a
        // string literal to a field that used to carry UI copy. Textual scan, so
        // it also catches a test or a new path that bypassed the constructors.
        let controller = include_str!("controller.rs");
        for setter in [
            "set_error_label",
            "set_failure_text",
            "set_connection_reason",
            "set_add_error",
            "set_diagnostic_error",
            "set_diagnostic_status",
            "set_diagnostic_device_meta",
        ] {
            assert!(
                !controller.contains(&format!("{setter}(\"")),
                "{setter} is called with a string literal; pass a UiMessage instead"
            );
        }
    }

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
    }

    #[test]
    fn a_setting_write_failure_carries_the_subject_and_reason_keys() {
        let message = setting_write_failure(SUBJECT_LDAC, REASON_NO_ANSWER);
        assert_eq!(message.key.as_str(), ERROR_SETTING_WRITE);
        assert_eq!(message.arg0.as_str(), SUBJECT_LDAC);
        assert_eq!(message.arg1.as_str(), REASON_NO_ANSWER);
        // The composition happens in the catalog; Rust holds no sentence.
        assert!(!message.arg0.contains(' '));
        assert!(!message.arg1.contains(' '));
        assert!(!profile_write_failure().key.is_empty());
        assert!(profile_write_failure().arg0.is_empty());
    }

    #[test]
    fn every_key_names_a_catalog_branch_and_its_source_is_translated() {
        // Keys are internal identifiers: each must appear in the Slint catalog,
        // and the English source the catalog returns for it must have a
        // non-empty Russian translation. This is the same data chain
        // `Messages.text` follows at runtime.
        let catalog = include_str!("../ui/messages.slint");
        let russian = include_str!("../translations/ru/LC_MESSAGES/soundcore-desktop.po");
        for key in ALL_KEYS {
            assert!(
                catalog.contains(&format!("\"{key}\"")),
                "ui/messages.slint has no branch for the message key {key}"
            );
            let marker = format!("\"{key}\"");
            let after_key = &catalog[catalog
                .find(&marker)
                .unwrap_or_else(|| panic!("catalog has no key {key}"))
                + marker.len()..];
            let source = catalog_source(after_key)
                .unwrap_or_else(|| panic!("the branch for {key} returns no @tr source"));
            assert!(
                !source.is_empty(),
                "the branch for {key} has an empty source"
            );
            assert!(
                !russian.contains(&format!("msgid \"{source}\"\nmsgstr \"\"")),
                "the Russian catalog has an empty msgstr for {source:?} ({key})"
            );
            assert!(
                russian.contains(&format!("msgid \"{source}\"")),
                "the Russian catalog has no msgid for {source:?} ({key})"
            );
        }
    }

    /// The first @tr source returned by the code that follows a key comparison,
    /// e.g. `\"error.setting-write\") { return @tr(\"…\") }`.
    fn catalog_source(after_key: &str) -> Option<&str> {
        let start = after_key.find("@tr(\"")? + "@tr(\"".len();
        let rest = &after_key[start..];
        let mut escaped = false;
        for (index, character) in rest.char_indices() {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return Some(&rest[..index]);
            }
        }
        None
    }

    #[test]
    fn message_sources_live_in_the_slint_catalog_not_in_rust() {
        // The guard for the migrated model fields: the Rust modules carry keys,
        // the English sentences sit in the Slint catalog, and the translation
        // document owns the Russian copy. Only the `Messages` half of the
        // catalog is a sentence; `MessageParts` holds single words that the model
        // may still carry as device-id label data (e.g. "LDAC").
        let catalog = include_str!("../ui/messages.slint");
        let messages = catalog
            .split_once("export global Messages")
            .expect("the catalog exports Messages")
            .1;
        let mut sources = Vec::new();
        let mut rest = messages;
        while let Some(start) = rest.find("@tr(\"") {
            let after = &rest[start + "@tr(\"".len()..];
            let mut escaped = false;
            let mut end = None;
            for (index, character) in after.char_indices() {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    end = Some(index);
                    break;
                }
            }
            let Some(end) = end else { break };
            sources.push(after[..end].to_owned());
            rest = &after[end..];
        }
        assert!(sources.len() > 20, "the catalog carries the migrated copy");
        for rust in [
            include_str!("controller.rs"),
            include_str!("overview.rs"),
            include_str!("demo.rs"),
        ] {
            for source in &sources {
                // A sentence must not be in Rust at all; a single word is also
                // checked as a quoted literal (the guard looks for UI copy, not
                // an identifier such as the `DiagnosticStep::Done` variant).
                assert!(
                    !(source.contains(' ') || source.contains('.'))
                        || !rust.contains(source.as_str()),
                    "Rust carries a ready-made UI sentence: {source}"
                );
                assert!(
                    !rust.contains(&format!("\"{source}\"")),
                    "Rust carries the UI copy as a literal: {source}"
                );
            }
        }
    }

    #[test]
    fn the_migrated_state_fields_are_messages_not_strings() {
        // The structural guarantee behind the guard: the fields that used to
        // carry ready-made English sentences are typed `UiMessage`, so a caller
        // cannot pass a sentence without a compiler error.
        let types = include_str!("../ui/types.slint");
        for field in [
            "connection-reason",
            "failure-text",
            "add-error",
            "error-label",
            "diagnostic-device-meta",
            "diagnostic-status",
            "diagnostic-error",
        ] {
            assert!(
                types.contains(&format!("in property <UiMessage> {field};")),
                "AppState.{field} must be a UiMessage"
            );
        }
        assert!(
            types.contains("status: UiMessage"),
            "DiagnosticProbe.status must be a UiMessage"
        );
    }
}
