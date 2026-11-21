# 03 — Sound mode acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify the sound mode slice on real hardware and capture evidence.

## Requirements

1. Build and run the app; connect Space One Pro (A3062).
2. Open Sound mode; verify the current mode; switch to each of noise canceling, transparency and normal; confirm the device changes (LED/app/ear) and the UI reflects state after re-poll.
3. Verify P20i (A3949) has no Sound mode page and no `06 81` is sent.
4. Failure path: with the device off/unreachable, the page reports a readable error; no crash/MAC.
5. Capture `desktop/sound-mode-connected.png` showing the Sound mode page; link from `desktop/README.md`.
6. Run all gates (desktop/core test, fmt, clippy, build).
7. Record devices/firmware (no serial/MAC), commands and results in the ticket.

## Acceptance

- [ ] Real mode switches verified on A3062.
- [ ] P20i no-page/no-command verified.
- [ ] Screenshot stored and linked; no identifiers.
- [ ] Gates pass; ticket done.
