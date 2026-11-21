# 02 — Saved profiles controller

**Type:** task
**Status:** done
**Blocked by:** 01

## Goal

Persist profiles and wire save/apply/delete to the vendored Equalizer page.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only for path dependency on the profiles crate (no new runtime crates needed)
- no `.slint` edits

## Requirements

1. File location: `%APPDATA%\OpenSCL4\equalizer-profiles.json` (or `directories`-free std env); create dir, atomic write (tmp+rename). Load at startup; missing/invalid file → empty profiles, log to stderr, no crash.
2. A3062: `has-saved-profiles=true`; `eq-profiles` list with name, `curve` path from bands, `bands=8`, `in-effect` when current custom bands equal profile.
3. Save dialog (`eq-saving`):
   - `Actions.eq-save(name, replace)`:
     - empty/whitespace name → `eq-save-problem=1`;
     - existing name without replace → `eq-save-problem=2` + `eq-save-existing=name`;
     - otherwise save current custom bands, persist, update list, close dialog (`eq-saving=false`), mark in-effect;
   - `Actions.eq-save` with replace true replaces bands.
4. `Actions.eq-apply(name)`: resolve profile, write custom `03 87`, re-poll, update bands/chart/in-effect; readable error.
5. `Actions.eq-delete(name)`: remove after `eq-deleting` confirmation, persist, update list.
6. Import/export/legacy fields stay empty/inert: `eq-export-count=0`, `eq-legacy=[]`; no file/clipboard actions.
7. A3949 deferred: no page, no writes.
8. Tests: profile load/save atomicity (temp dir), duplicate/empty/invalid, apply writes one curve, in-effect matching, delete persists, malformed file tolerated.
9. Real A3062: save a custom profile, apply, delete; restart persistence checked in acceptance.

## Implementation (2026-11-21)

Only `desktop/src/controller.rs`, `desktop/Cargo.toml` (a path dependency on the
`soundcore-profiles` core crate; no new runtime crate) and the resulting
`desktop/Cargo.lock` changed. The vendored `.slint` tree is untouched: its
saved-profile panel, dialogs and `Actions` already exist, and the controller
fills the fields they read.

### Persistence

- The document is `%APPDATA%\OpenSCL4\equalizer-profiles.json`
  (`APP_DATA_DIR_NAME`/`PROFILES_FILE_NAME`), built from `std::env::var_os`
  instead of a directory-lookup crate. On Windows this is the roaming app-data
  directory the spec names; when `%APPDATA%` is missing the profiles live in
  memory only and a stderr line says so.
- `load_profiles` reads schema v1 through `ProfileSet::from_json`: a missing
  document is an ordinary first run, and a read or parse failure is logged to
  stderr and also loads the empty set, so a bad file never crashes the app or
  blocks startup; the file is left untouched until the next save replaces it.
  `Controller::new` loads the document at startup, before the first device
  connects, so the panel already has the saved profiles.
- `save_profiles` creates the `OpenSCL4` directory, writes the compact schema v1
  JSON to `<document>.tmp`, `sync_all`s it, and renames it over the document.
  The rename is atomic on Windows and replaces the previous document as a
  whole, so a crash or a full disk cannot leave a half-written profile file; a
  failed save removes its temporary file. A failed save is logged to stderr and
  the in-memory profiles stay usable: the next successful save rewrites the
  whole set.

### Saved-profile rows

- `EqualizerDisplay::saved_profiles` is true exactly when the model's page is
  editable (the A3062), so `has-saved-profiles` is true only for the A3062 and
  the deferred A3949 keeps the panel, the Save button and every profile write
  hidden.
- `profile_rows` maps the loaded set to `AppState.eq-profiles`: each row has the
  trimmed name, a curve path generated from the eight bands in the 600×100
  viewbox the vendored `ProfileCurve` draws (`profile_curve` shares the chart's
  smoothing with the profile-height box), `bands = 8`, `in-effect` when the
  profile's bands equal the bands the page currently shows, and `export = false`
  because export is out of scope. The rows are refreshed after every connect,
  re-poll (preset or custom curve), band preview/commit and save/apply/delete.
- Import/export and legacy stay inert: `eq-export-count = 0` and
  `eq-legacy = []` are set on every `show_equalizer`, no export/import/migrate
  action is wired, and no file or clipboard API is used.

### Save, apply, delete

- `Actions.eq-save(name, replace)` is answered by `save_request` +
  `save_equalizer_profile`. A trimmed-empty name sets `eq-save-problem = 1`; a
  saved name without `replace` sets problem 2 and `eq-save-existing`, so the
  dialog asks before overwriting; otherwise the page's eight current (clamped)
  bands become an `EqualizerProfile` under the trimmed name (replacing the saved
  bands when it exists), the set is persisted, the row list is refreshed, the
  dialog closes (`eq-saving = false`) and the profile is already Applied because
  its bands are the current bands. The model's 60-character name limit has no
  own dialog slot, so an over-long name is reported like an empty one
  (problem 1) and its reason is logged to stderr.
- `Actions.eq-apply(name)` resolves the profile and queues its bands through the
  existing `DeviceCommand::WriteCustomCurve`: the device worker writes exactly
  one custom `03 87` body from its last re-polled state (preserving the HearID
  block, as the custom-equalizer slice does) and re-polls `01 01`. The re-polled
  state updates the bands, the chart and `eq-name`, and the refreshed rows mark
  the profile Applied. A stopped worker sets the readable banner label
  `Equalizer bands (connection lost)`; an unknown name queues nothing and is
  logged. The action is gated on a connected, editable A3062 page like the band
  edits.
- `Actions.eq-delete(name)` runs after the `eq-deleting` confirmation dialog:
  the profile is removed, the smaller set persisted, the rows refreshed, and
  `eq-deleting` is cleared last (a stale name only closes the dialog).
- The A3949 is deferred: it has no page, so `has-saved-profiles` is false, no
  rows are shown and the save/apply gates reject it; no profile file action ever
  writes a `03 87` frame for it.

## Verification

- `desktop` `cargo test`: **130 passed, 0 failed, 8 ignored** (was 117/7 before
  this ticket; +13 automated tests and one new ignored hardware test).
- New automated tests, all passing:
  - `the_profile_document_lives_in_the_app_data_directory` — the document path
    is `<app-data>\OpenSCL4\equalizer-profiles.json` and the temporary sibling is
    `<document>.tmp`.
  - `a_saved_profile_set_round_trips_through_the_document` — save then load
    gives the same set, the file is exactly `ProfileSet::to_json`, and no
    temporary sibling remains.
  - `a_missing_or_malformed_profile_document_loads_empty` — a missing file,
    invalid JSON, a non-array, wrong band count, empty name, out-of-range band
    and duplicate names each load as no profiles and are left in place; empty
    and whitespace-only files load empty.
  - `saving_replaces_the_previous_document_without_leaving_a_temporary` — the
    app-data directory is created, a second save replaces the whole document,
    and no temporary file remains.
  - `a_save_that_cannot_write_the_document_reports_an_error` — a file where the
    directory belongs makes the save return a readable error without panicking
    or leaving a temporary file, and the blocking file is untouched.
  - `a_profile_curve_is_drawn_in_the_thumbnail_box` — the curve path starts at
    x=37.5 in the 600-wide box, a flat 0 dB curve sits at y≈52.4 of the 100-unit
    box, and the range ends stay inside the margins.
  - `saved_profile_rows_carry_the_name_curve_band_count_and_applied_flag` —
    insertion order, name, curve path, `bands = 8`, Applied only for the shown
    bands, and `export = false`.
  - `an_empty_profile_name_is_rejected_with_problem_one` — `""`, spaces and
    tabs map to `eq-save-problem = 1` with and without replace.
  - `a_duplicate_profile_name_asks_before_replacing` — an existing name without
    replace maps to problem 2 with the existing name (also trimmed), and with
    replace or for a new name maps to a store request.
  - `applying_a_saved_profile_queues_its_curve_once` — an apply queues exactly
    one custom-curve write carrying the profile's eight bands (the worker test
    already proves that command writes one `03 87` frame and re-polls).
  - `an_unknown_profile_or_a_stopped_worker_queues_nothing` — unknown names and
    a stopped worker write nothing and report Unknown/Disconnected.
  - `deleting_a_saved_profile_persists_the_smaller_set` — removing a profile,
    saving and reloading leaves only the other one, and the file is the smaller
    set.
  - `only_the_editable_a3062_page_offers_saved_profiles` — the A3062 page offers
    saved profiles while the deferred A3949 page does not.
- `desktop` `cargo fmt --all -- --check` clean, `cargo clippy --all-targets --
  -D warnings` clean, `cargo build` ok (only the pre-existing vendored-Slint
  layout warnings).
- `core` (unchanged by this ticket, re-run): `cargo test` **88 passed, 0
  failed** across 13 test binaries plus doc-tests; fmt and clippy clean.

## Real hardware (2026-11-21)

Verified with the new ignored test on the real **soundcore Space One Pro**
(catalog **A3062**; the same device/firmware **04.39** as the custom-equalizer
slice) over the Windows RFCOMM transport:

```text
cargo test -p soundcore-desktop manual_saved_profiles -- --ignored --nocapture
```

- Connected; the page offered saved profiles (`has-saved-profiles: true`) with
  an empty list, `eq-export-count: 0` and an empty legacy list. Found on
  connect: preset **0**, **Soundcore Signature**, bands
  `[0, 0, 0, 0, 0, 0, 0, 0]`.
- Wrote a custom curve `[40, 0, 0, -40, 0, 0, 0, 0]` (+4.0 dB at 100 Hz,
  −4.0 dB at 800 Hz) through the typed and committed band paths.
- **Save “Test curve”:** one row with the name, a curve path, `bands = 8` and
  the Applied tag; the dialog closed; the document
  `%APPDATA%\OpenSCL4\equalizer-profiles.json` gained the profile with exactly
  those bands.
- **Restart path:** a fresh `Controller::new` on a new window (what a restarted
  app runs) loaded the real document and already listed 1 profile, “Test curve”.
- **Rejections:** a whitespace name set problem 1; a duplicate “Test curve”
  without replace set problem 2 and `eq-save-existing = "Test curve"` without
  adding a row.
- **Replace:** changing the curve cleared Applied; saving again with replace
  persisted `[40, 20, 0, -40, 0, 0, 0, 0]` under the same single profile and the
  Applied tag returned.
- **Apply:** clicked row → the re-polled state was the custom preset `65278` with
  bands `[40, 20, 0, -40, 0, 0, 0, 0]`, and the row showed Applied again.
- **Readable error:** with the worker stopped, an apply showed
  `"Equalizer bands (connection lost)"` and wrote nothing; the banner’s Retry
  reconnected.
- **Delete:** after the `eq-deleting` confirmation state the row was gone, the
  dialog cleared, and the document loaded empty.
- Left as found: the preset found on connect (Soundcore Signature, flat) was
  restored, and the profile document was backed up before the run and restored
  afterwards, so no profile was left behind. Addresses are masked; no
  MAC/serial appears in the output.

## Acceptance

- [x] desktop/core gates pass.
- [x] Real save/apply/delete verified.
- [x] No `.slint` edits; import/export inert.
