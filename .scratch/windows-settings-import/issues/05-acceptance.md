# 05 — Acceptance

**Type:** task
**Status:** done
**Blocked by:** 02, 04

## Goal

Verify settings and profile import on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062.
2. Visit Controls, Comfort, Power, Connections, Effects, About; verify populated values.
3. Change each setting type: toggle (e.g. side tone/low battery), select (auto-off/double press), range (limit dB); verify one write and re-polled state; restore original.
4. Connections: toggle dual connections; verify “open Bluetooth settings” opens Windows settings page.
5. Import: paste a valid profile JSON and import; verify preview/new, confirm, profile appears/applies; blocked import (7 bands/out-of-range) writes nothing; no legacy banner.
6. A3949 deferred: no pages beyond deferred behavior.
7. Capture `desktop/settings-a3062.png` and `desktop/import-profiles.png`; link README; no MAC/serial.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware, settings values, import JSON, results; restore device settings and user profile document.

## Acceptance

- [x] Settings writes/re-poll verified on real A3062.
- [x] Import valid/blocked verified; legacy absent.
- [x] Screenshots stored/linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. The running app was
driven through the Windows accessibility tree (UI Automation, the same approach
as the previous Windows acceptances), with the ignored settings and import
hardware tests as a second, scripted pass. No Bluetooth pairing was changed. The
Space One Pro was left with every setting found on connect (side tone on, limit
high volume off / 90 dB / real time, auto power off 60 minutes, low battery and
voice prompts on, dual connections on, LDAC and Dolby off), and the user's
`%APPDATA%\OpenSCL4\equalizer-profiles.json` document did not exist before the
run and was removed again afterwards. No MAC address or serial number is
recorded here or in any output or image: the hardware tests print every address
as `<address masked>`, the About page has no serial row, and the app never lays a
MAC or serial out on a settings page.

### Environment

- **OS:** Windows 11 Pro, 26H2, build `10.0.26300.9974`, x64; primary display
  2560×1440.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the paired devices.
- **Device under test:** soundcore **Space One Pro** — catalog **A3062**,
  ten-step single battery, firmware **04.39** and battery **40 %** re-read from
  the `01 01` state on this run.
- **Other paired devices:** soundcore P20i (A3949) and Liberty 4 Pro (A3954)
  were off/unreachable — both RFCOMM opens end in the readable “did not answer
  in time” failure. No pairing change was made.
- **Build:** `cd desktop; cargo build` → `Finished dev profile` (~2m37s; only
  the pre-existing vendored-Slint layout warnings), then
  `desktop/target/debug/soundcore-desktop.exe` was started for the in-app run.
- **User's saved-profile document:** `%APPDATA%\OpenSCL4\equalizer-profiles.json`
  did not exist before this acceptance. The live run created it while importing
  and left `[]` after deleting the imported profile; it was then removed, and
  the hardware import test's `RestoreDocument` guard backed up/restored the exact
  bytes around its own run. The directory's other entries were untouched.

### Real in-app acceptance (A3062, UI Automation)

The app was launched and driven through the Windows accessibility tree: **Add
Device** → search `A3062` → the **Space One Pro** card → the paired row
**soundcore Space One Pro** (marked “looks like the chosen model”). The
connection succeeded over RFCOMM; the window title became
“Overview · soundcore Space One Pro — OpenSCL4” and the sidebar gained the
**SOUND** group (Sound mode, Equalizer, Sound and effects) and the **DEVICE**
group (Controls, Comfort, Power and prompts, Connections, About the device).

**Pages and values.** Each page was opened from the sidebar and read from the
accessibility tree; the values are the device's own from the `01 01` state.

| page | what the page showed |
|---|---|
| Controls | **Single press** = “Ambient sound mode” (info row, no write); **Double press** = **None**; Switching modes with **Noise canceling** and **Transparency** on, **Normal** off; no left/right earbud selector |
| Comfort | **Side tone** on; **Limit high volume** off; **dB limit** 90 dB; **Refresh rate** **Real time** |
| Power and prompts | **Auto power off** **60 minutes**; **Low battery prompt** on; **Voice prompt** on |
| Connections | **Dual connections** on; the **Windows** aside with **Bluetooth and devices** and **Sound** links |
| Sound and effects | **LDAC** off; **Dolby audio** off; the Windows LDAC note |
| About the device | **Is charging** “Not charging”; **Battery level** “40%”; **Firmware version** “04.39”; Copy buttons; no serial row |

**One toggle.** On **Comfort**, clicking **Side tone** drew the switch off after
the round trip and no error banner appeared; clicking it again restored it. The
capture taken after the restore is byte-identical (same SHA-256) to the one taken
before the change, so the page was back on the value found on connect.

**One select.** On **Power and prompts**, the **Auto power off** combobox read
“60 minutes”; choosing **90 minutes** made the re-polled value read
“90 minutes”, and choosing **60 minutes** restored it.

**One range.** On **Comfort**, the **dB limit** slider read 90 (range 75..100,
step 5); one Up-arrow commit wrote and re-polled 95 (the value box and the
slider both read 95), and one Down-arrow commit restored 90.

The page reflects the device's answer, not the click: the controller sends one
frame and then re-fills the page from the `01 01` re-poll. The exact
one-frame-then-one-poll behaviour is asserted by the ticket 02 tests
`one_settings_write_is_one_frame_and_one_re_poll`,
`a_toggle_click_names_exactly_the_measured_command`,
`an_option_click_maps_to_the_measured_choice_commands`,
`a_range_release_keeps_only_the_measured_db_steps`,
`a_failed_settings_write_surfaces_and_ends_the_worker` and
`the_system_settings_links_open_the_windows_settings_pages`; the live pass
observes the re-polled state.

**Connections.** The **Dual connections** toggle was switched off (re-polled:
the page drew it off) and then back on. The restore write once lost the RFCOMM
link: the page showed the readable banner **“Could not change “Dual
connections (connection lost)”. The device kept its previous value.”**, with
**Retry**. Retry reconnected; the device then reported the toggle **off** (the
off write had taken, only the confirmation read had failed), and the restore
click wrote it back on and re-polled **on**. No error banner remained and later
writes worked normally. This is the idle-link behaviour noted under Deviations.

The **Open** button of **Bluetooth and devices** was clicked: the Windows
Settings app came to the foreground on its **Bluetooth & devices** page (UIA
read the page header «Bluetooth и устройства», the “discoverable as …” line and
the paired-device list) and the app kept its connection. No pairing was changed;
the Settings window was left as the user had it otherwise. The **Sound** link
shares the same `open-system-settings` mapping (unit tested) but was not opened
in the live run to avoid a second Settings page.

### Profile import in the app

The **Equalizer** page showed the empty **My profiles** note, the wired
**Import and export** panel (**Open file**, **Paste**) and no legacy banner.

**Valid paste.** The Windows clipboard held an OpenSCL4 transfer profile:

```json
[{"name": "Warm", "volumeAdjustments": [3.0, -3.0, 6.0, 0.0, -6.0, 2.0, 0.0, -2.0]}]
```

**Paste** opened the vendored **Import profiles** dialog with one **Warm** row,
its curve thumbnail, the **New** tag, an enabled **Import 1 profile** and no
blocked note (this is `desktop/import-profiles.png`). **Import 1 profile**
closed the dialog and the page showed the **Warm** row in **My profiles**
(“Bands: 8”), the “Imported 1 profile” transfer note and the same toast; the
document appeared as
`[{"name":"Warm","bands":[30,-30,60,0,-60,20,0,-20]}]` with no `.tmp` sibling.
Clicking the row applied it: one custom `03 87` write and a `01 01` re-poll gave
the bands `3, -3, 6, 0, -6, 2, 0, -2`, the name tag **Your own settings** and
the **Applied** tag on the row.

**Blocked paste.** A clipboard file with a 7-band profile and a `13.5 dB` value
opened the dialog with two red rows — tags “7 bands, this device has 8” and
“Values outside this device's range” — the red note “Profiles marked in red can
not be used with this device, so the file can not be imported.” and the Import
button greyed out. Clicking it left the dialog open and the document
byte-identical (same SHA-256; still only **Warm**). Cancel closed it and added
nothing.

**No legacy banner.** The Equalizer page never showed “Old equalizer profiles”
or the migration controls; `eq-legacy` stays empty by design. Export stays out
of scope and its **Save to file** / **Copy** buttons stay inert.

**Restored.** The **Soundcore Signature** chip wrote the flat preset found on
connect (re-polled bands all 0, name **Soundcore Signature**), the row menu's
**Delete** plus the confirmation dialog removed **Warm**, and the document
became `[]`. It was removed afterwards so the pre-run state (no document) came
back. The imported profile list is empty again.

### Scripted hardware run

`cargo test -p soundcore-desktop manual_settings_pages_and_writes --
--ignored --nocapture` connected the real A3062 and read every page, wrote one
toggle, one select and one range, restored them and compared the final settings
field for field with the ones found on connect (addresses masked, the serial
never printed, the temporary profile document scrubbed):

```text
== A3062 Space One Pro ==
  candidate soundcore P20i suggested=false (<address masked>)
  candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
  candidate soundcore Space One Pro suggested=true (<address masked>)
  connected via <address masked>
  device-pages: [Controls, Comfort, Power, Connections, About]
  sound-pages: [SoundModes, Equalizer, Effects]
  == Controls ==
  Presses
    singlePress Info ... text="Ambient sound mode"
    doublePress Select ... index=0
  Cycle
    noiseCancelingModeInCycle Toggle checked=true
    transparencyModeInCycle Toggle checked=true
    normalModeInCycle Toggle checked=false
  == Comfort ==
  Plain
    sideTone Toggle checked=true
  Volume
    limitHighVolume Toggle checked=false
    limitHighVolumeDbLimit Range ... range=75..100 step=5 value=90
    limitHighVolumeRefreshRate Select ... index=0
  == Power ==
  Plain
    autoPowerOff Select ... index=2
    lowBatteryPrompt Toggle checked=true
    voicePrompt Toggle checked=true
  == Connections ==
  Plain
    dualConnections Toggle checked=true
  == Effects ==
  Plain
    ldac Toggle checked=false
    dolbyAudio Toggle checked=false
  == About ==
  Info
    isCharging Info ... text="Not charging"
    batteryLevel Info ... text="40%"
    firmwareVersion Info ... text="04.39"
  About rows: ["isCharging", "batteryLevel", "firmwareVersion"]
  side tone true -> false
  side tone restored to true
  auto power off index 2 -> 3 (AutoPowerOff { enabled: true, duration: NinetyMinutes })
  auto power off restored to AutoPowerOff { enabled: true, duration: SixtyMinutes }
  dB limit 90 -> 95
  dB limit restored to 90
  clipboard: the About copy action works
  final settings: A3062Settings { button_double_press: None, ambient_sound_mode_cycle: AmbientSoundModeCycle { noise_canceling: true, transparency: true, normal: false }, low_battery_prompt: true, dolby_audio: false, ldac: false, dual_connections: true, auto_power_off: AutoPowerOff { enabled: true, duration: SixtyMinutes }, limit_high_volume: LimitHighVolume { enabled: false, db_limit: DbLimit(90), refresh_rate: RealTime }, side_tone: true, voice_prompt: true }
test result: ok. 1 passed; 0 failed; ...
```

`cargo test -p soundcore-desktop manual_import_profiles -- --ignored
--nocapture` repeated the import flow on the device (paste, apply, file input,
blocked file, delete, restore) and restored the user's document:

```text
  found: preset "0" name "Soundcore Signature" bands [0, 0, 0, 0, 0, 0, 0, 0]
  paste: importing true blocked false items ["Warm"] statuses [0]
  imported "Warm": rows 1 document 1 profile(s), toast Imported
  applied: re-polled bands [30, -30, 60, 0, -60, 20, 0, -20]
  file: importing true items ["File curve"]
  imported from file: rows 2 document 2 profile(s)
  blocked paste: importing true blocked true statuses [2, 3]
  blocked import wrote nothing; the dialog stays open with its red note
  deleted both imported profiles; the document is empty again
  restored preset "0" bands [0, 0, 0, 0, 0, 0, 0, 0]
test result: ok. 1 passed; 0 failed; ...
```

`cargo test -p soundcore-desktop manual_paired_connect -- --ignored
--nocapture` re-read the device on this tree: `connected: label 3=40%`,
`firmware: 04.39`; the A3949 and A3954 opens timed out with the readable
message, addresses masked. The scripted runs do not capture the bytes on the
wire (the in-app pass and these runs observe the re-polled state); the exact
single frame per write is the ticket 02 `one_settings_write_is_one_frame_and_one_re_poll`
test on the `FakeTransport`.

### A3949 (P20i) deferred

- Live check: **Add Device** → `A3949` → the **P20i / P25i / R50i** card → the
  paired row **soundcore P20i**. The P20i was off/unreachable, so the attempt
  ended in the readable **“Could not connect / The device did not answer in
  time. Try again.”** state; the visible tabs were only **My devices** and
  **Settings** — no Controls, Comfort, Power, Connections, About, Effects or
  Equalizer page and therefore no settings or equalizer write. Pairing was not
  changed.
- Automated: the A3949 state capture produces no device pages, no sound pages
  and no settings panels (`a_state_without_the_settings_block_has_no_pages_or_panels`,
  `only_the_editable_a3062_page_offers_saved_profiles`), and its equalizer stays
  deferred, so the deferral is unchanged by this ticket.

### Screenshots and docs

- `desktop/settings-a3062.png` (1700×1300): the **Comfort** page from the
  running app on Windows with a connected Space One Pro — title “Comfort ·
  soundcore Space One Pro — OpenSCL4”, the device card aside, and the re-polled
  **Side tone** on, **Limit high volume** off, **dB limit** 90 dB and
  **Refresh rate** Real time. No Bluetooth address or serial number is in the
  image.
- `desktop/import-profiles.png` (1700×1300): the **Import profiles** dialog with
  the pasted OpenSCL4 profile **Warm** previewed as **New** (curve thumbnail,
  **Cancel**, **Import 1 profile**), with the Equalizer page and the Import and
  export panel behind it. No Bluetooth address or serial number is in the image.
- `desktop/README.md` now documents the remaining settings pages and their
  values, the one-frame-write-then-re-poll rule and the readable error banner
  with Retry, the Connections dual toggle and Windows links, the profile import
  (paste/file, preview statuses, all-or-nothing merge, blocked files write
  nothing, no legacy banner, export inert, HearID out of scope), the A3949
  deferral, the idle-link note and both captures.
- The root `README.md` gained the A3062 remaining-settings and profile-import
  status rows, updated the A3949, Windows session and desktop rows, and lists
  `.scratch/windows-settings-import/` in the layout tree.

### Gates

Run from the final tree:

```
cd desktop
cargo test                                  # 165 passed; 0 failed; 10 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only the pre-existing vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 119 passed; 0 failed across 20 targets
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
cargo build                                 # ok
```

The ignored hardware tests (`manual_settings_pages_and_writes`,
`manual_import_profiles`, `manual_paired_connect`) were run separately on the
real A3062 and are not part of the automated suite.

### Recorded deviations and limitations

- **Idle RFCOMM link.** The shell keeps the link open between writes instead of
  polling. Twice during the live run the link dropped while the window sat idle
  between steps; the next write surfaced the readable “connection lost” banner
  (after the socket timeouts, up to about ten seconds) and **Retry** opened a
  fresh connection, after which the write and re-poll succeeded. This is not a
  settings- or import-specific defect (every scripted run connects fresh and
  every write succeeded), but it is a real bench behaviour and is documented in
  `desktop/README.md`.
- **No `.slint` changes.** The two captures were taken after enlarging the app
  window from its 1280×820 preferred size to 1700×1300 so the Comfort panels and
  the import dialog fit at once; no other visual change was made, and the
  vendored UI tree is untouched by this ticket.
- **About copy in-app.** The About **Copy** buttons were not clicked in the
  live window; the wired copy action put its text on the clipboard in
  `manual_settings_pages_and_writes` (“clipboard: the About copy action works”).
- **Sound link.** The **Sound** `ms-settings:sound` link shares the same wiring
  and pure mapping (`the_system_settings_links_open_the_windows_settings_pages`)
  but was not opened in the live run; only the Bluetooth link was exercised
  there.
- **Out of scope, per the spec:** export, legacy migration, HearID, the
  dual-connections remembered-device list, and A3949 settings/equalizer pages.

### Merge

The acceptance branch is based on the integration tip `55ded98` (merge of
`ticket/si-04-import-controller` into `integration/windows-settings-import`).
After the acceptance commit, `git merge
integration/windows-settings-import` reported “Already up to date”, so no merge
commit was needed and the gates above were re-run on that same tree (desktop
165 passed/10 ignored; core 119 passed; fmt, clippy and build clean). The ticket
branch is ready for the lead to merge into the integration branch.

