# 06 — Review fixes and release

**Status:** done
**Blocked by:** 05

## Goal

Review the completed Windows client and close the integration branch.

## Requirements

1. Run the code-review skill on `integration/windows-complete` against master
   (standards + spec axes); record findings.
2. Fix all hard findings and accepted judgement calls in one implementer pass.
3. Re-run desktop/core fmt, check, clippy, tests, build; update README/docs to
   match actual behavior (no overclaims).
4. Ensure no .po/reference artifacts or secrets are committed; ignore local
   scratch preview validation where appropriate.
5. Merge `integration/windows-complete` into `master` with a merge commit; clean
   ticket worktrees; report final commit and verification.

## Acceptance

- [x] Review findings resolved or documented.
- [ ] Gates green on merged master — the review-fix branch is green on the
      unchanged `integration/windows-complete` tip; merging the integration
      branch into `master` is the release step after ticket 05, and no hardware
      acceptance is claimed here.
- [x] README and UI_KIT/GUIDE accurate.
- [x] No local WIP/untracked production files left.

## Verification

### Review findings

The code review of `integration/windows-complete` against master (standards and
spec axes) left nine findings; all are fixed here, without changing device
behavior or adding any hardware acceptance claim.

1. **Scaled wide breakpoints.** `desktop/ui/app.slint` still chose the Equalizer,
   Presets and Settings two-column arrangements with the unscaled
   `root.width - sidebar.width > 900px`. All three now use
   `900px * Theme.content-scale`, like the Overview, Diagnostics and the section
   pages (`860px * Theme.content-scale`), as UI_KIT §3 requires. The wide and
   narrow branches are unchanged in `pages/equalizer.slint`,
   `pages/settings.slint` and `pages/presets.slint`; they still render the same
   content in two different arrangements.
2. **SOURCE.md completeness.** `desktop/ui/SOURCE.md` now records the remaining
   material vendored changes: the `pages/devices.slint` saved-card demo status
   text (`Connected · Demo device` / `Demo device · Not connected`), the
   `pages/presets.slint` `FieldRowView` `accessible-checkable` addition, the
   `RangeSlider` `accessible-action-set-value` wiring, this ticket's scaled
   breakpoints, the tokenised device identity and the 150 % choice, plus the
   `types.slint` content-scale comments.
3. **DeviceIdentity layout.** `pages/overview.slint`'s `DeviceIdentity` no longer
   positions the status dot with a manual `y` inside a managed layout and no
   longer uses raw 9/6/3 px or `24px * scale` values: it uses
   `cross-axis-alignment: center` and the `Theme.space-*`/`radius-control` tokens
   (the image and badge base sizes still multiply `Theme.content-scale` because
   they are intentional base dimensions, not tokens). Identity and status pill
   stay centred at 110/125/150 % in the preview matrix.
4. **Demo wording honesty.** `desktop/README.md`, `spec.md` and
   `issues/03-demo-device.md` said the demo path has no worker and sends no
   device commands, which is false: Demo mode uses an in-process demo worker
   thread on the same event/command channel as a real device worker. The README,
   the spec (in scope and acceptance 2/4), ticket 03 (requirements 2/5 and
   acceptance) and the ticket 04 verification now say: no Bluetooth/RFCOMM
   link, no device session and no Windows device lookup; the in-process demo
   worker serves the local simulated state; every `DeviceCommand` is handled in
   that process and no command leaves it; no frame is written to a device. The
   `Inner::demo` comment in `controller.rs` says the same. No behavior changed.
5. **Wrong Overview screenshot.** `desktop/demo-overview.png` was a Comfort page
   capture. It is replaced with
   `.scratch/wc04-uia/shots/overview-demo-1280x820-100-light.png` (126,987 bytes,
   sha256 `c43844f5…a0b0`), inspected visually: Overview heading, demo hero
   (`soundcore Space One Pro · Demo`, 20 % battery, Connected badge), Quick
   settings with the LDAC chip, Sound mode and Your sound panels, and the four
   Standard-layout shortcut cards. The README caption matches the image and the
   demo-worker wording; the Screenshots section now says the Demo-mode captures
   come from the ticket 04 acceptance build and that the Settings capture
   predates the 150 % choice.
6. **Sound-mode view deferred.** The 1–11 Sound-mode view is not implemented and
   no fake UI was added. `spec.md`, `issues/02-settings-wiring.md`,
   `issues/04-demo-acceptance.md` and the README record it as deferred until a
   core model exposes that scale; `render_settings` keeps
   `app-mode-view-applies=false`, so the Settings control stays hidden, while
   the saved choice still round-trips for a future model.
7. **150 % content scale.** `ContentScale::Percent150` was missing although the
   UI kit contract (UI_KIT §3, GUIDE §5) and the visual matrix require it.
   `desktop/src/settings.rs` adds it with the document value `"150"`, choice
   index 3 and factor 1.5 (`from_index`, `index`, `factor`, serde); the Settings
   page offers `100%`, `110%`, `125%`, `150%`; `render_settings` maps index 3 to
   `AppState.app-content-scale` and `apply_theme` applies
   `Theme.content-scale` 1.5. Tests cover index/serde/factor, the content-scale
   action and the AppState mapping; the invalid-index test accepts 0…3 and
   rejects 4. The preview matrix now renders the Settings control with the
   capture's own scale, so 150 % captures show the selected choice. The full
   matrix was rendered at 150 % (800×700, 1030×760, 1280×820, 1440×900) and
   inspected: the touched pages stack as intended and nothing is clipped
   horizontally, so there is no blocker to record.
8. **Cosmetic.** `.gitignore` has its final newline. No generated or scratch
   output is staged: `core/target/`, `desktop/target/` and
   `.scratch/ui-validation/` are ignored, and no `.po`/`.pot`, scratch capture,
   log or secret file is tracked.
9. **Gates, commit and merge.** Recorded below.

### Visual matrix

The preview harness (`.scratch/ui-preview`, software renderer under the machine's
125 % display scaling) produced **194 captures** and the contact sheets from
`.scratch/make_ui_sheets.py`; output is local in `.scratch/ui-validation/`
(ignored). The matrix covers 800×700, 1030×760, 1280×820 and 1440×900 at
100/125/150 % content scale, light, for 13 pages (Overview, My Devices,
Equalizer, Presets, Controls, Comfort, Power, Connections, About, Sound and
effects, Settings, Sound mode and Diagnostics), the dark captures and the
dialog/review captures. The Equalizer and Presets pages were added to the
harness for this ticket because their breakpoints changed.

Affected pages at 1280×820, checked here: at 100 % Equalizer, Presets and
Settings keep their wide two-column arrangement; at 125 % and 150 % they stack
(the scaled threshold) with the aside below the main column and all actions
reachable by scrolling; Settings shows the 150 % option selected at 150 % and
at 800×700; the Overview hero keeps the identity and status pill centred at
110/125/150 %. No horizontal clipping, no content outside its panel. Screenshots
are in `.scratch/ui-validation/` as noted above.

### Gates

On the final tree (fix commit `2fe6be2`, integration tip merged), from
`desktop`:

- `cargo fmt --all --check`: clean.
- `cargo build`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: **270 passed, 0 failed, 13 ignored** — the 13 ignored are the
  same manual real-device checks as tickets 01–04; no hardware was run.

From `core` (untouched by this ticket, run for the slice):

- `cargo fmt --all --check`: clean.
- `cargo build --workspace`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test`: **156 passed, 0 failed, 0 ignored** across 22 suites.

### Integration and leftover work

- Branch: `ticket/wc-06-review-fixes`, based on
  `integration/windows-complete` @ `c98917e`.
- Review fixes: commit `2fe6be2` ("Fix the Windows client review findings").
- `git merge integration/windows-complete`: **Already up to date** (the tip did
  not move, so there is no merge commit to make); gates were re-run on the
  merged tree afterwards. This ticket record is the commit after the fixes.
- Ticket 05 (real A3062 live acceptance) remains separate and is not claimed
  here. The `integration/windows-complete` → `master` merge and the ticket
  worktree cleanup are the release steps after ticket 05; ticket 06's review
  fixes and gates are complete on the integration tip.
