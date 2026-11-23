# 02 — Flow-based setting rows

**Status:** done
**Blocked by:** —
**Implemented:** 2026-11-23

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

## Implementation (2026-11-23)

- `desktop/ui/components/settings.slint`: `SettingRowView` now inherits
  `VerticalLayout` with `spacing: 0px`. The divider is the first flow child
  (`if !root.first: Divider`), replacing the absolutely placed 1px `Rectangle`
  at `y: 0px`; the non-list `min-height: 64px` is gone, so the row's height is
  the content's height. The content `HorizontalLayout` uses
  `cross-axis-alignment: center`, and the toggle, select, range/hue, info/copy,
  action and time controls (plus the range group's slider/value pair) no longer
  set a manual `y` and are centred by their layouts. The list row keeps its
  device cards, Connect/Disconnect/Forget actions and eliding name, and remains
  an unconditional flow child (empty and 0px tall for the other kinds) so the
  panel's note always follows it.
- No callback, row kind, device write, controller or model changed. The
  `SettingText`/`PanelText`/`SettingsPanelView` APIs are unchanged.
- `desktop/ui/UI_KIT.md` and `GUIDE.md` record the rule: a row's divider stays in
  normal flow with its content, a non-list row takes its height from the content
  (no fixed minimum height), and composed rows centre children with
  `cross-axis-alignment` instead of a manual `y`. `SOURCE.md` records the
  vendored-tree change.
- `.scratch/ui-preview`: new `settings-rows-long` capture state
  (`long_panels`, `Capture.long_rows`) renders one row of every kind — toggle,
  select, range, info, action, time and a described toggle — with a label and a
  device reason long enough to wrap, plus a normal row followed by a list row.
  Captures at 800×700 and 1280×820 at 100% and 150% show the flow behaviour;
  `.scratch/make_ui_sheets.py` gets a `settings-rows-long-light` contact sheet.
  The harness also restores the A3062 identity for non-A3954 captures, so the
  long-row captures are not mislabelled by the preceding A3954 pages.

## Verification

- Inspected every settings panel at the matrix sizes/scales: Controls, Comfort,
  Power, Connections, About, Sound and effects, Settings, the A3954 Controls/Case
  pages and the new long-row state. Rows grow with wrapped text, dividers stay
  below their own row, and controls stay vertically centred in the row.
- No divider crossing at 800×700 (narrow, single column), 1280×820 (two-column at
  100%) and 150% content scale, light and dark. Raw captures and contact sheets:
  `.scratch/ui-validation/settings-rows-long-*.png` and
  `.scratch/ui-validation/sheet-settings-rows-long-light.jpg`.
- desktop: `cargo fmt --all -- --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`
  (292 passed / 0 failed / 15 ignored) green. Core untouched.
