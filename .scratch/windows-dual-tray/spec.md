# Spec: OpenSCL4 2.0 — Windows dual connections + tray

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-dual-tray/`
**Integration branch:** `integration/windows-dual-tray`
**Depends on:** export (`master` 44070ae)
**Data:** OpenSCL4 A3062 modules permitted; exploration notes `D:\Projects\SoundcoreNext-notes\dual-tray.md`.

## Goal

Show the earbuds’ remembered/connected devices, allow forget plus connect/disconnect, and add a Windows notification-area tray with status, quick actions and close-to-tray. Minimal vendored-UI edits allowed only for the connect/disconnect buttons and their callbacks; design unchanged.

## In scope

- Core: `0B 01` multi-frame list parser (total/current, device length, connected flag, MAC order, UTF-8 truncated names), `0B 82` connect / `0B 81` disconnect / `0B 83` forget builders, session list read with bounded frames/timeouts, fixture from real A3062 capture.
- Controller: fetch list after connect when dual is enabled; Connections = toggle + DualDevices panel; options mark connected; forget/connect/disconnect actions write and re-read; refresh on navigate; errors.
- UI exception: add Connect/Disconnect buttons + `connect-item`/`disconnect-item` Actions callbacks to `desktop/ui/components/settings.slint` and `desktop/ui/types.slint`; no redesign.
- Tray: Slint `SystemTrayIcon` using the vendored `ui/tray.slint`; icon/tooltip/menu; status/battery/mode/LDAC/Connect/Open/Quit; close-to-tray; Settings TrayPanel actions; Windows single-instance mutex/event; no new crates.
- A3062 only; A3949 deferred.

## Out of scope

- Tray quick presets (app has none), mode changes beyond display, persistence of tray settings, Linux/macOS tray.
- Legacy migration/export.
- Dual list for other models.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-tray-shell.md` | — | build.rs tray unit, icon/tooltip/menu skeleton, Open/Quit, close-to-tray |
| `02-tray-state.md` | 01 | Controller owns tray, renders state, Settings TrayPanel, single instance |
| `03-dual-core.md` | — | Multi-frame parser, builders, session read, tests + fixture |
| `04-dual-controller.md` | 03 | List panel, connect/disconnect/forget UI + writes + re-read |
| `05-acceptance.md` | 02,04 | Real A3062 dual + tray checks, screenshots, docs |

## Acceptance criteria

1. Gates pass (desktop/core test, fmt, clippy, build).
2. Connections shows remembered/connected devices with names and connected marker; forget removes; connect/disconnect update after re-read; disabled dual hides the list and no `0B 01` is sent.
3. No MAC or serial in UI, logs or docs.
4. Tray icon appears with tooltip device/status/battery; menu status/battery, Connect (when disconnected), Sound mode submenu (read-only selection or current), LDAC toggle, Open, Quit.
5. Close hides to tray; tray Open restores; Quit exits; second launch reveals the hidden window.
6. Settings TrayPanel toggles and Quit button work in memory.
7. Screenshots of dual list and tray menu stored/linked; no identifiers.
8. Device pairing/settings left as found after acceptance.

## Closure

Ticket closed when acceptance passes and `Status: done`; merge to `master` after ticket 05 and `code-review`.
