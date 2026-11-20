# Soundcore Next

Clean-room implementation of a Soundcore control app for Android, Windows and Linux.

- **Core** (`core/`) — Rust: protocol framing, model state parsers, diagnostics. Written from the specification in the research wiki, without copying OpenSCL4 protocol code.
- **Android** (`android/`) — planned Kotlin/Compose UI; the OpenSCL4 UI may be reused and adapted under GPL-3.0 (ADR 0004).
- **Desktop** (`desktop/`) — planned Rust + Slint UI; the OpenSCL4 UI may be reused and adapted under GPL-3.0.

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
| A3954 100-step state | synthetic test, battery 100/75 |
| Single-battery layouts | intentionally unimplemented (`UnsupportedLayout`) |
| TLV D1203 | not started |
| Diagnostics | read-only probes, session manifest, serial/MAC redaction, ZIP export |
| Desktop UI | OpenSCL4 Slint shell vendored under GPL-3.0; opens with empty state, not wired to core yet |
| Android UI | not started |
| Desktop UI | not started |

## Build and test

```powershell
cd core
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
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
```

## Boundaries

- No serial numbers, MAC addresses or raw captures in commits.
- Core: no copying from `OpenSCL4/lib`; protocol facts come from the wiki.
- UI: OpenSCL4 Compose/Slint files may be ported under GPL-3.0 with copyright headers and `NOTICE`, per ADR 0004.
- No AI features, no vendor hints, no OTA.

## License

GPL-3.0-or-later. See `LICENSE`.
