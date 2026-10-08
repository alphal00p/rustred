use super::*;
use crate::algebra::IndexedAlgebraLimits;
use crate::reduction::terminal_normalization::{VacuumFamilyAliasError, VacuumFamilyAliasLimits};
use crate::reduction::terminal_relations::{TerminalEquation, TerminalRelationError};

/// Finite preparation limits; no automatic search escalation.
/// Source, column, replay, and flat-map counts cover the collection. Retained
/// matrix bounds are checked at phase boundaries and are not a bound on total
/// plan allocations or peak Symbolica scratch memory. Alias preparation has
/// the separate per-family and collection scopes documented by its limits.
#[derive(Clone, Copy, Debug)]
pub struct VacuumDiagonalCollectionLimits {
    pub max_corner_seeds: usize,
    pub max_source_rows: usize,
    pub max_source_terms: usize,
    pub max_columns: usize,
    pub max_reducer_nonzeros: usize,
    pub max_replay_operations: usize,
    pub max_flat_map_terms: usize,
    pub max_conditions: usize,
    /// Sum of numerator and denominator terms in one retained native matrix,
    /// authenticated at phase boundaries, not a peak or whole-plan term cap.
    pub max_coefficient_terms: usize,
    pub aliases: VacuumFamilyAliasLimits,
    pub algebra: IndexedAlgebraLimits,
}
impl Default for VacuumDiagonalCollectionLimits {
    fn default() -> Self {
        Self {
            max_corner_seeds: 128,
            max_source_rows: 2048,
            max_source_terms: 200_000,
            max_columns: 10_000,
            max_reducer_nonzeros: 1_000_000,
            max_replay_operations: 2_000_000,
            max_flat_map_terms: 100_000,
            max_conditions: 10_000,
            max_coefficient_terms: 2_000_000,
            aliases: Default::default(),
            algebra: Default::default(),
        }
    }
}

#[derive(Debug)]
pub enum VacuumCollectionError {
    Cancelled,
    Alias(VacuumFamilyAliasError),
    Source(TerminalRelationError),
    Unsupported {
        family: String,
        reason: &'static str,
    },
    WrongFamily,
    WrongArity,
    UndeclaredTerminal,
    OutputNotInReduction,
    VanishingCondition,
    ArithmeticOverflow,
    Limit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Algebra(String),
    Binary(String),
    ReplayFailed,
}
impl std::fmt::Display for VacuumCollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => {
                f.write_str("finite terminal feedback cancelled at a complete row boundary")
            }
            Self::Alias(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Unsupported { family, reason } => {
                write!(f, "unsupported scalar vacuum family {family}: {reason}")
            }
            Self::WrongFamily => f.write_str("vacuum collection: wrong family"),
            Self::WrongArity => f.write_str("vacuum collection: wrong physical arity"),
            Self::UndeclaredTerminal => f.write_str("vacuum collection: undeclared terminal"),
            Self::OutputNotInReduction => {
                f.write_str("vacuum collection: output is not in the requested reduction")
            }
            Self::VanishingCondition => f.write_str("vacuum collection: source condition vanishes"),
            Self::ArithmeticOverflow => {
                f.write_str("vacuum collection: resource or power arithmetic overflow")
            }
            Self::Limit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "vacuum collection {resource}: {requested} exceeds {limit}"
            ),
            Self::Algebra(e) => write!(f, "vacuum collection algebra: {e}"),
            Self::Binary(e) => write!(f, "vacuum collection binary: {e}"),
            Self::ReplayFailed => {
                f.write_str("vacuum collection: exact source replay or triangularity failed")
            }
        }
    }
}
impl std::error::Error for VacuumCollectionError {}
impl From<VacuumFamilyAliasError> for VacuumCollectionError {
    fn from(e: VacuumFamilyAliasError) -> Self {
        Self::Alias(e)
    }
}
impl From<TerminalRelationError> for VacuumCollectionError {
    fn from(e: TerminalRelationError) -> Self {
        Self::Source(e)
    }
}

/// One original specialized row, before summation or global column aliases.
#[derive(Clone, Debug)]
pub struct VacuumOrdinarySource {
    pub(super) row_id: RowId,
    pub(super) equation: TerminalEquation,
}
impl VacuumOrdinarySource {
    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }
    pub fn equation(&self) -> &TerminalEquation {
        &self.equation
    }
}
/// A unit-weight sum of native diagonal ordinary rows at one corner.
#[derive(Clone, Debug)]
pub struct VacuumDiagonalSource {
    pub(super) family: String,
    pub(super) corner: IntegralKey,
    pub(super) rows: Vec<VacuumOrdinarySource>,
    pub(super) sum: TerminalEquation,
}
impl VacuumDiagonalSource {
    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }
    pub fn corner(&self) -> &IntegralKey {
        &self.corner
    }
    pub fn rows(&self) -> &[VacuumOrdinarySource] {
        &self.rows
    }
    pub fn sum(&self) -> &TerminalEquation {
        &self.sum
    }
}
/// Sealed homogeneous relation. Weights refer to `plan.sources()` sums,
/// after the sealed full-U column aliases, without an auxiliary projection.
#[derive(Clone, Debug)]
pub struct VacuumCollectionEquation {
    pub(super) terms: Row,
    pub(super) source_weights: BTreeMap<usize, Coefficient>,
    pub(super) conditions: Arc<Vec<Coefficient>>,
}
impl VacuumCollectionEquation {
    pub fn terms(&self) -> &BTreeMap<VacuumIntegralKey, Coefficient> {
        &self.terms
    }
    pub fn source_weights(&self) -> &BTreeMap<usize, Coefficient> {
        &self.source_weights
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.conditions
    }
}
/// Flat reduction in the stored unit-mass loop-measure convention.
/// Conditions belong to this new collection, not any predecessor reduction.
/// Composing with an existing reduction must retain its conditions as well.
#[derive(Clone, Debug)]
pub struct GuardedVacuumReduction {
    pub(super) terms: Row,
    pub(super) conditions: Arc<Vec<Coefficient>>,
}
impl GuardedVacuumReduction {
    pub fn terms(&self) -> &BTreeMap<VacuumIntegralKey, Coefficient> {
        &self.terms
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.conditions
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumCollectionStatistics {
    pub raw_terminals: usize,
    pub global_target_classes: usize,
    pub corner_seeds: usize,
    pub native_source_rows: usize,
    pub columns: usize,
    pub auxiliary_columns: usize,
    pub terminal_equations: usize,
    pub remaining_terminals: usize,
    pub replay_operations: usize,
    pub reducer_nonzeros: usize,
}
/// Immutable finite collection. Preparation retains its original native
/// sources and exact aliases; application is a checked flat-map lookup.
#[derive(Clone, Debug)]
pub struct VacuumDiagonalCollectionPlan {
    pub(super) families: BTreeMap<String, Arc<IntegralFamily>>,
    pub(super) raw: BTreeSet<VacuumIntegralKey>,
    pub(super) remaining: BTreeSet<VacuumIntegralKey>,
    pub(super) aliases: VacuumFamilyAliasPlan,
    pub(super) sources: Vec<VacuumDiagonalSource>,
    pub(super) equations: Vec<VacuumCollectionEquation>,
    pub(super) reductions: BTreeMap<String, BTreeMap<IntegralKey, GuardedVacuumReduction>>,
    pub(super) conditions: Arc<Vec<Coefficient>>,
    pub(super) statistics: VacuumCollectionStatistics,
}
impl VacuumDiagonalCollectionPlan {
    pub fn families(&self) -> impl ExactSizeIterator<Item = &Arc<IntegralFamily>> {
        self.families.values()
    }
    pub fn family(&self, fingerprint: &str) -> Option<&IntegralFamily> {
        self.families.get(fingerprint).map(Arc::as_ref)
    }
    pub fn raw_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.raw
    }
    pub fn remaining_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.remaining
    }
    pub fn aliases(&self) -> &VacuumFamilyAliasPlan {
        &self.aliases
    }
    pub fn sources(&self) -> &[VacuumDiagonalSource] {
        &self.sources
    }
    pub fn equations(&self) -> &[VacuumCollectionEquation] {
        &self.equations
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.conditions
    }
    pub fn statistics(&self) -> VacuumCollectionStatistics {
        self.statistics
    }
    pub fn apply(
        &self,
        family: &IntegralFamily,
        key: &IntegralKey,
    ) -> Result<&GuardedVacuumReduction> {
        let rows = self
            .reductions
            .get(family.fingerprint())
            .ok_or(VacuumCollectionError::WrongFamily)?;
        if key.powers().len() != family.denominator_count() {
            return Err(VacuumCollectionError::WrongArity);
        }
        rows.get(key)
            .ok_or(VacuumCollectionError::UndeclaredTerminal)
    }
    /// Exponent of common `m^2` multiplying this stored-unit-mass output:
    /// `sum(output powers)-sum(original target powers)`. Equal loop counts,
    /// zero analytic shifts and active unit masses were checked at prepare.
    pub fn common_mass_squared_power(
        &self,
        family: &IntegralFamily,
        target: &IntegralKey,
        output: &VacuumIntegralKey,
    ) -> Result<i64> {
        if !self.apply(family, target)?.terms.contains_key(output) {
            return Err(VacuumCollectionError::OutputNotInReduction);
        }
        let total = |key: &IntegralKey| {
            key.powers().iter().try_fold(0_i128, |sum, &power| {
                sum.checked_add(i128::from(power))
                    .ok_or(VacuumCollectionError::ArithmeticOverflow)
            })
        };
        i64::try_from(
            total(output.integral())?
                .checked_sub(total(target)?)
                .ok_or(VacuumCollectionError::ArithmeticOverflow)?,
        )
        .map_err(|_| VacuumCollectionError::ArithmeticOverflow)
    }
}
