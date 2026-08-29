use std::fmt;
use std::str::FromStr;

/// Canonical family presets for closing-artifact generation.
///
/// This is a semantic family selector, not a graph/topology name.  Stage 1
/// currently installs only the one-denominator member; the enum is the
/// transport-neutral extension point for the `K=3` and `K=6` uniform-mass
/// vacuum families once their closure verifiers are available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosingFamilySelector {
    UnitMassVacuumK1,
}

impl ClosingFamilySelector {
    pub const EXPECTED_VALUES: &str = "unit-mass-vacuum-k1";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnitMassVacuumK1 => "unit-mass-vacuum-k1",
        }
    }
}

impl FromStr for ClosingFamilySelector {
    type Err = ParseClosingFamilySelectorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "unit-mass-vacuum-k1" => Ok(Self::UnitMassVacuumK1),
            _ => Err(ParseClosingFamilySelectorError {
                value: value.to_owned(),
            }),
        }
    }
}

/// Transport-neutral failure to parse a [`ClosingFamilySelector`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseClosingFamilySelectorError {
    value: String,
}

impl ParseClosingFamilySelectorError {
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ParseClosingFamilySelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid closing family selector {:?}; expected {}",
            self.value,
            ClosingFamilySelector::EXPECTED_VALUES
        )
    }
}

impl std::error::Error for ParseClosingFamilySelectorError {}

/// Accepted source encodings for family and campaign inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Toml,
    Symbolica,
}

impl InputFormat {
    pub const EXPECTED_VALUES: &str = "auto, toml, or symbolica";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Toml => "toml",
            Self::Symbolica => "symbolica",
        }
    }
}

impl FromStr for InputFormat {
    type Err = ParseInputFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "toml" => Ok(Self::Toml),
            "symbolica" => Ok(Self::Symbolica),
            _ => Err(ParseInputFormatError {
                value: value.to_owned(),
            }),
        }
    }
}

/// A transport-neutral failure to parse an [`InputFormat`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseInputFormatError {
    value: String,
}

impl ParseInputFormatError {
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ParseInputFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid input format {:?}; expected {}",
            self.value,
            InputFormat::EXPECTED_VALUES
        )
    }
}

impl std::error::Error for ParseInputFormatError {}

/// Relation families to include in one derivation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationSelection {
    All,
    Ordinary,
    LorentzInvariance,
}

impl RelationSelection {
    pub const EXPECTED_VALUES: &str = "all, ordinary, or li";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ordinary => "ordinary",
            Self::LorentzInvariance => "li",
        }
    }

    pub const fn includes_ordinary(self) -> bool {
        matches!(self, Self::All | Self::Ordinary)
    }

    pub const fn includes_li(self) -> bool {
        matches!(self, Self::All | Self::LorentzInvariance)
    }
}

impl FromStr for RelationSelection {
    type Err = ParseRelationSelectionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "ordinary" => Ok(Self::Ordinary),
            "li" => Ok(Self::LorentzInvariance),
            _ => Err(ParseRelationSelectionError {
                value: value.to_owned(),
            }),
        }
    }
}

/// A transport-neutral failure to parse a [`RelationSelection`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseRelationSelectionError {
    value: String,
}

impl ParseRelationSelectionError {
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ParseRelationSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid relation selection {:?}; expected {}",
            self.value,
            RelationSelection::EXPECTED_VALUES
        )
    }
}

impl std::error::Error for ParseRelationSelectionError {}
