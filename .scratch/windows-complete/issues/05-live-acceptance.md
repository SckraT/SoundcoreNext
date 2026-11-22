# 05 — Live acceptance

**Status:** ready-for-agent
**Blocked by:** 04

## Goal

Verify the finished client on the real soundcore Space One Pro (A3062) after the
demo acceptance.

## Requirements

1. Power the A3062, pair/reconnect over Windows RFCOMM; use a real saved card.
2. Overview: demo mode off, connected card, identity/battery, quick settings,
   layout editing, panels/shortcuts; check capability filtering.
3. Sound mode and Equalizer: preset, custom curve, reset, saved profile apply,
   import/export roundtrip; device re-poll after each write.
4. Remaining settings pages: one toggle, select and range write each, re-poll and
   restore the original value.
5. Settings: theme, content scale, language, tray choices; restart persistence;
   tray Open/Quit/close-to-tray; Copy logs.
6. Diagnostics: run 3 read-only probes, review, Save ZIP, inspect archive for no
   serial/MAC, open prefilled issue URL (do not submit).
7. Idle heartbeat: leave connected past one interval, verify no unwanted writes;
   simulate/observe Lost when possible and Retry.
8. Restore device presets/settings and close the app; record firmware, screenshots
   (no address/serial), gates and any limitations.

## Acceptance

- [ ] All real-device features pass and are restored.
- [ ] Diagnostics archive is redacted.
- [ ] Screenshots/docs updated; ticket done.
