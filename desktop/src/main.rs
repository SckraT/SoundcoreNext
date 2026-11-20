//! Desktop shell.
//!
//! The Slint interface is vendored from OpenSCL4 under GPL-3.0 (see
//! `ui/SOURCE.md`) and is intentionally not redesigned. This binary opens the
//! shell and fills the existing Add Device dialog from the clean-room catalog;
//! it does not connect to anything yet.

slint::include_modules!();

mod catalog_adapter;

use catalog_adapter::{KIND_ALL, filter_models, selection};
use slint::{ModelRc, VecModel};

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    wire(&ui);
    ui.run()
}

/// Fills the shell's state and answers the Add Device dialog's callbacks.
fn wire(ui: &AppWindow) {
    show_models(&ui.global::<AppState>(), "", KIND_ALL);

    ui.global::<Actions>().on_open_add_device({
        let ui = ui.as_weak();
        move || {
            let Some(ui) = ui.upgrade() else { return };
            let state = ui.global::<AppState>();
            state.set_adding(true);
            state.set_add_step(0);
            state.set_model_query("".into());
            state.set_model_kind(KIND_ALL);
            state.set_add_loading(false);
            state.set_add_bluetooth_off(false);
            // Pairing is not wired yet, so there is nothing to list.
            state.set_paired_options(ModelRc::new(VecModel::from(Vec::<PairedOption>::new())));
            show_models(&state, "", KIND_ALL);
        }
    });

    ui.global::<Actions>().on_filter_models({
        let ui = ui.as_weak();
        move |query, kind| {
            if let Some(ui) = ui.upgrade() {
                show_models(&ui.global::<AppState>(), query.as_str(), kind);
            }
        }
    });

    ui.global::<Actions>().on_choose_model({
        let ui = ui.as_weak();
        move |code| {
            let Some(ui) = ui.upgrade() else { return };
            let Some(selection) = selection(code.as_str()) else {
                return;
            };
            let state = ui.global::<AppState>();
            state.set_add_model_name(selection.name.into());
            state.set_add_model_code(selection.code.into());
            state.set_add_model_art(selection.art);
            state.set_add_model_is_speaker(selection.is_speaker);
            state.set_add_step(1);
        }
    });
}

/// Replaces the dialog's model list with the catalog entries for a query and chip.
fn show_models(state: &AppState<'_>, query: &str, kind: i32) {
    state.set_models(ModelRc::new(VecModel::from(filter_models(query, kind))));
}
