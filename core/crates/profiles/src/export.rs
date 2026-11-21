//! Exporting saved equalizer profiles in the OpenSCL4 transfer format.
//!
//! OpenSCL4 transfers custom equalizer profiles as one JSON array of
//! `{"name": <string>, "volumeAdjustments": [<dB values>]}` objects, the same
//! format [`crate::preview_import`] reads. This module writes that format for a
//! selected slice of saved profiles. It is pure: the controller owns the save
//! dialog, the clipboard and persistence.
//!
//! # Format
//!
//! ```json
//! [{"name":"Warm","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]}]
//! ```
//!
//! - The document is one compact JSON array in the given profile order, with no
//!   whitespace; serialization is deterministic, so exporting the same
//!   selection twice writes the same text.
//! - Each profile is one object writing `name`, then `volumeAdjustments`; an
//!   empty selection is `[]`.
//! - Names are written as JSON strings with the usual escapes, so the text is
//!   valid JSON for every profile name the model accepts.
//!
//! # Decibel conversion
//!
//! The internal bands are raw dB×10 integers, while the transfer file carries
//! dB numbers with exactly one decimal, the inverse of the import rounding:
//! `30 -> 3.0`, `-120 -> -12.0`, `134 -> 13.4`, `0 -> 0.0`. The conversion is a
//! decimal shift, not a rounding: an integer number of tenths divided by `10.0`
//! gives the nearest `f64` to that one-decimal dB value, and serde_json (ryu)
//! writes its shortest round-trip digits with a decimal point and at least one
//! fractional digit, so the text is always the exact one-decimal value. The
//! output is locale-independent: the decimal separator is always `.`, never a
//! locale's comma.
//!
//! # Roundtrip
//!
//! The exported text re-imports through [`crate::preview_import`]: every profile
//! is a new profile against an empty set or a replacement against a set that
//! already has its name, with exactly the exported bands.

use crate::EqualizerProfile;
use crate::transfer::RawTransferProfile;

/// Serializes `profiles` as an OpenSCL4 transfer-profile JSON array.
///
/// The output is compact, deterministic and locale-independent: one JSON array
/// in the given order, one object per profile with `name` and
/// `volumeAdjustments`, and exactly one decimal per dB value (`30 -> 3.0`,
/// `-120 -> -12.0`). An empty slice writes `[]`. The text re-imports through
/// [`crate::preview_import`] with the same bands.
///
/// # Panics
///
/// Panics only if `serde_json` reports a serialization error, which it cannot
/// here: the document is a plain array of objects with a string `name` and eight
/// finite `f64` bands (`i16 / 10.0` is always finite), so no value these
/// profiles carry has a failing serialization.
pub fn export_json(profiles: &[EqualizerProfile]) -> String {
    let transfer: Vec<RawTransferProfile> = profiles
        .iter()
        .map(|profile| RawTransferProfile {
            name: profile.name.clone(),
            volume_adjustments: profile.bands.map(decibels).to_vec(),
        })
        .collect();
    serde_json::to_string(&transfer).expect("transfer profiles always serialize")
}

/// Converts one raw dB×10 band to the transfer file's dB value.
///
/// An integer number of tenths divided by `10.0` is the nearest `f64` to the
/// exact one-decimal dB value, and serde_json writes the shortest round-trip
/// digits with a decimal point and at least one fractional digit, so every band
/// keeps exactly one decimal (`-120 -> -12.0`, `31 -> 3.1`).
fn decibels(band: i16) -> f64 {
    f64::from(band) / 10.0
}
