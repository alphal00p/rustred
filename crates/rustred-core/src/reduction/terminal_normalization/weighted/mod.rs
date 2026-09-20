//! Finite, one-hop terminal normalization by exact support symmetries.
//!
//! This does not generate IBPs, certify coverage or assert master independence.

mod prepare;
mod projection;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use symbolica::prelude::Rational;

use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::persistence::{BinaryIoError, BinaryIoLimits};
use crate::sector::{OrderingPolicy, symmetry::VerifiedMap, zero};

use super::{ProductSkipReason, TerminalAliasError, TerminalAliasPlan, VacuumParametricLimits};

/// Resource bounds, independent of loop count and topology names.
#[derive(Clone, Copy, Debug)]
pub struct TerminalNormalizationLimits {
    pub parametric: VacuumParametricLimits,
    pub max_terminals: usize,
    pub max_supports: usize,
    pub max_matrix_cells: usize,
    pub max_output_terms: usize,
}

impl Default for TerminalNormalizationLimits {
    fn default() -> Self {
        Self {
            parametric: VacuumParametricLimits::default(),
            max_terminals: 1_000_000,
            max_supports: 100_000,
            max_matrix_cells: 1_000_000,
            max_output_terms: 4_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalNormalizationSkipReason {
    UnsupportedGeometry(ProductSkipReason),
    NumeratorShape,
    NonUnitCircuit,
    UnboundPositiveOutput,
    NonDescendingOutput,
    IncompleteSymmetrySpan,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalNormalizationStatistics {
    pub raw_terminals: usize,
    pub unit_aliases: usize,
    pub analyzed_supports: usize,
    pub verified_generators: usize,
    pub projected_numerators: usize,
    pub canonical_terminals: usize,
    pub skipped: BTreeMap<TerminalNormalizationSkipReason, usize>,
}

/// Independently verified changes of variables and their complete affine rows.
#[derive(Clone, Debug)]
pub struct VerifiedNumeratorSupport {
    slots: Vec<usize>,
    generators: Vec<Arc<VerifiedMap>>,
    /// First column is 1, next columns are the active D_i, then S_i-g(S_i).
    columns: Vec<Vec<Rational>>,
}

impl VerifiedNumeratorSupport {
    pub fn slots(&self) -> &[usize] {
        &self.slots
    }
    pub fn generators(&self) -> &[Arc<VerifiedMap>] {
        &self.generators
    }
    pub fn affine_columns(&self) -> &[Vec<Rational>] {
        &self.columns
    }
}

/// Exact native combination that was multiplied back into the complete rows.
#[derive(Clone, Debug)]
pub struct TerminalProjectionWitness {
    target: IntegralKey,
    support: Arc<VerifiedNumeratorSupport>,
    combination: Vec<Rational>,
    unbound_terms: BTreeMap<IntegralKey, Coefficient>,
}

impl TerminalProjectionWitness {
    pub fn target(&self) -> &IntegralKey {
        &self.target
    }
    pub fn support(&self) -> &VerifiedNumeratorSupport {
        &self.support
    }
    pub fn native_combination(&self) -> &[Rational] {
        &self.combination
    }
    pub fn projected_terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.unbound_terms
    }
}

/// Immutable finite output convention, flattened onto already declared keys.
///
/// Every output is a fixed identity of this plan, so applying a terminal never
/// recursively applies another normalization. Raw declarations stay unchanged.
/// The current factory admits unshifted unit-mass vacuum L+1-line supports with
/// unit primitive circuits and one quadratic numerator; unsupported keys are
/// retained. All algebra and exact matrix operations are Symbolica native.
#[derive(Clone, Debug)]
pub struct TerminalNormalizationPlan {
    family: Arc<String>,
    ordering: OrderingPolicy,
    raw: BTreeSet<IntegralKey>,
    canonical: BTreeSet<IntegralKey>,
    terms: BTreeMap<IntegralKey, BTreeMap<IntegralKey, Coefficient>>,
    witnesses: BTreeMap<IntegralKey, TerminalProjectionWitness>,
    positive_aliases: TerminalAliasPlan,
    positive_bindings: BTreeMap<IntegralKey, IntegralKey>,
    zero_certificates: BTreeMap<IntegralKey, Arc<zero::Certificate>>,
    statistics: TerminalNormalizationStatistics,
}

impl TerminalNormalizationPlan {
    pub fn vacuum_quadratic_numerators(
        family: &IntegralFamily,
        raw: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
        limits: TerminalNormalizationLimits,
    ) -> Result<Self, TerminalNormalizationError> {
        prepare::prepare(family, raw, ordering, limits)
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family
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
    pub fn terms(&self) -> &BTreeMap<IntegralKey, BTreeMap<IntegralKey, Coefficient>> {
        &self.terms
    }
    pub fn projection_witnesses(&self) -> &BTreeMap<IntegralKey, TerminalProjectionWitness> {
        &self.witnesses
    }
    pub fn positive_aliases(&self) -> &TerminalAliasPlan {
        &self.positive_aliases
    }
    pub fn positive_bindings(&self) -> &BTreeMap<IntegralKey, IntegralKey> {
        &self.positive_bindings
    }
    pub fn zero_certificates(&self) -> &BTreeMap<IntegralKey, Arc<zero::Certificate>> {
        &self.zero_certificates
    }
    pub fn statistics(&self) -> &TerminalNormalizationStatistics {
        &self.statistics
    }

    /// Encode native coefficients and a small versioned structural recipe.
    pub fn encode_native(&self, limits: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        crate::persistence::terminal_normalization::encode(self, limits)
    }

    /// Import trusted generated Symbolica data and independently rebuild the
    /// same finite plan. Every key/coefficient and both ordered native maps must
    /// agree. Proof work happens once here, never in the reduction hot path.
    /// This grants no IBP-source, closure or minimal-master authority.
    pub fn decode_generated(
        bytes: &[u8],
        family: &IntegralFamily,
        raw: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
        preparation: TerminalNormalizationLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, TerminalNormalizationError> {
        crate::persistence::terminal_normalization::decode(
            bytes,
            family,
            raw,
            ordering,
            preparation,
            io,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalNormalizationError {
    Alias(TerminalAliasError),
    Binary(BinaryIoError),
    ExactAlgebra(String),
    InvalidWitness(&'static str),
    Limit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

impl fmt::Display for TerminalNormalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(e) => e.fmt(f),
            Self::Binary(e) => e.fmt(f),
            Self::ExactAlgebra(e) => write!(f, "terminal normalization exact algebra: {e}"),
            Self::InvalidWitness(e) => write!(f, "invalid terminal normalization witness: {e}"),
            Self::Limit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "terminal normalization {resource} requires {requested}; limit is {limit}"
            ),
        }
    }
}
impl std::error::Error for TerminalNormalizationError {}
impl From<TerminalAliasError> for TerminalNormalizationError {
    fn from(e: TerminalAliasError) -> Self {
        Self::Alias(e)
    }
}
impl From<BinaryIoError> for TerminalNormalizationError {
    fn from(e: BinaryIoError) -> Self {
        Self::Binary(e)
    }
}

fn check(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TerminalNormalizationError> {
    if requested > limit {
        Err(TerminalNormalizationError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
fn algebra(e: impl fmt::Display) -> TerminalNormalizationError {
    TerminalNormalizationError::ExactAlgebra(e.to_string())
}
