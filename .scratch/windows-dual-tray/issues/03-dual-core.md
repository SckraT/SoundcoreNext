# 03 — Dual connections core

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Parse the multi-frame dual-connections list and build connect/disconnect/forget frames.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-dual-connections.json` sanitized fixture

## Requirements

1. `0B 01` request empty body; response frames: `[total_packets, current_index, devices...]`, device = `len` (0x28), `is_connected` (exactly 1), 6 MAC bytes as-is, name bytes `len-8` NUL-padded 32, UTF-8 valid prefix if truncated; no terminator — last when current==total; out-of-order append; current==1 resets.
2. Parsed `DualConnectionDevice { name, mac: [u8;6], connected }`; list read with 5 s total / 2 s per-frame / 20-frame budget; errors on silence.
3. Builders: `0B 82` connect, `0B 81` disconnect, `0B 83` forget with 6 MAC bytes in reply order; `0B 84` enable already exists.
4. Session `read_dual_connections_devices` writes `0B 01`, collects frames; controller later re-reads after writes.
5. Capture one real `0B 01` list from A3062 (mask names/MACs? names are phone/PC names — sanitize to placeholders and MACs to neutral bytes, preserve flags/order/length) and commit fixture; tests parse it plus A3035/OpenSCL4 cases (frame split, multi-device, truncated UTF-8, bad total).
6. Tests exact list frames and write frames; A3949 no list/writes.
7. No UI.

## Acceptance

- [ ] core gates pass.
- [ ] Real capture parsed; fixture sanitized; no identifiers.
- [ ] Ticket records frames/offsets.
