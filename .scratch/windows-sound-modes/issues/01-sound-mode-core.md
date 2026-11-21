# 01 — Sound mode core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Add the sound-mode model, A3062 state offset and command builder, and a session write path.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/crates/protocol/src/lib.rs` (the `06 81` command constant)
- `core/data/a3062-sound-modes.json` (new sanitized fixtures)

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

## Measured offset and mapping (2026-11-21)

Measured on the real Space One Pro (A3062, firmware 04.39) over the Windows
RFCOMM transport with a temporary ignored test in the desktop crate. The test
was reverted and never committed; its only output was sanitized frames.

- `01 01` response: **105 bytes** = 9 header + **95 body** + 1 checksum.
- **Mode byte: body offset 69** (frame offset 78), one byte.
- Mapping, measured by sending each six-byte `06 81` body
  `[mode, 0x51, 0x01, 0x01, 0x00, 0x05]` and re-reading `01 01`:

  | command / state value | mode |
  |---:|---|
  | `0` | noise canceling |
  | `1` | transparency |
  | `2` | normal |

  After each write the state byte at body offset 69 equalled the mode value that
  was sent, and the three post-write frames differ only in that byte and the
  checksum.
- Every write was acknowledged with the same empty inbound frame
  `09 FF 00 00 01 06 81 0A 00 9A`. That frame is not a state response: the
  session never parses it as state and skips it in the next poll.
- The command body is fixed after the first byte for A3062, and the state
  echoes the five body bytes that follow the mode at body offsets 70..74 (the
  last applied manual/auto, transparency, automation, wind and sensitivity
  settings). ANC strength and those sub-settings are out of scope and are not
  parsed.
- The device was in mode `2` (normal) before the measurement, with echo bytes
  `11 01 00 00 01`; the temporary test wrote that original body back after the
  measurement and the final frame was byte-identical to the first one (serial
  masked), so the device was left as found.
- The pre-measurement frame matched the existing `core/data/a3062-state.json`
  capture except for the low-battery-prompt byte (body offset 76) and its
  checksum, which confirms that fixture was captured in normal mode and that
  the offset convention is right.
- A3949 (P20i) has no sound mode: `DeviceState::sound_mode` is `None` and
  `DeviceModel::sound_mode_body` returns `None`; A3035/A3040/A3947/A3954 are
  likewise unmeasured and return `None`.
- Sanitized fixtures: the three post-write frames are in
  `core/data/a3062-sound-modes.json`, one per mode, serial zeroed, checksums
  recalculated, no address present (the captured body was also scanned for the
  device address before the frame was written out).

## Verification

- `cargo test` in `core` covers:
  - each measured mode fixture parses to its `SoundMode`, and the three real
    frames differ only in the mode byte and the checksum;
  - the A3062 `06 81` body is exactly `[mode, 0x51, 0x01, 0x01, 0x00, 0x05]`
    and its full frames match the measured hex (`...E5`, `...E6`, `...E7`);
  - a truncated A3062 body and an out-of-range mode byte are rejected;
  - A3949 has no mode and no command body;
  - the session writes exactly one mode frame, does not read the ack, and a
    following `poll_state` skips the ack and a notification before the state;
  - a model without a measured mode writes nothing and reports
    `SessionError::UnsupportedSoundMode`.
- `cargo fmt --check` and `cargo clippy -- -D warnings` in `core`; the desktop
  crate still builds and its tests pass with the new `DeviceState` field.

## Acceptance

- [x] core tests/fmt/clippy pass.
- [x] Measured offsets and mapping recorded in the ticket.
- [x] No identifiers committed.
- [x] Ticket status done.
