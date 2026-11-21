# 01 — Saved equalizer profiles core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Add a pure profile model, validation and JSON schema, independent of UI/file IO.

## Files

- `core/Cargo.toml` (workspace member `crates/profiles` or a module in an existing crate)
- `core/crates/profiles/src/lib.rs`
- tests

## Requirements

1. `EqualizerProfile { name: String, bands: [i16; 8] }` using the same visible-band range as the A3062 EQ (−120..134 dB×10).
2. `ProfileStore`/`ProfileSet` supporting:
   - `add` / `replace` / `remove` / `get`,
   - duplicate-name detection,
   - non-empty trimmed name (max length e.g. 60),
   - band count/range validation.
3. JSON schema v1: array of profiles with name/bands; deterministic order (name or insertion); parse errors typed; unknown fields ignored; missing file is empty.
4. No filesystem, no UI, no Slint/Windows deps.
5. Tests: roundtrip, duplicate, empty name, out-of-range bands, malformed JSON, unknown fields, empty set.
6. serde/serde_json only.

## Implementation (2026-11-21)

New pure core crate **`soundcore-profiles`** (`core/crates/profiles`), added to
the `core` workspace. It depends only on `soundcore-models` (the measured
visible-band constants), `serde` and `serde_json`; there is no filesystem, UI,
Slint or Windows code.

### Schema v1

A saved-profile document is one JSON array of profile objects, `name` first and
then `bands`:

```json
[
  {
    "name": "Warm",
    "bands": [30, -30, 60, 0, -60, 20, 0, -20]
  },
  {
    "name": "Flat",
    "bands": [0, 0, 0, 0, 0, 0, 0, 0]
  }
]
```

- `name`: trimmed; 1..=60 Unicode characters (`MAX_PROFILE_NAME_LEN`); unique.
  Matching is exact after trimming, so `Warm` and `warm` are different names.
- `bands`: exactly `VISIBLE_EQ_BANDS` = 8 raw dB×10 values, each in
  `EQ_BAND_MIN..=EQ_BAND_MAX` = `-120..=134`, the measured A3062 range from
  `soundcore-models`.
- Unknown object fields are ignored on read and never written back.
- Order is insertion order and is preserved across load/save, so the document is
  deterministic; the compact serialization writes `name`, then `bands`.
- Empty or whitespace-only text parses as the empty set (a missing/empty file
  therefore needs no controller special case) and `[]` roundtrips.

Example set JSON:
`[{"name":"Warm","bands":[30,-30,60,0,-60,20,0,-20]}]`.

### API

- `EqualizerProfile { name: String, bands: [i16; VISIBLE_EQ_BANDS] }`;
  `EqualizerProfile::new(name, bands)` trims the name and validates both fields.
- `ProfileSet`: `new`, `len`, `is_empty`, `profiles`, `iter`, `get`, `contains`,
  `add`, `replace`, `remove`, `from_json`, `to_json`. Lookups, replace and
  remove trim the query name; `add` rejects a duplicate with
  `ProfileError::DuplicateName` and leaves the set unchanged; `replace` updates
  the bands in place (position kept) and returns `ProfileError::NotFound` for a
  missing name.
- Errors are typed: `ProfileError` (`EmptyName`, `NameTooLong`,
  `WrongBandCount`, `BandOutOfRange`, `DuplicateName`, `NotFound`) for the model
  and set operations, and `ProfileJsonError` (`InvalidJson`, `InvalidProfile {
  index, source }`, `DuplicateName`) for parsing. Both implement `Display` and
  `std::error::Error`; the JSON error chains its serde/validation source.
- JSON is deserialized into a private wire type and then validated, so a wrong
  band count is `WrongBandCount` instead of an opaque serde message. The set
  re-validates profiles handed in through the public fields, so an invalid
  `EqualizerProfile` cannot enter a set unchecked.

## Verification

- New `core/crates/profiles/tests/profiles.rs` (11 tests, all pass), written
  before the implementation — the first `cargo test -p soundcore-profiles` run
  failed to compile with the expected unresolved imports:
  - roundtrip through schema v1, including the exact compact JSON and set
    equality;
  - empty set: `""`, whitespace-only and `[]` parse empty; `to_json` is `[]`;
  - unknown fields (`curve`, `enabled`, `bands2`, `note`) are ignored on read and
    never written back;
  - duplicate `add` is rejected and does not overwrite the saved curve; `replace`
    updates in place, keeps the position, and reports `NotFound` when missing;
  - names are trimmed for storage, lookup and duplicate detection; a profile
    built through the public fields is normalized;
  - empty/whitespace names are rejected;
  - 60 characters accepted, 61 rejected; the limit counts characters, not bytes
    (60 multibyte `Ж` are accepted);
  - bands outside `-120..=134` are rejected with their index and value; both
    range ends are accepted;
  - malformed JSON, a non-array document, a 7-band entry, an out-of-range band, an
    unnamed profile and duplicate names are typed errors;
  - insertion order is deterministic and a load/save roundtrip does not reorder.
- Gates on the final tree (from `core/`): `cargo test` **88 passed / 0 failed**
  across 13 test binaries plus doc-tests (the new crate contributes 11),
  `cargo fmt --all -- --check` clean, and `cargo clippy --all-targets -- -D
  warnings` clean.
- `core/Cargo.lock` gained the new workspace member.
- No app/UI dependency: the crate's only dependencies are `soundcore-models`,
  `serde` and `serde_json`.

## Acceptance

- [x] core tests/fmt/clippy pass.
- [x] No dependency on app/UI code.
- [x] Ticket records schema example.
