# 01 — Custom equalizer core

**Type:** task
**Status:** ready-for-agent
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
OpenSCL4's A3062 custom-ear-id equalizer modifier does exactly that.

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
  | 0..2 | preset id `FE FE` ([`CUSTOM_EQUALIZER_PRESET_ID`]) |
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
  invisible bands are not caller-controlled: the eight-band API is the only
  curve input.
- **`DeviceSession::set_custom_equalizer(current, visible: [i16; 8])`** builds the
  body once and writes exactly one `03 87` frame. It never reads: the device's
  empty inbound `03 87` ack is skipped by the following `poll_state`. A model
  without equalizer support writes nothing and returns
  `SessionError::UnsupportedEqualizer` (its message is now "equalizer writes are
  not supported for this model", covering presets and custom curves).
- The preset builder was refactored onto the same private body builder, so both
  writes share one 53-byte shape implementation.

### Known-curve bytes (unit-tested)

Curve **+3.0, −3.0, +6.0, 0, −6.0, +2.0, 0, −2.0 dB** = raw
`[30, -30, 60, 0, -60, 20, 0, -20]`, starting from the parsed Soundcore
Signature state (its HearID defaults `00 FF… 00… FF… 00 00`):

- band bytes: `96 5A B4 78 3C 8C 78 64` + fixed `78 00`;
- 53-byte body: `FE FE 00 00 96 5A B4 78 3C 8C 78 64 78 00 00 00 00 FF FF FF
  FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 96 5A
  B4 78 3C 8C 78 64 78 00 00`;
- 63-byte `03 87` frame: `08 EE 00 00 00 03 87 3F 00` + body + `17`.

## Verification

- `core/crates/models/tests/equalizer.rs` (11 tests, all pass):
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
  - `a3949_equalizer_is_explicitly_deferred` — the A3949 has no custom body
    either;
  - the existing state/preset tests still pass unchanged.
- `core/crates/session/tests/session.rs` (22 tests, all pass):
  - `writes_the_exact_a3062_custom_equalizer_frame_once` — one exact frame, no
    read attempted (no reads scripted);
  - `a_deferred_equalizer_model_writes_nothing` — A3949 preset and custom writes
    both return `UnsupportedEqualizer` and write nothing.
- Gates on the final tree (core): `cargo test` 76 passed across 15 targets;
  `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings`
  clean. Desktop build with the changed core crates is clean (only the
  pre-existing vendored-Slint layout warnings).

## Real hardware (pending)

A real A3062 run was attempted on 2026-11-21 with a temporary ignored test in the
desktop crate (the same pattern the presets ticket used; the test was not
committed). It enumerates the paired **soundcore Space One Pro** (A3062 vendor
RFCOMM service present in the Windows SDP cache), opens the session and was
meant to write the distinctive curve, re-poll, verify `is_custom()`/bands and
restore the preset found on connect.

The device did not answer RFCOMM: `WindowsRfcommTransport::connect` returned
`BluetoothError::Timeout` for both the A3062 vendor service and the standard SPP
fallback, three attempts each, over roughly 37 s per run and three separate runs
between 13:14 and 13:40 MSK. No pairing or device setting was changed and no
`03 87` frame was written. The device was off/in its case/connected to another
host (the Windows audio endpoints for it were `Unknown`), so no live state frame
was captured and `core/data/a3062-eq-custom.json` is **not** committed.

To finish this ticket, with the headset powered on and not connected to a phone,
re-apply the scratch hardware test (or the equivalent) and record: the state
found on connect, the custom `01 01` answer showing preset `FEFE` and the curve,
and the restored `01 01` answer. Ticket `03-acceptance.md` re-verifies the same
custom write through the real UI.

## Acceptance

- [x] core gates pass (tests, fmt, clippy; desktop builds).
- [ ] Real custom write/re-poll verified; device left as found (blocked: device
      unreachable on 2026-11-21, re-check under ticket 03).
- [x] Ticket records frame hex, curve, offsets and body shape.
- [x] No serial, MAC or other identifier in the code, tests or this ticket.
