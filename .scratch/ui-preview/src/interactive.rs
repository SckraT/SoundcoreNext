use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::{
    Actions, AppState, AppWindow, BlockKind, LayoutBlock, OverviewBlockData, OverviewRow, Page,
    PaletteChoice, Theme,
};

fn block(kind: BlockKind) -> Option<OverviewBlockData> {
    match kind {
        BlockKind::SoundMode => Some(super::block(kind, Page::SoundModes)),
        BlockKind::Equalizer => Some(super::block(kind, Page::Equalizer)),
        BlockKind::Controls => Some(super::block(kind, Page::Controls)),
        BlockKind::Comfort => Some(super::block(kind, Page::Comfort)),
        BlockKind::Spatial => Some(super::block(kind, Page::Effects)),
        BlockKind::Power => Some(super::block(kind, Page::Power)),
        BlockKind::Connections => Some(super::block(kind, Page::Connections)),
        BlockKind::QuickPresets | BlockKind::Case => None,
    }
}

fn overview_rows(blocks: &[LayoutBlock]) -> ModelRc<OverviewRow> {
    let mut grouped: Vec<(bool, Vec<OverviewBlockData>)> = Vec::new();
    for layout in blocks.iter().filter(|layout| layout.visible) {
        let Some(data) = block(layout.kind) else {
            continue;
        };
        let shortcuts = !matches!(
            layout.kind,
            BlockKind::SoundMode | BlockKind::Equalizer | BlockKind::QuickPresets
        );
        match grouped.last_mut() {
            Some((is_shortcut, items))
                if *is_shortcut == shortcuts && (shortcuts || items.len() < 2) =>
            {
                items.push(data)
            }
            _ => grouped.push((shortcuts, vec![data])),
        }
    }
    ModelRc::new(VecModel::from(
        grouped
            .into_iter()
            .map(|(shortcuts, blocks)| super::row(shortcuts, blocks))
            .collect::<Vec<_>>(),
    ))
}

fn refresh_layout(state: &AppState<'_>, blocks: &[LayoutBlock]) {
    state.set_layout_blocks(ModelRc::new(VecModel::from(blocks.to_vec())));
    state.set_overview_rows(overview_rows(blocks));
}

fn default_toggle_state(id: &str) -> bool {
    matches!(id, "ldac" | "dualConnections" | "voicePrompt")
}

pub fn wire(ui: &AppWindow) {
    let actions = ui.global::<Actions>();

    actions.on_disconnect({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                let state = ui.global::<AppState>();
                state.set_connection(crate::Connection::Off);
                state.set_page(Page::Devices);
            }
        }
    });
    actions.on_connect({
        let weak = ui.as_weak();
        move |_mac| {
            if let Some(ui) = weak.upgrade() {
                super::populate(&ui);
                ui.global::<AppState>().set_page(Page::Overview);
            }
        }
    });
    actions.on_retry({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                super::populate(&ui);
                ui.global::<AppState>().set_page(Page::Overview);
            }
        }
    });
    actions.on_navigate({
        let weak = ui.as_weak();
        move |page| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_page(page);
            }
        }
    });
    actions.on_open_diagnostics({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_page(Page::Diagnostics);
            }
        }
    });
    actions.on_edit_layout({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_editing_layout(true);
            }
        }
    });
    actions.on_layout_visible({
        let weak = ui.as_weak();
        move |index, visible| {
            let Some(ui) = weak.upgrade() else { return };
            let state = ui.global::<AppState>();
            let mut blocks: Vec<_> = state.get_layout_blocks().iter().collect();
            if let Some(block) = usize::try_from(index)
                .ok()
                .and_then(|index| blocks.get_mut(index))
            {
                block.visible = visible;
                state.set_layout_preset(-1);
                refresh_layout(&state, &blocks);
            }
        }
    });
    actions.on_layout_move({
        let weak = ui.as_weak();
        move |index, direction| {
            let Some(ui) = weak.upgrade() else { return };
            let state = ui.global::<AppState>();
            let mut blocks: Vec<_> = state.get_layout_blocks().iter().collect();
            let Ok(from) = usize::try_from(index) else {
                return;
            };
            let Ok(offset) = isize::try_from(direction) else {
                return;
            };
            let Some(to) = from.checked_add_signed(offset) else {
                return;
            };
            if to < blocks.len() && from < blocks.len() {
                blocks.swap(from, to);
                state.set_layout_preset(-1);
                refresh_layout(&state, &blocks);
            }
        }
    });
    actions.on_layout_use({
        let weak = ui.as_weak();
        move |preset| {
            let Some(ui) = weak.upgrade() else { return };
            let state = ui.global::<AppState>();
            let mut blocks: Vec<_> = state.get_layout_blocks().iter().collect();
            for block in &mut blocks {
                block.visible = match preset {
                    0 => matches!(block.kind, BlockKind::SoundMode | BlockKind::Equalizer),
                    1 => block.kind != BlockKind::Power,
                    2 => true,
                    _ => block.visible,
                };
            }
            state.set_layout_preset(preset);
            refresh_layout(&state, &blocks);
        }
    });
    actions.on_choose_toggle({
        let weak = ui.as_weak();
        move |id, selected| {
            let Some(ui) = weak.upgrade() else { return };
            let state = ui.global::<AppState>();
            let previous: Vec<_> = state.get_quick_toggles().iter().collect();
            let mut all: Vec<_> = state.get_all_toggles().iter().collect();
            for toggle in &mut all {
                if toggle.id == id {
                    toggle.on = selected;
                }
            }
            let shown = all
                .iter()
                .filter(|toggle| toggle.on)
                .map(|toggle| {
                    let mut toggle = toggle.clone();
                    toggle.on = previous
                        .iter()
                        .find(|old| old.id == toggle.id)
                        .map_or_else(|| default_toggle_state(toggle.id.as_str()), |old| old.on);
                    toggle
                })
                .collect::<Vec<_>>();
            state.set_all_toggles(ModelRc::new(VecModel::from(all)));
            state.set_quick_toggles(ModelRc::new(VecModel::from(shown)));
        }
    });
    actions.on_set_toggle({
        let weak = ui.as_weak();
        move |id, on| {
            let Some(ui) = weak.upgrade() else { return };
            let model = ui.global::<AppState>().get_quick_toggles();
            for index in 0..model.row_count() {
                if let Some(mut toggle) = model.row_data(index)
                    && toggle.id == id
                {
                    toggle.on = on;
                    model.set_row_data(index, toggle);
                }
            }
        }
    });
    actions.on_set_option({
        let weak = ui.as_weak();
        move |id, index| {
            if id == "ambientSoundMode"
                && let Some(ui) = weak.upgrade()
            {
                ui.global::<AppState>().set_mode_index(index);
            }
        }
    });
    actions.on_set_app_theme({
        let weak = ui.as_weak();
        move |index| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_app_theme(index);
                if index == 1 || index == 2 {
                    ui.global::<Theme>().set_dark(index == 2);
                }
            }
        }
    });
    actions.on_set_palette({
        let weak = ui.as_weak();
        move |index| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_app_palette(index);
                ui.global::<Theme>().set_palette(match index {
                    1 => PaletteChoice::Sage,
                    2 => PaletteChoice::Rose,
                    _ => PaletteChoice::Purple,
                });
            }
        }
    });
    actions.on_set_content_scale({
        let weak = ui.as_weak();
        move |index| {
            if let Some(ui) = weak.upgrade() {
                let scale = match index {
                    1 => 1.1,
                    2 => 1.25,
                    _ => 1.0,
                };
                ui.global::<AppState>().set_app_content_scale(index);
                ui.global::<Theme>().set_content_scale(scale);
            }
        }
    });
    actions.on_set_badge({
        let weak = ui.as_weak();
        move |show| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_show_one_earbud_badge(show);
            }
        }
    });

    actions.on_diagnostic_start({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                let state = ui.global::<AppState>();
                state.set_diagnostic_busy(false);
                state.set_diagnostic_step(crate::DiagnosticStep::Capture);
            }
        }
    });
    actions.on_diagnostic_cancel({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>()
                    .set_diagnostic_step(crate::DiagnosticStep::Review);
            }
        }
    });
    actions.on_diagnostic_restart({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>()
                    .set_diagnostic_step(crate::DiagnosticStep::Device);
            }
        }
    });
    actions.on_diagnostic_note({
        let weak = ui.as_weak();
        move |note| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<AppState>().set_diagnostic_note(note);
            }
        }
    });
    actions.on_quit_app(|| {
        let _ = slint::quit_event_loop();
    });
}
