# 01 — Equalizer presets core

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Parse A3062 equalizer state, expose the preset catalog, build the `03 87` preset body, and write it through the session.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-eq-*.json` sanitized fixtures

## Data source

OpenSCL4 A3062 equalizer module is permitted:
- `lib/src/devices/soundcore/a3062/packets/inbound/state_update.rs`
- `lib/src/devices/soundcore/a3062/packets/outbound.rs` (`set_equalizer`)
- `lib/src/devices/soundcore/a3062/modules/equalizer.rs` (preset names/ids/bands)
- `common/structures/equalizer_configuration.rs`, `volume_adjustments.rs`

## Requirements

1. A3062 state parses:
   - EQ starts after battery(1)+unknown(1)+firmware(5)+serial(16) = body offset 23;
   - preset id u16 LE at 23..25;
   - 10 band bytes at 25..35, value = byte - 120 (dB×10);
   - record measured/derived offsets in the ticket; verify against a live A3062 state.
2. Preset catalog with ids/names/bands matching OpenSCL4's A3062 list (13 presets, signature id 0, custom 0xFEFE).
3. Build the full 53-byte `03 87` body:
   - preset id 2 bytes, hear-id genre 2, current band 10, unknown 2 zeros, hear-id enabled 0, hear-id volume 10, time 4 zero, type 2, custom 10, active bands 10, unknown 0 — preserve the A3062 packet shape; use OpenSCL4 defaults/current state so a preset write is accepted.
4. Session method to write the preset body exactly once; controller re-polls state. No ACK read.
5. Tests: real A3062 state parses preset/bands; each preset command hex; A3949 no EQ; session writes one frame; sanitized fixtures no serial/MAC.
6. No UI code.

## Acceptance

- [ ] core tests/fmt/clippy pass.
- [ ] Real A3062 state parse and one preset write verified in an ignored hardware test (device left as found).
- [ ] Ticket records offsets, preset table source, command bytes.
- [ ] No identifiers committed.
