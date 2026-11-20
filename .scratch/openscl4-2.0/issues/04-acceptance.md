# 04 — Acceptance and screenshot

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01, 02, 03

## Goal

Prove the slice on the integration branch and leave a screenshot for the 2.0 README.

## Requirements

1. Build the desktop app: `cd desktop; cargo build`.
2. Run it, open Add Device, capture a screenshot showing the populated model grid.
3. Save the screenshot as `desktop/add-device.png` and reference it from `desktop/README.md`.
4. Run:
   - `cd desktop; cargo fmt --all -- --check`
   - `cd desktop; cargo clippy --all-targets -- -D warnings`
   - `cd core; cargo test`
5. Record in the ticket: command outputs (pass/fail), OS, and what was verified manually.

## Acceptance

- [ ] Screenshot exists and shows at least three model cards with codes/names.
- [ ] All commands above pass.
- [ ] `desktop/README.md` links the screenshot and describes the 2.0 slice.
- [ ] No secrets, MAC addresses or serial numbers appear in the screenshot or docs.
- [ ] Ticket status set to `done`.
