# 01 — Read-only probe core

**Status:** done
**Blocked by:** —

## Outcome

`DeviceSession::probe_frame(command)` writes one read-only request and returns
the first complete reply frame of the same command, skipping unrelated frames.
The transport bounds a silent wait. Tests cover the exact write, skipped frames,
timeout mapping and the existing session suite.
