//! Desktop shell.
//!
//! The Slint interface is vendored from OpenSCL4 under GPL-3.0 (see
//! `ui/SOURCE.md`) and is intentionally not redesigned. This binary opens the
//! shell and hands it to [`controller::Controller`], which fills the Add Device
//! dialog from the model catalog and wires the Windows Bluetooth session that
//! shows the connected device's battery on the Overview page.
//!
//! [`tray::TrayShell`] adds the notification-area icon around that window: it
//! is created before the window's `run`, so a close can hide the window while
//! the icon keeps the program alive, and it is dropped when the run ends, which
//! removes the icon.

slint::include_modules!();

mod catalog_adapter;
mod controller;
mod transport_windows;
pub mod tray;

use controller::Controller;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    // The controller answers the shell's actions and keeps the worker-event
    // timer; it must live as long as the window does.
    let _controller = Controller::new(&ui);
    // The notification-area icon is created before the event loop starts: its
    // Open item brings the window back after a close hid it, and Quit ends the
    // loop. Keeping the shell alive for the whole run is what keeps the icon
    // there; dropping it on the way out removes it.
    let _tray = tray::TrayShell::new(&ui);
    ui.run()
}

/// The version the About section shows: the package version, which the spec
/// fixes at 2.0 for this branch.
pub(crate) fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_about_version_is_the_2_0_spec_version() {
        // `AppState.app-version` reads this, so About says OpenSCL4 2.0.
        assert_eq!(app_version(), "2.0.0");
    }
}
