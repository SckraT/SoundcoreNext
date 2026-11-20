# 01 — Branding and version 2.0

**Type:** task
**Status:** done
**Blocked by:** —

## Goal

Keep the product name **OpenSCL4** and present this branch as 2.0, without changing the vendored UI.

## Files

- `README.md`
- `NOTICE`
- `desktop/README.md`
- `desktop/ui/SOURCE.md` (only if the attribution wording needs the version)

Do **not** edit `desktop/Cargo.toml`; ticket 02 owns that file. Do not rename the window or any UI string.

## Acceptance

- [x] Root README states that this is OpenSCL4 2.0 development and that the UI is vendored from OpenSCL4 under GPL-3.0.
- [x] `desktop/README.md` says the shell is the OpenSCL4 2.0 desktop shell.
- [x] `NOTICE` mentions the vendored desktop UI and keeps the GPL-3.0 attribution.
- [x] No user-visible string is renamed; the title still says OpenSCL4.
- [x] Markdown links still resolve; no new broken links.
