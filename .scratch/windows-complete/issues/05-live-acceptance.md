# 05 — Live acceptance

**Status:** done
**Blocked by:** 04

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

- [x] All real-device features pass and are restored.
- [x] Diagnostics archive is redacted.
- [x] Screenshots/docs updated; ticket done.

## Verification (2026-11-23)

Accepted on the machine with the real paired Space One Pro (A3062) out of its
case, 12:06–12:44 MSK. No Bluetooth address or serial number is recorded here or
in any capture; pairing was not changed. The app was driven through the Windows
UI Automation tree (`.scratch/wc05-live/`, see its README) and through the
ignored `manual_*` hardware tests, all against the actual device.

### Environment

- **OS:** Windows 11 Pro 26H2, build `10.0.26300.9974`, x64; primary display
  2560×1440 at 125 % scaling.
- **Toolchain:** `rustc`/`cargo` 1.99.1 (`rust-toolchain.toml`); debug build of
  `desktop/target/debug/soundcore-desktop.exe`, app version **OpenSCL4 2.0.0**.
- **Branch:** `ticket/wc-05-live-acceptance`; the `integration/windows-complete`
  tip (`6334ead`, including the ticket 06 review fixes `48d600a`) is merged into
  it (merge commit `0ceadb3`), so the run covers the final code.
- **Radio:** Intel Wireless Bluetooth; Windows holds the pairing and cached
  vendor/SDP services for the paired Soundcore Space One Pro (A3062), P20i
  (A3949) and Liberty 4 Pro (A3954).
- **Device under test:** soundcore **Space One Pro**, catalog **A3062**,
  firmware **04.39**, battery **30 %** (not charging), current sound mode
  **Normal**, current equalizer preset **Soundcore Signature** with flat bands,
  **LDAC off**, dual connections on. Found in that state on connect and left in
  it after the run.
- **Method:** five UIA phases plus the ignored hardware tests; the real native
  Windows save dialog for **Save ZIP**; the prefilled issue URL was read from
  the browser's address bar. During the first Diagnostics attempt the owner used
  the desktop (the window was switched to Russian Overview while the capture
  ran); the affected phases were restarted from a fresh English app and passed
  cleanly, and the final runs below are undisturbed.

### Results

**Add Device / connection (requirement 1).** A fresh app starts on My devices;
Add Device lists the 39-model catalog, the Space One Pro card is chosen, and the
paired-device step lists the Windows-paired **soundcore Space One Pro** as
“Paired in Windows · looks like the chosen model” with Demo mode off. Choosing
the row opens the real RFCOMM link (the probe opened the vendor service) and
lands on the Overview with the title “Overview · soundcore Space One Pro”.
`manual_paired_connect` also connected the A3062 and printed `firmware: 04.39`
(addresses masked by the probe).

**Overview (requirement 2).** The hero shows “SOUNDCORE · A3062”, the device
name, the Connected badge and one battery tile at **30 %**. Quick settings holds
the saved **LDAC** chip (reported “LDAC · off” by the device's own state). The
**Sound mode** panel shows the device's three tiles with **Normal** selected, and
the **Your sound** panel shows **Soundcore Signature**, 8 bands,
−12.0…+13.4 dB. The shortcut cards are Controls, Comfort, Sound and effects and
Connections in the Standard layout; no Quick presets and no Case block are
offered (capability filtering). The layout editor applied the Full preset, hid
Controls (the `!` marker was saved and the block left the Overview), restored
Standard, and the quick-toggle chooser saved the per-model Dolby selection and
restored LDAC-only, with `overview-preferences.json` removed again afterwards
because it did not exist before the run. No address or serial appeared on any
page.

**Sound mode and Equalizer (requirement 3).** Sound mode: the current mode was
read from the device, choosing **Noise canceling** wrote one `06 81` frame and
the re-poll selected it, then **Normal** was restored the same way — no error
banner. Equalizer: 22 preset chips with Soundcore Signature selected; choosing
**Acoustic** wrote one `03 87` preset frame and the re-poll moved the selection
and the band curve; typing `9` into the 800 Hz field wrote a custom curve and
the re-poll showed “Your own settings” with the written value; **Reset** wrote
the flat custom curve; the preset found on connect (Soundcore Signature) was
restored. The ignored `manual_equalizer_preset_switch`, `manual_equalizer_custom_curve`,
`manual_saved_profiles`, `manual_import_profiles` and `manual_export_profiles`
runs passed against the same device: preset switches and quick clicks settle on
the last re-polled mode, the custom curve and Reset are re-polled, a saved
profile applies as one custom frame, the profile document loads through the
startup path and is restored, import from the clipboard and from a file goes
through the real import path (a blocked file writes nothing), export writes a
file and copies the same JSON, and the exported file re-imports with the same
bands and applies. Both the device and the user's profile document were left as
found.

**Remaining settings pages (requirement 4).** `manual_settings_pages_and_writes`
read every A3062 section page (Controls, Comfort, Power and prompts, Connections,
Sound and effects, About), asserted the aside capabilities and the Windows
links, then wrote one toggle (Comfort **Side tone**), one select (Power **Auto
power off**, moved and restored by index) and one range (Comfort **Limit high
volume dB limit**, moved one measured step and restored), each confirmed by the
`01 01` re-poll; it compared the entire settings struct with the state found on
connect and they were equal. The About page copy action put the row text on the
clipboard. The UI phase additionally toggled **Side tone** through the actual
UIA control and restored it, and read **Firmware version 04.39**, battery 30 %
and the charging row; the page has no serial row and no address.

**Settings, tray and persistence (requirement 5).** Through the real Settings
page: theme Follow system/Light/Dark, palette Purple/Sage/Rose, content scale
100/110/125 %, app language English→Русский→English (the window switched
immediately both ways), the tray sections (Battery, LDAC) and **Copy logs**
(which put the real recent application log on the Windows clipboard). Quit and
relaunch restored dark theme, Sage palette, 125 % scale and Russian, and the
baseline light/Purple/100 %/English was put back afterwards. The notification
area icon was found through the hidden-icons flyout with the accessible name
“OpenSCL4 · soundcore Space One Pro” over **“Battery 30%”** from the real
device. Closing the window with Close to notification area on kept the process
alive and hid the window; a second start exited and showed the hidden window
again; **Quit OpenSCL4** ended the process (and the icon) and the app started
again cleanly. The same document held all the choices and came back after the
restart.

**Diagnostics (requirement 6).** The page showed the connected A3062, the three
read-only probes (`01 01`, `01 03`, `01 04`) and the redaction notice. **Start
capture** ran them on the real device: all three answered (State 96 ms, Battery
13 ms, Charging 12 ms), 6 frames, archive 1.2 KB; **Review** listed
`manifest.json`, `frames.jsonl` and `README.txt`; **Save ZIP** opened the real
Windows save dialog and wrote a **1176-byte ZIP**. The extracted archive was
inspected: the manifest names A3062 and has no serial field; no colon-form or
bare 12-hex Bluetooth address appears anywhere; no serial-shaped 16-hex run
survives in the decoded manifest probe responses; `frames.jsonl` holds the six
frames; `README.txt` explains the archive. **Open GitHub issue** opened the
prefilled URL in the browser (GitHub's sign-in redirect whose `return_to` is
`https://github.com/SckraT/OpenSCL4/issues/new?labels=device-diagnostics&title=[device-diagnostics]
A3062: probe capture&body=...` with the probe summary and no address); nothing
was submitted.

**Idle heartbeat and Retry (requirement 7).** With the device connected the app
was left idle for 25 s (the heartbeat interval is 15 s): the connection, the
battery tile and the Overview stayed unchanged and no error banner appeared (the
heartbeat is a read-only `01 01`). Writing **LDAC** from Sound and effects is the
documented way the A3062 resets its Bluetooth links: the app showed the readable
**Connection lost** state with **Connect again**, and Retry reconnected the real
device; the LDAC value found on connect (off) was restored through the same
write → Lost → Retry cycle and re-polled as off. The app-level **Disconnect**
returned to My devices and kept the saved card.

**Restore and close (requirement 8).** Every changed device value was restored
(the `manual_settings_pages_and_writes` run asserts the full settings struct
equals the connect-time values; the sound mode and equalizer found on connect
were restored by the UI phase and by the equalizer tests; LDAC was restored to
off). No Bluetooth pairing was changed. The app was quit and the app-data
documents were restored to their pre-run bytes
(`desktop-preferences.json` only; no Overview or profile document existed before
or after). The device was left disconnected and on the state found at connect.

### Defect found and fixed

The real archive showed that **`manifest.json` carried the A3062 serial** inside
each probe's `responseHex`: the frames were redacted at record time, but the
`ProbeResult.responseHex` was stored raw, and `redact_text` cannot see into
space-separated hex, so the state response's serial bytes survived. Fixed in the
core diagnostics session (`core/crates/diagnostics/src/lib.rs`):
`DiagnosticSession::record_probe` now masks a state probe response with the same
byte-level `redact_state_frame` used for frames before storing it and returns the
stored result, and the desktop worker sends that stored (already redacted)
result to the UI. Two tests were added
(`core/crates/diagnostics/tests/export.rs`): the manifest probe response has its
serial bytes zeroed and the serial never appears, and a redacted probe keeps its
status and timing. Re-verified on the real device: the new archive has no
serial-shaped run in the decoded probe responses, no serial in the frames or the
manifest, and the reviewer can attach it as documented.

### Screenshots

Committed as `desktop/live-overview.png`, `desktop/live-equalizer.png` and
`desktop/live-settings.png` (real A3062, firmware 04.39, battery 30 %, light
theme, no address or serial) and linked from `desktop/README.md` with captions;
the root README diagnostics and desktop status rows record the redaction fix and
the acceptance.

### Gates (final tree)

Run on the committed tree in both crate trees:

- **desktop:** `cargo fmt --all --check` clean; `cargo build` clean;
  `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
  **270 passed / 0 failed / 13 ignored** (the 13 ignored are the manual
  hardware checks, which were run separately on the real A3062 above).
- **core:** `cargo fmt --all --check` clean; `cargo build --workspace` clean;
  `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test`
  **158 passed / 0 failed / 0 ignored** across 22 suites (two new diagnostics
  redaction tests included).

### Limitations

- The only device write available for the Lost/Retry check is the documented
  LDAC/dual-connections link reset; an idle-link loss cannot be forced without
  powering the headphones off, so the Lost state was produced by the LDAC write
  and Retry restored control (the failed-heartbeat path itself is covered by the
  deterministic worker tests).
- The notification-area icon is reached through the hidden-icons flyout on this
  Windows 11 build; the tray menu itself was exercised on the real device in the
  earlier dual/tray acceptance (unchanged code) and is not re-run here beyond
  close-to-tray, the second start and Quit.
