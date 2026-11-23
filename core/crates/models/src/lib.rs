//! Device models and their state parsers.
//!
//! Two state layouts are implemented from real captures:
//!
//! - the TWS Cmm2 layout, confirmed by the real A3949 capture (left/right
//!   batteries, two firmware strings, serial);
//! - the single-battery Cmm2 layout, measured on the real A3062 (one battery on
//!   the ten-step scale, one firmware string, serial, sound mode and equalizer).
//!
//! The Liberty 4 Pro (A3954) shares the TWS Cmm2 prefix and appends its own
//! measured 165-byte block: the case firmware, battery and serial, the
//! ten-band/two-channel equalizer with its HearID, the 1..=11 sound-mode scale,
//! the twelve button action bytes, the case features/language and the
//! remaining settings. Its state is additive ([`DeviceState::a3954`]) and its
//! write bodies are the 114-byte `03 87`, five-byte `06 81`, three-byte
//! `04 81`, four-byte `07 87` and one-byte `07 8A` frames of their own.
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

// ===========================================================================
// A3954 (Liberty 4 Pro)
// ===========================================================================
//
// Everything below was measured from the real Liberty 4 Pro and from the
// Soundcore app's own frames; see `.scratch/a3954-settings/spec.md` for the
// evidence. The base TWS fields (body 0..32) are shared with the A3949 and are
// parsed by `parse_tws_cmm2_body`; the A3954 block starts at the case firmware
// at body byte 32.

/// The measured length of the A3954 `01 01` response body: 165 bytes.
///
/// The A3954 block runs through the last slide-button byte at body 164, so a
/// shorter body is rejected instead of parsed with invented offsets.
pub const A3954_STATE_BODY_LEN: usize = 165;

/// Equalizer bands in each of the A3954's two channels.
pub const A3954_EQ_BANDS: usize = 10;

/// The measured length of the A3954 `03 87` equalizer body: 114 bytes.
///
/// It is larger than the A3062's 53-byte body because it carries both channels
/// of the curve, the whole 48-byte HearID block and an interleaved copy of the
/// active curve; see [`DeviceModel::a3954_equalizer_preset_body`].
pub const A3954_EQUALIZER_COMMAND_BODY_LEN: usize = 114;

/// The measured length of the A3954 `06 81` sound-mode body: five bytes, the
/// mode, the 1..=11 slider position, the airplane mode, the wind flag and a
/// trailing `0xFF`. Every one of the eleven app writes in the real capture has
/// this shape.
pub const A3954_SOUND_MODE_COMMAND_BODY_LEN: usize = 5;

/// The two A3954 equalizer bands the Soundcore app never shows, in the same raw
/// dB×10 units as every other band.
///
/// A custom write fixes them at 0 dB, which is what the app's own custom curve
/// capture contains; the built-in presets keep their own values for these two
/// bands.
pub const A3954_FIXED_EQ_BANDS: [i16; A3954_EQ_BANDS - VISIBLE_EQ_BANDS] = [0, 0];

/// The measured bytes at body 24..26 of the A3954 `03 87` body.
///
/// The Soundcore app sends `FF FF` there and the device acknowledged those
/// frames; OpenSCL4's builder sends `00 00` and that variant was never sent.
/// The app's measured bytes are used here. Their meaning is unknown.
const A3954_EQUALIZER_UNKNOWN_BODY_BYTES: [u8; 2] = [0xFF, 0xFF];

/// The length of the A3954 HearID block inside the `01 01` body (67..115).
const A3954_HEAR_ID_BLOCK_LEN: usize = 48;

/// The eight main-button action bytes in the A3954 `01 01` body (116..124), in
/// [`A3954Button::ALL`] order.
const A3954_MAIN_BUTTONS_OFFSET: usize = 116;
const A3954_MAIN_BUTTON_COUNT: usize = 8;

/// The four slide-button action bytes in the A3954 `01 01` body (161..165), in
/// [`A3954Button::ALL`] order after the main block.
const A3954_SLIDE_BUTTON_COUNT: usize = 4;

/// The A3954 body offset of the case firmware string.
const A3954_CASE_FIRMWARE_OFFSET: usize = 32;
/// The A3954 body offset of the raw case battery level.
const A3954_CASE_BATTERY_OFFSET: usize = 37;
/// The A3954 body offset of the six-byte case serial.
const A3954_CASE_SERIAL_OFFSET: usize = 38;
/// The A3954 body offset of the ambient-sound-mode cycle byte.
const A3954_AMBIENT_SOUND_MODE_CYCLE_OFFSET: usize = 124;
/// The A3954 body offset of the first of the four case-feature flags.
const A3954_CASE_FEATURES_OFFSET: usize = 132;
/// The A3954 body offset of the air-pressure byte.
const A3954_AIR_PRESSURE_OFFSET: usize = 136;
/// The A3954 body offset of the low-battery prompt flag.
const A3954_LOW_BATTERY_PROMPT_OFFSET: usize = 140;
/// The A3954 body offset of the LDAC flag.
const A3954_LDAC_OFFSET: usize = 141;
/// The A3954 body offset of the dual-connections flag.
const A3954_DUAL_CONNECTIONS_OFFSET: usize = 142;
/// The A3954 body offset of the auto-power-off enable flag and duration index.
const A3954_AUTO_POWER_OFF_OFFSET: usize = 143;
/// The A3954 body offset of the limit-high-volume enable flag, dB limit and
/// refresh rate.
const A3954_LIMIT_HIGH_VOLUME_OFFSET: usize = 145;
/// The A3954 body offset of the spatial-audio enable flag, music mode and mode.
const A3954_SPATIAL_AUDIO_OFFSET: usize = 148;
/// The A3954 body offset of the easy-chat enable flag.
const A3954_EASY_CHAT_ENABLED_OFFSET: usize = 151;
/// The A3954 body offset of the sound-leak-compensation flag.
const A3954_SOUND_LEAK_COMPENSATION_OFFSET: usize = 153;
/// The A3954 body offset of the case language.
const A3954_CASE_LANGUAGE_OFFSET: usize = 157;
/// The A3954 body offset of the easy-chat wait time.
const A3954_EASY_CHAT_WAIT_TIME_OFFSET: usize = 158;
/// The A3954 body offset of the wearing-detection flag.
const A3954_WEARING_DETECTION_OFFSET: usize = 159;
/// The A3954 body offset of the four slide-button bytes.
const A3954_SLIDE_BUTTONS_OFFSET: usize = 161;

/// The A3954 button configuration command, `04 81`.
///
/// The measured body is the three bytes `[side, button_id, (non_tws << 4) |
/// tws]`; see [`A3954Button::command_body`]. The A3954 has no set-all button
/// packet, so one write carries one gesture.
pub const A3954_BUTTON_CONFIGURATION: Command = Command::new(0x04, 0x81);

/// The measured A3954-specific blocks of the `01 01` state response.
///
/// A model with this capability is an A3954: its state is parsed into
/// [`DeviceState::a3954`] and it builds the A3954 `06 81`/`03 87`/`04 81`
/// and setting frames. The remaining A3954 offsets are fixed by the measured
/// body length and documented on [`A3954State`]; these two start blocks are
/// recorded here because the body parsers read them through the capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954Capability {
    /// Body offset of the four sound-mode bytes: the ambient mode at 125, then
    /// the slider position, the airplane mode and the wind noise. The
    /// ambient-sound-mode cycle byte is the byte just before them at 124.
    pub sound_mode_offset: usize,
    /// Body offset of the equalizer preset id (44). The two ten-band channels
    /// follow at 46 and 56 and the HearID block follows at 67.
    pub equalizer_offset: usize,
}

/// One of the four ambient-sound modes of the A3954 `06 81` body.
///
/// The mode is the first byte of the sound-mode body and of the state's
/// sound-mode block; the slider position at the next byte decides which half of
/// the 1..=11 scale is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954AmbientSoundMode {
    /// Noise canceling, `0`.
    NoiseCanceling,
    /// Transparency, `1`.
    Transparency,
    /// Normal, `2`.
    Normal,
    /// Airplane mode, `3`.
    AirplaneMode,
}

impl A3954AmbientSoundMode {
    /// Every measured mode, in body-value order.
    pub const ALL: [Self; 4] = [
        Self::NoiseCanceling,
        Self::Transparency,
        Self::Normal,
        Self::AirplaneMode,
    ];

    /// The mode's `06 81` body and state value.
    pub const fn value(self) -> u8 {
        match self {
            Self::NoiseCanceling => 0,
            Self::Transparency => 1,
            Self::Normal => 2,
            Self::AirplaneMode => 3,
        }
    }

    /// The mode a byte names, or `None` for a byte outside `0..=3`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::NoiseCanceling),
            1 => Some(Self::Transparency),
            2 => Some(Self::Normal),
            3 => Some(Self::AirplaneMode),
            _ => None,
        }
    }
}

/// The A3954 airplane-mode update setting: the third `06 81` body byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954AirplaneMode {
    /// Manual updates, `0`.
    ManualUpdate,
    /// Automatic updates, `1`.
    AutomaticUpdate,
}

impl A3954AirplaneMode {
    /// The mode's body and state value.
    pub const fn value(self) -> u8 {
        match self {
            Self::ManualUpdate => 0,
            Self::AutomaticUpdate => 1,
        }
    }

    /// The mode a byte names, or `None` for a byte outside `0..=1`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::ManualUpdate),
            1 => Some(Self::AutomaticUpdate),
            _ => None,
        }
    }
}

/// A validated A3954 sound-mode slider position on the measured 1..=11 scale.
///
/// The A3954 has one scale instead of separate ANC and transparency strengths:
/// positions 1..=5 are transparency (1 is the strongest), 6 is normal and 7..=11
/// is noise canceling (11 is the strongest). This type is the only way to build
/// a [`A3954SoundMode`], so `0` and `12..` cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct A3954SoundModeSlider(u8);

impl A3954SoundModeSlider {
    /// The lowest position, 1 (the strongest transparency).
    pub const MIN: u8 = 1;
    /// The highest position, 11 (the strongest noise canceling).
    pub const MAX: u8 = 11;
    /// The normal-mode position, 6.
    pub const NORMAL: u8 = 6;

    /// The slider a position names, or `None` for a position outside 1..=11.
    pub const fn new(position: u8) -> Option<Self> {
        if position < Self::MIN || position > Self::MAX {
            None
        } else {
            Some(Self(position))
        }
    }

    /// The normal-mode slider, position 6.
    pub const fn normal() -> Self {
        Self(Self::NORMAL)
    }

    /// The scale position, in `1..=11`.
    pub const fn value(self) -> u8 {
        self.0
    }

    /// The mode this position selects: 1..=5 transparency, 6 normal, 7..=11
    /// noise canceling.
    pub const fn mode(self) -> A3954AmbientSoundMode {
        if self.0 < Self::NORMAL {
            A3954AmbientSoundMode::Transparency
        } else if self.0 == Self::NORMAL {
            A3954AmbientSoundMode::Normal
        } else {
            A3954AmbientSoundMode::NoiseCanceling
        }
    }
}

/// The A3954 sound-mode block of `01 01` body 125..129 and its `06 81` write.
///
/// | body offset | field |
/// |---:|---|
/// | 125 | [`A3954AmbientSoundMode`] |
/// | 126 | the 1..=11 [`A3954SoundModeSlider`] position |
/// | 127 | [`A3954AirplaneMode`] |
/// | 128 | wind noise: bit 0 suppression, bit 1 detected |
///
/// The measured `06 81` body is `[mode, position, airplane, wind, 0xFF]`; all
/// eleven app writes in the real capture have that shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954SoundMode {
    /// The ambient sound mode; the slider decides which half of the scale is
    /// active, so this must agree with [`Self::slider`] for every mode except
    /// airplane mode. The parser keeps both bytes as measured instead of
    /// rejecting an inconsistent pair.
    pub mode: A3954AmbientSoundMode,
    /// The 1..=11 scale position.
    pub slider: A3954SoundModeSlider,
    /// The airplane-mode update setting.
    pub airplane_mode: A3954AirplaneMode,
    /// Wind-noise suppression: the wind flag bit 0 and the `06 81` wind byte.
    pub wind_noise_suppression: bool,
    /// Wind-noise detection: state body bit 1. Read-only, because the measured
    /// writes carry only the suppression bit here.
    pub wind_noise_detected: bool,
}

impl A3954SoundMode {
    /// The exact measured `06 81` body for this state.
    ///
    /// The write carries the state's mode, slider, airplane mode and the
    /// suppression bit; the detection bit is a status and is never written as
    /// part of the flag. The trailing byte is the measured `0xFF`.
    pub fn command_body(self) -> [u8; A3954_SOUND_MODE_COMMAND_BODY_LEN] {
        [
            self.mode.value(),
            self.slider.value(),
            self.airplane_mode.value(),
            u8::from(self.wind_noise_suppression),
            0xFF,
        ]
    }
}

/// One of the four measured A3954 auto-power-off durations.
///
/// The state byte and the `01 86` body carry an index `0..=3`; the A3954 uses
/// ten-minute increments, so the index names 10, 20, 30 or 60 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954AutoPowerOffDuration {
    TenMinutes,
    TwentyMinutes,
    ThirtyMinutes,
    SixtyMinutes,
}

impl A3954AutoPowerOffDuration {
    /// Every duration, in index order.
    pub const ALL: [Self; 4] = [
        Self::TenMinutes,
        Self::TwentyMinutes,
        Self::ThirtyMinutes,
        Self::SixtyMinutes,
    ];

    /// The duration's state and `01 86` body index, `0..=3`.
    pub const fn index(self) -> u8 {
        match self {
            Self::TenMinutes => 0,
            Self::TwentyMinutes => 1,
            Self::ThirtyMinutes => 2,
            Self::SixtyMinutes => 3,
        }
    }

    /// The duration an index names, or `None` for an index outside `0..=3`.
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::TenMinutes),
            1 => Some(Self::TwentyMinutes),
            2 => Some(Self::ThirtyMinutes),
            3 => Some(Self::SixtyMinutes),
            _ => None,
        }
    }

    /// The duration in minutes.
    pub const fn minutes(self) -> u16 {
        match self {
            Self::TenMinutes => 10,
            Self::TwentyMinutes => 20,
            Self::ThirtyMinutes => 30,
            Self::SixtyMinutes => 60,
        }
    }
}

/// The A3954 auto-power-off state and `01 86` body: enabled and a duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954AutoPowerOff {
    pub enabled: bool,
    pub duration: A3954AutoPowerOffDuration,
}

/// The four case-feature switches of the A3954 `01 01` body 132..136.
///
/// Each byte is a flag: `1` is on and every other byte reads as off, as in
/// OpenSCL4's parser. [`Self::command_body`] writes them back as the measured
/// four-byte `07 87` body, in the order the Soundcore app sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954CaseFeatures {
    /// The case's atmospheric-pressure display.
    pub atmospheric: bool,
    /// The case's remote-camera shutter button.
    pub remote_camera: bool,
    /// The case's find-device button.
    pub find_device: bool,
    /// The case's spatial-audio button (separate from the earbuds' spatial
    /// audio).
    pub case_spatial_audio: bool,
}

impl A3954CaseFeatures {
    /// The exact measured `07 87` body, in the Soundcore app's order:
    /// `[remote_camera, find_device, case_spatial_audio, atmospheric]`.
    ///
    /// The command order is **not** the state order: body 132..136 reports
    /// `[atmospheric, remote_camera, find_device, case_spatial_audio]`, while
    /// the app's `M5` builder (`CmmBtCmdService`) sends the remote camera,
    /// find-device, case-spatial and atmospheric flags in that order. The live
    /// acceptance found it on the real device: a body in state order toggled the
    /// neighbouring feature (writing `[1, 0, 0, 0]` switched the remote camera
    /// on), and this order toggles the requested feature and restores cleanly.
    pub fn command_body(self) -> [u8; 4] {
        [
            u8::from(self.remote_camera),
            u8::from(self.find_device),
            u8::from(self.case_spatial_audio),
            u8::from(self.atmospheric),
        ]
    }
}

/// The A3954 spatial-audio music mode: the second `10 81` body byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954SpatialAudioMusicMode {
    /// Fixed, body value `1`.
    Fixed,
    /// Head tracking, body value `2`.
    HeadTracking,
}

impl A3954SpatialAudioMusicMode {
    /// The mode's `10 81` body value.
    pub const fn value(self) -> u8 {
        match self {
            Self::Fixed => 1,
            Self::HeadTracking => 2,
        }
    }

    /// The mode a state byte names, or `None` for a byte outside the measured
    /// values.
    ///
    /// The state reports `0` for fixed music mode in the real capture even
    /// though every command writes `1`, so `0` reads as [`Self::Fixed`]; `1`
    /// and `2` are the command values and any other byte is rejected.
    pub const fn from_state_value(value: u8) -> Option<Self> {
        match value {
            0 | 1 => Some(Self::Fixed),
            2 => Some(Self::HeadTracking),
            _ => None,
        }
    }
}

/// The A3954 spatial-audio effect mode: the third `10 81` body byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954SpatialAudioMode {
    Music,
    Podcast,
    Movie,
    Gaming,
}

impl A3954SpatialAudioMode {
    /// Every mode, in body-value order.
    pub const ALL: [Self; 4] = [Self::Music, Self::Podcast, Self::Movie, Self::Gaming];

    /// The mode's `10 81` body and state value.
    pub const fn value(self) -> u8 {
        match self {
            Self::Music => 0,
            Self::Podcast => 1,
            Self::Movie => 2,
            Self::Gaming => 3,
        }
    }

    /// The mode a byte names, or `None` for a byte outside `0..=3`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Music),
            1 => Some(Self::Podcast),
            2 => Some(Self::Movie),
            3 => Some(Self::Gaming),
            _ => None,
        }
    }
}

/// The A3954 spatial-audio state and `10 81` body.
///
/// | body offset | field |
/// |---:|---|
/// | 148 | enabled |
/// | 149 | [`A3954SpatialAudioMusicMode`] |
/// | 150 | [`A3954SpatialAudioMode`] |
///
/// The `10 81` body is `[enabled, music_mode, mode]`, so turning spatial audio
/// off is a plain `10 81` write with the flag at `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954SpatialAudio {
    pub enabled: bool,
    pub music_mode: A3954SpatialAudioMusicMode,
    pub mode: A3954SpatialAudioMode,
}

impl A3954SpatialAudio {
    /// The exact `10 81` body for this state.
    pub fn command_body(self) -> [u8; 3] {
        [
            u8::from(self.enabled),
            self.music_mode.value(),
            self.mode.value(),
        ]
    }
}

/// One of the three measured A3954 easy-chat wait times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954EasyChatWaitTime {
    FiveSeconds,
    TenSeconds,
    FifteenSeconds,
}

impl A3954EasyChatWaitTime {
    /// Every wait time, in body-value order.
    pub const ALL: [Self; 3] = [Self::FiveSeconds, Self::TenSeconds, Self::FifteenSeconds];

    /// The wait time in seconds.
    pub const fn seconds(self) -> u16 {
        match self {
            Self::FiveSeconds => 5,
            Self::TenSeconds => 10,
            Self::FifteenSeconds => 15,
        }
    }

    /// The wait time's state and `10 9D` body value, `0..=2`.
    pub const fn value(self) -> u8 {
        match self {
            Self::FiveSeconds => 0,
            Self::TenSeconds => 1,
            Self::FifteenSeconds => 2,
        }
    }

    /// The wait time a value names, or `None` for a value outside `0..=2`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::FiveSeconds),
            1 => Some(Self::TenSeconds),
            2 => Some(Self::FifteenSeconds),
            _ => None,
        }
    }
}

/// The A3954 easy-chat state: enabled plus the wait time at body byte 158.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954EasyChat {
    pub enabled: bool,
    pub wait_time: A3954EasyChatWaitTime,
}

impl A3954EasyChat {
    /// The exact `10 9D` body the measured A3954 (firmware 04.29) takes:
    /// `[enabled, wait_time]`.
    ///
    /// The Soundcore app sends the wait time only from firmware 03.09 and
    /// 04.09 on; no older A3954 firmware was measured, so this slice always
    /// sends both bytes.
    pub fn command_body(self) -> [u8; 2] {
        [u8::from(self.enabled), self.wait_time.value()]
    }
}

/// The three measured A3954 case-display languages.
///
/// The `01 01` body reports the value at byte 157 and the app writes the same
/// value back as the measured one-byte `07 8A` body; see
/// [`A3954SettingCommand::CaseLanguage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954CaseLanguage {
    Japanese,
    English,
    Chinese,
}

impl A3954CaseLanguage {
    /// Every language, in body-value order.
    pub const ALL: [Self; 3] = [Self::Japanese, Self::English, Self::Chinese];

    /// The language's state and `07 8A` body value, `0..=2`.
    pub const fn value(self) -> u8 {
        match self {
            Self::Japanese => 0,
            Self::English => 1,
            Self::Chinese => 2,
        }
    }

    /// The language a value names, or `None` for a value outside `0..=2`.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Japanese),
            1 => Some(Self::English),
            2 => Some(Self::Chinese),
            _ => None,
        }
    }

    /// The exact measured `07 8A` body for this language.
    pub const fn command_body(self) -> [u8; 1] {
        [self.value()]
    }
}

/// The two sides of an A3954 gesture: the first `04 81` body byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954ButtonSide {
    /// Left, body value `0`.
    Left,
    /// Right, body value `1`.
    Right,
}

impl A3954ButtonSide {
    /// The side's `04 81` body value.
    pub const fn value(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }

    /// The side a value names, or `None` for a byte outside the measured
    /// values.
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Left),
            1 => Some(Self::Right),
            _ => None,
        }
    }
}

/// One of the six measured A3954 gesture kinds and its `04 81` button id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954ButtonPressKind {
    /// Single press, button id `2`.
    Single,
    /// Double press, button id `0`.
    Double,
    /// Triple press, button id `5`.
    Triple,
    /// Long press, button id `1`.
    Long,
    /// Slide up, button id `6`.
    SlideUp,
    /// Slide down, button id `7`.
    SlideDown,
}

impl A3954ButtonPressKind {
    /// Every measured press kind, in state order.
    pub const ALL: [Self; 6] = [
        Self::Single,
        Self::Double,
        Self::Triple,
        Self::Long,
        Self::SlideUp,
        Self::SlideDown,
    ];

    /// The measured `04 81` button id, as OpenSCL4's A3954 table reports it:
    /// single `2`, double `0`, triple `5`, long `1`, slide up `6` and slide
    /// down `7`.
    pub const fn button_id(self) -> u8 {
        match self {
            Self::Single => 2,
            Self::Double => 0,
            Self::Triple => 5,
            Self::Long => 1,
            Self::SlideUp => 6,
            Self::SlideDown => 7,
        }
    }

    /// Whether this gesture is one of the two swipes.
    pub const fn is_slide(self) -> bool {
        matches!(self, Self::SlideUp | Self::SlideDown)
    }

    /// The actions measured for this press kind.
    ///
    /// The six press gestures take [`A3954_COMMON_ACTIONS`]; the two slides take
    /// the volume up/down set only.
    pub const fn available_actions(self) -> &'static [A3954ButtonAction] {
        if self.is_slide() {
            A3954_SLIDE_ACTIONS
        } else {
            A3954_COMMON_ACTIONS
        }
    }
}

/// A measured A3954 gesture action id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954ButtonAction {
    /// Volume up, id `0`.
    VolumeUp,
    /// Volume down, id `1`.
    VolumeDown,
    /// Previous song, id `2`.
    PreviousSong,
    /// Next song, id `3`.
    NextSong,
    /// Ambient sound mode, id `4`.
    AmbientSoundMode,
    /// Voice assistant, id `5`.
    VoiceAssistant,
    /// Play/pause, id `6`.
    PlayPause,
}

impl A3954ButtonAction {
    /// Every measured action, in body-id order.
    pub const ALL: [Self; 7] = [
        Self::VolumeUp,
        Self::VolumeDown,
        Self::PreviousSong,
        Self::NextSong,
        Self::AmbientSoundMode,
        Self::VoiceAssistant,
        Self::PlayPause,
    ];

    /// The action's measured id, `0..=6`.
    pub const fn value(self) -> u8 {
        match self {
            Self::VolumeUp => 0,
            Self::VolumeDown => 1,
            Self::PreviousSong => 2,
            Self::NextSong => 3,
            Self::AmbientSoundMode => 4,
            Self::VoiceAssistant => 5,
            Self::PlayPause => 6,
        }
    }

    /// The action a value names, or `None` for any other byte, including
    /// [`A3954_NO_BUTTON_ACTION`].
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::VolumeUp),
            1 => Some(Self::VolumeDown),
            2 => Some(Self::PreviousSong),
            3 => Some(Self::NextSong),
            4 => Some(Self::AmbientSoundMode),
            5 => Some(Self::VoiceAssistant),
            6 => Some(Self::PlayPause),
            _ => None,
        }
    }
}

/// The measured A3954 "no action" value, `0x0F`.
///
/// The A3954 has no separate enabled flag: a gesture slot whose current action
/// is `0x0F` is disabled, exactly as OpenSCL4 reads the TwsLowBits action byte.
pub const A3954_NO_BUTTON_ACTION: u8 = 0x0F;

/// The actions the Soundcore app offers for the six A3954 press gestures:
/// volume up/down, previous/next, ambient sound mode, voice assistant and
/// play/pause (OpenSCL4's `COMMON_ACTIONS`, without gaming mode).
pub const A3954_COMMON_ACTIONS: &[A3954ButtonAction] = &[
    A3954ButtonAction::VolumeUp,
    A3954ButtonAction::VolumeDown,
    A3954ButtonAction::PreviousSong,
    A3954ButtonAction::NextSong,
    A3954ButtonAction::AmbientSoundMode,
    A3954ButtonAction::VoiceAssistant,
    A3954ButtonAction::PlayPause,
];

/// The actions the app offers for the two A3954 slides: volume up and volume
/// down only (`0` and `1`). The six common actions are not available on a
/// slide, and both slide gestures share these two ids.
///
/// The catalog is the measured *action* set; it is not the set of writable
/// slots. OpenSCL4's A3954 table sets `ButtonDisableMode::IndividualDisable` for
/// all twelve gestures, slides included, so [`A3954_NO_BUTTON_ACTION`] is a
/// valid value for a slide row as well (a caller builds that through
/// [`A3954ButtonActionValue::Disabled`]/[`A3954ButtonActionByte`], not through
/// this catalog).
pub const A3954_SLIDE_ACTIONS: &[A3954ButtonAction] =
    &[A3954ButtonAction::VolumeUp, A3954ButtonAction::VolumeDown];

/// One action slot of an A3954 gesture byte.
///
/// The measured state and `04 81` byte packs two ids,
/// `(non_tws << 4) | tws` (OpenSCL4's `TwsLowBits` encoding): the low nibble is
/// the action while the earbuds are TWS-connected and the high nibble the
/// action while they are not. A nibble the device reports outside the measured
/// action ids and [`A3954_NO_BUTTON_ACTION`] is kept as [`Self::Unknown`], so a
/// state value is never silently lost or rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954ButtonActionValue {
    /// A measured action id, `0..=6`.
    Action(A3954ButtonAction),
    /// No action, `0x0F`.
    Disabled,
    /// A nibble outside the measured action values, kept verbatim.
    Unknown(u8),
}

impl A3954ButtonActionValue {
    /// The value a nibble names.
    pub const fn from_nibble(nibble: u8) -> Self {
        match A3954ButtonAction::from_value(nibble) {
            Some(action) => Self::Action(action),
            None if nibble == A3954_NO_BUTTON_ACTION => Self::Disabled,
            None => Self::Unknown(nibble),
        }
    }

    /// The nibble this value writes.
    pub const fn nibble(self) -> u8 {
        match self {
            Self::Action(action) => action.value(),
            Self::Disabled => A3954_NO_BUTTON_ACTION,
            Self::Unknown(nibble) => nibble,
        }
    }

    /// The measured action, or `None` for the disabled and unknown values.
    pub const fn action(self) -> Option<A3954ButtonAction> {
        match self {
            Self::Action(action) => Some(action),
            Self::Disabled | Self::Unknown(_) => None,
        }
    }
}

impl From<A3954ButtonAction> for A3954ButtonActionValue {
    fn from(action: A3954ButtonAction) -> Self {
        Self::Action(action)
    }
}

/// One A3954 gesture's raw action byte, from the `01 01` state or the `04 81`
/// body: `(non_tws << 4) | tws`.
///
/// The byte is kept whole, so a caller can preserve the slot it is not editing
/// and restore exactly what the device reported. The two nibbles decode
/// through [`A3954ButtonActionValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954ButtonActionByte(u8);

impl A3954ButtonActionByte {
    /// The raw byte, accepted as the device reported it.
    pub const fn from_byte(byte: u8) -> Self {
        Self(byte)
    }

    /// The raw byte the state reported or the `04 81` body carries.
    pub const fn byte(self) -> u8 {
        self.0
    }

    /// The action while the two earbuds are TWS-connected: the low nibble.
    pub const fn tws(self) -> A3954ButtonActionValue {
        A3954ButtonActionValue::from_nibble(self.0 & 0x0F)
    }

    /// The action while the earbuds are not TWS-connected: the high nibble.
    pub const fn non_tws(self) -> A3954ButtonActionValue {
        A3954ButtonActionValue::from_nibble(self.0 >> 4)
    }

    /// This byte with the connected-state action replaced.
    pub const fn with_tws(self, value: A3954ButtonActionValue) -> Self {
        Self((self.0 & 0xF0) | value.nibble())
    }

    /// This byte with the disconnected-state action replaced.
    pub const fn with_non_tws(self, value: A3954ButtonActionValue) -> Self {
        Self((self.0 & 0x0F) | (value.nibble() << 4))
    }
}

/// The twelve measured A3954 gestures, in the order the `01 01` state reports
/// them: the eight press gestures at body 116..124 and the four slides at
/// 161..165.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954Button {
    LeftSingle,
    RightSingle,
    LeftDouble,
    RightDouble,
    LeftTriple,
    RightTriple,
    LeftLong,
    RightLong,
    LeftSlideUp,
    RightSlideUp,
    LeftSlideDown,
    RightSlideDown,
}

impl A3954Button {
    /// Every gesture, in measured state order.
    pub const ALL: [Self; 12] = [
        Self::LeftSingle,
        Self::RightSingle,
        Self::LeftDouble,
        Self::RightDouble,
        Self::LeftTriple,
        Self::RightTriple,
        Self::LeftLong,
        Self::RightLong,
        Self::LeftSlideUp,
        Self::RightSlideUp,
        Self::LeftSlideDown,
        Self::RightSlideDown,
    ];

    /// The gesture's position in measured state order.
    pub const fn index(self) -> usize {
        match self {
            Self::LeftSingle => 0,
            Self::RightSingle => 1,
            Self::LeftDouble => 2,
            Self::RightDouble => 3,
            Self::LeftTriple => 4,
            Self::RightTriple => 5,
            Self::LeftLong => 6,
            Self::RightLong => 7,
            Self::LeftSlideUp => 8,
            Self::RightSlideUp => 9,
            Self::LeftSlideDown => 10,
            Self::RightSlideDown => 11,
        }
    }

    /// The gesture at a measured state-order position, or `None` outside
    /// `0..12`.
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::LeftSingle),
            1 => Some(Self::RightSingle),
            2 => Some(Self::LeftDouble),
            3 => Some(Self::RightDouble),
            4 => Some(Self::LeftTriple),
            5 => Some(Self::RightTriple),
            6 => Some(Self::LeftLong),
            7 => Some(Self::RightLong),
            8 => Some(Self::LeftSlideUp),
            9 => Some(Self::RightSlideUp),
            10 => Some(Self::LeftSlideDown),
            11 => Some(Self::RightSlideDown),
            _ => None,
        }
    }

    /// Which earbud the gesture belongs to.
    pub const fn side(self) -> A3954ButtonSide {
        match self {
            Self::LeftSingle
            | Self::LeftDouble
            | Self::LeftTriple
            | Self::LeftLong
            | Self::LeftSlideUp
            | Self::LeftSlideDown => A3954ButtonSide::Left,
            Self::RightSingle
            | Self::RightDouble
            | Self::RightTriple
            | Self::RightLong
            | Self::RightSlideUp
            | Self::RightSlideDown => A3954ButtonSide::Right,
        }
    }

    /// The measured press kind.
    pub const fn press_kind(self) -> A3954ButtonPressKind {
        match self {
            Self::LeftSingle | Self::RightSingle => A3954ButtonPressKind::Single,
            Self::LeftDouble | Self::RightDouble => A3954ButtonPressKind::Double,
            Self::LeftTriple | Self::RightTriple => A3954ButtonPressKind::Triple,
            Self::LeftLong | Self::RightLong => A3954ButtonPressKind::Long,
            Self::LeftSlideUp | Self::RightSlideUp => A3954ButtonPressKind::SlideUp,
            Self::LeftSlideDown | Self::RightSlideDown => A3954ButtonPressKind::SlideDown,
        }
    }

    /// The measured `04 81` button id for this gesture.
    pub const fn button_id(self) -> u8 {
        self.press_kind().button_id()
    }

    /// Whether this gesture is one of the two swipes.
    pub const fn is_slide(self) -> bool {
        self.press_kind().is_slide()
    }

    /// The actions measured for this gesture.
    pub const fn available_actions(self) -> &'static [A3954ButtonAction] {
        self.press_kind().available_actions()
    }

    /// The exact measured `04 81` body: `[side, button_id, action_byte]`.
    pub const fn command_body(self, action: A3954ButtonActionByte) -> [u8; 3] {
        [self.side().value(), self.button_id(), action.byte()]
    }
}

/// The built-in equalizer presets of the Liberty 4 Pro (A3954), copied from
/// OpenSCL4's `common_settings_type_2` table, which is the table the reference
/// uses for this model.
///
/// The A3954 catalog has twenty-two presets with sequential ids `0..=21`: unlike
/// the A3062 list it *does* contain Bass Booster at id `2` and has no `0x7E7E`
/// entry. The names are the English display names of the same table; its
/// internal identifiers have no spaces (for example `SoundcoreSignature`).
///
/// Band values are in the same raw dB×10 units as the A3062 catalog: the `01 01`
/// byte is `value + 120`, so `0` is `0 dB` and `-120` is `-12.0 dB`. The preset
/// values stay inside a `-60..=60` range except the last band, which every
/// preset fixes at `-120`; the Equalizer page shows only the first eight bands.
/// Custom curves are reported as [`CUSTOM_EQUALIZER_PRESET_ID`] and are not a
/// preset.
pub const A3954_EQUALIZER_PRESETS: &[EqualizerPreset] = &[
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
        id: 2,
        name: "Bass Booster",
        bands: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
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
];

/// The built-in A3954 preset with `id`, or `None` for
/// [`CUSTOM_EQUALIZER_PRESET_ID`] and any id outside the A3954 list.
pub fn a3954_equalizer_preset(id: u16) -> Option<&'static EqualizerPreset> {
    A3954_EQUALIZER_PRESETS
        .iter()
        .find(|preset| preset.id == id)
}

/// The equalizer and HearID block of a parsed A3954 `01 01` state.
///
/// | body offset | field |
/// |---:|---|
/// | 44..46 | preset id, little-endian |
/// | 46..56 | left (first) channel, ten band bytes; a leading `0xFF` marks the whole channel absent |
/// | 56..66 | right (second) channel, ten band bytes |
/// | 66 | one unknown byte (the app model calls it the age range) |
/// | 67..115 | the 48-byte HearID block, kept verbatim for writes |
///
/// A band byte is `value + 120`, the same units as the A3062 equalizer. The
/// HearID block is preserved byte for byte so a preset or custom write does not
/// erase HearID data; its `enabled` flag is cleared in the write body, exactly
/// as OpenSCL4's A3954 equalizer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A3954EqualizerState {
    /// The preset id at body 44..46; [`CUSTOM_EQUALIZER_PRESET_ID`] when custom
    /// bands are active.
    pub preset_id: u16,
    /// The left channel's ten raw band values, or `None` when the device
    /// reports the channel absent (its first byte is `0xFF`).
    pub left: Option<[i16; A3954_EQ_BANDS]>,
    /// The right channel's ten raw band values, or `None` when absent.
    pub right: Option<[i16; A3954_EQ_BANDS]>,
    /// The measured 48-byte HearID block at body 67..115.
    hear_id: [u8; A3954_HEAR_ID_BLOCK_LEN],
}

impl A3954EqualizerState {
    /// Whether the device reports custom bands instead of a built-in preset.
    pub fn is_custom(&self) -> bool {
        self.preset_id == CUSTOM_EQUALIZER_PRESET_ID
    }

    /// The built-in A3954 preset this state reports, or `None` for custom bands
    /// and unknown ids.
    pub fn preset(&self) -> Option<&'static EqualizerPreset> {
        a3954_equalizer_preset(self.preset_id)
    }

    /// The eight bands the Equalizer page shows, from the first present
    /// channel, in raw dB×10 units.
    pub fn visible_bands(&self) -> Option<[i16; VISIBLE_EQ_BANDS]> {
        self.left_visible_bands()
            .or_else(|| self.right_visible_bands())
    }

    /// The left channel's eight visible bands, in raw dB×10 units.
    pub fn left_visible_bands(&self) -> Option<[i16; VISIBLE_EQ_BANDS]> {
        self.left.map(visible_bands)
    }

    /// The right channel's eight visible bands, in raw dB×10 units.
    pub fn right_visible_bands(&self) -> Option<[i16; VISIBLE_EQ_BANDS]> {
        self.right.map(visible_bands)
    }
}

/// The eight visible bands of a ten-band A3954 channel.
fn visible_bands(bands: [i16; A3954_EQ_BANDS]) -> [i16; VISIBLE_EQ_BANDS] {
    let mut visible = [0i16; VISIBLE_EQ_BANDS];
    visible.copy_from_slice(&bands[..VISIBLE_EQ_BANDS]);
    visible
}

/// The remaining measured settings of an A3954, parsed from the 165-byte
/// `01 01` body.
///
/// | body offset | field |
/// |---:|---|
/// | 32..37 | case firmware, ASCII `MM.mm` |
/// | 37 | case battery level, raw `0..=9` (255 = unavailable); the display level is raw + 1 of 10 |
/// | 38..44 | case serial, kept internal and usually `None` in masked captures |
/// | 44..115 | [`A3954EqualizerState`]: preset, two ten-band channels and the HearID block |
/// | 116..124 | eight main-button action bytes, [`A3954Button::ALL`] order |
/// | 124 | ambient-sound-mode cycle bits |
/// | 125..129 | [`A3954SoundMode`], or unavailable (`None`) when a value is unmeasured |
/// | 129..132 | three unknown bytes the parser skips |
/// | 132..136 | [`A3954CaseFeatures`] |
/// | 136 | air pressure, raw centi-atmospheres |
/// | 137..140 | three unknown bytes the parser skips |
/// | 140 | low-battery prompt |
/// | 141 | LDAC |
/// | 142 | dual connections |
/// | 143..145 | auto power off: enabled, duration index |
/// | 145..148 | limit high volume: enabled, dB limit, refresh rate |
/// | 148..151 | [`A3954SpatialAudio`] |
/// | 151 | easy chat enabled |
/// | 152 | one unknown byte the parser skips |
/// | 153 | sound leak compensation |
/// | 154..157 | three unknown bytes the parser skips |
/// | 157 | [`A3954CaseLanguage`] |
/// | 158 | [`A3954EasyChat::wait_time`] |
/// | 159 | wearing detection |
/// | 160 | one unknown byte the parser skips |
/// | 161..165 | four slide-button action bytes, [`A3954Button::ALL`] order |
///
/// The serials are deliberately optional and are never logged, exactly like
/// [`DeviceState::serial`]. The raw button blocks are kept as bytes and decoded
/// on demand through [`Self::button_action_byte`]; air-pressure *writes* stay
/// out of this slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A3954State {
    /// The case firmware, when the body holds a valid `MM.mm` pair.
    pub case_firmware: Option<String>,
    /// The raw case battery level at body 37, or `None` when the device reports
    /// 255 (unavailable). The display level is this value + 1 of 10.
    pub case_battery_level: Option<u8>,
    /// The case serial (`3954` plus six hex bytes). Callers must not log it.
    pub case_serial: Option<String>,
    /// The equalizer preset, bands and HearID block.
    pub equalizer: A3954EqualizerState,
    /// The eight main-button action bytes at body 116..124, in
    /// [`A3954Button::ALL`] order (the eight press gestures). See
    /// [`Self::button_action_byte`].
    pub main_buttons: [u8; A3954_MAIN_BUTTON_COUNT],
    /// The ambient-sound-mode cycle bits at body 124.
    pub ambient_sound_mode_cycle: AmbientSoundModeCycle,
    /// The sound-mode block at body 125..129, or `None` when the device reports
    /// a value this slice has not measured.
    ///
    /// The one known case is airplane mode: the real device then reports slider
    /// byte `13`, which is outside the measured 1..=11 scale (observed on the
    /// real A3954 in ticket 04; the byte's meaning is unmeasured). The block is
    /// unavailable in that case so the rest of the state still parses and the
    /// pages hide the scale instead of the whole poll failing; no scale
    /// position is invented for the byte.
    pub sound_mode: Option<A3954SoundMode>,
    /// The case features at body 132..136.
    pub case_features: A3954CaseFeatures,
    /// The raw air-pressure byte at body 136: centi-atmospheres, so `98` is
    /// `0.98` atm. Writes are out of this slice.
    pub air_pressure: u8,
    /// The low-battery voice prompt.
    pub low_battery_prompt: bool,
    /// LDAC; available only in the `01 01` state and confirmed by a separate
    /// `01 7F` notification after a write.
    pub ldac: bool,
    /// Whether a second device may be connected at the same time.
    pub dual_connections: bool,
    /// Auto power off, enabled and duration.
    pub auto_power_off: A3954AutoPowerOff,
    /// Limit high volume: enabled, dB limit and refresh rate.
    pub limit_high_volume: LimitHighVolume,
    /// The spatial-audio state.
    pub spatial_audio: A3954SpatialAudio,
    /// Easy chat, enabled and wait time.
    pub easy_chat: A3954EasyChat,
    /// Adaptive sound-leak compensation (`10 9A` write).
    pub sound_leak_compensation: bool,
    /// The case display language at body 157, written back through
    /// [`A3954SettingCommand::CaseLanguage`].
    pub case_language: A3954CaseLanguage,
    /// In-ear wearing detection.
    pub wearing_detection: bool,
    /// The four slide-button action bytes at body 161..165, in
    /// [`A3954Button::ALL`] order after the main block. See
    /// [`Self::button_action_byte`].
    pub slide_buttons: [u8; A3954_SLIDE_BUTTON_COUNT],
}

impl A3954State {
    /// The raw action byte of one of the twelve gestures.
    ///
    /// The eight press gestures come from the main block at body 116..124 and
    /// the four slides from the block at 161..165, both in
    /// [`A3954Button::ALL`] order. The byte is the measured TwsLowBits pair
    /// `(non_tws << 4) | tws`; [`A3954ButtonActionByte`] decodes both nibbles
    /// and keeps unknown values.
    pub fn button_action_byte(&self, button: A3954Button) -> A3954ButtonActionByte {
        let index = button.index();
        let byte = if button.is_slide() {
            self.slide_buttons[index - A3954_MAIN_BUTTON_COUNT]
        } else {
            self.main_buttons[index]
        };
        A3954ButtonActionByte::from_byte(byte)
    }

    /// All twelve raw action bytes, in [`A3954Button::ALL`] order.
    pub fn button_action_bytes(&self) -> [A3954ButtonActionByte; 12] {
        A3954Button::ALL.map(|button| self.button_action_byte(button))
    }
}

/// One A3954 settings write: a validated value plus the exact measured command
/// and body that carry it to a Liberty 4 Pro.
///
/// Every variant names the command OpenSCL4's A3954 module uses and the app was
/// measured sending; turn one into a complete outbound frame with
/// [`DeviceModel::a3954_setting_frame`]. Button remapping has its own command
/// and frame builder ([`DeviceModel::a3954_button_frame`]); air-pressure writes
/// and the case display's brightness/color/style/text stay deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A3954SettingCommand {
    /// `01 FF` body `[on]`; the device confirms with an `01 7F` notification.
    Ldac(bool),
    /// `0B 84` body `[on]`. The `0B 01` list and the `0B 82`/`0B 81`/`0B 83`
    /// connect, disconnect and forget writes are the existing dual-connections
    /// helpers.
    DualConnections(bool),
    /// `01 86` body `[enabled, index]`.
    AutoPowerOff(A3954AutoPowerOff),
    /// `20 82` body `[enabled, dB]`.
    LimitHighVolume { enabled: bool, db_limit: DbLimit },
    /// `20 81` body `[0/1/2]`.
    LimitHighVolumeRefreshRate(DecibelReadingRefreshRate),
    /// `10 82` body `[on]`.
    LowBatteryPrompt(bool),
    /// `06 82` body `[bits]`.
    AmbientSoundModeCycle(AmbientSoundModeCycle),
    /// `01 81` body `[on]`.
    WearingDetection(bool),
    /// `10 9A` body `[on]`: the A3954-specific sound-leak command the app
    /// sends, not the common `10 86` one.
    SoundLeakCompensation(bool),
    /// `10 81` body `[enabled, music_mode, mode]`.
    SpatialAudio(A3954SpatialAudio),
    /// `10 9D` body `[enabled, wait_time]`.
    EasyChat(A3954EasyChat),
    /// `07 87` body `[remote_camera, find_device, case_spatial_audio,
    /// atmospheric]`, the four measured flags in the Soundcore app's command
    /// order (the state reports a different order; see
    /// [`A3954CaseFeatures::command_body`]).
    CaseFeatures(A3954CaseFeatures),
    /// `07 8A` body `[language]`; see [`A3954CaseLanguage::command_body`].
    CaseLanguage(A3954CaseLanguage),
}

impl A3954SettingCommand {
    /// The measured command that carries this setting.
    pub const fn command(self) -> Command {
        match self {
            Self::Ldac(_) => Command::new(0x01, 0xFF),
            Self::DualConnections(_) => Command::new(0x0B, 0x84),
            Self::AutoPowerOff(_) => Command::new(0x01, 0x86),
            Self::LimitHighVolume { .. } => Command::new(0x20, 0x82),
            Self::LimitHighVolumeRefreshRate(_) => Command::new(0x20, 0x81),
            Self::LowBatteryPrompt(_) => Command::new(0x10, 0x82),
            Self::AmbientSoundModeCycle(_) => Command::new(0x06, 0x82),
            Self::WearingDetection(_) => Command::new(0x01, 0x81),
            Self::SoundLeakCompensation(_) => Command::new(0x10, 0x9A),
            Self::SpatialAudio(_) => Command::new(0x10, 0x81),
            Self::EasyChat(_) => Command::new(0x10, 0x9D),
            Self::CaseFeatures(_) => Command::new(0x07, 0x87),
            Self::CaseLanguage(_) => Command::new(0x07, 0x8A),
        }
    }

    /// The exact measured outbound body for this setting.
    pub fn body(self) -> Vec<u8> {
        match self {
            Self::Ldac(on)
            | Self::DualConnections(on)
            | Self::LowBatteryPrompt(on)
            | Self::WearingDetection(on)
            | Self::SoundLeakCompensation(on) => vec![on.into()],
            Self::AutoPowerOff(auto_power_off) => vec![
                auto_power_off.enabled.into(),
                auto_power_off.duration.index(),
            ],
            Self::LimitHighVolume { enabled, db_limit } => vec![enabled.into(), db_limit.value()],
            Self::LimitHighVolumeRefreshRate(refresh_rate) => vec![refresh_rate.value()],
            Self::AmbientSoundModeCycle(cycle) => vec![cycle.byte()],
            Self::SpatialAudio(spatial_audio) => spatial_audio.command_body().to_vec(),
            Self::EasyChat(easy_chat) => easy_chat.command_body().to_vec(),
            Self::CaseFeatures(case_features) => case_features.command_body().to_vec(),
            Self::CaseLanguage(case_language) => case_language.command_body().to_vec(),
        }
    }
}

/// The dual-connections `0B` command group of the A3062.
///
/// The command bytes and layout are OpenSCL4's
/// `common/packet/{inbound/dual_connections_device,outbound/dual_connections}.rs`
/// for the A3062 module: an empty-body `0B 01` asks for the list of remembered
/// devices, and `0B 82`/`0B 81`/`0B 83` connect, disconnect and forget the
/// device whose six address bytes follow.
pub const DUAL_CONNECTIONS_LIST: Command = Command::new(0x0B, 0x01);

/// The `0B 82` connect command.
const DUAL_CONNECTIONS_CONNECT: Command = Command::new(0x0B, 0x82);
/// The `0B 81` disconnect command.
const DUAL_CONNECTIONS_DISCONNECT: Command = Command::new(0x0B, 0x81);
/// The `0B 83` forget command.
const DUAL_CONNECTIONS_FORGET: Command = Command::new(0x0B, 0x83);

/// The `0B 01` list request: an empty body, so the frame is ten bytes.
pub fn dual_connections_request() -> Vec<u8> {
    encode(Direction::Outbound, DUAL_CONNECTIONS_LIST, &[])
}

/// The exact `0B 82` connect frame for a device's six address bytes.
///
/// The bytes travel in the order the `0B 01` reply reported them: this is not
/// the byte-reversed Windows Bluetooth address representation.
pub fn dual_connections_connect_frame(mac: [u8; 6]) -> Vec<u8> {
    encode(Direction::Outbound, DUAL_CONNECTIONS_CONNECT, &mac)
}

/// The exact `0B 81` disconnect frame; the address bytes travel as the reply
/// reported them, like [`dual_connections_connect_frame`].
pub fn dual_connections_disconnect_frame(mac: [u8; 6]) -> Vec<u8> {
    encode(Direction::Outbound, DUAL_CONNECTIONS_DISCONNECT, &mac)
}

/// The exact `0B 83` forget frame; the address bytes travel as the reply
/// reported them, like [`dual_connections_connect_frame`].
pub fn dual_connections_forget_frame(mac: [u8; 6]) -> Vec<u8> {
    encode(Direction::Outbound, DUAL_CONNECTIONS_FORGET, &mac)
}

/// One device of the dual-connections list: a name the earbuds remember and
/// whether it is currently connected.
///
/// The address bytes are kept exactly as the reply sent them (six bytes, reply
/// order) so a connect/disconnect/forget write can send them back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualConnectionDevice {
    /// The name with its NUL padding removed, and with an incomplete trailing
    /// UTF-8 character dropped; see [`parse_dual_connections_body`].
    pub name: String,
    /// The six Bluetooth address bytes in reply order, never byte-reversed.
    pub mac: [u8; 6],
    /// Whether the device is connected now (`is_connected` byte exactly `1`;
    /// every other value reads as not connected).
    pub connected: bool,
}

/// One `0B 01` reply frame.
///
/// A reply spans one or more frames: `total` counts the frames, `current` is
/// the one-based index of this frame, and there is no terminator. Several
/// devices can share one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualConnectionsFrame {
    /// The number of frames in the whole reply, at least 1.
    pub total: u8,
    /// This frame's one-based index, in `1..=total`.
    pub current: u8,
    /// The devices carried by this frame, in reply order.
    pub devices: Vec<DualConnectionDevice>,
}

/// Why a `0B 01` reply frame could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DualConnectionsError {
    /// The frame is not an inbound `0B 01` frame.
    NotADualConnectionsFrame,
    /// The body is shorter than its frame-count prefix.
    ShortBody { actual: usize, minimum: usize },
    /// `total` or `current` is outside the measured `1..=total` range.
    BadFrameCount { total: u8, current: u8 },
    /// A device record claims a length below its fixed eight bytes.
    BadDeviceLength { offset: usize, length: u8 },
    /// A device record is cut off before its fixed eight bytes.
    ShortDevice { offset: usize, remaining: usize },
}

impl std::fmt::Display for DualConnectionsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotADualConnectionsFrame => {
                write!(formatter, "frame is not a dual-connections 0B 01 response")
            }
            Self::ShortBody { actual, minimum } => {
                write!(formatter, "body is {actual} bytes, need at least {minimum}")
            }
            Self::BadFrameCount { total, current } => {
                write!(
                    formatter,
                    "bad dual-connections frame count {current}/{total}"
                )
            }
            Self::BadDeviceLength { offset, length } => {
                write!(
                    formatter,
                    "device at body offset {offset} has length {length}, need at least 8"
                )
            }
            Self::ShortDevice { offset, remaining } => {
                write!(
                    formatter,
                    "device at body offset {offset} needs 8 bytes, only {remaining} remain"
                )
            }
        }
    }
}

impl std::error::Error for DualConnectionsError {}

/// Parse one complete inbound `0B 01` frame into a [`DualConnectionsFrame`].
pub fn parse_dual_connections_frame(
    raw: &[u8],
) -> Result<DualConnectionsFrame, DualConnectionsError> {
    let frame: Frame = parse(raw).map_err(|_| DualConnectionsError::NotADualConnectionsFrame)?;
    if frame.command != DUAL_CONNECTIONS_LIST {
        return Err(DualConnectionsError::NotADualConnectionsFrame);
    }
    parse_dual_connections_body(&frame.body)
}

/// Parse the body of one `0B 01` reply frame.
///
/// The body is `[total, current, devices...]`. A device is its own length byte
/// (`0x28` = eight fixed bytes plus a 32-byte name field), the `is_connected`
/// flag, the six address bytes as-is and `length - 8` name bytes right-padded
/// with NULs. There is no terminator: the reply ends at `current == total`.
///
/// A name is cut at its first NUL and decoded as UTF-8. The device pads the
/// name field to 32 bytes and can cut a multibyte character in half; that
/// keeps the valid prefix instead of dropping the whole list. A final record
/// padded shorter than its declared length is clamped to what is present, as
/// observed on a real device in OpenSCL4.
pub fn parse_dual_connections_body(
    body: &[u8],
) -> Result<DualConnectionsFrame, DualConnectionsError> {
    if body.len() < 2 {
        return Err(DualConnectionsError::ShortBody {
            actual: body.len(),
            minimum: 2,
        });
    }
    let (total, current) = (body[0], body[1]);
    if total == 0 || current == 0 || current > total {
        return Err(DualConnectionsError::BadFrameCount { total, current });
    }

    let mut devices = Vec::new();
    let mut offset = 2;
    while offset < body.len() {
        let length = body[offset];
        if length < 8 {
            return Err(DualConnectionsError::BadDeviceLength { offset, length });
        }
        let remaining = body.len() - offset;
        if remaining < 8 {
            return Err(DualConnectionsError::ShortDevice { offset, remaining });
        }
        let connected = body[offset + 1] == 1;
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&body[offset + 2..offset + 8]);

        let name_start = offset + 8;
        let declared_name_length = usize::from(length - 8);
        let available = body.len() - name_start;
        let name_length = declared_name_length.min(available);
        let name = dual_connection_name(&body[name_start..name_start + name_length]);

        devices.push(DualConnectionDevice {
            name,
            mac,
            connected,
        });
        offset = name_start + name_length;
    }

    Ok(DualConnectionsFrame {
        total,
        current,
        devices,
    })
}

/// A device name: the bytes before the first NUL, decoded as UTF-8 with an
/// incomplete trailing character dropped.
fn dual_connection_name(bytes: &[u8]) -> String {
    let bytes = bytes
        .iter()
        .position(|byte| *byte == 0)
        .map_or(bytes, |nul| &bytes[..nul]);
    match std::str::from_utf8(bytes) {
        Ok(name) => name.to_owned(),
        Err(error) => String::from_utf8_lossy(&bytes[..error.valid_up_to()]).into_owned(),
    }
}

/// The devices collected from the frames of one `0B 01` reply.
///
/// `current == 1` starts a new list, every frame appends its devices (so an
/// out-of-order reply still parses) and the reply completes on the frame whose
/// `current == total`: there is no terminator byte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DualConnectionsList {
    devices: Vec<DualConnectionDevice>,
}

impl DualConnectionsList {
    /// An empty list, ready for the first reply frame.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one frame's devices and report whether it was the last frame.
    pub fn push_frame(&mut self, frame: DualConnectionsFrame) -> bool {
        if frame.current == 1 {
            self.devices.clear();
        }
        self.devices.extend(frame.devices);
        frame.current == frame.total
    }

    /// The devices collected so far, in reply order.
    pub fn devices(&self) -> &[DualConnectionDevice] {
        &self.devices
    }

    /// The complete list, in reply order.
    pub fn into_devices(self) -> Vec<DualConnectionDevice> {
        self.devices
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
    /// The A3954-specific state; `None` for every model that is not an A3954.
    ///
    /// The A3954 shares the TWS Cmm2 prefix with the A3949, so its battery and
    /// identity fields are in [`Self::battery`], [`Self::firmware`] and
    /// [`Self::serial`]; everything the A3954 adds beyond that prefix is here.
    pub a3954: Option<A3954State>,
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
    /// The measured A3954-specific state blocks, or `None` for every other
    /// model. A model with this capability parses [`DeviceState::a3954`] and
    /// builds the A3954 `06 81`, `03 87`, `04 81` and setting frames.
    pub a3954: Option<A3954Capability>,
    /// Whether this slice implements the model's dual-connections list and its
    /// connect/disconnect/forget writes.
    ///
    /// Only the A3062 was measured (OpenSCL4 uses the same commands for every
    /// model that has the feature). The A3949's dual support stays deferred, so
    /// its session list read and writes fail as unsupported and nothing is
    /// written.
    pub dual_connections: bool,
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

    /// The exact five-byte `06 81` body for an A3954 sound mode, or `None` when
    /// this model has no measured A3954 state blocks.
    ///
    /// The body is `[mode, position, airplane, wind, 0xFF]`; see
    /// [`A3954SoundMode::command_body`]. This is the A3954 scale write, which is
    /// not the three-mode [`Self::sound_mode_body`] used by the A3062.
    pub fn a3954_sound_mode_body(
        self,
        sound_mode: A3954SoundMode,
    ) -> Option<[u8; A3954_SOUND_MODE_COMMAND_BODY_LEN]> {
        self.a3954?;
        Some(sound_mode.command_body())
    }

    /// The built-in equalizer presets of this model, in display order; empty
    /// when this slice does not implement the model's A3062-shaped equalizer
    /// (the deliberately deferred A3949, and the A3954, whose equalizer has its
    /// own 114-byte body and catalog; see [`Self::a3954_equalizer_presets`]).
    pub fn equalizer_presets(self) -> &'static [EqualizerPreset] {
        if self.code == A3062.code {
            A3062_EQUALIZER_PRESETS
        } else {
            &[]
        }
    }

    /// The built-in equalizer presets of the A3954's twenty-two-preset
    /// catalog, in display order; empty for every other model.
    ///
    /// This is separate from [`Self::equalizer_presets`] because the two
    /// models' equalizers have different command shapes: the A3954 write body
    /// is 114 bytes ([`Self::a3954_equalizer_preset_body`]) while the A3062
    /// one is 53.
    pub fn a3954_equalizer_presets(self) -> &'static [EqualizerPreset] {
        if self.a3954.is_some() {
            A3954_EQUALIZER_PRESETS
        } else {
            &[]
        }
    }

    /// The full 114-byte A3954 `03 87` body that selects `preset` on both
    /// channels while keeping the device's current HearID block, or `None`
    /// when this model has no measured A3954 state blocks.
    ///
    /// | bytes | field |
    /// |---:|---|
    /// | 0..2 | preset id, little-endian |
    /// | 2..4 | HearID favorite music genre, from the state |
    /// | 4..14 | the preset's ten band bytes on the left channel |
    /// | 14..24 | the same ten band bytes on the right channel |
    /// | 24..26 | unknown, the measured app bytes `FF FF` |
    /// | 26 | HearID enabled: always `0` for an equalizer write |
    /// | 27..37 | HearID left volume adjustments, from the state |
    /// | 37..47 | HearID right volume adjustments, from the state |
    /// | 47..51 | HearID time, big-endian, from the state |
    /// | 51 | HearID type, from the state |
    /// | 52..62 | HearID custom left volume adjustments, from the state |
    /// | 62..72 | HearID custom right volume adjustments, from the state |
    /// | 72..92 | the active left curve, each band byte followed by `0` |
    /// | 92..112 | the active right curve, each band byte followed by `0` |
    /// | 112..114 | unknown, zero |
    ///
    /// The HearID fields are copied byte for byte and its enabled flag is
    /// cleared, so the selected curve is in effect, exactly as OpenSCL4's
    /// A3954 equalizer does. The app's measured bodies carry `FF FF` at bytes
    /// 24..26; OpenSCL4's builder carries `00 00` there and that variant was
    /// never sent, so the measured bytes win.
    pub fn a3954_equalizer_preset_body(
        self,
        current: &A3954EqualizerState,
        preset: &EqualizerPreset,
    ) -> Option<[u8; A3954_EQUALIZER_COMMAND_BODY_LEN]> {
        self.a3954?;
        Some(a3954_equalizer_body(current, preset.id, preset.bands))
    }

    /// The full 114-byte A3954 `03 87` body that applies a custom curve on both
    /// channels while keeping the device's current HearID block, or `None` when
    /// this model has no measured A3954 state blocks.
    ///
    /// `visible_bands` are the eight bands the Equalizer page shows, in raw
    /// dB×10 units (`0` is `0 dB`, `-120` is `-12.0 dB`, `134` is `+13.4 dB`),
    /// clamped to the measured `-120..=134` range. The two invisible bands are
    /// fixed at [`A3954_FIXED_EQ_BANDS`], so a caller cannot select them. The
    /// body has the same 114-byte shape as
    /// [`Self::a3954_equalizer_preset_body`], with
    /// [`CUSTOM_EQUALIZER_PRESET_ID`] as the preset id.
    pub fn a3954_custom_equalizer_body(
        self,
        current: &A3954EqualizerState,
        visible_bands: [i16; VISIBLE_EQ_BANDS],
    ) -> Option<[u8; A3954_EQUALIZER_COMMAND_BODY_LEN]> {
        self.a3954?;
        let mut bands = [0i16; A3954_EQ_BANDS];
        bands[..VISIBLE_EQ_BANDS].copy_from_slice(&visible_bands);
        bands[VISIBLE_EQ_BANDS..].copy_from_slice(&A3954_FIXED_EQ_BANDS);
        Some(a3954_equalizer_body(
            current,
            CUSTOM_EQUALIZER_PRESET_ID,
            bands,
        ))
    }

    /// The exact outbound frame for one A3954 settings write, or `None` when
    /// this model has no measured A3954 state blocks.
    ///
    /// The frame is built in one step from [`A3954SettingCommand::command`] and
    /// [`A3954SettingCommand::body`]; a caller writes it once and re-polls
    /// `01 01`, because the app does not wait for the empty same-command
    /// acknowledgement.
    pub fn a3954_setting_frame(self, setting: A3954SettingCommand) -> Option<Vec<u8>> {
        self.a3954?;
        Some(encode(
            Direction::Outbound,
            setting.command(),
            &setting.body(),
        ))
    }

    /// The exact `04 81` frame for one A3954 gesture, or `None` when this model
    /// has no measured A3954 state blocks.
    ///
    /// The body is the measured three bytes `[side, button_id, action]`; see
    /// [`A3954Button::command_body`]. The action is the whole TwsLowBits byte
    /// ([`A3954ButtonActionByte`]), so a caller keeps the nibble it is not
    /// editing and can restore exactly what the device reported. The A3954 has
    /// no set-all button packet: one call is one gesture and one frame.
    pub fn a3954_button_frame(
        self,
        button: A3954Button,
        action: A3954ButtonActionByte,
    ) -> Option<Vec<u8>> {
        self.a3954?;
        Some(encode(
            Direction::Outbound,
            A3954_BUTTON_CONFIGURATION,
            &button.command_body(action),
        ))
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
    /// implement the model's A3062-shaped equalizer (the deferred A3949, or the
    /// A3954, which has its own 114-byte body; see
    /// [`Self::a3954_custom_equalizer_body`]).
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

/// Builds the measured 114-byte A3954 `03 87` body for an arbitrary preset id
/// and two ten-band raw curves; see [`DeviceModel::a3954_equalizer_preset_body`]
/// for the layout.
fn a3954_equalizer_body(
    current: &A3954EqualizerState,
    preset_id: u16,
    bands: [i16; A3954_EQ_BANDS],
) -> [u8; A3954_EQUALIZER_COMMAND_BODY_LEN] {
    let band_bytes = bands.map(band_byte);
    let mut body = [0u8; A3954_EQUALIZER_COMMAND_BODY_LEN];
    body[0..2].copy_from_slice(&preset_id.to_le_bytes());
    body[2..4].copy_from_slice(&current.hear_id[46..48]);
    body[4..14].copy_from_slice(&band_bytes);
    body[14..24].copy_from_slice(&band_bytes);
    body[24..26].copy_from_slice(&A3954_EQUALIZER_UNKNOWN_BODY_BYTES);
    // body[26] is the cleared HearID enabled flag.
    body[27..37].copy_from_slice(&current.hear_id[1..11]);
    body[37..47].copy_from_slice(&current.hear_id[11..21]);
    body[47..51].copy_from_slice(&current.hear_id[21..25]);
    body[51] = current.hear_id[25];
    body[52..62].copy_from_slice(&current.hear_id[26..36]);
    body[62..72].copy_from_slice(&current.hear_id[36..46]);
    for (index, byte) in band_bytes.iter().enumerate() {
        body[72 + index * 2] = *byte;
        body[92 + index * 2] = *byte;
    }
    // body[112..114] are the two trailing unknown zeroes.
    body
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
    a3954: None,
    dual_connections: false,
};

pub const A3947: DeviceModel = DeviceModel {
    code: "A3947",
    name: "Soundcore Liberty 4 NC",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
    a3954: None,
    dual_connections: false,
};

pub const A3954: DeviceModel = DeviceModel {
    code: "A3954",
    name: "Soundcore Liberty 4 Pro",
    battery_scale: BatteryScale::Hundred,
    layout: Layout::TwsCmm2,
    // The A3954's ambient sound is one 1..=11 scale, not the three measured
    // A3062 modes, so it has no `SoundModeCapability`; its scale lives in
    // `A3954State::sound_mode` and writes through `a3954_sound_mode_body`.
    sound_mode: None,
    // The A3954 equalizer block (body 44..115) has a 114-byte write body of its
    // own, not the A3062 53-byte shape, so it has no `EqualizerCapability`.
    equalizer: None,
    // The measured A3954 settings use their own commands and durations, not the
    // A3062 `SettingCommand` block.
    settings: None,
    // Measured from the real Liberty 4 Pro capture on 2026-11-04: the
    // equalizer preset id is body 44..46 and the sound-mode block is body
    // 125..129.
    a3954: Some(A3954Capability {
        sound_mode_offset: 125,
        equalizer_offset: 44,
    }),
    // The app's `0B 84` toggle and the `0B 01` list use the existing
    // dual-connections helpers; the state also reports the feature at body 142.
    dual_connections: true,
};

pub const A3035: DeviceModel = DeviceModel {
    code: "A3035",
    name: "Soundcore Space One",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
    a3954: None,
    dual_connections: false,
};

pub const A3040: DeviceModel = DeviceModel {
    code: "A3040",
    name: "Soundcore Space Q45",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
    sound_mode: None,
    equalizer: None,
    settings: None,
    a3954: None,
    dual_connections: false,
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
    a3954: None,
    // OpenSCL4 enables the dual-connections module for A3062; the list and the
    // connect/disconnect/forget writes were captured on the real device in this
    // ticket (2026-11-21).
    dual_connections: true,
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
    InvalidA3954AutoPowerOffIndex(u8),
    InvalidA3954SpatialAudioMusicMode(u8),
    InvalidA3954SpatialAudioMode(u8),
    InvalidA3954EasyChatWaitTime(u8),
    InvalidA3954CaseLanguage(u8),
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
            Self::InvalidA3954AutoPowerOffIndex(value) => {
                write!(f, "invalid A3954 auto power off duration index {value}")
            }
            Self::InvalidA3954SpatialAudioMusicMode(value) => {
                write!(f, "invalid A3954 spatial audio music mode {value}")
            }
            Self::InvalidA3954SpatialAudioMode(value) => {
                write!(f, "invalid A3954 spatial audio mode {value}")
            }
            Self::InvalidA3954EasyChatWaitTime(value) => {
                write!(f, "invalid A3954 easy-chat wait time {value}")
            }
            Self::InvalidA3954CaseLanguage(value) => {
                write!(f, "invalid A3954 case language {value}")
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
        a3954: parse_a3954_state(model, body)?,
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
        // The A3954 is the only measured TWS-layout model so far; every other
        // TWS model has no A3954 capability and stays `None`.
        a3954: None,
        firmware: FirmwareVersions {
            left: None,
            right: None,
        },
        serial: parse_serial(&body[7..23]),
    })
}

/// Parses the A3954 sound-mode block at body 125..129.
///
/// `None` when any of the three closed values is one this slice has not
/// measured: the real device reports slider byte `13` in airplane mode, so the
/// block is unavailable rather than the whole state poll failing. The wind bits
/// are flags and only the slider/mode/airplane bytes can make the block
/// unavailable.
fn parse_a3954_sound_mode(body: &[u8], offset: usize) -> Option<A3954SoundMode> {
    Some(A3954SoundMode {
        mode: A3954AmbientSoundMode::from_value(body[offset])?,
        slider: A3954SoundModeSlider::new(body[offset + 1])?,
        airplane_mode: A3954AirplaneMode::from_value(body[offset + 2])?,
        wind_noise_suppression: body[offset + 3] & 1 != 0,
        wind_noise_detected: body[offset + 3] & 2 != 0,
    })
}

/// Parses the A3954-specific blocks of the measured 165-byte `01 01` body.
///
/// `None` when the model has no [`A3954Capability`], which is every model but
/// the A3954. The base TWS fields (`0..32`) are parsed by the caller; this
/// parser owns everything from the case firmware at body 32 through the four
/// slide-button bytes at 161..165. A body shorter than
/// [`A3954_STATE_BODY_LEN`] is an error: the parser does not invent offsets.
///
/// Every closed enum is validated instead of defaulted, so firmware that
/// reports a value this slice was not measured with fails the poll instead of
/// looking like a known setting. The sound-mode block is the one measured
/// exception: the real device reports the unmeasured slider byte `13` in
/// airplane mode, so that block reads as [`A3954State::sound_mode`] `None` and
/// the rest of the state still parses. The button action bytes are kept raw by
/// design: [`A3954ButtonActionValue`] represents unmeasured nibbles instead of
/// failing the poll, so a state value is never lost. Flags are on only when the
/// byte is `1`, as in OpenSCL4's parser, so an unexpected flag byte reads as
/// off.
fn parse_a3954_state(model: DeviceModel, body: &[u8]) -> Result<Option<A3954State>, StateError> {
    let Some(capability) = model.a3954 else {
        return Ok(None);
    };
    if body.len() < A3954_STATE_BODY_LEN {
        return Err(StateError::ShortBody {
            actual: body.len(),
            minimum: A3954_STATE_BODY_LEN,
        });
    }

    let equalizer_offset = capability.equalizer_offset;
    let bands_offset = equalizer_offset + 2;
    let hear_id_offset = bands_offset + A3954_EQ_BANDS * 2 + 1;
    let preset_id = u16::from_le_bytes([body[equalizer_offset], body[equalizer_offset + 1]]);
    let left = parse_a3954_bands(&body[bands_offset..bands_offset + A3954_EQ_BANDS]);
    let right = parse_a3954_bands(&body[bands_offset + A3954_EQ_BANDS..hear_id_offset - 1]);
    let mut hear_id = [0u8; A3954_HEAR_ID_BLOCK_LEN];
    hear_id.copy_from_slice(&body[hear_id_offset..hear_id_offset + A3954_HEAR_ID_BLOCK_LEN]);

    let sound_mode = parse_a3954_sound_mode(body, capability.sound_mode_offset);

    let duration_index = body[A3954_AUTO_POWER_OFF_OFFSET + 1];
    let auto_power_off = A3954AutoPowerOff {
        enabled: a3954_flag(body, A3954_AUTO_POWER_OFF_OFFSET),
        duration: A3954AutoPowerOffDuration::from_index(duration_index)
            .ok_or(StateError::InvalidA3954AutoPowerOffIndex(duration_index))?,
    };

    let db_limit = DbLimit::new(body[A3954_LIMIT_HIGH_VOLUME_OFFSET + 1]).ok_or(
        StateError::InvalidLimitHighVolumeDbLimit(body[A3954_LIMIT_HIGH_VOLUME_OFFSET + 1]),
    )?;
    let refresh_rate =
        DecibelReadingRefreshRate::from_value(body[A3954_LIMIT_HIGH_VOLUME_OFFSET + 2]).ok_or(
            StateError::InvalidDecibelReadingRefreshRate(body[A3954_LIMIT_HIGH_VOLUME_OFFSET + 2]),
        )?;

    let spatial_audio = A3954SpatialAudio {
        enabled: a3954_flag(body, A3954_SPATIAL_AUDIO_OFFSET),
        music_mode: A3954SpatialAudioMusicMode::from_state_value(
            body[A3954_SPATIAL_AUDIO_OFFSET + 1],
        )
        .ok_or(StateError::InvalidA3954SpatialAudioMusicMode(
            body[A3954_SPATIAL_AUDIO_OFFSET + 1],
        ))?,
        mode: A3954SpatialAudioMode::from_value(body[A3954_SPATIAL_AUDIO_OFFSET + 2]).ok_or(
            StateError::InvalidA3954SpatialAudioMode(body[A3954_SPATIAL_AUDIO_OFFSET + 2]),
        )?,
    };

    let wait_time_value = body[A3954_EASY_CHAT_WAIT_TIME_OFFSET];
    let easy_chat = A3954EasyChat {
        enabled: a3954_flag(body, A3954_EASY_CHAT_ENABLED_OFFSET),
        wait_time: A3954EasyChatWaitTime::from_value(wait_time_value)
            .ok_or(StateError::InvalidA3954EasyChatWaitTime(wait_time_value))?,
    };

    let case_language_value = body[A3954_CASE_LANGUAGE_OFFSET];
    let case_language = A3954CaseLanguage::from_value(case_language_value)
        .ok_or(StateError::InvalidA3954CaseLanguage(case_language_value))?;

    let case_battery_level = match body[A3954_CASE_BATTERY_OFFSET] {
        255 => None,
        level => Some(level),
    };

    Ok(Some(A3954State {
        case_firmware: parse_firmware(&body[A3954_CASE_FIRMWARE_OFFSET..A3954_CASE_BATTERY_OFFSET]),
        case_battery_level,
        case_serial: parse_case_serial(
            &body[A3954_CASE_SERIAL_OFFSET..A3954_CASE_SERIAL_OFFSET + 6],
        ),
        equalizer: A3954EqualizerState {
            preset_id,
            left,
            right,
            hear_id,
        },
        main_buttons: std::array::from_fn(|index| body[A3954_MAIN_BUTTONS_OFFSET + index]),
        ambient_sound_mode_cycle: AmbientSoundModeCycle::from_byte(
            body[A3954_AMBIENT_SOUND_MODE_CYCLE_OFFSET],
        ),
        sound_mode,
        case_features: A3954CaseFeatures {
            atmospheric: a3954_flag(body, A3954_CASE_FEATURES_OFFSET),
            remote_camera: a3954_flag(body, A3954_CASE_FEATURES_OFFSET + 1),
            find_device: a3954_flag(body, A3954_CASE_FEATURES_OFFSET + 2),
            case_spatial_audio: a3954_flag(body, A3954_CASE_FEATURES_OFFSET + 3),
        },
        air_pressure: body[A3954_AIR_PRESSURE_OFFSET],
        low_battery_prompt: a3954_flag(body, A3954_LOW_BATTERY_PROMPT_OFFSET),
        ldac: a3954_flag(body, A3954_LDAC_OFFSET),
        dual_connections: a3954_flag(body, A3954_DUAL_CONNECTIONS_OFFSET),
        auto_power_off,
        limit_high_volume: LimitHighVolume {
            enabled: a3954_flag(body, A3954_LIMIT_HIGH_VOLUME_OFFSET),
            db_limit,
            refresh_rate,
        },
        spatial_audio,
        easy_chat,
        sound_leak_compensation: a3954_flag(body, A3954_SOUND_LEAK_COMPENSATION_OFFSET),
        case_language,
        wearing_detection: a3954_flag(body, A3954_WEARING_DETECTION_OFFSET),
        slide_buttons: std::array::from_fn(|index| body[A3954_SLIDE_BUTTONS_OFFSET + index]),
    }))
}

/// One A3954 flag byte: on only when it is exactly `1`.
fn a3954_flag(body: &[u8], offset: usize) -> bool {
    body[offset] == 1
}

/// One A3954 equalizer channel's ten raw band values, or `None` when the
/// channel is absent.
///
/// The device reports an absent channel as ten `0xFF` bytes; only the first
/// byte decides, as in OpenSCL4's optional volume-adjustment parser, so the
/// whole channel is either present or absent.
fn parse_a3954_bands(bytes: &[u8]) -> Option<[i16; A3954_EQ_BANDS]> {
    if bytes.first() == Some(&0xFF) {
        return None;
    }
    Some(std::array::from_fn(|index| {
        i16::from(bytes[index]) - BAND_BYTE_OFFSET
    }))
}

/// Parses the A3954 six-byte case serial into the `3954` + hex form OpenSCL4
/// reports, or `None` when the bytes are zeroed in a masked capture.
fn parse_case_serial(bytes: &[u8]) -> Option<String> {
    use std::fmt::Write as _;

    if bytes.iter().all(|byte| *byte == 0) {
        return None;
    }
    let mut serial = String::with_capacity(16);
    serial.push_str("3954");
    for byte in bytes {
        write!(serial, "{byte:02X}").expect("writing to a string is infallible");
    }
    Some(serial)
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
