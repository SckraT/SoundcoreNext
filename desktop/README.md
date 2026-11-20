# OpenSCL4 2.0 desktop shell

The desktop app is the **OpenSCL4 2.0 desktop shell**: the product keeps the
OpenSCL4 name, and the UI is **vendored from OpenSCL4** under GPL-3.0; it is not
a new design. The Slint tree in `ui/` comes from
`D:\Projects\OpenSCL4\desktop\ui` at commit `01276e08` and keeps the upstream
layout, components, pages, icons, fonts and art.

## Status

- The shell compiles and opens; the window title still says OpenSCL4.
- The vendored **Add Device** dialog is populated from the clean-room catalog
  (`core/crates/models/src/catalog.rs`) through `src/catalog_adapter.rs`: all
  39 supported models as cards with their name, code and device art.
- The existing category chips filter by form factor: **All**, **Earbuds**
  (TWS and sleep buds — the vendored UI has no Sleep chip), **Over-ear**,
  **Open-ear** and **Speakers**. The search field filters by model name or code.
- Choosing a model advances the existing dialog to its paired-device step with
  that model's name, code and art. No transport, pairing or connection is
  attempted yet.
- Pages for a connected device still open in their placeholder state, which is
  expected until the desktop controller replaces the OpenSCL4 library adapter.

## Screenshot

![Add device](add-device.png)

The Add Device dialog with the catalog populated, captured from the running app
on Windows.

## Run

```powershell
cd desktop
cargo run
```

## License and attribution

GPL-3.0-or-later. The vendored files keep their upstream copyright; see
`ui/SOURCE.md` and the root `NOTICE`. Changes to vendored files are recorded in
the commit that makes them.
