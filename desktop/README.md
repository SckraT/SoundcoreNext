# Desktop shell

The desktop UI is **vendored from OpenSCL4** under GPL-3.0; it is not a new
design. The Slint tree in `ui/` comes from
`D:\Projects\OpenSCL4\desktop\ui` at commit `01276e08` and keeps the upstream
layout, components, pages, icons, fonts and art.

## Status

- The shell compiles and opens.
- It is not wired to the clean-room core yet: the screen opens with empty
  state, which is expected until the desktop controller replaces the OpenSCL4
  library adapter.
- The grouped model catalog in `core/crates/models/src/catalog.rs` will feed
  the add-device list; it does not replace the OpenSCL4 screens.

## Run

```powershell
cd desktop
cargo run
```

## License and attribution

GPL-3.0-or-later. The vendored files keep their upstream copyright; see
`ui/SOURCE.md` and the root `NOTICE`. Changes to vendored files are recorded in
the commit that makes them.
