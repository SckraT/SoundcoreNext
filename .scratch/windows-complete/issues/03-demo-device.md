# 03 — Demo device mode

**Status:** ready-for-agent
**Blocked by:** 01

## Goal

Add a simulated A3062 so the Windows client can be exercised end-to-end without
Bluetooth hardware. No command may leave the process.

## Requirements

1. `Actions.set-demo(true)` on the model-picker step populates
   `paired-options` with a single deterministic simulated device (neutral MAC,
   e.g. `00:00:00:00:00:01`, name `soundcore Space One Pro · Demo`, code A3062).
2. `pair` in demo mode creates a `demo: true` saved card and a simulated
   connected state without opening RFCOMM or spawning a device worker.
3. Demo state is parsed from a committed sanitized A3062 fixture or built from
   the same `DeviceState` shape; it supplies sound mode, equalizer, settings,
   battery/firmware and Overview data.
4. All implemented pages render with demo data: Overview, Sound mode, Equalizer,
   Controls, Comfort, Power and prompts, Connections, About, Diagnostics empty/
   ready state as appropriate. `My devices` shows `Demo device`.
5. Controls update only the local demo `DeviceState` and visible models:
   - sound mode / EQ preset / custom bands / settings toggles;
   - EQ profile save/apply/delete and Overview layout/quick-toggle choices;
   - no `DeviceSession`, `DeviceCommand`, RFCOMM, filesystem device writes.
   Diagnostics probes are not run and no ZIP is produced from demo data; the
   page must say demo mode cannot capture or be disabled.
6. Demo disconnect returns to My Devices and keeps the demo saved card; demo
   reconnect restores the same state.
7. Settings writes in demo do not update through optimistic UI only in a way
   that hides failures; since there is no device, document demo-only behavior.
8. Tests: demo connect, every page populated, writes update local state, no
   command channel, disconnect/reconnect, Diagnostics demo messaging.

## Acceptance

- [ ] Add Device → Demo mode → connect works without Windows pairing.
- [ ] Every implemented page opens with data.
- [ ] Changing mode/EQ/settings updates locally; no RFCOMM or worker exists.
- [ ] Diagnostics clearly marked unavailable in demo.
- [ ] Desktop gates green.
