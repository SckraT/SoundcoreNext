//! The desktop client's own appearance, language and notification-area choices.
//!
//! The Slint page only reports choice indexes. This module owns their validated
//! meaning and the version-tolerant JSON document, so an unknown index cannot
//! silently reset a preference.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tray::TraySettings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::System),
            1 => Some(Self::Light),
            2 => Some(Self::Dark),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    pub const fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    Purple,
    Sage,
    Rose,
}

impl Accent {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Purple),
            1 => Some(Self::Sage),
            2 => Some(Self::Rose),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Purple => 0,
            Self::Sage => 1,
            Self::Rose => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceColor {
    #[default]
    AsTheme,
    Silver,
    Black,
}

impl DeviceColor {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::AsTheme),
            1 => Some(Self::Silver),
            2 => Some(Self::Black),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::AsTheme => 0,
            Self::Silver => 1,
            Self::Black => 2,
        }
    }

    pub const fn is_black(self, dark: bool) -> bool {
        match self {
            Self::AsTheme => dark,
            Self::Silver => false,
            Self::Black => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoundModeView {
    #[default]
    Tiles,
    Scale,
}

impl SoundModeView {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Tiles),
            1 => Some(Self::Scale),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Tiles => 0,
            Self::Scale => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentScale {
    #[default]
    #[serde(rename = "100")]
    Percent100,
    #[serde(rename = "110")]
    Percent110,
    #[serde(rename = "125")]
    Percent125,
}

impl ContentScale {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Percent100),
            1 => Some(Self::Percent110),
            2 => Some(Self::Percent125),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Percent100 => 0,
            Self::Percent110 => 1,
            Self::Percent125 => 2,
        }
    }

    pub const fn factor(self) -> f32 {
        match self {
            Self::Percent100 => 1.0,
            Self::Percent110 => 1.1,
            Self::Percent125 => 1.25,
        }
    }
}

/// The complete contents of `desktop-preferences.json`.
///
/// `flatten` retains the existing flat tray fields, so documents written before
/// appearance settings existed load without a migration or a second file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesktopSettings {
    #[serde(flatten)]
    pub tray: TraySettings,
    pub theme: AppTheme,
    pub accent: Accent,
    pub content_scale: ContentScale,
    pub device_color: DeviceColor,
    pub sound_mode_view: SoundModeView,
    pub one_earbud_badge: bool,
    /// A bundled language code, or `None` to follow Windows.
    pub language: Option<String>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            tray: TraySettings::default(),
            theme: AppTheme::System,
            accent: Accent::Purple,
            content_scale: ContentScale::Percent100,
            device_color: DeviceColor::AsTheme,
            sound_mode_view: SoundModeView::Tiles,
            one_earbud_badge: true,
            language: None,
        }
    }
}

impl DesktopSettings {
    /// Treats a stale or hand-edited language code as Follow system without
    /// discarding the other valid choices in the same document.
    fn normalized(mut self) -> Self {
        if self
            .language
            .as_deref()
            .is_some_and(|code| find_language(code).is_none())
        {
            self.language = None;
        }
        self
    }
}

pub fn load_settings(path: &Path) -> DesktopSettings {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<DesktopSettings>(&text)
            .map(DesktopSettings::normalized)
            .unwrap_or_else(|error| {
                crate::app_log!(
                    "OpenSCL4: ignoring unreadable desktop preferences ({}): {error}",
                    path.display()
                );
                DesktopSettings::default()
            }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => DesktopSettings::default(),
        Err(error) => {
            crate::app_log!(
                "OpenSCL4: could not read desktop preferences ({}): {error}",
                path.display()
            );
            DesktopSettings::default()
        }
    }
}

/// Saves through a flushed sibling temporary file and an atomic replacement.
pub fn save_settings(path: &Path, settings: &DesktopSettings) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    let temp = std::path::PathBuf::from(temp);
    let written = (|| {
        let json = serde_json::to_vec_pretty(settings)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

const fn language(code: &'static str, name: &'static str) -> Language {
    Language { code, name }
}

/// English is the source language; every other entry has a bundled translation.
pub const LANGUAGES: [Language; 16] = [
    language("en", "English"),
    language("arz", "مصري"),
    language("bn", "বাংলা"),
    language("de", "Deutsch"),
    language("es", "Español"),
    language("fr", "Français"),
    language("he", "עברית"),
    language("id", "Bahasa Indonesia"),
    language("it", "Italiano"),
    language("ja", "日本語"),
    language("ko", "한국어"),
    language("pl", "Polski"),
    language("pt-BR", "Português (Brasil)"),
    language("ru", "Русский"),
    language("tr", "Türkçe"),
    language("uk", "Українська"),
];

pub fn find_language(code: &str) -> Option<&'static Language> {
    LANGUAGES
        .iter()
        .find(|language| language.code.eq_ignore_ascii_case(code))
}

/// The optional language at a Select index: zero is Follow system, all other
/// indexes are shifted by one. Invalid indexes change nothing.
pub fn language_from_index(index: i32) -> Option<Option<&'static Language>> {
    if index == 0 {
        return Some(None);
    }
    usize::try_from(index.checked_sub(1)?)
        .ok()
        .and_then(|index| LANGUAGES.get(index))
        .map(Some)
}

pub fn language_index(code: Option<&str>) -> i32 {
    code.and_then(find_language)
        .and_then(|wanted| {
            LANGUAGES
                .iter()
                .position(|language| language.code == wanted.code)
        })
        .map_or(0, |index| index as i32 + 1)
}

/// Chooses the first supported Windows locale, accepting both `ru-RU` and the
/// legacy/special language tags the previous client handled.
pub fn choose_language(requested: &[&str]) -> &'static Language {
    requested
        .iter()
        .find_map(|tag| {
            find_language(tag).or_else(|| {
                let primary = tag.split(['-', '_']).next().unwrap_or_default();
                let primary = match primary.to_ascii_lowercase().as_str() {
                    "iw" => "he",
                    "in" => "id",
                    "ar" => "arz",
                    "pt" => "pt-BR",
                    other => return find_language(other),
                };
                find_language(primary)
            })
        })
        .unwrap_or(&LANGUAGES[0])
}

pub fn system_language() -> &'static Language {
    sys_locale::get_locale()
        .map(|locale| choose_language(&[locale.as_str()]))
        .unwrap_or(&LANGUAGES[0])
}

/// The bundled language the window should show: the saved choice when it is one
/// of [`LANGUAGES`], otherwise the system's language.
pub fn preferred_language(language: Option<&str>) -> &'static Language {
    language
        .and_then(find_language)
        .unwrap_or_else(system_language)
}

/// Reads the real Windows app-mode preference used by Follow system.
#[cfg(windows)]
pub fn system_prefers_dark() -> bool {
    use windows::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
    };
    use windows::core::w;

    let mut light: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `AppsUseLightTheme` is requested as a DWORD and `light` is a
    // writable DWORD buffer of exactly the supplied size.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut light).cast()),
            Some(&raw mut size),
        )
    };
    result == ERROR_SUCCESS && light == 0
}

#[cfg(not(windows))]
pub fn system_prefers_dark() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(label: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openscl4-settings-{}-{sequence}-{label}.json",
            std::process::id()
        ))
    }

    #[test]
    fn choice_indexes_are_strict_and_round_trip() {
        for index in 0..3 {
            assert_eq!(
                AppTheme::from_index(index).map(AppTheme::index),
                Some(index)
            );
            assert_eq!(Accent::from_index(index).map(Accent::index), Some(index));
            assert_eq!(
                DeviceColor::from_index(index).map(DeviceColor::index),
                Some(index)
            );
            assert_eq!(
                ContentScale::from_index(index).map(ContentScale::index),
                Some(index)
            );
        }
        for index in [-1, 3, i32::MAX] {
            assert_eq!(AppTheme::from_index(index), None);
            assert_eq!(Accent::from_index(index), None);
            assert_eq!(DeviceColor::from_index(index), None);
            assert_eq!(ContentScale::from_index(index), None);
        }
        assert_eq!(SoundModeView::from_index(0), Some(SoundModeView::Tiles));
        assert_eq!(SoundModeView::from_index(1), Some(SoundModeView::Scale));
        assert_eq!(SoundModeView::from_index(-1), None);
        assert_eq!(SoundModeView::from_index(2), None);
        assert_eq!(ContentScale::Percent110.factor(), 1.1);
        assert_eq!(ContentScale::Percent125.factor(), 1.25);
    }

    #[test]
    fn language_indexes_and_windows_locale_fallbacks_are_stable() {
        assert_eq!(language_from_index(0), Some(None));
        assert_eq!(
            language_from_index(1)
                .flatten()
                .map(|language| language.code),
            Some("en")
        );
        assert_eq!(
            language_from_index(14)
                .flatten()
                .map(|language| language.code),
            Some("ru")
        );
        assert_eq!(language_from_index(-1), None);
        assert_eq!(language_from_index(17), None);
        assert_eq!(language_index(Some("ru")), 14);
        assert_eq!(language_index(Some("not-a-language")), 0);
        assert_eq!(choose_language(&["ru-RU"]).code, "ru");
        assert_eq!(choose_language(&["pt-PT"]).code, "pt-BR");
        assert_eq!(choose_language(&["iw-IL"]).code, "he");
        assert_eq!(choose_language(&["unknown"]).code, "en");
    }

    #[test]
    fn the_preferred_language_is_the_choice_or_the_system() {
        assert_eq!(preferred_language(Some("ru")).code, "ru");
        assert_eq!(preferred_language(Some("RU")).code, "ru");
        // A stale or hand-edited code cannot stick: Follow system takes over.
        assert_eq!(
            preferred_language(Some("not-a-language")).code,
            system_language().code
        );
        assert_eq!(preferred_language(None).code, system_language().code);
    }

    #[test]
    fn theme_and_art_color_follow_the_real_resolved_theme() {
        assert!(!AppTheme::Light.is_dark(true));
        assert!(AppTheme::Dark.is_dark(false));
        assert!(AppTheme::System.is_dark(true));
        assert!(!AppTheme::System.is_dark(false));
        assert!(DeviceColor::AsTheme.is_black(true));
        assert!(!DeviceColor::AsTheme.is_black(false));
        assert!(DeviceColor::Black.is_black(false));
        assert!(!DeviceColor::Silver.is_black(true));
    }

    #[test]
    fn an_old_flat_tray_document_gets_appearance_defaults() {
        let path = path("legacy");
        fs::write(&path, br#"{"icon":false,"close_to_tray":false}"#).unwrap();
        let loaded = load_settings(&path);
        assert!(!loaded.tray.icon);
        assert!(!loaded.tray.close_to_tray);
        assert_eq!(loaded.theme, AppTheme::System);
        assert_eq!(loaded.accent, Accent::Purple);
        assert_eq!(loaded.content_scale, ContentScale::Percent100);
        assert_eq!(loaded.language, None);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn all_desktop_choices_round_trip_without_a_temporary_file() {
        let path = path("roundtrip");
        let _ = fs::remove_file(&path);
        let settings = DesktopSettings {
            tray: TraySettings {
                icon: false,
                close_to_tray: false,
                battery: false,
                modes: false,
                presets: true,
                ldac: false,
            },
            theme: AppTheme::Dark,
            accent: Accent::Rose,
            content_scale: ContentScale::Percent125,
            device_color: DeviceColor::Black,
            sound_mode_view: SoundModeView::Scale,
            one_earbud_badge: false,
            language: Some("ru".into()),
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(load_settings(&path), settings);
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"icon\": false"), "tray fields stay flat");
        assert!(json.contains("\"theme\": \"dark\""));
        assert!(json.contains("\"contentScale\": \"125\""));
        assert!(!std::path::PathBuf::from(format!("{}.tmp", path.display())).exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn a_missing_document_loads_the_defaults() {
        let missing = path("missing");
        let _ = fs::remove_file(&missing);
        assert_eq!(load_settings(&missing), DesktopSettings::default());
    }

    #[test]
    fn malformed_files_and_unknown_languages_fall_back_safely() {
        let malformed = path("malformed");
        fs::write(&malformed, b"not json").unwrap();
        assert_eq!(load_settings(&malformed), DesktopSettings::default());
        assert_eq!(fs::read_to_string(&malformed).unwrap(), "not json");

        let unknown = path("unknown-language");
        fs::write(
            &unknown,
            br#"{"language":"not-a-language","accent":"sage"}"#,
        )
        .unwrap();
        let loaded = load_settings(&unknown);
        assert_eq!(loaded.language, None);
        assert_eq!(loaded.accent, Accent::Sage);
        let _ = fs::remove_file(malformed);
        let _ = fs::remove_file(unknown);
    }
}
