# 01 — Core button and case commands

**Status:** done
**Blocked by:** —

## Goal

Add the measured A3954 button and case write APIs to core, reusing the parsed
state and existing button/case structures.

## Requirements

1. Button model:
   - A3954 gesture enum/order for the 12 buttons (left/right × single, double,
     triple, long, slide up, slide down), with side and measured button id
     (single 2, double 0, triple 5, long 1, slide-up 6, slide-down 7).
   - Action IDs from the measured A3954 table: `COMMON_ACTIONS` (0 volume up,
     1 volume down, 2 previous, 3 next, 4 ambient mode, 5 voice assistant,
     6 play/pause) and slide actions (0 volume up, 1 volume down); disabled
     action `0x0F`.
   - State helpers to read the main 8 actions (body 116..124) and four slide
     actions (161..165) in the measured order.
   - `04 81` body `[side, button_id, action]`; no set-all packet for A3954.
   - Validate unknown action/button values; expose option catalogs.
2. Case:
   - `07 87` body four flags from parsed `A3954CaseFeatures`.
   - `07 8A` body `[language]`; re-add `A3954SettingCommand::CaseLanguage` if
     needed (the previous slice removed the unused command).
   - Read-only case battery/firmware unchanged; no serial/address.
3. Session method(s) for the button/case writes; one frame per write, no read.
4. Tests: action/button id vectors, body vectors, state helper order, all
   existing A3062/A3954 tests unchanged.
5. Gates: core fmt/build/test/clippy.

## Acceptance

- Core green; exact bodies/ids documented; deferred case display/air pressure
  untouched.

## Implementation (2026-11-23)

### Button model (`core/crates/models/src/lib.rs`)

- `A3954Button`: the twelve gestures in measured state order — `LeftSingle`,
  `RightSingle`, `LeftDouble`, `RightDouble`, `LeftTriple`, `RightTriple`,
  `LeftLong`, `RightLong`, `LeftSlideUp`, `RightSlideUp`, `LeftSlideDown`,
  `RightSlideDown`. `index()`/`from_index()` follow that order; the first eight
  are state body 116..124 and the last four are 161..165.
- `A3954ButtonSide`: left `0`, right `1`.
- `A3954ButtonPressKind` with the measured `04 81` button ids: single `2`,
  double `0`, triple `5`, long `1`, slide up `6`, slide down `7`
  (`A3954Button::button_id()`).
- `A3954ButtonAction`: `0` VolumeUp, `1` VolumeDown, `2` PreviousSong,
  `3` NextSong, `4` AmbientSoundMode, `5` VoiceAssistant, `6` PlayPause.
  `A3954_NO_BUTTON_ACTION = 0x0F`. The catalogs are `A3954_COMMON_ACTIONS` (the
  seven ids for the six presses) and `A3954_SLIDE_ACTIONS` (`0`/`1`, shared by
  both slides; the ticket's slide set). No other id is exposed.
- `A3954ButtonActionByte` keeps the measured TwsLowBits byte
  `(non_tws << 4) | tws`. `tws()`/`non_tws()` decode it to
  `A3954ButtonActionValue` (`Action`, `Disabled`, `Unknown(nibble)`), and
  `with_tws`/`with_non_tws` replace one nibble while keeping the other, so a
  read→edit→write keeps the disconnected-state action and a restore sends
  exactly the reported byte. Unknown nibbles never fail the poll and are kept
  verbatim; only `A3954ButtonActionByte::from_byte` can carry one.
- `A3954State::button_action_byte(button)` and `button_action_bytes()` read the
  main block 116..124 and the slide block 161..165 in gesture order.
- `DeviceModel::a3954_button_frame(button, action)` builds the exact `04 81`
  frame with body `[side, button_id, action_byte]`. There is no set-all builder:
  the A3954 has no measured set-all button packet.

Measured action bytes in the committed capture: main
`66 66 32 33 FF FF 44 44`, slides `00 00 11 11`; the app's own writes are
`04 81 01 00 11` (right double, connected `1`, disconnected `1`) and
`04 81 01 00 12` (connected `2`), matching the research test vectors.

### Case commands

- `A3954SettingCommand::CaseFeatures(A3954CaseFeatures)` → `07 87`, body
  `[atmospheric, remote_camera, find_device, case_spatial_audio]` in state
  order (`A3954CaseFeatures::command_body`).
- `A3954SettingCommand::CaseLanguage(A3954CaseLanguage)` → `07 8A`, body
  `[language]`: Japanese `0`, English `1`, Chinese `2`
  (`A3954CaseLanguage::command_body`).
- State parsing is unchanged: case features 132..136 and language 157 remain
  parsed fields. Case battery/firmware remain read-only; nothing writes the
  serial or an address.

Exact frame examples: case features `08 EE 00 00 00 07 87 0E 00 00 00 01 00 93`
(the fixture's flags), language English `08 EE 00 00 00 07 8A 0B 00 01 93`.

### Session

- `set_a3954_button(button, action)` writes exactly one `04 81` frame and never
  reads; `poll_state` skips the empty ack and notifications afterwards.
- Case writes use the existing `set_a3954_setting`.
- A model without the A3954 blocks writes nothing: the button method returns
  the new `SessionError::UnsupportedButtonConfiguration`, the case settings keep
  `UnsupportedSetting`.

### Tests

- `core/crates/models/tests/a3954.rs`: gesture order/side/button-id and catalog
  vectors; action id/ActionByte decode, edit and unknown-preservation vectors;
  all twelve exact `04 81` frames plus the two app-captured bodies; the real
  fixture's twelve actions decoded; `07 87`/`07 8A` bodies and frames; unknown
  bytes kept through parse and write; non-A3954 models build no A3954 frames.
- `core/crates/session/tests/a3954.rs`: one-frame writes for all twelve gestures
  and the app vectors, the parsed state edited and written back with both case
  settings, and the unsupported-model path writing nothing.
- A3062/A3949: the full existing core suite still passes unchanged.

### Deferred (deliberately, documented in code)

Case display brightness/color/style/text writes, the `07 88` display read and
air-pressure writes stay out of this slice. No live hardware writes; no desktop
UI wiring (ticket 02). `README.md` and the live-device record are ticket 03.

### Desktop impact (not touched, per the ticket)

`desktop` no longer compiles until ticket 02 updates exhaustive matches:
`SessionError::UnsupportedButtonConfiguration` in the five
`controller.rs` conversions and the two new `A3954SettingCommand` variants in
`a3954_setting_label`. No desktop file was modified.

## Verification

- `core`: `cargo test` **195 passed / 0 failed / 0 ignored** across all targets,
  `cargo fmt --all -- --check` clean, `cargo build` clean, `cargo clippy
  --all-targets -- -D warnings` clean.
- New tests: `models/tests/a3954.rs` 26 tests (7 new), `session/tests/a3954.rs`
  11 tests (2 new); every frame is asserted byte for byte against
  `core/data/a3954-state.json` and the OpenSCL4-measured vectors.
- `desktop`: `cargo check --tests` reports only the six expected exhaustive-match
  errors listed above; nothing else regressed at the type level.
- Serial/address: unchanged and never written; case battery/firmware unchanged
  read-only fields.

## Acceptance

- [x] Core fmt/build/test/clippy green.
- [x] Exact IDs/bodies documented above; every frame tested.
- [x] Deferred case display/air-pressure untouched; no live hardware writes.
