//! Desktop shell.
//!
//! The Slint interface is vendored from OpenSCL4 under GPL-3.0 (see
//! `ui/SOURCE.md`) and is intentionally not redesigned. This binary opens the
//! shell and hands it to [`controller::Controller`], which fills the Add Device
//! dialog from the model catalog, wires the Windows Bluetooth session that shows
//! the connected device's battery on the Overview page, and owns the
//! notification-area icon from [`tray`].
//!
//! The window goes into the notification area on close (the icon is created
//! before the window's `run`), and only one copy of the app runs at a time: a
//! second start asks the first to show its window, which may be hidden there,
//! and ends without opening one ([`single_instance`]).

slint::include_modules!();

mod catalog_adapter;
mod controller;
mod single_instance;
#[cfg(test)]
mod test_support;
mod transport_windows;
pub mod tray;

use controller::Controller;

fn main() -> Result<(), slint::PlatformError> {
    // One copy at a time, before any window exists: the second start asks the
    // running one to show its window and leaves without a window of its own.
    let config_dir = controller::default_config_dir();
    let Some(instance) = single_instance::claim(&config_dir) else {
        return Ok(());
    };
    let ui = AppWindow::new()?;
    // The controller answers the shell's actions, keeps the worker-event timer,
    // and owns the notification-area icon. Its Open item brings the window back
    // after a close hid it, and Quit ends the loop; dropping the controller on
    // the way out removes the icon.
    let _controller = Controller::new(&ui, Some(instance.show_request()));
    // The claim has to live for as long as the app runs, or a later start would
    // take this one for another copy.
    let _instance = instance;
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
