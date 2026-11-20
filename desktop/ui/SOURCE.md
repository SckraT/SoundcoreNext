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

This application adapts the shell to a new controller and clean-room core.
Until that adapter lands, the shell opens with empty state. Any change to a
vendored file is made in a commit that names the file and the reason; the
upstream copyright and GPL-3.0 terms stay in place.
