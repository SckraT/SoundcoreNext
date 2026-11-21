# 04 — Dual connections controller (list + connect/disconnect/forget)

**Type:** task
**Status:** ready-for-agent
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

- [ ] desktop/core gates pass.
- [ ] Real list/forget and connect/disconnect verified where possible.
- [ ] Only minimal UI edit; no redesign.
