# 03 — Saved profiles acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify saved equalizer profiles on real A3062 across restarts.

## Requirements

1. Build/run; connect A3062; create a custom curve; save profile “Test curve”.
2. Verify it appears with curve, bands=8, Applied tag.
3. Change the curve; apply profile; verify one `03 87`, re-poll, Applied tag.
4. Delete profile after confirmation; verify gone and JSON updated.
5. Restart the app; verify saved profiles reload from `%APPDATA%\OpenSCL4\equalizer-profiles.json`.
6. Verify duplicate-name replace prompt, empty-name rejection and invalid bands rejection (unit/UI).
7. A3949 deferred: no page/no profile writes.
8. Screenshot `desktop/equalizer-profiles.png`; link README; document import/export out of scope.
9. Gates: desktop/core test/fmt/clippy/build.
10. Record OS/firmware, profile names/curves, restart result; no MAC/serial.

## Acceptance

- [ ] Save/apply/delete and restart persistence verified on real A3062.
- [ ] Screenshot stored/linked; no identifiers.
- [ ] Gates pass; ticket done.
