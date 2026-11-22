# 01 — Build recovery

**Status:** ready-for-agent
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

- [ ] `cargo build` and `cargo test` green in desktop and core.
- [ ] Translation files load from the expected domain.
- [ ] Tray/settings persistence tests green.
- [ ] No new Slint warnings that the UI-kit contract requires fixing.
