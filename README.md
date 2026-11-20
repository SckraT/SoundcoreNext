# Soundcore Next

Clean-room implementation of a Soundcore control app for Android, Windows and Linux.

**OpenSCL4 2.0 development.** This branch is the first OpenSCL4 2.0 slice: the
desktop product keeps the OpenSCL4 name, and its UI is vendored from OpenSCL4
under GPL-3.0 — the window title, screens and strings stay unchanged.

- **Core** (`core/`) — Rust: protocol framing, model state parsers, diagnostics. Written from the specification in the research wiki, without copying OpenSCL4 protocol code.
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
| A3954 100-step state | synthetic test, battery 100/75 |
| Other single-battery layouts | intentionally unimplemented (`UnsupportedLayout`) |
| TLV D1203 | not started |
| Diagnostics | read-only probes, session manifest, serial/MAC redaction, ZIP export |
| Desktop (OpenSCL4 2.0) | OpenSCL4 Slint shell vendored under GPL-3.0; Add Device populated from the 39-model catalog, with category and search filters |
| Android UI | not started |

## Build and test

Core:

```powershell
cd core
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Desktop (Rust + Slint, depends on `core/crates/models` by path):

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
  crates/
    protocol/             # framing, checksum, request builders
    models/               # registry, catalog taxonomy and state parsers
    diagnostics/          # read-only probes and session manifest
android/                  # Kotlin/Compose app (next)
desktop/                  # Slint shell vendored from OpenSCL4 (GPL-3.0)
  src/main.rs             # shell wiring: window state and Add Device callbacks
  src/catalog_adapter.rs  # core catalog -> Add Device models, chips and art
  ui/                     # vendored OpenSCL4 Slint tree, not edited
.scratch/
  openscl4-2.0/           # spec and tickets for this 2.0 slice
```

The desktop crate depends on `core/crates/models` by path (`soundcore-models`);
the protocol and diagnostics crates stay core-side.

## Boundaries

- No serial numbers, MAC addresses or raw captures in commits.
- Core: no copying from `OpenSCL4/lib`; protocol facts come from the wiki.
- UI: OpenSCL4 Compose/Slint files may be ported under GPL-3.0 with copyright headers and `NOTICE`, per ADR 0004.
- No AI features, no vendor hints, no OTA.

## License

GPL-3.0-or-later. See `LICENSE`.
