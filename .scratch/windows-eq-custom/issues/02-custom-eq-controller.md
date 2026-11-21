# 02 — Custom equalizer controller

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Enable editing in the vendored Equalizer page and wire commits.

## Files

- `desktop/src/controller.rs`
- no `.slint` edits

## Requirements

1. For A3062: `eq-read-only = false`; keep 8 visible bands, min/max, preset chips.
2. Wire `Actions.eq-preview(index, value)`: update local `eq-bands`/chart immediately (no write).
3. Wire `Actions.eq-commit(index, value)`: clamp, write custom body via session, re-poll, update state; readable error on failure.
4. Wire `Actions.eq-type(index, text)`: parse integer then same commit path.
5. Wire `Actions.eq-reset()`: set all visible bands to 0 dB, write custom flat, re-poll; update.
6. Update `eq-preset`/`eq-name` to custom (`0xFEFE` → empty name/“Your own settings”), regenerate `eq-chart-line/area` from bands.
7. Built-in preset selection must still work and switch back from custom.
8. P20i/A3949: no page/no writes.
9. Tests for band conversion/clamping, preview no-write, commit one write, reset flat, custom state mapping, error mapping.
10. No profiles/import-export/HearID.

## Acceptance

- [ ] build/test/fmt/clippy pass.
- [ ] Real A3062: editable bands, preview, commit, re-poll, reset, preset switch.
- [ ] No `.slint` edits.
