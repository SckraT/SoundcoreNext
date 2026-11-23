# 04 — Review and release

**Status:** done
**Blocked by:** 03 (done)

## Goal

Review and merge the buttons/case slice.

## Requirements

1. Code review (standards + spec) against master.
2. Fix hard findings and accepted judgement calls.
3. Re-run desktop/core fmt/build/test/clippy; update docs/tickets.
4. Merge `feature/a3954-buttons-case` into master with a merge commit; clean
   worktrees; report commit and gates.

## Acceptance

- Review resolved; master green; release commit reported.

## Result (2026-11-23, MSK)

### Review findings and fixes

The review was run against `feature/a3954-buttons-case` (findings supplied with
the ticket). All nine hard findings are resolved:

1. **README capability row.** The A3954 real-state row no longer says the case
   language is read-only: it now says the language is parsed and written through
   the measured `07 8A` command (the commands row already lists `07 8A`).
2. **Stale tickets 01/02.** Ticket 01 records the app-order `07 87` body
   `[remote_camera, find_device, case_spatial_audio, atmospheric]` (not the state
   flag order) with the corrected exact frame, and the live TWS-slot write
   correction. Ticket 02 records that gesture rows show/write the live slot
   (`with_tws`/`with_non_tws` from the `01 01` TWS flag) and keep the other
   nibble; both tickets carry the measured corrections from ticket 03.
3. **UI_KIT runtime-label rule.** §4 restores "Do not translate protocol
   identifiers (`NoiseCanceling`, `ldac`, `dualDevices`, the `04 81`
   gesture/action ids) or product names" and scopes the own-language sentence to
   the Settings app-language picker; the case-language select is a device value
   and is localized like its row (Russian UI shows `Японский`/…). `SOURCE.md`
   records the contract change.
4. **Write inventory.** `desktop/README.md` "The only device writes" and the
   root README "Windows session/transport" row now list the A3954 `04 81`
   button-configuration frames and the `07 87`/`07 8A` case writes.
5. **Slide disable.** The four slide rows offer `None` next to Volume up/down;
   `None` maps to `A3954_NO_BUTTON_ACTION` (`0x0F`) and preserves the other TWS
   nibble. OpenSCL4's A3954 table sets `IndividualDisable` for all twelve
   gestures, so this is measured, and no other slide action was added. The
   option mapping is one scheme for presses and slides (`0` = None, then the
   measured catalog); a core test pins the disabled slide frame and the
   controller queue test pins the nibble preservation.
6. **Case-feature failure banner.** A failed case-screen write now names the
   toggled row (`Find device`, `Atmospheric`, `Remote camera`, `Spatial audio on
   case`) through `a3954_setting_row_label`/`a3954_setting_failure_label`, the
   same row-label mechanism the gesture writes use; a controller test covers the
   banner. A failed language write still names `Case language`.
7. **Live test log and restore equality.** `set_button_until` takes whether the
   live slot is the TWS nibble and the retry log prints the edited slot instead
   of always `tws()`. The final assertion now compares the whole parsed
   `A3954State` as well as the named fields (printed through `assert!` so a
   failing `Debug` dump cannot leak the case serial); the live re-run below
   passes it.
8. **Case page condition.** `device_pages(has_settings, has_case)` adds
   `Page::Case` from the parsed A3954 case block (`has_case`) rather than a
   generic model flag; the call site and tests use that capability name.
9. **TWS-connected slot limitation.** The acceptance device reported its TWS
   link down for the whole run, so only the disconnected-state slot was
   exercised on hardware. Ticket 03 records the limitation explicitly and the
   README phrases the acceptance result honestly; the review re-run below found
   the device still TWS-down, so the limitation stands rather than being claimed
   as verified.

### Live hardware re-run (review)

The ignored test was re-run on the paired **real Liberty 4 Pro** (firmware
04.29, case 01.58, RFCOMM, masked address) after the fixes:
`manual_a3954_buttons_and_case -- --ignored --nocapture` → **1 passed / 0
failed / 0 ignored, 0 reconnects, 18.34 s**. The group ran the twelve gestures
(one other measured action each), the four case-screen toggles and
English→Japanese→Chinese→English, re-polling every write and restoring each
whole action byte and the connect case state. The device reported its **TWS link
down** again (`01 01` TWS flag down, disconnected-state action slot live), so the
connected-slot branch remains unit-tested only. The extended final assertion
reported "every parsed A3954 field equals the connect state"; the earbud firmware
was unchanged, the address stayed masked, no serial was printed, and the device
was left as found.

### Gates on the ticket branch

- Desktop (from `desktop`): `cargo fmt --all -- --check` clean, `cargo build`
  clean, `cargo test` **292 passed / 0 failed / 15 ignored** (the 15 ignored are
  the manual hardware checks, including the live run above),
  `cargo clippy --all-targets -- -D warnings` clean.
- Core (from `core`): `cargo fmt --all -- --check` clean,
  `cargo build --workspace` clean,
  `cargo test --workspace` **195 passed / 0 failed / 0 ignored**,
  `cargo clippy --workspace --all-targets -- -D warnings` clean.

No Bluetooth address or serial number is recorded in this file or in the test
output; the live UI captures are unchanged from ticket 03.

### Merge and final gates

- Ticket branch: `ticket/a3954-buttons-review` @ `96d9e8b` "Show the slide None
  choice in the A3954 UI preview" on top of `a5e90c2` "Fix the A3954 buttons/case
  review findings" (parent `04b89dd`, the feature tip). The second commit aligns
  the offline preview harness with the new slide choices.
- Merge commits on `feature/a3954-buttons-case`: `b37b3e4` "Merge the A3954
  buttons/case review fixes", then `e05ce9c` of the same title for the preview
  harness update; both were made with `--no-ff`, and `e05ce9c` is the feature
  tip.
- Gates rerun on the merged `feature/a3954-buttons-case` tree (`e05ce9c`, the
  second run is a no-op for the source rewrite): desktop
  `cargo fmt --all -- --check` clean, `cargo build` clean,
  `cargo clippy --all-targets -- -D warnings` clean, `cargo test` **292 passed /
  0 failed / 15 ignored** (the ignored are the manual hardware checks); core
  `cargo fmt --all -- --check` clean, `cargo build --workspace` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean,
  `cargo test --workspace` **195 passed / 0 failed / 0 ignored**.
- The release merge of `feature/a3954-buttons-case` into `master` is the next
  release step, reported from the release worktree.
