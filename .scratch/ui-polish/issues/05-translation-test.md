# 05 — Translation completeness test

**Status:** ready-for-agent
**Blocked by:** 03,04

## Requirements

1. Add `desktop/tests/translations.rs`:
   - scan all `@tr("...")` from `.slint` (including messages/localized);
   - parse `.po` msgid/msgstr (multi-line/escapes);
   - for `REQUIRED_TRANSLATIONS` (currently `["ru"]`) assert every msgid has a
     non-empty msgstr;
   - assert every `messages.rs` key exists in `messages.slint` and `.po`;
   - fail with a readable list of missing/empty keys; document future locale
     extension.
2. Update `tools/ui-capture/capture-ui.ps1` preflight to use the same rule
   (non-empty required translations), not just msgid presence.
3. Add a deliberate-failure demonstration in the ticket (temporarily remove one
   msgstr, test fails; restore) and record it.
4. Tests run in `cargo test` and are not hardware-only.

## Acceptance

- Test catches missing/empty required translations; gates green.
