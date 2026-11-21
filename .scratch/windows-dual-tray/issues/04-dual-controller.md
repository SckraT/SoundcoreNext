# 04 — Dual connections controller (list + connect/disconnect/forget)

**Type:** task
**Status:** done
**Blocked by:** 03

## Goal

Show the dual-connections list and let the user forget, connect and disconnect devices.

## Files

- `desktop/src/controller.rs`
- `desktop/ui/types.slint` (Actions: add `connect-item(string,int)`, `disconnect-item(string,int)`)
- `desktop/ui/components/settings.slint` (list row: add Connect/Disconnect ghost buttons next to Forget; same style)
- no other `.slint` changes

## UI exception

This spec allows the minimal `.slint` edit above because the vendored list only offers Forget. Keep layout/style/strings consistent; no redesign.

## Requirements

1. After A3062 connect, if dual connections is on: read `0B 01` list on worker (multi-frame, bounded); store devices aligned with displayed options.
2. Connections page: Plain panel with dual toggle + DualDevices list (name, connected marker e.g. “Name — connected”); list hidden/cleared when dual off and no `0B 01` sent.
3. `remove-item(id,index)` → `0B 83` forget MAC; wait/ACK semantics; re-read list; update panel.
4. New `connect-item`/`disconnect-item(id,index)` → `0B 82`/`0B 81` MAC; connect can take seconds; then re-read list; update connected markers; readable error on failure.
5. Refresh list when navigating to Connections and after toggle on.
6. MAC bytes kept in reply order (not Windows ullLong); names sanitized/truncated as parsed; no MAC/serial in UI/logs.
7. A3949 deferred: no page/list/writes.
8. Tests: option/index alignment, forget/connect/disconnect command dispatch, re-read after write, dual off hides/no fetch, error mapping, A3949.
9. Hardware A3062: list, forget (use a disposable/non-critical remembered device), connect/disconnect if a second device available; restore pairing as found.

## Acceptance

- [x] desktop/core gates pass.
- [x] Real list/forget and connect/disconnect verified where possible.
- [x] Only minimal UI edit; no redesign.

## Implementation (2026-11-22)

All controller work is in `desktop/src/controller.rs`; the only `.slint` change is
the two ghost buttons and their callbacks the spec allows (see below).

### Worker: read after connect, ordering around the toggle

`serve_device` computes `dual_enabled` from the model capability
(`DeviceModel::dual_connections`, A3062 only) and the re-polled
`settings.dual_connections`, sends the `Connected` event, then reads the `0B 01`
list once on that same session when the setting is on. With the setting off no
`0B 01` is written by the connect worker.

A `SetSetting(DualConnections(..))` command goes through the existing
`change_setting` (one `0B 84` frame, then the `01 01` re-poll). The worker then
reads the list only when the device's own re-polled answer reports the setting
turned **on** — so the order is `0B 84`, `01 01` re-poll, `0B 01`, and a
refused/failed toggle reads nothing. Turning the setting off does the write and
re-poll only; the UI clears the stored list, which removes the panel.

`ReadDualConnections` re-reads on demand (entering the Connections page), and
`DualConnectionWrite(DualAction)` writes exactly one measured `0B 82`/`0B 81`/
`0B 83` frame and then re-reads:

- `DualAction::Connect`/`Disconnect` re-read (bounded: 4 attempts, 400 ms apart)
  until the list reports the device connected/disconnected, because the A3062
  can take a few seconds; the last reply is shown either way.
- `DualAction::Forget` re-reads until the device is gone from the list.
- A dropped link (`DualFailure::Disconnected`) ends the worker; a timeout or
  unreadable answer keeps it serving and surfaces the banner.

### Connections page and actions

`panels_for_page` keeps the `Plain` dual-connections toggle and, only while
`settings.dual_connections` is on, appends a `DualDevices` panel whose single
`List` row id is `dualDevices`. The option strings and the row actions are both
built from the same `Vec<DualConnectionDevice>` the worker last reported, in
reply order, so an option index is always the index of the same device:
`Name — connected` for a connected device, `Unknown device` for an empty name.
The six address bytes are never formatted for the UI or the logs; only the
parsed name reaches the options.

The wired `Actions.remove-item` (previously unhandled), `connect-item` and
`disconnect-item` resolve the row id/index against that stored list (an unknown
row id, an out-of-range index or a hidden list writes nothing) and queue the
matching `DualAction`; the index's MAC is used unchanged. When the worker of a
failed write has already closed the session, the click writes nothing and the
banner names the action, like the other pages' writes.

Entering the Connections page queues a fresh `0B 01` read only when the list is
visible (dual on); the list is cleared on a new connect attempt, a connection
failure, a disconnect and when the device reports dual connections off, so no
stale row can be forgotten or connected while it is off.

Errors map to readable banners: `Dual connections list (no answer)` / `(connection
lost)` / `(unreadable answer)` for reads, `Forget device (…)`,
`Connect device (…)`, `Disconnect device (…)` for row actions — never an address.

The A3949 (no settings block, no dual capability) gets no panel, no list read and
no write; the capability flag is false too.

### UI edit

`desktop/ui/types.slint`: `connect-item(string,int)` and
`disconnect-item(string,int)` callbacks in `Actions`.
`desktop/ui/components/settings.slint`: one Connect and one Disconnect
`ButtonKind.ghost` button next to the existing Forget button in the list row,
same `y`/style pattern; no layout, label or design change.

## Verification

- `desktop`: `cargo test --offline` **215 passed / 0 failed / 12 ignored** (the
  12 are the pre-existing manual hardware checks plus the new read-only
  `manual_dual_connections_list`), `cargo fmt --all -- --check` clean,
  `cargo clippy --offline --all-targets -- -D warnings` clean (only the
  pre-existing vendored-Slint `y` layout warnings), `cargo build --offline`
  clean.
- `core`: `cargo test --offline` **153 passed / 0 failed**, `cargo fmt --all --
  --check` and `cargo clippy --offline --all-targets -- -D warnings` clean (core
  unchanged by this ticket).
- The 10 worker-test failures left by the previous partial implementation were
  fixed, not weakened:
  the unrelated worker tests now connect with a dual-off A3062 state (new
  `frame_with_dual` helper patches body offset 79 and the checksum), so the
  automatic list read is not part of their scripted sequence; the exact write
  logs are unchanged otherwise. The toggle test now runs one worker through
  off→on→off and asserts the exact sequence `01 01`, `0B 84`, `01 01`, `0B 01`,
  `0B 84`, `01 01` with exactly one list read; the lost-link write test asserts
  the `0B 01` frame written before the scripted read failure.
- New/kept automated tests:
  - list option/index alignment (`dual_list_options_and_the_actions_share_one_index`,
    plus the UI-level `a_dual_list_row_action_writes_the_device_at_that_option_index`),
    empty-name option, connected marker;
  - worker sequences: read after connect when on and none when off, the toggle
    ordering above, each of `0B 82/81/83` + re-read, connect settling over two
    replies, read failure keeps the worker serving, lost link ends it;
  - UI: row action refused while the list is hidden or the worker is gone,
    toggling off clears the stored list and hides the panel, entering
    Connections queues a fresh read only while the list is visible;
  - error mapping (`a_dual_failure_names_the_list_or_the_action_and_the_reason`);
  - A3949: no Connections panel and no list read/write
    (`the_deferred_a3949_never_reads_or_writes_a_dual_list`).
- The wire format and parser used here are ticket 03's: the committed sanitized
  real `0B 01` capture (`core/data/a3062-dual-connections.json`) is what the
  controller parses and displays here.

## Hardware check (2026-11-22, read-only, not verified live)

The read-only `manual_dual_connections_list` check connects the real paired
A3062, enters Connections through the wired action and asserts the panel/option
alignment without writing any list-row frame. Windows lists three paired A3062
candidates; none answered the RFCOMM connection within 20 s (tried each
candidate, also with 15 s and 30 s waits around 01:45–01:55 MSK — the earbuds
appear to be off or out of range). No frame beyond the `01 01` state poll was
sent; no forget/connect/disconnect was attempted, so the user's phone pairing
and device settings are untouched.

What is real, not scripted: the wire format and parser used here are the
sanitized capture from this same A3062 taken read-only in ticket 03, and the
auto-read/order logic is covered by the scripted-transport tests above. Live
list display and the write actions still need the ticket 05 hardware
acceptance run when the earbuds are powered on; forget/connect/disconnect were
deliberately not exercised on the real device because the only remembered
devices are the user's own phone/PC pairings.
