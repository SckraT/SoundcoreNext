//! The notification-area icon and the window lifecycle that goes with it.
//!
//! `ui/tray.slint` is the vendored Slint tray component: the OpenSCL4 icon, a
//! tooltip, and a menu built from a handful of properties. `build.rs` compiles
//! it as its own unit (it exports a `SystemTrayIcon` and no window, so Slint
//! creates no window adapter for it). [`TrayShell`] creates that component
//! before the window's `run`, keeps it alive for the whole run, and answers its
//! menu:
//!
//! - **Open** (the menu item and a left-click on the icon) shows and focuses the
//!   window again;
//! - **Quit** ends the event loop, after which the shell is dropped and the icon
//!   with it;
//! - **Connect**, the **Sound mode** submenu and the **LDAC** item are the
//!   controller's: the shell passes them on through [`TrayActions`], so they go
//!   through the same writes the window's own controls use;
//! - closing the window hides it instead of quitting while the icon can bring it
//!   back ("close to notification area").
//!
//! The controller fills [`TrayContent`] from the device state it last read and
//! the settings choices in [`TraySettings`]; the component words the tooltip and
//! the menu items itself from those values. [`tooltip`], [`status_line`],
//! [`battery_line`] and [`menu_lines`] keep that wording in Rust as well, so it
//! can be asserted without a notification area.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/tray.rs"));
}

pub use generated::Tray;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use soundcore_models::DeviceState;

use crate::{AppState, AppWindow, Battery, Connection};

/// The product name the tray shows in its tooltip, its title and its "Open" item.
const APP_NAME: &str = "OpenSCL4";

/// What the Settings page's "Desktop and quick access" panel chooses.
///
/// The controller stores this small document separately from device state and
/// Overview preferences. Missing fields use the defaults so newer versions can
/// add choices without invalidating an existing document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraySettings {
    /// `app-tray`: whether the notification-area icon is shown at all.
    pub icon: bool,
    /// `app-close-to-tray`: whether closing the window keeps the program in the
    /// notification area.
    pub close_to_tray: bool,
    /// `tray-battery`: whether the menu shows the device's battery line.
    pub battery: bool,
    /// `tray-modes`: whether the menu shows the Sound mode submenu.
    pub modes: bool,
    /// `tray-presets`: the vendored panel's quick-preset switch. This app has no
    /// quick presets, so the menu never shows that section; the switch keeps
    /// working in memory like the other tray choices.
    pub presets: bool,
    /// `tray-ldac`: whether the menu shows the LDAC item.
    pub ldac: bool,
}

impl Default for TraySettings {
    /// The panel's starting choices: the icon and close-to-tray on, the battery,
    /// sound-mode and LDAC menu sections on. Quick presets start off because the
    /// app has none.
    fn default() -> Self {
        Self {
            icon: true,
            close_to_tray: true,
            battery: true,
            modes: true,
            presets: false,
            ldac: true,
        }
    }
}

/// What the tray shows, in the values the vendored `Tray` component renders.
///
/// The properties are the component's own; the tooltip and the menu wording and
/// visibility rules live in `ui/tray.slint` and are derived from them there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayContent {
    /// Whether a device is saved, which decides between the empty menu and the
    /// device lines, and whether Connect is offered.
    pub has_device: bool,
    /// The device's name, as the window shows it. No address or serial.
    pub device_name: String,
    /// Whether the device is connected right now.
    pub connected: bool,
    /// Whether a connection attempt is in flight.
    pub connecting: bool,
    /// The battery line, such as "Left 97% · Right 76% · Case 60%", or empty
    /// when the device reports no battery.
    pub battery: String,
    /// The sound modes the menu can offer, in the window's language.
    pub modes: Vec<String>,
    /// The index of the current mode in [`Self::modes`], or `-1`.
    pub mode_index: i32,
    /// Whether the connected device has an LDAC switch.
    pub has_ldac: bool,
    /// Whether LDAC is on.
    pub ldac: bool,
    /// Whether the settings ask the menu to show the battery line.
    pub show_battery: bool,
    /// Whether the settings ask the menu to show the sound-mode submenu.
    pub show_modes: bool,
    /// Whether the settings ask the menu to show the quick-preset submenu. This
    /// app has no quick presets, so it is always false.
    pub show_presets: bool,
    /// Whether the settings ask the menu to show the LDAC item.
    pub show_ldac: bool,
}

impl Default for TrayContent {
    /// The component's own defaults with the app's settings: nothing known, and
    /// every optional menu section shown except quick presets.
    fn default() -> Self {
        let settings = TraySettings::default();
        Self {
            has_device: false,
            device_name: String::new(),
            connected: false,
            connecting: false,
            battery: String::new(),
            modes: Vec::new(),
            mode_index: -1,
            has_ldac: false,
            ldac: false,
            show_battery: settings.battery,
            show_modes: settings.modes,
            show_presets: settings.presets,
            show_ldac: settings.ldac,
        }
    }
}

impl TrayContent {
    /// The tray's view of the window state.
    ///
    /// Whether a device is known comes from the saved cards rather than from the
    /// connection, so a disconnected device still offers Connect; the battery,
    /// modes and LDAC come from the state the controller last read and are shown
    /// only while connected. The quick-preset section stays hidden: this app has
    /// no quick presets, so the vendored menu would only ever say "No presets
    /// yet".
    pub fn from_state(
        state: &AppState<'_>,
        device: Option<&DeviceState>,
        settings: &TraySettings,
    ) -> Self {
        let connection = state.get_connection();
        let connected = connection == Connection::Connected;
        let connecting = connection == Connection::Connecting;
        let battery = match (connected, device) {
            (true, Some(device)) => battery_line(&crate::controller::batteries(device)),
            _ => String::new(),
        };
        let modes: Vec<String> = if connected {
            state
                .get_modes()
                .iter()
                .map(|mode| mode.label.to_string())
                .collect()
        } else {
            Vec::new()
        };
        Self {
            has_device: state.get_devices().row_count() > 0,
            device_name: state.get_device_name().to_string(),
            connected,
            connecting,
            battery,
            modes,
            mode_index: if connected {
                state.get_mode_index()
            } else {
                -1
            },
            has_ldac: connected && state.get_has_ldac(),
            ldac: connected
                && device
                    .and_then(|device| device.settings)
                    .is_some_and(|settings| settings.ldac),
            show_battery: settings.battery,
            show_modes: settings.modes,
            show_presets: false,
            show_ldac: settings.ldac,
        }
    }
}

/// Writes a [`TrayContent`] into the vendored component's properties.
///
/// The quick-preset submenu stays empty: this app has no quick presets, and the
/// controller's content hides the section as well.
pub fn render(tray: &Tray, content: &TrayContent) {
    tray.set_has_device(content.has_device);
    tray.set_device_name(SharedString::from(content.device_name.as_str()));
    tray.set_connected(content.connected);
    tray.set_connecting(content.connecting);
    tray.set_battery(SharedString::from(content.battery.as_str()));
    tray.set_modes(ModelRc::new(VecModel::from(
        content
            .modes
            .iter()
            .map(|mode| SharedString::from(mode.as_str()))
            .collect::<Vec<_>>(),
    )));
    tray.set_mode_index(content.mode_index);
    tray.set_presets(ModelRc::default());
    tray.set_has_ldac(content.has_ldac);
    tray.set_ldac(content.ldac);
    tray.set_show_battery(content.show_battery);
    tray.set_show_modes(content.show_modes);
    tray.set_show_presets(content.show_presets);
    tray.set_show_ldac(content.show_ldac);
}

/// The tooltip `ui/tray.slint` shows for this content: the product name, the
/// device when one is known, and the battery line under it when the device is
/// connected and reports one.
pub fn tooltip(content: &TrayContent) -> String {
    if !content.has_device {
        return APP_NAME.to_string();
    }
    let mut text = format!("{APP_NAME} · {}", content.device_name);
    if content.connected && !content.battery.is_empty() {
        text.push('\n');
        text.push_str(&content.battery);
    }
    text
}

/// The menu's first line, as `ui/tray.slint` words it: the disabled "No
/// devices" line before a device is known, otherwise the device with its
/// connection status.
pub fn status_line(content: &TrayContent) -> String {
    if !content.has_device {
        return "No devices".to_string();
    }
    format!("{} · {}", content.device_name, status_word(content))
}

/// The connection status the menu's device line and the tooltip wording share.
fn status_word(content: &TrayContent) -> &'static str {
    if content.connected {
        "Connected"
    } else if content.connecting {
        "Connecting…"
    } else {
        "Not connected"
    }
}

/// The menu's battery line for the Overview tiles, such as
/// "Left 97% · Right 76% · Case 60%"; empty when the device reports no battery.
pub fn battery_line(tiles: &[Battery]) -> String {
    tiles
        .iter()
        .map(|tile| format!("{} {}", battery_word(tile.label), tile.value.as_str()))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The vendored `Strings` word for a battery tile's label: 0 left, 1 right,
/// 2 case, 3 the device as a whole.
fn battery_word(label: i32) -> &'static str {
    match label {
        0 => "Left",
        1 => "Right",
        2 => "Case",
        _ => "Battery",
    }
}

/// The menu `ui/tray.slint` builds from this content, as text lines: a mirror
/// of the component's `Menu` so the menu's rules can be asserted without a
/// notification area. Separators are not lines; the Sound mode submenu is
/// flattened to its title followed by one line per mode, with "✓ " on the
/// selected one. The quick-preset section never appears: this app has none.
pub fn menu_lines(content: &TrayContent) -> Vec<String> {
    let mut lines = vec![status_line(content)];
    if content.connected && content.show_battery && !content.battery.is_empty() {
        lines.push(content.battery.clone());
    }
    if content.has_device && !content.connected && !content.connecting {
        lines.push("Connect".to_string());
    }
    if content.connected && content.show_modes && !content.modes.is_empty() {
        lines.push("Sound mode".to_string());
        for (index, mode) in content.modes.iter().enumerate() {
            let selected = i32::try_from(index).ok() == Some(content.mode_index);
            lines.push(format!("{}{mode}", if selected { "✓ " } else { "" }));
        }
    }
    if content.connected && content.show_ldac && content.has_ldac {
        lines.push(format!("{}LDAC", if content.ldac { "✓ " } else { "" }));
    }
    lines.push("Open OpenSCL4".to_string());
    lines.push("Quit".to_string());
    lines
}

/// Whether closing the window leaves the program running: only while a
/// notification-area icon is there to reopen it from and the setting allows it.
pub fn closes_to_tray(icon_hosted: bool, close_to_tray: bool) -> bool {
    icon_hosted && close_to_tray
}

/// Whether a `Tray::show` result means the platform put the icon in the
/// notification area.
///
/// A platform error is the only signal the app gets that the icon is not there;
/// it is logged, and the caller must not record the shell as available or as
/// hosting an icon. Otherwise the Settings page would report the tray as
/// available and closing the window would hide it with no icon to reopen it
/// from, instead of quitting.
fn show_hosted(result: Result<(), slint::PlatformError>) -> bool {
    match result {
        Ok(()) => true,
        Err(err) => {
            eprintln!("OpenSCL4: could not show the notification-area icon: {err}");
            false
        }
    }
}

/// The menu choices the controller answers. Open and Quit stay in the shell
/// because it owns the window it shows and the loop it ends.
pub struct TrayActions {
    /// The Connect item: the window's own Retry, which reconnects the saved card.
    pub connect: Rc<dyn Fn()>,
    /// A Sound mode submenu choice: the index into [`TrayContent::modes`].
    pub choose_mode: Rc<dyn Fn(i32)>,
    /// The LDAC item's next value.
    pub set_ldac: Rc<dyn Fn(bool)>,
}

/// Owns the notification-area icon for the whole run and answers its menu.
///
/// The controller creates the shell before the window's `run` and keeps it in
/// its own state; the visible icon is what keeps Slint's event loop alive with
/// the window hidden, and dropping the shell removes it from the notification
/// area.
pub struct TrayShell {
    /// The window Open brings back and Quit hides.
    window: Weak<AppWindow>,
    /// The menu items the controller answers.
    actions: TrayActions,
    /// The last content rendered, re-applied when the icon is created again.
    content: RefCell<TrayContent>,
    /// The generated component, or `None` when the icon is switched off or the
    /// platform refused it.
    tray: RefCell<Option<Tray>>,
    /// The close-to-tray choice; shared with the window's close handler.
    close_to_tray: Rc<Cell<bool>>,
    /// Whether an icon is on screen right now; shared with the close handler.
    icon_hosted: Rc<Cell<bool>>,
    /// Whether a `show` has succeeded at least once: `app-tray-available`.
    host_available: Cell<bool>,
}

impl TrayShell {
    /// Creates the shell from the already-loaded settings, shows the icon when
    /// requested, fills in the empty menu, and wires Open, Quit and close.
    ///
    /// A platform without a notification area gets a warning and the program
    /// keeps working with no icon; closing the window then quits as usual.
    pub fn new(ui: &AppWindow, actions: TrayActions, settings: TraySettings) -> Self {
        let window = ui.as_weak();
        let close_to_tray = Rc::new(Cell::new(settings.close_to_tray));
        let icon_hosted = Rc::new(Cell::new(false));
        let shell = Self {
            window: window.clone(),
            actions,
            content: RefCell::new(TrayContent::default()),
            tray: RefCell::new(None),
            close_to_tray: Rc::clone(&close_to_tray),
            icon_hosted: Rc::clone(&icon_hosted),
            host_available: Cell::new(false),
        };
        ui.window().on_close_requested({
            let window = window.clone();
            let close_to_tray = Rc::clone(&close_to_tray);
            let icon_hosted = Rc::clone(&icon_hosted);
            move || {
                // Slint hides the window after this handler returns either way;
                // without an icon, or with close-to-tray off, the close also
                // ends the program.
                if !closes_to_tray(icon_hosted.get(), close_to_tray.get()) {
                    quit(&window);
                }
                CloseRequestResponse::HideWindow
            }
        });
        shell.set_icon(settings.icon);
        shell
    }

    /// Shows or takes away the icon, as the Settings panel's "Notification area
    /// icon" switch asks. The component itself is what draws the icon, so
    /// dropping it is what removes the icon from the notification area.
    pub fn set_icon(&self, shown: bool) {
        if shown {
            self.create_icon();
        } else {
            self.tray.borrow_mut().take();
            self.icon_hosted.set(false);
        }
    }

    /// Creates the component if there is none yet and shows it.
    ///
    /// The icon is only recorded as hosted when the platform's `show` succeeds:
    /// a refused icon must leave `available` false and `icon_hosted` false, so
    /// the Settings page does not offer a tray that is not there and the window's
    /// close handler quits instead of hiding with nothing to reopen it from.
    fn create_icon(&self) {
        if self.tray.borrow().is_some() {
            return;
        }
        let tray = match Tray::new() {
            Ok(tray) => tray,
            Err(err) => {
                eprintln!("OpenSCL4: could not create the notification-area icon: {err}");
                return;
            }
        };
        let window = self.window.clone();
        tray.on_open({
            let window = window.clone();
            move || open_window(&window)
        });
        tray.on_quit({
            let window = window.clone();
            move || quit(&window)
        });
        tray.on_connect({
            let connect = Rc::clone(&self.actions.connect);
            move || connect()
        });
        tray.on_choose_mode({
            let choose_mode = Rc::clone(&self.actions.choose_mode);
            move |index| choose_mode(index)
        });
        tray.on_set_ldac({
            let set_ldac = Rc::clone(&self.actions.set_ldac);
            move |on| set_ldac(on)
        });
        render(&tray, &self.content.borrow());
        if !show_hosted(tray.show()) {
            // The component is dropped here and the icon never reaches the
            // notification area; `host_available` and `icon_hosted` keep their
            // previous (false) values and the close handler quits the program.
            return;
        }
        self.host_available.set(true);
        self.icon_hosted.set(true);
        *self.tray.borrow_mut() = Some(tray);
    }

    /// Writes what the icon shows; kept for when the icon is created again.
    pub fn render(&self, content: &TrayContent) {
        *self.content.borrow_mut() = content.clone();
        if let Some(tray) = self.tray.borrow().as_ref() {
            render(tray, content);
        }
    }

    /// Sets the close-to-tray choice.
    pub fn set_close_to_tray(&self, enabled: bool) {
        self.close_to_tray.set(enabled);
    }

    /// Whether this platform has hosted a notification-area icon:
    /// `app-tray-available` on the Settings page.
    pub fn available(&self) -> bool {
        self.host_available.get()
    }
}

/// Shows the window again from the notification area and restores it from
/// minimized. The platform's show is also what gives it the focus; asking winit
/// for the focus explicitly would need Slint's unstable `winit-030` feature,
/// which the exploration notes leave optional.
pub(crate) fn open_window(window: &Weak<AppWindow>) {
    let Some(ui) = window.upgrade() else { return };
    ui.window().set_minimized(false);
    if let Err(err) = ui.show() {
        eprintln!("OpenSCL4: could not show the window: {err}");
    }
}

/// Ends the program: the window goes, then the event loop. Dropping the shell
/// on the way out removes the icon.
pub(crate) fn quit(window: &Weak<AppWindow>) {
    if let Some(ui) = window.upgrade() {
        let _ = ui.hide();
    }
    if let Err(err) = slint::quit_event_loop() {
        eprintln!("OpenSCL4: could not end the event loop: {err}");
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;

    use super::*;

    /// A content for a connected device with a battery line and the three sound
    /// modes.
    fn connected() -> TrayContent {
        TrayContent {
            has_device: true,
            device_name: "Space One Pro".to_string(),
            connected: true,
            battery: "Battery 20%".to_string(),
            modes: vec![
                "Noise canceling".to_string(),
                "Transparency".to_string(),
                "Normal".to_string(),
            ],
            mode_index: 1,
            has_ldac: true,
            ldac: true,
            ..TrayContent::default()
        }
    }

    /// A content for a saved but disconnected device.
    fn disconnected() -> TrayContent {
        TrayContent {
            has_device: true,
            device_name: "Space One Pro".to_string(),
            ..TrayContent::default()
        }
    }

    fn tile(label: i32, value: &str) -> Battery {
        Battery {
            label,
            value: value.into(),
            progress: 0.5,
            known: true,
        }
    }

    #[test]
    fn the_empty_tray_is_the_empty_menu_with_the_plain_tooltip() {
        let content = TrayContent::default();
        // The vendored tray words exactly these values as its skeleton.
        assert_eq!(tooltip(&content), "OpenSCL4");
        assert_eq!(status_line(&content), "No devices");
        assert!(!content.has_device);
        assert_eq!(content.device_name, "");
        assert!(!content.connected);
        assert!(!content.connecting);
        assert_eq!(content.battery, "");
        assert!(content.modes.is_empty());
        assert_eq!(content.mode_index, -1);
        assert!(!content.has_ldac);
        assert!(!content.ldac);
        assert!(content.show_battery);
        assert!(content.show_modes);
        assert!(!content.show_presets, "this app has no quick presets");
        assert!(content.show_ldac);
    }

    #[test]
    fn the_tooltip_names_the_battery_of_a_connected_device() {
        assert_eq!(
            tooltip(&connected()),
            "OpenSCL4 · Space One Pro\nBattery 20%"
        );
    }

    #[test]
    fn the_tooltip_leaves_the_battery_out_until_it_is_connected_and_reported() {
        let mut content = connected();
        content.connected = false;
        assert_eq!(tooltip(&content), "OpenSCL4 · Space One Pro");
        content.connected = true;
        content.battery.clear();
        assert_eq!(tooltip(&content), "OpenSCL4 · Space One Pro");
    }

    #[test]
    fn the_menu_status_follows_the_connection() {
        let mut content = connected();
        assert_eq!(status_line(&content), "Space One Pro · Connected");
        content.connected = false;
        content.connecting = true;
        assert_eq!(status_line(&content), "Space One Pro · Connecting…");
        content.connecting = false;
        assert_eq!(status_line(&content), "Space One Pro · Not connected");
    }

    #[test]
    fn the_battery_line_names_every_tile_in_menu_order() {
        let tiles = [tile(0, "97%"), tile(1, "76%"), tile(2, "60%")];
        assert_eq!(battery_line(&tiles), "Left 97% · Right 76% · Case 60%");
        assert_eq!(battery_line(&[tile(3, "20%")]), "Battery 20%");
        assert_eq!(battery_line(&[]), "");
    }

    #[test]
    fn the_menu_is_the_status_battery_connect_modes_ldac_open_and_quit() {
        let mut content = connected();
        content.mode_index = 1;
        assert_eq!(
            menu_lines(&content),
            [
                "Space One Pro · Connected",
                "Battery 20%",
                "Sound mode",
                "Noise canceling",
                "✓ Transparency",
                "Normal",
                "✓ LDAC",
                "Open OpenSCL4",
                "Quit",
            ]
        );
    }

    #[test]
    fn the_menu_offers_connect_only_while_disconnected() {
        let mut content = disconnected();
        assert_eq!(
            menu_lines(&content),
            [
                "Space One Pro · Not connected",
                "Connect",
                "Open OpenSCL4",
                "Quit",
            ]
        );
        content.connecting = true;
        assert_eq!(
            menu_lines(&content),
            ["Space One Pro · Connecting…", "Open OpenSCL4", "Quit",],
            "Connect is not offered while an attempt is in flight"
        );
        assert!(
            !menu_lines(&connected()).contains(&"Connect".to_string()),
            "Connect is not offered while connected"
        );
    }

    #[test]
    fn the_menu_never_offers_quick_presets() {
        let mut content = connected();
        content.show_presets = true;
        let lines = menu_lines(&content);
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("presets") || line.contains("preset")),
            "{lines:?} must not name quick presets"
        );
    }

    #[test]
    fn the_menu_hides_the_sections_the_settings_switch_off() {
        let mut content = connected();
        content.show_battery = false;
        content.show_modes = false;
        content.show_ldac = false;
        assert_eq!(
            menu_lines(&content),
            ["Space One Pro · Connected", "Open OpenSCL4", "Quit",]
        );
    }

    #[test]
    fn closing_keeps_the_program_only_while_the_icon_can_reopen_it() {
        assert!(closes_to_tray(true, true));
        assert!(!closes_to_tray(true, false));
        assert!(!closes_to_tray(false, true));
        assert!(!closes_to_tray(false, false));
    }

    #[test]
    fn a_refused_show_is_not_a_hosted_icon_and_close_quits() {
        assert!(show_hosted(Ok(())), "a shown icon counts as hosted");
        let refused =
            || slint::PlatformError::Other("the platform refused the notification icon".into());
        assert!(
            !show_hosted(Err(refused())),
            "a failed show must not be recorded as a hosted icon"
        );
        assert!(
            !closes_to_tray(show_hosted(Err(refused())), true),
            "with no icon on screen, closing the window quits instead of hiding"
        );
    }

    #[test]
    fn rendering_the_empty_content_sets_every_component_property() {
        crate::test_support::with_slint_thread(|| {
            let tray = Tray::new().expect("the tray component builds");
            render(&tray, &TrayContent::default());
            assert!(!tray.get_has_device());
            assert_eq!(tray.get_device_name().as_str(), "");
            assert!(!tray.get_connected());
            assert!(!tray.get_connecting());
            assert_eq!(tray.get_battery().as_str(), "");
            assert_eq!(tray.get_modes().row_count(), 0);
            assert_eq!(tray.get_mode_index(), -1);
            assert_eq!(tray.get_presets().row_count(), 0);
            assert!(!tray.get_has_ldac());
            assert!(!tray.get_ldac());
            assert!(tray.get_show_battery());
            assert!(tray.get_show_modes());
            assert!(!tray.get_show_presets());
            assert!(tray.get_show_ldac());
        });
    }

    #[test]
    fn rendering_the_connected_content_sets_the_device_and_menu_values() {
        crate::test_support::with_slint_thread(|| {
            let tray = Tray::new().expect("the tray component builds");
            render(&tray, &connected());
            assert!(tray.get_has_device());
            assert_eq!(tray.get_device_name().as_str(), "Space One Pro");
            assert!(tray.get_connected());
            assert!(!tray.get_connecting());
            assert_eq!(tray.get_battery().as_str(), "Battery 20%");
            assert_eq!(tray.get_modes().row_count(), 3);
            assert_eq!(
                tray.get_modes()
                    .row_data(1)
                    .expect("the second mode")
                    .as_str(),
                "Transparency"
            );
            assert_eq!(tray.get_mode_index(), 1);
            assert_eq!(tray.get_presets().row_count(), 0);
            assert!(tray.get_has_ldac());
            assert!(tray.get_ldac());
        });
    }

    fn settings_path(label: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openscl4-desktop-preferences-{}-{sequence}-{label}.json",
            std::process::id()
        ))
    }

    #[test]
    fn missing_and_malformed_desktop_preferences_use_defaults() {
        let missing = settings_path("missing");
        let _ = fs::remove_file(&missing);
        assert_eq!(load_settings(&missing), TraySettings::default());

        let malformed = settings_path("malformed");
        fs::write(&malformed, b"not json").expect("the malformed fixture is written");
        assert_eq!(load_settings(&malformed), TraySettings::default());
        assert_eq!(
            fs::read_to_string(&malformed).expect("the invalid source remains readable"),
            "not json",
            "loading does not silently destroy the user's document"
        );
        let _ = fs::remove_file(malformed);
    }

    #[test]
    fn missing_desktop_preference_fields_keep_their_defaults() {
        let path = settings_path("partial");
        fs::write(&path, br#"{"icon":false}"#).expect("the partial fixture is written");
        assert_eq!(
            load_settings(&path),
            TraySettings {
                icon: false,
                ..TraySettings::default()
            }
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn desktop_preferences_roundtrip_through_an_atomic_save() {
        let path = settings_path("roundtrip");
        let _ = fs::remove_file(&path);
        let settings = TraySettings {
            icon: false,
            close_to_tray: false,
            battery: false,
            modes: false,
            presets: true,
            ldac: false,
        };
        save_settings(&path, &settings).expect("the first save succeeds");
        assert_eq!(load_settings(&path), settings);
        save_settings(&path, &TraySettings::default()).expect("replacement succeeds");
        assert_eq!(load_settings(&path), TraySettings::default());
        assert!(
            !std::path::PathBuf::from(format!("{}.tmp", path.display())).exists(),
            "the temporary file is renamed away"
        );
        let _ = fs::remove_file(path);
    }
}
