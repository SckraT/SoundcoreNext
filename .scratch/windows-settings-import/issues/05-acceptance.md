# 05 — Acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02, 04

## Goal

Verify settings and profile import on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062.
2. Visit Controls, Comfort, Power, Connections, Effects, About; verify populated values.
3. Change each setting type: toggle (e.g. side tone/low battery), select (auto-off/double press), range (limit dB); verify one write and re-polled state; restore original.
4. Connections: toggle dual connections; verify “open Bluetooth settings” opens Windows settings page.
5. Import: paste a valid profile JSON and import; verify preview/new, confirm, profile appears/applies; blocked import (7 bands/out-of-range) writes nothing; no legacy banner.
6. A3949 deferred: no pages beyond deferred behavior.
7. Capture `desktop/settings-a3062.png` and `desktop/import-profiles.png`; link README; no MAC/serial.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware, settings values, import JSON, results; restore device settings and user profile document.

## Acceptance

- [ ] Settings writes/re-poll verified on real A3062.
- [ ] Import valid/blocked verified; legacy absent.
- [ ] Screenshots stored/linked; no identifiers.
- [ ] Gates pass; ticket done.
