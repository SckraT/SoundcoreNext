# 03 — Profile import core

**Type:** task
**Status:** ready-for-agent
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

## Acceptance

- [ ] core gates pass.
- [ ] Format/statuses/rounding recorded.
