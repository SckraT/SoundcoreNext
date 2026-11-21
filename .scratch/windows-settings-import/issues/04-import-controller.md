# 04 — Profile import controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 03

## Goal

Wire the vendored Import dialog with paste and file input, preview and confirm.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only for clipboard/file-dialog crates (arboard already in lock; rfd allowed)

## Requirements

1. `Actions.eq-import-paste`: read clipboard text; empty → `transfer-note=ClipboardEmpty`; parse → preview.
2. `Actions.eq-import-file`: open Windows file dialog filtered to JSON; reject >1 MiB (`TooLarge`), read failure (`ReadFailed`), parse.
3. Preview: set `importing=true`, `import-items` with name/bands/status/export=false, `import-blocked` when any status ≥2, `transfer-note` per result; unparseable → `NotProfiles`.
4. `Actions.eq-import`: re-validate all-or-nothing; merge/replace into saved profiles, persist via existing save path, set `transfer-note=Imported`/`toast=Imported`, close dialog, refresh list; blocked import writes nothing.
5. `eq-legacy` stays empty; no migration/export actions.
6. Tests: paste empty/valid/blocked, file too large/read fail (file dialog seam), preview statuses, confirm merge/persist, blocked no-op, transfer notes.
7. Hardware: import a file/paste with a real profile, apply it, delete; restore user document.

## Acceptance

- [ ] build/test/fmt/clippy pass.
- [ ] Real import and apply verified.
- [ ] No `.slint` edits; no legacy.
