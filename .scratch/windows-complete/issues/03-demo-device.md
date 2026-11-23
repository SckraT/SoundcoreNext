# 03 — Demo device mode

**Status:** done
**Blocked by:** 01

## Goal

Add a simulated A3062 so the Windows client can be exercised end-to-end without
Bluetooth hardware. No command may leave the process.

## Requirements

1. `Actions.set-demo(true)` on the model-picker step populates
   `paired-options` with a single deterministic simulated device (neutral MAC,
   e.g. `00:00:00:00:00:01`, name `soundcore Space One Pro · Demo`, code A3062).
2. `pair` in demo mode creates a `demo: true` saved card and a simulated
   connected state without opening RFCOMM or spawning a device worker.
3. Demo state is parsed from a committed sanitized A3062 fixture or built from
   the same `DeviceState` shape; it supplies sound mode, equalizer, settings,
   battery/firmware and Overview data.
4. All implemented pages render with demo data: Overview, Sound mode, Equalizer,
   Controls, Comfort, Power and prompts, Connections, About, Diagnostics empty/
   ready state as appropriate. `My devices` shows `Demo device`.
5. Controls update only the local demo `DeviceState` and visible models:
   - sound mode / EQ preset / custom bands / settings toggles;
   - EQ profile save/apply/delete and Overview layout/quick-toggle choices;
   - no `DeviceSession`, `DeviceCommand`, RFCOMM, filesystem device writes.
   Diagnostics probes are not run and no ZIP is produced from demo data; the
   page must say demo mode cannot capture or be disabled.
6. Demo disconnect returns to My Devices and keeps the demo saved card; demo
   reconnect restores the same state.
7. Settings writes in demo do not update through optimistic UI only in a way
   that hides failures; since there is no device, document demo-only behavior.
8. Tests: demo connect, every page populated, writes update local state, no
   command channel, disconnect/reconnect, Diagnostics demo messaging.

## Acceptance

- [x] Add Device → Demo mode → connect works without Windows pairing.
- [x] Every implemented page opens with data.
- [x] Changing mode/EQ/settings updates locally; no RFCOMM or worker exists.
- [x] Diagnostics clearly marked unavailable in demo.
- [x] Desktop gates green.

## Verification

- `desktop/src/demo.rs` holds the simulated device: `DEMO_MAC`
  (`00:00:00:00:00:01`), `DEMO_NAME` (`soundcore Space One Pro · Demo`),
  `demo_model()` (the core's A3062) and `demo_state()`, which parses the
  committed, sanitized `core/data/a3062-state.json` `frame_hex` with
  `soundcore_models::parse_state_frame`. The parsed state is the measured one:
  battery 2/10, firmware 04.39, no serial (the capture masks it), sound mode
  Normal, the Soundcore Signature equalizer, and the settings block with dual
  connections on and LDAC off. `demo_dual_devices()` supplies the Connections
  list — “Demo desktop” (connected) and “Demo phone” — with all-zero neutral
  address bytes and no real fixture name or address. Two module tests cover the
  fixture parse and the neutral/deterministic identifiers.
- `Inner` has a `demo: Cell<bool>` flag, false in `Controller::new` and in
  `test_inner`. `Actions.set-demo` is wired to `set_demo`: switching it on with
  the Space One Pro chosen sets `AppState.add-demo`, clears loading/failure and
  `paired-options` to the single suggested simulated row; any other catalog
  model is refused with the dialog's error line “Demo mode is available for
  Space One Pro” and an empty list. Switching it off clears the row, sets
  `add-loading` and calls `request_paired_for_selection` (the real Windows
  enumeration). `choose-model` and `refresh-paired` keep Demo mode consistent
  instead of enumerating real devices, and opening the Add Device dialog resets
  `add-demo` so one dialog visit starts from the real list.
- `pair` branches to `begin_demo_connect` before `parse_mac`, so the simulated
  row needs no real address. `begin_card_connect` branches on `card.demo` and
  reconnects through the catalog selection (or the demo model when the card's
  code is gone), without `parse_mac` or `request_connect`. `begin_demo_connect`
  saves a `demo: true` current card, shows the demo name/code/art, closes the
  dialog, puts the shell on Connecting/Overview, resets diagnostics, section
  pages and sound/equalizer models, and starts the demo worker with the same
  attempt counter and command channel a real connect uses. It never constructs
  a `DeviceSession`, a `WindowsRfcommTransport` or `open_session`; the shared
  real-connect path resets `demo` to false in `begin_connecting` so a later real
  device is never treated as simulated.
- `serve_demo_device` reports `Event::Connected{attempt,
  Connected(A3062, fixture state)}` and then `DualConnectionsRead` with the
  simulated remembered devices, exactly the event order a real A3062 with dual
  connections on produces. It then serves the existing `DeviceCommand`s on the
  worker's own state copy: `ChangeSoundMode` sets the mode and answers
  `ModeChanged`; `ChangeEqualizerPreset` finds the preset in the model's catalog
  and answers `PresetChanged`; `WriteCustomCurve` sets `0xFEFE` with the eight
  visible bands plus `FIXED_EQ_BANDS` and answers `CurveChanged`; `SetSetting`
  applies all eleven measured settings (including the auto-power-off pair, the
  dB-limit pair that keeps the refresh rate, and the per-bit mode cycle) and
  answers `SettingChanged` with the whole re-polled state, then re-reads the
  dual list when dual connections turn on; `ReadDualConnections` and
  `DualConnectionWrite` connect/disconnect/forget on the local neutral list and
  answer `DualConnectionsRead`/`DualConnectionsWritten`; `RunDiagnostics` is
  ignored, so no capture or archive is ever invented. Every event carries the
  connect attempt and click request numbers, so the controller's existing stale-
  result handling applies unchanged; dropping the channel (Disconnect, Retry,
  app close) ends the worker. No frame is written and no transport exists.
- `drain_events`' `Event::Connected` arm detects the demo flag before applying
  the outcome and, after `apply_outcome` has filled the Diagnostics card, sets
  `diagnostic-available` false and the page error to “Demo mode cannot collect
  diagnostics. No probes are run and no archive is written.”; the Start capture
  button is therefore disabled. `apply_outcome` still renders the Overview,
  section pages and Settings for the simulated state.
- Disconnect keeps the `demo: true` saved card and stops the demo worker; Retry
  reconnects it through the demo branch with no Windows enumeration and no
  address parse. Each demo connect starts from the committed capture again, so
  demo writes do not persist — a documented demo-only behaviour (`demo.rs`
  module docs and the README), as is the fact that demo writes always succeed
  because there is no device to refuse them.
- No `.slint` file changed; the vendored Demo mode switch/row icon/wording,
  `DeviceCard.demo`, and the Diagnostics error banner already existed and are
  now driven by the controller. No real-connection behaviour changed; the only
  touch on the real path is resetting the demo flag in `begin_connecting`.
- Tests: 8 new tests — `demo.rs` fixture parse and neutral identifiers; in
  `controller.rs`, Demo mode list/refusal/off-refresh, a demo connection that
  fills every implemented page and the DualDevices panel with Diagnostics marked
  unavailable, demo writes (mode tile, preset, custom curve, LDAC setting, list
  connect/forget) that leave the connection and demo channel open, the demo
  worker's complete event sequence for every `DeviceCommand` including the
  ignored diagnostics request, every settings variant mapping, and a
  disconnect/reconnect that keeps the card, skips Windows enumeration and
  starts from a fresh fixture state.
- `desktop` gates on the final tree: `cargo fmt --check` clean, `cargo build`
  clean, `cargo test -p soundcore-desktop` **269 passed / 0 failed / 13 ignored**
  (the same 13 manual hardware checks as tickets 01/02; no hardware was run),
  `cargo clippy --all-targets -- -D warnings` clean.
- `core` gates (untouched by this ticket, run for the slice): `cargo fmt
  --check` clean, `cargo build --workspace` clean, `cargo test` **156 passed /
  0 failed / 0 ignored**, `cargo clippy --workspace --all-targets -D warnings`
  clean.

## Integration

The ticket branch was based on the `integration/windows-complete` tip
`3d4509a`; the tip had not moved when the ticket finished, so
`git merge integration/windows-complete` is a no-op and there is no merge commit
to make.
