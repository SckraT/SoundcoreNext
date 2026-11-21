//! Device models and their state parsers.
//!
//! Two state layouts are implemented from real captures:
//!
//! - the TWS Cmm2 layout, confirmed by the real A3949 capture (left/right
//!   batteries, two firmware strings, serial);
//! - the single-battery Cmm2 layout, measured on the real A3062 (one battery on
//!   the ten-step scale, one firmware string, serial, sound mode and equalizer).
//!
//! The TLV D1203 layout stays unimplemented: returning `UnsupportedLayout` is
//! better than inventing offsets.

use soundcore_protocol::{Command, Frame, parse};

pub mod catalog;

/// Raw maximum of a battery field. A value of 5 means 20 % steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryScale {
    Five,
    Ten,
    Hundred,
}

impl BatteryScale {
    pub const fn maximum(self) -> u8 {
        match self {
            Self::Five => 5,
            Self::Ten => 10,
            Self::Hundred => 100,
        }
    }
}

/// The ambient-sound mode of a model with a measured `06 81` control.
///
/// The variants are the three modes the vendored Sound mode page offers. ANC
/// strength, wind noise and adaptive ANC are separate controls and are not
/// modelled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundMode {
    NoiseCanceling,
    Transparency,
    Normal,
}

impl SoundMode {
    /// Every measured mode, in the order the Sound mode page shows them.
    pub const ALL: [Self; 3] = [Self::NoiseCanceling, Self::Transparency, Self::Normal];

    /// The mode's `06 81` command value, which is also its `01 01` state byte.
    ///
    /// Measured on A3062 (2026-11-21): `0` noise canceling, `1` transparency,
    /// `2` normal.
    pub const fn value(self) -> u8 {
        match self {
            Self::NoiseCanceling => 0,
            Self::Transparency => 1,
            Self::Normal => 2,
        }
    }

    /// The mode a `06 81` or `01 01` value names, or `None` for a value that is
    /// not one of the measured modes.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::NoiseCanceling),
            1 => Some(Self::Transparency),
            2 => Some(Self::Normal),
            _ => None,
        }
    }
}

/// A measured sound-mode control.
///
/// The mode value is the first byte of the `06 81` command body and the only
/// part of it that changes; the rest of the body was measured on A3062 and is
/// recorded here so the command builder has no per-model code branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundModeCapability {
    /// Body offset of the mode byte in the `01 01` state response.
    pub state_offset: usize,
    /// The five fixed bytes of the `06 81` body that follow the mode byte.
    pub command_tail: [u8; 5],
}

/// Number of measured A3062 equalizer bands: ten state bytes, one per band.
pub const EQ_BANDS: usize = 10;

/// The preset id a device reports while custom band values are active, so the
/// current curve is not one of the built-in [`EqualizerPreset`]s.
pub const CUSTOM_EQUALIZER_PRESET_ID: u16 = 0xFEFE;

/// The full length of the A3062 `03 87` equalizer body, measured from
/// OpenSCL4's A3062 packet builder and verified on the real device.
pub const EQUALIZER_COMMAND_BODY_LEN: usize = 53;

/// One built-in equalizer preset of a model: its id, display name and ten raw
/// band values.
///
/// A raw band value is in the device's dB×10 units: the `01 01` state byte is
/// `value + 120`, so `0` is `0 dB` and `-120` is `-12.0 dB`. The A3062 band
/// byte range is `0..=254`, i.e. `-120..=134`; OpenSCL4 clamps preset values
/// to the same range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqualizerPreset {
    /// The preset id the state reports, little-endian in the `01 01` body.
    pub id: u16,
    /// The English display name from OpenSCL4's A3062 preset list.
    pub name: &'static str,
    /// Ten raw band values in dB×10.
    pub bands: [i16; EQ_BANDS],
}

/// The built-in equalizer presets of the Space One Pro (A3062), copied from
/// OpenSCL4's `devices/soundcore/a3062/modules/equalizer.rs` list.
///
/// OpenSCL4 lists twenty-two presets for A3062: the ids `0`, `1` and `3..=21`,
/// plus Bass Booster under the unusual id `0x7E7E`. Id `2` is not in the A3062
/// list. Custom bands are reported as [`CUSTOM_EQUALIZER_PRESET_ID`] and are
/// deliberately not a preset here: this app can only select built-ins.
///
/// The names are OpenSCL4's English localized names, which are what its UI
/// shows; its internal settings names have no spaces (for example
/// `SoundcoreSignature`).
pub const A3062_EQUALIZER_PRESETS: &[EqualizerPreset] = &[
    EqualizerPreset {
        id: 0,
        name: "Soundcore Signature",
        bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
    },
    EqualizerPreset {
        id: 1,
        name: "Acoustic",
        bands: [40, 10, 20, 20, 40, 40, 40, 20, 0, -120],
    },
    EqualizerPreset {
        id: 3,
        name: "Bass Reducer",
        bands: [-40, -30, -10, 0, 0, 0, 0, 0, 0, -120],
    },
    EqualizerPreset {
        id: 4,
        name: "Classical",
        bands: [30, 30, -20, -20, 0, 20, 30, 40, 0, -120],
    },
    EqualizerPreset {
        id: 5,
        name: "Podcast",
        bands: [-30, 20, 40, 40, 30, 20, 0, -20, 0, -120],
    },
    EqualizerPreset {
        id: 6,
        name: "Dance",
        bands: [20, -30, -10, 10, 20, 20, 10, -30, 0, -120],
    },
    EqualizerPreset {
        id: 7,
        name: "Deep",
        bands: [20, 10, 30, 30, 20, -20, -40, -50, 0, -120],
    },
    EqualizerPreset {
        id: 8,
        name: "Electronic",
        bands: [30, 20, -20, 20, 10, 20, 30, 30, 0, -120],
    },
    EqualizerPreset {
        id: 9,
        name: "Flat",
        bands: [-20, -20, -10, 0, 0, 0, -20, -20, 0, -120],
    },
    EqualizerPreset {
        id: 10,
        name: "Hip Hop",
        bands: [20, 30, -10, -10, 20, -10, 20, 30, 0, -120],
    },
    EqualizerPreset {
        id: 11,
        name: "Jazz",
        bands: [20, 20, -20, -20, 0, 20, 30, 40, 0, -120],
    },
    EqualizerPreset {
        id: 12,
        name: "Latin",
        bands: [0, 0, -20, -20, -20, 0, 30, 50, 0, -120],
    },
    EqualizerPreset {
        id: 13,
        name: "Lounge",
        bands: [-10, 20, 40, 30, 0, -20, 20, 10, 0, -120],
    },
    EqualizerPreset {
        id: 14,
        name: "Piano",
        bands: [0, 30, 30, 20, 40, 50, 30, 40, 0, -120],
    },
    EqualizerPreset {
        id: 15,
        name: "Pop",
        bands: [-10, 10, 30, 30, 10, -10, -20, -30, 0, -120],
    },
    EqualizerPreset {
        id: 16,
        name: "RnB",
        bands: [60, 20, -20, -20, 20, 30, 30, 40, 0, -120],
    },
    EqualizerPreset {
        id: 17,
        name: "Rock",
        bands: [30, 20, -10, -10, 10, 30, 40, 50, 0, -120],
    },
    EqualizerPreset {
        id: 18,
        name: "Small Speakers",
        bands: [40, 30, 10, 0, -20, -30, -40, -40, 0, -120],
    },
    EqualizerPreset {
        id: 19,
        name: "Spoken Word",
        bands: [-30, -20, 10, 20, 20, 10, 0, -30, 0, -120],
    },
    EqualizerPreset {
        id: 20,
        name: "Treble Booster",
        bands: [-20, -20, -20, -10, 10, 20, 20, 40, 0, -120],
    },
    EqualizerPreset {
        id: 21,
        name: "Treble Reducer",
        bands: [0, 0, 0, -20, -30, -40, -40, -60, 0, -120],
    },
    EqualizerPreset {
        // OpenSCL4: "yes, this number is correct. no idea why it's this
        // instead of the usual 2."
        id: 0x7E7E,
        name: "Bass Booster",
        bands: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
    },
];

/// The built-in preset with `id`, or `None` for [`CUSTOM_EQUALIZER_PRESET_ID`]
/// and any id that is not in the A3062 list.
pub fn a3062_equalizer_preset(id: u16) -> Option<&'static EqualizerPreset> {
    A3062_EQUALIZER_PRESETS
        .iter()
        .find(|preset| preset.id == id)
}

/// The equalizer block of a parsed A3062 `01 01` state.
///
/// | body offset | field |
/// |---:|---|
/// | 23..25 | preset id, little-endian |
/// | 25..35 | ten band bytes; value = byte - 120 |
/// | 35..37 | unknown, skipped |
/// | 37..65 | the HearID block, kept verbatim for writes |
///
/// The HearID block is not interpreted: HearID is out of scope. It is copied
/// from the state into every preset write so selecting a preset does not erase
/// the device's HearID data, exactly as OpenSCL4's A3062 equalizer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqualizerState {
    /// The preset id at body offset 23..25; [`CUSTOM_EQUALIZER_PRESET_ID`] when
    /// custom bands are active.
    pub preset_id: u16,
    /// Ten raw band values in dB×10, read as `byte - 120`.
    pub bands: [i16; EQ_BANDS],
    /// The measured HearID state block at body offset 37..65.
    hear_id: [u8; HEAR_ID_BLOCK_LEN],
}

impl EqualizerState {
    /// Whether the device reports custom bands instead of a built-in preset.
    pub fn is_custom(&self) -> bool {
        self.preset_id == CUSTOM_EQUALIZER_PRESET_ID
    }

    /// The built-in preset this state reports, or `None` for custom bands and
    /// unknown ids.
    pub fn preset(&self) -> Option<&'static EqualizerPreset> {
        a3062_equalizer_preset(self.preset_id)
    }
}

/// The measured A3062 equalizer control: where its block starts in the `01 01`
/// body and how many bands it carries.
///
/// A model's custom-band marker is [`CUSTOM_EQUALIZER_PRESET_ID`]: it is a
/// property of the protocol shape, not a per-model setting, so it is not stored
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqualizerCapability {
    /// Body offset of the preset id in the `01 01` state response.
    pub state_offset: usize,
    /// Number of bands in the state and in the command body.
    pub band_count: usize,
}

/// The A3062 band conversion offset: a state byte is `value + 120`.
const BAND_BYTE_OFFSET: i16 = 120;
/// Bytes of the A3062 HearID block inside the state, from the end of the two
/// unknown bytes after the bands to the end of the favorite music genre.
const HEAR_ID_BLOCK_LEN: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDevice {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryState {
    pub left: u8,
    pub right: u8,
    pub left_charging: bool,
    pub right_charging: bool,
    pub scale: BatteryScale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareVersions {
    pub left: Option<String>,
    pub right: Option<String>,
}

/// The battery and identity fields of a single-battery layout.
///
/// A3062 answers `01 01` with one battery value instead of the left/right pair
/// of the TWS layout. The measured body holds, in order: the raw battery step,
/// one reserved byte, a single firmware string (`MM.mm` ASCII) and the serial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleBatteryState {
    /// Raw battery step, at most [`BatteryScale::maximum`].
    pub value: u8,
    /// The scale of [`Self::value`]; A3062 uses [`BatteryScale::Ten`], where a
    /// raw `5` is 50 %.
    pub scale: BatteryScale,
    /// The charging flag, when the state frame carries one.
    ///
    /// The measured A3062 `01 01` body has no charging byte: charging is
    /// reported by a separate `01 04` notification, which this state poll does
    /// not request. The parser therefore leaves this `None` instead of
    /// inventing `false`.
    pub charging: Option<bool>,
    /// The single firmware string, when the body holds a valid `MM.mm` pair.
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceState {
    /// The TWS link state. TWS-only: single-battery layouts fix it at
    /// [`HostDevice::Left`]/`false`, because they have no host earbud.
    pub host_device: HostDevice,
    /// Whether the two earbuds are connected to each other. TWS-only: always
    /// `false` for single-battery layouts.
    pub tws_connected: bool,
    /// Left/right batteries of the TWS Cmm2 layout; `None` for single-battery
    /// layouts.
    pub battery: Option<BatteryState>,
    /// The single battery of the single-battery layout; `None` for TWS layouts.
    pub single_battery: Option<SingleBatteryState>,
    /// The measured environmental sound mode; `None` for models without a
    /// measured sound-mode control (for example the P20i).
    pub sound_mode: Option<SoundMode>,
    /// The measured equalizer preset and band values; `None` for models whose
    /// equalizer this slice does not implement (for example the P20i A3949,
    /// whose equalizer block is known but deliberately deferred).
    pub equalizer: Option<EqualizerState>,
    /// The two TWS firmware versions. Single-battery layouts report their one
    /// firmware string in [`SingleBatteryState::firmware`] instead.
    pub firmware: FirmwareVersions,
    /// The serial is deliberately optional and usually `None` in captures.
    /// Callers must not log it.
    pub serial: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    TwsCmm2,
    SingleCmm2,
    Tlv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceModel {
    pub code: &'static str,
    pub name: &'static str,
    pub battery_scale: BatteryScale,
    pub layout: Layout,
    /// The measured sound-mode control, or `None` when the model's sound modes
    /// were not measured (or it has none, like the P20i).
    pub sound_mode: Option<SoundModeCapability>,
    /// The measured equalizer control, or `None` when this slice does not
    /// implement the model's equalizer. The A3949 is in that group deliberately:
    /// its state block is known (body 32..44) but its write body is unverified,
    /// so it has no capability, page or `03 87` frame yet.
    pub equalizer: Option<EqualizerCapability>,
}

impl DeviceModel {
    pub const fn is_tws(self) -> bool {
        matches!(self.layout, Layout::TwsCmm2)
    }

    /// The exact six-byte `06 81` body for a sound mode, or `None` when this
    /// model has no measured sound-mode control.
    pub fn sound_mode_body(self, mode: SoundMode) -> Option<[u8; 6]> {
        let capability = self.sound_mode?;
        let [a, b, c, d, e] = capability.command_tail;
        Some([mode.value(), a, b, c, d, e])
    }

    /// The built-in equalizer presets of this model, in display order; empty
    /// when this slice does not implement the model's equalizer (for example
    /// the deferred A3949).
    pub fn equalizer_presets(self) -> &'static [EqualizerPreset] {
        if self.code == A3062.code {
            A3062_EQUALIZER_PRESETS
        } else {
            &[]
        }
    }

    /// The full A3062 `03 87` body that selects `preset` while keeping the
    /// device's current HearID block, or `None` when this slice does not
    /// implement the model's equalizer (for example the deferred A3949).
    ///
    /// The body is 53 bytes, the shape OpenSCL4's A3062 equalizer builder
    /// writes:
    ///
    /// | bytes | field |
    /// |---:|---|
    /// | 0..2 | preset id, little-endian |
    /// | 2..4 | HearID favorite music genre, from the state |
    /// | 4..14 | the preset's ten band bytes |
    /// | 14..16 | unknown, zero |
    /// | 16 | HearID enabled: always `0` for a preset write |
    /// | 17..27 | HearID volume adjustments, from the state |
    /// | 27..31 | HearID time, big-endian, from the state |
    /// | 31 | HearID type, from the state |
    /// | 32..42 | HearID custom volume adjustments, from the state |
    /// | 42..52 | the preset's ten band bytes again (active curve) |
    /// | 52 | unknown, zero |
    ///
    /// The HearID fields are preserved, and its enabled flag is cleared, so the
    /// selected preset curve is in effect; this is what OpenSCL4's A3062
    /// equalizer does.
    pub fn equalizer_preset_body(
        self,
        current: &EqualizerState,
        preset: &EqualizerPreset,
    ) -> Option<[u8; EQUALIZER_COMMAND_BODY_LEN]> {
        let capability = self.equalizer?;
        if capability.band_count != EQ_BANDS {
            return None;
        }
        let mut body = [0u8; EQUALIZER_COMMAND_BODY_LEN];
        body[0..2].copy_from_slice(&preset.id.to_le_bytes());
        body[2..4].copy_from_slice(&current.hear_id[26..28]);
        let bands = preset.bands.map(band_byte);
        body[4..14].copy_from_slice(&bands);
        // 14..16 are unknown zeroes.
        body[16] = 0;
        body[17..27].copy_from_slice(&current.hear_id[1..11]);
        body[27..31].copy_from_slice(&current.hear_id[11..15]);
        body[31] = current.hear_id[15];
        body[32..42].copy_from_slice(&current.hear_id[16..26]);
        body[42..52].copy_from_slice(&bands);
        // 52 is an unknown zero.
        Some(body)
    }
}

/// The `01 01` byte of a raw band value in dB×10.
fn band_byte(value: i16) -> u8 {
    let clamped = value.clamp(-BAND_BYTE_OFFSET, 254 - BAND_BYTE_OFFSET);
    u8::try_from(clamped + BAND_BYTE_OFFSET).expect("a clamped band value fits the state byte")
}

/// The P20i (A3949): its `01 01` capture carries an equalizer block at body
/// 32..44 (preset id `00 00` and ten flat band bytes), so the model does have
/// equalizer data, but no A3949 `03 87` write body is verified. This slice
/// therefore defers A3949 equalizer support deliberately: no capability, no
/// preset catalog, no page and no command. That is a limitation, not evidence
/// that the model has no equalizer.
pub const A3949: DeviceModel = DeviceModel {
    code: "A3949",
    name: "Soundcore P20i / P25i / R50i",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
};

pub const A3947: DeviceModel = DeviceModel {
    code: "A3947",
    name: "Soundcore Liberty 4 NC",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
};

pub const A3954: DeviceModel = DeviceModel {
    code: "A3954",
    name: "Soundcore Liberty 4 Pro",
    battery_scale: BatteryScale::Hundred,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
};

pub const A3035: DeviceModel = DeviceModel {
    code: "A3035",
    name: "Soundcore Space One",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
};

pub const A3040: DeviceModel = DeviceModel {
    code: "A3040",
    name: "Soundcore Space Q45",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
};

pub const A3062: DeviceModel = DeviceModel {
    code: "A3062",
    name: "Soundcore Space One Pro",
    battery_scale: BatteryScale::Ten,
    layout: Layout::SingleCmm2,
    // Measured on the real device on 2026-11-21: the mode is body byte 69 and
    // the command body is `[mode, 0x51, 0x01, 0x01, 0x00, 0x05]`.
    sound_mode: Some(SoundModeCapability {
        state_offset: 69,
        command_tail: [0x51, 0x01, 0x01, 0x00, 0x05],
    }),
    // Measured on the real device on 2026-11-21: the preset id is body
    // 23..25, the ten band bytes are 25..35 and custom bands report 0xFEFE.
    equalizer: Some(EqualizerCapability {
        state_offset: 23,
        band_count: EQ_BANDS,
    }),
};

pub const SUPPORTED_MODELS: &[DeviceModel] = &[A3035, A3040, A3062, A3947, A3949, A3954];

pub fn model_by_code(code: &str) -> Option<DeviceModel> {
    SUPPORTED_MODELS
        .iter()
        .copied()
        .find(|model| model.code == code)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    NotAStateFrame,
    ShortBody { actual: usize, minimum: usize },
    UnsupportedLayout(Layout),
    InvalidHostDevice(u8),
    InvalidTwsFlag(u8),
    InvalidSoundMode(u8),
    BatteryAboveScale { value: u8, maximum: u8 },
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAStateFrame => write!(f, "frame is not a 01 01 response"),
            Self::ShortBody { actual, minimum } => {
                write!(f, "body is {actual} bytes, need at least {minimum}")
            }
            Self::UnsupportedLayout(layout) => write!(f, "layout {layout:?} is not implemented"),
            Self::InvalidHostDevice(value) => write!(f, "invalid host device {value}"),
            Self::InvalidTwsFlag(value) => write!(f, "invalid TWS flag {value}"),
            Self::InvalidSoundMode(value) => write!(f, "invalid sound mode {value}"),
            Self::BatteryAboveScale { value, maximum } => {
                write!(f, "battery {value} is above scale {maximum}")
            }
        }
    }
}

impl std::error::Error for StateError {}

/// Parse a complete inbound `01 01` frame into a device state.
pub fn parse_state_frame(model: DeviceModel, raw: &[u8]) -> Result<DeviceState, StateError> {
    let frame: Frame = parse(raw).map_err(|_| StateError::NotAStateFrame)?;
    if frame.command != Command::STATE {
        return Err(StateError::NotAStateFrame);
    }
    parse_state_body(model, &frame.body)
}

pub fn parse_state_body(model: DeviceModel, body: &[u8]) -> Result<DeviceState, StateError> {
    match model.layout {
        Layout::TwsCmm2 => parse_tws_cmm2_body(model, body),
        // Only A3062's single-battery frame has been measured on this branch.
        // A3035/A3040 share the layout enum but keep failing as unsupported
        // until their own frames are captured.
        Layout::SingleCmm2 if model.code == A3062.code => parse_single_cmm2_body(model, body),
        layout => Err(StateError::UnsupportedLayout(layout)),
    }
}

/// The measured single-battery body prefix: battery, filler, firmware, serial.
const MIN_SINGLE_CMM2_BODY: usize = 23;

fn parse_tws_cmm2_body(model: DeviceModel, body: &[u8]) -> Result<DeviceState, StateError> {
    // Prefix, batteries, charging flags, two firmware strings and the serial.
    const MIN_TWS_BODY: usize = 33;
    if body.len() < MIN_TWS_BODY {
        return Err(StateError::ShortBody {
            actual: body.len(),
            minimum: MIN_TWS_BODY,
        });
    }

    let host_device = match body[0] {
        0 => HostDevice::Left,
        1 => HostDevice::Right,
        other => return Err(StateError::InvalidHostDevice(other)),
    };
    let tws_connected = match body[1] {
        0 => false,
        1 => true,
        other => return Err(StateError::InvalidTwsFlag(other)),
    };
    let scale = model.battery_scale;
    let maximum = scale.maximum();
    for value in [body[2], body[3]] {
        if value > maximum {
            return Err(StateError::BatteryAboveScale { value, maximum });
        }
    }

    Ok(DeviceState {
        host_device,
        tws_connected,
        battery: Some(BatteryState {
            left: body[2],
            right: body[3],
            left_charging: body[4] != 0,
            right_charging: body[5] != 0,
            scale,
        }),
        single_battery: None,
        sound_mode: parse_sound_mode(model, body)?,
        // The A3949's equalizer block (body 32..44) is known from its committed
        // capture, but this slice deliberately defers A3949 equalizer support
        // (no verified `03 87` write body), so the model has no capability and
        // this returns `None` rather than inventing offsets.
        equalizer: parse_equalizer(model, body)?,
        firmware: FirmwareVersions {
            left: parse_firmware(&body[6..11]),
            right: parse_firmware(&body[11..16]),
        },
        serial: parse_serial(&body[16..32]),
    })
}

/// Parses the single-battery Cmm2 body measured on Space One Pro (A3062):
///
/// | offset | field |
/// |---:|---|
/// | 0 | battery step, [`BatteryScale::Ten`] |
/// | 1 | constant `0xFF`; not a host or charging field |
/// | 2..7 | firmware, ASCII `MM.mm` |
/// | 7..23 | serial, 16 ASCII characters |
/// | 23..25 | equalizer preset id, little-endian, [`EqualizerState`] |
/// | 25..35 | ten equalizer bands, byte `value + 120` |
/// | 35..37 | two unknown bytes the equalizer write does not echo |
/// | 37..65 | HearID block, kept verbatim for the equalizer write |
/// | 69 | sound mode, [`SoundMode`] |
///
/// Everything after offset 23 except the measured equalizer block and mode byte
/// is model-specific and is not interpreted here. The measured body is 95
/// bytes.
fn parse_single_cmm2_body(model: DeviceModel, body: &[u8]) -> Result<DeviceState, StateError> {
    if body.len() < MIN_SINGLE_CMM2_BODY {
        return Err(StateError::ShortBody {
            actual: body.len(),
            minimum: MIN_SINGLE_CMM2_BODY,
        });
    }

    let value = body[0];
    let scale = model.battery_scale;
    let maximum = scale.maximum();
    if value > maximum {
        return Err(StateError::BatteryAboveScale { value, maximum });
    }

    Ok(DeviceState {
        // Single-battery layouts have no host earbud and no TWS link.
        host_device: HostDevice::Left,
        tws_connected: false,
        battery: None,
        single_battery: Some(SingleBatteryState {
            value,
            scale,
            // `body[1]` is a constant 0xFF in the measured frames, not a
            // charging flag; charging arrives in a separate `01 04` frame.
            charging: None,
            firmware: parse_firmware(&body[2..7]),
        }),
        // The equalizer block starts at body offset 23, before the mode byte;
        // parsing it first also reports a body truncated inside the block with
        // the block's own minimum length.
        equalizer: parse_equalizer(model, body)?,
        sound_mode: parse_sound_mode(model, body)?,
        firmware: FirmwareVersions {
            left: None,
            right: None,
        },
        serial: parse_serial(&body[7..23]),
    })
}

/// Parses the equalizer block of a model whose equalizer control this slice
/// implements.
///
/// `None` when the model has no equalizer capability, including models whose
/// equalizer is deliberately deferred (the A3949). A body too short to hold the
/// whole measured block is an error: the parser does not invent band values.
fn parse_equalizer(model: DeviceModel, body: &[u8]) -> Result<Option<EqualizerState>, StateError> {
    let Some(capability) = model.equalizer else {
        return Ok(None);
    };
    if capability.band_count != EQ_BANDS {
        // Only the measured ten-band shape is implemented.
        return Ok(None);
    }
    let preset_offset = capability.state_offset;
    let bands_offset = preset_offset + 2;
    let hear_id_offset = bands_offset + capability.band_count + 2;
    let minimum = hear_id_offset + HEAR_ID_BLOCK_LEN;
    if body.len() < minimum {
        return Err(StateError::ShortBody {
            actual: body.len(),
            minimum,
        });
    }

    let preset_id = u16::from_le_bytes([body[preset_offset], body[preset_offset + 1]]);
    let bands =
        std::array::from_fn(|index| i16::from(body[bands_offset + index]) - BAND_BYTE_OFFSET);
    let mut hear_id = [0u8; HEAR_ID_BLOCK_LEN];
    hear_id.copy_from_slice(&body[hear_id_offset..minimum]);
    Ok(Some(EqualizerState {
        preset_id,
        bands,
        hear_id,
    }))
}

/// Parses the mode byte of a model whose sound-mode control was measured.
///
/// `None` when the model has no measured control. A body too short to hold the
/// measured byte, or a value outside the measured mapping, is an error: the
/// parser does not invent a default mode.
fn parse_sound_mode(model: DeviceModel, body: &[u8]) -> Result<Option<SoundMode>, StateError> {
    let Some(capability) = model.sound_mode else {
        return Ok(None);
    };
    let offset = capability.state_offset;
    let value = body.get(offset).copied().ok_or(StateError::ShortBody {
        actual: body.len(),
        minimum: offset + 1,
    })?;
    let mode = SoundMode::from_value(value).ok_or(StateError::InvalidSoundMode(value))?;
    Ok(Some(mode))
}

fn parse_firmware(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let (major, minor) = text.split_once('.')?;
    if major.len() != 2 || minor.len() != 2 {
        return None;
    }
    if major.bytes().all(|byte| byte.is_ascii_digit())
        && minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        Some(text.to_owned())
    } else {
        None
    }
}

fn parse_serial(bytes: &[u8]) -> Option<String> {
    if bytes.iter().all(|byte| *byte == 0) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if text.len() == 16 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        // Masked captures use sixteen ASCII zeroes. Treat them as absent so a
        // redacted capture never looks like a real serial.
        if text.bytes().all(|byte| byte == b'0') {
            None
        } else {
            Some(text.to_owned())
        }
    } else {
        None
    }
}
