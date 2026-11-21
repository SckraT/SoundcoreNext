# Spec: OpenSCL4 2.0 — Windows remaining settings + profile import

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-settings-import/`
**Integration branch:** `integration/windows-settings-import`
**Depends on:** saved equalizer profiles (`master` bdc955e)
**Data:** OpenSCL4 A3062 modules permitted; exploration notes at `D:\Projects\SoundcoreNext-notes\remaining-settings.md`.

## Goal

Complete the A3062 Windows settings surface in the vendored UI: Controls, Comfort, Power and prompts, Sound effects, About, plus a dual-connections toggle. Add equalizer profile import (paste and file) in the existing Import dialog. No legacy migration.

## In scope

- Core parse of A3062 body 67..87: button double press, ambient cycle, low-battery prompt, Dolby, LDAC, dual-connections toggle, auto power off, limit high volume (enable/dB/refresh), side tone, voice prompt, charging/battery/firmware/serial for About.
- Core command builders for all these settings plus session write→re-poll.
- Controller panels and callbacks: `set-toggle`, `set-option`, `set-number`, `copy`, `open-system-settings`; pages Controls/Comfort/Power/Connections/About/Effects; `has-ldac`, `has-spatial=false`, `system-settings-available=true`.
- Profile import: parse OpenSCL4 transfer-profile JSON (`name` + 8 `volumeAdjustments` dB values), preview statuses, all-or-nothing merge into saved profiles, persist.
- Import UI: paste from clipboard, file picker; `importing`/`import-items`/`import-blocked`/`transfer-note`; close on success; error notes.
- A3062 only; A3949 deferred.

## Out of scope

- Legacy equalizer migration (`eq-migrate`, `eq-legacy` stays empty).
- Profile export (existing panel remains inert).
- Dual-connections remembered-device list/connect/disconnect/forget (`0B 01` multi-frame and MAC list) — follow-up spec.
- Wearing detection, auto play/pause, gestures beyond double press, case/lights/alarms/sleep, spatial audio; not present on A3062.
- Android/Linux.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-settings-core.md` | — | Parse body 67..87 + command builders + session set + tests |
| `02-settings-controller.md` | 01 | Panels/writes/links for Controls, Comfort, Power, Connections toggle, Effects, About |
| `03-import-core.md` | — | Transfer-profile parse/validate/statuses + merge + tests |
| `04-import-controller.md` | 03 | Import dialog paste/file, preview, merge/persist, notes |
| `05-acceptance.md` | 02, 04 | Real A3062 settings writes + import, screenshot, docs |

## Acceptance criteria

1. Gates pass (desktop/core test, fmt, clippy, build).
2. A3062 shows Controls, Comfort, Power, Connections, About and Sound effects pages, populated from `01 01`.
3. Each scalar control writes exactly one frame and re-polls; UI reflects the device answer; failed writes show a readable banner.
4. Controls: double-press select and cycle toggles work. Comfort: side tone, limit high volume enable/dB/refresh. Power: auto-off select, low battery prompt, voice prompt. Effects: LDAC and Dolby. Connections: dual-connections toggle + working “open Bluetooth settings” link. About: charging/battery/firmware; serial hidden or masked.
5. Profile import: paste/file parses OpenSCL4 format; preview shows new/replace/wrong-band/out-of-range; blocked imports do not write; valid import adds/replaces and persists; no legacy banner/migration.
6. A3949 has no equalizer/settings pages beyond the existing deferred behavior.
7. Screenshots stored/linked; no MAC/serial in docs/images.
8. Device settings left as found after acceptance.

## Closure

Ticket closed when acceptance passes and `Status: done`; merge to `master` after ticket 05 and `code-review`.
