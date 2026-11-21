//! Importing custom equalizer profiles exported by OpenSCL4.
//!
//! OpenSCL4 transfers custom equalizer profiles as one JSON array of
//! `{"name": <string>, "volumeAdjustments": [<dB values>]}` objects. This
//! module reads that format, says what importing each profile would do, and
//! merges an importable preview into a [`ProfileSet`]. It is pure: the
//! controller owns the file dialog, the clipboard and persistence.
//!
//! # Format
//!
//! ```json
//! [{"name": "Warm", "volumeAdjustments": [0.4, -0.3, 0.5, 0.0, -6.0, 2.0, 0.0, -2.0]}]
//! ```
//!
//! - The document is one JSON array. Unknown object fields are ignored, so an
//!   export from a newer OpenSCL4 still reads.
//! - `name` must be a string that is non-empty after trimming and at most
//!   [`crate::MAX_PROFILE_NAME_LEN`] characters, the same rule as a saved
//!   profile. Names must be unique after trimming; matching is exact, so
//!   `Warm` and `warm` are different names.
//! - `volumeAdjustments` must be present and numeric. A missing field, a
//!   non-array or a non-numeric value means the text is not a profiles file
//!   ([`ImportError::NotProfiles`]).
//! - Eight values are expected, but a different count is not a parse error: it
//!   is a blocked preview status, so the Import dialog can list the profile
//!   and show why it cannot be imported.
//!
//! # Decibel conversion and half rounding
//!
//! Each value is multiplied by 10 in `f64` and rounded to the nearest integer
//! with [`f64::round`], which rounds halves away from zero: `0.05` → `1`,
//! `-0.05` → `-1`, `0.15` → `2`. This is OpenSCL4's `scaled_to_device` rule, so
//! the same file gives the same bands in both apps. The multiplication is IEEE
//! 754; for one-decimal inputs the product is the exact decimal in practice,
//! so a decimal tie is a real tie: `13.45` → `134.5` → `135`. The rule applies
//! to the `f64` product, not to the decimal text.
//!
//! After rounding, a band outside [`EQ_BAND_MIN`]..=[`EQ_BAND_MAX`] dB×10
//! (`-12.0`..=`+13.4` dB) makes the profile [`ImportStatus::OutOfRange`]:
//! `13.5` dB → `135`, `-12.1` dB → `-121`, `13.45` dB → `135`, `-12.05` dB →
//! `-121`; `13.44` dB → `134` and `-12.04` dB → `-120` are accepted.
//!
//! # Preview statuses
//!
//! Every profile gets OpenSCL4's status, and the explicit numeric value is the
//! vendored Import dialog's `ImportItem.status` tag:
//!
//! - [`ImportStatus::New`] = 0: the name is not saved; importing adds it.
//! - [`ImportStatus::ReplacesExisting`] = 1: a saved profile has the same
//!   trimmed name; importing replaces its bands in place.
//! - [`ImportStatus::WrongBandCount`] = 2: the profile does not carry eight
//!   values; blocked.
//! - [`ImportStatus::OutOfRange`] = 3: a rounded band is outside the measured
//!   A3062 range; blocked.
//!
//! The band count is checked before the values, as OpenSCL4 does. A preview is
//! blocked when any status is 2 or 3 ([`ImportPreview::is_blocked`]), and a
//! blocked preview cannot be merged: [`ImportPreview::merge_into`] refuses it
//! and leaves the set unchanged.
//!
//! # Merge
//!
//! Merging is all-or-nothing: a blocked preview is refused, every profile is
//! rebuilt before the set is touched, and then each same-named profile is
//! replaced in its saved position while new profiles are appended in file
//! order.
//!
//! [`EQ_BAND_MAX`]: soundcore_models::EQ_BAND_MAX
//! [`EQ_BAND_MIN`]: soundcore_models::EQ_BAND_MIN

use std::collections::HashSet;

use serde::Deserialize;
use soundcore_models::{EQ_BAND_MAX, EQ_BAND_MIN, VISIBLE_EQ_BANDS};

use crate::{EqualizerProfile, ProfileError, ProfileSet, normalize_name};

/// What importing one transfer profile would do.
///
/// The numeric value of each variant is the vendored Import dialog's
/// `ImportItem.status` tag, so the controller can hand it to Slint unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStatus {
    /// The name is not saved, so importing adds the profile.
    New = 0,
    /// A saved profile has the same trimmed name, so importing replaces its
    /// bands and keeps its position.
    ReplacesExisting = 1,
    /// The profile does not carry exactly [`VISIBLE_EQ_BANDS`] values; blocked.
    WrongBandCount = 2,
    /// A band is outside [`EQ_BAND_MIN`]..=[`EQ_BAND_MAX`] dB×10 after
    /// rounding; blocked.
    OutOfRange = 3,
}

impl ImportStatus {
    /// Whether an import would save this profile, as a new one or a replacement.
    pub fn is_importable(self) -> bool {
        matches!(self, Self::New | Self::ReplacesExisting)
    }

    /// Whether this status blocks the whole import (wrong band count or range).
    pub fn is_blocking(self) -> bool {
        !self.is_importable()
    }
}

/// One profile parsed from a transfer file and what importing it would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportItem {
    /// The trimmed profile name.
    pub name: String,
    /// The number of `volumeAdjustments` values the file carries; this is what
    /// the dialog shows as the profile's band count, including a wrong one.
    pub band_count: usize,
    /// What importing the profile would do; the variant's value is the dialog
    /// tag.
    pub status: ImportStatus,
    /// The rounded dB×10 bands, present exactly when the profile is importable
    /// ([`ImportStatus::is_importable`]); blocked items carry `None`.
    pub bands: Option<[i16; VISIBLE_EQ_BANDS]>,
}

/// A parsed OpenSCL4 transfer file: one item per profile, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPreview {
    items: Vec<ImportItem>,
}

impl ImportPreview {
    /// The preview items, in file order.
    pub fn items(&self) -> &[ImportItem] {
        &self.items
    }

    /// The number of profiles the file carries.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the file carries no profiles (`[]`).
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether any item is blocked, meaning the whole import must not be merged.
    pub fn is_blocked(&self) -> bool {
        self.items.iter().any(|item| item.status.is_blocking())
    }

    /// Merges every preview item into `saved`, all-or-nothing.
    ///
    /// A profile whose name is already saved replaces that profile's bands in
    /// place; a new profile is appended in file order. The statuses recorded what
    /// the dialog showed at preview time, while the merge decides by the name it
    /// finds, so a set changed between the preview and the merge still merges
    /// sensibly. The profiles are rebuilt from the preview before the set is
    /// touched, so a refused merge leaves the set exactly as it was.
    ///
    /// # Errors
    ///
    /// [`ImportError::Blocked`] when [`Self::is_blocked`] is true; nothing is
    /// merged.
    ///
    /// # Panics
    ///
    /// Panics only when a preview is internally inconsistent: an importable item
    /// without its rounded bands, a blocked item reaching the rebuild, a band
    /// outside the measured range, or a saved name that changed between the
    /// duplicate check and the merge. [`preview_import`] is the only way to build
    /// an [`ImportPreview`], and it validates every item, so the API cannot
    /// produce such a preview; these are invariant assertions, not an input
    /// error the caller can trigger.
    pub fn merge_into(&self, saved: &mut ProfileSet) -> Result<(), ImportError> {
        if self.is_blocked() {
            return Err(ImportError::Blocked);
        }
        let profiles = self
            .items
            .iter()
            .map(|item| {
                let bands = item
                    .bands
                    .expect("an importable preview item carries its rounded bands");
                EqualizerProfile::new(item.name.clone(), bands)
                    .expect("preview items are validated when the preview is built")
            })
            .collect::<Vec<_>>();
        for profile in profiles {
            if saved.contains(&profile.name) {
                saved
                    .replace(profile)
                    .expect("the saved name was just found");
            } else {
                saved
                    .add(profile)
                    .expect("a preview carries valid, unique names");
            }
        }
        Ok(())
    }
}

/// Parses an OpenSCL4 transfer-profile file and previews importing it into
/// `saved`.
///
/// `[]` is a valid, empty document. The text is never changed and `saved` is
/// only read, so a preview can be shown before anything is merged.
///
/// # Errors
///
/// [`ImportError::NotProfiles`] when the text is not a JSON array of transfer
/// profiles, [`ImportError::InvalidName`] when a profile name is empty or too
/// long, and [`ImportError::DuplicateName`] when the file repeats a trimmed
/// name.
pub fn preview_import(text: &str, saved: &ProfileSet) -> Result<ImportPreview, ImportError> {
    let raw: Vec<RawTransferProfile> =
        serde_json::from_str(text).map_err(ImportError::NotProfiles)?;
    let mut names = HashSet::with_capacity(raw.len());
    let mut items = Vec::with_capacity(raw.len());
    for (index, raw) in raw.into_iter().enumerate() {
        let name = normalize_name(&raw.name)
            .map_err(|source| ImportError::InvalidName { index, source })?;
        if !names.insert(name.to_owned()) {
            return Err(ImportError::DuplicateName {
                name: name.to_owned(),
            });
        }
        let band_count = raw.volume_adjustments.len();
        let (status, bands) = if band_count == VISIBLE_EQ_BANDS {
            match rounded_bands(&raw.volume_adjustments) {
                Some(bands) => {
                    let status = if saved.contains(name) {
                        ImportStatus::ReplacesExisting
                    } else {
                        ImportStatus::New
                    };
                    (status, Some(bands))
                }
                None => (ImportStatus::OutOfRange, None),
            }
        } else {
            (ImportStatus::WrongBandCount, None)
        };
        items.push(ImportItem {
            name: name.to_owned(),
            band_count,
            status,
            bands,
        });
    }
    Ok(ImportPreview { items })
}

/// The wire shape of one OpenSCL4 transfer profile.
///
/// Deserialization goes through this type so a malformed document is a single
/// typed [`ImportError::NotProfiles`]; unknown fields are ignored by serde's
/// default behavior.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTransferProfile {
    name: String,
    volume_adjustments: Vec<f64>,
}

/// Converts the file's dB values to rounded dB×10 bands, or `None` when a band
/// is outside the measured range after rounding.
///
/// The caller only passes exactly [`VISIBLE_EQ_BANDS`] values.
fn rounded_bands(values: &[f64]) -> Option<[i16; VISIBLE_EQ_BANDS]> {
    debug_assert_eq!(values.len(), VISIBLE_EQ_BANDS);
    let allowed = f64::from(EQ_BAND_MIN)..=f64::from(EQ_BAND_MAX);
    let mut bands = [0i16; VISIBLE_EQ_BANDS];
    for (band, &decibels) in bands.iter_mut().zip(values) {
        let scaled = scale_to_tenths(decibels);
        if !allowed.contains(&scaled) {
            return None;
        }
        *band = scaled as i16;
    }
    Some(bands)
}

/// OpenSCL4's `scaled_to_device`: decibels × 10 rounded to the nearest
/// dB×10, halves away from zero.
fn scale_to_tenths(decibels: f64) -> f64 {
    (decibels * 10.0).round()
}

/// Why an OpenSCL4 transfer-profile import was rejected or refused.
#[derive(Debug)]
pub enum ImportError {
    /// The text is not a JSON array of transfer profiles: it is not valid JSON,
    /// not an array, or an entry is missing a `name` or numeric
    /// `volumeAdjustments`.
    NotProfiles(serde_json::Error),
    /// The profile at this zero-based array index has a name the saved-profile
    /// model rejects.
    InvalidName {
        /// Zero-based array index of the profile.
        index: usize,
        /// The name rejection: empty after trimming or too long.
        source: ProfileError,
    },
    /// The file carries the same trimmed name more than once.
    DuplicateName {
        /// The repeated trimmed name.
        name: String,
    },
    /// Merging was refused because at least one preview item is blocked, so the
    /// saved set was left unchanged.
    Blocked,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotProfiles(error) => {
                write!(
                    f,
                    "imported text is not an equalizer profiles file: {error}"
                )
            }
            Self::InvalidName { index, source } => {
                write!(
                    f,
                    "imported profile at index {index} has an invalid name: {source}"
                )
            }
            Self::DuplicateName { name } => {
                write!(f, "imported profiles repeat the name {name:?}")
            }
            Self::Blocked => write!(
                f,
                "imported profiles are blocked, so no saved profile was changed"
            ),
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotProfiles(error) => Some(error),
            Self::InvalidName { source, .. } => Some(source),
            Self::DuplicateName { .. } | Self::Blocked => None,
        }
    }
}
