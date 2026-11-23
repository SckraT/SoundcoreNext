# 04 — Migrate controller UI copy

**Status:** ready-for-agent
**Blocked by:** 03

## Requirements

1. Audit controller-originated display strings: error labels, failure text,
   connection reasons, add-device errors, diagnostics errors, setting-write
   failure labels/reasons, toast-adjacent copy.
2. Replace every user-facing sentence with a `UiMessage` key + args; technical
   stderr/log lines, URLs, file names and protocol ids stay as code.
3. Add `Messages` branches and Russian translations for each.
4. Update Slint `AppState` fields (`error-label`, `failure-text`, `add-error`,
   `diagnostic-error`, `connection-reason`, etc.) to `UiMessage` and all UI
   renderers to `Messages.text`.
5. Add a test/guard that Rust UI fields no longer accept raw display sentences
   (at least for the migrated fields), and controller tests assert keys/args.
6. No behavior change; all existing tests adapted.

## Acceptance

- All controller-originated visible copy is translated via `.po`; no English
  sentences in Rust for UI.
