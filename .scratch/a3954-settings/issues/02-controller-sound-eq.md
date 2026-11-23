# 02 — Controller: sound-mode scale and equalizer

**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Wire the A3954 sound-mode scale and equalizer pages end to end.

## Requirements

1. On connect, show `Page::SoundModes` for A3954; `AppState.mode-scale=true`,
   `scale-position` from state, Russian labels, wind toggle row.
2. Wire `Actions.set-scale(position)` to a worker command that writes
   `06 81 [mode,position,0,wind,FF]`, re-polls and updates the page.
3. Equalizer display: 8 visible bands, A3954 preset chips (22), custom curve,
   editable; write preset/custom using the 114-byte body and re-poll.
4. Bind `eq-*` actions on A3954 to the new worker commands; saved profiles use
   the same visible bands.
5. Tests: scale mode mapping, wind, preset catalog size/id, custom body,
   controller state and commands; no A3062 regressions.

## Acceptance

- Both pages work in demo and on real A3954; fmt/build/test/clippy green.
