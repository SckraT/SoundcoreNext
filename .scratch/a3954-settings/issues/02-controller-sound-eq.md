# 02 — Controller: sound-mode scale and equalizer

**Status:** done
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

- [x] Sound mode and Equalizer pages are filled from the committed A3954 state
      through the same controller/worker path the app uses, and every change
      writes one measured frame and re-polls; fmt/build/test/clippy green.
- [ ] Real A3954 acceptance (ticket 04): connect, move the scale, edit the
      equalizer and restore every changed value. Live acceptance is deliberately
      not claimed here.

## Comments

Implemented on `ticket/a3954-controller` against the core from
`ticket/a3954-core` (feature tip `4711f9c`). No core behavior changed.

- **State and pages.** The A3954's ambient sound is one 1–11 scale held in
  `DeviceState.a3954.sound_mode`, so the controller treats a state with an
  `a3954` block as the A3954 surface everywhere the A3062 uses
  `sound_mode`/`equalizer`/`settings`: `has_sound_mode`, `has_equalizer`,
  `has_effects` and `has_worker_surface` helpers, `sound_pages(has_tiles,
  has_scale, has_equalizer, has_effects)`, `device_pages(has_settings,
  has_a3954)`. On connect an A3954 fills `mode-scale = true`, `scale-position`
  from `sound_mode.slider` (11 in the committed capture), `mode-rows =
  [windNoiseSuppression]` with the measured suppression flag, the scale's mode
  labels, `sound-pages = [SoundModes, Equalizer, Effects]` and opens
  `Page::SoundModes`. `modes` stays empty, so the A3062 three-tile path is not
  shown; the scale's airplane chip is hidden because the 1–11 scale has no
  airplane position and the airplane-mode setting write is deferred.
- **Scale and wind writes.** `Actions.set-scale(position)` and the
  `windNoiseSuppression` toggle both queue `DeviceCommand::ChangeA3954SoundMode`
  (an `A3954SoundModeChange::Scale(u8)` or `::Wind(bool)`); the worker derives
  the whole body from the state it last read — mode half from the position
  (1–5 transparency, 6 normal, 7–11 noise canceling), airplane mode and wind
  flag kept/updated — writes one `06 81` `[mode, position, airplane, wind,
  0xFF]` frame, re-polls `01 01` and answers `Event::ScaleChanged`. The page's
  `scale-position` and the wind row follow the device's answer, never the click;
  superseded clicks are dropped by request number and a failed write/read ends
  the worker with the existing readable "Sound mode (…)" banner and Retry.
  Out-of-range positions (0, 12) and every non-A3954 model write nothing.
- **Equalizer.** `EqualizerDisplay::new_a3954` uses the A3954's own 22-preset
  catalog (`a3954_preset_options`, `A3954_EQUALIZER_PRESETS`) and the eight
  visible bands of its two-channel block (left channel, falling back to the
  right); the page is editable and shows the custom id `0xFEFE` with no name
  for the committed capture. `eq-preset` queues `ChangeA3954Preset` (114-byte
  preset body), `eq-commit`/`eq-type`/`eq-reset` and saved-profile Apply queue
  `WriteA3954CustomCurve` (114-byte custom body, eight visible bands), each
  re-polls and returns the reported block. The worker keeps the last reported
  block so a following preset or custom write keeps the device's HearID fields,
  exactly as the A3062 path does. Preview still moves only the page; a stopped
  worker still shows the existing equalizer error banner.
- **A3062 untouched.** The A3062 state has no `a3954` block, so its `sound_mode`
  tiles, 53-byte equalizer bodies, `settings` block and demo worker run through
  the old branches unchanged; the demo test and all A3062 tests still pass.
- **UI/localization.** `components/modes.slint`'s `ModeRow` now renders
  `RowKind.toggle` rows (label, localized description, shared `Toggle`) and uses
  `LocalizedText.setting-label` as the row-name fallback; every new setting id
  and option value is mapped in `components/localized.slint` and translated in
  `desktop/translations/ru/LC_MESSAGES/soundcore-desktop.po`. The Overview
  renders the scale through its existing `mode-scale` branch; the tray hides
  the A3062 mode submenu for A3954 (its `modes` list is empty) and reads its
  LDAC flag from the A3954 block.
- **Tests (new, 15 A3954 controller tests in total; the scale/EQ ones here plus
  the settings ones in ticket 03).**
  `the_real_a3954_capture_exposes_the_scale_equalizer_and_settings_pages`,
  `the_a3954_connection_fills_the_scale_page_and_the_equalizer`,
  `the_a3954_scale_position_maps_to_the_measured_mode_halves`,
  `a_scale_click_writes_the_measured_frame_and_re_reads_the_scale` (the worker
  writes the app's captured position-3 frame and one `01 01` re-poll),
  `a_wind_toggle_writes_the_same_measured_body_shape_and_re_reads_it`,
  `the_a3954_equalizer_page_uses_its_own_catalog_and_visible_bands`,
  `an_a3954_preset_click_writes_the_114_byte_body_and_re_reads_it`,
  `an_a3954_custom_curve_writes_the_114_byte_body_and_re_reads_it`, plus the
  wired-action and reported-state tests listed in ticket 03. No hardware write
  happens in any test.

## Verification

- Desktop (from `desktop`): `cargo fmt --all -- --check`, `cargo build`,
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test` **286 passed
  / 0 failed / 13 ignored** (15 new A3954 tests; the ignored tests are the
  pre-existing manual hardware checks). A3062 and demo tests pass.
- Core (from `core`, unchanged by this ticket): `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build
  --workspace` clean; `cargo test --workspace` **185 passed / 0 failed** across
  24 targets.
- No live A3954 hardware run in this ticket; ticket 04 owns acceptance and the
  restore of every changed value. No MAC or serial is logged or shown.
- The feature tip `4711f9c` was already the branch base, so `git merge
  feature/a3954-settings` reported "Already up to date" and no merge commit was
  needed; the gates were re-run after that check.
