# 03 — Export acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify profile export on real A3062 and roundtrip.

## Requirements

1. Build/run; connect A3062; create/save at least two profiles.
2. Select one and all; verify count and row checkboxes.
3. Export to file (save dialog) and via Copy; capture `desktop/export-profiles.png`.
4. Re-import exported JSON; verify preview statuses/bands match and profiles apply.
5. Cancel save dialog does nothing; failure note path unit-tested.
6. A3949 deferred: no export.
7. Update READMEs: export implemented, legacy absent, out-of-scope notes; link screenshot.
8. Gates: desktop/core test/fmt/clippy/build.
9. Record OS/firmware, profile names/bands, file/clipboard result; restore user document/device; no MAC/serial.

## Acceptance

- [ ] Real export file/copy and re-import roundtrip verified.
- [ ] Screenshot linked; no identifiers.
- [ ] Gates pass; ticket done.
