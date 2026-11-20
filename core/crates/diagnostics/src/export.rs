//! Diagnostic ZIP export.
//!
//! The bundle is produced locally and is never uploaded by the core. The caller
//! decides whether to save it, share it, or attach it to a GitHub issue.

use std::io::{Cursor, Write};

use crate::{DiagnosticSession, redact::redact_text};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub const MANIFEST: &str = "manifest.json";
pub const FRAMES: &str = "frames.jsonl";
pub const README: &str = "README.txt";

#[derive(Debug)]
pub enum ExportError {
    Zip(String),
    Serialization(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(message) | Self::Serialization(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ExportError {}

/// Build a ZIP in memory.
///
/// `manifest.json` is the redacted session manifest. `frames.jsonl` contains
/// one redacted frame record per line. `README.txt` explains how to attach the
/// archive to an issue and what was redacted.
pub fn export_zip(session: &DiagnosticSession) -> Result<Vec<u8>, ExportError> {
    let manifest = redact_text(&session.manifest_json());
    let mut frames = String::new();
    for frame in &session.frames {
        let line = serde_json::to_string(frame).map_err(|error| {
            ExportError::Serialization(format!("frame serialization failed: {error}"))
        })?;
        frames.push_str(&redact_text(&line));
        frames.push('\n');
    }

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        write_file(&mut zip, MANIFEST, manifest.as_bytes(), options)?;
        write_file(&mut zip, FRAMES, frames.as_bytes(), options)?;
        write_file(&mut zip, README, readme().as_bytes(), options)?;
        zip.finish()
            .map_err(|error| ExportError::Zip(error.to_string()))?;
    }
    Ok(cursor.into_inner())
}

fn write_file(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), ExportError> {
    zip.start_file(name, options)
        .map_err(|error| ExportError::Zip(error.to_string()))?;
    zip.write_all(bytes)
        .map_err(|error| ExportError::Zip(error.to_string()))
}

pub fn readme() -> String {
    [
        "Soundcore diagnostics",
        "",
        "Attach this ZIP to the GitHub issue opened by the app.",
        "The archive contains:",
        "- manifest.json: session summary and probe results;",
        "- frames.jsonl: redacted tx/rx frames;",
        "- README.txt: this file.",
        "",
        "Serial numbers and Bluetooth addresses are removed by default.",
        "Review the archive before sharing it publicly.",
    ]
    .join("\n")
}
