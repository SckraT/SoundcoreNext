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

use soundcore_protocol::{Command, Direction, Frame, encode, parse};

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

/// Number of A3062 equalizer bands the vendored Equalizer page shows: the ten
/// measured bands minus the two the Soundcore app never exposes.
pub const VISIBLE_EQ_BANDS: usize = 8;

/// The A3062 band conversion offset: a `01 01` band byte is `value + 120`.
const BAND_BYTE_OFFSET: i16 = 120;

/// The measured A3062 band-value range in raw dB×10: `-12.0..=+13.4 dB`, the
/// full `0..=254` band-byte range shifted by the measured 120 offset.
///
/// The `03 87` body builder clamps to this range, and the desktop Equalizer page
/// shows and clamps to these same two values, so the page cannot offer a value
/// the device encodes differently.
pub const EQ_BAND_MIN: i16 = -BAND_BYTE_OFFSET;
/// See [`EQ_BAND_MIN`].
pub const EQ_BAND_MAX: i16 = 254 - BAND_BYTE_OFFSET;

/// The two A3062 bands the Soundcore app fixes and never shows, in the same raw
/// dB×10 units as every other band: `0` (0 dB) and `-120` (-12.0 dB).
///
/// Every measured built-in preset and OpenSCL4's A3062 equalizer fix them at
/// these values, so a custom write uses them too; see
/// [`DeviceModel::custom_equalizer_body`].
pub const FIXED_EQ_BANDS: [i16; EQ_BANDS - VISIBLE_EQ_BANDS] = [0, -120];

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
/// deliberately not a preset here: they are not a chip, because the Equalizer
/// page edits and writes them as a custom curve
/// ([`DeviceModel::custom_equalizer_body`]).
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

    /// The eight bands the Equalizer page shows, in raw dB×10 units.
    ///
    /// The last two measured bands are fixed by every built-in preset and by
    /// custom writes; see [`FIXED_EQ_BANDS`].
    pub fn visible_bands(&self) -> [i16; VISIBLE_EQ_BANDS] {
        let mut visible = [0i16; VISIBLE_EQ_BANDS];
        visible.copy_from_slice(&self.bands[..VISIBLE_EQ_BANDS]);
        visible
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

/// Bytes of the A3062 HearID block inside the state, from the end of the two
/// unknown bytes after the bands to the end of the favorite music genre.
const HEAR_ID_BLOCK_LEN: usize = 28;

/// A measured A3062 double-press action.
///
/// OpenSCL4 registers one action for this model: `7`, BassUp. The A3062 `01 01`
/// state and the `04 81` command use [`NO_BUTTON_ACTION`] when the double press
/// is disabled, which is not an action variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    BassUp,
}

/// The double-press body byte that means "no action" (`0x0F` in OpenSCL4's
/// A3062 `set_button_double_press_action`).
pub const NO_BUTTON_ACTION: u8 = 0x0F;

impl ButtonAction {
    /// The action's `04 81` body byte.
    pub const fn value(self) -> u8 {
        match self {
            Self::BassUp => 7,
        }
    }

    /// The action a byte names, or `None` for any other byte, including
    /// [`NO_BUTTON_ACTION`].
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            7 => Some(Self::BassUp),
            _ => None,
        }
    }
}

/// The ambient-sound modes the button cycles through: state body byte 68 and
/// the one-byte `06 82` command body.
///
/// Bit 0 is noise canceling, bit 1 transparency and bit 2 normal, as in
/// OpenSCL4's A3062 parser. Bits 3..7 are not part of the cycle: they are
/// ignored on read and written as zero, exactly as OpenSCL4 parses and
/// serializes the byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmbientSoundModeCycle {
    pub noise_canceling: bool,
    pub transparency: bool,
    pub normal: bool,
}

impl AmbientSoundModeCycle {
    const NOISE_CANCELING_BIT: u8 = 1 << 0;
    const TRANSPARENCY_BIT: u8 = 1 << 1;
    const NORMAL_BIT: u8 = 1 << 2;

    /// The three cycle bits of a state byte; bits 3..7 are ignored.
    pub const fn from_byte(byte: u8) -> Self {
        Self {
            noise_canceling: byte & Self::NOISE_CANCELING_BIT != 0,
            transparency: byte & Self::TRANSPARENCY_BIT != 0,
            normal: byte & Self::NORMAL_BIT != 0,
        }
    }

    /// The `06 82` body byte for this cycle.
    pub const fn byte(self) -> u8 {
        let mut byte = 0;
        if self.noise_canceling {
            byte |= Self::NOISE_CANCELING_BIT;
        }
        if self.transparency {
            byte |= Self::TRANSPARENCY_BIT;
        }
        if self.normal {
            byte |= Self::NORMAL_BIT;
        }
        byte
    }
}

/// One of the four measured A3062 auto-power-off durations.
///
/// The state byte and the `01 86` body carry an index `0..=3`; OpenSCL4
/// registers the A3062 durations in half-hour increments, so the index names
/// 30, 60, 90 or 120 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPowerOffDuration {
    ThirtyMinutes,
    SixtyMinutes,
    NinetyMinutes,
    OneHundredTwentyMinutes,
}

impl AutoPowerOffDuration {
    /// Every duration, in index order.
    pub const ALL: [Self; 4] = [
        Self::ThirtyMinutes,
        Self::SixtyMinutes,
        Self::NinetyMinutes,
        Self::OneHundredTwentyMinutes,
    ];

    /// The duration's state and `01 86` body index, `0..=3`.
    pub const fn index(self) -> u8 {
        match self {
            Self::ThirtyMinutes => 0,
            Self::SixtyMinutes => 1,
            Self::NinetyMinutes => 2,
            Self::OneHundredTwentyMinutes => 3,
        }
    }

    /// The duration an index names, or `None` for an index outside `0..=3`.
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::ThirtyMinutes),
            1 => Some(Self::SixtyMinutes),
            2 => Some(Self::NinetyMinutes),
            3 => Some(Self::OneHundredTwentyMinutes),
            _ => None,
        }
    }

    /// The duration in minutes.
    pub const fn minutes(self) -> u16 {
        match self {
            Self::ThirtyMinutes => 30,
            Self::SixtyMinutes => 60,
            Self::NinetyMinutes => 90,
            Self::OneHundredTwentyMinutes => 120,
        }
    }
}

/// The A3062 auto-power-off state and `01 86` body: whether it is enabled and
/// the duration to use when it is.
///
/// The duration is kept while the feature is disabled, so switching it back on
/// restores the last duration; the measured capture reports enabled and
/// 60 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPowerOff {
    pub enabled: bool,
    pub duration: AutoPowerOffDuration,
}

/// A validated A3062 limit-high-volume dB limit: `75..=100` in `5` dB steps.
///
/// The state byte and the second `20 82` body byte are the dB value itself; a
/// value outside the measured range or between steps cannot be represented,
/// because this type is the only way to build the write body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DbLimit(u8);

impl DbLimit {
    /// The lowest measured limit, 75 dB.
    pub const MIN: u8 = 75;
    /// The highest measured limit, 100 dB.
    pub const MAX: u8 = 100;
    /// The measured step between limits, 5 dB.
    pub const STEP: u8 = 5;

    /// The limit a value names, or `None` when it is outside
    /// [`Self::MIN`]..=[`Self::MAX`] or not a multiple of [`Self::STEP`].
    pub const fn new(value: u8) -> Option<Self> {
        if value < Self::MIN || value > Self::MAX {
            return None;
        }
        if !value.is_multiple_of(Self::STEP) {
            return None;
        }
        Some(Self(value))
    }

    /// The dB value, in the range [`Self::MIN`]..=[`Self::MAX`].
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// How often the A3062 re-reads the ambient volume while limiting high volume:
/// state body byte 84 and the one-byte `20 81` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecibelReadingRefreshRate {
    RealTime,
    TenSeconds,
    OneMinute,
}

impl DecibelReadingRefreshRate {
    /// Every rate, in value order.
    pub const ALL: [Self; 3] = [Self::RealTime, Self::TenSeconds, Self::OneMinute];

    /// The state and `20 81` body value, `0`, `1` or `2`.
    pub const fn value(self) -> u8 {
        match self {
            Self::RealTime => 0,
            Self::TenSeconds => 1,
            Self::OneMinute => 2,
        }
    }

    /// The rate a value names, or `None` for a value outside `0..=2`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::RealTime),
            1 => Some(Self::TenSeconds),
            2 => Some(Self::OneMinute),
            _ => None,
        }
    }
}

/// The A3062 limit-high-volume state: enabled, dB limit and refresh rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitHighVolume {
    pub enabled: bool,
    pub db_limit: DbLimit,
    pub refresh_rate: DecibelReadingRefreshRate,
}

/// The remaining settings of the A3062, parsed from `01 01` body 67..87.
///
/// | body offsets | field |
/// |---:|---|
/// | 67 | button double press: `7` BassUp, [`NO_BUTTON_ACTION`] none |
/// | 68 | ambient sound mode cycle bits |
/// | 76 | low battery prompt |
/// | 77 | Dolby audio |
/// | 78 | LDAC |
/// | 79 | dual connections |
/// | 80..82 | auto power off: enabled, duration index |
/// | 82..85 | limit high volume: enabled, dB, refresh rate |
/// | 85 | side tone |
/// | 86 | voice prompt |
///
/// Every byte here is a flag, an enum value or a validated range; the parser
/// rejects a closed enum's unknown value rather than inventing a default. The
/// measured values are the same in every committed A3062 capture.
///
/// Writes are [`SettingCommand`]s, because most of these settings have their own
/// command and only the enable flag and the value travel together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3062Settings {
    /// The double-press action; `None` when the button does nothing.
    pub button_double_press: Option<ButtonAction>,
    /// The modes the button cycles through.
    pub ambient_sound_mode_cycle: AmbientSoundModeCycle,
    /// The low-battery voice prompt.
    pub low_battery_prompt: bool,
    /// Dolby audio processing.
    pub dolby_audio: bool,
    /// LDAC, available only in the `01 01` state; a write is confirmed by a
    /// separate `01 7F` notification.
    pub ldac: bool,
    /// Whether a second device may be connected at the same time.
    pub dual_connections: bool,
    /// Auto power off, enabled and duration.
    pub auto_power_off: AutoPowerOff,
    /// Limit high volume, enabled, dB limit and refresh rate.
    pub limit_high_volume: LimitHighVolume,
    /// Side tone.
    pub side_tone: bool,
    /// The voice prompt (A3062 uses the `01 AE` command for it).
    pub voice_prompt: bool,
}

/// The measured settings block of a model whose remaining settings this slice
/// implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsCapability {
    /// Body offset of the first settings byte in the `01 01` state response
    /// (the button double press; body 67 on the A3062).
    pub state_offset: usize,
}

/// One settings write: a validated value plus the exact command and body that
/// carry it to an A3062.
///
/// Every variant is built from validated parts ([`DbLimit`],
/// [`AutoPowerOffDuration`], [`DecibelReadingRefreshRate`], [`ButtonAction`]),
/// so an out-of-range value cannot be represented. Turn one into a complete
/// outbound frame with [`DeviceModel::setting_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingCommand {
    /// `04 81` body `00 00 action`; `None` writes [`NO_BUTTON_ACTION`].
    ButtonDoublePress(Option<ButtonAction>),
    /// `06 82` body `[bits]`.
    AmbientSoundModeCycle(AmbientSoundModeCycle),
    /// `10 82` body `[on]`.
    LowBatteryPrompt(bool),
    /// `02 86` body `[on]`.
    DolbyAudio(bool),
    /// `01 FF` body `[on]`; the device confirms with an `01 7F` notification.
    Ldac(bool),
    /// `0B 84` body `[on]`.
    DualConnections(bool),
    /// `01 86` body `[enabled, index]`.
    AutoPowerOff(AutoPowerOff),
    /// `20 82` body `[enabled, dB]`.
    LimitHighVolume { enabled: bool, db_limit: DbLimit },
    /// `20 81` body `[0/1/2]`.
    LimitHighVolumeRefreshRate(DecibelReadingRefreshRate),
    /// `01 84` body `[on]`.
    SideTone(bool),
    /// `01 AE` body `[on]`; the A3062-specific voice prompt command.
    VoicePrompt(bool),
}

impl SettingCommand {
    /// The measured command that carries this setting.
    pub const fn command(self) -> Command {
        match self {
            Self::ButtonDoublePress(_) => Command::new(0x04, 0x81),
            Self::AmbientSoundModeCycle(_) => Command::new(0x06, 0x82),
            Self::LowBatteryPrompt(_) => Command::new(0x10, 0x82),
            Self::DolbyAudio(_) => Command::new(0x02, 0x86),
            Self::Ldac(_) => Command::new(0x01, 0xFF),
            Self::DualConnections(_) => Command::new(0x0B, 0x84),
            Self::AutoPowerOff(_) => Command::new(0x01, 0x86),
            Self::LimitHighVolume { .. } => Command::new(0x20, 0x82),
            Self::LimitHighVolumeRefreshRate(_) => Command::new(0x20, 0x81),
            Self::SideTone(_) => Command::new(0x01, 0x84),
            Self::VoicePrompt(_) => Command::new(0x01, 0xAE),
        }
    }

    /// The exact outbound body for this setting.
    pub fn body(self) -> Vec<u8> {
        match self {
            Self::ButtonDoublePress(action) => {
                vec![0, 0, action.map_or(NO_BUTTON_ACTION, ButtonAction::value)]
            }
            Self::AmbientSoundModeCycle(cycle) => vec![cycle.byte()],
            Self::LowBatteryPrompt(on)
            | Self::DolbyAudio(on)
            | Self::Ldac(on)
            | Self::DualConnections(on)
            | Self::SideTone(on)
            | Self::VoicePrompt(on) => vec![on.into()],
            Self::AutoPowerOff(auto_power_off) => vec![
                auto_power_off.enabled.into(),
                auto_power_off.duration.index(),
            ],
            Self::LimitHighVolume { enabled, db_limit } => vec![enabled.into(), db_limit.value()],
            Self::LimitHighVolumeRefreshRate(refresh_rate) => vec![refresh_rate.value()],
        }
    }
}

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
/// the charging flag, a single firmware string (`MM.mm` ASCII) and the serial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleBatteryState {
    /// Raw battery step, at most [`BatteryScale::maximum`].
    pub value: u8,
    /// The scale of [`Self::value`]; A3062 uses [`BatteryScale::Ten`], where a
    /// raw `5` is 50 %.
    pub scale: BatteryScale,
    /// The charging flag, when the state frame carries one.
    ///
    /// Only the measured A3062 `01 01` body carries it, at byte 1: `1` means
    /// charging and any other value means not charging, which is how OpenSCL4's
    /// A3062 `SingleBattery` parser reads it. Every committed capture carries
    /// `0xFF` there, so they report not charging. Charging changes also arrive
    /// as a separate `01 04` notification, which this state poll does not read.
    ///
    /// The other single-battery models (A3035/A3040) share the layout enum but
    /// their frames were never captured, so the parser leaves this `None` for
    /// them instead of reading an unmeasured byte as a flag.
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
    /// The measured remaining settings; `None` for models whose settings this
    /// slice does not implement (the A3949 and every other model so far).
    pub settings: Option<A3062Settings>,
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
    /// The measured remaining-settings block, or `None` when this slice does
    /// not implement the model's settings (the whole catalog except A3062).
    pub settings: Option<SettingsCapability>,
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

    /// The exact outbound frame for one settings write, or `None` when this
    /// model has no measured settings block (for example the A3949).
    ///
    /// The frame is built in one step from [`SettingCommand::command`] and
    /// [`SettingCommand::body`]; a caller writes it once and re-polls `01 01`,
    /// because the app does not wait for the empty same-command acknowledgement.
    pub fn setting_frame(self, setting: SettingCommand) -> Option<Vec<u8>> {
        self.settings?;
        Some(encode(
            Direction::Outbound,
            setting.command(),
            &setting.body(),
        ))
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
    /// | 4..14 | the curve's ten band bytes |
    /// | 14..16 | unknown, zero |
    /// | 16 | HearID enabled: always `0` for an equalizer write |
    /// | 17..27 | HearID volume adjustments, from the state |
    /// | 27..31 | HearID time, big-endian, from the state |
    /// | 31 | HearID type, from the state |
    /// | 32..42 | HearID custom volume adjustments, from the state |
    /// | 42..52 | the curve's ten band bytes again (active curve) |
    /// | 52 | unknown, zero |
    ///
    /// The HearID fields are preserved, and its enabled flag is cleared, so the
    /// selected curve is in effect; this is what OpenSCL4's A3062 equalizer
    /// does.
    pub fn equalizer_preset_body(
        self,
        current: &EqualizerState,
        preset: &EqualizerPreset,
    ) -> Option<[u8; EQUALIZER_COMMAND_BODY_LEN]> {
        self.equalizer_body(current, preset.id, preset.bands)
    }

    /// The full A3062 `03 87` body that applies a custom curve while keeping the
    /// device's current HearID block, or `None` when this slice does not
    /// implement the model's equalizer (for example the deferred A3949).
    ///
    /// The body has the same 53-byte shape as [`Self::equalizer_preset_body`],
    /// with [`CUSTOM_EQUALIZER_PRESET_ID`] as the preset id and a ten-band curve
    /// made of `visible_bands` followed by [`FIXED_EQ_BANDS`].
    ///
    /// `visible_bands` are the eight bands the Equalizer page shows, in the same
    /// raw dB×10 units as [`EqualizerState::bands`]: `0` is `0 dB`, `-120` is
    /// `-12.0 dB` and `134` is `+13.4 dB`. Values outside the measured
    /// `-120..=134` range are clamped to it; the packet byte written is
    /// `value + 120`. The two invisible bands are fixed as measured, so a
    /// caller cannot select them.
    pub fn custom_equalizer_body(
        self,
        current: &EqualizerState,
        visible_bands: [i16; VISIBLE_EQ_BANDS],
    ) -> Option<[u8; EQUALIZER_COMMAND_BODY_LEN]> {
        let mut bands = [0i16; EQ_BANDS];
        bands[..VISIBLE_EQ_BANDS].copy_from_slice(&visible_bands);
        bands[VISIBLE_EQ_BANDS..].copy_from_slice(&FIXED_EQ_BANDS);
        self.equalizer_body(current, CUSTOM_EQUALIZER_PRESET_ID, bands)
    }

    /// Builds the measured 53-byte `03 87` body for an arbitrary preset id and
    /// ten raw band values.
    fn equalizer_body(
        self,
        current: &EqualizerState,
        preset_id: u16,
        bands: [i16; EQ_BANDS],
    ) -> Option<[u8; EQUALIZER_COMMAND_BODY_LEN]> {
        let capability = self.equalizer?;
        if capability.band_count != EQ_BANDS {
            return None;
        }
        let mut body = [0u8; EQUALIZER_COMMAND_BODY_LEN];
        body[0..2].copy_from_slice(&preset_id.to_le_bytes());
        body[2..4].copy_from_slice(&current.hear_id[26..28]);
        let band_bytes = bands.map(band_byte);
        body[4..14].copy_from_slice(&band_bytes);
        // 14..16 are unknown zeroes.
        body[16] = 0;
        body[17..27].copy_from_slice(&current.hear_id[1..11]);
        body[27..31].copy_from_slice(&current.hear_id[11..15]);
        body[31] = current.hear_id[15];
        body[32..42].copy_from_slice(&current.hear_id[16..26]);
        body[42..52].copy_from_slice(&band_bytes);
        // 52 is an unknown zero.
        Some(body)
    }
}

/// The `01 01` byte of a raw band value in dB×10, clamped to the measured range.
fn band_byte(value: i16) -> u8 {
    let clamped = value.clamp(EQ_BAND_MIN, EQ_BAND_MAX);
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
    settings: None,
};

pub const A3947: DeviceModel = DeviceModel {
    code: "A3947",
    name: "Soundcore Liberty 4 NC",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
};

pub const A3954: DeviceModel = DeviceModel {
    code: "A3954",
    name: "Soundcore Liberty 4 Pro",
    battery_scale: BatteryScale::Hundred,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
};

pub const A3035: DeviceModel = DeviceModel {
    code: "A3035",
    name: "Soundcore Space One",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
};

pub const A3040: DeviceModel = DeviceModel {
    code: "A3040",
    name: "Soundcore Space Q45",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
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
    // Measured on the real device on 2026-11-21 and copied from OpenSCL4's
    // A3062 state parser: the settings block starts at body byte 67 (the
    // button double press) and runs through byte 86 (the voice prompt).
    settings: Some(SettingsCapability { state_offset: 67 }),
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
    InvalidButtonAction(u8),
    InvalidAutoPowerOffIndex(u8),
    InvalidLimitHighVolumeDbLimit(u8),
    InvalidDecibelReadingRefreshRate(u8),
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
            Self::InvalidButtonAction(value) => {
                write!(f, "invalid button double-press action {value}")
            }
            Self::InvalidAutoPowerOffIndex(value) => {
                write!(f, "invalid auto power off duration index {value}")
            }
            Self::InvalidLimitHighVolumeDbLimit(value) => {
                write!(f, "invalid limit high volume dB limit {value}")
            }
            Self::InvalidDecibelReadingRefreshRate(value) => {
                write!(f, "invalid decibel reading refresh rate {value}")
            }
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
        settings: parse_settings(model, body)?,
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
/// | 1 | charging on the measured A3062: `1` charging, every other value (the captures carry `0xFF`) not charging; the A3035/A3040 bytes were not measured, so they stay `None` |
/// | 2..7 | firmware, ASCII `MM.mm` |
/// | 7..23 | serial, 16 ASCII characters |
/// | 23..25 | equalizer preset id, little-endian, [`EqualizerState`] |
/// | 25..35 | ten equalizer bands, byte `value + 120` |
/// | 35..37 | two unknown bytes the equalizer write does not echo |
/// | 37..65 | HearID block, kept verbatim for the equalizer write |
/// | 67 | button double press, [`A3062Settings`] |
/// | 68 | ambient sound mode cycle |
/// | 69..75 | sound modes block; only the mode byte at 69 is interpreted |
/// | 75 | one unknown sound-mode byte the app never sends |
/// | 76 | low battery prompt |
/// | 77 | Dolby audio |
/// | 78 | LDAC |
/// | 79 | dual connections |
/// | 80..82 | auto power off: enabled, duration index |
/// | 82..85 | limit high volume: enabled, dB, refresh rate |
/// | 85 | side tone |
/// | 86 | voice prompt |
///
/// Everything after offset 23 except the measured equalizer, sound-mode and
/// settings blocks is model-specific and is not interpreted here. The measured
/// body is 95 bytes; the settings block ends at byte 86, so the eight trailing
/// bytes are not parsed.
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
            // Body 1 is the A3062's charging flag: `1` is charging, anything
            // else (the measured captures carry `0xFF`) is not. Only the A3062
            // byte was measured; the A3035/A3040 frames were never captured, so
            // they keep the earlier `None` instead of reading an unmeasured byte
            // as a flag. `parse_state_body` also rejects those models before
            // they reach this parser.
            charging: (model.code == A3062.code).then(|| body[1] == 1),
            firmware: parse_firmware(&body[2..7]),
        }),
        // The equalizer block starts at body offset 23, before the mode byte;
        // parsing it first also reports a body truncated inside the block with
        // the block's own minimum length.
        equalizer: parse_equalizer(model, body)?,
        sound_mode: parse_sound_mode(model, body)?,
        settings: parse_settings(model, body)?,
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

/// How many bytes an A3062 settings block spans: the double-press byte at body
/// 67 through the voice-prompt byte at body 86.
const SETTINGS_BLOCK_LEN: usize = 20;

// Offsets inside the A3062 settings block (body 67..87), measured from
// OpenSCL4's A3062 `A3062StateUpdatePacket` parser.
const BUTTON_DOUBLE_PRESS_OFFSET: usize = 0;
const AMBIENT_SOUND_MODE_CYCLE_OFFSET: usize = 1;
const LOW_BATTERY_PROMPT_OFFSET: usize = 9;
const DOLBY_AUDIO_OFFSET: usize = 10;
const LDAC_OFFSET: usize = 11;
const DUAL_CONNECTIONS_OFFSET: usize = 12;
const AUTO_POWER_OFF_ENABLED_OFFSET: usize = 13;
const AUTO_POWER_OFF_DURATION_OFFSET: usize = 14;
const LIMIT_HIGH_VOLUME_ENABLED_OFFSET: usize = 15;
const LIMIT_HIGH_VOLUME_DB_OFFSET: usize = 16;
const LIMIT_HIGH_VOLUME_REFRESH_OFFSET: usize = 17;
const SIDE_TONE_OFFSET: usize = 18;
const VOICE_PROMPT_OFFSET: usize = 19;

/// Parses the remaining-settings block of a model whose settings this slice
/// implements.
///
/// `None` when the model has no settings capability (the A3949 and every other
/// model so far). A body too short to hold the whole measured block is an
/// error, and so is an unknown value of a closed enum (button action, auto
/// power off index, dB limit, refresh rate): the parser does not invent a
/// default. Flags are on only when the byte is `1`, which is how OpenSCL4 reads
/// them, so an unexpected non-zero byte reads as off instead of breaking the
/// whole state poll.
fn parse_settings(model: DeviceModel, body: &[u8]) -> Result<Option<A3062Settings>, StateError> {
    let Some(capability) = model.settings else {
        return Ok(None);
    };
    let start = capability.state_offset;
    let Some(block) = body.get(start..start + SETTINGS_BLOCK_LEN) else {
        return Err(StateError::ShortBody {
            actual: body.len(),
            minimum: start + SETTINGS_BLOCK_LEN,
        });
    };

    let button_double_press = match block[BUTTON_DOUBLE_PRESS_OFFSET] {
        NO_BUTTON_ACTION => None,
        value => {
            Some(ButtonAction::from_value(value).ok_or(StateError::InvalidButtonAction(value))?)
        }
    };
    let auto_power_off_duration =
        AutoPowerOffDuration::from_index(block[AUTO_POWER_OFF_DURATION_OFFSET]).ok_or(
            StateError::InvalidAutoPowerOffIndex(block[AUTO_POWER_OFF_DURATION_OFFSET]),
        )?;
    let db_limit = DbLimit::new(block[LIMIT_HIGH_VOLUME_DB_OFFSET]).ok_or(
        StateError::InvalidLimitHighVolumeDbLimit(block[LIMIT_HIGH_VOLUME_DB_OFFSET]),
    )?;
    let refresh_rate =
        DecibelReadingRefreshRate::from_value(block[LIMIT_HIGH_VOLUME_REFRESH_OFFSET]).ok_or(
            StateError::InvalidDecibelReadingRefreshRate(block[LIMIT_HIGH_VOLUME_REFRESH_OFFSET]),
        )?;

    Ok(Some(A3062Settings {
        button_double_press,
        ambient_sound_mode_cycle: AmbientSoundModeCycle::from_byte(
            block[AMBIENT_SOUND_MODE_CYCLE_OFFSET],
        ),
        low_battery_prompt: block[LOW_BATTERY_PROMPT_OFFSET] == 1,
        dolby_audio: block[DOLBY_AUDIO_OFFSET] == 1,
        ldac: block[LDAC_OFFSET] == 1,
        dual_connections: block[DUAL_CONNECTIONS_OFFSET] == 1,
        auto_power_off: AutoPowerOff {
            enabled: block[AUTO_POWER_OFF_ENABLED_OFFSET] == 1,
            duration: auto_power_off_duration,
        },
        limit_high_volume: LimitHighVolume {
            enabled: block[LIMIT_HIGH_VOLUME_ENABLED_OFFSET] == 1,
            db_limit,
            refresh_rate,
        },
        side_tone: block[SIDE_TONE_OFFSET] == 1,
        voice_prompt: block[VOICE_PROMPT_OFFSET] == 1,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the A3062's `01 01` body has a measured charging byte. A3035/A3040
    /// share the `SingleCmm2` layout enum but were never captured, so the parser
    /// must leave their `charging` as the earlier `None` instead of reading an
    /// unmeasured byte as a flag.
    #[test]
    fn only_the_a3062_reads_the_single_battery_charging_byte() {
        // The body is long enough for the A3062's equalizer and settings blocks;
        // A3035/A3040 stop reading after the 23-byte prefix.
        let mut body = vec![0u8; 87];
        body[0] = 5;
        body[1] = 1;
        body[2..7].copy_from_slice(b"04.39");
        body[7..23].copy_from_slice(b"0123456789ABCDEF");
        // The measured A3062 settings block needs a valid double-press byte and
        // dB limit even though this test only looks at the battery prefix.
        body[67] = NO_BUTTON_ACTION;
        body[83] = DbLimit::MIN;

        for code in [A3035.code, A3040.code] {
            let model = model_by_code(code).expect("the catalog lists the model");
            assert_eq!(
                parse_single_cmm2_body(model, &body)
                    .expect("the measured prefix parses")
                    .single_battery
                    .expect("single battery")
                    .charging,
                None,
                "{code} has no measured charging byte"
            );
        }

        let a3062 = model_by_code(A3062.code).expect("A3062");
        assert_eq!(
            parse_single_cmm2_body(a3062, &body)
                .expect("the measured frame parses")
                .single_battery
                .expect("single battery")
                .charging,
            Some(true)
        );
    }
}
