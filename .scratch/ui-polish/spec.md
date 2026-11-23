# Spec: UI polish and translated runtime messages

**Status:** ready for implementation
**Tracker:** `.scratch/ui-polish/`
**Integration branch:** `feature/ui-polish-translations`
**Based on:** master `3ae7303`

## Goal

Fix four visible UI defects and make Rust-originated UI copy translatable by
construction: breadcrumb alignment, the duplicated Diagnostics block in Settings,
row text crossing dividers, and the English LDAC quick-toggle reason.

## In scope

- Breadcrumb arrow centered by layout, no manual `y`.
- Remove the Diagnostics aside panel from Settings; move Copy logs to Diagnostics.
- `SettingRowView` in normal flow: divider as a child, height from content; no
  absolute divider or fixed row height.
- Message catalog: Rust supplies message keys and arguments only; Slint resolves
  them through `@tr` and `.po`.
- Migrate quick-toggle reasons, setting-row reasons, and all controller-originated
  error/failure/add/diagnostics UI strings to message keys.
- Translation completeness test for required locales (currently `ru`), extensible
  to future languages.
- Russian translations and visual verification.

## Out of scope

- Adding new languages or machine-translating the other 14 catalogs.
- Protocol/device behavior changes.

## Tickets

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-layout-fixes.md` | — | breadcrumb, Settings diagnostics removal, Copy logs in Diagnostics |
| `02-row-layout.md` | — | flow-based SettingRowView, auto height, long-string tests |
| `03-message-catalog.md` | — | UiMessage + messages.slint/messages.rs; quick/setting reasons migrated |
| `04-controller-copy.md` | 03 | errors/failures/add/diagnostics migrated |
| `05-translation-test.md` | 03,04 | translation completeness test + capture preflight |
| `06-acceptance.md` | 01–05 | visual matrix, gates, review, merge |

## Acceptance

1. Breadcrumb arrow centered at 100/125/150%.
2. No Diagnostics block in Settings; Copy logs on Diagnostics; no button overflow.
3. Wrapped descriptions/reasons never cross a divider at 800×700/150%.
4. LDAC quick-toggle reason is Russian and follows the UI language; no UI copy
   hardcoded in Rust.
5. Translation test fails on a missing/empty required translation.
6. Desktop/core gates green; docs and screenshots updated.
