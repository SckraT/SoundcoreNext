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
6. Verify duplicate-name replace prompt and empty-name rejection (live dialog/UI). Invalid bands are not a dialog case: the page's edits clamp to the measured range and the profile model/load rejects out-of-range bands (unit tests).
7. A3949 deferred: no page/no profile writes.
8. Screenshot `desktop/equalizer-profiles.png`; link README; document import/export out of scope and the vendored Import and export panel visible but inert (buttons do nothing).
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
Applying goes through the ticket 02 path — one custom `03 87` frame (`0xFEFE` +
the profile's eight bands) followed by one `01 01` state read. The exact
single-frame write is asserted by the ticket 02 `FakeTransport` worker test
`the_worker_writes_one_custom_frame_and_reports_the_custom_state` (and the
apply-queues-once test), not by either live run: the in-app pass and the scripted
hardware run below only observe the re-polled bands, preset and Applied tag.

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

### Out-of-scope controls stay visible but inert

The vendored **Import and export** panel is rendered by the page whenever
`has-saved-profiles` is set, and the controller sets that field for the editable
A3062 page with or without saved profiles. The panel is therefore visible, not
hidden; in this slice it is inert, as ticket 02 intended: every control was
clicked on the live window and nothing happened. **Open file**, **Paste**,
**Select all**, **Save to file** and **Copy** were each invoked through UI
Automation: the app kept one process and one window, no file dialog or transfer
screen opened, no transfer note appeared, no toast showed, the profile list and
**Applied** tag stayed unchanged, the export selection count stayed `0` and the
legacy migration banner stayed hidden. No file or clipboard API is wired in
`controller.rs`; only `%APPDATA%\OpenSCL4\equalizer-profiles.json` is written by
the shell. The panel cannot be hidden without editing the vendored `.slint`
tree, which this slice deliberately does not do.

### Rejections covered by tests (dialog states, clamping and invalid bands)

- Invalid bands are not a dialog rejection in the live UI: the typed field and
  the slider/edit path clamp every value to the measured −12.0…+13.4 dB range
  (`a_band_value_clamps_to_the_measured_range`,
  `a_commit_clamps_the_band_and_writes_the_whole_curve_once`), so the page
  cannot produce an out-of-range band. Validation/rejection of out-of-range
  bands lives in the profile model and on document load:
- `core/crates/profiles` (11 tests): `rejects_empty_and_whitespace_names`,
  `rejects_duplicate_names_and_replace_updates_the_existing_profile`,
  `accepts_a_name_of_the_maximum_length_and_rejects_longer_ones`,
  `rejects_bands_outside_the_measured_range` (a band outside −120..+134 dB×10
  is a typed `BandOutOfRange` error with its index/value, both range ends are
  accepted), and `reports_typed_errors_for_malformed_and_invalid_documents`
  (7-band profile, out-of-range band, unnamed and duplicate entries load as
  typed errors and the file is left in place).
- `desktop/src/controller.rs` (19 saved-profile tests):
  `an_empty_profile_name_is_rejected_with_problem_one`,
  `a_duplicate_profile_name_asks_before_replacing`,
  `the_save_problem_values_match_the_vendored_dialog`,
  `the_dialog_shows_the_name_problems_on_the_page`,
  `applying_a_saved_profile_queues_its_curve_once`,
  `an_unknown_profile_or_a_stopped_worker_queues_nothing`,
  `deleting_a_saved_profile_persists_the_smaller_set`,
  `a_delete_that_cannot_write_the_document_keeps_the_profile`,
  `saved_profile_rows_carry_the_name_curve_band_count_and_applied_flag`,
  `a_profile_curve_is_drawn_in_the_thumbnail_box`,
  `a_saved_profile_set_round_trips_through_the_document`,
  `a_missing_or_malformed_profile_document_loads_empty`,
  `saving_replaces_the_previous_document_without_leaving_a_temporary`,
  `a_save_that_cannot_write_the_document_reports_an_error`,
  `a_save_that_cannot_write_the_document_rolls_back_and_reports`,
  `a_missing_app_data_directory_is_not_reported_as_saved`,
  `a_written_profile_closes_the_dialog_and_clears_a_stale_banner`,
  `the_profile_document_lives_in_the_app_data_directory` and
  `only_the_editable_a3062_page_offers_saved_profiles`.
- The live dialog covered the two name cases: a whitespace name showed
  “Enter a profile name.” and a saved name showed the replace question. No
  out-of-range band can be entered, so that case has no dialog message.

### File persistence failure behavior

- A failed **save** and a failed **delete** of the profile document are visible
  UX, not silent: the page error banner shows the label
  **“Could not save equalizer profiles”** (the vendored banner wraps it as
  “Could not change “…”.”; its second sentence is the vendored device-change
  wording and not accurate for a file failure, but it is the only visible error
  slot without a `.slint` edit) and the failure is also logged to stderr.
- The in-memory change is rolled back, so the profile list never shows a profile
  the document does not have: a failed save leaves no row, and a failed delete
  keeps its row. The dialog closes on a save failure too, because the vendored
  dialog only has name-problem slots and a closed dialog is the only way the
  page banner can be seen; the saved set and document are unchanged.
- With no `%APPDATA%` directory there is no document at all, so a save is
  reported the same way (not persisted, no row) instead of pretending the
  profile is durable.
- A failed **read** at startup is still stderr-only, because the page does not
  exist yet: the unreadable/missing document loads as no profiles and is left in
  place until the next successful save.
- Automated evidence: `a_save_that_cannot_write_the_document_rolls_back_and_reports`,
  `a_delete_that_cannot_write_the_document_keeps_the_profile`,
  `a_missing_app_data_directory_is_not_reported_as_saved`,
  `a_written_profile_closes_the_dialog_and_clears_a_stale_banner`,
  `a_save_that_cannot_write_the_document_reports_an_error`, and the scripted run
  below (blocked temporary document path; the user's real document is not
  touched).

### Scripted hardware run

`cd desktop; cargo test -p soundcore-desktop manual_saved_profiles -- --ignored --nocapture`
built the real `AppWindow`, wired the controller's actions and drove the same
type/save/apply/replace/delete callbacks on the real device through the
startup/load path. The run asserts the re-polled bands, preset and Applied flag
plus the document states; it does not capture or inspect the bytes on the wire,
so the exact single-`03 87` frame evidence is the ticket 02 `FakeTransport`
worker test named above. All Bluetooth addresses are masked and the environment's
local app-data path is shown as `%APPDATA%` (the local username is scrubbed):

```text
OpenSCL4: no saved equalizer profiles yet: %APPDATA%\OpenSCL4\equalizer-profiles.json
== A3062 Space One Pro ==
  candidate soundcore P20i suggested=false (<address masked>)
  candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
  candidate soundcore Space One Pro suggested=true (<address masked>)
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
  save to a blocked document: error-label "Could not save equalizer profiles"
  restored preset "0" bands [0, 0, 0, 0, 0, 0, 0, 0]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 143 filtered out; finished in 3.14s
```

The run started from the document removed by its `RestoreDocument` guard, so it
also re-verified the missing-document path loaded as no profiles; the guard put
the document back afterwards.

### A3949 (P20i) deferred

- Unit/UI evidence: `only_the_editable_a3062_page_offers_saved_profiles` checks
  that `EqualizerDisplay::editable` is true for the A3062 and false for the
  A3949, and the controller fills `has-saved-profiles` from that field and gates
  the Save/apply/delete actions on the editable A3062 page; the A3949 state
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
  persistence path/schema/atomic write, a failed write reported in the page
  banner and rolled back, missing/malformed file tolerated, empty and duplicate
  names, out-of-range bands clamped by the page edits and rejected by the
  model/load), states that import/export/legacy migration/HearID stay out of
  scope with the vendored Import and export panel visible but inert, notes the
  A3949 has no page and no profile writes, and links the new capture.
- The root `README.md` status table gained the A3062 saved-profiles row and the
  desktop row now mentions editable custom bands and saved profiles; the
  layout tree lists `core/crates/profiles` and `.scratch/windows-eq-profiles/`.

### Recorded deviations and limitations

- **Import and export panel visible but inert.** The vendored page renders its
  “Import and export” panel whenever `has-saved-profiles` is set, which the
  controller sets for the editable A3062 page with or without saved profiles; the
  panel is visible, not hidden, and this slice keeps it non-functional rather
  than editing the vendored UI (ticket 02 made no `.slint` changes). The buttons
  are present, do nothing, and no transfer note or file/clipboard code path
  exists.
- **A3949 profile behaviour is test/state evidence, not a live equalizer run.**
  The P20i was off/unreachable on this run; no A3949 page or profile write could
  be attempted. This matches the deferral record of the earlier tickets.
- **Page resized for the capture.** The window was set to 1700×1300 for the
  screenshot so the wide two-column layout shows the My profiles panel next to
  the bands; the app's normal preferred size is 1280×820 (logical), where the
  same panel stacks below the bands.
- No vendored `.slint` file changed by this ticket. The acceptance commit's diff
  is `desktop/equalizer-profiles.png`, `desktop/README.md`, `README.md` and this
  ticket; the later review fixes also change `desktop/src/controller.rs` (the
  save/delete failure path and small test/quality cleanups), this ticket,
  `spec.md`, ticket 02 and `desktop/README.md` (see Review fixes below).

### Gates

Run from the final tree (after the review fixes):

```
cd desktop
cargo test                                  # 136 passed; 0 failed; 8 ignored
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
the automated desktop suite contributes 136 tests (the review fixes added six
save/delete failure-path tests to ticket 02's saved-profile set) and the core
suite the profiles-crate validation tests (88, unchanged by the review fixes).

### Review fixes (2026-11-21)

A code review of the slice raised these findings; this ticket and the code were
corrected on branch `ticket/pf-04-review-fixes`:

1. **PII/local path.** The scripted-run transcript's first line showed the local
   username inside `C:\Users\<user>\AppData\Roaming\…`; it is now the app-data
   path as `%APPDATA%\OpenSCL4\equalizer-profiles.json`, and all changed docs
   were scanned for local usernames/paths (none remain).
2. **Hardware overclaim.** The exact single `03 87` frame is asserted by the
   ticket 02 `FakeTransport` worker test, not by the scripted hardware run; the
   wording now says the live runs only observe the re-polled bands/preset/
   Applied state.
3. **Silent persistence failures.** A failed save no longer closes the dialog as
   a success and no longer lists an unserialized profile: save/delete failures
   are rolled back, show the readable page banner label
   “Could not save equalizer profiles”, keep the stderr log, and are covered by
   the six new tests plus the blocked-path line in the scripted hardware run.
4. **Spec honesty (AC6/AC9).** Invalid bands are rejected by the model/load and
   the page edits clamp, not a dialog rejection; the failure behavior of a
   profile write and of a startup read is documented in `spec.md`, ticket 02 and
   this ticket.
5. **Import/export panel.** Documented as visible but inert (rendered by the
   vendored page when `has-saved-profiles` is set, buttons do nothing), not
   hidden; no `.slint` edit.
6. **Small cleanups.** The duplicated “no app-data directory” stderr literal is
   one helper; `EqualizerDisplay::saved_profiles()` (a pass-through to
   `editable`) is gone and the two call sites use `editable`; the dialog's bare
   `eq-save-problem` integers 1/2 are named by a `SaveProblem` enum at the
   controller boundary. No broad controller/test refactor.

The hardware smoke was re-run after the fixes
(`cargo test -p soundcore-desktop manual_saved_profiles -- --ignored
--nocapture`, A3062): the saved-profile flow and the new blocked-document
failure check passed, and the user's profile document was backed up/restored by
the test's `RestoreDocument` guard.

### Merge

The acceptance branch is based on the integration tip `3b49858` (merge of
`ticket/pf-02-controller` into `integration/windows-eq-profiles`).
`git merge integration/windows-eq-profiles` after the acceptance commit
reported “Already up to date”, so no merge commit was needed and the gates above
were run on that same tree. The review-fix branch was based on the later
integration tip `de21001` (merge of `ticket/pf-03-acceptance`) and its merge of
`integration/windows-eq-profiles` was “Already up to date” as well; the fix
commit and the gates above are on that tree.
