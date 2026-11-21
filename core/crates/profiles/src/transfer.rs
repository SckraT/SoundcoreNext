//! The OpenSCL4 transfer-profile wire shape shared by import and export.

use serde::{Deserialize, Serialize};

/// The wire shape of one OpenSCL4 transfer profile: a `name` and its
/// `volumeAdjustments` dB values.
///
/// Import reads through this type so a malformed document is a single typed
/// error and a wrong band count stays a value the preview can report, while
/// export writes the same shape, so the two directions cannot drift apart.
/// Serde ignores unknown fields on read (its default) and writes `name` before
/// `volumeAdjustments`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawTransferProfile {
    /// The profile name: untrimmed as the file carries it on read, the trimmed
    /// saved name on write.
    pub(crate) name: String,
    /// The transfer dB values: any count as the file carries them on read,
    /// exactly the eight visible bands on write.
    pub(crate) volume_adjustments: Vec<f64>,
}
