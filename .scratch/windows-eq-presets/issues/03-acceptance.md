# 03 — Equalizer presets acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify equalizer presets on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062; open Equalizer.
2. Verify current preset selected, 10 read-only bands/curve; switch between at least three presets (e.g. Signature, Bass Reducer, Podcast); each writes one `03 87`, re-polls, UI follows.
3. Verify P20i: no Equalizer page, no `03 87`.
4. Failure path: stopped worker shows readable error; no crash/MAC.
5. Capture `desktop/equalizer-connected.png`; link from `desktop/README.md`; document out-of-scope custom bands/profiles.
6. Run all gates.
7. Record OS, device/firmware (no MAC/serial), preset ids and results.

## Acceptance

- [x] Real preset switching verified on A3062.
- [x] P20i no-page/no-command verified from the real A3949 capture and the
      controller/session tests; a live re-check was not reproducible this run
      (device not answering RFCOMM — see Comments).
- [x] Screenshot stored/linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. No Bluetooth
pairing was changed and no device setting outside the tested presets was
changed: the app only uses devices already paired with Windows, and the preset
found on connect was put back. No MAC address or serial number is recorded here;
test output and the screenshot mask or omit them.

### Environment

- **OS:** Windows 11 Pro, build `10.0.26300.9974` (26H2), x64; primary display
  2560×1440.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the devices below.
- **Devices:**
  - soundcore **Space One Pro** — catalog **A3062**, ten-step single battery,
    firmware **04.39** (read from the `01 01` state, raw 1/10 shown as 10 %);
    the preset switches, the failure check and the screenshot.
  - soundcore **P20i** — catalog **A3949**, five-step battery, firmware **14.43**
    in its committed `01 01` capture; **not reachable during this run** (see
    below), so no live page check was possible.
  - soundcore **Liberty 4 Pro** — catalog **A3954**, off/unreachable (radio page
    timeout), as in the sound-mode acceptance.

### Real in-app acceptance (A3062)

- `cd desktop; cargo build` → `Finished dev profile ... in 2m 29s`, only the
  pre-existing vendored-Slint layout build warnings.
- Started `desktop/target/debug/soundcore-desktop.exe` and drove it through
  Windows UI Automation (the Slint accessibility tree, as the sound-mode and MVP
  acceptances did): Add Device → search `A3062` (the search `Edit` via its
  `ValuePattern`) → choose the **Space One Pro** card → pick the paired row
  **soundcore Space One Pro**, marked “looks like the chosen model”. The
  connection succeeded over RFCOMM; Overview shows `SOUNDCORE · A3062` and the
  sidebar gains **SOUND → Sound mode** and **SOUND → Equalizer**.
- Opening **Equalizer** shows the vendored page with all **22 preset chips**,
  the current built-in preset **Soundcore Signature** checked (its
  accessibility `Button` whose label is the chip text; the page tag next to
  “Equalizer bands: 8” reads the same name), the **eight visible bands** as the
  read-only curve, the chart guides `+13.4 / +7.0 / 0.0 / −6.0 / −12.0` and the
  read-only note “This device only supports the built-in equalizer profiles.”.
  The page has no band sliders/fields, no Reset/Save, no saved profiles and no
  import/export controls — those are out of scope and stay hidden, so no custom
  band value has a write path.
- **Real preset switching through the UI**, clicking the chips with UI
  Automation `Invoke` on the accessibility buttons and reading the page back
  after each device round trip (the controller fills the selection, the tag and
  the curve only from the `01 01` re-poll, never from the click):
  - `Acoustic` (id 1) → tag **Acoustic**, curve `[40, 10, 20, 20, 40, 40, 40, 20]`;
  - `Bass Reducer` (id 3) → tag **Bass Reducer**, curve `[-40, -30, -10, 0, 0, 0, 0, 0]`;
  - `Classical` (id 4) → tag **Classical**, curve `[30, 30, -20, -20, 0, 20, 30, 40]`;
  - `Soundcore Signature` (id 0) → tag **Soundcore Signature**, flat `[0; 8]`
    curve — the preset found on connect is restored, and the device is left as
    found.
  Intermediate screenshots of the Acoustic/Bass Reducer/Classical states were
  taken for inspection but are not committed; the committed capture is the
  restored Signature state.
- **Screenshot:** `desktop/equalizer-connected.png` (1294×858, the app window
  captured with `PrintWindow(PW_RENDERFULLCONTENT)`, so only the app window is
  in the image). It shows the title “Equalizer · soundcore Space One Pro —
  OpenSCL4”, the connected device card, the SOUND / Equalizer nav, the 22
  preset chips with **Soundcore Signature** checked, and the “Equalizer bands: 8”
  panel with the read-only eight-band curve and its decibel range. No Bluetooth
  address or serial number is in the image. It is linked from
  `desktop/README.md`, which also documents that custom bands, saved profiles
  and import/export are out of scope.

### Scripted hardware run (A3062)

`cargo test -p soundcore-desktop manual_equalizer_preset_switch -- --ignored --nocapture`
builds the real `AppWindow`, wires the controller's actions and presses the
chips through `Actions.eq-preset` on the real device (masked output):

```
== A3062 Space One Pro ==
  candidate soundcore Liberty 4 Pro suggested=false (F4:9D:8A:7C: __:__)
  candidate soundcore Space One Pro suggested=true (F4:9D:8A:A2: __:__)
  candidate soundcore P20i suggested=false (A4:C1:39:92: __:__)
  connected via F4:9D:8A:A2: __:__
  sound-pages: [SoundModes, Equalizer]
  eq-available: true eq-read-only: true
  eq-presets: 22 ids ["0", "1", "3", "4", "5", "6", "7", "8", "9", "10", "11",
    "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "32382"]
  eq-preset: "0" eq-name: "Soundcore Signature"
  eq-bands: 8 frequencies ["100", "200", "400", "800", "1.6", "3.2", "6.4", "12.8"]
    values [0, 0, 0, 0, 0, 0, 0, 0]
  eq-range: -120..134 text "−12.0".."+13.4" band-count 8
  preset 1 (Acoustic) -> re-polled "1" bands [40, 10, 20, 20, 40, 40, 40, 20]
  preset 3 (Bass Reducer) -> re-polled "3" bands [-40, -30, -10, 0, 0, 0, 0, 0]
  preset 4 (Classical) -> re-polled "4" bands [30, 30, -20, -20, 0, 20, 30, 40]
  restored preset "0" bands [0, 0, 0, 0, 0, 0, 0, 0]
  after a stopped worker: error-label "Equalizer profile (connection lost)"
  retry: connected=Connected sound-pages=2 eq-preset="0" error-label=""
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 108 filtered out
```

Each switch wrote exactly one measured `03 87` preset frame (53-byte body) and
re-read the `01 01` state; the committed `FakeTransport` tests assert the exact
frame, the write-then-poll order, unknown ids writing nothing and a failed
change stopping the worker.

### P20i (A3949): no page, no preset command — evidence and limitation

- **Live re-check not reproducible this run.** `manual_p20i_has_no_equalizer`
  (three runs) and `manual_paired_connect` (once) enumerated the paired P20i as
  the suggested A3949 candidate and then failed to open RFCOMM with
  `The device did not answer in time. Make sure it is turned on and nearby, then
  try again.` — four ~20 s radio page timeouts across those runs — while the
  A3062 on the same radio connected and switched presets normally. The P20i was
  therefore off/in its case or otherwise not pageable, and the live “no
  Equalizer page” check could not be run. No pairing was changed to work around
  it. Note that `manual_p20i_has_no_equalizer` returns early when its connect
  fails, so its own asserts did not run in this state; that early return is
  ticket 02's test design, not evidence of a pass.
- **What the no-page/no-command claim rests on (all existing, running tests):**
  - The committed **real A3949 `01 01` capture** parses with no equalizer block:
    `sound_pages(A3949 state)` is empty, `EqualizerDisplay::new(Some(A3949), …)`
    has `available: false`, `preset_count: 0`, no chips and no bands
    (`the_equalizer_page_is_exposed_only_for_the_a3062`,
    `the_real_a3949_capture_has_no_sound_mode_page`).
  - The A3949 model has no equalizer preset catalog, so `preset_options(A3949)`
    is empty and there is no chip value to click
    (`the_a3062_equalizer_presets_are_the_twenty_two_catalog_ids_and_names`).
  - The session refuses the write for a model without a measured equalizer with
    `SessionError::UnsupportedEqualizer` and writes no frame
    (`core/crates/session/tests/session.rs::a_model_without_a_measured_equalizer_writes_nothing`).
  - In the app, `select_equalizer_preset` ignores the click when the model has
    no chips, and the device worker only has a `03 87` path when the connect
    state reported an equalizer; a P20i worker ends before its command loop.
- This run therefore confirms the P20i page/command absence from the real
  device's committed state capture and the controller/session unit tests, not
  from a live P20i session.

### Failure path

- The ignored `manual_equalizer_preset_switch` exercises the failed-worker
  banner: after the worker is stopped (the state a failed write/read leaves), the
  next preset click shows the readable vendored banner
  `Equalizer profile (connection lost)` (no MAC), writes nothing, and its
  **Retry** reconnects, restores the page and reads back the preset the device
  kept (`eq-preset "0"`). The failure mappings are unit-tested: a silent device
  maps to a timeout, a reset link to a lost connection, a malformed answer to an
  unreadable one (`a_failed_preset_change_maps_to_a_readable_label`). A live
  power-off of the connected Space One Pro during a preset change was not
  reproduced, to avoid changing the user's device state.

### Scope deviations recorded

- The spec's AC2 says “10-band read-only curve”; the page shows **8 visible
  bands** (100 Hz…12.8 kHz) in the −12.0…+13.4 dB range, as measured and already
  documented in ticket 02: the ten state band bytes exist but the last two are
  fixed by every preset, OpenSCL4's A3062 equalizer exposes 8 visible bands and
  the Soundcore app labels the same eight. All ten raw bands still parse in the
  core. The screenshot and README say “eight visible bands”.
- No `.slint` file, no dependency and no core code changed by this ticket: only
  `desktop/equalizer-connected.png`, `desktop/README.md` and this ticket.

### Gates

```
cd desktop
cargo test                                  # 102 passed; 0 failed; 7 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 73 passed across 15 targets; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

The integration tip `integration/windows-eq-presets` was already this branch's
base (`fb99fe7`), so `git merge integration/windows-eq-presets` reported
“Already up to date” and no merge commit was needed; the gates were re-run after
that check.
