# 01 — Profile export core

**Type:** task
**Status:** ready-for-agent
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

- [ ] core gates pass.
- [ ] JSON example recorded.
