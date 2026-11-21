# 02 — Saved profiles controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Persist profiles and wire save/apply/delete to the vendored Equalizer page.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only for path dependency on the profiles crate (no new runtime crates needed)
- no `.slint` edits

## Requirements

1. File location: `%APPDATA%\OpenSCL4\equalizer-profiles.json` (or `directories`-free std env); create dir, atomic write (tmp+rename). Load at startup; missing/invalid file → empty profiles, log to stderr, no crash.
2. A3062: `has-saved-profiles=true`; `eq-profiles` list with name, `curve` path from bands, `bands=8`, `in-effect` when current custom bands equal profile.
3. Save dialog (`eq-saving`):
   - `Actions.eq-save(name, replace)`:
     - empty/whitespace name → `eq-save-problem=1`;
     - existing name without replace → `eq-save-problem=2` + `eq-save-existing=name`;
     - otherwise save current custom bands, persist, update list, close dialog (`eq-saving=false`), mark in-effect;
   - `Actions.eq-save` with replace true replaces bands.
4. `Actions.eq-apply(name)`: resolve profile, write custom `03 87`, re-poll, update bands/chart/in-effect; readable error.
5. `Actions.eq-delete(name)`: remove after `eq-deleting` confirmation, persist, update list.
6. Import/export/legacy fields stay empty/inert: `eq-export-count=0`, `eq-legacy=[]`; no file/clipboard actions.
7. A3949 deferred: no page, no writes.
8. Tests: profile load/save atomicity (temp dir), duplicate/empty/invalid, apply writes one curve, in-effect matching, delete persists, malformed file tolerated.
9. Real A3062: save a custom profile, apply, delete; restart persistence checked in acceptance.

## Acceptance

- [ ] desktop/core gates pass.
- [ ] Real save/apply/delete verified.
- [ ] No `.slint` edits; import/export inert.
