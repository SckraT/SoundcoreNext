# Spec: OpenSCL4 2.0 — Windows connection MVP

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-mvp/`
**Integration branch:** `integration/windows-mvp`
**Home repo:** `D:\Projects\SoundcoreNext`
**Exploration notes:** `D:\Projects\SoundcoreNext-notes\windows-mvp.md` (transport API, UI state map, paired hardware list)

## Goal

Make the vendored OpenSCL4 Windows shell connect to a real Soundcore device over classic Bluetooth RFCOMM and show its battery in the existing Overview page.

This is the first functional Windows slice. It proves the clean-room session layer and the Windows transport against real hardware, without adding settings or equalizer.

## In scope

- Windows paired-device enumeration via the `windows` crate 0.62 (`BluetoothFindFirstDevice`, `BluetoothEnumerateInstalledServices`).
- RFCOMM connect by vendor service UUID with standard SPP fallback; Winsock resolves the channel.
- A transport/session contract in the clean-room core: send `01 01`, read a complete inbound frame, parse state.
- Add Device: list paired Soundcore candidates for the chosen model, mark suggested candidates, pair/connect.
- Overview: connected device name/code/art and battery tiles from the parsed state.
- Disconnect callback, retry, cancel and navigation wiring needed for the above (the vendored UI has no disconnect control).
- Error mapping: Bluetooth off, not found, timeout, connection failed, unsupported layout.

## Out of scope

- Settings, sound modes, equalizer, presets, controls, comfort, connections, case.
- Demo mode and simulated device data: the vendored demo toggle stays inert; only real paired hardware is shown.
- Tray, autostart, notifications, diagnostics UI.
- Android and Linux.
- Writes to the device; only `01 01` is sent.
- Single-battery/TLV state layouts that the core does not implement yet; they should fail gracefully with an unsupported message.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-session-core.md` | — | `soundcore-session` crate: Transport trait, frame assembly, state polling, tests |
| `02-windows-transport.md` | — | Windows enumeration + RFCOMM transport implementing the core trait, tests + manual enumeration |
| `03-controller-ui.md` | 01, 02 | Add Device paired list, connect flow, Overview battery, navigation/errors |
| `04-acceptance.md` | 03, 05 | Real-device check (Space One Pro; P20i/Liberty if reachable), screenshot, docs |
| `05-single-battery-a3062.md` | — | Single-battery state layout so Space One Pro can show battery |

## Acceptance criteria

1. `cargo build`, `cargo test`, `fmt --check`, `clippy -D warnings` pass in `desktop` and `core`.
2. The app lists at least one paired Soundcore model for the chosen catalog model and marks the vendor-UUID candidate as suggested.
3. Connecting to the available real Space One Pro (A3062) shows its single battery tile from the captured ten-step value (the real capture is raw 2/10 → 20 %; a synthetic 5/10 → 50 % covers the scale); if P20i (A3949) or Liberty 4 Pro (A3954) are reachable, their left/right values are shown. The `01 01` state answer for these models carries no case value, so case is not parsed in this MVP.
4. Bluetooth-off, missing device, timeout and unsupported-layout cases show a readable failure, not a crash or a raw MAC.
5. The disconnect callback clears the current device and returns to the Devices page, verified by the ignored hardware test; the vendored UI has no control that calls it in this MVP, so disconnect is not user-verifiable. Retry reconnects; navigation works.
6. Only `01 01` is written; no settings are changed.
7. A screenshot of the connected Overview with battery is stored in `desktop/` and linked from `desktop/README.md`.
8. No serial number or MAC appears in committed docs or screenshots (mask in UI captures if needed).

## Closure

A ticket is closed when its acceptance items pass and its file says `Status: done`. Merge to `master` only after ticket 04 and `code-review` pass.
