# 03 — Runtime message catalog

**Status:** done
**Blocked by:** —
**Implemented:** 2026-11-23

## Goal

Rust must not contain ready-made user-facing English sentences. It supplies
message keys and arguments; Slint resolves them through `@tr` and `.po`.

## Requirements

1. Add `desktop/ui/messages.slint` with `UiMessage { key, arg0, arg1 }` and
   `export global Messages { public pure function text(UiMessage) -> string }`.
   Each branch is `@tr("English source", args...)`.
2. Add `desktop/src/messages.rs` with typed key constants and constructors; no
   display sentences. Rust model fields carry `UiMessage` (or key/args).
3. Change `QuickToggle.reason` and `SettingRow.reason` to `UiMessage`; UI renders
   `Messages.text`. Remove the English LDAC reason from `overview.rs`.
4. Add Russian `.po` entries for every new source string.
5. Update `LocalizedText` only for device-id labels/options; messages for
   sentences belong to `Messages`.
6. Tests: quick-toggle reason key/args, setting reason mapping, Russian text
   resolution (via UI model), no raw English reason in Rust.
7. Update UI_KIT/GUIDE/SOURCE with the contract.

## Acceptance

- LDAC quick-toggle reason follows the app language; no user-facing sentence is
  hardcoded in Rust for these fields.

## Implementation (2026-11-23)

- `desktop/ui/types.slint`: new `export struct UiMessage { key, arg0, arg1 }` at
  the top of the file, before the models that use it. `SettingRow.reason` and
  `QuickToggle.reason` changed from `string` to `UiMessage`, so the generated
  Rust model fields carry a key and arguments, never a sentence; the empty key
  is the empty message. Other `AppState` fields are untouched (ticket 04).
- `desktop/ui/messages.slint` (new): `export global Messages` with
  `public pure function text(UiMessage) -> string`. Branches compare the internal
  key and return the English source through `@tr`; `message.arg0`/`arg1` fill the
  source's `{}` placeholders where a branch has them. Empty and unknown keys
  return `""`. The English LDAC reason and the existing spatial-audio reason live
  here, so the `.po` catalogs translate them like static copy.
- `desktop/src/messages.rs` (new): typed key constants
  (`QUICK_TOGGLE_ONE_EARBUD`, `SETTING_TURN_ON_SPATIAL_AUDIO`), the `with_args`
  constructor, the `empty()` helper and one constructor per migrated reason. No
  display sentence lives here; tests assert the keys, empty arguments, that
  `messages.slint` has a branch per key, and that the English sources exist only
  in the Slint catalog and the Russian document.
- `desktop/ui/components/settings.slint`: the row reason renders through
  `Messages.text(root.setting.reason)` when `reason.key != ""`.
  `LocalizedText.setting-reason` is removed from
  `desktop/ui/components/localized.slint`; `LocalizedText` keeps the device-id
  label/text and option-value mappings. `desktop/ui/pages/overview.slint`
  renders a disabled quick toggle's reason through `Messages.text`.
- `desktop/src/overview.rs`: the LDAC one-earbud reason is
  `messages::quick_toggle_one_earbud()`, and available toggles send
  `messages::empty()`. `desktop/src/controller.rs`: the two disabled
  spatial-audio rows share `messages::turn_on_spatial_audio_first()`; the shared
  row constructors start with the empty message. Device capability rules and the
  visible strings are unchanged; only their delivery is now the catalog.
- `desktop/translations/ru/LC_MESSAGES/soundcore-desktop.po`: new msgid
  `Changing is available when both earbuds are connected` with its Russian
  translation. The spatial reason reuses the existing
  `Turn on spatial audio first` msgid. The preview's wrapped reason string is
  also translated (see below).
- `.scratch/ui-preview`: the tracked preview harness builds rows with `UiMessage`
  values (`message(...)`) instead of raw strings. The ticket-02 long-row fixture
  uses the preview-only `preview.long-reason` branch, so wrapped-reason coverage
  keeps going through the same `Messages` path; the English source stays in
  `messages.slint` and no production message uses that key.
- `desktop/ui/UI_KIT.md`, `GUIDE.md`, `SOURCE.md` and the UI-kit agent skill
  record the contract: UI-facing sentences from Rust are `UiMessage` + `Messages`
  (`key` is an identifier, not UI copy; args fill the `@tr` source);
  `LocalizedText` remains for device-id labels and options. Diagnostics, error,
  failure and connection-reason fields stay as they are for ticket 04.

## Verification

- The message chain is tested through the UI model:
  `overview::tests::the_quick_toggle_reason_resolves_to_russian_copy` builds the
  one-earbud `QuickToggle` from the device state, takes its reason key, finds the
  English source in `messages.slint`, and asserts the `ru` document carries a
  non-empty Russian `msgstr` for that source — the same data chain
  `Messages.text` follows at runtime.
- Rust source guard: `messages::tests::
  english_reason_sources_live_in_the_slint_catalog_not_in_rust` asserts the two
  reason sources are in `messages.slint` and the Russian `.po`, and do not appear
  in `overview.rs`/`controller.rs`; `messages::tests` also covers the key/argument
  round trip, the empty message, and that every `messages.rs` key has a
  `Messages` branch.
- `overview::tests::a_disabled_quick_toggle_reason_is_a_message_key` and
  `an_available_quick_toggle_has_the_empty_reason` cover the quick-toggle model;
  `controller::tests::the_row_constructors_start_with_the_empty_reason` and
  `a_disabled_setting_row_reason_names_its_catalog_key` cover the setting rows,
  and the existing Effects-page test now asserts the key and the empty arguments
  of both spatial rows.
- Desktop gates, from `desktop`: `cargo fmt --all -- --check`, `cargo build`,
  `cargo test` (301 passed / 0 failed / 15 ignored, hardware only),
  `cargo clippy --all-targets -- -D warnings` — green. Core is untouched but was
  re-run: `cargo fmt --all -- --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (195 passed / 0 failed).
- The tracked `.scratch/ui-preview` harness compiles against the new model
  (`cargo check`, Slint 1.19) and keeps its wrapped-row fixture through the
  preview-only `preview.long-reason` catalog branch.

