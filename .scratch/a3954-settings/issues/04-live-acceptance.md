# 04 — Live A3954 acceptance

**Status:** done
**Blocked by:** 03 (done)

## Goal

Verify on the connected Liberty 4 Pro and restore it.

## Requirements

1. Fresh build; connect A3954 over RFCOMM (device is paired and powered).
2. Capture all supported pages; verify sound-mode scale positions 1/6/11,
   wind, EQ preset/custom, Effects (LDAC/spatial/leak), Comfort, Power,
   Connections, About; one write per control with re-poll.
3. Restore every changed value to the initial reading; assert equality.
4. No serial/MAC in screenshots, logs or issue text; no diagnostics changes.
5. Record firmware, screenshots (no address), exact checks, and any deferred
   setting. Update README only with real-device facts.
6. Gates: desktop/core fmt/build/test/clippy.

## Acceptance

- All checks pass; device left as found; ticket done.

## Result (2026-11-23, MSK)

**Hardware.** Real Liberty 4 Pro paired with the test machine and powered on,
connected over Windows RFCOMM. The runs read firmware **04.29** on both earbuds
and case firmware **01.58**; the device reported left/right/case batteries
69/69/80% during the UI captures (the protocol run started at 71/71%; the
battery changed during the long acceptance, as expected). No pairing was
touched.

**Protocol run.** From `desktop`, on the fresh build:
`cargo test -p soundcore-desktop manual_a3954_settings -- --ignored
--nocapture` → **1 passed, 0 failed**. The ignored test
`desktop/src/controller.rs::manual_a3954_settings` connects the paired A3954
through `open_session` (the same RFCOMM path as `manual_paired_connect`) and
drives a real `DeviceSession`.

Each line below is one write, a `01 01` re-poll assertion, then a restore
asserted by a second re-poll:

- sound-mode scale positions **1** (Transparency), **6** (Normal) and **11**
  (Noise canceling), then back to the connect position 6 (Normal); airplane
  mode and the wind flag kept;
- wind-noise suppression off → on → off;
- equalizer preset Acoustic id 1 → Rock id 17 → Acoustic id 1, curve asserted
  as the eight visible bands each time (custom-curve and HearID writes were not
  run: deferred by this slice);
- low battery prompt on → off → on;
- auto power off on/30 min → off → on/30 min;
- limit high volume off/90 dB → on/75 dB → off/90 dB, and refresh rate
  RealTime → TenSeconds → RealTime;
- ambient sound-mode cycle ANC+Transparency → Normal → ANC+Transparency;
- wearing detection off → on → off;
- sound leak compensation off → on → off;
- spatial audio off → on → (music mode Head tracking, effect mode Podcast) →
  off with the connect modes (Fixed, Music);
- easy chat off/5 s → on/5 s → on/10 s → off/5 s;
- LDAC off → on → off (it did not reset the RFCOMM link in this run: 0
  reconnects);
- case language English → Japanese → English (the `07 8A` language write has no
  page in this slice; it was exercised once because the re-poll confirms it and
  it restored. Ticket 05 removed the unused `07 8A` write API as out of scope;
  the parsed state field stays read-only);
- Connections: the read-only `0B 01` list returned **4 remembered devices with
  1 connected**, so the dual-connections toggle was **skipped** (turning it off
  would disconnect that other device), as required; only the list was read.

The final poll matched the connect state for the scale, the equalizer preset and
visible curve, every settings value, the case language, the deferred raw
button/case-feature/air-pressure bytes and both firmware strings; the test
printed no serial or address.

**Live device quirk found.** The device silently drops a setting command that
arrives too soon after the previous one. It reproduced on the scale writes:
position 11 needed 3 attempts and the restore to position 6 needed 3 attempts
before the re-poll showed them. The test therefore waits 350 ms before every
write (the measured minimum is 250 ms) and retries until the re-poll confirms
the requested value, so a dropped command never leaves a half-applied value.
This is a protocol-behavior fact worth keeping for the Android client.

**UI run.** The built app (`cargo build` from `desktop`) was started with an
isolated `%APPDATA%` (English UI; the user's app data untouched), connected the
paired Liberty 4 Pro through the real Add Device dialog (real RFCOMM, Demo mode
off) and every A3954 page populated:
Overview (hero, left/right/case battery, scale at position 6, Acoustic curve,
shortcuts), Sound mode (1–11 slider at position 6, wind row), Equalizer (22
presets, Acoustic selected, 8 bands), Sound and effects (LDAC/spatial/leak rows
with the device reason for the disabled spatial selects), Controls (button
remapping note, cycle toggles), Comfort (wearing/easy chat/wait/leak/limit
rows), Power and prompts (auto power off, low battery prompt), Connections
(dual toggle plus the 4-device remembered list and its connected marker) and
About (left/right/case battery, both 04.29 and case 01.58, no serial row). The
UIA run clicked no setting control, so no value was written through the UI, and
it checked every page for a Bluetooth address and a serial. The 1–11 slider
exposed the device's position 6. Screenshots (no address/serial) are committed
as `desktop/live-a3954-overview.png`, `...-sound-mode.png`, `...-equalizer.png`,
`...-effects.png` and `...-about.png`.

**Deferred.** No custom equalizer curve write, no HearID change, no button
remapping, no case-features/display write and no air-pressure write. The case
language command has no page in this slice (its write was exercised only as the
safe, restore-verified protocol check above; ticket 05 removed that unused write
API as out of scope). Ticket 05 later added a hardware-proof custom-curve phase
to the ignored test and ran it: the custom curve was written and re-polled as
`0xFEFE`, then the connect preset was restored and re-polled (see
`05-review.md`). HearID editing is still not done.

## Follow-up observation (outside this ticket's exercised path)

While probing the write protocol before the acceptance run, a `06 81` write
with the airplane mode (`mode` byte 3, slider 6) made the following `01 01`
states report slider byte **13**; the current parser rejects the whole state
with `InvalidA3954SoundModeSlider(13)`. A device switched to airplane mode would
therefore fail the state poll. The observation was reproducible across repeated
polls and a reconnect, and writing Normal recovered the device; the probe
restored Normal **before** the acceptance run, and the acceptance itself never
enters airplane mode (the UI has no airplane chip and the airplane-mode update
setting is deferred). Ticket 05 should decide whether the A3954 state parser
must tolerate the airplane-mode slider value instead of rejecting the state,
with a captured frame as evidence; it is not changed here because the exact
meaning of byte 13 is unmeasured and inventing a value would violate the
parser's measured-offsets rule.

**Resolved by ticket 05.** The A3954 sound-mode block is now optional
(`A3954State::sound_mode: Option<A3954SoundMode>`): an unmeasured byte in the
block makes only the block unavailable (`None`), while the rest of the `01 01`
state parses and the connection stays usable. The controller hides the scale
page and the Overview's sound-mode block in that state, and no `06 81` frame is
derived without a measured block. A regression test uses the observed
mode=3/slider=13 bytes; the byte's meaning is still unmeasured and no scale
position is invented for it.

## Gates

- Desktop (from `desktop`): `cargo fmt --all -- --check` clean,
  `cargo build` clean, `cargo clippy --all-targets -- -D warnings` clean,
  `cargo test` **286 passed / 0 failed / 14 ignored** (the 14th ignored test is
  this ticket's `manual_a3954_settings`; the other 13 are the existing manual
  hardware checks).
- Core (from `core`): `cargo fmt --all -- --check` clean,
  `cargo build --workspace` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean,
  `cargo test --workspace` **185 passed / 0 failed / 0 ignored**.

No Bluetooth address or serial number is recorded in this file, the test
output, the UI run or the captures.
