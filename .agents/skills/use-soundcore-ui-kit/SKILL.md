---
name: use-soundcore-ui-kit
description: Use when creating or changing SoundcoreNext desktop Slint UI, pages, dialogs, shared components, theme tokens, responsive layouts, accessibility, translations, or visual styling; also use for visual regression fixes and when preparing the future Android mapping of these UI semantics.
---

# SoundcoreNext UI-Kit workflow

Work in the project workspace. Treat `desktop/ui/UI_KIT.md` as the design-system contract and `desktop/ui/GUIDE.md` as the implementation procedure. Read both before editing UI, then read `desktop/ui/SOURCE.md` and the complete component being changed.

## 1. Reuse before adding

- Use existing `Theme` color, spacing, shape, control, icon and type tokens. Dimensional tokens already include `Theme.content-scale`; never multiply them by the scale again.
- Prefer components from `desktop/ui/components/basics.slint` and the existing `ScrollArea`. Extend a shared component only when the pattern is genuinely reusable; keep device-domain presentation beside its page.
- Do not add a third-party UI framework or page-local copies of buttons, fields, panels, metrics, status indicators or dialogs.
- Check the complete surrounding layout before changing alignment. In a Slint layout, use layout alignment rather than manually positioning a child's `y` coordinate.

## 2. Preserve behavior and domain truth

- Slint renders state. Keep Bluetooth commands, persistence, validation and capability decisions in Rust.
- Never invent device capabilities, values, progress, quick presets or success states. Use only the connected model's reported state and existing core capabilities.
- Device settings follow write-and-re-poll: do not optimistically substitute the clicked value for the device's answer.
- Keep view callbacks compatible with `desktop/ui/types.slint`; add controller handling and regression tests when behavior changes.
- Disconnecting the app link is not forgetting a device. Destructive actions require the established confirmation and danger treatment; ordinary disconnect does not.

## 3. Build accessible, adaptive UI

- Mark user-visible copy with `@tr`; do not hard-code a new English-only label.
- Give icon-only controls and interactive rows accessible names and roles. Preserve keyboard focus and activation, disabled behavior and readable focus indication.
- Use shared text roles. Consolas is only for literal commands, file names and identifiers through `CodeText`.
- Bind scrollable content width to `ScrollArea.visible-width`; keep every action reachable at the minimum window size.
- Do not assume one window width or one language. Check long labels, elision/wrapping, narrow dialogs, light/dark themes and 100/125/150% content scale.
- Keep desktop and future Android semantics aligned, but do not copy desktop pixel geometry into Android. Share IDs, capabilities, state meaning and action hierarchy.

## 4. Verify and record

1. Add or update Rust model/controller tests for behavioral changes.
2. Add capture states for new dialogs or controls in `.scratch/ui-preview`; do not validate a new control only by compiling it.
3. From `desktop`, run `cargo fmt --all`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`. Resolve new Slint warnings rather than accepting them as baseline.
4. When shared core behavior changes, run the corresponding checks and tests from `core` too.
5. Generate the visual matrix with the software-backed preview and inspect the affected captures at their required widths, scales and themes. Check image size before opening it.
6. Update `UI_KIT.md` when a contract changes, `GUIDE.md` when the workflow changes, and `SOURCE.md` for material changes to vendored UI files.
7. Report automated checks separately from real-device acceptance. Never claim hardware acceptance from a demo, screenshot or software renderer alone.

The release checklist and exact local commands are in `desktop/ui/GUIDE.md`.
