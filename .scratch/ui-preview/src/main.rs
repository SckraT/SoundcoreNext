slint::include_modules!();

mod interactive;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};

#[derive(Clone)]
struct Capture {
    width: f32,
    height: f32,
    scale: f32,
    dark: bool,
    page: Page,
    diagnostic_step: DiagnosticStep,
    dialog: Dialog,
    name: String,
}

#[derive(Clone, Copy)]
enum Dialog {
    None,
    Layout,
    Toggles,
    AddDevice,
    SaveProfile,
    CreatePreset,
}

fn block(kind: BlockKind, page: Page) -> OverviewBlockData {
    OverviewBlockData {
        kind,
        page,
        number: 1,
        total: 2,
        text: if kind == BlockKind::Power {
            "60 minutes".into()
        } else {
            "LDAC".into()
        },
        on: kind == BlockKind::Spatial,
        has_mode_cycle: kind == BlockKind::Controls,
    }
}

fn row(shortcuts: bool, blocks: Vec<OverviewBlockData>) -> OverviewRow {
    OverviewRow {
        shortcuts,
        blocks: ModelRc::new(VecModel::from(blocks)),
    }
}

fn toggle(id: &str, label: &str, on: bool) -> QuickToggle {
    QuickToggle {
        id: id.into(),
        label: label.into(),
        on,
        enabled: true,
        reason: "".into(),
    }
}

fn populate(ui: &AppWindow) {
    let state = ui.global::<AppState>();
    state.set_connection(Connection::Connected);
    state.set_device_name("soundcore Space One Pro · Demo".into());
    state.set_device_code("A3062".into());
    let art = slint::Image::load_from_path(std::path::Path::new(
        "../../desktop/ui/art/sound_art_headphones_silver.svg",
    ))
    .expect("load the demo headphones");
    state.set_device_art(art.clone());
    state.set_models(ModelRc::new(VecModel::from(vec![
        ModelOption {
            code: "A3062".into(),
            name: "soundcore Space One Pro".into(),
            art: art.clone(),
        },
        ModelOption {
            code: "A3949".into(),
            name: "soundcore P20i".into(),
            art: art.clone(),
        },
        ModelOption {
            code: "A3954".into(),
            name: "soundcore Liberty 5".into(),
            art: art.clone(),
        },
    ])));
    state.set_devices(ModelRc::new(VecModel::from(vec![DeviceCard {
        mac: "00:00:00:00:00:00".into(),
        name: "soundcore Space One Pro · Demo".into(),
        code: "A3062".into(),
        art,
        current: true,
        demo: true,
    }])));
    state.set_sound_pages(ModelRc::new(VecModel::from(vec![
        Page::SoundModes,
        Page::Equalizer,
        Page::Effects,
    ])));
    state.set_device_pages(ModelRc::new(VecModel::from(vec![
        Page::Controls,
        Page::Comfort,
        Page::Power,
        Page::Connections,
        Page::About,
    ])));
    state.set_batteries(ModelRc::new(VecModel::from(vec![
        Battery {
            label: 0,
            value: "82 %".into(),
            progress: 0.82,
            known: true,
        },
        Battery {
            label: 1,
            value: "79 %".into(),
            progress: 0.79,
            known: true,
        },
    ])));
    let toggles = vec![
        toggle("ldac", "LDAC", true),
        toggle("dolbyAudio", "Dolby audio", false),
        toggle(
            "dualConnections",
            "Одновременное подключение двух устройств",
            true,
        ),
        toggle("sideTone", "Side tone", false),
        toggle("voicePrompt", "Очень длинная подсказка голосом", true),
    ];
    state.set_quick_toggles(ModelRc::new(VecModel::from(toggles.clone())));
    state.set_all_toggles(ModelRc::new(VecModel::from(toggles)));
    state.set_modes(ModelRc::new(VecModel::from(vec![
        ModeOption {
            value: "NoiseCanceling".into(),
            label: "Noise canceling".into(),
        },
        ModeOption {
            value: "Transparency".into(),
            label: "Transparency".into(),
        },
        ModeOption {
            value: "Normal".into(),
            label: "Normal".into(),
        },
    ])));
    state.set_mode_index(0);
    state.set_noise_canceling_label("Noise canceling".into());
    state.set_transparency_label("Transparency".into());
    state.set_normal_label("Normal".into());
    state.set_eq_name("Очень длинный пользовательский профиль звука".into());
    state.set_eq_curve("M 0 50 C 80 10 160 85 240 45 S 400 20 600 55".into());
    state.set_eq_band_count(8);
    state.set_eq_min_text("−12.0".into());
    state.set_eq_max_text("+13.4".into());
    state.set_overview_rows(ModelRc::new(VecModel::from(vec![
        row(
            false,
            vec![
                block(BlockKind::SoundMode, Page::SoundModes),
                block(BlockKind::Equalizer, Page::Equalizer),
            ],
        ),
        row(
            true,
            vec![
                block(BlockKind::Controls, Page::Controls),
                block(BlockKind::Comfort, Page::Comfort),
                block(BlockKind::Spatial, Page::Effects),
                block(BlockKind::Power, Page::Power),
                block(BlockKind::Connections, Page::Connections),
            ],
        ),
    ])));
    state.set_layout_blocks(ModelRc::new(VecModel::from(vec![
        LayoutBlock {
            kind: BlockKind::SoundMode,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Equalizer,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Controls,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Comfort,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Spatial,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Power,
            visible: true,
        },
        LayoutBlock {
            kind: BlockKind::Connections,
            visible: true,
        },
    ])));
    state.set_layout_preset(2);

    state.set_diagnostic_available(true);
    state.set_diagnostic_device_name("soundcore Space One Pro · Demo".into());
    state.set_diagnostic_device_code("SOUNDCORE · A3062".into());
    state.set_diagnostic_device_meta("Demo data · no Bluetooth connection".into());
    state.set_diagnostic_status("Demo".into());
    state.set_diagnostic_probes(ModelRc::new(VecModel::from(vec![
        DiagnosticProbe {
            label: "Device state".into(),
            command: "01 01".into(),
            status: "Answered".into(),
            detail: "128 ms".into(),
            ok: true,
            failed: false,
        },
        DiagnosticProbe {
            label: "Battery and charging".into(),
            command: "01 03".into(),
            status: "Answered".into(),
            detail: "94 ms".into(),
            ok: true,
            failed: false,
        },
        DiagnosticProbe {
            label: "Charging state".into(),
            command: "01 04".into(),
            status: "Timed out".into(),
            detail: "10 s".into(),
            ok: false,
            failed: true,
        },
    ])));
    state.set_diagnostic_answered(2);
    state.set_diagnostic_frames(5);
    state.set_diagnostic_timeouts(1);
    state.set_diagnostic_progress(1.0);
    state.set_diagnostic_archive_size("123 KB".into());
    state.set_diagnostic_file_name("openscl4-diagnostics-A3062.zip".into());
    state.set_diagnostic_note("Описание проблемы без личных данных".into());
}

fn configure(ui: &AppWindow, capture: &Capture) {
    ui.window()
        .set_size(slint::LogicalSize::new(capture.width, capture.height));
    ui.global::<Theme>().set_content_scale(capture.scale);
    ui.global::<Theme>().set_dark(capture.dark);
    let state = ui.global::<AppState>();
    state.set_page(capture.page);
    state.set_diagnostic_step(capture.diagnostic_step);
    state.set_editing_layout(matches!(capture.dialog, Dialog::Layout));
    state.set_choosing_toggles(matches!(capture.dialog, Dialog::Toggles));
    state.set_adding(matches!(capture.dialog, Dialog::AddDevice));
    state.set_add_step(0);
    state.set_eq_saving(matches!(capture.dialog, Dialog::SaveProfile));
    state.set_eq_save_problem(0);
    state.set_creating_preset(matches!(capture.dialog, Dialog::CreatePreset));
    state.set_preset_create_problem(0);
}

fn captures() -> Vec<Capture> {
    let mut result = Vec::new();
    let sizes = [
        (800.0, 700.0),
        (1030.0, 760.0),
        (1280.0, 820.0),
        (1440.0, 900.0),
    ];
    for (width, height) in sizes {
        for scale in [1.0, 1.25, 1.5] {
            for (page, step, label) in [
                (Page::Overview, DiagnosticStep::Device, "overview"),
                (Page::Devices, DiagnosticStep::Device, "devices"),
                (Page::Controls, DiagnosticStep::Device, "controls"),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Device,
                    "diagnostics-device",
                ),
                (Page::SoundModes, DiagnosticStep::Device, "sound-mode"),
            ] {
                result.push(Capture {
                    width,
                    height,
                    scale,
                    dark: false,
                    page,
                    diagnostic_step: step,
                    dialog: Dialog::None,
                    name: format!("{label}-{width}x{height}-{}.light", (scale * 100.0) as i32),
                });
            }
        }
    }
    for dark in [true] {
        for scale in [1.0, 1.5] {
            for (page, step, label) in [
                (Page::Overview, DiagnosticStep::Device, "overview"),
                (Page::SoundModes, DiagnosticStep::Device, "sound-mode"),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Review,
                    "diagnostics-review",
                ),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Capture,
                    "diagnostics-capture",
                ),
            ] {
                result.push(Capture {
                    width: 1280.0,
                    height: 900.0,
                    scale,
                    dark,
                    page,
                    diagnostic_step: step,
                    dialog: Dialog::None,
                    name: format!("{label}-1280x900-{}.dark", (scale * 100.0) as i32),
                });
            }
        }
    }
    for (width, height) in [(800.0, 900.0), (1280.0, 900.0)] {
        for scale in [1.0, 1.5] {
            result.push(Capture {
                width,
                height,
                scale,
                dark: false,
                page: Page::Diagnostics,
                diagnostic_step: DiagnosticStep::Review,
                dialog: Dialog::None,
                name: format!(
                    "diagnostics-review-{width}x{height}-{}.light",
                    (scale * 100.0) as i32
                ),
            });
            for (dialog, label) in [
                (Dialog::Layout, "layout-dialog"),
                (Dialog::Toggles, "toggles-dialog"),
            ] {
                result.push(Capture {
                    width,
                    height,
                    scale,
                    dark: false,
                    page: Page::Overview,
                    diagnostic_step: DiagnosticStep::Device,
                    dialog,
                    name: format!("{label}-{width}x{height}-{}.light", (scale * 100.0) as i32),
                });
            }
        }
    }
    for (width, height) in [(800.0, 900.0), (1280.0, 900.0)] {
        for scale in [1.0, 1.5] {
            for (dialog, page, label) in [
                (Dialog::AddDevice, Page::Devices, "add-device-dialog"),
                (Dialog::SaveProfile, Page::Equalizer, "save-profile-dialog"),
                (Dialog::CreatePreset, Page::Presets, "create-preset-dialog"),
            ] {
                result.push(Capture {
                    width,
                    height,
                    scale,
                    dark: false,
                    page,
                    diagnostic_step: DiagnosticStep::Device,
                    dialog,
                    name: format!("{label}-{width}x{height}-{}.light", (scale * 100.0) as i32),
                });
            }
        }
    }
    result
}

fn save_capture(ui: &AppWindow, output: &PathBuf, capture: &Capture) -> Result<(), String> {
    let snapshot = ui
        .window()
        .take_snapshot()
        .map_err(|error| format!("{}: {error}", capture.name))?;
    let path = output.join(format!(
        "{}-{}x{}.rgba",
        capture.name,
        snapshot.width(),
        snapshot.height()
    ));
    std::fs::write(&path, snapshot.as_bytes())
        .map_err(|error| format!("{}: {error}", path.display()))?;
    println!("{}", path.file_name().unwrap().to_string_lossy());
    Ok(())
}

fn main() -> Result<(), slint::PlatformError> {
    let output = PathBuf::from("../ui-validation");
    std::fs::create_dir_all(&output).expect("create output directory");
    let ui = AppWindow::new()?;
    populate(&ui);
    if std::env::args().any(|argument| argument == "--interactive") {
        configure(
            &ui,
            &Capture {
                width: 1280.0,
                height: 820.0,
                scale: 1.0,
                dark: false,
                page: Page::Overview,
                diagnostic_step: DiagnosticStep::Device,
                dialog: Dialog::None,
                name: "interactive".into(),
            },
        );
        interactive::wire(&ui);
        return ui.run();
    }
    let all = captures();
    configure(&ui, &all[0]);
    ui.show()?;

    let index = Rc::new(Cell::new(0usize));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let captures_for_timer = all.clone();
    let index_for_timer = index.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(ui) = weak.upgrade() else {
                let _ = slint::quit_event_loop();
                return;
            };
            let current = index_for_timer.get();
            if let Err(error) = save_capture(&ui, &output, &captures_for_timer[current]) {
                eprintln!("capture failed: {error}");
                let _ = slint::quit_event_loop();
                return;
            }
            let next = current + 1;
            if next == captures_for_timer.len() {
                let _ = slint::quit_event_loop();
            } else {
                configure(&ui, &captures_for_timer[next]);
                index_for_timer.set(next);
            }
        },
    );
    let result = ui.run();
    drop(timer);
    result
}
