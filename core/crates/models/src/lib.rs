//! Device models and their state parsers.
//!
//! The first milestone covers the TWS Cmm2 layout that is confirmed by the
//! real A3949 capture. Single-battery layouts and the TLV D1203 layout are
//! deliberately unimplemented: returning `UnsupportedLayout` is better than
//! inventing offsets.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceState {
    pub host_device: HostDevice,
    pub tws_connected: bool,
    pub battery: BatteryState,
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
    if model.layout != Layout::TwsCmm2 {
        return Err(StateError::UnsupportedLayout(model.layout));
    }
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
        battery: BatteryState {
            left: body[2],
            right: body[3],
            left_charging: body[4] != 0,
            right_charging: body[5] != 0,
            scale,
        },
        firmware: FirmwareVersions {
            left: parse_firmware(&body[6..11]),
            right: parse_firmware(&body[11..16]),
        },
        serial: parse_serial(&body[16..32]),
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
