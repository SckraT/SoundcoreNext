# 02 — Diagnostics page and worker

**Status:** done
**Blocked by:** 01

## Outcome

- `Page.diagnostics` with sidebar/Settings entry.
- `pages/diagnostics.slint`: Device, Capture, Review, Done; read-only copy;
  probe rows; archive summary; note; Save ZIP / Open GitHub issue.
- `DeviceCommand::RunDiagnostics` runs `PROBES` on the existing device worker,
  sends per-probe events and a completed `DiagnosticSession`.
- ZIP save worker uses `rfd`; issue URL is percent-encoded and opens through
  `ShellExecuteW`.
- A3949 (and any model without a kept worker) shows no available capture.
- Tests: probe-row mapping, issue URL, worker writes exactly one frame per probe
  and records tx/rx pairs.
