# OpenSCL4 UI screenshots

This folder captures the finished Windows client in a deterministic, isolated
demo environment. It never opens a Bluetooth connection, never writes to a real
device and never reads the user's OpenSCL4 profile.

## Requirements

- Windows 10/11 with UI Automation.
- PowerShell 7.
- Rust toolchain and a built `desktop/target/debug/soundcore-desktop.exe`.
- Python 3 with Pillow (`python -c "import PIL"`).
- `desktop/translations/ru/.../soundcore-desktop.po` must give every `@tr`
  string a **non-empty** translation; the script checks this before launching the
  app (the same rule as `desktop/tests/translations.rs`, not just msgid presence).

## Run

From the repository root:

```powershell
# Full matrix, rebuild first, generate sheets:
pwsh tools/ui-capture/capture-ui.ps1 -Build -Offline

# Quick smoke run against an already-built executable:
pwsh tools/ui-capture/capture-ui.ps1 -Themes light -Scales 100 -Pages overview,equalizer -Dialogs none
```

Useful parameters:

| Parameter | Meaning |
|---|---|
| `-Build` / `-Offline` | rebuild the app before the run; pass `--offline` to Cargo |
| `-Themes light,dark` | themes to capture |
| `-Scales 100,110,125,150` | content scales (the app's Settings scale) |
| `-Pages all\|overview,equalizer,...` | page filter |
| `-Dialogs none\|core\|all` | key dialogs; `core` is Add Device, layout, quick toggles, save profile, import |
| `-OutputRoot artifacts/ui-screenshots` | parent folder; a timestamped child is created |
| `-SkipSheets` | raw PNGs only |
| `-KeepApp` | leave the last app running for debugging |
| `-IncludeTitleBar` | accepted for future use; raw captures are client-only |
| `-Force` | allow `-Build` to stop an already-running app |

Screenshots are **Russian only**: the isolated profile starts with
`language: "ru"` and a Russian window title. If the UI were English because of a
missing or empty translation, the run fails in the preflight. The preflight
reports every missing or empty source in one list, so a newly added runtime
message is visible before the app can start.

## Output

```
artifacts/ui-screenshots/<timestamp>/
  raw/<theme>/<scale>/<page>.png
  sheets/by-theme-scale/<theme>-<scale>.jpg
  sheets/by-theme-scale/dialogs-<theme>-<scale>.jpg
  sheets/by-page/<page>.jpg
  sheets/by-area/<area>.jpg
  sheets/by-flow/<theme>-<scale>.jpg
  index.html
  manifest.json
  run.log
  appdata/<theme>-<scale>/OpenSCL4/...
```

`index.html` is the easiest entry point. It contains the sheets and any capture
errors. `../index.html` and `../latest.txt` point at the newest run.

## Grouping choices

- **theme × scale** (default, recommended first) — one contact sheet per visual
  state; quickest way to spot clipping, contrast and type-size problems.
- **page** — one sheet per page across all themes/scales; best for regressions
  and diff review.
- **area** — Sound / Device / App / Dialogs; best for design consistency review.
- **flow** — captures in user-journey order; useful for a UX walkthrough, not for
  state comparison.

The script writes all four groups. Disable individual groups only by editing the
script if a much smaller run is needed; the quick smoke run already limits
themes/scales/pages.

## Notes

- The app is launched with a temporary `APPDATA` under the run folder, so
  language, theme and scale do not touch the user's `%APPDATA%\OpenSCL4`.
- Demo mode is used for the device pages; Diagnostics therefore shows the
  demo-unavailable state. Real-device screenshots are in
  `desktop/live-*.png`.
- Raw captures are client-area PNGs made with `PrintWindow(..., PW_RENDERFULLCONTENT)`
  after `SetProcessDPIAware`; the window is resized only when `-ClientWidth` and
  `-ClientHeight` are passed.
- The UIA code lives in `uia.ps1`; page names and the Russian button map live in
  `labels.ru.json`. Update both when a page or label changes.
