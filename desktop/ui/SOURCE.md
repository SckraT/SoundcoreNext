# Vendored OpenSCL4 UI

- Upstream: https://github.com/SckraT/OpenSCL4
- Commit: `01276e08` (the checkout used for the port)
- Source path: `desktop/ui`
- License: GNU General Public License v3.0 (see root `LICENSE`)
- Copyright: OpenSCL4 contributors; upstream copyright and GPL-3.0 terms remain
  in place.

## What is vendored

The original tree supplied `app.slint`, `shell.slint`, `theme.slint`,
`types.slint`, `strings.slint`, `tray.slint`, `icons.slint`, `components/`,
`pages/`, `icons/`, `art/` and `fonts/`. The application adapts that shell to its
controller and Rust core (whose protocol/preset data is attributed in the root
`NOTICE`). The shell opens with empty state until the controller fills it.

## Local changes

### Connections

Commit `cd309be` ("Show and drive the A3062 dual-connections list") added the
spec's dual-connections row actions:

- `types.slint`: `connect-item(string, int)` and `disconnect-item(string, int)`
  carry a list row's id and option index, like the existing Forget callback.
- `components/settings.slint`: the list row adds Connect and Disconnect ghost
  buttons beside Forget.

### Diagnostics plumbing

The Diagnostics feature adds:

- `types.slint`: `Page.diagnostics`, `DiagnosticStep`, `DiagnosticProbe`, the
  diagnostics `AppState` fields and `Actions` callbacks.
- `strings.slint`: the Diagnostics title, slogan, description and shield icon.
- `shell.slint`: a Diagnostics navigation item in the bottom group.
- `app.slint`: the Diagnostics route. It opens without a connection and has no
  device-error banner because it is not a device settings page.
- `pages/settings.slint`: an Open diagnostics button beside Copy logs.
- `pages/diagnostics.slint`: a new page, not copied upstream code.

### Internal UI kit, restored Overview and Diagnostics styling

The current redesign keeps the upstream palette, Onest font, radii and general
visual language. It does not introduce a third-party UI framework.

- `theme.slint`: adds semantic spacing, small-radius, control-height, icon-size
  and device/metric type tokens. All dimensional tokens include content scale.
- `components/basics.slint`: adds `StatusKind`, `Stack`, `Row`, `Spacer`,
  `BodyText`, `CaptionText`, `CodeText`, `TextField`, `StatusIcon`, `ProgressBar`
  and `MetricTile`; ordinary buttons use the control-height token. Chips use the
  compact control token and elide labels so long quick toggles do not expand a
  fixed grid. Text fields use the shared surface, focus, font and radius tokens.
- `pages/overview.slint`: restores the previous client's useful Overview
  structure. On Windows the device hero contains only identity and batteries;
  capability-filtered quick toggles use compact, bounded-width buttons in their
  own panel below it. Sound mode and Equalizer panels, shortcut cards, layout
  editing and quick toggle selection follow. Wide and narrow arrangements share
  the same content; wrapping grids and scrollable dialogs keep controls
  reachable. Quick presets and case stay hidden because this core provides no
  real data for them.
- `pages/sections.slint`: sound-mode explanations use fixed icon wells and the
  shared type scale so differently shaped icons stay aligned.
- `pages/diagnostics.slint`: replaces page-local visual treatment with the
  common typography, panels, buttons, tags, status, progress and metric
  components. Review metrics are borderless, with accent values and readable
  muted labels rather than nested gray cards. The note uses `TextField` and Save
  ZIP is a soft secondary action. The device summary follows the Overview
  sage-card language; wide layouts use a main column and supporting aside,
  narrow layouts one column. Probe rows, metrics and actions adapt without
  changing the existing diagnostics callbacks or capture/export logic.
- `app.slint`: Overview and Diagnostics wide breakpoints account for
  `Theme.content-scale`.
- The existing `types.slint` Overview models and `strings.slint` display strings
  are reused; the Rust controller now fills and updates them.
- `pages/add_device.slint`, `pages/equalizer.slint` and `pages/presets.slint`:
  their remaining standard `LineEdit` controls now use the shared `TextField`,
  including accessible labels and the existing edited/accepted behavior.
- `components/settings.slint`: action and dual-connection row controls use layout
  alignment instead of manual child `y` coordinates managed by the parent.
- Accessibility state coverage: `Button`, `Chip`, `Toggle`, `CheckRow`,
  `RangeSlider`, the `Segmented` options, the Settings palette `Swatch`, the
  `Select` popup rows, the sound-mode tiles and the preset editor's settings
  rows (`pages/presets.slint` `FieldRowView`) now bind `accessible-enabled`
  and/or `accessible-checkable` next to their existing labels and checked state.
  Slint only exposes a disabled state when it is bound, and only exposes a
  checked state on an item marked checkable, so without these bindings an
  assistive-technology client saw disabled actions as available and switches,
  check rows and selected options as stateless. `RangeSlider` also implements
  `accessible-action-set-value` through the same clamped, step-snapped commit as
  a drag, so the range value its accessibility properties advertise is
  settable. No visual, callback or device behavior changed.
- `app.slint`: the section (device-settings) pages choose their two-column
  arrangement with `860px * Theme.content-scale`, like the Overview and
  Diagnostics, so a raised content scale stacks the aside instead of squeezing
  the main column.
- `components/settings.slint`: a dual-connections list row's remembered-device
  name elides (with `min-width: 0px`) before the Connect/Disconnect/Forget
  actions, so a long name or a large content scale cannot push an action out of
  the panel. No callback or device behavior changed.
- `pages/devices.slint`: the connected saved-device card adds a non-destructive
  Disconnect ghost action beside Open; the card remains available for reconnect.
  Its status line marks a simulated (Demo-mode) card as `Connected · Demo
  device` or `Demo device · Not connected`, driven by the existing
  `DeviceCard.demo` field, so a simulated device stays recognisable on My
  Devices while connected.
- `app.slint` (review fixes): the Equalizer, Presets and Settings pages now pick
  their two-column arrangement with the same `900px * Theme.content-scale` rule
  as the Overview, Diagnostics and the section pages, so scaling the content
  stacks those pages instead of squeezing the main column. No content or
  callback changed.
- `pages/overview.slint` (review fixes): the device identity uses layout
  alignment (`cross-axis-alignment`) and the spacing, radius and control tokens
  instead of manual child `y` coordinates and raw 9/6/3 px values, so it stays
  centred and scales cleanly at 110/125/150 %.
- Content scale 150 %: `pages/settings.slint` offers `100 %`, `110 %`, `125 %`
  and `150 %` (the extra option is wired through the existing
  `set-content-scale` callback and Rust `ContentScale` mapping); `types.slint`
  comments record the four choices.
- `UI_KIT.md`: records the reusable component, state, responsive, persistence
  and future Compose mapping contracts.
- `GUIDE.md`: adds the practical component/page workflow, local visual checks and
  the Windows release checklist. The project agent workflow is in
  `.agents/skills/use-soundcore-ui-kit/SKILL.md`.

The old Overview layout serialization is intentionally retained. The UI-only
Overview preferences and the separate desktop/tray preferences documents do not
contain device state. The desktop controller restores tray choices before icon
creation and performs serial read-only idle heartbeats without automatic
reconnect. No hardware acceptance claim is implied by this source record.

- `components/localized.slint` (screenshot-tool follow-up): the setting-row names,
  option values, About values and sound-mode names that the Rust model supplies
  as English data are translated in the UI layer through `LocalizedText`, keyed
  by the stable setting id or protocol value. This keeps protocol identifiers
  and the Rust command tables untouched while letting the Russian UI show
  `Однократное нажатие`, `Шумоподавление`, `Прошивка` and similar. `Select`
  translates only values it knows, so native language names in Settings remain
  in their own language. The Diagnostics demo banner, status and firmware/battery
  meta line are localized in the controller because they are composed from live
  values.
- `tools/ui-capture/` (outside the vendored tree): a reusable PowerShell + UIA
  capture runner and Pillow contact-sheet tool for the Russian demo matrix. It
  does not change the UI; `desktop/translations/ru/...po` gained the strings the
  screens exposed as untranslated.
- A3954 (Liberty 4 Pro) sound-mode scale and settings rows: the vendored
  `components/modes.slint` already had the 1-to-11 `ModeScale`; its `ModeRow`
  now also renders `RowKind.toggle` rows (a label, the localized description and
  the shared `Toggle`), with `LocalizedText.setting-label` as the label fallback,
  so the A3954's wind-noise suppression is a real row under the scale. The row
  uses the vendored layout vocabulary (`Theme.space-*` spacing and cross-axis
  centering) instead of manual child coordinates. No callback or existing A3062
  tile/row behaviour changed. The A3954's own setting rows reuse the existing
  `SettingsPanelView` and row kinds; only the labels and option values new to
  the model were added to `components/localized.slint` (`Wind noise
  suppression`, `Spatial audio`, the easy-chat/limit-high-volume rows, the
  left/right/case battery and firmware About ids, and the `5/10/15 seconds`,
  `10/20/30/60 minutes` and spatial-audio mode values). The disabled spatial
  select rows' device reason (`Turn on spatial audio first`) is translated the
  same way through `LocalizedText.setting-reason`, because it is English data
  from the controller. `desktop/translations/ru/...po` gained their Russian
  translations. The controller fills the rows and owns every device write; the
  view still renders state only. The A3954's sound-mode block is optional: when
  the device reports an unmeasured value for it, the controller hides the scale
  and the page, so `modes.slint` only ever renders a measured scale.
- A3954 controls and case pages: the vendored `PanelKind.presses`,
  `PanelKind.gestures`, `PanelKind.case-screen` and `PanelKind.case-status` had
  no controller producing them, so the controller now fills the Presses,
  Slides and Case panels with the existing `SettingRowView` row kinds. The only
  vendored-component changes are additive: `components/settings.slint`'s
  `PanelText` gains the `Slides` title and its wave icon for
  `PanelKind.gestures`, and `SettingText.short-label` no longer shortens the
  side-specific gesture ids (`leftSinglePress`, `rightSlideUp`, …) to the
  A3062's side-less `Single press` wording: this app shows both earbuds in the
  one panel, so the side stays in the row label and the twelve names live in
  `components/localized.slint` with the rest of the runtime labels (the case
  feature and language ids, the gesture actions, `Japanese`/`English`/`Chinese`).
  `desktop/translations/ru/...po` gained their Russian translations. No visible
  layout, token, callback or A3062 behaviour changed. The controller edits the
  gesture action byte's live connection slot (the `01 01` TWS flag: low nibble
  when the earbuds are TWS-connected, high when they are not) and keeps the
  other nibble; the core's `07 87` command body uses the Soundcore app's flag
  order, which differs from the state order.
- A3954 controls/case review (ticket 04): each Controls gesture row now offers
  `None` (the measured `0x0F` no-action value) plus its measured action catalog,
  and the four slide rows gain that choice too because the A3954 accepts
  `IndividualDisable` for all twelve gestures. A failed case-screen write's
  banner names the flag row the user toggled (`Find device`) instead of the
  panel title, and the case-language select follows the runtime-label mapping
  like its row (localized names in the Russian UI), unlike the Settings page's
  app-language picker, which keeps each language's own name. `UI_KIT.md` records
  the restored "do not translate protocol identifiers or product names" rule and
  scopes the own-language rule to the app-language picker. No layout, callback
  or device behavior changed beyond the extra `None` choice and the banner
  wording; the controller still derives every row from state and owns every
  device write.
- UI-polish layout tickets 01 and 02 (breadcrumb, one Diagnostics entry point,
  flow-based setting rows):
  - `shell.slint`: the `PageHeading` breadcrumb is a normal horizontal layout
    child; `cross-axis-alignment: center` centres the device-name `TextLink`,
    the chevron `Icon` and the section name, and the chevron no longer carries a
    manual `y`. The chevron is therefore centred by the layout at every content
    scale instead of being positioned with an offset the layout also manages.
  - `pages/settings.slint`: the Settings aside's `AsidePanel` "Diagnostics" card
    is removed, so Settings no longer duplicates the sidebar's Diagnostics route
    or its Copy logs action; imports that only that card used are dropped. The
    page keeps the Showcase, Home screen and About cards.
  - `pages/diagnostics.slint`: a compact support `Row` at the bottom of
    `DiagnosticsPage` offers **Copy logs** through the existing
    `Actions.copy-logs()` on every step, so Diagnostics is the one entry point
    for the app log. No callback, controller or translation document changed:
    the label reuses the existing `Copy logs` message.
  - `components/settings.slint`: `SettingRowView` is a `VerticalLayout` in normal
    flow instead of a `Rectangle` with an absolutely placed divider and a fixed
    non-list `min-height: 64px`. The divider (`Divider`, drawn only for
    non-first rows) is the first child, the content row follows it, and the
    row's height comes from its content, so a wrapped label, description or
    device reason pushes the next divider down instead of drawing over it.
    Toggle, select, range/hue, info/copy, action and time controls keep their
    behavior and are centred with `cross-axis-alignment: center` in the content
    row (and in the small groups that used to position them with manual `y`);
    the list row keeps its device cards and stays an unconditional (empty when
    not a list row) flow child. No row kind, callback or device write changed.
  - `.scratch/ui-preview`: a `settings-rows-long` capture state renders every row
    kind with a wrapped label and a wrapped device reason at 800×700 and
    1280×820, 100 % and 150 %, plus a list row after a normal one; it is the
    fixture for the "text never crosses a divider" acceptance. `make_ui_sheets.py`
    groups those captures into their own sheet.
  - `UI_KIT.md` records the flow-divider/auto-height rule; `GUIDE.md` records the
    same rule as the way to change row layout and points at the long-string
    preview fixture; `README.md` no longer places Copy logs on Settings.
