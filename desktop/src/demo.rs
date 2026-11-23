//! The simulated A3062 the Add Device dialog's Demo mode connects to.
//!
//! Demo mode exists so every page and control of the Windows client can be
//! exercised without Bluetooth hardware: the controller parses the committed,
//! sanitized A3062 capture in `core/data` into a real [`DeviceState`] and serves
//! it on the same event and command channel a device worker uses. Nothing in
//! this module opens RFCOMM, creates a `DeviceSession` or writes a device
//! command; the state changes live only in the demo worker's copy of the state.
//!
//! The identifiers are deliberately neutral: an all-zero address with product
//! suffix `1` for the headphones, and `Demo …` names for the remembered devices
//! of the Connections page. Nothing here is a real address or a real device
//! name, so a demo transcript can be shared safely.

use soundcore_models::{DeviceModel, DeviceState, DualConnectionDevice, parse_state_frame};

/// The address the simulated device answers on. It never leaves the process:
/// the demo card stores it and no transport or pairing call consumes it.
pub const DEMO_MAC: &str = "00:00:00:00:00:01";

/// The saved card name and device name of the simulated Space One Pro. The
/// `· Demo` suffix keeps it distinguishable from a real saved device.
pub const DEMO_NAME: &str = "soundcore Space One Pro · Demo";

/// The catalog code the simulated device is: the only model whose demo data the
/// committed capture supplies.
pub const DEMO_MODEL_CODE: &str = "A3062";

/// Shown in the Add Device dialog when Demo mode is switched on for a catalog
/// model whose state the capture does not describe.
pub const DEMO_UNAVAILABLE: &str = "Demo mode is available for Space One Pro";

/// Shown on the Diagnostics page after a demo connection: the page's probes are
/// never run against simulated data, so the Start capture button stays disabled.
pub const DEMO_DIAGNOSTICS_UNAVAILABLE: &str =
    "Demo mode cannot collect diagnostics. No probes are run and no archive is written.";

/// The demo Diagnostics explanation in the app language; the fallback is the
/// English constant above.
pub fn diagnostics_unavailable(language: &str) -> &'static str {
    if language.starts_with("ru") {
        "Деморежим не собирает диагностику. Пробы не запускаются, архив не создаётся."
    } else {
        DEMO_DIAGNOSTICS_UNAVAILABLE
    }
}

/// The neutral address bytes of the simulated remembered desktop, in the reply
/// order the Connections page's list uses.
pub const DEMO_DESKTOP_MAC: [u8; 6] = [0, 0, 0, 0, 0, 2];

/// The neutral address bytes of the simulated remembered phone.
pub const DEMO_PHONE_MAC: [u8; 6] = [0, 0, 0, 0, 0, 3];

/// The committed, sanitized A3062 capture the demo state is parsed from. Its
/// serial bytes are masked to zero, so the parsed state carries no serial and
/// nothing from it can leak into a transcript or an archive.
const DEMO_STATE_FIXTURE: &str = include_str!("../../core/data/a3062-state.json");

#[derive(serde::Deserialize)]
struct StateFixture {
    frame_hex: String,
}

/// The model the demo device is: the core's A3062 parser, whose `01 01` layout
/// the committed capture carries.
pub fn demo_model() -> DeviceModel {
    soundcore_models::model_by_code(DEMO_MODEL_CODE)
        .expect("the demo model is a model of the core catalog")
}

/// The simulated A3062 state, parsed from the committed sanitized capture with
/// the core's own parser. Every connect starts from this state; a demo write
/// changes only the demo worker's copy, so a reconnect starts fresh.
pub fn demo_state() -> DeviceState {
    let fixture: StateFixture = serde_json::from_str(DEMO_STATE_FIXTURE)
        .expect("the committed demo capture is a JSON document");
    let frame = hex_bytes(&fixture.frame_hex);
    parse_state_frame(demo_model(), &frame)
        .expect("the committed demo capture parses as A3062 state")
}

/// The remembered-device list the Connections page shows in demo mode: one
/// connected and one remembered device, with neutral addresses and names.
pub fn demo_dual_devices() -> Vec<DualConnectionDevice> {
    vec![
        DualConnectionDevice {
            name: "Demo desktop".to_owned(),
            mac: DEMO_DESKTOP_MAC,
            connected: true,
        },
        DualConnectionDevice {
            name: "Demo phone".to_owned(),
            mac: DEMO_PHONE_MAC,
            connected: false,
        },
    ]
}

/// Parses the whitespace-separated hex bytes of a committed capture.
fn hex_bytes(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("the capture stores hex bytes"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_windows::parse_mac;
    use soundcore_models::{BatteryScale, SoundMode};

    #[test]
    fn the_demo_diagnostics_message_follows_the_app_language() {
        assert!(diagnostics_unavailable("ru").contains("Деморежим"));
        assert_eq!(diagnostics_unavailable("en"), DEMO_DIAGNOSTICS_UNAVAILABLE);
    }

    #[test]
    fn the_committed_capture_parses_into_the_demo_state() {
        let state = demo_state();

        // The capture is the measured A3062 layout: one battery on the ten-step
        // scale, one firmware string, the masked serial, and the measured blocks.
        let single = state
            .single_battery
            .as_ref()
            .expect("A3062 is a single-battery layout");
        assert_eq!(single.value, 2);
        assert_eq!(single.scale, BatteryScale::Ten);
        assert_eq!(single.firmware.as_deref(), Some("04.39"));
        assert_eq!(state.serial, None, "the capture's serial stays masked");
        assert!(state.battery.is_none());
        assert!(!state.tws_connected);
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert!(state.equalizer.is_some());
        let settings = state
            .settings
            .expect("the capture reports the settings block");
        assert!(
            settings.dual_connections,
            "the capture reports dual connections on"
        );
        assert!(!settings.ldac, "the capture reports LDAC off");
        assert!(settings.voice_prompt);
        assert_eq!(
            demo_model(),
            soundcore_models::model_by_code("A3062").unwrap()
        );
    }

    #[test]
    fn the_demo_identifiers_are_neutral_and_deterministic() {
        // The address parses and is not a real-looking device address: five zero
        // bytes and the product suffix.
        assert_eq!(parse_mac(DEMO_MAC), Some(1));
        assert_eq!(DEMO_MAC, "00:00:00:00:00:01");
        assert!(DEMO_NAME.to_lowercase().contains("demo"));
        assert_eq!(demo_model().code, DEMO_MODEL_CODE);

        let devices = demo_dual_devices();
        assert_eq!(devices, demo_dual_devices(), "the list is deterministic");
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|device| device.connected));
        for device in &devices {
            assert!(device.name.starts_with("Demo"));
            assert_eq!(&device.mac[..5], &[0; 5], "no real address bytes");
            assert_ne!(0, device.mac[5], "each neutral address has a suffix");
        }
        assert_ne!(
            devices[0].mac, devices[1].mac,
            "each remembered device has its own neutral address"
        );
        // No fixture address leaks into the simulated remembered devices.
        assert!(
            devices
                .iter()
                .all(|device| device.mac != [2, 0, 0, 0, 0, 1])
        );
        assert!(
            devices
                .iter()
                .all(|device| device.mac != [2, 0, 0, 0, 0, 2])
        );
    }
}
