# 03 — Wire the catalog into Add Device

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Populate the vendored Add Device dialog from the adapter. The UI files are not edited; all data and callbacks are provided from Rust through the generated `AppState` global.

## Pointers

- `desktop/ui/types.slint`: `AppState.models`, `model-kind`, `model-query`, `add-step`, `adding`, `add-model-name`, `add-model-code`, `add-model-art`, `add-model-is-speaker`, `paired-options`, `add-loading`.
- `desktop/ui/pages/add_device.slint`:
  - `Actions.filter-models(query, kind)` is called by the search field and category chips;
  - `Actions.choose-model(code)` is called by a model card;
  - the model grid renders `AppState.models`.
- Generated Slint API examples live in `desktop/target/debug/build/soundcore-desktop-*/out/app.rs` after a build; inspect that file rather than guessing names.
- The adapter from ticket 02 is the only source of model data.

## Requirements

1. On startup, `AppState.models` contains all 39 catalog models as `ModelOption` (code, name, art).
2. `on_filter_models(query, kind)` replaces `AppState.models` with `catalog_adapter::filter_models`.
3. `on_choose_model(code)`:
   - sets `add-model-name`, `add-model-code`, `add-model-art`;
   - sets `add-model-is-speaker` from the catalog form factor;
   - advances `add-step` to 1 and keeps `adding` true;
   - does not start a connection, pair or transport.
4. Unknown code is ignored safely (no panic).
5. No `.slint` edits. If the generated API makes a requirement impossible without UI changes, document the exact blocker in the ticket instead of redesigning the screen.

## Acceptance

- [x] `cargo build` and `cargo test` pass.
- [x] Running the app and opening Add Device shows the catalog (manual or automated check).
- [x] Search `A3954` shows Liberty 4 Pro; `P20` shows P20i/P25i/R50i; unknown text shows the existing empty state.
- [x] Category chips filter as specified.
- [x] Clicking a model reaches the existing paired-device step with the right name/code.
- [x] No protocol code or connection attempt is added.

The callbacks were exercised on the real `AppWindow` headlessly with Slint's
testing backend (temporary local harness, not committed: the internal
`i-slint-backend-testing` crate is not a project dependency). It confirmed
`invoke_open_add_device` resets `adding`/`add-step`/`model-query`/`model-kind`/
`add-loading`/`add-bluetooth-off`/`paired-options` and refreshes all 39 models,
`invoke_filter_models` narrows to A3954, P20 and the empty state, and
`invoke_choose_model` sets the name/code/art/speaker flag and `add-step = 1`
while ignoring an unknown code. The visible-window check was not run because
the desktop session was in active use; the screenshot stays with ticket 04.
