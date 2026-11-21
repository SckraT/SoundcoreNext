# 02 — Equalizer presets controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Populate the vendored Equalizer page with the A3062 preset catalog and wire preset selection.

## Files

- `desktop/src/controller.rs`
- no `.slint` edits

## Requirements

1. On connect to A3062, set:
   - `eq-available = true`, `eq-read-only = true`;
   - `eq-preset-count`, `eq-presets` from the core catalog (values = preset ids as strings, labels = names);
   - `eq-preset` = current preset id string;
   - `eq-band-count = 10`, `eq-min`, `eq-max`, `eq-min-text`, `eq-max-text`;
   - `eq-bands` from current state (frequency labels 31…16k, `khz`, value, text) — read-only;
   - `sound-pages` includes `Page.equalizer` in addition to sound-modes;
   - no `eq-profiles`/`eq-legacy`/import/export content.
2. Handle `Actions.eq-preset(value)`:
   - resolve id; write the preset via session; re-poll; update `eq-preset`, `eq-bands`;
   - readable `error-label` on failure; ignore unknown ids.
3. P20i: no Equalizer page, no `03 87` path.
4. Custom band callbacks (`eq-commit`/`eq-preview`/`eq-type`/`eq-reset`) are inert; `eq-read-only=true` keeps the UI hidden.
5. Tests: preset id/name mapping, band labels/values, page exposure A3062 vs A3949, error mapping, no custom writes.
6. No strength/profiles/import-export.

## Acceptance

- [ ] Build/test/fmt/clippy pass.
- [ ] Real A3062: page shows current preset and bands; changing preset writes and updates.
- [ ] P20i no page/no command.
- [ ] No `.slint` edits.
