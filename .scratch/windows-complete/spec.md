# Spec: finish the OpenSCL4 2.0 Windows client

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-complete/`
**Integration branch:** `integration/windows-complete`
**Depends on:** `master` b2fd859 and the imported UI-kit WIP

## Goal

Finish the Windows desktop client that another developer left mid-way: make the
UI-kit branch build and pass its tests, wire every Settings choice and persistence
path, implement a working demo-device mode for no-hardware UI acceptance, and
then verify the finished client on a real Space One Pro (A3062).

## In scope

- Build recovery: translation domain and bundled translations, logs, tray/settings
  persistence, `Controller::shown`, compile and tests.
- Settings: theme, palette, content scale, device art color, sound-mode view,
  one-earbud badge, language, Copy logs; persisted in
  `%APPDATA%\OpenSCL4\desktop-preferences.json`.
- Demo device mode: Add Device → Demo mode creates a simulated A3062; the app
  connects to it without RFCOMM, all supported pages render and controls update
  local simulated state; no device commands are sent.
- Overview preferences, tray persistence, Disconnect, idle heartbeat and Lost/
  Retry behaviour exercised from demo and tests.
- UI-kit visual/adaptive acceptance at 800×700, 1030×760, 1280×820, 1440×900,
  100/125/150%, light/dark, with long strings.
- Real A3062 acceptance: connection, every implemented page, settings writes,
  EQ/custom/profiles, tray, diagnostics capture + redacted ZIP, heartbeat/Retry,
  restore device state.

## Out of scope

- Android Compose implementation.
- Unsupported/unknown-device diagnostics flow.
- A3949 equalizer/settings/diagnostics (still deferred).
- Replacing the vendored UI with a third-party framework.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-build-recovery.md` | — | Green build/tests; translations, logs, settings/tray wiring, `shown` |
| `02-settings-wiring.md` | 01 | Every Settings control persists and applies; Copy logs |
| `03-demo-device.md` | 01 | Simulated A3062 demo connect + interactive pages, no device I/O |
| `04-demo-acceptance.md` | 02,03 | Demo-only visual/adaptive/functional acceptance; fix defects |
| `05-live-acceptance.md` | 04 | Real A3062 acceptance of all Windows features |
| `06-review-fixes.md` | 05 | Code review findings fixed and final gates |

## Acceptance criteria

1. `cargo build` succeeds; desktop tests and core tests pass; fmt/clippy clean.
2. Settings page changes theme, palette, content scale, device art, sound-mode
   view, badge and language and survives restart; Copy logs puts the real recent
   log on the clipboard.
3. Demo mode adds a simulated A3062 without Windows pairing and opens its
   Overview; all implemented Sound/Device pages, EQ, profiles and Diagnostics
   entry states render; changing a setting updates the demo state without any
   RFCOMM write.
4. No demo path sends a device command or claims a real connection.
5. Visual matrix is clean at the required widths/scales/themes; no clipping.
6. Real A3062 acceptance passes for connect, modes, EQ, settings, tray,
   diagnostics ZIP redaction and heartbeat Lost/Retry; device left as found.
7. Code review has no unresolved hard findings.

## Closure

Each ticket is `Status: done` with evidence; the integration branch merges to
`master` after ticket 06.
