# 01 — Core button and case commands

**Status:** ready-for-agent
**Blocked by:** —

## Goal

Add the measured A3954 button and case write APIs to core, reusing the parsed
state and existing button/case structures.

## Requirements

1. Button model:
   - A3954 gesture enum/order for the 12 buttons (left/right × single, double,
     triple, long, slide up, slide down), with side and measured button id
     (single 2, double 0, triple 5, long 1, slide-up 6, slide-down 7).
   - Action IDs from the measured A3954 table: `COMMON_ACTIONS` (0 volume up,
     1 volume down, 2 previous, 3 next, 4 ambient mode, 5 voice assistant,
     6 play/pause) and slide actions (0 volume up, 1 volume down); disabled
     action `0x0F`.
   - State helpers to read the main 8 actions (body 116..124) and four slide
     actions (161..165) in the measured order.
   - `04 81` body `[side, button_id, action]`; no set-all packet for A3954.
   - Validate unknown action/button values; expose option catalogs.
2. Case:
   - `07 87` body four flags from parsed `A3954CaseFeatures`.
   - `07 8A` body `[language]`; re-add `A3954SettingCommand::CaseLanguage` if
     needed (the previous slice removed the unused command).
   - Read-only case battery/firmware unchanged; no serial/address.
3. Session method(s) for the button/case writes; one frame per write, no read.
4. Tests: action/button id vectors, body vectors, state helper order, all
   existing A3062/A3954 tests unchanged.
5. Gates: core fmt/build/test/clippy.

## Acceptance

- Core green; exact bodies/ids documented; deferred case display/air pressure
  untouched.
