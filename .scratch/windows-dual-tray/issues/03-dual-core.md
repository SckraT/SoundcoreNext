# 03 — Dual connections core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Parse the multi-frame dual-connections list and build connect/disconnect/forget frames.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-dual-connections.json` sanitized fixture (real A3062 capture)

## Requirements

1. `0B 01` request empty body; response frames: `[total_packets, current_index, devices...]`, device = `len` (0x28), `is_connected` (exactly 1), 6 MAC bytes as-is, name bytes `len-8` NUL-padded 32, UTF-8 valid prefix if truncated; no terminator — last when current==total; out-of-order append; current==1 resets.
2. Parsed `DualConnectionDevice { name, mac: [u8;6], connected }`; list read with 5 s total / 2 s per-frame / 20-frame budget; errors on silence.
3. Builders: `0B 82` connect, `0B 81` disconnect, `0B 83` forget with 6 MAC bytes in reply order; `0B 84` enable already exists.
4. Session `read_dual_connections_devices` writes `0B 01`, collects frames; controller later re-reads after writes.
5. Capture one real `0B 01` list from A3062 (mask names/MACs? names are phone/PC names — sanitize to placeholders and MACs to neutral bytes, preserve flags/order/length) and commit fixture; tests parse it plus A3035/OpenSCL4 cases (frame split, multi-device, truncated UTF-8, bad total).
6. Tests exact list frames and write frames; A3949 no list/writes.
7. No UI.

## Implementation (2026-11-21)

The `0B 01` reply parser, its list accumulator, the `0B 82/81/83` write
builders and the session read/write methods live in the core crates; no `.slint`
or UI code changed. `DeviceModel` gained one capability flag
(`dual_connections`, `true` for the measured A3062 only) so every other model,
including the deferred A3949, refuses the list and the writes before any byte is
written.

### Wire layout

Command group `0B`, from OpenSCL4's
`common/packet/{inbound/dual_connections_device,outbound/dual_connections}.rs`
(permitted) and checked against the real capture below.

| frame | command | body |
|---|---|---|
| list request | `0B 01` | empty |
| connect | `0B 82` | the 6 address bytes, reply order |
| disconnect | `0B 81` | the 6 address bytes, reply order |
| forget | `0B 83` | the 6 address bytes, reply order |
| enable (already implemented) | `0B 84` | `[on]` |

`0B 01` reply body: `[total_packets, current_packet_index, devices...]`:
`total` counts frames, `current` is one-based, and both must be in
`1..=total` (a `total` of 0 is rejected). A device record is:

| record offset | field |
|---:|---|
| 0 | `len` = 8 + name-field bytes (measured `0x28`, i.e. 32 name bytes) |
| 1 | `is_connected` (connected only when exactly `1`, `take_bool` semantics) |
| 2..8 | 6 address bytes, exactly as the reply sent them (no byte reversal) |
| 8..len | name bytes, right-padded with NULs |

The parser cuts the name at the first NUL and decodes UTF-8; a multibyte
character cut in half by the 32-byte field keeps its valid prefix and never
drops the list. A final record padded shorter than its own length byte is
clamped to the bytes present (an observation OpenSCL4 records for another model;
the same tolerant rule is used here). A record whose fixed 8 bytes are missing,
or whose `len < 8`, is a typed error instead of a misparse.

The whole reply has no terminator byte: it ends at the frame with
`current == total`. `DualConnectionsList::push_frame` clears on `current == 1`,
appends every frame's devices (out-of-order frames append in arrival order) and
reports completion on `current == total`.

Errors (`DualConnectionsError`): `NotADualConnectionsFrame`, `ShortBody`,
`BadFrameCount`, `BadDeviceLength`, `ShortDevice`.

### Core API

`core/crates/models/src/lib.rs`:

- `DUAL_CONNECTIONS_LIST: Command` (`0B 01`).
- `dual_connections_request()`, `dual_connections_connect_frame(mac)`,
  `dual_connections_disconnect_frame(mac)`, `dual_connections_forget_frame(mac)`
  — the last three put `[u8; 6]` into the body unchanged, in reply order.
- `DualConnectionDevice { name: String, mac: [u8; 6], connected: bool }`,
  `DualConnectionsFrame { total, current, devices }`,
  `parse_dual_connections_frame(raw)` / `parse_dual_connections_body(body)`.
- `DualConnectionsList` accumulator (`push_frame`, `devices`, `into_devices`).
- `DeviceModel::dual_connections` capability (A3062 `true`; A3035/A3040/A3947/
  A3949/A3954 `false`).

`core/crates/session/src/lib.rs`:

- `DeviceSession::read_dual_connections_devices()` writes the empty `0B 01`
  frame and collects reply frames until the last one, skipping frames of other
  commands. Budgets are OpenSCL4's packet-controller values, exported as
  `DUAL_CONNECTIONS_MAX_FRAMES` (20), `DUAL_CONNECTIONS_FRAME_TIMEOUT` (2 s) and
  `DUAL_CONNECTIONS_TOTAL_TIMEOUT` (5 s); exhausting a budget, or the
  transport's read deadline when the device is silent, is
  `SessionError::Timeout` with the partial list discarded. A body the parser
  rejects becomes `SessionError::Protocol(ProtocolError::DualConnections(..))`.
- `connect_dual_connection(mac)`, `disconnect_dual_connection(mac)` and
  `forget_dual_connection(mac)` write exactly one frame and never read: connect
  and disconnect are fire-and-forget as in OpenSCL4, forget's empty
  same-command ack is left for the caller's next `01 01`/list re-read. A model
  without the capability writes nothing and returns the new
  `SessionError::UnsupportedDualConnections` ("dual connections are not
  supported for this model").
- No UI code: the desktop diff is compile-only, adding the new `SessionError`
  arm to the exhaustive `ModeFailure`/`EqFailure`/`SettingFailure`/
  `ConnectFailure` mappings.

### Exact frames

The usual outbound frame shape (5-byte prefix, command, LE total length, body,
additive checksum), asserted byte-for-byte with the fixture's neutral address
`02 00 00 00 00 01`:

| frame | hex |
|---|---|
| list request | `08 EE 00 00 00 0B 01 0A 00 0C` |
| connect | `08 EE 00 00 00 0B 82 10 00 02 00 00 00 00 01 96` |
| disconnect | `08 EE 00 00 00 0B 81 10 00 02 00 00 00 00 01 95` |
| forget | `08 EE 00 00 00 0B 83 10 00 02 00 00 00 00 01 97` |

## Real hardware capture (2026-11-21)

Captured from the real **soundcore Space One Pro** (catalog A3062, firmware
04.39) over the Windows RFCOMM transport with a temporary ignored desktop test
that was removed before committing (the previous tickets' pattern). The test
first read `01 01` (read-only) to confirm dual connections is enabled, then
wrote only the empty `0B 01` list request; the device was left as found. The
reply arrived complete in one read as a single 92-byte frame:

```text
09 FF 00 00 01 0B 01 5C 00 | 01 01 | 28 01 <6 neutral bytes> <name + NUL pad> | 28 00 <6 neutral bytes> <name + NUL pad> | D0
prefix + command + length   | total/current | device 0 (connected)          | device 1 (not connected)               | checksum
```

Frame offsets: prefix 0..5, command 5..7 (`0B 01`), LE length 7..9 (`0x005C` =
92), body 9..91, checksum 91. Within the body: 0 `total=1`, 1 `current=1`,
2..42 device 0 (`len=0x28`, flag `1`, address body 4..10, name field body
10..42), 42..82 device 1 (`len=0x28`, flag `0`, address body 44..50, name field
body 50..82). Device order, connected flags and both 32-byte name-field lengths
are those of the real reply; the two names were replaced by same-byte-length
placeholders ("Desktop", 7 bytes; "Phone model", 11 bytes) and both addresses by
neutral `02 00 00 00 00:01` / `..:02` bytes, with the checksum recalculated. No
real name, address, serial or adapter id is in the fixture, the tests or this
record.

The committed fixture also keeps per-frame `total`/`current`, so the fixture test
asserts both the frame count fields and the parsed devices. The capture harness
stopped reading on the terminating `current == total` frame; a silent transport
read on this hardware is discussed under Verification as the transport's own
deadline responsibility (it is not exercised by this read).

## Verification

- `core`: `cargo test` **153 passed / 0 failed / 0 ignored** across 17 targets
  (new: 16 models tests and 10 session tests), `cargo fmt --all -- --check`
  clean, `cargo clippy --all-targets -- -D warnings` clean.
- `desktop`: `cargo test` **172 passed / 0 failed / 11 ignored** despite no UI
  change (the compile-only `SessionError` arms), `cargo fmt --all -- --check`
  and `cargo clippy --all-targets -- -D warnings` clean (only the pre-existing
  vendored-Slint layout warnings).
- `core/crates/models/tests/dual_connections.rs` (16 tests):
  - the committed real capture parses to the two expected devices, with the
    fixture's frame `total`/`current` asserted and completion on the last frame;
  - several devices in one frame (`28` records back to back);
  - a name cut mid-UTF-8 at the 32-byte boundary keeps its valid prefix, a name
    that fills all 32 bytes without a NUL keeps all 32 characters, and a record
    padded short of its declared length is clamped;
  - bad frame counts (`0/1`, `2/0`, `2/3`), a body shorter than the two count
    bytes, a `len < 8` record, a record cut off in its fixed bytes and a
    non-`0B 01` frame are all typed errors;
  - the accumulator appends out-of-order frames, restarts on `current == 1` and
    completes on `current == total`; an empty last frame is an empty list;
  - the request and all three write frames are asserted byte-for-byte, and the
    address bytes are asserted unchanged and in order inside each frame;
  - only the A3062 reports the `dual_connections` capability.
- `core/crates/session/tests/session.rs` (36 tests, 10 new):
  - the real fixture frames are collected through one read;
  - the same reply split at every prefix/header/body frame boundary parses;
  - unrelated frames (another command's ack and a battery notification) are
    skipped before the list;
  - silence (`TimedOut` and `WouldBlock`) and a reply that never reaches its
    last frame are `SessionError::Timeout` with exactly one `0B 01` write;
  - a reply with more frames than the 20-frame budget, none terminating, is a
    timeout;
  - a malformed frame count surfaces as
    `Protocol(ProtocolError::DualConnections(BadFrameCount { total: 0, current: 1 }))`;
  - connect, disconnect and forget each write exactly one exact frame and read
    nothing;
  - the A3949 reads nothing, writes nothing and returns
    `UnsupportedDualConnections` for all four methods; a failed write maps to
    `SessionError::Io`.

## Known transport note for ticket 05

The session checks the 2 s/5 s budgets between transport reads and the session
docs make the transport's own read deadline the bound for a silent wait. During
this capture the temporary harness observed that a silent read on this Windows
RFCOMM socket did not return within its configured 5 s `SO_RCVTIMEO` (the test
was abandoned rather than wait), so ticket 05's "timeout → banner, no panic"
hardware check should confirm the transport actually bounds a silent read; the
core timeout paths themselves are covered by the scripted-transport tests above.

## Acceptance

- [x] core gates pass (tests, fmt, clippy; desktop tests/fmt/clippy too).
- [x] Real capture parsed; fixture sanitized; no identifiers.
- [x] Ticket records frames/offsets above.
