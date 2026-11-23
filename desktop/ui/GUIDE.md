# Desktop UI implementation guide

This guide explains how to work with the OpenSCL4 UI kit. `UI_KIT.md` remains the
normative component and design contract; this file is the practical workflow for
developers and agents. Project agents should also use
`.agents/skills/use-soundcore-ui-kit/SKILL.md` for Slint UI work.

## 1. Find the right layer

- `theme.slint`: palette and semantic dimensional/type tokens.
- `components/basics.slint`: reusable controls, composition and feedback.
- `components/scrollarea.slint`: shared overflow behavior.
- `pages/`: page composition and page-specific domain components.
- `types.slint`: Slint-facing models and callbacks, not device logic.
- `messages.slint`: the `Messages` catalog; it resolves a Rust-supplied
  `UiMessage` key through `@tr` and the translation documents.
- `../src/messages.rs`: typed keys and constructors for the sentences the Rust
  model sends (keys only, never English copy).
- `../src/controller.rs`: callbacks, connection state and persistence wiring.
- `../src/overview.rs`: Overview capability filtering, row models and layout
  preferences.

Read the whole component before editing it. Start from an existing consumer:
Overview is the reference for configurable content and compact quick settings;
Diagnostics is the reference for capture state, metrics, progress and a responsive
main/aside layout.

## 2. Add or change a shared component

1. Search `components/` and `pages/` for an existing equivalent. Reuse it when
   only its content or layout differs.
2. If the pattern is shared, add one component to `basics.slint` with a small
   semantic API. Prefer `compact`, `wide` or an explicit kind over a page-name
   switch.
3. Use `Theme.space-*`, `radius-*`, `control-height-*`, `icon-*` and `font-*`.
   These lengths already include `Theme.content-scale`.
4. Define the relevant default, hover, focus, selected, disabled and result
   states. Interactive controls need pointer and keyboard activation plus an
   accessible name. Decorative status and metrics must not masquerade as buttons.
5. Use `@tr` for user-visible copy. A sentence the Rust model composes is a
   `UiMessage` key resolved by `Messages.text` (add the key in
   `../src/messages.rs` and the branch with its English source in
   `messages.slint`); never put an English sentence in a Rust model field. When a
   sentence quotes a translatable word, pass the *word's key* as an argument
   (`subject.*`/`reason.*`, resolved by `MessageParts`) instead of the word. Set
   an accessible `label` on `TextField`, `Toggle` and icon-only controls.
6. Replace existing local copies only after the shared component supports their
   actual behavior; do not change callbacks or device semantics as a side effect.
7. Update the component table or state rules in `UI_KIT.md` and record the change
   in `SOURCE.md`.

Example field binding:

```slint
import { TextField } from "../components/basics.slint";

name := TextField {
    label: @tr("Profile name");
    placeholder: @tr("My sound");
    edited => { AppState.eq-save-problem = 0; }
    accepted => { Actions.eq-save(self.text, false); }
}
```

Use `text <=> model` only when the model itself is intentionally two-way. For a
dialog-local name, pass `name.text` to the action as above.

## 3. Add a page or dialog

1. Define the page enum, display strings and route in the existing shell; do not
   create a parallel navigation system.
2. Add only state the controller or preview can really provide. Unsupported
   features remain absent rather than becoming decorative controls.
3. Compose with `Panel`, `PanelHeading`, `BodyText`, `CaptionText`, `Button`,
   `Chip`, `Banner`, `Note` and `ScrollArea` before introducing local primitives.
4. Put page copy and error text behind `@tr`. A sentence the controller composes
   arrives as a `UiMessage` (`key`, `arg0`, `arg1`) and is rendered with
   `Messages.text`; the English source stays in `messages.slint` and the `.po`
   documents, so a translated build never falls back to Rust English. The
   migrated `AppState` sentence fields are `connection-reason`, `failure-text`,
   `add-error`, `error-label`, `diagnostic-device-meta`, `diagnostic-status` and
   `diagnostic-error`, plus `DiagnosticProbe.status`; keep a new one a
   `UiMessage` rather than adding a string back. A disabled device control needs
   a reason from state, not an invented explanation.
5. Give dialogs a bounded height and a scrollable list when content can grow.
   Their primary action must remain reachable at 800×700 and 150% scale.
6. Wire the callback in Rust and add tests for accepted, rejected, cancelled,
   stale-result and disconnected paths where those paths exist.
7. Add a preview capture state for the new page/dialog, including its populated
   and validation-error presentation when relevant.

## 4. Change layout, alignment or tokens

- Change a token only after searching all consumers. A token change is a global
  visual change, not a local spacing fix.
- Do not multiply a dimensional token by `Theme.content-scale` again. Multiply
  only a genuinely new base dimension that has no token, and prefer adding a
  semantic token when the value repeats.
- In `HorizontalLayout` and `VerticalLayout`, use `alignment` or
  `cross-axis-alignment` instead of assigning a child's `y`/`x` manually. Manual
  coordinates in a managed layout can be overridden and produce compiler
  warnings.
- Keep a row's divider in normal flow with its content, and take the row's height
  from that content. A divider at a fixed offset, or a fixed row height for
  visual alignment, is only correct until a description wraps: then the text
  runs over the next divider, or two rows overlap. `SettingRowView` is the
  reference row (`VerticalLayout`, divider first, content row after, controls
  centred with `cross-axis-alignment`) and the preview's `settings-rows-long`
  captures are the fixture that shows the wrapped case.
- Add new long-string cases to the preview's capture states rather than
  eyeballing the normal ones: the failure appears only at narrow widths, 150 %
  scale and long translated text.
- Give grid children explicit cell dimensions and derive column counts from the
  available width. Long labels elide or wrap inside the cell; they must not widen
  the window.
- Keep wide and narrow branches on the same domain model. A responsive branch
  changes arrangement, not what the device supports.
- After a style-only edit, check the affected page in both themes and with long
  strings; a successful build is not visual acceptance.

## 5. Local verification

Use the Cargo executable available on the machine if `cargo` is not on `PATH`.
From `desktop`:

```powershell
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run the same relevant check/clippy/test sequence from `core` when shared session,
protocol or model behavior changes. The repository root has no Cargo workspace
manifest; run each crate tree explicitly.

`cargo test` includes `desktop/tests/translations.rs`, which fails when a `@tr`
source has a missing or empty translation in a required locale (`ru` today) or
when a `messages.rs` key has no catalog source. When adding UI copy, run it
before the capture tool: it prints the complete missing/empty list.

### Full Russian screenshot matrix

For a reviewable Windows matrix, use `tools/ui-capture/capture-ui.ps1`. It
launches the app in an isolated profile, connects the demo A3062, captures every
page and the core dialogs at 100/110/125/150 % in both themes, and generates
`index.html` plus contact sheets grouped by theme×scale, page and feature area.
It verifies that every `@tr` string has a **non-empty** Russian translation first
(the same rule as the translation test) and fails the capture if the window is not
in Russian. See `tools/ui-capture/README.md`.
### Visual matrix

The preview harness lives in `.scratch/ui-preview`. From that directory:

```powershell
$env:SLINT_BACKEND = "winit-software"
$env:CARGO_TARGET_DIR = "../../desktop/target"
cargo run
```

Raw captures are written to `.scratch/ui-validation`. Generate review sheets from
the workspace root when needed:

```powershell
python .scratch/make_ui_sheets.py
```

Add capture variants for new controls and dialogs. At minimum inspect the
affected views at 800×700, 1030×760, 1280×820 and 1440×900, at 100/125/150%, in
light and dark palettes where supported. Verify:

- no horizontal clipping or accidental overflow;
- readable text and matching `on-*` foregrounds;
- correct selected, disabled, focus and error states;
- reachable actions and contained dialogs;
- stable alignment at every breakpoint.

For an interactive software-rendered demo, run `cargo run -- --interactive` from
`.scratch/ui-preview` with the same environment. Stop an older demo before
rebuilding on Windows so its executable is not locked.

## 6. Windows release checklist

Automated:

- [ ] Formatting, check and Clippy pass without new UI warnings.
- [ ] Desktop and affected core tests pass; ignored hardware tests are reported
      separately.
- [ ] Text inputs, buttons, icon buttons and dialogs use the shared UI kit.
- [ ] Add Device, Save Profile and Create Preset captures have been inspected.
- [ ] Narrow/wide, scale and theme captures have been inspected.
- [ ] UI contract, guide and `SOURCE.md` reflect material changes.
- [ ] No device capability or acceptance result was inferred from demo data.

Real-device acceptance still requires a paired supported device:

- [ ] Add/connect, app-level Disconnect, cancel, Retry and reconnect.
- [ ] Sound mode, equalizer, profiles and every visible settings row write and
      re-read the device state correctly.
- [ ] Idle link loss is detected without another click; Retry restores control.
- [ ] Tray Open/Quit, close-to-tray on/off, icon on/off and tray section switches.
- [ ] Tray and close-to-tray choices survive restart; a missing or malformed
      preferences document safely falls back to defaults.
- [ ] Keyboard-only navigation, accessible names and 125/150% Windows scaling.
- [ ] Accessibility states: disabled actions report disabled and switches,
      check rows and selected options report their checked state in the
      accessibility tree (a visual check does not prove the bindings exist).
- [ ] Diagnostics capture and ZIP redaction; no address or serial leaks.

A software demo and unit tests do not complete the real-device items.
