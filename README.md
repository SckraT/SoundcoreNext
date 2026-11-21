# Soundcore Next

Open-source Soundcore control app for Android, Windows and Linux.

**OpenSCL4 2.0 development.** This branch is the first OpenSCL4 2.0 slice: the
desktop product keeps the OpenSCL4 name, and its UI is vendored from OpenSCL4
under GPL-3.0 — the window title, screens and strings stay unchanged. Protocol
command/response data and device preset data are adapted from OpenSCL4 under
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
| A3949 equalizer | deliberately deferred: the committed A3949 state carries an equalizer block at body 32..44, but its write body is unverified, so the page is hidden and no `03 87` is sent |
| A3954 100-step state | synthetic test, battery 100/75 |
| Other single-battery layouts | intentionally unimplemented (`UnsupportedLayout`) |
| TLV D1203 | not started |
| Diagnostics | read-only probes, session manifest, serial/MAC redaction, ZIP export |
| Windows session/transport | session requests `01 01`, parses the answer and writes the measured A3062 `06 81` sound-mode and `03 87` equalizer frames; the Windows RFCOMM transport lists paired devices and connects by vendor UUID with an SPP fallback |
| Desktop (OpenSCL4 2.0) | OpenSCL4 Slint shell vendored under GPL-3.0; Add Device populated from the 39-model catalog; real paired Soundcore devices connect over Windows RFCOMM and Overview shows their battery (P20i A3949 and Space One Pro A3062 verified on hardware); the Space One Pro also gets the Sound mode and Equalizer pages |
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
    a3062-eq-presets.json # real captures, one per switched preset, serial masked
  crates/
    protocol/             # framing, checksum, request builders
    models/               # registry, catalog taxonomy and state parsers
    session/              # transport trait, frame assembly, state polling
    diagnostics/          # read-only probes and session manifest
android/                  # Kotlin/Compose app (next)
desktop/                  # Slint shell vendored from OpenSCL4 (GPL-3.0)
  src/main.rs             # shell wiring: window state and Add Device callbacks
  src/controller.rs       # UI actions, Bluetooth workers and battery mapping
  src/catalog_adapter.rs  # core catalog -> Add Device models, chips and art
  src/transport_windows.rs # paired devices, SDP services and RFCOMM transport
  ui/                     # vendored OpenSCL4 Slint tree, not edited
.scratch/
  openscl4-2.0/           # spec and tickets for this 2.0 slice
  windows-mvp/            # spec and tickets for the Windows MVP
  windows-sound-modes/    # spec and tickets for the Windows sound-modes slice
  windows-eq-presets/     # spec and tickets for the Windows equalizer-presets slice
```

The desktop crate depends on `core/crates/models` and `core/crates/session` by
path (`soundcore-models`, `soundcore-session`); the protocol and diagnostics
crates stay core-side.

## Boundaries

- No serial numbers, MAC addresses or raw captures in commits.
- Core: protocol and preset data (command/response layouts, state offsets, preset tables and labels) adapted from `OpenSCL4/lib` under GPL-3.0 with attribution in `NOTICE`; the Rust implementation is this project's own.
- UI: OpenSCL4 Compose/Slint files may be ported under GPL-3.0 with copyright headers and `NOTICE`, per ADR 0004.
- No AI features, no vendor hints, no OTA.

## License

GPL-3.0-or-later. See `LICENSE`.
