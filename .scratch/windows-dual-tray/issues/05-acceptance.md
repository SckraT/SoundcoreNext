# 05 — Acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02, 04

## Goal

Verify tray and dual-connections on real A3062 and capture evidence.

## Requirements

1. Build/run; tray icon appears; tooltip shows device/status/battery; menu Actions work (Open/Quit/Connect/mode/LDAC).
2. Close hides to tray; tray Open restores; second launch reveals window; Quit exits and removes icon.
3. Settings TrayPanel toggles/Quit work.
4. Connections: dual on lists devices with connected marker; forget works; connect/disconnect works where a second device exists; dual off hides list and no `0B 01`; refresh on navigate.
5. No MAC/serial in UI/logs/docs; device pairing/settings restored.
6. Capture `desktop/tray.png` (tray menu) and `desktop/dual-connections.png`; link READMEs; no identifiers.
7. Gates: desktop/core test/fmt/clippy/build.
8. Record OS/firmware, devices (names masked if needed), actions/results; ticket done only if real checks pass.

## Acceptance

- [ ] Tray and dual checks verified on real A3062.
- [ ] Screenshots stored/linked; no identifiers.
- [ ] Gates pass; ticket done.
