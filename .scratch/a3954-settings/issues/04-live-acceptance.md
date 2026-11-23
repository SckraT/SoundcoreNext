# 04 — Live A3954 acceptance

**Status:** ready-for-agent
**Blocked by:** 03

## Goal

Verify on the connected Liberty 4 Pro and restore it.

## Requirements

1. Fresh build; connect A3954 over RFCOMM (device is paired and powered).
2. Capture all supported pages; verify sound-mode scale positions 1/6/11,
   wind, EQ preset/custom, Effects (LDAC/spatial/leak), Comfort, Power,
   Connections, About; one write per control with re-poll.
3. Restore every changed value to the initial reading; assert equality.
4. No serial/MAC in screenshots, logs or issue text; no diagnostics changes.
5. Record firmware, screenshots (no address), exact checks, and any deferred
   setting. Update README only with real-device facts.
6. Gates: desktop/core fmt/build/test/clippy.

## Acceptance

- All checks pass; device left as found; ticket done.
