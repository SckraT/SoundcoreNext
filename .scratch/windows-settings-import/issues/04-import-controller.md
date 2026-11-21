# 04 — Profile import controller

**Type:** task
**Status:** done
**Blocked by:** 03

## Goal

Wire the vendored Import dialog with paste and file input, preview and confirm.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only for clipboard/file-dialog crates (arboard already in lock; rfd allowed)

## Requirements

1. `Actions.eq-import-paste`: read clipboard text; empty → `transfer-note=ClipboardEmpty`; parse → preview.
2. `Actions.eq-import-file`: open Windows file dialog filtered to JSON; reject >1 MiB (`TooLarge`), read failure (`ReadFailed`), parse.
3. Preview: set `importing=true`, `import-items` with name/bands/status/export=false, `import-blocked` when any status ≥2, `transfer-note` per result; unparseable → `NotProfiles`.
4. `Actions.eq-import`: re-validate all-or-nothing; merge/replace into saved profiles, persist via existing save path, set `transfer-note=Imported`/`toast=Imported`, close dialog, refresh list; blocked import writes nothing.
5. `eq-legacy` stays empty; no migration/export actions.
6. Tests: paste empty/valid/blocked, file too large/read fail (file dialog seam), preview statuses, confirm merge/persist, blocked no-op, transfer notes.
7. Hardware: import a file/paste with a real profile, apply it, delete; restore user document.

## Acceptance

- [x] build/test/fmt/clippy pass.
- [x] Real import and apply verified.
- [x] No `.slint` edits; no legacy.

## Implementation (2026-11-21)

The vendored Import dialog is wired end to end: paste and file input produce a
preview through the core's `preview_import`, and `Actions.eq-import` re-validates
the previewed text, merges it all-or-nothing into the saved `ProfileSet`, writes
the whole set to the existing profile document and closes the dialog. The desktop
diff is `desktop/src/controller.rs`, the `rfd` dependency for the Windows file
picker and the resulting `desktop/Cargo.lock` entries; no `.slint` file changes.

### Machine seam

The clipboard and the native file dialog are behind one trait, `ImportIo`
(`clipboard_text`, `pick_file`, `read_import_file`). `NativeImportIo` is the real
implementation: `arboard` for the clipboard and
`rfd = { version = "0.17", default-features = false }` with a JSON filter (plus
"All files") for the dialog; the default rfd features are the Linux xdg-portal
and Wayland backends and are not needed on Windows. Tests substitute a fake that
serves fixed clipboard text and a fixed picked path, and can make the read fail,
so no test touches the real clipboard or a dialog. `ImportIo` is held by `Inner`
as an `Arc<dyn ImportIo + Send + Sync>` because the file dialog runs on a thread.

### Actions

- `Actions.eq-import-paste`: `arboard` reads the clipboard. Empty or
  whitespace-only text sets `transfer-note = ClipboardEmpty` (OpenSCL4 parity);
  anything else is parsed and previewed.
- `Actions.eq-import-file`: a worker thread opens the native JSON file dialog and
  reads the picked file, then posts `Event::ImportFilePicked { result }` through
  the existing worker-event channel, which the UI timer drains. The dialog never
  blocks the UI thread; `Inner.picking_import_file` stops a second click from
  opening a second dialog. A cancelled dialog changes nothing.
- Files over 1 MiB (`MAX_IMPORT_BYTES`) are refused before reading and the note
  says `TooLarge`; a metadata/read/UTF-8 failure says `ReadFailed` and logs the
  OS error to stderr.
- `Actions.eq-import`: if `importing` is false it does nothing. It re-parses the
  kept preview text against the current saved set, so a blocked file writes
  nothing: the click returns `Blocked`, the dialog stays open with its red
  `import-blocked` note, and neither the set nor the document changes. An
  importable preview is merged (`merge_into`: same-named profiles replaced in
  place, new ones appended), persisted with the existing `persist_saved_profiles`
  path (a failed write rolls the set back and shows the readable page banner),
  then the dialog closes, `transfer-note = Imported` with `transfer-count`, the
  `Imported` toast shows (and clears itself after three seconds), and the saved
  rows are refreshed.

### Preview

`import_rows` builds the dialog's `ImportItem` rows: trimmed name, the status tag
(`New = 0`, `ReplacesExisting = 1`, `WrongBandCount = 2`, `OutOfRange = 3`), the
file's band count and the thumbnail curve from the preview's rounded bands (empty
for a blocked row). `import-blocked` is set when any row is 2/3. Parsing failures
(`NotProfiles`) and a valid but empty `[]` document show the `NotProfiles` note
and open nothing, matching OpenSCL4, which does not offer an empty import.

Export and legacy stay inert: `eq-export-count` and `eq-legacy` are still always
empty, and no export or `eq-migrate` action is wired.

## Verification

- `desktop`: `cargo test` **165 passed / 0 failed / 10 ignored** (14 new
  automated tests plus the ignored hardware check), `cargo fmt --all -- --check`
  clean, `cargo clippy --all-targets -- -D warnings` clean (only the pre-existing
  vendored-Slint layout warnings from the build script), `cargo build` clean.
- `core`: unchanged by this ticket; `cargo test` **119 passed / 0 failed** across
  20 targets, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D
  warnings` and `cargo build` clean.
- New automated tests:
  - paste: empty and whitespace-only clipboards show `ClipboardEmpty` and open no
    dialog; a valid pasted file opens the dialog with one `New` row (name, eight
    bands, curve path) and keeps the text; garbage, `{}`, `null`, `[]`, an empty
    name and duplicate names show `NotProfiles`;
  - file seam: a cancelled dialog does nothing; `TooLarge` and `ReadFailed` show
    their notes; a picked real file is read and previewed;
  - file reader: size is checked before reading (exactly 1 MiB reads, 1 MiB + 1 is
    `TooLarge`); a missing file and a directory are `ReadFailed`;
  - preview statuses: one file with a new, a replacing, a 7-band and an
    out-of-range profile yields tags 0/1/2/3, band counts 8/8/7/8, curves only on
    the importable rows and `import-blocked = true`;
  - confirm: `Warm` is replaced in place and `Fresh` appended; the document is
    written with the merged set; `Imported` note and toast carry count 2; the
    dialog closes and the preview text is consumed;
  - blocked confirm: nothing is merged and the document bytes are unchanged, the
    dialog stays open and no note is added; a click with no preview is a no-op;
  - a document that cannot be written rolls the import back and shows
    `PROFILE_WRITE_FAILURE_LABEL` in the page banner.

## Real hardware (2026-11-21)

The real **soundcore Space One Pro** (A3062, firmware **04.39**, battery 40 %)
was reachable over the Windows RFCOMM transport;
`cargo test -p soundcore-desktop manual_import_profiles -- --ignored --nocapture`
passed:

- paste through the real Windows clipboard and the wired actions: the dialog
  opened with one `New` row ("Warm"), the import closed it and the document
  gained `Warm` with bands `[30, -30, 60, 0, -60, 20, 0, -20]`; applying the
  imported profile wrote the custom curve and the re-poll reported exactly those
  bands, the toast was `Imported 1 profile`;
- file input: a real `openscq30-equalizer-profiles.json` written to the temp
  directory was read by the real reader through the file-dialog seam (the native
  picker itself is not automatable) and imported as "File curve"; the list and
  the document then had two profiles;
- blocked paste: a file with a 7-band profile and a 13.5 dB value previewed with
  statuses 2 and 3, `import-blocked = true`, and the Import click left the dialog
  open with its red note and wrote nothing (document bytes identical, no row
  added);
- deleting both imported profiles through the confirmation path emptied the list
  and the document; the device curve found on connect (`Soundcore Signature`,
  flat bands) was restored, and the user's profile document was restored by the
  test guard. Addresses were masked and no serial was printed or committed.
