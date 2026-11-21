//! The notification-area icon and the window lifecycle that goes with it.
//!
//! `ui/tray.slint` is the vendored Slint tray component: the OpenSCL4 icon, a
//! tooltip, and a menu built from a handful of properties. `build.rs` compiles
//! it as its own unit (it exports a `SystemTrayIcon` and no window, so Slint
//! creates no window adapter for it). This module creates that component before
//! the window's `run`, keeps it alive for the whole run, and wires it to the
//! window:
//!
//! - **Open** (the menu item and a left-click on the icon) shows and focuses the
//!   window again;
//! - **Quit** ends the event loop, after which `main` drops this shell and with
//!   it the icon;
//! - closing the window hides it instead of quitting: the icon keeps the program
//!   running in the notification area ("close to notification area"), which is
//!   on by default and stays a setting in memory until the vendored Settings
//!   page is wired to it in the next ticket.
//!
//! The menu is still the skeleton this ticket asks for. No device is known yet,
//! so the tooltip reads just "OpenSCL4" and the menu shows the disabled
//! "No devices" line, Open and Quit; the device, battery, Connect, Sound mode
//! and LDAC entries appear when a later ticket fills [`TrayContent`] from the
//! controller. The vendors' component words the tooltip and the menu items
//! itself from those values; [`tooltip`] and [`status_line`] keep the wording in
//! Rust as well so it can be asserted without a notification area.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/tray.rs"));
}

pub use generated::Tray;

use std::cell::Cell;
use std::rc::Rc;

use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::AppWindow;

/// The product name the tray shows in its tooltip and title.
const APP_NAME: &str = "OpenSCL4";

/// Whether closing the window keeps the program running in the notification
/// area when the shell starts. On by default; the vendored Settings page will
/// turn it into a real setting in the next ticket.
const CLOSE_TO_TRAY: bool = true;

/// What the tray shows, in the values the vendored `Tray` component renders.
///
/// The properties are the component's own; the tooltip and the menu wording and
/// visibility rules live in `ui/tray.slint` and are derived from them there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayContent {
    /// Whether a device is remembered/chosen, which decides between the empty
    /// menu and the device lines.
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
    /// Whether the settings ask the menu to show the quick-preset submenu.
    pub show_presets: bool,
    /// Whether the settings ask the menu to show the LDAC item.
    pub show_ldac: bool,
}

impl Default for TrayContent {
    /// The component's own defaults: nothing known, every optional menu section
    /// allowed. `mode_index` is `-1`, the component's "no mode selected" value.
    fn default() -> Self {
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
            show_battery: true,
            show_modes: true,
            show_presets: true,
            show_ldac: true,
        }
    }
}

impl TrayContent {
    /// The skeleton before any device is known: the menu has nothing but the
    /// disabled "No devices" line, Open and Quit, and the tooltip is "OpenSCL4".
    pub fn no_device() -> Self {
        Self::default()
    }
}

/// Writes a [`TrayContent`] into the vendored component's properties.
///
/// The quick-preset submenu stays empty: this shell has no quick presets, and
/// the vendored menu words an empty section as "No presets yet".
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
    let status = if content.connected {
        "Connected"
    } else if content.connecting {
        "Connecting…"
    } else {
        "Not connected"
    };
    format!("{} · {status}", content.device_name)
}

/// Whether closing the window leaves the program running: only while a
/// notification-area icon is there to reopen it from and the setting allows it.
pub fn closes_to_tray(icon_hosted: bool, close_to_tray: bool) -> bool {
    icon_hosted && close_to_tray
}

/// Owns the notification-area icon for the whole run and answers its menu.
///
/// Created before the window's `run` and dropped by `main` when it returns: the
/// visible icon is what keeps Slint's event loop alive with the window hidden,
/// and dropping this struct is what removes it from the notification area.
pub struct TrayShell {
    /// The generated component. It is never dropped before the run ends: a
    /// hidden window needs the icon to come back from.
    _tray: Option<Tray>,
    /// The close-to-tray setting, in memory. The window's close handler reads
    /// it through this shared cell, so a later ticket can change the setting
    /// while the program runs.
    close_to_tray: Rc<Cell<bool>>,
}

impl TrayShell {
    /// Creates and shows the icon, fills in the empty menu, and wires the menu's
    /// Open and Quit items and the window's close button.
    ///
    /// A platform without a notification area gets a warning and the program
    /// keeps working with no icon; closing the window then quits as usual.
    pub fn new(ui: &AppWindow) -> Self {
        let window = ui.as_weak();
        let tray = match Tray::new() {
            Ok(tray) => Some(tray),
            Err(err) => {
                eprintln!("OpenSCL4: could not create the notification-area icon: {err}");
                None
            }
        };
        if let Some(tray) = &tray {
            tray.on_open({
                let window = window.clone();
                move || open_window(&window)
            });
            tray.on_quit({
                let window = window.clone();
                move || quit(&window)
            });
            // No controller state yet: the component renders its empty menu.
            render(tray, &TrayContent::no_device());
            if let Err(err) = tray.show() {
                eprintln!("OpenSCL4: could not show the notification-area icon: {err}");
            }
        }
        let close_to_tray = Rc::new(Cell::new(CLOSE_TO_TRAY));
        let icon_hosted = tray.is_some();
        ui.window().on_close_requested({
            let window = window.clone();
            let close_to_tray = Rc::clone(&close_to_tray);
            move || {
                // Slint hides the window after this handler returns either way;
                // without an icon, or with close-to-tray off, the close also
                // ends the program.
                if !closes_to_tray(icon_hosted, close_to_tray.get()) {
                    quit(&window);
                }
                CloseRequestResponse::HideWindow
            }
        });
        Self {
            _tray: tray,
            close_to_tray,
        }
    }

    /// Whether closing the window currently leaves the program in the
    /// notification area.
    pub fn closes_to_tray(&self) -> bool {
        closes_to_tray(self._tray.is_some(), self.close_to_tray.get())
    }

    /// Sets the close-to-tray choice, which the vendored Settings page will do
    /// in the next ticket.
    pub fn set_close_to_tray(&mut self, enabled: bool) {
        self.close_to_tray.set(enabled);
    }
}

/// Shows the window again from the notification area and restores it from
/// minimized. The platform's show is also what gives it the focus; asking winit
/// for the focus explicitly would need Slint's unstable `winit-030` feature,
/// which the exploration notes leave optional.
fn open_window(window: &Weak<AppWindow>) {
    let Some(ui) = window.upgrade() else { return };
    ui.window().set_minimized(false);
    if let Err(err) = ui.show() {
        eprintln!("OpenSCL4: could not show the window: {err}");
    }
}

/// Ends the program: the window goes, then the event loop. `main` drops the
/// shell on the way out, which removes the icon.
fn quit(window: &Weak<AppWindow>) {
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

    /// A content for a connected device with a battery line, the state a later
    /// ticket will fill from the controller.
    fn connected() -> TrayContent {
        TrayContent {
            has_device: true,
            device_name: "Space One Pro".to_string(),
            connected: true,
            battery: "Left 97% · Right 76%".to_string(),
            ..TrayContent::default()
        }
    }

    #[test]
    fn the_skeleton_is_the_empty_menu_with_the_plain_tooltip() {
        let content = TrayContent::no_device();
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
        assert!(content.show_presets);
        assert!(content.show_ldac);
    }

    #[test]
    fn the_tooltip_names_the_battery_of_a_connected_device() {
        assert_eq!(
            tooltip(&connected()),
            "OpenSCL4 · Space One Pro\nLeft 97% · Right 76%"
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
    fn closing_keeps_the_program_only_while_the_icon_can_reopen_it() {
        assert!(closes_to_tray(true, true));
        assert!(!closes_to_tray(true, false));
        assert!(!closes_to_tray(false, true));
        assert!(!closes_to_tray(false, false));
    }

    #[test]
    fn rendering_the_skeleton_sets_every_component_property() {
        let tray = Tray::new().expect("the tray component builds");
        render(&tray, &TrayContent::no_device());
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
        assert!(tray.get_show_presets());
        assert!(tray.get_show_ldac());
    }
}
