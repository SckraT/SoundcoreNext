# 02 — Tray state + settings + single instance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Drive the tray from device state and wire the vendored TrayPanel + single instance.

## Files

- `desktop/src/controller.rs`
- `desktop/src/main.rs`
- `desktop/src/single_instance.rs` (new)
- `desktop/Cargo.toml` (only `Win32_System_Threading` feature on existing windows dep)

## Requirements

1. Controller owns the `Tray`; renders on connect outcome, status changes, battery/state re-poll, mode/LDAC changes and tray setting toggles.
2. Tray tooltip/menu from state: device name, connected/connecting/off, battery string, sound mode current selection, LDAC check, Connect enabled only when disconnected (reuse reconnect), Open/Quit. No presets submenu; hide presets.
3. Wire tray callbacks: open → show/focus; quit; connect; choose-mode (use existing sound-mode write path; A3062 only); set-ldac (existing setting write).
4. `on_close_requested` hides to tray when `app-close-to-tray` (default true); Open restores; Quit exits.
5. Settings TrayPanel: set `app-tray-available`, `app-tray`, `app-close-to-tray`, `tray-*` fields; handle `set-tray`, `set-close-to-tray`, `set-tray-item`, `quit-app` in memory (no persistence).
6. Windows single instance: named mutex + event; second launch signals the first and exits; first shows the hidden window. No new crates.
7. Tests: pure tooltip/menu/status builders, tray-action mapping, close-to-tray logic; manual tray checks.
8. No `.slint` edits.

## Acceptance

- [ ] desktop/core gates pass.
- [ ] Icon/tooltip/menu, Open/Quit/Connect/mode/LDAC, close-to-tray, second launch verified.
- [ ] Settings TrayPanel works.
