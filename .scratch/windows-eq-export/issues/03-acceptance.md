# 03 — Export acceptance

**Type:** task
**Status:** done
**Blocked by:** 02

## Goal

Verify profile export on real A3062 and roundtrip.

## Requirements

1. Build/run; connect A3062; create/save at least two profiles.
2. Select one and all; verify count and row checkboxes.
3. Export to file (save dialog) and via Copy; capture `desktop/export-profiles.png`.
4. Re-import exported JSON; verify preview statuses/bands match and profiles apply.
5. Cancel save dialog does nothing; failure note path unit-tested.
6. A3949 deferred: no export.
7. Update READMEs: export implemented, legacy absent, out-of-scope notes; link screenshot.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware, profile names/bands, file/clipboard result; restore user document/device; no MAC/serial.

## Acceptance

- [x] Real export file/copy and re-import roundtrip verified.
- [x] Screenshot linked; no identifiers.
- [x] Gates pass; ticket done.

## Comments

Accepted on the machine with the paired devices, 2026-11-21. The running app was
driven through the Windows accessibility tree (UI Automation, the same approach
as the previous Windows acceptances); the export actions were invoked through
their UIA `Invoke` patterns and the two native Windows file dialogs (save and
open) were driven with their `File name` boxes and buttons. No Bluetooth pairing
was changed. The Space One Pro was left on the **Soundcore Signature** preset and
flat bands found on connect, and the user's
`%APPDATA%\OpenSCL4\equalizer-profiles.json` document did not exist before the
run and was removed again afterwards. No MAC address or serial number is
recorded here or in any output or image: the connection log formats every
address as `<address masked>`, the About page has no serial row, and the
screenshot shows only the device's user-facing name.

### Environment

- **OS:** Windows 11 Pro, 26H2, build `10.0.26300.9974`, x64; primary display
  2560×1440 at 125 % scaling.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the paired devices.
- **Device under test:** soundcore **Space One Pro** — catalog **A3062**,
  firmware **04.39**, battery **40 %**, not charging (re-read from the `01 01`
  state on this run); the connection found preset `0` **Soundcore Signature**
  with flat bands `[0, 0, 0, 0, 0, 0, 0, 0]`.
- **Other paired devices:** soundcore P20i (A3949) was off/unreachable during
  the main run — its first RFCOMM open ended in the readable “did not answer in
  time” failure. It came on later in the acceptance, and the A3949 deferral
  check below then ran live on it. No pairing change was made.
- **Build:** `cd desktop; cargo build` → `Finished dev profile` (~2m15s; only
  the pre-existing vendored-Slint layout warnings), then
  `desktop/target/debug/soundcore-desktop.exe` was started for the in-app run.
- **User's saved-profile document:** `%APPDATA%\OpenSCL4\equalizer-profiles.json`
  did not exist before this acceptance (so the backup is “absent”). The live run
  created it while saving/importing and left it as `[]` after the cleanup; it
  was removed afterwards, so the pre-run absence came back. The app also
  rewrote `openscl4-config.toml` (`lastDevice`); that file's pre-run contents
  were captured first and restored byte for byte after the run. No other
  `%APPDATA%\OpenSCL4` entry was touched.

### Profiles

The app was launched and driven through the accessibility tree: **Add Device** →
search `A3062` → the **Space One Pro** card → the paired row **soundcore Space
One Pro** (marked “looks like the chosen model”). The connection succeeded over
RFCOMM and the window title became
“Equalizer · soundcore Space One Pro — OpenSCL4”.

Two profiles with distinct curves were saved through the vendored **Save
profile** dialog after typing the bands into the page's band fields (each commit
writes one custom `03 87` frame and re-reads `01 01`):

| profile | bands (dB×10) | bands (dB) |
|---|---|---|
| **Exp one** | `[40, 0, 0, -40, 0, 0, 0, 0]` | `+4.0, 0.0, 0.0, −4.0, 0.0, 0.0, 0.0, 0.0` |
| **Exp two** | `[-20, 0, 0, -40, 0, 0, 0, 30]` | `−2.0, 0.0, 0.0, −4.0, 0.0, 0.0, 0.0, +3.0` |

The document on disk held exactly
`[{"name":"Exp one","bands":[40,0,0,-40,0,0,0,0]},{"name":"Exp two","bands":[-20,0,0,-40,0,0,0,30]}]`
and the **My profiles** panel listed both rows with “Bands: 8”.

### Selection, count and the export panel

The vendored **Import and export** panel showed the **Export** block with one
checkbox per saved profile plus **Select all**, and **Save to file** / **Copy**.
The UIA provider of the Slint window exposes the check rows with an `Invoke`
action only (no `TogglePattern`), so the checkboxes were clicked through that
action and their state was verified from the window capture; `eq-export-count`
itself is not exposed as a UIA property, so the count was verified through its
two bindings in the vendored page (`Select all` is checked exactly when the
count equals the number of saved profiles, and the buttons are enabled exactly
when it is above zero) and through the exported payloads.

- **Nothing selected:** **Copy** left a sentinel clipboard value untouched and
  opened no note/toast; **Save to file** opened no dialog (the process kept only
  its main window) — no empty transfer file has a path.
- **One profile:** invoking the **Exp one** checkbox left only it checked (the
  **Select all** box stayed unchecked; the buttons became enabled). **Copy** put
  `[{"name":"Exp one","volumeAdjustments":[4.0,0.0,0.0,-4.0,0.0,0.0,0.0,0.0]}]`
  on the Windows clipboard and showed the **Copied** toast. This state is also
  captured in a working screenshot during the run (the delivered capture is the
  two-profile one).
- **All profiles:** **Select all** checked all three boxes and **Copy** put
  `[{"name":"Exp one","volumeAdjustments":[4.0,0.0,0.0,-4.0,0.0,0.0,0.0,0.0]},{"name":"Exp two","volumeAdjustments":[-2.0,0.0,0.0,-4.0,0.0,0.0,0.0,3.0]}]`
  on the clipboard — both profiles, in the saved order (not the click order) —
  and showed the **Copied** toast.

### File export through the native save dialog

**Save to file** opened the real Windows save dialog
(“Export equalizer profiles”, `#32770`); its **File name** box already held the
default `openscl4-equalizer-profiles.json` (read from the accessibility tree),
the JSON filter was offered, and the chosen path was typed in and confirmed.

- One chosen profile → `one-profile.json`, **75 bytes**, exactly the one-profile
  JSON above; the panel showed the green **Saved to the file** note and the
  **Saved to the file** toast.
- Both chosen profiles → `all-profiles.json`, **150 bytes**, byte-identical to
  the clipboard text above; the panel showed the **Saved to the file** note and
  toast again. This is the state of `desktop/export-profiles.png`.

**Cancel.** With one profile chosen, **Save to file** was opened again and its
**Cancel** button clicked: the dialog closed and nothing else changed — no note,
no toast, no file written, and the previous **Saved to the file** note stayed as
it was (so a cancel is not a failure). The unit test
`a_cancelled_save_dialog_changes_nothing` covers the same path (the dialog names
no file, nothing is written, no note and no toast, the temporary directory stays
empty).

**Failure path.** `a_file_that_cannot_be_written_shows_the_failed_note` covers
the write failure: the panel shows the `file-save-failed` note
(“Could not save the file”), no toast and no file. Both tests are part of the
passing desktop suite below.

### Re-import roundtrip

The export texts were fed back through the shipped import paths.

- **Paste with the profiles still saved** (clipboard held the all-profiles JSON):
  the vendored **Import profiles** dialog listed **Exp one** and **Exp two**, each
  tagged **Replaces the saved profile**, with an enabled **Import 2 profiles**
  button. The dialog was cancelled and wrote nothing.
- **File after deleting the saved profiles:** both profiles were removed through
  the row menus and the delete confirmation (the document became `[]` and the
  panel showed the empty-profile note). **Open file** opened the real Windows
  open dialog (“Import equalizer profiles”), the exported
  `all-profiles.json` was chosen in its **File name** box, and the **Import
  profiles** dialog then listed **Exp one** and **Exp two**, each tagged **New**,
  with **Import 2 profiles**.
- **Import:** confirming wrote both profiles back — the panel showed the
  **Imported 2 profiles** note and toast, the document became
  `[{"name":"Exp one","bands":[40,0,0,-40,0,0,0,0]},{"name":"Exp two","bands":[-20,0,0,-40,0,0,0,30]}]`
  (the exact exported bands), and both rows were listed with “Bands: 8”.
- **Apply:** clicking the imported **Exp one** row wrote one custom `03 87` frame
  and re-read `01 01`; the eight band fields and sliders then read
  `4, 0, 0, -4, 0, 0, 0, 0` and the row carried the **Applied** tag. The exported
  file and clipboard text therefore roundtrip through import with matching
  statuses, names and bands.

### Restore

- The **Soundcore Signature** chip was pressed: one `03 87` preset frame, then
  the `01 01` re-poll showed flat bands `[0, 0, 0, 0, 0, 0, 0, 0]` and the name
  tag **Soundcore Signature**.
- The imported profiles were deleted through the row menus; the document became
  `[]`; the app was closed and `equalizer-profiles.json` was removed, so the
  pre-run “no document” state came back. `openscl4-config.toml` was restored to
  its pre-run bytes (`lastDevice = "00:00:00:00:00:09"`).
- A fresh app start (used for the A3949 check below) reconnected the A3062 and
  the Equalizer page showed **Soundcore Signature** with flat bands, proving the
  device was left as found. **About the device** read firmware `04.39`, battery
  `40%`, not charging; no serial row.

### A3949 (P20i) deferred

- First live attempt (during the main run): **Add Device** → `A3949` → the
  **P20i / P25i / R50i** card → the paired row **soundcore P20i**. The P20i was
  off/unreachable then, so that attempt ended in the readable **“Could not
  connect / The device did not answer in time. Try again.”** state with only
  **My devices** and **Settings** visible.
- Live check with the device on (later in the acceptance): the same flow
  connected over RFCOMM. The window showed **Overview**, **My devices** and
  **Settings** only — the Overview tile read **SOUNDCORE · A3949**,
  `soundcore P20i`, **Connected**, **Left: 100 %** and **Right: 100 %** — and
  there was no Equalizer page, no **Import and export** panel, no profile panel
  and no export checkbox or button anywhere in the accessibility tree (a search
  of all elements found none of “Export”, “Import”, “Select all”, “Save to
  file”, “Copy” or “Equalizer”). Pairing was not changed and the P20i was left
  untouched; no A3949 write path was taken.
- Automated: `only_the_editable_a3062_page_offers_saved_profiles` asserts that the
  deferred A3949 page is read-only, has no saved-profile panel, and every export
  action is refused; the panel's `band_edit_allowed` gate and the model's
  missing equalizer data mean the A3949 has no export path.
- The ignored `manual_p20i_has_no_sound_mode` hardware check ran on the reachable
  P20i and passed: it connects the real A3949, asserts no sound pages and no mode
  write path, invokes every export action (choose, one, copy, file) and asserts
  that no save dialog opens, the export count stays 0 and no note or toast is
  shown:

  ```text
  == A3949 P20i / P25i / R50i ==
    candidate soundcore Liberty 4 Pro suggested=false (<address masked>)
    candidate soundcore P20i suggested=true (<address masked>)
    candidate soundcore Space One Pro suggested=false (<address masked>)
    connected via <address masked>
    sound-pages: 0 modes: 0 mode-index: -1
    ambientSoundMode click ignored: no page, no 06 81 path
    export actions ignored: no panel, no save dialog
  test result: ok. 1 passed; 0 failed; ...
  ```

  (`cargo test -p soundcore-desktop manual_p20i_has_no_sound_mode -- --ignored
  --nocapture`; the user's profile document was not touched, and the P20i was
  disconnected afterwards.)

### Screenshot and docs

- `desktop/export-profiles.png` (1700×1300): the Equalizer page from the running
  app on Windows with the connected **soundcore Space One Pro**, both saved
  profiles **Exp one** / **Exp two** and the vendored **Import and export**
  panel: **Select all**, **Exp one** and **Exp two** checked, **Save to file**
  and **Copy** enabled, and the green **Saved to the file** note from the real
  native save-dialog export (150 bytes). No Bluetooth address or serial number
  is in the image.
- `desktop/README.md` now documents profile export end to end (checkbox
  selection and count, the native save dialog and its default file name, the
  transfer JSON format, **Saved to the file** / **Copied** / cancel / failure
  behaviour, the re-import roundtrip, the A3949 deferral), notes that legacy
  migration stays absent and out of scope, and links the new capture.
- The root `README.md` updated the saved-profiles, profile-import/export and
  desktop status rows, marked the profiles crate and controller as owning the
  transfer format, and lists `.scratch/windows-eq-export/` in the layout tree.

### Gates

Run from the final tree:

```
cd desktop
cargo test                                  # 172 passed; 0 failed; 11 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only the pre-existing vendored-Slint build warnings)
cargo build                                 # ok

cd ../core
cargo test                                  # 127 passed; 0 failed across 21 targets
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
cargo build                                 # ok
```

The 11 ignored desktop tests are the manual hardware checks (including
`manual_export_profiles` and `manual_p20i_has_no_sound_mode`); they are not part
of the automated suite. `manual_p20i_has_no_sound_mode` was additionally run on
the reachable P20i and passed (see the A3949 section); `manual_export_profiles`
was run in ticket 02. The new export unit tests
(`choosing_profiles_fills_the_export_flags_and_count`,
`the_row_menu_chooses_only_that_profile_for_export`,
`exporting_to_a_file_writes_exactly_the_chosen_profiles`,
`a_cancelled_save_dialog_changes_nothing`,
`a_file_that_cannot_be_written_shows_the_failed_note`,
`copying_the_chosen_profiles_puts_the_same_json_on_the_clipboard`,
`exporting_with_nothing_chosen_does_nothing`) all run in the desktop suite.

### Merge

The acceptance branch is based on the integration tip `88d668b` (merge of
`ticket/ex-02-controller` into `integration/windows-eq-export`). After the
acceptance commit, `git merge integration/windows-eq-export` reported
“Already up to date”, so no merge commit was needed and the gates above were
re-run on that same tree. The ticket branch is ready for the lead to merge into
the integration branch.
