# 03 — Saved profiles acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify saved equalizer profiles on real A3062 across restarts.

## Requirements

1. Build/run; connect A3062; create a custom curve; save profile “Test curve”.
2. Verify it appears with curve, bands=8, Applied tag.
3. Change the curve; apply profile; verify one `03 87`, re-poll, Applied tag.
4. Delete profile after confirmation; verify gone and JSON updated.
5. Restart the app; verify saved profiles reload from `%APPDATA%\OpenSCL4\equalizer-profiles.json`.
6. Verify duplicate-name replace prompt, empty-name rejection and invalid bands rejection (unit/UI).
7. A3949 deferred: no page/no profile writes.
8. Screenshot `desktop/equalizer-profiles.png`; link README; document import/export out of scope.
9. Gates: desktop/core test/fmt/clippy/build.
10. Record OS/firmware, profile names/curves, restart result; no MAC/serial.

## Acceptance

- [x] Save/apply/delete and restart persistence verified on real A3062.
- [x] Screenshot stored/linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. The running app was
driven through the Windows accessibility tree (the same UI Automation approach as
the previous Windows acceptances), with the ignored `manual_saved_profiles`
hardware test as a second, scripted pass. No Bluetooth pairing was changed and
the Space One Pro was left on the state found on connect (built-in **Soundcore
Signature**, id 0, flat bands). No MAC address or serial number is recorded here
or in any output below; the hardware tests print every address as
`<address masked>`.

### Environment

- **OS:** Windows 11 Pro, 26H2, build `10.0.26300.9974`, x64; primary display
  2560×1440.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the paired devices.
- **Device under test:** soundcore **Space One Pro** — catalog **A3062**,
  ten-step single battery, firmware **04.39** re-read from the `01 01` state on
  this run by `manual_paired_connect` (`raw 5/10` shown as `50 %`).
- **Other paired devices:** soundcore P20i (A3949) and Liberty 4 Pro (A3954)
  were off/unreachable for this run — both RFCOMM opens fail with the readable
  `Timed out` message after the radio page timeout. No pairing change was made.
- **Build:** `cd desktop; cargo build` → `Finished dev profile … in 2m 29s`
  (only the pre-existing vendored-Slint layout warnings), then
  `desktop/target/debug/soundcore-desktop.exe` was started for the in-app run.
- **User's saved-profile document:** `%APPDATA%\OpenSCL4\equalizer-profiles.json`
  did not exist before this acceptance (first run). It was created by the run,
  inspected at every step, and removed again afterwards, so no saved profile was
  left behind; the hardware test's `RestoreDocument` guard did the same around
  its own run.

### Real in-app acceptance (A3062, UI Automation)

The app was launched and driven through the Windows accessibility tree:
**Add Device** → model search `A3062` → the **Space One Pro** card → the paired
row **soundcore Space One Pro** (marked “looks like the chosen model”). The
connection succeeded over RFCOMM and the sidebar gained **SOUND → Sound mode /
Equalizer**. Opening **Equalizer** gave the editable page:

- 8 band value fields (`Edit`) labelled `100 Hz, dB` … `12.8 kHz, dB`, all `0`
  on connect, and 8 sliders with the same frequencies and values;
- the **Reset** and **Save profile** buttons, the “Values in decibels, from
  −12.0 to +13.4 dB” range note and the “Drag the points on the chart…” hint;
- all **22 catalog preset chips**, the “Equalizer bands: 8” heading and the name
  tag **Soundcore Signature**;
- the vendored **My profiles** panel with its empty note “Save the current sound
  to come back to it later.”.

**Custom curve.** The 100 Hz field was clicked, select-all and `4.0` typed, then
Enter: the device round trip replaced the page state with the custom curve
`[4, 0, 0, 0, 0, 0, 0, 0]` and the tag **Your own settings**. The 800 Hz field
was then set to `−4.0`, giving the re-polled curve
`[4, 0, 0, −4, 0, 0, 0, 0]`.

**Save “Test curve”.** Clicking **Save profile** opened the vendored dialog
(title “Save profile”, one name `Edit`, Cancel/Save). `Test curve` was entered
and **Save** clicked: the dialog closed and the My profiles panel showed one
row — the name **Test curve**, its curve thumbnail, **Bands: 8** and the
**Applied** tag — with a “More for “Test curve”” menu button and no empty note.
The document appeared at
`%APPDATA%\OpenSCL4\equalizer-profiles.json`:

```json
[{"name":"Test curve","bands":[40,0,0,-40,0,0,0,0]}]
```

The write is atomic (`<document>.tmp` then rename); no `.tmp` sibling remained.

**Change, then apply.** Typing `2.0` into the 200 Hz field wrote and re-polled
`[4, 2, 0, −4, 0, 0, 0, 0]`; the **Applied** tag cleared (now 0 tags). Clicking
the profile row applied it: the re-polled fields and sliders returned to the
profile's `[4, 0, 0, −4, 0, 0, 0, 0]` and the row showed **Applied** again.
Applying goes through the ticket 02 path — one custom `03 87` frame
(`0xFEFE` + the profile's eight bands) followed by one `01 01` state read; the
exact frame write is asserted by the ticket 02 `FakeTransport`/frame tests and
by the scripted hardware run below, since the in-app pass only observes the
re-polled page.

**Empty and duplicate names (in the live dialog).** With the dialog reopened and
the name set to three spaces, **Save** left the dialog open with the readable
message **“Enter a profile name.”** and added nothing. Setting the saved name
`Test curve` and clicking **Save** showed **“A profile named “Test curve”
already exists. Replace its saved values?”** and changed the primary button to
**Replace profile**, still with one row. Clicking **Replace profile** (after
changing 200 Hz to `2.0` again) closed the dialog, kept one profile and stored
the new bands:

```json
[{"name":"Test curve","bands":[40,20,0,-40,0,0,0,0]}]
```

The row showed **Applied** again because the replaced bands are the current
bands.

**Restart persistence.** The app process was closed and
`desktop/target/debug/soundcore-desktop.exe` started again (a new process). The
controller loads the document at startup, and after reconnecting the A3062 and
opening **Equalizer** the My profiles panel again listed **Test curve** with its
curve thumbnail, **Bands: 8** and the **Applied** tag, while the page read the
device's `[4, 2, 0, −4, 0, 0, 0, 0]` — i.e. the profile came back from the JSON
file, not from the page. The document was unchanged by the restart.

**Delete after confirmation.** The profile row's menu → **Delete** opened the
vendored confirmation dialog (“Delete equalizer profile?” / “Are you sure you
want to delete “Test curve”?“). Clicking its **Delete** closed the dialog,
removed the row and restored the empty-note state; the document became `[]`
(2 bytes) with no `.tmp` sibling. The out-of-scope import/export controls then
showed no profile checkboxes.

**Restored.** Clicking the **Soundcore Signature** chip wrote one preset
`03 87` frame and re-polled the state found on connect — name **Soundcore
Signature**, bands `[0, 0, 0, 0, 0, 0, 0, 0]` — then the app was closed.

### Out-of-scope controls stay inert

With a saved profile present, the vendored **Import and export** panel becomes
visible on the page (it is gated by the same `has-saved-profiles` field as the
profile panel). In this slice it is inert, as ticket 02 intended: every control
was clicked on the live window and nothing happened. **Open file**, **Paste**,
**Select all**, **Save to file** and **Copy** were each invoked through UI
Automation: the app kept one process and one window, no file dialog or transfer
screen opened, no transfer note appeared, no toast showed, the profile list and
**Applied** tag stayed unchanged, the export selection count stayed `0` and the
legacy migration banner stayed hidden. No file or clipboard API is wired in
`controller.rs`; only `%APPDATA%\OpenSCL4\equalizer-profiles.json` is written by
the shell.

### Rejections covered by tests (invalid bands and the dialog states)

- `core/crates/profiles` (11 tests): `rejects_empty_and_whitespace_names`,
  `rejects_duplicate_names_and_replace_updates_the_existing_profile`,
  `accepts_a_name_of_the_maximum_length_and_rejects_longer_ones`,
  `rejects_bands_outside_the_measured_range` (a band outside −1200…+1340 dB×10
  is a typed `BandOutOfRange` error with its index/value, both range ends are
  accepted), and `reports_typed_errors_for_malformed_and_invalid_documents`
  (7-band profile, out-of-range band, unnamed and duplicate entries load as
  typed errors and the file is left in place).
- `desktop/src/controller.rs` (13 saved-profile tests):
  `an_empty_profile_name_is_rejected_with_problem_one`,
  `a_duplicate_profile_name_asks_before_replacing`,
  `applying_a_saved_profile_queues_its_curve_once`,
  `an_unknown_profile_or_a_stopped_worker_queues_nothing`,
  `deleting_a_saved_profile_persists_the_smaller_set`,
  `saved_profile_rows_carry_the_name_curve_band_count_and_applied_flag`,
  `a_profile_curve_is_drawn_in_the_thumbnail_box`,
  `a_saved_profile_set_round_trips_through_the_document`,
  `a_missing_or_malformed_profile_document_loads_empty`,
  `saving_replaces_the_previous_document_without_leaving_a_temporary`,
  `a_save_that_cannot_write_the_document_reports_an_error`,
  `the_profile_document_lives_in_the_app_data_directory` and
  `only_the_editable_a3062_page_offers_saved_profiles`.
- A failed write/read shows the readable banner: with the worker stopped an
  apply produced `Equalizer bands (connection lost)` and wrote nothing; the
  banner's Retry reconnected (printed by the scripted run below). The mapping
  tests are ticket 02's `a_failed_curve_change_maps_to_a_readable_label` and
  `a_stopped_worker_shows_the_readable_curve_error`.

### A3949 (P20i) deferred

- Unit/UI evidence: `only_the_editable_a3062_page_offers_saved_profiles` checks
  that `EqualizerDisplay::saved_profiles()` is true for the A3062 and false for
  the A3949, and the controller gates the Save/apply/delete actions and the
  `has-saved-profiles` field on the editable A3062 page; the A3949 state
  capture's equalizer block is still unwritable, so no page and no `03 87` is
  produced for it.
- Live check: a P20i connect attempt was made for this run (Add Device → A3949 →
  the **P20i / P25i / R50i** card → paired row **soundcore P20i**). The P20i was
  off/unreachable, so the attempt ended in the readable
  **“Could not connect / The device did not answer in time. Try again.”** state
  instead of a page; the visible tabs were only My devices and Settings (no
  Equalizer), and the saved-profile document was not touched (same size and
  mtime before/after). Pairing was not changed.
- No A3949 profile write exists to verify; the deferral is unchanged by this
  ticket.

### Scripted hardware run

`cd desktop; cargo test -p soundcore-desktop manual_saved_profiles -- --ignored --nocapture`
built the real `AppWindow`, wired the controller's actions and drove the same
`eq-type`/`eq-save`/`eq-apply`/`eq-delete` callbacks on the real device through
the startup/load path. All Bluetooth addresses are masked:

```text
OpenSCL4: no saved equalizer profiles yet: C:\Users\Dmitr\AppData\Roaming\OpenSCL4\equalizer-profiles.json
== A3062 Space One Pro ==
  candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
  candidate soundcore Space One Pro suggested=true (<address masked>)
  candidate soundcore P20i suggested=false (<address masked>)
  connected via <address masked>
  found: preset "0" name "Soundcore Signature" bands [0, 0, 0, 0, 0, 0, 0, 0]
  custom curve [40, 0, 0, -40, 0, 0, 0, 0]
  saved "Test curve": rows 1 bands 8 applied true; document has 1 profile(s)
  restart: Controller::new loaded 1 profile(s), first "Test curve"
  empty name -> problem 1; duplicate name -> problem 2
  replaced bands [40, 20, 0, -40, 0, 0, 0, 0]; Applied is back, one profile remains
  applied: re-polled bands [40, 20, 0, -40, 0, 0, 0, 0] preset "65278" applied true
  after a stopped worker: error-label "Equalizer bands (connection lost)"
  deleted: rows 0; document has 0 profile(s)
  restored preset "0" bands [0, 0, 0, 0, 0, 0, 0, 0]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 137 filtered out; finished in 3.27s
```

The run started from the document removed by its `RestoreDocument` guard, so it
also re-verified the missing-document path loaded as no profiles; the guard put
the document back afterwards.

### Firmware and state read

`cargo test -p soundcore-desktop manual_paired_connect -- --ignored --nocapture`
re-read the A3062 state on this tree: `connected: label 3=50%`, `firmware:
04.39`; the A3949 and A3954 opens timed out (`TimedOut`, readable message) with
addresses masked.

### Screenshot and docs

- `desktop/equalizer-profiles.png` (1700×1300): the app window captured with
  `PrintWindow(PW_RENDERFULLCONTENT)` from a DPI-aware process, so only the app
  window is in the image. It shows the title “Equalizer · soundcore Space One
  Pro — OpenSCL4”, the connected device card, the preset chips, the chart with
  the re-polled `+4.0 / +2.0 / −4.0` curve, the **Your own settings** tag, the
  eight band fields (`4 2 0 −4 0 0 0 0`), the Reset and **Save profile**
  buttons, and the **My profiles** panel with the **Test curve** row (curve
  thumbnail, “Bands: 8”, **Applied**, more menu) after the app restart. The
  vendored Import and export panel at the bottom is visible but inert. No
  Bluetooth address or serial number is in the image.
- `desktop/README.md` now documents saved profiles (Save dialog, row with curve
  and Applied, apply writes one `03 87` and re-polls, delete confirmation,
  persistence path/schema/atomic write, missing/malformed file tolerated, empty
  and duplicate names, out-of-range bands rejected), states that
  import/export/legacy migration/HearID stay out of scope with the vendored
  Import and export panel inert, notes the A3949 has no page and no profile
  writes, and links the new capture.
- The root `README.md` status table gained the A3062 saved-profiles row and the
  desktop row now mentions editable custom bands and saved profiles; the
  layout tree lists `core/crates/profiles` and `.scratch/windows-eq-profiles/`.

### Recorded deviations and limitations

- **Import and export panel visible but inert.** The vendored page shows its
  “Import and export” panel whenever saved profiles are available; this slice
  keeps it non-functional rather than editing the vendored UI (ticket 02 made no
  `.slint` changes). The buttons are present, do nothing, and no transfer note
  or file/clipboard code path exists.
- **A3949 profile behaviour is test/state evidence, not a live equalizer run.**
  The P20i was off/unreachable on this run; no A3949 page or profile write could
  be attempted. This matches the deferral record of the earlier tickets.
- **Page resized for the capture.** The window was set to 1700×1300 for the
  screenshot so the wide two-column layout shows the My profiles panel next to
  the bands; the app's normal preferred size is 1280×820 (logical), where the
  same panel stacks below the bands.
- No product code, dependency or vendored `.slint` file changed by this ticket:
  the diff is `desktop/equalizer-profiles.png`, `desktop/README.md`, `README.md`
  and this ticket.

### Gates

Run from the final tree:

```
cd desktop
cargo test                                  # 130 passed; 0 failed; 8 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only the pre-existing vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 88 passed across 13 test binaries plus doc-tests; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

The ignored hardware tests (`manual_saved_profiles`, `manual_paired_connect`)
were run separately on the real A3062 and are not part of the automated suite;
the automated desktop suite contributes the ticket 01/02 saved-profile tests
(130 passed, same count as ticket 02) and the core suite the profiles-crate
validation tests (88 passed, same count as ticket 01).

### Merge

The acceptance branch is based on the integration tip `3b49858` (merge of
`ticket/pf-02-controller` into `integration/windows-eq-profiles`).
`git merge integration/windows-eq-profiles` after the acceptance commit
reported “Already up to date”, so no merge commit was needed and the gates above
were run on that same tree.
