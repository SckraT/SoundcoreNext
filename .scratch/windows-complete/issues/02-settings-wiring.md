# 02 — Settings wiring

**Status:** done
**Blocked by:** 01

## Goal

Wire and persist every control the Settings page offers. The 1–11 sound-mode view
is deferred until a core model exposes that scale, so its control stays hidden
rather than becoming a fake choice.

## Requirements

1. Load `DesktopSettings` at startup; keep the document at
   `%APPDATA%\OpenSCL4\desktop-preferences.json` (atomic tmp+rename).
2. Implement handlers for all `Actions`:
   - `set-app-theme`, `set-palette`, `set-device-color`, `set-mode-view`,
     `set-content-scale`, `set-badge`, `set-language`, `copy-logs`.
3. Apply settings to `AppState` and the Slint globals:
   - `Theme.dark`, `Theme.palette`, `Theme.content-scale`;
   - device art in the selected silver/black color through
     `catalog_adapter::art_in_color`, including the current device and cards;
   - `app-mode-view-applies` (false until a core model exposes the 1–11 scale)
     and `one-earbud-badge-applies` from the connected state;
   - `language-names` (Follow system + bundled languages) and `app-language`.
4. Language: `slint::select_bundled_translation` with the chosen code or the
   system locale; update after window is shown; fallback English safely.
5. Follow-system theme reads Windows app mode and reacts after the native window
   is shown; no crash if the window/theme is unavailable.
6. Copy logs puts `logs::recent()` on the clipboard and shows the Copied toast;
   failures are logged, never fatal.
7. Tray choices persist through the same document, not a second file.
8. Tests: choice mapping, index validation, persistence roundtrip, light/dark and
   language selection, missing/malformed document, copy logs (through a seam).
9. Do not change device write behavior.

## Acceptance

- [x] Settings changes take effect immediately and survive restart.
- [x] Language and theme apply to the running window.
- [x] Copy logs works.
- [x] Desktop/core gates green.

## Verification

- Startup/persistence: `Controller::new` still loads `DesktopSettings` from
  `%APPDATA%\OpenSCL4\desktop-preferences.json` and keeps the document path; the
  tray and Settings handlers all save the whole document through the atomic
  `settings::save_settings` (tmp+rename), so there is still exactly one
  preferences file. `persist_tray_settings` was renamed `persist_settings`,
  because it no longer belongs to the tray alone.
- Rendering: `render_settings(state, inner)` maps every saved choice to
  `AppState` — `app-theme`, `app-palette`, `app-content-scale`,
  `app-device-color`, `app-mode-view` — sets `app-language` from
  `settings::language_index`, fills `language-names` with the translated
  `Strings.system-language` followed by all 16 `settings::LANGUAGES` names,
  offers `one-earbud-badge-applies` only for a connected TWS pair (`DeviceState`
  with left/right batteries; the single-battery A3062 gets false), sets
  `show-one-earbud-badge`, and fills the notification-area fields through the
  existing `show_tray_settings`. `app-mode-view-applies` stays false: the clean
  core has no 1–11 modes-scale control, so no connected device can use that
  view. The 1–11 Sound-mode view is deferred until a core model exposes that
  scale; the saved choice is still loaded for later, but the Settings control
  remains hidden and no fake view is implemented.
- Theme: `apply_theme(inner, dark)` resolves Follow system once with
  `settings::system_prefers_dark`, writes `Theme.dark`, `Theme.palette` (from the
  saved accent) and `Theme.content-scale` (the saved content-scale factor; the
  150 % choice was added in ticket 06), and repaints the current device, every
  saved card, the Add Device model list and
  the chosen-model confirmation picture through
  `catalog_adapter::form_factor`/`art_in_color`. A model the catalog does not know
  keeps the picture it has, and with no model the default art stays. The paths
  that rewrite a picture after the theme was applied (pairing, retrying a saved
  card, filtering the model list, choosing a model) repaint in the chosen color,
  so a filter click cannot reset a black appearance to silver.
- Language: `apply_language(inner)` selects the saved code or
  `settings::preferred_language` (which falls back to `settings::system_language`)
  with `slint::select_bundled_translation`, logs and restores English when Slint
  does not bundle the code, then calls `slint::update_all_translations()` and
  renders the Settings fields again so "Follow system" is in the new language.
- `Controller::shown` applies the theme first (Follow system needs the native
  window), then the language, then starts the Follow-system poll. While the saved
  theme is `System`, a 2 s `slint::Timer` re-reads the real Windows app mode and
  repaints only when it changed; choosing Light or Dark drops the timer, and a
  missing window is a quiet no-op. `Controller` retains `Inner` (new field) and
  the timer lives in it, so the poll lives exactly as long as the controller and
  never keeps a dropped window alive (the timer callback holds a `Weak`).
- Actions: `set-app-theme`, `set-palette`, `set-device-color`, `set-mode-view`,
  `set-content-scale`, `set-badge`, `set-language` and `copy-logs` are wired.
  Each validates its index first (an invalid index changes nothing and, with no
  path, writes nothing), mutates its own saved field, saves the document, and
  applies/renders immediately. `copy-logs` puts `logs::recent()` on the clipboard
  through the existing `TransferIo` seam and shows `ToastKind::Copied`; a
  clipboard failure is only logged.
- No `.slint` file changed; no device write path changed; no hardware run.
- Tests: 17 new tests — `settings.rs` preferred-language helper; controller
  palette mapping, language-name model, every `AppState` index and both applies
  flags, theme/palette/scale application, black and silver art for the current
  device, cards, model list and Add Device picture, default art without a model,
  unknown-model art kept, language selection and English fallback, Follow-system
  poll lifecycle, no-window no-panic, immediate action effects, document
  persistence that preserves the tray fields, invalid indexes, Copy logs and the
  clipboard-failure seam.
- `desktop` gates on the final tree: `cargo fmt --all --check` clean, `cargo
  build` clean, `cargo test` **261 passed / 0 failed / 13 ignored** (the same 13
  manual hardware checks as ticket 01), `cargo clippy --all-targets -- -D
  warnings` clean.
- `core` gates (untouched by this ticket, run for the slice): `cargo fmt --all
  --check` clean, `cargo build` clean, `cargo test` **156 passed / 0 failed / 0
  ignored** across 22 suites including doc-tests, `cargo clippy --all-targets --
  -D warnings` clean.

## Integration

The ticket branch was based on the `integration/windows-complete` tip
`48b1e0b`; the tip had not moved when the ticket finished, so
`git merge integration/windows-complete` is a no-op and there is no merge commit
to make.
