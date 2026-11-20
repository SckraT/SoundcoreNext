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
use soundcore_models::{DeviceModel, DeviceState};
use soundcore_session::{DeviceSession, SessionError};
use windows::core::GUID;

use crate::catalog_adapter::{self, Chip};
use crate::transport_windows::{
    BluetoothError, VENDOR_SERVICE_PREFIX, WindowsRfcommTransport, format_mac, installed_services,
    list_paired_devices, parse_mac, pick_service, vendor_service,
};
use crate::{
    Actions, AppState, AppWindow, Battery, Connection, DeviceCard, FailureKind, Page, PairedOption,
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
    pub name: String,
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
            SessionError::Protocol(_) => Self::Protocol,
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
    pub address: u64,
    pub name: String,
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

/// Demo rows use locally administered addresses and never reach Bluetooth.
const DEMO_MAC_PREFIX: &str = "02:00:00:00:00:";

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
}

/// State shared between the UI callbacks and the polling timer. Every field is
/// touched on the Slint thread only; workers only hold a clone of `sender`.
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

    /// Connects and polls the state on a worker thread.
    fn request_connect(&self, request: ConnectRequest) {
        let attempt = self.connect.get() + 1;
        self.connect.set(attempt);
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.borrow_mut() = Some(Arc::clone(&cancel));
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("bluetooth-connect".into())
            .spawn(move || {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(outcome) = connect_device(&request, &cancel) {
                    let _ = sender.send(Event::Connected { attempt, outcome });
                }
            });
        if spawned.is_err() {
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
        self.connect.set(0);
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
    // Only Add Device, the device cards and the Overview battery are wired in
    // this MVP; every other page's content stays empty.
    state.set_sound_pages(ModelRc::default());
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

    actions.on_set_demo({
        let inner = Rc::clone(inner);
        move |on| {
            if let Some(window) = inner.ui.upgrade() {
                set_demo(&window, &inner, on);
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
                    apply_outcome(&window, outcome);
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

/// Opens the RFCOMM connection, writes the single `01 01` state request, and
/// parses the answer. Returns `None` when the attempt was cancelled first.
fn connect_device(request: &ConnectRequest, cancel: &AtomicBool) -> Option<ConnectOutcome> {
    let cancelled = || cancel.load(Ordering::Relaxed);
    let Some(model) = request.protocol else {
        return Some(ConnectOutcome::Failed(ConnectFailure::NoParser));
    };
    if cancelled() {
        return None;
    }
    let services = match installed_services(request.address) {
        Ok(services) => services,
        Err(error) => {
            return Some(ConnectOutcome::Failed(ConnectFailure::from_bluetooth(
                error,
            )));
        }
    };
    if cancelled() {
        return None;
    }
    let Some(service) = pick_service(&services, &request.model_code) else {
        return Some(ConnectOutcome::Failed(ConnectFailure::NoService));
    };
    let transport = match WindowsRfcommTransport::connect(request.address, service) {
        Ok(transport) => transport,
        Err(error) => {
            return Some(ConnectOutcome::Failed(ConnectFailure::from_bluetooth(
                error,
            )));
        }
    };
    if cancelled() {
        // Dropping the transport closes the socket.
        return None;
    }
    // `poll_state` is the only place that writes: one `01 01` state request.
    let mut session = DeviceSession::new(model, transport);
    Some(match session.poll_state() {
        Ok(state) => ConnectOutcome::Connected(state),
        Err(error) => ConnectOutcome::Failed(ConnectFailure::from_session(error)),
    })
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
    if state.get_add_demo() {
        state.set_add_loading(false);
        show_demo_options(&state);
    } else {
        state.set_add_loading(true);
        inner.request_paired(Some((model.code.to_owned(), model.name.to_owned())));
    }
}

/// Repeats the enumeration for the current selection, or refreshes the demo
/// rows when demo mode is on.
fn refresh_paired(window: &AppWindow, inner: &Rc<Inner>) {
    let state = window.global::<AppState>();
    state.set_add_failure_kind(FailureKind::None);
    state.set_add_error("".into());
    if state.get_add_demo() {
        state.set_add_loading(false);
        show_demo_options(&state);
    } else {
        state.set_add_bluetooth_off(false);
        request_paired_for_selection(&state, inner);
    }
}

/// Turns demo mode on or off; the on state lists simulated rows instead of
/// paired devices, and pairing one never opens Bluetooth.
fn set_demo(window: &AppWindow, inner: &Rc<Inner>, on: bool) {
    let state = window.global::<AppState>();
    state.set_add_demo(on);
    state.set_add_bluetooth_off(false);
    state.set_add_failure_kind(FailureKind::None);
    state.set_add_error("".into());
    if on {
        // A paired result for the previous selection must not replace these.
        inner.enumeration.set(inner.enumeration.get() + 1);
        state.set_add_loading(false);
        show_demo_options(&state);
    } else {
        state.set_paired_options(ModelRc::default());
        request_paired_for_selection(&state, inner);
    }
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

/// The simulated rows demo mode lists.
fn show_demo_options(state: &AppState<'_>) {
    let options: Vec<PairedOption> = (1..=2)
        .map(|index| PairedOption {
            mac: format!("{DEMO_MAC_PREFIX}{index:02X}").into(),
            name: format!("Demo device {index}").into(),
            suggested: index == 1,
        })
        .collect();
    state.set_paired_options(ModelRc::new(VecModel::from(options)));
}

/// Pairs the picked Windows device with the chosen catalog model and starts the
/// connection attempt.
fn pair(window: &AppWindow, inner: &Rc<Inner>, mac: &str, name: &str) {
    let state = window.global::<AppState>();
    if state.get_add_demo() {
        finish_demo_device(window, mac, name);
        return;
    }
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

/// Starts the connection of a saved card: a demo card shows simulated state
/// instead of opening Bluetooth.
fn begin_card_connect(window: &AppWindow, inner: &Rc<Inner>, card: DeviceCard) {
    if card.demo {
        show_demo_card(window, &card);
        return;
    }
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
    state.set_batteries(ModelRc::default());
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
    state.set_page(Page::Devices);
}

/// Shows a connected demo device: no Bluetooth is involved.
fn show_demo_card(window: &AppWindow, card: &DeviceCard) {
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
    state.set_batteries(demo_batteries());
    state.set_connection(Connection::Connected);
    state.set_connection_reason("".into());
    state.set_failure_kind(FailureKind::None);
    state.set_failure_text("".into());
}

/// Pairs a simulated device from the dialog's demo rows.
fn finish_demo_device(window: &AppWindow, mac: &str, name: &str) {
    let state = window.global::<AppState>();
    let chosen = chosen_model(&state);
    let code = chosen.as_ref().map(|model| model.code).unwrap_or_default();
    let art = chosen
        .as_ref()
        .map(|model| model.art.clone())
        .unwrap_or_default();
    let is_speaker = chosen.as_ref().is_some_and(|model| model.is_speaker);
    show_current_card(
        &state,
        DeviceCard {
            mac: mac.into(),
            name: name.into(),
            code: code.into(),
            art: art.clone(),
            current: true,
            demo: true,
        },
    );
    state.set_device_name(name.into());
    state.set_device_code(code.into());
    state.set_device_art(art);
    state.set_device_is_speaker(is_speaker);
    state.set_one_earbud(false);
    state.set_batteries(demo_batteries());
    state.set_connection(Connection::Connected);
    state.set_connection_reason("".into());
    state.set_failure_kind(FailureKind::None);
    state.set_failure_text("".into());
    state.set_adding(false);
    state.set_add_step(0);
    state.set_page(Page::Overview);
}

/// Sets the saved cards to `card` as their only current one.
fn show_current_card(state: &AppState<'_>, card: DeviceCard) {
    let cards: Vec<DeviceCard> = state.get_devices().iter().collect();
    state.set_devices(ModelRc::new(VecModel::from(with_current_card(
        &cards, card,
    ))));
}

/// The simulated battery tiles of a demo device.
fn demo_batteries() -> ModelRc<Battery> {
    ModelRc::new(VecModel::from(vec![
        battery_tile(0, 5, 5),
        battery_tile(1, 4, 5),
    ]))
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
            state.set_connection(Connection::Connected);
            state.set_connection_reason("".into());
            state.set_failure_kind(FailureKind::None);
            state.set_failure_text("".into());
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
    state.set_connection(failure.connection());
    state.set_connection_reason("".into());
    state.set_failure_kind(failure.kind());
    state.set_failure_text(failure.text().into());
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
    use soundcore_models::{BatteryScale, BatteryState, FirmwareVersions, HostDevice, Layout};

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
            address: 0x0000_F49D_8AA2_0001,
            name: "soundcore P20i".into(),
            suggested: true,
        };
        let option = paired_option(&candidate);
        assert_eq!(option.name.as_str(), "soundcore P20i");
        assert!(option.suggested);
        assert_eq!(option.mac.as_str(), "F4:9D:8A:A2:00:01");
    }

    #[test]
    fn a_paired_option_mac_parses_back_to_its_numeric_address() {
        for address in [
            0x0000_0000_0000_0000,
            0x0000_F49D_8AA2_0001,
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
            match connect_device(&request, &AtomicBool::new(false)) {
                Some(ConnectOutcome::Connected(state)) => {
                    let shown: Vec<String> = batteries(&state)
                        .iter()
                        .map(|tile| format!("label {}={}", tile.label, tile.value))
                        .collect();
                    println!("  connected: {}", shown.join(" "));
                }
                Some(ConnectOutcome::Failed(failure)) => {
                    println!("  failed ({:?}): {}", failure.kind(), failure.text());
                }
                None => println!("  cancelled"),
            }
        }
    }

    fn masked_mac(mac: &str) -> String {
        format!("{} __:__", &mac[..12])
    }
}
