# 05 — Live acceptance

**Status:** ready-for-agent
**Blocked by:** 04
**Last attempt:** 2026-11-23 05:00–05:26 MSK — blocked: the A3062 was powered off

## Goal

Verify the finished client on the real soundcore Space One Pro (A3062) after the
demo acceptance.

## Requirements

1. Power the A3062, pair/reconnect over Windows RFCOMM; use a real saved card.
2. Overview: demo mode off, connected card, identity/battery, quick settings,
   layout editing, panels/shortcuts; check capability filtering.
3. Sound mode and Equalizer: preset, custom curve, reset, saved profile apply,
   import/export roundtrip; device re-poll after each write.
4. Remaining settings pages: one toggle, select and range write each, re-poll and
   restore the original value.
5. Settings: theme, content scale, language, tray choices; restart persistence;
   tray Open/Quit/close-to-tray; Copy logs.
6. Diagnostics: run 3 read-only probes, review, Save ZIP, inspect archive for no
   serial/MAC, open prefilled issue URL (do not submit).
7. Idle heartbeat: leave connected past one interval, verify no unwanted writes;
   simulate/observe Lost when possible and Retry.
8. Restore device presets/settings and close the app; record firmware, screenshots
   (no address/serial), gates and any limitations.

## Acceptance

- [ ] All real-device features pass and are restored.
- [ ] Diagnostics archive is redacted.
- [ ] Screenshots/docs updated; ticket done.

## Attempt record (2026-11-23): blocked by hardware availability

**Outcome: not accepted.** The paired soundcore Space One Pro (A3062) never
answered the Windows RFCOMM link during the whole acceptance window, so no
real-device check was run and nothing under `## Acceptance` is checked. The
ticket stays `ready-for-agent`; no demo-mode result is used as hardware
acceptance and no hardware result is claimed.

### Environment

- Windows 11 Pro 26H2, build `10.0.26300.9974`, x64; display scaling 125 %.
- `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`); debug build
  `desktop/target/debug/soundcore-desktop.exe`, app version OpenSCL4 2.0.0.
- Branch `ticket/wc-05-live-acceptance` at `c98917e` (the
  `integration/windows-complete` tip); no source changes were made, because the
  device was never reached and no defect could be observed.
- Radio: Intel Wireless Bluetooth present and OK; `bthserv` and `BthAvctpSvc`
  running; Windows holds the pairing and cached vendor/SDP services for the
  paired soundcore Space One Pro (A3062), P20i (A3949) and Liberty 4 Pro
  (A3954). No Bluetooth address is recorded here or anywhere in this ticket.

### Blocker

- The ignored manual hardware probe
  `cargo test -p soundcore-desktop manual_paired_connect -- --ignored --nocapture`
  (it only writes the read-only `01 01` state request, and only after a link
  opens) was run every ~40 s from **05:00:38 to 05:25:50**: **35 probe cycles,
  every one timed out** with `failed (TimedOut): The device did not answer in
  time` for the A3062 (and the same for the two other paired Soundcore models).
  No link ever opened, so no frame was written and no device state was read or
  changed.
- The A3062 was powered off rather than merely idle: Bluetooth paging, which is
  what an RFCOMM connect does, cannot wake headphones that are switched off and
  there is no remote wake path. The neighbouring paired Soundcore models were
  off too, so this is device availability, not a radio failure.
- Windows pairing, Windows Bluetooth settings and the device's own controls were
  not touched; the app-data documents were restored to their pre-run bytes
  (`desktop-preferences.json`; no Overview or profile document existed before or
  after the attempt).

### Software-only checks completed (gates)

Run on this branch; there are no source changes, so they still stand:

- **desktop:** `cargo fmt --all --check`, `cargo build` and
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
  **269 passed / 0 failed / 13 ignored** (the 13 ignored tests are exactly the
  manual hardware checks).
- **core:** `cargo fmt --all --check`, `cargo build` and
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
  **156 passed / 0 failed**.
- These are the gates the spec asks for, but they do not substitute for
  requirement 6 (real-device acceptance) and are not presented as such.

### Prepared acceptance harness (rerun start point)

`.scratch/wc05-live/` holds the acceptance automation prepared for the rerun;
its README describes each script:

- `poll.ps1` — read-only RFCOMM reachability poll; the 35-cycle log from this
  attempt is kept locally (no address is printed).
- `live-phase1-connect-overview.ps1` — Add Device (real paired list, Demo mode
  off), RFCOMM connect, Overview identity/battery/quick settings/layout editing,
  page captures, preference restore.
- `live-phase2-sound-eq-device.ps1` — sound-mode write/re-poll/restore,
  equalizer preset/custom/Reset/restore, one Comfort toggle write/restore, About
  real values.
- `live-phase3-settings-tray.ps1` — Settings theme/scale/palette/language/tray,
  Copy logs, restart persistence, notification area, close-to-tray, second
  start and Quit.
- `live-phase4-diagnostics.ps1` — three read-only probes, Review, Save ZIP
  through the native dialog, archive redaction inspection and the prefilled
  issue URL (opened, never submitted).
- `live-phase5-heartbeat-retry.ps1` — idle heartbeat past one interval and the
  Lost/Retry path through the documented LDAC link reset.

The phases were validated end to end against the built-in Demo device (they pass
there, apart from the checks that inherently need a real link: a real RFCOMM log
line and the real tray tooltip). That validation only proves the harness steps
and UIA element names; it is **not** a hardware result and no demo result is
used as acceptance evidence.

### Needed action

1. Power the A3062 on (press its power button) and keep it near the machine and
   awake for the whole run.
2. Rerun this ticket: start `.scratch/wc05-live/poll.ps1` first; when the device
   answers, run phases 1, 2, 4, then the ignored `manual_*` hardware tests, then
   phases 3 and 5. Record the firmware, the real-device screenshots (no address
   or serial), the gates and this ticket to `done`.

### Not done (all blocked by the same hardware availability)

- Real RFCOMM connect through Add Device and a real saved card; Overview
  identity, battery, quick settings, layout editing and capability filtering.
- Sound mode, Equalizer preset/custom/reset, saved-profile apply and the
  import/export roundtrip on the real device, with a re-poll after each write.
- One toggle/select/range write plus re-poll plus restore on the Controls,
  Comfort, Power and Connections pages, and the About values (firmware).
- Settings appearance/language/tray persistence and restart on a real
  connection; the notification-area Open/Quit/close-to-tray with a real device
  connected; Copy logs on a real session.
- Diagnostics three probes, Review, Save ZIP and archive redaction on a real
  capture; opening the prefilled issue URL without submitting.
- Idle heartbeat and Lost/Retry on a real link, and leaving the device as found.
- Screenshots and README updates: no new real-device capture was taken because
  there was no real device state to capture, so `desktop/README.md` and the
  committed screenshots are unchanged by this attempt.
