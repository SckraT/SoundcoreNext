# Spec: OpenSCL4 2.0 — Windows equalizer profile export

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-eq-export/`
**Integration branch:** `integration/windows-eq-export`
**Depends on:** import (`master` 7474400)

## Goal

Export selected saved equalizer profiles in OpenSCL4 transfer format, via file or clipboard, from the vendored Import and export panel. No legacy.

## In scope

- Core JSON serialization matching the import format (`name` + 8 `volumeAdjustments` dB values with one decimal), roundtrip with `preview_import`.
- Controller: profile `export` flags, `eq-export-count`, select all/one, `eq-export-one`/`eq-export-choose`, `eq-export-file` (save dialog + default filename), `eq-export-copy` (clipboard + toast), transfer notes.
- A3062 only; A3949 deferred.

## Out of scope

- Legacy migration; export for other models; Android/Linux.
- New UI.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-export-core.md` | — | Transfer JSON serializer + roundtrip tests |
| `02-export-controller.md` | 01 | Selection/count/file/copy wiring + tests |
| `03-acceptance.md` | 02 | Real A3062 export file/copy/roundtrip, screenshot |

## Acceptance criteria

1. Gates pass.
2. Saved profiles can be selected individually or all; count enables Save/Copy.
3. Save writes a JSON file (default `openscl4-equalizer-profiles.json`) with selected profiles; cancel does nothing; write failure shows `file-save-failed`.
4. Copy puts the same JSON on the clipboard and shows the Copied toast.
5. Exported JSON re-imports via the existing import path with New/Replaces statuses and matching bands.
6. One-click row export selects that profile (selection reflected in UI).
7. No legacy/export for A3949; no `.slint` edits; no MAC/serial.

## Closure

Ticket closed when acceptance passes and `Status: done`; merge to `master` after ticket 03 and `code-review`.
