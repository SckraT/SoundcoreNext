# Spec: OpenSCL4 2.0 — Windows equalizer presets

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-eq-presets/`
**Integration branch:** `integration/windows-eq-presets`
**Depends on:** sound modes (`master` ba0d79f)
**Data source:** OpenSCL4 A3062 equalizer module (`lib/src/devices/soundcore/a3062/...`) is permitted by the user for command/state data.

## Goal

Expose the built-in equalizer presets of Space One Pro (A3062) in the vendored Equalizer page: show the current preset and curve, let the user select another preset. Custom band editing, saved profiles, import/export and HearID stay out.

## In scope

- A3062 state parsing: preset id and 10 band values.
- Preset catalog (names/ids/bands) matching OpenSCL4's A3062 presets.
- `03 87` command body for preset selection, written through the existing session; re-poll `01 01`.
- Controller: populate the vendored Equalizer page (`eq-available`, `eq-read-only=true`, `eq-presets`, `eq-preset`, `eq-bands`, `eq-preset-count`, labels), handle `Actions.eq-preset(value)`.
- P20i has no Equalizer page and no `03 87`.

## Out of scope

- Custom band writes (`eq-commit`, `eq-preview`, `eq-type`, reset), profiles/save/delete/import/export.
- HearID, DRC, EQ for other models, Android/Linux.
- New UI; only the existing vendored page.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-eq-core.md` | — | EQ state parse, preset catalog, command body, session write, tests |
| `02-eq-controller.md` | 01 | Equalizer page populated, preset changes wired |
| `03-acceptance.md` | 02 | Real A3062 preset switching, screenshot, docs |

## Acceptance criteria

1. Gates pass (desktop/core test, fmt, clippy, build).
2. A3062 shows the Equalizer page with the current preset selected and the 10-band read-only curve/values from state.
3. Selecting a different preset writes one `03 87` frame, re-polls `01 01`, and the selected chip/curve follow the device.
4. P20i has no Equalizer page and no `03 87` is sent.
5. Custom band editing/profiles/import/export are hidden or read-only; no invented behavior.
6. A readable error is shown if a preset write/read fails.
7. Screenshot stored in `desktop/` and linked from `desktop/README.md`; no MAC/serial.

## Closure

Ticket closed when acceptance passes and file says `Status: done`; merge to `master` after ticket 03 and `code-review`.
