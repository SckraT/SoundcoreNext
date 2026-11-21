# 02 — Custom equalizer controller

**Type:** task
**Status:** done
**Blocked by:** 01

## Goal

Enable editing in the vendored Equalizer page and wire commits.

## Files

- `desktop/src/controller.rs`
- no `.slint` edits

## Requirements

1. For A3062: `eq-read-only = false`; keep 8 visible bands, min/max, preset chips.
2. Wire `Actions.eq-preview(index, value)`: update local `eq-bands`/chart immediately (no write).
3. Wire `Actions.eq-commit(index, value)`: clamp, write custom body via session, re-poll, update state; readable error on failure.
4. Wire `Actions.eq-type(index, text)`: parse integer then same commit path.
5. Wire `Actions.eq-reset()`: set all visible bands to 0 dB, write custom flat, re-poll; update.
6. Update `eq-preset`/`eq-name` to custom (`0xFEFE` → empty name/“Your own settings”), regenerate `eq-chart-line/area` from bands.
7. Built-in preset selection must still work and switch back from custom.
8. P20i/A3949: no page/no writes.
9. Tests for band conversion/clamping, preview no-write, commit one write, reset flat, custom state mapping, error mapping.
10. No profiles/import-export/HearID.

## Implementation (2026-11-21)

Only `desktop/src/controller.rs` changed; the vendored `.slint` tree is
untouched (its editable-band controls and `Actions` already exist).

- **Editable page.** `EqualizerDisplay` has an `editable` flag, worked out by
  probing `DeviceModel::custom_equalizer_body`: true only for the A3062, the
  model whose custom `03 87` body this slice can write. `show_equalizer` sets
  `eq-read-only = !editable`, so the A3062 page enables the eight band fields,
  sliders, Reset button and range note while it keeps the 22 preset chips, the
  eight visible bands and the measured −12.0…+13.4 dB range. A model whose
  equalizer is deferred (the A3949) still has no page, no chips and now also no
  editable flag.
- **Band edit model.** The four wired actions are answered by one small path:
  - `BandAction::Preview` (`eq-preview`) shows the dragged value on
    `eq-bands` and regenerates `eq-chart-line`/`eq-chart-area`; it has no write
    path at all (the function it calls does not take the worker's command
    channel and returns "no write queued").
  - `BandAction::Commit` (`eq-commit`) clamps the value to the measured range,
    shows it, and writes the whole eight-band curve on the device worker.
  - `eq-type` parses the field text as decibels into device units — point or
    comma, an optional sign (`-`, `+` or the vendored U+2212 minus) and tenths,
    rejecting non-numbers and finer precision — then takes the same commit path
    (an out-of-range number is clamped there, like a dragged value).
  - `BandAction::Reset` (`eq-reset`) shows and writes the flat 0 dB curve.
  - The page fields are behind a tiny `EqPage` trait (real implementation: the
    Slint `AppState`; tests record the calls) and the device write behind
    `BandWriter` (real implementation: the worker's command channel), so the
    preview-vs-commit behaviour is tested without a window.
- **One write, then a re-poll.** `DeviceCommand::WriteCustomCurve { request,
  bands }` makes the worker call `change_custom_equalizer`, which writes exactly
  one `03 87` custom body (built from the last re-polled state, so the HearID
  block is preserved, exactly like the preset path) and then polls `01 01`. The
  re-polled equalizer replaces the page state: `eq-preset` becomes the custom id
  string `"65278"`, `eq-name` becomes empty (the vendored Tag then shows "Your
  own settings"), and the bands/chart/curve follow the device. A result of a
  superseded request is dropped by the existing request-counter rule, now shared
  by preset and curve changes (`equalizer_change_applies`).
- **Readable failures.** A failed curve write/read maps to `EqFailure` and the
  banner label names the setting: "Equalizer bands (no answer)", "… (connection
  lost)", "… (unreadable answer)". The worker stops after a failure (Retry
  reconnects, as for modes/presets), and a band action on a stopped worker shows
  the connection-lost label instead of silently doing nothing.
- **Presets still work, both ways.** A chip still sends
  `DeviceCommand::ChangeEqualizerPreset`; the worker keeps the *re-polled* state
  as the base of each `03 87` body, so after a custom curve a preset write is
  built from the custom state's HearID block (preserved by the custom write) and
  switches the device back to the measured preset curve, whose re-poll then
  updates the chips, name and bands again.
- **P20i/A3949 unchanged and deferred.** It reports no equalizer state, so the
  page is hidden and `EqualizerDisplay` is neither available nor editable; the
  band actions are gated on connected + available + editable, and the core still
  refuses a custom write for a model without the capability.
- Commands were renamed (`ChangeSoundMode`, `ChangeEqualizerPreset`,
  `WriteCustomCurve`) so the three variants no longer share one prefix, which
  `clippy -D warnings` (enum variant names) requires.
- Nothing about saved profiles, import/export or HearID was added: the page's
  profile/legacy fields stay empty and HearID is only copied verbatim inside the
  measured `03 87` body.

## Verification

- `desktop` `cargo test`: **117 passed, 0 failed, 7 ignored** (was 102/6 before
  this ticket; +15 automated tests and one new ignored hardware test).
- New automated tests, all passing:
  - `a_band_text_parses_decibels_into_device_units` — "3.5" → 35, "-3.5" → −35,
    U+2212 minus and comma forms, "+13.4" → 134, trailing zeros; rejects empty,
    non-numeric, finer precision, overflow.
  - `a_band_value_clamps_to_the_measured_range` — i32 extremes, ±1 past the
    range, and the clamped field text.
  - `a_preview_moves_the_page_without_a_curve_write` — the page follows the
    drag and the recorded writer receives nothing.
  - `a_commit_clamps_the_band_and_writes_the_whole_curve_once` and
    `a_commit_keeps_the_other_bands_the_page_shows` — one write of all eight
    shown bands with the clamped value.
  - `a_reset_shows_and_writes_the_flat_curve`.
  - `a_band_index_the_page_does_not_show_is_ignored`.
  - `a_stopped_worker_shows_the_readable_curve_error`.
  - `a_failed_curve_change_maps_to_a_readable_label` — timeout / dropped link /
    malformed answer / unsupported model each get their label.
  - `a_finished_curve_change_shows_the_reported_state_or_the_error` — success
    shows the custom state and clears the banner; failure stops the worker and
    shows the band label.
  - `a_custom_state_shows_the_custom_preset_id_and_no_built_in_name` — custom
    state maps to preset id `"65278"`, empty name, the re-polled curve, and the
    22 chips stay available.
  - `a_custom_curve_write_writes_the_measured_frame_and_re_reads_it` — the
    captured real write frame equals the body builder's, exactly one `03 87`
    frame then `01 01`, and the re-poll is `0xFEFE` with the written bands.
  - `a_silent_device_after_the_custom_curve_write_is_a_timeout` — one 63-byte
    custom frame and the re-poll are written; the failure is the band timeout
    label.
  - `a_reset_writes_a_flat_custom_curve_and_re_reads_it` — the flat body
    (`FE FE`, `78`×8 + `78 00` in both band regions) once, then the re-poll.
  - `the_worker_writes_one_custom_frame_and_reports_the_custom_state` — worker
    path, one exact custom frame, `Event::CurveChanged` with the custom state.
  - `the_worker_switches_back_to_a_built_in_preset_after_a_custom_curve` — after
    a custom curve, a preset write is built from the custom state's HearID block
    and is the measured Bass Reducer frame; both changes re-poll.
  - The A3062/A3949 page tests now assert `editable` (true / false) and the
    preset-chip and error-mapping tests are unchanged.
- `desktop` `cargo fmt --all -- --check` clean; `cargo clippy --all-targets --
  -D warnings` clean; `cargo build` ok (only the pre-existing vendored-Slint
  layout warnings).
- `core` (unchanged by this ticket, re-run): `cargo test` **77 passed, 0
  failed** across 15 targets; fmt and clippy clean.

## Real hardware (2026-11-21)

Verified on the real **soundcore Space One Pro** (catalog **A3062**; the same
device/firmware **04.39** as ticket 01, which this run did not re-print) over the
Windows RFCOMM transport. The run used the new ignored test
(`cargo test -p soundcore-desktop manual_equalizer_custom_curve -- --ignored
--nocapture`); addresses are masked by the test.

- Connected; page: `eq-available: true eq-read-only: false bands: 8 presets: 22`.
- Found on connect: preset **0**, **Soundcore Signature**, bands
  `[0, 0, 0, 0, 0, 0, 0, 0]`.
- **Typed commit:** `eq-type` band 0 `4.0` → re-polled custom curve
  `[40, 0, 0, 0, 0, 0, 0, 0]` with preset `"65278"` and empty `eq-name`.
- **Preview + commit:** `eq-preview` band 3 `-4.0` moved the page at once to
  `[40, 0, 0, -40, 0, 0, 0, 0]`; `eq-commit` re-polled exactly
  `[40, 0, 0, -40, 0, 0, 0, 0]`, preset `"65278"`.
- **Reset:** re-polled the flat custom curve `[0, 0, 0, 0, 0, 0, 0, 0]` under
  preset `"65278"`.
- **Preset switch from custom:** clicking **Acoustic** (id 1) re-polled
  `[40, 10, 20, 20, 40, 40, 40, 20]`, name **Acoustic**.
- **Restored:** clicking **Soundcore Signature** (id 0) re-polled its flat
  bands; the device was left on the state found on connect.
- **Failure path:** with the worker stopped, a commit showed the readable banner
  `"Equalizer bands (connection lost)"` and wrote nothing; the banner's Retry
  reconnected and re-read the restored state.
- The earlier `manual_equalizer_preset_switch` was re-run after the
  `eq-read-only = false` change and still passes (three preset switches, the
  found preset restored, a readable preset error and a successful Retry).
- A preview cannot be observed as "no write" through the RFCOMM transport (the
  transport reports bytes, not protocol operations), so the no-write evidence is
  the automated recorded-writer test above; the commit/re-poll/reset/preset
  evidence above is from the real device.

## Acceptance

- [x] build/test/fmt/clippy pass.
- [x] Real A3062: editable bands, preview, commit, re-poll, reset, preset switch.
- [x] No `.slint` edits.
