# 02 — Flow-based setting rows

**Status:** ready-for-agent
**Blocked by:** —

## Requirements

1. Change `SettingRowView` (`desktop/ui/components/settings.slint`) from a
   `Rectangle` with an absolute divider to a `VerticalLayout`:
   - divider is the first child in flow (`if !first: Divider`);
   - content row follows, height from content;
   - remove non-list `min-height: 64px`;
   - remove child `y` in the content row; use `cross-axis-alignment: center`.
2. Keep list-row and all control kinds working.
3. Check every settings panel for regressions (Effects/Comfort/Power/Controls/
   Case/About/Settings).
4. Add long-description/reason fixture or preview state; verify at 800×700,
   1280×820, 150% that text never crosses a divider.
5. Document the rule in `UI_KIT.md`/`GUIDE.md`.

## Acceptance

- Rows grow with wrapped content; no overlap; all gates green.
