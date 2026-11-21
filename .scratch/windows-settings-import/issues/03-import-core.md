# 03 — Profile import core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Parse and validate OpenSCL4 transfer-profile JSON without UI/file IO.

## Files

- `core/crates/profiles/src/lib.rs` (+ tests)

## Format (OpenSCL4)

`[{"name": "Warm", "volumeAdjustments": [0.4, -0.3, ...]}]` — 8 dB values with one decimal; convert to dB×10 rounded; −120..134 range.

## Requirements

1. `parse_import(text) -> Result<Vec<ImportPreview>, ImportError>` or equivalent:
   - validate JSON array, name non-empty/unique, 8 values, numeric, range after rounding;
   - status per item: New (0), ReplacesExisting (1), WrongBandCount (2), OutOfRange (3);
   - `blocked` when any status >= 2; unparseable top-level → NotProfiles error.
2. Merge function: all-or-nothing upsert by name into `ProfileSet` (replace existing or add).
3. Rounding rule matches OpenSCL4: dB×10 rounded; document half cases.
4. No filesystem/clipboard/UI; serde only.
5. Tests: valid new/replace, 7/9 bands, 13.5 dB, −12.1 dB, decimals/rounding, malformed JSON, duplicate names, empty array, blocked all-or-nothing.

## Implementation (2026-11-21)

Added a pure import module to the existing **`soundcore-profiles`** crate
(`core/crates/profiles/src/import.rs`, re-exported from the crate root; the
lib crate doc gains an import paragraph). The crate still depends only on
`soundcore-models`, `serde` and `serde_json`: no filesystem, clipboard, UI,
Slint or Windows code, and no new dependency (the duplicate-name set is
`std::collections::HashSet`).

### API

- `preview_import(text: &str, saved: &ProfileSet) -> Result<ImportPreview, ImportError>`
  parses and previews in one call. The issue's `parse_import(text)` cannot hand
  out New/ReplacesExisting on its own, because that needs the saved names, so
  the "or equivalent" shape takes the set as well.
- `ImportPreview`: `items()` (file order), `len`, `is_empty`, `is_blocked`
  (any status 2 or 3), `merge_into(&mut ProfileSet)` (all-or-nothing upsert).
- `ImportItem { name: String, band_count: usize, status: ImportStatus,
  bands: Option<[i16; VISIBLE_EQ_BANDS]> }` — `name` is the trimmed name,
  `band_count` is what the dialog shows for a wrong-count row, and `bands` is
  `Some` exactly when the item is importable, so the controller can draw the
  curve for new/replacing rows and needs no re-parse.
- `ImportStatus` with the vendored dialog's tags: `New = 0`,
  `ReplacesExisting = 1`, `WrongBandCount = 2`, `OutOfRange = 3`; helpers
  `is_importable()` and `is_blocking()`.
- `ImportError`: `NotProfiles(serde_json::Error)` (not JSON, not an array, or an
  entry is missing/non-numeric `name`/`volumeAdjustments`), `InvalidName {
  index, source }` (empty after trimming or longer than 60 characters),
  `DuplicateName { name }` (same trimmed name twice in the file), and `Blocked`
  (merge refused). It implements `Display` and `std::error::Error` and chains
  its serde/name source.
- The name rule moved into one shared `normalize_name` helper used by both
  `EqualizerProfile::new` and the import, so an imported name is exactly as
  valid as a saved one (trim, 1..=`MAX_PROFILE_NAME_LEN` characters).

### Format

- One JSON array of `{"name": <string>, "volumeAdjustments": [<number>...]}`;
  the camelCase field name comes from `#[serde(rename_all = "camelCase")]`.
- Unknown object fields are ignored, so a newer OpenSCL4 export still reads;
  missing fields, a non-array document, `null`, a bare number, `[1]`, a
  non-string name and string values are all `NotProfiles`.
- `[]` is a valid empty document: the preview is empty, not blocked, and the
  merge is a no-op. Empty or whitespace-only text is `NotProfiles` (unlike the
  saved-profile document, an import must actually contain a profiles array).
- Names are trimmed, must be 1..=60 characters and unique after trimming;
  matching against saved names is exact, so `Warm` and `warm` are different.
- Statuses are assigned in OpenSCL4's order: band count first (7 or 9 values ->
  `WrongBandCount`), then range (`OutOfRange`), then `ReplacesExisting` when the
  trimmed name is saved, else `New`. A preview is blocked when any item is 2/3.

### Rounding

Matches OpenSCL4's `scaled_to_device`: `value * 10.0` in `f64`, then
`f64::round`, which rounds halves **away from zero**: `0.05` -> `1`,
`-0.05` -> `-1`, `0.15` -> `2`, `-0.15` -> `-2`, `0.04` -> `0`. The rule is
applied to the IEEE-754 product, not to the decimal text; for one-decimal
inputs the product is exact, so decimal ties are real ties: `13.45` -> `135`
and `-12.05` -> `-121`.
The rounded dB×10 must be inside `EQ_BAND_MIN..=EQ_BAND_MAX` = `-120..=134`
(`-12.0`..=`+13.4` dB from `soundcore-models`): `13.4`/`13.44` -> `134` and
`-12.0`/`-12.04` -> `-120` are accepted; `13.5` -> `135`, `-12.1` -> `-121`,
`1e6` -> `10000000` are `OutOfRange`. Non-finite values (not producible by
`serde_json`) also fail the range check.

### Merge

`merge_into` refuses a blocked preview with `ImportError::Blocked` and leaves
the set unchanged, including the valid items in a partly blocked file. Every
profile is rebuilt from the preview before the set is touched; a same-named
profile is replaced in its saved position and a new one is appended in file
order, matching OpenSCL4's bulk upsert.

## Verification

- New `core/crates/profiles/tests/import.rs` (15 tests, all pass), written
  before the implementation: the first `cargo test -p soundcore-profiles` run
  failed to compile with the expected unresolved imports (`preview_import`,
  `ImportItem`, `ImportPreview`, `ImportStatus`, `ImportError`), then the
  implementation made it green.
  - OpenSCL4 export parses, `bands` rounded to dB×10, status `New`;
  - integer values and unknown fields (`enabled`, `curve`, `note`) read;
  - saved names -> `ReplacesExisting` (trimmed `  Warm  ` matches `Warm`),
    `warm` stays `New` (case-sensitive); merge replaces in place and appends
    new profiles in file order, verified by position and bands;
  - rounding halves away from zero (`0.04`, `0.05`, `-0.05`, `0.15`, `-0.15`,
    `0.25`, `-0.25` -> `0, 1, -1, 2, -2, 3, -3`);
  - `13.4`, `13.44`, `-12.0`, `-12.04` accepted at `134`/`-120`;
  - `13.5`, `-12.1`, `13.45`, `-12.05`, `1000000` -> `OutOfRange`, blocked,
    merge `Blocked` and the set stays empty;
  - 7, 9 and 0 values -> `WrongBandCount` with the file's count, blocked, merge
    refuses and an existing profile set is unchanged;
  - count is checked before range (`13.5` in a 7-value profile is
    `WrongBandCount`);
  - a partly blocked file never applies its two valid profiles (all-or-nothing);
  - `[]` and `" \n\t [] \n"` preview empty and merge nothing; 15 malformed
    documents are `NotProfiles`;
  - repeated `Same` / ` Same ` -> `DuplicateName { name: "Same" }`;
    empty/whitespace/61-character names -> `InvalidName` with the matching
    `ProfileError`; a 60-character name is accepted;
  - status tag values 0..=3 and `is_importable`/`is_blocking` helpers.
- Gates from `core/` on the final tree: `cargo test` **103 passed / 0 failed**
  across all crates (was 88; the new 15 are the import tests),
  `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D
  warnings` clean. `RUSTDOCFLAGS="-D warnings" cargo doc -p
  soundcore-profiles --no-deps` is clean, so the module's intra-doc links
  resolve.
- Integration: the ticket branch was based on the
  `integration/windows-settings-import` tip `dbaf123`; the tip had not moved
  when this ticket finished, so `git merge integration/windows-settings-import`
  is a no-op and there is no merge commit to make.

## Acceptance

- [x] core gates pass.
- [x] Format/statuses/rounding recorded.
