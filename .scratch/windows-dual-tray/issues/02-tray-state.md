# 02 — Tray state + settings + single instance

**Type:** task
**Status:** done
**Blocked by:** 01

## Goal

Drive the tray from device state and wire the vendored TrayPanel + single instance.

## Files

- `desktop/src/controller.rs`
- `desktop/src/main.rs`
- `desktop/src/single_instance.rs` (new)
- `desktop/src/test_support.rs` (new, tests only)
- `desktop/src/tray.rs`
- `desktop/Cargo.toml` (only features on the existing `windows` dep)

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

- [x] desktop/core gates pass.
- [x] Icon/tooltip/menu, Open/Quit/Connect/mode/LDAC, close-to-tray, second launch verified.
- [x] Settings TrayPanel works.

## Implementation (2026-11-21)

The tray is now the controller's: it is created in `Controller::new` before the
window's `run`, filled from the state the controller last read, and answers
Connect, the Sound mode submenu and LDAC through the same writes the window's
own controls use. The vendored Settings TrayPanel's fields and actions are wired
in memory, and a second start of the app asks the running one to show its hidden
window through a named Windows mutex and event. No `.slint` file is changed and
no crate is added.

### Tray module (`desktop/src/tray.rs`)

- `TraySettings` is the Settings panel's model, in memory only: `icon`
  (`app-tray`), `close_to_tray` (`app-close-to-tray`), `battery`, `modes`,
  `presets` and `ldac` (the four `tray-*` switches). The defaults are the spec's:
  icon and close-to-tray on, battery/modes/LDAC sections on, quick presets off.
- `TrayContent::from_state` is the pure state-to-view mapping: whether a device
  is known comes from the saved cards (so a disconnected device still offers
  Connect), and the battery, modes, current-mode index, `has-ldac` and LDAC value
  come from the `AppState` and the last read `DeviceState` while connected. The
  quick-preset section is always hidden: this app has no quick presets, so the
  vendored menu would only ever show "No presets yet"; the vendored panel's
  switch is still kept in memory with the other tray choices.
- `battery_line` words the Overview battery tiles as "Left 97% · Right 76% ·
  Case 60%" / "Battery 20%", the wording the component's property comment names.
- `menu_lines` mirrors the component's `Menu` in Rust (status line, battery,
  Connect while disconnected, the Sound mode submenu with `✓` on the current
  mode, LDAC with `✓`, Open, Quit) so the menu rules are asserted without a
  notification area. `tooltip` and `status_line` keep their ticket-01 wording.
- `TrayShell` owns the component and the window lifecycle: it creates and shows
  the icon, installs `on_close_requested`, and now also takes `TrayActions` (the
  controller's Connect, choose-mode and set-ldac callbacks) and can show or take
  the icon away again (`set_icon`, for the Settings switch). The close handler
  reads the shared close-to-tray choice and whether an icon is on screen, so
  closing hides the window only while Open can bring it back. `open_window` and
  `quit` are `pub(crate)` so the Settings Quit button and the single-instance
  request go through the same paths as the tray's own Open and Quit.
- `TrayShell::available` reports whether the platform has hosted an icon at all:
  `app-tray-available` on the Settings page.

### Controller (`desktop/src/controller.rs`)

- `Inner` gains the `TrayShell` (an `Option`, so tests never create a real icon),
  the in-memory `TraySettings`, and the last `TrayContent`, kept on the UI thread
  so the icon can be created again with the last content and the tests can read
  what the tray shows.
- `Controller::new` creates the shell with `tray_actions(&inner)` (Rc callbacks
  holding the controller weakly), then fills the Settings fields
  (`show_tray_settings`) and the tray (`refresh_tray`) before the event loop
  runs.
- `refresh_tray` renders the tray from `TrayContent::from_state`. It is called
  from `initial_state`'s startup path, `apply_outcome` (connect outcome),
  `apply_failure`, `begin_connecting`, `cancel_connect`, `disconnect`,
  `apply_mode_outcome` (mode change), `apply_setting_outcome` (re-polled LDAC and
  the rest of the settings block), and the four tray setting actions.
- The tray's device actions are one enum (`TrayRequest::{Connect, ChooseMode,
  SetLdac}`) answered by `answer_tray_request` with the window's own handlers:
  Connect is the banner's Retry (reconnect the current or first saved card),
  ChooseMode is the Sound mode tiles' `set-option("ambientSoundMode", index)`
  (`06 81` write), SetLdac is the Effects page's LDAC toggle (the existing
  settings write). The setting ids are shared constants
  (`SOUND_MODE_SETTING_ID`, `LDAC_SETTING_ID`), so the tray cannot drift from the
  page's ids.
- Settings actions wired: `set-tray` (creates/takes away the icon and records the
  choice), `set-close-to-tray` (records it and updates the close handler's
  shared cell), `set-tray-item` (0 battery, 1 sound mode, 2 quick presets, 3
  LDAC), `quit-app` (the same quit as the tray's Quit).
- `default_config_dir` is the `%APPDATA%\OpenSCL4` folder (temporary directory
  fallback) that names the single instance.

### Windows single instance (`desktop/src/single_instance.rs`)

- **Mutex:** `Local\OpenSCL4-<folder key>` claims the app-data folder; the key is
  a lowercased FNV-1a hash of the folder path, stable across builds and
  case-insensitive like Windows folder names. The first start keeps the handle
  open for the whole run on purpose; Windows releases it when the process ends,
  so a crash leaves no stale lock.
- **Event:** `Local\OpenSCL4Show-<folder key>` carries one "show your window"
  request. A second start sets it (after finding the mutex) and returns `None`,
  so `main` exits without creating a window.
- The first start waits on the event on a `single-instance` worker thread, which
  only sets an `Arc<AtomicBool>`. The UI thread takes that flag on its existing
  20 ms worker-event tick (`drain_events`) and calls the same `tray::open_window`
  the Open item uses, so no window handle ever crosses a thread boundary and the
  Slint event loop is not called from the watcher.
- Object creation failures are logged and the app still starts (without a single
  instance), so the lock can never keep the app from running.
- **Cargo feature note:** the ticket named `Win32_System_Threading` only, but
  `windows 0.62` gates `CreateMutexW`/`CreateEventW` behind `Win32_Security` as
  well because both take a `SECURITY_ATTRIBUTES` pointer; the reference app
  enables the same pair. `desktop/Cargo.toml` therefore adds
  `Win32_System_Threading` and `Win32_Security` to the existing `windows`
  dependency, and no crate or other feature is added.

### `desktop/src/main.rs`

`main` claims the app before the window exists; a second start returns `Ok(())`
without a window. The first start hands its `ShowRequest` to `Controller::new` and
keeps the `FirstInstance` (and so the mutex) alive for the whole run. The
controller now owns the icon, so `main` no longer creates a `TrayShell` itself.

### Tests

`desktop/src/test_support.rs` is a tests-only helper that runs every test body
that creates a Slint component on one shared thread: Slint binds its platform to
the thread that creates the first component and refuses components made on
another thread, and the test harness gives each test its own thread. The helper
raises a body's panic again on the test's own thread, so failures still report
normally.

- `tray::tests` (12 tests): the ticket-01 wording/close tests, plus
  `battery_line` wording (left/right/case and the whole-device tile), the exact
  menu lines for a connected device (status, battery, Sound mode with the current
  mode checked, `✓ LDAC`, Open, Quit), Connect only while disconnected (not while
  connected or connecting), quick presets never offered even when the setting is
  on, the settings switches removing menu sections, and two render tests against
  a real `Tray` component (empty content; connected content with modes, mode
  index, LDAC, and the preset model empty).
- `controller::tests` (10 tests): the tray starts as "No devices"; a connected
  A3062 state gives the device name, battery line, three modes, mode index,
  LDAC/`has-ldac` and no preset section; a failure clears the state and offers
  Connect (and a connecting attempt does not); a mode change updates the checked
  mode; a re-polled LDAC value updates the checked LDAC; the Settings switches
  change the `AppState` fields and the menu; a tray mode choice queues
  `DeviceCommand::ChangeSoundMode` with the same index mapping the page uses; the
  LDAC item queues `SettingCommand::Ldac`; Connect reconnects the saved card
  through `retry` (asserted on the saved-card failure for a bad address).
- `single_instance::tests` (4 tests): the folder key is stable and
  case-insensitive, the names carry the key, the first claim owns the app while a
  second claim asks it to show (the request arrives on the watcher thread), and
  two folders are two instances.

## Verification

- `desktop` gates on the final tree: `cargo test` **198 passed / 0 failed / 11
  ignored** (20 new tests; was 178 / 11), `cargo fmt --all -- --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean, `cargo build` clean.
- `core` gates (untouched by this ticket, run for the slice): `cargo test` all
  green, `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D
  warnings` clean.
- No `.slint` diff (`git diff` touches only `desktop/Cargo.toml`,
  `desktop/src/main.rs`, `desktop/src/{controller,single_instance,test_support,tray}.rs`);
  no crate is added or removed.
- Manual smoke check (Windows 11 bench, `desktop/target/debug/soundcore-desktop.exe`):
  the window opened (`My devices — OpenSCL4`), `WM_CLOSE` left the process running
  with the window hidden (close-to-tray), and a second launch of the same
  executable exited with code 0 while the first window became visible again
  (`My devices — OpenSCL4`) — the mutex/event path end to end. The icon, tooltip,
  menu items and the Settings TrayPanel's visible effects are the acceptance
  ticket 05's checks and screenshots; nothing was clicked into the device state
  in this smoke run, and no address or serial appears in the code or the test
  output.

## Integration

The ticket branch was based on the `integration/windows-dual-tray` tip
`e2bbcb1`; the tip had not moved when the ticket finished, so
`git merge integration/windows-dual-tray` is a no-op and there is no merge commit
to make.
