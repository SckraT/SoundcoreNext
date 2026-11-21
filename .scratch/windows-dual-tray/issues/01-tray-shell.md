# 01 — Tray shell

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Create the Slint system-tray component and basic window lifecycle.

## Files

- `desktop/build.rs` (compile `ui/tray.slint` to OUT_DIR)
- `desktop/src/tray.rs` (new)
- `desktop/src/main.rs`
- no new crates; `windows`/Slint system-tray already available

## Requirements

1. `build.rs` compiles `ui/tray.slint` separately with `compile_with_output_path` (as OpenSCL4 does) plus the existing app.slint.
2. `Tray` component instance created before `ui.run()`, kept alive, dropped on quit; title "OpenSCL4", app icon.
3. Tooltip/menu skeleton: status/device, battery, Connect (disabled when connected), Sound mode submenu, LDAC, Open, Quit. Wire Open → show/focus window; Quit → quit event loop.
4. `on_close_requested` hides the window while the tray remains (close-to-tray), unless a config flag says otherwise (in memory).
5. Pure helper builders tested (tooltip/menu labels); no controller state yet.
6. Hardware/manual: app starts with tray icon, Open/Quit/close work.

## Acceptance

- [ ] desktop gates pass.
- [ ] Tray icon and Open/Quit/close verified manually.
- [ ] No `.slint` edits (tray component already vendored).
