# 04 — Demo acceptance

**Status:** done
**Blocked by:** 02, 03

## Goal

Use the demo device to accept the finished Windows UI without hardware.

## Requirements

1. Launch a fresh build; Add Device → select A3062 → Demo mode → connect.
2. Exercise Overview (layout presets, reorder/hide, quick-toggle chooser, chips),
   Sound mode, Equalizer (preset/custom/reset/profiles/import/export preview),
   every Settings page and device page, My Devices disconnect/reconnect, tray
   open/close/quit, Settings appearance/language/copy logs.
3. Change theme (light/dark/follow), palette, content scale (100/110/125 in this
   ticket; ticket 06 added 150), device color, language (RU/EN), one-earbud
   badge; verify persistence after restart.
4. Capture the required visual matrix at 800×700, 1030×760, 1280×820 and
   1440×900, at 100/125/150%, light/dark; check long strings and no clipping.
5. Verify Diagnostics demo messaging and that no ZIP/issue is produced.
6. Fix any defects found (controller/UI) in the same ticket.
7. Record OS, app version, demo device identity, screenshots and gate results.
   No MAC/serial in docs/images.

## Acceptance

- [x] All UI surfaces work with the demo device.
- [x] Settings + preferences persist across restart.
- [x] Visual matrix clean.
- [x] Desktop/core gates green; ticket done.

## Verification

### Environment and build

- Windows 11 Pro 26H2, build 10.0.26300 (display scaling 125 %), x86_64.
- App version **OpenSCL4 2.0.0** (`AppState.app-version` from the crate version);
  fresh `cargo build` of `desktop` on `ticket/wc-04-demo-acceptance` based on
  `integration/windows-complete` @ `eec50dc`.
- Demo device identity: the simulated **soundcore Space One Pro (A3062)** with the
  neutral all-zero placeholder address and the name `soundcore Space One Pro ·
  Demo`; its state is parsed from the committed sanitized capture
  (`core/data/a3062-state.json`). No real address, serial number or device name is
  used or recorded anywhere in this document or in the screenshots.
- No Bluetooth hardware was used, no RFCOMM link was opened and no device session
  was created during this ticket. The Windows paired-device list seen on the
  Add Device step before Demo mode is switched on is a read-only Windows
  enumeration; the demo path never looks a device up or connects to one. Demo
  controls travel as `DeviceCommand`s to the in-process demo worker, which
  answers them from its local simulated state; none of them leaves the process
  and no frame is written to a device.

### Method

The running build was driven through Windows UI Automation (PowerShell
`UIAutomationClient`, `WindowPattern`/`Invoke`/`Toggle`/`RangeValue`/
`Value`/`ExpandCollapse` patterns, plus user32 mouse and keyboard for wheel
scrolling and typed fields). The scripts live in `.scratch/wc04-uia/`:
`uia-lib.ps1` (UIA helpers, `PrintWindow` captures with `SetProcessDPIAware`),
`session.ps1` (app launch, element lookup, scroll/toggle helpers),
`check-settings.ps1` (appearance, language, Copy logs, tray switches, prefs
document) and `check-demo-final.ps1` (the end-to-end demo flow, log in
`.scratch/wc04-uia/state/check-demo-final.log`). The visual matrix uses the
documented software-rendered preview harness (`.scratch/ui-preview`) and
`.scratch/make_ui_sheets.py`.

Evidence is local in `.scratch/wc04-uia/shots/` and `.scratch/ui-validation/`
(ignored generated output); the three screenshots linked from the README are
committed as `desktop/demo-overview.png`, `desktop/demo-diagnostics.png` and
`desktop/demo-settings.png`.

### Functional results with the demo device

`check-demo-final.ps1`: **67 checks, 67 passed, 0 failed** on the final build.

- **Add Device → Demo mode → connect**: the model picker (39 models, search and
  category chips) chooses Space One Pro; the Demo mode switch replaces the real
  paired list with the one simulated row (“Simulated device data”); connecting
  needs no pairing and lands on the Overview with the device name, code, a
  single-battery tile (20 % from the capture) and the Connected badge. No
  Bluetooth address appears in any window text.
- **Overview**: hero, Quick settings chips, the edit dialog's Minimal/Standard/
  Full presets, per-block hide (`!` marker in the document) and move-up/down
  reorder all work and are saved in `overview-preferences.json`; the quick-toggle
  chooser writes only the per-model selection, and an LDAC chip click writes the
  simulated setting (the Sound and effects summary followed the answer). No
  device file or real write is involved.
- **Sound mode**: the three tiles are offered and choosing Transparency/Normal
  is accepted and re-polled from the simulated state.
- **Equalizer**: all 22 preset chips; a preset change; typing a band value
  creates a custom curve (“Your own settings”) whose re-polled value is shown;
  Reset writes a flat custom curve; a profile saves (document checked), applies
  and deletes through the row menu and the confirmation dialog; Paste from the
  clipboard shows the import preview (“Preview only”, New, Import 1 profile) and
  Cancel writes nothing; export selection enables Copy, which puts the transfer
  JSON on the clipboard; the row menu also offers Save to file.
- **Device pages**: Controls (Double press select + mode-cycle toggle),
  Comfort (Side tone toggle, dB limit range, Refresh rate select), Power (Auto
  power off select, Low battery prompt toggle), Connections (dual-connections
  toggle and the two simulated remembered devices with Connect/Disconnect/
  Forget; no address shown) and About (charging, battery 20 %, firmware 04.39;
  the serial is not a row and no address is shown) each changed a value that the
  simulated device reported back.
- **My Devices**: the live card is marked “Connected · Demo device”, Disconnect
  keeps the saved demo card (marked “Demo device · Not connected”) and stops the
  demo worker, Connect reconnects and starts from the captured state again
  (LDAC back to off, i.e. demo writes do not persist).
- **Settings**: theme Follow system/Light/Dark, palette Purple/Sage/Rose,
  content scale 100/110/125, device art color As the theme/Silver/Black, language
  English/Russian (the window switched immediately and back), Copy logs put the
  real recent log on the clipboard, and the one-earbud badge is not offered for
  the A3062. The sound-mode scale view is not offered for any device: no core
  model exposes the 1–11 scale yet, so the choice is deferred (the Settings
  control stays hidden via `app-mode-view-applies=false`; ticket 06 records the
  deferral). Theme, palette, scale, art color,
  language, tray choices and the Overview layout/quick-toggle selection survived
  a full quit + relaunch; the same choices are in
  `%APPDATA%\OpenSCL4\desktop-preferences.json` / `overview-preferences.json`.
- **Tray**: the notification-area icon's tooltip names the demo device and its
  battery; the menu shows the device with Connected, the battery line, the Sound
  mode submenu (the current mode checked, choosing Transparency wrote the
  simulated mode and the reopened menu checked it), LDAC, **Open OpenSCL4** and
  **Quit**; a left click restored a window hidden with close-to-tray; the
  Settings tray switches persisted in the same document. Quit was exercised
  through the Settings **Quit OpenSCL4** button.
- **Diagnostics**: after a demo connect the page shows “Demo mode cannot collect
  diagnostics. No probes are run and no archive is written.”, **Start capture**
  is disabled, and no **Save ZIP** / **Open GitHub issue** action exists; no
  archive or issue was produced. The device summary is still shown.
- **One-earbud badge** stays hidden for the single-battery A3062 both without a
  device and while connected.

### Defects found and fixed in this ticket

1. **Assistive-technology state was missing** in the shared controls: disabled
   buttons were announced as available and switches, check rows, segmented
   options, palette swatches, `Select` rows and sound-mode tiles reported no
   selected/checked state, because Slint only exposes those states when
   `accessible-enabled`/`accessible-checkable` are bound. Fixed in
   `components/basics.slint`, `components/settings.slint`,
   `components/modes.slint`, `components/select.slint`, `pages/settings.slint`
   and `pages/presets.slint`. Verified through UIA on the fixed build: the
   Diagnostics **Start capture** reports disabled, the tray switch, the Battery
   check row and the selected theme/palette options report their state
   (`TogglePattern` On/Off), and every chip/row still activates.
2. **The device-list rows overflowed their panel** on the Connections page at
   125 % and 150 % content scale: the section pages chose the two-column
   arrangement with an unscaled width test while the scaled rows no longer fit,
   and the remembered-device name did not shrink, so Connect/Disconnect/Forget
   were cut off. Fixed by using `860px * Theme.content-scale` for the section
   pages (`app.slint`) and by eliding the row name (`min-width: 0px; overflow:
   elide`) in `components/settings.slint`. Verified in the final preview matrix
   at 800×700, 1030×760, 1280×820 and 1440×900 for 100/125/150 %, and in the real
   app at 125 % content scale.
3. **The range slider announced a settable value but UIA SetValue did nothing**
   (the pattern existed, the write did not). Fixed with
   `accessible-action-set-value` on `RangeSlider`, which clamps and snaps to the
   step and sends through the same commit path as a drag; verified 90 → 85 → 90
   dB with the visible value following.
4. **The connected demo card on My Devices did not identify the device as
   simulated** (“Connected” only, against ticket 03's “My devices shows Demo
   device”). Fixed to “Connected · Demo device” (`pages/devices.slint`).
5. **README drift**: the Settings page was undocumented and the Demo-mode
   paragraph was imprecise (it contained the placeholder demo address, described
   the worker loosely, and described the My Devices marking loosely). The Status
   section no longer describes the shell as a compile-only WIP. The README now
   documents every Settings control and its persistence, the exact demo
   behaviour, three demo-mode screenshots, and a caveat for strings newer than
   the bundled translation documents. One screenshot caption still claimed no
   device worker existed (the demo uses an in-process worker); ticket 06
   corrected that caption and the Demo-mode wording.

No controller/device-flow defect was found: the demo event/command path fills
every page and re-polls exactly like the real worker, and no frame is written to
a device and no Windows device lookup happens.

### Visual matrix

170 captures from the preview harness: 800×700, 1030×760, 1280×820 and
1440×900 at 100/125/150 % content scale, light for Overview, My Devices,
Controls, Comfort, Power, Connections, About, Sound and effects, Settings,
Sound mode and Diagnostics, plus dark Overview/Sound mode/Settings/Controls/
My Devices/Diagnostics states and the layout, quick-toggle, Add Device,
Save-profile and Create-preset dialogs at 800 and 1280 for 100/150 %. The
software renderer runs under the machine's 125 % display scaling, so the PNGs
are physical pixels of the same logical layout. Captures were inspected page by
page: no horizontal clipping, no content outside its panel, long labels elide or
wrap, and dialog actions stay reachable at 800×700 and 150 %. The Connections
regression above was found this way and re-verified in the final matrix.
The app's own Settings control offered 100/110/125 % content scale at the time of
this ticket (UI-kit contract); the 100/125/150 % matrix is rendered through the
harness as the ticket specifies. Ticket 06 added the missing 150 % choice so the
app control matches the contract.

### Gates

On the final tree, from `desktop`:

- `cargo fmt --all --check`: clean.
- `cargo build`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: **269 passed, 0 failed, 13 ignored** (the same manual
  real-device checks as tickets 01–03; no hardware was run).

From `core` (untouched by this ticket, run for the slice):

- `cargo fmt --all --check`: clean.
- `cargo build --workspace`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test`: **156 passed, 0 failed, 0 ignored** across 22 suites.

### Caveats

- **Demo only.** These results are not hardware acceptance: a simulated device
  has no radio, so sound quality, real settings writes, LDAC behaviour, the
  idle-link heartbeat and the real Bluetooth diagnostics capture remain ticket
  05. No hardware acceptance is claimed anywhere.
- Demo writes change the simulated state only and are not persisted by design;
  each connect starts from the committed capture.
- The tray icon lives in the Windows 11 hidden-icons overflow; its tooltip,
  menu, Sound mode submenu and Open action were verified through UIA, and the
  tray switches were verified through Settings and the preferences document.
  Automated right-clicking of the overflow icon is not fully reproducible, so
  the LDAC menu item was not re-verified this run; the tray itself was accepted
  on real hardware in the earlier dual-connections/tray slice.
- Strings added after the bundled translation documents were imported (the
  Diagnostics panel and the Demo-mode copy, including the new
  “Connected · Demo device”) fall back to English in every language; the
  language choice itself applies to the whole window. Translating 15 languages
  is not part of this ticket.
- Windows display scaling on this machine is 125 %; the per-page captures are
  physical pixels at that scaling (logical layouts are the requested sizes).

## Integration

The ticket branch is based on the `integration/windows-complete` tip
`eec50dc`. The work is committed as `25717bb` (“Accept the Windows client on the
demo device and fix what it found”). The tip had not moved when the ticket
finished, so `git merge integration/windows-complete` reported “Already up to
date” and there is no merge commit to make; the gates were re-run on the
committed tree afterwards and are the results recorded above.
