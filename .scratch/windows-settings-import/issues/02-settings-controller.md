# 02 — Remaining settings controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Populate and wire the vendored settings pages for A3062.

## Files

- `desktop/src/controller.rs`
- no `.slint` edits

## Requirements

1. Connect A3062 → set `device-pages`/`sound-pages` and `panels`:
   - Controls: Presses [singlePress info, doublePress select], Cycle [normal/transparency/noise-canceling toggles]; `has-sides=false`.
   - Comfort: side tone toggle; limit high volume toggle + dB range 75..100 step5 + refresh select.
   - Power: auto power off select (disabled/30/60/90/120m), low battery prompt, voice prompt.
   - Connections: dual connections toggle; `system-settings-available=true` and working `open-system-settings` (`ms-settings:bluetooth`).
   - Effects: ldac, dolbyAudio toggles; `has-ldac=true`, `has-spatial=false`.
   - About: charging, battery, firmware; serial masked/omitted; copy action for info rows.
2. Wire `Actions.set-toggle`, `set-option`, `set-number`, `copy`, `open-system-settings`; unknown ids ignored.
3. Writes go through the worker (`DeviceCommand::SetSetting`), exactly one frame, then `poll_state`; update panels/About from re-poll; readable error banner on failure; no UI-thread blocking.
4. A3949 deferred: no settings pages/writes.
5. Tests: page/panel mapping, row ids/values, command dispatch, one write+re-poll, unknown id, error mapping, A3949 no page.
6. No dual-connections device list/connect/forget; no legacy migration; no `.slint` edits.

## Acceptance

- [ ] desktop/core gates pass.
- [ ] Real A3062: toggle/select/range writes reflected after re-poll.
- [ ] No `.slint` edits.
