# 03 — Controller: A3954 settings pages

**Status:** done
**Blocked by:** 01, 02

## Goal

Populate and wire the remaining A3954 pages.

## Requirements

1. `device_pages`: Controls (ambient cycle), Comfort, Power, Connections, About
   for A3954; hide what the device does not report.
2. Effects: LDAC toggle, spatial-audio enable + music mode + mode rows, sound
   leak compensation; `has-spatial` flag.
3. Comfort: wearing detection, easy chat + wait time, sound leak, limit high
   volume enable/dB/refresh.
4. Power: auto power off (10/20/30/60), low battery prompt.
5. Connections: dual-connections toggle and remembered-device list.
6. About: left/right/case battery plus case firmware; no serial/address.
7. `set-toggle`, `set-option`, `set-number`, list actions and page entry read
   the right A3954 command and re-poll. Unknown ids never write.
8. Tests: each page's rows, values and command vector; no A3062 regression.
9. Demo worker: A3954 demo can be added later; do not break A3062 demo.

## Acceptance

- [x] All five A3954 pages populate from the committed state and every control
      names one measured A3954 frame and re-polls; fmt/build/test/clippy green.
- [ ] Real A3954 acceptance (ticket 04): every page and control on hardware with
      restore. Live acceptance is deliberately not claimed here.

## Comments

Implemented on `ticket/a3954-controller`; it builds on ticket 02's state and
worker changes. No core change.

- **Pages.** `panels_for_page` dispatches to `a3954_panels_for_page` when the
  state has an `a3954` block, leaving the A3062 panels code untouched.
  `device_pages(has_settings, has_a3954)` shows Controls, Comfort, Power,
  Connections and About for either settings surface and none for a model with
  neither (P20i/A3949). `show_device_pages` sets `has-ldac = true`,
  `has-spatial = true`, `has-sides = false`, `case-battery` from the measured
  level (`100%` in the capture) and enables the Connections system links.
- **Effects.** LDAC toggle, spatial-audio enable toggle, spatial music-mode
  select (Fixed / Head tracking), spatial mode select (Music / Podcast / Movie /
  Gaming) and a sound-leak-compensation toggle. While spatial audio is off the
  two spatial choices are disabled with the reason "Turn on spatial audio
  first"; their row still shows the device's value. All write `10 81` (one body
  for enable/music/mode), `01 FF`, and `10 9A`.
- **Comfort.** Wearing detection (`01 81`), easy chat (`10 9D` with the reported
  wait time), easy-chat wait-time select (5/10/15 seconds, keeping the enable
  flag), sound-leak compensation (`10 9A`) and the Volume panel: limit high
  volume (`20 82` enable + dB, `DbLimit` 75..100 step 5) and refresh rate
  (`20 81`, real time / 10 seconds / 1 minute).
- **Power.** Auto power off select (Disabled + the A3954's own 10/20/30/60
  minutes, `01 86`; choosing Disabled keeps the reported duration) and the
  low-battery prompt toggle (`10 82`).
- **Connections.** Dual-connections toggle (`0B 84`) plus the existing
  remembered-device list: the worker reads `0B 01` on connect while the setting
  is on, the page refreshes it on entry, per-row Connect/Disconnect/Forget write
  the existing `0B 82`/`0B 81`/`0B 83` frames and re-read the list, and turning
  the toggle on writes `0B 84`, re-polls and then reads. `dual_list_visible`
  reads the flag from either model's block.
- **Controls.** The ambient-sound-mode cycle toggles (`06 82`), reusing
  `cycle_rows`, plus an Info row stating button remapping is not available in
  this version; the eight main and four slide button bytes stay raw and no
  remapping write exists.
- **About.** Left battery, right battery, case battery (raw level + 1 of 10),
  left firmware, right firmware and case firmware, all from the measured state.
  No serial, no Bluetooth address, no case-language row (the case display
  writes are deferred).
- **Writes and errors.** `set-toggle`/`set-option`/`set-number` branch to
  `a3954_toggle_command`/`a3954_option_command`/`a3954_number_command` and queue
  `DeviceCommand::SetA3954Setting`; the worker writes exactly one
  `a3954_setting_frame` frame, re-polls `01 01` and answers
  `Event::A3954SettingChanged` with the whole state, which refills the current
  page, the Overview and the tray and clears the remembered list when dual
  connections turns off. Unknown ids, A3062-only ids (`sideTone`,
  `dolbyAudio`, `voicePrompt`, `doublePress`), out-of-range indexes/values and
  the wind row (which belongs to the `06 81` body) never write a settings frame.
  A failed write/read ends the worker with the existing "Setting (…)" banner and
  Retry.
- **Overview and tray.** `overview.rs` treats the A3954 block as the sound-mode,
  equalizer and settings surface: the sound-mode topic renders the 1–11 scale
  from `AppState`, the Comfort shortcut counts its three switches, Power shows
  the auto-power-off value, Connections the dual flag, Sound and effects LDAC;
  its quick-toggle set (LDAC, spatial audio, wearing detection, easy chat,
  sound-leak compensation, limit high volume, low-battery prompt, dual
  connections, the three cycle bits) is its own id list and its chips write
  through the same `set-toggle` path. The title bar counts the right pages, and
  the tray keeps the mode submenu hidden while the A3954 LDAC item reads the
  A3954 flag.
- **A3062 unchanged.** The A3062 settings block keeps the original
  `panels_for_page` branch, its `SettingCommand` path and its demo worker; the
  demo connection tests still pass. An A3954 demo capture can be added later
  (ticket wording allows it); demo mode still connects only the A3062.
- **Deferred (documented).** Button remapping, the `07 87` case-features write,
  the case display/brightness/color/language settings, air-pressure writes,
  HearID editing and changing the airplane-mode update setting on its own.
- **Tests (new, 15 A3954 controller tests in total; the settings ones here plus the scale/EQ ones in ticket 02).**
  `the_real_a3954_capture_exposes_the_scale_equalizer_and_settings_pages`,
  `the_a3954_settings_pages_have_their_measured_rows` (every page's ids, kinds,
  labels, values and options from the committed capture),
  `a3954_row_ids_name_only_measured_setting_commands` (the toggle/option/
  number mapping and its refusals),
  `each_a3954_settings_page_writes_one_measured_frame_and_re_polls` (13 cases
  across the five pages: one frame each, then exactly one `01 01` re-poll, with
  the reported A3954 state in the event),
  `an_a3954_connect_reads_the_dual_list_while_the_setting_is_on`,
  `the_a3954_pages_queue_their_measured_commands_from_the_wired_actions`
  (wired `Actions` for scale, wind, settings, dual/unknown ids, preset, commit,
  saved profile and a missing profile),
  `a_reported_a3954_scale_and_setting_refresh_the_pages`,
  `the_a3954_tray_hides_the_a3062_mode_list_and_shows_its_ldac`. No hardware
  write happens in any test.

## Verification

- Desktop (from `desktop`): `cargo fmt --all -- --check`, `cargo build`,
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test` **286 passed
  / 0 failed / 13 ignored** (15 new A3954 tests). The A3062 settings, demo,
  equalizer, profile and tray tests all still pass.
- Core (from `core`, unchanged by this ticket): `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build
  --workspace` clean; `cargo test --workspace` **185 passed / 0 failed** across
  24 targets.
- No live A3954 hardware run; ticket 04 owns page-by-page acceptance with
  restore. No serial or address is shown, copied or logged (the About rows
  deliberately omit both, and the case serial stays internal in the core).
- The feature tip `4711f9c` was already the branch base, so `git merge
  feature/a3954-settings` reported "Already up to date" and no merge commit was
  needed; the gates were re-run after that check.
