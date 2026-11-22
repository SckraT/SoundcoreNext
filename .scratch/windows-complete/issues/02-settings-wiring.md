# 02 — Settings wiring

**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Wire and persist every control on the Settings page.

## Requirements

1. Load `DesktopSettings` at startup; keep the document at
   `%APPDATA%\OpenSCL4\desktop-preferences.json` (atomic tmp+rename).
2. Implement handlers for all `Actions`:
   - `set-app-theme`, `set-palette`, `set-device-color`, `set-mode-view`,
     `set-content-scale`, `set-badge`, `set-language`, `copy-logs`.
3. Apply settings to `AppState` and the Slint globals:
   - `Theme.dark`, `Theme.palette`, `Theme.content-scale`;
   - device art in the selected silver/black color through
     `catalog_adapter::art_in_color`, including the current device and cards;
   - `app-mode-view-applies` and `one-earbud-badge-applies` from the connected
     state;
   - `language-names` (Follow system + bundled languages) and `app-language`.
4. Language: `slint::select_bundled_translation` with the chosen code or the
   system locale; update after window is shown; fallback English safely.
5. Follow-system theme reads Windows app mode and reacts after the native window
   is shown; no crash if the window/theme is unavailable.
6. Copy logs puts `logs::recent()` on the clipboard and shows the Copied toast;
   failures are logged, never fatal.
7. Tray choices persist through the same document, not a second file.
8. Tests: choice mapping, index validation, persistence roundtrip, light/dark and
   language selection, missing/malformed document, copy logs (through a seam).
9. Do not change device write behavior.

## Acceptance

- [ ] Settings changes take effect immediately and survive restart.
- [ ] Language and theme apply to the running window.
- [ ] Copy logs works.
- [ ] Desktop/core gates green.
