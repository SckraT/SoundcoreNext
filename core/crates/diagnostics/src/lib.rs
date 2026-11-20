//! Read-only diagnostics.
//!
//! The module never writes to a device. It records frames already exchanged by
//! the transport and can run three safe probes: state, battery and charging.
//! Export and redaction are the next milestone; this crate defines the data
//! that those steps operate on.

use serde::Serialize;
use soundcore_models::DeviceModel;
use soundcore_protocol::{Command, Direction, request};

pub mod export;
pub mod redact;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeId {
    State,
    Battery,
    Charging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub id: ProbeId,
    pub command: Command,
    pub read_only: bool,
    pub timeout_ms: u64,
}

/// The complete v1 probe catalog. Write commands are intentionally absent.
pub const PROBES: [Probe; 3] = [
    Probe {
        id: ProbeId::State,
        command: Command::STATE,
        read_only: true,
        timeout_ms: 4_000,
    },
    Probe {
        id: ProbeId::Battery,
        command: Command::BATTERY,
        read_only: true,
        timeout_ms: 3_000,
    },
    Probe {
        id: ProbeId::Charging,
        command: Command::CHARGING,
        read_only: true,
        timeout_ms: 3_000,
    },
];

impl Probe {
    pub fn request_hex(self) -> String {
        hex(&request(self.command))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeStatus {
    Ok,
    Timeout,
    Empty,
    Unexpected,
    ChecksumError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub id: ProbeId,
    pub request_hex: String,
    pub response_hex: Option<String>,
    pub status: ProbeStatus,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameRecord {
    pub elapsed_ms: u64,
    pub direction: Direction,
    pub hex: String,
    pub command: String,
    pub checksum_ok: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticSession {
    pub model: Option<DeviceModel>,
    pub probes: Vec<ProbeResult>,
    pub frames: Vec<FrameRecord>,
    pub notes: String,
}

impl DiagnosticSession {
    pub fn new(model: DeviceModel) -> Self {
        Self {
            model: Some(model),
            ..Self::default()
        }
    }

    pub fn record_probe(&mut self, result: ProbeResult) {
        self.probes.push(result);
    }

    /// Record a raw frame. The caller parses it; a broken frame is recorded by
    /// error string instead of being silently dropped.
    pub fn record_raw_frame(&mut self, elapsed_ms: u64, direction: Direction, raw: &[u8]) {
        let stored = match self.model {
            Some(model) => redact::redact_state_frame(raw, model).unwrap_or_else(|| raw.to_vec()),
            None => raw.to_vec(),
        };
        let (command, checksum_ok) = match soundcore_protocol::parse(&stored) {
            Ok(frame) => (frame.command.to_string(), true),
            Err(_) => ("?".to_owned(), false),
        };
        self.frames.push(FrameRecord {
            elapsed_ms,
            direction,
            hex: hex(&stored),
            command,
            checksum_ok,
        });
    }

    pub fn manifest_json(&self) -> String {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Summary<'a> {
            schema: u32,
            model: Option<&'a str>,
            probe_count: usize,
            frame_count: usize,
            probes: &'a [ProbeResult],
            frames: &'a [FrameRecord],
            notes: &'a str,
        }
        serde_json::to_string_pretty(&Summary {
            schema: 1,
            model: self.model.map(|model| model.code),
            probe_count: self.probes.len(),
            frame_count: self.frames.len(),
            probes: &self.probes,
            frames: &self.frames,
            notes: &self.notes,
        })
        .expect("manifest serialization cannot fail")
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut out, "{byte:02X} ").expect("write to string");
    }
    out.trim_end().to_owned()
}
