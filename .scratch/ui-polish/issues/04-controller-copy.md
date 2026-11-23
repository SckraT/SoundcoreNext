# 04 — Migrate controller UI copy

**Status:** done
**Blocked by:** 03
**Implemented:** 2026-11-24

## Requirements

1. Audit controller-originated display strings: error labels, failure text,
   connection reasons, add-device errors, diagnostics errors, setting-write
   failure labels/reasons, toast-adjacent copy.
2. Replace every user-facing sentence with a `UiMessage` key + args; technical
   stderr/log lines, URLs, file names and protocol ids stay as code.
3. Add `Messages` branches and Russian translations for each.
4. Update Slint `AppState` fields (`error-label`, `failure-text`, `add-error`,
   `diagnostic-error`, `connection-reason`, etc.) to `UiMessage` and all UI
   renderers to `Messages.text`.
5. Add a test/guard that Rust UI fields no longer accept raw display sentences
   (at least for the migrated fields), and controller tests assert keys/args.
6. No behavior change; all existing tests adapted.

## Acceptance

- All controller-originated visible copy is translated via `.po`; no English
  sentences in Rust for UI.

## Implementation (2026-11-24)

- `desktop/ui/types.slint`: `AppState.connection-reason`, `failure-text`,
  `add-error`, `error-label`, `diagnostic-device-meta`, `diagnostic-status` and
  `diagnostic-error` are `UiMessage` instead of `string`; `DiagnosticProbe.status`
  is a `UiMessage` too. The structural type change is the compiler-level guard
  against a caller passing a sentence. Identifiers and technical values stay
  `string`: device/model names, `diagnostic-file-name`, `diagnostic-issue-url`,
  `diagnostic-note`, `diagnostic-archive-size`, `FailureKind`, the connection and
  toast enums and the numeric fields.
- `desktop/ui/messages.slint`: the `Messages` half of the catalog now owns every
  controller sentence:
  - the failed-write banner, composed from argument keys:
    `Could not change “{} ({})”. The device kept its previous value.`;
  - the saved-profile document failure as its own sentence (the old label was
    wrapped by the device-change sentence, which described a file failure
    wrongly);
  - one branch per `ConnectFailure` (not paired, not found, timed out, Bluetooth
    off, connection failed, no service, no parser, unsupported layout, protocol,
    transport, worker start, demo worker start, unreadable saved address). The OS
    transport detail is a technical argument inside the translated sentence;
  - the Add Device errors (choose a model, unreadable paired device, enumeration
    worker start, demo unavailable);
  - the Diagnostics errors (demo mode, worker stopped, no capture, archive build,
    save dialog, archive write, issue link), the device-card status/meta line and
    each probe status word.
  A new `MessageParts` global resolves the words a sentence quotes from the
  argument keys: `SUBJECT_*` (the setting/action a failed write names, including
  the specific A3954 case-screen flag and gesture), `REASON_*` (no answer,
  connection lost, unreadable answer) and `PART_UNKNOWN` (the word for a missing
  firmware/battery reading). Any other value argument is a technical reading and
  passes through unchanged, so the catalog never receives English prose from Rust.
- `desktop/src/messages.rs`: constants and constructors for every new branch
  (`setting_write_failure`, `profile_write_failure`, the `connect_*`, `add_*`,
  `demo_*` and `diagnostics_*` sets, probe statuses, the subject/reason/part keys)
  and tests that:
  - assert the key/argument shape of the composed banner and profile-failure
    message;
  - assert every declared key has a catalog branch whose English source has a
    non-empty Russian translation in the `.po`;
  - assert the migrated `AppState` fields are declared `<UiMessage>` in
    `types.slint` (the structural guard) and that no Rust call passes a string
    literal to one of the migrated sentence setters;
  - assert the English sentences of the `Messages` catalog do not appear in
    `controller.rs`/`overview.rs`/`demo.rs` (single words are checked as quoted
    literals so enum variants such as `DiagnosticStep::Done` do not false-positive).
- `desktop/src/controller.rs`: every controller-originated UI sentence is now a
  message:
  - `ModeFailure::message`, `EqFailure::message`/`bands_message`,
    `SettingFailure::message`/`row_message`, `DualFailure::list_message`/
    `action_message` replace the English label builders; `setting_subject`,
    `a3954_setting_subject`, `a3954_setting_row_subject` and
    `a3954_button_subject` map a command to its banner subject key, and
    `a3954_setting_failure_message`/`a3954_button_failure_message` return the
    composed message;
  - `ConnectFailure` gained `WorkerStart`, `DemoWorkerStart` and `SavedAddress`
    for the app's own failures (they used to be English text inside a
    `Transport(String)`), and `message()` replaces `text()`; the enumeration
    worker failure is a distinct `PairedFailure::WorkerStart`;
  - every `set_error_label`/`set_failure_text`/`set_connection_reason`/
    `set_add_error`/`set_diagnostic_*` call passes a message, and clears pass
    `messages::empty()`; the saved-profile banner is compared by message value,
    not by an English constant;
  - `show_diagnostics_device` no longer branches on the language in Rust: it
    passes the firmware/battery readings (or `part.unknown`) to the catalog, which
    also fixes the English branch that appended a second `%` to the battery;
  - `diagnostic_probe_rows` statuses are catalog keys and the saved-size/detail
    values stay technical data;
  - the save worker passes the raw IO error string as the message argument
    instead of formatting an English sentence on the worker thread;
  - `EqPage`, `SaveDialogPage` and `ImportPage` test seams take `UiMessage`.
- `desktop/src/demo.rs`: the demo-mode refusal and demo Diagnostics explanation
  (and their hardcoded Russian branch) moved into the catalog and were removed
  from Rust.
- `desktop/ui/pages/`: `app.slint` (error banner, reconnecting banner),
  `devices.slint` (failed connection and lost reason), `add_device.slint` (error
  line), `diagnostics.slint` (device status/meta, error banners, probe status) and
  `overview.slint` (the power/spatial shortcut values are localized option data
  via `LocalizedText.option`, not sentences) render through `Messages.text`; the
  obsolete `FailureText` global and its imports were removed.
- `desktop/translations/ru/LC_MESSAGES/soundcore-desktop.po`: translations for
  every new source (connection failures, Add Device, Diagnostics, the subject and
  reason words) and the banner msgid now takes two placeholders; no duplicate
  msgid.
- Tests adapted: existing controller tests now assert the key/args of the
  migrated paths (`assert_write_failure` helper, `ConnectFailure::message`
  mapping tests including the transport argument and internal worker variants,
  probe-status keys, A3954 subject mapping including the specific case flags,
  demo/Add Device message equality, hardware-test prints of the key), and the
  mocks store `UiMessage` values.
- Documentation: `ui/UI_KIT.md`, `ui/GUIDE.md`, `ui/SOURCE.md` and both READMEs
  record the field list, the composed-message contract, the technical-argument
  rule and the explicit non-migrated surfaces.

## Verification

- `desktop`: `cargo fmt --all -- --check`, `cargo build`, `cargo test`
  (306 passed / 0 failed / 15 ignored hardware-only + 7 translation tests),
  `cargo clippy --all-targets -- -D warnings` — green.
- Guard: `messages::tests::the_migrated_state_fields_are_messages_not_strings`
  fails if a migrated field becomes a `string` again;
  `message_sources_live_in_the_slint_catalog_not_in_rust` fails if any catalog
  sentence re-appears in `controller.rs`, `overview.rs` or `demo.rs`.
- No hardware was needed: the demo worker exercises connection, settings, mode,
  equalizer, diagnostics and tray paths in the tests. The ignored hardware tests
  only had their printed values changed to message keys.

## Deliberately not migrated (documented)

- stderr/log lines, the local file names and the GitHub issue URL/body, protocol
  ids and enum values (`NoiseCanceling`, `ldac`, `04 81`, …), option data values
  localized by `LocalizedText`, and product/model names.
- the dual-connections list's `— connected` marker
  (`dual_device_option`): the row model is a plain list of option strings whose
  device name is data, so the marker is composed value decoration rather than a
  sentence; a translated marker would need a structured list-row model. The
  adjacent `Unknown device` fallback is translated by `LocalizedText.option`.
- the native notification-area menu wording (`desktop/src/tray.rs`): it is built
  from tray data, not from the Slint `Messages` catalog, and is the largest
  remaining untranslated UI surface; the README records it.
- the native file-dialog chrome (the import/export dialog titles and the
  `JSON`/`ZIP archive`/`All files` filter names in `NativeTransferIo` and the
  Diagnostics save dialog): these are OS dialog strings created on worker threads
  through `rfd`, never Slint components. A filter name is a format identifier
  here; the titles are documented as OS chrome. Translating them would need a
  separate Rust-side translation channel, which is outside this ticket's
  "reaches Slint" contract.
