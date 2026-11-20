# 04 — Hardware acceptance

**Type:** task
**Status:** done
**Blocked by:** 03

## Goal

Prove the Windows MVP on real paired Soundcore hardware and leave evidence.

## Requirements

1. Build and run the app.
2. Connect to **P20i (A3949)** and **Liberty 4 Pro (A3954)** if available. If one is busy (phone holds SPP), document it and use the other; do not change the user's Bluetooth pairing.
3. Verify:
   - the paired candidate is marked suggested;
   - Overview shows device name/code and battery tiles;
   - values match the device (P20i 5-step mapped to %, Liberty 4 Pro 100/75/case);
   - disconnect returns to Devices; retry reconnects;
   - only `01 01` is written (code review of the session path).
4. Capture `desktop/overview-connected.png` showing the connected Overview with battery; add it to `desktop/README.md`. Mask or avoid MAC display.
5. Run:
   - `cd desktop; cargo test; cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings; cargo build`
   - `cd core; cargo test; cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings`
6. Record OS, device model/firmware (no serial/MAC), commands and results in the ticket.

## Acceptance

- [x] Real hardware connected and battery verified.
- [x] Screenshot stored and linked; no identifiers.
- [x] All gates pass.
- [x] Ticket status done.

## Comments

Run on the machine with the paired devices, 2026-11-21. No Bluetooth pairing
was changed: the app only uses devices already paired with Windows. No MAC
address or serial number is recorded in this file; test output and screenshots
mask or omit them.

### Environment

- **OS:** Windows 11 Pro, build `10.0.26300.9974` (26H2), x64.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`).
- **Rig:** Intel Wireless Bluetooth radio; the Windows SDP cache has the
  Soundcore vendor service UUID and standard SPP for the devices below.
- **Devices used:**
  - soundcore **P20i** — catalog **A3949**, five-step battery, firmware
    **14.43** (read from the state frame);
  - soundcore **Space One Pro** — catalog **A3062**, ten-step single battery,
    firmware **04.39** (read from the state frame);
  - soundcore **Liberty 4 Pro** — catalog **A3954**, did not answer (radio page
    timeout) for the whole session; firmware not read.

### Real-device results

- **Paired candidate is suggested.** With a model chosen, Add Device lists only
  paired SPP/Soundcore devices and marks the model's device with
  “Paired in Windows · looks like the chosen model” (vendor-UUID match):
  `soundcore P20i` for A3949 and `soundcore Space One Pro` for A3062, while the
  other Soundcore devices are listed unsuggested. Verified from the running
  app's accessibility tree and by `manual_paired_connect`, which prints
  `suggested=true` for the matching candidate and `false` for the others.
- **P20i (A3949):** RFCOMM connect and `01 01` succeeded. The stored state has
  `left = 5/5`, `right = 5/5` on the five-step scale, so the Overview shows the
  device as `SOUNDCORE · A3949` / `soundcore P20i` / `Connected` with
  **Left 100 %** and **Right 100 %** tiles. The live firmware (`14.43`) matches
  the firmware in the stored real A3949 capture that
  `polls_the_real_a3949_state` parses. Screenshot:
  `desktop/overview-connected.png` (captured from the running app window, no
  MAC/serial).
- **Space One Pro (A3062):** RFCOMM connect and `01 01` succeeded; the single
  whole-device tile reads **20 %** from a raw `2/10`, matching the stored real
  capture `core/data/a3062-state.json` (battery 2, scale 10, firmware 04.39).
- **Liberty 4 Pro (A3954):** the device did not answer (page timeout). The app
  showed the readable “Could not connect” / “The device did not answer in time.
  Try again.” state, not a crash and not a raw MAC, and Retry repeated the
  attempt (saw `Connecting…`, then the timeout again after ~5 s).
- **Disconnect returns to Devices; retry reconnects.** The vendored UI has no
  disconnect control, so this was checked through the controller callbacks in
  the ignored hardware test `manual_disconnect_and_retry`: real A3062 connect →
  `disconnect` → page `Devices`, `Connection.off`, no current card, no tiles →
  `retry` → real reconnect with the battery tile again. Output:
  `connected [(3, "20%")]` → `disconnect: page=Devices connection=Off` →
  `retry connected [(3, "20%")]`.
- **Only `01 01` is written.** `DeviceSession::poll_state` is the only writer
  (`core/crates/session/src/lib.rs:175-188`); it writes exactly
  `request(Command::STATE)` once per poll (wire frame
  `08 EE 00 00 00 01 01 0A 00 02`, i.e. command `01 01` with an empty body) and
  then only reads. The session test `polls_the_real_a3949_state` asserts the
  transport received only `request(Command::STATE)`
  (`core/crates/session/tests/session.rs:203`). The controller calls
  `poll_state` once per connection (`desktop/src/controller.rs:681-686`) and the
  Windows transport has a single `send` call site, inside
  `WindowsRfcommTransport::write_all` (`desktop/src/transport_windows.rs:270`).
  No settings, equalizer or other command is written.

### Run and capture

`desktop/target/debug/soundcore-desktop.exe` was driven through Windows UI
Automation (the Slint accessibility tree): Add Device → search a model → choose
the model card → pick the paired row; the connection result was read back from
the tree. `desktop/overview-connected.png` is the app window captured with
`PrintWindow(PW_RENDERFULLCONTENT)`, which avoids capturing the desktop and
shows only the window (title bar, Overview hero, battery tiles, status bar).

### Gates

```
cd desktop
cargo test                                  # 73 passed; 0 failed; 3 ignored
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean (only vendored .slint build warnings)
cargo build                                 # ok

cargo test -p soundcore-desktop manual_paired_connect -- --ignored --nocapture
# A3949  connected: label 0=100% label 1=100%  firmware: 14.43
# A3954  failed (TimedOut): The device did not answer in time. ...
# A3062  connected: label 3=20%  firmware: 04.39

cargo test -p soundcore-desktop manual_disconnect_and_retry -- --ignored --nocapture
# A3062  connected [(3, "20%")]
#        disconnect: page=Devices connection=Off
#        retry connected [(3, "20%")]

cd ../core
cargo test                                  # 50 passed; 0 failed
cargo fmt --all -- --check                  # clean
cargo clippy --all-targets -- -D warnings   # clean
```

`manual_disconnect_and_retry` is the ignored test added in this ticket; the two
existing manual hardware checks stay ignored, so the automated suites are still
hardware-free.
