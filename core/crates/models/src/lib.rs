//! Device models and their state parsers.
//!
//! Two state layouts are implemented from real captures:
//!
//! - the TWS Cmm2 layout, confirmed by the real A3949 capture (left/right
//!   batteries, two firmware strings, serial);
//! - the single-battery Cmm2 layout, measured on the real A3062 (one battery on
//!   the ten-step scale, one firmware string, serial).
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
}

impl DeviceModel {
    pub const fn is_tws(self) -> bool {
        matches!(self.layout, Layout::TwsCmm2)
    }
}

pub const A3949: DeviceModel = DeviceModel {
    code: "A3949",
    name: "Soundcore P20i / P25i / R50i",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
};

pub const A3947: DeviceModel = DeviceModel {
    code: "A3947",
    name: "Soundcore Liberty 4 NC",
    battery_scale: BatteryScale::Five,
    layout: Layout::TwsCmm2,
};

pub const A3954: DeviceModel = DeviceModel {
    code: "A3954",
    name: "Soundcore Liberty 4 Pro",
    battery_scale: BatteryScale::Hundred,
    layout: Layout::TwsCmm2,
};

pub const A3035: DeviceModel = DeviceModel {
    code: "A3035",
    name: "Soundcore Space One",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
};

pub const A3040: DeviceModel = DeviceModel {
    code: "A3040",
    name: "Soundcore Space Q45",
    battery_scale: BatteryScale::Five,
    layout: Layout::SingleCmm2,
};

pub const A3062: DeviceModel = DeviceModel {
    code: "A3062",
    name: "Soundcore Space One Pro",
    battery_scale: BatteryScale::Ten,
    layout: Layout::SingleCmm2,
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
///
/// Everything after offset 23 is model-specific and is not interpreted here.
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
        firmware: FirmwareVersions {
            left: None,
            right: None,
        },
        serial: parse_serial(&body[7..23]),
    })
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
