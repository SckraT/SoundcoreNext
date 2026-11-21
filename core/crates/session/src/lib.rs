//! Platform-independent device session.
//!
//! The session sits between a byte transport (Bluetooth RFCOMM on Windows, or a
//! test double) and the model-specific state parser. It owns four
//! platform-neutral jobs:
//!
//! - frame the outbound `01 01` state request;
//! - accumulate inbound bytes until one complete, checksum-valid frame is
//!   available, tolerating leading garbage and several frames per read;
//! - hand that frame to [`soundcore_models::parse_state_frame`];
//! - write the measured `06 81` sound-mode frame for models that have one;
//! - write the measured settings frames (double press, ambient cycle, power,
//!   prompts and effects) for models that have a settings block;
//! - write the measured `03 87` equalizer preset and custom frames for models
//!   that have one.
//!
//! Nothing here knows about Windows, sockets, threads or timers.
//!
//! # Blocking and threading
//!
//! Everything in this crate is blocking. The session owns no thread, no timer
//! and no OS handle: the caller runs it on a worker thread and never calls
//! [`DeviceSession::poll_state`] from a UI thread.
//!
//! Read deadlines belong to the transport. [`Transport::read`] must bound its
//! own blocking wait (a Windows RFCOMM socket does this with `SO_RCVTIMEO`) and
//! report expiry as [`io::ErrorKind::TimedOut`] or
//! [`io::ErrorKind::WouldBlock`]; the session maps both to
//! [`SessionError::Timeout`]. Dropping a [`DeviceSession`] merely drops the
//! transport, which closes the connection.

use std::io;
use std::time::{Duration, Instant};

use soundcore_models::{
    DUAL_CONNECTIONS_LIST, DeviceModel, DeviceState, DualConnectionDevice, DualConnectionsError,
    DualConnectionsList, EQUALIZER_COMMAND_BODY_LEN, EqualizerPreset, EqualizerState,
    SettingCommand, SoundMode, StateError, VISIBLE_EQ_BANDS, dual_connections_connect_frame,
    dual_connections_disconnect_frame, dual_connections_forget_frame, dual_connections_request,
    parse_dual_connections_body, parse_state_frame,
};
use soundcore_protocol::{
    Command, Direction, INBOUND_PREFIX, MIN_FRAME_LEN, ParseError, encode, parse, request,
};

/// Bytes requested from the transport in a single read.
const READ_CHUNK: usize = 1024;

/// Prefix (5) + command (2) + total length (2).
const HEADER_LEN: usize = INBOUND_PREFIX.len() + 4;

/// The maximum number of frames a dual-connections list reply may span.
pub const DUAL_CONNECTIONS_MAX_FRAMES: usize = 20;

/// How long the next dual-connections frame may take before the read fails.
pub const DUAL_CONNECTIONS_FRAME_TIMEOUT: Duration = Duration::from_secs(2);

/// How long the whole dual-connections list reply may take.
pub const DUAL_CONNECTIONS_TOTAL_TIMEOUT: Duration = Duration::from_secs(5);

/// Blocking byte transport.
///
/// A stream socket does not preserve frame boundaries, so [`Transport::read`]
/// returns whatever arrived and the session reassembles frames from it.
pub trait Transport {
    /// Write every byte of `bytes`, blocking until all of them are written.
    ///
    /// The session always passes a complete frame, so an implementation must
    /// retry short writes internally and report an error rather than a partial
    /// write.
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Block until at least one byte is available, copy up to `buffer.len()`
    /// bytes into `buffer`, and return how many bytes were copied.
    ///
    /// The contract mirrors a socket `recv`:
    ///
    /// - `Ok(0)` means the peer closed the connection;
    /// - the wait must be bounded by the transport (for a socket, set
    ///   `SO_RCVTIMEO`) and an expired deadline reported as
    ///   [`io::ErrorKind::TimedOut`] or [`io::ErrorKind::WouldBlock`].
    ///
    /// A single call may return any number of bytes, including half a frame.
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

/// Why an inbound frame could not become a [`DeviceState`].
#[derive(Debug)]
pub enum ProtocolError {
    /// The bytes are not a valid Soundcore frame (prefix, length or checksum).
    Frame(ParseError),
    /// The frame is valid but the model parser rejected its body.
    State(StateError),
    /// The frame is a valid `0B 01` frame but its dual-connections body is
    /// malformed (frame count, device length or a cut-off device record).
    DualConnections(DualConnectionsError),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "invalid frame: {error}"),
            Self::State(error) => write!(formatter, "invalid state: {error}"),
            Self::DualConnections(error) => {
                write!(formatter, "invalid dual-connections response: {error}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::State(error) => Some(error),
            Self::DualConnections(error) => Some(error),
        }
    }
}

/// Typed failure of a blocking session operation.
#[derive(Debug)]
pub enum SessionError {
    /// No complete frame arrived before the transport's read deadline.
    Timeout,
    /// The transport failed, including an unexpected end of stream.
    Io(io::Error),
    /// Bytes arrived but they are not a valid frame or state response.
    Protocol(ProtocolError),
    /// The device answered, but this model's battery layout has no parser yet.
    UnsupportedLayout,
    /// The model has no measured sound-mode control, so there is no `06 81`
    /// frame to write.
    UnsupportedSoundMode,
    /// The model has no equalizer support in this slice, so there is no
    /// `03 87` frame to write.
    UnsupportedEqualizer,
    /// The model has no measured settings block, so there is no settings frame
    /// to write (for example the A3949).
    UnsupportedSetting,
    /// The model's dual-connections list and writes were not measured in this
    /// slice, so nothing is read or written (for example the A3949).
    UnsupportedDualConnections,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(formatter, "timed out waiting for a device state frame"),
            Self::Io(error) => write!(formatter, "transport error: {error}"),
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
            Self::UnsupportedLayout => {
                write!(
                    formatter,
                    "battery layout is not implemented for this model"
                )
            }
            Self::UnsupportedSoundMode => {
                write!(formatter, "sound modes were not measured for this model")
            }
            Self::UnsupportedEqualizer => {
                write!(
                    formatter,
                    "equalizer writes are not supported for this model"
                )
            }
            Self::UnsupportedSetting => {
                write!(formatter, "settings are not supported for this model")
            }
            Self::UnsupportedDualConnections => {
                write!(
                    formatter,
                    "dual connections are not supported for this model"
                )
            }
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Timeout
            | Self::UnsupportedLayout
            | Self::UnsupportedSoundMode
            | Self::UnsupportedEqualizer
            | Self::UnsupportedSetting
            | Self::UnsupportedDualConnections => None,
        }
    }
}

impl From<io::Error> for SessionError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Self::Timeout,
            _ => Self::Io(error),
        }
    }
}

impl From<StateError> for SessionError {
    fn from(error: StateError) -> Self {
        match error {
            StateError::UnsupportedLayout(_) => Self::UnsupportedLayout,
            other => Self::Protocol(ProtocolError::State(other)),
        }
    }
}

/// A device model plus a transport, driven from a worker thread.
pub struct DeviceSession<T: Transport> {
    model: DeviceModel,
    transport: T,
    buffer: Vec<u8>,
}

impl<T: Transport> DeviceSession<T> {
    /// Creates a session that parses `model`'s state from `transport`.
    ///
    /// Nothing is spawned here: the caller drives [`Self::poll_state`] on its
    /// own worker thread, and dropping the session drops (and closes) the
    /// transport.
    pub fn new(model: DeviceModel, transport: T) -> Self {
        Self {
            model,
            transport,
            buffer: Vec::new(),
        }
    }

    /// The model this session parses state for.
    pub fn model(&self) -> DeviceModel {
        self.model
    }

    /// Ask the device for its state by writing `01 01`, then return the next
    /// complete state frame parsed for this model.
    ///
    /// Blocking. Reads until a checksum-valid frame is available; frames of
    /// other commands (acknowledgements, unsolicited battery updates) are
    /// consumed and skipped, and leading garbage is discarded.
    pub fn poll_state(&mut self) -> Result<DeviceState, SessionError> {
        self.transport.write_all(&request(Command::STATE))?;
        loop {
            if let Some(raw) = self.next_frame() {
                let frame = parse(&raw)
                    .map_err(|error| SessionError::Protocol(ProtocolError::Frame(error)))?;
                if frame.command != Command::STATE {
                    continue;
                }
                return parse_state_frame(self.model, &raw).map_err(SessionError::from);
            }
            self.fill()?;
        }
    }

    /// Write this model's `06 81` sound-mode frame and return once every byte
    /// of it is written.
    ///
    /// This method never reads. The device acknowledges with an empty inbound
    /// `06 81` frame and may also push notifications, but neither is a state
    /// response: call [`Self::poll_state`] afterwards to read the new mode, and
    /// it will skip those frames.
    ///
    /// Fails with [`SessionError::UnsupportedSoundMode`] when the model has no
    /// measured sound-mode control, and writes nothing in that case.
    pub fn set_sound_mode(&mut self, mode: SoundMode) -> Result<(), SessionError> {
        let body = self
            .model
            .sound_mode_body(mode)
            .ok_or(SessionError::UnsupportedSoundMode)?;
        self.transport.write_all(&encode(
            Direction::Outbound,
            Command::ACK_AMBIENT_SOUND_MODE,
            &body,
        ))?;
        Ok(())
    }

    /// Write one A3062 settings frame and return once every byte of it is
    /// written.
    ///
    /// The exact command and body come from [`SettingCommand`], so each call
    /// writes exactly one frame; for example the limit-high-volume enable flag,
    /// dB limit and refresh rate are three separate [`SettingCommand`]s and
    /// three calls.
    ///
    /// This method never reads. The device acknowledges with an empty inbound
    /// frame of the same command and may also push a notification (LDAC is
    /// confirmed by a separate `01 7F` notification); none of those is a state
    /// response. Call [`Self::poll_state`] afterwards to read the new value, and
    /// it will skip them.
    ///
    /// Fails with [`SessionError::UnsupportedSetting`] when the model has no
    /// measured settings block (for example the A3949), and writes nothing in
    /// that case.
    pub fn set_setting(&mut self, setting: SettingCommand) -> Result<(), SessionError> {
        let frame = self
            .model
            .setting_frame(setting)
            .ok_or(SessionError::UnsupportedSetting)?;
        self.transport.write_all(&frame)?;
        Ok(())
    }

    /// Ask for the device's dual-connections list by writing the empty `0B 01`
    /// frame, then collect the reply frames until the last one arrives.
    ///
    /// Blocking. The budgets follow OpenSCL4's packet controller: at most
    /// [`DUAL_CONNECTIONS_MAX_FRAMES`] reply frames, a
    /// [`DUAL_CONNECTIONS_FRAME_TIMEOUT`] wait for the next frame and
    /// [`DUAL_CONNECTIONS_TOTAL_TIMEOUT`] for the whole exchange. Running out of
    /// any budget, including the transport's read deadline when the device says
    /// nothing, is [`SessionError::Timeout`]; partial data is discarded.
    ///
    /// Frames of other commands that arrive before or between the reply frames
    /// are consumed and skipped. A reply frame the parser rejects fails with a
    /// [`SessionError::Protocol`] error instead of returning a partial list.
    ///
    /// Fails with [`SessionError::UnsupportedDualConnections`] and writes
    /// nothing when the model has no measured dual-connections support (for
    /// example the A3949).
    pub fn read_dual_connections_devices(
        &mut self,
    ) -> Result<Vec<DualConnectionDevice>, SessionError> {
        if !self.model.dual_connections {
            return Err(SessionError::UnsupportedDualConnections);
        }
        self.transport.write_all(&dual_connections_request())?;
        let exchange_started = Instant::now();
        let mut frame_started = exchange_started;
        let mut list = DualConnectionsList::new();
        let mut frames = 0usize;
        loop {
            if frames >= DUAL_CONNECTIONS_MAX_FRAMES
                || exchange_started.elapsed() >= DUAL_CONNECTIONS_TOTAL_TIMEOUT
                || frame_started.elapsed() >= DUAL_CONNECTIONS_FRAME_TIMEOUT
            {
                // A command that returns nothing still has to terminate the
                // exchange: silence and a reply that never reaches its last
                // frame are timeouts, never an empty success.
                return Err(SessionError::Timeout);
            }
            let Some(raw) = self.next_frame() else {
                self.fill()?;
                continue;
            };
            let frame =
                parse(&raw).map_err(|error| SessionError::Protocol(ProtocolError::Frame(error)))?;
            if frame.command != DUAL_CONNECTIONS_LIST {
                continue;
            }
            let frame = parse_dual_connections_body(&frame.body)
                .map_err(|error| SessionError::Protocol(ProtocolError::DualConnections(error)))?;
            frames += 1;
            frame_started = Instant::now();
            if list.push_frame(frame) {
                return Ok(list.into_devices());
            }
        }
    }

    /// Write the exact `0B 82` connect frame for a device's six address bytes
    /// and return once every byte of it is written.
    ///
    /// Fire-and-forget, exactly like OpenSCL4: the device does not acknowledge
    /// connect (or disconnect), so this never reads. Re-read
    /// [`Self::read_dual_connections_devices`] afterwards to see the outcome.
    ///
    /// Fails with [`SessionError::UnsupportedDualConnections`] and writes
    /// nothing when the model has no measured dual-connections support.
    pub fn connect_dual_connection(&mut self, mac: [u8; 6]) -> Result<(), SessionError> {
        self.write_dual_connection_frame(dual_connections_connect_frame(mac))
    }

    /// Write the exact `0B 81` disconnect frame for a device's six address bytes
    /// and return once every byte of it is written.
    ///
    /// Fire-and-forget like [`Self::connect_dual_connection`]: it never reads.
    ///
    /// Fails with [`SessionError::UnsupportedDualConnections`] and writes
    /// nothing when the model has no measured dual-connections support.
    pub fn disconnect_dual_connection(&mut self, mac: [u8; 6]) -> Result<(), SessionError> {
        self.write_dual_connection_frame(dual_connections_disconnect_frame(mac))
    }

    /// Write the exact `0B 83` forget frame for a device's six address bytes and
    /// return once every byte of it is written.
    ///
    /// The device acknowledges forget with an empty same-command frame, but this
    /// method does not read it: the caller re-reads the list, which is also what
    /// the controller does after every dual-connections write. Forgetting does
    /// not disconnect the device; disconnect first when it is connected.
    ///
    /// Fails with [`SessionError::UnsupportedDualConnections`] and writes
    /// nothing when the model has no measured dual-connections support.
    pub fn forget_dual_connection(&mut self, mac: [u8; 6]) -> Result<(), SessionError> {
        self.write_dual_connection_frame(dual_connections_forget_frame(mac))
    }

    /// Writes one dual-connections frame, after checking the model supports the
    /// feature; the one gate every dual-connections write shares.
    fn write_dual_connection_frame(&mut self, frame: Vec<u8>) -> Result<(), SessionError> {
        if !self.model.dual_connections {
            return Err(SessionError::UnsupportedDualConnections);
        }
        self.transport.write_all(&frame)?;
        Ok(())
    }

    /// Write this model's `03 87` equalizer preset frame exactly once and
    /// return once every byte of it is written.
    ///
    /// The 53-byte body keeps the HearID fields of `current` and applies
    /// `preset`'s id and curve; see
    /// [`DeviceModel::equalizer_preset_body`](soundcore_models::DeviceModel::equalizer_preset_body).
    ///
    /// This method never reads. The device acknowledges with an empty inbound
    /// `03 87` frame and may push notifications; call [`Self::poll_state`]
    /// afterwards to read the preset and bands the device now reports, and it
    /// will skip those frames.
    ///
    /// Fails with [`SessionError::UnsupportedEqualizer`] when this slice does
    /// not support the model's equalizer (for example the deliberately deferred
    /// A3949), and writes nothing in that case.
    pub fn set_equalizer_preset(
        &mut self,
        current: &EqualizerState,
        preset: &EqualizerPreset,
    ) -> Result<(), SessionError> {
        let body = self.model.equalizer_preset_body(current, preset);
        self.write_equalizer_body(body)
    }

    /// Write this model's `03 87` custom equalizer frame exactly once and
    /// return once every byte of it is written.
    ///
    /// `visible_bands` are the eight bands the Equalizer page shows in raw
    /// dB×10 units (`0` is `0 dB`, `-120` is `-12.0 dB`, `134` is `+13.4 dB`),
    /// clamped to the measured `-120..=134` range. The 53-byte body keeps the
    /// HearID fields of `current`, sets the preset id to
    /// [`CUSTOM_EQUALIZER_PRESET_ID`](soundcore_models::CUSTOM_EQUALIZER_PRESET_ID)
    /// and fixes the two invisible bands; see
    /// [`DeviceModel::custom_equalizer_body`](soundcore_models::DeviceModel::custom_equalizer_body).
    ///
    /// This method never reads. The device acknowledges with an empty inbound
    /// `03 87` frame and may push notifications; call [`Self::poll_state`]
    /// afterwards to read the preset id and bands the device now reports, and it
    /// will skip those frames.
    ///
    /// Fails with [`SessionError::UnsupportedEqualizer`] when this slice does
    /// not support the model's equalizer (for example the deliberately deferred
    /// A3949), and writes nothing in that case.
    pub fn set_custom_equalizer(
        &mut self,
        current: &EqualizerState,
        visible_bands: [i16; VISIBLE_EQ_BANDS],
    ) -> Result<(), SessionError> {
        let body = self.model.custom_equalizer_body(current, visible_bands);
        self.write_equalizer_body(body)
    }

    /// Encodes and writes one measured 53-byte `03 87` body, the single
    /// body-to-frame step every equalizer write shares; `None` means this
    /// model has no equalizer support in this slice, so nothing is written.
    fn write_equalizer_body(
        &mut self,
        body: Option<[u8; EQUALIZER_COMMAND_BODY_LEN]>,
    ) -> Result<(), SessionError> {
        let body = body.ok_or(SessionError::UnsupportedEqualizer)?;
        self.transport
            .write_all(&encode(Direction::Outbound, Command::EQUALIZER, &body))?;
        Ok(())
    }

    /// Read one chunk from the transport into the assembly buffer.
    ///
    /// `Interrupted` is retried because interrupting a blocking read is not a
    /// transport failure; `Ok(0)` is an end of stream.
    fn fill(&mut self) -> Result<(), SessionError> {
        let mut chunk = [0u8; READ_CHUNK];
        loop {
            match self.transport.read(&mut chunk) {
                Ok(0) => {
                    return Err(SessionError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "transport closed while waiting for a state frame",
                    )));
                }
                Ok(read) => {
                    let data = chunk.get(..read).ok_or_else(|| {
                        SessionError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "transport reported more bytes than the read buffer holds",
                        ))
                    })?;
                    self.buffer.extend_from_slice(data);
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Take the next complete frame out of the assembly buffer, if one is there.
    ///
    /// Discards bytes that cannot start an inbound frame first, so a stream
    /// that began mid-frame or carries noise still resynchronises on the next
    /// prefix.
    fn next_frame(&mut self) -> Option<Vec<u8>> {
        loop {
            discard_leading_garbage(&mut self.buffer);
            if self.buffer.len() < HEADER_LEN {
                return None;
            }
            let declared = usize::from(u16::from_le_bytes([self.buffer[7], self.buffer[8]]));
            if declared < MIN_FRAME_LEN {
                // A frame cannot be this short, so this prefix is noise. Drop
                // one byte and look for the next prefix.
                self.buffer.remove(0);
                continue;
            }
            if self.buffer.len() < declared {
                return None;
            }
            return Some(self.buffer.drain(..declared).collect());
        }
    }
}

/// Drop everything before the first inbound prefix.
///
/// When no full prefix is buffered yet, keep only the trailing bytes that could
/// still become one, so a prefix split across reads is not lost.
fn discard_leading_garbage(buffer: &mut Vec<u8>) {
    if buffer.is_empty() {
        return;
    }
    if let Some(index) = buffer
        .windows(INBOUND_PREFIX.len())
        .position(|window| window == INBOUND_PREFIX.as_slice())
    {
        if index > 0 {
            buffer.drain(..index);
        }
        return;
    }
    let keep = (1..INBOUND_PREFIX.len())
        .rev()
        .find(|length| buffer.ends_with(&INBOUND_PREFIX[..*length]))
        .unwrap_or(0);
    let length = buffer.len();
    buffer.drain(..length - keep);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_frame_length_is_consistent_with_the_header() {
        assert_eq!(HEADER_LEN, MIN_FRAME_LEN - 1);
    }
}
