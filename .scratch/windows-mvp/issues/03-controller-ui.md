# 03 — Controller and UI wiring

**Type:** task
**Status:** done
**Blocked by:** 01, 02

## Goal

Wire the Windows transport and session into the vendored Add Device / Overview flow.

## Files

- `desktop/src/controller.rs` (new)
- `desktop/src/main.rs`
- `desktop/Cargo.toml` if needed

Do not edit any `.slint` file.

## Requirements

1. Add Device opening:
   - `adding = true`, step 0, reset query/kind, `add-loading = true`;
   - enumerate paired devices on a worker thread; set `paired-options` and `add-loading = false`;
   - mark `suggested = true` when the device exposes the chosen model's vendor RFCOMM UUID (or its name/code matches the model);
   - `add-bluetooth-off = true` when enumeration fails because the radio is off.
2. `refresh-paired` repeats the enumeration for the current selection.
3. `pair(mac, name)`:
   - read the selected `add-model-code`;
   - set `device-name`, `device-code`, `device-art`, `device-is-speaker`, `one-earbud = false`, `failure-kind = none`, `failure-text = ""`;
   - add/replace the current card in `devices` (`current = true`);
   - close the dialog, set `page = Page::overview`, `connection = Connection::connecting`;
   - connect on a worker thread: pick vendor/SPP service, open the Windows transport, create the session, `poll_state()`;
   - on success set `connection = connected`, `batteries` from `DeviceState`, and keep the UI responsive;
   - on failure set `connection = failed`, `failure-kind` (`not-found`/`timed-out`/`other`) and a readable `failure-text`; never expose a raw MAC.
4. Battery mapping: label 0/1 for left/right, `value = "NN%"`, `progress = raw / scale.maximum()`, `known = true`; a case tile (label 2) only if a state ever provides a case value (the MVP's `01 01` answers carry none); unsupported single-battery layout → `batteries = []` and a readable failure/empty state, no crash.
5. `connect(mac)` reconnects a saved card; `disconnect` sets `connection = off` (or `no-device` when nothing selected) and clears the current card (the vendored UI has no control that calls it in this MVP, so it is verified by the ignored hardware test); `retry` repeats connect; `cancel-connect` aborts the pending attempt; `navigate(page)` sets `AppState.page`.
6. UI thread safety: worker → UI through an mpsc channel polled by `slint::Timer` or `invoke_from_event_handle`; never touch `AppWindow` from the worker.
7. Keep `sound-pages`, `device-pages`, `overview-rows`, `panels`, `quick-toggles` empty for this MVP; `linux = false`, `system-settings-available = false`.
8. Only `01 01` may be written; no settings writes.

## Acceptance

- [x] Build/test/fmt/clippy pass.
- [x] Manual: choose a model, see paired candidates, pair one, reach Overview, see battery tiles.
- [x] Connect, retry and the disconnect callback work (the vendored UI has no disconnect control; disconnect is verified by the ignored hardware test); Bluetooth-off and timeout show readable errors.
- [x] No `.slint` edits; no MAC/serial in logs or UI text.

## Comments

- Implemented on `ticket/win-03-controller`; `desktop/src/controller.rs` owns the worker
  threads and the single `mpsc` channel, and a `slint::Timer` (20 ms) drains it on the Slint
  thread. No worker touches `AppWindow`; stale enumeration/connect results are dropped by
  sequence number, and `cancel-connect` sets the attempt's cancellation flag without waiting.
- Pairing reads the chosen `add-model-code`, fills the hero/card, closes the dialog, navigates
  to Overview, and connects on a worker with the cached SDP vendor UUID (standard SPP
  fallback). Only the session's `01 01` state request is written; nothing else is sent.
- Pure helpers are unit-tested (70 fast tests): selected-model lookup, paired-row MAC
  round-trip, battery mapping for the 5/10/100-step scales, failure mapping, candidate
  filtering/suggestion (vendor UUID first, name/code match for SPP-only models), and current
  card replacement. The ignored `manual_paired_connect` test repeats the hardware check with
  masked addresses.
- Battery tiles use labels 0/1 (left/right) with `NN%` and `raw / scale.maximum()`. The TWS
  CMM2 state parser has no case field, so a case tile (label 2) is not invented. The real
  masked A3949 capture in `core/data/a3949-state.json` parses to 100 %/80 % in a test.
- Manual UI check with Windows UI Automation on the running app (`target/debug/soundcore-desktop.exe`):
  - Add Device → Liberty 4 Pro listed `soundcore P20i`, `soundcore Liberty 4 Pro`
    (`Paired in Windows · looks like the chosen model`) and `soundcore Space One Pro`.
  - Real P20i (A3949 candidate suggested): pairing reached Overview `Connecting…`, then after
    ~5 s `Could not connect` + `The device did not answer in time. Try again.`; no MAC in the
    UI. Retry showed `Connecting…` again, Cancel returned to `Not connected` and the stale
    timeout result did not override it.
  - Real Space One Pro (A3062): RFCOMM connect and `01 01` succeeded; the state frame parsed
    as an unsupported battery layout and the UI showed `This model reports a battery layout the
    app cannot read yet, so battery levels are unavailable.` — readable, no crash, no MAC.
  - The P20i and Liberty 4 Pro were not powered on/reachable at test time (radio page timeout).
    A positive battery read from them on hardware is still pending for ticket 04; the
    captured-frame checks cover the UI path and the mapping meanwhile.
- `disconnect` is wired but the vendored UI has no control that calls it; `connect`/`retry`/
  `cancel-connect`/`navigate`/`refresh-paired`/`pair` are wired. `sound-pages`,
  `device-pages`, `overview-rows`, `panels`, `quick-toggles`, `all-toggles` stay empty;
  `linux = false` and `system-settings-available = false`.
- Checks run from `desktop`: `cargo test` (70 passed, 2 ignored), `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`; from `core`: `cargo test`.
  No `.slint` file changed.
