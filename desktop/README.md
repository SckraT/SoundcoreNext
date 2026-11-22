# OpenSCL4 2.0 desktop shell

The desktop app is the **OpenSCL4 2.0 desktop shell**: the product keeps the
OpenSCL4 name, and the UI is **vendored from OpenSCL4** under GPL-3.0; it is not
a new design. The Slint tree in `ui/` comes from
`D:\Projects\OpenSCL4\desktop\ui` at commit `01276e08` and keeps the upstream
layout, components, pages, icons, fonts and art.

## Status

- The shell compiles and opens; the window title still says OpenSCL4.
- The app is built as crate version `2.0.0`, and `AppState.app-version` is
  filled from that crate version, so the About section says
  “OpenSCL4 2.0.0”.
- The vendored **Add Device** dialog is populated from the model catalog
  (`core/crates/models/src/catalog.rs`) through `src/catalog_adapter.rs`: all
  39 supported models as cards with their name, code and device art.
- The existing category chips filter by form factor: **All**, **Earbuds**
  (TWS and sleep buds — the vendored UI has no Sleep chip), **Over-ear**,
  **Open-ear** and **Speakers**. The search field filters by model name or code.
- Choosing a model advances the existing dialog to its paired-device step with
  that model's name, code and art. The step lists the Windows-paired Soundcore
  candidates for that model — devices that advertise the model's Soundcore
  vendor RFCOMM service, or the standard SPP service when the model has none —
  and marks the likely match with “looks like the chosen model”.
- The MVP connects to real paired devices over classic Bluetooth RFCOMM. The
  Windows transport (`src/transport_windows.rs`) opens the model's vendor
  service with the standard SPP fallback; the session
  (`core/crates/session`) writes the `01 01` state request and parses the
  answer, and writes the measured A3062 `06 81` sound-mode, `03 87` equalizer
  and remaining-settings frames; **Overview** then shows the connected device's
  name, code and battery tiles (`src/controller.rs`).
- Battery tiles are left/right for the TWS models. Single-battery models show
  one whole-device tile on their own step scale, e.g. a raw `2/10` reads `20 %`.
- The **Sound mode** page is implemented for the models the core measures a mode
  for — today the Space One Pro (A3062). On connect the page shows the device's
  current mode (noise canceling, transparency or normal); choosing a tile writes
  one `06 81` mode frame, then re-reads the `01 01` state (skipping the mode
  acknowledgement) and selects the mode the device reports. A model without a
  measured sound mode, e.g. the P20i (A3949), gets no Sound mode page and no
  mode frame.
- The **Equalizer** page is implemented for the models the core reports an
  equalizer for — today the Space One Pro (A3062). On connect the page shows the
  device's current preset and its curve: the 22 catalog presets as chips (the
  current one checked) and the eight visible band values in the measured
  −12.0…+13.4 dB range on the vendored chart. Choosing another chip writes one
  `03 87` preset frame, then re-reads the `01 01` state and selects the preset,
  its name and its curve from what the device reports.
- The eight A3062 bands are **editable**, so the page is not read-only: each
  band has a value field and a slider, and there is a Reset button. Dragging a
  chart point or a slider previews the curve locally and writes nothing; letting
  go, or pressing Enter in a field, writes one `03 87` custom frame (`0xFEFE`
  and the eight visible values, the rest of the measured body copied from the
  last re-poll) and then re-reads the `01 01` state, so the chart, the name tag
  (“Your own settings”) and the fields follow the device. Reset writes a flat
  custom curve. Built-in preset chips still work and switch the device away
  from a custom curve and back.
- **Saved equalizer profiles** are implemented for the editable A3062 page: the
  vendored **My profiles** panel and its **Save profile** dialog, row list and
  delete confirmation are wired. Saving stores the eight current bands under a
  name; a saved row shows the profile's curve thumbnail, “Bands: 8” and an
  **Applied** tag while the device's bands are exactly the profile's. Clicking a
  row applies it as one custom `03 87` frame (`0xFEFE` and the profile's eight
  bands) followed by a `01 01` re-poll, so the chart, fields and Applied tag
  follow the device. The row menu's **Delete** removes the profile after the
  confirmation dialog. Profiles persist across restarts in
  `%APPDATA%\OpenSCL4\equalizer-profiles.json` (schema v1, written atomically
  through a temporary file and a rename); the file is loaded at startup, and a
  missing, unreadable or invalid document starts empty and is logged to stderr
  instead of crashing. A duplicate name asks before replacing, an empty or
  whitespace-only name is rejected with a dialog message, and out-of-range bands
  are clamped by the page's edits to the measured range and rejected by the core
  model and on document load before they can be stored. A save or delete whose
  document cannot be written is not silent: the failed change is rolled back (the
  list and the file stay as they were), the page error banner says
  “Could not save equalizer profiles” and the reason is also logged to stderr.
- **Profile import** is implemented in that panel: **Paste** reads the Windows
  clipboard and **Open file** opens a native JSON picker (files over 1 MiB are
  refused). A parseable OpenSCL4 transfer-profile document opens the vendored
  **Import profiles** dialog with one row per profile — **New**, **Replaces**
  (a saved profile has the same name), and the blocking **wrong band count** and
  **out of range** statuses — plus a red note and a disabled **Import** button
  when anything is blocked. Confirming re-validates the whole file all-or-nothing
  and merges it into the saved profiles (same-named profiles are replaced),
  writes the document and closes the dialog with the **Imported** note and toast;
  blocked files write nothing and leave the dialog open. A parse failure shows
  “This is not an equalizer profiles file” and an empty clipboard “The clipboard
  is empty”. Legacy migration stays out of scope: the legacy migration banner
  stays hidden because `eq-legacy` is always empty. HearID is out of scope too.
- **Profile export** is implemented in the same panel. The Export block lists
  the saved profiles with a checkbox each plus **Select all**, and the number of
  chosen profiles (the same count that checks **Select all**) enables **Save to
  file** and **Copy**; the profile row menu's **Save to file** item chooses that
  profile alone. **Save to file** opens the native Windows save dialog with the
  default name `openscl4-equalizer-profiles.json` and writes the chosen profiles
  as OpenSCL4 transfer JSON — one compact array of `name` plus the eight
  `volumeAdjustments` dB values with one decimal, e.g.
  `[{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]}]`.
  A written file shows the **Saved to the file** note and toast; cancelling the
  dialog changes nothing (no note, no toast, no file) and a failed write shows
  “Could not save the file”. **Copy** puts exactly the same JSON on the Windows
  clipboard and shows the **Copied** toast. The exported file or pasted text
  re-imports through the import path above, so it previews as **New** (or
  **Replaces** for names already saved) with the same bands. The deferred A3949
  has no saved-profile panel and therefore no export either.
- The **Controls**, **Comfort**, **Power and prompts**, **Sound and effects**,
  **Connections** and **About the device** pages are implemented for the
  A3062: they are filled from the settings block of the `01 01` state (body
  67..87), so the buttons, comfort and power settings, LDAC/Dolby toggles and
  About values (charging, battery, firmware; the serial is deliberately not a
  row) are the device's own. **Connections** also offers the Windows links
  **Bluetooth and devices** and **Sound**, which open the corresponding
  `ms-settings:` page in the system Settings app. Every change writes exactly
  one measured settings frame and then re-reads `01 01`, so the row, the page
  and About follow the device's answer; a control dragged or selected more than
  once only keeps the newest write. A failed write shows the readable page
  banner (“Could not change …”, with **Retry** to reconnect) instead of leaving
  a stale value. The banner means the write or its confirmation poll failed, not
  that the device kept the old value: the vendored second sentence is fixed
  wording, so the device's state is unknown until **Retry** re-polls it.
- **Dual connections** is implemented for the A3062: after connecting, while the
  device reports dual connections on, the app reads the earbuds' remembered-device
  list with a bounded multi-frame `0B 01` read and shows it on **Connections** as
  the vendored list panel — one row per remembered device with the parsed name and
  a “— connected” marker taken from the device's own connected flag (the address
  bytes are never shown, logged or put in an option string). Each row offers
  **Connect**, **Disconnect** and **Forget**; a click writes exactly one measured
  `0B 82` / `0B 81` / `0B 83` frame and then re-reads `0B 01` (the connect and
  disconnect writes retry the read for a few seconds because the earbuds take a
  moment to settle), so the list and its markers follow the device's own answer.
  With dual connections off the list is hidden and no `0B 01` is sent; turning it
  on writes `0B 84` on, re-polls `01 01`, and only then reads the list. Options and
  row actions are built from the same last reply, so an index can never name a
  different device. **Forget** is the device's own `0B 83`: it removes the row from
  the earbuds' remembered list for good (it un-pairs that device from the earbuds),
  which is why the vendored note warns that forgetting this computer ends control
  of the earbuds. The list is A3062-only; the P20i (A3949) never reads or writes a
  `0B` frame and keeps the Connections page without the list.
- The **notification-area tray** uses the vendored `ui/tray.slint` component and
  Slint's Windows `SystemTrayIcon`: the OpenSCL4 icon with a tooltip naming the
  device and, while connected, its battery (“OpenSCL4 · soundcore Space One Pro”
  over “Battery 40%”), and a menu with the device and its connection status, the
  battery line, the **Sound mode** submenu (the current mode checked; choosing one
  writes the same `06 81` frame as the page and re-reads `01 01`), the **LDAC**
  item (the same settings write as the Sound and effects row), **Connect** while a
  saved device is not connected (the same retry as the page's banner),
  **Open OpenSCL4** and **Quit**. Closing the window hides it to the notification
  area while the icon is hosted and close-to-tray is on; the icon's Open item (or
  a left click) shows it again; **Quit** ends the program and removes the icon. A
  second start of the executable finds the running instance, asks it to show the
  window and exits, so there is only ever one window and one icon. The Settings
  page's **Desktop and quick access** panel hosts the switches (the icon, the four
  menu sections, close-to-tray) and a **Quit** button; the choices live in memory
  for the run and are not persisted.
- The P20i (A3949) equalizer is deliberately deferred in this slice: the
  committed A3949 state carries an equalizer block at body 32..44, but no A3949
  `03 87` write body is verified, so the page stays hidden and no equalizer
  frame is sent for that model — the P20i therefore also has no saved-profile
  panel, no profile writes and no import or export controls, and no section
  pages or settings writes: it keeps only Overview and My devices as before.
- Connecting shows “Connecting…”, and Bluetooth off, not found, timeout and
  unsupported-layout failures each show a readable message instead of a crash.
  Cancel and Retry are wired. The vendored UI has no control that drops the app's
  own connection (the dual-connections list disconnects remembered devices, not
  the app's link); the controller's `disconnect` action, used by the tests,
  clears the current device and returns to My Devices.
- The only device writes are the `01 01` state request, the `06 81` sound-mode
  frame, the `03 87` equalizer frames (a built-in preset, the custom bands, and
  an applied saved profile's custom curve), the measured one-frame settings
  writes behind the section pages, and the dual-connections writes (`0B 84` for
  the setting, `0B 82`/`0B 81`/`0B 83` for a list row); its `0B 01` list request
  is the only other read. The only app file the shell writes is the saved-profile
  document above. Windows pairing stays in Windows settings and the app never
  changes it; **Forget** asks the earbuds themselves to drop a remembered device,
  which is the un-pairing the vendored note warns about.
- ANC strength, wind noise, adaptive ANC and the 1-to-11 strength scale are not
  part of this slice, so the Sound mode page shows only the three mode tiles.
  Legacy migration and HearID are out of scope: their vendored controls keep
  their empty or hidden state, as described above.
- Tray **Quick presets** stay out of scope: the app has no quick presets, so the
  vendored panel's preset switch is kept in memory but the menu never shows a
  preset section (and never says “No presets yet”). The tray does not change the
  sound mode beyond the three modes the Sound mode page has. The tray choices are
  not persisted, so a new run starts with the defaults (icon and close-to-tray on,
  battery/modes/LDAC sections on, presets off), and this slice has no tray on
  Linux or macOS.
- Writing the dual-connections setting or LDAC makes the real A3062 reset its
  Bluetooth links: the write can be acknowledged and the following re-poll or
  list read then hits a closed link, so the page shows the readable “connection
  lost” banner (and `0B 01` list read/`LDAC (connection lost)` names) and
  **Retry** opens a fresh connection. The same happens when the list's
  **Disconnect** is used on this computer's own row: the earbuds drop the PC link
  the app talks over, the banner says the list could not be re-read, and
  **Retry** reconnects. That is device behaviour, not a crash: every reconnect
  then read or wrote normally again.
- The shell keeps the RFCOMM link open between writes rather than polling, so a
  link that Windows or the device drops while the window sits idle is noticed on
  the next write: the page shows the readable “connection lost” banner (after
  the socket timeouts, up to about ten seconds) and **Retry** opens a fresh
  connection. During the acceptance run this happened twice on the bench while
  the window was idle between steps; every fresh connection then wrote and
  re-polled normally.
- Pages other than Overview, My Devices, Settings, Sound mode, Equalizer, Sound
  and effects and the five A3062 device pages still open in their placeholder
  state, which is expected: the A3062 has no case, sleep, lights or alarms page
  and this shell does not implement them.

## Screenshots

![Tray menu with a connected Space One Pro](tray.png)

The notification-area menu from the running app on Windows: a paired
**soundcore Space One Pro** (A3062) is connected over RFCOMM, so the icon's
tooltip reads “OpenSCL4 · soundcore Space One Pro” over “Battery 40%” and the
menu shows the device with **Connected**, the battery line, the **Sound mode**
submenu with the current mode checked (“✓ Normal”), the **LDAC** item, **Open
OpenSCL4** and **Quit**. This capture was taken with the Sound mode submenu open;
the menu was opened from the notification-area icon through its Win32 callback
message, and every action was exercised on the bench: a mode chosen from the
submenu was written and re-read (the check moved from Normal to Transparency and
back), LDAC was toggled on and off again, Open restored the window after a close,
and Quit ended the process and removed the icon. No Bluetooth address or serial
number appears in the capture; the names are the devices' own names.

![Dual connections with a connected Space One Pro](dual-connections.png)

The **Connections** page from the running app on Windows: a paired **soundcore
Space One Pro** (A3062) is connected over RFCOMM with dual connections on, and the
vendored **Connected devices** list shows the devices the earbuds remember —
**DALUNEV — connected** (this computer) and **Pixel 8 Pro** — each with
**Connect**, **Disconnect** and **Forget**. The list is read with a bounded
multi-frame `0B 01` read; the row actions write one measured `0B 82` / `0B 81` /
`0B 83` frame and re-read it. On the bench **Forget** removed **Pixel 8 Pro** from
the earbuds' remembered list (a re-read then showed only this computer) and the
row was gone; **Connect** on the disconnected phone row wrote `0B 82` and re-read
the list, but the phone did not report connected (it did not answer the earbuds'
page during the run). No Bluetooth address or serial number appears in the
capture.

![Overview with a connected P20i](overview-connected.png)

The connected Overview from the running app on Windows: a paired
**soundcore P20i** (A3949) answered the `01 01` state request over RFCOMM, and
its left and right battery tiles read 100 % each (raw 5 of 5 steps). No
Bluetooth address or serial number appears on this page or in the capture.

![Sound mode with a connected Space One Pro](sound-mode-connected.png)

The Sound mode page from the running app on Windows: a paired **soundcore
Space One Pro** (A3062) is connected over RFCOMM and its current mode (normal)
is selected; noise canceling, transparency and normal are the three choices.
Choosing another mode writes one `06 81` frame and re-reads the `01 01` state.
No Bluetooth address or serial number appears on this page or in the capture.

![Custom equalizer with a connected Space One Pro](equalizer-custom.png)

The Equalizer page from the running app on Windows: a paired **soundcore
Space One Pro** (A3062) is connected over RFCOMM and its bands are editable.
The eight band fields and sliders, the chart, the Reset button and all 22
catalog preset chips are shown. A typed custom curve (+4.0 dB at 100 Hz,
−4.0 dB at 800 Hz) was committed and re-read from the device, so the name tag
reads “Your own settings” and the fields hold the re-polled values. Typing a
value or letting go of a drag writes one `03 87` custom frame (`0xFEFE`), then
re-reads the `01 01` state; Reset writes a flat custom curve. That capture was
taken before the saved-profile panel was wired; the current page also shows the
My profiles panel, its Save profile button and the Import and export panel (see
the saved-profiles, import and export captures). Legacy migration and HearID stay
out of scope. No Bluetooth address or serial number appears on this page or in
the capture.

![Saved equalizer profiles with a connected Space One Pro](equalizer-profiles.png)

The Equalizer page after restarting the app on Windows: a paired **soundcore
Space One Pro** (A3062) reconnected over RFCOMM, and the saved profile
**Test curve** was loaded from `%APPDATA%\OpenSCL4\equalizer-profiles.json` and
listed in the **My profiles** panel with its curve thumbnail, “Bands: 8” and the
**Applied** tag (the device reported the same eight bands, so the profile is in
effect). This window was driven through the Windows accessibility tree: the
+4.0 / +2.0 / −4.0 dB curve was saved as “Test curve”, the row was clicked to
apply it as one `03 87` custom frame and a `01 01` re-poll, the app was closed
and restarted, and the profile came back from the file. Deleting it afterwards
through the row menu and the confirmation dialog removed the row and left `[]`
in the document. The vendored **Import and export** panel at the bottom is
visible; import and export are implemented now (see the import and export
captures). No Bluetooth address or serial
number appears on this page or in the capture.

![Remaining settings on a connected Space One Pro](settings-a3062.png)

The **Comfort** page from the running app on Windows: a paired **soundcore
Space One Pro** (A3062) is connected over RFCOMM and the page is filled from the
device's own settings block — **Side tone** on, the **Volume** panel with
**Limit high volume** off, **dB limit** 90 dB and **Refresh rate** Real time.
The same connection fills **Controls** (single/double press and the three modes
of the ambient cycle), **Power and prompts** (auto power off 60 minutes, low
battery and voice prompts), **Connections** (dual connections and the Windows
links) and **About the device** (charging, battery 40 %, firmware 04.39; the
serial is not shown). Toggling a row writes exactly one measured settings frame
and re-reads the `01 01` state, so values shown here are the device's answer.
The screenshot was taken after the acceptance run restored every changed value;
no Bluetooth address or serial number appears on the page or in the capture.

![Importing OpenSCL4 profiles](import-profiles.png)

The vendored **Import profiles** dialog after **Paste** read an OpenSCL4
transfer-profile document from the Windows clipboard: one **Warm** profile with
its curve thumbnail and the **New** tag, **Cancel** and **Import 1 profile**.
Confirming writes the profile into the saved set and the
`%APPDATA%\OpenSCL4\equalizer-profiles.json` document and closes the dialog;
clicking the new **My profiles** row then applies it as one custom `03 87` frame
and a `01 01` re-poll (the chart and the **Applied** tag follow the device). A
file the device cannot take — a 7-band profile or a value outside the measured
range — previews in red with **Import** unavailable, and confirming writes
nothing. The legacy migration banner stays hidden (the legacy list is always
empty). No Bluetooth address or serial number appears in the capture.

![Exporting equalizer profiles](export-profiles.png)

The Equalizer page from the running app on Windows with two saved profiles,
**Exp one** and **Exp two**, chosen for export in the vendored **Import and
export** panel: **Select all** and both row checkboxes are checked and **Save to
file** and **Copy** are enabled (`eq-export-count` is 2). The green **Saved to
the file** note is the panel's answer to a real native save-dialog export of
both profiles (150 bytes of OpenSCL4 transfer JSON); **Copy** put exactly the
same JSON on the Windows clipboard. The exported file then re-imported through
the shipped import path — **Replaces the saved profile** while the two profiles
were still saved, **New** after they were deleted — and the profiles applied to
the device as one custom `03 87` frame and a `01 01` re-poll. No Bluetooth
address or serial number appears in the capture.

![Add device](add-device.png)

The Add Device dialog with the catalog populated, captured from the running app
on Windows.

## Run

```powershell
cd desktop
cargo run
```

Closing the window keeps OpenSCL4 running in the notification area while the
icon is shown and **Close to notification area** is on; the tray's **Open** item
(or a left click on the icon) brings the window back, and **Quit** in the tray or
on the Settings page ends the program and removes the icon. Starting the
executable again while it runs only asks the running copy to show its window.
The tray and desktop switches are not persisted: they start at their defaults on
every run. The app needs the earbuds paired in Windows (pairing itself is done in
Windows settings) and never changes the pairing.

## License and attribution

GPL-3.0-or-later. The vendored files keep their upstream copyright; see
`ui/SOURCE.md` and the root `NOTICE`. The protocol command/response and
equalizer-preset data adapted from OpenSCL4 is attributed in the root `NOTICE`
too. Changes to vendored files are recorded in the commit that makes them.
