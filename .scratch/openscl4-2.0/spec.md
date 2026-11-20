# Spec: OpenSCL4 2.0 — desktop catalog

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/openscl4-2.0/`
**Integration branch:** `integration/openscl4-2.0`
**Spec source:** `D:\Pixel 8 Pro\Soundcore\wiki` (clean-room protocol, model taxonomy, diagnostics) and the approved 2.0 plan.

## Goal

Ship the first 2.0 desktop slice without inventing UI: keep the OpenSCL4 name and the vendored OpenSCL4 Slint shell, and populate its existing Add Device screen from the clean-room model catalog instead of an empty list.

The result is a runnable desktop shell where:

- the window is still OpenSCL4, versioned as 2.0;
- Add Device shows the 39 supported models from `soundcore-models::catalog`;
- the existing category chips (All / Earbuds / Over-ear / Open-ear / Speakers) filter the catalog;
- the search field filters by model name or code;
- choosing a model advances the existing dialog to its paired-device step;
- no custom screens, no protocol transport and no device connection yet.

## Out of scope

- Android UI and Android build.
- RFCOMM transport, state requests, settings and equalizer.
- Pairing, connection and diagnostics.
- New visual design or new screens; only wiring into the vendored OpenSCL4 UI.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-branding-version.md` | — | Docs/version say OpenSCL4 2.0, name stays |
| `02-catalog-adapter.md` | — | Adapter from core catalog to Slint model options with tests |
| `03-add-device-wiring.md` | 02 | Existing Add Device list is populated and filters/chooses work |
| `04-acceptance.md` | 01, 02, 03 | Build, screenshot, acceptance notes |

## Acceptance criteria

1. `cd desktop && cargo build` succeeds on Windows.
2. The app opens the vendored OpenSCL4 shell; the title still says OpenSCL4.
3. Add Device lists exactly the 39 supported models and no unsupported codes.
4. The category chips match the taxonomy: TWS → Earbuds, headphones → Over-ear, open-ear → Open-ear, speakers → Speakers; sleep buds are shown under Earbuds because the vendored UI has no Sleep chip.
5. Search `A3954` returns Liberty 4 Pro; search `P20` returns P20i/P25i/R50i; an unknown query shows the existing empty state.
6. Choosing a model shows its name/code/art in the existing confirmation step; no transport is attempted.
7. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass for the desktop crate and for `core`.
8. A screenshot of the populated Add Device dialog is stored in `desktop/` and referenced from `desktop/README.md`.

## Closure

A ticket is closed when its acceptance section is satisfied and its file says `Status: done`. The integration branch is merged to `master` only after ticket 04 and the `code-review` skill pass.
