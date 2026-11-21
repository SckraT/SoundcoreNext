# 01 — Remaining settings core

**Type:** task
**Status:** done
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

## Implementation (2026-11-21)

The remaining A3062 settings are parsed into a new `A3062Settings` state type
(`DeviceState::settings`, `None` for every model without a measured settings
block), and each scalar setting has a `SettingCommand` variant that builds the
exact outbound body and, through `DeviceModel::setting_frame`, the complete
frame. The session writes exactly one frame per call and never reads an ack.

### State: `01 01` body 67..87

`DeviceModel::settings` is `Some(SettingsCapability { state_offset: 67 })` for
A3062 and `None` for the rest of the catalog. The parser requires the whole
20-byte block (body 87), so a body truncated inside it is `ShortBody { minimum:
87 }`.

| offset | field | measured | rule |
|---:|---|---|---|
| 67 | button double press | `0x0F` none | `7` BassUp, `0x0F` none; any other byte is `InvalidButtonAction` |
| 68 | ambient cycle bits | `0x03` | bit0 NC, bit1 transparency, bit2 normal; bits 3..7 ignored as OpenSCL4 does |
| 76 | low battery prompt | `1` | `1` on, anything else off |
| 77 | Dolby audio | `0` | `1` on |
| 78 | LDAC | `0` | `1` on |
| 79 | dual connections | `1` | `1` on |
| 80..82 | auto power off | `1,1` = on, 60m | enabled `1`; index `0..=3` = 30/60/90/120m, anything else is `InvalidAutoPowerOffIndex` |
| 82..85 | limit high volume | `0,90,0` | enabled `1`; dB `75..=100` step 5 else `InvalidLimitHighVolumeDbLimit`; refresh `0/1/2` else `InvalidDecibelReadingRefreshRate` |
| 85 | side tone | `1` | `1` on |
| 86 | voice prompt | `1` | `1` on |

Flags read as on only for byte `1`, matching OpenSCL4's `take_bool`, so an
unexpected non-zero flag byte reads as off instead of failing the whole poll;
closed enums/ranges do fail the poll rather than inventing a default.

Validated value types: `ButtonAction` (`7`), `AmbientSoundModeCycle`
(`from_byte`/`byte`), `AutoPowerOffDuration` (`index`/`from_index`/`minutes`;
`ALL` = 30/60/90/120m), `AutoPowerOff { enabled, duration }`, `DbLimit`
(`new` → `Option`, `MIN` 75, `MAX` 100, `STEP` 5), `DecibelReadingRefreshRate`
(`0/1/2`), `LimitHighVolume { enabled, db_limit, refresh_rate }`.

The same committed block is the measured field set of the captures: `67=0F,
68=03, 76..86 = 01 00 00 01 01 01 00 5A 00 01 01` (button none, NC+transparency
cycle, low battery on, Dolby off, LDAC off, dual connections on, auto-off 60m,
limit off/90 dB/real-time, side tone on, voice prompt on). The sound-mode bytes
inside body 69..76 differ between the first capture and the later ones, so the
settings tests assert the settings ranges only.

### About: charging at body 1

`SingleBatteryState::charging` now parses body byte 1 (`1` charging, anything
else not; every committed capture carries `0xFF`, i.e. not charging). That is
how OpenSCL4's A3062 `SingleBattery` parser reads it and what this ticket's data
section records as the About charging source; the earlier single-battery ticket
had conservatively left it `None`. Charging changes still also arrive as a
separate `01 04` notification, which this poll does not request. Battery and
firmware parsing are unchanged.

### Commands (`SettingCommand`, exact frames)

Frame shape is the usual outbound prefix + command + LE length + body +
additive checksum; every example below is asserted byte-for-byte.

| setting | command | body | frame (example) |
|---|---|---|---|
| double press | `04 81` | `00 00 action` | `08 EE 00 00 00 04 81 0D 00 00 00 07 8F` (BassUp), `… 0F 97` (none) |
| ambient cycle | `06 82` | `[bits]` | `08 EE 00 00 00 06 82 0B 00 03 8C` |
| low battery prompt | `10 82` | `[on]` | `08 EE 00 00 00 10 82 0B 00 01 94` |
| Dolby audio | `02 86` | `[on]` | `08 EE 00 00 00 02 86 0B 00 01 8A` |
| LDAC | `01 FF` | `[on]` | `08 EE 00 00 00 01 FF 0B 00 01 02` (device confirms with `01 7F`) |
| dual connections | `0B 84` | `[on]` | `08 EE 00 00 00 0B 84 0B 00 01 91` |
| auto power off | `01 86` | `[enabled, index]` | `08 EE 00 00 00 01 86 0C 00 01 01 8B` (60m) |
| limit high volume | `20 82` | `[enabled, dB]` | `08 EE 00 00 00 20 82 0C 00 00 5A FE` (off/90) |
| limit refresh | `20 81` | `[0/1/2]` | `08 EE 00 00 00 20 81 0B 00 00 A2` (real time) |
| side tone | `01 84` | `[on]` | `08 EE 00 00 00 01 84 0B 00 01 87` |
| voice prompt | `01 AE` | `[on]` | `08 EE 00 00 00 01 AE 0B 00 01 B1` |

`DeviceModel::setting_frame(SettingCommand)` returns `Option<Vec<u8>>`, `None`
for a model without the settings block. `DeviceSession::set_setting` writes that
frame once and never reads: the empty same-command ack and notifications are
skipped by the controller's following `poll_state` (the LDAC `01 7F` one is not
read here either). A model without the block writes nothing and returns the new
`SessionError::UnsupportedSetting` ("settings are not supported for this
model") — the A3949 path required by the ticket.

### No UI code

No `.slint` edit and no new desktop behaviour. The desktop diff is compile-only:
the three exhaustive `SessionError` mappings gained the new
`UnsupportedSetting` arm, and the two `DeviceState` test constructors gained
`settings: None`.

## Verification

- `core`: `cargo test` **104 passed / 0 failed / 0 ignored** across 19 targets
  (including doc-tests, new: 11 models settings tests and 4 session tests),
  `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D
  warnings` clean.
- `core/crates/models/tests/settings.rs` (11 tests):
  - every committed A3062 `01 01` capture parses to the measured settings block
    — the state fixture, three sound-mode captures, three preset captures plus
    the restored one, and the three custom-curve captures (11 frames);
    capability offset 67 and the raw `67..69` / `76..87` bytes are asserted;
  - a model without the block has `settings == None` and builds no frame (A3949,
    A3947, A3954, A3035, A3040); the A3949 state parses with `settings == None`;
  - every `SettingCommand` produces its exact frame (27 cases: all variants,
    both bool values, all four auto-off durations, limits 75/90/100, all three
    refresh rates, BassUp/none, cycle 0x07/0x03/0x00);
  - boundaries/validation: dB limits 75..=100 step 5 accepted / 74, 76, 78, 101
    etc. rejected; auto-off index 0..=3 mapped with minutes, 4/255 rejected;
    refresh 0/1/2 mapped, 3/255 rejected; button 7 mapped, 0/garbage rejected;
    cycle all eight bit combinations roundtrip and high bits are ignored;
  - state rejections: button `3`, auto-off index `4`, dB `78`, refresh `3` are
    typed `StateError`s; a non-one flag byte reads as off; body[..86] is
    `ShortBody { actual: 86, minimum: 87 }`.
- `core/crates/models/tests/state.rs` (17 tests): the real A3062 capture reports
  `charging: Some(false)`; a new test checks body byte 1 maps `1` → charging and
  `0`/`0xFF`/`2` → not charging; the synthetic-body helper now carries a valid
  87-byte settings block.
- `core/crates/models/tests/equalizer.rs`: the synthetic 95-byte body test gained
  the two settings bytes a valid body needs (button `0F`, dB `80`); its 12
  tests still pass.
- `core/crates/session/tests/session.rs` (26 tests, 4 new): each of 11 settings
  writes exactly one exact frame with no read scripted (so no ack read is
  attempted); a model without the block writes nothing and returns
  `UnsupportedSetting`; a failed write maps to `SessionError::Io`; a write
  followed by an ack + battery notification + real state re-poll yields the
  parsed settings and exactly two writes (setting frame, then `01 01`).
- `desktop`: `cargo test` **136 passed / 0 failed / 8 ignored**,
  `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
  clean (only the pre-existing vendored-Slint layout warnings).

## Real hardware (2026-11-21, read-only)

The real **soundcore Space One Pro** (catalog A3062, firmware **04.39**) was
reachable over the Windows RFCOMM transport, so the parse was checked on it with
a temporary ignored desktop test (removed before committing, the presets-ticket
pattern; only `01 01` was written, no setting was changed). The parsed state was
exactly the measured block above: button none, NC+transparency cycle, low battery
prompt on, Dolby off, LDAC off, dual connections on, auto power off 60m, limit
off/90 dB/real-time, side tone on, voice prompt on, `charging: Some(false)`.
No address or serial was printed or committed, and the device was left as found
(nothing was written except the state request).

## Acceptance

- [x] core gates pass (tests, fmt, clippy; desktop tests/fmt/clippy too).
- [x] Offsets, commands and tests recorded above.
- [x] No identifiers committed (fixtures stay sanitized; hardware output had no
  MAC/serial).
