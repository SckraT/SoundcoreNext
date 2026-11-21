# Spec: OpenSCL4 2.0 — Windows custom equalizer bands

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-eq-custom/`
**Integration branch:** `integration/windows-eq-custom`
**Depends on:** equalizer presets (`master` a1739f7/3c77438)
**Data:** OpenSCL4 A3062 equalizer module permitted.

## Goal

Make the vendored Equalizer page editable for Space One Pro (A3062): drag/type the 8 visible bands, write a custom equalizer (`0xFEFE`), read it back, reset to flat. Saved profiles/import-export stay out.

## In scope

- Core: build the same 53-byte `03 87` body with `preset = 0xFEFE` and chosen 10 band bytes (8 visible + 2 fixed); session write.
- Controller: `eq-read-only=false`; wire `eq-preview`, `eq-commit`, `eq-type`, `eq-reset`; regenerate chart/curve from bands; update preset name to “Your own settings”/custom; re-poll after commit.
- Real hardware: custom curve persists after re-poll; reset works; presets still work.
- P20i/A3949 remains deferred (no page).

## Out of scope

- Saved profiles, import/export, HearID, DRC.
- Editable band count beyond the measured 8; other models.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-custom-eq-core.md` | — | Custom preset body/session + tests |
| `02-custom-eq-controller.md` | 01 | Editable page, preview/commit/reset, chart |
| `03-acceptance.md` | 02 | Real A3062 custom curve/reset, screenshot |

## Acceptance criteria

1. Gates pass (desktop/core test, fmt, clippy, build).
2. A3062 Equalizer sliders/fields are editable; dragging updates the chart immediately, committing writes one `03 87` with `0xFEFE` and the chosen bands, then re-polls `01 01`.
3. The re-polled state shows the custom bands and preset “Your own settings”; values match the device.
4. Reset writes a flat custom curve and the UI follows.
5. Built-in preset selection still works afterwards.
6. No saved profiles/import/export; no invented controls.
7. Screenshot of the custom curve stored/linked; no MAC/serial.
8. A failed write/read shows a readable error.

## Closure

Ticket closed when acceptance passes and `Status: done`; merge to `master` after ticket 03 and `code-review`.
