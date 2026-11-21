# 03 — Custom equalizer acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify custom equalizer editing on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062; open Equalizer.
2. Drag/type at least two bands to a distinctive curve; commit; verify re-polled state and device sound/state match; screenshot `desktop/equalizer-custom.png`.
3. Press Reset; verify flat curve and state.
4. Switch to a built-in preset and back to custom; verify both still work.
5. Verify chart preview updates during drag without writes (code/test evidence).
6. Failure path: stopped worker shows readable error; no crash/MAC.
7. Link screenshot from `desktop/README.md`, update out-of-scope note.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware (no MAC/serial), curve values, results.

## Acceptance

- [ ] Real custom curve/commit/reset/preset verified.
- [ ] Screenshot stored/linked; no identifiers.
- [ ] Gates pass; ticket done.
