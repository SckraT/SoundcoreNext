# 05 — Review and release

**Status:** done
**Blocked by:** 04 (done)

## Goal

Review and merge the A3954 settings branch.

## Requirements

1. Run code review (standards + spec) against master.
2. Fix hard findings and accepted judgement calls.
3. Re-run desktop/core fmt/build/test/clippy; update docs (README, SOURCE if UI
   strings changed) and ticket records.
4. Merge `feature/a3954-settings` into master with a merge commit; clean
   worktrees; report commit and gates.

## Acceptance

- Review findings resolved; master green; release commit reported.

## Result (2026-11-23, MSK)

### Review findings and fixes

The review was run against `feature/a3954-settings` (findings supplied with the
ticket). Every finding is resolved:

1. **`desktop/ui/components/modes.slint` ModeRow toggle branch.** The manual
   `Toggle { y: (parent.height - self.height) / 2 }` is gone: the branch is a
   `HorizontalLayout` with `cross-axis-alignment: center`, and the raw `12px` /
   `4px` gaps are `Theme.space-3` / `Theme.space-1`. The label/description
   `VerticalLayout` keeps its main-axis centering, so the rendered row is
   unchanged (checked in the offline preview sheets below).
2. **NOTICE attribution.** Added the A3954 attribution block: the `01 01` state
   offsets/blocks, the `06 81` and 114-byte `03 87` body layouts and the measured
   settings frames from `lib/src/devices/soundcore/a3954/` (`state.rs`,
   `structures.rs`, `packets/`, `modules/`, `a3954.rs`), the 22-preset catalog
   from `common_settings_type_2()` in
   `lib/src/devices/soundcore/common/modules/equalizer.rs`, and the real-capture
   fixtures/evidence (`.scratch/a3954-settings/`). The trailing evidence list now
   names `.scratch/a3954-settings/`.
3. **A3954 states in `.scratch/ui-preview`.** The preview harness gained an
   `a3954` capture variant that renders the 1–11 scale with the wind-noise
   `ModeRow` toggle (`a3954-scale` on the Sound mode page and `a3954-overview`
   in the Overview block), 26 new captures over the light matrix (4 window sizes
   × 3 content scales × 2 states) and the dark matrix (2 scales). The preview
   compiled and was run end to end: 220 captures, and
   `.scratch/make_ui_sheets.py` gained `a3954-scale-light`,
   `a3954-overview-light` groups (the dark A3954 state lands in `dark-states`).
   The Russian real-app UIA matrix was not regenerated: its demo device is the
   A3062 and the A3954 pages need a real connection, but its translation
   preflight passes — every `@tr` string in `desktop/ui` is covered by
   `desktop/translations/ru/...po`, including the new reason string.
4. **English-only row copy.** The disabled spatial-audio selects' reason
   `Turn on spatial audio first` stays English controller data at the
   protocol/controller boundary and is now translated in the view through
   `LocalizedText.setting-reason` (`components/localized.slint`,
   `components/settings.slint`); `ru.po` gained
   `Сначала включите пространственный звук`. Unknown reasons pass through.
   UI_KIT/SOURCE record the rule; the controller test still asserts the English
   data value.
5. **Case language is out of scope.** The dead write API is removed:
   `A3954SettingCommand::CaseLanguage` and its `07 8A` body/frame are gone from
   `core`, `session` and the controller, along with the manual test phase and the
   related tests. The state byte 157 is still parsed as the read-only
   `A3954CaseLanguage` field (the case display has no page in this slice);
   README, NOTICE and the ticket-04 record say so.
6. **Misleading unused slider API.** `A3954SoundModeSlider::from_mode_and_strength`
   and `A3954SoundModeSlider::strength` were unused (tests only) and claimed a
   strength mapping this slice does not expose; both and their tests are deleted.
   The measured `new`/`normal`/`value`/`mode` mapping and its tests stay.
7. **Airplane-mode robustness.** The A3954 sound-mode block is now optional:
   `A3954State::sound_mode: Option<A3954SoundMode>`. The parser returns `None`
   when the block reports an unmeasured value (the real device's airplane-mode
   slider byte `13`) and the rest of the `01 01` state still parses; the three
   sound-mode `StateError` variants are gone. The controller hides the scale
   page, the `ModeRow`s and the Overview sound-mode block in that state, keeps
   the equalizer/settings pages and the connection, and refuses to derive a
   `06 81` body without a measured block. Regression tests use the observed
   mode=3/slider=13 bytes at the models and controller levels; the manual test
   prints “unavailable” and asks for airplane mode to be left before running.
   The byte's meaning stays unmeasured and no scale position is invented.
8. **Custom-EQ hardware proof.** The ignored `manual_a3954_settings` test gained a
   custom-curve phase and was run on the real A3954 (see the hardware run
   below): the connect preset is saved, a custom curve that differs from it is
   written, the re-poll must report the custom id `0xFEFE` and the written
   visible bands, then the connect preset is written back and re-polled before
   the existing final-state comparison. HearID remains preserved only.

### Live hardware run (ticket 05 custom-EQ proof)

`cargo test -p soundcore-desktop manual_a3954_settings -- --ignored
--nocapture` from `desktop` on the paired **real Liberty 4 Pro** (RFCOMM, the
same path as `manual_paired_connect`): **1 passed, 0 failed** in 16.4 s, **0
reconnects**. The device reported firmware **04.29/04.29**, case firmware
**01.58**, batteries 67/57 % and case 80 % during this run; the address is
masked in the output and no serial is read or printed.

- Connect equalizer: preset Acoustic id 1, left channel
  `[40, 10, 20, 20, 40, 40, 40, 20, 0, -120]` raw, right channel absent — the
  real device reports only its left equalizer channel.
- Built-in preset: Rock id 17 written, re-polled with its eight visible bands,
  then Acoustic id 1 restored and re-polled.
- **Custom curve:** `[-120, 10, 20, 20, 40, 40, 40, -60]` written through the
  measured 114-byte `03 87` body; the re-poll reported the custom id `65278`
  (`0xFEFE`) and left channel
  `[-120, 10, 20, 20, 40, 40, 40, -60, 0, 0]` (the written curve plus the two
  fixed bands), right channel still absent. The connect preset Acoustic id 1 and
  its curve were then restored and re-polled.
- The rest of the tested surface (scale positions 1/6/11, wind, every supported
  settings command, the mode cycle and spatial/easy-chat values, LDAC) was
  written, re-polled and restored again; the final poll matched the connect
  state field by field, the deferred button/case-feature/air-pressure bytes were
  untouched, and the dual-connections toggle was skipped because the
  `0B 01` list reported 4 remembered devices with 1 connected (turning it off
  would disconnect that other device). The case-language write is no longer part
  of the test (finding 5).

### Airplane-mode limitation (documented)

The real device reports sound-mode slider byte `13` while airplane mode is on.
The byte's meaning is unmeasured, so this slice does not map it: the sound-mode
block (and only that block) reads as unavailable, the scale/sound-mode page and
Overview block are hidden, and the rest of the state — batteries, equalizer,
every settings page, About — stays usable. Writing a `06 81` body from that
state is refused, so no invented position reaches the device. This is covered by
two unit regressions using the observed mode=3/slider=13 bytes and recorded in
README and ticket 04.

### Gates (on the ticket branch before the merge)

- Desktop (from `desktop`): `cargo fmt --all -- --check` clean, `cargo build`
  clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
  **288 passed / 0 failed / 14 ignored** (the 14 ignored are the existing manual
  hardware checks, including `manual_a3954_settings` that passed on the real
  A3954 above).
- Core (from `core`): `cargo fmt --all -- --check` clean,
  `cargo build --workspace` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean,
  `cargo test --workspace` **186 passed / 0 failed / 0 ignored**.
- Offline UI preview: `cargo build` + `cargo run` in `.scratch/ui-preview`
  clean, 220 captures, contact sheets regenerated; Russian translation
  preflight covers every `@tr` string.

No Bluetooth address or serial number is recorded in this file, the test output,
the UI runs or the captures.

### Merge and final gates

- Ticket branch: `ticket/a3954-review` @ `f076a1d` “Fix the A3954 settings review
  findings” (parent `7236a76` “Merge the live A3954 acceptance”).
- Merge commit: `feature/a3954-settings` @ `ff249f0` “Merge the A3954 settings
  review fixes”, made with `--no-ff`.
- Gates rerun on the merged `feature/a3954-settings` tree: desktop
  `cargo fmt --all -- --check` clean, `cargo build` clean,
  `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
  **288 passed / 0 failed / 14 ignored**; core `cargo fmt --all -- --check`
  clean, `cargo build --workspace` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean,
  `cargo test --workspace` **186 passed / 0 failed / 0 ignored**.
- The release merge of `feature/a3954-settings` into `master` is the next
  release step, reported from the release worktree.
### Release result

`feature/a3954-settings` was merged into `master` as `c377854` ("Add Liberty 4
Pro settings"). The final master tree was re-gated: desktop fmt/build/clippy
clean, 288 passed / 14 ignored; core fmt/build/clippy clean, 186 passed / 0
ignored. Ticket worktrees a3954-core/controller/live/review were removed. The
real A3954 was left as found; custom EQ and every supported setting were
restored and re-polled. Deferred: button remapping, case display/features/case
language writes, air-pressure writes, HearID editing; an airplane-mode state
whose slider byte is not 1..=11 now parses with the scale hidden instead of
failing the connection.
