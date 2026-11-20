//! Desktop shell.
//!
//! The Slint interface is vendored from OpenSCL4 under GPL-3.0 (see
//! `ui/SOURCE.md`) and is intentionally not redesigned. This binary only opens
//! the shell; the controller and the clean-room core are wired next.

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}
