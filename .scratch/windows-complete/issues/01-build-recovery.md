# 01 — Build recovery

**Status:** done
**Blocked by:** —

## Goal

Make the imported UI-kit WIP compile and pass the existing tests without
reverting the redesign.

## Known blockers

- Bundled translations are named `openscq30-desktop.po`; Slint expects
  `soundcore-desktop.po` (crate name). Rename all 16 locale files and update
  `Project-Id-Version`; keep `DefaultTranslationContext::None` compatible.
- `logs.rs:87` calls `line("Application started")` but `line` takes `String`.
- `controller.rs` calls `crate::tray::load_settings/save_settings`; the real
  `DesktopSettings` load/save live in `settings.rs`. Use those (flattened tray
  settings) and remove the stale tray test helpers or move them to settings.
- `main.rs` calls `Controller::shown()`, which does not exist yet.
- Tray tests refer to missing `fs`/load/save helpers.
- Unused imports in controller show unwired settings; wiring is ticket 02.

## Requirements

1. Rename translation files and make the build script load them.
2. Fix logs type error.
3. Make settings/tray persistence compile and round-trip; keep the user document
   missing/malformed-safe and atomic.
4. Add the minimal `Controller::shown` hook needed for later theme/language work.
5. Run desktop/core fmt, check, clippy, tests; report ignored hardware tests
   separately.
6. No device or UI behavior changes beyond compile/persistence corrections.

## Acceptance

- [x] `cargo build` and `cargo test` green in desktop and core.
- [x] Translation files load from the expected domain.
- [x] Tray/settings persistence tests green.
- [x] No new Slint warnings that the UI-kit contract requires fixing.

## Verification

- `desktop` gates on the final tree: `cargo fmt --all --check` clean,
  `cargo build` clean (no warnings), `cargo test` **244 passed / 0 failed / 13
  ignored**, `cargo clippy --all-targets -- -D warnings` clean.
- The 13 ignored tests are the manual hardware checks only (12 controller
  `manual_*` runs and `transport_windows::tests::manual_paired_enumeration`,
  each marked `#[ignore = "manual hardware check: needs a paired …"]`); no
  hardware was run for this ticket.
- `core` gates (untouched by this ticket, run for the slice): `cargo fmt --all
  --check` clean, `cargo build` clean, `cargo test` **156 passed / 0 failed / 0
  ignored** across 22 suites including doc-tests, `cargo clippy --all-targets
  -- -D warnings` clean.
- Translation domain: the 15 `desktop/translations/*/LC_MESSAGES/
  soundcore-desktop.po` documents parse, and both generated Slint units carry
  every bundled language (`_SLINT_BUNDLED_TRANSLATIONS` lists `arz, bn, de, es,
  fr, he, id, it, ja, ko, pl, pt-BR, ru, tr, uk` plus the source language), so
  App language and Follow system have real catalogs; `DefaultTranslationContext
  ::None` is kept because the documents are context-free.
- Logs: `logs::line` takes `String`; the startup line is passed as a `String`
  and the bounded-history test passes after binding its text for the borrow.
- Settings/tray persistence: `Controller::new` loads a `DesktopSettings`, keeps
  it in `Inner`, initializes the tray from `DesktopSettings::tray` and saves the
  whole document through the atomic `settings::save_settings`, so tray toggles
  preserve the appearance/language fields ticket 02 wires. The controller test
  sets non-default theme and language, flips every tray switch, reloads and
  asserts both the tray values and the preserved fields; `settings.rs` covers
  missing, malformed and old flat documents plus the tmp+rename save.
- `Controller::shown` exists as the documented no-op hook so `main.rs` compiles;
  the TODO names ticket 02 for theme and language.
- The stale `tray.rs` document tests were removed because `settings.rs` owns
  that document and its tests (`missing_document_loads_the_defaults` was added
  there); the tray behavior tests (tooltip, menu, render, close rules) remain.
- Unwired helpers kept for ticket 02 (`settings.rs` mappings, `logs::recent`,
  `catalog_adapter::form_factor`) are annotated with the repository's
  ticket-scoped `allow(dead_code)` pattern; imports only the unwired Settings
  controls need were removed.
- No `.slint` file changed; no device or UI behavior changed beyond compile and
  persistence corrections.

## Integration

The ticket branch was based on the `integration/windows-complete` tip
`8428061`; the tip had not moved when the ticket finished, so
`git merge integration/windows-complete` is a no-op and there is no merge commit
to make.
