# 05 — Single-battery layout for Space One Pro (A3062)

**Type:** task
**Status:** done
**Blocked by:** 01, 02

## Goal

The available real device on this machine is Space One Pro (A3062), which connects but reports a single-battery layout the parser rejects. Add that layout so the Windows MVP can show its battery.

This is a narrowly measured extension of the MVP, not a general single-battery implementation.

## Files

- `core/crates/models/src/lib.rs` (implement `Layout::SingleCmm2`)
- `core/crates/models/tests/state.rs`
- `core/data/a3062-state.json` (new sanitized fixture)
- `desktop/src/controller.rs` (map the single battery tile)

## Requirements

1. Capture a real `01 01` response from Space One Pro using the existing Windows transport (temporary ignored test or a small example; do not commit the capture with identifiers).
2. Locate the battery byte and the firmware ASCII `04.39`; record the offsets and body length in the ticket.
3. Implement `SingleCmm2` parsing for A3062:
   - validate host/battery/charging/firmware bounds;
   - use `BatteryScale::Ten` (raw 5 = 50 %);
   - return a single battery value; model API may add `DeviceState::single_battery` or reuse left with a flag — choose a minimal explicit shape and document it.
4. Sanitize the captured frame: zero the serial field, recompute checksum, store `core/data/a3062-state.json`; no MAC/serial in the repo.
5. Controller: for single-battery state show one tile with `label = 3` (the vendored UI renders “Battery”), value `NN%`, progress.
6. Tests: real sanitized A3062 state parses to 50 %, firmware 04.39; short/oversized battery rejected; controller maps one tile.

## Measured layout (2026-11-21)

Captured from the real Space One Pro over the Windows RFCOMM transport with a
temporary ignored test; the serial was masked before printing and nothing was
written except `01 01`. The device answered in 76 ms.

- Total frame: **105 bytes** = 9 header + **95 body** + 1 checksum.
- Body offset 0: battery step on the ten-step scale. The live capture was raw
  `2` (20 %); a raw `5` is 50 %.
- Body offset 1: constant `0xFF`; not a host and not a charging field.
- Body offsets 2..7: firmware as ASCII `04.39`.
- Body offsets 7..23: serial, 16 ASCII characters; zeroed in the fixture and
  the checksum recalculated (`core/data/a3062-state.json`).
- Charging is not part of `01 01`: the device reports it in a separate `01 04`
  notification, which this MVP does not poll, so `SingleBatteryState::charging`
  is `None` rather than an invented `false`.
- Parsing is gated to A3062, the model that was measured; A3035/A3040 still
  return `UnsupportedLayout`.

## Verification

- `cargo test` in `core` and `desktop` covers the real sanitized fixture
  (raw 2/10, firmware 04.39), the `5/10 -> 50 %` mapping, short/oversized
  battery rejection, and the controller's one `label = 3` tile.
- Ignored hardware test `manual_paired_connect` connected to Space One Pro:
  Overview mapped `label 3=20%` (the live device was at 2/10 at capture time;
  the 100 %-step P20i still mapped its two TWS tiles). Liberty 4 Pro timed out
  as expected on this machine.

## Acceptance

- [x] Space One Pro connects and Overview shows one battery tile (20 % on the
  live device; raw 5/10 maps to 50 %).
- [x] `core` and `desktop` tests/fmt/clippy/build pass.
- [x] No identifiers committed.
- [x] Ticket status done.
