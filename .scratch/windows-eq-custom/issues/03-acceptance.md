# 03 — Custom equalizer acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify custom equalizer editing on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062; open Equalizer.
2. Drag/type at least two bands to a distinctive curve; commit; verify re-polled state and device sound/state match; screenshot `desktop/equalizer-custom.png`.
3. Press Reset; verify flat curve and state.
4. Switch to a built-in preset and back to custom; verify both still work.
5. Verify chart preview updates during drag without writes (code/test evidence).
6. Failure path: stopped worker shows readable error; no crash/MAC.
7. Link screenshot from `desktop/README.md`, update out-of-scope note.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware (no MAC/serial), curve values, results.

## Acceptance

- [x] Real custom curve/commit/reset/preset verified.
- [x] Screenshot stored/linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. No Bluetooth
pairing was changed and the device was left on the state found on connect
(built-in **Soundcore Signature**, id 0, flat bands): the custom curve was
written, re-read, reset and screenshotted during the run, then a preset chip
restored the found state. No MAC address or serial number is recorded here or
in any output below; the hardware tests print every address as
`<address masked>`.

### Environment

- **OS:** Windows 11 Pro, 26H2, build `10.0.26300.9974`, x64; primary display
  2560×1440.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the devices below.
- **Device under test:** soundcore **Space One Pro** — catalog **A3062**,
  ten-step single battery, firmware **04.39** re-read from the `01 01` state on
  this run (`raw 5/10` shown as `50 %`).
- **Other paired devices:** soundcore P20i (A3949) and Liberty 4 Pro (A3954)
  were off/unreachable for this run — enumeration lists them, every RFCOMM open
  fails with the readable `Timed out` message after the radio page timeout. No
  pairing change was made. Neither model is needed by this ticket; the A3949
  equalizer stays deliberately deferred (see ticket 02 and the root README).
- **Build:** `cd desktop; cargo build` → `Finished dev profile … in 2m 09s`
  (only the pre-existing vendored-Slint layout warnings), started
  `desktop/target/debug/soundcore-desktop.exe` for the in-app run.

### Real in-app acceptance (A3062, UI Automation)

The running app was driven through the Windows accessibility tree (the Slint
tree, as the previous Windows acceptances did): **Add Device** → model search
`A3062` → the **Space One Pro** card → the paired row **soundcore Space One
Pro** (marked “looks like the chosen model”). The connection succeeded over
RFCOMM, Overview showed the device and the sidebar gained
**SOUND → Sound mode / Equalizer**. Opening **Equalizer** gave the editable
page:

- 8 band value fields (`Edit`) labelled `100 Hz, dB` … `12.8 kHz, dB`, all
  `0` on connect, and 8 `Slider`s with the same frequencies and values;
- the **Reset** button, the “Values in decibels, from −12.0 to +13.4 dB” range
  note and the “Drag the points on the chart, move the sliders with ↑ ↓ or type
  the values.” hint;
- **no** read-only note (“This device only supports the built-in equalizer
  profiles.”), so the page is editable (`eq-read-only = false`); the
  `manual_equalizer_custom_curve` run below prints the same
  `eq-read-only: false` directly from the controller;
- all **22 catalog preset chips** and the “Equalizer bands: 8” heading; the
  name tag next to the heading showed the found preset **Soundcore Signature**.

**Custom curve, typed and dragged.** The 100 Hz field was clicked, select-all
and `4.0` typed, then Enter: the device round trip replaced the page state
with preset id `65278` (custom), the name tag became **Your own settings** and
the fields read `[4, 0, 0, 0, 0, 0, 0, 0]` (re-polled). The 800 Hz point on the
chart was then dragged down with real mouse input; the page followed the drag,
and letting go committed the curve: the re-polled fields read
`[4, 0, 0, −3.9, 0, 0, 0, 0]` (one chart pixel is about 0.15 dB, so the release
landed one tenth low). Typing `−4.0` into the 800 Hz field gave the exact
distinctive curve `[4, 0, 0, −4, 0, 0, 0, 0]` with the name tag still
**Your own settings** — the commit wrote one `03 87` custom frame (`0xFEFE`)
and the page re-read `01 01`.

**Reset.** Pressing **Reset** produced the flat custom curve
`[0, 0, 0, 0, 0, 0, 0, 0]` with the name tag still **Your own settings** (the
device reports `0xFEFE`, not a built-in id), i.e. a flat custom curve was
written and re-polled, not just a local clear.

**Built-in preset and back to custom.** Pressing the **Acoustic** chip wrote
one `03 87` preset frame and the re-polled page showed the tag **Acoustic** and
the measured curve `[4, 1, 2, 2, 4, 4, 4, 2]`. Pressing **Reset** and typing
`4.0` / `−4.0` again moved the device back to the custom curve
`[4, 0, 0, −4, 0, 0, 0, 0]` with the tag **Your own settings**, so both the
built-in preset path and the custom path still work after each other.

**Restored.** Finally the **Soundcore Signature** chip was pressed; the
re-polled tag read **Soundcore Signature** and the bands
`[0, 0, 0, 0, 0, 0, 0, 0]` — the state found on connect.

Every value read back in the UI was obtained after a device round trip: the
controller fills the tag, the fields, the sliders and the chart only from the
re-polled `01 01` state, never from the click or the typed text.

### Scripted hardware run (the committed writable path)

`cd desktop; cargo test -p soundcore-desktop manual_equalizer_custom_curve -- --ignored --nocapture`
builds the real `AppWindow`, wires the controller's actions and drives the same
`eq-type`/`eq-preview`/`eq-commit`/`eq-reset`/`eq-preset` callbacks on the real
device. All Bluetooth addresses are masked:

```
== A3062 Space One Pro ==
  candidate soundcore P20i suggested=false (<address masked>)
  candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
  candidate soundcore Space One Pro suggested=true (<address masked>)
  connected via <address masked>
  eq-available: true eq-read-only: false bands: 8 presets: 22
  found: preset "65278" name "" bands [0, 0, 0, 0, 0, 0, 0, 0]
  eq-type band 0 4.0 -> re-polled custom curve [40, 0, 0, 0, 0, 0, 0, 0]
  eq-preview band 3 -4.0 -> page curve [40, 0, 0, -40, 0, 0, 0, 0]
  eq-commit band 3 -4.0 -> re-polled custom curve [40, 0, 0, -40, 0, 0, 0, 0] preset "65278"
  eq-reset -> re-polled flat custom curve [0, 0, 0, 0, 0, 0, 0, 0]
  preset 0 (Soundcore Signature) after custom -> bands [0, 0, 0, 0, 0, 0, 0, 0] name "Soundcore Signature"
  restored custom curve [0, 0, 0, 0, 0, 0, 0, 0]
  after a stopped worker: error-label "Equalizer bands (connection lost)"
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 123 filtered out; finished in 1.87s
```

The run started from a flat custom curve: the in-app pass had left a distinctive
custom curve, so it was set flat with the page's Reset before the scripted run,
which is also why the test found `0xFEFE` with flat bands. A built-in preset and
the distinctive custom curve were written again through the app afterwards (see
above) and the found Soundcore Signature preset was restored at the end. The
committed `FakeTransport` tests assert the exact writes off the real device: one
63-byte `03 87` custom frame (`FE FE` plus the eight `78`-offset values in both
band regions) followed by exactly one `01 01`, the flat reset frame, a preset
frame built from the custom state's HearID block, and no write at all for a
preview.

### Preview updates without writes

- Automated evidence: `a_preview_moves_the_page_without_a_curve_write` (the
  page follows the drag and the recorded writer receives nothing) and the
  same drag is printed by the hardware run above
  (`eq-preview band 3 -4.0 -> page curve [...]`) before the commit line — the
  page curve moved with no `03 87` write and no re-poll in between.
- In the app the chart moved on pointer-down/move and the name tag stayed on
  the previous re-polled state until the release, when the one custom write
  happened and the re-poll changed the fields.

### Failure path

- With the worker stopped (the state a failed write/read leaves), the next
  band commit showed the readable vendored banner
  `Equalizer bands (connection lost)` (no MAC, no crash) and wrote nothing; the
  banner's **Retry** reconnected and the device kept the restored state
  (`eq-preset` unchanged, error banner gone). That is the last line of the
  hardware transcript above.
- The failure mappings are unit-tested in ticket 02:
  `a_failed_curve_change_maps_to_a_readable_label` (timeout, lost link,
  malformed answer, unsupported model) and
  `a_stopped_worker_shows_the_readable_curve_error`.
- A live power-off of the connected Space One Pro during a write was not
  reproduced, to avoid changing the user's device state.

### Screenshot and docs

- `desktop/equalizer-custom.png` (1400×1180): the app window captured with
  `PrintWindow(PW_RENDERFULLCONTENT)`, so only the app window is in the image.
  It shows the title “Equalizer · soundcore Space One Pro — OpenSCL4”, the
  connected device card, the 22 preset chips, the chart with the
  `+4.0 dB` / `−4.0 dB` custom curve, the **Your own settings** tag, the eight
  editable band fields (`4 0 0 −4 0 0 0 0`) and sliders, the Reset button and
  the range note. No Bluetooth address or serial number is in the image; the
  page shows no saved-profile or import/export controls (out of scope).
- `desktop/README.md` now documents the editable bands (fields, sliders,
  preview/commit/re-poll, Reset, still-switchable presets) instead of the
  read-only page, keeps saved profiles/import-export/HearID and the A3949
  deferral out of scope, and links the new capture. The old read-only
  equalizer capture from the preset slice stays in the tree but is no longer
  linked, because the A3062 page it showed has the editable controls now.
- The root `README.md` status row for the A3062 equalizer was corrected the
  same way (editable custom bands, preset and custom `03 87` bodies).

### Recorded deviations and limitations

- **8 visible bands, 10 parsed.** The page shows the eight visible bands
  (100 Hz…12.8 kHz) in the measured −12.0…+13.4 dB range; all ten raw band
  bytes still parse in the core. This is the measured A3062 layout from
  tickets 01/02, not a reduction introduced here.
- **A3949 equalizer stays deferred.** No A3949 page and no A3949 `03 87` was
  written or re-checked live in this run; the deferral rests on ticket 02's
  core/session tests and the committed A3949 state capture. The P20i was not
  reachable (radio page timeout) and no pairing change was made to reach it.
- **Manual-test precondition (observed, documented, not changed).** The
  ignored `manual_equalizer_custom_curve` asserts that a `−4.0 dB` preview
  changes the chart. When the device was already left on the same `−4.0 dB`
  value by the preceding in-app pass, the preview was a correct no-op and that
  assertion failed (the run before the config above). The acceptance run
  started the scripted pass from a flat custom curve, where the preview has
  something to move, and then restored the distinctive curve through the app
  and finally the found preset. The assertion could be made independent of the
  state found on connect in a follow-up (only the manual hardware test would
  change). This is a test-precondition finding, not a product defect.
- No `.slint` file, no dependency and no product code changed by this ticket:
  the diff is `desktop/equalizer-custom.png`, `desktop/README.md`,
  `README.md` and this ticket.
- The battery read `50 %` on this run (raw 5/10) where earlier tickets read
  `10 %`; the battery value is not part of this ticket.

### Gates

```
cd desktop
cargo test                                  # 117 passed; 0 failed; 7 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 77 passed across 15 targets; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

Ticket 02's hardware tests were not re-run this pass except
`manual_equalizer_custom_curve` (above) and `manual_paired_connect` (firmware
and state for this record); the automated desktop suite includes the ticket 02
curve tests (117 passed, same count as ticket 02).

### Merge

The acceptance branch is based on the integration tip
`bab31f9` (merge of `ticket/cq-02-controller`). `git merge
integration/windows-eq-custom` after the acceptance commit reported
“Already up to date”, so no merge commit was needed; the gates above were run
on that tree.
