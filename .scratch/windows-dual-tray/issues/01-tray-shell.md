# 01 — Tray shell

**Type:** task
**Status:** done
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

- [x] desktop gates pass.
- [x] Tray icon and Open/Quit/close verified manually.
- [x] No `.slint` edits (tray component already vendored).

## Implementation (2026-11-21)

The vendored `ui/tray.slint` component now appears in the Windows notification
area and the window's close button puts the program there. The desktop diff is
`desktop/build.rs`, the new `desktop/src/tray.rs` and `desktop/src/main.rs`; no
`.slint` file is changed, no dependency is added and no settings are persisted.

### Build script (`desktop/build.rs`)

`ui/tray.slint` is compiled as its own Slint unit with
`slint_build::compile_with_output_path` into `$OUT_DIR/tray.rs`, before the
existing `slint_build::compile("ui/app.slint")` that `include_modules!()` keeps
using. The tray unit exports a `SystemTrayIcon` and no window, so Slint creates
no window adapter for it (Slint 1.19 compiles such units first-class); that is
what lets `src/tray.rs` create the icon before the event loop runs.
`compile_with_output_path` prints no cargo lines of its own, so the build script
prints `rerun-if-changed` for every dependency the call returns (the tray file
and its `icons/app.svg` asset); the app compile still tracks its own tree.

### Tray module (`desktop/src/tray.rs`)

- The generated unit is included in a private `generated` module and only `Tray`
  is re-exported, so the tray's types do not mix with the window's.
- `TrayContent` is the pure view model of everything the vendored component
  shows: `has-device`, `device-name`, `connected`, `connecting`, `battery`,
  `modes`, `mode-index`, `has-ldac`, `ldac` and the four `show-*` switches. Its
  `no_device()` builder is the skeleton this ticket renders: no device, empty
  name and battery, no modes, no LDAC, every optional menu section allowed, and
  `render()` copies the values onto the component's own properties. The
  quick-preset model is deliberately always empty: this app has no quick
  presets, so the vendored menu words that section as "No presets yet" when a
  later ticket shows it.
- `TrayShell::new` creates the icon, wires the menu's `open`/`quit` callbacks,
  renders `TrayContent::no_device()`, calls `show()`, and installs the window's
  `on_close_requested` handler. The icon is kept in the shell for the whole run;
  `main` drops the shell after `ui.run()` returns, and dropping the component is
  what removes the icon from the notification area.
- **Open** (menu item or a left-click on the icon, both wired in the vendored
  component) upgrades the window handle, clears `minimized`, and `show()`s it.
  An explicit owner focus would need Slint's `unstable-winit-030` feature (the
  exploration notes mark it optional), so this ticket leaves the dependency set
  untouched: the window is shown and restored, and Windows decides the
  foreground under its usual foreground-lock rules; the automation clicks below
  left the foreground on the previously active window.
- **Quit** hides the window and calls `slint::quit_event_loop()`; the icon then
  disappears with the shell as the program exits.
- The window's close handler returns `CloseRequestResponse::HideWindow`, which
  keeps the window hidden while the visible icon keeps Slint's event loop alive
  — close-to-tray. The in-memory `close_to_tray` cell is on by default and a
  later ticket's Settings page can flip it; when it is off (or when the platform
  refused an icon) the same handler also quits.
- Pure label builders keep the tray's wording testable without a notification
  area: `tooltip()` (the product name, the device, and the battery line under it
  once connected), `status_line()` (the menu's disabled "No devices" line or
  "device · Connected/Connecting…/Not connected"), and `closes_to_tray()`
  (icon present and the setting on). They mirror the wording `ui/tray.slint`
  renders from the same values.
- No controller state is read: the Settings TrayPanel's `app-tray`,
  `app-close-to-tray` and `tray-*` fields, the `set-tray`/`set-tray-item`/
  `set-close-to-tray`/`quit-app` actions and the Windows single instance are
  ticket 02's work.

### `desktop/src/main.rs`

`tray::TrayShell::new(&ui)` runs after the controller and before `ui.run()`, and
its value is kept for the whole `main` body, so the icon exists before the first
event-loop iteration and is dropped after the loop ends. A platform without a
notification area logs one line and the program keeps working with the normal
close behaviour.

## Verification

- `desktop` gates on the final tree: `cargo test` **178 passed / 0 failed / 11
  ignored** (6 new tray tests; was 172 / 11), `cargo fmt --all -- --check`
  clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo build` clean.
  The build script prints only the pre-existing vendored-Slint layout warnings
  from `ui/components/settings.slint`.
- New automated tests in `tray::tests`, with no GUI and no controller state:
  - the skeleton is the empty menu with the plain tooltip: `tooltip` is
    `OpenSCL4`, `status_line` is `No devices`, every content field has its
    no-device value (`mode-index` `-1`, empty modes, no LDAC, `show-*` on);
  - the tooltip names the battery of a connected device:
    `OpenSCL4 · Space One Pro\nLeft 97% · Right 76%`;
  - the tooltip leaves the battery out until the device is connected and reports
    one (disconnected with a battery, and connected with an empty battery, both
    read `OpenSCL4 · Space One Pro`);
  - the menu status follows the connection: `Space One Pro · Connected`, then
    `Space One Pro · Connecting…`, then `Space One Pro · Not connected`;
  - `closes_to_tray` is true only with an icon and the setting on;
  - `render` sets every property of a real `Tray` component: the skeleton leaves
    `has-device`/`connected`/`connecting`/`has-ldac`/`ldac` false, device name
    and battery empty, both models empty, `mode-index -1`, and the four `show-*`
    switches true.
- No `.slint` diff (`git diff` touches only `desktop/build.rs`,
  `desktop/src/main.rs` and the new `desktop/src/tray.rs`); `desktop/Cargo.toml`
  and its lock file are unchanged, and no crate or feature is added.

## Manual check (2026-11-21)

Ran `desktop/target/debug/soundcore-desktop.exe` on the Windows 11 bench
(2560×1440) and drove it through the Windows UI Automation tree and window
messages; no device was connected, so the skeleton is what was checked.

- **Icon:** the app window (`My devices — OpenSCL4`) opened and the notification
  area listed a `SystemTray.NormalButton` named `OpenSCL4` in the overflow
  flyout — the native tooltip from the vendored component — with the vendored
  `icons/app.svg` icon; hovering it showed the tooltip `OpenSCL4`.
- **Close-to-tray:** sent `WM_CLOSE` to the window (handle `11667092`). It became
  `visible=False`, the process stayed alive and responding.
- **Open:** invoked the notification-area `OpenSCL4` button through UI
  Automation. The same window handle became `visible=True` with its
  `My devices — OpenSCL4` title again. Then minimized the window with
  `SW_MINIMIZE` (`IsIconic=True`) and clicked the tray icon with a real mouse
  click: the window restored (`IsIconic=False`, `visible=True`). The explicit
  keyboard focus is the one part this ticket leaves to the platform (see the
  Open bullet above); the simulated clicks kept the foreground on the
  previously active window under the Windows foreground lock.
- **Menu skeleton:** posted the tray's right-click callback (`WM_TRAYICON` with
  `WM_RBUTTONUP`) to Slint's hidden `SlintSystemTrayWindow`; the native menu came
  up with the disabled `No devices` line, a separator, `Open OpenSCL4` and
  `Quit` (UI Automation: `No devices` `enabled=False`, then `Open OpenSCL4` and
  `Quit` enabled), which is the requirement 3 skeleton for a no-device state.
- **Quit:** clicked `Quit` in that menu; the process exited and the
  `OpenSCL4` button disappeared from the notification area.
- The screenshots used for the icon, tooltip and menu are not committed here:
  the acceptance ticket 05 stores and links the tray menu capture.

## Integration

The ticket branch was based on the `integration/windows-dual-tray` tip
`8ba314a`; the tip had not moved when the ticket finished, so
`git merge integration/windows-dual-tray` is a no-op and there is no merge
commit to make.
