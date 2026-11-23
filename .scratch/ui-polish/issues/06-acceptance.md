# 06 — Acceptance and release

**Status:** done
**Blocked by:** 01–05
**Implemented:** 2026-11-24

## Requirements

1. Run the app (demo A3062 and real A3954) and verify:
   - breadcrumb arrow centered at 100/125/150%;
   - Settings has no Diagnostics block; Copy logs works from Diagnostics;
   - Effects page long descriptions/reasons do not cross dividers at 800×700 and
     150%;
   - LDAC quick-toggle reason is Russian; switch language and verify.
2. Regenerate the Russian visual matrix/contact sheets where useful; no clipping.
3. Run desktop/core fmt/build/test/clippy, including translation test.
4. Code review standards/spec; fix findings.
5. Merge into master; clean worktrees; report commits, gates, screenshots and any
   remaining limitations.

## Acceptance

- All four user-visible issues fixed; translation test enforced; master green.

## Verification (2026-11-24)

The run used the ticket branch (`f2669c1` plus the acceptance commit) on Windows
11 Pro 10.0.26300, rustc/cargo 1.99.1, a 2560×1440 display at 125 % scaling. The
app was always started with `APPDATA`/`LOCALAPPDATA` pointing at the run folder
(`.scratch/ui-validation/acceptance-06/appdata/…`), so the user's
`%APPDATA%\OpenSCL4` was never read or written. The demo device is the built-in
simulated Space One Pro (A3062); the device checks used the paired soundcore
Liberty 4 Pro (A3954, firmware 04.29) which was in its one-earbud state (left /
right / case 54 / 61 / 60 %). The app only sends the read-only `01 01` state
request there; no device setting was written or restored. No Bluetooth address
or serial number is printed in the log or present in the captures.

The automated acceptance script is `.scratch/ui-polish/acceptance-06.ps1`
(`-Phase demo|live|all`); it drives the window through Windows UI Automation,
saves the raw captures under `.scratch/ui-validation/acceptance-06/shots/`, and
writes `acceptance.log` with PASS/FAIL and the measured values.

### Results

**Breadcrumb (requirement 1).** The chevron is centred by the page-heading
layout at every content scale. Measured from the UIA rectangles on the
Sound-and-effects breadcrumb (device name / chevron / section name):

| state | device y-centre | chevron y-centre | section y-centre | max delta |
|---|---|---|---|---|
| demo 1280×820, 100 % | 110.0 | 109.5 | 110.0 | 0.5 px |
| demo 1280×820, 125 % | 112.5 | 112.5 | 112.5 | 0.0 px |
| demo 1280×820, 150 % | 114.0 | 113.5 | 114.0 | 0.5 px |
| A3954 1280×820, 100 % | 238.0 | 237.5 | 238.0 | 0.5 px |
| A3954 1280×820, 150 % | 242.0 | 241.5 | 242.0 | 0.5 px |
| A3954 800×700, 150 % | 122.0 | 121.5 | 122.0 | 0.5 px |

**Settings (requirement 1).** The demo A3062 Settings page has no Diagnostics
card: the only Diagnostics entry point is the sidebar tab, and no
`Открыть диагностику` / `Скопировать логи` buttons exist in the page. No button
overflows the window at 1280×820 (100 % and 150 %) or at the 800×700 minimum
window with 150 % content scale; the content buttons are the expected segmented
controls and the aside actions only. Captures:
`shots/settings-ru-1280x820-100.png`, `…-150.png`, `…-800x700-150.png`.

**Copy logs (requirement 1).** Diagnostics shows **Скопировать логи** at the foot
of the page on every step. Clicking it put 53 characters on the Windows
clipboard: the `OpenSCL4 2.0.0 (windows, x86_64)` header followed by the run's
application log (`Application started` included), and showed the **Скопировано**
toast. The previous clipboard text was restored afterwards. Verified on both
the demo Diagnostics page and the real-device Diagnostics page; the capture
`shots/diagnostics-copy-logs-live-1280x820.png` shows the real A3954 device
summary with the toast.

**Effects rows (requirement 1).** The real A3954 Effects page (one earbud)
shows the disabled spatial-audio rows with the Russian reason
“Сначала включите пространственный звук” under the long labels
“Режим пространственного звука для музыки” and “Режим пространственного
звука”. Captured at 1280×820 with 100 % and 150 % content scale and at 800×700
with 150 %: the labels and reasons wrap, each row grows, and every divider stays
below its own row — no text crosses a divider and no control is clipped
horizontally. Captures: `shots/effects-long-rows-ru-1280x820-100.png`,
`…-150.png`, `…-800x700-150.png`.

**Quick-toggle reason (requirement 1).** With the real A3954 reporting only one
earbud, the Overview Quick settings **LDAC** chip is disabled and its reason is
“LDAC: Изменение доступно, когда подключены оба наушника” in Russian. The
**App language** select was switched to English in the running window and the
reason became “LDAC: Changing is available when both earbuds are connected”,
then switched back to Russian; no restart was needed. The one-earbud badge
(“1 наушник” / “1 earbud”) is present in both captures.

**Translation test (requirement 1/3).** `cd desktop && cargo test --test translations`
passes (8 tests). Re-running the recorded deliberate failure reproduced the
documented rejection: emptying the Russian `msgstr` of
`Could not build the archive: {}` made
`required_translations_have_a_non_empty_translation_for_every_ui_string` and
`every_messages_key_has_a_catalog_branch_and_a_translated_source` fail with
`empty msgstr for "Could not build the archive: {}"` (and the `messages.rs` key
`diagnostics.build-failed`), and the string was restored afterwards.

**Visual matrix (requirement 2).** The Russian demo matrix was regenerated with
`tools/ui-capture/capture-ui.ps1` at 100/125/150 % on the fixed pages plus a
800×700 run (100 % and 150 %), and the new page/scale states were inspected with
no clipping, no missing translation and no button overflow. The
`tools/ui-capture` preflight now also enforces the plural-form rule below.

### Code review (requirement 4)

Reviewed the branch against master `3ae7303` along the Standards and Spec axes
(diff `git diff 3ae7303...HEAD`).

Fixed hard findings:

1. **Translation test did not enforce all required plural forms.** It required
   at least two non-empty plural `msgstr` forms, so removing `msgstr[2]` from the
   Russian plural entries (a required form for the header's
   `nplurals=3`) still passed. This was demonstrated on the branch and is a gap
   against acceptance item 5 (“the test catches a missing/empty required
   translation”). `desktop/tests/translations.rs` now reads `nplurals` from the
   `Plural-Forms:` header, requires every declared form to be present and
   non-empty, and a new test pins the header parser and the missing-form case.
   The capture preflight (`tools/ui-capture/capture-ui.ps1`) was extended with
   the same plural-form rule; with two `msgstr[2]` lines removed both the test
   (`8 tests, 3 problems for Imported/Moved`) and the preflight
   (`incomplete plural msgstr: Imported {n} profile (needs 3 non-empty forms)`)
   now fail, and pass again after restoring the document.
2. **`tools/ui-capture/README.md` documented a `-NoBuild` switch that does not
   exist** (the script only builds with the opt-in `-Build`). The quick-run
   example now omits it and says it runs against an already-built executable.
3. **`desktop/README.md` still called the failure banner's second sentence a
   “vendored” sentence** after the banner moved to the runtime message catalog;
   the wording now matches the catalog and the plural-form rule is described
   next to the translation-test contract.

Judgement calls recorded, no code change: `src/messages.rs` keeps an
`ALL_KEYS` test list that duplicates the constants (the authoritative scan lives
in `tests/translations.rs`); the preview-only `preview.long-reason` catalog
branch is production code used only by `.scratch/ui-preview` (documented in the
catalog); the text-level empty-`msgstr` guard in `messages.rs` is weaker than
the `.po` parser in the integration test, which remains the primary check.

Spec review found no missing requirement and no scope creep. Protocol/device
behavior is unchanged: the diff touches UI strings, message plumbing, layout and
docs only. The `ConnectFailure` enum gained `WorkerStart`, `DemoWorkerStart` and
`SavedAddress` so the app's own failures use catalog keys instead of English
transport text; the `Transport` OS detail is now a technical argument inside the
translated sentence. `show_diagnostics_device` no longer branches on the app
language in Rust (the branch and hardcoded Russian moved to the catalog, which
also fixes the old double-`%` battery text).

### Gates (requirement 3)

From `desktop/` on the final ticket tree:

- `cargo fmt --all -- --check` — pass
- `cargo check` — pass
- `cargo test` — 306 passed, 0 failed, 15 ignored (hardware only), 8 translation
  tests passed (the translation test runs both in `cargo test` and on its own)
- `cargo clippy --all-targets -- -D warnings` — pass
- `cargo test --test translations` — 8 passed
- core (unchanged by this ticket, re-run): `cargo fmt --all -- --check`,
  `cargo check --workspace`, `cargo test --workspace` — 195 passed, 0 failed;
  `cargo clippy --workspace --all-targets -- -D warnings` — pass.
- `tools/ui-capture/capture-ui.ps1` preflight — pass (Russian translations cover
  every `@tr` source with a non-empty translation and every declared plural
  form); the app launched with the Russian title and captured every page.

### Screenshots

The ticket-06 evidence images are tracked in `desktop/` and described in
`desktop/README.md` under “UI polish and translated runtime copy (ticket 06)”:

- `desktop/settings-ru.png` — Russian Settings page, no Diagnostics card.
- `desktop/diagnostics-copy-logs-ru.png` — real A3954 Diagnostics with
  **Скопировать логи** and the **Скопировано** toast.
- `desktop/breadcrumb-100-125-150-ru.png` — breadcrumb crops at 100/125/150 %.
- `desktop/quick-settings-one-earbud-ru.png` / `…-en.png` — the Russian LDAC
  one-earbud reason and the English result after switching the app language.
- `desktop/effects-long-rows-ru-1280x820-150.png` and
  `desktop/effects-long-rows-ru-800x700-150.png` — wrapped disabled spatial rows
  with no text crossing a divider.

The raw captures are in `.scratch/ui-validation/acceptance-06/shots/` (generated,
not tracked) and the capture-matrix runs are in `artifacts/ui-screenshots/`
(generated, ignored).

## Limitations

- The real A3954 happened to be in its one-earbud/TWS-down state, which is
  exactly the state the quick-toggle reason needs; the verification therefore
  used read-only state polling. No device write path was exercised by this
  ticket (ticket 01–05 already cover behavior), so the run makes no new
  hardware-acceptance claim beyond the read-only state and the screenshots.
- The native notification-area menu wording and the native file-dialog titles /
  filter names remain untranslated; they are outside the Slint catalog and are
  recorded as the remaining native surfaces in `desktop/README.md`.
- The screenshot pixel sizes are physical pixels at the display's 125 % Windows
  scaling (1280×820 logical → 1600×1025 px), the same convention as the earlier
  demo/live captures.

## Integration

Merged into `feature/ui-polish-translations` and then into `master` with
`--no-ff` after these gates; the branch worktrees were removed as part of the
release cleanup. The merge commit hashes are recorded in the handoff report.
