# 03 — Controller and UI wiring

**Type:** task
**Status:** ready-for-agent
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
4. Battery mapping: label 0/1/2 for left/right/case, `value = "NN%"`, `progress = raw / scale.maximum()`, `known = true`; unsupported single-battery layout → `batteries = []` and a readable failure/empty state, no crash.
5. `connect(mac)` reconnects a saved card; `disconnect` sets `connection = off` (or `no-device` when nothing selected) and clears the current card; `retry` repeats connect; `cancel-connect` aborts the pending attempt; `navigate(page)` sets `AppState.page`.
6. UI thread safety: worker → UI through an mpsc channel polled by `slint::Timer` or `invoke_from_event_handle`; never touch `AppWindow` from the worker.
7. Keep `sound-pages`, `device-pages`, `overview-rows`, `panels`, `quick-toggles` empty for this MVP; `linux = false`, `system-settings-available = false`.
8. Only `01 01` may be written; no settings writes.

## Acceptance

- [ ] Build/test/fmt/clippy pass.
- [ ] Manual: choose a model, see paired candidates, pair one, reach Overview, see battery tiles.
- [ ] Disconnect and retry work; Bluetooth-off and timeout show readable errors.
- [ ] No `.slint` edits; no MAC/serial in logs or UI text.
