# 03 — Runtime message catalog

**Status:** ready-for-agent
**Blocked by:** —

## Goal

Rust must not contain ready-made user-facing English sentences. It supplies
message keys and arguments; Slint resolves them through `@tr` and `.po`.

## Requirements

1. Add `desktop/ui/messages.slint` with `UiMessage { key, arg0, arg1 }` and
   `export global Messages { public pure function text(UiMessage) -> string }`.
   Each branch is `@tr("English source", args...)`.
2. Add `desktop/src/messages.rs` with typed key constants and constructors; no
   display sentences. Rust model fields carry `UiMessage` (or key/args).
3. Change `QuickToggle.reason` and `SettingRow.reason` to `UiMessage`; UI renders
   `Messages.text`. Remove the English LDAC reason from `overview.rs`.
4. Add Russian `.po` entries for every new source string.
5. Update `LocalizedText` only for device-id labels/options; messages for
   sentences belong to `Messages`.
6. Tests: quick-toggle reason key/args, setting reason mapping, Russian text
   resolution (via UI model), no raw English reason in Rust.
7. Update UI_KIT/GUIDE/SOURCE with the contract.

## Acceptance

- LDAC quick-toggle reason follows the app language; no user-facing sentence is
  hardcoded in Rust for these fields.
