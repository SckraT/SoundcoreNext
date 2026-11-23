# Spec: Liberty 4 Pro button remapping and case settings

**Status:** ready for implementation
**Tracker:** `.scratch/a3954-buttons-case/`
**Integration branch:** `feature/a3954-buttons-case`
**Based on:** master `d2894d8` (A3954 settings slice)

## Goal

Finish the deferred A3954 surfaces: remap the twelve Liberty 4 Pro gestures
(left/right single, double, triple, long, slide up, slide down) and expose the
case controls the device actually supports (case-screen features, case language,
and case status), with live verification on the connected real device.

## Evidence

- A3954 state already parses raw button bytes and case features/language/battery
  in `soundcore-models`.
- OpenSCL4 reference: `D:\Pixel 8 Pro\Soundcore\.patchgen\openscl4-work\lib\src\devices\soundcore\a3954.rs`
  (`BUTTON_CONFIGURATION_SETTINGS`, `SLIDE_UP_ACTIONS`, `SLIDE_DOWN_ACTIONS`),
  `a3954/modules/case_features.rs`, `case_language.rs`;
  `common/packet/outbound/set_button_configuration.rs` and
  `common/modules/button_configuration/*`.
- Real A3954 capture / prior acceptance: firmware 04.29, case 01.58.

## In scope

- **Button remapping:** 12 gestures; action choices from the measured
  `COMMON_ACTIONS` (volume up/down, previous/next, ambient mode, voice assistant,
  play/pause) and the slide actions (volume up/down); per-button disable
  (action `0x0F`) where the measured mode allows it.
  - Write `04 81` body `[side, button_id, action]`; map button ids from the
    measured A3954 table (single 2, double 0, triple 5, long 1, slide-up 6,
    slide-down 7); order and side are measured.
  - Parse state bytes 116..124 (main gestures) and 161..165 (slides).
  - Controls page: Presses panel with 8 select rows and a slide panel with 4
    select rows; cycle panel remains.
- **Case settings:**
  - Case screen features `07 87` body `[atmospheric, remote_camera, find_device,
    case_spatial]` as four toggles.
  - Case language `07 8A` body `[language]` with Japanese/English/Chinese.
  - Case page (`Page::case` already exists in the vendored UI): features panel,
    language row, status panel (case battery/firmware; serial/address never).
  - Add `Page::case` to A3954 `device_pages` when the device reports the blocks.
- Localization: every new label/option uses `@tr`/`LocalizedText`; Russian
  screenshots/strings updated.
- Live acceptance on the real A3954: read original button/case state, change
  one action per gesture class and each case toggle/language, re-poll, restore,
  assert full equality; no serial/address.

## Out of scope

- Case brightness/color/style/display text (no measured write);
- air-pressure writes; HearID editing; A3062 behavior changes.

## Tickets

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-core-buttons-case.md` | — | Core button/case commands + channel state helpers/tests |
| `02-controller-buttons-case.md` | 01 | Controls/Case pages and writes wired |
| `03-live-acceptance.md` | 02 | Real A3954 verification and restore |
| `04-review.md` | 03 | Review, fixes, merge |

## Acceptance

1. All 12 gestures populate from state and write the measured `04 81` frame;
   action choices match the measured table.
2. Case features and language populate/write `07 87`/`07 8A`; case status shows
   battery/firmware only.
3. Russian UI, adaptive layout, no serial/address.
4. Desktop/core gates green; live A3954 restored; master merge reported.
