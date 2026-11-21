# 02 — Controller and Sound mode page

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Expose the core sound mode through the vendored Sound mode page and write changes.

## Files

- `desktop/src/controller.rs`
- `desktop/Cargo.toml` only if a dependency is genuinely needed
- no `.slint` edits

## Requirements

1. On connect, read `DeviceState.sound_mode`; for A3062 set:
   - `AppState.modes = [Noise canceling, Transparency, Normal]` with `ModeOption.value` values the vendored `ModeText` recognises (`NoiseCanceling`, `Transparency`, `Normal`);
   - `mode-index`, `normal-label`, `transparency-label`, `noise-canceling-label`;
   - `sound-pages = [Page.sound-modes]`, `page` stays Overview;
   - `mode-scale = false`, `mode-rows = []` (strength/wind out of scope).
2. For models without sound mode (A3949/P20i), `sound-pages = []` and no mode commands.
3. Handle `Actions.set-option("ambientSoundMode", index)`:
   - map the index to `SoundMode`; ignore other setting ids;
   - write via the session; re-poll state; update `mode-index`;
   - on error show `error-label`/failure text; never write on unknown index.
4. UI worker rules from the MVP still apply: no UI thread blocking, stale results dropped.
5. Tests for index mapping, labels, A3062 vs A3949 page exposure, and error mapping. Hardware test path may be ignored.
6. No strength, wind or EQ wiring.

## Acceptance

- [ ] Build/test/fmt/clippy pass.
- [ ] Real A3062: page appears, switches modes, state stays in sync.
- [ ] P20i: no page, no `06 81`.
- [ ] No `.slint` edits.
