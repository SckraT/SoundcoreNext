# Soundcore Next

Open-source Soundcore control app for Android, Windows and Linux.

**OpenSCL4 2.0 development.** This branch is the first OpenSCL4 2.0 slice: the
desktop product keeps the OpenSCL4 name, and its UI is vendored from OpenSCL4
under GPL-3.0 — the window title and product name stay unchanged. The vendored
tree keeps its upstream visual language; local UI-kit, Overview, Diagnostics,
connection and settings adaptations are recorded in `desktop/ui/SOURCE.md`, with
component contracts in `desktop/ui/UI_KIT.md` and the implementation workflow in
`desktop/ui/GUIDE.md`. Protocol command/response data and device preset data are
adapted from OpenSCL4 under
GPL-3.0 too; the attribution is in `NOTICE`.

- **Core** (`core/`) — Rust: protocol framing, model state parsers, diagnostics. The command/response layouts, state offsets and device preset tables it implements are adapted from OpenSCL4's command/response and preset data (authorized use; see `NOTICE`) and from the research wiki; the Rust implementation is this project's own.
- **Android** (`android/`) — planned Kotlin/Compose UI; the OpenSCL4 UI may be reused and adapted under GPL-3.0 (ADR 0004).
- **Desktop** (`desktop/`) — Rust + Slint: the OpenSCL4 2.0 desktop shell, with the UI vendored from OpenSCL4 under GPL-3.0.

The normative specification lives in the research repository at `D:\Pixel 8 Pro\Soundcore\wiki` (`wiki/docs/index.md`). In particular:

- `wiki/docs/protocol/framing.md`
- `wiki/docs/protocol/models.md`
- `wiki/docs/diagnostics/session-format.md`
- `wiki/docs/process/adr/0004-openscl4-ui-reuse.md`

## Current status

| Area | Status |
|---|---|
| Frame parser/encoder | implemented, 503/503 corpus frames parse |
| Model catalog | 39 models grouped into 5 form factors and 11 families |
| A3949 real state | parsed, battery 5/4, firmware 14.43, masked serial |
| A3062 real single-battery state | parsed, one battery tile, firmware 04.39, masked serial |
| A3062 sound modes | parsed at state body offset 69 and writable through a measured `06 81` body (0 = noise canceling, 1 = transparency, 2 = normal); sanitized fixture per mode |
| A3062 equalizer | parsed at state body offset 23 (preset id + ten bands); the page shows the eight visible bands (100 Hz…12.8 kHz, −12.0…+13.4 dB), the 22 built-in presets adapted from OpenSCL4 and editable custom bands (fields, sliders, Reset), writes one measured `03 87` preset or custom (`0xFEFE`) body and re-reads `01 01`; sanitized post-write fixtures |
| A3062 saved equalizer profiles | named profiles of the eight visible bands, saved/applied/deleted on the Equalizer page; applied through one measured `03 87` custom body and a `01 01` re-poll; persisted in `%APPDATA%\OpenSCL4\equalizer-profiles.json` (schema v1, atomic write) and reloaded at startup; duplicate/empty names rejected in the dialog, out-of-range bands clamped by the page edits and rejected by the model/load, a failed document write reported in the page banner and rolled back; the same profile set is imported and exported as OpenSCL4 transfer JSON (see the profile import and export row); legacy migration and A3949 profiles are deferred |
| A3062 remaining settings | parsed from the `01 01` state body 67..87 (button double press and ambient cycle, low-battery prompt, Dolby and LDAC, dual connections, auto power off, limit high volume enable/dB/refresh, side tone, voice prompt, charging/battery/firmware); the Controls, Comfort, Power and prompts, Sound and effects, Connections and About pages show the device's own values; each change writes exactly one measured settings frame and re-reads `01 01`, a failed write shows a readable banner with Retry that means the write or its confirmation poll failed, so the device's state is unknown until Retry re-polls it |
| A3062 dual connections | remembered-device list read with a bounded multi-frame `0B 01` after connect while dual connections is on, shown on Connections with the parsed names and the device's connected marker; per-row Connect/Disconnect/Forget write one measured `0B 82`/`0B 81`/`0B 83` frame and re-read the list; dual connections off hides the list and sends no `0B 01`, turning it on writes `0B 84`, re-polls and then reads; A3949 deferred |
| A3062 profile import and export | OpenSCL4 transfer-profile JSON (`name` + eight `volumeAdjustments` dB values with one decimal) both ways: import pasted from the clipboard or picked with a native JSON dialog (1 MiB cap); the vendored Import profiles dialog previews New/Replaces/wrong-band-count/out-of-range; confirming re-validates the whole file all-or-nothing, merges it into the saved profiles (same names replaced) and persists; blocked files write nothing. Export chooses saved profiles individually or all (row checkboxes and Select all, count enables the buttons), writes the same JSON through the native save dialog with default `openscl4-equalizer-profiles.json`, or copies it to the Windows clipboard; a written or copied export re-imports as New/Replaces with the same bands; a cancelled dialog changes nothing and a failed write shows “Could not save the file”; no legacy migration; A3949 deferred |
| A3949 equalizer and settings | deliberately deferred: the committed A3949 state carries an equalizer block at body 32..44, but its write bodies are unverified, so the Equalizer, Effects and section pages are hidden and no `03 87` or settings frame is sent |
| A3954 real state | parsed the real 165-byte `01 01` body: TWS batteries, both firmwares, case firmware/battery/serial (masked), the ten-band equalizer channel (the real device reports its left channel; the right-channel slot parses as absent), its HearID block, the 1–11 sound-mode scale, buttons, ambient cycle, case features, air pressure, LDAC/dual connections, auto power off, limit high volume, spatial audio, easy chat, sound leak, case language (parsed read-only) and wearing detection; sanitized fixture. **Measured limitation:** in airplane mode the real device reports slider byte `13`, outside the measured 1–11 scale, so the whole sound-mode block reads as unavailable (`None`) and the scale page is hidden while the rest of the state still parses; the byte's meaning is unmeasured and no scale position is invented for it |
| A3954 commands | writes the measured `06 81` `[mode, position, airplane, wind, FF]` body for the 1–11 scale (1–5 transparency, 6 normal, 7–11 noise canceling), the 22-preset `common_settings_type_2` catalog and the 114-byte `03 87` preset/custom bodies that keep the HearID fields, plus the `01 FF`/`0B 84`/`01 86`/`20 82`/`20 81`/`10 82`/`06 82`/`01 81`/`10 9A`/`10 81`/`10 9D` settings frames; the case-display language write (`07 8A`) is out of this slice and its state field stays read-only; sanitized real state, scale-write and equalizer-body fixtures |
| A3954 desktop pages | the Liberty 4 Pro opens on the Sound mode page as one 1–11 scale with the reported position and a wind-noise suppression toggle (one measured `06 81` write per change and a `01 01` re-poll); the Equalizer page uses the A3954's own 22-preset catalog and the eight visible bands of its two-channel block, writes the 114-byte preset/custom body and keeps the same editable bands, saved profiles and import/export as the A3062; Sound and effects (LDAC, spatial audio enable/music mode/mode, sound leak), Comfort (wearing detection, easy chat and wait time, sound leak, limit high volume enable/dB/refresh), Power and prompts (10/20/30/60-minute auto power off, low battery prompt), Connections (dual toggle and remembered list) and About (left/right/case battery, both earbud firmwares and case firmware; no serial) each write one measured A3954 frame and re-read `01 01`; Overview shows the scale, its shortcuts and quick toggles. **Accepted on the real Liberty 4 Pro (firmware 04.29, case firmware 01.58)** on 2026-11-23: the ignored `manual_a3954_settings` test connected the paired device over RFCOMM and wrote/re-polled positions 1/6/11, the wind flag, one equalizer preset, a custom curve that differed from the connect curve (re-polled as the custom id `0xFEFE` and the written visible bands), and every supported settings command, restoring the state found on connect field by field (the final poll matched it, zero reconnects in the review run); it also recorded that the device silently drops a setting command sent too soon after the previous one, so the test spaces each write and retries until the re-poll confirms it. The real Add Device UI populated every page (the scale at the device's position 6, the 22-preset Equalizer, Sound and effects, Controls, Comfort, Power and prompts, Connections with the remembered-device list, About with 04.29/01.58 and no serial) without a write through the UI; captures are in `desktop/live-a3954-*.png`. When the device reports the unmeasured airplane-mode slider byte, only the scale and its page/Overview block are hidden; the equalizer, settings pages and connection stay. Button remapping, case display/features/language and air-pressure writes are deferred |
| Other single-battery layouts | intentionally unimplemented (`UnsupportedLayout`) |
| TLV D1203 | not started |
| Diagnostics | Windows Diagnostics page with three read-only probes (`01 01`, `01 03`, `01 04`), per-probe progress, a local redacted ZIP (`manifest.json`, `frames.jsonl`, `README.txt`) saved through a native dialog, and a prefilled GitHub issue; no automatic upload, A3062 serial and Bluetooth addresses redacted from the frames and from the manifest's probe responses |
| Windows session/transport | session requests `01 01`, parses the answer, reads the bounded multi-frame `0B 01` dual-connections list and writes the measured A3062 `06 81` sound-mode, `03 87` equalizer, one-frame settings and `0B 82`/`0B 81`/`0B 83` dual-connection bodies, plus the measured A3954 scale, 114-byte equalizer and setting frames; the Windows RFCOMM transport lists paired devices and connects by vendor UUID with an SPP fallback |
| Desktop (OpenSCL4 2.0) | OpenSCL4 Slint shell vendored under GPL-3.0; Add Device populated from the 39-model catalog; real paired Soundcore devices connect over Windows RFCOMM and Overview shows their battery (P20i A3949 and Space One Pro A3062 verified on hardware); the Space One Pro also gets the Sound mode, Equalizer and remaining-settings pages (Controls, Comfort, Power and prompts, Sound and effects, Connections, About), editable custom bands, saved equalizer profiles that persist across restarts, equalizer profile import and export, the dual-connections remembered-device list (names and connected marker, per-row Connect/Disconnect/Forget writes that re-read the list), and a Diagnostics page with read-only probes, a local redacted ZIP and a prefilled GitHub issue. The Liberty 4 Pro (A3954) gets its scale, equalizer and settings pages through the same shell and is accepted on the real device (see the A3954 desktop-pages row). The shared UI kit and its agent workflow are documented; the connected card exposes app-level Disconnect, and a serial read-only idle heartbeat reports Lost without automatic reconnect. **The finished client is accepted end to end on the real Space One Pro (A3062, firmware 04.39)** — RFCOMM Add Device connection, every implemented page, sound mode/equalizer/preset/custom/profile writes with `01 01` re-polls and restores, one settings write per section page with restore, Settings persistence, copy logs, tray icon/close-to-tray/Quit, Diagnostics save-ZIP redaction and prefilled issue, and idle heartbeat with Lost/Retry; that run also found and fixed the manifest probe-response serial leak |
| Desktop tray | notification-area icon and menu from the vendored `ui/tray.slint`: the tooltip names the device and, while connected, its battery (the vendored tooltip has no status word; status is the menu's device line); menu status, battery, Sound mode submenu (writes and re-reads like the page), LDAC, Connect while a saved device is disconnected, Open and Quit; close-to-tray while the icon is shown; single instance (a second start reveals the hidden window); Settings “Desktop and quick access” switches and Quit; no quick presets. The A3954's 1–11 scale has no menu entries, so its Sound mode submenu stays hidden and its LDAC item reads the A3954 state. Tray choices persist atomically in `%APPDATA%\OpenSCL4\desktop-preferences.json` and load before icon creation |
| Android UI | not started |

## Build and test

Core:

```powershell
cd core
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Desktop (Rust + Slint, depends on `core/crates/models` and `core/crates/session` by path):

```powershell
cd desktop
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build
```

## Layout

```text
core/
  data/
    conformance.json      # 503 sanitized frames
    a3949-state.json      # real capture, serial masked, checksum recalculated
    a3062-state.json      # real capture, serial masked, checksum recalculated
    a3062-sound-modes.json # real captures, one per sound mode, serial masked
    a3062-eq-presets.json  # real captures, one per switched preset, serial masked
    a3062-eq-custom.json   # real custom-write captures (found/custom/restored), serial masked
    a3062-dual-connections.json # real 0B 01 list capture, names/addresses masked
    a3954-state.json      # real 165-byte capture, earbud/case serial masked, checksum recalculated
    a3954-sound-modes.json # real 06 81 scale writes (normal 6, transparency 1-5, ANC 7-11)
    a3954-eq-presets.json # real Soundcore-app 114-byte 03 87 bodies (Rock, Signature)
  crates/
    protocol/             # framing, checksum, request builders
    models/               # registry, catalog taxonomy and state parsers
    profiles/             # saved equalizer profile model, validation, schema v1 and transfer JSON
    session/              # transport trait, frame assembly, state polling
    diagnostics/          # read-only probes, session manifest, A3062/TWS serial redaction and ZIP export
android/                  # Kotlin/Compose app (next)
desktop/                  # Slint shell vendored from OpenSCL4 (GPL-3.0)
  build.rs                # compiles ui/app.slint and the separate ui/tray.slint unit
  src/main.rs             # shell wiring: window state and Add Device callbacks
  src/controller.rs       # UI actions, Bluetooth workers, battery mapping, settings, tray, diagnostics and saved profiles
  src/catalog_adapter.rs  # core catalog -> Add Device models, chips and art
  src/transport_windows.rs # paired devices, SDP services and RFCOMM transport
  src/tray.rs             # notification-area icon, menu and close-to-tray lifecycle
  src/single_instance.rs  # Windows named mutex/event; a second start shows the hidden window
  src/test_support.rs     # tests-only shared Slint thread for component tests
  ui/                     # vendored OpenSCL4 Slint tree; only the changes SOURCE.md records
  ui/pages/diagnostics.slint # Diagnostics page built on the vendored shell components
.scratch/
  openscl4-2.0/            # spec and tickets for this 2.0 slice
  a3954-settings/          # spec, tickets, live UI script and evidence for the Liberty 4 Pro settings slice
  windows-mvp/             # spec and tickets for the Windows MVP
  windows-sound-modes/     # spec and tickets for the Windows sound-modes slice
  windows-eq-presets/      # spec and tickets for the Windows equalizer-presets slice
  windows-eq-custom/       # spec and tickets for the Windows custom-equalizer slice
  windows-eq-profiles/     # spec and tickets for the Windows saved-equalizer-profiles slice
  windows-settings-import/ # spec and tickets for the Windows remaining-settings and profile-import slice
  windows-dual-tray/       # spec and tickets for the Windows dual connections and tray slice
  windows-diagnostics/     # spec and tickets for the Windows diagnostics page
  windows-eq-export/       # spec and tickets for the Windows equalizer-profile export slice
```

The desktop crate depends on `core/crates/models` and `core/crates/session` by
path (`soundcore-models`, `soundcore-session`) and on `core/crates/profiles`
(`soundcore-profiles`) for the saved-profile model; its tests also use
`soundcore-protocol` for the frame checksum, and the diagnostics crate stays
core-side.

## Boundaries

- No serial numbers, MAC addresses or raw captures in commits: a real capture is
  committed only as a sanitized fixture, with names and addresses replaced by
  neutral values and the checksum recalculated (as the `core/data/*.json`
  fixtures do); raw traffic logs are never committed.
- Core: protocol and preset data (command/response layouts, state offsets, preset tables and labels) adapted from `OpenSCL4/lib` under GPL-3.0 with attribution in `NOTICE`; the Rust implementation is this project's own.
- UI: OpenSCL4 Compose/Slint files may be ported under GPL-3.0 with copyright headers and `NOTICE`, per ADR 0004.
- No AI features, no vendor hints, no OTA.

## License

GPL-3.0-or-later. See `LICENSE`.
