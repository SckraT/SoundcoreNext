//! Overview layout, summaries, quick toggles and their small preferences file.
//!
//! Block ids and layout rules follow the previous desktop and Android clients:
//! ids are comma-separated, hidden blocks have a `!` prefix, large panels group
//! two to a row and consecutive shortcuts share a row. Device writes stay in the
//! controller; choosing which quick toggles are visible only changes preferences.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use slint::{ModelRc, VecModel};
use soundcore_models::DeviceState;

use crate::{BlockKind, LayoutBlock, OverviewBlockData, OverviewRow, Page, QuickToggle};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverviewBlock {
    SoundMode,
    Equalizer,
    QuickPresets,
    Controls,
    Comfort,
    Spatial,
    Power,
    Connections,
    Case,
}

impl OverviewBlock {
    pub const ALL: [Self; 9] = [
        Self::SoundMode,
        Self::Equalizer,
        Self::QuickPresets,
        Self::Controls,
        Self::Comfort,
        Self::Spatial,
        Self::Power,
        Self::Connections,
        Self::Case,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::SoundMode => "soundMode",
            Self::Equalizer => "equalizer",
            Self::QuickPresets => "quickPresets",
            Self::Controls => "controls",
            Self::Comfort => "comfort",
            // Keep the old id so existing desktop and Android layouts migrate.
            Self::Spatial => "spatial",
            Self::Power => "power",
            Self::Connections => "connections",
            Self::Case => "case",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|block| block.id() == id)
    }

    const fn kind(self) -> BlockKind {
        match self {
            Self::SoundMode => BlockKind::SoundMode,
            Self::Equalizer => BlockKind::Equalizer,
            Self::QuickPresets => BlockKind::QuickPresets,
            Self::Controls => BlockKind::Controls,
            Self::Comfort => BlockKind::Comfort,
            Self::Spatial => BlockKind::Spatial,
            Self::Power => BlockKind::Power,
            Self::Connections => BlockKind::Connections,
            Self::Case => BlockKind::Case,
        }
    }

    const fn page(self) -> Page {
        match self {
            Self::SoundMode => Page::SoundModes,
            Self::Equalizer => Page::Equalizer,
            Self::QuickPresets => Page::Presets,
            Self::Controls => Page::Controls,
            Self::Comfort => Page::Comfort,
            Self::Spatial => Page::Effects,
            Self::Power => Page::Power,
            Self::Connections => Page::Connections,
            Self::Case => Page::Case,
        }
    }

    const fn is_panel(self) -> bool {
        matches!(self, Self::SoundMode | Self::Equalizer | Self::QuickPresets)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewPreset {
    Minimal,
    Standard,
    Full,
}

impl OverviewPreset {
    pub const ALL: [Self; 3] = [Self::Minimal, Self::Standard, Self::Full];

    pub fn from_index(index: i32) -> Option<Self> {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
    }

    const fn index(self) -> i32 {
        match self {
            Self::Minimal => 0,
            Self::Standard => 1,
            Self::Full => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewLayout {
    pub blocks: Vec<(OverviewBlock, bool)>,
}

impl Default for OverviewLayout {
    fn default() -> Self {
        Self::of(OverviewPreset::Standard)
    }
}

impl OverviewLayout {
    pub fn of(preset: OverviewPreset) -> Self {
        let visible = |block| match preset {
            OverviewPreset::Minimal => {
                matches!(block, OverviewBlock::SoundMode | OverviewBlock::Equalizer)
            }
            OverviewPreset::Standard => {
                !matches!(block, OverviewBlock::QuickPresets | OverviewBlock::Power)
            }
            OverviewPreset::Full => true,
        };
        Self {
            blocks: OverviewBlock::ALL
                .into_iter()
                .map(|block| (block, visible(block)))
                .collect(),
        }
    }

    pub fn visible_blocks(&self) -> Vec<OverviewBlock> {
        self.blocks
            .iter()
            .filter(|(_, visible)| *visible)
            .map(|(block, _)| *block)
            .collect()
    }

    pub fn with_visibility(&self, block: OverviewBlock, visible: bool) -> Self {
        Self {
            blocks: self
                .blocks
                .iter()
                .map(|&(candidate, current)| {
                    if candidate == block {
                        (candidate, visible)
                    } else {
                        (candidate, current)
                    }
                })
                .collect(),
        }
    }

    pub fn moved(&self, from: usize, to: usize) -> Self {
        if from >= self.blocks.len() || to >= self.blocks.len() {
            return self.clone();
        }
        let mut blocks = self.blocks.clone();
        let block = blocks.remove(from);
        blocks.insert(to, block);
        Self { blocks }
    }

    /// Moves between two entries of the supported-only dialog list. Unsupported
    /// blocks remain in the stored layout and are skipped by the move.
    pub fn moved_listed(&self, listed: &[OverviewBlock], from: usize, to: usize) -> Option<Self> {
        let position = |block: OverviewBlock| {
            self.blocks
                .iter()
                .position(|(candidate, _)| *candidate == block)
        };
        Some(self.moved(position(*listed.get(from)?)?, position(*listed.get(to)?)?))
    }

    pub fn preset(&self) -> Option<OverviewPreset> {
        OverviewPreset::ALL
            .into_iter()
            .find(|preset| Self::of(*preset) == *self)
    }

    pub fn preset_index(&self) -> i32 {
        self.preset().map_or(-1, OverviewPreset::index)
    }

    pub fn serialize(&self) -> String {
        self.blocks
            .iter()
            .map(|(block, visible)| {
                if *visible {
                    block.id().to_owned()
                } else {
                    format!("!{}", block.id())
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Unknown and duplicate ids are skipped. Blocks added after the saved
    /// value keep their default visibility and are appended in catalog order.
    pub fn parse(stored: Option<&str>) -> Self {
        let Some(stored) = stored.filter(|stored| !stored.trim().is_empty()) else {
            return Self::default();
        };
        let mut blocks = Vec::new();
        for token in stored.split(',') {
            let token = token.trim();
            let hidden = token.starts_with('!');
            if let Some(block) = Self::block_from_token(token)
                && !blocks.iter().any(|(candidate, _)| *candidate == block)
            {
                blocks.push((block, !hidden));
            }
        }
        let missing = Self::default()
            .blocks
            .into_iter()
            .filter(|(block, _)| !blocks.iter().any(|(saved, _)| saved == block))
            .collect::<Vec<_>>();
        blocks.extend(missing);
        Self { blocks }
    }

    fn block_from_token(token: &str) -> Option<OverviewBlock> {
        OverviewBlock::from_id(token.trim_start_matches('!'))
    }
}

/// Whether the current device state has real data and a real destination page
/// for a block. Quick presets and case data are not implemented by this core.
///
/// The A3062 keeps its sound mode in `sound_mode`, its equalizer in
/// `equalizer` and its remaining settings in `settings`; the A3954 keeps all
/// three in its own `a3954` block, so both models are checked before a block is
/// offered.
pub fn supported(block: OverviewBlock, device: &DeviceState) -> bool {
    match block {
        // The A3954's sound-mode block is optional: an unmeasured value (the
        // airplane-mode slider byte) hides the scale block instead of the whole
        // page showing a control the device cannot drive.
        OverviewBlock::SoundMode => {
            device.sound_mode.is_some()
                || device
                    .a3954
                    .as_ref()
                    .is_some_and(|a3954| a3954.sound_mode.is_some())
        }
        OverviewBlock::Equalizer => device.equalizer.is_some() || device.a3954.is_some(),
        OverviewBlock::QuickPresets | OverviewBlock::Case => false,
        OverviewBlock::Controls
        | OverviewBlock::Comfort
        | OverviewBlock::Spatial
        | OverviewBlock::Power
        | OverviewBlock::Connections => device.settings.is_some() || device.a3954.is_some(),
    }
}

fn summary(block: OverviewBlock, device: &DeviceState) -> OverviewBlockData {
    let mut data = OverviewBlockData {
        kind: block.kind(),
        page: block.page(),
        ..OverviewBlockData::default()
    };
    // The A3954's own block carries every setting its shortcut summarizes.
    if let Some(a3954) = &device.a3954 {
        match block {
            OverviewBlock::Controls => data.has_mode_cycle = true,
            OverviewBlock::Comfort => {
                // The three comfort switches the page exposes.
                data.total = 3;
                data.number = i32::from(a3954.wearing_detection)
                    + i32::from(a3954.easy_chat.enabled)
                    + i32::from(a3954.sound_leak_compensation);
            }
            OverviewBlock::Spatial => {
                // LDAC is the compact state label, as on the A3062; the page
                // exposes LDAC, spatial audio and the sound-leak switch.
                data.text = "LDAC".into();
                data.on = a3954.ldac;
            }
            OverviewBlock::Power => {
                data.text = if a3954.auto_power_off.enabled {
                    format!("{} minutes", a3954.auto_power_off.duration.minutes())
                } else {
                    "Disabled".to_owned()
                }
                .into();
            }
            OverviewBlock::Connections => data.on = a3954.dual_connections,
            _ => {}
        }
        return data;
    }
    let Some(settings) = device.settings else {
        return data;
    };
    match block {
        OverviewBlock::Controls => {
            // This model has button presses and an ambient-mode cycle, but no
            // slide gesture settings in the measured state.
            data.has_mode_cycle = true;
        }
        OverviewBlock::Comfort => {
            data.total = 2;
            data.number =
                i32::from(settings.side_tone) + i32::from(settings.limit_high_volume.enabled);
        }
        OverviewBlock::Spatial => {
            // The A3062 has LDAC and Dolby rather than spatial audio. LDAC is
            // the compact state label, while the page exposes both toggles.
            data.text = "LDAC".into();
            data.on = settings.ldac;
        }
        OverviewBlock::Power => {
            data.text = if settings.auto_power_off.enabled {
                format!("{} minutes", settings.auto_power_off.duration.minutes())
            } else {
                "Disabled".to_owned()
            }
            .into();
        }
        OverviewBlock::Connections => data.on = settings.dual_connections,
        _ => {}
    }
    data
}

/// Visible supported blocks in display order. Large panels group at most two
/// to a row; consecutive shortcuts form one responsive grid.
pub fn rows(layout: &OverviewLayout, device: &DeviceState) -> Vec<OverviewRow> {
    let blocks = layout
        .visible_blocks()
        .into_iter()
        .filter(|block| supported(*block, device));
    let mut rows: Vec<(bool, Vec<OverviewBlockData>)> = Vec::new();
    for block in blocks {
        let shortcut = !block.is_panel();
        match rows.last_mut() {
            Some((is_shortcut, items))
                if *is_shortcut == shortcut && (shortcut || items.len() < 2) =>
            {
                items.push(summary(block, device));
            }
            _ => rows.push((shortcut, vec![summary(block, device)])),
        }
    }
    rows.into_iter()
        .map(|(shortcuts, blocks)| OverviewRow {
            shortcuts,
            blocks: ModelRc::new(VecModel::from(blocks)),
        })
        .collect()
}

pub fn listed_blocks(layout: &OverviewLayout, device: &DeviceState) -> Vec<OverviewBlock> {
    layout
        .blocks
        .iter()
        .map(|(block, _)| *block)
        .filter(|block| supported(*block, device))
        .collect()
}

pub fn layout_blocks(layout: &OverviewLayout, device: &DeviceState) -> Vec<LayoutBlock> {
    layout
        .blocks
        .iter()
        .filter(|(block, _)| supported(*block, device))
        .map(|(block, visible)| LayoutBlock {
            kind: block.kind(),
            visible: *visible,
        })
        .collect()
}

/// The quick toggles of the A3062's settings block, in the chooser's order.
pub const TOGGLE_IDS: [&str; 10] = [
    "ldac",
    "dolbyAudio",
    "dualConnections",
    "sideTone",
    "limitHighVolume",
    "lowBatteryPrompt",
    "voicePrompt",
    "noiseCancelingModeInCycle",
    "transparencyModeInCycle",
    "normalModeInCycle",
];

/// The quick toggles the A3954's own settings block can show: the ids that also
/// name an A3954 setting, plus its comfort and spatial switches. The A3954 has
/// no Dolby audio, side tone or voice prompt.
pub const A3954_TOGGLE_IDS: [&str; 11] = [
    "ldac",
    "spatialAudio",
    "dualConnections",
    "wearingDetection",
    "easyChat",
    "soundLeakCompensation",
    "limitHighVolume",
    "lowBatteryPrompt",
    "noiseCancelingModeInCycle",
    "transparencyModeInCycle",
    "normalModeInCycle",
];

pub fn available_toggle_ids(device: &DeviceState) -> Vec<&'static str> {
    if device.settings.is_some() {
        TOGGLE_IDS.to_vec()
    } else if device.a3954.is_some() {
        A3954_TOGGLE_IDS.to_vec()
    } else {
        Vec::new()
    }
}

/// The selected ids, or LDAC by default. Unknown and duplicate stored ids are
/// ignored; an explicitly saved empty selection stays empty.
pub fn quick_toggle_ids(stored: Option<&[String]>, available: &[&str]) -> Vec<String> {
    let default = ["ldac".to_owned()];
    let chosen = stored.unwrap_or(&default);
    let mut ids: Vec<String> = Vec::new();
    for id in chosen {
        let id = id.trim();
        if available.contains(&id) && !ids.iter().any(|candidate| candidate == id) {
            ids.push(id.to_owned());
        }
    }
    ids
}

/// One visible quick toggle, from the A3062 settings block or the A3954 one;
/// `None` for an id the connected model does not have.
fn toggle(device: &DeviceState, id: &str) -> Option<QuickToggle> {
    let (label, on) = if let Some(settings) = device.settings.as_ref() {
        match id {
            "ldac" => ("LDAC", settings.ldac),
            "dolbyAudio" => ("Dolby audio", settings.dolby_audio),
            "dualConnections" => ("Dual connections", settings.dual_connections),
            "sideTone" => ("Side tone", settings.side_tone),
            "limitHighVolume" => ("Limit high volume", settings.limit_high_volume.enabled),
            "lowBatteryPrompt" => ("Low battery prompt", settings.low_battery_prompt),
            "voicePrompt" => ("Voice prompt", settings.voice_prompt),
            "noiseCancelingModeInCycle" => (
                "Noise canceling in mode cycle",
                settings.ambient_sound_mode_cycle.noise_canceling,
            ),
            "transparencyModeInCycle" => (
                "Transparency in mode cycle",
                settings.ambient_sound_mode_cycle.transparency,
            ),
            "normalModeInCycle" => (
                "Normal in mode cycle",
                settings.ambient_sound_mode_cycle.normal,
            ),
            _ => return None,
        }
    } else {
        let a3954 = device.a3954.as_ref()?;
        match id {
            "ldac" => ("LDAC", a3954.ldac),
            "spatialAudio" => ("Spatial audio", a3954.spatial_audio.enabled),
            "dualConnections" => ("Dual connections", a3954.dual_connections),
            "wearingDetection" => ("Wearing detection", a3954.wearing_detection),
            "easyChat" => ("Easy chat", a3954.easy_chat.enabled),
            "soundLeakCompensation" => ("Sound leak compensation", a3954.sound_leak_compensation),
            "limitHighVolume" => ("Limit high volume", a3954.limit_high_volume.enabled),
            "lowBatteryPrompt" => ("Low battery prompt", a3954.low_battery_prompt),
            "noiseCancelingModeInCycle" => (
                "Noise canceling in mode cycle",
                a3954.ambient_sound_mode_cycle.noise_canceling,
            ),
            "transparencyModeInCycle" => (
                "Transparency in mode cycle",
                a3954.ambient_sound_mode_cycle.transparency,
            ),
            "normalModeInCycle" => (
                "Normal in mode cycle",
                a3954.ambient_sound_mode_cycle.normal,
            ),
            _ => return None,
        }
    };
    let one_earbud = device.battery.is_some() && !device.tws_connected;
    let unavailable = one_earbud && id == "ldac";
    Some(QuickToggle {
        id: id.into(),
        label: label.into(),
        on,
        enabled: !unavailable,
        reason: if unavailable {
            "Changing is available when both earbuds are connected".into()
        } else {
            "".into()
        },
    })
}

/// The chips on the device card, and every available toggle with `on` meaning
/// selected for the chooser rather than enabled on the device.
pub fn quick_toggles(
    device: &DeviceState,
    stored: Option<&[String]>,
) -> (Vec<QuickToggle>, Vec<QuickToggle>) {
    let available = available_toggle_ids(device);
    if available.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let selected = quick_toggle_ids(stored, &available);
    let shown = selected
        .iter()
        .filter_map(|id| toggle(device, id))
        .collect();
    let all = available
        .iter()
        .filter_map(|id| {
            toggle(device, id).map(|toggle| QuickToggle {
                on: selected.iter().any(|selected| selected == id),
                ..toggle
            })
        })
        .collect();
    (shown, all)
}

/// Only UI choices live here, never device state. The map key is the catalog
/// model code so two models can have different quick-toggle selections.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewPreferences {
    pub overview_layout: Option<String>,
    pub quick_toggles: BTreeMap<String, Vec<String>>,
}

impl OverviewPreferences {
    pub fn layout(&self) -> OverviewLayout {
        OverviewLayout::parse(self.overview_layout.as_deref())
    }
}

pub fn load_preferences(path: &Path) -> OverviewPreferences {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
            eprintln!(
                "OpenSCL4: ignoring unreadable overview preferences ({}): {error}",
                path.display()
            );
            OverviewPreferences::default()
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => OverviewPreferences::default(),
        Err(error) => {
            eprintln!(
                "OpenSCL4: could not read overview preferences ({}): {error}",
                path.display()
            );
            OverviewPreferences::default()
        }
    }
}

pub fn save_preferences(path: &Path, preferences: &OverviewPreferences) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    let temp = std::path::PathBuf::from(temp);
    let written = (|| {
        let json = serde_json::to_vec_pretty(preferences)?;
        let mut file = fs::File::create(&temp)?;
        file.write_all(&json)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcore_models::{
        A3062Settings, AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffDuration, DbLimit,
        DecibelReadingRefreshRate, FirmwareVersions, HostDevice, LimitHighVolume,
    };

    fn visible(layout: &OverviewLayout, block: OverviewBlock) -> Option<bool> {
        layout
            .blocks
            .iter()
            .find(|(candidate, _)| *candidate == block)
            .map(|(_, visible)| *visible)
    }

    #[test]
    fn layout_roundtrip_and_migration_match_the_previous_clients() {
        let layout = OverviewLayout::default()
            .moved(0, 3)
            .with_visibility(OverviewBlock::Case, false);
        assert_eq!(layout, OverviewLayout::parse(Some(&layout.serialize())));

        let migrated = OverviewLayout::parse(Some("equalizer,!soundMode,removedBlock,equalizer"));
        assert_eq!(
            vec![OverviewBlock::Equalizer, OverviewBlock::SoundMode],
            migrated
                .blocks
                .iter()
                .take(2)
                .map(|(block, _)| *block)
                .collect::<Vec<_>>()
        );
        assert_eq!(Some(false), visible(&migrated, OverviewBlock::SoundMode));
        assert_eq!(OverviewBlock::ALL.len(), migrated.blocks.len());
        assert_eq!(Some(true), visible(&migrated, OverviewBlock::Controls));
    }

    #[test]
    fn presets_and_custom_layouts_are_distinct() {
        assert_eq!(
            vec![OverviewBlock::SoundMode, OverviewBlock::Equalizer],
            OverviewLayout::of(OverviewPreset::Minimal).visible_blocks()
        );
        assert_eq!(
            Some(false),
            visible(&OverviewLayout::default(), OverviewBlock::Power)
        );
        assert_eq!(
            Some(true),
            visible(
                &OverviewLayout::of(OverviewPreset::Full),
                OverviewBlock::Power
            )
        );
        assert_eq!(1, OverviewLayout::default().preset_index());
        assert_eq!(None, OverviewLayout::default().moved(0, 1).preset());
        assert_eq!(None, OverviewPreset::from_index(9));
    }

    #[test]
    fn listed_moves_skip_unsupported_blocks_and_check_bounds() {
        let layout = OverviewLayout::default();
        let listed = [
            OverviewBlock::SoundMode,
            OverviewBlock::Controls,
            OverviewBlock::Comfort,
        ];
        let moved = layout
            .moved_listed(&listed, 0, 1)
            .expect("the move is valid");
        assert_eq!(OverviewBlock::Equalizer, moved.blocks[0].0);
        assert_eq!(OverviewBlock::Controls, moved.blocks[2].0);
        assert_eq!(OverviewBlock::SoundMode, moved.blocks[3].0);
        assert_eq!(OverviewBlock::Comfort, moved.blocks[4].0);
        assert_eq!(None, layout.moved_listed(&listed, 0, 3));
        assert_eq!(layout, layout.moved(0, 99));
    }

    fn settings() -> A3062Settings {
        A3062Settings {
            button_double_press: None,
            ambient_sound_mode_cycle: AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: true,
            },
            low_battery_prompt: true,
            dolby_audio: false,
            ldac: true,
            dual_connections: false,
            auto_power_off: AutoPowerOff {
                enabled: true,
                duration: AutoPowerOffDuration::SixtyMinutes,
            },
            limit_high_volume: LimitHighVolume {
                enabled: false,
                db_limit: DbLimit::new(90).expect("90 is a measured dB limit"),
                refresh_rate: DecibelReadingRefreshRate::RealTime,
            },
            side_tone: true,
            voice_prompt: true,
        }
    }

    fn device_with_settings(settings: Option<A3062Settings>) -> DeviceState {
        DeviceState {
            host_device: HostDevice::Left,
            tws_connected: false,
            battery: None,
            single_battery: None,
            sound_mode: None,
            equalizer: None,
            settings,
            a3954: None,
            firmware: FirmwareVersions {
                left: None,
                right: None,
            },
            serial: None,
        }
    }

    #[test]
    fn quick_toggle_defaults_and_explicit_empty_selection_are_stable() {
        let device = device_with_settings(Some(settings()));
        let available = available_toggle_ids(&device);
        assert_eq!(vec!["ldac"], quick_toggle_ids(None, &available));
        assert!(quick_toggle_ids(Some(&[]), &available).is_empty());
        assert_eq!(
            vec!["ldac", "sideTone"],
            quick_toggle_ids(
                Some(&[
                    "unknown".into(),
                    "ldac".into(),
                    "ldac".into(),
                    " sideTone ".into()
                ]),
                &available,
            )
        );

        let (shown, all) = quick_toggles(&device, None);
        assert_eq!(1, shown.len());
        assert!(shown[0].on, "the chip shows the device's LDAC value");
        assert_eq!(TOGGLE_IDS.len(), all.len());
        assert!(all.iter().find(|toggle| toggle.id == "ldac").unwrap().on);
        assert!(
            !all.iter()
                .find(|toggle| toggle.id == "dolbyAudio")
                .unwrap()
                .on
        );
    }

    #[test]
    fn models_without_settings_have_no_invented_toggles_or_shortcuts() {
        let device = device_with_settings(None);
        let (shown, all) = quick_toggles(&device, None);
        assert!(shown.is_empty());
        assert!(all.is_empty());
        assert!(rows(&OverviewLayout::of(OverviewPreset::Full), &device).is_empty());
        assert!(!supported(OverviewBlock::QuickPresets, &device));
        assert!(!supported(OverviewBlock::Case, &device));
    }

    #[test]
    fn preferences_roundtrip_uses_the_shared_layout_format() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time is after the epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("openscl4-overview-{}-{unique}", std::process::id()));
        let path = directory.join("overview-preferences.json");
        let preferences = OverviewPreferences {
            overview_layout: Some("equalizer,!soundMode".into()),
            quick_toggles: BTreeMap::from([("A3062".into(), vec!["ldac".into()])]),
        };
        save_preferences(&path, &preferences).expect("preferences are written");
        assert_eq!(preferences, load_preferences(&path));

        let mut updated = preferences.clone();
        updated.overview_layout = Some("controls,equalizer,!soundMode".into());
        updated
            .quick_toggles
            .insert("A3062".into(), vec!["sideTone".into(), "ldac".into()]);
        save_preferences(&path, &updated).expect("an existing document is replaced");
        assert_eq!(updated, load_preferences(&path));
        assert!(!directory.join("overview-preferences.json.tmp").exists());

        assert_eq!(
            OverviewPreferences::default(),
            load_preferences(&directory.join("missing.json"))
        );
        fs::remove_dir_all(directory).expect("the temporary preferences are removed");
    }
}
