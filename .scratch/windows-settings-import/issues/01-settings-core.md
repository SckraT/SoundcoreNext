# 01 — Remaining settings core

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Parse A3062 settings bytes and build/write their commands.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- fixtures if needed from existing captures

## Data (OpenSCL4 permitted; see notes)

Body offsets and commands from `D:\Projects\SoundcoreNext-notes\remaining-settings.md`:
- 67 button double press (`04 81` `00 00 action`; 7 BassUp, 0x0F none)
- 68 ambient cycle bits (`06 82`)
- 76 low battery prompt (`10 82`), 77 Dolby (`02 86`), 78 LDAC (`01 FF`), 79 dual connections (`0B 84`)
- 80 auto power off (`01 86` `[enabled,index]`; 0..3 = 30/60/90/120m)
- 82 limit high volume (`20 82` `[enabled,dB]` 75..100 step 5; `20 81` refresh 0/1/2)
- 85 side tone (`01 84`), 86 voice prompt (`01 AE`)
- About: charging body[1], battery body[0] (ten scale/offset semantics already measured), firmware body[2..7], serial body[7..23] masked.

## Requirements

1. `A3062Settings` struct in `DeviceState` parsed for A3062 only; enums for button action, cycle bits, auto-off index, refresh rate; validated ranges.
2. Command builders returning exact bodies/frames for every scalar setting; a `SettingValue`/`SettingCommand` enum or equivalent for the controller.
3. `DeviceSession::set_setting(...)` writes exactly one frame; controller re-polls. No ACK read.
4. A3949 returns None/no writes; invalid values rejected.
5. Tests: parse all settings from the committed A3062 captures; exact frame hex for each setting; boundary values; A3949 no-op; session one write.
6. No UI code.

## Acceptance

- [ ] core gates pass.
- [ ] Offsets/commands/tests recorded in ticket.
- [ ] No identifiers committed.
