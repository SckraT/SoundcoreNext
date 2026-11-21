# Spec: OpenSCL4 2.0 — Windows equalizer presets

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-eq-presets/`
**Integration branch:** `integration/windows-eq-presets`
**Depends on:** sound modes (`master` ba0d79f)
**Data source:** OpenSCL4 command/response and preset data (`lib/src/devices/soundcore/...`) is authorized for this project and adapted under GPL-3.0-or-later with attribution in `NOTICE`.

## Goal

Expose the built-in equalizer presets of Space One Pro (A3062) in the vendored Equalizer page: show the current preset and curve, let the user select another preset. Custom band editing, saved profiles, import/export and HearID stay out.

## In scope

- A3062 state parsing: preset id and 10 band values.
- Preset catalog (the 22 names/ids/bands of OpenSCL4's A3062 list).
- `03 87` command body for preset selection, written through the existing session; re-poll `01 01`.
- Controller: populate the vendored Equalizer page (`eq-available`, `eq-read-only=true`, `eq-presets`, `eq-preset`, `eq-bands`, `eq-preset-count`, labels), handle `Actions.eq-preset(value)`.
- A3949 equalizer: deliberately deferred in this slice (its state block is known but its write body is unverified), so no Equalizer page and no `03 87`.

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
2. A3062 shows the Equalizer page with the current preset selected and the 8 visible read-only bands (100 Hz…12.8 kHz, −12.0…+13.4 dB); the core still parses all 10 band bytes from state.
3. Selecting a different preset writes one `03 87` frame, re-polls `01 01`, and the selected chip/curve follow the device.
4. A3949 equalizer support is deliberately deferred in this slice: no Equalizer page and no `03 87` is sent. This is a recorded limitation, not evidence that the model has no equalizer: its committed `01 01` capture carries an equalizer block at body 32..44, but no A3949 write body is verified.
5. Custom band editing/profiles/import/export are hidden or read-only; no invented behavior.
6. A readable error is shown if a preset write/read fails.
7. Screenshot stored in `desktop/` and linked from `desktop/README.md`; no MAC/serial.

## Recorded limitations

- **8 visible bands, not 10.** The state carries ten band bytes and the core parses all of them, but the last two are fixed by every preset (0 dB and −12.0 dB) and are not adjustable. OpenSCL4's A3062 equalizer exposes 8 visible bands with the frequencies 100 Hz…12.8 kHz, and the Soundcore app labels the same eight, so the vendored page shows eight. Evidence in tickets 02 and 03.
- **A3949 equalizer deferred.** The committed A3949 `01 01` capture has an equalizer block at body 32..44 (preset id 0 and ten flat band bytes), so the model does have equalizer data; the A3949 `03 87` write body is unverified, so this slice sends nothing for A3949 and hides the page.
- **A3949 live re-check not reproducible.** In the acceptance run the paired P20i was not pageable over RFCOMM, so the no-page/no-frame policy for A3949 is covered by the committed state capture and the core/controller/session tests, not by a live session (ticket 03).

## Closure

Ticket closed when acceptance passes and file says `Status: done`; merge to `master` after ticket 03 and `code-review`.
