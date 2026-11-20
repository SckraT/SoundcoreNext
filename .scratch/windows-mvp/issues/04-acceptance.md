# 04 — Hardware acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 03

## Goal

Prove the Windows MVP on real paired Soundcore hardware and leave evidence.

## Requirements

1. Build and run the app.
2. Connect to **P20i (A3949)** and **Liberty 4 Pro (A3954)** if available. If one is busy (phone holds SPP), document it and use the other; do not change the user's Bluetooth pairing.
3. Verify:
   - the paired candidate is marked suggested;
   - Overview shows device name/code and battery tiles;
   - values match the device (P20i 5-step mapped to %, Liberty 4 Pro 100/75/case);
   - disconnect returns to Devices; retry reconnects;
   - only `01 01` is written (code review of the session path).
4. Capture `desktop/overview-connected.png` showing the connected Overview with battery; add it to `desktop/README.md`. Mask or avoid MAC display.
5. Run:
   - `cd desktop; cargo test; cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings; cargo build`
   - `cd core; cargo test; cargo fmt --all -- --check; cargo clippy --all-targets -- -D warnings`
6. Record OS, device model/firmware (no serial/MAC), commands and results in the ticket.

## Acceptance

- [ ] Real hardware connected and battery verified.
- [ ] Screenshot stored and linked; no identifiers.
- [ ] All gates pass.
- [ ] Ticket status done.
