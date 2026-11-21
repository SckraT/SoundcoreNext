//! Saved equalizer profiles for the A3062 custom equalizer.
//!
//! A profile is a name plus the eight visible A3062 equalizer bands, in the same
//! raw dB×10 units as the parsed device state: `0` is `0 dB`, `-120` is
//! `-12.0 dB` and `134` is `+13.4 dB`. This crate is pure: it has no
//! filesystem, UI or platform dependencies. The desktop controller owns file
//! IO and calls [`ProfileSet::from_json`] / [`ProfileSet::to_json`].
//!
//! # JSON schema v1
//!
//! A saved-profile document is one JSON array of profile objects:
//!
//! ```json
//! [
//!   { "name": "Warm", "bands": [30, -30, 60, 0, -60, 20, 0, -20] },
//!   { "name": "Flat", "bands": [0, 0, 0, 0, 0, 0, 0, 0] }
//! ]
//! ```
//!
//! - `name` is trimmed and must be 1..=[`MAX_PROFILE_NAME_LEN`] characters long;
//!   names are unique and match exactly after trimming (so `Warm` and `warm`
//!   are two different names).
//! - `bands` is exactly [`VISIBLE_EQ_BANDS`] raw dB×10 values in
//!   [`EQ_BAND_MIN`]..=[`EQ_BAND_MAX`].
//! - Unknown object fields are ignored on read and never written back.
//! - The array order is the profile order and is preserved exactly, so the
//!   serialized document is deterministic for a given operation sequence.
//! - Empty or whitespace-only content parses as the empty set, so a missing or
//!   never-written file is loaded without a special case.
//!
//! Invalid JSON, a wrong band count, an invalid name or band value, and
//! duplicate names are typed errors ([`ProfileJsonError`]); the controller turns
//! any of them into an empty set and a readable log line.

use serde::{Deserialize, Serialize};
use soundcore_models::{EQ_BAND_MAX, EQ_BAND_MIN, VISIBLE_EQ_BANDS};

/// The longest accepted profile name, counted in Unicode scalar values
/// (characters), not bytes.
pub const MAX_PROFILE_NAME_LEN: usize = 60;

/// A named snapshot of the eight visible A3062 equalizer bands.
///
/// Construct one with [`EqualizerProfile::new`], which trims the name and
/// validates the name and bands. The fields are public for reading, and
/// [`ProfileSet`] re-validates any profile it is given, so a hand-built profile
/// cannot enter a set unchecked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EqualizerProfile {
    /// The trimmed profile name, 1..=[`MAX_PROFILE_NAME_LEN`] characters.
    pub name: String,
    /// The eight visible bands in raw dB×10.
    pub bands: [i16; VISIBLE_EQ_BANDS],
}

impl EqualizerProfile {
    /// Creates a profile, trimming `name` and validating the name and bands.
    ///
    /// # Errors
    ///
    /// [`ProfileError::EmptyName`] when the trimmed name is empty,
    /// [`ProfileError::NameTooLong`] when it is longer than
    /// [`MAX_PROFILE_NAME_LEN`] characters, and
    /// [`ProfileError::BandOutOfRange`] when a band is outside the measured
    /// A3062 range.
    pub fn new(
        name: impl Into<String>,
        bands: [i16; VISIBLE_EQ_BANDS],
    ) -> Result<Self, ProfileError> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() {
            return Err(ProfileError::EmptyName);
        }
        let length = name.chars().count();
        if length > MAX_PROFILE_NAME_LEN {
            return Err(ProfileError::NameTooLong {
                actual: length,
                max: MAX_PROFILE_NAME_LEN,
            });
        }
        validate_bands(&bands)?;
        Ok(Self {
            name: name.to_owned(),
            bands,
        })
    }

    /// Re-validates and normalizes a profile supplied through the public fields.
    fn normalized(self) -> Result<Self, ProfileError> {
        Self::new(self.name, self.bands)
    }
}

/// Validates that every band is inside the measured A3062 dB×10 range.
fn validate_bands(bands: &[i16; VISIBLE_EQ_BANDS]) -> Result<(), ProfileError> {
    for (index, &value) in bands.iter().enumerate() {
        if !(EQ_BAND_MIN..=EQ_BAND_MAX).contains(&value) {
            return Err(ProfileError::BandOutOfRange {
                index,
                value,
                min: EQ_BAND_MIN,
                max: EQ_BAND_MAX,
            });
        }
    }
    Ok(())
}

/// An ordered, duplicate-free collection of saved profiles.
///
/// Profiles keep insertion order; that order is what [`Self::to_json`] writes
/// and what [`Self::from_json`] keeps, so a load-and-save roundtrip is stable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileSet {
    profiles: Vec<EqualizerProfile>,
}

impl ProfileSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of saved profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether the set has no profiles.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// The saved profiles in save order.
    pub fn profiles(&self) -> &[EqualizerProfile] {
        &self.profiles
    }

    /// Iterates over the saved profiles in save order.
    pub fn iter(&self) -> impl Iterator<Item = &EqualizerProfile> {
        self.profiles.iter()
    }

    /// The profile named `name`, or `None` when no profile has that name.
    ///
    /// The lookup name is trimmed, so a name as typed by the user matches the
    /// trimmed name the set stores. Matching is case-sensitive: `Warm` and
    /// `warm` are different profiles.
    pub fn get(&self, name: &str) -> Option<&EqualizerProfile> {
        let name = name.trim();
        self.profiles.iter().find(|profile| profile.name == name)
    }

    /// Whether a profile named `name` is saved; the lookup name is trimmed.
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Adds a validated profile, trimming its name.
    ///
    /// # Errors
    ///
    /// The same validation errors as [`EqualizerProfile::new`], and
    /// [`ProfileError::DuplicateName`] when the trimmed name is already saved.
    /// A rejected profile never changes the set.
    pub fn add(&mut self, profile: EqualizerProfile) -> Result<(), ProfileError> {
        let profile = profile.normalized()?;
        if self.contains(&profile.name) {
            return Err(ProfileError::DuplicateName { name: profile.name });
        }
        self.profiles.push(profile);
        Ok(())
    }

    /// Replaces the bands of an existing profile, keeping its position.
    ///
    /// # Errors
    ///
    /// The same validation errors as [`EqualizerProfile::new`], and
    /// [`ProfileError::NotFound`] when no saved profile has that name; use
    /// [`Self::add`] for a new profile.
    pub fn replace(&mut self, profile: EqualizerProfile) -> Result<(), ProfileError> {
        let profile = profile.normalized()?;
        match self
            .profiles
            .iter()
            .position(|existing| existing.name == profile.name)
        {
            Some(index) => {
                self.profiles[index] = profile;
                Ok(())
            }
            None => Err(ProfileError::NotFound { name: profile.name }),
        }
    }

    /// Removes and returns the profile named `name`, if it is saved.
    ///
    /// The lookup name is trimmed.
    pub fn remove(&mut self, name: &str) -> Option<EqualizerProfile> {
        let name = name.trim();
        let index = self
            .profiles
            .iter()
            .position(|profile| profile.name == name)?;
        Some(self.profiles.remove(index))
    }

    /// Serializes the set as JSON schema v1.
    ///
    /// The output is compact and deterministic: the array keeps the set order
    /// and every object writes `name`, then `bands`. Serializing this plain
    /// data cannot fail.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.profiles).expect("a profile set always serializes")
    }

    /// Parses JSON schema v1.
    ///
    /// Empty or whitespace-only content parses as the empty set. Every entry is
    /// validated exactly like a profile added through [`Self::add`], so a
    /// malformed document never yields a half-loaded set.
    ///
    /// # Errors
    ///
    /// [`ProfileJsonError::InvalidJson`] when the text is not a JSON array of
    /// profile objects, [`ProfileJsonError::InvalidProfile`] when an entry
    /// fails validation, and [`ProfileJsonError::DuplicateName`] when the same
    /// name is saved twice.
    pub fn from_json(text: &str) -> Result<Self, ProfileJsonError> {
        if text.trim().is_empty() {
            return Ok(Self::new());
        }
        let raw: Vec<RawProfile> =
            serde_json::from_str(text).map_err(ProfileJsonError::InvalidJson)?;
        let mut set = Self::new();
        for (index, raw) in raw.into_iter().enumerate() {
            let profile = raw.into_profile(index)?;
            set.add(profile).map_err(|error| match error {
                ProfileError::DuplicateName { name } => ProfileJsonError::DuplicateName { name },
                other => ProfileJsonError::InvalidProfile {
                    index,
                    source: other,
                },
            })?;
        }
        Ok(set)
    }
}

/// The wire shape of one profile object.
///
/// Deserialization goes through this type instead of validating in serde
/// callbacks so the band count is a typed [`ProfileError::WrongBandCount`]
/// rather than an opaque serde message. Unknown JSON fields are ignored by
/// serde's default behavior.
#[derive(Deserialize)]
struct RawProfile {
    name: String,
    bands: Vec<i16>,
}

impl RawProfile {
    /// Validates one parsed profile object, naming its array index in errors.
    fn into_profile(self, index: usize) -> Result<EqualizerProfile, ProfileJsonError> {
        let actual = self.bands.len();
        let bands: [i16; VISIBLE_EQ_BANDS] =
            self.bands
                .try_into()
                .map_err(|_| ProfileJsonError::InvalidProfile {
                    index,
                    source: ProfileError::WrongBandCount {
                        actual,
                        expected: VISIBLE_EQ_BANDS,
                    },
                })?;
        EqualizerProfile::new(self.name, bands)
            .map_err(|source| ProfileJsonError::InvalidProfile { index, source })
    }
}

/// Why a profile was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// The profile name is empty or only whitespace after trimming.
    EmptyName,
    /// The trimmed profile name is longer than [`MAX_PROFILE_NAME_LEN`]
    /// characters.
    NameTooLong {
        /// The trimmed name length in characters.
        actual: usize,
        /// The accepted maximum, [`MAX_PROFILE_NAME_LEN`].
        max: usize,
    },
    /// The profile does not carry exactly [`VISIBLE_EQ_BANDS`] bands.
    WrongBandCount {
        /// The number of bands the profile carries.
        actual: usize,
        /// The required band count, [`VISIBLE_EQ_BANDS`].
        expected: usize,
    },
    /// A band value is outside the measured A3062 raw dB×10 range.
    BandOutOfRange {
        /// Zero-based index of the offending band.
        index: usize,
        /// The rejected value in raw dB×10.
        value: i16,
        /// The lowest accepted value, [`EQ_BAND_MIN`].
        min: i16,
        /// The highest accepted value, [`EQ_BAND_MAX`].
        max: i16,
    },
    /// A profile with this trimmed name is already saved.
    DuplicateName {
        /// The duplicate trimmed name.
        name: String,
    },
    /// No saved profile has this trimmed name.
    NotFound {
        /// The missing trimmed name.
        name: String,
    },
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "profile name is empty"),
            Self::NameTooLong { actual, max } => {
                write!(f, "profile name is {actual} characters, maximum is {max}")
            }
            Self::WrongBandCount { actual, expected } => {
                write!(f, "profile has {actual} bands, expected {expected}")
            }
            Self::BandOutOfRange {
                index,
                value,
                min,
                max,
            } => write!(
                f,
                "band {index} value {value} is outside the measured range {min}..={max} dB×10"
            ),
            Self::DuplicateName { name } => {
                write!(f, "a profile named {name:?} is already saved")
            }
            Self::NotFound { name } => write!(f, "no profile named {name:?} is saved"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// Why a JSON profile document was rejected.
#[derive(Debug)]
pub enum ProfileJsonError {
    /// The document is not valid JSON or not a JSON array of profile objects.
    InvalidJson(serde_json::Error),
    /// The profile at this zero-based array index failed validation.
    InvalidProfile {
        /// Zero-based array index of the invalid profile.
        index: usize,
        /// The validation failure.
        source: ProfileError,
    },
    /// The document saves the same trimmed name more than once.
    DuplicateName {
        /// The duplicate trimmed name.
        name: String,
    },
}

impl std::fmt::Display for ProfileJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "profile file is not valid JSON: {error}"),
            Self::InvalidProfile { index, source } => {
                write!(f, "profile at index {index} is invalid: {source}")
            }
            Self::DuplicateName { name } => {
                write!(f, "profile file saves the name {name:?} more than once")
            }
        }
    }
}

impl std::error::Error for ProfileJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(error) => Some(error),
            Self::InvalidProfile { source, .. } => Some(source),
            Self::DuplicateName { .. } => None,
        }
    }
}
