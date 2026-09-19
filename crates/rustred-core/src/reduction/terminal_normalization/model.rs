use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::{OrderingPolicy, symmetry::VerifiedMap};

/// Reasons why the deliberately narrow routing lanes leave a terminal alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProductSkipReason {
    ExternalMomenta,
    AnalyticPowerShifts,
    NumeratorPowers,
    ActiveLineCount,
    NonUnitMass,
    NonIntegerQuadratic,
    NotLinearSquare,
    SingularMomentumBasis,
    NonUnimodularMomentumBasis,
    ConditionalMomentumMap,
    NonIntegralMomentumMap,
    NonUnimodularMomentumMap,
    ParametricPreparationLimit,
}

/// Preparation diagnostics; no count denotes an independent-master census.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalAliasStatistics {
    pub raw_terminals: usize,
    pub eligible_products: usize,
    /// Full-rank `L+1`-line keys admitted by the optional routing extension.
    pub eligible_corank_one: usize,
    /// Distinct active supports examined by the optional routing extension.
    pub analyzed_corank_one_supports: usize,
    /// Native momentum proposals, cached per denominator within each lane.
    pub analyzed_denominators: usize,
    /// Positive-power keys admitted to the separately selected U-polynomial lane.
    pub eligible_parametric: usize,
    pub analyzed_parametric_supports: usize,
    pub parametric_canonicalizations: usize,
    pub verified_aliases: usize,
    pub canonical_terminals: usize,
    pub skipped: BTreeMap<ProductSkipReason, usize>,
}

impl TerminalAliasStatistics {
    pub(super) fn skip(&mut self, reason: ProductSkipReason) {
        *self.skipped.entry(reason).or_default() += 1;
    }
}

/// An exact, unit-coefficient, one-hop equality between declared terminals.
/// The source-to-representative exact witness is checked during preparation.
#[derive(Clone, Debug)]
pub struct VerifiedTerminalAlias {
    pub(super) representative: IntegralKey,
    pub(super) witness: TerminalAliasWitness,
}

/// Distinct exact justifications for the same unit-coefficient terminal API.
/// A parameter relabeling is not claimed to be a loop-momentum transformation.
#[derive(Clone, Debug)]
pub enum TerminalAliasWitness {
    Momentum(Arc<VerifiedMap>),
    Parametric(Arc<super::parametric::VerifiedVacuumParameterMap>),
}

impl TerminalAliasWitness {
    pub fn as_momentum(&self) -> Option<&VerifiedMap> {
        match self {
            Self::Momentum(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_parametric(&self) -> Option<&super::parametric::VerifiedVacuumParameterMap> {
        match self {
            Self::Parametric(map) => Some(map),
            _ => None,
        }
    }
}

impl VerifiedTerminalAlias {
    pub fn representative(&self) -> &IntegralKey {
        &self.representative
    }

    pub fn witness(&self) -> &TerminalAliasWitness {
        &self.witness
    }
}

/// Immutable same-family terminal equalities, not a closure certificate.
///
/// Raw declarations are retained verbatim. Representatives are a subset of
/// those declarations; aliases never create terminals, change the positive
/// power multiset, use an oracle basis, or assert master independence. This service is not connected
/// to the candidate reducer by default.
#[derive(Clone, Debug)]
pub struct TerminalAliasPlan {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) arity: usize,
    pub(super) ordering: OrderingPolicy,
    pub(super) raw: BTreeSet<IntegralKey>,
    pub(super) canonical: BTreeSet<IntegralKey>,
    pub(super) aliases: BTreeMap<IntegralKey, VerifiedTerminalAlias>,
    pub(super) statistics: TerminalAliasStatistics,
}

impl TerminalAliasPlan {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub fn raw_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.raw
    }

    pub fn canonical_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.canonical
    }

    pub fn aliases(&self) -> &BTreeMap<IntegralKey, VerifiedTerminalAlias> {
        &self.aliases
    }

    pub fn statistics(&self) -> &TerminalAliasStatistics {
        &self.statistics
    }

    /// Check family ownership and resolve exactly one declared key. Unknown
    /// keys are errors, not implicit new terminals. No proof is rerun here.
    pub fn representative(
        &self,
        family: &IntegralFamily,
        key: &IntegralKey,
    ) -> Result<&IntegralKey, TerminalAliasError> {
        if family.fingerprint() != self.family_fingerprint() {
            return Err(TerminalAliasError::WrongFamily);
        }
        if key.powers().len() != self.arity {
            return Err(TerminalAliasError::WrongArity {
                expected: self.arity,
                actual: key.powers().len(),
            });
        }
        if let Some(alias) = self.aliases.get(key) {
            return Ok(alias.representative());
        }
        self.raw
            .get(key)
            .ok_or(TerminalAliasError::UndeclaredTerminal)
    }
}

/// Malformed input or failure of exact preparation. Unsupported mathematical
/// classes instead remain unchanged and are counted in `skipped`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalAliasError {
    WrongFamily,
    WrongArity { expected: usize, actual: usize },
    UndeclaredTerminal,
    InvalidCoefficientContext,
    Ordering(String),
    ExactAlgebra(String),
    MomentumVerification(String),
    InvalidProductWitness,
    InvalidCircuitWitness,
    InvalidParametricWitness,
}

impl fmt::Display for TerminalAliasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => f.write_str("terminal aliases belong to another family"),
            Self::WrongArity { expected, actual } => {
                write!(f, "terminal has arity {actual}; expected {expected}")
            }
            Self::UndeclaredTerminal => f.write_str("terminal was not declared in this alias plan"),
            Self::InvalidCoefficientContext => {
                f.write_str("foreign terminal-normalization coefficient context")
            }
            Self::Ordering(error) => write!(f, "terminal ordering: {error}"),
            Self::ExactAlgebra(error) => write!(f, "terminal product algebra: {error}"),
            Self::MomentumVerification(error) => {
                write!(f, "terminal momentum verification: {error}")
            }
            Self::InvalidProductWitness => {
                f.write_str("momentum map does not prove the proposed unit terminal equality")
            }
            Self::InvalidCircuitWitness => {
                f.write_str("native momentum circuit failed its exact replay")
            }
            Self::InvalidParametricWitness => {
                f.write_str("vacuum parameter permutation failed its exact U replay")
            }
        }
    }
}

impl std::error::Error for TerminalAliasError {}
