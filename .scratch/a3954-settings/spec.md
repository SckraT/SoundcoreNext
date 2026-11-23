# Spec: Liberty 4 Pro (A3954) settings

**Status:** ready for implementation
**Tracker:** `.scratch/a3954-settings/`
**Branch:** `feature/a3954-settings`

## Goal

Replace the A3954's battery-only MVP surface with the measured Liberty 4 Pro
settings: state parsing, the 1–11 sound-mode scale, the ten-band/two-channel
equalizer with its own 22-preset catalog, and the settings pages the device
supports. Verify on the connected real A3954 and restore every changed value.

## Evidence

- Real A3954 capture: `D:\Pixel 8 Pro\Soundcore\analysis\captures\20261104-151814-A3954-Liberty4Pro/122013-797800-btsnoop_hci.log.decoded.json`
  (state body 165 bytes, EQ commands, 1–11 scale capture).
- `analysis/capabilities/a3954-eq-presets.json` — 114-byte EQ body and Rock/Flat examples.
- `analysis/capabilities/state-byte-map.csv` — A3954 field offsets.
- OpenSCL4 reference implementation (GPL, user-authorized data):
  `D:\Pixel 8 Pro\Soundcore\.patchgen\openscl4-work\lib\src\devices\soundcore\a3954\`
  (`state.rs`, `structures.rs`, `packets/inbound/state_update.rs`, `packets/outbound.rs`,
  `modules/`). Reuse protocol facts, not code wholesale.

## In scope

- **State:** A3954 body parsing for TWS batteries, charging flags, both firmware
  versions, serial (kept internal), case firmware/battery, equalizer, HearID raw
  block, buttons, ambient cycle, sound modes, case features, low-battery prompt,
  LDAC, dual connections, auto power off, limit high volume, spatial audio,
  easy chat, sound leak, wearing detection.
- **Sound mode:** 1–11 scale (1–5 transparency, 6 normal, 7–11 ANC), wind
  toggle; `06 81` five-byte body `[mode, position, airplane, wind, 0xFF]`.
- **Equalizer:** 8 visible / 10 band values per channel, 22 A3954 presets
  (including Bass Booster id 2), custom curve; 114-byte `03 87` body preserving
  HearID fields. Read-only? No: editable like A3062.
- **Effects:** LDAC, spatial audio switch + music mode/mode, sound leak
  compensation.
- **Comfort:** wearing detection, easy chat (enable + wait time), sound leak
  compensation, limit high volume (enable/dB/refresh).
- **Power:** auto power off (10/20/30/60 minutes), low-battery prompt.
- **Connections:** dual connections switch + remembered-device list.
- **About:** both earbud batteries, case battery and case firmware; no serial.
- **Controls:** ambient-sound cycle toggles; full per-gesture button remapping
  is deferred in this slice (document it).
- Live A3954 acceptance: connect, every page, one write per control with
  re-poll and restore; battery/identity; no serial leakage.

## Out of scope

- Button remapping (12 gestures), case display/features/language, air pressure,
  HearID editing, Android.
- A3062 behavior changes.

## Tickets

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-core-a3954.md` | — | Model/state/commands and tests in `core` |
| `02-controller-sound-eq.md` | 01 | Sound-mode scale and EQ pages wired end to end |
| `03-controller-settings.md` | 01,02 | Effects/Comfort/Power/Connections/About pages + writes |
| `04-live-acceptance.md` | 03 | Real A3954 acceptance and restore |
| `05-review.md` | 04 | Review, gates, release |

## Acceptance

1. A3954 state parses from the committed/live capture; A3062/A3949 tests unchanged.
2. All supported A3954 pages populate and every control writes the measured frame,
   re-polls, and shows the new state.
3. `cargo test/fmt/clippy` green in core and desktop; no serial in logs/UI.
4. Live A3954 acceptance passes; all changed settings restored.
