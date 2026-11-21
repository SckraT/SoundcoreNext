# 03 — Equalizer presets acceptance

**Type:** task
**Status:** ready-for-agent
**Blocked by:** 02

## Goal

Verify equalizer presets on real A3062 and capture evidence.

## Requirements

1. Build/run; connect A3062; open Equalizer.
2. Verify current preset selected, 10 read-only bands/curve; switch between at least three presets (e.g. Signature, Bass Reducer, Podcast); each writes one `03 87`, re-polls, UI follows.
3. Verify P20i: no Equalizer page, no `03 87`.
4. Failure path: stopped worker shows readable error; no crash/MAC.
5. Capture `desktop/equalizer-connected.png`; link from `desktop/README.md`; document out-of-scope custom bands/profiles.
6. Run all gates.
7. Record OS, device/firmware (no MAC/serial), preset ids and results.

## Acceptance

- [ ] Real preset switching verified on A3062.
- [ ] P20i no-page/no-command verified.
- [ ] Screenshot stored/linked; no identifiers.
- [ ] Gates pass; ticket done.
