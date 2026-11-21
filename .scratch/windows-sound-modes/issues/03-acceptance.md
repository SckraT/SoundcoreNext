# 03 — Sound mode acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify the sound mode slice on real hardware and capture evidence.

## Requirements

1. Build and run the app; connect Space One Pro (A3062).
2. Open Sound mode; verify the current mode; switch to each of noise canceling, transparency and normal; confirm the device changes (LED/app/ear) and the UI reflects state after re-poll.
3. Verify P20i (A3949) has no Sound mode page and no `06 81` is sent.
4. Failure path: with the device off/unreachable, the page reports a readable error; no crash/MAC.
5. Capture `desktop/sound-mode-connected.png` showing the Sound mode page; link from `desktop/README.md`.
6. Run all gates (desktop/core test, fmt, clippy, build).
7. Record devices/firmware (no serial/MAC), commands and results in the ticket.

## Acceptance

- [x] Real mode switches verified on A3062.
- [x] P20i no-page/no-command verified.
- [x] Screenshot stored and linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. No Bluetooth
pairing was changed: the app only uses devices already paired with Windows. No
MAC address or serial number is recorded here; test output and the screenshot
mask or omit them.

### Environment

- **OS:** Windows 11 Pro, build `10.0.26300.9974` (26H2), x64; primary display
  2560×1440.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the devices below.
- **Devices:**
  - soundcore **Space One Pro** — catalog **A3062**, ten-step single battery,
    firmware **04.39** (read from the `01 01` state); the switch run and the
    screenshot.
  - soundcore **P20i** — catalog **A3949**, five-step battery, firmware
    **14.43**; no sound mode.
  - soundcore **Liberty 4 Pro** — catalog **A3954**, off/unreachable (radio page
    timeout); used for the readable-failure check only.

### Real in-app acceptance (A3062)

- `cd desktop; cargo build` → `Finished dev profile ... in 2m 11s`, only the
  pre-existing vendored-Slint layout build warnings.
- Started `desktop/target/debug/soundcore-desktop.exe` and drove it through
  Windows UI Automation (the Slint accessibility tree, as the MVP acceptance
  did): Add Device → search `A3062` → choose the **Space One Pro** card → pick
  the paired row **soundcore Space One Pro**, marked “looks like the chosen
  model”. The connection succeeded over RFCOMM; the Overview shows
  `SOUNDCORE · A3062`, one whole-device **20 %** tile (raw 2/10) and the sidebar
  gains **SOUND → Sound mode**.
- Opening **Sound mode** shows the vendored page with the three tiles
  **Noise canceling / Transparency / Normal**; the current mode reported on
  connect was **Normal** (the check mark was on the Normal tile), and the page
  shows only the mode tiles: ANC strength, wind and the 1-to-11 scale stay out
  of scope.
- Switching through real UI clicks on the accessibility buttons
  (`accessible-action-default` → `Actions.set-option("ambientSoundMode", i)`),
  reading the checked tile from the accessibility tree after each round trip:
  Noise canceling → Noise canceling selected; Transparency → Transparency
  selected; Normal → Normal selected (the device was left as found, in normal
  mode). Each click drives the single `06 81` write path; the UI selects the
  mode only from the subsequent `01 01` re-poll, so the selected tile is the
  mode the device itself reports (state body offset 69, measured in ticket 01).
  The scripted-transport test in ticket 02 asserts that path writes exactly one
  mode frame followed by one state request.
- **Screenshot:** `desktop/sound-mode-connected.png` (1618×1072, the app window
  captured with `PrintWindow(PW_RENDERFULLCONTENT)`, so only the app window is
  in the image). It shows the title “Sound mode · soundcore Space One Pro —
  OpenSCL4”, the connected device card, the SOUND / Sound mode nav, and the
  three tiles with Normal selected. No Bluetooth address or serial number is in
  the image.

### P20i (A3949): no page, no mode command

- In the same running app: Add Device → search `A3949` → choose the P20i card →
  pick **soundcore P20i**; it connects (Overview: Left 80 %, Right 80 %). The
  sidebar then shows only **Overview / My devices / Settings** — there is no
  SOUND group, no Sound mode tab — and the accessibility tree has no mode
  tiles, so no mode change is reachable from the UI.
- The ignored real-hardware test `manual_p20i_has_no_sound_mode` additionally
  presses the `ambientSoundMode` action on the live connection: nothing changes,
  no banner, no page. The session refuses such a write with
  `UnsupportedSoundMode` (ticket 01 unit test) and the model has no `06 81`
  body, so no mode frame has a path on this model.

### Failure path

- **A change failing on a live connection:** the ignored
  `manual_sound_mode_switch` covers the failed-worker branch — the state a
  failed write/read leaves. The next click shows the vendored banner
  `Sound mode (connection lost)` (readable, no MAC) and the banner's **Retry**
  reconnects, restores the page and reads the mode back. This was re-run on the
  real A3062 in this ticket.
- **Unreachable device:** Liberty 4 Pro (A3954, off) ends in the readable
  “The device did not answer in time. Make sure it is turned on and nearby,
  then try again.” state — no crash, no address. Powering the connected Space
  One Pro off mid-session was not done, to avoid changing the user's device and
  Bluetooth state; the timeout/lost/unreadable mappings of that branch are also
  covered by the unit tests in ticket 02.

### Hardware commands and output (masked)

```
cd desktop
cargo test -p soundcore-desktop manual_sound_mode_switch -- --ignored --nocapture
# == A3062 Space One Pro ==
#   sound-pages: [SoundModes]
#   modes: ["NoiseCanceling", "Transparency", "Normal"] labels: ["Noise canceling", "Transparency", "Normal"]
#   mode-index: 2 (mode-scale: false, mode-rows: 0)
#   tile 0 -> re-polled mode index 0
#   tile 1 -> re-polled mode index 1
#   tile 2 -> re-polled mode index 2
#   quick clicks 0, 1, 2 -> settled on 2
#   after a failed worker: error-label "Sound mode (connection lost)"
#   retry: connected=Connected sound-pages=1 error-label=""
#   retried connection reports mode index 2
# test result: ok. 1 passed; 0 failed

cargo test -p soundcore-desktop manual_p20i_has_no_sound_mode -- --ignored --nocapture
# == A3949 P20i / P25i / R50i ==
#   sound-pages: 0 modes: 0 mode-index: -1
#   ambientSoundMode click ignored: no page, no 06 81 path
# test result: ok. 1 passed; 0 failed

cargo test -p soundcore-desktop manual_paired_connect -- --ignored --nocapture
# A3949: connected: label 0=80% label 1=80%, firmware 14.43
# A3954: failed (TimedOut): The device did not answer in time. Make sure it is
#        turned on and nearby, then try again.
# A3062: connected: label 3=20%, firmware 04.39
# test result: ok. 1 passed; 0 failed
```

### Gates

```
cd desktop
cargo test                                  # 85 passed; 0 failed; 5 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 60 passed across 16 targets; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```
