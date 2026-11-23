# 03 — Controller: A3954 settings pages

**Status:** ready-for-agent
**Blocked by:** 01, 02

## Goal

Populate and wire the remaining A3954 pages.

## Requirements

1. `device_pages`: Controls (ambient cycle), Comfort, Power, Connections, About
   for A3954; hide what the device does not report.
2. Effects: LDAC toggle, spatial-audio enable + music mode + mode rows, sound
   leak compensation; `has-spatial` flag.
3. Comfort: wearing detection, easy chat + wait time, sound leak, limit high
   volume enable/dB/refresh.
4. Power: auto power off (10/20/30/60), low battery prompt.
5. Connections: dual-connections toggle and remembered-device list.
6. About: left/right/case battery plus case firmware; no serial/address.
7. `set-toggle`, `set-option`, `set-number`, list actions and page entry read
   the right A3954 command and re-poll. Unknown ids never write.
8. Tests: each page's rows, values and command vector; no A3062 regression.
9. Demo worker: A3954 demo can be added later; do not break A3062 demo.

## Acceptance

- All A3954 pages populate; each control writes and re-polls; fmt/build/test/
  clippy green.
