# 02 — Controller and Sound mode page

**Type:** task
**Status:** done
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

- [x] Build/test/fmt/clippy pass.
- [x] Real A3062: page appears, switches modes, state stays in sync.
- [x] P20i: no page, no `06 81`.
- [x] No `.slint` edits.

## Comments

Implemented on `ticket/sm-02-controller`; only `desktop/src/controller.rs`
changed (plus this ticket). No `.slint` file and no dependency changed.

- On a successful `01 01` parse, `show_sound_mode` fills the vendored page from
  `DeviceState.sound_mode`: `modes` are the three `ModeOption`s with the value
  strings `ModeText.icon` knows (`NoiseCanceling`, `Transparency`, `Normal`) and
  the labels `Noise canceling`, `Transparency`, `Normal`; `mode-index` is the
  reported mode (0/1/2, else -1); `noise-canceling-label`,
  `transparency-label` and `normal-label` are set; `sound-pages` is
  `[Page.sound-modes]`; `page` stays where the window was (Overview after Add
  Device). `mode-scale = false` and `mode-rows = []`, so ANC strength, wind, the
  1-to-11 scale and EQ stay out.
- A model without a measured mode (P20i/A3949 and every unmeasured model)
  reports `sound_mode: None`, so it gets no tiles, no labels, no Sound mode
  page, `mode-index = -1`, and no mode command path: its worker ends after the
  first state (closing the socket as in the MVP) and `set_option` ignores the id
  when there are no tiles. The session would also refuse with
  `UnsupportedSoundMode` and write nothing.
- The connect worker now stays alive for a mode-capable model: it polls the
  initial state, reports it, then serves `06 81` writes from an `mpsc` command
  channel on the same session. `Actions.set-option("ambientSoundMode", index)`
  maps the index to `SoundMode` (other ids and out-of-range indexes are
  ignored), increments a request number and sends the command; the UI thread
  only sends and never blocks. The worker writes the frame, re-polls `01 01`,
  and sends the mode the device reports. Stale results (a superseded click, a
  cancelled or new connection) are dropped by attempt/request number, like the
  MVP drops stale connect results. A failed write/read ends the worker (the
  session is out of step) and sets `error-label` to a readable string such as
  "Sound mode (no answer)" or "Sound mode (connection lost)"; the vendored error
  banner shows it and its Retry reconnects. Disconnect, cancel, retry, a new
  connect and window teardown all drop the command channel, which ends the
  worker and closes the socket.
- Pure helpers are unit-tested: `mode_value`/`mode_label` and page-order
  `mode_options`, `mode_index` ↔ `mode_for_index`, `sound_pages` (the real A3062
  capture exposes the page, the real A3949 capture does not), and
  `ModeFailure::from_session` with its readable labels. A scripted
  `FakeTransport` tests `change_sound_mode`: each of the three measured `06 81`
  frames is written exactly once, followed by exactly one `01 01` request; the
  reported mode is what the page shows; a model without a mode writes nothing;
  a silent device maps to a timeout and a reset link to a lost connection.

## Verification

- Desktop (from `desktop`): `cargo test` 85 passed / 0 failed / 5 ignored,
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo build` all pass.
- Core (from `core`): `cargo test` (26 passed across its test targets),
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo build` all pass.
- Real hardware, Space One Pro (A3062, firmware 04.39), through the ignored
  `manual_sound_mode_switch` test, which builds the real `AppWindow`, wires the
  controller's actions and presses the tiles through `Actions.set-option`:
  - on connect: `sound-pages = [SoundModes]`,
    `modes = [NoiseCanceling, Transparency, Normal]` with labels
    `[Noise canceling, Transparency, Normal]`, `mode-index = 2` (normal),
    `mode-scale = false`, `mode-rows = 0`, battery `20 %` (raw 2/10);
  - tiles 0, 1 and 2: each click wrote its `06 81` frame and the re-polled state
    moved `mode-index` to 0, 1 and 2 in turn;
  - three quick clicks 0, 1, 2 settled on 2: only the last result is applied;
  - after the worker was stopped (the state a failed change leaves), a click
    showed the readable banner `Sound mode (connection lost)` and wrote nothing,
    and the banner's Retry reconnected, cleared the banner, restored the page
    and read back `mode-index = 2`; the device was left in normal mode as found.
- Real hardware, P20i (A3949, firmware 14.43), through
  `manual_p20i_has_no_sound_mode`: `sound-pages = 0`, `modes = 0`,
  `mode-index = -1`; a real `Actions.set-option("ambientSoundMode", 0)` click
  changed nothing and showed no banner. There is no `06 81` path on that model:
  the worker ends before the command loop and the session refuses the write,
  which the `FakeTransport` test asserts writes nothing.
- MVP regressions re-run on the same hardware: `manual_paired_connect`
  (A3949 80 %/80 %, firmware 14.43; A3062 20 %, firmware 04.39; A3954 times out
  while off) and `manual_disconnect_and_retry` (A3062 connect, disconnect,
  retry) pass.
- No `.slint` file changed. No MAC or serial appears in the UI text or test
  output; the hardware tests mask addresses.
- The integration tip `e1e1d65` was already the branch base, so
  `git merge integration/windows-sound-modes` reported “Already up to date” and
  no merge commit was needed; the gates were re-run after that check.

## Review fixes

Resolved on `ticket/sm-04-review-fixes`:

- **Failure/click race.** A mode result is stale page data only when it
  succeeded: a failed write/read of the current attempt now always reaches
  `error-label`, even when a newer click has already superseded its request
  number. The worker ends after a failure, so the newer queued click never
  runs; surfacing the failure is the only report the user gets, the worker is
  stopped and the banner's Retry is available, so the queued click is clearly
  discarded rather than silently lost. Regression tests:
  `a_failed_change_surfaces_although_a_newer_click_was_queued` (the
  result-validity rule) and
  `a_failed_change_ends_the_worker_without_running_the_queued_click` (a
  scripted-transport worker run: one `06 81`, its timed-out re-poll, no second
  frame for the queued click, one surfaced failure). The worker body moved to
  the transport-generic `serve_device` so a `FakeTransport` can drive it.
- **Dead branches removed.** `ConnectFailure::NoSoundMode` and
  `ModeFailure::Unsupported` were unreachable — a connect only polls `01 01`,
  and a change is only written for a model whose state reported a mode — so
  the variants, labels and tests are gone; the mapping folds the write-only
  `SessionError::UnsupportedSoundMode` into the existing readable outcomes.
- **Top module doc** now describes the long-lived mode-capable worker and its
  second command channel instead of "short-lived worker threads over one
  mpsc".
- Hardware smoke re-run after the fixes: `manual_sound_mode_switch` (A3062
  connects, shows `[SoundModes]`, switches 0/1/2 with a re-poll each, quick
  clicks settle on the last, stopped-worker banner + Retry pass, device left in
  normal mode) and `manual_p20i_has_no_sound_mode` (P20i: no page, no tiles,
  no banner, no `06 81`) both pass.
