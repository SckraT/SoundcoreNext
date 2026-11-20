# 05 — Single-battery layout for Space One Pro (A3062)

**Type:** task
**Status:** ready-for-agent
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

## Acceptance

- [ ] Space One Pro connects and Overview shows one battery tile (e.g. 50 %).
- [ ] `core` and `desktop` tests/fmt/clippy/build pass.
- [ ] No identifiers committed.
- [ ] Ticket status done.
