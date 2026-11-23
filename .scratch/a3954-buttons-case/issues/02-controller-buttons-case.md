# 02 — Controller: Controls and Case pages

**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Wire the A3954 button and case settings into the desktop UI.

## Requirements

1. Controls page:
   - Add a Presses panel with 8 select rows (left/right single, double, triple,
     long), each listing the measured actions plus None; current action from
     state.
   - Add a slide panel (or gestures panel) with 4 select rows for slide up/down,
     options volume up/down only.
   - Keep the ambient cycle panel.
   - Map `set-option` row ids to the core button command; re-poll and refresh.
   - Add `PanelKind.gestures` title/icon in the UI kit if used.
2. Case page:
   - Add `Page::case` to A3954 device pages when the case blocks are present.
   - Case-screen panel: four toggles (atmospheric, remote camera, find device,
     case spatial audio) writing `07 87`.
   - Case language select (Japanese/English/Chinese) writing `07 8A`.
   - Case status panel: case battery/firmware from state; no serial/address.
   - The vendored section page already has the case battery visual and note.
3. Localization: new row labels/options via `LocalizedText`/`@tr`; Russian
   translations added. Use Theme tokens/alignment (no manual y).
4. Overview/tray: ensure nothing breaks; optionally add a Case shortcut only if
   the existing design supports it.
5. Tests: each page's rows and values, command body/worker dispatch, state
   refresh, no A3062 regressions; no hardware writes in tests.
6. Gates: desktop/core fmt/build/test/clippy.

## Acceptance

- Controls and Case pages populate from the A3954 fixture and every row writes
  one measured frame; Russian UI complete.
