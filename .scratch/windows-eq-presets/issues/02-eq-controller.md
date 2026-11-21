# 02 — Equalizer presets controller

**Type:** task
**Status:** done
**Blocked by:** 01 (done)

## Goal

Populate the vendored Equalizer page with the A3062 preset catalog and wire preset selection.

## Files

- `desktop/src/controller.rs`
- no `.slint` edits

## Requirements

1. On connect to A3062, set:
   - `eq-available = true`, `eq-read-only = true`;
   - `eq-preset-count`, `eq-presets` from the core catalog (values = preset ids as strings, labels = names);
   - `eq-preset` = current preset id string;
   - `eq-band-count = 8`, `eq-min`, `eq-max`, `eq-min-text`, `eq-max-text`;
   - `eq-bands` from current state (frequency labels 100 Hz…12.8 kHz, `khz`, value, text) — read-only;
   - the chart's guides (`eq-lines`) and curve (`eq-chart-line`, `eq-chart-area`);
   - `sound-pages` includes `Page.equalizer` in addition to sound-modes;
   - no `eq-profiles`/`eq-legacy`/import/export content.
2. Handle `Actions.eq-preset(value)`:
   - resolve id; write the preset via session; re-poll; update `eq-preset`, `eq-bands`, `eq-name`, the chart;
   - readable `error-label` on failure; ignore unknown ids.
3. A3949 equalizer is deferred in this slice: no Equalizer page and no `03 87` path.
4. Custom band callbacks (`eq-commit`/`eq-preview`/`eq-type`/`eq-reset`) are inert; `eq-read-only=true` keeps the UI hidden.
5. Tests: preset id/name mapping, band labels/values, page exposure A3062 vs A3949, error mapping, no custom writes.
6. No strength/profiles/import-export.

## Visible band count: 8 visible, 10 parsed

`eq-band-count` is **8**, not 10, and the frequency labels are **100 Hz…12.8 kHz**,
not 31 Hz…16 kHz. This is measured, not a shortcut:

- OpenSCL4's A3062 equalizer module (the permitted data source) is
  `EqualizerModuleSettings<VISIBLE_BANDS = 8, PRESET_BANDS = 10, -120, 134, 1>`
  with `band_hz = [100, 200, 400, 800, 1600, 3200, 6400, 12800]`. The state's
  9th and 10th band bytes exist but are invisible: every preset fixes them at
  0 dB and -12.0 dB.
- The official Soundcore app (decompiled 5.0.32) labels the same eight bands
  (`com.soundcore.eq.conf.EQConstants.defaultXValue`) and its preset arrays
  contain the same curves (the first eight state bands).
- The vendored UI's own upstream screenshot
  (`D:/Projects/OpenSCL4/docs/images/openscl4-equalizer-en.png`) shows
  "Equalizer bands: 8" with 100 Hz, 200 Hz, 400 Hz, 800 Hz, 1.6 kHz, 3.2 kHz,
  6.4 kHz, 12.8 kHz and the range "-12.0 to +13.4 dB".

All ten raw state bands still parse in the core (ticket 01); the page shows the
eight visible ones. The original spec AC2 said "10-band" and has been corrected
to "8 visible bands" with this evidence. See the Verification section for the
real-device confirmation.

## Implementation notes

- The page exposes all 22 presets at once (`eq-preset-count == eq-presets.length`),
  so the vendored "All profiles (n)" link never appears and `eq-expanded` stays
  false.
- The chip `value` is the decimal preset id the state reports (`"0"`, `"3"`,
  `"32382"` for 0x7E7E), so `eq-preset` and the chip comparison share one
  identifier; `label` is the catalog name.
- A custom state (preset id `0xFEFE`) still shows the read-only curve and its
  eight band values, with no chip selected and no name (`eq-name = ""` ⇒ the
  page's "Your own settings" tag); the read-only flag hides the custom controls.
- The chart is built in `controller.rs`: the eight points use the same 600×200
  box and 0.06 margin as the vendored `EqualizerChart`, joined by a
  Fritsch–Carlson (no-overshoot) cubic curve; guides are +13.4, +7.0, 0.0,
  -6.0, -12.0 dB.
- The device worker keeps the last read `EqualizerState` so the `03 87` body
  preserves the device's HearID block (ticket 01's measured body shape), and
  serves equalizer writes in the same loop as `06 81` mode writes.
- `EqFailure` maps timeouts/I/O/protocol errors to "Equalizer profile (no
  answer)", "(connection lost)" and "(unreadable answer)"; a failed change stops
  the worker and the banner's Retry reconnects, as for sound modes.
- `equalizer_preset` flow is TDD'd with the masked real frames in `core/data`:
  the measured Bass Reducer `03 87` frame, exact write/poll order, mode+preset
  in one worker, an unknown preset id writing nothing, and a failed change
  dropping a queued click.

## Verification

- `desktop/src/controller.rs` tests (run through the desktop crate):
  preset id/name mapping and all 22 catalog values; only catalog ids selectable;
  real A3062 capture → preset "0", eight bands 100…12.8 kHz, range
  "−12.0"…"+13.4", five chart guides, non-empty curve; Bass Reducer boost/cut
  values and field texts; custom 0xFEFE state read-only with no selected chip;
  page exposure A3062 vs A3949; error mapping; stale-result rule; exact measured
  `03 87` write; re-poll; mode + preset writes in one worker; unknown preset id
  writes nothing; failed preset change ends the worker and drops the queued
  click; the custom band actions (`eq-commit`/`eq-preview`/`eq-type`/`eq-reset`)
  have no write path.
- Real hardware, 2026-11-21, Windows 11, A3062 firmware 04.39
  (`cargo test -p soundcore-desktop manual_equalizer_preset_switch -- --ignored --nocapture`):
  - `sound-pages: [SoundModes, Equalizer]`, `eq-available: true`,
    `eq-read-only: true`;
  - 22 preset ids, current preset `"0"` = Soundcore Signature;
  - eight bands 100, 200, 400, 800, 1.6, 3.2, 6.4, 12.8 kHz, values
    `[0, 0, 0, 0, 0, 0, 0, 0]`, range −12.0…+13.4 dB;
  - selected Acoustic (`1`), Bass Reducer (`3`) and Classical (`4`) through
    `Actions.eq-preset`; each re-polled state matched the catalog curve
    (`[40, 10, 20, 20, 40, 40, 40, 20]`, `[-40, -30, -10, 0, 0, 0, 0, 0]`,
    `[30, 30, -20, -20, 0, 20, 30, 40]`);
  - restored Soundcore Signature (`0`); a stopped worker then produced the
    readable banner "Equalizer profile (connection lost)", Retry reconnected and
    the device kept the restored preset. The device was left as found.
- Deferred A3949 equalizer, from the committed real capture and unit tests: the
  real A3949 `01 01` capture does hold an equalizer block at body 32..44, but
  A3949 equalizer support is deliberately deferred, so the model has no
  equalizer capability or catalog and the page stays hidden with no `03 87`
  path. The acceptance run could not page the paired P20i (ticket 03 records
  that limitation), so this is unit-test evidence on the real capture, not a
  live P20i session.
- Gates: desktop/core `cargo test`, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`.

## Acceptance

- [x] Build/test/fmt/clippy pass.
- [x] Real A3062: page shows current preset and bands; changing preset writes and updates.
- [x] A3949 equalizer deferred: no page, no `03 87`.
- [x] No `.slint` edits.
