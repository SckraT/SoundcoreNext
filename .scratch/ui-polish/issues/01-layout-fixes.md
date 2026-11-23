# 01 — Layout fixes

**Status:** ready-for-agent
**Blocked by:** —

## Requirements

1. Breadcrumb in `PageHeading` (`desktop/ui/shell.slint`): center the chevron by
   layout (`cross-axis-alignment: center`), remove its manual `y`; keep TextLink
   and section text aligned.
2. Remove `AsidePanel` "Diagnostics" from Settings; no duplicate entry point.
3. Add a compact `Copy logs` action to the Diagnostics page (e.g. support row at
   the bottom) wired to `Actions.copy-logs()`.
4. Remove now-unused imports; verify no button overflows the Settings aside at
   280–380px and narrow window.
5. Tests/visual check at 100/125/150%; update UI_KIT/SOURCE if needed.

## Acceptance

- Arrow centered; Settings has no Diagnostics card; Copy logs works from
  Diagnostics; no overflow.
