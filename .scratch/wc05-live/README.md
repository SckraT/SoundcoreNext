# wc05 live-acceptance tooling

Scripts used for ticket 05 (real A3062 acceptance) of the Windows-complete
feature. The real device was powered off for the acceptance window, so these
scripts are the prepared harness for the rerun; the flow itself was validated
end to end against the built-in Demo device (which is not acceptance evidence).

## Reachability

- `poll.ps1` — runs the read-only `manual_paired_connect` probe every ~40 s and
  stops when the A3062 answers. Writes `poll.log`, `poll-last-cycle.txt` and
  `reachable.flag`. It only ever sends the read-only `01 01` state request, and
  only after a link opens; a powered-off device writes nothing.
- `probe-first.log` in this directory is the first single probe run.

## Acceptance phases (run in order against one app instance)

1. `live-phase1-connect-overview.ps1` — starts a fresh app, drives Add Device
   (real paired list, Demo mode off), connects the A3062 over RFCOMM and checks
   Overview identity/battery/quick settings/capability filtering/layout editing,
   captures the page shots, then restores `overview-preferences.json`.
2. `live-phase2-sound-eq-device.ps1` — Sound mode write + re-poll + restore,
   Equalizer preset write, custom band, Reset, restore, one Comfort toggle write
   + restore, About real values.
3. `live-phase4-diagnostics.ps1` — three read-only probes, Review, Save ZIP
   through the real Windows save dialog, archive redaction inspection, and the
   prefilled GitHub issue (opened, never submitted).
4. `live-phase3-settings-tray.ps1` — Settings appearance/language/tray choices,
   Copy logs, restart persistence, notification-area icon, close-to-tray,
   second start, Quit. Reconnects the device at the end unless `-SkipReconnect`.
5. `live-phase5-heartbeat-retry.ps1` — idle heartbeat past one interval, the
   Lost/Retry path (through the documented LDAC link reset), LDAC restore and
   Disconnect.

Helpers: `connect-real.ps1` (real Add Device connect), `demo-connect.ps1`
(Demo-mode connect used only to validate the harness), `dump-live-tree.ps1`
(UIA tree dump), `appdata-snapshot.ps1` / `appdata-restore.ps1` (restore the
three app documents to the pre-run state).

Run each phase from PowerShell (`pwsh`) with the app built at
`desktop/target/debug/soundcore-desktop.exe`. The scripts print PASS/FAIL per
check and exit 1 on failure. They reuse the UIA helpers from
`.scratch/wc04-uia/`.

The scripts assume the app language is English for element lookup; the run
switches English→Russian→English as part of the Settings checks.

No Bluetooth address, serial number or MAC is printed by these scripts: the
probe output masks addresses itself, the UIA checks look at user-facing names
and numbers only, and the archive inspection counts address-shaped strings
instead of printing them.
