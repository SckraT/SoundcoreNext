//! Desktop shell.
//!
//! The Slint interface is vendored from OpenSCL4 under GPL-3.0 (see
//! `ui/SOURCE.md`) and is intentionally not redesigned. This binary opens the
//! shell and fills the existing Add Device dialog from the clean-room catalog;
//! it does not connect to anything yet.

slint::include_modules!();

mod catalog_adapter;

use catalog_adapter::{Chip, filter_models, selection};
use slint::{ModelRc, VecModel};

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    wire_add_device(&ui);
    ui.run()
}

/// The version the About section shows: the package version, which the spec
/// fixes at 2.0 for this branch.
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Fills the shell's state and answers the Add Device dialog's callbacks.
fn wire_add_device(ui: &AppWindow) {
    let state = ui.global::<AppState>();
    state.set_app_version(app_version().into());
    reset_models(&state);

    ui.global::<Actions>().on_open_add_device({
        let ui = ui.as_weak();
        move || {
            let Some(window) = ui.upgrade() else { return };
            let state = window.global::<AppState>();
            state.set_adding(true);
            state.set_add_step(0);
            state.set_add_loading(false);
            state.set_add_bluetooth_off(false);
            // Pairing is not wired yet, so there is nothing to list.
            state.set_paired_options(ModelRc::new(VecModel::from(Vec::<PairedOption>::new())));
            reset_models(&state);
        }
    });

    ui.global::<Actions>().on_filter_models({
        let ui = ui.as_weak();
        move |query, kind| {
            let Some(window) = ui.upgrade() else { return };
            // The vendored chip row is the callback's only source of indexes.
            show_models(
                &window.global::<AppState>(),
                query.as_str(),
                Chip::from_index(kind),
            );
        }
    });

    ui.global::<Actions>().on_choose_model({
        let ui = ui.as_weak();
        move |code| {
            let Some(window) = ui.upgrade() else { return };
            let Some(chosen) = selection(code.as_str()) else {
                return;
            };
            let state = window.global::<AppState>();
            state.set_add_model_name(chosen.name.into());
            state.set_add_model_code(chosen.code.into());
            state.set_add_model_art(chosen.art);
            state.set_add_model_is_speaker(chosen.is_speaker);
            state.set_add_step(1);
        }
    });
}

/// Replaces the dialog's model list with the models for a query and chip; an
/// index that is not one of the vendored chips lists nothing.
fn show_models(state: &AppState<'_>, query: &str, chip: Option<Chip>) {
    let models = match chip {
        Some(chip) => filter_models(query, chip),
        None => Vec::new(),
    };
    state.set_models(ModelRc::new(VecModel::from(models)));
}

/// Resets the dialog's search and chip to All, then lists the whole catalog.
fn reset_models(state: &AppState<'_>) {
    state.set_model_query("".into());
    state.set_model_kind(Chip::All.index());
    show_models(state, "", Some(Chip::All));
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
