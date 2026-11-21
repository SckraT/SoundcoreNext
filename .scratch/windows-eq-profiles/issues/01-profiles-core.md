# 01 — Saved equalizer profiles core

**Type:** task
**Status:** ready-for-agent
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

## Acceptance

- [ ] core tests/fmt/clippy pass.
- [ ] No dependency on app/UI code.
- [ ] Ticket records schema example.
