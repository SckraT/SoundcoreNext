# OpenSCL4 UI kit

This is the internal design-system contract for the Windows Slint client and the
future Android client. It is intentionally small: keep the existing OpenSCL4
visual language and extend the shared components instead of adding a third-party
UI framework or creating page-local copies of common controls.

The Windows implementation lives in `theme.slint` and `components/basics.slint`.
The Overview and Diagnostics pages are the reference consumers. Older controls
may still contain their original fixed dimensions; new code and touched layouts
should use the tokens below rather than adding another spacing or type scale.

## 1. Tokens

All dimensional tokens are Slint lengths and already include
`Theme.content-scale`. Do not multiply them by the scale a second time.

| Role | Tokens | Use |
| --- | --- | --- |
| Spacing | `space-1` … `space-8` | 4, 8, 12, 16, 20, 24, 28 and 32 px at 100 % |
| Shape | `radius-small`, `radius-chip`, `radius-control`, `radius-card`, `radius-panel` | Small indicators through page panels |
| Controls | `control-height-compact`, `control-height` | Chips and ordinary actions |
| Icons | `icon-small`, `icon-medium`, `icon-large` | Inline, ordinary and hero icons |
| Type | `font-caption`, `font-small`, `font-body`, `font-section`, `font-title`, `font-device`, `font-metric`, `font-slogan` | Semantic text roles |

The existing color tokens remain the source of truth. Use `bg`, `surface`,
`container`, `text`, `muted` and `line` for neutral surfaces; `primary` and its
`on-*`/`container` companions for interactive emphasis; `sage`/`on-sage` and
`pink`/`on-pink` for the established informational cards; `green` and `error`
for results. Every colored foreground must use the matching `on-*` token.
Onest remains the UI family. Consolas is reserved by `CodeText` for commands,
file names and other literal identifiers, not for general body copy.

## 2. Composition and component contracts

| Slint component | Responsibility | Important properties |
| --- | --- | --- |
| `Stack`, `Row` | Vertical/horizontal composition with the standard rhythm | `gap`, ordinary Slint layout properties |
| `Spacer` | Deliberate empty vertical space | `size` |
| `BodyText`, `CaptionText` | Wrapping body and supporting copy | `secondary` controls emphasis |
| `CodeText` | Eliding literal text | `secondary` |
| `TextField` | Single-line input with the app surface and focus ring | `text`, `placeholder`, accessible `label`, `edited`/`accepted` |
| `Button` | One action, keyboard and pointer accessible | `kind`, `enabled`, optional `icon`/`has-icon` |
| `Chip` | Selection or a compact immediate action | `selected`, `enabled`, `on-card` |
| `Toggle` | One boolean setting | `checked`, `enabled`, required accessible `label` |
| `Panel`, `PanelHeading` | Standard surface and section heading | `inner-padding`, heading `text`/`icon` |
| `Banner`, `Note`, `Tag` | Page feedback, explanatory note and compact metadata | Semantic color properties |
| `StatusIcon` | Accessible non-interactive operation state | `status`: `neutral`, `pending`, `success`, `error` |
| `ProgressBar` | Clamped determinate progress | `progress` in 0…1, optional `thickness` |
| `MetricTile` | One prominent value and a short label | `value`, `label`, `compact` |
| `ScrollArea` | Reachable overflow in pages and dialogs | Bind content width to `visible-width` |

### Control states

- **Button kinds:** `normal` is a neutral outlined action, `primary` is the main
  action, `soft` is an emphasized secondary action, `ghost` is low-emphasis and
  `danger` is destructive. Disabled buttons retain their wording at reduced
  opacity and do not fire from touch, accessibility or keyboard focus. Hover and
  focus are separate states; do not replace the focus ring with a hover fill.
- **Chips:** `selected` describes the current value, while `enabled` describes
  whether the user may act. `on-card` selects foreground and border colors that
  remain readable on a sage device card. Labels elide inside fixed grid cells;
  provide a nearby explanation for a disabled quick toggle.
- **Toggles:** never hide a device failure by optimistically changing `checked`.
  The controller writes the setting, re-reads device state and only then updates
  the model. An unavailable toggle explains why in adjacent text.
- **Mode rows:** the sound-mode page's rows under the mode control support a
  select, a range and a toggle. The A3954 wind-noise suppression is such a
  toggle and writes through the normal `set-toggle` path with a `01 01` re-poll;
  it is never applied optimistically.
- **Status and metrics:** `StatusIcon` and `MetricTile` are displays, not buttons.
  Metrics are borderless within their parent panel: use an accent-colored value,
  a muted `font-small` label and no second gray card background. Use
  `StatusKind.pending` while work is running and `success`/`error` only from
  the actual result. Progress and metrics must not imply data the core did not
  provide.
- **Assistive state:** a shared control binds the state its visual already shows.
  Bind `accessible-enabled` from `enabled`, and `accessible-checkable` before
  `accessible-checked`/`accessible-value`. Slint exposes neither a disabled state
  nor a checked state unless the corresponding accessibility property is bound,
  so a disabled action or a switch would otherwise be announced as available and
  stateless. A component review or a screenshot does not catch a missing binding:
  check the state through the accessibility tree as well.

Prefer one reusable component with an explicit `wide` or `compact` property over
copying a control for another page. Layout variation belongs around the shared
content, not in duplicated action or state logic.

## 3. Responsive Windows layout

- The shell chooses wide Overview, Equalizer, Presets, Settings and Diagnostics
  layouts only when usable content width exceeds `900px * Theme.content-scale`;
  the section (device-settings) pages use `860px * Theme.content-scale` for
  their main/aside split. Both branches render the same domain content in a
  different arrangement.
- Wide pages may place large panels in two columns and keep a supporting aside
  aligned to the top rather than stretching it to the main column's height.
  Narrow pages use one column; larger action groups stack vertically. The outer
  page `ScrollArea` keeps every action reachable. Every two-column page decides
  on the *scaled* usable width (the same `px * Theme.content-scale` rule as the
  Overview), so raising the content scale stacks the aside instead of squeezing
  the main column narrower than its rows. A row's label shrinks and elides
  before its actions do: a long device or setting name must never push a button
  out of its panel.
- On Windows, quick settings occupy a separate panel below the device hero (and
  the one-earbud notice when present), not the sage device card. Their grid uses
  the compact control height, left-aligned rows and cell widths clamped to
  156–204 px at 100 % scale, with at most six columns wide and three narrow.
  The heading's `+` action opens the selection dialog; an empty selection offers
  an explicit add action. Shortcut and metric grids similarly derive their
  column count from available width and scale. Grid children have explicit cell
  widths; labels must elide or wrap rather than push the grid wider.
- Dialog lists own their vertical overflow with `ScrollArea`; dialogs must not
  grow past the window when all blocks or toggles are present.
- Validate changes at 800×700, 1030×760, 1280×820 and 1440×900, at content scale
  100 %, 125 % and 150 %, in light and dark palettes and with long translated
  strings. A passing layout has no horizontal clipping and leaves primary
  actions reachable by scrolling and keyboard.

## 4. Overview domain contract

The Slint page is a renderer. `src/overview.rs` decides what the connected model
actually supports and produces `AppState.overview-rows`, `quick-toggles`,
`all-toggles` and `layout-blocks`.

### Capability filtering

- Sound mode and equalizer require their corresponding state: the A3062's
  `sound_mode`/`equalizer` blocks or the Liberty 4 Pro's own `a3954` block.
  Controls, Comfort, Sound and effects, Power, and Connections require a
  settings surface: the A3062 `settings` block or the A3954 `a3954` block. The
  A3954 also exposes the Case page, because its state always parses the measured
  case features and language; the A3062 has no case block and no Case page.
- The A3954 Controls page shows both earbuds on one page: the eight measured
  presses (left/right single, double, triple and long) as select rows in the
  Presses panel, the four measured slides as select rows in the Slides
  (`PanelKind.gestures`) panel, and the ambient-sound-mode cycle panel below.
  Every gesture row names its side and writes exactly one measured `04 81`
  frame on change; it edits the action byte's live slot (the `01 01` TWS flag
  names it: the low nibble while the earbuds are TWS-connected, the high nibble
  while they are not) and keeps the other nibble, then re-polls `01 01`. The
  case-page toggles write `07 87` with the other three measured flags kept, and
  the language row writes `07 8A`; the `07 87` body is in the Soundcore app's
  command order (`[remote camera, find device, case spatial audio,
  atmospheric]`), which is not the state's flag order and is handled by the
  core command body. The case status (battery and firmware) is read-only and
  never shows the case serial.
- The A3954's ambient sound is one 1–11 scale, not three tiles: the Sound mode
  page sets `mode-scale` and `scale-position` from the device and shows its
  wind-noise suppression as a `ModeRow` toggle. The A3062 keeps the three tiles
  and `mode-scale = false`.
- The A3954 sound-mode block is optional. When the device reports a value this
  slice has not measured for it (the real device reports an unmeasured slider
  byte in airplane mode), `mode-scale` stays false, the mode rows are empty and
  the Sound mode page and Overview block are hidden; the equalizer and the
  device pages stay available instead of the connection failing. Do not invent
  a scale position or send a `06 81` frame without a measured block.
- A disabled row's device reason is English data from the controller and is
  localized by the UI through `LocalizedText.setting-reason` when it is one the
  kit knows (for example `Turn on spatial audio first`); unknown reasons pass
  through unchanged.
- Quick presets and the Overview's case shortcut stay unsupported until the core
  supplies real data and working behavior. Do not create decorative
  substitutes. The A3954's case settings are a real Case page, not an Overview
  block.
- A model with no supported block keeps its device identity and batteries but
  gets no invented shortcut or quick toggle.

### Layout and persistence

The serialized layout retains the previous desktop/Android representation:
comma-separated IDs, with `!` marking a hidden block. The IDs are `soundMode`,
`equalizer`, `quickPresets`, `controls`, `comfort`, `spatial`, `power`,
`connections` and `case`. Unknown and duplicate IDs are ignored; newly added
blocks are appended with default visibility. The legacy `spatial` ID now labels
Sound and effects where the measured device exposes LDAC/Dolby instead of a
spatial-audio setting.

Large panels group at most two to a row. Consecutive shortcut blocks form one
responsive row model. Reordering through the editor operates on the
supported-only list while unsupported entries remain in stored order. Minimal,
Standard and Full are presets, not separate layout implementations.

Quick-toggle selection is per catalog model code and defaults to LDAC only when
that model supports it. The A3062 offers its settings ids; the A3954 offers its
own switch set (LDAC, spatial audio, wearing detection, easy chat, sound-leak
compensation, limit high volume, low-battery prompt, dual connections and the
mode-cycle bits), and stored ids the connected model does not have are filtered
out. Changing the selection writes no device command;
clicking a displayed chip still uses the normal `set-toggle` device workflow.
Layout and selection persist in
`%APPDATA%\OpenSCL4\overview-preferences.json`, separately from equalizer
profiles. Device state is never stored there.

### Desktop shell actions and preferences

The connected saved-device card exposes app-level **Disconnect** beside Open. It
is a non-destructive ghost action: the saved card remains available for a later
Connect. Do not reuse the earbuds' remembered-device Forget semantics for it.

Tray visibility, close-to-tray and tray section choices persist in
`%APPDATA%\OpenSCL4\desktop-preferences.json`. This document is separate from
Overview and equalizer profiles, contains no device state, and is loaded before
icon creation. Missing fields use defaults; missing or malformed documents do
not prevent startup. Saves use a temporary sibling and rename.

While a supported connected worker is idle, its serial read-only heartbeat may
transition the page to the existing **Connection lost / Connect again** state.
The view must not start reconnects, invent recovered state or optimistically
restore controls; Retry remains an explicit user action.

### Runtime labels

Static UI copy uses `@tr`. Names and values that come from the device model are
English data at the protocol/controller boundary; the UI translates the ones it
knows through `components/localized.slint` (`LocalizedText.setting-label`,
`setting-text`, `option`, `mode-label`), keyed by the stable setting id or
protocol value. Add a mapping there when a new row, option or mode label is
introduced, and add the `@tr` strings to the locale documents. This includes the
A3954 rows (`windNoiseSuppression`, `wearingDetection`, `easyChat`,
`easyChatWaitTime`, `soundLeakCompensation`, `spatialAudio`,
`spatialAudioMusicMode`, `spatialAudioMode`, the twelve gesture ids
`leftSinglePress`…`rightSlideDown`, the case ids `atmospheric`, `remoteCamera`,
`findDevice`, `caseSpatialAudio` and `caseLanguage`, and the About
battery/firmware ids), its option values (`5/10/15 seconds`,
`10/20/30/60 minutes`, `Fixed`, `Head tracking`, `Music`, `Podcast`, `Movie`,
`Gaming`, `Volume up`, `Volume down`, `Previous song`, `Next song`,
`Ambient sound mode`, `Voice assistant`, `Play/pause`, `Japanese`, `English`,
`Chinese`), and the panel titles `Presses`, `Slides` and `Case screen`. The
gesture row ids stay side-specific on this page because both earbuds are shown
together; do not strip the side to the A3062 page's shorter `Single press`
wording.
The language picker intentionally shows each language in its own language.

### Capture tool

`tools/ui-capture/capture-ui.ps1` drives the demo device through Windows UI
Automation, captures every page and the core dialogs at 100/110/125/150 % in
light and dark, and writes raw PNGs, contact sheets and an HTML index under
`artifacts/ui-screenshots/<timestamp>/`. Run it with `-Build` after changing
Slint or translations. It uses an isolated `APPDATA`, so it never touches the
user's OpenSCL4 documents.
## 5. Future Android mapping

Share behavior and semantics, not Slint source or desktop pixel values. Compose
should consume the same core capabilities, block IDs, visibility rules,
quick-toggle IDs and serialization format so saved or migrated preferences have
the same meaning.

| Shared concept | Windows Slint | Android Compose |
| --- | --- | --- |
| Color and type | `Theme` tokens | Material `ColorScheme` and `Typography` derived from the same palette/roles |
| Stack / row | `Stack` / `Row` | `Column` / `Row` with named spacing constants |
| Panel and heading | `Panel` / `PanelHeading` | A shared `Card` with the same content hierarchy |
| Main and secondary actions | `Button` and `ButtonKind` | Material buttons with the same action hierarchy |
| Quick toggles | `Chip` in `QuickToggleGrid` | `FilterChip` in `FlowRow` |
| Boolean setting | `Toggle` | `Switch`, including disabled and busy behavior |
| Result and progress | `StatusIcon`, `ProgressBar` | Shared status icon and `LinearProgressIndicator` |
| Metric | `MetricTile` | Shared metric `Card` |
| Scrollable overflow | `ScrollArea` | `verticalScroll`, lazy lists where appropriate |
| Overview row models | `AppState.overview-rows` | The same ordered panel/shortcut groups from core |

Android touch targets, typography and window-size classes must follow current
Android accessibility and adaptive-layout guidance. Desktop `px` values are not
Android `dp` values and must not be copied mechanically. Compose's `FlowRow` may
size chips to content rather than reproducing the Windows fixed-cell geometry;
the ordering, disabled reasons and actions remain identical. Where vertical
space is constrained, Android may place this quick-settings group inside the
hero below device identity and batteries. That placement is platform-specific:
it must not pull the desktop buttons back into the device card or duplicate the
selection and device-write behavior.

## 6. Adding a component

1. Reuse an existing token and component first.
2. Put a genuinely repeated pattern in `basics.slint`; keep page-specific domain
   components beside that page.
3. Define default, hover, focus, selected, disabled, success and error behavior
   where applicable, plus accessible names and keyboard activation.
4. Keep device writes and persistence in Rust, not in the view.
5. Add or update model/controller tests for new capabilities and state changes.
6. Exercise both narrow and wide layouts and record material vendored-tree
   changes in `SOURCE.md`.
