# 02 — Profile export controller

**Type:** task
**Status:** done
**Blocked by:** 01

## Goal

Wire export selection and file/clipboard actions in the vendored panel.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only if a save-dialog API is needed (`rfd` already present)

## Requirements

1. `eq-profiles.export` reflects controller selection; `eq-export-count` = selected count; select-all/row toggles call `eq-export-choose(name|"", checked)`.
2. `eq-export-one(name)`: select only that profile (others false), update count; UI reflects.
3. `eq-export-file`: save dialog (default `openscl4-equalizer-profiles.json`), JSON of selected via core; `file-saved` note + file-saved toast; cancel → no note; write failure → `file-save-failed`.
4. `eq-export-copy`: clipboard selected JSON; `copied` toast; no note needed.
5. Import/legacy remain; no legacy; A3949 no export.
6. Tests: selection/count, one-click selection, exact file contents, cancel, write failure note, clipboard text, no selection no-op, A3949.
7. Hardware: export one profile via file and clipboard, re-import and compare.

## Acceptance

- [x] desktop/core gates pass.
- [x] Real export/re-import verified.
- [x] No `.slint` edits; no legacy.

## Implementation (2026-11-21)

The vendored "Import and export" panel is wired end to end: the Export block's
checkboxes choose saved profiles, and its Save to file and Copy buttons write that
choice to a native save dialog or the Windows clipboard in the OpenSCL4 transfer
format from ticket 01's `export_json`. The desktop diff is
`desktop/src/controller.rs` plus comment-only `desktop/Cargo.toml` wording (no
dependency change); no `.slint` file changes, no legacy migration and no A3949
export.

### Selection (`eq-profiles.export`, `eq-export-count`)

`Inner.export_selection` keeps the chosen profile names. The new
`Inner::refresh_saved_profiles` refills `AppState.eq-profiles` (through
`profile_rows`, which now carries the `export` flag) and `eq-export-count` from
the saved set and the choice, dropping names that are no longer saved, so the
Select all checkbox's `eq-export-count == eq-profiles.length` stays consistent
and a deleted profile cannot leave a stale choice behind. The pure helpers are:

- `choose_export` — `eq-export-choose`: `""` chooses or clears every saved
  profile, a name toggles one row, unknown names change nothing;
- `choose_only_export` — `eq-export-one`: only that saved profile stays chosen
  (the other rows go false). The row menu's "Save to file" item only chooses the
  profile, as the ticket specifies; the panel's Save to file button then writes
  it;
- `chosen_profiles` / `chosen_profiles_json` — the saved-order slice and its
  exact transfer text (`None` for no choice, so the disabled buttons cannot
  open a dialog or touch the clipboard);
- `export_count` — the chosen profiles that are still saved.

### Transfer seam

The import-only `ImportIo` / `NativeImportIo` seam became `TransferIo` /
`NativeTransferIo`, the machine side of the whole Import and export panel. It
still provides the clipboard text and the open-dialog file read for import, and
now adds `set_clipboard_text`, `pick_save_file(default_name)` and
`write_export_file`. `NativeTransferIo` uses arboard for the clipboard and rfd
for the save dialog (JSON and "All files" filters, default
`openscl4-equalizer-profiles.json`); the save dialog runs on a worker thread like
the import picker because the rfd Windows backend initializes COM as an
apartment-threaded process. Tests use one fake, `RecordedTransferIo`, which now
also records the copied text, the save-dialog default name and the written
path and text.

### Actions

- `eq-export-choose(name|"", checked)` and `eq-export-one(name)` update the
  choice and refill the rows and count. Both refuse when the page is not the
  editable A3062 one (`band_edit_allowed`), so the deferred read-only A3949 has
  no export path; its `has-saved-profiles` is false as well, so the panel is not
  shown at all.
- `eq-export-file`: with nothing chosen it opens no dialog. Otherwise the chosen
  JSON is computed on the UI thread, and a worker opens the native save dialog
  and writes the file, posting `Event::ExportFileSaved` on the existing worker
  channel; `Inner.picking_export_file` stops a second save dialog from opening.
  A written file sets `transfer-note = FileSaved` and the `FileSaved` toast;
  cancelling changes nothing (no note, no toast, no file); a failed write sets
  `transfer-note = FileSaveFailed` and logs the operating system's error text to
  stderr.
- `eq-export-copy` computes the same JSON and puts it on the clipboard through
  the seam, then shows the `Copied` toast. Nothing chosen is a no-op; a clipboard
  failure is only logged, as the vendored panel has no note for it. Copying does
  not change `transfer-note`.
- The `FileSaved` and `Copied` toasts clear themselves after three seconds, like
  the existing `Imported` toast; one timer now covers all three transfer toasts.

Import behaviour is unchanged: the dialog still previews through
`preview_import`, merges all-or-nothing and persists through the existing
document path. `eq-legacy` stays empty and no `eq-migrate` action is wired.

## Verification

- `desktop`: `cargo test` **172 passed / 0 failed / 11 ignored** (7 new
  automated tests plus the ignored hardware check; was 165 / 10), `cargo fmt
  --all -- --check` clean, `cargo clippy --all-targets -- -D warnings` clean
  (only the pre-existing vendored-Slint layout warnings from the build script),
  `cargo build` clean.
- `core`: unchanged by this ticket and still green: `cargo test` **127 passed /
  0 failed** across 21 targets, `cargo fmt --all -- --check`, `cargo clippy
  --all-targets -- -D warnings` and `cargo build` clean.
- New automated tests:
  - selection/count: a row checkbox chooses one profile, Select all chooses every
    saved profile and clears them again, unknown names and repeated choices do
    not change the choice, the export text keeps the saved order (not the click
    order), and a stale name is neither counted nor exported;
  - one-click selection: after Select all, `eq-export-one` leaves only that row
    chosen and the other unchecked; an unknown name changes nothing;
  - exact file: the chosen `Warm`+`Flat` write the exact transfer text
    `[{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]},{"name":"Flat","volumeAdjustments":[0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]}]`
    to disk, the save dialog is offered `openscl4-equalizer-profiles.json`, the
    note is `FileSaved` and the toast `FileSaved`, and the text previews through
    the core import path as replacements with the same bands;
  - cancel: the save dialog names no file, nothing is written, no note and no
    toast are shown, and the temporary directory stays empty;
  - write failure: a file chosen under a file (so its directory cannot be
    created) yields `FileSaveFailed`, no toast and no file;
  - clipboard: Copy puts exactly the file's JSON on the clipboard and shows the
    `Copied` toast with no panel note;
  - no selection: no text, no clipboard write, no save dialog, no note and no
    toast;
  - A3949: the deferred `EqualizerDisplay` is not editable, so its page shows no
    saved-profile or export panel, and the read-only page's export actions are
    refused by the same editability gate (also exercised on the P20i hardware
    test below).

## Real hardware (2026-11-21)

The real **soundcore Space One Pro** (A3062) was reachable over the Windows
RFCOMM transport and
`cargo test -p soundcore-desktop manual_export_profiles -- --ignored --nocapture`
passed. The test started from no saved-profile document (the user's document was
captured and restored by the same guard the other hardware tests use) and found
the device on preset `0` “Soundcore Signature”, flat bands
`[0, 0, 0, 0, 0, 0, 0, 0]`:

- no choice: the wired `eq-export-file` action opened no save dialog and showed
  no note, so nothing empty can be exported;
- two profiles were saved with distinct curves: “Export one” at
  `[40, 0, 0, -40, 0, 0, 0, 0]` and “Export two” at
  `[-20, 0, 0, -40, 0, 0, 0, 30]` (both were written as custom `03 87` curves and
  re-polled before saving);
- `eq-export-one("Export one")` chose exactly one profile (count 1, the other row
  unchecked); the chosen profile was exported through the real write half to a
  real file (78 bytes) and through the wired `eq-export-copy` to the real Windows
  clipboard, with the `FileSaved` and `Copied` toasts;
- Select all chose both profiles (count 2, both rows checked); both were written
  to `openscl4-equalizer-profiles.json`-named real file (156 bytes) and copied to
  the clipboard, and the clipboard text matched the file text exactly;
- both saved profiles were deleted, the exported file was re-imported through the
  shipped file seam and the real reader: the preview listed two rows with New
  statuses and the exported bands, the import saved both profiles again with the
  same bands, and the imported “Export one” was applied to the device, whose
  `01 01` re-poll reported exactly `[40, 0, 0, -40, 0, 0, 0, 0]` (the row then
  carried Applied);
- the imported profiles were deleted, the document was empty again, the
  Soundcore Signature preset found on connect was restored, and the test
  disconnected. Addresses were masked; no serial was printed or committed.

The P20i hardware check (`manual_p20i_has_no_sound_mode`) now also invokes every
export action on the A3949 and asserts that no save dialog opens, the count stays
0 and no note or toast is shown.

## Integration

The ticket branch was based on `integration/windows-eq-export` at `806ceb0`; the
integration tip had not moved when this ticket finished, so `git merge
integration/windows-eq-export` is a no-op and there is no merge commit (the same
as ticket 01). The desktop and core gates were re-run on the merged tree after
the merge attempt.

## Follow-ups

- Ticket 03 (acceptance) owns the screenshot, the README updates and the full
  real-hardware acceptance record; this ticket leaves the READMEs untouched.
