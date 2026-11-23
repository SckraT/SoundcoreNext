slint::include_modules!();

mod interactive;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

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
    /// Render the Liberty 4 Pro (A3954) surface: the 1..=11 scale page and its
    /// wind-noise ModeRow toggle instead of the A3062 three-mode tiles.
    a3954: bool,
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

// A representative settings row of each kind, so the section pages render the
// same content their controller fills for the A3062 capture.
fn setting_row(id: &str, kind: RowKind, label: &str) -> SettingRow {
    SettingRow {
        id: id.into(),
        kind,
        label: label.into(),
        enabled: true,
        reason: "".into(),
        checked: false,
        options: ModelRc::default(),
        index: -1,
        minimum: 0,
        maximum: 0,
        step: 0,
        value: 0,
        text: "".into(),
    }
}

fn toggle_row(id: &str, label: &str, checked: bool) -> SettingRow {
    SettingRow {
        checked,
        ..setting_row(id, RowKind::Toggle, label)
    }
}

fn select_row(id: &str, label: &str, options: Vec<&str>, index: i32) -> SettingRow {
    SettingRow {
        options: ModelRc::new(VecModel::from(
            options
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        )),
        index,
        ..setting_row(id, RowKind::Select, label)
    }
}

fn info_row(id: &str, label: &str, text: &str) -> SettingRow {
    SettingRow {
        text: text.into(),
        ..setting_row(id, RowKind::Info, label)
    }
}

fn panel(kind: PanelKind, rows: Vec<SettingRow>) -> SettingsPanel {
    SettingsPanel {
        kind,
        rows: ModelRc::new(VecModel::from(rows)),
    }
}

// The panels the controller fills for each section page of a connected A3062,
// and the Liberty 4 Pro's own Controls and Case pages when the capture is an
// A3954 one.
fn panels(page: Page, a3954: bool) -> ModelRc<SettingsPanel> {
    if a3954 {
        return ModelRc::new(VecModel::from(a3954_panels(page)));
    }
    let panels = match page {
        Page::Controls => vec![
            panel(
                PanelKind::Presses,
                vec![
                    info_row("singlePress", "Single press", "Ambient sound mode"),
                    select_row(
                        "doublePress",
                        "Double press",
                        vec!["None", "Ambient sound mode", "Voice assistant"],
                        0,
                    ),
                ],
            ),
            panel(
                PanelKind::Cycle,
                vec![
                    toggle_row("noiseCancelingModeInCycle", "Noise canceling", true),
                    toggle_row("transparencyModeInCycle", "Transparency", true),
                    toggle_row("normalModeInCycle", "Normal", false),
                ],
            ),
        ],
        Page::Comfort => vec![
            panel(
                PanelKind::Plain,
                vec![toggle_row("sideTone", "Side tone", true)],
            ),
            panel(
                PanelKind::Volume,
                vec![
                    toggle_row("limitHighVolume", "Limit high volume", false),
                    SettingRow {
                        minimum: 75,
                        maximum: 100,
                        step: 5,
                        value: 90,
                        text: "90".into(),
                        ..setting_row("limitHighVolumeDbLimit", RowKind::Range, "dB limit")
                    },
                    select_row(
                        "limitHighVolumeRefreshRate",
                        "Refresh rate",
                        vec!["Real time", "Every 5 minutes"],
                        0,
                    ),
                ],
            ),
        ],
        Page::Power => vec![panel(
            PanelKind::Plain,
            vec![
                select_row(
                    "autoPowerOff",
                    "Auto power off",
                    vec!["5 minutes", "30 minutes", "60 minutes", "Never"],
                    2,
                ),
                toggle_row("lowBatteryPrompt", "Low battery prompt", true),
                toggle_row("voicePrompt", "Voice prompt", true),
            ],
        )],
        Page::Connections => vec![
            panel(
                PanelKind::Plain,
                vec![toggle_row("dualConnections", "Dual connections", true)],
            ),
            panel(
                PanelKind::DualDevices,
                vec![SettingRow {
                    options: ModelRc::new(VecModel::from(vec![
                        SharedString::from("Demo desktop — connected"),
                        SharedString::from("Demo phone"),
                    ])),
                    ..setting_row("dualDevices", RowKind::List, "Paired devices")
                }],
            ),
        ],
        Page::About => vec![panel(
            PanelKind::Info,
            vec![
                info_row("charging", "Charging", "Not charging"),
                info_row("battery", "Battery", "20 %"),
                info_row("firmware", "Firmware", "04.39"),
            ],
        )],
        Page::Effects => vec![panel(
            PanelKind::Plain,
            vec![
                toggle_row("ldac", "LDAC", true),
                toggle_row("dolbyAudio", "Dolby audio", false),
            ],
        )],
        _ => vec![panel(
            PanelKind::Plain,
            vec![
                toggle_row("ldac", "LDAC", true),
                toggle_row("dualConnections", "Dual connections", true),
            ],
        )],
    };
    ModelRc::new(VecModel::from(panels))
}

// The A3954's own section pages: the twelve measured gestures and the case
// page, with the same ids and English fallbacks the controller supplies.
fn a3954_panels(page: Page) -> Vec<SettingsPanel> {
    let press = |id: &str, label: &str, index: i32| {
        select_row(
            id,
            label,
            vec![
                "None",
                "Volume up",
                "Volume down",
                "Previous song",
                "Next song",
                "Ambient sound mode",
                "Voice assistant",
                "Play/pause",
            ],
            index,
        )
    };
    let slide = |id: &str, label: &str, index: i32| {
        select_row(
            id,
            label,
            vec!["None", "Volume up", "Volume down"],
            index,
        )
    };
    match page {
        Page::Controls => vec![
            panel(
                PanelKind::Presses,
                vec![
                    press("leftSinglePress", "Left single press", 7),
                    press("rightSinglePress", "Right single press", 7),
                    press("leftDoublePress", "Left double press", 3),
                    press("rightDoublePress", "Right double press", 4),
                    press("leftTriplePress", "Left triple press", 0),
                    press("rightTriplePress", "Right triple press", 0),
                    press("leftLongPress", "Left long press", 5),
                    press("rightLongPress", "Right long press", 5),
                ],
            ),
            panel(
                PanelKind::Gestures,
                vec![
                    slide("leftSlideUp", "Left slide up", 1),
                    slide("rightSlideUp", "Right slide up", 1),
                    slide("leftSlideDown", "Left slide down", 2),
                    slide("rightSlideDown", "Right slide down", 2),
                ],
            ),
            panel(
                PanelKind::Cycle,
                vec![
                    toggle_row("noiseCancelingModeInCycle", "Noise canceling", true),
                    toggle_row("transparencyModeInCycle", "Transparency", true),
                    toggle_row("normalModeInCycle", "Normal", false),
                ],
            ),
        ],
        Page::Case => vec![
            panel(
                PanelKind::CaseScreen,
                vec![
                    toggle_row("atmospheric", "Atmospheric", false),
                    toggle_row("remoteCamera", "Remote camera", false),
                    toggle_row("findDevice", "Find device", true),
                    toggle_row("caseSpatialAudio", "Spatial audio on case", false),
                    select_row(
                        "caseLanguage",
                        "Case language",
                        vec!["Japanese", "English", "Chinese"],
                        1,
                    ),
                ],
            ),
            panel(
                PanelKind::CaseStatus,
                vec![
                    info_row("caseBattery", "Case battery", "100%"),
                    info_row("caseFirmware", "Case firmware", "01.58"),
                ],
            ),
        ],
        _ => Vec::new(),
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
    // The Equalizer and Presets pages are captured too: their wide/narrow
    // breakpoints are part of the adaptive matrix.
    state.set_eq_available(true);
    state.set_eq_read_only(false);
    state.set_eq_preset("Signature".into());
    state.set_eq_preset_count(22);
    state.set_eq_presets(ModelRc::new(VecModel::from(vec![
        ModeOption {
            value: "Signature".into(),
            label: "Soundcore Signature".into(),
        },
        ModeOption {
            value: "Acoustic".into(),
            label: "Acoustic".into(),
        },
        ModeOption {
            value: "Bass".into(),
            label: "Bass Booster".into(),
        },
    ])));
    state.set_eq_min(-12);
    state.set_eq_max(13);
    state.set_eq_bands(ModelRc::new(VecModel::from(
        [
            ("100", false, -4),
            ("200", false, 3),
            ("400", false, -2),
            ("800", false, 6),
            ("1.6", true, 1),
            ("3.2", true, -3),
            ("6.4", true, 2),
            ("12.8", true, 5),
        ]
        .into_iter()
        .map(|(frequency, khz, value)| EqBand {
            frequency: frequency.into(),
            khz,
            value,
            text: format!("{value:+.1}").into(),
        })
        .collect::<Vec<_>>(),
    )));
    state.set_eq_lines(ModelRc::new(VecModel::from(vec![
        EqLine {
            value: 13,
            label: "+13.4".into(),
        },
        EqLine {
            value: 0,
            label: "0 dB".into(),
        },
        EqLine {
            value: -12,
            label: "−12.0".into(),
        },
    ])));
    state.set_eq_chart_line("M 0 100 C 80 20 160 170 240 90 S 400 40 600 110".into());
    state.set_eq_chart_area(
        "M 0 100 C 80 20 160 170 240 90 S 400 40 600 110 L 600 200 L 0 200 Z".into(),
    );
    state.set_has_saved_profiles(true);
    state.set_eq_profiles(ModelRc::new(VecModel::from(vec![EqProfile {
        name: "Warm".into(),
        curve: "M 0 50 C 80 10 160 85 240 45 S 400 20 600 55".into(),
        bands: 8,
        in_effect: true,
        export: false,
    }])));
    state.set_preset_cards(ModelRc::new(VecModel::from(vec![
        PresetCard {
            name: "Work".into(),
            status: 2,
            fields: ModelRc::new(VecModel::from(vec![
                SharedString::from("LDAC · on"),
                SharedString::from("Dual connections · on"),
            ])),
            more: 1,
            enabled_count: 3,
            curve: "M 0 50 C 80 10 160 85 240 45 S 400 20 600 55".into(),
        },
        PresetCard {
            name: "Night".into(),
            status: 0,
            fields: ModelRc::new(VecModel::from(vec![SharedString::from(
                "Volume limit · on",
            )])),
            more: 0,
            enabled_count: 1,
            curve: "".into(),
        },
    ])));
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

    // The Settings page's own choices and the section-page asides, so every
    // page in the visual matrix renders populated content.
    state.set_language_names(ModelRc::new(VecModel::from(vec![
        SharedString::from("Follow system"),
        SharedString::from("English"),
        SharedString::from("Русский"),
    ])));
    state.set_app_language(1);
    state.set_app_version("2.0.0".into());
    state.set_app_theme(1);
    state.set_app_content_scale(0);
    state.set_app_device_color(0);
    state.set_app_mode_view(0);
    state.set_app_mode_view_applies(false);
    state.set_one_earbud_badge_applies(false);
    state.set_show_one_earbud_badge(true);
    state.set_app_tray(true);
    state.set_app_tray_available(true);
    state.set_app_close_to_tray(true);
    state.set_tray_battery(true);
    state.set_tray_modes(true);
    state.set_tray_presets(false);
    state.set_tray_ldac(true);
    state.set_has_ldac(true);
    state.set_has_spatial(true);
    state.set_has_sides(false);
    state.set_controls_side(ControlSide::Left);
    // Windows, so the Connections aside renders its system-settings links and the
    // section pages are captured with the aside the real window shows.
    state.set_system_settings_available(true);
    state.set_panels(panels(Page::Controls, false));
}

fn configure(ui: &AppWindow, capture: &Capture) {
    ui.window()
        .set_size(slint::LogicalSize::new(capture.width, capture.height));
    ui.global::<Theme>().set_content_scale(capture.scale);
    ui.global::<Theme>().set_dark(capture.dark);
    let state = ui.global::<AppState>();
    state.set_page(capture.page);
    // The A3954 states show its 1..=11 scale and the wind-noise `ModeRow`
    // toggle instead of the demo A3062 three-mode tiles. The rest of the page
    // data stays the demo data, so the capture shows the real layout.
    state.set_mode_scale(capture.a3954);
    state.set_scale_position(6);
    state.set_airplane_index(-1);
    state.set_airplane_label("".into());
    state.set_mode_rows(if capture.a3954 {
        ModelRc::new(VecModel::from(vec![toggle_row(
            "windNoiseSuppression",
            "Wind noise suppression",
            false,
        )]))
    } else {
        ModelRc::default()
    });
    if capture.a3954 {
        state.set_device_name("soundcore Liberty 4 Pro · Demo".into());
        state.set_device_code("A3954".into());
        // The Case page's aside shows the measured case battery, and the sidebar
        // lists the A3954's own six device pages.
        state.set_case_battery("100%".into());
        state.set_device_pages(ModelRc::new(VecModel::from(vec![
            Page::Controls,
            Page::Comfort,
            Page::Power,
            Page::Connections,
            Page::Case,
            Page::About,
        ])));
    }
    // The Settings page's segmented control has to show the scale the capture
    // renders with, or a 150 % capture would claim the 100 % choice is selected.
    state.set_app_content_scale(if capture.scale >= 1.5 {
        3
    } else if capture.scale >= 1.25 {
        2
    } else if capture.scale >= 1.1 {
        1
    } else {
        0
    });
    state.set_panels(panels(capture.page, capture.a3954));
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
            for (page, step, label, a3954) in [
                (Page::Overview, DiagnosticStep::Device, "overview", false),
                (Page::Devices, DiagnosticStep::Device, "devices", false),
                (Page::Equalizer, DiagnosticStep::Device, "equalizer", false),
                (Page::Presets, DiagnosticStep::Device, "presets", false),
                (Page::Controls, DiagnosticStep::Device, "controls", false),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Device,
                    "diagnostics-device",
                    false,
                ),
                (
                    Page::SoundModes,
                    DiagnosticStep::Device,
                    "sound-mode",
                    false,
                ),
                (Page::Comfort, DiagnosticStep::Device, "comfort", false),
                (Page::Power, DiagnosticStep::Device, "power", false),
                (
                    Page::Connections,
                    DiagnosticStep::Device,
                    "connections",
                    false,
                ),
                (Page::About, DiagnosticStep::Device, "about", false),
                (
                    Page::Effects,
                    DiagnosticStep::Device,
                    "sound-effects",
                    false,
                ),
                (Page::Settings, DiagnosticStep::Device, "settings", false),
                // The Liberty 4 Pro's own sound surface: the 1..=11 scale page
                // with its wind-noise ModeRow toggle.
                (
                    Page::SoundModes,
                    DiagnosticStep::Device,
                    "a3954-scale",
                    true,
                ),
                // The same scale and toggle row inside the Overview's
                // sound-mode block.
                (
                    Page::Overview,
                    DiagnosticStep::Device,
                    "a3954-overview",
                    true,
                ),
                // The Liberty 4 Pro's own Controls and Case pages: the twelve
                // measured gestures and the case screen/language/status.
                (
                    Page::Controls,
                    DiagnosticStep::Device,
                    "a3954-controls",
                    true,
                ),
                (Page::Case, DiagnosticStep::Device, "a3954-case", true),
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
                    a3954,
                });
            }
        }
    }
    for dark in [true] {
        for scale in [1.0, 1.5] {
            for (page, step, label, a3954) in [
                (Page::Overview, DiagnosticStep::Device, "overview", false),
                (
                    Page::SoundModes,
                    DiagnosticStep::Device,
                    "sound-mode",
                    false,
                ),
                (Page::Settings, DiagnosticStep::Device, "settings", false),
                (Page::Controls, DiagnosticStep::Device, "controls", false),
                (Page::Devices, DiagnosticStep::Device, "devices", false),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Review,
                    "diagnostics-review",
                    false,
                ),
                (
                    Page::Diagnostics,
                    DiagnosticStep::Capture,
                    "diagnostics-capture",
                    false,
                ),
                (
                    Page::SoundModes,
                    DiagnosticStep::Device,
                    "a3954-scale",
                    true,
                ),
                (
                    Page::Controls,
                    DiagnosticStep::Device,
                    "a3954-controls",
                    true,
                ),
                (Page::Case, DiagnosticStep::Device, "a3954-case", true),
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
                    a3954,
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
                a3954: false,
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
                    a3954: false,
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
                    a3954: false,
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
                a3954: false,
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
