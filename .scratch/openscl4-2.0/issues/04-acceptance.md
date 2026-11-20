# 04 — Acceptance and screenshot

**Type:** task
**Status:** done
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

- [x] Screenshot exists and shows at least three model cards with codes/names.
- [x] All commands above pass.
- [x] `desktop/README.md` links the screenshot and describes the 2.0 slice.
- [x] No secrets, MAC addresses or serial numbers appear in the screenshot or docs.
- [x] Ticket status set to `done`.

## Verification

**Environment:** Windows 11 Pro 10.0.26300 (build 26300), `rustc 1.99.1` /
`cargo 1.99.1`, primary display 2560×1440 at 125 % scaling.

**Commands** (run in the worktree):

| Command | Result |
|---|---|
| `cd desktop; cargo build` | pass — `Finished dev profile [unoptimized + debuginfo] target(s) in 2m 12s`; only the pre-existing vendored-Slint layout warnings |
| `cd desktop; cargo fmt --all -- --check` | pass — no diff |
| `cd desktop; cargo clippy --all-targets -- -D warnings` | pass — exit 0 |
| `cd desktop; cargo test` | pass — 15 passed, 0 failed (catalog adapter) |
| `cd core; cargo test` | pass — 32 passed, 0 failed (protocol, models, state, diagnostics) |

**Screenshot:** `desktop/add-device.png`, 1602×1064 px. It shows the running
`soundcore-desktop` window titled “My devices — OpenSCL4” with the Add Device
dialog open: the search field, the five category chips (All selected) and eight
model cards with names and codes — Liberty 2 Pro/A3909, Liberty 2 Pro+/A3930,
Liberty Air 2 Pro/A3951, Liberty 3 Pro/A3952, Liberty 4/A3953, Liberty 4
Pro/A3954, Liberty 5/A3957, Liberty 4 NC/A3947 — plus the start of the next row.
No MAC address, serial number or other personal data appears in the image or in
the docs touched by this ticket.

**How it was captured:** the debug binary was started and the existing “Add
device” button was invoked through the UI Automation tree that the Slint window
exposes (no UI or Rust behaviour was changed for the screenshot). The window was
raised with `SetWindowPos(HWND_TOPMOST)` and only that window was captured with
`DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` + GDI `CopyFromScreen`.
The capture helper lived in `%TEMP%`; the only new repository file is the PNG.

**Verified manually in the running app:**

- Opening Add Device lists the catalog; 12 cards are reachable in the visible
  viewport and the rest scroll.
- Chips filter as specified: Over-ear → Space One, Space Q45, Space One Pro,
  Q20I, Q11i, Life Q35, Q30 / Life Q30, Life Tune, Life Tune Pro; Open-ear →
  C30i, AeroClip, V20i, C50i; Speakers → Motion+.
- Search `A3954` narrows to Liberty 4 Pro; `P20` shows P20i / P25i / R50i; an
  unknown query shows the vendored “No models found…” empty state.
- Choosing Liberty 4 Pro advances the dialog to “Ready to connect your
  headphones?” with its name and code; no connection or transport is attempted.
- The window title still says OpenSCL4.

**Files changed:** `desktop/add-device.png` (new), `desktop/README.md`
(screenshot + 2.0 slice status), and the root `README.md` status row that still
said the desktop “opens with empty state until the catalog wiring lands”.
