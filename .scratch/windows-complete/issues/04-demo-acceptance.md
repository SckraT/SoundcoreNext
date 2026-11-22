# 04 — Demo acceptance

**Status:** ready-for-agent
**Blocked by:** 02, 03

## Goal

Use the demo device to accept the finished Windows UI without hardware.

## Requirements

1. Launch a fresh build; Add Device → select A3062 → Demo mode → connect.
2. Exercise Overview (layout presets, reorder/hide, quick-toggle chooser, chips),
   Sound mode, Equalizer (preset/custom/reset/profiles/import/export preview),
   every Settings page and device page, My Devices disconnect/reconnect, tray
   open/close/quit, Settings appearance/language/copy logs.
3. Change theme (light/dark/follow), palette, content scale 100/110/125, device
   color, language (RU/EN), one-earbud badge; verify persistence after restart.
4. Capture the required visual matrix at 800×700, 1030×760, 1280×820 and
   1440×900, at 100/125/150%, light/dark; check long strings and no clipping.
5. Verify Diagnostics demo messaging and that no ZIP/issue is produced.
6. Fix any defects found (controller/UI) in the same ticket.
7. Record OS, app version, demo device identity, screenshots and gate results.
   No MAC/serial in docs/images.

## Acceptance

- [ ] All UI surfaces work with the demo device.
- [ ] Settings + preferences persist across restart.
- [ ] Visual matrix clean.
- [ ] Desktop/core gates green; ticket done.
