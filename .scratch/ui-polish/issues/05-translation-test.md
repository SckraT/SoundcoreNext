# 05 — Translation completeness test

**Status:** done
**Blocked by:** 03, 04
**Implemented:** 2026-11-24

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

## Implementation (2026-11-24)

- `desktop/tests/translations.rs` (new, standard `cargo test` integration test, no
  display or device):
  - `REQUIRED_TRANSLATIONS = ["ru"]`; the file documents how to extend it (drop a
    translated `.po` in `desktop/translations/<code>/LC_MESSAGES/` and add the
    code to the list).
  - Slint scanner: walks `desktop/ui/**/*.slint` (static pages, components,
    `messages.slint`, `localized.slint`), skips line and block comments, decodes
    Slint string escapes (`\"`, `\\`, `\n`, `\u{…}`), skips other string
    literals and argument strings, and captures both strings of a plural
    `@tr("x" | "xs" % n)` call.
  - `.po` parser: msgid/msgstr/msgid_plural/msgstr[i], multi-line continuation
    lines, C-style escapes, plural forms and duplicate-msgid rejection. An empty
    `msgstr` (or an all-empty plural set) counts as missing.
  - Tests:
    - `required_translations_have_a_non_empty_translation_for_every_ui_string`
      fails with one readable list naming the file, the locale, the source and
      whether it is missing or empty; plural entries must exist, match their
      msgid_plural and have non-empty forms;
    - `every_messages_key_has_a_catalog_branch_and_a_translated_source` extracts
      the `pub const … : &str` keys from `src/messages.rs`, asserts each is an
      identifier (no spaces), finds its `Messages`/`MessageParts` branch and
      checks the catalog's English source has a non-empty Russian translation;
    - `required_translation_documents_have_no_duplicate_msgids` (case-sensitive;
      the parser itself rejects a duplicate);
    - `the_required_locale_list_documents_how_to_extend_it`;
    - `the_slint_scanner_handles_escapes_plurals_and_comments` and
      `the_po_scanner_handles_continuations_escapes_and_plurals` (plus a
      `#[should_panic]` duplicate fixture) pin the instruments themselves.
- `tools/ui-capture/capture-ui.ps1`: `Assert-RussianTranslations` now parses the
  `.po` (continuations, escapes, plurals), decodes the `@tr` sources (comments
  stripped) and fails on a **missing or empty** required translation before the
  app is launched, reporting every problem in one list; the unused
  `Get-MissingRussianStrings` helper was replaced by the shared parser.
  `tools/ui-capture/README.md` and `desktop/ui/GUIDE.md` record the non-empty
  rule.
- Russian translations for every source ticket 04 added are in
  `desktop/translations/ru/LC_MESSAGES/soundcore-desktop.po`; `translations.rs`
  passes, including `messages.rs` key coverage.

## Deliberate-failure demonstration

Temporarily emptying the Russian translation of a migrated runtime message:

```text
msgid "Could not build the archive: {}"
msgstr ""
```

`cd desktop && cargo test --test translations` then fails with:

```text
---- required_translations_have_a_non_empty_translation_for_every_ui_string stdout ----
1 translation problem(s):
  - ru: ui\messages.slint: empty msgstr for "Could not build the archive: {}"

---- every_messages_key_has_a_catalog_branch_and_a_translated_source stdout ----
1 translation problem(s):
  - ru: empty msgstr for "Could not build the archive: {}", the source of messages.rs key "diagnostics.build-failed"
```

The same emptied string is rejected by the capture preflight (verified by
running the preflight functions against a temporary copy of the tree):

```text
PREFLIGHT FAILED: The required translations do not cover every @tr string (1 problem(s)):
empty msgstr: Supported
```

(`Supported` was emptied for the preflight check.) The string was restored and
both the test suite and the preflight are green again.

## Verification

- `desktop`: `cargo test` — 306 controller/unit tests passed, 15 ignored
  hardware-only, 7 translation tests passed; `cargo fmt --all -- --check`,
  `cargo build`, `cargo clippy --all-targets -- -D warnings` — green.
- Runtime chain: `messages::tests::russian_messages_resolve_through_the_slint_catalog`
  creates the shell window, selects the bundled Russian translation, and asserts
  the compiled `Messages` global returns the Russian copy for the quick-toggle
  reason and for the composed failed-write banner
  (`Не удалось изменить «LDAC (нет ответа)». На устройстве осталось прежнее
  значение.`), then restores English. `app.slint` exports `Messages` for that
  test. This proves `@tr`, the nested `MessageParts` calls and the `.po` lookup
  work at runtime, not only as data.
- `cargo test --test translations` alone: 7 passed.
- The preflight keeps its existing place in the capture script
  (`Assert-RussianTranslations` before the app launch) and now enforces the
  non-empty rule.
