# 03 — Equalizer presets acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify equalizer presets on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062; open Equalizer.
2. Verify the current preset is selected and the 8 visible read-only bands/curve
   are shown (the core still parses all 10 band bytes); switch between at least
   three presets (e.g. Signature, Bass Reducer, Podcast); each writes one
   `03 87`, re-polls, UI follows.
3. Verify the A3949 equalizer deferral: no Equalizer page, no `03 87`. This is a
   limitation, not evidence of absence — the committed A3949 state carries an
   equalizer block at body 32..44, but no A3949 write body is verified.
4. Failure path: stopped worker shows readable error; no crash/MAC.
5. Capture `desktop/equalizer-connected.png`; link from `desktop/README.md`; document out-of-scope custom bands/profiles.
6. Run all gates.
7. Record OS, device/firmware (no MAC/serial), preset ids and results.

## Acceptance

- [x] Real preset switching verified on A3062.
- [x] A3949 equalizer deferral (no page/no `03 87`) verified from the real A3949
      capture and the controller/session unit tests; a live re-check was not
      reproducible this run (device not answering RFCOMM — see Comments).
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
chips through `Actions.eq-preset` on the real device. Every Bluetooth address is
masked in full (`<address masked>`); no octet of any address is recorded:

```
== A3062 Space One Pro ==
  candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
  candidate soundcore Space One Pro suggested=true (<address masked>)
  candidate soundcore P20i suggested=false (<address masked>)
  connected via <address masked>
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
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 107 filtered out
```

Each switch wrote exactly one measured `03 87` preset frame (53-byte body) and
re-read the `01 01` state; the committed `FakeTransport` tests assert the exact
frame, the write-then-poll order, unknown ids writing nothing and a failed
change stopping the worker.

### A3949 (P20i): equalizer deliberately deferred — a limitation, not absence

- **The A3949 does carry equalizer data.** The committed real A3949 `01 01`
  capture (`core/data/a3949-state.json`) has an equalizer block at body
  32..44: preset id `00 00` at body 32..34 and ten band bytes at 34..44
  (`78 78 78 78 78 78 78 78 78 00` = nine 0 dB bands and a −12.0 dB last band in
  the same byte = value + 120 mapping as A3062). OpenSCL4's A3949 state parser
  reads the same block (`CommonEqualizerConfiguration<1, 10>`, right after the
  16-byte serial), so “the P20i has no equalizer” is **not** a supportable claim
  and this ticket does not make it.
- **Why this slice still hides it.** No A3949 `03 87` write body is verified:
  OpenSCL4 has no A3949 equalizer module (no setting handler and no outbound
  packet), so an A3949 write would be invented. A3949 equalizer support is
  therefore deliberately deferred, and the page stays hidden with no `03 87`
  path. This is a recorded limitation.
- **What the deferral rests on (unit tests, all running):**
  - The A3949 model has no equalizer capability, catalog or write body, and its
    committed capture's block bytes at body 32..44 are asserted as the
    known-but-deferred data
    (`core/crates/models/tests/equalizer.rs::a3949_equalizer_is_explicitly_deferred`);
    the committed capture still parses with `state.equalizer == None` because
    the capability is deliberately absent.
  - `sound_pages(A3949 state)` is empty, `preset_options(A3949)` is empty,
    `EqualizerDisplay::new(Some(A3949), …)` has `available: false`,
    `preset_count: 0`, no chips and no bands, and no chip value is selectable
    (`the_equalizer_page_is_exposed_for_the_a3062_and_deferred_for_the_a3949`);
    the real A3949 capture also has no sound mode page
    (`the_real_a3949_capture_has_no_sound_mode_page`).
  - The session refuses the write with `SessionError::UnsupportedEqualizer` and
    writes no frame
    (`core/crates/session/tests/session.rs::a_deferred_equalizer_model_writes_nothing`).
  - In the app, `select_equalizer_preset` ignores the click when the model has
    no chips, and the device worker only has a `03 87` path when the connect
    state reported an equalizer; an A3949 worker ends before its command loop.
- **Live re-check not reproducible this run.** The former
  `manual_p20i_has_no_equalizer` hardware test was removed in the review-fix
  pass: it returned early when its connect failed, so it could pass without
  testing anything, and its premise (“no A3949 equalizer”) was wrong. During the
  acceptance run, `manual_paired_connect` (once) and the old P20i equalizer test
  (three runs) enumerated the paired P20i as the suggested A3949 candidate and
  then failed to open RFCOMM with `The device did not answer in time. Make sure
  it is turned on and nearby, then try again.` — four ~20 s radio page timeouts —
  while the A3062 on the same radio connected and switched presets normally. The
  P20i was therefore off/in its case or otherwise not pageable. No pairing was
  changed to work around it.
- This run therefore confirms the A3949 page/command deferral from the real
  device's committed state capture and the core/controller/session unit tests,
  not from a live P20i session.

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

### Recorded deviations and limitations

- **8 visible bands, 10 parsed.** The page shows **8 visible bands**
  (100 Hz…12.8 kHz) in the −12.0…+13.4 dB range, as measured and documented in
  ticket 02: the ten state band bytes exist but the last two are fixed by every
  preset, OpenSCL4's A3062 equalizer exposes 8 visible bands and the Soundcore
  app labels the same eight. All ten raw bands still parse in the core. The
  screenshot and README say “eight visible bands”; the review-fix pass corrected
  the spec's AC2 from “10-band” to “8 visible bands” so the spec, tickets and
  implementation agree.
- **A3949 equalizer deferred.** Its equalizer block (body 32..44) is known, but
  no A3949 write body is verified, so the page is hidden and no `03 87` is sent
  for that model. This is a limitation, not evidence of absence; see the A3949
  section above.
- No `.slint` file, no dependency and no core code changed by this ticket: only
  `desktop/equalizer-connected.png`, `desktop/README.md` and this ticket.

### Gates

```
cd desktop
cargo test                                  # 102 passed; 0 failed; 6 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 73 passed across 15 targets; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

The review-fix branch `ticket/eq-04-review-fixes` was based on the integration
tip `b795c5e`; after the fixes `git merge integration/windows-eq-presets`
reported “Already up to date”, so no merge commit was needed, and the gates were
re-run on that tree.

### Review fixes (2026-11-21)

- **Docs/licensing.** README no longer calls the core clean-room and no longer
  says protocol facts come only from the wiki: the core adapts OpenSCL4's
  authorized command/response, state-offset and preset data under GPL-3.0, and
  `NOTICE` now attributes the A3062 `03 87` body layout, the A3062 preset table
  and band frequencies, and the deferred A3949 block. README gained an A3062
  equalizer status row and an A3949-deferral row, the
  `core/data/a3062-eq-presets.json` fixture and the `.scratch/windows-eq-presets/`
  layout entry.
- **Redaction.** Every byte of every Bluetooth address is removed from this
  ticket: the former transcript printed four of six octets. The hardware-test
  helper now prints `<address masked>` and no run output contains an address
  octet.
- **Spec honesty.** AC2 now says 8 visible read-only bands (the core still parses
  all 10) with the measured evidence; the A3062 preset count is the actual
  22-entry OpenSCL4 list everywhere (the “13” in ticket 01 is corrected); AC4 now
  records the A3949 deferral instead of the false “P20i has no EQ” premise, and
  the A3949 section above documents the block at body 32..44.
- **Vacuous test removed.** `manual_p20i_has_no_equalizer` is gone: it returned
  early when its connect failed (so it could pass without testing anything) and
  its premise was wrong. The deferral is covered by the running unit tests named
  in the A3949 section.
- **Code quality.** The protocol constant is `Command::EQUALIZER` (the same
  `03 87` is outbound write and inbound ack); the unused
  `EqualizerCapability::custom_preset_id` field was removed while
  `CUSTOM_EQUALIZER_PRESET_ID` still drives `is_custom`.
- **Re-verification.** `manual_equalizer_preset_switch` was re-run on the real
  A3062 after the fixes: it switched Acoustic/Bass Reducer/Classical, re-polled
  the measured curves and restored the preset found on connect (Soundcore
  Signature, id 0) — the output above. The device was left as found; the gates
  below are the final counts after the fixes.
