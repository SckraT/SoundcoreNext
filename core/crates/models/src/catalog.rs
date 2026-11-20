//! Model catalog grouped by physical shape and by families with shared
//! capabilities.
//!
//! This is navigation metadata, not a capability claim. Protocol support still
//! comes from the state parser and the settings model; a trait here means "this
//! family is expected to expose this class of control", with the evidence note
//! on every family.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FormFactor {
    /// True-wireless earbuds.
    TwsEarbuds,
    /// Over-ear and on-ear headphones.
    Headphones,
    /// Open-ear clips and hooks.
    OpenEar,
    /// Portable speakers.
    Speaker,
    /// Earbuds designed for sleep.
    SleepBuds,
}

impl FormFactor {
    pub const ALL: [Self; 5] = [
        Self::TwsEarbuds,
        Self::Headphones,
        Self::OpenEar,
        Self::Speaker,
        Self::SleepBuds,
    ];

    pub const fn title_ru(self) -> &'static str {
        match self {
            Self::TwsEarbuds => "TWS-наушники",
            Self::Headphones => "Полноразмерные наушники",
            Self::OpenEar => "Открытые наушники",
            Self::Speaker => "Портативные колонки",
            Self::SleepBuds => "Наушники для сна",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    Flagship,
    Midrange,
    Budget,
    Sport,
    OpenEar,
    Sleep,
    Speaker,
    Legacy,
}

/// A broad control class expected in a family. It is intentionally coarser
/// than a `SettingId`: the UI uses it for grouping and copy, not for deciding
/// whether a concrete setting exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Anc,
    AdaptiveAnc,
    Transparency,
    WindNoiseSuppression,
    SpatialAudio,
    Ldac,
    Multipoint,
    CaseBattery,
    WirelessCharging,
    EarDetection,
    ParametricEq,
    TenBandEq,
    CaseDisplay,
    EasyChat,
    TouchControls,
    ButtonGestures,
    AirplaneMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatteryTopology {
    Single,
    Dual,
    DualWithCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Confirmed by the product name, photos or observed fields.
    Confirmed,
    /// Derived from the product line and battery shape; verify against a
    /// product page or a real unit.
    Inferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFamily {
    pub id: &'static str,
    pub title: &'static str,
    pub form_factor: FormFactor,
    pub tier: Tier,
    pub shared: &'static [Capability],
    pub evidence: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogModel {
    pub code: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub battery: BatteryTopology,
    pub confidence: Confidence,
}

pub const FAMILIES: &[ModelFamily] = &[
    ModelFamily {
        id: "liberty-flagship",
        title: "Liberty Pro / флагманы",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Flagship,
        shared: &[
            Capability::Anc,
            Capability::AdaptiveAnc,
            Capability::Transparency,
            Capability::WindNoiseSuppression,
            Capability::SpatialAudio,
            Capability::Ldac,
            Capability::Multipoint,
            Capability::CaseBattery,
            Capability::EarDetection,
            Capability::ParametricEq,
            Capability::CaseDisplay,
        ],
        evidence: "Product line and the rich A3954 observed fields: case display, spatial audio, adaptive ANC, parametric EQ.",
    },
    ModelFamily {
        id: "liberty-nc",
        title: "Liberty NC / средний сегмент",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Midrange,
        shared: &[
            Capability::Anc,
            Capability::Transparency,
            Capability::WindNoiseSuppression,
            Capability::Ldac,
            Capability::Multipoint,
            Capability::CaseBattery,
            Capability::TouchControls,
        ],
        evidence: "Liberty 4 NC is the only supported member; shared list is intentionally conservative.",
    },
    ModelFamily {
        id: "p-series",
        title: "P-серия и A-серия (бюджет)",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Budget,
        shared: &[
            Capability::Anc,
            Capability::Transparency,
            Capability::Multipoint,
            Capability::TouchControls,
        ],
        evidence: "P20i/P25i/R50i, A20i, P40i, P30i/R50i NC and P31i; ANC and multipoint depend on the exact model.",
    },
    ModelFamily {
        id: "life-legacy",
        title: "Life / Dot / Note (legacy)",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Legacy,
        shared: &[
            Capability::Anc,
            Capability::Transparency,
            Capability::TouchControls,
        ],
        evidence: "Older Life line; many units predate spatial audio and LDAC. Capability still comes from the state.",
    },
    ModelFamily {
        id: "space-tws",
        title: "Space A-серия (TWS)",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Midrange,
        shared: &[
            Capability::Anc,
            Capability::AdaptiveAnc,
            Capability::Transparency,
            Capability::Ldac,
            Capability::Multipoint,
            Capability::CaseBattery,
        ],
        evidence: "Space A40 is the single supported member; case scale is 10 while earbuds use 5.",
    },
    ModelFamily {
        id: "sport",
        title: "Sport",
        form_factor: FormFactor::TwsEarbuds,
        tier: Tier::Sport,
        shared: &[
            Capability::Anc,
            Capability::Transparency,
            Capability::TouchControls,
        ],
        evidence: "Sport X20; water resistance and fit are product traits, not protocol traits.",
    },
    ModelFamily {
        id: "space-headphones",
        title: "Space (полноразмерные)",
        form_factor: FormFactor::Headphones,
        tier: Tier::Flagship,
        shared: &[
            Capability::Anc,
            Capability::AdaptiveAnc,
            Capability::Transparency,
            Capability::WindNoiseSuppression,
            Capability::Ldac,
            Capability::Multipoint,
            Capability::EarDetection,
        ],
        evidence: "Space One, Space Q45 and Space One Pro; single battery, over-ear.",
    },
    ModelFamily {
        id: "q-headphones",
        title: "Q-серия и Life Tune (полноразмерные)",
        form_factor: FormFactor::Headphones,
        tier: Tier::Midrange,
        shared: &[
            Capability::Anc,
            Capability::Transparency,
            Capability::Multipoint,
            Capability::ButtonGestures,
        ],
        evidence: "Q20I, Q11i, Q30, Q35, Life Tune and Life Tune Pro; some are wired-only or lack LDAC.",
    },
    ModelFamily {
        id: "open-ear",
        title: "Открытые (клипсы и заушные)",
        form_factor: FormFactor::OpenEar,
        tier: Tier::OpenEar,
        shared: &[
            Capability::Multipoint,
            Capability::TouchControls,
            Capability::CaseBattery,
        ],
        evidence: "C30i, AeroClip, V20i and C50i; no ANC by design of the open form factor.",
    },
    ModelFamily {
        id: "speaker",
        title: "Портативные колонки",
        form_factor: FormFactor::Speaker,
        tier: Tier::Speaker,
        shared: &[Capability::Multipoint, Capability::ButtonGestures],
        evidence: "Motion+ is the only supported speaker; Motion X600 is known but not implemented.",
    },
    ModelFamily {
        id: "sleep",
        title: "Сон",
        form_factor: FormFactor::SleepBuds,
        tier: Tier::Sleep,
        shared: &[Capability::TouchControls],
        evidence: "Sleep A30; sleep-specific alarms and masking are separate from ANC.",
    },
];

pub const CATALOG: &[CatalogModel] = &[
    // Liberty flagship / Pro.
    CatalogModel {
        code: "A3909",
        name: "Liberty 2 Pro",
        family: "liberty-flagship",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3930",
        name: "Liberty 2 Pro+",
        family: "liberty-flagship",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3951",
        name: "Liberty Air 2 Pro",
        family: "liberty-flagship",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3952",
        name: "Liberty 3 Pro",
        family: "liberty-flagship",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3953",
        name: "Liberty 4",
        family: "liberty-flagship",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3954",
        name: "Liberty 4 Pro",
        family: "liberty-flagship",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3957",
        name: "Liberty 5",
        family: "liberty-flagship",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    // Liberty NC.
    CatalogModel {
        code: "A3947",
        name: "Liberty 4 NC",
        family: "liberty-nc",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    // P/A budget.
    CatalogModel {
        code: "A3948",
        name: "A20i",
        family: "p-series",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3949",
        name: "P20i / P25i / R50i",
        family: "p-series",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3955",
        name: "P40i",
        family: "p-series",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3959",
        name: "P30i / R50i NC",
        family: "p-series",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "D1202",
        name: "P31i",
        family: "p-series",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    // Life legacy.
    CatalogModel {
        code: "A3031",
        name: "Vortex",
        family: "life-legacy",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Inferred,
    },
    CatalogModel {
        code: "A3033",
        name: "Life 2 Neo",
        family: "life-legacy",
        battery: BatteryTopology::Single,
        confidence: Confidence::Inferred,
    },
    CatalogModel {
        code: "A3926",
        name: "Life Dot 2S",
        family: "life-legacy",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3931",
        name: "Life Dot 2 NC",
        family: "life-legacy",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3933",
        name: "Life Note 3",
        family: "life-legacy",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3935",
        name: "Life A2 NC",
        family: "life-legacy",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3939",
        name: "Life P3",
        family: "life-legacy",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3944",
        name: "Life P2 Mini",
        family: "life-legacy",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3945",
        name: "Life Note 3S",
        family: "life-legacy",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    // Space TWS, sport, headphones, open-ear, speaker, sleep.
    CatalogModel {
        code: "A3936",
        name: "Space A40",
        family: "space-tws",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3968",
        name: "Sport X20",
        family: "sport",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3035",
        name: "Space One",
        family: "space-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3040",
        name: "Space Q45",
        family: "space-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3062",
        name: "Space One Pro",
        family: "space-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3004",
        name: "Q20I",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3005",
        name: "Q11i",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Inferred,
    },
    CatalogModel {
        code: "A3027",
        name: "Life Q35",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3028",
        name: "Q30 / Life Q30",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3029",
        name: "Life Tune",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3030",
        name: "Life Tune Pro",
        family: "q-headphones",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3330",
        name: "C30i",
        family: "open-ear",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3388",
        name: "AeroClip",
        family: "open-ear",
        battery: BatteryTopology::DualWithCase,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3876",
        name: "V20i",
        family: "open-ear",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Inferred,
    },
    CatalogModel {
        code: "D1101",
        name: "C50i",
        family: "open-ear",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "A3116",
        name: "Motion+",
        family: "speaker",
        battery: BatteryTopology::Single,
        confidence: Confidence::Confirmed,
    },
    CatalogModel {
        code: "D1301",
        name: "Sleep A30",
        family: "sleep",
        battery: BatteryTopology::Dual,
        confidence: Confidence::Confirmed,
    },
];

pub fn family(id: &str) -> Option<&'static ModelFamily> {
    FAMILIES.iter().find(|family| family.id == id)
}

pub fn model(code: &str) -> Option<&'static CatalogModel> {
    CATALOG.iter().find(|model| model.code == code)
}

impl ModelFamily {
    pub fn models(&self) -> impl Iterator<Item = &'static CatalogModel> + '_ {
        CATALOG.iter().filter(move |model| model.family == self.id)
    }
}

/// Families in display order, grouped by physical shape.
pub fn grouped_families() -> Vec<(FormFactor, Vec<&'static ModelFamily>)> {
    FormFactor::ALL
        .into_iter()
        .map(|form_factor| {
            let families = FAMILIES
                .iter()
                .filter(|family| family.form_factor == form_factor)
                .collect();
            (form_factor, families)
        })
        .collect()
}
