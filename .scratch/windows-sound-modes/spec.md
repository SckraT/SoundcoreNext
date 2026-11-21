# Spec: OpenSCL4 2.0 — Windows sound modes

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-sound-modes/`
**Integration branch:** `integration/windows-sound-modes`
**Depends on:** Windows connection MVP (`master` ff7d640)
**Exploration notes:** `D:\Projects\SoundcoreNext-notes\windows-mvp.md`; protocol facts in `D:\Pixel 8 Pro\Soundcore\wiki` and the research captures.

## Goal

The Windows shell connects to Space One Pro (A3062) and can read and change the sound mode (noise canceling / transparency / normal) from the existing vendored Sound mode page.

This is the second Windows functional slice: one real setting, end to end, with no new UI.

## In scope

- Core sound-mode model: enum, A3062 state offset, A3062 command body for `06 81`.
- Session write path: send the mode command, wait for ACK/state, re-read state.
- Controller: populate `AppState.modes`, `mode-index`, mode labels, expose `Page.sound-modes`, handle `Actions.set-option("ambientSoundMode", index)`.
- Read current mode on connect; update after writes; readable errors.
- Real hardware acceptance on Space One Pro; P20i has no ANC modes and must show no Sound mode page.

## Out of scope

- ANC strength, wind noise, adaptive ANC, transport/airplane mode.
- Equalizer, presets, controls, comfort, connections, case.
- A3954/A3949 mode support (A3949 is a normal-mode-only model; A3954 is unreachable and its state offset is unmeasured).
- Writes other than `06 81`.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-sound-mode-core.md` | — | Enum, A3062 state offset, command builder, session write, tests |
| `02-controller-modes.md` | 01 | Sound mode page populated and writable in the vendored UI |
| `03-acceptance.md` | 02 | Real A3062 toggle verified, screenshot, docs |

## Acceptance criteria

1. All gates pass: desktop/core test, fmt, clippy, build.
2. Space One Pro connects and the sidebar shows **Sound mode**; the page shows the current mode and can switch between noise canceling, transparency and normal.
3. Each switch writes exactly one `06 81` frame and the UI reflects the new state from a subsequent `01 01`.
4. P20i connects but has no Sound mode page and no mode commands are sent.
5. A failed write/read shows a readable error, never a raw MAC/serial.
6. A screenshot of the Sound mode page is stored in `desktop/` and linked from `desktop/README.md`.
7. No case/strength/EQ behavior is invented; unsupported controls stay hidden.

## Closure

Ticket closed when acceptance passes and file says `Status: done`. Merge to `master` after ticket 03 and `code-review`.
