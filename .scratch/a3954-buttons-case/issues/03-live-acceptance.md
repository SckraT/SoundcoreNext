# 03 — Live A3954 buttons/case acceptance

**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify the 12 gestures and case settings on the real A3954 and restore them.

## Requirements

1. Add/extend an ignored hardware test that connects the paired A3954 and:
   - reads all 12 button actions and the case features/language;
   - changes one action per gesture class (single, double, triple, long, slide
     up, slide down), polling/asserting the re-read value;
   - toggles each case feature and changes the case language, polling/asserting;
   - restores every original button action, case toggle and language; asserts
     final state field-by-field;
   - no serial/address printed; masked address only.
2. Run the test with `--ignored --nocapture`; if a write drops the link,
   reconnect and restore.
3. Build/run the app with isolated APPDATA; verify Controls/Case pages populate
   (UIA/screenshots), no values clicked through UI except the test's restores.
4. Update README with real facts and ticket record; no serial/MAC.
5. Gates: desktop/core fmt/build/test/clippy.

## Acceptance

- All gesture/case writes confirmed and restored; device left as found; ticket
  done.
