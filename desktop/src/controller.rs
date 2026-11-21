//! The desktop controller: wires the vendored UI to the Windows Bluetooth
//! transport and the session.
//!
//! The controller keeps every `AppWindow` access on the Slint thread. Bluetooth
//! work happens on worker threads that report results over one `mpsc` channel;
//! a [`slint::Timer`] drains that channel on the UI thread. A connection
//! attempt is short-lived, but when the model reports a sound mode or an
//! equalizer its worker outlives the connect: it keeps the session open and
//! serves writes from a second `mpsc` command channel, dropped by the UI to end
//! the worker and close the connection. Nothing here waits for Bluetooth on the
//! UI thread.

use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use soundcore_models::{
    A3062Settings, AmbientSoundModeCycle, AutoPowerOff, AutoPowerOffDuration, ButtonAction,
    DbLimit, DecibelReadingRefreshRate, DeviceModel, DeviceState, EqualizerPreset, EqualizerState,
    SettingCommand, SoundMode, VISIBLE_EQ_BANDS,
};
use soundcore_profiles::{EqualizerProfile, ImportPreview, ProfileSet, preview_import};
use soundcore_session::{DeviceSession, SessionError, Transport};
use windows::core::GUID;

use crate::catalog_adapter::{self, Chip};
use crate::transport_windows::{
    BluetoothError, VENDOR_SERVICE_PREFIX, WindowsRfcommTransport, format_mac, installed_services,
    list_paired_devices, parse_mac, pick_service, vendor_service,
};
use crate::{
    Actions, AppState, AppWindow, Battery, Connection, DeviceCard, EqBand, EqLine, EqProfile,
    FailureKind, ImportItem, ModeOption, Page, PairedOption, PanelKind, RowKind, SettingRow,
    SettingsPanel, SystemPage, ToastKind, TransferNote,
};

/// The model chosen in the Add Device dialog: what the catalog card shows plus
/// the state parser the session needs.
#[derive(Debug, Clone)]
pub struct SelectedModel {
    /// The catalog code, e.g. `A3949`; the vendor service UUID derives from it.
    pub code: &'static str,
    /// The catalog name, as the vendored model card lists it.
    pub name: &'static str,
    /// The vendored device art of the model's form factor.
    pub art: slint::Image,
    /// Whether the Add Device dialog should word itself for a speaker.
    pub is_speaker: bool,
    /// The state parser for this model, or `None` when the core has no parser
    /// for it yet; such a model can be chosen but not read.
    pub protocol: Option<DeviceModel>,
}

/// One paired device the Add Device dialog can list: its numeric address, the
/// name Windows shows, and whether it probably is the chosen model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The numeric Bluetooth address; the UI gets [`crate::transport_windows::format_mac`].
    pub address: u64,
    /// The device name Windows shows in its own Bluetooth settings.
    pub name: String,
    /// Whether the device probably is the chosen model: it advertises the
    /// model's vendor service, or, when it has no Soundcore vendor service at
    /// all, its name or code matches the catalog entry.
    pub suggested: bool,
}

/// The dialog's row for a paired device.
pub fn paired_option(candidate: &Candidate) -> PairedOption {
    PairedOption {
        mac: format_mac(candidate.address).into(),
        name: candidate.name.clone().into(),
        suggested: candidate.suggested,
    }
}

/// One Overview battery tile: `label` 0 left, 1 right, 2 case, 3 whole device;
/// `raw` is the device's own step value out of `maximum`.
pub fn battery_tile(label: i32, raw: u8, maximum: u8) -> Battery {
    let maximum = maximum.max(1);
    let raw = raw.min(maximum);
    let percent = (u32::from(raw) * 100 + u32::from(maximum) / 2) / u32::from(maximum);
    let progress = f32::from(raw) / f32::from(maximum);
    Battery {
        label,
        value: format!("{percent}%").into(),
        progress,
        known: true,
    }
}

/// The vendored UI's label for the battery of the device as a whole, which it
/// renders as "Battery". Labels 0, 1 and 2 are left, right and case.
pub const WHOLE_DEVICE_BATTERY_LABEL: i32 = 3;

/// The Overview tiles of a parsed state: one whole-device tile for a
/// single-battery layout, otherwise left and right. The TWS CMM2 parser has no
/// case field yet, so the case tile is not invented.
pub fn batteries(state: &DeviceState) -> Vec<Battery> {
    if let Some(single) = &state.single_battery {
        return vec![battery_tile(
            WHOLE_DEVICE_BATTERY_LABEL,
            single.value,
            single.scale.maximum(),
        )];
    }
    let Some(battery) = &state.battery else {
        return Vec::new();
    };
    let maximum = battery.scale.maximum();
    vec![
        battery_tile(0, battery.left, maximum),
        battery_tile(1, battery.right, maximum),
    ]
}

/// The `ModeOption.value` the vendored `ModeText.icon` recognises for a mode.
/// These strings are part of the UI contract, not the protocol values.
pub const fn mode_value(mode: SoundMode) -> &'static str {
    match mode {
        SoundMode::NoiseCanceling => "NoiseCanceling",
        SoundMode::Transparency => "Transparency",
        SoundMode::Normal => "Normal",
    }
}

/// The tile label of a mode, in the words the vendored Sound mode page uses.
pub const fn mode_label(mode: SoundMode) -> &'static str {
    match mode {
        SoundMode::NoiseCanceling => "Noise canceling",
        SoundMode::Transparency => "Transparency",
        SoundMode::Normal => "Normal",
    }
}

/// The `AppState.modes` tiles of a parsed state: the three measured modes in
/// page order, or nothing for a model without a sound-mode control.
pub fn mode_options(sound_mode: Option<SoundMode>) -> Vec<ModeOption> {
    if sound_mode.is_none() {
        return Vec::new();
    }
    SoundMode::ALL
        .into_iter()
        .map(|mode| ModeOption {
            value: mode_value(mode).into(),
            label: mode_label(mode).into(),
        })
        .collect()
}

/// The index of a reported mode in `AppState.modes`, or -1 when the device has
/// no sound mode.
pub fn mode_index(sound_mode: Option<SoundMode>) -> i32 {
    sound_mode
        .and_then(|mode| {
            SoundMode::ALL
                .iter()
                .position(|candidate| *candidate == mode)
        })
        .map_or(-1, |index| index as i32)
}

/// The mode a `Actions.set-option("ambientSoundMode", index)` click names, or
/// `None` for an index outside the three tiles.
pub fn mode_for_index(index: i32) -> Option<SoundMode> {
    usize::try_from(index)
        .ok()
        .and_then(|index| SoundMode::ALL.get(index).copied())
}

/// The sidebar sound pages of a parsed state: Sound mode and Equalizer only when
/// the device reports a measured mode and a supported equalizer, and Effects
/// only when it reports the remaining-settings block (the A3062's LDAC and
/// Dolby toggles). A P20i has no measured sound mode, a deferred equalizer and
/// no settings block, so it shows none of them.
pub fn sound_pages(
    sound_mode: Option<SoundMode>,
    has_equalizer: bool,
    has_settings: bool,
) -> Vec<Page> {
    let mut pages = Vec::new();
    if sound_mode.is_some() {
        pages.push(Page::SoundModes);
    }
    if has_equalizer {
        pages.push(Page::Equalizer);
    }
    if has_settings {
        pages.push(Page::Effects);
    }
    pages
}

/// The sidebar device pages of a state with the remaining-settings block: the
/// A3062's Controls, Comfort, Power, Connections and About. A model without the
/// block (the A3949 and the rest of the catalog) has no pages and no write path.
pub fn device_pages(has_settings: bool) -> Vec<Page> {
    if !has_settings {
        return Vec::new();
    }
    vec![
        Page::Controls,
        Page::Comfort,
        Page::Power,
        Page::Connections,
        Page::About,
    ]
}

/// The A3062 equalizer range in device units (dB×10): -12.0..+13.4 dB. The
/// measured band range is defined once in `soundcore-models`
/// (`EQ_BAND_MIN`/`EQ_BAND_MAX`) and is the same clamp the core's `03 87` body
/// builder applies to a band byte, so the page cannot show a range the device
/// encodes differently.
pub const EQ_MIN: i16 = soundcore_models::EQ_BAND_MIN;
/// See [`EQ_MIN`].
pub const EQ_MAX: i16 = soundcore_models::EQ_BAND_MAX;

/// The measured frequency of each visible A3062 band in hertz, as OpenSCL4's
/// A3062 equalizer settings and the Soundcore app's equalizer chart label them.
pub const EQ_BAND_HZ: [u16; VISIBLE_EQ_BANDS] = [100, 200, 400, 800, 1600, 3200, 6400, 12800];

/// The app's directory under `%APPDATA%`, where it keeps its own files.
pub const APP_DATA_DIR_NAME: &str = "OpenSCL4";

/// The saved equalizer profiles document inside [`APP_DATA_DIR_NAME`].
pub const PROFILES_FILE_NAME: &str = "equalizer-profiles.json";

/// The saved-profile document's path inside an app-data directory:
/// `<app-data>\OpenSCL4\equalizer-profiles.json`.
pub fn app_data_profiles_path(app_data: &Path) -> PathBuf {
    app_data.join(APP_DATA_DIR_NAME).join(PROFILES_FILE_NAME)
}

/// The saved-profile document's path from the Windows `%APPDATA%` variable, or
/// `None` when the variable is missing (the profiles then stay in memory only).
///
/// The standard library environment is deliberately used instead of a
/// directory-lookup crate: the app targets Windows, where `%APPDATA%` is the
/// roaming app-data directory the spec names.
pub fn default_profiles_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|app_data| app_data_profiles_path(Path::new(&app_data)))
}

/// The temporary sibling file an atomic save writes before renaming it over the
/// document: `<document>.tmp`.
fn temporary_profile_path(path: &Path) -> PathBuf {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    PathBuf::from(temp)
}

/// Logs one saved-profile file message to stderr. The app has no log view, and
/// the spec asks the file problems to be reported without disturbing the page.
fn log_profiles(message: &str, path: &Path) {
    eprintln!("OpenSCL4: {message}: {}", path.display());
}

/// Logs that the shell has no app-data directory, so the saved profiles stay in
/// memory only and are lost when the app closes. The startup load and a save use
/// this one wording.
fn log_no_app_data_directory() {
    eprintln!("OpenSCL4: no app-data directory; saved equalizer profiles will not persist");
}

/// Loads the saved profile set from `path`.
///
/// A missing document is a normal first run and loads the empty set. Any other
/// read or parse failure is logged to stderr and also loads the empty set, so a
/// malformed document never crashes the app or blocks it from starting; the file
/// itself is left untouched until the next save replaces it.
pub fn load_profiles(path: &Path) -> ProfileSet {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log_profiles("no saved equalizer profiles yet", path);
            return ProfileSet::new();
        }
        Err(error) => {
            log_profiles(
                &format!("could not read the saved equalizer profiles ({error})"),
                path,
            );
            return ProfileSet::new();
        }
    };
    match ProfileSet::from_json(&text) {
        Ok(set) => set,
        Err(error) => {
            log_profiles(
                &format!("ignoring an unreadable saved equalizer profiles file ({error})"),
                path,
            );
            ProfileSet::new()
        }
    }
}

/// Writes `set` to `path` atomically: the JSON goes to a temporary sibling file,
/// is flushed to disk, and is then renamed over the document. A crash or a full
/// disk therefore never leaves a half-written profile document behind.
pub fn save_profiles(path: &Path, set: &ProfileSet) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let temp = temporary_profile_path(path);
    let written = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(set.to_json().as_bytes())?;
        file.sync_all()?;
        drop(file);
        // A rename inside one directory is atomic on Windows and replaces the
        // previous document as a whole.
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        // Best effort: a failed save must not leave its temporary file behind.
        let _ = fs::remove_file(&temp);
    }
    written
}

/// The chart's line through eight raw dB×10 values, in the 600×100 path box the
/// vendored `ProfileCurve` draws a saved profile's curve in.
pub fn profile_curve(bands: &[i16]) -> String {
    smooth_path(&curve_points(bands, EQ_MIN, EQ_MAX, PROFILE_CURVE_HEIGHT))
}

/// The `AppState.eq-profiles` rows of the saved set against the bands the page
/// currently shows: each row carries the profile's curve, its eight-band count,
/// and whether it is applied (its bands are exactly the visible bands).
///
/// Export stays false: import/export is out of scope for this slice and its
/// checkboxes never activate.
pub fn profile_rows(profiles: &ProfileSet, current_bands: &[i16]) -> Vec<EqProfile> {
    profiles
        .iter()
        .map(|profile| EqProfile {
            name: profile.name.clone().into(),
            curve: profile_curve(&profile.bands).into(),
            bands: i32::try_from(VISIBLE_EQ_BANDS).unwrap_or(0),
            in_effect: profile.bands.as_slice() == current_bands,
            export: false,
        })
        .collect()
}

/// The `AppState.eq-presets` chips of a model: one per built-in preset, in the
/// catalog's display order. The chip value is the decimal preset id the state
/// reports, so a click names exactly one catalog entry (the page compares the
/// value with `eq-preset`, which is also the id string); the label is the
/// preset's name. A model whose equalizer this slice does not implement (the
/// deferred A3949) gets no chips.
pub fn preset_options(model: Option<DeviceModel>) -> Vec<ModeOption> {
    model
        .map(DeviceModel::equalizer_presets)
        .unwrap_or_default()
        .iter()
        .map(|preset| ModeOption {
            value: preset.id.to_string().into(),
            label: preset.name.into(),
        })
        .collect()
}

/// The preset id a chip value names, when the page offers that value. Anything
/// else is ignored, so an unknown or stale value never writes.
pub fn selectable_preset_id(presets: &[ModeOption], value: &str) -> Option<u16> {
    let id = value.parse::<u16>().ok()?;
    presets
        .iter()
        .any(|preset| preset.value.as_str() == value)
        .then_some(id)
}

/// The `EqBand.frequency` text and whether it is in kilohertz: `100` is
/// ("100", false), `1600` is ("1.6", true), `12800` is ("12.8", true).
pub fn band_frequency(hz: u16) -> (String, bool) {
    if hz < 1000 {
        return (hz.to_string(), false);
    }
    let khz = format!("{}.{:03}", hz / 1000, hz % 1000);
    (
        khz.trim_end_matches('0').trim_end_matches('.').to_string(),
        true,
    )
}

/// A band value in decibels as the value field shows it: 134 is "13.4", -120
/// is "-12", 0 is "0". The device stores tenths of a decibel.
pub fn band_value_text(value: i16) -> String {
    let magnitude = i32::from(value).unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    let (whole, fraction) = (magnitude / 10, magnitude % 10);
    if fraction == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

/// A value with a sign and one decimal digit, as the chart's guide labels and
/// the range note show it: "+13.4", "0.0", "−12.0". The minus is U+2212 as in
/// the vendored strings.
pub fn signed_decibels(value: i16) -> String {
    let magnitude = i32::from(value).unsigned_abs();
    let sign = match value.signum() {
        1 => "+",
        -1 => "\u{2212}",
        _ => "",
    };
    format!("{sign}{}.{}", magnitude / 10, magnitude % 10)
}

/// The closest value a band may take in device units (dB×10): the measured
/// `-12.0..+13.4 dB` range, i.e. the `0..=254` band-byte range minus the
/// [`EQ_MIN`] offset.
pub fn clamp_band_value(value: i32) -> i16 {
    value.clamp(i32::from(EQ_MIN), i32::from(EQ_MAX)) as i16
}

/// The device value a typed band field names, in dB×10 units, or `None` when
/// the text is not one the device can store.
///
/// The field shows decibels with at most one fraction digit ("13.4", "-12",
/// "0"), so a point or a comma, an optional sign (`-`, `+` or the vendored
/// minus U+2212) and at most one non-zero fraction digit are accepted. The
/// value is not range-checked: the commit path clamps it, exactly like a
/// dragged value.
pub fn parse_band_text(text: &str) -> Option<i32> {
    let text = text.trim().replace(',', ".");
    let (negative, digits) = match text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('\u{2212}'))
    {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(&text)),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty()
        || !whole
            .chars()
            .chain(fraction.chars())
            .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    // The device stores tenths of a decibel: digits past the first fraction
    // digit may only be zeros.
    let (kept, dropped) = fraction.split_at(fraction.len().min(1));
    if dropped.chars().any(|character| character != '0') {
        return None;
    }
    let whole: i32 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: i32 = kept.parse().unwrap_or(0);
    let units = whole.checked_mul(10)?.checked_add(fraction)?;
    Some(if negative { -units } else { units })
}

/// The visible bands of a parsed state: the first eight band values with their
/// measured frequencies.
pub fn equalizer_bands(equalizer: &EqualizerState) -> Vec<EqBand> {
    equalizer_bands_from_values(&equalizer_values(equalizer))
}

/// The `EqBand` rows of eight visible band values in raw dB×10 units.
fn equalizer_bands_from_values(values: &[i16]) -> Vec<EqBand> {
    EQ_BAND_HZ
        .iter()
        .zip(values.iter())
        .map(|(hz, value)| {
            let (frequency, khz) = band_frequency(*hz);
            EqBand {
                frequency: frequency.into(),
                khz,
                value: i32::from(*value),
                text: band_value_text(*value).into(),
            }
        })
        .collect()
}

/// The chart's guides: the range ends, half of each, and zero, with their
/// decibel labels.
pub fn equalizer_lines() -> Vec<EqLine> {
    [EQ_MAX, 70, 0, -60, EQ_MIN]
        .into_iter()
        .map(|value| EqLine {
            value: i32::from(value),
            label: signed_decibels(value).into(),
        })
        .collect()
}

/// The chart's path commands are in a box of this size; the vendored chart
/// stretches it to its own size.
const CHART_WIDTH: f32 = 600.0;
const CHART_HEIGHT: f32 = 200.0;
/// The saved-profile thumbnail's path box height; its width is [`CHART_WIDTH`].
/// The vendored `ProfileCurve` draws its commands in a 600×100 viewbox.
const PROFILE_CURVE_HEIGHT: f32 = 100.0;
/// The part of the chart's height kept above the highest and below the lowest
/// value, so the points are not cut off. `EqualizerChart` uses the same margin.
const CHART_MARGIN: f32 = 0.06;

/// The chart's points for `values` in raw dB×10: `x` spreads the bands evenly
/// across [`CHART_WIDTH`], `y` maps each value into the `height` box keeping
/// [`CHART_MARGIN`] free at both ends.
fn curve_points(values: &[i16], min: i16, max: i16, height: f32) -> Vec<(f32, f32)> {
    let count = values.len() as f32;
    values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| {
            let x = (index as f32 + 0.5) / count;
            let span = f32::from((max - min).max(1));
            let fraction = f32::from(value.clamp(min, max) - min) / span;
            let y = CHART_MARGIN + (1.0 - fraction) * (1.0 - 2.0 * CHART_MARGIN);
            (x * CHART_WIDTH, y * height)
        })
        .collect()
}

/// The curve through `values` and the area under it, as path commands for the
/// vendored chart. Empty when there is nothing to draw.
pub fn equalizer_chart(values: &[i16], min: i16, max: i16) -> (String, String) {
    if values.is_empty() || max <= min {
        return (String::new(), String::new());
    }
    let points = curve_points(values, min, max, CHART_HEIGHT);
    let line = smooth_path(&points);
    let first = points[0].0;
    let last = points[points.len() - 1].0;
    // The fill walks the same segments as the line, but starts and ends at its
    // own bottom corners so a flat run has no gap between fill and stroke.
    let area = format!(
        "M {first:.1} {CHART_HEIGHT:.1} L {first:.1} {:.1}{} L {last:.1} {CHART_HEIGHT:.1} Z",
        points[0].1,
        smooth_segments(&points),
    );
    (line, area)
}

/// A path through `points`, smoothed so it follows the values instead of
/// stepping between them (Fritsch–Carlson: zero slope at a turning point, the
/// harmonic mean of the neighbouring slopes elsewhere, so it never overshoots a
/// band's value).
fn smooth_path(points: &[(f32, f32)]) -> String {
    let Some(first) = points.first() else {
        return String::new();
    };
    format!("M {:.1} {:.1}{}", first.0, first.1, smooth_segments(points))
}

/// The `C` commands that continue a path through `points`; empty when there is
/// nothing to join.
fn smooth_segments(points: &[(f32, f32)]) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let slopes = slopes(points);
    let mut segments = String::new();
    for (index, pair) in points.windows(2).enumerate() {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let dx = x1 - x0;
        let control0 = (x0 + dx / 3.0, y0 + slopes[index] * dx / 3.0);
        let control1 = (x1 - dx / 3.0, y1 - slopes[index + 1] * dx / 3.0);
        segments.push_str(&format!(
            " C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            control0.0, control0.1, control1.0, control1.1, x1, y1,
        ));
    }
    segments
}

/// The slope of the curve at each point, one per point.
fn slopes(points: &[(f32, f32)]) -> Vec<f32> {
    let between: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            if x1 > x0 { (y1 - y0) / (x1 - x0) } else { 0.0 }
        })
        .collect();
    let mut slopes = Vec::with_capacity(points.len());
    // The ends keep the slope of their only neighbouring segment.
    slopes.push(between[0]);
    for index in 1..between.len() {
        let (before, after) = (between[index - 1], between[index]);
        slopes.push(if before * after <= 0.0 {
            0.0
        } else {
            2.0 * before * after / (before + after)
        });
    }
    slopes.push(between[between.len() - 1]);
    slopes
}

/// Everything the vendored Equalizer page shows for a parsed state, worked out
/// on the worker's result so the UI thread only copies it into `AppState`.
#[derive(Debug, Clone, Default)]
pub struct EqualizerDisplay {
    /// Whether the device reported an equalizer this slice can show.
    pub available: bool,
    /// Whether the page's band controls are enabled, and with them the
    /// saved-profile panel and its Save button. Only a model whose custom
    /// equalizer body this slice can write (the A3062) is editable: every other
    /// model with an equalizer would get a read-only page without saved profiles.
    pub editable: bool,
    /// The preset chips, their count, and the id string of the current preset
    /// (`""` when the device reports custom bands).
    pub presets: Vec<ModeOption>,
    pub preset_count: i32,
    pub preset: String,
    /// The band values, their count, and the decibel range.
    pub band_count: i32,
    pub min: i32,
    pub max: i32,
    pub min_text: String,
    pub max_text: String,
    pub bands: Vec<EqBand>,
    pub lines: Vec<EqLine>,
    pub chart_line: String,
    pub chart_area: String,
    /// The name of the current built-in preset, empty for custom bands.
    pub name: String,
}

impl EqualizerDisplay {
    /// The page data of a parsed state: presets from `model`'s catalog, the
    /// current preset and its curve from `equalizer`, and whether its band
    /// controls may write a custom curve.
    pub fn new(model: Option<DeviceModel>, equalizer: Option<&EqualizerState>) -> Self {
        let Some(equalizer) = equalizer else {
            return Self::default();
        };
        let editable = model
            .and_then(|model| model.custom_equalizer_body(equalizer, [0; VISIBLE_EQ_BANDS]))
            .is_some();
        let presets = preset_options(model);
        let bands = equalizer_bands(equalizer);
        let (chart_line, chart_area) =
            equalizer_chart(&equalizer_values(equalizer), EQ_MIN, EQ_MAX);
        let built_in = equalizer.preset();
        Self {
            available: true,
            editable,
            preset_count: i32::try_from(presets.len()).unwrap_or(0),
            presets,
            preset: equalizer.preset_id.to_string(),
            band_count: i32::try_from(bands.len()).unwrap_or(0),
            min: i32::from(EQ_MIN),
            max: i32::from(EQ_MAX),
            min_text: signed_decibels(EQ_MIN),
            max_text: signed_decibels(EQ_MAX),
            bands,
            lines: equalizer_lines(),
            chart_line,
            chart_area,
            name: built_in
                .map(|preset| preset.name.to_owned())
                .unwrap_or_default(),
        }
    }
}

/// The `EqBand` values of a state, as integers for the chart.
fn equalizer_values(equalizer: &EqualizerState) -> Vec<i16> {
    equalizer
        .bands
        .iter()
        .take(VISIBLE_EQ_BANDS)
        .copied()
        .collect()
}

/// One edit of the Equalizer page's bands, as the vendored page's actions
/// report it. A preview only moves the page; a commit and a reset also write
/// the whole eight-band curve to the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandAction {
    /// `Actions.eq-preview(index, value)`: a drag. The page follows at once and
    /// the device is not told; the release sends a commit.
    Preview { index: i32, value: i32 },
    /// `Actions.eq-commit(index, value)`: a slider release, arrow key or typed
    /// value. The page follows and the curve is written.
    Commit { index: i32, value: i32 },
    /// `Actions.eq-reset()`: the flat 0 dB curve is written.
    Reset,
}

/// The Equalizer page fields a band edit changes. The real implementation is
/// the Slint `AppState`; the tests record the same calls so the wiring can be
/// checked without a window.
trait EqPage {
    /// The eight band values the page currently shows, in raw dB×10 units.
    fn shown_band_values(&self) -> Vec<i16>;
    /// Shows `values` as the eight band rows and the curve through them.
    fn show_curve(&self, values: &[i16]);
    /// Shows the equalizer state the device reported: its preset id and name
    /// (empty for custom bands) and its curve.
    fn show_reported_curve(&self, equalizer: &EqualizerState);
    /// Shows a readable error banner label; `""` clears the banner.
    fn show_error(&self, label: &str);
}

impl EqPage for AppState<'_> {
    fn shown_band_values(&self) -> Vec<i16> {
        let bands: Vec<EqBand> = self.get_eq_bands().iter().collect();
        bands
            .iter()
            .map(|band| clamp_band_value(band.value))
            .collect()
    }

    fn show_curve(&self, values: &[i16]) {
        let (chart_line, chart_area) = equalizer_chart(values, EQ_MIN, EQ_MAX);
        self.set_eq_bands(ModelRc::new(VecModel::from(equalizer_bands_from_values(
            values,
        ))));
        self.set_eq_chart_line(chart_line.into());
        self.set_eq_chart_area(chart_area.into());
    }

    fn show_reported_curve(&self, equalizer: &EqualizerState) {
        show_equalizer_state(self, equalizer);
    }

    fn show_error(&self, label: &str) {
        self.set_error_label(label.into());
    }
}

/// Queues a custom eight-band curve write on the connected device worker. The
/// real implementation is [`Inner`]; the tests record the curves instead.
trait BandWriter {
    /// Queues `bands`; false when the worker is gone (a failed or stopped
    /// connection), so the page can say why nothing was written.
    fn write_curve(&self, bands: [i16; VISIBLE_EQ_BANDS]) -> bool;
}

impl BandWriter for Inner {
    fn write_curve(&self, bands: [i16; VISIBLE_EQ_BANDS]) -> bool {
        self.send_queued(&self.curve_change, |request| {
            DeviceCommand::WriteCustomCurve { request, bands }
        })
    }
}

/// What an `Actions.eq-apply(name)` click did with a saved profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyOutcome {
    /// The click names no saved profile; nothing was queued.
    Unknown,
    /// The profile's curve was queued on the device worker.
    Queued,
    /// The profile exists but the worker is gone (a failed or stopped
    /// connection), so nothing was written.
    Disconnected,
}

/// Queues a saved profile's bands through the same custom-curve command a band
/// commit uses: the worker writes exactly one `03 87` frame built from its last
/// re-polled state, then re-polls `01 01`.
fn apply_saved_profile(set: &ProfileSet, name: &str, writer: &impl BandWriter) -> ApplyOutcome {
    let Some(profile) = set.get(name) else {
        return ApplyOutcome::Unknown;
    };
    if writer.write_curve(profile.bands) {
        ApplyOutcome::Queued
    } else {
        ApplyOutcome::Disconnected
    }
}

/// The eight band values after `index` is set to `value`, clamped to the
/// device's range; `None` when the page does not show that band.
fn edited_band_values(shown: &[i16], index: i32, value: i32) -> Option<Vec<i16>> {
    let mut values = shown.to_vec();
    let band = values.get_mut(usize::try_from(index).ok()?)?;
    *band = clamp_band_value(value);
    Some(values)
}

/// Answers one Equalizer page band action on the page, then queues the write a
/// commit or a reset asks for. Returns whether a write was queued: a preview
/// only moves the page and never writes.
fn answer_band_action(page: &impl EqPage, writer: &impl BandWriter, action: BandAction) -> bool {
    match action {
        BandAction::Preview { index, value } => {
            let Some(values) = edited_band_values(&page.shown_band_values(), index, value) else {
                return false;
            };
            page.show_curve(&values);
            false
        }
        BandAction::Commit { index, value } => {
            let Some(values) = edited_band_values(&page.shown_band_values(), index, value) else {
                return false;
            };
            page.show_curve(&values);
            write_curve(page, writer, &values)
        }
        BandAction::Reset => {
            let values = vec![0i16; VISIBLE_EQ_BANDS];
            page.show_curve(&values);
            write_curve(page, writer, &values)
        }
    }
}

/// Queues the whole visible curve; a stopped worker becomes the readable error
/// instead of a silent no-op.
fn write_curve(page: &impl EqPage, writer: &impl BandWriter, values: &[i16]) -> bool {
    let Ok(bands) = <[i16; VISIBLE_EQ_BANDS]>::try_from(values) else {
        return false;
    };
    if writer.write_curve(bands) {
        true
    } else {
        page.show_error(EqFailure::Disconnected.bands_label());
        false
    }
}

/// Applies a finished custom-curve change to the page: the state the device
/// reports, or the readable label of what failed. A failed change stops the
/// worker (`stop`), as every failed equalizer change does.
fn apply_curve_outcome(
    page: &impl EqPage,
    stop: &impl Fn(),
    outcome: Result<EqualizerState, EqFailure>,
) {
    match outcome {
        Ok(reported) => {
            page.show_reported_curve(&reported);
            page.show_error("");
        }
        Err(failure) => {
            stop();
            page.show_error(failure.bands_label());
        }
    }
}

/// Whether the page's band controls may act: a connected device whose
/// equalizer is editable. A model without a custom equalizer body (the A3949)
/// has no page and no editable controls, so no band frame can be written for
/// it.
fn band_edit_allowed(state: &AppState<'_>) -> bool {
    state.get_connection() == Connection::Connected
        && state.get_eq_available()
        && !state.get_eq_read_only()
}

/// Answers a band action on the window's page when it is alive and editable,
/// then refreshes the saved-profile rows: the profile shown as applied changes
/// as soon as the page's curve moves away from (or back to) its bands.
fn answer_band_edit(inner: &Rc<Inner>, action: BandAction) {
    let Some(window) = inner.ui.upgrade() else {
        return;
    };
    let state = window.global::<AppState>();
    if !band_edit_allowed(&state) {
        return;
    }
    answer_band_action(&state, &**inner, action);
    show_saved_profiles(&state, &inner.profiles.borrow());
}

/// Why a sound-mode change did not take, in the words the vendored error banner
/// shows above the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeFailure {
    /// The device did not answer the write or the state re-read in time.
    TimedOut,
    /// The Bluetooth link dropped before the new mode was confirmed.
    Disconnected,
    /// The device answered with something this app cannot read.
    Unreadable,
}

impl ModeFailure {
    /// Reduces a session failure to the case the page has words for.
    pub fn from_session(error: SessionError) -> Self {
        match error {
            SessionError::Timeout => Self::TimedOut,
            SessionError::Io(_) => Self::Disconnected,
            // A change is only written for a model whose state reported a sound
            // mode, so the session always has a `06 81` body to write; the
            // write-only variants keep the mapping total.
            SessionError::Protocol(_)
            | SessionError::UnsupportedLayout
            | SessionError::UnsupportedSoundMode
            | SessionError::UnsupportedEqualizer
            | SessionError::UnsupportedSetting => Self::Unreadable,
        }
    }

    /// The `AppState.error-label` of a failed change; the vendored banner wraps
    /// it as "Could not change “{}”.".
    pub fn label(self) -> &'static str {
        match self {
            Self::TimedOut => "Sound mode (no answer)",
            Self::Disconnected => "Sound mode (connection lost)",
            Self::Unreadable => "Sound mode (unreadable answer)",
        }
    }
}

/// Writes the mode and re-reads the state, returning the mode the device
/// reports. Blocking: only the device worker calls this, never the UI thread.
pub fn change_sound_mode<T: Transport>(
    session: &mut DeviceSession<T>,
    mode: SoundMode,
) -> Result<SoundMode, ModeFailure> {
    session
        .set_sound_mode(mode)
        .map_err(ModeFailure::from_session)?;
    let state = session.poll_state().map_err(ModeFailure::from_session)?;
    state.sound_mode.ok_or(ModeFailure::Unreadable)
}

/// Why an equalizer preset change did not take, in the words the vendored error
/// banner shows above the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqFailure {
    /// The device did not answer the write or the state re-read in time.
    TimedOut,
    /// The Bluetooth link dropped before the new preset was confirmed.
    Disconnected,
    /// The device answered with something this app cannot read.
    Unreadable,
}

impl EqFailure {
    /// Reduces a session failure to the case the page has words for.
    pub fn from_session(error: SessionError) -> Self {
        match error {
            SessionError::Timeout => Self::TimedOut,
            SessionError::Io(_) => Self::Disconnected,
            // A change is only written for a model whose state reported an
            // equalizer, so the session always has a `03 87` body to write; the
            // write-only variants keep the mapping total.
            SessionError::Protocol(_)
            | SessionError::UnsupportedLayout
            | SessionError::UnsupportedSoundMode
            | SessionError::UnsupportedEqualizer
            | SessionError::UnsupportedSetting => Self::Unreadable,
        }
    }

    /// The banner labels of a failed equalizer change: the preset wording and
    /// the custom-band wording. They share one table so the two settings cannot
    /// drift apart; [`Self::label`] and [`Self::bands_label`] pick one each.
    fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::TimedOut => (
                "Equalizer profile (no answer)",
                "Equalizer bands (no answer)",
            ),
            Self::Disconnected => (
                "Equalizer profile (connection lost)",
                "Equalizer bands (connection lost)",
            ),
            Self::Unreadable => (
                "Equalizer profile (unreadable answer)",
                "Equalizer bands (unreadable answer)",
            ),
        }
    }

    /// The `AppState.error-label` of a failed preset change; the vendored banner
    /// wraps it as "Could not change “{}”.".
    pub fn label(self) -> &'static str {
        self.labels().0
    }

    /// The `AppState.error-label` of a failed custom-curve change, worded like
    /// [`Self::label`] but naming the bands that were being written.
    pub fn bands_label(self) -> &'static str {
        self.labels().1
    }
}

/// Writes `preset` and re-reads the state, returning the equalizer the device
/// reports. `current` is the last state the worker read: the write keeps its
/// HearID block, exactly as OpenSCL4's A3062 equalizer does.
///
/// Blocking: only the device worker calls this, never the UI thread.
pub fn change_equalizer_preset<T: Transport>(
    session: &mut DeviceSession<T>,
    current: &EqualizerState,
    preset: &EqualizerPreset,
) -> Result<EqualizerState, EqFailure> {
    session
        .set_equalizer_preset(current, preset)
        .map_err(EqFailure::from_session)?;
    let state = session.poll_state().map_err(EqFailure::from_session)?;
    state.equalizer.ok_or(EqFailure::Unreadable)
}

/// Writes the eight visible bands as a custom curve, re-reads the state and
/// returns the equalizer the device reports. `current` is the last state the
/// worker read: the write keeps its HearID block, exactly as OpenSCL4's A3062
/// equalizer does.
///
/// Blocking: only the device worker calls this, never the UI thread.
pub fn change_custom_equalizer<T: Transport>(
    session: &mut DeviceSession<T>,
    current: &EqualizerState,
    visible_bands: [i16; VISIBLE_EQ_BANDS],
) -> Result<EqualizerState, EqFailure> {
    session
        .set_custom_equalizer(current, visible_bands)
        .map_err(EqFailure::from_session)?;
    let state = session.poll_state().map_err(EqFailure::from_session)?;
    state.equalizer.ok_or(EqFailure::Unreadable)
}

/// Why a remaining-settings write did not take, in the words the vendored error
/// banner shows above the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingFailure {
    /// The device did not answer the write or the state re-read in time.
    TimedOut,
    /// The Bluetooth link dropped before the new value was confirmed.
    Disconnected,
    /// The device answered with something this app cannot read.
    Unreadable,
}

impl SettingFailure {
    /// Reduces a session failure to the case the page has words for.
    pub fn from_session(error: SessionError) -> Self {
        match error {
            SessionError::Timeout => Self::TimedOut,
            SessionError::Io(_) => Self::Disconnected,
            // A setting is only written for a model whose state reported the
            // settings block, so the session always has a frame to write; the
            // write-only variants keep the mapping total.
            SessionError::Protocol(_)
            | SessionError::UnsupportedLayout
            | SessionError::UnsupportedSoundMode
            | SessionError::UnsupportedEqualizer
            | SessionError::UnsupportedSetting => Self::Unreadable,
        }
    }

    /// The parenthesised reason the banner appends to the setting's name.
    pub fn reason(self) -> &'static str {
        match self {
            Self::TimedOut => "no answer",
            Self::Disconnected => "connection lost",
            Self::Unreadable => "unreadable answer",
        }
    }

    /// The `AppState.error-label` of a failed write, naming the setting the row
    /// names; the vendored banner wraps it as "Could not change “{}”.".
    pub fn label(self, setting: SettingCommand) -> String {
        format!("{} ({})", setting_label(setting), self.reason())
    }
}

/// The row name a failed write's banner shows, matching the setting it wrote.
pub fn setting_label(setting: SettingCommand) -> &'static str {
    match setting {
        SettingCommand::ButtonDoublePress(_) => "Double press",
        SettingCommand::AmbientSoundModeCycle(_) => "Mode cycle",
        SettingCommand::LowBatteryPrompt(_) => "Low battery prompt",
        SettingCommand::DolbyAudio(_) => "Dolby audio",
        SettingCommand::Ldac(_) => "LDAC",
        SettingCommand::DualConnections(_) => "Dual connections",
        SettingCommand::AutoPowerOff(_) => "Auto power off",
        SettingCommand::LimitHighVolume { .. } => "Limit high volume",
        SettingCommand::LimitHighVolumeRefreshRate(_) => "Refresh rate",
        SettingCommand::SideTone(_) => "Side tone",
        SettingCommand::VoicePrompt(_) => "Voice prompt",
    }
}

/// Writes one settings frame and re-reads the state, returning the whole
/// re-polled state so the caller can update the panels and About.
///
/// Blocking: only the device worker calls this, never the UI thread.
pub fn change_setting<T: Transport>(
    session: &mut DeviceSession<T>,
    setting: SettingCommand,
) -> Result<Box<DeviceState>, SettingFailure> {
    session
        .set_setting(setting)
        .map_err(SettingFailure::from_session)?;
    let state = session.poll_state().map_err(SettingFailure::from_session)?;
    Ok(Box::new(state))
}

/// A failed connection, reduced to the cases the window has words for. It
/// deliberately holds no Bluetooth address, so nothing built from it can leak a
/// MAC into the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectFailure {
    /// The device is paired, but Windows rejected the link key.
    NotPaired,
    /// No paired device has the address, or it has no cached Soundcore service.
    NotFound,
    /// The radio or the device did not answer in time.
    TimedOut,
    /// Windows reports no usable Bluetooth radio.
    BluetoothOff,
    /// The RFCOMM connection could not be opened or was dropped.
    ConnectionFailed,
    /// The device is paired but advertises neither its vendor service nor SPP.
    NoService,
    /// The catalog lists the model, but the core has no state parser for it yet.
    NoParser,
    /// The device answered, but its battery layout is not implemented yet.
    UnsupportedLayout,
    /// The device answered with bytes that are not a valid state frame.
    Protocol,
    /// Any other OS or transport failure, with its readable text.
    Transport(String),
}

impl ConnectFailure {
    /// Maps a transport failure to the words the window uses.
    pub fn from_bluetooth(error: BluetoothError) -> Self {
        match error {
            BluetoothError::NotPaired => Self::NotPaired,
            BluetoothError::NotFound => Self::NotFound,
            BluetoothError::Timeout => Self::TimedOut,
            BluetoothError::BluetoothOff => Self::BluetoothOff,
            BluetoothError::ConnectionFailed => Self::ConnectionFailed,
            BluetoothError::Other(detail) => Self::Transport(detail),
        }
    }

    /// Maps a session failure to the words the window uses.
    pub fn from_session(error: SessionError) -> Self {
        match error {
            SessionError::Timeout => Self::TimedOut,
            SessionError::Io(error) => Self::Transport(error.to_string()),
            // A connection attempt only polls `01 01`; `UnsupportedSoundMode`
            // only comes from a mode write, `UnsupportedEqualizer` only from an
            // equalizer write and `UnsupportedSetting` only from a setting
            // write, so they cannot reach this mapping.
            SessionError::Protocol(_)
            | SessionError::UnsupportedSoundMode
            | SessionError::UnsupportedEqualizer
            | SessionError::UnsupportedSetting => Self::Protocol,
            SessionError::UnsupportedLayout => Self::UnsupportedLayout,
        }
    }

    /// The connection state the window shows for this failure.
    pub fn connection(&self) -> Connection {
        match self {
            Self::BluetoothOff => Connection::BluetoothOff,
            _ => Connection::Failed,
        }
    }

    /// The failure kind the window words itself: the two cases it names, and
    /// `Other` for everything it must word from [`Self::text`].
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::NotFound => FailureKind::NotFound,
            Self::TimedOut => FailureKind::TimedOut,
            // The window already explains the radio state on its own.
            Self::BluetoothOff => FailureKind::None,
            _ => FailureKind::Other,
        }
    }

    /// The readable explanation; never contains a Bluetooth address.
    pub fn text(&self) -> String {
        match self {
            Self::NotPaired => {
                "The device is not paired with Windows. Pair it in Windows settings, then try again.".into()
            }
            Self::NotFound => {
                "The device was not found. Make sure it is turned on and nearby.".into()
            }
            Self::TimedOut => {
                "The device did not answer in time. Make sure it is turned on and nearby, then try again.".into()
            }
            Self::BluetoothOff => {
                "Bluetooth is off. Turn it on in Windows settings, then try again.".into()
            }
            Self::ConnectionFailed => {
                "Could not open a Bluetooth connection to the device. Make sure it is nearby and not connected to another device.".into()
            }
            Self::NoService => {
                "This paired device does not offer the Soundcore Bluetooth service. Pair it again in Windows settings and try again.".into()
            }
            Self::NoParser => {
                "The app cannot read this model's state yet, so its battery levels are unavailable.".into()
            }
            Self::UnsupportedLayout => {
                "This model reports a battery layout the app cannot read yet, so battery levels are unavailable.".into()
            }
            Self::Protocol => {
                "The device answered with a Bluetooth message the app could not read.".into()
            }
            Self::Transport(detail) => detail.clone(),
        }
    }
}

/// A paired device and the SDP services Windows cached for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedServices {
    /// The numeric Bluetooth address, as Windows reports it.
    pub address: u64,
    /// The device name Windows shows.
    pub name: String,
    /// The service class UUIDs from the device's cached SDP record.
    pub services: Vec<GUID>,
}

/// The high 112 bits every Soundcore vendor RFCOMM service shares; the low 16
/// bits are the model's product code.
const VENDOR_SERVICE_MASK: u128 = 0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_0000;

/// Whether a paired device is likely the chosen model: it advertises the
/// model's own vendor RFCOMM service, or, when it has no Soundcore vendor
/// service at all, its Windows name matches the model name or code.
pub fn is_suggested(
    services: &[GUID],
    device_name: &str,
    model_code: &str,
    model_name: &str,
) -> bool {
    if vendor_service(model_code).is_some_and(|vendor| services.contains(&vendor)) {
        return true;
    }
    if advertises_another_model(services, model_code) {
        return false;
    }
    name_matches_model(device_name, model_code, model_name)
}

/// Whether the device advertises a Soundcore vendor service of another model;
/// such a device is definitely not the chosen model, whatever its name says.
fn advertises_another_model(services: &[GUID], model_code: &str) -> bool {
    let chosen = vendor_service(model_code);
    services.iter().any(|service| {
        service.to_u128() & VENDOR_SERVICE_MASK == VENDOR_SERVICE_PREFIX && Some(*service) != chosen
    })
}

/// Whether the Windows device name contains the model code or one of the slash
/// separated product names of the catalog ("P20i / P25i / R50i").
fn name_matches_model(device_name: &str, model_code: &str, model_name: &str) -> bool {
    let device = device_name.to_lowercase();
    if !model_code.is_empty() && device.contains(&model_code.to_lowercase()) {
        return true;
    }
    model_name
        .split('/')
        .map(str::trim)
        .filter(|alternative| alternative.chars().count() >= 3)
        .any(|alternative| device.contains(&alternative.to_lowercase()))
}

/// The Add Device rows for a chosen model (`None` before a model is chosen):
/// only devices that offer the model's vendor service or the standard SPP
/// service, with the likely model marked suggested.
pub fn candidates(paired: &[PairedServices], model: Option<(&str, &str)>) -> Vec<Candidate> {
    let (model_code, model_name) = model.unwrap_or(("", ""));
    paired
        .iter()
        .filter(|device| pick_service(&device.services, model_code).is_some())
        .map(|device| Candidate {
            address: device.address,
            name: device.name.clone(),
            suggested: !model_code.is_empty()
                && is_suggested(&device.services, &device.name, model_code, model_name),
        })
        .collect()
}

/// The saved-card list with `card` as its only current card: a card of the same
/// MAC is replaced, a new one is appended, and all others stop being current.
pub fn with_current_card(cards: &[DeviceCard], card: DeviceCard) -> Vec<DeviceCard> {
    let mut updated: Vec<DeviceCard> = cards
        .iter()
        .map(|existing| DeviceCard {
            current: false,
            ..existing.clone()
        })
        .collect();
    match updated.iter_mut().find(|existing| existing.mac == card.mac) {
        Some(slot) => *slot = card,
        None => updated.push(card),
    }
    updated
}

/// The Add Device confirmation step of a catalog code, or `None` when the
/// catalog lists no model of that exact code.
pub fn selected_model(code: &str) -> Option<SelectedModel> {
    let selection = catalog_adapter::selection(code)?;
    Some(SelectedModel {
        code: selection.code,
        name: selection.name,
        art: selection.art,
        is_speaker: selection.is_speaker,
        protocol: soundcore_models::model_by_code(selection.code),
    })
}

/// How often the UI thread drains the worker channel.
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Everything a worker needs to connect: the address, the catalog code the
/// vendor service derives from, and the state parser. It holds no Slint
/// handles, so it can cross the thread boundary.
#[derive(Debug, Clone)]
struct ConnectRequest {
    address: u64,
    model_code: String,
    protocol: Option<DeviceModel>,
}

/// What the Bluetooth worker found.
#[derive(Debug)]
enum ConnectOutcome {
    /// The device answered `01 01` with a state this model parses. The model is
    /// carried along so the page can show its preset catalog; the state is
    /// boxed because it is much larger than a failure.
    Connected(DeviceModel, Box<DeviceState>),
    /// The attempt ended in a failure the window has words for.
    Failed(ConnectFailure),
}

/// A write the connected device worker has to perform.
#[derive(Debug, Clone, Copy)]
enum DeviceCommand {
    /// Write a sound mode and re-read the state; `request` identifies the click
    /// so the UI can drop a result a newer click has superseded.
    ChangeSoundMode { request: u64, mode: SoundMode },
    /// Write an equalizer preset and re-read the state; `request` identifies the
    /// click the same way `ChangeSoundMode` does.
    ChangeEqualizerPreset { request: u64, preset_id: u16 },
    /// Write a custom eight-band equalizer curve and re-read the state;
    /// `request` identifies the drag release or typed value the same way
    /// `ChangeSoundMode` does.
    WriteCustomCurve {
        request: u64,
        bands: [i16; VISIBLE_EQ_BANDS],
    },
    /// Write one A3062 remaining setting and re-read the state; `request`
    /// identifies the toggle, choice or slider release the same way
    /// `ChangeSoundMode` does.
    SetSetting {
        request: u64,
        setting: SettingCommand,
    },
}

/// A worker-to-UI message; the sequence number tells the UI whether the result
/// still belongs to the attempt the user is waiting for.
#[derive(Debug)]
enum Event {
    Paired {
        request: u64,
        result: Result<Vec<Candidate>, BluetoothError>,
    },
    Connected {
        attempt: u64,
        outcome: ConnectOutcome,
    },
    /// A device worker finished a sound-mode change and re-read the state.
    ModeChanged {
        attempt: u64,
        request: u64,
        outcome: Result<SoundMode, ModeFailure>,
    },
    /// A device worker finished an equalizer preset change and re-read the
    /// state.
    PresetChanged {
        attempt: u64,
        request: u64,
        outcome: Result<EqualizerState, EqFailure>,
    },
    /// A device worker finished a custom-curve write and re-read the state.
    CurveChanged {
        attempt: u64,
        request: u64,
        outcome: Result<EqualizerState, EqFailure>,
    },
    /// A device worker finished a remaining-settings write and re-read the
    /// state; the whole state comes along so the panels and About can be
    /// refilled from the device's answer.
    SettingChanged {
        attempt: u64,
        request: u64,
        setting: SettingCommand,
        outcome: Result<Box<DeviceState>, SettingFailure>,
    },
    /// The import-file worker closed its file dialog: the file it read, or `None`
    /// when the dialog was cancelled. A read failure carries the note to show.
    ImportFilePicked {
        result: Option<Result<String, ImportFileError>>,
    },
}

/// State shared between the UI callbacks and the polling timer. Every field is
/// touched on the Slint thread only; workers only hold clones of the channels.
struct Inner {
    ui: slint::Weak<AppWindow>,
    sender: Sender<Event>,
    receiver: RefCell<Receiver<Event>>,
    /// The newest paired-device request; results of older requests are stale.
    enumeration: Cell<u64>,
    /// The newest connect attempt; results of older attempts are stale.
    connect: Cell<u64>,
    /// Cancels the in-flight connect attempt without waiting for it.
    cancel: RefCell<Option<Arc<AtomicBool>>>,
    /// The command channel of the connected device worker; dropping it stops the
    /// worker and closes the device connection.
    commands: RefCell<Option<Sender<DeviceCommand>>>,
    /// The newest sound-mode change; results of older changes are stale.
    mode_change: Cell<u64>,
    /// The newest equalizer preset change; results of older changes are stale.
    preset_change: Cell<u64>,
    /// The newest custom equalizer curve change; results of older changes are
    /// stale.
    curve_change: Cell<u64>,
    /// The newest remaining-settings change; results of older changes are
    /// stale.
    setting_change: Cell<u64>,
    /// The last state of the connected device. It is kept on the UI thread so
    /// a section-page change refills the panels, and a settings write's
    /// re-poll replaces it, without another poll.
    device: RefCell<Option<Box<DeviceState>>>,
    /// The saved equalizer profiles, loaded from the document at startup and
    /// edited by the Equalizer page's save/apply/delete actions. UI thread only.
    profiles: RefCell<ProfileSet>,
    /// The document the profiles are persisted to; `None` when Windows gives no
    /// app-data directory, in which case the profiles stay in memory only.
    profiles_path: Option<PathBuf>,
    /// The text of the import the dialog is previewing, kept so `eq-import` can
    /// re-validate it before anything is written; `None` when no preview is
    /// open. UI thread only.
    import_text: RefCell<Option<String>>,
    /// The clipboard and file-dialog seam the import actions use. The real
    /// implementation is [`NativeImportIo`]; tests substitute a fake.
    import_io: Arc<dyn ImportIo + Send + Sync>,
    /// Whether the file-dialog worker is running, so a second click does not
    /// open a second dialog. UI thread only.
    picking_import_file: Cell<bool>,
}

impl Inner {
    /// Enumerates paired devices on a worker thread for a model, or for every
    /// SPP device when no model is chosen yet.
    fn request_paired(&self, model: Option<(String, String)>) {
        let request = self.enumeration.get() + 1;
        self.enumeration.set(request);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("paired-enumeration".into())
            .spawn(move || {
                let result = enumerate_candidates(
                    model
                        .as_ref()
                        .map(|(code, name)| (code.as_str(), name.as_str())),
                );
                let _ = sender.send(Event::Paired { request, result });
            });
        if spawned.is_err() {
            let _ = self.sender.send(Event::Paired {
                request,
                result: Err(BluetoothError::Other(
                    "could not start the paired-device worker".into(),
                )),
            });
        }
    }

    /// Connects on a worker thread and keeps that worker for mode and equalizer
    /// writes.
    fn request_connect(&self, request: ConnectRequest) {
        let attempt = self.connect.get() + 1;
        self.connect.set(attempt);
        self.mode_change.set(0);
        self.preset_change.set(0);
        self.curve_change.set(0);
        self.setting_change.set(0);
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.borrow_mut() = Some(Arc::clone(&cancel));
        let (commands, command_receiver) = mpsc::channel();
        // Replacing the channel stops a worker of an older attempt.
        *self.commands.borrow_mut() = Some(commands);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("bluetooth-device".into())
            .spawn(move || run_device_worker(request, attempt, cancel, sender, command_receiver));
        if spawned.is_err() {
            self.stop_worker();
            let _ = self.sender.send(Event::Connected {
                attempt,
                outcome: ConnectOutcome::Failed(ConnectFailure::Transport(
                    "could not start the Bluetooth worker".into(),
                )),
            });
        }
    }

    /// Forgets the in-flight connect attempt; its eventual result is stale.
    fn cancel_connect(&self) {
        if let Some(cancel) = self.cancel.borrow_mut().take() {
            cancel.store(true, Ordering::Relaxed);
        }
        // A cancelled worker may still send its result, so move past its number
        // instead of reusing it for the next attempt.
        self.connect.set(self.connect.get() + 1);
        self.stop_worker();
    }

    /// Drops the device worker's command channel, which stops the worker and
    /// closes the connection once any change in flight has finished.
    fn stop_worker(&self) {
        self.commands.borrow_mut().take();
        self.mode_change.set(0);
        self.preset_change.set(0);
        self.curve_change.set(0);
        self.setting_change.set(0);
    }

    /// Queues one write on the device worker, carrying the next value of
    /// `request` so a superseded result can be dropped. False when the worker is
    /// gone (a failed or stopped connection), so the caller can say why nothing
    /// was written.
    fn send_queued(&self, request: &Cell<u64>, command: impl FnOnce(u64) -> DeviceCommand) -> bool {
        let next = request.get() + 1;
        request.set(next);
        self.commands
            .borrow()
            .as_ref()
            .is_some_and(|sender| sender.send(command(next)).is_ok())
    }
}

/// Owns the timer that drains worker events. Keep it alive for as long as the
/// window runs; the window's actions are already wired when this returns.
pub struct Controller {
    /// The polling timer; dropping it stops the polling.
    _timer: Timer,
}

impl Controller {
    /// Fills the shell's one-time state, answers the window's actions, and
    /// starts the worker-event timer. Every callback runs on the Slint thread.
    ///
    /// The saved profiles are loaded here, before the first window is filled, so
    /// the panel already has them when a device connects. A missing or malformed
    /// document loads as no profiles and is only logged to stderr.
    pub fn new(ui: &AppWindow) -> Self {
        let profiles_path = default_profiles_path();
        let profiles = match profiles_path.as_deref() {
            Some(path) => load_profiles(path),
            None => {
                log_no_app_data_directory();
                ProfileSet::new()
            }
        };
        let (sender, receiver) = mpsc::channel();
        let inner = Rc::new(Inner {
            ui: ui.as_weak(),
            sender,
            receiver: RefCell::new(receiver),
            enumeration: Cell::new(0),
            connect: Cell::new(0),
            cancel: RefCell::new(None),
            commands: RefCell::new(None),
            mode_change: Cell::new(0),
            preset_change: Cell::new(0),
            curve_change: Cell::new(0),
            setting_change: Cell::new(0),
            device: RefCell::new(None),
            profiles: RefCell::new(profiles),
            profiles_path,
            import_text: RefCell::new(None),
            import_io: Arc::new(NativeImportIo),
            picking_import_file: Cell::new(false),
        });
        initial_state(ui);
        show_saved_profiles(&ui.global::<AppState>(), &inner.profiles.borrow());
        wire_actions(ui, &inner);
        let timer = Timer::default();
        let polling = Rc::clone(&inner);
        timer.start(TimerMode::Repeated, EVENT_POLL_INTERVAL, move || {
            drain_events(&polling);
        });
        Self { _timer: timer }
    }
}

/// The shell's never-changing state: version, Windows wording, and the pages
/// this MVP deliberately leaves empty.
fn initial_state(ui: &AppWindow) {
    let state = ui.global::<AppState>();
    state.set_app_version(crate::app_version().into());
    // The vendored shell keeps its Windows wording; the deep links become
    // available when a device with the settings block connects.
    state.set_linux(false);
    // Only Add Device, the device cards, the Overview battery, the Sound mode
    // and the Equalizer page are wired before a device connects; every section
    // page's content stays empty until an A3062 reports its settings.
    show_sound_mode(&state, None, false, false);
    show_equalizer(&state, &EqualizerDisplay::default());
    clear_section_pages(&state);
    state.set_overview_rows(ModelRc::default());
    state.set_quick_toggles(ModelRc::default());
    state.set_all_toggles(ModelRc::default());
    state.set_page(Page::Devices);
    state.set_connection(Connection::NoDevice);
    reset_models(&state);
}

/// Answers every action this MVP needs; the others stay unwired and disabled by
/// their now-empty state.
fn wire_actions(ui: &AppWindow, inner: &Rc<Inner>) {
    let actions = ui.global::<Actions>();

    actions.on_open_add_device({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                open_add_device(&window, &inner);
            }
        }
    });

    actions.on_filter_models({
        let inner = Rc::clone(inner);
        move |query, kind| {
            if let Some(window) = inner.ui.upgrade() {
                show_models(
                    &window.global::<AppState>(),
                    query.as_str(),
                    Chip::from_index(kind),
                );
            }
        }
    });

    actions.on_choose_model({
        let inner = Rc::clone(inner);
        move |code| {
            if let Some(window) = inner.ui.upgrade() {
                choose_model(&window, &inner, code.as_str());
            }
        }
    });

    actions.on_refresh_paired({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                refresh_paired(&window, &inner);
            }
        }
    });

    actions.on_pair({
        let inner = Rc::clone(inner);
        move |mac, name| {
            if let Some(window) = inner.ui.upgrade() {
                pair(&window, &inner, mac.as_str(), name.as_str());
            }
        }
    });

    actions.on_connect({
        let inner = Rc::clone(inner);
        move |mac| {
            if let Some(window) = inner.ui.upgrade() {
                connect_saved(&window, &inner, mac.as_str());
            }
        }
    });

    actions.on_retry({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                retry(&window, &inner);
            }
        }
    });

    actions.on_cancel_connect({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                cancel_connect(&window, &inner);
            }
        }
    });

    actions.on_disconnect({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                disconnect(&window, &inner);
            }
        }
    });

    actions.on_set_option({
        let inner = Rc::clone(inner);
        move |setting_id, index| {
            if let Some(window) = inner.ui.upgrade() {
                set_option(&window, &inner, setting_id.as_str(), index);
            }
        }
    });

    actions.on_set_toggle({
        let inner = Rc::clone(inner);
        move |setting_id, on| {
            if let Some(window) = inner.ui.upgrade() {
                set_toggle(&window, &inner, setting_id.as_str(), on);
            }
        }
    });

    actions.on_set_number({
        let inner = Rc::clone(inner);
        move |setting_id, value| {
            if let Some(window) = inner.ui.upgrade() {
                set_number(&window, &inner, setting_id.as_str(), value);
            }
        }
    });

    actions.on_copy(move |text| copy_text(text.as_str()));

    actions.on_open_system_settings(open_system_settings);

    actions.on_eq_preset({
        let inner = Rc::clone(inner);
        move |value| {
            if let Some(window) = inner.ui.upgrade() {
                select_equalizer_preset(&window, &inner, value.as_str());
            }
        }
    });

    actions.on_eq_preview({
        let inner = Rc::clone(inner);
        move |index, value| answer_band_edit(&inner, BandAction::Preview { index, value })
    });

    actions.on_eq_commit({
        let inner = Rc::clone(inner);
        move |index, value| answer_band_edit(&inner, BandAction::Commit { index, value })
    });

    actions.on_eq_type({
        let inner = Rc::clone(inner);
        move |index, text| {
            let Some(value) = parse_band_text(text.as_str()) else {
                return;
            };
            answer_band_edit(&inner, BandAction::Commit { index, value });
        }
    });

    actions.on_eq_reset({
        let inner = Rc::clone(inner);
        move || answer_band_edit(&inner, BandAction::Reset)
    });

    actions.on_eq_save({
        let inner = Rc::clone(inner);
        move |name, replace| {
            if let Some(window) = inner.ui.upgrade() {
                save_equalizer_profile(&window, &inner, name.as_str(), replace);
            }
        }
    });

    actions.on_eq_apply({
        let inner = Rc::clone(inner);
        move |name| {
            if let Some(window) = inner.ui.upgrade() {
                apply_equalizer_profile(&window, &inner, name.as_str());
            }
        }
    });

    actions.on_eq_delete({
        let inner = Rc::clone(inner);
        move |name| {
            if let Some(window) = inner.ui.upgrade() {
                delete_equalizer_profile(&window, &inner, name.as_str());
            }
        }
    });

    actions.on_eq_import_file({
        let inner = Rc::clone(inner);
        move || begin_import_file(&inner)
    });

    actions.on_eq_import_paste({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                eq_import_paste(&window, &inner);
            }
        }
    });

    actions.on_eq_import({
        let inner = Rc::clone(inner);
        move || answer_import(&inner)
    });

    actions.on_navigate({
        let inner = Rc::clone(inner);
        move |page| {
            if let Some(window) = inner.ui.upgrade() {
                let state = window.global::<AppState>();
                state.set_page(page);
                // The panels belong to the page, so a sidebar click refits them
                // from the last connected state.
                show_section_pages(&state, &inner);
            }
        }
    });

    actions.on_dismiss_error({
        let inner = Rc::clone(inner);
        move || {
            if let Some(window) = inner.ui.upgrade() {
                window.global::<AppState>().set_error_label("".into());
            }
        }
    });
}

/// Whether a finished sound-mode change still updates the page.
///
/// A success is page data: it only applies when it belongs to the current
/// connection attempt and to the newest click. A failure is not stale page
/// data — the worker stops after it, so no newer result can replace it, and a
/// click queued behind the failed one will never run. Surfacing the failure is
/// then the only report the user gets, and the banner's Retry stays available.
fn mode_result_applies(
    outcome: &Result<SoundMode, ModeFailure>,
    attempt: u64,
    request: u64,
    current_attempt: u64,
    newest_request: u64,
) -> bool {
    attempt == current_attempt && (outcome.is_err() || request == newest_request)
}

/// Whether a finished equalizer change (a preset or a custom curve) still
/// updates the page, for the same reasons as [`mode_result_applies`].
fn equalizer_change_applies<T>(
    outcome: &Result<T, EqFailure>,
    attempt: u64,
    request: u64,
    current_attempt: u64,
    newest_request: u64,
) -> bool {
    attempt == current_attempt && (outcome.is_err() || request == newest_request)
}

/// Whether a finished remaining-settings change still updates the page, for the
/// same reasons as [`mode_result_applies`].
fn setting_result_applies(
    outcome: &Result<Box<DeviceState>, SettingFailure>,
    attempt: u64,
    request: u64,
    current_attempt: u64,
    newest_request: u64,
) -> bool {
    attempt == current_attempt && (outcome.is_err() || request == newest_request)
}

/// Drains everything the workers sent since the last tick and applies it on the
/// Slint thread. Stale results (from a superseded enumeration or a cancelled
/// connect attempt) are dropped.
fn drain_events(inner: &Rc<Inner>) {
    loop {
        let event = inner.receiver.borrow_mut().try_recv();
        match event {
            Ok(Event::Paired { request, result }) => {
                if request == inner.enumeration.get()
                    && let Some(window) = inner.ui.upgrade()
                {
                    apply_paired(&window, result);
                }
            }
            Ok(Event::Connected { attempt, outcome }) => {
                if attempt == inner.connect.get()
                    && let Some(window) = inner.ui.upgrade()
                {
                    // Only a connected model with a measured mode, a supported
                    // equalizer or the settings block keeps its worker (and
                    // socket) for writes; everything else ends as in the MVP.
                    let keeps_worker = matches!(
                        &outcome,
                        ConnectOutcome::Connected(_, state)
                            if state.sound_mode.is_some()
                                || state.equalizer.is_some()
                                || state.settings.is_some()
                    );
                    if !keeps_worker {
                        inner.stop_worker();
                    }
                    apply_outcome(&window, inner, outcome);
                    // A connected A3062 shows the Saved profiles panel, so the
                    // loaded rows and their Applied tags are filled here.
                    show_saved_profiles(&window.global::<AppState>(), &inner.profiles.borrow());
                }
            }
            Ok(Event::ModeChanged {
                attempt,
                request,
                outcome,
            }) => {
                let applies = mode_result_applies(
                    &outcome,
                    attempt,
                    request,
                    inner.connect.get(),
                    inner.mode_change.get(),
                );
                if applies && let Some(window) = inner.ui.upgrade() {
                    apply_mode_outcome(&window, inner, outcome);
                }
            }
            Ok(Event::PresetChanged {
                attempt,
                request,
                outcome,
            }) => {
                let applies = equalizer_change_applies(
                    &outcome,
                    attempt,
                    request,
                    inner.connect.get(),
                    inner.preset_change.get(),
                );
                if applies && let Some(window) = inner.ui.upgrade() {
                    apply_preset_outcome(&window, inner, outcome);
                    show_saved_profiles(&window.global::<AppState>(), &inner.profiles.borrow());
                }
            }
            Ok(Event::CurveChanged {
                attempt,
                request,
                outcome,
            }) => {
                let applies = equalizer_change_applies(
                    &outcome,
                    attempt,
                    request,
                    inner.connect.get(),
                    inner.curve_change.get(),
                );
                if applies && let Some(window) = inner.ui.upgrade() {
                    let state = window.global::<AppState>();
                    apply_curve_outcome(&state, &|| inner.stop_worker(), outcome);
                    // A re-polled curve changes which saved profile is applied.
                    show_saved_profiles(&state, &inner.profiles.borrow());
                }
            }
            Ok(Event::SettingChanged {
                attempt,
                request,
                setting,
                outcome,
            }) => {
                let applies = setting_result_applies(
                    &outcome,
                    attempt,
                    request,
                    inner.connect.get(),
                    inner.setting_change.get(),
                );
                if applies && let Some(window) = inner.ui.upgrade() {
                    apply_setting_outcome(&window, inner, setting, outcome);
                }
            }
            Ok(Event::ImportFilePicked { result }) => {
                inner.picking_import_file.set(false);
                if let (Some(result), Some(window)) = (result, inner.ui.upgrade()) {
                    let state = window.global::<AppState>();
                    let mut import_text = inner.import_text.borrow_mut();
                    preview_import_file(&state, &inner.profiles.borrow(), &mut import_text, result);
                }
            }
            // Both an empty queue and a closed channel mean nothing more will
            // arrive until a new worker is started.
            Err(_) => break,
        }
    }
}

/// Lists paired devices and their cached SDP services on the worker thread.
fn enumerate_candidates(model: Option<(&str, &str)>) -> Result<Vec<Candidate>, BluetoothError> {
    let devices = list_paired_devices()?;
    let mut with_services = Vec::with_capacity(devices.len());
    for device in devices {
        // A device whose cached services cannot be read cannot be a Soundcore
        // SPP candidate; skip it rather than failing the whole list.
        if let Ok(services) = installed_services(device.address) {
            with_services.push(PairedServices {
                address: device.address,
                name: device.name,
                services,
            });
        }
    }
    Ok(candidates(&with_services, model))
}

/// The beginning of a device connection: an open session, or why it stopped.
enum OpenOutcome {
    Opened(DeviceSession<WindowsRfcommTransport>),
    Cancelled,
    Failed(ConnectFailure),
}

/// Opens the RFCOMM connection and picks the model's vendor service. Blocking.
fn open_session(request: &ConnectRequest, cancel: &AtomicBool) -> OpenOutcome {
    let cancelled = || cancel.load(Ordering::Relaxed);
    let Some(model) = request.protocol else {
        return OpenOutcome::Failed(ConnectFailure::NoParser);
    };
    if cancelled() {
        return OpenOutcome::Cancelled;
    }
    let services = match installed_services(request.address) {
        Ok(services) => services,
        Err(error) => return OpenOutcome::Failed(ConnectFailure::from_bluetooth(error)),
    };
    if cancelled() {
        return OpenOutcome::Cancelled;
    }
    let Some(service) = pick_service(&services, &request.model_code) else {
        return OpenOutcome::Failed(ConnectFailure::NoService);
    };
    let transport = match WindowsRfcommTransport::connect(request.address, service) {
        Ok(transport) => transport,
        Err(error) => return OpenOutcome::Failed(ConnectFailure::from_bluetooth(error)),
    };
    if cancelled() {
        // Dropping the transport closes the socket.
        return OpenOutcome::Cancelled;
    }
    OpenOutcome::Opened(DeviceSession::new(model, transport))
}

/// The connected device worker: connects, then hands the open session to
/// [`serve_device`] for the initial state report and the mode and equalizer
/// writes.
///
/// Every change runs on this worker, so the UI thread never blocks on
/// Bluetooth, and a re-poll after each write keeps the page in step with the
/// device.
fn run_device_worker(
    request: ConnectRequest,
    attempt: u64,
    cancel: Arc<AtomicBool>,
    events: Sender<Event>,
    commands: Receiver<DeviceCommand>,
) {
    let session = match open_session(&request, &cancel) {
        OpenOutcome::Opened(session) => session,
        OpenOutcome::Cancelled => return,
        OpenOutcome::Failed(failure) => {
            let _ = events.send(Event::Connected {
                attempt,
                outcome: ConnectOutcome::Failed(failure),
            });
            return;
        }
    };
    serve_device(session, attempt, events, commands);
}

/// Reports the initial state, then serves sound-mode and equalizer changes on
/// the same session until the UI drops the command channel or a change fails.
///
/// The last read equalizer state is kept for the equalizer writes: the `03 87`
/// body preserves its HearID block, and it is replaced by each re-polled state
/// so a preset can follow a custom curve and the other way round.
///
/// Generic over the transport so the tests can drive it with a scripted one.
fn serve_device<T: Transport>(
    mut session: DeviceSession<T>,
    attempt: u64,
    events: Sender<Event>,
    commands: Receiver<DeviceCommand>,
) {
    // `poll_state` here is the MVP's one initial `01 01` request.
    let state = match session.poll_state() {
        Ok(state) => state,
        Err(error) => {
            let _ = events.send(Event::Connected {
                attempt,
                outcome: ConnectOutcome::Failed(ConnectFailure::from_session(error)),
            });
            return;
        }
    };
    let model = session.model();
    let has_sound_mode = state.sound_mode.is_some();
    let mut equalizer = state.equalizer;
    let has_equalizer = equalizer.is_some();
    let has_settings = state.settings.is_some();
    let _ = events.send(Event::Connected {
        attempt,
        outcome: ConnectOutcome::Connected(model, Box::new(state)),
    });
    // A model with neither a measured mode, a supported equalizer nor the
    // settings block has nothing this worker can write; ending it closes the
    // socket as the MVP did. Any other model keeps its session open for the
    // `06 81`, `03 87` and settings writes.
    if !has_sound_mode && !has_equalizer && !has_settings {
        return;
    }
    while let Ok(command) = commands.recv() {
        match command {
            DeviceCommand::ChangeSoundMode { request, mode } => {
                let outcome = change_sound_mode(&mut session, mode);
                let failed = outcome.is_err();
                let _ = events.send(Event::ModeChanged {
                    attempt,
                    request,
                    outcome,
                });
                if failed {
                    // The session is out of step after a failed write or read;
                    // the banner's Retry opens a fresh connection instead of
                    // writing on a broken one. Any click already queued behind
                    // this one is dropped with the channel, and the surfaced
                    // failure explains why.
                    return;
                }
            }
            DeviceCommand::ChangeEqualizerPreset { request, preset_id } => {
                // The UI only offers catalog ids; an id that is not one of the
                // model's presets is treated as an unreadable answer rather than
                // writing an invented body.
                let preset = model
                    .equalizer_presets()
                    .iter()
                    .find(|preset| preset.id == preset_id);
                let outcome = match (preset, equalizer.as_ref()) {
                    (Some(preset), Some(current)) => {
                        let reported = change_equalizer_preset(&mut session, current, preset);
                        if let Ok(reported) = &reported {
                            equalizer = Some(*reported);
                        }
                        reported
                    }
                    _ => Err(EqFailure::Unreadable),
                };
                let failed = outcome.is_err();
                let _ = events.send(Event::PresetChanged {
                    attempt,
                    request,
                    outcome,
                });
                if failed {
                    // Same rule as a failed mode change: stop writing on a
                    // session that is out of step and let Retry reconnect.
                    return;
                }
            }
            DeviceCommand::WriteCustomCurve { request, bands } => {
                // A curve written from the state the worker last read keeps
                // that state's HearID block; the re-polled state becomes the
                // base of the next preset or curve write.
                let outcome = match equalizer.as_ref() {
                    Some(current) => {
                        let reported = change_custom_equalizer(&mut session, current, bands);
                        if let Ok(reported) = &reported {
                            equalizer = Some(*reported);
                        }
                        reported
                    }
                    None => Err(EqFailure::Unreadable),
                };
                let failed = outcome.is_err();
                let _ = events.send(Event::CurveChanged {
                    attempt,
                    request,
                    outcome,
                });
                if failed {
                    // Same rule as a failed preset change: stop writing on a
                    // session that is out of step and let Retry reconnect.
                    return;
                }
            }
            DeviceCommand::SetSetting { request, setting } => {
                let outcome = change_setting(&mut session, setting);
                // The re-polled state is the base of the next equalizer write,
                // exactly as after a mode change.
                if let Ok(reported) = &outcome {
                    equalizer = reported.equalizer;
                }
                let failed = outcome.is_err();
                let _ = events.send(Event::SettingChanged {
                    attempt,
                    request,
                    setting,
                    outcome,
                });
                if failed {
                    // Same rule as a failed mode change: stop writing on a
                    // session that is out of step and let Retry reconnect.
                    return;
                }
            }
        }
    }
}

/// Resets the dialog to its model step and starts the paired enumeration.
fn open_add_device(window: &AppWindow, inner: &Rc<Inner>) {
    let state = window.global::<AppState>();
    state.set_adding(true);
    state.set_add_step(0);
    state.set_add_loading(true);
    state.set_add_bluetooth_off(false);
    state.set_add_failure_kind(FailureKind::None);
    state.set_add_error("".into());
    state.set_paired_options(ModelRc::default());
    reset_models(&state);
    request_paired_for_selection(&state, inner);
}

/// Advances the dialog to the paired-device step of the chosen model and lists
/// its candidates.
fn choose_model(window: &AppWindow, inner: &Rc<Inner>, code: &str) {
    let Some(model) = selected_model(code) else {
        return;
    };
    let state = window.global::<AppState>();
    state.set_add_model_name(model.name.into());
    state.set_add_model_code(model.code.into());
    state.set_add_model_art(model.art);
    state.set_add_model_is_speaker(model.is_speaker);
    state.set_add_step(1);
    state.set_add_bluetooth_off(false);
    state.set_add_failure_kind(FailureKind::None);
    state.set_add_error("".into());
    state.set_paired_options(ModelRc::default());
    state.set_add_loading(true);
    inner.request_paired(Some((model.code.to_owned(), model.name.to_owned())));
}

/// Repeats the enumeration for the current selection.
fn refresh_paired(window: &AppWindow, inner: &Rc<Inner>) {
    let state = window.global::<AppState>();
    state.set_add_failure_kind(FailureKind::None);
    state.set_add_error("".into());
    state.set_add_bluetooth_off(false);
    request_paired_for_selection(&state, inner);
}

/// Reads the chosen model from the dialog state and starts an enumeration for
/// it; with no model chosen the worker lists every SPP device.
fn request_paired_for_selection(state: &AppState<'_>, inner: &Rc<Inner>) {
    let model = chosen_model(state).map(|model| (model.code.to_owned(), model.name.to_owned()));
    inner.request_paired(model);
}

/// The selected model, or `None` while the dialog has no model chosen.
fn chosen_model(state: &AppState<'_>) -> Option<SelectedModel> {
    selected_model(state.get_add_model_code().as_str())
}

/// Pairs the picked Windows device with the chosen catalog model and starts the
/// connection attempt.
fn pair(window: &AppWindow, inner: &Rc<Inner>, mac: &str, name: &str) {
    let state = window.global::<AppState>();
    let Some(model) = chosen_model(&state) else {
        state.set_add_failure_kind(FailureKind::Other);
        state.set_add_error("Choose a model before picking a paired device.".into());
        return;
    };
    let Some(address) = parse_mac(mac) else {
        state.set_add_failure_kind(FailureKind::Other);
        state.set_add_error(
            "This paired device could not be read. Refresh the list and try again.".into(),
        );
        return;
    };
    let card = DeviceCard {
        mac: mac.into(),
        name: name.into(),
        code: model.code.into(),
        art: model.art.clone(),
        current: true,
        demo: false,
    };
    show_current_card(&state, card);
    state.set_device_name(name.into());
    state.set_device_code(model.code.into());
    state.set_device_art(model.art);
    state.set_device_is_speaker(model.is_speaker);
    state.set_one_earbud(false);
    state.set_adding(false);
    state.set_add_step(0);
    state.set_page(Page::Overview);
    begin_connecting(
        &state,
        inner,
        ConnectRequest {
            address,
            model_code: model.code.to_owned(),
            protocol: model.protocol,
        },
    );
}

/// Reconnects a saved card from My Devices.
fn connect_saved(window: &AppWindow, inner: &Rc<Inner>, mac: &str) {
    let state = window.global::<AppState>();
    let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
    let Some(card) = cards.iter().find(|card| card.mac == mac).cloned() else {
        return;
    };
    begin_card_connect(window, inner, card);
}

/// Repeats the connect of the current card, or of the first saved card.
fn retry(window: &AppWindow, inner: &Rc<Inner>) {
    let state = window.global::<AppState>();
    let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
    let Some(card) = cards
        .iter()
        .find(|card| card.current)
        .or_else(|| cards.first())
        .cloned()
    else {
        state.set_connection(Connection::NoDevice);
        return;
    };
    begin_card_connect(window, inner, card);
}

/// Starts the connection of a saved card.
fn begin_card_connect(window: &AppWindow, inner: &Rc<Inner>, card: DeviceCard) {
    let state = window.global::<AppState>();
    show_current_card(
        &state,
        DeviceCard {
            current: true,
            ..card.clone()
        },
    );
    state.set_device_name(card.name.clone());
    state.set_device_code(card.code.clone());
    state.set_device_art(card.art.clone());
    state.set_device_is_speaker(
        selected_model(card.code.as_str()).is_some_and(|model| model.is_speaker),
    );
    state.set_one_earbud(false);
    let Some(address) = parse_mac(card.mac.as_str()) else {
        apply_failure(
            inner,
            &state,
            ConnectFailure::Transport(
                "The saved device's Bluetooth address cannot be read. Add the device again.".into(),
            ),
        );
        return;
    };
    begin_connecting(
        &state,
        inner,
        ConnectRequest {
            address,
            model_code: card.code.to_string(),
            protocol: soundcore_models::model_by_code(card.code.as_str()),
        },
    );
}

/// Puts the window into "connecting" and starts the worker; the timer applies
/// the result later.
fn begin_connecting(state: &AppState<'_>, inner: &Rc<Inner>, request: ConnectRequest) {
    state.set_connection(Connection::Connecting);
    state.set_connection_reason("".into());
    state.set_failure_kind(FailureKind::None);
    state.set_failure_text("".into());
    state.set_error_label("".into());
    state.set_batteries(ModelRc::default());
    inner.device.borrow_mut().take();
    show_sound_mode(state, None, false, false);
    show_equalizer(state, &EqualizerDisplay::default());
    clear_section_pages(state);
    inner.request_connect(request);
}

/// Stops a pending attempt; its worker result no longer matches the attempt the
/// window is waiting for.
fn cancel_connect(window: &AppWindow, inner: &Rc<Inner>) {
    inner.cancel_connect();
    let state = window.global::<AppState>();
    state.set_connection(if state.get_devices().row_count() > 0 {
        Connection::Off
    } else {
        Connection::NoDevice
    });
    state.set_batteries(ModelRc::default());
    state.set_connection_reason("".into());
    state.set_failure_kind(FailureKind::None);
    state.set_failure_text("".into());
    state.set_error_label("".into());
    inner.device.borrow_mut().take();
    show_sound_mode(&state, None, false, false);
    show_equalizer(&state, &EqualizerDisplay::default());
    clear_section_pages(&state);
}

/// Drops the current device and returns to My Devices.
fn disconnect(window: &AppWindow, inner: &Rc<Inner>) {
    inner.cancel_connect();
    let state = window.global::<AppState>();
    let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
    let has_devices = !cards.is_empty();
    let cleared: Vec<DeviceCard> = cards
        .iter()
        .map(|card| DeviceCard {
            current: false,
            ..card.clone()
        })
        .collect();
    state.set_devices(ModelRc::new(VecModel::from(cleared)));
    state.set_connection(if has_devices {
        Connection::Off
    } else {
        Connection::NoDevice
    });
    state.set_batteries(ModelRc::default());
    state.set_connection_reason("".into());
    state.set_failure_kind(FailureKind::None);
    state.set_failure_text("".into());
    state.set_error_label("".into());
    inner.device.borrow_mut().take();
    show_sound_mode(&state, None, false, false);
    show_equalizer(&state, &EqualizerDisplay::default());
    clear_section_pages(&state);
    state.set_page(Page::Devices);
}

/// Sets the saved cards to `card` as their only current one.
fn show_current_card(state: &AppState<'_>, card: DeviceCard) {
    let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
    state.set_devices(ModelRc::new(VecModel::from(with_current_card(
        &cards, card,
    ))));
}

/// Applies a finished enumeration to the dialog.
fn apply_paired(window: &AppWindow, result: Result<Vec<Candidate>, BluetoothError>) {
    let state = window.global::<AppState>();
    state.set_add_loading(false);
    match result {
        Ok(found) => {
            let options: Vec<PairedOption> = found.iter().map(paired_option).collect();
            state.set_paired_options(ModelRc::new(VecModel::from(options)));
            state.set_add_bluetooth_off(false);
            state.set_add_failure_kind(FailureKind::None);
            state.set_add_error("".into());
        }
        Err(BluetoothError::BluetoothOff) => {
            state.set_paired_options(ModelRc::default());
            state.set_add_bluetooth_off(true);
            state.set_add_failure_kind(FailureKind::None);
            state.set_add_error("".into());
        }
        Err(error) => {
            let failure = ConnectFailure::from_bluetooth(error);
            state.set_paired_options(ModelRc::default());
            state.set_add_failure_kind(failure.kind());
            state.set_add_error(failure.text().into());
        }
    }
}

/// Applies a finished connection attempt to the shell.
///
/// A connected state is also kept on the controller, because its settings
/// decide the section pages and their panels; a failure clears it.
fn apply_outcome(window: &AppWindow, inner: &Rc<Inner>, outcome: ConnectOutcome) {
    let state = window.global::<AppState>();
    match outcome {
        ConnectOutcome::Connected(model, device_state) => {
            let equalizer = EqualizerDisplay::new(Some(model), device_state.equalizer.as_ref());
            let has_settings = device_state.settings.is_some();
            state.set_batteries(ModelRc::new(VecModel::from(batteries(&device_state))));
            show_sound_mode(
                &state,
                device_state.sound_mode,
                equalizer.available,
                has_settings,
            );
            show_device_pages(&state, has_settings);
            show_equalizer(&state, &equalizer);
            state.set_connection(Connection::Connected);
            state.set_connection_reason("".into());
            state.set_failure_kind(FailureKind::None);
            state.set_failure_text("".into());
            state.set_error_label("".into());
            *inner.device.borrow_mut() = Some(device_state);
            show_section_pages(&state, inner);
        }
        ConnectOutcome::Failed(failure) => apply_failure(inner, &state, failure),
    }
}

/// Shows a failed connection: no battery tiles, the mapped state, and the
/// readable explanation. The section pages of a previous connection are gone.
fn apply_failure(inner: &Rc<Inner>, state: &AppState<'_>, failure: ConnectFailure) {
    state.set_batteries(ModelRc::default());
    inner.device.borrow_mut().take();
    show_sound_mode(state, None, false, false);
    show_equalizer(state, &EqualizerDisplay::default());
    clear_section_pages(state);
    state.set_connection(failure.connection());
    state.set_connection_reason("".into());
    state.set_failure_kind(failure.kind());
    state.set_failure_text(failure.text().into());
    state.set_error_label("".into());
}

/// Fills the vendored Sound mode page from a parsed state.
///
/// A device that reports a mode gets the three tiles, their labels and the
/// Sound mode page; a P20i gets none of them, so it never offers a mode change.
/// The 1-to-11 scale and the per-mode strength/wind rows stay out of scope, so
/// `mode-scale` is false and `mode-rows` stays empty.
fn show_sound_mode(
    state: &AppState<'_>,
    sound_mode: Option<SoundMode>,
    has_equalizer: bool,
    has_settings: bool,
) {
    state.set_modes(ModelRc::new(VecModel::from(mode_options(sound_mode))));
    state.set_mode_index(mode_index(sound_mode));
    state.set_mode_scale(false);
    state.set_mode_rows(ModelRc::default());
    state.set_sound_pages(ModelRc::new(VecModel::from(sound_pages(
        sound_mode,
        has_equalizer,
        has_settings,
    ))));
    let label = |mode: SoundMode| {
        if sound_mode.is_some() {
            mode_label(mode)
        } else {
            ""
        }
    };
    state.set_noise_canceling_label(label(SoundMode::NoiseCanceling).into());
    state.set_transparency_label(label(SoundMode::Transparency).into());
    state.set_normal_label(label(SoundMode::Normal).into());
}

/// Fills the sidebar device pages and the aside capability flags of a connected
/// state. Only the A3062's settings block adds the pages; the A3949 and every
/// other model keeps the MVP's surface, so it has no page and no write path.
fn show_device_pages(state: &AppState<'_>, has_settings: bool) {
    state.set_device_pages(ModelRc::new(VecModel::from(device_pages(has_settings))));
    // The A3062 has one control surface, an LDAC toggle and no spatial audio;
    // its case has no registers this slice reads.
    state.set_has_sides(false);
    state.set_has_ldac(has_settings);
    state.set_has_spatial(false);
    state.set_case_battery("".into());
    // The Connections page's "open Windows settings" links need a device page
    // that has them.
    state.set_system_settings_available(has_settings);
}

/// Empties the section pages of a device that is connecting, disconnected or
/// failed: no sidebar pages and no panels.
fn clear_section_pages(state: &AppState<'_>) {
    show_device_pages(state, false);
    state.set_panels(ModelRc::default());
}

/// Fills `AppState.panels` with the current page's settings from the last
/// connected state: on connect, after a sidebar page change, and after a
/// settings write's re-poll. A page without panels (Overview, the Sound mode
/// and Equalizer pages, or a model without the settings block such as the
/// A3949) gets none.
fn show_section_pages(state: &AppState<'_>, inner: &Rc<Inner>) {
    let device = inner.device.borrow();
    let panels = device
        .as_deref()
        .map(|device_state| panels_for_page(state.get_page(), device_state))
        .unwrap_or_default();
    state.set_panels(ModelRc::new(VecModel::from(panels)));
}

/// The double-press choices, in the page's order: the button does nothing, or
/// Bass Up.
pub const BUTTON_ACTION_OPTIONS: [&str; 2] = ["None", "Bass Up"];

/// The auto-power-off choices: off, then the four measured durations in minutes.
pub const AUTO_POWER_OFF_OPTIONS: [&str; 5] = [
    "Disabled",
    "30 minutes",
    "60 minutes",
    "90 minutes",
    "120 minutes",
];

/// The limit-high-volume dB reading refresh choices.
pub const REFRESH_RATE_OPTIONS: [&str; 3] = ["Real time", "10 seconds", "1 minute"];

/// The settings panels of one sidebar page, in the vendored section page's panel
/// layout.
///
/// The A3062 is the only model with a settings block; a state without one (the
/// A3949 and the rest of the catalog so far) has no panels, so it can neither
/// show a settings page nor name a setting for a write.
pub fn panels_for_page(page: Page, device_state: &DeviceState) -> Vec<SettingsPanel> {
    let Some(settings) = device_state.settings else {
        return Vec::new();
    };
    let panels: Vec<Option<SettingsPanel>> = match page {
        Page::Controls => vec![
            settings_panel(
                PanelKind::Presses,
                vec![
                    info_row("singlePress", "Single press", "Ambient sound mode"),
                    select_row(
                        "doublePress",
                        "Double press",
                        &BUTTON_ACTION_OPTIONS,
                        button_action_index(settings.button_double_press),
                    ),
                ],
            ),
            settings_panel(
                PanelKind::Cycle,
                cycle_rows(settings.ambient_sound_mode_cycle),
            ),
        ],
        Page::Comfort => vec![
            settings_panel(
                PanelKind::Plain,
                vec![toggle_row("sideTone", "Side tone", settings.side_tone)],
            ),
            settings_panel(
                PanelKind::Volume,
                vec![
                    toggle_row(
                        "limitHighVolume",
                        "Limit high volume",
                        settings.limit_high_volume.enabled,
                    ),
                    range_row(
                        "limitHighVolumeDbLimit",
                        "dB limit",
                        i32::from(DbLimit::MIN),
                        i32::from(DbLimit::MAX),
                        i32::from(DbLimit::STEP),
                        i32::from(settings.limit_high_volume.db_limit.value()),
                    ),
                    select_row(
                        "limitHighVolumeRefreshRate",
                        "Refresh rate",
                        &REFRESH_RATE_OPTIONS,
                        refresh_rate_index(settings.limit_high_volume.refresh_rate),
                    ),
                ],
            ),
        ],
        Page::Power => vec![settings_panel(
            PanelKind::Plain,
            vec![
                select_row(
                    "autoPowerOff",
                    "Auto power off",
                    &AUTO_POWER_OFF_OPTIONS,
                    auto_power_off_index(settings.auto_power_off),
                ),
                toggle_row(
                    "lowBatteryPrompt",
                    "Low battery prompt",
                    settings.low_battery_prompt,
                ),
                toggle_row("voicePrompt", "Voice prompt", settings.voice_prompt),
            ],
        )],
        // The A3062's dual-connections device list is a separate follow-up
        // slice, so the page has the toggle only.
        Page::Connections => vec![settings_panel(
            PanelKind::Plain,
            vec![toggle_row(
                "dualConnections",
                "Dual connections",
                settings.dual_connections,
            )],
        )],
        Page::Effects => vec![settings_panel(
            PanelKind::Plain,
            vec![
                toggle_row("ldac", "LDAC", settings.ldac),
                toggle_row("dolbyAudio", "Dolby audio", settings.dolby_audio),
            ],
        )],
        Page::About => vec![settings_panel(PanelKind::Info, about_rows(device_state))],
        _ => Vec::new(),
    };
    panels.into_iter().flatten().collect()
}

/// One settings panel, or `None` when it would have no rows; an empty panel
/// would still draw its frame.
fn settings_panel(kind: PanelKind, rows: Vec<SettingRow>) -> Option<SettingsPanel> {
    (!rows.is_empty()).then(|| SettingsPanel {
        kind,
        rows: ModelRc::new(VecModel::from(rows)),
    })
}

/// A base row with no control-specific fields; the helpers below fill in theirs.
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

fn select_row(id: &str, label: &str, options: &[&str], index: i32) -> SettingRow {
    let options: Vec<SharedString> = options.iter().map(|option| (*option).into()).collect();
    SettingRow {
        options: ModelRc::new(VecModel::from(options)),
        index,
        ..setting_row(id, RowKind::Select, label)
    }
}

fn range_row(
    id: &str,
    label: &str,
    minimum: i32,
    maximum: i32,
    step: i32,
    value: i32,
) -> SettingRow {
    SettingRow {
        minimum,
        maximum,
        step,
        value,
        ..setting_row(id, RowKind::Range, label)
    }
}

fn info_row(id: &str, label: &str, text: &str) -> SettingRow {
    SettingRow {
        text: text.into(),
        ..setting_row(id, RowKind::Info, label)
    }
}

/// The three cycle toggles of the Controls page, in the page's order.
fn cycle_rows(cycle: AmbientSoundModeCycle) -> Vec<SettingRow> {
    vec![
        toggle_row(
            "noiseCancelingModeInCycle",
            "Noise canceling",
            cycle.noise_canceling,
        ),
        toggle_row(
            "transparencyModeInCycle",
            "Transparency",
            cycle.transparency,
        ),
        toggle_row("normalModeInCycle", "Normal", cycle.normal),
    ]
}

/// The About rows: charging, battery and firmware. The serial is deliberately
/// omitted: the vendored Info row copies its whole text, and the spec keeps
/// serials out of screenshots and reports.
fn about_rows(device_state: &DeviceState) -> Vec<SettingRow> {
    let Some(single) = &device_state.single_battery else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if let Some(charging) = single.charging {
        let text = if charging { "Charging" } else { "Not charging" };
        rows.push(info_row("isCharging", "Is charging", text));
    }
    let battery = battery_tile(
        WHOLE_DEVICE_BATTERY_LABEL,
        single.value,
        single.scale.maximum(),
    );
    rows.push(info_row(
        "batteryLevel",
        "Battery level",
        battery.value.as_str(),
    ));
    if let Some(firmware) = single.firmware.as_deref() {
        rows.push(info_row("firmwareVersion", "Firmware version", firmware));
    }
    rows
}

/// The index of a double-press action in [`BUTTON_ACTION_OPTIONS`].
pub fn button_action_index(action: Option<ButtonAction>) -> i32 {
    match action {
        None => 0,
        Some(ButtonAction::BassUp) => 1,
    }
}

/// The index of an auto-power-off state in [`AUTO_POWER_OFF_OPTIONS`]: zero is
/// off, then the four durations.
pub fn auto_power_off_index(auto_power_off: AutoPowerOff) -> i32 {
    if auto_power_off.enabled {
        i32::from(auto_power_off.duration.index()) + 1
    } else {
        0
    }
}

/// The index of a dB reading refresh rate in [`REFRESH_RATE_OPTIONS`].
pub fn refresh_rate_index(refresh_rate: DecibelReadingRefreshRate) -> i32 {
    i32::from(refresh_rate.value())
}

/// The write a section-page toggle names, or `None` for any other id: every
/// setting that is not a toggle (or that has its own page, such as the Sound
/// mode tiles) has its own path, so an unknown id never writes.
pub fn toggle_command(id: &str, on: bool, current: &A3062Settings) -> Option<SettingCommand> {
    Some(match id {
        "sideTone" => SettingCommand::SideTone(on),
        "lowBatteryPrompt" => SettingCommand::LowBatteryPrompt(on),
        "voicePrompt" => SettingCommand::VoicePrompt(on),
        "ldac" => SettingCommand::Ldac(on),
        "dolbyAudio" => SettingCommand::DolbyAudio(on),
        "dualConnections" => SettingCommand::DualConnections(on),
        // The enable flag and the dB limit travel in one measured body, so the
        // toggle keeps the limit the device reports.
        "limitHighVolume" => SettingCommand::LimitHighVolume {
            enabled: on,
            db_limit: current.limit_high_volume.db_limit,
        },
        // A cycle toggle changes one bit and keeps the other two.
        "noiseCancelingModeInCycle" => {
            SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: on,
                ..current.ambient_sound_mode_cycle
            })
        }
        "transparencyModeInCycle" => SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
            transparency: on,
            ..current.ambient_sound_mode_cycle
        }),
        "normalModeInCycle" => SettingCommand::AmbientSoundModeCycle(AmbientSoundModeCycle {
            normal: on,
            ..current.ambient_sound_mode_cycle
        }),
        _ => return None,
    })
}

/// The write a section-page select names, or `None` for an unknown id or an
/// index outside the row's choices.
pub fn option_command(id: &str, index: i32, current: &A3062Settings) -> Option<SettingCommand> {
    match id {
        // Choosing a duration also enables auto power off; choosing "Disabled"
        // keeps the duration the device reports, exactly as OpenSCL4's handler
        // does, so switching back on restores it.
        "autoPowerOff" => {
            let auto_power_off = match index {
                0 => AutoPowerOff {
                    enabled: false,
                    duration: current.auto_power_off.duration,
                },
                1..=4 => AutoPowerOff {
                    enabled: true,
                    duration: AutoPowerOffDuration::from_index(u8::try_from(index - 1).ok()?)?,
                },
                _ => return None,
            };
            Some(SettingCommand::AutoPowerOff(auto_power_off))
        }
        "limitHighVolumeRefreshRate" => {
            refresh_rate_from_index(index).map(SettingCommand::LimitHighVolumeRefreshRate)
        }
        "doublePress" => match index {
            0 => Some(SettingCommand::ButtonDoublePress(None)),
            1 => Some(SettingCommand::ButtonDoublePress(Some(
                ButtonAction::BassUp,
            ))),
            _ => None,
        },
        _ => None,
    }
}

/// The write a section-page range release names, or `None` for an unknown id or
/// a value outside the measured dB steps.
pub fn number_command(id: &str, value: i32, current: &A3062Settings) -> Option<SettingCommand> {
    if id != "limitHighVolumeDbLimit" {
        return None;
    }
    let db_limit = DbLimit::new(u8::try_from(value).ok()?)?;
    Some(SettingCommand::LimitHighVolume {
        enabled: current.limit_high_volume.enabled,
        db_limit,
    })
}

fn refresh_rate_from_index(index: i32) -> Option<DecibelReadingRefreshRate> {
    usize::try_from(index)
        .ok()
        .and_then(|index| DecibelReadingRefreshRate::ALL.get(index).copied())
}

/// Fills the vendored Equalizer page from the parsed state.
///
/// The page is editable exactly when this slice can write a custom equalizer
/// body for the model (the A3062): the band controls, the range note, the reset
/// button and the saved-profile panel with its Save button are enabled and the
/// eight visible bands are editable. A model whose equalizer is deferred (the
/// A3949) keeps all of them hidden, so it has no page and no profile writes.
/// The saved-profile panel offers import (paste and file), which the same
/// editability gates: only the A3062's page shows the panel. Export and the
/// legacy migration stay inert: the export count and the legacy list are always
/// empty and no export or migrate action is wired.
fn show_equalizer(state: &AppState<'_>, display: &EqualizerDisplay) {
    state.set_eq_available(display.available);
    state.set_eq_read_only(!display.editable);
    state.set_eq_presets(ModelRc::new(VecModel::from(display.presets.clone())));
    state.set_eq_preset(display.preset.as_str().into());
    state.set_eq_preset_count(display.preset_count);
    state.set_eq_expanded(false);
    state.set_eq_band_count(display.band_count);
    state.set_eq_min(display.min);
    state.set_eq_max(display.max);
    state.set_eq_min_text(display.min_text.as_str().into());
    state.set_eq_max_text(display.max_text.as_str().into());
    state.set_eq_bands(ModelRc::new(VecModel::from(display.bands.clone())));
    state.set_eq_lines(ModelRc::new(VecModel::from(display.lines.clone())));
    state.set_eq_chart_line(display.chart_line.as_str().into());
    state.set_eq_chart_area(display.chart_area.as_str().into());
    state.set_eq_name(display.name.as_str().into());
    state.set_has_saved_profiles(display.editable);
    state.set_eq_profiles(ModelRc::default());
    state.set_eq_export_count(0);
    state.set_eq_legacy(ModelRc::default());
}

/// Fills the saved-profile rows of the vendored Equalizer page from the saved
/// set and the bands the page now shows, so the profile whose bands are exactly
/// the shown bands carries the “Applied” tag.
fn show_saved_profiles(state: &AppState<'_>, profiles: &ProfileSet) {
    let bands = state.shown_band_values();
    state.set_eq_profiles(ModelRc::new(VecModel::from(profile_rows(profiles, &bands))));
}

/// Applies a finished sound-mode change: the mode the device reports becomes the
/// selected tile, or the error banner names the setting that failed.
fn apply_mode_outcome(
    window: &AppWindow,
    inner: &Rc<Inner>,
    outcome: Result<SoundMode, ModeFailure>,
) {
    let state = window.global::<AppState>();
    match outcome {
        Ok(reported) => {
            state.set_mode_index(mode_index(Some(reported)));
            state.set_error_label("".into());
        }
        Err(failure) => {
            // The worker closes the session after a failed change; Retry in the
            // banner opens a fresh connection.
            inner.stop_worker();
            state.set_error_label(failure.label().into());
        }
    }
}

/// Applies a finished settings change: the re-polled state replaces the
/// controller's copy and refills the panels and About, or the error banner names
/// the setting that failed.
fn apply_setting_outcome(
    window: &AppWindow,
    inner: &Rc<Inner>,
    setting: SettingCommand,
    outcome: Result<Box<DeviceState>, SettingFailure>,
) {
    let state = window.global::<AppState>();
    match outcome {
        Ok(reported) => {
            *inner.device.borrow_mut() = Some(reported);
            show_section_pages(&state, inner);
            state.set_error_label("".into());
        }
        Err(failure) => {
            // The worker closes the session after a failed write; Retry in the
            // banner opens a fresh connection.
            inner.stop_worker();
            state.set_error_label(failure.label(setting).into());
        }
    }
}

/// Applies a finished equalizer preset change: the preset and curve the device
/// reports become the page's, or the error banner names the setting that
/// failed. The preset catalog is kept; only the state-dependent fields change.
fn apply_preset_outcome(
    window: &AppWindow,
    inner: &Rc<Inner>,
    outcome: Result<EqualizerState, EqFailure>,
) {
    let state = window.global::<AppState>();
    match outcome {
        Ok(reported) => {
            show_equalizer_state(&state, &reported);
            state.set_error_label("".into());
        }
        Err(failure) => {
            // The worker closes the session after a failed change; Retry in the
            // banner opens a fresh connection.
            inner.stop_worker();
            state.set_error_label(failure.label().into());
        }
    }
}

/// Replaces the state-dependent Equalizer page fields with a re-read state: the
/// current preset (the custom id when the device reports custom bands), its
/// bands and the curve. The catalog, range and page availability are unchanged
/// by a write.
fn show_equalizer_state(state: &AppState<'_>, equalizer: &EqualizerState) {
    let bands = equalizer_bands(equalizer);
    let (chart_line, chart_area) = equalizer_chart(&equalizer_values(equalizer), EQ_MIN, EQ_MAX);
    state.set_eq_preset(equalizer.preset_id.to_string().into());
    state.set_eq_bands(ModelRc::new(VecModel::from(bands)));
    state.set_eq_lines(ModelRc::new(VecModel::from(equalizer_lines())));
    state.set_eq_chart_line(chart_line.into());
    state.set_eq_chart_area(chart_area.into());
    state.set_eq_name(
        equalizer
            .preset()
            .map(|preset| preset.name)
            .unwrap_or_default()
            .into(),
    );
}

/// Answers a choice control on the UI thread. The Sound mode tiles keep their
/// own `ambientSoundMode` id and `06 81` write; every other id is a section row,
/// and an id or index this slice does not write is ignored, so no other frame
/// can be written.
fn set_option(window: &AppWindow, inner: &Rc<Inner>, setting_id: &str, index: i32) {
    if setting_id == "ambientSoundMode" {
        set_ambient_sound_mode(window, inner, index);
        return;
    }
    answer_setting(window, inner, |settings| {
        option_command(setting_id, index, settings)
    });
}

/// Answers a Sound mode tile: queues one `06 81` write on the device worker.
fn set_ambient_sound_mode(window: &AppWindow, inner: &Rc<Inner>, index: i32) {
    let state = window.global::<AppState>();
    // No connection, or a model without a measured sound mode, means no tiles
    // and no `06 81` frame. The session refuses the write as well.
    if state.get_connection() != Connection::Connected || state.get_modes().row_count() == 0 {
        return;
    }
    let Some(mode) = mode_for_index(index) else {
        return;
    };
    let sent = inner.send_queued(&inner.mode_change, |request| {
        DeviceCommand::ChangeSoundMode { request, mode }
    });
    if !sent {
        // The worker of a previous failed change has closed the connection; the
        // banner's Retry reconnects.
        state.set_error_label(ModeFailure::Disconnected.label().into());
    }
}

/// Answers a toggle on a section page.
fn set_toggle(window: &AppWindow, inner: &Rc<Inner>, setting_id: &str, on: bool) {
    answer_setting(window, inner, |settings| {
        toggle_command(setting_id, on, settings)
    });
}

/// Answers a range release on a section page.
fn set_number(window: &AppWindow, inner: &Rc<Inner>, setting_id: &str, value: i32) {
    answer_setting(window, inner, |settings| {
        number_command(setting_id, value, settings)
    });
}

/// Queues one settings write on the device worker, named from the last connected
/// state. Nothing is sent when the connection cannot write (not connected, or a
/// state without the settings block such as the A3949) or the click names no
/// write.
fn answer_setting(
    window: &AppWindow,
    inner: &Rc<Inner>,
    command: impl FnOnce(&A3062Settings) -> Option<SettingCommand>,
) {
    let state = window.global::<AppState>();
    // No connection, or a model without the settings block, means no settings
    // page and no settings frame; the session refuses the write as well.
    if state.get_connection() != Connection::Connected {
        return;
    }
    let settings = inner
        .device
        .borrow()
        .as_ref()
        .and_then(|device_state| device_state.settings);
    let Some(settings) = settings else {
        return;
    };
    let Some(setting) = command(&settings) else {
        return;
    };
    let sent = inner.send_queued(&inner.setting_change, |request| DeviceCommand::SetSetting {
        request,
        setting,
    });
    if !sent {
        // The worker of a previous failed write has closed the connection; the
        // banner's Retry reconnects.
        state.set_error_label(SettingFailure::Disconnected.label(setting).into());
    }
}

/// Copies a section row's text to the Windows clipboard, as the vendored Info
/// row's copy button asks.
fn copy_text(text: &str) {
    if text.is_empty() {
        return;
    }
    if let Err(error) = set_clipboard_text(text) {
        // Copying is not a device write; a clipboard problem is only logged.
        eprintln!("OpenSCL4: could not copy to the clipboard: {error}");
    }
}

/// Puts `text` on the clipboard; split from [`copy_text`] so the action stays
/// one line.
fn set_clipboard_text(text: &str) -> Result<(), arboard::Error> {
    arboard::Clipboard::new()?.set_text(text.to_owned())
}

/// The Windows Settings URI of a vendored system page.
pub fn system_settings_uri(page: SystemPage) -> &'static str {
    match page {
        SystemPage::Bluetooth => "ms-settings:bluetooth",
        SystemPage::Sound => "ms-settings:sound",
    }
}

/// Opens one of the Windows settings pages, as the Connections page's links ask.
fn open_system_settings(page: SystemPage) {
    let uri = windows::core::HSTRING::from(system_settings_uri(page));
    unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            &uri,
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

/// Answers an Equalizer preset chip. Only a preset id the page offers is
/// accepted, so an unknown or stale value never writes. A chip written after a
/// custom curve switches the device back to that built-in preset; the custom
/// band callbacks live in [`answer_band_edit`].
fn select_equalizer_preset(window: &AppWindow, inner: &Rc<Inner>, value: &str) {
    let state = window.global::<AppState>();
    // No connection, or a model whose equalizer is deferred (the A3949), means
    // no chips and no `03 87` frame.
    if state.get_connection() != Connection::Connected || state.get_eq_presets().row_count() == 0 {
        return;
    }
    let presets: Vec<ModeOption> = state.get_eq_presets().iter().collect();
    let Some(preset_id) = selectable_preset_id(&presets, value) else {
        return;
    };
    let sent = inner.send_queued(&inner.preset_change, |request| {
        DeviceCommand::ChangeEqualizerPreset { request, preset_id }
    });
    if !sent {
        // The worker of a previous failed change has closed the connection; the
        // banner's Retry reconnects.
        state.set_error_label(EqFailure::Disconnected.label().into());
    }
}

/// The value of `AppState.eq-save-problem` the vendored save dialog compares
/// against: `None` shows no message, `EmptyName` shows “Enter a profile name.”
/// and `NeedsReplace` shows the replace question. The numbers are part of the
/// vendored `.slint` dialog contract; the controller only names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveProblem {
    None,
    EmptyName,
    NeedsReplace,
}

impl SaveProblem {
    /// The `eq-save-problem` integer the vendored dialog compares to.
    const fn value(self) -> i32 {
        match self {
            Self::None => 0,
            Self::EmptyName => 1,
            Self::NeedsReplace => 2,
        }
    }
}

/// The `AppState.error-label` of a failed saved-profile document write. The
/// vendored banner wraps a label as “Could not change “{}”.”, so the visible
/// text says the profiles could not be saved; the banner's second sentence
/// (“The device kept its previous value.”) is the vendored device-change wording
/// and does not describe a file failure, but there is no other visible error
/// slot without editing the vendored UI.
const PROFILE_WRITE_FAILURE_LABEL: &str = "Could not save equalizer profiles";

/// The saved-profile dialog's answer to a save click: either the fields the
/// dialog shows for a name it cannot store, or the name to store and whether its
/// saved bands are being replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SaveRequest {
    /// The trimmed name is empty, so the dialog shows `eq-save-problem = 1`.
    EmptyName,
    /// A profile of that name is saved and the click did not ask to replace it:
    /// the dialog shows problem 2 and the existing name.
    NeedsReplace { name: String },
    /// The name can be stored: added, or its saved bands replaced when
    /// `replace` is true and the name already exists.
    Store { name: String, replace: bool },
}

/// Decides what an `Actions.eq-save(name, replace)` click asks for. Names are
/// compared after trimming, exactly like the profile model does.
fn save_request(set: &ProfileSet, name: &str, replace: bool) -> SaveRequest {
    let name = name.trim();
    if name.is_empty() {
        return SaveRequest::EmptyName;
    }
    let exists = set.contains(name);
    if exists && !replace {
        return SaveRequest::NeedsReplace {
            name: name.to_owned(),
        };
    }
    SaveRequest::Store {
        name: name.to_owned(),
        replace: exists,
    }
}

/// Whether a saved-profile change reached the profile document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Persistence {
    /// The whole set was written to the document.
    Written,
    /// The document could not be written; the set was rolled back.
    NotWritten,
}

/// Writes `set` to the saved-profile document, restoring `before` and returning
/// [`Persistence::NotWritten`] when the document cannot be written, so the saved
/// list and the document always agree.
///
/// A missing app-data directory is a persistence failure too: the profiles would
/// not survive a restart, so the page must not report them as saved. The failure
/// is also logged to stderr, which is the console trace the spec asks for next to
/// the page banner.
fn persist_saved_profiles(
    set: &mut ProfileSet,
    before: ProfileSet,
    path: Option<&Path>,
) -> Persistence {
    let Some(path) = path else {
        log_no_app_data_directory();
        *set = before;
        return Persistence::NotWritten;
    };
    match save_profiles(path, set) {
        Ok(()) => Persistence::Written,
        Err(error) => {
            log_profiles(
                &format!("could not save the equalizer profiles ({error})"),
                path,
            );
            *set = before;
            Persistence::NotWritten
        }
    }
}

/// What an `eq-save` click did, after the dialog decision and the file write.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SaveOutcome {
    /// The dialog stays open with problem 1: the typed name is empty.
    EmptyName,
    /// The dialog stays open with problem 2 and the existing name, asking before
    /// its saved values are replaced.
    NeedsReplace { name: String },
    /// The bands or the name cannot become a profile. Defensive: the editable
    /// page always shows eight in-range bands and the dialog rejects empty names.
    Invalid,
    /// The profile is saved and the document written; the dialog closes.
    Saved,
    /// The profile is not saved: the document cannot be written, so the set is
    /// rolled back, the dialog closes and the page banner explains why.
    NotWritten,
}

/// Saves the eight `bands` under `name` and writes the whole set to the
/// document. A click that cannot store the profile changes nothing; a failed
/// document write rolls the set back, so the saved list never shows a profile the
/// document does not have.
fn save_profile(
    set: &mut ProfileSet,
    path: Option<&Path>,
    name: &str,
    replace: bool,
    bands: [i16; VISIBLE_EQ_BANDS],
) -> SaveOutcome {
    let (name, replace) = match save_request(set, name, replace) {
        SaveRequest::EmptyName => return SaveOutcome::EmptyName,
        SaveRequest::NeedsReplace { name } => return SaveOutcome::NeedsReplace { name },
        SaveRequest::Store { name, replace } => (name, replace),
    };
    let profile = match EqualizerProfile::new(name, bands) {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("OpenSCL4: cannot save the equalizer profile: {error}");
            return SaveOutcome::Invalid;
        }
    };
    let before = set.clone();
    let stored = if replace {
        set.replace(profile)
    } else {
        set.add(profile)
    };
    if let Err(error) = stored {
        // The dialog checked the name, so this is only reached defensively.
        eprintln!("OpenSCL4: cannot save the equalizer profile: {error}");
        *set = before;
        return SaveOutcome::Invalid;
    }
    if persist_saved_profiles(set, before, path) == Persistence::Written {
        SaveOutcome::Saved
    } else {
        SaveOutcome::NotWritten
    }
}

/// Clears the error banner when it shows a failed saved-profile write, leaving
/// any device-change error banner in place.
fn clear_profile_write_error(state: &AppState<'_>) {
    if state.get_error_label() == PROFILE_WRITE_FAILURE_LABEL {
        state.set_error_label("".into());
    }
}

/// The Save profile dialog fields an `eq-save` click changes. The real
/// implementation is the Slint `AppState`; tests record the calls so the dialog
/// behaviour can be checked without a window.
trait SaveDialogPage {
    /// Shows the `eq-save-problem` value the vendored dialog compares to.
    fn show_save_problem(&self, problem: SaveProblem);
    /// Names the saved profile the replace question is about.
    fn show_save_existing(&self, name: &str);
    /// Closes the dialog (`eq-saving = false`).
    fn close_save_dialog(&self);
    /// Shows the readable banner label of a failed document write.
    fn show_profile_error(&self, label: &str);
    /// Clears the banner when it shows a saved-profile write failure.
    fn clear_profile_error(&self);
}

impl SaveDialogPage for AppState<'_> {
    fn show_save_problem(&self, problem: SaveProblem) {
        self.set_eq_save_problem(problem.value());
    }

    fn show_save_existing(&self, name: &str) {
        self.set_eq_save_existing(name.into());
    }

    fn close_save_dialog(&self) {
        self.set_eq_saving(false);
    }

    fn show_profile_error(&self, label: &str) {
        self.set_error_label(label.into());
    }

    fn clear_profile_error(&self) {
        clear_profile_write_error(self);
    }
}

/// Answers an `eq-save` click: stores the profile and shows the outcome on the
/// dialog or the page banner. The dialog closes both on success and on a failed
/// document write: the vendored dialog only has name-problem slots, so closing it
/// is the only way the page's error banner can be seen.
fn answer_save_click(
    page: &impl SaveDialogPage,
    set: &mut ProfileSet,
    path: Option<&Path>,
    name: &str,
    replace: bool,
    bands: [i16; VISIBLE_EQ_BANDS],
) -> SaveOutcome {
    let outcome = save_profile(set, path, name, replace, bands);
    match &outcome {
        SaveOutcome::EmptyName => page.show_save_problem(SaveProblem::EmptyName),
        SaveOutcome::NeedsReplace { name } => {
            page.show_save_problem(SaveProblem::NeedsReplace);
            page.show_save_existing(name);
        }
        // The vendored dialog has one rejection slot for a name it cannot store,
        // so an unusable profile is reported like an empty name.
        SaveOutcome::Invalid => page.show_save_problem(SaveProblem::EmptyName),
        SaveOutcome::Saved => {
            page.show_save_problem(SaveProblem::None);
            page.show_save_existing("");
            page.close_save_dialog();
            page.clear_profile_error();
        }
        SaveOutcome::NotWritten => {
            page.show_save_problem(SaveProblem::None);
            page.close_save_dialog();
            page.show_profile_error(PROFILE_WRITE_FAILURE_LABEL);
        }
    }
    outcome
}

/// Answers `Actions.eq-save(name, replace)`.
///
/// The page's eight current (clamped) bands are saved under the trimmed name and
/// the whole set is written to the document; the dialog closes when the profile is
/// saved. A profile that cannot be written is not kept: the set is rolled back, the
/// saved list is unchanged and the page banner says the profiles could not be
/// saved.
fn save_equalizer_profile(window: &AppWindow, inner: &Rc<Inner>, name: &str, replace: bool) {
    let state = window.global::<AppState>();
    // Saving needs the editable A3062 page the Save button is shown on.
    if !band_edit_allowed(&state) {
        return;
    }
    let Ok(bands) = <[i16; VISIBLE_EQ_BANDS]>::try_from(state.shown_band_values()) else {
        // The editable A3062 page always shows eight bands; anything else cannot
        // become a profile and is reported like an unusable name.
        SaveDialogPage::show_save_problem(&state, SaveProblem::EmptyName);
        return;
    };
    let outcome = {
        let mut profiles = inner.profiles.borrow_mut();
        answer_save_click(
            &state,
            &mut profiles,
            inner.profiles_path.as_deref(),
            name,
            replace,
            bands,
        )
    };
    if outcome == SaveOutcome::Saved {
        show_saved_profiles(&state, &inner.profiles.borrow());
    }
}

/// Answers `Actions.eq-apply(name)`: queues the saved profile's bands as exactly
/// one custom `03 87` write; the device worker re-polls `01 01` afterwards, so
/// the bands, the chart and the Applied tag follow the device. A missing profile
/// queues nothing, and a stopped worker becomes the readable error banner
/// instead of a silent no-op.
fn apply_equalizer_profile(window: &AppWindow, inner: &Rc<Inner>, name: &str) {
    let state = window.global::<AppState>();
    // No page (the deferred A3949), no editable bands or no connection means
    // there is no profile write to make.
    if !band_edit_allowed(&state) {
        return;
    }
    let outcome = apply_saved_profile(&inner.profiles.borrow(), name, &**inner);
    match outcome {
        ApplyOutcome::Queued => state.set_error_label("".into()),
        ApplyOutcome::Disconnected => {
            state.set_error_label(EqFailure::Disconnected.bands_label().into());
        }
        ApplyOutcome::Unknown => {
            eprintln!("OpenSCL4: no saved equalizer profile named {name:?}");
        }
    }
}

/// What an `eq-delete` click did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteOutcome {
    /// The profile was removed and the smaller set written.
    Removed,
    /// No saved profile has that name; the dialog just closes.
    NotFound,
    /// The document could not be written, so the profile is still saved: the set
    /// was rolled back and the page banner explains why.
    NotWritten,
}

/// Removes the profile named `name` and writes the smaller set to the document.
/// A failed document write restores the profile, so the row list and the document
/// always agree.
fn delete_profile(set: &mut ProfileSet, path: Option<&Path>, name: &str) -> DeleteOutcome {
    if !set.contains(name) {
        return DeleteOutcome::NotFound;
    }
    let before = set.clone();
    set.remove(name);
    if persist_saved_profiles(set, before, path) == Persistence::Written {
        DeleteOutcome::Removed
    } else {
        DeleteOutcome::NotWritten
    }
}

/// Answers `Actions.eq-delete(name)`, after the `eq-deleting` dialog confirmed
/// it: the profile is removed, persisted and the list is refreshed. A document
/// write failure restores the profile and shows the page banner, so the row is
/// not silently lost. The `eq-deleting` field is cleared last, so a name that is
/// no longer saved only closes the dialog.
fn delete_equalizer_profile(window: &AppWindow, inner: &Rc<Inner>, name: &str) {
    let state = window.global::<AppState>();
    let outcome = {
        let mut profiles = inner.profiles.borrow_mut();
        delete_profile(&mut profiles, inner.profiles_path.as_deref(), name)
    };
    match outcome {
        DeleteOutcome::Removed => {
            show_saved_profiles(&state, &inner.profiles.borrow());
            clear_profile_write_error(&state);
        }
        DeleteOutcome::NotWritten => {
            show_saved_profiles(&state, &inner.profiles.borrow());
            state.set_error_label(PROFILE_WRITE_FAILURE_LABEL.into());
        }
        DeleteOutcome::NotFound => {}
    }
    state.set_eq_deleting("".into());
}

/// The largest equalizer-profiles file the Import dialog accepts, in bytes: the
/// same one-mebibyte limit OpenSCL4 uses. A larger file is refused before it is
/// read, so it never reaches memory.
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

/// Why an import file cannot be read; the Import panel's note says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportFileError {
    /// The file is larger than [`MAX_IMPORT_BYTES`].
    TooLarge,
    /// The file cannot be read as text: it is missing, is a directory, or is not
    /// UTF-8.
    ReadFailed,
}

impl ImportFileError {
    /// The `TransferNote` the Import panel shows for this error.
    fn note(self) -> TransferNote {
        match self {
            Self::TooLarge => TransferNote::TooLarge,
            Self::ReadFailed => TransferNote::ReadFailed,
        }
    }
}

/// Reads an import file, checking its size before reading it.
fn read_import_file(path: &Path) -> Result<String, ImportFileError> {
    let size = fs::metadata(path)
        .map_err(|error| {
            log_import_file(&format!("could not read its size ({error})"), path);
            ImportFileError::ReadFailed
        })?
        .len();
    if size > MAX_IMPORT_BYTES {
        return Err(ImportFileError::TooLarge);
    }
    fs::read_to_string(path).map_err(|error| {
        log_import_file(&format!("could not read it ({error})"), path);
        ImportFileError::ReadFailed
    })
}

/// Logs one import-file message to stderr. The Import panel's note is the visible
/// report; the console keeps the operating system's error text.
fn log_import_file(message: &str, path: &Path) {
    eprintln!("OpenSCL4: {message}: {}", path.display());
}

/// What the Import dialog asks the machine for: the clipboard's text and a file
/// picked in the native dialog. The real implementation is [`NativeImportIo`];
/// tests substitute a fake so neither the clipboard nor a dialog is touched.
trait ImportIo {
    /// The clipboard's text, or `None` when it holds no text or cannot be read.
    fn clipboard_text(&self) -> Option<String>;
    /// The file the user picked, or `None` when the dialog was cancelled.
    fn pick_file(&self) -> Option<PathBuf>;
    /// The text of an import file, or why it cannot be imported.
    fn read_import_file(&self, path: &Path) -> Result<String, ImportFileError>;
}

/// The real [`ImportIo`]: arboard for the clipboard and rfd for the Windows JSON
/// file dialog.
struct NativeImportIo;

impl ImportIo for NativeImportIo {
    fn clipboard_text(&self) -> Option<String> {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        clipboard.get_text().ok()
    }

    /// The rfd Windows backend initializes COM as an apartment-threaded process;
    /// [`begin_import_file`] calls this on a worker thread, so the dialog never
    /// fights the Slint window thread's own COM mode.
    fn pick_file(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title("Import equalizer profiles")
            .add_filter("JSON", &["json"])
            .add_filter("All files", &["*"])
            .pick_file()
    }

    fn read_import_file(&self, path: &Path) -> Result<String, ImportFileError> {
        read_import_file(path)
    }
}

/// The Import dialog fields an import action changes. The real implementation is
/// the Slint `AppState`; the tests record the calls so the dialog behaviour can
/// be checked without a window.
trait ImportPage {
    /// Opens the dialog on the parsed file: its rows and whether the Import
    /// button is blocked.
    fn show_preview(&self, rows: Vec<ImportItem>, blocked: bool);
    /// Shows the Import panel's note and the number its text may use.
    fn show_import_note(&self, note: TransferNote, count: i32);
    /// Closes the dialog (`importing = false`).
    fn close_import_dialog(&self);
    /// Shows the short toast at the bottom of the window.
    fn show_import_toast(&self, kind: ToastKind, count: i32);
    /// Shows the page banner for a failed profile-document write.
    fn show_import_error(&self, label: &str);
}

impl ImportPage for AppState<'_> {
    fn show_preview(&self, rows: Vec<ImportItem>, blocked: bool) {
        self.set_import_items(ModelRc::new(VecModel::from(rows)));
        self.set_import_blocked(blocked);
        self.set_importing(true);
    }

    fn show_import_note(&self, note: TransferNote, count: i32) {
        self.set_transfer_note(note);
        self.set_transfer_count(count);
    }

    fn close_import_dialog(&self) {
        self.set_importing(false);
    }

    fn show_import_toast(&self, kind: ToastKind, count: i32) {
        self.set_toast(kind);
        self.set_toast_count(count);
    }

    fn show_import_error(&self, label: &str) {
        self.set_error_label(label.into());
    }
}

/// The vendored `ImportDialog` rows of a preview: the name, the curve through
/// the bands (none for a blocked row), the numeric status its tag compares to,
/// and the band count the file carries. The dialog adds the tag's text.
fn import_rows(preview: &ImportPreview) -> Vec<ImportItem> {
    preview
        .items()
        .iter()
        .map(|item| ImportItem {
            name: item.name.as_str().into(),
            curve: item
                .bands
                .as_ref()
                .map(|bands| profile_curve(bands))
                .unwrap_or_default()
                .into(),
            status: item.status as i32,
            bands: i32::try_from(item.band_count).unwrap_or(0),
        })
        .collect()
}

/// Parses `text` and opens the Import dialog for it, or shows the note that says
/// why it is not an equalizer-profiles file. A file with no profiles counts as
/// not a profiles file, as OpenSCL4 does: there would be nothing to import.
fn preview_import_text(
    page: &impl ImportPage,
    saved: &ProfileSet,
    import_text: &mut Option<String>,
    text: &str,
) {
    match preview_import(text, saved) {
        Ok(preview) if !preview.is_empty() => {
            let blocked = preview.is_blocked();
            page.show_preview(import_rows(&preview), blocked);
            page.show_import_note(TransferNote::None, 0);
            *import_text = Some(text.to_owned());
        }
        _ => {
            *import_text = None;
            page.show_import_note(TransferNote::NotProfiles, 0);
        }
    }
}

/// Shows what reading an import file became: its preview, or the note saying why
/// it cannot be imported.
fn preview_import_file(
    page: &impl ImportPage,
    saved: &ProfileSet,
    import_text: &mut Option<String>,
    result: Result<String, ImportFileError>,
) {
    match result {
        Ok(text) => preview_import_text(page, saved, import_text, &text),
        Err(error) => {
            *import_text = None;
            page.show_import_note(error.note(), 0);
        }
    }
}

/// Answers `Actions.eq-import-paste`: previews the clipboard's text, or notes
/// that the clipboard is empty. A whitespace-only clipboard is empty, as it is
/// in OpenSCL4.
fn import_from_clipboard(
    page: &impl ImportPage,
    saved: &ProfileSet,
    import_text: &mut Option<String>,
    io: &dyn ImportIo,
) {
    match io.clipboard_text() {
        Some(text) if !text.trim().is_empty() => {
            preview_import_text(page, saved, import_text, &text);
        }
        _ => {
            *import_text = None;
            page.show_import_note(TransferNote::ClipboardEmpty, 0);
        }
    }
}

/// Answers `Actions.eq-import-file` through the [`ImportIo`] seam: previews the
/// file the user picked. A cancelled dialog changes nothing.
#[cfg(test)]
fn import_from_file(
    page: &impl ImportPage,
    saved: &ProfileSet,
    import_text: &mut Option<String>,
    io: &dyn ImportIo,
) {
    let Some(path) = io.pick_file() else {
        return;
    };
    let result = io.read_import_file(&path);
    preview_import_file(page, saved, import_text, result);
}

/// What an `Actions.eq-import` click did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportOutcome {
    /// No preview is open, so there was nothing to import.
    Nothing,
    /// The preview is blocked: the dialog stays open and nothing is written.
    Blocked,
    /// The text is not an importable profiles file; the dialog closed.
    NotProfiles,
    /// The profiles were merged and persisted; the number is how many.
    Imported(usize),
    /// The profile document could not be written; the set was rolled back and
    /// the page banner says why.
    NotWritten,
}

/// Answers `Actions.eq-import` for a previewed file: the text is re-parsed
/// against the saved set, so a blocked file writes nothing. An importable
/// preview is merged all-or-nothing (a same-named profile is replaced in place,
/// a new one appended) and the whole set is persisted, so the list, the document
/// and the Imported note always agree.
fn answer_import_click(
    page: &impl ImportPage,
    saved: &mut ProfileSet,
    path: Option<&Path>,
    import_text: &mut Option<String>,
) -> ImportOutcome {
    let Some(text) = import_text.as_deref() else {
        return ImportOutcome::Nothing;
    };
    let preview = match preview_import(text, saved) {
        Ok(preview) if !preview.is_empty() => preview,
        _ => {
            *import_text = None;
            page.close_import_dialog();
            page.show_import_note(TransferNote::NotProfiles, 0);
            return ImportOutcome::NotProfiles;
        }
    };
    if preview.is_blocked() {
        // The dialog keeps its rows and its red note; nothing is written.
        return ImportOutcome::Blocked;
    }
    let count = preview.len();
    let before = saved.clone();
    if preview.merge_into(saved).is_err() {
        // `merge_into` refuses only a blocked preview, which is handled above;
        // this keeps the importable path harmless if that ever changes.
        *saved = before;
        *import_text = None;
        page.close_import_dialog();
        page.show_import_note(TransferNote::NotProfiles, 0);
        return ImportOutcome::NotProfiles;
    }
    if persist_saved_profiles(saved, before, path) == Persistence::NotWritten {
        *import_text = None;
        page.close_import_dialog();
        page.show_import_note(TransferNote::None, 0);
        page.show_import_error(PROFILE_WRITE_FAILURE_LABEL);
        return ImportOutcome::NotWritten;
    }
    *import_text = None;
    page.close_import_dialog();
    let shown = i32::try_from(count).unwrap_or(i32::MAX);
    page.show_import_note(TransferNote::Imported, shown);
    page.show_import_toast(ToastKind::Imported, shown);
    ImportOutcome::Imported(count)
}

/// Answers `Actions.eq-import-paste` on the window through the real clipboard.
fn eq_import_paste(window: &AppWindow, inner: &Rc<Inner>) {
    let state = window.global::<AppState>();
    let mut import_text = inner.import_text.borrow_mut();
    import_from_clipboard(
        &state,
        &inner.profiles.borrow(),
        &mut import_text,
        &*inner.import_io,
    );
}

/// Answers `Actions.eq-import-file`: the native dialog and the file read run on a
/// worker thread, so the window keeps drawing while the dialog is open. The
/// result arrives through the same event channel as the Bluetooth workers.
fn begin_import_file(inner: &Rc<Inner>) {
    if inner.picking_import_file.get() {
        return;
    }
    inner.picking_import_file.set(true);
    let io = Arc::clone(&inner.import_io);
    let sender = inner.sender.clone();
    std::thread::spawn(move || {
        let result = io.pick_file().map(|path| io.read_import_file(&path));
        // A closed channel means the window is gone; nothing is left to report.
        let _ = sender.send(Event::ImportFilePicked { result });
    });
}

/// Answers `Actions.eq-import` on the window: imports the preview open in the
/// dialog, relists the saved profiles and clears a stale write banner.
fn answer_import(inner: &Rc<Inner>) {
    let Some(window) = inner.ui.upgrade() else {
        return;
    };
    let state = window.global::<AppState>();
    // The vendored Import button only exists while the dialog is open, so a
    // stray click with no preview does nothing.
    if !state.get_importing() {
        return;
    }
    let outcome = {
        let mut import_text = inner.import_text.borrow_mut();
        let mut profiles = inner.profiles.borrow_mut();
        answer_import_click(
            &state,
            &mut profiles,
            inner.profiles_path.as_deref(),
            &mut import_text,
        )
    };
    if let ImportOutcome::Imported(_) = outcome {
        show_saved_profiles(&state, &inner.profiles.borrow());
        clear_profile_write_error(&state);
        clear_import_toast_after_a_while(&window);
    }
}

/// Hides the Imported toast after a few seconds, unless a newer toast replaced
/// it. The vendored window has no toast timer of its own.
fn clear_import_toast_after_a_while(window: &AppWindow) {
    let weak = window.as_weak();
    Timer::single_shot(Duration::from_secs(3), move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<AppState>();
        if state.get_toast() == ToastKind::Imported {
            state.set_toast(ToastKind::None);
            state.set_toast_count(0);
        }
    });
}

/// Replaces the dialog's model list with the models for a query and chip; an
/// index that is not one of the vendored chips lists nothing.
fn show_models(state: &AppState<'_>, query: &str, chip: Option<Chip>) {
    let models = match chip {
        Some(chip) => catalog_adapter::filter_models(query, chip),
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
    use soundcore_models::{
        BatteryScale, BatteryState, FirmwareVersions, HostDevice, Layout, SoundMode,
    };
    use std::sync::atomic::AtomicU64;

    /// A saved-profile document some tests write: a profile with the given name
    /// and bands, validated exactly like the page saves it.
    fn profile(name: &str, bands: [i16; VISIBLE_EQ_BANDS]) -> EqualizerProfile {
        EqualizerProfile::new(name, bands).expect("the test profile is valid")
    }

    /// A unique file path under the system temp directory for a test document.
    fn temp_profiles_path(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openscl4-profiles-{}-{sequence}-{label}.json",
            std::process::id()
        ))
    }

    /// A unique temporary directory the file tests write into; it is removed
    /// when the guard drops, so a test leaves nothing behind.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = temp_profiles_path(label).with_extension("dir");
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("the test's temporary directory is created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// The saved-profile document inside this directory: the test directory
        /// stands in for the app-data directory.
        fn document(&self) -> PathBuf {
            self.0.join(PROFILES_FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Keeps the app's real saved-profile document as it was and puts it back
    /// when the guard drops, so a manual hardware run never loses the profiles
    /// a user already saved, even when an assertion fails midway.
    struct RestoreDocument {
        path: PathBuf,
        previous: Option<Vec<u8>>,
    }

    impl RestoreDocument {
        /// Reads the document at `path` and removes it, so the run starts from
        /// no profiles; dropping the guard restores the exact previous bytes.
        fn capture(path: &Path) -> Self {
            let previous = fs::read(path).ok();
            let _ = fs::remove_file(path);
            Self {
                path: path.to_path_buf(),
                previous,
            }
        }
    }

    impl Drop for RestoreDocument {
        fn drop(&mut self) {
            match &self.previous {
                Some(bytes) => {
                    let _ = fs::write(&self.path, bytes);
                }
                None => {
                    let _ = fs::remove_file(&self.path);
                }
            }
        }
    }

    #[test]
    fn the_profile_document_lives_in_the_app_data_directory() {
        // `%APPDATA%\OpenSCL4\equalizer-profiles.json`, as the spec names it.
        let app_data = Path::new(r"C:\Users\example\AppData\Roaming");
        assert_eq!(APP_DATA_DIR_NAME, "OpenSCL4");
        assert_eq!(PROFILES_FILE_NAME, "equalizer-profiles.json");
        assert_eq!(
            app_data_profiles_path(app_data),
            app_data.join(APP_DATA_DIR_NAME).join(PROFILES_FILE_NAME)
        );
        assert_eq!(
            temporary_profile_path(Path::new(r"C:\app\equalizer-profiles.json")),
            PathBuf::from(r"C:\app\equalizer-profiles.json.tmp")
        );
    }

    #[test]
    fn a_saved_profile_set_round_trips_through_the_document() {
        let directory = TempDir::new("roundtrip");
        let path = directory.document();
        let mut set = ProfileSet::new();
        set.add(profile("Warm", [30, -30, 60, 0, -60, 20, 0, -20]))
            .expect("the first profile is added");
        set.add(profile("Flat", [0; VISIBLE_EQ_BANDS]))
            .expect("the second profile is added");

        save_profiles(&path, &set).expect("the document is written");

        assert_eq!(
            fs::read_to_string(&path).expect("the document is readable"),
            set.to_json(),
            "the file is exactly the schema v1 document"
        );
        assert_eq!(load_profiles(&path), set, "load reads back the same set");
        assert!(
            !temporary_profile_path(&path).exists(),
            "the temporary sibling is renamed away"
        );
    }

    #[test]
    fn a_missing_or_malformed_profile_document_loads_empty() {
        let directory = TempDir::new("malformed");
        let path = directory.document();
        assert!(
            load_profiles(&path).is_empty(),
            "a missing document is an empty set, not a crash"
        );

        for text in [
            "not json at all",
            "{}",
            "[{\"name\":\"Warm\"}]",
            "[{\"name\":\"Warm\",\"bands\":[0,0,0,0,0,0,0]}]",
            "[{\"name\":\"\",\"bands\":[0,0,0,0,0,0,0,0]}]",
            "[{\"name\":\"Warm\",\"bands\":[999,0,0,0,0,0,0,0]}]",
            "[{\"name\":\"Warm\",\"bands\":[0,0,0,0,0,0,0,0]},\
             {\"name\":\"Warm\",\"bands\":[0,0,0,0,0,0,0,0]}]",
        ] {
            fs::write(&path, text).expect("the malformed document is written");
            assert!(
                load_profiles(&path).is_empty(),
                "{text:?} is not a saved profile set and must load as none"
            );
            assert_eq!(
                fs::read_to_string(&path).expect("the document is left in place"),
                text,
                "loading never rewrites the document"
            );
        }

        // Empty and whitespace-only files are a document with no profiles.
        for text in ["", "  \n\t"] {
            fs::write(&path, text).expect("the empty document is written");
            assert!(load_profiles(&path).is_empty());
        }
    }

    #[test]
    fn saving_replaces_the_previous_document_without_leaving_a_temporary() {
        let directory = TempDir::new("replace");
        // The app-data directory itself does not exist yet: saving creates it.
        let path = directory.path().join("OpenSCL4").join(PROFILES_FILE_NAME);
        assert!(!path.parent().expect("the document has a parent").exists());

        let mut first = ProfileSet::new();
        first
            .add(profile("Warm", [30, -30, 60, 0, -60, 20, 0, -20]))
            .expect("the profile is added");
        save_profiles(&path, &first).expect("the document is written");
        assert_eq!(load_profiles(&path), first);

        let mut second = ProfileSet::new();
        second
            .add(profile("Flat", [0; VISIBLE_EQ_BANDS]))
            .expect("the profile is added");
        save_profiles(&path, &second).expect("the document is replaced as a whole");
        assert_eq!(
            load_profiles(&path),
            second,
            "no part of the old document remains"
        );
        assert!(!temporary_profile_path(&path).exists());
    }

    #[test]
    fn a_save_that_cannot_write_the_document_reports_an_error() {
        let directory = TempDir::new("unwritable");
        // A file where the app-data directory should be makes the directory
        // creation fail; the save reports it instead of panicking.
        let blocker = directory.path().join("OpenSCL4");
        fs::write(&blocker, b"not a directory").expect("the blocking file is written");
        let path = blocker.join(PROFILES_FILE_NAME);

        let error = save_profiles(&path, &ProfileSet::new())
            .expect_err("a document under a file cannot be written");
        assert!(
            !error.to_string().is_empty(),
            "the save failure has a readable message"
        );
        assert!(
            !temporary_profile_path(&path).exists(),
            "a failed save does not leave a temporary file"
        );
        assert_eq!(
            fs::read(&blocker).expect("the blocking file is untouched"),
            b"not a directory".to_vec()
        );
    }

    #[test]
    fn a_profile_curve_is_drawn_in_the_thumbnail_box() {
        // The vendored ProfileCurve's viewbox is 600x100; a flat 0 dB curve sits
        // slightly above the middle because the range is -12.0…+13.4 dB.
        let flat = profile_curve(&[0; VISIBLE_EQ_BANDS]);
        assert!(flat.starts_with("M 37.5 52.4"), "{flat}");
        assert!(flat.ends_with("562.5 52.4"), "{flat}");
        // The range ends sit inside the 6% margins of the 100-unit box.
        let ends = profile_curve(&[EQ_MAX, EQ_MIN, 0, 0, 0, 0, 0, 0]);
        assert!(ends.starts_with("M 37.5 6.0"), "{ends}");
        // A one-band curve still draws a point rather than nothing.
        assert!(profile_curve(&[0; 1]).starts_with("M 300.0"));
    }

    #[test]
    fn saved_profile_rows_carry_the_name_curve_band_count_and_applied_flag() {
        let warm = [30, -30, 60, 0, -60, 20, 0, -20];
        let flat = [0i16; VISIBLE_EQ_BANDS];
        let mut set = ProfileSet::new();
        set.add(profile("Warm", warm)).expect("Warm is added");
        set.add(profile("Flat", flat)).expect("Flat is added");

        // The device is on the Warm curve: only Warm carries the Applied tag.
        let rows = profile_rows(&set, &warm);
        assert_eq!(rows.len(), 2, "insertion order is kept");
        assert_eq!(rows[0].name.as_str(), "Warm");
        assert_eq!(rows[0].bands, VISIBLE_EQ_BANDS as i32);
        assert!(rows[0].in_effect, "Warm's bands are the shown bands");
        assert!(!rows[0].curve.is_empty(), "the row carries its curve");
        assert!(rows[0].curve.starts_with("M "), "the curve is a path");
        assert!(!rows[0].export, "export checkboxes stay inert");
        assert_eq!(rows[1].name.as_str(), "Flat");
        assert!(!rows[1].in_effect, "Flat is not the shown curve");
        assert!(!rows[1].export);

        // Any other curve applies no profile, and no shown bands apply none.
        let rows = profile_rows(&set, &[1i16; VISIBLE_EQ_BANDS]);
        assert!(rows.iter().all(|row| !row.in_effect));
        let rows = profile_rows(&set, &[]);
        assert!(rows.iter().all(|row| !row.in_effect));
    }

    #[test]
    fn an_empty_profile_name_is_rejected_with_problem_one() {
        let set = ProfileSet::new();
        for name in ["", " ", "\t\r\n"] {
            assert_eq!(
                save_request(&set, name, false),
                SaveRequest::EmptyName,
                "{name:?} is not a profile name"
            );
            assert_eq!(save_request(&set, name, true), SaveRequest::EmptyName);
        }
    }

    #[test]
    fn a_duplicate_profile_name_asks_before_replacing() {
        let warm = [30, -30, 60, 0, -60, 20, 0, -20];
        let mut set = ProfileSet::new();
        set.add(profile("Warm", warm)).expect("Warm is added");

        // The dialog shows problem 2 and the existing name; names are trimmed.
        assert_eq!(
            save_request(&set, "Warm", false),
            SaveRequest::NeedsReplace {
                name: "Warm".into()
            }
        );
        assert_eq!(
            save_request(&set, "  Warm  ", false),
            SaveRequest::NeedsReplace {
                name: "Warm".into()
            }
        );
        // Asking to replace stores the bands; a new name is stored either way.
        assert_eq!(
            save_request(&set, "Warm", true),
            SaveRequest::Store {
                name: "Warm".into(),
                replace: true
            }
        );
        assert_eq!(
            save_request(&set, "Cool", false),
            SaveRequest::Store {
                name: "Cool".into(),
                replace: false
            }
        );
    }

    #[test]
    fn the_save_problem_values_match_the_vendored_dialog() {
        // `SaveProfileDialog` shows “Enter a profile name.” for value 1 and the
        // replace question for value 2; the controller names those values instead
        // of writing bare integers at the dialog boundary.
        assert_eq!(SaveProblem::None.value(), 0);
        assert_eq!(SaveProblem::EmptyName.value(), 1);
        assert_eq!(SaveProblem::NeedsReplace.value(), 2);
    }

    #[test]
    fn the_dialog_shows_the_name_problems_on_the_page() {
        let mut set = ProfileSet::new();
        set.add(profile("Warm", [0; VISIBLE_EQ_BANDS]))
            .expect("Warm is added");
        let page = RecordedSaveDialog::default();
        let bands = [30, -30, 60, 0, -60, 20, 0, -20];

        assert_eq!(
            answer_save_click(&page, &mut set, None, "  ", false, bands),
            SaveOutcome::EmptyName
        );
        assert_eq!(
            page.problem.get(),
            SaveProblem::EmptyName.value(),
            "the dialog shows its empty-name message"
        );
        assert!(!page.closed.get(), "a name problem keeps the dialog open");

        assert_eq!(
            answer_save_click(&page, &mut set, None, "Warm", false, bands),
            SaveOutcome::NeedsReplace {
                name: "Warm".into()
            }
        );
        assert_eq!(
            page.problem.get(),
            SaveProblem::NeedsReplace.value(),
            "the dialog asks before replacing saved values"
        );
        assert_eq!(page.existing.borrow().as_str(), "Warm");
        assert!(
            !page.closed.get(),
            "the replace question keeps the dialog open"
        );
        assert_eq!(set.len(), 1, "nothing is replaced before the answer");
    }

    #[test]
    fn a_written_profile_closes_the_dialog_and_clears_a_stale_banner() {
        let directory = TempDir::new("save-written");
        let path = directory.document();
        let mut set = ProfileSet::new();
        let page = RecordedSaveDialog::default();
        page.show_profile_error(PROFILE_WRITE_FAILURE_LABEL);

        let warm = [30, -30, 60, 0, -60, 20, 0, -20];
        let outcome = answer_save_click(&page, &mut set, Some(&path), "  Warm ", false, warm);

        assert_eq!(outcome, SaveOutcome::Saved);
        assert!(set.contains("Warm"), "the trimmed name is saved");
        assert_eq!(load_profiles(&path), set, "the document has the profile");
        assert!(page.closed.get(), "a written save closes the dialog");
        assert_eq!(page.problem.get(), SaveProblem::None.value());
        assert_eq!(
            page.error.borrow().as_str(),
            "",
            "a successful write clears the stale write-failure banner"
        );
    }

    #[test]
    fn a_save_that_cannot_write_the_document_rolls_back_and_reports() {
        let directory = TempDir::new("save-failure");
        // A file where the app-data directory belongs makes the write fail.
        let blocker = directory.path().join(APP_DATA_DIR_NAME);
        fs::write(&blocker, b"not a directory").expect("the blocking file is written");
        let path = blocker.join(PROFILES_FILE_NAME);
        let mut set = ProfileSet::new();
        let page = RecordedSaveDialog::default();

        let outcome = answer_save_click(
            &page,
            &mut set,
            Some(&path),
            "Warm",
            false,
            [30, -30, 60, 0, -60, 20, 0, -20],
        );

        assert_eq!(outcome, SaveOutcome::NotWritten, "the profile is not saved");
        assert!(
            set.is_empty(),
            "the unsaved profile is rolled back out of the set"
        );
        assert_eq!(
            page.problem.get(),
            SaveProblem::None.value(),
            "the dialog shows no name problem"
        );
        assert!(
            page.closed.get(),
            "the dialog closes so the page banner is visible"
        );
        assert_eq!(
            page.error.borrow().as_str(),
            PROFILE_WRITE_FAILURE_LABEL,
            "the page banner says the profiles could not be saved"
        );
        assert!(!path.exists(), "the failed save wrote no document");
        assert_eq!(
            fs::read(&blocker).expect("the blocking file is untouched"),
            b"not a directory".to_vec()
        );
    }

    #[test]
    fn a_missing_app_data_directory_is_not_reported_as_saved() {
        let mut set = ProfileSet::new();
        let page = RecordedSaveDialog::default();

        assert_eq!(
            answer_save_click(&page, &mut set, None, "Warm", false, [0; VISIBLE_EQ_BANDS]),
            SaveOutcome::NotWritten,
            "a profile that would not survive a restart is not reported as saved"
        );
        assert!(set.is_empty(), "nothing is kept in memory either");
        assert!(page.closed.get(), "the dialog closes on the failure");
        assert_eq!(page.error.borrow().as_str(), PROFILE_WRITE_FAILURE_LABEL);
    }

    #[test]
    fn a_delete_that_cannot_write_the_document_keeps_the_profile() {
        let directory = TempDir::new("delete-failure");
        let blocker = directory.path().join(APP_DATA_DIR_NAME);
        fs::write(&blocker, b"not a directory").expect("the blocking file is written");
        let path = blocker.join(PROFILES_FILE_NAME);
        let mut set = ProfileSet::new();
        set.add(profile("Warm", [30, -30, 60, 0, -60, 20, 0, -20]))
            .expect("Warm is added");

        assert_eq!(
            delete_profile(&mut set, Some(&path), "Warm"),
            DeleteOutcome::NotWritten,
            "a delete that cannot be persisted is not reported as done"
        );
        assert!(set.contains("Warm"), "the profile stays in the list");
    }

    #[test]
    fn applying_a_saved_profile_queues_its_curve_once() {
        let warm = [30, -30, 60, 0, -60, 20, 0, -20];
        let mut set = ProfileSet::new();
        set.add(profile("Warm", warm)).expect("Warm is added");
        let writer = RecordedWriter::default();

        assert_eq!(
            apply_saved_profile(&set, "Warm", &writer),
            ApplyOutcome::Queued
        );
        assert_eq!(
            writer.curves.borrow().len(),
            1,
            "a click queues exactly one custom-curve write, whose worker then \
             re-polls the state"
        );
        assert_eq!(
            writer.curves.borrow()[0],
            warm,
            "the profile's eight bands are written"
        );

        // The lookup trims the name like the profile model does.
        assert_eq!(
            apply_saved_profile(&set, "  Warm ", &writer),
            ApplyOutcome::Queued
        );
        assert_eq!(writer.curves.borrow().len(), 2);
    }

    #[test]
    fn an_unknown_profile_or_a_stopped_worker_queues_nothing() {
        let mut set = ProfileSet::new();
        set.add(profile("Warm", [30, -30, 60, 0, -60, 20, 0, -20]))
            .expect("Warm is added");
        let writer = RecordedWriter::default();

        assert_eq!(
            apply_saved_profile(&set, "Missing", &writer),
            ApplyOutcome::Unknown,
            "an unknown name is not a profile"
        );
        assert!(writer.curves.borrow().is_empty());

        writer.connected.set(false);
        assert_eq!(
            apply_saved_profile(&set, "Warm", &writer),
            ApplyOutcome::Disconnected,
            "a stopped worker cannot write"
        );
        assert!(writer.curves.borrow().is_empty());
    }

    #[test]
    fn deleting_a_saved_profile_persists_the_smaller_set() {
        let directory = TempDir::new("delete");
        let path = directory.document();
        let mut set = ProfileSet::new();
        set.add(profile("Warm", [30, -30, 60, 0, -60, 20, 0, -20]))
            .expect("Warm is added");
        set.add(profile("Flat", [0; VISIBLE_EQ_BANDS]))
            .expect("Flat is added");
        save_profiles(&path, &set).expect("the document is written");

        // The delete action removes the profile and persists the smaller set.
        let mut loaded = load_profiles(&path);
        assert_eq!(
            delete_profile(&mut loaded, Some(&path), "Warm"),
            DeleteOutcome::Removed,
            "the saved profile is removed and the smaller document is written"
        );
        assert_eq!(
            delete_profile(&mut loaded, Some(&path), "Missing"),
            DeleteOutcome::NotFound,
            "an unknown name only closes the dialog"
        );

        let reloaded = load_profiles(&path);
        assert_eq!(reloaded.len(), 1);
        assert!(reloaded.contains("Flat"), "the other profile stays");
        assert!(!reloaded.contains("Warm"), "the deleted profile is gone");
        assert_eq!(
            fs::read_to_string(&path).expect("the document is readable"),
            reloaded.to_json()
        );
    }

    /// A valid OpenSCL4 transfer file: one new `Warm` profile with eight values.
    const WARM_IMPORT: &str =
        r#"[{"name": "Warm", "volumeAdjustments": [3.0, -3.0, 6.0, 0.0, -6.0, 2.0, 0.0, -2.0]}]"#;
    /// The dB×10 bands [`WARM_IMPORT`] parses to.
    const WARM_IMPORT_BANDS: [i16; VISIBLE_EQ_BANDS] = [30, -30, 60, 0, -60, 20, 0, -20];
    /// A valid file that replaces the saved `Warm` and adds `Fresh`.
    const TWO_PROFILE_IMPORT: &str = r#"[
        {"name": "Warm", "volumeAdjustments": [3.0, -3.0, 6.0, 0.0, -6.0, 2.0, 0.0, -2.0]},
        {"name": "Fresh", "volumeAdjustments": [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}
    ]"#;

    /// One ImportDialog row as a test checks it: the values the Slint model gets.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ShownImportRow {
        name: String,
        bands: i32,
        status: i32,
        has_curve: bool,
    }

    /// An Import dialog that records what the import actions show, in place of
    /// the Slint `AppState`.
    #[derive(Default)]
    struct RecordedImportDialog {
        rows: RefCell<Vec<ShownImportRow>>,
        blocked: Cell<bool>,
        importing: Cell<bool>,
        notes: RefCell<Vec<(TransferNote, i32)>>,
        toasts: RefCell<Vec<(ToastKind, i32)>>,
        errors: RefCell<Vec<String>>,
        closes: Cell<u32>,
    }

    impl RecordedImportDialog {
        /// The note the Import panel last showed.
        fn last_note(&self) -> Option<(TransferNote, i32)> {
            self.notes.borrow().last().copied()
        }

        /// The toast the window last showed.
        fn last_toast(&self) -> Option<(ToastKind, i32)> {
            self.toasts.borrow().last().copied()
        }

        /// The dialog's rows as plain values.
        fn rows(&self) -> Vec<ShownImportRow> {
            self.rows.borrow().clone()
        }
    }

    impl ImportPage for RecordedImportDialog {
        fn show_preview(&self, rows: Vec<ImportItem>, blocked: bool) {
            *self.rows.borrow_mut() = rows
                .into_iter()
                .map(|row| ShownImportRow {
                    name: row.name.to_string(),
                    bands: row.bands,
                    status: row.status,
                    has_curve: !row.curve.is_empty(),
                })
                .collect();
            self.blocked.set(blocked);
            self.importing.set(true);
        }

        fn show_import_note(&self, note: TransferNote, count: i32) {
            self.notes.borrow_mut().push((note, count));
        }

        fn close_import_dialog(&self) {
            self.importing.set(false);
            self.closes.set(self.closes.get() + 1);
        }

        fn show_import_toast(&self, kind: ToastKind, count: i32) {
            self.toasts.borrow_mut().push((kind, count));
        }

        fn show_import_error(&self, label: &str) {
            self.errors.borrow_mut().push(label.to_owned());
        }
    }

    /// The import actions' machine seam in tests: the clipboard and the file
    /// picker serve fixed values, and `read` overrides the file read so a size or
    /// IO failure can be simulated without touching the disk.
    #[derive(Default)]
    struct RecordedImportIo {
        clipboard: RefCell<Option<String>>,
        picked: RefCell<Option<PathBuf>>,
        read: RefCell<Option<Result<String, ImportFileError>>>,
    }

    impl RecordedImportIo {
        /// An IO whose clipboard holds `text`.
        fn pasting(text: &str) -> Self {
            Self {
                clipboard: RefCell::new(Some(text.to_owned())),
                ..Self::default()
            }
        }

        /// An IO whose file dialog picks `path` and whose file read fails with
        /// `error`, so the file does not have to exist.
        fn failing_file(path: impl Into<PathBuf>, error: ImportFileError) -> Self {
            Self {
                picked: RefCell::new(Some(path.into())),
                read: RefCell::new(Some(Err(error))),
                ..Self::default()
            }
        }

        /// An IO whose file dialog picks `path`; the file is read for real.
        fn picking(path: impl Into<PathBuf>) -> Self {
            Self {
                picked: RefCell::new(Some(path.into())),
                ..Self::default()
            }
        }
    }

    impl ImportIo for RecordedImportIo {
        fn clipboard_text(&self) -> Option<String> {
            self.clipboard.borrow().clone()
        }

        fn pick_file(&self) -> Option<PathBuf> {
            self.picked.borrow().clone()
        }

        fn read_import_file(&self, path: &Path) -> Result<String, ImportFileError> {
            if let Some(result) = self.read.borrow().as_ref() {
                return result.clone();
            }
            read_import_file(path)
        }
    }

    #[test]
    fn an_empty_clipboard_shows_the_clipboard_empty_note() {
        for clipboard in [None, Some("  \n\t ".to_owned())] {
            let page = RecordedImportDialog::default();
            let mut import_text: Option<String> = None;
            import_from_clipboard(
                &page,
                &ProfileSet::new(),
                &mut import_text,
                &RecordedImportIo {
                    clipboard: RefCell::new(clipboard),
                    ..RecordedImportIo::default()
                },
            );

            assert!(!page.importing.get(), "no dialog opens");
            assert!(page.rows().is_empty(), "no rows are shown");
            assert_eq!(
                page.last_note(),
                Some((TransferNote::ClipboardEmpty, 0)),
                "the Import panel says the clipboard is empty"
            );
            assert_eq!(import_text, None, "no preview text is kept");
        }
    }

    #[test]
    fn pasting_a_profiles_file_opens_the_import_preview() {
        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_clipboard(
            &page,
            &ProfileSet::new(),
            &mut import_text,
            &RecordedImportIo::pasting(WARM_IMPORT),
        );

        assert!(page.importing.get(), "the dialog opens");
        assert!(!page.blocked.get(), "a valid file is not blocked");
        assert_eq!(
            page.rows(),
            vec![ShownImportRow {
                name: "Warm".to_owned(),
                bands: 8,
                status: 0,
                has_curve: true,
            }]
        );
        assert_eq!(
            page.last_note(),
            Some((TransferNote::None, 0)),
            "a preview open in the dialog has no panel note"
        );
        assert_eq!(
            import_text.as_deref(),
            Some(WARM_IMPORT),
            "the text is kept for eq-import to re-validate"
        );
    }

    #[test]
    fn a_preview_tags_new_replacing_wrong_count_and_out_of_range_rows() {
        let mut saved = ProfileSet::new();
        saved
            .add(profile("Warm", [0; VISIBLE_EQ_BANDS]))
            .expect("Warm is saved");
        let text = r#"[
            {"name": "Warm", "volumeAdjustments": [3.0, -3.0, 6.0, 0.0, -6.0, 2.0, 0.0, -2.0]},
            {"name": "Fresh", "volumeAdjustments": [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]},
            {"name": "Seven", "volumeAdjustments": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]},
            {"name": "Loud", "volumeAdjustments": [13.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}
        ]"#;
        let page = RecordedImportDialog::default();
        let mut import_text = None;
        preview_import_text(&page, &saved, &mut import_text, text);

        assert!(page.importing.get(), "the dialog opens");
        assert!(
            page.blocked.get(),
            "a wrong-count or out-of-range row blocks"
        );
        assert_eq!(
            page.rows(),
            vec![
                ShownImportRow {
                    name: "Warm".to_owned(),
                    bands: 8,
                    status: 1,
                    has_curve: true,
                },
                ShownImportRow {
                    name: "Fresh".to_owned(),
                    bands: 8,
                    status: 0,
                    has_curve: true,
                },
                ShownImportRow {
                    name: "Seven".to_owned(),
                    bands: 7,
                    status: 2,
                    has_curve: false,
                },
                ShownImportRow {
                    name: "Loud".to_owned(),
                    bands: 8,
                    status: 3,
                    has_curve: false,
                },
            ]
        );
    }

    #[test]
    fn text_that_is_not_a_profiles_file_shows_the_not_profiles_note() {
        for text in [
            "not json",
            "{}",
            "null",
            "[]",
            r#"[{"name": "", "volumeAdjustments": []}]"#,
            r#"[{"name": "Same", "volumeAdjustments": []}, {"name": "Same", "volumeAdjustments": []}]"#,
        ] {
            let page = RecordedImportDialog::default();
            let mut import_text: Option<String> = None;
            import_from_clipboard(
                &page,
                &ProfileSet::new(),
                &mut import_text,
                &RecordedImportIo::pasting(text),
            );

            assert!(!page.importing.get(), "{text:?} opens no dialog");
            assert!(
                page.rows().is_empty(),
                "{text:?} shows no rows in the dialog"
            );
            assert_eq!(
                page.last_note(),
                Some((TransferNote::NotProfiles, 0)),
                "{text:?} is not a profiles file"
            );
            assert_eq!(import_text, None, "{text:?} keeps no preview text");
        }
    }

    #[test]
    fn a_cancelled_file_dialog_does_nothing() {
        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_file(
            &page,
            &ProfileSet::new(),
            &mut import_text,
            &RecordedImportIo::default(),
        );

        assert!(!page.importing.get());
        assert!(page.notes.borrow().is_empty(), "no note is shown");
        assert_eq!(import_text, None);
    }

    #[test]
    fn a_large_import_file_shows_the_too_large_note() {
        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_file(
            &page,
            &ProfileSet::new(),
            &mut import_text,
            &RecordedImportIo::failing_file("C:\\profiles.json", ImportFileError::TooLarge),
        );

        assert!(!page.importing.get());
        assert!(page.rows().is_empty());
        assert_eq!(page.last_note(), Some((TransferNote::TooLarge, 0)));
        assert_eq!(import_text, None);
    }

    #[test]
    fn an_unreadable_import_file_shows_the_read_failed_note() {
        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_file(
            &page,
            &ProfileSet::new(),
            &mut import_text,
            &RecordedImportIo::failing_file("C:\\profiles.json", ImportFileError::ReadFailed),
        );

        assert!(!page.importing.get());
        assert_eq!(page.last_note(), Some((TransferNote::ReadFailed, 0)));
        assert_eq!(import_text, None);
    }

    #[test]
    fn a_picked_import_file_is_read_and_previewed() {
        let directory = TempDir::new("import-file");
        let path = directory.path().join("profiles.json");
        fs::write(&path, WARM_IMPORT).expect("the import file is written");

        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_file(
            &page,
            &ProfileSet::new(),
            &mut import_text,
            &RecordedImportIo::picking(path.clone()),
        );

        assert!(page.importing.get(), "the picked file is previewed");
        assert!(!page.blocked.get());
        assert_eq!(page.rows().len(), 1);
        assert_eq!(page.rows()[0].name, "Warm");
        assert_eq!(
            import_text.as_deref(),
            Some(WARM_IMPORT),
            "the file text is kept for eq-import"
        );
    }

    #[test]
    fn the_import_file_reader_checks_the_size_before_reading_it() {
        let directory = TempDir::new("import-reader");
        let small = directory.path().join("profiles.json");
        fs::write(&small, WARM_IMPORT).expect("the small file is written");
        assert_eq!(read_import_file(&small), Ok(WARM_IMPORT.to_owned()));

        // Exactly the one-mebibyte limit is read; one byte more is refused.
        let limit = directory.path().join("limit.json");
        let mut text = String::from("[]");
        text.push_str(&" ".repeat(MAX_IMPORT_BYTES as usize - text.len()));
        fs::write(&limit, &text).expect("the file at the limit is written");
        assert_eq!(read_import_file(&limit), Ok(text));
        let too_large = directory.path().join("large.json");
        let mut text = String::from("[]");
        text.push_str(&" ".repeat(MAX_IMPORT_BYTES as usize + 1 - text.len()));
        fs::write(&too_large, &text).expect("the oversized file is written");
        assert_eq!(read_import_file(&too_large), Err(ImportFileError::TooLarge));
    }

    #[test]
    fn the_import_file_reader_reports_an_unreadable_file() {
        let directory = TempDir::new("import-unreadable");
        // A missing file and a directory are both unreadable text.
        assert_eq!(
            read_import_file(&directory.path().join("missing.json")),
            Err(ImportFileError::ReadFailed)
        );
        assert_eq!(
            read_import_file(directory.path()),
            Err(ImportFileError::ReadFailed)
        );
    }

    #[test]
    fn answering_import_merges_replaces_and_persists_all_or_nothing() {
        let directory = TempDir::new("import-persist");
        let path = directory.document();
        let mut saved = ProfileSet::new();
        saved
            .add(profile("Warm", [0; VISIBLE_EQ_BANDS]))
            .expect("Warm is saved");
        save_profiles(&path, &saved).expect("the document is written");
        let mut profiles = load_profiles(&path);

        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_clipboard(
            &page,
            &profiles,
            &mut import_text,
            &RecordedImportIo::pasting(TWO_PROFILE_IMPORT),
        );
        let outcome = answer_import_click(&page, &mut profiles, Some(&path), &mut import_text);

        assert_eq!(outcome, ImportOutcome::Imported(2));
        assert!(!page.importing.get(), "the dialog closes after the import");
        assert_eq!(page.closes.get(), 1);
        assert_eq!(
            page.last_note(),
            Some((TransferNote::Imported, 2)),
            "the Import panel says how many profiles were imported"
        );
        assert_eq!(
            page.last_toast(),
            Some((ToastKind::Imported, 2)),
            "the toast says how many profiles were imported"
        );
        assert_eq!(import_text, None, "the preview text is consumed");
        assert_eq!(profiles.len(), 2, "Warm is replaced and Fresh is added");
        assert_eq!(
            profiles.get("Warm").expect("Warm is saved").bands,
            WARM_IMPORT_BANDS
        );
        assert_eq!(
            profiles.profiles()[0].name,
            "Warm",
            "Warm keeps its position"
        );
        assert_eq!(profiles.profiles()[1].name, "Fresh", "new profiles append");
        assert_eq!(
            load_profiles(&path),
            profiles,
            "the document is written with the merged set"
        );
    }

    #[test]
    fn a_blocked_import_writes_nothing_and_keeps_the_dialog_open() {
        let directory = TempDir::new("import-blocked");
        let path = directory.document();
        let mut saved = ProfileSet::new();
        saved
            .add(profile("Warm", [0; VISIBLE_EQ_BANDS]))
            .expect("Warm is saved");
        save_profiles(&path, &saved).expect("the document is written");
        let document_before = fs::read_to_string(&path).expect("the document is readable");
        let text = r#"[
            {"name": "Warm", "volumeAdjustments": [3.0, -3.0, 6.0, 0.0, -6.0, 2.0, 0.0, -2.0]},
            {"name": "Seven", "volumeAdjustments": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]},
            {"name": "Loud", "volumeAdjustments": [13.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}
        ]"#;
        let mut profiles = load_profiles(&path);
        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = None;
        import_from_clipboard(
            &page,
            &profiles,
            &mut import_text,
            &RecordedImportIo::pasting(text),
        );
        assert!(page.blocked.get(), "the wrong-count and range rows block");
        assert!(page.importing.get());

        let outcome = answer_import_click(&page, &mut profiles, Some(&path), &mut import_text);

        assert_eq!(outcome, ImportOutcome::Blocked);
        assert!(
            page.importing.get(),
            "the dialog stays open with its red note"
        );
        assert_eq!(page.closes.get(), 0, "the dialog is not closed");
        assert_eq!(
            page.notes.borrow().len(),
            1,
            "no note is added by the blocked click"
        );
        assert_eq!(profiles, saved, "no profile is merged");
        assert_eq!(
            fs::read_to_string(&path).expect("the document is readable"),
            document_before,
            "the document is untouched"
        );
        assert_eq!(
            import_text.as_deref(),
            Some(text),
            "the preview is kept for a later retry"
        );
    }

    #[test]
    fn importing_without_a_preview_does_nothing() {
        let page = RecordedImportDialog::default();
        let mut profiles = ProfileSet::new();
        let mut import_text: Option<String> = None;

        let outcome = answer_import_click(&page, &mut profiles, None, &mut import_text);

        assert_eq!(outcome, ImportOutcome::Nothing);
        assert!(profiles.is_empty());
        assert!(page.notes.borrow().is_empty());
        assert_eq!(page.closes.get(), 0);
    }

    #[test]
    fn an_import_that_cannot_write_the_document_is_rolled_back() {
        let directory = TempDir::new("import-write-failure");
        let blocker = directory.path().join(APP_DATA_DIR_NAME);
        fs::write(&blocker, b"not a directory").expect("the blocking file is written");
        let path = blocker.join(PROFILES_FILE_NAME);
        let mut profiles = ProfileSet::new();
        profiles
            .add(profile("Warm", [0; VISIBLE_EQ_BANDS]))
            .expect("Warm is saved");

        let page = RecordedImportDialog::default();
        let mut import_text: Option<String> = Some(WARM_IMPORT.to_owned());
        let outcome = answer_import_click(&page, &mut profiles, Some(&path), &mut import_text);

        assert_eq!(outcome, ImportOutcome::NotWritten);
        assert_eq!(profiles.len(), 1, "the imported profile is rolled back");
        assert!(!page.importing.get(), "the dialog closes");
        assert_eq!(
            page.errors.borrow().first().map(String::as_str),
            Some(PROFILE_WRITE_FAILURE_LABEL),
            "the page banner says the profiles could not be saved"
        );
        assert_eq!(
            page.last_note(),
            Some((TransferNote::None, 0)),
            "a failed write has no Import panel note"
        );
        assert_eq!(import_text, None);
    }

    #[test]
    fn only_the_editable_a3062_page_offers_saved_profiles() {
        let a3062 = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let a3062_state = soundcore_models::parse_state_frame(a3062, &captured_a3062_frame())
            .expect("the A3062 capture parses");
        let display = EqualizerDisplay::new(Some(a3062), a3062_state.equalizer.as_ref());
        assert!(
            display.editable,
            "the editable A3062 page saves, lists and applies profiles"
        );

        // The deferred A3949 has no page and therefore no saved profiles: no
        // save button, no list, no apply and no `03 87` write.
        let a3949 = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        let a3949_state = soundcore_models::parse_state_frame(a3949, &captured_a3949_frame())
            .expect("the A3949 capture parses");
        let display = EqualizerDisplay::new(Some(a3949), a3949_state.equalizer.as_ref());
        assert!(
            !display.editable,
            "the deferred A3949 page stays read-only and gets no profile panel"
        );
    }

    #[test]
    fn a_chosen_p20i_resolves_to_its_catalog_card_and_state_parser() {
        let model = selected_model("A3949").expect("A3949 is in the catalog");
        assert_eq!(model.code, "A3949");
        assert_eq!(model.name, "P20i / P25i / R50i");
        assert!(!model.is_speaker);
        assert_ne!(model.art, slint::Image::default());
        let protocol = model.protocol.expect("A3949 has a state parser");
        assert_eq!(protocol.code, "A3949");
        assert_eq!(protocol.layout, Layout::TwsCmm2);
        assert_eq!(protocol.battery_scale, BatteryScale::Five);
    }

    #[test]
    fn a_supported_single_battery_headphone_still_resolves() {
        let model = selected_model("A3035").expect("A3035 is in the catalog");
        assert_eq!(model.name, "Space One");
        let protocol = model.protocol.expect("A3035 has a state model");
        assert_eq!(protocol.layout, Layout::SingleCmm2);
        assert_eq!(protocol.battery_scale, BatteryScale::Five);
    }

    #[test]
    fn a_speaker_is_marked_as_a_speaker() {
        let model = selected_model("A3116").expect("A3116 is in the catalog");
        assert_eq!(model.name, "Motion+");
        assert!(model.is_speaker);
        assert_ne!(model.art, slint::Image::default());
    }

    #[test]
    fn a_catalog_model_the_core_cannot_read_has_no_parser() {
        let model = selected_model("A3909").expect("A3909 is in the catalog");
        assert_eq!(model.protocol, None);
    }

    #[test]
    fn an_unknown_code_selects_nothing() {
        assert!(selected_model("").is_none());
        assert!(selected_model("A0000").is_none());
    }

    #[test]
    fn a_paired_option_carries_the_address_in_the_ui_mac_form() {
        let candidate = Candidate {
            address: 0x0000_0200_0000_0001,
            name: "soundcore P20i".into(),
            suggested: true,
        };
        let option = paired_option(&candidate);
        assert_eq!(option.name.as_str(), "soundcore P20i");
        assert!(option.suggested);
        assert_eq!(option.mac.as_str(), "02:00:00:00:00:01");
    }

    #[test]
    fn a_paired_option_mac_parses_back_to_its_numeric_address() {
        for address in [
            0x0000_0000_0000_0000,
            0x0000_0200_0000_0001,
            0x0000_FFFF_FFFF_FFFF,
        ] {
            let option = paired_option(&Candidate {
                address,
                name: String::new(),
                suggested: false,
            });
            assert_eq!(option.mac.as_str(), format_mac(address));
            assert_eq!(parse_mac(option.mac.as_str()), Some(address));
        }
    }

    fn state_of(left: u8, right: u8, scale: BatteryScale) -> DeviceState {
        DeviceState {
            host_device: HostDevice::Left,
            tws_connected: true,
            battery: Some(BatteryState {
                left,
                right,
                left_charging: false,
                right_charging: false,
                scale,
            }),
            single_battery: None,
            sound_mode: None,
            equalizer: None,
            settings: None,
            firmware: FirmwareVersions {
                left: None,
                right: None,
            },
            serial: None,
        }
    }

    fn single_state_of(value: u8) -> DeviceState {
        DeviceState {
            host_device: HostDevice::Left,
            tws_connected: false,
            battery: None,
            single_battery: Some(soundcore_models::SingleBatteryState {
                value,
                scale: BatteryScale::Ten,
                charging: None,
                firmware: Some("04.39".into()),
            }),
            sound_mode: None,
            equalizer: None,
            settings: None,
            firmware: FirmwareVersions {
                left: None,
                right: None,
            },
            serial: None,
        }
    }

    fn assert_ratio(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{actual} is not {expected}"
        );
    }

    #[test]
    fn a_five_step_battery_is_shown_as_a_percentage() {
        let tiles = batteries(&state_of(5, 4, BatteryScale::Five));
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].label, 0);
        assert_eq!(tiles[0].value.as_str(), "100%");
        assert!(tiles[0].known);
        assert_ratio(tiles[0].progress, 1.0);
        assert_eq!(tiles[1].label, 1);
        assert_eq!(tiles[1].value.as_str(), "80%");
        assert_ratio(tiles[1].progress, 0.8);
    }

    #[test]
    fn a_ten_step_battery_is_shown_as_a_percentage() {
        let tiles = batteries(&state_of(7, 10, BatteryScale::Ten));
        assert_eq!(tiles[0].value.as_str(), "70%");
        assert_ratio(tiles[0].progress, 0.7);
        assert_eq!(tiles[1].value.as_str(), "100%");
    }

    #[test]
    fn a_single_battery_state_is_one_whole_device_tile() {
        // label 3 is the vendored UI's "whole device" battery, rendered as
        // "Battery"; a raw 5 of 10 is 50 %.
        let tiles = batteries(&single_state_of(5));
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].label, 3);
        assert_eq!(tiles[0].value.as_str(), "50%");
        assert!(tiles[0].known);
        assert_ratio(tiles[0].progress, 0.5);
    }

    #[test]
    fn a_single_battery_state_has_no_left_or_right_tiles() {
        let tiles = batteries(&single_state_of(2));
        assert_eq!(tiles.len(), 1);
        assert!(tiles.iter().all(|tile| tile.label == 3));
        assert_eq!(tiles[0].value.as_str(), "20%");
    }

    #[test]
    fn a_hundred_step_battery_is_its_own_percentage() {
        let tiles = batteries(&state_of(87, 100, BatteryScale::Hundred));
        assert_eq!(tiles[0].value.as_str(), "87%");
        assert_ratio(tiles[0].progress, 0.87);
        assert_eq!(tiles[1].value.as_str(), "100%");
        assert_ratio(tiles[1].progress, 1.0);
    }

    #[test]
    fn a_case_tile_keeps_the_case_label() {
        let tile = battery_tile(2, 5, 5);
        assert_eq!(tile.label, 2);
        assert_eq!(tile.value.as_str(), "100%");
        assert!(tile.known);
    }

    #[test]
    fn a_battery_above_its_maximum_is_capped() {
        // The parser rejects out-of-range values; the tile must still be sane
        // if another layout ever hands one in.
        let tile = battery_tile(0, 7, 5);
        assert_eq!(tile.value.as_str(), "100%");
        assert_ratio(tile.progress, 1.0);
    }

    fn readable(text: &str) -> String {
        assert!(!text.is_empty(), "failure has no text");
        for token in text.split_whitespace() {
            let token = token.trim_matches(|character: char| !character.is_ascii_alphanumeric());
            assert_eq!(parse_mac(token), None, "a MAC leaked into {text:?}");
        }
        text.to_lowercase()
    }

    #[test]
    fn a_missing_device_is_a_not_found_failure() {
        let failure = ConnectFailure::from_bluetooth(BluetoothError::NotFound);
        assert_eq!(failure.kind(), FailureKind::NotFound);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("not found"));
    }

    #[test]
    fn a_page_timeout_is_a_timed_out_failure() {
        let failure = ConnectFailure::from_bluetooth(BluetoothError::Timeout);
        assert_eq!(failure.kind(), FailureKind::TimedOut);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("did not answer"));
    }

    #[test]
    fn bluetooth_off_switches_the_connection_state_not_the_failure_kind() {
        let failure = ConnectFailure::from_bluetooth(BluetoothError::BluetoothOff);
        assert_eq!(failure.kind(), FailureKind::None);
        assert_eq!(failure.connection(), Connection::BluetoothOff);
        assert!(readable(&failure.text()).contains("bluetooth is off"));
    }

    #[test]
    fn a_rejected_link_key_tells_the_user_to_pair_in_windows() {
        let failure = ConnectFailure::from_bluetooth(BluetoothError::NotPaired);
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        let text = readable(&failure.text());
        assert!(text.contains("not paired"));
        assert!(text.contains("windows"));
    }

    #[test]
    fn a_refused_connection_is_explained() {
        let failure = ConnectFailure::from_bluetooth(BluetoothError::ConnectionFailed);
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("bluetooth connection"));
    }

    #[test]
    fn a_device_without_the_soundcore_service_is_explained() {
        let failure = ConnectFailure::NoService;
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("soundcore"));
    }

    #[test]
    fn a_model_the_core_cannot_read_is_explained() {
        let failure = ConnectFailure::NoParser;
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("battery"));
    }

    #[test]
    fn an_unsupported_battery_layout_is_explained() {
        let failure = ConnectFailure::from_session(SessionError::UnsupportedLayout);
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("battery"));
    }

    #[test]
    fn a_session_timeout_is_a_timed_out_failure() {
        let failure = ConnectFailure::from_session(SessionError::Timeout);
        assert_eq!(failure.kind(), FailureKind::TimedOut);
        assert!(readable(&failure.text()).contains("did not answer"));
    }

    #[test]
    fn a_malformed_answer_is_explained() {
        let failure = ConnectFailure::from_session(SessionError::Protocol(
            soundcore_session::ProtocolError::State(soundcore_models::StateError::NotAStateFrame),
        ));
        assert_eq!(failure.kind(), FailureKind::Other);
        assert!(readable(&failure.text()).contains("message"));
    }

    #[test]
    fn a_transport_error_keeps_its_readable_text() {
        let failure = ConnectFailure::from_session(SessionError::Io(std::io::Error::other(
            "the socket closed",
        )));
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.text(), "the socket closed");

        let failure =
            ConnectFailure::from_bluetooth(BluetoothError::Other("Winsock error 7".into()));
        assert_eq!(failure.text(), "Winsock error 7");
    }

    const SPP: u128 = 0x0000_1101_0000_1000_8000_0080_5F9B_34FB;

    fn spp() -> GUID {
        GUID::from_u128(SPP)
    }

    fn vendor(code: &str) -> GUID {
        vendor_service(code).expect("the code is hexadecimal")
    }

    #[test]
    fn the_models_vendor_service_marks_the_suggested_device() {
        let services = [vendor("A3949"), spp()];
        assert!(is_suggested(
            &services,
            "soundcore P20i",
            "A3949",
            "P20i / P25i / R50i"
        ));
    }

    #[test]
    fn another_models_vendor_service_is_not_suggested() {
        let services = [vendor("A3954"), spp()];
        assert!(!is_suggested(
            &services,
            "soundcore Liberty 4 Pro",
            "A3949",
            "P20i / P25i / R50i"
        ));
    }

    #[test]
    fn a_name_match_suggests_a_model_that_has_no_vendor_service() {
        let services = [spp()];
        assert!(is_suggested(
            &services,
            "soundcore P20i",
            "A3949",
            "P20i / P25i / R50i"
        ));
        // Case and the "soundcore" prefix do not hide the product name.
        assert!(is_suggested(
            &services,
            "Soundcore Liberty 4 Pro",
            "A3954",
            "Liberty 4 Pro"
        ));
    }

    #[test]
    fn the_code_in_the_windows_name_also_suggests_the_model() {
        assert!(is_suggested(
            &[],
            "Soundcore A3949",
            "A3949",
            "P20i / P25i / R50i"
        ));
    }

    #[test]
    fn a_different_name_without_a_vendor_service_is_not_suggested() {
        assert!(!is_suggested(
            &[spp()],
            "soundcore Q30",
            "A3949",
            "P20i / P25i / R50i"
        ));
    }

    fn paired(address: u64, name: &str, services: Vec<GUID>) -> PairedServices {
        PairedServices {
            address,
            name: name.into(),
            services,
        }
    }

    fn candidate_addresses(found: &[Candidate]) -> Vec<u64> {
        found.iter().map(|candidate| candidate.address).collect()
    }

    #[test]
    fn candidates_list_spp_devices_and_drop_non_soundcore_ones() {
        let paired = [
            paired(1, "soundcore P20i", vec![vendor("A3949"), spp()]),
            paired(2, "Pixel 8 Pro", vec![GUID::from_u128(1)]),
            paired(3, "soundcore Liberty 4 Pro", vec![vendor("A3954"), spp()]),
        ];
        let found = candidates(&paired, Some(("A3949", "P20i / P25i / R50i")));
        assert_eq!(candidate_addresses(&found), [1, 3]);
        assert!(found[0].suggested);
        assert!(!found[1].suggested);
    }

    #[test]
    fn without_a_chosen_model_only_spp_devices_are_listed() {
        let paired = [
            paired(1, "soundcore P20i", vec![spp()]),
            paired(2, "Redmi TV Soundbar", vec![GUID::from_u128(1)]),
        ];
        let found = candidates(&paired, None);
        assert_eq!(candidate_addresses(&found), [1]);
        assert!(found.iter().all(|candidate| !candidate.suggested));
    }

    fn card(mac: &str, name: &str, current: bool) -> DeviceCard {
        DeviceCard {
            mac: mac.into(),
            name: name.into(),
            code: "A3949".into(),
            art: slint::Image::default(),
            current,
            demo: false,
        }
    }

    #[test]
    fn a_paired_card_replaces_the_saved_card_of_the_same_mac() {
        let cards = [card("AA", "old name", false), card("BB", "other", false)];
        let updated = with_current_card(&cards, card("AA", "soundcore P20i", true));
        assert_eq!(updated.len(), 2);
        assert_eq!(updated[0].name.as_str(), "soundcore P20i");
        assert!(updated[0].current);
        assert!(!updated[1].current);
    }

    #[test]
    fn a_new_card_is_added_and_becomes_the_only_current_one() {
        let cards = [card("AA", "one", true)];
        let updated = with_current_card(&cards, card("BB", "two", true));
        assert_eq!(updated.len(), 2);
        assert!(!updated[0].current);
        assert!(updated[1].current);
    }

    /// A hex frame field of a real, masked capture the core keeps in
    /// `core/data`, read from the JSON without a JSON parser.
    fn captured_hex_field(json: &str, key: &str) -> Vec<u8> {
        let anchor = format!("\"{key}\"");
        let at = json
            .find(&anchor)
            .unwrap_or_else(|| panic!("the capture has no {key:?} field"));
        let after_key = &json[at..];
        let colon = after_key.find(':').expect("the key has a value");
        let after_colon = &after_key[colon..];
        let open = after_colon.find('"').expect("the frame is quoted");
        let quoted = &after_colon[open + 1..];
        let end = quoted.find('"').expect("the frame has a closing quote");
        quoted[..end]
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("the capture stores hex bytes"))
            .collect()
    }

    /// The `frame_hex` of a real, masked capture: the frame it is about.
    fn captured_frame(json: &str) -> Vec<u8> {
        captured_hex_field(json, "frame_hex")
    }

    /// The real A3062 equalizer capture of this ticket: the state found on
    /// connect (Soundcore Signature), the custom write frame and the `01 01`
    /// answers before and after it.
    fn custom_eq_capture() -> &'static str {
        include_str!("../../core/data/a3062-eq-custom.json")
    }

    /// The `01 01` answer found on connect in the custom-capture fixture: the
    /// Soundcore Signature preset.
    fn captured_before_custom_eq_frame() -> Vec<u8> {
        captured_hex_field(custom_eq_capture(), "before_frame_hex")
    }

    /// The exact custom `03 87` frame the real device accepted from that state:
    /// the curve +3.0, −3.0, +6.0, 0, −6.0, +2.0, 0, −2.0 dB.
    fn captured_custom_eq_write_frame() -> Vec<u8> {
        captured_frame(custom_eq_capture())
    }

    /// The `01 01` answer after that custom write: preset id `0xFEFE` and the
    /// written curve.
    fn captured_custom_eq_frame() -> Vec<u8> {
        captured_hex_field(custom_eq_capture(), "custom_frame_hex")
    }

    /// The parsed equalizer of a capture frame.
    fn captured_equalizer(frame: &[u8]) -> EqualizerState {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        soundcore_models::parse_state_frame(model, frame)
            .expect("the real capture parses")
            .equalizer
            .expect("the real capture reports an equalizer")
    }

    /// The parsed custom equalizer of the committed real A3062 custom capture:
    /// curve +3.0, −3.0, +6.0, 0, −6.0, +2.0, 0, −2.0 dB.
    fn custom_equalizer_state() -> EqualizerState {
        captured_equalizer(&captured_custom_eq_frame())
    }

    fn captured_a3949_frame() -> Vec<u8> {
        captured_frame(include_str!("../../core/data/a3949-state.json"))
    }

    fn captured_a3062_frame() -> Vec<u8> {
        captured_frame(include_str!("../../core/data/a3062-state.json"))
    }

    /// The parsed state of the committed real A3062 capture.
    fn a3062_state() -> DeviceState {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        soundcore_models::parse_state_frame(model, &captured_a3062_frame())
            .expect("the real capture parses")
    }

    /// The parsed state of the committed real A3949 capture.
    fn a3949_state() -> DeviceState {
        let model = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        soundcore_models::parse_state_frame(model, &captured_a3949_frame())
            .expect("the real capture parses")
    }

    /// The measured A3062 settings block of the committed capture.
    fn measured_settings() -> A3062Settings {
        a3062_state().settings.expect("the A3062 reports settings")
    }

    /// The measured A3062 `01 01` frame with body bytes replaced and the
    /// checksum recomputed, for a device answer after a setting changed. The
    /// offsets are `01 01` body offsets, as the core parses them.
    fn a3062_frame_with(changes: &[(usize, u8)]) -> Vec<u8> {
        let mut frame = captured_a3062_frame();
        for (offset, value) in changes {
            frame[STATE_FRAME_HEADER_LEN + offset] = *value;
        }
        let checksum_at = frame.len() - 1;
        frame[checksum_at] = soundcore_protocol::checksum(&frame[..checksum_at]);
        frame
    }

    /// The committed capture with a valid firmware string (body 2..7, `04.39`),
    /// which the sanitized capture zeroes.
    fn a3062_state_with_firmware() -> DeviceState {
        let frame = a3062_frame_with(&[(2, b'0'), (3, b'4'), (4, b'.'), (5, b'3'), (6, b'9')]);
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        soundcore_models::parse_state_frame(model, &frame).expect("the patched capture parses")
    }

    fn panel_kinds(panels: &[SettingsPanel]) -> Vec<PanelKind> {
        panels.iter().map(|panel| panel.kind).collect()
    }

    fn row_ids(panel: &SettingsPanel) -> Vec<String> {
        panel.rows.iter().map(|row| row.id.to_string()).collect()
    }

    fn row_kinds(panel: &SettingsPanel) -> Vec<RowKind> {
        panel.rows.iter().map(|row| row.kind).collect()
    }

    fn select_options(panel: &SettingsPanel, index: usize) -> Vec<String> {
        let row: SettingRow = panel.rows.iter().nth(index).expect("the row exists");
        row.options
            .iter()
            .map(|option| option.to_string())
            .collect()
    }

    #[test]
    fn the_a3062_settings_block_adds_the_measured_pages() {
        let state = a3062_state();
        assert_eq!(state.settings, Some(measured_settings()));
        assert_eq!(
            device_pages(state.settings.is_some()),
            vec![
                Page::Controls,
                Page::Comfort,
                Page::Power,
                Page::Connections,
                Page::About
            ]
        );
        assert_eq!(
            sound_pages(
                state.sound_mode,
                state.equalizer.is_some(),
                state.settings.is_some()
            ),
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
    }

    #[test]
    fn a_state_without_the_settings_block_has_no_pages_or_panels() {
        // The A3949 is the deferred model: its capture parses, but it has no
        // settings block, so no page, no panel and no writable row exists.
        let state = a3949_state();
        assert_eq!(state.settings, None);
        assert!(device_pages(state.settings.is_some()).is_empty());
        assert!(
            sound_pages(
                state.sound_mode,
                state.equalizer.is_some(),
                state.settings.is_some()
            )
            .is_empty()
        );
        for page in [
            Page::Controls,
            Page::Comfort,
            Page::Power,
            Page::Connections,
            Page::About,
            Page::Effects,
        ] {
            assert!(
                panels_for_page(page, &state).is_empty(),
                "{page:?} must have no panels"
            );
        }
    }

    #[test]
    fn the_controls_page_has_the_presses_and_cycle_panels() {
        let state = a3062_state();
        let panels = panels_for_page(Page::Controls, &state);
        assert_eq!(
            panel_kinds(&panels),
            vec![PanelKind::Presses, PanelKind::Cycle]
        );

        let presses = &panels[0];
        assert_eq!(row_ids(presses), ["singlePress", "doublePress"]);
        assert_eq!(row_kinds(presses), vec![RowKind::Info, RowKind::Select]);
        let single: SettingRow = presses.rows.iter().next().expect("the single press row");
        assert_eq!(single.text.as_str(), "Ambient sound mode");
        assert!(single.enabled);
        assert_eq!(select_options(presses, 1), ["None", "Bass Up"]);
        let double: SettingRow = presses.rows.iter().nth(1).expect("the double press row");
        assert_eq!(double.index, 0, "the measured double press does nothing");

        let cycle = &panels[1];
        assert_eq!(
            row_ids(cycle),
            [
                "noiseCancelingModeInCycle",
                "transparencyModeInCycle",
                "normalModeInCycle"
            ]
        );
        assert_eq!(
            row_kinds(cycle),
            vec![RowKind::Toggle, RowKind::Toggle, RowKind::Toggle]
        );
        let checked: Vec<bool> = cycle.rows.iter().map(|row| row.checked).collect();
        assert_eq!(checked, [true, true, false], "the measured cycle bits");
    }

    #[test]
    fn the_comfort_page_has_the_side_tone_and_volume_panels() {
        let state = a3062_state();
        let panels = panels_for_page(Page::Comfort, &state);
        assert_eq!(
            panel_kinds(&panels),
            vec![PanelKind::Plain, PanelKind::Volume]
        );
        assert_eq!(row_ids(&panels[0]), ["sideTone"]);
        assert_eq!(row_kinds(&panels[0]), vec![RowKind::Toggle]);
        let side: SettingRow = panels[0].rows.iter().next().expect("the side tone row");
        assert!(side.checked, "the measured side tone is on");

        let volume = &panels[1];
        assert_eq!(
            row_ids(volume),
            [
                "limitHighVolume",
                "limitHighVolumeDbLimit",
                "limitHighVolumeRefreshRate"
            ]
        );
        assert_eq!(
            row_kinds(volume),
            vec![RowKind::Toggle, RowKind::Range, RowKind::Select]
        );
        let rows: Vec<SettingRow> = volume.rows.iter().collect();
        assert!(!rows[0].checked, "the measured limit high volume is off");
        assert_eq!(
            (
                rows[1].minimum,
                rows[1].maximum,
                rows[1].step,
                rows[1].value
            ),
            (75, 100, 5, 90),
            "the measured dB range and the reported limit"
        );
        assert_eq!(
            select_options(volume, 2),
            ["Real time", "10 seconds", "1 minute"]
        );
        assert_eq!(rows[2].index, 0, "the measured refresh rate is real time");
    }

    #[test]
    fn the_power_page_has_the_auto_power_off_and_prompts() {
        let state = a3062_state();
        let panels = panels_for_page(Page::Power, &state);
        assert_eq!(panel_kinds(&panels), vec![PanelKind::Plain]);
        assert_eq!(
            row_ids(&panels[0]),
            ["autoPowerOff", "lowBatteryPrompt", "voicePrompt"]
        );
        assert_eq!(
            row_kinds(&panels[0]),
            vec![RowKind::Select, RowKind::Toggle, RowKind::Toggle]
        );
        assert_eq!(
            select_options(&panels[0], 0),
            [
                "Disabled",
                "30 minutes",
                "60 minutes",
                "90 minutes",
                "120 minutes"
            ]
        );
        let rows: Vec<SettingRow> = panels[0].rows.iter().collect();
        assert_eq!(
            rows[0].index, 2,
            "the measured auto power off is on at 60 minutes"
        );
        assert!(rows[1].checked, "the measured low battery prompt is on");
        assert!(rows[2].checked, "the measured voice prompt is on");
    }

    #[test]
    fn the_connections_page_has_the_dual_connections_toggle_only() {
        // The remembered-device list and its forget action belong to the
        // follow-up dual-connections slice, so this page has one panel and one
        // row.
        let state = a3062_state();
        let panels = panels_for_page(Page::Connections, &state);
        assert_eq!(panel_kinds(&panels), vec![PanelKind::Plain]);
        assert_eq!(panels.len(), 1);
        assert_eq!(row_ids(&panels[0]), ["dualConnections"]);
        let row: SettingRow = panels[0].rows.iter().next().expect("the toggle row");
        assert!(row.checked, "the measured dual connections are on");
    }

    #[test]
    fn the_effects_page_has_the_ldac_and_dolby_toggles() {
        let state = a3062_state();
        let panels = panels_for_page(Page::Effects, &state);
        assert_eq!(panel_kinds(&panels), vec![PanelKind::Plain]);
        assert_eq!(row_ids(&panels[0]), ["ldac", "dolbyAudio"]);
        assert_eq!(
            row_kinds(&panels[0]),
            vec![RowKind::Toggle, RowKind::Toggle]
        );
        let checked: Vec<bool> = panels[0].rows.iter().map(|row| row.checked).collect();
        assert_eq!(
            checked,
            [false, false],
            "the measured LDAC and Dolby are off"
        );
    }

    #[test]
    fn the_about_page_has_the_charging_battery_and_firmware_information() {
        let state = a3062_state_with_firmware();
        let panels = panels_for_page(Page::About, &state);
        assert_eq!(panel_kinds(&panels), vec![PanelKind::Info]);
        assert_eq!(
            row_ids(&panels[0]),
            ["isCharging", "batteryLevel", "firmwareVersion"]
        );
        let rows: Vec<SettingRow> = panels[0].rows.iter().collect();
        assert!(rows.iter().all(|row| row.kind == RowKind::Info));
        assert_eq!(
            rows[0].text.as_str(),
            "Not charging",
            "the measured capture is not charging"
        );
        assert_eq!(
            rows[1].text.as_str(),
            "20%",
            "the measured ten-step battery reads two"
        );
        assert_eq!(rows[2].text.as_str(), "04.39", "the firmware of body 2..7");
        assert!(
            !row_ids(&panels[0]).iter().any(|id| id == "serialNumber"),
            "the serial is never shown"
        );
    }

    #[test]
    fn a_toggle_click_names_exactly_the_measured_command() {
        let settings = measured_settings();
        let command = |id, on| toggle_command(id, on, &settings);
        assert_eq!(
            command("sideTone", false),
            Some(SettingCommand::SideTone(false))
        );
        assert_eq!(
            command("lowBatteryPrompt", false),
            Some(SettingCommand::LowBatteryPrompt(false))
        );
        assert_eq!(
            command("voicePrompt", false),
            Some(SettingCommand::VoicePrompt(false))
        );
        assert_eq!(command("ldac", true), Some(SettingCommand::Ldac(true)));
        assert_eq!(
            command("dolbyAudio", true),
            Some(SettingCommand::DolbyAudio(true))
        );
        assert_eq!(
            command("dualConnections", false),
            Some(SettingCommand::DualConnections(false))
        );
        assert_eq!(
            command("limitHighVolume", true),
            Some(SettingCommand::LimitHighVolume {
                enabled: true,
                db_limit: settings.limit_high_volume.db_limit,
            })
        );
        // A cycle toggle changes its own bit and keeps the other two.
        assert_eq!(
            command("normalModeInCycle", true),
            Some(SettingCommand::AmbientSoundModeCycle(
                AmbientSoundModeCycle {
                    noise_canceling: true,
                    transparency: true,
                    normal: true,
                }
            ))
        );
        assert_eq!(
            command("noiseCancelingModeInCycle", false),
            Some(SettingCommand::AmbientSoundModeCycle(
                AmbientSoundModeCycle {
                    noise_canceling: false,
                    transparency: true,
                    normal: false,
                }
            ))
        );
        assert_eq!(
            command("transparencyModeInCycle", false),
            Some(SettingCommand::AmbientSoundModeCycle(
                AmbientSoundModeCycle {
                    noise_canceling: true,
                    transparency: false,
                    normal: false,
                }
            ))
        );
        // The dispatched command builds the recorded measured frame.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        assert_eq!(
            model.setting_frame(command("sideTone", false).expect("a side tone command")),
            Some(hex("08 EE 00 00 00 01 84 0B 00 00 86"))
        );
        assert_eq!(
            model.setting_frame(command("ldac", true).expect("an LDAC command")),
            Some(hex("08 EE 00 00 00 01 FF 0B 00 01 02"))
        );
        // Read-only, other-page and unknown ids never write.
        for id in [
            "singlePress",
            "ambientSoundMode",
            "limitHighVolumeDbLimit",
            "autoPowerOff",
            "doublePress",
            "spatialAudio",
            "",
        ] {
            assert_eq!(command(id, true), None, "{id:?}");
        }
    }

    #[test]
    fn an_option_click_maps_to_the_measured_choice_commands() {
        let settings = measured_settings();
        let command = |id, index| option_command(id, index, &settings);
        // Zero disables auto power off and keeps the reported duration;
        // 1..=4 enable the duration at that page index.
        assert_eq!(
            command("autoPowerOff", 0),
            Some(SettingCommand::AutoPowerOff(AutoPowerOff {
                enabled: false,
                duration: AutoPowerOffDuration::SixtyMinutes,
            }))
        );
        for (index, duration) in [
            (1, AutoPowerOffDuration::ThirtyMinutes),
            (2, AutoPowerOffDuration::SixtyMinutes),
            (3, AutoPowerOffDuration::NinetyMinutes),
            (4, AutoPowerOffDuration::OneHundredTwentyMinutes),
        ] {
            assert_eq!(
                command("autoPowerOff", index),
                Some(SettingCommand::AutoPowerOff(AutoPowerOff {
                    enabled: true,
                    duration,
                })),
                "{index}"
            );
        }
        for index in [-1, 5, 99] {
            assert_eq!(command("autoPowerOff", index), None, "{index}");
        }
        for (index, rate) in [
            (0, DecibelReadingRefreshRate::RealTime),
            (1, DecibelReadingRefreshRate::TenSeconds),
            (2, DecibelReadingRefreshRate::OneMinute),
        ] {
            assert_eq!(
                command("limitHighVolumeRefreshRate", index),
                Some(SettingCommand::LimitHighVolumeRefreshRate(rate)),
                "{index}"
            );
        }
        for index in [-1, 3] {
            assert_eq!(
                command("limitHighVolumeRefreshRate", index),
                None,
                "{index}"
            );
        }
        assert_eq!(
            command("doublePress", 0),
            Some(SettingCommand::ButtonDoublePress(None))
        );
        assert_eq!(
            command("doublePress", 1),
            Some(SettingCommand::ButtonDoublePress(Some(
                ButtonAction::BassUp
            )))
        );
        for index in [-1, 2] {
            assert_eq!(command("doublePress", index), None, "{index}");
        }
        // The dispatched choices build the recorded measured frames.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        assert_eq!(
            model.setting_frame(command("autoPowerOff", 2).expect("an auto power off command")),
            Some(hex("08 EE 00 00 00 01 86 0C 00 01 01 8B"))
        );
        assert_eq!(
            model.setting_frame(command("doublePress", 1).expect("a double press command")),
            Some(hex("08 EE 00 00 00 04 81 0D 00 00 00 07 8F"))
        );
        assert_eq!(
            model.setting_frame(
                command("limitHighVolumeRefreshRate", 1).expect("a refresh rate command")
            ),
            Some(hex("08 EE 00 00 00 20 81 0B 00 01 A3"))
        );
        // The Sound mode tiles have their own path; toggles are not choices.
        for id in ["ambientSoundMode", "sideTone", "spatialAudio", ""] {
            assert_eq!(command(id, 0), None, "{id:?}");
        }
    }

    #[test]
    fn a_range_release_keeps_only_the_measured_db_steps() {
        let settings = measured_settings();
        let command = |value| number_command("limitHighVolumeDbLimit", value, &settings);
        for value in [75i32, 80, 85, 90, 95, 100] {
            let db_limit =
                DbLimit::new(u8::try_from(value).expect("the loop holds measured dB steps"))
                    .expect("a measured dB step");
            assert_eq!(
                command(value),
                Some(SettingCommand::LimitHighVolume {
                    enabled: settings.limit_high_volume.enabled,
                    db_limit,
                }),
                "{value}"
            );
        }
        for value in [-5, 0, 74, 76, 79, 101, 1000, i32::MAX] {
            assert_eq!(command(value), None, "{value}");
        }
        assert_eq!(
            number_command("sideTone", 80, &settings),
            None,
            "an unknown id never writes"
        );
        // The enable flag travels with the dB limit, exactly as the device's
        // measured body counts it.
        let enabled = A3062Settings {
            limit_high_volume: soundcore_models::LimitHighVolume {
                enabled: true,
                ..settings.limit_high_volume
            },
            ..settings
        };
        assert_eq!(
            number_command("limitHighVolumeDbLimit", 100, &enabled),
            Some(SettingCommand::LimitHighVolume {
                enabled: true,
                db_limit: DbLimit::new(100).expect("100 dB is a measured step"),
            })
        );
    }

    #[test]
    fn a_failed_setting_write_names_the_setting_and_the_reason() {
        let cases = [
            (SessionError::Timeout, SettingFailure::TimedOut),
            (
                SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "connection reset by peer",
                )),
                SettingFailure::Disconnected,
            ),
            (
                SessionError::Protocol(soundcore_session::ProtocolError::State(
                    soundcore_models::StateError::NotAStateFrame,
                )),
                SettingFailure::Unreadable,
            ),
            (SessionError::UnsupportedSetting, SettingFailure::Unreadable),
        ];
        for (error, expected) in cases {
            let failure = SettingFailure::from_session(error);
            assert_eq!(failure, expected);
            let label = failure.label(SettingCommand::SideTone(true));
            assert_eq!(label, format!("Side tone ({})", failure.reason()));
            assert!(readable(&label).contains("side tone"), "{label:?}");
        }
        // Every setting the toggles and choices write has a row name.
        let settings = measured_settings();
        let commands = [
            SettingCommand::ButtonDoublePress(None),
            SettingCommand::AmbientSoundModeCycle(settings.ambient_sound_mode_cycle),
            SettingCommand::LowBatteryPrompt(true),
            SettingCommand::DolbyAudio(true),
            SettingCommand::Ldac(true),
            SettingCommand::DualConnections(true),
            SettingCommand::AutoPowerOff(settings.auto_power_off),
            SettingCommand::LimitHighVolume {
                enabled: false,
                db_limit: settings.limit_high_volume.db_limit,
            },
            SettingCommand::LimitHighVolumeRefreshRate(settings.limit_high_volume.refresh_rate),
            SettingCommand::SideTone(true),
            SettingCommand::VoicePrompt(true),
        ];
        for setting in commands {
            assert!(!setting_label(setting).is_empty(), "{setting:?}");
            assert!(readable(setting_label(setting)).len() > 2);
        }
    }

    #[test]
    fn one_settings_write_is_one_frame_and_one_re_poll() {
        // The connect state, then the device's answer after the write: side
        // tone off.
        let transport =
            FakeTransport::scripted([captured_a3062_frame(), a3062_frame_with(&[(85, 0)])]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::SetSetting {
                request: 1,
                setting: SettingCommand::SideTone(false),
            })
            .expect("the settings write is queued");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![
                state_request(),
                hex("08 EE 00 00 00 01 84 0B 00 00 86"),
                state_request(),
            ],
            "the connect poll, exactly one settings frame, then the re-poll"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected {
                attempt: 9,
                outcome: ConnectOutcome::Connected(..)
            }
        ));
        match received.try_recv().expect("the settings change") {
            Event::SettingChanged {
                attempt: 9,
                request: 1,
                setting: SettingCommand::SideTone(false),
                outcome: Ok(state),
            } => {
                assert!(
                    !state.settings.expect("the re-polled settings").side_tone,
                    "the event carries the device's answer"
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn a_failed_settings_write_surfaces_and_ends_the_worker() {
        // Only the connect state is scripted, so the re-poll times out and the
        // click queued behind the failing one must not be written.
        let transport = FakeTransport::scripted([captured_a3062_frame()]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::SetSetting {
                request: 1,
                setting: SettingCommand::SideTone(false),
            })
            .expect("the first write is queued");
        commands
            .send(DeviceCommand::SetSetting {
                request: 2,
                setting: SettingCommand::Ldac(true),
            })
            .expect("the newer write is queued behind the first");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![
                state_request(),
                hex("08 EE 00 00 00 01 84 0B 00 00 86"),
                state_request(),
            ],
            "the queued click must not be written after the failure"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected {
                attempt: 9,
                outcome: ConnectOutcome::Connected(..)
            }
        ));
        assert!(matches!(
            received.try_recv().expect("the failed write"),
            Event::SettingChanged {
                attempt: 9,
                request: 1,
                setting: SettingCommand::SideTone(false),
                outcome: Err(SettingFailure::TimedOut)
            }
        ));
        assert!(
            received.try_recv().is_err(),
            "the worker ended without running the queued click"
        );
    }

    #[test]
    fn the_system_settings_links_open_the_windows_settings_pages() {
        // The Connections aside's two links and the Bluetooth-off banners all
        // go through this mapping.
        assert_eq!(
            system_settings_uri(SystemPage::Bluetooth),
            "ms-settings:bluetooth"
        );
        assert_eq!(system_settings_uri(SystemPage::Sound), "ms-settings:sound");
    }

    #[test]
    fn the_real_a3949_capture_reads_as_a_hundred_and_eighty_percent() {
        // The acceptance case: a real P20i answered a 5-step battery of 5/4.
        let model = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        let state = soundcore_models::parse_state_frame(model, &captured_a3949_frame())
            .expect("the real capture parses");
        let tiles = batteries(&state);
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].label, 0);
        assert_eq!(tiles[0].value.as_str(), "100%");
        assert_eq!(tiles[1].label, 1);
        assert_eq!(tiles[1].value.as_str(), "80%");
    }

    #[test]
    fn the_real_a3062_capture_reads_as_a_single_twenty_percent_tile() {
        // The acceptance case for ticket 05: the real Space One Pro answered
        // `01 01` with a single 10-step battery; it was at 2/10 when captured.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let state = soundcore_models::parse_state_frame(model, &captured_a3062_frame())
            .expect("the real capture parses");
        let tiles = batteries(&state);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].label, 3);
        assert_eq!(tiles[0].value.as_str(), "20%");
        assert!(tiles[0].known);
        assert_ratio(tiles[0].progress, 0.2);
    }

    fn hex(bytes: &str) -> Vec<u8> {
        bytes
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("the fixture stores hex bytes"))
            .collect()
    }

    /// The masked real A3062 `01 01` frame of one mode from the core fixtures.
    fn captured_mode_frame(mode: &str) -> Vec<u8> {
        let json = include_str!("../../core/data/a3062-sound-modes.json");
        let anchor = format!("\"mode\": \"{mode}\"");
        let at = json.find(&anchor).expect("the fixture lists every mode");
        captured_frame(&json[at..])
    }

    /// The masked real A3062 `01 01` frame after one equalizer preset write,
    /// from the core fixtures.
    fn captured_eq_frame(preset: &str) -> Vec<u8> {
        let json = include_str!("../../core/data/a3062-eq-presets.json");
        let anchor = format!("\"name\": \"{preset}\"");
        let at = json.find(&anchor).expect("the fixture lists every preset");
        captured_frame(&json[at..])
    }

    /// The parsed Bass Reducer state of the committed capture.
    fn bass_reducer_equalizer() -> EqualizerState {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        soundcore_models::parse_state_frame(model, &captured_eq_frame("Bass Reducer"))
            .expect("the real capture parses")
            .equalizer
            .expect("the capture reports an equalizer")
    }

    /// The measured full `03 87` frame that selects Bass Reducer from the
    /// capture's Signature state (ticket 01 recorded it from the real device).
    fn bass_reducer_frame() -> Vec<u8> {
        hex(
            "08 EE 00 00 00 03 87 3F 00 03 00 00 00 50 5A 6E 78 78 78 78 78 78 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 00 00 00 00 00 FF FF FF FF FF FF FF FF FF FF 50 5A 6E 78 78 78 78 78 78 00 00 7E",
        )
    }

    /// The exact 63-byte `03 87` frame of a 53-byte equalizer body: the measured
    /// header, the body and the checksum over every preceding frame byte.
    fn equalizer_frame(body: &[u8]) -> Vec<u8> {
        let mut frame = hex("08 EE 00 00 00 03 87 3F 00");
        frame.extend_from_slice(body);
        let checksum = soundcore_protocol::checksum(&frame);
        frame.push(checksum);
        frame
    }

    /// The parsed Soundcore Signature state the real device reported on connect
    /// in the custom-capture fixture.
    fn signature_equalizer() -> EqualizerState {
        captured_equalizer(&captured_before_custom_eq_frame())
    }

    /// The inbound `01 01` frame header: prefix (5), command (2) and total
    /// length (2). The state body starts after it.
    const STATE_FRAME_HEADER_LEN: usize = soundcore_protocol::INBOUND_PREFIX.len() + 4;
    /// The measured `01 01` body offsets of the A3062 equalizer block (ticket
    /// 01): the preset id and the first of the ten band bytes.
    const EQ_PRESET_OFFSET: usize = 23;
    const EQ_BANDS_OFFSET: usize = 25;
    /// The measured band-byte conversion: a state byte is `value + 120`, the
    /// inverse of the core's exported `EQ_BAND_MIN`..=`EQ_BAND_MAX` range.
    const BAND_BYTE_OFFSET: i16 = -soundcore_models::EQ_BAND_MIN;

    /// The `01 01` answer of a custom curve with `visible_bands`, built from the
    /// real capture frame of `base` by replacing its preset id and band bytes
    /// and recomputing the checksum.
    ///
    /// The measured state body holds the preset id at body offset 23 and the ten
    /// band bytes at 25; the body starts after the 9-byte frame header and the
    /// last frame byte is the checksum.
    fn custom_state_frame(base: &[u8], visible_bands: [i16; VISIBLE_EQ_BANDS]) -> Vec<u8> {
        const PRESET_AT: usize = STATE_FRAME_HEADER_LEN + EQ_PRESET_OFFSET;
        const BANDS_AT: usize = STATE_FRAME_HEADER_LEN + EQ_BANDS_OFFSET;
        let mut frame = base.to_vec();
        frame[PRESET_AT..PRESET_AT + 2]
            .copy_from_slice(&soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_le_bytes());
        let bands: Vec<i16> = visible_bands
            .into_iter()
            .chain(soundcore_models::FIXED_EQ_BANDS)
            .collect();
        for (index, band) in bands.iter().enumerate() {
            frame[BANDS_AT + index] =
                u8::try_from(band + BAND_BYTE_OFFSET).expect("a band byte is value + 120");
        }
        let checksum_at = frame.len() - 1;
        frame[checksum_at] = soundcore_protocol::checksum(&frame[..checksum_at]);
        frame
    }

    #[test]
    fn the_real_a3062_capture_exposes_the_sound_mode_page() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let state =
            soundcore_models::parse_state_frame(model, &captured_a3062_frame()).expect("parses");
        // The committed capture was taken while the device was in normal mode.
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            sound_pages(
                state.sound_mode,
                state.equalizer.is_some(),
                state.settings.is_some()
            ),
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
        assert_eq!(mode_index(state.sound_mode), 2);
    }

    #[test]
    fn the_real_a3949_capture_has_no_sound_mode_page() {
        let model = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        let state =
            soundcore_models::parse_state_frame(model, &captured_a3949_frame()).expect("parses");
        assert_eq!(state.sound_mode, None);
        assert!(sound_pages(state.sound_mode, state.equalizer.is_some(), false).is_empty());
        assert!(mode_options(state.sound_mode).is_empty());
        assert_eq!(mode_index(state.sound_mode), -1);
    }

    #[test]
    fn the_sound_mode_tiles_are_the_three_measured_modes_in_page_order() {
        let options = mode_options(Some(SoundMode::NoiseCanceling));
        let values: Vec<&str> = options.iter().map(|option| option.value.as_str()).collect();
        let labels: Vec<&str> = options.iter().map(|option| option.label.as_str()).collect();
        // `ModeText.icon` recognises exactly these value strings.
        assert_eq!(values, ["NoiseCanceling", "Transparency", "Normal"]);
        assert_eq!(labels, ["Noise canceling", "Transparency", "Normal"]);
    }

    #[test]
    fn a_device_without_a_sound_mode_has_no_tiles() {
        assert!(mode_options(None).is_empty());
    }

    #[test]
    fn a_mode_index_maps_to_its_mode_and_unknown_indexes_are_ignored() {
        for (index, mode) in [
            (0, SoundMode::NoiseCanceling),
            (1, SoundMode::Transparency),
            (2, SoundMode::Normal),
        ] {
            assert_eq!(mode_for_index(index), Some(mode));
            assert_eq!(mode_index(Some(mode)), index);
        }
        for index in [-1, 3, 11, i32::MAX] {
            assert_eq!(mode_for_index(index), None, "index {index} is not a tile");
        }
    }

    #[test]
    fn a_failed_mode_change_maps_to_a_readable_label() {
        let cases = [
            (SessionError::Timeout, ModeFailure::TimedOut),
            (
                SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "connection reset by peer",
                )),
                ModeFailure::Disconnected,
            ),
            (
                SessionError::Protocol(soundcore_session::ProtocolError::State(
                    soundcore_models::StateError::NotAStateFrame,
                )),
                ModeFailure::Unreadable,
            ),
            (SessionError::UnsupportedLayout, ModeFailure::Unreadable),
        ];
        for (error, expected) in cases {
            let failure = ModeFailure::from_session(error);
            assert_eq!(failure, expected);
            let label = readable(failure.label());
            assert!(label.contains("sound mode"), "{label:?} names the setting");
        }
    }

    /// The parsed equalizer block of the committed real A3062 capture.
    fn captured_a3062_equalizer() -> EqualizerState {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        soundcore_models::parse_state_frame(model, &captured_a3062_frame())
            .expect("the real capture parses")
            .equalizer
            .expect("the real capture reports an equalizer")
    }

    #[test]
    fn the_a3062_equalizer_presets_are_the_twenty_two_catalog_ids_and_names() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let options = preset_options(Some(model));
        assert_eq!(options.len(), 22, "the full A3062 catalog");
        assert_eq!(
            options.len(),
            soundcore_models::A3062_EQUALIZER_PRESETS.len(),
            "the page offers every catalog entry"
        );
        // The chip value is the decimal preset id the state reports; the label
        // is the display name.
        assert_eq!(options[0].value.as_str(), "0");
        assert_eq!(options[0].label.as_str(), "Soundcore Signature");
        let bass_reducer = options
            .iter()
            .find(|option| option.label.as_str() == "Bass Reducer")
            .expect("the catalog has Bass Reducer");
        assert_eq!(bass_reducer.value.as_str(), "3");
        let bass_booster = options
            .iter()
            .find(|option| option.label.as_str() == "Bass Booster")
            .expect("the catalog has Bass Booster");
        // The catalog's unusual id 0x7E7E, as a decimal string.
        assert_eq!(bass_booster.value.as_str(), "32382");
        // A model whose equalizer is deferred offers no chips.
        assert!(
            preset_options(soundcore_models::model_by_code("A3949")).is_empty(),
            "the A3949 equalizer is deferred in this slice"
        );
        assert!(preset_options(None).is_empty());
    }

    #[test]
    fn only_a_catalog_preset_id_is_selectable() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let presets = preset_options(Some(model));
        assert_eq!(selectable_preset_id(&presets, "0"), Some(0));
        assert_eq!(selectable_preset_id(&presets, "3"), Some(3));
        assert_eq!(selectable_preset_id(&presets, "32382"), Some(0x7E7E));
        // Id 2 is not in the A3062 list, custom bands (0xFEFE) are not a chip,
        // and a value that is not a number is not an id.
        for value in ["2", "65278", "abc", "", "-1", "32383", "90000"] {
            assert_eq!(
                selectable_preset_id(&presets, value),
                None,
                "{value:?} is not a chip"
            );
        }
        // The deferred A3949 offers no chips, so no value selects anything.
        let p20i = preset_options(soundcore_models::model_by_code("A3949"));
        assert!(p20i.is_empty());
        assert_eq!(selectable_preset_id(&p20i, "0"), None);
    }

    #[test]
    fn a_band_text_parses_decibels_into_device_units() {
        // The field shows decibels with at most one fraction digit; the device
        // stores tenths, so "3.5" is 35.
        for (text, expected) in [
            ("3.5", 35),
            ("-3.5", -35),
            ("\u{2212}1,5", -15),
            ("+13.4", 134),
            ("0", 0),
            ("0.0", 0),
            ("13.40", 134),
            (" 2,5 ", 25),
            ("-12", -120),
        ] {
            assert_eq!(parse_band_text(text), Some(expected), "{text:?}");
        }
        for text in [
            "",
            " ",
            "abc",
            "3.55",
            "1.2.3",
            "3,5,5",
            "++1",
            "--1",
            "1 000",
            "\u{221E}",
            "999999999999999999999",
        ] {
            assert_eq!(parse_band_text(text), None, "{text:?} is not a value");
        }
    }

    #[test]
    fn a_band_value_clamps_to_the_measured_range() {
        assert_eq!(clamp_band_value(0), 0);
        assert_eq!(clamp_band_value(EQ_MAX.into()), EQ_MAX);
        assert_eq!(clamp_band_value(EQ_MIN.into()), EQ_MIN);
        assert_eq!(clamp_band_value(135), EQ_MAX);
        assert_eq!(clamp_band_value(i32::MAX), EQ_MAX);
        assert_eq!(clamp_band_value(-121), EQ_MIN);
        assert_eq!(clamp_band_value(i32::MIN), EQ_MIN);
        // The clamped value is what the band field and the chart show: the
        // device range is -12.0…+13.4 dB.
        assert_eq!(band_value_text(clamp_band_value(999)), "13.4");
        assert_eq!(band_value_text(clamp_band_value(-999)), "-12");
    }

    #[test]
    fn the_custom_preset_id_is_never_a_chip() {
        // Custom bands are written by the band controls, never by a chip: the
        // 0xFEFE id is not one of the catalog values.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let presets = preset_options(Some(model));
        assert_eq!(
            selectable_preset_id(
                &presets,
                &soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_string()
            ),
            None,
            "a custom preset id cannot be written through the preset chips"
        );
        for value in ["commit", "preview", "type", "reset", "0.0", "+3"] {
            assert_eq!(
                selectable_preset_id(&presets, value),
                None,
                "{value:?} is not a chip"
            );
        }
    }

    /// A page that records what a band edit shows, in place of the Slint
    /// `AppState`.
    #[derive(Default)]
    struct RecordedPage {
        shown: RefCell<Vec<i16>>,
        curves: RefCell<Vec<Vec<i16>>>,
        reported: RefCell<Vec<EqualizerState>>,
        errors: RefCell<Vec<String>>,
    }

    impl RecordedPage {
        fn showing(values: impl IntoIterator<Item = i16>) -> Self {
            Self {
                shown: RefCell::new(values.into_iter().collect()),
                ..Self::default()
            }
        }
    }

    impl EqPage for RecordedPage {
        fn shown_band_values(&self) -> Vec<i16> {
            self.shown.borrow().clone()
        }

        fn show_curve(&self, values: &[i16]) {
            *self.shown.borrow_mut() = values.to_vec();
            self.curves.borrow_mut().push(values.to_vec());
        }

        fn show_reported_curve(&self, equalizer: &EqualizerState) {
            *self.shown.borrow_mut() = equalizer.visible_bands().to_vec();
            self.reported.borrow_mut().push(*equalizer);
        }

        fn show_error(&self, label: &str) {
            self.errors.borrow_mut().push(label.to_owned());
        }
    }

    /// A writer that records the curves a band edit queues, in place of the
    /// device worker's command channel.
    struct RecordedWriter {
        curves: RefCell<Vec<[i16; VISIBLE_EQ_BANDS]>>,
        connected: Cell<bool>,
    }

    impl Default for RecordedWriter {
        fn default() -> Self {
            Self {
                curves: RefCell::default(),
                connected: Cell::new(true),
            }
        }
    }

    impl BandWriter for RecordedWriter {
        fn write_curve(&self, bands: [i16; VISIBLE_EQ_BANDS]) -> bool {
            if !self.connected.get() {
                return false;
            }
            self.curves.borrow_mut().push(bands);
            true
        }
    }

    /// A Save profile dialog that records what an `eq-save` click shows, in place
    /// of the Slint `AppState`.
    #[derive(Default)]
    struct RecordedSaveDialog {
        problem: Cell<i32>,
        existing: RefCell<String>,
        closed: Cell<bool>,
        error: RefCell<String>,
    }

    impl SaveDialogPage for RecordedSaveDialog {
        fn show_save_problem(&self, problem: SaveProblem) {
            self.problem.set(problem.value());
        }

        fn show_save_existing(&self, name: &str) {
            *self.existing.borrow_mut() = name.to_owned();
        }

        fn close_save_dialog(&self) {
            self.closed.set(true);
        }

        fn show_profile_error(&self, label: &str) {
            *self.error.borrow_mut() = label.to_owned();
        }

        fn clear_profile_error(&self) {
            let shows_write_failure = self.error.borrow().as_str() == PROFILE_WRITE_FAILURE_LABEL;
            if shows_write_failure {
                self.error.borrow_mut().clear();
            }
        }
    }

    #[test]
    fn a_preview_moves_the_page_without_a_curve_write() {
        let page = RecordedPage::showing([0; VISIBLE_EQ_BANDS]);
        let writer = RecordedWriter::default();

        let wrote = answer_band_action(
            &page,
            &writer,
            BandAction::Preview {
                index: 2,
                value: 40,
            },
        );

        assert!(!wrote, "a preview never queues a write");
        assert!(
            writer.curves.borrow().is_empty(),
            "a preview does not tell the device"
        );
        assert_eq!(*page.shown.borrow(), [0, 0, 40, 0, 0, 0, 0, 0]);
        assert_eq!(
            page.curves.borrow().len(),
            1,
            "the page curve follows the drag at once"
        );
    }

    #[test]
    fn a_commit_clamps_the_band_and_writes_the_whole_curve_once() {
        let page = RecordedPage::showing([0; VISIBLE_EQ_BANDS]);
        let writer = RecordedWriter::default();

        let wrote = answer_band_action(
            &page,
            &writer,
            BandAction::Commit {
                index: 1,
                value: 999,
            },
        );

        assert!(wrote);
        assert_eq!(writer.curves.borrow().len(), 1, "exactly one write");
        assert_eq!(
            writer.curves.borrow()[0],
            [0, EQ_MAX, 0, 0, 0, 0, 0, 0],
            "the out-of-range value is clamped and the whole curve is written"
        );
        assert_eq!(
            *page.shown.borrow(),
            [0, EQ_MAX, 0, 0, 0, 0, 0, 0],
            "the page shows the clamped value"
        );
    }

    #[test]
    fn a_commit_keeps_the_other_bands_the_page_shows() {
        let page = RecordedPage::showing([30, -30, 60, 0, -60, 20, 0, -20]);
        let writer = RecordedWriter::default();

        answer_band_action(
            &page,
            &writer,
            BandAction::Commit {
                index: 6,
                value: -50,
            },
        );

        assert_eq!(
            writer.curves.borrow()[0],
            [30, -30, 60, 0, -60, 20, -50, -20],
            "a commit writes every band the page shows, not only the edited one"
        );
    }

    #[test]
    fn a_reset_shows_and_writes_the_flat_curve() {
        let page = RecordedPage::showing([30, -30, 60, 0, -60, 20, 0, -20]);
        let writer = RecordedWriter::default();

        let wrote = answer_band_action(&page, &writer, BandAction::Reset);

        assert!(wrote);
        assert_eq!(writer.curves.borrow().len(), 1, "one flat custom curve");
        assert_eq!(
            writer.curves.borrow()[0],
            [0; VISIBLE_EQ_BANDS],
            "every visible band is 0 dB"
        );
        assert_eq!(*page.shown.borrow(), [0; VISIBLE_EQ_BANDS]);
    }

    #[test]
    fn a_band_index_the_page_does_not_show_is_ignored() {
        let page = RecordedPage::showing([0; VISIBLE_EQ_BANDS]);
        let writer = RecordedWriter::default();

        for index in [-1, VISIBLE_EQ_BANDS as i32, 100] {
            for action in [
                BandAction::Preview { index, value: 40 },
                BandAction::Commit { index, value: 40 },
            ] {
                assert!(
                    !answer_band_action(&page, &writer, action),
                    "{action:?} matches no band"
                );
            }
        }
        assert!(writer.curves.borrow().is_empty());
        assert!(page.curves.borrow().is_empty());
    }

    #[test]
    fn a_stopped_worker_shows_the_readable_curve_error() {
        let page = RecordedPage::showing([0; VISIBLE_EQ_BANDS]);
        let writer = RecordedWriter::default();
        writer.connected.set(false);

        let wrote = answer_band_action(
            &page,
            &writer,
            BandAction::Commit {
                index: 0,
                value: 30,
            },
        );

        assert!(!wrote);
        assert_eq!(
            page.errors.borrow()[0],
            EqFailure::Disconnected.bands_label(),
            "the banner names the bands and why nothing was written"
        );
    }

    #[test]
    fn a_failed_curve_change_maps_to_a_readable_label() {
        let cases = [
            (SessionError::Timeout, EqFailure::TimedOut),
            (
                SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "connection reset by peer",
                )),
                EqFailure::Disconnected,
            ),
            (
                SessionError::Protocol(soundcore_session::ProtocolError::State(
                    soundcore_models::StateError::NotAStateFrame,
                )),
                EqFailure::Unreadable,
            ),
            (SessionError::UnsupportedEqualizer, EqFailure::Unreadable),
        ];
        for (error, expected) in cases {
            let failure = EqFailure::from_session(error);
            assert_eq!(failure, expected);
            let label = readable(failure.bands_label());
            assert!(
                label.contains("equalizer bands"),
                "{label:?} names the bands"
            );
        }
    }

    #[test]
    fn a_finished_curve_change_shows_the_reported_state_or_the_error() {
        let custom = custom_equalizer_state();
        let page = RecordedPage::showing([0; VISIBLE_EQ_BANDS]);
        let stopped = Cell::new(0);
        let stop = || stopped.set(stopped.get() + 1);

        apply_curve_outcome(&page, &stop, Ok(custom));

        assert_eq!(page.reported.borrow().len(), 1);
        assert_eq!(page.reported.borrow()[0], custom);
        assert_eq!(
            *page.shown.borrow(),
            custom.visible_bands(),
            "the page follows the re-polled device state"
        );
        assert_eq!(stopped.get(), 0);
        assert_eq!(
            page.errors.borrow().last().map(String::as_str),
            Some(""),
            "a success clears the banner"
        );

        apply_curve_outcome(&page, &stop, Err(EqFailure::TimedOut));
        assert_eq!(stopped.get(), 1, "a failed change stops the worker");
        assert_eq!(
            page.errors.borrow().last().map(String::as_str),
            Some("Equalizer bands (no answer)")
        );
    }

    #[test]
    fn the_real_a3062_capture_shows_its_preset_and_eight_editable_bands() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let equalizer = captured_a3062_equalizer();
        let display = EqualizerDisplay::new(Some(model), Some(&equalizer));

        assert!(display.available);
        assert!(display.editable, "the A3062 page writes custom band values");
        // The committed capture was taken on Soundcore Signature.
        assert_eq!(display.preset, "0");
        assert_eq!(display.name, "Soundcore Signature");
        assert_eq!(display.preset_count, 22);
        assert_eq!(display.min, -120);
        assert_eq!(display.max, 134);
        assert_eq!(display.min_text, "\u{2212}12.0");
        assert_eq!(display.max_text, "+13.4");

        // Eight visible bands: 100 Hz … 12.8 kHz, the signature curve at 0 dB.
        assert_eq!(display.band_count, 8);
        let frequencies: Vec<&str> = display
            .bands
            .iter()
            .map(|band| band.frequency.as_str())
            .collect();
        let khz: Vec<bool> = display.bands.iter().map(|band| band.khz).collect();
        let values: Vec<i32> = display.bands.iter().map(|band| band.value).collect();
        assert_eq!(
            frequencies,
            ["100", "200", "400", "800", "1.6", "3.2", "6.4", "12.8"]
        );
        assert_eq!(khz, [false, false, false, false, true, true, true, true]);
        assert_eq!(values, [0; 8]);
        assert!(
            display.bands.iter().all(|band| band.text.as_str() == "0"),
            "0 dB fields show \"0\""
        );

        // The chart's guides are the range ends, their halves and zero.
        let lines: Vec<i32> = display.lines.iter().map(|line| line.value).collect();
        let labels: Vec<&str> = display
            .lines
            .iter()
            .map(|line| line.label.as_str())
            .collect();
        assert_eq!(lines, [134, 70, 0, -60, -120]);
        assert_eq!(
            labels,
            ["+13.4", "+7.0", "0.0", "\u{2212}6.0", "\u{2212}12.0"]
        );
        assert!(
            display.chart_line.starts_with("M "),
            "{}",
            display.chart_line
        );
        assert!(display.chart_area.ends_with('Z'), "{}", display.chart_area);
    }

    #[test]
    fn a_preset_with_boost_and_cut_bands_shows_their_values_and_texts() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let equalizer = bass_reducer_equalizer();
        let display = EqualizerDisplay::new(Some(model), Some(&equalizer));
        assert_eq!(display.preset, "3");
        assert_eq!(display.name, "Bass Reducer");
        let values: Vec<i32> = display.bands.iter().map(|band| band.value).collect();
        let texts: Vec<&str> = display
            .bands
            .iter()
            .map(|band| band.text.as_str())
            .collect();
        assert_eq!(values, [-40, -30, -10, 0, 0, 0, 0, 0]);
        assert_eq!(texts, ["-4", "-3", "-1", "0", "0", "0", "0", "0"]);
        // The equalizer band fields never show the invisible last two bytes.
        assert_eq!(display.bands.len(), 8);
    }

    #[test]
    fn a_custom_state_shows_the_custom_preset_id_and_no_built_in_name() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let equalizer = custom_equalizer_state();
        assert!(equalizer.is_custom());

        let display = EqualizerDisplay::new(Some(model), Some(&equalizer));
        assert!(display.available);
        assert!(
            display.editable,
            "custom bands stay editable after a re-poll"
        );
        assert_eq!(
            display.preset,
            soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_string(),
            "the custom id string"
        );
        assert_eq!(display.name, "", "custom bands have no built-in name");
        let values: Vec<i32> = display.bands.iter().map(|band| band.value).collect();
        assert_eq!(
            values,
            [30, -30, 60, 0, -60, 20, 0, -20],
            "the re-polled curve is the one the device reports"
        );
        // No chip is selected: the custom id is not one of the catalog values.
        assert!(
            !display
                .presets
                .iter()
                .any(|preset| preset.value.as_str() == display.preset)
        );
        assert_eq!(
            display.preset_count, 22,
            "the preset chips stay available after custom bands"
        );
    }

    #[test]
    fn the_equalizer_page_is_exposed_for_the_a3062_and_deferred_for_the_a3949() {
        let a3062 = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let a3062_state = soundcore_models::parse_state_frame(a3062, &captured_a3062_frame())
            .expect("the A3062 capture parses");
        assert_eq!(
            sound_pages(
                a3062_state.sound_mode,
                a3062_state.equalizer.is_some(),
                a3062_state.settings.is_some()
            ),
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
        assert!(
            EqualizerDisplay::new(Some(a3062), a3062_state.equalizer.as_ref()).editable,
            "the A3062 page is editable"
        );

        // The A3949's equalizer is deliberately deferred in this slice: the
        // real capture's block (body 32..44) is known, but no A3949 `03 87`
        // write body is verified, so the model has no capability, catalog or
        // page, no editable controls and no chip value is selectable. This is a
        // limitation, not a claim that the model has no equalizer.
        let a3949 = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        let a3949_state = soundcore_models::parse_state_frame(a3949, &captured_a3949_frame())
            .expect("the A3949 capture parses");
        assert!(
            a3949_state.equalizer.is_none(),
            "A3949 equalizer parsing is deferred"
        );
        assert!(
            sound_pages(
                a3949_state.sound_mode,
                a3949_state.equalizer.is_some(),
                a3949_state.settings.is_some()
            )
            .is_empty()
        );
        assert!(preset_options(Some(a3949)).is_empty());
        assert_eq!(
            selectable_preset_id(&preset_options(Some(a3949)), "0"),
            None,
            "no chip value is offered, so no preset click can name one"
        );
        let display = EqualizerDisplay::new(Some(a3949), a3949_state.equalizer.as_ref());
        assert!(!display.available);
        assert!(
            !display.editable,
            "the deferred A3949 page has no band controls"
        );
        assert_eq!(display.preset_count, 0);
        assert!(display.presets.is_empty());
        assert!(display.bands.is_empty());
        assert_eq!(display.band_count, 0);
    }

    #[test]
    fn the_chart_follows_the_band_values_inside_its_margins() {
        // The flat signature curve is a straight line: 0 dB sits slightly above
        // the middle, because the range is -12.0…+13.4 dB.
        let flat = [0i16; VISIBLE_EQ_BANDS];
        let (line, area) = equalizer_chart(&flat, EQ_MIN, EQ_MAX);
        assert!(line.starts_with("M 37.5 104.9"), "{line}");
        assert!(line.ends_with("562.5 104.9"), "{line}");
        assert!(area.starts_with("M 37.5 200.0 L 37.5 104.9"), "{area}");
        assert!(area.ends_with("L 562.5 200.0 Z"), "{area}");

        // The highest and lowest values sit at the chart's margins, not past
        // them.
        let (line, _) = equalizer_chart(&[EQ_MAX, EQ_MIN], EQ_MIN, EQ_MAX);
        assert!(line.starts_with("M 150.0 12.0"), "{line}");
        assert!(line.ends_with("450.0 188.0"), "{line}");
        // Nothing to draw.
        assert_eq!(
            equalizer_chart(&[], EQ_MIN, EQ_MAX),
            (String::new(), String::new())
        );
        assert_eq!(
            equalizer_chart(&flat, 10, 10),
            (String::new(), String::new())
        );
    }

    #[test]
    fn a_failed_preset_change_maps_to_a_readable_label() {
        let cases = [
            (SessionError::Timeout, EqFailure::TimedOut),
            (
                SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "connection reset by peer",
                )),
                EqFailure::Disconnected,
            ),
            (
                SessionError::Protocol(soundcore_session::ProtocolError::State(
                    soundcore_models::StateError::NotAStateFrame,
                )),
                EqFailure::Unreadable,
            ),
            (SessionError::UnsupportedEqualizer, EqFailure::Unreadable),
        ];
        for (error, expected) in cases {
            let failure = EqFailure::from_session(error);
            assert_eq!(failure, expected);
            let label = readable(failure.label());
            assert!(label.contains("equalizer"), "{label:?} names the setting");
        }
    }

    #[test]
    fn a_failed_preset_change_surfaces_although_a_newer_click_was_queued() {
        let failure: Result<EqualizerState, EqFailure> = Err(EqFailure::TimedOut);
        assert!(
            equalizer_change_applies(&failure, 4, 1, 4, 2),
            "a failure of the current attempt always surfaces"
        );
        let success = Ok(captured_a3062_equalizer());
        assert!(!equalizer_change_applies(&success, 4, 1, 4, 2));
        assert!(equalizer_change_applies(&success, 4, 2, 4, 2));
        assert!(!equalizer_change_applies(&failure, 3, 1, 4, 1));
    }

    /// A scripted transport: writes are recorded and reads come from a queue.
    #[derive(Clone, Default)]
    struct FakeTransport {
        writes: Rc<RefCell<Vec<Vec<u8>>>>,
        reads: Rc<RefCell<std::collections::VecDeque<std::io::Result<Vec<u8>>>>>,
    }

    impl FakeTransport {
        fn scripted(chunks: impl IntoIterator<Item = Vec<u8>>) -> Self {
            let transport = Self::default();
            for chunk in chunks {
                let mut reads = transport.reads.borrow_mut();
                reads.push_back(Ok(chunk));
            }
            transport
        }

        fn failing(kind: std::io::ErrorKind) -> Self {
            let transport = Self::default();
            transport
                .reads
                .borrow_mut()
                .push_back(Err(std::io::Error::new(kind, "scripted read failure")));
            transport
        }

        fn writes(&self) -> Vec<Vec<u8>> {
            self.writes.borrow().clone()
        }
    }

    impl Transport for FakeTransport {
        fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.writes.borrow_mut().push(bytes.to_vec());
            Ok(())
        }

        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let next = self.reads.borrow_mut().pop_front();
            match next {
                Some(Ok(chunk)) => {
                    assert!(
                        chunk.len() <= buffer.len(),
                        "a scripted chunk fits in one read"
                    );
                    buffer[..chunk.len()].copy_from_slice(&chunk);
                    Ok(chunk.len())
                }
                Some(Err(error)) => Err(error),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "no read was scripted",
                )),
            }
        }
    }

    /// The masked empty inbound `06 81` acknowledgement the real A3062 sends.
    fn mode_ack() -> Vec<u8> {
        hex("09 FF 00 00 01 06 81 0A 00 9A")
    }

    fn session_with(model_code: &str, transport: &FakeTransport) -> DeviceSession<FakeTransport> {
        let model = soundcore_models::model_by_code(model_code).expect("the model is supported");
        DeviceSession::new(model, transport.clone())
    }

    #[test]
    fn each_measured_mode_switch_writes_exactly_its_frame_and_re_reads_it() {
        let expected_frames = [
            "08 EE 00 00 00 06 81 10 00 00 51 01 01 00 05 E5",
            "08 EE 00 00 00 06 81 10 00 01 51 01 01 00 05 E6",
            "08 EE 00 00 00 06 81 10 00 02 51 01 01 00 05 E7",
        ];
        let names = ["NoiseCanceling", "Transparency", "Normal"];
        for ((mode, name), expected) in SoundMode::ALL.into_iter().zip(names).zip(expected_frames) {
            // The real device acknowledges the write, then answers the 01 01
            // re-poll with the mode it now reports.
            let mut answer = mode_ack();
            answer.extend_from_slice(&captured_mode_frame(name));
            let transport = FakeTransport::scripted([answer]);
            let mut session = session_with("A3062", &transport);

            let reported = change_sound_mode(&mut session, mode).expect("the change takes");

            assert_eq!(reported, mode);
            assert_eq!(mode_index(Some(reported)), mode_index(Some(mode)));
            assert_eq!(
                transport.writes(),
                vec![hex(expected), hex("08 EE 00 00 00 01 01 0A 00 02")],
                "{name}: one 06 81 frame, then the 01 01 re-poll"
            );
        }
    }

    #[test]
    fn the_page_shows_the_mode_the_device_reports_after_the_write() {
        // The write was acknowledged but the device reports another mode: the
        // page follows the device, not the click.
        let mut answer = mode_ack();
        answer.extend_from_slice(&captured_mode_frame("Transparency"));
        let transport = FakeTransport::scripted([answer]);
        let mut session = session_with("A3062", &transport);

        let reported =
            change_sound_mode(&mut session, SoundMode::NoiseCanceling).expect("the change is read");

        assert_eq!(reported, SoundMode::Transparency);
        assert_eq!(mode_index(Some(reported)), 1);
    }

    #[test]
    fn a_silent_device_after_the_write_is_a_timeout() {
        let transport = FakeTransport::failing(std::io::ErrorKind::TimedOut);
        let mut session = session_with("A3062", &transport);

        let failure =
            change_sound_mode(&mut session, SoundMode::Transparency).expect_err("no answer");

        assert_eq!(failure, ModeFailure::TimedOut);
        assert_eq!(failure.label(), "Sound mode (no answer)");
        assert_eq!(
            transport.writes(),
            vec![
                hex("08 EE 00 00 00 06 81 10 00 01 51 01 01 00 05 E6"),
                hex("08 EE 00 00 00 01 01 0A 00 02"),
            ],
            "the mode frame and the 01 01 re-poll were written"
        );
    }

    #[test]
    fn a_dropped_link_after_the_write_is_a_lost_connection() {
        let transport = FakeTransport::failing(std::io::ErrorKind::ConnectionReset);
        let mut session = session_with("A3062", &transport);

        let failure =
            change_sound_mode(&mut session, SoundMode::Normal).expect_err("the link dropped");

        assert_eq!(failure, ModeFailure::Disconnected);
        assert_eq!(failure.label(), "Sound mode (connection lost)");
    }

    #[test]
    fn a_failed_change_surfaces_although_a_newer_click_was_queued() {
        // Regression: request 1 fails while request 2 is already queued.
        // Request 1 is superseded, but the worker stops after the failure, so
        // request 2 never runs: the failure must still reach the page or the
        // click leaves no feedback at all.
        let failure = Err(ModeFailure::TimedOut);
        assert!(
            mode_result_applies(&failure, 4, 1, 4, 2),
            "a failure of the current attempt always surfaces"
        );
        // A superseded success is still stale; the newest result applies.
        let success = Ok(SoundMode::Normal);
        assert!(!mode_result_applies(&success, 4, 1, 4, 2));
        assert!(mode_result_applies(&success, 4, 2, 4, 2));
        assert!(mode_result_applies(&failure, 4, 2, 4, 2));
        // Results of an older connection attempt never apply, failure or not.
        assert!(!mode_result_applies(&failure, 3, 1, 4, 1));
        assert!(!mode_result_applies(&success, 3, 2, 4, 2));
    }

    #[test]
    fn a_failed_change_ends_the_worker_without_running_the_queued_click() {
        // The initial state is a real masked A3062 capture (normal mode); no
        // read is scripted after it, so the re-poll of the first change times
        // out.
        let transport = FakeTransport::scripted([captured_a3062_frame()]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::ChangeSoundMode {
                request: 1,
                mode: SoundMode::Transparency,
            })
            .expect("the first click is queued");
        commands
            .send(DeviceCommand::ChangeSoundMode {
                request: 2,
                mode: SoundMode::Normal,
            })
            .expect("the newer click is queued behind the first");

        serve_device(session, 9, events, command_receiver);

        // Only the first change reached the device: the connect `01 01`, its
        // `06 81`, and the `01 01` re-poll that timed out. The queued click
        // wrote nothing.
        assert_eq!(
            transport.writes(),
            vec![
                hex("08 EE 00 00 00 01 01 0A 00 02"),
                hex("08 EE 00 00 00 06 81 10 00 01 51 01 01 00 05 E6"),
                hex("08 EE 00 00 00 01 01 0A 00 02"),
            ],
            "the queued click must not be written after the failure"
        );
        // The page gets the state, then the failure of the request that was in
        // flight; the drain surfaces it even though request 2 is newer.
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected {
                attempt: 9,
                outcome: ConnectOutcome::Connected(..)
            }
        ));
        assert!(matches!(
            received.try_recv().expect("the failed change"),
            Event::ModeChanged {
                attempt: 9,
                request: 1,
                outcome: Err(ModeFailure::TimedOut)
            }
        ));
        // The worker ended instead of running the queued click, so the banner's
        // Retry is the way back to a working connection.
        assert!(
            received.try_recv().is_err(),
            "the worker reports nothing else and stops"
        );
    }

    /// The `01 01` re-poll request the worker writes after every change.
    fn state_request() -> Vec<u8> {
        hex("08 EE 00 00 00 01 01 0A 00 02")
    }

    #[test]
    fn a_preset_switch_writes_the_measured_frame_and_re_reads_the_curve() {
        let current = captured_a3062_equalizer();
        let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
        // The real device acknowledges `03 87`; the re-poll skips the ack and
        // reads the state it now reports.
        let transport = FakeTransport::scripted([captured_eq_frame("Bass Reducer")]);
        let mut session = session_with("A3062", &transport);

        let reported =
            change_equalizer_preset(&mut session, &current, preset).expect("the change takes");

        assert_eq!(reported.preset_id, 3);
        assert_eq!(
            &reported.bands[..VISIBLE_EQ_BANDS],
            &preset.bands[..VISIBLE_EQ_BANDS]
        );
        assert_eq!(
            transport.writes(),
            vec![bass_reducer_frame(), state_request()],
            "one measured 03 87 frame, then the 01 01 re-poll"
        );
        // The page follows the re-polled state.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let display = EqualizerDisplay::new(Some(model), Some(&reported));
        assert_eq!(display.preset, "3");
        assert_eq!(display.name, "Bass Reducer");
        let values: Vec<i32> = display.bands.iter().map(|band| band.value).collect();
        assert_eq!(values, [-40, -30, -10, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn the_page_shows_the_preset_the_device_reports_after_the_write() {
        // The write was acknowledged but the device reports another preset: the
        // page follows the device, not the click.
        let current = captured_a3062_equalizer();
        let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
        let transport = FakeTransport::scripted([captured_eq_frame("Podcast")]);
        let mut session = session_with("A3062", &transport);

        let reported =
            change_equalizer_preset(&mut session, &current, preset).expect("the change is read");

        assert_eq!(reported.preset_id, 5);
        assert_eq!(reported.preset().map(|preset| preset.name), Some("Podcast"));
    }

    #[test]
    fn a_silent_device_after_the_preset_write_is_a_timeout() {
        let current = captured_a3062_equalizer();
        let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
        let transport = FakeTransport::failing(std::io::ErrorKind::TimedOut);
        let mut session = session_with("A3062", &transport);

        let failure = change_equalizer_preset(&mut session, &current, preset)
            .expect_err("the device stays silent");

        assert_eq!(failure, EqFailure::TimedOut);
        assert_eq!(failure.label(), "Equalizer profile (no answer)");
        assert_eq!(
            transport.writes(),
            vec![bass_reducer_frame(), state_request()],
            "the preset frame and the 01 01 re-poll were written"
        );
    }

    #[test]
    fn a_dropped_link_after_the_preset_write_is_a_lost_connection() {
        let current = captured_a3062_equalizer();
        let preset = soundcore_models::a3062_equalizer_preset(3).expect("Bass Reducer");
        let transport = FakeTransport::failing(std::io::ErrorKind::ConnectionReset);
        let mut session = session_with("A3062", &transport);

        let failure =
            change_equalizer_preset(&mut session, &current, preset).expect_err("the link dropped");

        assert_eq!(failure, EqFailure::Disconnected);
        assert_eq!(failure.label(), "Equalizer profile (connection lost)");
    }

    #[test]
    fn a_custom_curve_write_writes_the_measured_frame_and_re_reads_it() {
        let current = signature_equalizer();
        let curve = [30, -30, 60, 0, -60, 20, 0, -20];
        // The captured frame is exactly what the core body builder produces
        // from the state found on connect: the fixture is a real accepted frame.
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        assert_eq!(
            captured_custom_eq_write_frame(),
            equalizer_frame(
                &model
                    .custom_equalizer_body(&current, curve)
                    .expect("the A3062 custom body")
            ),
            "the captured write frame is the body of the captured state"
        );

        let transport = FakeTransport::scripted([captured_custom_eq_frame()]);
        let mut session = session_with("A3062", &transport);

        let reported =
            change_custom_equalizer(&mut session, &current, curve).expect("the write is accepted");

        assert!(reported.is_custom());
        assert_eq!(
            reported.preset_id,
            soundcore_models::CUSTOM_EQUALIZER_PRESET_ID
        );
        assert_eq!(reported.visible_bands(), curve);
        assert_eq!(
            transport.writes(),
            vec![captured_custom_eq_write_frame(), state_request()],
            "one measured custom 03 87 frame, then the 01 01 re-poll"
        );
    }

    #[test]
    fn a_silent_device_after_the_custom_curve_write_is_a_timeout() {
        let current = signature_equalizer();
        let transport = FakeTransport::failing(std::io::ErrorKind::TimedOut);
        let mut session = session_with("A3062", &transport);

        let failure = change_custom_equalizer(&mut session, &current, [0; VISIBLE_EQ_BANDS])
            .expect_err("the device stays silent");

        assert_eq!(failure, EqFailure::TimedOut);
        assert_eq!(failure.bands_label(), "Equalizer bands (no answer)");
        let writes = transport.writes();
        assert_eq!(writes.len(), 2, "the custom frame and the 01 01 re-poll");
        assert_eq!(&writes[0][..9], &hex("08 EE 00 00 00 03 87 3F 00")[..]);
        assert_eq!(&writes[0][9..11], &[0xFE, 0xFE], "the custom preset id");
        assert_eq!(writes[0].len(), 63, "the measured 53-byte body");
        assert_eq!(writes[1], state_request());
    }

    #[test]
    fn a_reset_writes_a_flat_custom_curve_and_re_reads_it() {
        let current = bass_reducer_equalizer();
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let expected_body = model
            .custom_equalizer_body(&current, [0; VISIBLE_EQ_BANDS])
            .expect("the A3062 custom body");
        // The measured flat custom body: custom preset id, the eight 0 dB bands
        // and the two fixed bands, written in both band regions.
        assert_eq!(&expected_body[..2], &[0xFE, 0xFE]);
        assert_eq!(
            &expected_body[4..14],
            &[0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00],
            "eight 0 dB bands, then 0 dB and -12.0 dB"
        );
        assert_eq!(&expected_body[42..52], &expected_body[4..14]);

        let reset_answer =
            custom_state_frame(&captured_eq_frame("Bass Reducer"), [0; VISIBLE_EQ_BANDS]);
        let transport = FakeTransport::scripted([reset_answer]);
        let mut session = session_with("A3062", &transport);

        let reported = change_custom_equalizer(&mut session, &current, [0; VISIBLE_EQ_BANDS])
            .expect("the reset is accepted");

        assert!(reported.is_custom());
        assert_eq!(reported.visible_bands(), [0; VISIBLE_EQ_BANDS]);
        assert_eq!(
            transport.writes(),
            vec![equalizer_frame(&expected_body), state_request()],
            "one flat custom 03 87 frame, then the 01 01 re-poll"
        );
    }

    #[test]
    fn the_worker_writes_one_custom_frame_and_reports_the_custom_state() {
        let transport = FakeTransport::scripted([
            captured_before_custom_eq_frame(),
            captured_custom_eq_frame(),
        ]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::WriteCustomCurve {
                request: 1,
                bands: [30, -30, 60, 0, -60, 20, 0, -20],
            })
            .expect("the curve change is queued");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![
                state_request(),
                captured_custom_eq_write_frame(),
                state_request(),
            ],
            "one custom 03 87 frame, then the 01 01 re-poll"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected {
                attempt: 9,
                outcome: ConnectOutcome::Connected(..)
            }
        ));
        assert!(matches!(
            received.try_recv().expect("the curve change"),
            Event::CurveChanged {
                attempt: 9,
                request: 1,
                outcome: Ok(reported)
            } if reported.is_custom()
                && reported.visible_bands() == [30, -30, 60, 0, -60, 20, 0, -20]
        ));
    }

    #[test]
    fn the_worker_switches_back_to_a_built_in_preset_after_a_custom_curve() {
        // The worker keeps the re-polled custom state, so the preset frame is
        // built from the custom state's HearID block — which the custom write
        // preserved — and is the same measured frame as from the Signature
        // state.
        let transport = FakeTransport::scripted([
            captured_before_custom_eq_frame(),
            captured_custom_eq_frame(),
            captured_eq_frame("Bass Reducer"),
        ]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::WriteCustomCurve {
                request: 1,
                bands: [30, -30, 60, 0, -60, 20, 0, -20],
            })
            .expect("the curve change is queued");
        commands
            .send(DeviceCommand::ChangeEqualizerPreset {
                request: 1,
                preset_id: 3,
            })
            .expect("the preset change is queued behind it");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![
                state_request(),
                captured_custom_eq_write_frame(),
                state_request(),
                bass_reducer_frame(),
                state_request(),
            ],
            "the custom curve, then the measured preset frame, each re-polled"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected { .. }
        ));
        assert!(matches!(
            received.try_recv().expect("the curve change"),
            Event::CurveChanged {
                request: 1,
                outcome: Ok(reported),
                ..
            } if reported.is_custom()
        ));
        assert!(matches!(
            received.try_recv().expect("the preset change"),
            Event::PresetChanged {
                request: 1,
                outcome: Ok(reported),
                ..
            } if reported.preset_id == 3
                && reported.visible_bands() == [-40, -30, -10, 0, 0, 0, 0, 0]
        ));
    }

    #[test]
    fn the_worker_serves_equalizer_presets_alongside_sound_modes() {
        let mut mode_answer = mode_ack();
        mode_answer.extend_from_slice(&captured_mode_frame("Transparency"));
        let transport = FakeTransport::scripted([
            captured_a3062_frame(),
            mode_answer,
            captured_eq_frame("Bass Reducer"),
        ]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::ChangeSoundMode {
                request: 1,
                mode: SoundMode::Transparency,
            })
            .expect("the mode click is queued");
        commands
            .send(DeviceCommand::ChangeEqualizerPreset {
                request: 1,
                preset_id: 3,
            })
            .expect("the preset click is queued behind it");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![
                state_request(),
                hex("08 EE 00 00 00 06 81 10 00 01 51 01 01 00 05 E6"),
                state_request(),
                bass_reducer_frame(),
                state_request(),
            ],
            "both writes and both re-polls, in click order"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected {
                attempt: 9,
                outcome: ConnectOutcome::Connected(..)
            }
        ));
        assert!(matches!(
            received.try_recv().expect("the mode change"),
            Event::ModeChanged {
                attempt: 9,
                request: 1,
                outcome: Ok(SoundMode::Transparency)
            }
        ));
        assert!(matches!(
            received.try_recv().expect("the preset change"),
            Event::PresetChanged {
                attempt: 9,
                request: 1,
                outcome: Ok(reported)
            } if reported.preset_id == 3
        ));
    }

    #[test]
    fn an_unknown_preset_id_writes_no_equalizer_frame() {
        let transport = FakeTransport::scripted([captured_a3062_frame()]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        // Id 2 is not in the A3062 catalog; the UI never offers it, and the
        // worker refuses to invent a body for it.
        commands
            .send(DeviceCommand::ChangeEqualizerPreset {
                request: 1,
                preset_id: 2,
            })
            .expect("the click is queued");
        drop(commands);

        serve_device(session, 3, events, command_receiver);

        assert_eq!(
            transport.writes(),
            vec![state_request()],
            "no 03 87 frame is written for an unknown preset"
        );
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected { .. }
        ));
        assert!(matches!(
            received.try_recv().expect("the refused change"),
            Event::PresetChanged {
                request: 1,
                outcome: Err(EqFailure::Unreadable),
                ..
            }
        ));
    }

    #[test]
    fn a_failed_preset_change_ends_the_worker_without_running_the_queued_click() {
        // The initial state is a real masked A3062 capture; no read is scripted
        // after it, so the re-poll of the first change times out.
        let transport = FakeTransport::scripted([captured_a3062_frame()]);
        let session = session_with("A3062", &transport);
        let (events, received) = mpsc::channel();
        let (commands, command_receiver) = mpsc::channel();
        commands
            .send(DeviceCommand::ChangeEqualizerPreset {
                request: 1,
                preset_id: 1,
            })
            .expect("the first click is queued");
        commands
            .send(DeviceCommand::ChangeEqualizerPreset {
                request: 2,
                preset_id: 3,
            })
            .expect("the newer click is queued behind the first");
        drop(commands);

        serve_device(session, 9, events, command_receiver);

        // Only the first change reached the device: the connect `01 01`, its
        // `03 87`, and the `01 01` re-poll that timed out. The queued click
        // wrote nothing.
        let writes = transport.writes();
        assert_eq!(writes.len(), 3, "the queued click must not be written");
        assert_eq!(writes[0], state_request());
        assert_eq!(&writes[1][..9], &hex("08 EE 00 00 00 03 87 3F 00")[..]);
        assert_eq!(writes[1].len(), 63, "the measured 53-byte body");
        assert_eq!(writes[2], state_request());
        assert!(matches!(
            received.try_recv().expect("the state report"),
            Event::Connected { .. }
        ));
        assert!(matches!(
            received.try_recv().expect("the failed change"),
            Event::PresetChanged {
                attempt: 9,
                request: 1,
                outcome: Err(EqFailure::TimedOut)
            }
        ));
        assert!(
            received.try_recv().is_err(),
            "the worker reports nothing else and stops"
        );
    }

    /// Manual hardware check, not part of the automated suite:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_paired_connect -- --ignored --nocapture
    /// ```
    ///
    /// Enumerates the paired Soundcore candidates for the MVP models, connects
    /// to the suggested one through the same worker path the window uses (only
    /// `01 01` is written), and prints the mapped battery percentages.
    /// Addresses are masked, so the output is safe to paste into a report.
    #[test]
    #[ignore = "manual hardware check: needs a paired Soundcore device"]
    fn manual_paired_connect() {
        // A3949/A3954 are the TWS MVP models; A3062 is the single-battery model
        // of ticket 05 and maps to one whole-device battery tile.
        for code in ["A3949", "A3954", "A3062"] {
            let Some(model) = selected_model(code) else {
                continue;
            };
            println!("== {code} {} ==", model.name);
            let found = match enumerate_candidates(Some((model.code, model.name))) {
                Ok(found) => found,
                Err(error) => {
                    println!("  enumeration failed: {error}");
                    continue;
                }
            };
            if found.is_empty() {
                println!("  no paired candidate");
                continue;
            }
            for candidate in &found {
                println!(
                    "  candidate {} suggested={} ({})",
                    candidate.name,
                    candidate.suggested,
                    masked_mac(&format_mac(candidate.address))
                );
            }
            let Some(protocol) = model.protocol else {
                println!("  {code} has no state parser yet");
                continue;
            };
            let candidate = found
                .iter()
                .find(|candidate| candidate.suggested)
                .or_else(|| found.first())
                .expect("the list is not empty");
            let request = ConnectRequest {
                address: candidate.address,
                model_code: code.to_owned(),
                protocol: Some(protocol),
            };
            match open_session(&request, &AtomicBool::new(false)) {
                OpenOutcome::Opened(mut session) => match session.poll_state() {
                    Ok(state) => {
                        let shown: Vec<String> = batteries(&state)
                            .iter()
                            .map(|tile| format!("label {}={}", tile.label, tile.value))
                            .collect();
                        println!("  connected: {}", shown.join(" "));
                        // Firmware is part of the acceptance record; the serial is
                        // deliberately not printed.
                        let firmware = state
                            .single_battery
                            .as_ref()
                            .and_then(|single| single.firmware.clone())
                            .or_else(|| state.firmware.left.clone())
                            .or_else(|| state.firmware.right.clone());
                        println!("  firmware: {}", firmware.as_deref().unwrap_or("unknown"));
                    }
                    Err(error) => {
                        let failure = ConnectFailure::from_session(error);
                        println!("  failed ({:?}): {}", failure.kind(), failure.text());
                    }
                },
                OpenOutcome::Failed(failure) => {
                    println!("  failed ({:?}): {}", failure.kind(), failure.text());
                }
                OpenOutcome::Cancelled => println!("  cancelled"),
            }
        }
    }

    /// Manual hardware check of the controller's `disconnect` and `retry`
    /// callbacks, not part of the automated suite:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_disconnect_and_retry -- --ignored --nocapture
    /// ```
    ///
    /// The vendored UI has no disconnect control, so this drives the same
    /// callbacks the window's actions call, against a real paired device:
    /// connect the suggested A3062, disconnect back to My Devices, then retry
    /// the saved card and wait for the second real connection. Addresses are
    /// masked, so the output is safe to paste into a report.
    #[test]
    #[ignore = "manual hardware check: needs a paired Soundcore device"]
    fn manual_disconnect_and_retry() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        let state = ui.global::<AppState>();

        let model = selected_model("A3062").expect("A3062 has a catalog entry and a parser");
        let found = enumerate_candidates(Some((model.code, model.name))).expect("enumeration");
        let candidate = found
            .iter()
            .find(|candidate| candidate.suggested)
            .or_else(|| found.first())
            .expect("a paired A3062");
        let mac = format_mac(candidate.address);
        println!(
            "== {} {} via {} ==",
            model.code,
            model.name,
            masked_mac(&mac)
        );

        // `pair` reads the chosen model from the dialog state; set what
        // `choose_model` would set and skip its own enumeration worker.
        state.set_add_model_code(model.code.into());
        state.set_add_model_name(model.name.into());
        pair(&ui, &inner, &mac, &candidate.name);
        assert_eq!(state.get_connection(), Connection::Connecting);
        assert_eq!(state.get_page(), Page::Overview);
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the first real connection succeeds"
        );
        let tiles = shown_batteries(&state);
        println!("  connected: {tiles:?}");
        assert!(!tiles.is_empty(), "the connection shows battery tiles");

        // Disconnect returns to My Devices and forgets the current card.
        disconnect(&ui, &inner);
        assert_eq!(state.get_page(), Page::Devices);
        assert_eq!(state.get_connection(), Connection::Off);
        assert_eq!(state.get_batteries().row_count(), 0);
        let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
        assert!(cards.iter().all(|card| !card.current));
        println!(
            "  disconnect: page={:?} connection={:?}",
            state.get_page(),
            state.get_connection()
        );

        // Retry reconnects the saved card on the real device.
        retry(&ui, &inner);
        assert_eq!(state.get_connection(), Connection::Connecting);
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the retried connection succeeds"
        );
        let tiles = shown_batteries(&state);
        println!("  retry connected: {tiles:?}");
        assert!(!tiles.is_empty(), "the retry shows battery tiles");
    }

    /// The controller state of a manual test: the same fields `Controller::new`
    /// fills, with the worker channel drained by hand instead of by the timer.
    /// The profiles are loaded from a unique temporary document, so a hardware
    /// test never touches the app-data directory of the running app.
    fn test_inner(ui: &AppWindow) -> Rc<Inner> {
        test_inner_with_profiles(ui, temp_profiles_path("manual"))
    }

    /// [`test_inner`], using `path` as the saved-profile document instead of a
    /// temporary one.
    fn test_inner_with_profiles(ui: &AppWindow, path: PathBuf) -> Rc<Inner> {
        let (sender, receiver) = mpsc::channel();
        Rc::new(Inner {
            ui: ui.as_weak(),
            sender,
            receiver: RefCell::new(receiver),
            enumeration: Cell::new(0),
            connect: Cell::new(0),
            cancel: RefCell::new(None),
            commands: RefCell::new(None),
            mode_change: Cell::new(0),
            preset_change: Cell::new(0),
            curve_change: Cell::new(0),
            setting_change: Cell::new(0),
            device: RefCell::new(None),
            profiles: RefCell::new(load_profiles(&path)),
            profiles_path: Some(path),
            import_text: RefCell::new(None),
            import_io: Arc::new(NativeImportIo),
            picking_import_file: Cell::new(false),
        })
    }

    /// Connects the suggested real device of a catalog code through the same
    /// `pair` path the window uses; `None` when no device answered.
    fn connect_real_device(ui: &AppWindow, inner: &Rc<Inner>, code: &str) -> Option<String> {
        let model = selected_model(code).expect("the model is in the catalog");
        println!("== {} {} ==", model.code, model.name);
        let found = match enumerate_candidates(Some((model.code, model.name))) {
            Ok(found) => found,
            Err(error) => {
                println!("  enumeration failed: {error}");
                return None;
            }
        };
        for candidate in &found {
            println!(
                "  candidate {} suggested={} ({})",
                candidate.name,
                candidate.suggested,
                masked_mac(&format_mac(candidate.address))
            );
        }
        let candidate = found
            .iter()
            .find(|candidate| candidate.suggested)
            .or_else(|| found.first())?;
        let mac = format_mac(candidate.address);
        let state = ui.global::<AppState>();
        // `pair` reads the chosen model from the dialog state; set what
        // `choose_model` would set and skip its own enumeration worker.
        state.set_add_model_code(model.code.into());
        state.set_add_model_name(model.name.into());
        pair(ui, inner, &mac, &candidate.name);
        if !wait_for_connection(inner, ui, Connection::Connected, Duration::from_secs(20)) {
            println!(
                "  connect failed ({:?}): {}",
                state.get_failure_kind(),
                state.get_failure_text()
            );
            return None;
        }
        println!("  connected via {}", masked_mac(&mac));
        Some(mac)
    }

    /// Pumps the worker channel until the page shows `want` as the current
    /// mode. `None` when the error banner appeared first or the timeout expired.
    fn wait_for_mode(ui: &AppWindow, inner: &Rc<Inner>, want: i32) -> Option<i32> {
        let state = ui.global::<AppState>();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            drain_events(inner);
            if !state.get_error_label().is_empty() {
                println!("  error banner: {}", state.get_error_label());
                return None;
            }
            if state.get_mode_index() == want {
                return Some(want);
            }
            if std::time::Instant::now() >= deadline {
                println!(
                    "  timed out waiting for mode index {want}; current {}",
                    state.get_mode_index()
                );
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Clicks a sound-mode tile through the same function the `Actions.set-option`
    /// callback calls, then pumps the worker channel until the re-polled state
    /// selects it. `None` when the change showed an error instead.
    fn switch_mode(ui: &AppWindow, inner: &Rc<Inner>, index: i32) -> Option<i32> {
        click_mode(ui, index);
        wait_for_mode(ui, inner, index)
    }

    /// Presses a Sound mode tile through the wired `Actions.set-option` callback,
    /// as the vendored page's touch area does.
    fn click_mode(ui: &AppWindow, index: i32) {
        ui.global::<Actions>()
            .invoke_set_option("ambientSoundMode".into(), index);
    }

    /// Flips a section-page toggle through the wired `Actions.set-toggle`
    /// callback, as the vendored row's toggle does.
    fn click_toggle(ui: &AppWindow, id: &str, on: bool) {
        ui.global::<Actions>().invoke_set_toggle(id.into(), on);
    }

    /// Chooses a section-page option through the wired `Actions.set-option`
    /// callback.
    fn click_option(ui: &AppWindow, id: &str, index: i32) {
        ui.global::<Actions>().invoke_set_option(id.into(), index);
    }

    /// Releases a section-page range through the wired `Actions.set-number`
    /// callback.
    fn click_number(ui: &AppWindow, id: &str, value: i32) {
        ui.global::<Actions>().invoke_set_number(id.into(), value);
    }

    /// The settings of the controller's last re-polled device state.
    fn current_settings(inner: &Rc<Inner>) -> Option<A3062Settings> {
        inner
            .device
            .borrow()
            .as_ref()
            .and_then(|device_state| device_state.settings)
    }

    /// Pumps the worker channel until the re-polled settings match `want`.
    /// `None` when the error banner appeared first or the timeout expired.
    fn wait_for_settings(
        ui: &AppWindow,
        inner: &Rc<Inner>,
        want: impl Fn(&A3062Settings) -> bool,
        label: &str,
    ) -> Option<A3062Settings> {
        let state = ui.global::<AppState>();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            drain_events(inner);
            if !state.get_error_label().is_empty() {
                println!("  error banner: {}", state.get_error_label());
                return None;
            }
            if let Some(settings) = current_settings(inner)
                && want(&settings)
            {
                return Some(settings);
            }
            if std::time::Instant::now() >= deadline {
                println!("  timed out waiting for {label}");
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The panels as printable lines: each panel's kind and each row's whole
    /// value, for the hardware report.
    fn describe_panels(panels: &[SettingsPanel]) -> Vec<String> {
        let mut lines = Vec::new();
        for panel in panels {
            lines.push(format!("{:?}", panel.kind));
            for row in panel.rows.iter() {
                lines.push(format!(
                    "  {} {:?} checked={} index={} range={}..{} step={} value={} text={:?}",
                    row.id,
                    row.kind,
                    row.checked,
                    row.index,
                    row.minimum,
                    row.maximum,
                    row.step,
                    row.value,
                    row.text,
                ));
            }
        }
        lines
    }

    /// Manual hardware check of the Sound mode page on the real Space One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_sound_mode_switch -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, asserts the vendored page state (three
    /// tiles, labels, a current mode, no strength/wind rows, no 1-to-11 scale),
    /// then clicks every mode through the `ambientSoundMode` action and checks
    /// that the mode index follows the `01 01` re-poll after each one. Three
    /// quick clicks then check that only the last result is applied, and a
    /// stopped worker checks the readable error banner and its Retry. The mode
    /// found on connect is left in place. Addresses are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_sound_mode_switch() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        // The page's tiles call `Actions.set-option`; wire the same callbacks the
        // running app wires and press them.
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let pages: Vec<Page> = state.get_sound_pages().iter().collect();
        let modes: Vec<ModeOption> = state.get_modes().iter().collect();
        let values: Vec<String> = modes.iter().map(|mode| mode.value.to_string()).collect();
        let labels: Vec<String> = modes.iter().map(|mode| mode.label.to_string()).collect();
        println!("  sound-pages: {pages:?}");
        println!("  modes: {values:?} labels: {labels:?}");
        println!(
            "  mode-index: {} (mode-scale: {}, mode-rows: {})",
            state.get_mode_index(),
            state.get_mode_scale(),
            state.get_mode_rows().row_count()
        );
        assert_eq!(
            pages,
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
        assert_eq!(values, ["NoiseCanceling", "Transparency", "Normal"]);
        assert_eq!(labels, ["Noise canceling", "Transparency", "Normal"]);
        assert!(!state.get_mode_scale(), "the 1-to-11 scale is out of scope");
        assert_eq!(
            state.get_mode_rows().row_count(),
            0,
            "strength/wind rows are out of scope"
        );
        let initial = state.get_mode_index();
        assert!((0..3).contains(&initial), "the page reports a current mode");
        println!("  reported mode on connect: index {initial}");

        // Open the page the way the sidebar does.
        state.set_page(Page::SoundModes);
        assert_eq!(state.get_page(), Page::SoundModes);

        // Visit every other tile first and the starting mode last, so each
        // click expects a different mode than the one shown before it.
        let mut clicked = Vec::new();
        for offset in [1, 2, 0] {
            let index = (initial + offset) % 3;
            let reported = switch_mode(&ui, &inner, index)
                .unwrap_or_else(|| panic!("switching to tile {index} fails"));
            clicked.push(reported);
            println!("  tile {index} -> re-polled mode index {reported}");
            assert_eq!(reported, index);
        }
        assert_eq!(clicked.len(), 3, "all three modes were switched");

        // Three clicks before the first answer: the worker applies them in order
        // and the page shows only the last re-polled mode; older results are
        // stale and dropped.
        let first = (initial + 1) % 3;
        let second = (initial + 2) % 3;
        click_mode(&ui, first);
        click_mode(&ui, second);
        click_mode(&ui, initial);
        let settled =
            wait_for_mode(&ui, &inner, initial).expect("three quick clicks settle on the last one");
        println!("  quick clicks {first}, {second}, {initial} -> settled on {settled}");

        // A failed change closes the session. The next click shows the readable
        // error banner instead of writing, and the banner's Retry reconnects.
        inner.stop_worker();
        click_mode(&ui, first);
        let error = state.get_error_label().to_string();
        println!("  after a failed worker: error-label {error:?}");
        assert!(error.contains("Sound mode"), "{error:?} names the setting");
        // Give the stopped worker time to drop its socket: a real Retry click
        // comes much later than this line.
        std::thread::sleep(Duration::from_millis(1000));
        ui.global::<Actions>().invoke_retry();
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the banner's Retry reconnects"
        );
        println!(
            "  retry: connected={:?} sound-pages={} error-label={:?}",
            state.get_connection(),
            state.get_sound_pages().row_count(),
            state.get_error_label()
        );
        assert_eq!(state.get_sound_pages().row_count(), 3, "the pages are back");
        assert_eq!(state.get_error_label().as_str(), "", "the banner is gone");
        let reported = state.get_mode_index();
        println!("  retried connection reports mode index {reported}");
        assert_eq!(reported, initial, "the device kept the last mode");
        disconnect(&ui, &inner);
    }

    /// Manual hardware check of the remaining settings pages on the real Space
    /// One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_settings_pages_and_writes -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, reads every section page's panels and rows,
    /// checks the aside capabilities and the Connections links, then writes one
    /// toggle, one select and one range through the wired actions and verifies
    /// each against the `01 01` re-poll. Every setting found on connect is
    /// restored and the final state is compared with it. Addresses and serials
    /// are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_settings_pages_and_writes() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let Some(initial) = current_settings(&inner) else {
            println!("  the connected state carries no settings block");
            disconnect(&ui, &inner);
            return;
        };

        // The pages and their capabilities.
        let device_pages: Vec<Page> = state.get_device_pages().iter().collect();
        let sound_pages: Vec<Page> = state.get_sound_pages().iter().collect();
        println!("  device-pages: {device_pages:?}");
        println!("  sound-pages: {sound_pages:?}");
        assert_eq!(
            device_pages,
            vec![
                Page::Controls,
                Page::Comfort,
                Page::Power,
                Page::Connections,
                Page::About
            ]
        );
        assert_eq!(
            sound_pages,
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
        assert!(!state.get_has_sides(), "the A3062 has one control surface");
        assert!(
            state.get_has_ldac(),
            "the Effects page offers the LDAC note"
        );
        assert!(!state.get_has_spatial(), "the A3062 has no spatial audio");
        assert!(
            state.get_system_settings_available(),
            "the Connections links are offered"
        );

        // Every section page renders its measured rows from the connection.
        for page in [
            Page::Controls,
            Page::Comfort,
            Page::Power,
            Page::Connections,
            Page::Effects,
            Page::About,
        ] {
            ui.global::<Actions>().invoke_navigate(page);
            assert_eq!(state.get_page(), page);
            let panels: Vec<SettingsPanel> = state.get_panels().iter().collect();
            println!("  == {page:?} ==");
            for line in describe_panels(&panels) {
                println!("  {line}");
            }
            assert!(!panels.is_empty(), "{page:?} has panels");
        }
        ui.global::<Actions>().invoke_navigate(Page::Controls);
        let panels: Vec<SettingsPanel> = state.get_panels().iter().collect();
        assert_eq!(row_ids(&panels[0]), ["singlePress", "doublePress"]);
        assert_eq!(select_options(&panels[0], 1), ["None", "Bass Up"]);
        assert_eq!(panels[1].rows.row_count(), 3, "the three cycle toggles");
        ui.global::<Actions>().invoke_navigate(Page::Connections);
        let panels: Vec<SettingsPanel> = state.get_panels().iter().collect();
        assert_eq!(row_ids(&panels[0]), ["dualConnections"]);
        assert_eq!(panels.len(), 1, "no remembered-device list in this slice");
        ui.global::<Actions>().invoke_navigate(Page::About);
        let panels: Vec<SettingsPanel> = state.get_panels().iter().collect();
        let about_ids = row_ids(&panels[0]);
        println!("  About rows: {about_ids:?}");
        assert!(
            !about_ids.iter().any(|id| id == "serialNumber"),
            "the serial is never shown"
        );

        // One toggle: the side tone, flipped and restored.
        let target = !initial.side_tone;
        println!("  side tone {} -> {target}", initial.side_tone);
        click_toggle(&ui, "sideTone", target);
        let after = wait_for_settings(
            &ui,
            &inner,
            |settings| settings.side_tone == target,
            "the side tone toggle",
        )
        .expect("the re-poll reports the written side tone");
        assert_eq!(after.side_tone, target);
        click_toggle(&ui, "sideTone", initial.side_tone);
        wait_for_settings(
            &ui,
            &inner,
            |settings| settings.side_tone == initial.side_tone,
            "the side tone restore",
        )
        .expect("the re-poll reports the restored side tone");
        println!("  side tone restored to {}", initial.side_tone);

        // One select: the auto power off choice, moved to the next page index
        // and restored, duration included.
        let initial_index = auto_power_off_index(initial.auto_power_off);
        let target_index = (initial_index + 1) % 5;
        let Some(SettingCommand::AutoPowerOff(target_auto_power_off)) =
            option_command("autoPowerOff", target_index, &initial)
        else {
            panic!("the auto power off index {target_index} names no command");
        };
        println!(
            "  auto power off index {initial_index} -> {target_index} ({target_auto_power_off:?})"
        );
        click_option(&ui, "autoPowerOff", target_index);
        let after = wait_for_settings(
            &ui,
            &inner,
            |settings| settings.auto_power_off == target_auto_power_off,
            "the auto power off select",
        )
        .expect("the re-poll reports the written auto power off");
        assert_eq!(after.auto_power_off, target_auto_power_off);
        click_option(&ui, "autoPowerOff", initial_index);
        wait_for_settings(
            &ui,
            &inner,
            |settings| settings.auto_power_off == initial.auto_power_off,
            "the auto power off restore",
        )
        .expect("the re-poll reports the restored auto power off");
        println!("  auto power off restored to {:?}", initial.auto_power_off);

        // One range: the limit high volume dB limit, moved to another measured
        // step and restored.
        let initial_db = initial.limit_high_volume.db_limit.value();
        let target_db = if initial_db < 100 {
            initial_db + 5
        } else {
            initial_db - 5
        };
        let target_limit = soundcore_models::LimitHighVolume {
            db_limit: DbLimit::new(target_db).expect("a measured dB step"),
            ..initial.limit_high_volume
        };
        println!("  dB limit {initial_db} -> {target_db}");
        click_number(&ui, "limitHighVolumeDbLimit", i32::from(target_db));
        let after = wait_for_settings(
            &ui,
            &inner,
            |settings| settings.limit_high_volume == target_limit,
            "the dB limit range",
        )
        .expect("the re-poll reports the written dB limit");
        assert_eq!(after.limit_high_volume, target_limit);
        click_number(
            &ui,
            "limitHighVolumeDbLimit",
            i32::from(initial.limit_high_volume.db_limit.value()),
        );
        wait_for_settings(
            &ui,
            &inner,
            |settings| settings.limit_high_volume == initial.limit_high_volume,
            "the dB limit restore",
        )
        .expect("the re-poll reports the restored dB limit");
        println!(
            "  dB limit restored to {}",
            initial.limit_high_volume.db_limit.value()
        );

        // The About copy button copies the row text through the wired action.
        ui.global::<Actions>().invoke_copy("OpenSCL4".into());
        std::thread::sleep(Duration::from_millis(200));
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            Ok(text) if text == "OpenSCL4" => println!("  clipboard: the About copy action works"),
            other => println!("  clipboard: could not verify the copy: {other:?}"),
        }

        // Every setting found on connect is restored, field by field.
        let final_settings = current_settings(&inner).expect("the re-polled settings");
        assert_eq!(
            final_settings, initial,
            "the device is left with every setting found on connect"
        );
        println!("  final settings: {final_settings:?}");
        disconnect(&ui, &inner);
    }

    /// Manual hardware check that a P20i never offers the Sound mode page:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_p20i_has_no_sound_mode -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3949 and asserts the page, the tiles and the mode
    /// index stay empty; the `ambientSoundMode` action is then ignored, so no
    /// `06 81` write has a path on this model. Addresses are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired P20i"]
    fn manual_p20i_has_no_sound_mode() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3949").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        println!(
            "  sound-pages: {} modes: {} mode-index: {}",
            state.get_sound_pages().row_count(),
            state.get_modes().row_count(),
            state.get_mode_index()
        );
        assert_eq!(state.get_sound_pages().row_count(), 0, "no page");
        assert_eq!(state.get_modes().row_count(), 0, "no tiles");
        assert_eq!(state.get_mode_index(), -1, "no selected mode");

        click_mode(&ui, 0);
        std::thread::sleep(Duration::from_millis(300));
        drain_events(&inner);
        assert_eq!(state.get_modes().row_count(), 0);
        assert_eq!(state.get_mode_index(), -1);
        assert_eq!(
            state.get_error_label().as_str(),
            "",
            "a P20i tile click changes nothing"
        );
        println!("  ambientSoundMode click ignored: no page, no 06 81 path");
        disconnect(&ui, &inner);
    }

    /// Presses an Equalizer preset chip through the wired `Actions.eq-preset`
    /// callback, as the vendored page's chip does.
    fn click_preset(ui: &AppWindow, value: &str) {
        ui.global::<Actions>().invoke_eq_preset(value.into());
    }

    /// Drags a band through the wired `Actions.eq-preview` callback, as the
    /// vendored chart and sliders do while the pointer moves.
    fn preview_band(ui: &AppWindow, index: i32, value: i32) {
        ui.global::<Actions>().invoke_eq_preview(index, value);
    }

    /// Lets a band go through the wired `Actions.eq-commit` callback, as the
    /// vendored chart and sliders do on release.
    fn commit_band(ui: &AppWindow, index: i32, value: i32) {
        ui.global::<Actions>().invoke_eq_commit(index, value);
    }

    /// Types a band value and presses Enter through the wired `Actions.eq-type`
    /// callback.
    fn type_band(ui: &AppWindow, index: i32, text: &str) {
        ui.global::<Actions>().invoke_eq_type(index, text.into());
    }

    /// Presses the Save-profile dialog's button through the wired
    /// `Actions.eq-save` callback.
    fn click_save_profile(ui: &AppWindow, name: &str, replace: bool) {
        ui.global::<Actions>().invoke_eq_save(name.into(), replace);
    }

    /// Clicks a saved-profile row through the wired `Actions.eq-apply` callback.
    fn click_apply_profile(ui: &AppWindow, name: &str) {
        ui.global::<Actions>().invoke_eq_apply(name.into());
    }

    /// Presses the confirmation dialog's Delete button through the wired
    /// `Actions.eq-delete` callback.
    fn click_delete_profile(ui: &AppWindow, name: &str) {
        ui.global::<Actions>().invoke_eq_delete(name.into());
    }

    /// The saved-profile rows the page shows, as plain values for test output.
    fn profile_rows_of(state: &AppState<'_>) -> Vec<EqProfile> {
        state.get_eq_profiles().iter().collect()
    }

    /// The eight visible band values the page shows, in raw dB×10.
    fn band_values(state: &AppState<'_>) -> Vec<i16> {
        state
            .get_eq_bands()
            .iter()
            .map(|band| clamp_band_value(band.value))
            .collect()
    }

    /// Pumps the worker channel until the page shows `want` as the custom curve
    /// (`0xFEFE`) and its eight band values, or the error banner appears.
    fn wait_for_custom_curve(
        ui: &AppWindow,
        inner: &Rc<Inner>,
        want: [i16; VISIBLE_EQ_BANDS],
    ) -> Option<Vec<i32>> {
        let state = ui.global::<AppState>();
        let expected: Vec<i32> = want.iter().map(|value| i32::from(*value)).collect();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            drain_events(inner);
            if !state.get_error_label().is_empty() {
                println!("  error banner: {}", state.get_error_label());
                return None;
            }
            let bands: Vec<EqBand> = state.get_eq_bands().iter().collect();
            let values: Vec<i32> = bands.iter().map(|band| band.value).collect();
            if state.get_eq_preset().as_str()
                == soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_string()
                && values == expected
            {
                return Some(values);
            }
            if std::time::Instant::now() >= deadline {
                println!(
                    "  timed out waiting for the custom curve {expected:?}; current {:?} {values:?}",
                    state.get_eq_preset()
                );
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Pumps the worker channel until the page shows `want` as the current
    /// preset id. `None` when the error banner appeared first or the timeout
    /// expired.
    fn wait_for_preset(ui: &AppWindow, inner: &Rc<Inner>, want: &str) -> Option<String> {
        let state = ui.global::<AppState>();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            drain_events(inner);
            if !state.get_error_label().is_empty() {
                println!("  error banner: {}", state.get_error_label());
                return None;
            }
            if state.get_eq_preset().as_str() == want {
                return Some(want.to_owned());
            }
            if std::time::Instant::now() >= deadline {
                println!(
                    "  timed out waiting for preset {want}; current {:?}",
                    state.get_eq_preset()
                );
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Manual hardware check of the Equalizer page on the real Space One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_equalizer_preset_switch -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, asserts the vendored page state (22 preset
    /// chips, the current preset selected, eight editable bands with their
    /// frequencies and values, the -12.0…+13.4 dB range), then clicks three
    /// different presets through the `eq-preset` action and checks that the
    /// selection and the bands follow the `01 01` re-poll after each one. The
    /// preset found on connect is restored, a stopped worker checks the readable
    /// error banner and its Retry, and the device is left as found. Addresses
    /// are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_equalizer_preset_switch() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let model = soundcore_models::model_by_code("A3062").expect("A3062 has a parser");
        let presets: Vec<ModeOption> = state.get_eq_presets().iter().collect();
        let values: Vec<String> = presets.iter().map(|p| p.value.to_string()).collect();
        let labels: Vec<String> = presets.iter().map(|p| p.label.to_string()).collect();
        let bands: Vec<EqBand> = state.get_eq_bands().iter().collect();
        let frequencies: Vec<String> = bands.iter().map(|b| b.frequency.to_string()).collect();
        let band_values: Vec<i32> = bands.iter().map(|b| b.value).collect();
        let pages: Vec<Page> = state.get_sound_pages().iter().collect();
        println!("  sound-pages: {pages:?}");
        println!(
            "  eq-available: {} eq-read-only: {}",
            state.get_eq_available(),
            state.get_eq_read_only()
        );
        println!(
            "  eq-presets: {} ids {values:?}",
            state.get_eq_presets().row_count()
        );
        println!(
            "  eq-preset: {:?} eq-name: {:?}",
            state.get_eq_preset(),
            state.get_eq_name()
        );
        println!(
            "  eq-bands: {} frequencies {frequencies:?} values {band_values:?}",
            bands.len()
        );
        println!(
            "  eq-range: {}..{} text {:?}..{:?} band-count {}",
            state.get_eq_min(),
            state.get_eq_max(),
            state.get_eq_min_text(),
            state.get_eq_max_text(),
            state.get_eq_band_count()
        );
        assert_eq!(
            pages,
            vec![Page::SoundModes, Page::Equalizer, Page::Effects]
        );
        assert!(state.get_eq_available(), "the page is available");
        assert!(
            !state.get_eq_read_only(),
            "the A3062 band controls are editable"
        );
        assert_eq!(values.len(), 22, "the catalog is the 22-entry list");
        assert_eq!(labels.len(), 22);
        assert_eq!(bands.len(), 8, "eight visible bands");
        assert_eq!(
            frequencies,
            ["100", "200", "400", "800", "1.6", "3.2", "6.4", "12.8"]
        );
        assert_eq!(state.get_eq_band_count(), 8);
        assert_eq!(state.get_eq_min(), -120);
        assert_eq!(state.get_eq_max(), 134);
        assert_eq!(state.get_eq_min_text().as_str(), "\u{2212}12.0");
        assert_eq!(state.get_eq_max_text().as_str(), "+13.4");
        assert!(state.get_eq_lines().row_count() > 0, "the chart has guides");
        assert!(
            !state.get_eq_chart_line().is_empty(),
            "the chart has a curve"
        );
        let initial = state.get_eq_preset().to_string();
        let Some(initial_id) = initial.parse::<u16>().ok() else {
            println!("  current preset {initial:?} is not a preset id; leaving the device as is");
            disconnect(&ui, &inner);
            return;
        };
        let Some(initial_preset) = model
            .equalizer_presets()
            .iter()
            .find(|preset| preset.id == initial_id)
        else {
            // A custom curve has no catalog entry and cannot be restored through
            // the preset chips, so do not change it.
            println!("  current preset id {initial_id} is custom; leaving the device as is");
            disconnect(&ui, &inner);
            return;
        };
        assert_eq!(state.get_eq_name().as_str(), initial_preset.name);

        // Open the page the way the sidebar does.
        state.set_page(Page::Equalizer);
        assert_eq!(state.get_page(), Page::Equalizer);

        // Switch between three presets, checking the selection and the curve
        // follow the device after each write.
        let targets: Vec<&soundcore_models::EqualizerPreset> = model
            .equalizer_presets()
            .iter()
            .filter(|preset| preset.id != initial_id)
            .take(3)
            .collect();
        assert_eq!(targets.len(), 3, "three presets to switch between");
        for target in &targets {
            click_preset(&ui, &target.id.to_string());
            let reported = wait_for_preset(&ui, &inner, &target.id.to_string())
                .unwrap_or_else(|| panic!("switching to {} fails", target.name));
            let bands: Vec<EqBand> = state.get_eq_bands().iter().collect();
            let values: Vec<i32> = bands.iter().map(|band| band.value).collect();
            println!(
                "  preset {} ({}) -> re-polled {:?} bands {values:?}",
                target.id, target.name, reported
            );
            assert_eq!(reported, target.id.to_string());
            assert_eq!(state.get_eq_name().as_str(), target.name);
            assert_eq!(
                values,
                target.bands[..8]
                    .iter()
                    .map(|v| i32::from(*v))
                    .collect::<Vec<_>>()
            );
        }

        // Put the preset found on connect back.
        click_preset(&ui, &initial);
        let restored =
            wait_for_preset(&ui, &inner, &initial).expect("the original preset is restored");
        let bands: Vec<EqBand> = state.get_eq_bands().iter().collect();
        let values: Vec<i32> = bands.iter().map(|band| band.value).collect();
        println!("  restored preset {restored:?} bands {values:?}");
        assert_eq!(state.get_eq_name().as_str(), initial_preset.name);
        assert_eq!(
            values,
            initial_preset.bands[..8]
                .iter()
                .map(|v| i32::from(*v))
                .collect::<Vec<_>>()
        );

        // A stopped worker shows the readable error banner instead of writing;
        // the banner's Retry reconnects and the device kept the restored preset.
        inner.stop_worker();
        let other = targets[0].id.to_string();
        click_preset(&ui, &other);
        let error = state.get_error_label().to_string();
        println!("  after a stopped worker: error-label {error:?}");
        assert!(error.contains("Equalizer"), "{error:?} names the setting");
        // Give the stopped worker time to drop its socket: a real Retry click
        // comes much later than this line.
        std::thread::sleep(Duration::from_millis(1000));
        ui.global::<Actions>().invoke_retry();
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the banner's Retry reconnects"
        );
        println!(
            "  retry: connected={:?} sound-pages={} eq-preset={:?} error-label={:?}",
            state.get_connection(),
            state.get_sound_pages().row_count(),
            state.get_eq_preset(),
            state.get_error_label()
        );
        assert_eq!(state.get_sound_pages().row_count(), 3, "the pages are back");
        assert_eq!(state.get_error_label().as_str(), "", "the banner is gone");
        assert_eq!(
            state.get_eq_preset().as_str(),
            initial,
            "the device kept the restored preset"
        );
        disconnect(&ui, &inner);
    }

    /// Manual hardware check of the editable equalizer bands on the real Space
    /// One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_equalizer_custom_curve -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, asserts the band controls are editable,
    /// types one band and drags another, commits both and checks the re-polled
    /// custom state (`0xFEFE`, empty name) reports the written curve, resets to
    /// flat, switches to a built-in preset and restores the state found on
    /// connect. A stopped worker checks the readable error banner and its Retry.
    /// Addresses are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_equalizer_custom_curve() {
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner(&ui);
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let model = soundcore_models::model_by_code("A3062").expect("A3062 has a parser");

        // Record the state found on connect so the device can be left as found.
        let initial_preset = state.get_eq_preset().to_string();
        let initial_name = state.get_eq_name().to_string();
        let initial_bands: Vec<i16> = state
            .get_eq_bands()
            .iter()
            .map(|band| clamp_band_value(band.value))
            .collect();
        let initial_values: Vec<i32> = initial_bands
            .iter()
            .map(|value| i32::from(*value))
            .collect();
        println!(
            "  eq-available: {} eq-read-only: {} bands: {} presets: {}",
            state.get_eq_available(),
            state.get_eq_read_only(),
            state.get_eq_bands().row_count(),
            state.get_eq_presets().row_count()
        );
        println!(
            "  found: preset {:?} name {:?} bands {initial_values:?}",
            initial_preset, initial_name
        );
        assert!(state.get_eq_available(), "the Equalizer page is available");
        assert!(
            !state.get_eq_read_only(),
            "the A3062 band controls are editable"
        );
        assert_eq!(state.get_eq_bands().row_count(), 8, "eight visible bands");
        assert_eq!(
            state.get_eq_presets().row_count(),
            22,
            "the preset chips stay"
        );
        assert_eq!(state.get_eq_band_count(), 8);
        assert_eq!(state.get_eq_min(), i32::from(EQ_MIN));
        assert_eq!(state.get_eq_max(), i32::from(EQ_MAX));
        assert!(
            !state.get_eq_chart_line().is_empty(),
            "the chart has a curve"
        );
        assert_eq!(
            initial_bands.len(),
            VISIBLE_EQ_BANDS,
            "the page shows every visible band"
        );

        // Open the page the way the sidebar does.
        state.set_page(Page::Equalizer);
        assert_eq!(state.get_page(), Page::Equalizer);

        // A typed value commits through the same path as a dragged one: the
        // first band goes to +4.0 dB and the re-poll reports a custom curve.
        type_band(&ui, 0, "4.0");
        let mut typed: [i16; VISIBLE_EQ_BANDS] =
            initial_bands.clone().try_into().expect("eight bands");
        typed[0] = 40;
        let reported =
            wait_for_custom_curve(&ui, &inner, typed).expect("the typed band is re-polled");
        println!("  eq-type band 0 4.0 -> re-polled custom curve {reported:?}");
        assert_eq!(
            state.get_eq_name().as_str(),
            "",
            "custom bands have no built-in name"
        );
        assert_eq!(
            state.get_eq_preset().as_str(),
            soundcore_models::CUSTOM_EQUALIZER_PRESET_ID.to_string(),
            "the device reports the custom preset id"
        );

        // A drag previews locally only; the release commits the whole curve, so
        // the second band goes to -4.0 dB while the first keeps its value.
        let chart_before_preview = state.get_eq_chart_line().to_string();
        preview_band(&ui, 3, -40);
        let mut dragged = typed;
        dragged[3] = -40;
        let dragged_values: Vec<i32> = dragged.iter().map(|value| i32::from(*value)).collect();
        let previewed: Vec<i32> = state.get_eq_bands().iter().map(|band| band.value).collect();
        println!("  eq-preview band 3 -4.0 -> page curve {previewed:?}");
        assert_eq!(
            previewed, dragged_values,
            "the page follows the drag at once"
        );
        assert_ne!(
            state.get_eq_chart_line().as_str(),
            chart_before_preview.as_str(),
            "the chart follows the drag"
        );
        commit_band(&ui, 3, -40);
        let reported =
            wait_for_custom_curve(&ui, &inner, dragged).expect("the dragged band is re-polled");
        println!(
            "  eq-commit band 3 -4.0 -> re-polled custom curve {reported:?} preset {:?}",
            state.get_eq_preset()
        );

        // Reset writes the flat custom curve.
        ui.global::<Actions>().invoke_eq_reset();
        let reset = wait_for_custom_curve(&ui, &inner, [0; VISIBLE_EQ_BANDS])
            .expect("the flat curve is re-polled");
        println!("  eq-reset -> re-polled flat custom curve {reset:?}");

        // A built-in preset still works after a custom curve and switches the
        // device back.
        let target = model
            .equalizer_presets()
            .iter()
            .find(|preset| preset.id.to_string() != initial_preset)
            .expect("a preset other than the current one");
        click_preset(&ui, &target.id.to_string());
        wait_for_preset(&ui, &inner, &target.id.to_string())
            .unwrap_or_else(|| panic!("switching to {} fails", target.name));
        let target_values: Vec<i32> = state.get_eq_bands().iter().map(|band| band.value).collect();
        println!(
            "  preset {} ({}) after custom -> bands {target_values:?} name {:?}",
            target.id,
            target.name,
            state.get_eq_name()
        );
        assert_eq!(state.get_eq_name().as_str(), target.name);
        assert_eq!(
            target_values,
            target
                .bands
                .iter()
                .take(VISIBLE_EQ_BANDS)
                .map(|value| i32::from(*value))
                .collect::<Vec<_>>()
        );

        // Put back what was found on connect: a built-in preset by clicking its
        // chip, or the eight custom bands through the same band path.
        let initial_is_built_in = initial_preset.parse::<u16>().ok().is_some_and(|id| {
            model
                .equalizer_presets()
                .iter()
                .any(|preset| preset.id == id)
        });
        if initial_is_built_in {
            click_preset(&ui, &initial_preset);
            wait_for_preset(&ui, &inner, &initial_preset)
                .expect("the preset found on connect is restored");
            let restored: Vec<i32> = state.get_eq_bands().iter().map(|band| band.value).collect();
            println!("  restored preset {:?} bands {restored:?}", initial_preset);
            assert_eq!(state.get_eq_name().as_str(), initial_name);
            assert_eq!(restored, initial_values);
        } else {
            for (index, value) in initial_bands.iter().enumerate() {
                preview_band(&ui, index as i32, i32::from(*value));
            }
            commit_band(&ui, 0, i32::from(initial_bands[0]));
            let restored: [i16; VISIBLE_EQ_BANDS] =
                initial_bands.clone().try_into().expect("eight bands");
            let reported = wait_for_custom_curve(&ui, &inner, restored)
                .expect("the custom curve found on connect is restored");
            println!("  restored custom curve {reported:?}");
        }

        // A stopped worker shows the readable error banner instead of writing;
        // the banner's Retry reconnects and reads the restored state back.
        inner.stop_worker();
        commit_band(&ui, 0, 0);
        let error = state.get_error_label().to_string();
        println!("  after a stopped worker: error-label {error:?}");
        assert!(
            error.contains("Equalizer bands"),
            "{error:?} names the bands"
        );
        // Give the stopped worker time to drop its socket: a real Retry click
        // comes much later than this line.
        std::thread::sleep(Duration::from_millis(1000));
        ui.global::<Actions>().invoke_retry();
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the banner's Retry reconnects"
        );
        assert_eq!(state.get_sound_pages().row_count(), 3, "the pages are back");
        assert_eq!(state.get_error_label().as_str(), "", "the banner is gone");
        assert_eq!(
            state.get_eq_preset().as_str(),
            initial_preset,
            "the device kept the restored state"
        );
        disconnect(&ui, &inner);
    }

    /// Manual hardware check of the saved equalizer profiles on the real Space
    /// One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_saved_profiles -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, asserts the page offers saved profiles and
    /// that export/legacy stay inert, writes a custom curve, saves it as “Test
    /// curve”, checks its row (curve path, eight bands, Applied tag) and the
    /// real document in `%APPDATA%\OpenSCL4\equalizer-profiles.json`, loads that
    /// document through a fresh `Controller::new` (the startup path a restarted
    /// app runs), changes the curve so Applied clears, applies the profile so
    /// one custom write is re-polled and Applied returns, checks the empty-name
    /// and duplicate-name rejections, deletes it after the confirmation state,
    /// checks that a save whose document cannot be written reports the page
    /// banner without adding a profile, and leaves both the device and the user's
    /// profile document as found. Addresses are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_saved_profiles() {
        let path = default_profiles_path().expect("Windows has %APPDATA%");
        // A failed assertion must not leave the user's real profiles changed.
        let _restore = RestoreDocument::capture(&path);
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner_with_profiles(&ui, path.clone());
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let model = soundcore_models::model_by_code("A3062").expect("A3062 has a parser");

        // Record the state found on connect so the device can be left as found.
        let initial_preset = state.get_eq_preset().to_string();
        let initial_name = state.get_eq_name().to_string();
        let initial_bands: [i16; VISIBLE_EQ_BANDS] = band_values(&state)
            .try_into()
            .expect("the A3062 page shows eight bands");
        println!(
            "  found: preset {initial_preset:?} name {initial_name:?} bands {initial_bands:?}"
        );
        assert!(
            state.get_has_saved_profiles(),
            "the A3062 page offers saved profiles"
        );
        assert!(
            profile_rows_of(&state).is_empty(),
            "the run starts from an empty document"
        );
        assert_eq!(state.get_eq_export_count(), 0, "export stays inert");
        assert_eq!(state.get_eq_legacy().row_count(), 0, "legacy stays empty");
        assert_eq!(
            state.get_eq_profiles().row_count(),
            0,
            "no saved-profile row is listed before the first save"
        );

        // Write a custom curve to save: +4.0 dB at 100 Hz and −4.0 dB at 800 Hz
        // on top of the bands found on connect.
        let mut curve = initial_bands;
        curve[0] = 40;
        type_band(&ui, 0, "4.0");
        wait_for_custom_curve(&ui, &inner, curve).expect("the typed band is re-polled");
        curve[3] = -40;
        commit_band(&ui, 3, -40);
        wait_for_custom_curve(&ui, &inner, curve).expect("the committed band is re-polled");
        println!("  custom curve {curve:?}");

        // Save it as "Test curve": the row shows the name, the curve path, the
        // eight band count and the Applied tag, and the dialog closes.
        click_save_profile(&ui, "Test curve", false);
        assert_eq!(
            state.get_eq_save_problem(),
            SaveProblem::None.value(),
            "a valid new name saves without a problem"
        );
        assert!(!state.get_eq_saving(), "the dialog closes after the save");
        assert_eq!(state.get_eq_save_existing().as_str(), "");
        let rows = profile_rows_of(&state);
        assert_eq!(rows.len(), 1, "the saved profile is listed");
        assert_eq!(rows[0].name.as_str(), "Test curve");
        assert_eq!(rows[0].bands, VISIBLE_EQ_BANDS as i32, "bands=8");
        assert!(rows[0].in_effect, "the saved curve is the current curve");
        assert!(!rows[0].curve.is_empty(), "the row carries a curve path");
        assert!(rows[0].curve.starts_with("M "), "the curve is a path");
        let document = load_profiles(&path);
        assert!(
            document.contains("Test curve"),
            "the profile is persisted to the app-data document"
        );
        assert_eq!(
            document.get("Test curve").expect("the saved profile").bands,
            curve
        );
        println!(
            "  saved {:?}: rows {} bands {} applied {}; document has {} profile(s)",
            rows[0].name,
            rows.len(),
            rows[0].bands,
            rows[0].in_effect,
            document.len()
        );

        // A restarted app runs `Controller::new`, which loads this document
        // before the first device connects: the fresh shell already lists it.
        let restarted = AppWindow::new().expect("a second shell window builds");
        {
            let _controller = Controller::new(&restarted);
            let reloaded: Vec<EqProfile> = restarted
                .global::<AppState>()
                .get_eq_profiles()
                .iter()
                .collect();
            assert_eq!(reloaded.len(), 1, "the restart loads the saved profile");
            assert_eq!(reloaded[0].name.as_str(), "Test curve");
            println!(
                "  restart: Controller::new loaded {} profile(s), first {:?}",
                reloaded.len(),
                reloaded[0].name
            );
        }
        drop(restarted);

        // The empty-name and duplicate-name answers of the save dialog.
        click_save_profile(&ui, "   ", false);
        assert_eq!(
            state.get_eq_save_problem(),
            SaveProblem::EmptyName.value(),
            "a whitespace name is rejected"
        );
        click_save_profile(&ui, "Test curve", false);
        assert_eq!(
            state.get_eq_save_problem(),
            SaveProblem::NeedsReplace.value(),
            "a duplicate name asks before replacing"
        );
        assert_eq!(state.get_eq_save_existing().as_str(), "Test curve");
        assert_eq!(
            profile_rows_of(&state).len(),
            1,
            "nothing is added before the replace answer"
        );
        println!("  empty name -> problem 1; duplicate name -> problem 2");

        // Changing the curve clears Applied; replacing the profile's bands with
        // the new curve brings it back.
        let mut replaced = curve;
        replaced[1] = 20;
        type_band(&ui, 1, "2.0");
        wait_for_custom_curve(&ui, &inner, replaced).expect("the typed band is re-polled");
        assert!(
            !profile_rows_of(&state)[0].in_effect,
            "a changed curve clears the Applied tag"
        );
        click_save_profile(&ui, "Test curve", true);
        assert_eq!(state.get_eq_save_problem(), SaveProblem::None.value());
        assert!(!state.get_eq_saving(), "the replace closes the dialog");
        let rows = profile_rows_of(&state);
        assert_eq!(rows.len(), 1, "replacing keeps one profile of the name");
        assert!(rows[0].in_effect, "the replaced bands are applied");
        assert_eq!(
            load_profiles(&path)
                .get("Test curve")
                .expect("the replaced profile")
                .bands,
            replaced
        );
        println!("  replaced bands {replaced:?}; Applied is back, one profile remains");

        // Applying the profile writes its curve on the real device and the
        // re-polled state marks it Applied again.
        click_apply_profile(&ui, "Test curve");
        wait_for_custom_curve(&ui, &inner, replaced).expect("the applied curve is re-polled");
        let rows = profile_rows_of(&state);
        assert!(
            rows[0].in_effect,
            "the applied profile is marked Applied after the re-poll"
        );
        println!(
            "  applied: re-polled bands {:?} preset {:?} applied {}",
            band_values(&state),
            state.get_eq_preset(),
            rows[0].in_effect
        );

        // A stopped worker turns an apply into the readable error banner; the
        // banner's Retry reconnects and re-reads the device.
        inner.stop_worker();
        click_apply_profile(&ui, "Test curve");
        let error = state.get_error_label().to_string();
        println!("  after a stopped worker: error-label {error:?}");
        assert!(
            error.contains("Equalizer bands"),
            "{error:?} names the bands"
        );
        std::thread::sleep(Duration::from_millis(1000));
        ui.global::<Actions>().invoke_retry();
        assert!(
            wait_for_connection(&inner, &ui, Connection::Connected, Duration::from_secs(20)),
            "the banner's Retry reconnects"
        );
        assert_eq!(state.get_error_label().as_str(), "", "the banner is gone");

        // Delete after the confirmation state: the row and the document lose it.
        state.set_eq_deleting("Test curve".into());
        assert_eq!(state.get_eq_deleting().as_str(), "Test curve");
        click_delete_profile(&ui, state.get_eq_deleting().as_str());
        assert_eq!(state.get_eq_deleting().as_str(), "", "the dialog is closed");
        assert!(
            profile_rows_of(&state).is_empty(),
            "the deleted profile is no longer listed"
        );
        let document = load_profiles(&path);
        assert!(
            document.is_empty(),
            "the deleted profile is gone from the document"
        );
        println!(
            "  deleted: rows 0; document has {} profile(s)",
            document.len()
        );

        // A save whose document cannot be written is reported in the page banner
        // and leaves the saved set alone. The blocked path is a temporary one, so
        // the user's real document is never touched by this check.
        let blocked_directory = TempDir::new("blocked-save");
        let blocker = blocked_directory.path().join(APP_DATA_DIR_NAME);
        fs::write(&blocker, b"not a directory").expect("the blocking file is written");
        let blocked = test_inner_with_profiles(&ui, blocker.join(PROFILES_FILE_NAME));
        save_equalizer_profile(&ui, &blocked, "Not saved", false);
        let error = state.get_error_label().to_string();
        println!("  save to a blocked document: error-label {error:?}");
        assert_eq!(error, PROFILE_WRITE_FAILURE_LABEL, "the banner is readable");
        assert!(!state.get_eq_saving(), "the save dialog closes");
        assert!(
            blocked.profiles.borrow().is_empty(),
            "the unsaved profile is rolled back out of the set"
        );
        assert!(
            profile_rows_of(&state).is_empty(),
            "the page lists no profile that the document does not have"
        );
        assert!(
            load_profiles(&path).is_empty(),
            "the user's document is unchanged by the failed save"
        );
        state.set_error_label("".into());

        // Put the state found on connect back, so the device is left as found.
        let initial_is_built_in = initial_preset.parse::<u16>().ok().is_some_and(|id| {
            model
                .equalizer_presets()
                .iter()
                .any(|preset| preset.id == id)
        });
        if initial_is_built_in {
            click_preset(&ui, &initial_preset);
            wait_for_preset(&ui, &inner, &initial_preset)
                .expect("the preset found on connect is restored");
            println!(
                "  restored preset {initial_preset:?} bands {:?}",
                band_values(&state)
            );
        } else {
            for (index, value) in initial_bands.iter().enumerate() {
                preview_band(&ui, index as i32, i32::from(*value));
            }
            commit_band(&ui, 0, i32::from(initial_bands[0]));
            let restored = wait_for_custom_curve(&ui, &inner, initial_bands)
                .expect("the found curve is restored");
            println!("  restored custom curve {restored:?}");
        }
        disconnect(&ui, &inner);
    }

    /// Manual hardware check of the Import dialog on the real Space One Pro:
    ///
    /// ```text
    /// cargo test -p soundcore-desktop manual_import_profiles -- --ignored --nocapture
    /// ```
    ///
    /// Connects the suggested A3062, imports a profile through the real Windows
    /// clipboard and the wired `eq-import-paste`/`eq-import` actions, applies it
    /// and checks the re-polled custom curve, imports a second profile from a
    /// real file through the file-dialog seam, refuses a blocked paste without
    /// changing the document or the list, deletes both imported profiles through
    /// the confirmation path, and leaves the device and the user's profile
    /// document as found. Addresses are masked.
    #[test]
    #[ignore = "manual hardware check: needs a paired A3062"]
    fn manual_import_profiles() {
        let path = default_profiles_path().expect("Windows has %APPDATA%");
        // A failed assertion must not leave the user's real profiles changed.
        let _restore = RestoreDocument::capture(&path);
        let ui = AppWindow::new().expect("the shell window builds");
        let inner = test_inner_with_profiles(&ui, path.clone());
        wire_actions(&ui, &inner);
        if connect_real_device(&ui, &inner, "A3062").is_none() {
            return;
        }
        let state = ui.global::<AppState>();
        let model = soundcore_models::model_by_code("A3062").expect("A3062 has a parser");

        // Record the state found on connect so the device can be left as found.
        let initial_preset = state.get_eq_preset().to_string();
        let initial_name = state.get_eq_name().to_string();
        let initial_bands: [i16; VISIBLE_EQ_BANDS] = band_values(&state)
            .try_into()
            .expect("the A3062 page shows eight bands");
        assert!(
            state.get_has_saved_profiles(),
            "the A3062 page shows the Import panel"
        );
        assert!(
            profile_rows_of(&state).is_empty(),
            "the run starts from an empty document"
        );
        assert_eq!(state.get_eq_export_count(), 0, "export stays inert");
        assert_eq!(state.get_eq_legacy().row_count(), 0, "legacy stays empty");
        println!(
            "  found: preset {initial_preset:?} name {initial_name:?} bands {initial_bands:?}"
        );

        // Paste a real OpenSCL4 transfer profile through the Windows clipboard and
        // the wired actions.
        set_clipboard_text(WARM_IMPORT).expect("the clipboard takes the import text");
        ui.global::<Actions>().invoke_eq_import_paste();
        let items: Vec<ImportItem> = state.get_import_items().iter().collect();
        let statuses: Vec<i32> = items.iter().map(|item| item.status).collect();
        let names: Vec<String> = items.iter().map(|item| item.name.to_string()).collect();
        println!(
            "  paste: importing {} blocked {} items {names:?} statuses {statuses:?}",
            state.get_importing(),
            state.get_import_blocked()
        );
        assert!(state.get_importing(), "pasting a profile opens the dialog");
        assert!(!state.get_import_blocked(), "a valid profile is importable");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name.as_str(), "Warm");
        assert_eq!(items[0].status, 0, "the profile is new");
        assert_eq!(items[0].bands, 8, "the row carries the eight-band count");
        assert!(!items[0].curve.is_empty(), "the row carries the curve");
        assert_eq!(state.get_transfer_note(), TransferNote::None);

        ui.global::<Actions>().invoke_eq_import();
        assert!(!state.get_importing(), "the dialog closes after the import");
        assert_eq!(state.get_transfer_note(), TransferNote::Imported);
        assert_eq!(state.get_transfer_count(), 1);
        assert_eq!(state.get_toast(), ToastKind::Imported);
        let rows = profile_rows_of(&state);
        assert_eq!(rows.len(), 1, "the imported profile is listed");
        assert_eq!(rows[0].name.as_str(), "Warm");
        assert!(
            !rows[0].in_effect,
            "the imported curve is not on the device yet"
        );
        let document = load_profiles(&path);
        assert_eq!(
            document
                .get("Warm")
                .expect("the imported profile is saved")
                .bands,
            WARM_IMPORT_BANDS
        );
        println!(
            "  imported {:?}: rows {} document {} profile(s), toast {:?}",
            rows[0].name,
            rows.len(),
            document.len(),
            state.get_toast()
        );

        // Applying the imported profile writes its curve on the real device.
        click_apply_profile(&ui, "Warm");
        let applied = wait_for_custom_curve(&ui, &inner, WARM_IMPORT_BANDS)
            .expect("the imported curve is written and re-polled");
        let rows = profile_rows_of(&state);
        assert!(
            rows[0].in_effect,
            "the applied imported profile is marked Applied"
        );
        println!("  applied: re-polled bands {applied:?}");

        // Import a second profile from a real file. The native file dialog cannot
        // be driven from a test, so the seam supplies the picked path while the
        // real reader loads the file.
        let directory = TempDir::new("manual-import-file");
        let file = directory.path().join("openscq30-equalizer-profiles.json");
        let file_text = r#"[{"name": "File curve", "volumeAdjustments": [0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}]"#;
        fs::write(&file, file_text).expect("the import file is written");
        {
            let mut import_text = inner.import_text.borrow_mut();
            import_from_file(
                &state,
                &inner.profiles.borrow(),
                &mut import_text,
                &RecordedImportIo::picking(file.clone()),
            );
        }
        let items: Vec<ImportItem> = state.get_import_items().iter().collect();
        let names: Vec<String> = items.iter().map(|item| item.name.to_string()).collect();
        println!(
            "  file: importing {} items {names:?}",
            state.get_importing()
        );
        assert!(state.get_importing(), "the picked file opens the dialog");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name.as_str(), "File curve");
        ui.global::<Actions>().invoke_eq_import();
        assert_eq!(state.get_transfer_note(), TransferNote::Imported);
        assert_eq!(state.get_transfer_count(), 1);
        let rows = profile_rows_of(&state);
        assert_eq!(rows.len(), 2, "both imported profiles are listed");
        let document = load_profiles(&path);
        assert!(document.contains("File curve"));
        assert_eq!(document.len(), 2);
        println!(
            "  imported from file: rows {} document {} profile(s)",
            rows.len(),
            document.len()
        );

        // A blocked import writes nothing: the dialog keeps its red note, the
        // list and the document stay as they were.
        let blocked_text = r#"[
            {"name": "Seven bands", "volumeAdjustments": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]},
            {"name": "Too loud", "volumeAdjustments": [13.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}
        ]"#;
        set_clipboard_text(blocked_text).expect("the clipboard takes the blocked text");
        ui.global::<Actions>().invoke_eq_import_paste();
        let items: Vec<ImportItem> = state.get_import_items().iter().collect();
        let statuses: Vec<i32> = items.iter().map(|item| item.status).collect();
        println!(
            "  blocked paste: importing {} blocked {} statuses {statuses:?}",
            state.get_importing(),
            state.get_import_blocked()
        );
        assert!(state.get_importing());
        assert!(
            state.get_import_blocked(),
            "the invalid rows block the file"
        );
        assert_eq!(statuses, [2, 3]);
        let document_before = fs::read_to_string(&path).expect("the document is readable");
        let rows_before = profile_rows_of(&state).len();
        ui.global::<Actions>().invoke_eq_import();
        assert!(
            state.get_importing(),
            "a blocked import leaves the dialog open"
        );
        assert_eq!(
            profile_rows_of(&state).len(),
            rows_before,
            "a blocked import adds no profile"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("the document is readable"),
            document_before,
            "a blocked import writes nothing"
        );
        println!("  blocked import wrote nothing; the dialog stays open with its red note");
        state.set_importing(false);
        *inner.import_text.borrow_mut() = None;

        // Delete both imported profiles after the confirmation state.
        for name in ["File curve", "Warm"] {
            state.set_eq_deleting(name.into());
            click_delete_profile(&ui, name);
            assert_eq!(
                state.get_eq_deleting().as_str(),
                "",
                "the delete dialog closes"
            );
            assert!(
                !profile_rows_of(&state)
                    .iter()
                    .any(|row| row.name.as_str() == name),
                "{name:?} is no longer listed"
            );
        }
        assert!(
            profile_rows_of(&state).is_empty(),
            "the imported profiles are all deleted"
        );
        assert!(load_profiles(&path).is_empty());
        println!("  deleted both imported profiles; the document is empty again");

        // Put the state found on connect back, so the device is left as found.
        let initial_is_built_in = initial_preset.parse::<u16>().ok().is_some_and(|id| {
            model
                .equalizer_presets()
                .iter()
                .any(|preset| preset.id == id)
        });
        if initial_is_built_in {
            click_preset(&ui, &initial_preset);
            wait_for_preset(&ui, &inner, &initial_preset)
                .expect("the preset found on connect is restored");
            println!(
                "  restored preset {initial_preset:?} bands {:?}",
                band_values(&state)
            );
        } else {
            for (index, value) in initial_bands.iter().enumerate() {
                preview_band(&ui, index as i32, i32::from(*value));
            }
            commit_band(&ui, 0, i32::from(initial_bands[0]));
            let restored = wait_for_custom_curve(&ui, &inner, initial_bands)
                .expect("the found curve is restored");
            println!("  restored custom curve {restored:?}");
        }
        disconnect(&ui, &inner);
    }

    /// The battery tiles of an Overview, as plain values for test output.
    fn shown_batteries(state: &AppState<'_>) -> Vec<(i32, String)> {
        state
            .get_batteries()
            .iter()
            .map(|tile| (tile.label, tile.value.to_string()))
            .collect()
    }

    /// Pumps the worker channel until the window reaches `want`, or the timeout.
    fn wait_for_connection(
        inner: &Rc<Inner>,
        ui: &AppWindow,
        want: Connection,
        timeout: Duration,
    ) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            drain_events(inner);
            if ui.global::<AppState>().get_connection() == want {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A Bluetooth address for test output. Every octet is replaced: no part of
    /// a real address may reach a log, a terminal transcript or a ticket.
    fn masked_mac(_mac: &str) -> String {
        "<address masked>".to_owned()
    }
}
