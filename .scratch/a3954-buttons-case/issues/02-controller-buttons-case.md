# 02 — Controller: Controls and Case pages

**Status:** done
**Blocked by:** 01
**Implemented:** 2026-11-23

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

## Implementation (2026-11-23)

### Controller (`desktop/src/controller.rs`)

- Exhaustive matches: `SessionError::UnsupportedButtonConfiguration` maps to
  `Unreadable` (the four command failure kinds) and `Protocol` (connect); the
  ignored write-only reasons are documented in their comment.
- `a3954_setting_label` names the two new commands `Case screen` and
  `Case language`, so a failed case write's banner names the row.
- Device pages: `device_pages` appends `Page::Case` for the A3954 between
  Connections and About (its state always parses the measured case
  features/language, so the block is present); the A3062 keeps its five pages.
  The vendored Case visuals, including `AppState.case-battery` (the measured
  case level), already existed and now have a page to render on.
- Controls: `PanelKind::Presses` with the eight measured press rows
  (`leftSinglePress`, `rightSinglePress`, `leftDoublePress`, `rightDoublePress`,
  `leftTriplePress`, `rightTriplePress`, `leftLongPress`, `rightLongPress`),
  `PanelKind::Gestures` (titled "Slides") with the four slide rows
  (`leftSlideUp`, `rightSlideUp`, `leftSlideDown`, `rightSlideDown`), and the
  existing ambient-mode cycle panel. Press options are `None` plus
  `A3954_COMMON_ACTIONS`; slide options are the two `A3954_SLIDE_ACTIONS`. The
  shown index is the state's TWS nibble
  (`A3954State::button_action_byte(..).tws()`); an unmeasured nibble stays
  unselected rather than showing an action the device did not report.
- Gesture writes: a gesture `set-option` queues
  `DeviceCommand::SetA3954Button` carrying
  `A3954ButtonActionByte::with_tws(value)`, so the disconnected-state nibble the
  device reported is written back unchanged. The worker calls
  `Session::set_a3954_button` once, re-polls `01 01`, and reports
  `Event::A3954ButtonChanged`; the UI replaces its state copy and refills the
  pages, or names the row in the banner. One click is one measured `04 81`
  frame and one re-poll.
- Case page: `PanelKind::CaseScreen` has the four measured toggles
  (`atmospheric`, `remoteCamera`, `findDevice`, `caseSpatialAudio`), each
  writing `A3954SettingCommand::CaseFeatures` with the other three flags kept,
  plus the `caseLanguage` select (`Japanese`/`English`/`Chinese` →
  `A3954CaseLanguage`, `07 8A`). `PanelKind::CaseStatus` shows the read-only
  case battery (the measured step + 1 of 10) and case firmware; the case serial
  never reaches the page. Case writes reuse the existing
  `SetA3954Setting`/`A3954SettingChanged` path and `01 01` re-poll.
- The A3062 Controls page, its row ids and its command builders are untouched;
  only the A3954 branch gained rows.

### UI and localization

- `desktop/ui/components/localized.slint`: the twelve side-specific gesture
  names, the four case-feature ids, `caseLanguage`, the gesture action options
  and the three case languages. The obsolete `buttonRemapping` /
  "Not available in this version" mappings are removed.
- `desktop/ui/components/settings.slint`: `PanelText` gains the `Slides` title
  and wave icon for `PanelKind.gestures`; `SettingText.short-label` no longer
  shortens the side-specific gesture ids, because this page shows both earbuds
  and each row names its side (the A3062's side-less wording is unchanged).
- `desktop/translations/ru/...po`: Russian translations for every new `@tr`
  string; the capture tool's completeness check finds no gap.
- `desktop/ui/UI_KIT.md` and `SOURCE.md` record the page/label contract and the
  vendored-UI change.
- `.scratch/ui-preview` renders the A3954 Controls and Case pages (light/dark,
  800×700…1440×900, 100/125/150 %); the 1280×820 light and dark and the
  800×700 150 % captures were inspected with no clipping and no raw English.

### Tests

- `the_real_a3954_capture_exposes_the_scale_equalizer_and_settings_pages`:
  six device pages including `Page::Case`.
- `the_a3954_settings_pages_have_their_measured_rows`: Presses/Slides/Cycle row
  ids, kinds, options and the captured action indices; Case rows, flags,
  language index and status values.
- `every_a3954_gesture_row_writes_one_measured_frame_and_re_polls`: for every
  one of the twelve gestures — row-id round trip, every option index, the
  measured current index, and one `04 81` frame plus one `01 01` re-poll.
- `a_reported_a3954_gesture_and_case_refresh_the_pages`: re-polled gestures and
  case state refill the panels and the controller's copy.
- `a3954_row_ids_name_only_measured_setting_commands`: the four case toggles
  keep the other flags, all three languages map to `07 8A` bodies, and the
  banner names the case/new rows.
- `each_a3954_settings_page_writes_one_measured_frame_and_re_polls`: now
  fifteen measured commands including `07 87` and `07 8A`.
- `the_a3954_pages_queue_their_measured_commands_from_the_wired_actions`: a
  press row queues the whole action byte with its high nibble kept, a slide row
  keeps its nibble, case toggle/language queue the measured commands, and
  invalid ids/indexes write nothing.
- A3062 tests are unchanged and green.

### Verification

- desktop: `cargo fmt --all -- --check`, `cargo build`, `cargo test`
  **290 passed / 0 failed / 14 ignored** (the ignored are hardware tests),
  `cargo clippy --all-targets -- -D warnings` clean.
- core: `cargo fmt --all -- --check`, `cargo build`, `cargo test`
  **195 passed / 0 failed**, `cargo clippy --all-targets -- -D warnings` clean.
- No hardware writes and no live device were used; ticket 03 owns the live
  acceptance.
