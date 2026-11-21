# 01 — Custom equalizer core

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Build/write a custom equalizer body and expose the current custom state.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-eq-custom.json` sanitized fixture

## Requirements

1. Use the existing A3062 `03 87` builder shape; custom `preset = 0xFEFE`, active bands = user bands, fixed last two bands as measured (`[0, -120]`), preserve HearID bytes from the current state/defaults.
2. `DeviceModel::custom_equalizer_body(bands: [u8; 8])` (or equivalent) returns the 53-byte body; values are packet bytes (`dB×10 + 120`), clamped -120..134.
3. Session method writes the custom body once; controller re-polls. No ACK read.
4. Parse custom state (`is_custom()`, bands) already exists; verify.
5. Tests: exact command frame for a known curve; custom preset id; clamping; A3949 deferred/no write; session one write; fixture sanitized.
6. Real A3062: write a distinctive custom curve, re-poll, verify; restore previous preset/curve; record evidence.

## Acceptance

- [ ] core gates pass.
- [ ] Real custom write/re-poll verified; device left as found.
- [ ] Ticket records frame hex/curve/offsets.
