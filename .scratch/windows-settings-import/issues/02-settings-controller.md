# 02 — Remaining settings controller

**Type:** task
**Status:** done
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

- [x] desktop/core gates pass.
- [x] Real A3062: toggle/select/range writes reflected after re-poll.
- [x] No `.slint` edits.

## Implementation (2026-11-21)

The A3062's parsed settings block now drives the vendored section pages: the
sidebar lists Controls, Comfort, Power, Connections and About under **Device**
and adds Effects under **Sound**, each page's panels and rows are built from
`DeviceState::settings`, and the vendored `Actions` write one frame through the
device worker, re-poll `01 01` and refill the page from the device's answer.
The desktop diff is `desktop/src/controller.rs` plus the `arboard` dependency
(the About copy button) and the Windows `ShellExecuteW` features (the
Connections links); no `.slint` file changes.

### Pages and aside fields

A connected state with a settings block (`state.settings.is_some()`, i.e. the
A3062) gets:

| state | value |
|---|---|
| `sound-pages` | Sound modes, Equalizer, Effects |
| `device-pages` | Controls, Comfort, Power, Connections, About |
| `has-sides` | `false` (one control surface) |
| `has-ldac` | `true` |
| `has-spatial` | `false` |
| `case-battery` | `""` (no case page) |
| `system-settings-available` | `true` (Connections aside links) |

A state without the block (the A3949 and every other model so far) keeps
`device-pages` empty and gets no Effects page, no panels and no write path;
connecting or disconnecting clears all of it. `panels` is the current page's
panels: navigating re-renders it from the last state, so a page switch needs no
new poll.

### Panels and rows

| page | panels | rows (`SettingRow.id`, kind, value) |
|---|---|---|
| Controls | Presses, Cycle | `singlePress` info "Ambient sound mode"; `doublePress` select 0 None / 1 Bass Up; `noiseCancelingModeInCycle`, `transparencyModeInCycle`, `normalModeInCycle` toggles from body 68 bits |
| Comfort | Plain, Volume | `sideTone` toggle; `limitHighVolume` toggle, `limitHighVolumeDbLimit` range 75..100 step 5 (from `DbLimit`), `limitHighVolumeRefreshRate` select Real time / 10 seconds / 1 minute |
| Power | Plain | `autoPowerOff` select Disabled / 30 / 60 / 90 / 120 minutes (off keeps the duration), `lowBatteryPrompt` toggle, `voicePrompt` toggle |
| Connections | Plain | `dualConnections` toggle; no remembered-device list (follow-up slice) |
| Effects | Plain | `ldac` toggle, `dolbyAudio` toggle |
| About | Info | `isCharging` "Charging"/"Not charging", `batteryLevel` (the single-battery tile percentage), `firmwareVersion`; the serial is deliberately omitted, so neither the page nor a copy can leak it |

Ids are the camelCase `SettingId` strings the vendored `SettingText` labels and
icons key on, so the short labels ("Single press", "Noise canceling", …) and
descriptions come from the UI itself.

### Writes: one frame, then a re-poll

`DeviceCommand::SetSetting { request, setting }` carries a validated
`SettingCommand` to the connected worker, which calls `DeviceSession::set_setting`
(exactly one frame, no ack read), then `poll_state`, and posts
`Event::SettingChanged { attempt, request, setting, outcome }` with the whole
re-polled state. The UI thread drains it and replaces its copy of the state,
refilling `panels`/About from the device's answer; the worker never runs on the
UI thread. A failed write stops the worker (like a failed mode or equalizer
change) and the banner shows the setting's name and reason, e.g. "Side tone (no
answer)", which the vendored banner wraps as “Could not change …”; Retry
reconnects. A click superseded by a newer click is dropped, except a failure
like the other writes.

`Actions.set-toggle`, `set-option`, `set-number` are wired: unknown ids and
values outside a control's choices produce no command (so no frame); the Sound
mode tile keeps its own `ambientSoundMode` path, and `doublePress`,
`autoPowerOff` and `limitHighVolumeRefreshRate` selects dispatch too.
`Actions.copy` puts an Info row's text on the clipboard via `arboard`.
`Actions.open-system-settings` opens `ms-settings:bluetooth` or
`ms-settings:sound` with `ShellExecuteW`; `system_settings_uri` is the pure
mapping the tests assert. Toggle, select and range writes each send one frame
and then one `01 01` re-poll.

## Verification

- `desktop`: `cargo test -p soundcore-desktop` **151 passed / 0 failed / 9
  ignored** (15 new tests plus the ignored hardware check),
  `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean (only the pre-existing vendored-Slint layout warnings), `cargo build`
  clean.
- `core`: unchanged by this ticket; `cargo test` **119 passed / 0 failed** across
  20 targets, `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` clean, `cargo build` clean.
- New automated tests:
  - pages/panels: the A3062 gets the five device pages and Sound modes,
    Equalizer, Effects; the A3949 capture gets no page and no panel on any
    section page (`device_pages`, `sound_pages`, `panels_for_page`);
  - row mapping per page: Controls (info/select ids, "None"/"Bass Up", three
    cycle toggles and measured bits), Comfort (side tone; limit trio, range
    75..100 step 5 value 90, refresh options/index), Power (auto power off
    options and the measured 60-minute index 2, prompts), Connections (single
    toggle, no list), Effects (LDAC/Dolby), About (charging/battery/firmware
    04.39 from a patched capture; no serial row);
  - dispatch: every toggle names its exact `SettingCommand` including one-bit
    cycle changes and the limit toggle keeping the dB; auto power off indexes
    0..=4 (0 keeps the duration), refresh 0..=2, double press 0/1; dB range
    releases outside the measured steps are ignored; unknown ids (including
    `spatialAudio`) are ignored; the dispatched commands build the recorded
    frames (`01 84`, `01 FF`, `01 86`, `04 81`, `20 81`);
  - failure mapping: timeout/IO/protocol map to TimedOut/Disconnected/
    Unreadable and the banner label names the setting, with no MAC;
  - worker: one `SetSetting` writes exactly one settings frame then one
    `01 01`, and the event carries the re-polled state; a silent re-poll ends
    the worker, surfaces `Err(TimedOut)`, and the queued click writes nothing;
  - `system_settings_uri` maps Bluetooth/Sound to their `ms-settings:` pages.
- `manual_settings_pages_and_writes` (ignored) is the reusable hardware check.

## Real hardware (2026-11-21)

The real **soundcore Space One Pro** (A3062, firmware **04.39**) was reachable
over the Windows RFCOMM transport; `cargo test -p soundcore-desktop
manual_settings_pages_and_writes -- --ignored --nocapture` passed:

- `device-pages: [Controls, Comfort, Power, Connections, About]`,
  `sound-pages: [SoundModes, Equalizer, Effects]`, `has-sides=false`,
  `has-ldac=true`, `has-spatial=false`, `system-settings-available=true`;
- every page's panels and rows read as above (Controls single press/desc,
  double press None, cycle 110; Comfort side tone on, limit off/90 dB/real
  time; Power auto-off 60 minutes, both prompts on; Connections dual on;
  Effects both off; About "Not charging", "40%", "04.39", no serial row);
- one toggle: side tone `true -> false` re-polled, restored to `true`;
- one select: auto power off index 2 -> 3 (60 -> 90 minutes) re-polled,
  restored to index 2/60 minutes;
- one range: dB limit 90 -> 95 re-polled, restored to 90;
- the About copy action put its text on the clipboard;
- the final printed settings block is field-for-field the one found on
  connect, so the device was left as found. Addresses were masked and no
  serial was printed or committed.

