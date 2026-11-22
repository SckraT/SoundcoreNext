# Vendored OpenSCL4 UI

- Upstream: https://github.com/SckraT/OpenSCL4
- Commit: `01276e08` (the checkout used for the port)
- Source path: `desktop/ui`
- License: GNU General Public License v3.0 (see root `LICENSE`)
- Copyright: OpenSCL4 contributors; each file retains its upstream content.

## What is vendored

The complete `desktop/ui` tree: `app.slint`, `shell.slint`, `theme.slint`,
`types.slint`, `strings.slint`, `tray.slint`, `icons.slint`, `components/`,
`pages/`, `icons/`, `art/`, `fonts/`.

## Changes

This application adapts the shell to a new controller and its own Rust core
(whose protocol/preset data is attributed in the root `NOTICE`). The shell opens
with empty state until the controller fills it.

The spec's one UI exception — the dual-connections row actions — is the only
change made to the vendored tree so far. It was made in commit `cd309be` ("Show
and drive the A3062 dual-connections list") and is limited to:

- `types.slint`: the `Actions` global gains `connect-item(string, int)` and
  `disconnect-item(string, int)` callbacks. They carry a list row's id and the
  option index, the same shape the existing `remove-item(string, int)` Forget
  callback uses.
- `components/settings.slint`: the `SettingRowView` list row gains one
  **Connect** and one **Disconnect** `ButtonKind.ghost` button next to the
  existing **Forget** button, with the same style and vertical placement.

No layout, wording, colour or other visual change is made; the two buttons are
the spec's minimal exception and no other `.slint` file is edited. Any later
change to a vendored file is made in a commit that names the file and the
reason; the upstream copyright and GPL-3.0 terms stay in place.
