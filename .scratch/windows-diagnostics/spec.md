# Spec: OpenSCL4 2.0 — Windows diagnostics page

**Status:** done
**Tracker:** local Markdown in `.scratch/windows-diagnostics/`
**Branch:** `feature/windows-diagnostics`
**Prototype:** `D:\Pixel 8 Pro\Soundcore\wiki\docs\mockups\desktop-diagnostics.html`

## Goal

Bring the reviewed diagnostics prototype into the Windows app: a standalone
Diagnostics page with three read-only probes, a local redacted ZIP and a
prefilled GitHub issue. Nothing is uploaded by the app.

## In scope

- Session `probe_frame` for read-only requests.
- Controller diagnostics state and worker command.
- Diagnostics page, sidebar entry and Settings entry.
- ZIP save dialog, issue URL, optional note.
- Read-only, serial/MAC redaction from the core.

## Out of scope

- Unsupported-device catalog/selection flow.
- Automatic upload / GitHub API.
- Probes beyond state, battery and charging.
- Android.

## Tickets

| Ticket | Status | Outcome |
|---|---|---|
| `01-probe-core.md` | done | Session read-only probe + tests |
| `02-diagnostics-page.md` | done | UI/controller/worker/ZIP |
| `03-acceptance.md` | pending | Real A3062 capture and screenshot |

## Acceptance criteria

1. Diagnostics opens from Settings/sidebar without a device.
2. Connected A3062 runs 3 read-only probes and shows progress/statuses.
3. Review shows counts and archive size; Save ZIP builds the redacted archive.
4. Prefilled issue URL opens; nothing is uploaded automatically.
5. No MAC/serial in UI/docs; A3949 has no diagnostics worker.
6. Gates pass; hardware capture verified.
7. Adaptive checks at 800/1030/1280/1440.
