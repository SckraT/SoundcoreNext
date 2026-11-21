# 02 — Profile export controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Wire export selection and file/clipboard actions in the vendored panel.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only if a save-dialog API is needed (`rfd` already present)

## Requirements

1. `eq-profiles.export` reflects controller selection; `eq-export-count` = selected count; select-all/row toggles call `eq-export-choose(name|"", checked)`.
2. `eq-export-one(name)`: select only that profile (others false), update count; UI reflects.
3. `eq-export-file`: save dialog (default `openscl4-equalizer-profiles.json`), JSON of selected via core; `file-saved` note + file-saved toast; cancel → no note; write failure → `file-save-failed`.
4. `eq-export-copy`: clipboard selected JSON; `copied` toast; no note needed.
5. Import/legacy remain; no legacy; A3949 no export.
6. Tests: selection/count, one-click selection, exact file contents, cancel, write failure note, clipboard text, no selection no-op, A3949.
7. Hardware: export one profile via file and clipboard, re-import and compare.

## Acceptance

- [ ] desktop/core gates pass.
- [ ] Real export/re-import verified.
- [ ] No `.slint` edits; no legacy.
