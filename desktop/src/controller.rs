//! The desktop controller: wires the vendored UI to the Windows Bluetooth
//! transport and the clean-room session.
//!
//! The controller keeps every `AppWindow` access on the Slint thread. Bluetooth
//! work happens on short-lived worker threads that report back over one `mpsc`
//! channel; a [`slint::Timer`] drains that channel on the UI thread. Nothing
//! here waits for Bluetooth on the UI thread.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};
use soundcore_models::{DeviceModel, DeviceState, SoundMode};
use soundcore_session::{DeviceSession, SessionError, Transport};
use windows::core::GUID;

use crate::catalog_adapter::{self, Chip};
use crate::transport_windows::{
    BluetoothError, VENDOR_SERVICE_PREFIX, WindowsRfcommTransport, format_mac, installed_services,
    list_paired_devices, parse_mac, pick_service, vendor_service,
};
use crate::{
    Actions, AppState, AppWindow, Battery, Connection, DeviceCard, FailureKind, ModeOption, Page,
    PairedOption,
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

/// The sidebar pages of a parsed state: Sound mode only when the device reports
/// a mode, so a P20i never shows the page.
pub fn sound_pages(sound_mode: Option<SoundMode>) -> Vec<Page> {
    if sound_mode.is_some() {
        vec![Page::SoundModes]
    } else {
        Vec::new()
    }
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
    /// The model has no measured sound-mode control, so nothing was written.
    Unsupported,
}

impl ModeFailure {
    /// Reduces a session failure to the case the page has words for.
    pub fn from_session(error: SessionError) -> Self {
        match error {
            SessionError::Timeout => Self::TimedOut,
            SessionError::Io(_) => Self::Disconnected,
            SessionError::Protocol(_) | SessionError::UnsupportedLayout => Self::Unreadable,
            SessionError::UnsupportedSoundMode => Self::Unsupported,
        }
    }

    /// The `AppState.error-label` of a failed change; the vendored banner wraps
    /// it as "Could not change “{}”.".
    pub fn label(self) -> &'static str {
        match self {
            Self::TimedOut => "Sound mode (no answer)",
            Self::Disconnected => "Sound mode (connection lost)",
            Self::Unreadable => "Sound mode (unreadable answer)",
            Self::Unsupported => "Sound mode (not supported)",
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
    /// The model has no measured sound-mode control, so no mode can be sent.
    NoSoundMode,
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
            SessionError::Protocol(_) => Self::Protocol,
            SessionError::UnsupportedLayout => Self::UnsupportedLayout,
            SessionError::UnsupportedSoundMode => Self::NoSoundMode,
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
            Self::NoSoundMode => {
                "This model does not expose the sound mode the app can change.".into()
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
    /// The device answered `01 01` with a state this model parses.
    Connected(DeviceState),
    /// The attempt ended in a failure the window has words for.
    Failed(ConnectFailure),
}

/// A write the connected device worker has to perform.
#[derive(Debug, Clone, Copy)]
enum DeviceCommand {
    /// Write a sound mode and re-read the state; `request` identifies the click
    /// so the UI can drop a result a newer click has superseded.
    SetSoundMode { request: u64, mode: SoundMode },
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

    /// Connects on a worker thread and keeps that worker for sound-mode writes.
    fn request_connect(&self, request: ConnectRequest) {
        let attempt = self.connect.get() + 1;
        self.connect.set(attempt);
        self.mode_change.set(0);
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
    pub fn new(ui: &AppWindow) -> Self {
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
        });
        initial_state(ui);
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
    // The vendored shell keeps its Windows wording, and the MVP has no system
    // settings deep links to offer.
    state.set_linux(false);
    state.set_system_settings_available(false);
    // Only Add Device, the device cards, the Overview battery and the Sound
    // mode page are wired in this shell; every other page's content stays empty.
    show_sound_mode(&state, None);
    state.set_device_pages(ModelRc::default());
    state.set_overview_rows(ModelRc::default());
    state.set_panels(ModelRc::default());
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

    actions.on_navigate({
        let inner = Rc::clone(inner);
        move |page| {
            if let Some(window) = inner.ui.upgrade() {
                window.global::<AppState>().set_page(page);
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
                    // Only a connected model with a measured sound mode keeps its
                    // worker (and socket) for writes; everything else ends as in
                    // the MVP.
                    let keeps_worker = matches!(
                        &outcome,
                        ConnectOutcome::Connected(state) if state.sound_mode.is_some()
                    );
                    if !keeps_worker {
                        inner.stop_worker();
                    }
                    apply_outcome(&window, outcome);
                }
            }
            Ok(Event::ModeChanged {
                attempt,
                request,
                outcome,
            }) => {
                if attempt == inner.connect.get()
                    && request == inner.mode_change.get()
                    && let Some(window) = inner.ui.upgrade()
                {
                    apply_mode_outcome(&window, inner, outcome);
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

/// The connected device worker: connects, writes the single `01 01` state
/// request, reports the state, then serves sound-mode changes on the same
/// session until the UI drops the command channel.
///
/// Every change runs here, so the UI thread never blocks on Bluetooth, and a
/// re-poll after each write keeps `mode-index` in step with the device.
fn run_device_worker(
    request: ConnectRequest,
    attempt: u64,
    cancel: Arc<AtomicBool>,
    events: Sender<Event>,
    commands: Receiver<DeviceCommand>,
) {
    let mut session = match open_session(&request, &cancel) {
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
    let has_sound_mode = state.sound_mode.is_some();
    let _ = events.send(Event::Connected {
        attempt,
        outcome: ConnectOutcome::Connected(state),
    });
    // A model without a measured mode has nothing this worker can write; ending
    // it closes the socket as the MVP did. A model with a mode keeps its session
    // open for the `06 81` writes.
    if !has_sound_mode {
        return;
    }
    while let Ok(DeviceCommand::SetSoundMode { request, mode }) = commands.recv() {
        let outcome = change_sound_mode(&mut session, mode);
        let failed = outcome.is_err();
        let _ = events.send(Event::ModeChanged {
            attempt,
            request,
            outcome,
        });
        if failed {
            // The session is out of step after a failed write or read; the
            // banner's Retry opens a fresh connection instead of writing on a
            // broken one.
            return;
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
    show_sound_mode(state, None);
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
    show_sound_mode(&state, None);
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
    show_sound_mode(&state, None);
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
fn apply_outcome(window: &AppWindow, outcome: ConnectOutcome) {
    let state = window.global::<AppState>();
    match outcome {
        ConnectOutcome::Connected(device_state) => {
            state.set_batteries(ModelRc::new(VecModel::from(batteries(&device_state))));
            show_sound_mode(&state, device_state.sound_mode);
            state.set_connection(Connection::Connected);
            state.set_connection_reason("".into());
            state.set_failure_kind(FailureKind::None);
            state.set_failure_text("".into());
            state.set_error_label("".into());
        }
        // An unsupported battery layout reads as a failure with no tiles; the
        // ConnectionState then shows the readable message.
        ConnectOutcome::Failed(failure) => apply_failure(&state, failure),
    }
}

/// Shows a failed connection: no battery tiles, the mapped state, and the
/// readable explanation.
fn apply_failure(state: &AppState<'_>, failure: ConnectFailure) {
    state.set_batteries(ModelRc::default());
    show_sound_mode(state, None);
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
fn show_sound_mode(state: &AppState<'_>, sound_mode: Option<SoundMode>) {
    state.set_modes(ModelRc::new(VecModel::from(mode_options(sound_mode))));
    state.set_mode_index(mode_index(sound_mode));
    state.set_mode_scale(false);
    state.set_mode_rows(ModelRc::default());
    state.set_sound_pages(ModelRc::new(VecModel::from(sound_pages(sound_mode))));
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

/// Answers a choice control on the UI thread. Only the Sound mode tiles are
/// wired in this slice; every other setting id and any index outside them is
/// ignored, so no other frame can be written.
fn set_option(window: &AppWindow, inner: &Rc<Inner>, setting_id: &str, index: i32) {
    if setting_id != "ambientSoundMode" {
        return;
    }
    let state = window.global::<AppState>();
    // No connection, or a model without a measured sound mode, means no tiles
    // and no `06 81` frame. The session refuses the write as well.
    if state.get_connection() != Connection::Connected || state.get_modes().row_count() == 0 {
        return;
    }
    let Some(mode) = mode_for_index(index) else {
        return;
    };
    let request = inner.mode_change.get() + 1;
    inner.mode_change.set(request);
    let command = DeviceCommand::SetSoundMode { request, mode };
    let sent = inner
        .commands
        .borrow()
        .as_ref()
        .is_some_and(|sender| sender.send(command).is_ok());
    if !sent {
        // The worker of a previous failed change has closed the connection; the
        // banner's Retry reconnects.
        state.set_error_label(ModeFailure::Disconnected.label().into());
    }
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
    fn a_model_without_a_measured_sound_mode_is_explained() {
        let failure = ConnectFailure::from_session(SessionError::UnsupportedSoundMode);
        assert_eq!(failure.kind(), FailureKind::Other);
        assert_eq!(failure.connection(), Connection::Failed);
        assert!(readable(&failure.text()).contains("sound mode"));
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

    /// The frame of a real, masked capture the core keeps in `core/data`, read
    /// from the JSON without a JSON parser.
    fn captured_frame(json: &str) -> Vec<u8> {
        let key = json
            .find("\"frame_hex\"")
            .expect("the capture names its frame");
        let after_key = &json[key..];
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

    fn captured_a3949_frame() -> Vec<u8> {
        captured_frame(include_str!("../../core/data/a3949-state.json"))
    }

    fn captured_a3062_frame() -> Vec<u8> {
        captured_frame(include_str!("../../core/data/a3062-state.json"))
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

    #[test]
    fn the_real_a3062_capture_exposes_the_sound_mode_page() {
        let model = soundcore_models::model_by_code("A3062").expect("A3062 is a state model");
        let state =
            soundcore_models::parse_state_frame(model, &captured_a3062_frame()).expect("parses");
        // The committed capture was taken while the device was in normal mode.
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(sound_pages(state.sound_mode), vec![Page::SoundModes]);
        assert_eq!(mode_index(state.sound_mode), 2);
    }

    #[test]
    fn the_real_a3949_capture_has_no_sound_mode_page() {
        let model = soundcore_models::model_by_code("A3949").expect("A3949 is a state model");
        let state =
            soundcore_models::parse_state_frame(model, &captured_a3949_frame()).expect("parses");
        assert_eq!(state.sound_mode, None);
        assert!(sound_pages(state.sound_mode).is_empty());
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
            (SessionError::UnsupportedSoundMode, ModeFailure::Unsupported),
        ];
        for (error, expected) in cases {
            let failure = ModeFailure::from_session(error);
            assert_eq!(failure, expected);
            let label = readable(failure.label());
            assert!(label.contains("sound mode"), "{label:?} names the setting");
        }
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
    fn a_model_without_a_sound_mode_is_never_sent_a_mode_frame() {
        let transport = FakeTransport::default();
        let mut session = session_with("A3949", &transport);

        let failure = change_sound_mode(&mut session, SoundMode::NoiseCanceling)
            .expect_err("the P20i has no mode");

        assert_eq!(failure, ModeFailure::Unsupported);
        assert_eq!(failure.label(), "Sound mode (not supported)");
        assert!(
            transport.writes().is_empty(),
            "nothing may be written to a model without a sound mode"
        );
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
    fn test_inner(ui: &AppWindow) -> Rc<Inner> {
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
        assert_eq!(pages, vec![Page::SoundModes]);
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
        assert_eq!(state.get_sound_pages().row_count(), 1, "the page is back");
        assert_eq!(state.get_error_label().as_str(), "", "the banner is gone");
        let reported = state.get_mode_index();
        println!("  retried connection reports mode index {reported}");
        assert_eq!(reported, initial, "the device kept the last mode");
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

    fn masked_mac(mac: &str) -> String {
        format!("{} __:__", &mac[..12])
    }
}
