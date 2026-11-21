# 01 — Profile export core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Serialize saved profiles in OpenSCL4 transfer format and roundtrip with import.

## Files

- `core/crates/profiles/src/export.rs` (new) + re-exports/tests

## Requirements

1. `export_json(profiles: &[EqualizerProfile]) -> String`: JSON array `[{"name": "...", "volumeAdjustments": [dB values with one decimal]}]`.
2. Convert internal dB×10 to dB with exactly one decimal (`30 -> 3.0`, `-120 -> -12.0`); use locale-independent JSON; deterministic order.
3. Empty selection returns `[]`.
4. Tests: exact JSON, roundtrip through `preview_import` (all New, bands equal), replace-import roundtrip, negative/flat values, empty.
5. No filesystem/clipboard/UI.

## Acceptance

- [x] core gates pass.
- [x] JSON example recorded.

## Implementation (2026-11-21)

Added a pure export module to the existing **`soundcore-profiles`** crate
(`core/crates/profiles/src/export.rs`, re-exported from the crate root; the lib
crate doc gains an export paragraph and the crate description now says
import/export). The crate still depends only on `soundcore-models`, `serde` and
`serde_json`: no filesystem, clipboard, UI, Slint or Windows code, and no new
dependency.

### API

- `export_json(profiles: &[EqualizerProfile]) -> String` — the selected profiles
  as one compact transfer-profile JSON array in the given order; `&[]` writes
  `[]`. There is no error type: serializing a validated name and eight finite dB
  values cannot fail, and the model has already validated the profiles.
- Serialization goes through the private wire struct the import reader uses
  too: `#[serde(rename_all = "camelCase")] name: String, volume_adjustments:
  Vec<f64>`, so serde_json owns the JSON structure and the name escaping, and
  the read and write shapes cannot drift apart.

### Format and conversion

- One profile:
  `[{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]}]`
- Two profiles, in selection order:
  `[{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]},{"name":"Flat","volumeAdjustments":[0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]}]`
- Each raw dB×10 band is divided by `10.0` in `f64` (`30 -> 3.0`,
  `-120 -> -12.0`, `134 -> 13.4`, `0 -> 0.0`). serde_json (ryu) writes the
  shortest round-trip digits with a decimal point and at least one fractional
  digit, so an integer number of tenths always prints as its exact one-decimal
  dB value. The separator is always `.` (Rust formatting and serde_json are
  locale-independent), and the order is exactly the input slice order, so the
  output is deterministic and byte-identical for the same selection.
- Names are ordinary JSON strings through serde_json, so quotes, newlines,
  backslashes and non-ASCII names stay valid and on one compact line.

## Verification

- New `core/crates/profiles/tests/export.rs` (7 tests), written before the
  implementation: the first `cargo test -p soundcore-profiles --test export` run
  failed on the expected unresolved `export_json` import, then the
  implementation made it green.
  - exact JSON for Warm + Flat (order, camelCase, compact, one decimal) and a
    second export is byte-identical;
  - an empty slice writes `[]`, and the empty export previews as an empty,
    unblocked file;
  - a hard-coded negative/flat/extreme profile:
    `-12.0,13.4,-0.1,0.1,-11.9,11.9,0.0,-10.0`;
  - exhaustive: every band value in `EQ_BAND_MIN..=EQ_BAND_MAX` (255 values)
    exports as the string built from the integers without floating point
    (`-120 -> -12.0`, `31 -> 3.1`, ...), so no value gains or loses a decimal;
  - roundtrip through `preview_import` against an empty set: three profiles, all
    `New`, band count 8, bands equal; the merge reproduces names, order and
    bands;
  - replace roundtrip: a same-named export previews as `ReplacesExisting`,
    merges in place without touching other profiles, and
    export-preview-merge-export is stable;
  - a quoted/newline/backslash/non-ASCII name is escaped, stays on one line and
    roundtrips through the parser unchanged.
- Gates from `core/` on the final tree: `cargo test` **127 passed / 0 failed**
  across all crates (was 120; the 7 new are the export tests),
  `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D warnings`
  clean, and `RUSTDOCFLAGS="-D warnings" cargo doc -p soundcore-profiles
  --no-deps` clean.
- Integration: the ticket branch was based on the
  `integration/windows-eq-export` tip `aa0471d`; the tip had not moved when this
  ticket finished, so `git merge integration/windows-eq-export` is a no-op and
  there is no merge commit to make (same as the previous core ticket).
