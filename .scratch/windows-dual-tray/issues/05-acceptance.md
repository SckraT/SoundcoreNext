# 05 — Acceptance

**Type:** task
**Status:** done
**Blocked by:** 02, 04

## Goal

Verify tray and dual-connections on real A3062 and capture evidence.

## Requirements

1. Build/run; tray icon appears; tooltip names the device and, while connected, its battery (the vendored tooltip has no status word; status is the menu's device line); menu Actions work (Open/Quit/Connect/mode/LDAC).
2. Close hides to tray; tray Open restores; second launch reveals window; Quit exits and removes icon.
3. Settings TrayPanel toggles/Quit work.
4. Connections: dual on lists devices with connected marker; forget works; connect/disconnect works where a second device exists; dual off hides list and no `0B 01`; refresh on navigate.
5. No MAC/serial in UI/logs/docs; device settings left as found. Pairing: the one user-approved **Forget** of the owner's phone is the deliberate exception — that pairing was removed on the earbuds and cannot be restored from this PC, so the owner must re-pair the phone; no other pairing changed.
6. Capture `desktop/tray.png` (tray menu) and `desktop/dual-connections.png`; link READMEs; no identifiers.
7. Gates: desktop/core test/fmt/clippy/build.
8. Record OS/firmware, devices (names masked if needed), actions/results; ticket done only if real checks pass.

## Acceptance

- [x] Tray and dual checks verified on real A3062.
- [x] Screenshots stored/linked; no identifiers.
- [x] Gates pass; ticket done.
- [x] One approved **Forget** recorded as the deliberate pairing exception: the phone was un-paired from the earbuds, it cannot be restored from this PC, and the owner must re-pair it.

## Comments

Run on the bench machine with the real paired A3062 out of the case, 2026-11-22,
~02:27–03:15 MSK. No Bluetooth address or serial number is recorded here; test
output and screenshots omit them. Pairing was changed only by the one
user-approved **Forget** the ticket allows (see below): the owner's phone is no
longer in the earbuds' remembered list, that pairing cannot be restored from this
PC, and the owner must re-pair the phone; every other pairing and device setting
was left as found.

### Environment

- **OS:** Windows 11 Pro 26H2, build `10.0.26300.9974`, x64.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`); debug build of
  `desktop/target/debug/soundcore-desktop.exe`.
- **Radio:** Intel Wireless Bluetooth; the Soundcore vendor service UUID and
  standard SPP are in the Windows SDP cache for the paired devices.
- **Devices:** soundcore **Space One Pro** (catalog A3062, firmware **04.39**,
  battery 40 % during the run) — connected for the tray and dual-connections
  checks. Paired but only seen in lists: soundcore **P20i** (A3949) and soundcore
  **Liberty 4 Pro** (A3954; it also answered a scripted connection once while a
  failed-connection state was being staged). This listener's remembered devices
  were **DALUNEV** (this computer) and **Pixel 8 Pro** (the owner's phone).
- **Driver:** the app window was driven through the Windows UI Automation tree;
  outputs below are what the running app showed, not test doubles. The tray menu
  is a native Win32 menu, so it was opened through Slint's own notification-icon
  callback message and its pixels were captured from the screen.

### Tray (real A3062 connected)

- **Icon and tooltip.** The icon registered in the notification area and appeared
  in the hidden-icons flyout with the accessible name `OpenSCL4 OpenSCL4 ·
  soundcore Space One Pro\nBattery 40%` — i.e. title plus the tooltip that names
  the device and its battery (the same tooltip the Slint component shows on
  hover). It appears on a fresh run before any device is connected too
  (tooltip just `OpenSCL4`).
- **Menu.** With the A3062 connected the menu showed the device with
  **Connected**, the **Battery 40%** line, the **Sound mode** submenu (the current
  mode checked: Normal at the start), **LDAC**, **Open OpenSCL4** and **Quit**
  (screenshot `desktop/tray.png`).
- **Mode action.** Choosing **Transparency** in the tray submenu wrote the
  `06 81` mode frame and re-read `01 01`: a fresh tray menu then showed
  `✓ Transparency`. Choosing **Normal** restored the original mode and a fresh
  menu showed `✓ Normal` again.
- **LDAC action.** The **LDAC** item toggled LDAC on (fresh menu: `✓ LDAC`) and
  back off (fresh menu: no check). The original setting was LDAC off; it is off
  again after the run.
- **Open action.** After the window was closed (hidden), the **Open OpenSCL4**
  item showed and focused it again; a left click on the icon did the same.
- **Connect action (disconnected state).** A failed connection was staged on
  purpose (a saved card whose connection fails) so the menu offered **Connect**
  (`soundcore Space One Pro · Not connected` / **Connect** / **Open OpenSCL4** /
  **Quit**). Clicking **Connect** started the retry — the window changed to
  “Connecting…” and the attempt ran to its outcome — so the item is wired to the
  same retry path as the page.
- **Quit action.** **Quit** ended the process (process gone), the window closed and
  the notification icon was removed (the hidden tray window was destroyed and the
  hidden-icons list no longer contained OpenSCL4).
- **Close to tray and second launch.** Closing the window (WM_CLOSE, the title-bar
  X) hid it while the process kept running and the icon stayed; tray Open / icon
  click showed it again. With the window hidden, starting the executable a second
  time printed `OpenSCL4: already running; asked the running app to show its
  window`, the second process exited and the first window became visible.
- **Settings TrayPanel.** The **Desktop and quick access** panel's switches work
  in memory: turning **Notification area icon** off removed the icon from the
  hidden-icons list and turning it on brought it back; switching the **Battery**
  section off removed the battery line from a freshly opened menu and on restored
  it; turning **Close to notification area** off made the window's close end the
  program instead of hiding it (process and icon gone). The panel's **Quit
  OpenSCL4** button ended the program and removed the icon. Quick presets are
  absent by design (the app has none; the menu never shows that section).

### Dual connections (real A3062, dual connections on)

- **List.** On connect with dual connections on, the app read the earbuds'
  remembered list (`0B 01`, multi-frame, bounded) and **Connections** showed the
  vendored **Connected devices** panel: `DALUNEV — connected` and `Pixel 8 Pro`,
  each with **Connect**, **Disconnect** and **Forget**. No address appears in the
  rows or the option strings (screenshot `desktop/dual-connections.png`).
- **Refresh on navigate.** Leaving and re-entering **Connections** queued a fresh
  `0B 01` and the panel was rebuilt from the new reply; every re-read after the
  actions below returned a list consistent with the device's answer.
- **Connect on a disconnected remembered device.** Clicking **Connect** on the
  `Pixel 8 Pro` row wrote one `0B 82` frame with that device's address bytes and
  re-read the list (with up to four read attempts while the earbuds settle). The
  row did not gain the connected marker: the phone did not answer the earbuds'
  page during the run (it was either off or out of range). The write and re-read
  path is what the app did; the phone never reported connected.
- **Disconnect on a connected device.** Clicking **Disconnect** on the
  `DALUNEV — connected` row wrote one `0B 81` frame; the earbuds dropped this
  computer's Bluetooth link (the Windows audio endpoint went away), which also
  closed the app's control link, so the next list read failed and the page showed
  the readable banner `Could not change “Disconnect device (connection lost)”` with
  **Retry**. **Retry** reconnected and a fresh list read showed
  `DALUNEV — connected` again. (This is the expected consequence of disconnecting
  the computer the app talks over; the vendored note warns about it for Forget.)
- **Dual off hides the list and no list read.** Switching **Dual connections** off
  wrote `0B 84` off, re-polled `01 01` and hid the **Connected devices** panel;
  leaving and re-entering **Connections** while it was off queued no list read and
  the panel stayed gone (the read path is gated on the re-polled setting, and the
  schedule/ordering is pinned by the automated worker tests). Turning it back on
  wrote `0B 84` on, re-polled and then read `0B 01`; the panel came back after
  **Retry** once the device reset its links (see below). The setting was left on,
  as found.
- **Device link resets during writes.** On this A3062 a `0B 84` dual-connections
  write and an LDAC write each reset the earbuds' Bluetooth links: the write (and
  its `01 01` re-poll) could succeed and the immediately following list read then
  hit the closed link, so the page showed `Dual connections list (connection
  lost)` / `LDAC (connection lost)` with **Retry**. Every **Retry** opened a fresh
  connection, after which reads and writes were normal; this was reproduced for
  the dual toggle (both directions) and for the second LDAC toggle. It is
  recorded in `desktop/README.md`; no frame beyond the measured ones was sent.

### Forget (the one pairing change the ticket allows)

- **Forgotten device: `Pixel 8 Pro`** (the owner's phone name as the earbuds report
  it; the ticket explicitly allows one forget, and none of the two remembered
  devices is non-essential — the other one is this computer). It was the
  disconnected remembered device, so forgetting it did not touch the app's own
  link and the removal could be verified on the device.
- Clicking **Forget** on its row wrote one `0B 83` frame and re-read the list; the
  list then showed only `DALUNEV — connected`, a fresh `0B 01` after navigating
  away and back still showed only that row, and no error banner appeared. The
  earbuds no longer remember the phone.
- **Consequence, stated explicitly:** the phone's pairing with the earbuds was
  removed by the device's own Forget command. Re-pairing the phone is **not**
  possible from this machine/tool (it needs the phone in pairing mode and the
  Bluetooth pairing flow on the phone), so the owner must re-pair the phone in
  the earbuds' Bluetooth settings. The computer's own pairing (`DALUNEV`) was not
  forgotten and the app can connect from this computer as before.
- No address or serial number was recorded; only the device name.

### State restored

Sound mode back to **Normal** (original), **LDAC** off (original), dual
connections **on** (original), tray choices back to their run defaults (memory
only), app not running at the end, notification icon gone. The A3062's audio link
was connected again before finishing (`Наушники (soundcore Space One Pro)` active;
default output unchanged). The only deliberate state change left behind is the
phone pairing described above.

### Screenshots

- `desktop/tray.png` — the tray menu with the connected A3062 (status, battery,
  Sound mode submenu with the current mode checked, LDAC, Open, Quit).
- `desktop/dual-connections.png` — the **Connections** page with the dual list
  (`DALUNEV — connected`, `Pixel 8 Pro`) and the per-row Connect/Disconnect/Forget
  buttons.
- Both are linked from `desktop/README.md`; neither shows a Bluetooth address or
  serial number (the captures show the devices' own names only).

### Gates (this branch tip)

```
cd desktop
cargo test                                  # 215 passed; 0 failed; 12 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only the vendored .slint layout warnings)
cargo build                                 # ok
cd ../core
cargo test                                  # 153 passed; 0 failed (workspace suites)
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

Gates were run after the README and ticket edits in this commit; the same
commands were rerun after merging the integration tip (a no-op: the tip was
already the branch base) and stayed clean.

### Blockers / caveats

- **Silent-read bounding was not rechecked on hardware.** Ticket 03 observed that
  a silent read on this Windows RFCOMM socket did not return within its
  configured 5 s `SO_RCVTIMEO` and asked this ticket to confirm the transport
  actually bounds a silent wait. This run did not reproduce that check, so the
  bound stays unverified on hardware: the transport's own read deadline is the
  limitation, and a silent device may keep the session waiting instead of
  producing the session's timeout banner. The session and controller timeout
  paths remain covered by the scripted-transport tests (ticket 03 and ticket 04),
  not by a live silent read.
- The phone row's **Connect** did not reach a connected state (the phone did not
  answer during the run); the write and re-read were still exercised. The tray
  **Connect** item was verified against a staged disconnected saved card because
  the vendored shell has no control that drops the app's own connection while a
  device is connected.
- The link resets after the dual/LDAC writes are the device's behaviour; the app
  reports them with its readable banner and **Retry** recovers. Recorded in
  `desktop/README.md` as a limitation, not fixed in this ticket.
