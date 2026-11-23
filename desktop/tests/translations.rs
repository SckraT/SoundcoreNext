//! Translation completeness for the runtime UI copy.
//!
//! The Slint build bundles `desktop/translations/<locale>/LC_MESSAGES/soundcore-desktop.po`;
//! a `@tr("...")` source with no entry there falls back to English, so the
//! window would mix languages. This test enforces the contract for every locale
//! listed in [`REQUIRED_TRANSLATIONS`]:
//!
//! 1. every `@tr` source in every `desktop/ui/**/*.slint` file (static pages,
//!    `messages.slint` and `localized.slint` alike) has a `.po` entry;
//! 2. that entry's translation is non-empty (`msgstr` for a plain source,
//!    `msgstr[0]` for a plural one, and the plural msgid's own forms);
//! 3. every message key declared in `desktop/src/messages.rs` exists in the
//!    `Messages` / `MessageParts` catalog (`desktop/ui/messages.slint`), and the
//!    source that catalog returns for the key has a non-empty translation.
//!
//! **Extending the required locales:** add the locale code to
//! [`REQUIRED_TRANSLATIONS`] after `desktop/translations/<code>/LC_MESSAGES/soundcore-desktop.po`
//! is added and translated. The test then enforces the same rule for it; the UI
//! capture preflight in `tools/ui-capture/capture-ui.ps1` uses the same
//! non-empty rule for Russian. `.po` files are hand-maintained: a source with an
//! empty `msgstr` is treated as missing, not as "untranslated but acceptable".
//!
//! The test is part of `cargo test` and needs no device or display.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Locales whose translation documents must be complete. Russian is the only
/// maintained non-source language on this branch; the source language itself
/// (English) needs no `.po`.
const REQUIRED_TRANSLATIONS: &[&str] = &["ru"];

/// The desktop crate's manifest directory (`desktop/`).
fn desktop_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.slint` file under `desktop/ui`, in a stable order.
fn slint_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("the directory entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "slint")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// One `@tr` call of a Slint source file.
#[derive(Debug, PartialEq, Eq)]
struct Translatable {
    /// The file the source was found in, for readable failures.
    file: PathBuf,
    /// The singular (plain) source: the `.po` `msgid`.
    singular: String,
    /// The plural source of `@tr("x" | "xs" % n)`, if the call has one.
    plural: Option<String>,
}

/// Extracts every `@tr(...)` source string from a Slint file. Comments and
/// other string literals are skipped; escapes are decoded; a plural form
/// contributes its singular and plural sources.
fn translatables(file: &Path, source: &str) -> Vec<Translatable> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index = skip_line(bytes, index);
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = skip_block_comment(bytes, index);
        } else if byte == b'"' {
            index = skip_string(bytes, index);
        } else if byte == b'@' && source[index..].starts_with("@tr") {
            let mut cursor = skip_whitespace(bytes, index + "@tr".len());
            if bytes.get(cursor) != Some(&b'(') {
                index += 1;
                continue;
            }
            cursor = skip_whitespace(bytes, cursor + 1);
            if bytes.get(cursor) != Some(&b'"') {
                index += 1;
                continue;
            }
            let (singular, next) = read_string(bytes, cursor);
            let mut cursor = skip_whitespace(bytes, next);
            let mut plural = None;
            if bytes.get(cursor) == Some(&b'|') {
                cursor = skip_whitespace(bytes, cursor + 1);
                if bytes.get(cursor) == Some(&b'"') {
                    let (plural_source, next) = read_string(bytes, cursor);
                    plural = Some(plural_source);
                    cursor = next;
                }
            }
            // Skip to the call's closing parenthesis, so a string in another
            // argument cannot be mistaken for a source.
            index = skip_parenthesised(bytes, cursor);
            found.push(Translatable {
                file: file.to_path_buf(),
                singular,
                plural,
            });
        } else {
            index += 1;
        }
    }
    found
}

fn skip_line(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    index += 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn skip_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

/// Reads a Slint string literal starting at `index` (the opening quote) and
/// returns its decoded contents and the index after the closing quote.
fn read_string(bytes: &[u8], start: usize) -> (String, usize) {
    assert_eq!(bytes[start], b'"', "a string literal starts with a quote");
    let source = std::str::from_utf8(bytes).expect("the Slint source is UTF-8");
    let mut characters = source[start + 1..].char_indices().peekable();
    let mut decoded = String::new();
    while let Some((offset, character)) = characters.next() {
        match character {
            '"' => return (decoded, start + 1 + offset + 1),
            '\\' => {
                let Some((_, escape)) = characters.next() else {
                    panic!("an escape ends the Slint source");
                };
                match escape {
                    'n' => decoded.push('\n'),
                    't' => decoded.push('\t'),
                    'r' => decoded.push('\r'),
                    '0' => decoded.push('\0'),
                    'u' => {
                        // `\u{XXXX}`: decode the braced code point when present.
                        let mut looked = characters.clone();
                        if let Some((_, '{')) = looked.next() {
                            let mut hex = String::new();
                            let mut closed = false;
                            for (_, next) in looked.by_ref() {
                                if next == '}' {
                                    closed = true;
                                    break;
                                }
                                hex.push(next);
                            }
                            if closed && let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(character) = char::from_u32(code) {
                                    decoded.push(character);
                                }
                                characters = looked;
                                continue;
                            }
                        }
                        decoded.push('u');
                    }
                    other => decoded.push(other),
                }
            }
            other => decoded.push(other),
        }
    }
    panic!("unterminated string literal in a Slint source");
}

/// Skips a string literal without decoding it.
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    panic!("unterminated string literal in a Slint source");
}

/// Finds the index just after the `)` that closes a call whose argument text
/// starts at `index`, skipping nested parentheses and strings.
fn skip_parenthesised(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 1usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    return index;
                }
            }
            b'"' => index = skip_string(bytes, index),
            _ => index += 1,
        }
    }
    bytes.len()
}

/// One `.po` entry: the msgid and its translations.
#[derive(Debug, Default, Clone)]
struct PoEntry {
    msgid_plural: Option<String>,
    msgstr: String,
    plural_msgstr: Vec<String>,
}

impl PoEntry {
    /// Whether this entry carries a translation: the singular form of a plural
    /// entry, or the plain form otherwise.
    fn has_translation(&self) -> bool {
        if self.msgid_plural.is_some() {
            self.plural_msgstr
                .first()
                .is_some_and(|form| !form.is_empty())
        } else {
            !self.msgstr.is_empty()
        }
    }

    /// Whether the singular source plus the given plural forms (one per
    /// `nplurals`) are all non-empty.
    fn plural_forms_are_complete(&self, expected: usize) -> bool {
        self.plural_msgstr.len() >= expected
            && self.plural_msgstr.iter().all(|form| !form.is_empty())
    }
}

#[derive(Clone, Copy)]
enum Field {
    Msgid,
    MsgidPlural,
    Msgstr,
    Plural(usize),
}

/// Parses a gettext `.po` document into `msgid -> entry`, handling continuation
/// lines, C-style escapes and plural forms. Duplicate literal msgids are
/// rejected: one source string must have exactly one translation.
fn parse_po(path: &Path) -> BTreeMap<String, PoEntry> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let mut entries = BTreeMap::new();
    let mut msgid: Option<String> = None;
    let mut entry = PoEntry::default();
    let mut field: Option<Field> = None;

    for (number, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            flush_po_entry(path, &mut entries, &mut msgid, &mut entry);
            field = None;
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid_plural ") {
            field = Some(Field::MsgidPlural);
            entry_msgid_plural(&mut entry).push_str(&parse_po_string(rest, path, number, line));
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid ") {
            msgid = Some(parse_po_string(rest, path, number, line));
            field = Some(Field::Msgid);
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgstr[") {
            let Some(close) = rest.find(']') else {
                po_fail(path, number, line, "broken plural msgstr index");
            };
            let Ok(index) = rest[..close].parse::<usize>() else {
                po_fail(path, number, line, "broken plural msgstr index");
            };
            let Some(rest) = rest[close + 1..].strip_prefix(' ') else {
                po_fail(path, number, line, "a msgstr value follows its index");
            };
            field = Some(Field::Plural(index));
            let value = parse_po_string(rest, path, number, line);
            set_plural_msgstr(&mut entry, index, &value);
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgstr ") {
            field = Some(Field::Msgstr);
            entry
                .msgstr
                .push_str(&parse_po_string(rest, path, number, line));
            continue;
        }
        if let Some(rest) = line.strip_prefix('"') {
            let Some(field) = field else {
                po_fail(path, number, line, "a continuation line has no field");
            };
            let decoded = parse_po_quoted(rest, path, number, line);
            match field {
                Field::Msgid => {
                    if let Some(msgid) = msgid.as_mut() {
                        msgid.push_str(&decoded);
                    }
                }
                Field::MsgidPlural => {
                    entry_msgid_plural(&mut entry).push_str(&decoded);
                }
                Field::Msgstr => entry.msgstr.push_str(&decoded),
                Field::Plural(index) => set_plural_msgstr(&mut entry, index, &decoded),
            }
            continue;
        }
        // A gettext keyword this parser does not need (`msgctxt`, obsolete
        // entries, flags) is skipped without failing: it cannot satisfy a
        // `@tr` source.
    }
    flush_po_entry(path, &mut entries, &mut msgid, &mut entry);
    entries
}

fn entry_msgid_plural(entry: &mut PoEntry) -> &mut String {
    entry.msgid_plural.get_or_insert_with(String::new)
}

fn set_plural_msgstr(entry: &mut PoEntry, index: usize, value: &str) {
    if entry.plural_msgstr.len() <= index {
        entry.plural_msgstr.resize(index + 1, String::new());
    }
    entry.plural_msgstr[index].push_str(value);
}

fn flush_po_entry(
    path: &Path,
    entries: &mut BTreeMap<String, PoEntry>,
    msgid: &mut Option<String>,
    entry: &mut PoEntry,
) {
    let Some(id) = msgid.take() else {
        *entry = PoEntry::default();
        return;
    };
    let previous = std::mem::take(entry);
    // The header entry has an empty msgid and no translation.
    if id.is_empty() {
        return;
    }
    if entries.insert(id.clone(), previous).is_some() {
        panic!("{}: duplicate msgid {id:?}", path.display());
    }
}

fn parse_po_string(rest: &str, path: &Path, number: usize, line: &str) -> String {
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('"') else {
        po_fail(path, number, line, "a string value starts with a quote");
    };
    parse_po_quoted(rest, path, number, line)
}

fn po_fail(path: &Path, number: usize, line: &str, message: &str) -> ! {
    panic!("{}:{}: {message} ({line:?})", path.display(), number + 1)
}

/// Decodes the inside of a quoted `.po` string (the opening quote is removed).
fn parse_po_quoted(rest: &str, path: &Path, number: usize, line: &str) -> String {
    let mut decoded = String::new();
    let mut characters = rest.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => return decoded,
            '\\' => {
                let Some(escape) = characters.next() else {
                    po_fail(path, number, line, "unterminated escape in a quoted string");
                };
                match escape {
                    'n' => decoded.push('\n'),
                    't' => decoded.push('\t'),
                    'r' => decoded.push('\r'),
                    '"' => decoded.push('"'),
                    '\\' => decoded.push('\\'),
                    other => decoded.push(other),
                }
            }
            other => decoded.push(other),
        }
    }
    po_fail(path, number, line, "unterminated quoted string")
}

/// The message keys declared in `desktop/src/messages.rs`, in declaration
/// order.
fn message_keys(messages_rs: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for line in messages_rs.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": &str = ") else {
            continue;
        };
        if !name.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        }) {
            continue;
        }
        let value = value.trim();
        if let Some(value) = value.strip_prefix('"')
            && let Some(value) = value.strip_suffix("\";")
        {
            keys.push(value.to_owned());
        }
    }
    keys
}

/// The first `@tr` source that follows `key` in the message catalog. A catalog
/// branch for a message key is `if message.key == "<key>" { return @tr("..."); }`;
/// a `MessageParts` word has the same shape with a plain `key` variable.
fn source_after_key(catalog: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = catalog.find(&marker)? + marker.len();
    let after = &catalog[start..];
    let call = after.find("@tr(")?;
    translatables(Path::new("messages.slint"), &after[call..])
        .into_iter()
        .next()
        .map(|translatable| translatable.singular)
}

/// The required locale's `.po` path.
fn po_path(locale: &str) -> PathBuf {
    desktop_dir()
        .join("translations")
        .join(locale)
        .join("LC_MESSAGES")
        .join("soundcore-desktop.po")
}

/// Every `@tr` source in the UI.
fn ui_translatables() -> Vec<Translatable> {
    let mut all = Vec::new();
    for file in slint_files(&desktop_dir().join("ui")) {
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
        all.extend(translatables(&file, &source));
    }
    all
}

/// Formats a failure list so the test output names every offender, not just the
/// first one.
fn report(problems: &[String]) -> String {
    let mut text = String::new();
    writeln!(text, "{} translation problem(s):", problems.len()).expect("write to string");
    for problem in problems.iter().take(80) {
        writeln!(text, "  - {problem}").expect("write to string");
    }
    if problems.len() > 80 {
        writeln!(text, "  ... and {} more", problems.len() - 80).expect("write to string");
    }
    text
}

#[test]
fn required_translations_have_a_non_empty_translation_for_every_ui_string() {
    let translatables = ui_translatables();
    assert!(
        translatables.len() > 200,
        "the UI scan found only {} @tr sources; the scanner is broken",
        translatables.len()
    );
    let mut problems = Vec::new();
    for locale in REQUIRED_TRANSLATIONS {
        let path = po_path(locale);
        assert!(
            path.is_file(),
            "the required locale {locale} has no translation document at {}",
            path.display()
        );
        let entries = parse_po(&path);
        for translatable in &translatables {
            let file = translatable
                .file
                .strip_prefix(desktop_dir())
                .unwrap_or(&translatable.file)
                .display();
            let Some(entry) = entries.get(&translatable.singular) else {
                problems.push(format!(
                    "{locale}: {file}: missing msgid {:?}",
                    translatable.singular
                ));
                continue;
            };
            if !entry.has_translation() {
                problems.push(format!(
                    "{locale}: {file}: empty msgstr for {:?}",
                    translatable.singular
                ));
            }
            if let Some(plural) = &translatable.plural {
                match &entry.msgid_plural {
                    Some(msgid_plural) if msgid_plural == plural => {
                        if entry.plural_msgstr.len() < 2 {
                            problems.push(format!(
                                "{locale}: {file}: missing plural forms for {plural:?}"
                            ));
                        } else if entry.plural_msgstr.iter().any(String::is_empty) {
                            problems.push(format!(
                                "{locale}: {file}: empty plural msgstr for {plural:?}"
                            ));
                        }
                    }
                    _ => problems.push(format!(
                        "{locale}: {file}: msgid_plural {plural:?} is missing for {:?}",
                        translatable.singular
                    )),
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", report(&problems));
}

#[test]
fn every_messages_key_has_a_catalog_branch_and_a_translated_source() {
    let messages_rs = std::fs::read_to_string(desktop_dir().join("src").join("messages.rs"))
        .expect("desktop/src/messages.rs is readable");
    let catalog = std::fs::read_to_string(desktop_dir().join("ui").join("messages.slint"))
        .expect("desktop/ui/messages.slint is readable");
    let russian = parse_po(&po_path("ru"));
    let keys = message_keys(&messages_rs);
    assert!(
        keys.len() > 20,
        "messages.rs declares only {} keys; the scanner is broken",
        keys.len()
    );
    let mut problems = Vec::new();
    for key in &keys {
        assert!(
            !key.contains(' '),
            "a message key is an identifier, not UI copy: {key:?}"
        );
        let Some(source) = source_after_key(&catalog, key) else {
            problems.push(format!(
                "messages.rs key {key:?} has no Messages/MessageParts branch in ui/messages.slint"
            ));
            continue;
        };
        match russian.get(&source) {
            Some(entry) if entry.has_translation() => {}
            Some(_) => problems.push(format!(
                "ru: empty msgstr for {source:?}, the source of messages.rs key {key:?}"
            )),
            None => problems.push(format!(
                "ru: no msgid {source:?} for messages.rs key {key:?}"
            )),
        }
    }
    assert!(problems.is_empty(), "{}", report(&problems));
}

#[test]
fn required_translation_documents_have_no_duplicate_msgids() {
    for locale in REQUIRED_TRANSLATIONS {
        // `parse_po` panics on a case-sensitive duplicate, so reaching here is
        // the assertion; the count is asserted so an empty file is noticed.
        let entries = parse_po(&po_path(locale));
        assert!(
            entries.len() > 200,
            "{locale}: only {} msgids parsed; the document looks wrong",
            entries.len()
        );
    }
}

#[test]
fn the_required_locale_list_documents_how_to_extend_it() {
    // A future language is added by dropping its translated `.po` document into
    // `desktop/translations/<code>/LC_MESSAGES/` and adding the code here; the
    // tests above then enforce the same non-empty rule for it.
    for locale in REQUIRED_TRANSLATIONS {
        assert!(
            locale.is_ascii() && !locale.is_empty(),
            "a required locale is a directory name: {locale:?}"
        );
        assert!(
            po_path(locale).is_file(),
            "a required locale needs a .po document: {locale}"
        );
    }
}

#[test]
fn the_slint_scanner_handles_escapes_plurals_and_comments() {
    // The scanner is the test's own instrument, so its rules are pinned here:
    // comments are skipped, escapes are decoded, a plural contributes both
    // sources, and an argument string is not mistaken for a source.
    let source = "// @tr(\"a comment is not copy\")\n\
                  if toast: @tr(\"Imported {n} profile\" | \"Imported {n} profiles\" % count);\n\
                  @tr(\"Quote \\\" and brace \\\\\", name);\n\
                  /* @tr(\"a block comment is not copy\") */ text: @tr(\"plain\");";
    let found = translatables(Path::new("example.slint"), source);
    let sources: Vec<&str> = found
        .iter()
        .map(|translatable| translatable.singular.as_str())
        .collect();
    assert_eq!(
        sources,
        vec!["Imported {n} profile", "Quote \" and brace \\", "plain"]
    );
    assert_eq!(
        found[0].plural.as_deref(),
        Some("Imported {n} profiles"),
        "a plural call keeps both sources"
    );
}

#[test]
fn the_po_scanner_handles_continuations_escapes_and_plurals() {
    // The `.po` scanner is the other half of the instrument: multi-line values,
    // escapes and plural forms must round-trip, including a duplicate msgid
    // being an error.
    let directory = std::env::temp_dir().join(format!(
        "openscl4-po-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time is after the epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("a temporary directory is created");
    let path = directory.join("test.po");
    std::fs::write(
        &path,
        "msgid \"\"\n\
         msgstr \"\"\n\
         \"Language: ru\\n\"\n\
         \n\
         msgid \"a multi-line \"\n\
         \"msgid\"\n\
         msgstr \"a multi-line \"\n\
         \"translation\"\n\
         \n\
         msgid \"Quoted \\\" and \\\\\"\n\
         msgstr \"Перевод\"\n\
         \n\
         msgid \"one file\"\n\
         msgid_plural \"{} files\"\n\
         msgstr[0] \"один файл\"\n\
         msgstr[1] \"{} файла\"\n\
         msgstr[2] \"{} файлов\"\n",
    )
    .expect("the fixture is written");
    let entries = parse_po(&path);
    assert_eq!(
        entries["a multi-line msgid"].msgstr,
        "a multi-line translation"
    );
    assert_eq!(entries["Quoted \" and \\"].msgstr, "Перевод");
    let plural = &entries["one file"];
    assert_eq!(plural.msgid_plural.as_deref(), Some("{} files"));
    assert!(plural.has_translation() && plural.plural_forms_are_complete(3));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
#[should_panic(expected = "duplicate msgid")]
fn the_po_scanner_rejects_a_duplicate_msgid() {
    let directory = std::env::temp_dir().join(format!(
        "openscl4-po-duplicate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time is after the epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("a temporary directory is created");
    let path = directory.join("duplicate.po");
    std::fs::write(
        &path,
        "msgid \"same\"\nmsgstr \"one\"\n\nmsgid \"same\"\nmsgstr \"two\"\n",
    )
    .expect("the duplicate fixture is written");
    let _ = parse_po(&path);
    let _ = std::fs::remove_dir_all(directory);
}
