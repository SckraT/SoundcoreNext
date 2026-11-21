# 01 — Custom equalizer core

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Build/write a custom equalizer body and expose the current custom state.

## Files

- `core/crates/models/src/lib.rs` (+ tests)
- `core/crates/session/src/lib.rs` (+ tests)
- `core/data/a3062-eq-custom.json` sanitized fixture

## Requirements

1. Use the existing A3062 `03 87` builder shape; custom `preset = 0xFEFE`, active bands = user bands, fixed last two bands as measured (`[0, -120]`), preserve HearID bytes from the current state/defaults.
2. `DeviceModel::custom_equalizer_body(bands: [u8; 8])` (or equivalent) returns the 53-byte body; values are packet bytes (`dB×10 + 120`), clamped -120..134.
3. Session method writes the custom body once; controller re-polls. No ACK read.
4. Parse custom state (`is_custom()`, bands) already exists; verify.
5. Tests: exact command frame for a known curve; custom preset id; clamping; A3949 deferred/no write; session one write; fixture sanitized.
6. Real A3062: write a distinctive custom curve, re-poll, verify; restore previous preset/curve; record evidence.

## Implementation (2026-11-21)

The custom write reuses the measured A3062 `03 87` body verbatim; only the
preset id and the curve differ from the preset write, and the HearID handling is
the same (copy the state's HearID fields, clear the enabled flag) because
OpenSCL4's A3062 `EqualizerWithCustomHearIdStateModifier` does exactly that for
its custom band writes.

- **`VISIBLE_EQ_BANDS = 8`, `FIXED_EQ_BANDS = [0, -120]`** (raw dB×10) in
  `soundcore_models`; the fixed pair is the measured A3062 invisible bands every
  built-in preset also carries.
- **`EqualizerState::visible_bands()`** returns the first eight of the ten parsed
  bands, so the controller can seed the page's editable bands.
- **`DeviceModel::custom_equalizer_body(current, visible: [i16; 8])`** returns the
  53-byte body (`None` for a model without the equalizer capability, e.g. the
  deferred A3949):

  | bytes | custom write |
  |---:|---|
  | 0..2 | preset id `FE FE` (`CUSTOM_EQUALIZER_PRESET_ID`) |
  | 2..4 | HearID favorite music genre, from the current state |
  | 4..14 | 8 user bands, then the fixed `[0x78, 0x00]` (`0` dB and −12.0 dB) |
  | 14..16 | unknown, zero |
  | 16 | HearID enabled: `0` |
  | 17..27 | HearID volume adjustments, from the current state |
  | 27..31 | HearID time, big-endian, from the current state |
  | 31 | HearID type, from the current state |
  | 32..42 | HearID custom volume adjustments, from the current state |
  | 42..52 | the same ten active band bytes again |
  | 52 | unknown, zero |

  A band byte is `clamp(value, -120, 134) + 120`, i.e. the raw dB×10 value is
  clamped to the measured `-120..=134` range (0..254 as a packet byte). The two
  invisible bands are not caller-controlled: only eight values can be passed.
  No raw ten-band variant is exposed, because a caller must not be able to
  select the two bands the device fixes; the fixture/rebuild tests cover the
  ten-byte result.
- **`DeviceSession::set_custom_equalizer(current, visible: [i16; 8])`** builds the
  body once and writes exactly one `03 87` frame. It never reads: the device's
  empty inbound `03 87` ack is skipped by the following `poll_state`. A model
  without equalizer support writes nothing and returns
  `SessionError::UnsupportedEqualizer` (its message is now "equalizer writes are
  not supported for this model", covering presets and custom curves).
- The preset builder was refactored onto the same private body builder, so both
  writes share one 53-byte shape implementation.

### Known-curve bytes (unit-tested and real-device-confirmed)

Curve **+3.0, −3.0, +6.0, 0, −6.0, +2.0, 0, −2.0 dB** = raw
`[30, -30, 60, 0, -60, 20, 0, -20]`, starting from the parsed Soundcore
Signature state found on connect (its HearID defaults `00 FF… 00… FF… 00 00`):

- band bytes: `96 5A B4 78 3C 8C 78 64` + fixed `78 00`;
- 53-byte body: `FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 00 00 00 00 FF FF FF
  FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 96 5A
  B4 78 3C 8C 78 64 78 00 00`;
- 63-byte `03 87` frame: `08 EE 00 00 00 03 87 3F 00` + body + `17`.

## Verification

- `core/crates/models/tests/equalizer.rs` (12 tests, all pass):
  - `builds_the_exact_a3062_custom_body_and_frame` — the exact 53-byte body and
    full 63-byte frame above, custom preset id `0xFEFE`, the fixed invisible
    bands and the repeated active curve;
  - `clamps_custom_bands_to_the_measured_range` — `135`, `i16::MAX`, `-121` and
    `i16::MIN` clamp to the measured range, band bytes become
    `[254, 254, 0, 0, …]`;
  - `keeps_the_state_hear_id_fields_but_clears_the_enabled_flag` — a custom body
    copies the same HearID fields as a preset body and clears the enabled flag;
  - `custom_preset_id_is_not_a_built_in_preset` — custom parsed state reports
    `is_custom()`, `preset()` is `None`, `visible_bands()` is the first eight
    bands, and rebuilding from that state fixes the invisible bands;
  - `parses_the_real_a3062_custom_equalizer_fixture` — the committed real frames
    (below): the found state builds exactly the frame the device accepted, the
    re-polled state is `FEFE`/the written curve, and the restored state equals
    the found state; the fixture serial parses as absent;
  - `a3949_equalizer_is_explicitly_deferred` — the A3949 has no custom body
    either;
  - the existing state/preset tests still pass unchanged.
- `core/crates/session/tests/session.rs` (22 tests, all pass):
  - `writes_the_exact_a3062_custom_equalizer_frame_once` — one exact frame, no
    read attempted (no reads scripted);
  - `a_deferred_equalizer_model_writes_nothing` — A3949 preset and custom writes
    both return `UnsupportedEqualizer` and write nothing.
- Gates on the final tree: core `cargo test` 77 passed / 0 failed / 0 ignored
  across 15 targets, `cargo fmt --all -- --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean; desktop `cargo test` 102
  passed / 6 ignored, fmt and clippy clean, `cargo build` ok (only the
  pre-existing vendored-Slint layout warnings).

## Real hardware (2026-11-21)

Verified on the real **soundcore Space One Pro** (catalog **A3062**, firmware
**04.39**, ten-step battery) over the Windows RFCOMM transport, after the user
powered the headset on and disconnected it from the phone. The run used a
temporary ignored test in the desktop crate (the presets-ticket pattern); the
test was removed after the run and no desktop source is part of this commit.
Because the desktop controller still has no custom write path (ticket 02), the
test drove the core session directly.

- Found on connect: preset **0**, **Soundcore Signature**, bands
  `[0, 0, 0, 0, 0, 0, 0, 0, 0, -120]`.
- Wrote the custom frame above exactly once; the write path did not read the
  ack.
- Next `01 01` answer: preset **`FEFE`**, `is_custom() == true`, no built-in
  preset, bands exactly `[30, -30, 60, 0, -60, 20, 0, -20, 0, -120]` — the eight
  written bands and the fixed invisible pair. The device accepted the write.
- Restored the found preset (id 0) with the same preset builder, re-polled: the
  equalizer block returned to preset 0 / the Signature bands.
- The sanitized before and restored `01 01` frames are **byte-identical**, so the
  device was left as found.
- Exactly one frame with preset id `FEFE` was written; the only other `03 87`
  was the deliberate restore write.
- No Bluetooth address, serial or other identifier was printed or committed: the
  captured frames have sixteen ASCII-zero serial bytes, no address, and
  checksums recalculated after masking.

### Sanitized fixture

`core/data/a3062-eq-custom.json` holds the three real sanitized `01 01` frames
(found, custom, restored), the eight visible dB×10 values, the ten parsed band
values, the custom preset id, and the exact body/frame hex above. The header
notes it is a real capture and how it was sanitized.

## Acceptance

- [x] core gates pass (tests, fmt, clippy; desktop tests/build).
- [x] Real custom write/re-poll verified; device left as found.
- [x] Ticket records frame hex, curve, offsets and body shape.
- [x] No serial, MAC or other identifier in the code, tests or this ticket.
