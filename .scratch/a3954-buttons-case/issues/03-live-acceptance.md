# 03 — Live A3954 buttons/case acceptance

**Status:** done
**Blocked by:** 02 (done)
**Implemented:** 2026-11-23

## Goal

Verify the 12 gestures and case settings on the real A3954 and restore them.

## Requirements

1. Add/extend an ignored hardware test that connects the paired A3954 and:
   - reads all 12 button actions and the case features/language;
   - changes one action per gesture class (single, double, triple, long, slide
     up, slide down), polling/asserting the re-read value;
   - toggles each case feature and changes the case language, polling/asserting;
   - restores every original button action, case toggle and language; asserts
     final state field-by-field;
   - no serial/address printed; masked address only.
2. Run the test with `--ignored --nocapture`; if a write drops the link,
   reconnect and restore.
3. Build/run the app with isolated APPDATA; verify Controls/Case pages populate
   (UIA/screenshots), no values clicked through UI except the test's restores.
4. Update README with real facts and ticket record; no serial/MAC.
5. Gates: desktop/core fmt/build/test/clippy.

## Result (2026-11-23, MSK)

**Hardware.** Real Liberty 4 Pro paired with the test machine, connected over
Windows RFCOMM by the ignored test (no pairing change). Firmware **04.29** on
both earbuds, case firmware **01.58**; the state carried case battery **60 %**
and earbud batteries 57/48 % during the UI run. The device reported its **TWS
link down** (`01 01` body byte 1 = 0, host = left earbud): only the
disconnected-state action slot was live, and the device applies a `04 81` write
only to that slot (see the two measured corrections below). The address is
masked in every output; no serial, MAC or frame bytes were printed.

**Protocol run.** From `desktop`, with the finished test:
`cargo test -p soundcore-desktop manual_a3954_buttons_and_case -- --ignored
--nocapture` → **1 passed / 0 failed / 0 ignored, 0 reconnects, 18.09 s**. The
test is `desktop/src/controller.rs::manual_a3954_buttons_and_case`; it connects
through `open_session` (the same RFCOMM path as the window's connect) and drives
a real `DeviceSession`, with the existing 350 ms write spacing / retry and the
reconnect-and-retry path.

Every line below is one write, a `01 01` re-poll assertion, then a restore
asserted by a second re-poll. The test edits the action byte's **live**
connection slot and keeps the other nibble, so a restore writes the whole byte
back exactly:

- left/right single press: Play/pause → Volume up → Play/pause;
- left/right double press: Next song → Volume up → Next song;
- left/right triple press: None → Volume up → None;
- left/right long press: Ambient sound mode → Volume up → Ambient sound mode;
- left/right slide up: Volume up → Volume down → Volume up;
- left/right slide down: Volume down → Volume up → Volume down;
- case-screen features, one at a time (other three kept): atmospheric off→on→off,
  remote camera off→on→off, find device off→on→off, case spatial audio
  off→on→off;
- case language: English → Japanese → Chinese → English.

The final poll matched the connect state byte for byte for all 12 action bytes
(main `66 66 32 33 FF FF 44 44`, slides `00 00 11 11`) and for the four case
features, the case language, the air pressure (raw `0x61` = 0.97 atm) and both
firmwares; `0 reconnect(s)`; pairing untouched. `desktop` reports **291 passed /
0 failed / 15 ignored** (this is the 15th ignored hardware test).

**Two measured corrections found by the live run (implemented here).**

1. **`04 81` writes the live connection slot only.** With the TWS link down, a
   write of `[side, button_id, 0x60]` changed only the action byte's high
   (disconnected-state) nibble; the low nibble stayed as the device reported it.
   The controller previously always showed/wrote the TWS (low) nibble, so a
   remap would silently do nothing on a real device in this state. The Controls
   rows and `answer_a3954_button` now use the `01 01` TWS flag to pick the live
   slot (`with_tws` when connected, `with_non_tws` when not) and keep the other
   nibble, matching the OpenSCL4 `TwsStatus` behavior. A regression test uses a
   TWS-down fixture (`left single press` `0x36` shows next song, the write keeps
   the connected `6` and sends `0x06`).
2. **`07 87` case-feature body order is not the state order.** The app's
   `M5` builder (`CmmBtCmdService`) sends
   `[remote camera, find device, case spatial audio, atmospheric]`, while the
   state reports `[atmospheric, remote camera, find device, case spatial
   audio]`. Writing the state order on the real device toggled the neighbouring
   feature (a body of `[1, 0, 0, 0]` switched the remote camera on). The core
   `A3954CaseFeatures::command_body` now uses the app's order; the exact `07 87`
   frames in `core/crates/models/tests/a3954.rs` and
   `core/crates/session/tests/a3954.rs` were updated.

**UI run.** The built app (`cargo build` from `desktop`) was started with an
isolated `%APPDATA%` (English UI; the user's app data untouched), connected the
paired Liberty 4 Pro through the real Add Device dialog (real RFCOMM, Demo mode
off) and both pages populated from the device:
Controls — the Presses panel with all eight press rows showing the
device-reported actions shown above, the Slides panel with the four slide rows
(Volume up/up/down/down), the "Switching modes" ambient-cycle panel
(Noise canceling on, Transparency on, Normal off), and 12 select controls found
by UIA; Case — the Case screen panel with the four feature toggles and the
English case-language row, and the read-only Status panel with case battery
60 % and case firmware 01.58. The UIA run clicked no setting control (tabs and
the add-device buttons only; the page is scrolled with wheel events and no
mouse press) and checked both pages for a Bluetooth address and a serial.
Screenshots (no address/serial) are committed as
`desktop/live-a3954-controls.png`, `desktop/live-a3954-controls-slides.png` and
`desktop/live-a3954-case.png`; the script is
`.scratch/a3954-buttons-case/live-ui-a3954-buttons-case.ps1`.

**Restore check after the whole UI run.** A second protocol run of the same
ignored test read exactly the connect state printed above before writing, and
the final poll matched it again; the device is left as found. (One UI attempt
before the recorded run used a click-based scroll helper that opened a select
popup and changed `left long press` to "Voice assistant". That byte was written
back to the connect value from the pre-run state, the helper was replaced by a
wheel-only scroll, and both the recorded UI run and the final protocol run
verified the device as found. It is recorded here so the review knows the
device was restored and how the script now avoids the hazard.)

**Deferred (unchanged).** Case display brightness/color/style/text and
air-pressure writes stay out of this slice; no serial/address is read for output
or shown on a page.

## Gates

- Desktop (from `desktop`): `cargo fmt --all -- --check` clean, `cargo build`
  clean, `cargo test` **291 passed / 0 failed / 15 ignored** (the ignored tests
  are the manual hardware checks, including this ticket's
  `manual_a3954_buttons_and_case`), `cargo clippy --all-targets -- -D warnings`
  clean.
- Core (from `core`): `cargo fmt --all -- --check` clean,
  `cargo build --workspace` clean, `cargo test --workspace` **195 passed / 0
  failed / 0 ignored**, `cargo clippy --workspace --all-targets -- -D warnings`
  clean.

## Acceptance

- [x] All gesture/case writes confirmed and restored; device left as found.
- [x] Live findings measured and fixed; ticket done.
