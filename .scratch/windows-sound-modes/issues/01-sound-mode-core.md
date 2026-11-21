# 01 — Sound mode core

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Add the sound-mode model, A3062 state offset and command builder, and a session write path.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-*.json` if a new sanitized fixture is needed

## Requirements

1. `SoundMode { NoiseCanceling, Transparency, Normal }`.
2. A3062 state parser exposes `sound_mode: Option<SoundMode>`:
   - measure the offset on the real device by capturing `01 01`, changing mode in the official flow or via known command, and diffing the body;
   - document the offset and evidence in the ticket;
   - A3949 and unmeasured models return `None`.
3. Command builder for A3062 `06 81` with the known 6-byte body shape `[mode, 0x51, 0x01, 0x01, 0x00, 0x05]`; mode mapping measured (0 = noise canceling, 1 = transparency, 2 = normal, or the measured truth) recorded in the ticket.
4. Session method to write the mode command and return only after the frame is written; controller then re-polls state. ACK/notification frames must not be mistaken for state.
5. Tests: sanitized A3062 state with each mode parses; command hex exact; A3949 has no mode; session writes exactly one command and then a state poll.
6. No UI code.

## Acceptance

- [ ] core tests/fmt/clippy pass.
- [ ] Measured offsets and mapping recorded in the ticket.
- [ ] No identifiers committed.
- [ ] Ticket status done.
