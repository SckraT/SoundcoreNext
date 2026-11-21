# 01 — Equalizer presets core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Parse A3062 equalizer state, expose the preset catalog, build the `03 87` preset body, and write it through the session.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-eq-*.json` sanitized fixtures

## Data source

OpenSCL4 command/response and preset data is authorized for this project and
adapted under GPL-3.0-or-later with attribution in `NOTICE`. The A3062
equalizer data comes from:

- `lib/src/devices/soundcore/a3062/packets/inbound/state_update.rs`
- `lib/src/devices/soundcore/a3062/packets/outbound.rs` (`set_equalizer`)
- `lib/src/devices/soundcore/a3062/modules/equalizer.rs` (preset names/ids/bands)
- `common/structures/equalizer_configuration.rs`, `volume_adjustments.rs`

## Requirements

1. A3062 state parses:
   - EQ starts after battery(1)+unknown(1)+firmware(5)+serial(16) = body offset 23;
   - preset id u16 LE at 23..25;
   - 10 band bytes at 25..35, value = byte - 120 (dB×10);
   - record measured/derived offsets in the ticket; verify against a live A3062 state.
2. Preset catalog with ids/names/bands matching OpenSCL4's A3062 list (22 presets, signature id 0, custom 0xFEFE).
3. Build the full 53-byte `03 87` body:
   - preset id 2 bytes, hear-id genre 2, current band 10, unknown 2 zeros, hear-id enabled 0, hear-id volume 10, time 4 zero, type 2, custom 10, active bands 10, unknown 0 — preserve the A3062 packet shape; use OpenSCL4 defaults/current state so a preset write is accepted.
4. Session method to write the preset body exactly once; controller re-polls state. No ACK read.
5. Tests: real A3062 state parses preset/bands; each preset command hex; A3949 equalizer deferred (its state block at body 32..44 is known but not parsed and has no write body); session writes one frame; sanitized fixtures no serial/MAC.
6. No UI code.

## Measured offsets and command shape (2026-11-21)

Measured on the real Space One Pro (**A3062**, firmware **04.39**) over the
Windows RFCOMM transport with a temporary ignored test in the desktop crate. The
test was reverted and never committed; its only output was sanitized frames.
No MAC address or serial number is recorded here.

- `01 01` response: **105 bytes** = 9 header + **95 body** + 1 checksum, as in
  the sound-mode ticket.
- **Equalizer block: body offset 23**, immediately after
  battery(2: level + charging/reserved)+firmware(5)+serial(16):

  | body offset | field | live evidence |
  |---:|---|---|
  | 23..25 | preset id, u16 LE | `00 00` = Soundcore Signature |
  | 25..35 | 10 band bytes, value = byte - 120 (dB×10) | `78 78 78 78 78 78 78 78 78 00` = `[0,0,0,0,0,0,0,0,0,-120]` |
  | 35..37 | unknown | live `1E 00`; the write sends `00 00` and all three preset writes were accepted |
  | 37..65 | HearID block (enabled, 10 volume bytes, time, type, 10 custom bytes, 2 music-genre bytes) | kept verbatim by the write; `00 FF… 00… 00 FF… 00 00` on the found state |

  The mode byte at body offset 69 and the prefix are unchanged by this slice.
- Starting state: preset id 0, signature bands. The initial and final captures
  below are byte-identical after serial masking.
- The write was accepted for three presets: **Soundcore Signature (0)**,
  **Bass Reducer (3)** and **Podcast (5)**. After each write the next `01 01`
  reported exactly that preset id and the catalog's band values, at body
  offsets 23..25 and 25..35.
- Each write is acknowledged with the empty inbound frame
  `09 FF 00 00 01 03 87 0A 00 9D`; the session write does not read it, and the
  next `01 01` poll skips it (unit-tested with a scripted ack).
- The original preset was restored by running the same body builder with the
  original id and bands; the final `01 01` equalizer block equalled the first
  one, so the device was left as found.

### Command bytes

The body is `preset(2) genre(2) bands(10) 00 00 enabled(0) hear-id-volume(10)
time(4) type(1) custom(10) active-bands(10) 00` = 53 bytes. The three writes
captured on the device:

| preset | body hex | full `03 87` frame |
|---|---|---|
| Soundcore Signature (0) | `00 00 00 00 78 78 78 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 78 78 78 78 78 78 78 78 78 00 00` | `08 EE 00 00 00 03 87 3F 00` + body + `1B` |
| Bass Reducer (3) | `03 00 00 00 50 5A 6E 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 50 5A 6E 78 78 78 78 78 78 00 00` | `08 EE 00 00 00 03 87 3F 00` + body + `7E` |
| Podcast (5) | `05 00 00 00 5A 8C A0 A0 96 8C 78 64 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 5A 8C A0 A0 96 8C 78 64 78 00 00` | `08 EE 00 00 00 03 87 3F 00` + body + `E8` |

The HearID `type` byte is copied from the state: the found state reported type
`0` (Initial), so the measured bodies carry `00`; OpenSCL4's A3062 test uses `2`
for FavoriteMusicGenre. The HearID enabled byte is always `0` in a preset write,
and the HearID sibling fields are copied from the state, as OpenSCL4's A3062
equalizer does. The two band regions are the selected preset's curve (current
and active).

### Preset table source

The catalog is OpenSCL4's
`lib/src/devices/soundcore/a3062/modules/equalizer.rs` list, names taken from
OpenSCL4's English display strings in `lib/i18n/en/openscq30-lib.ftl`. It
contains **22 built-in presets**: ids `0`, `1`, `3..=21` and `0x7E7E`
(Bass Booster; id `2` is absent for this model). An earlier draft of this ticket
said "(13 presets)", which matches only ids `0..=13`; requirement 2 and this
record were corrected to the actual 22-entry OpenSCL4 list. Every preset has 10
raw band values; the last two bands are invisible on this model and fixed at `0`
and `-120`, as in OpenSCL4.

Custom bands are reported as preset id **0xFEFE**; the parser exposes
`EqualizerState::is_custom()` and no catalog entry exists for 0xFEFE, so the UI
can show no selected preset while the read-only curve still follows the device.
Selecting a custom preset was not exercised on hardware (the device was found
on a built-in preset); parsing the id is covered by unit tests.

## Verification

- New `core/crates/models/tests/equalizer.rs`:
  - the three real sanitized frames parse preset id and bands at body offsets
    23..25 and 25..35; the parser's declared offsets match the fixture, and the
    serial is absent;
  - the restored real frame parses back to the original preset;
  - the catalog is the 22-entry OpenSCL4 list (ids, signature bands, spot-checks
    for Bass Reducer/Podcast/Bass Booster, invisible last bands, A3949 empty);
  - the exact 53-byte bodies and full `03 87` frames above match the captures;
  - the state's HearID fields are copied and its enabled flag is cleared;
  - custom `0xFEFE` parses as custom and is not a built-in preset;
  - A3949's equalizer is deferred: its model has no capability, catalog or write
    body, and its committed `01 01` capture (which does hold an equalizer block
    at body 32..44) parses with no equalizer;
  - a body truncated at the HearID block, and band bytes at both ends of the
    range, are tested.
- New session tests: exactly one measured `03 87` frame is written and no read
  is attempted; A3949 writes nothing and reports
  `SessionError::UnsupportedEqualizer`; the `01 01` re-poll skips the empty
  `03 87` ack and a battery notification; a failed write is an I/O error.
- Hardware evidence came from `manual_equalizer_preset_switch` and
  `manual_equalizer_ack_shape` in the desktop crate; both were removed after the
  run (the state and ack captures are in `core/data/a3062-eq-presets.json`).
- Sanitized fixture: `core/data/a3062-eq-presets.json` (three post-write
  `01 01` frames plus the restored frame; serial bytes zero, checksums
  recalculated, no address).

## Acceptance

- [x] core tests/fmt/clippy pass.
- [x] Real A3062 state parse and preset writes verified in an ignored hardware test (device left as found).
- [x] Ticket records offsets, preset table source, command bytes.
- [x] No identifiers committed.
