# 01 — Session core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Add the platform-independent session layer that sits between a transport and the model parser.

## Files

- `core/Cargo.toml` (workspace member `crates/session`)
- `core/crates/session/Cargo.toml`
- `core/crates/session/src/lib.rs`
- tests under `core/crates/session/tests/`

## Requirements

1. `Transport` trait: blocking `write_all(&mut self, bytes: &[u8])`, and a read method that can return partial data (a socket does not preserve frame boundaries).
2. `DeviceSession<T: Transport>` holding a `DeviceModel`.
3. `poll_state()`:
   - writes `soundcore_protocol::request(Command::STATE)`;
   - accumulates reads until a complete frame is available (validate prefix + length + checksum via `soundcore_protocol::parse`);
   - returns `DeviceState` via `parse_state_frame`;
   - tolerates leading garbage and multiple frames in one read;
   - returns a typed error (`Timeout`, `Io`, `Protocol`, `UnsupportedLayout`).
4. No Windows/OS code and no thread spawning in this crate.
5. Tests with a fake transport: real A3949 state frame, frame split across chunks, two frames in one chunk, bad checksum, timeout, unsupported single-battery layout.
6. Document the blocking/threading expectation: the caller runs the session on a worker thread.

## Acceptance

- [x] `cargo test` and `cargo clippy --all-targets -- -D warnings` pass in `core`.
- [x] Existing protocol/models tests still pass.
- [x] No `windows` or Slint dependency in this crate.
