//! Guarded, replayable sparse elimination over the parametric field `K(n)`.
//!
//! This module is a solver-facing foundation, not a catalogue of recurrence
//! relations.  It accepts arbitrary generated [`ParametricRelation`] rows,
//! chooses pivots deterministically at a caller-supplied integer anchor, and
//! performs every normalization through Symbolica-backed guarded division.
//! The result retains exact source traces and is replayed before it is
//! returned.
//!
//! Pivot selection at an anchor is only an elimination heuristic.  A centered
//! pivot equation is **not** yet a sector rule: the rule-discovery layer must
//! separately prove its integer-domain applicability and strict descent under
//! the persisted ordering policy.
//!
//! Resource accounting bounds every RustRed-visible sparse term operation,
//! dense exponent-entry traversal/construction, integer-bit envelope, and
//! deterministic conservative certificate-retention envelope. The work
//! counters are conservative execution envelopes: RustRed charges repeated
//! lower-layer authentication and manifest passes even when an earlier pass
//! has already established the same sparse shape. Symbolica's native
//! rational-polynomial canonicalization may invoke a polynomial GCD whose
//! transient internal workspace is not exposed by its Rust API. That native
//! workspace is the deliberate remaining seam: inputs and raw product
//! envelopes are charged before the call, and the canonical output is
//! censused immediately after it returns, but this module does not claim to
//! bound memory temporarily allocated inside the GCD itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::parametric_coefficient::{
    PendingGuardedParametricDivision, insert_parametric_condition,
};
use crate::{
    GuardOrigin, GuardedParametricCoefficient, IndexShift, IndexSpace, IntegralOrderingPolicy,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    ParametricRelation, ParametricRelationError, ParametricRowId, SectorFoundationError,
};

/// Stable schema for the first generic parametric-elimination certificate.
pub const PARAMETRIC_ELIMINATION_V1_SCHEMA: &str = "rustred-parametric-elimination-v1";
pub const PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA: &str = "rustred-parametric-source-manifest-v1";

/// Deterministic column order used to scout parametric pivots.
///
/// The concrete anchor resolves the ordering of the finitely many shifted
/// integrals occurring in the source rows.  It does not establish that a
/// centered symbolic recurrence descends everywhere in a sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricEliminationOrdering {
    policy: IntegralOrderingPolicy,
    anchor: Box<[i64]>,
}

impl ParametricEliminationOrdering {
    pub fn try_new(
        policy: IntegralOrderingPolicy,
        anchor: impl IntoIterator<Item = i64>,
    ) -> Result<Self, ParametricEliminationError> {
        let anchor: Box<[i64]> = anchor.into_iter().collect();
        if anchor.is_empty() {
            return Err(ParametricEliminationError::EmptyIndexSpace);
        }
        Ok(Self { policy, anchor })
    }

    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub fn anchor(&self) -> &[i64] {
        &self.anchor
    }

    pub fn stable_string(&self) -> String {
        let anchor = self
            .anchor
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("{}|anchor=[{anchor}]", self.policy.stable_id())
    }

    pub(crate) fn shifted_indices(
        &self,
        shift: &IndexShift,
    ) -> Result<Vec<i64>, ParametricEliminationError> {
        if shift.arity() != self.anchor.len() {
            return Err(ParametricEliminationError::WrongArity {
                expected: self.anchor.len(),
                actual: shift.arity(),
            });
        }
        self.anchor
            .iter()
            .zip(shift.values())
            .enumerate()
            .map(|(position, (&anchor, &offset))| {
                anchor
                    .checked_add(offset)
                    .ok_or(ParametricEliminationError::IndexOverflow { position })
            })
            .collect()
    }
}

/// Explicit work and retained-payload bounds for parametric elimination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricEliminationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_source_rows: usize,
    pub max_source_manifest_bytes: usize,
    pub max_columns: usize,
    pub max_input_terms: usize,
    pub max_input_guards: usize,
    pub max_input_guard_origins: usize,
    pub max_pivots: usize,
    pub max_reductions: usize,
    pub max_sparse_updates: usize,
    pub max_retained_terms: usize,
    pub max_retained_guards: usize,
    pub max_retained_guard_origins: usize,
    pub max_replay_reductions: usize,
    pub max_replay_updates: usize,
    /// Cumulative sparse polynomial-term work charged before coefficient
    /// arithmetic during construction.  This is an operation census, not a
    /// wall-clock estimate.
    pub max_construction_coefficient_algebra_work: usize,
    /// Cumulative dense polynomial-exponent entries visited or constructed
    /// during coefficient arithmetic in construction. Raw products charge
    /// `left_terms * right_terms * variable_count` before Symbolica runs.
    pub max_construction_coefficient_exponent_entry_work: usize,
    /// Cumulative integer-bit work conservatively derived from the operands
    /// and raw polynomial-product envelopes during construction.
    pub max_construction_coefficient_integer_bit_work: usize,
    /// Replay counterpart of
    /// [`Self::max_construction_coefficient_algebra_work`].
    pub max_replay_coefficient_algebra_work: usize,
    /// Replay counterpart of
    /// [`Self::max_construction_coefficient_exponent_entry_work`].
    pub max_replay_coefficient_exponent_entry_work: usize,
    /// Replay counterpart of
    /// [`Self::max_construction_coefficient_integer_bit_work`].
    pub max_replay_coefficient_integer_bit_work: usize,
    /// Deterministic conservative byte envelope for the completed elimination certificate.
    /// Shared Symbolica variable maps and pre-existing source rows are not
    /// charged; every certificate-owned row, coefficient, shift, trace,
    /// vector payload, fingerprint, and manifest is charged.
    pub max_retained_bytes: usize,
}

impl Default for ParametricEliminationLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_source_rows: 100_000,
            max_source_manifest_bytes: 512 * 1024 * 1024,
            max_columns: 1_000_000,
            max_input_terms: 10_000_000,
            max_input_guards: 1_000_000,
            max_input_guard_origins: 10_000_000,
            max_pivots: 100_000,
            max_reductions: 100_000_000,
            max_sparse_updates: 1_000_000_000,
            max_retained_terms: 100_000_000,
            max_retained_guards: 10_000_000,
            max_retained_guard_origins: 100_000_000,
            max_replay_reductions: 200_000_000,
            max_replay_updates: 2_000_000_000,
            max_construction_coefficient_algebra_work: 16_000_000_000_000,
            max_construction_coefficient_exponent_entry_work: 256_000_000_000_000,
            max_construction_coefficient_integer_bit_work: 64_000_000_000_000,
            max_replay_coefficient_algebra_work: 32_000_000_000_000,
            max_replay_coefficient_exponent_entry_work: 512_000_000_000_000,
            max_replay_coefficient_integer_bit_work: 128_000_000_000_000,
            max_retained_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

/// One exact elimination of a prior unit pivot from a source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricEliminationReduction {
    prior_pivot_ordinal: usize,
    factor: ParametricCoefficient,
}

impl ParametricEliminationReduction {
    pub const fn prior_pivot_ordinal(&self) -> usize {
        self.prior_pivot_ordinal
    }

    /// Coefficient `c` in `row <- row - c * prior_pivot`.
    pub const fn factor(&self) -> &ParametricCoefficient {
        &self.factor
    }
}

/// Compact exact derivation of one normalized pivot row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricEliminationTrace {
    base_source_row_index: usize,
    reductions: Vec<ParametricEliminationReduction>,
    divisor: ParametricCoefficient,
}

impl ParametricEliminationTrace {
    pub const fn base_source_row_index(&self) -> usize {
        self.base_source_row_index
    }

    pub fn reductions(&self) -> &[ParametricEliminationReduction] {
        &self.reductions
    }

    /// Pre-normalization coefficient of the pivot integral.
    pub const fn divisor(&self) -> &ParametricCoefficient {
        &self.divisor
    }
}

/// One exact unit-pivot equation and its source derivation.
#[derive(Clone, Debug)]
pub struct ParametricPivotEquation {
    ordinal: usize,
    pivot: IndexShift,
    unit_relation: ParametricRelation,
    trace: ParametricEliminationTrace,
}

impl ParametricPivotEquation {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn pivot(&self) -> &IndexShift {
        &self.pivot
    }

    /// Equation with coefficient one at [`Self::pivot`].
    pub const fn unit_relation(&self) -> &ParametricRelation {
        &self.unit_relation
    }

    pub const fn trace(&self) -> &ParametricEliminationTrace {
        &self.trace
    }

    /// Translate the unit equation so its pivot is `J(n)` (zero shift).
    ///
    /// This is a symbolic recurrence candidate.  Its RHS still needs a
    /// separate sector-domain and strict-descent proof before application.
    pub fn centered_relation(
        &self,
        context: &ParametricCoefficientContext,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricRelation, ParametricEliminationError> {
        let translation = self
            .pivot
            .values()
            .iter()
            .enumerate()
            .map(|(position, &value)| {
                value
                    .checked_neg()
                    .ok_or(ParametricEliminationError::IndexOverflow { position })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let translation = IndexSpace::try_new(self.pivot.arity())?.shift(translation)?;
        let centered = self.unit_relation.translated(
            context,
            &translation,
            ParametricRowId::Derived {
                label: Arc::from(format!(
                    "parametric-elimination-centered-pivot-{}",
                    self.ordinal
                )),
            },
            limits,
        )?;
        let zero = IndexSpace::try_new(self.pivot.arity())?.zero();
        if let Some(coefficient) = centered.terms().get(&zero) {
            let delta =
                context.sub_with_limits(coefficient, &context.one(), limits.exact_algebra)?;
            if delta.is_zero() {
                return Ok(centered);
            }
        }
        Err(ParametricEliminationError::InternalReplayFailure {
            detail: format!(
                "centered pivot {} does not have unit coefficient at zero shift",
                self.ordinal
            ),
        })
    }
}

/// Exact construction and replay census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParametricEliminationStats {
    source_rows: usize,
    columns: usize,
    input_terms: usize,
    input_guards: usize,
    input_guard_origins: usize,
    rank: usize,
    free_columns: usize,
    construction_reductions: usize,
    construction_updates: usize,
    construction_coefficient_algebra_work: usize,
    construction_coefficient_exponent_entry_work: usize,
    construction_coefficient_integer_bit_work: usize,
    retained_terms: usize,
    retained_guards: usize,
    retained_guard_origins: usize,
    retained_bytes: usize,
    maximum_row_width: usize,
    replay_reductions: usize,
    replay_updates: usize,
    replay_coefficient_algebra_work: usize,
    replay_coefficient_exponent_entry_work: usize,
    replay_coefficient_integer_bit_work: usize,
}

impl ParametricEliminationStats {
    pub const fn source_rows(self) -> usize {
        self.source_rows
    }
    pub const fn columns(self) -> usize {
        self.columns
    }
    pub const fn input_terms(self) -> usize {
        self.input_terms
    }
    pub const fn input_guards(self) -> usize {
        self.input_guards
    }
    pub const fn input_guard_origins(self) -> usize {
        self.input_guard_origins
    }
    pub const fn rank(self) -> usize {
        self.rank
    }
    pub const fn free_columns(self) -> usize {
        self.free_columns
    }
    pub const fn construction_reductions(self) -> usize {
        self.construction_reductions
    }
    pub const fn construction_updates(self) -> usize {
        self.construction_updates
    }
    pub const fn construction_coefficient_algebra_work(self) -> usize {
        self.construction_coefficient_algebra_work
    }
    pub const fn construction_coefficient_exponent_entry_work(self) -> usize {
        self.construction_coefficient_exponent_entry_work
    }
    pub const fn construction_coefficient_integer_bit_work(self) -> usize {
        self.construction_coefficient_integer_bit_work
    }
    pub const fn retained_terms(self) -> usize {
        self.retained_terms
    }
    pub const fn retained_guards(self) -> usize {
        self.retained_guards
    }
    pub const fn retained_guard_origins(self) -> usize {
        self.retained_guard_origins
    }
    pub const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
    pub const fn maximum_row_width(self) -> usize {
        self.maximum_row_width
    }
    pub const fn replay_reductions(self) -> usize {
        self.replay_reductions
    }
    pub const fn replay_updates(self) -> usize {
        self.replay_updates
    }
    pub const fn replay_coefficient_algebra_work(self) -> usize {
        self.replay_coefficient_algebra_work
    }
    pub const fn replay_coefficient_exponent_entry_work(self) -> usize {
        self.replay_coefficient_exponent_entry_work
    }
    pub const fn replay_coefficient_integer_bit_work(self) -> usize {
        self.replay_coefficient_integer_bit_work
    }
}

/// A generic exact rank/elimination certificate over `K(n)`.
#[derive(Clone)]
pub struct ParametricElimination {
    ordering: ParametricEliminationOrdering,
    kernel: ParametricEliminationKernel,
}

// Preserve the original flat diagnostic shape even though construction now
// delegates to a shared private kernel.
impl fmt::Debug for ParametricElimination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricElimination")
            .field("family_fingerprint", &self.kernel.family_fingerprint)
            .field("context_fingerprint", &self.kernel.context_fingerprint)
            .field("source_manifest", &self.kernel.source_manifest)
            .field("ordering", &self.ordering)
            .field("limits", &self.kernel.limits)
            .field("columns_easiest_first", &self.kernel.columns_easiest_first)
            .field("pivots", &self.kernel.pivots)
            .field("free_columns", &self.kernel.free_columns)
            .field("stats", &self.kernel.stats)
            .finish()
    }
}

/// Crate-internal elimination whose complete column order comes from an
/// already-authenticated ordering certificate.
///
/// This is deliberately a separate wrapper from [`ParametricElimination`]:
/// public anchor-based callers keep their concrete
/// [`ParametricEliminationOrdering`], while residual-affine callers do not
/// need to manufacture a fake integer anchor.  The opaque ordering identity
/// must be the stable manifest of the upstream ordering certificate after
/// that certificate has been replayed.  RustRed binds it to the exact ordered
/// column list and requires both again on replay.
#[derive(Clone, Debug)]
pub(crate) struct PreorderedParametricElimination {
    ordering_identity: Arc<str>,
    kernel: ParametricEliminationKernel,
}

/// Algebraic payload shared by concrete-anchor and authenticated-preordered
/// construction.  Keeping one kernel is important: pivot choice, guarded
/// normalization, trace construction, and replay cannot drift between the
/// two entry points.
#[derive(Clone, Debug)]
struct ParametricEliminationKernel {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    source_manifest: Arc<str>,
    limits: ParametricEliminationLimits,
    columns_easiest_first: Vec<IndexShift>,
    pivots: Vec<ParametricPivotEquation>,
    free_columns: Vec<IndexShift>,
    stats: ParametricEliminationStats,
}

impl ParametricElimination {
    pub const SCHEMA: &'static str = PARAMETRIC_ELIMINATION_V1_SCHEMA;

    /// Build, normalize, and independently replay a deterministic elimination.
    pub fn build(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        ordering: ParametricEliminationOrdering,
        limits: ParametricEliminationLimits,
    ) -> Result<Self, ParametricEliminationError> {
        let wrapper_retained_bytes = checked_count_add(
            size_of::<Self>(),
            ordering.anchor.len().checked_mul(size_of::<i64>()).ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                },
            )?,
            "retained parametric elimination bytes",
        )?;
        check_limit(
            "retained parametric elimination bytes",
            wrapper_retained_bytes,
            limits.max_retained_bytes,
        )?;
        let mut work = WorkBudget::construction(limits, 0);
        let input = validate_source(
            context,
            source_rows,
            &ordering,
            wrapper_retained_bytes,
            limits,
            &mut work,
        )?;
        let kernel = ParametricEliminationKernel::build(context, source_rows, input, limits, work)?;
        Ok(Self { ordering, kernel })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.kernel.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.kernel.context_fingerprint
    }

    /// Complete collision-free manifest of the ordered generated source rows.
    pub fn source_manifest(&self) -> &str {
        &self.kernel.source_manifest
    }

    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn limits(&self) -> ParametricEliminationLimits {
        self.kernel.limits
    }

    pub fn columns_easiest_first(&self) -> &[IndexShift] {
        &self.kernel.columns_easiest_first
    }

    pub fn pivots(&self) -> &[ParametricPivotEquation] {
        &self.kernel.pivots
    }

    pub fn free_columns(&self) -> &[IndexShift] {
        &self.kernel.free_columns
    }

    pub const fn stats(&self) -> ParametricEliminationStats {
        self.kernel.stats
    }

    /// Independently replay every stored pivot and reduce all source rows.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
    ) -> Result<(), ParametricEliminationError> {
        let replay = self.kernel.replay_internal(context, source_rows)?;
        self.kernel.validate_stored_replay_work(&replay)
    }
}

impl PreorderedParametricElimination {
    /// Build from a complete easiest-first column list produced by an
    /// authenticated ordering layer.
    ///
    /// No comparator crosses this seam.  The supplied list must contain every
    /// and only every shift in `source_rows`, once each, and every shift must
    /// have the context arity.  `authenticated_ordering_identity` is opaque to
    /// this algebra layer; its caller is responsible for replaying the source
    /// ordering certificate before entering here.
    pub(crate) fn build(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        columns_easiest_first: Vec<IndexShift>,
        authenticated_ordering_identity: impl AsRef<str>,
        limits: ParametricEliminationLimits,
    ) -> Result<Self, ParametricEliminationError> {
        let ordering_identity = authenticated_ordering_identity.as_ref();
        validate_ordering_identity(ordering_identity, limits.max_source_manifest_bytes)?;
        let wrapper_retained_bytes = checked_count_add(
            size_of::<Self>(),
            arc_str_allocation_bound(ordering_identity.len())?,
            "retained parametric elimination bytes",
        )?;
        check_limit(
            "retained parametric elimination bytes",
            wrapper_retained_bytes,
            limits.max_retained_bytes,
        )?;
        // Conversion can allocate and copy the complete caller-controlled
        // identity, so it occurs only after both byte limits are known to
        // admit its deterministic retained envelope.
        let ordering_identity: Arc<str> = Arc::from(ordering_identity);
        let mut work = WorkBudget::construction(limits, 0);
        let input = validate_source_preordered(
            context,
            source_rows,
            columns_easiest_first,
            wrapper_retained_bytes,
            limits,
            &mut work,
        )?;
        let kernel = ParametricEliminationKernel::build(context, source_rows, input, limits, work)?;
        Ok(Self {
            ordering_identity,
            kernel,
        })
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        &self.kernel.family_fingerprint
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.kernel.context_fingerprint
    }

    pub(crate) fn source_manifest(&self) -> &str {
        &self.kernel.source_manifest
    }

    pub(crate) fn ordering_identity(&self) -> &str {
        &self.ordering_identity
    }

    pub(crate) const fn limits(&self) -> ParametricEliminationLimits {
        self.kernel.limits
    }

    pub(crate) fn columns_easiest_first(&self) -> &[IndexShift] {
        &self.kernel.columns_easiest_first
    }

    pub(crate) fn pivots(&self) -> &[ParametricPivotEquation] {
        &self.kernel.pivots
    }

    pub(crate) fn free_columns(&self) -> &[IndexShift] {
        &self.kernel.free_columns
    }

    pub(crate) const fn stats(&self) -> ParametricEliminationStats {
        self.kernel.stats
    }

    /// Replay under the same upstream ordering authority.  The caller must
    /// replay that authority before invoking this method; this boundary then
    /// proves that its stable identity and complete column list are exactly
    /// the values bound into the elimination certificate.
    pub(crate) fn replay(
        &self,
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        columns_easiest_first: &[IndexShift],
        authenticated_ordering_identity: &str,
    ) -> Result<(), ParametricEliminationError> {
        if authenticated_ordering_identity != self.ordering_identity.as_ref() {
            return Err(ParametricEliminationError::OrderingIdentityMismatch);
        }
        if columns_easiest_first != self.kernel.columns_easiest_first {
            return Err(ParametricEliminationError::ColumnOrderMismatch);
        }
        let replay = self.kernel.replay_internal(context, source_rows)?;
        self.kernel.validate_stored_replay_work(&replay)
    }
}

impl ParametricEliminationKernel {
    fn validate_stored_replay_work(
        &self,
        replay: &WorkBudget,
    ) -> Result<(), ParametricEliminationError> {
        if replay.reductions != self.stats.replay_reductions
            || replay.updates != self.stats.replay_updates
            || replay.coefficient_algebra_work != self.stats.replay_coefficient_algebra_work
            || replay.coefficient_exponent_entry_work
                != self.stats.replay_coefficient_exponent_entry_work
            || replay.coefficient_integer_bit_work != self.stats.replay_coefficient_integer_bit_work
        {
            return Err(ParametricEliminationError::InternalReplayFailure {
                detail: "replay work census differs from the stored certificate".to_owned(),
            });
        }
        Ok(())
    }

    fn build(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        input: ValidatedInput,
        limits: ParametricEliminationLimits,
        mut work: WorkBudget,
    ) -> Result<Self, ParametricEliminationError> {
        let source_manifest_bytes =
            source_manifest_byte_len(source_rows, limits.max_source_manifest_bytes, &mut work)?;
        let mut retained_bytes = elimination_base_retained_byte_bound(
            input.wrapper_retained_bytes,
            context,
            source_rows[0].family_fingerprint(),
            source_manifest_bytes,
            &input.columns_easiest_first,
        )?;
        check_limit(
            "retained parametric elimination bytes",
            retained_bytes,
            limits.max_retained_bytes,
        )?;
        // The complete aggregate length and its certificate-retention
        // envelope were checked without allocating the aggregate String.
        let source_manifest = source_manifest_with_exact_len(
            source_rows,
            limits.max_source_manifest_bytes,
            source_manifest_bytes,
            &mut work,
        )?;
        let column_rank = input
            .columns_easiest_first
            .iter()
            .cloned()
            .enumerate()
            .map(|(rank, shift)| (shift, rank))
            .collect::<BTreeMap<_, _>>();
        let mut pivots = Vec::new();
        let maximum_pivots = source_rows
            .len()
            .min(input.column_count)
            .min(limits.max_pivots);
        let pivot_slots = maximum_pivots
            .checked_mul(size_of::<ParametricPivotEquation>())
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "retained parametric elimination bytes",
            })?;
        retained_bytes = checked_count_add(
            retained_bytes,
            pivot_slots,
            "retained parametric elimination bytes",
        )?;
        check_limit(
            "retained parametric elimination bytes",
            retained_bytes,
            limits.max_retained_bytes,
        )?;
        pivots.try_reserve_exact(maximum_pivots).map_err(|_| {
            ParametricEliminationError::ResourceLimit {
                resource: "parametric pivot vector allocation",
                requested: maximum_pivots,
                limit: limits.max_pivots,
            }
        })?;
        let mut retained_terms = 0usize;
        let mut retained_guards = 0usize;
        let mut retained_guard_origins = 0usize;

        for (source_row_index, source) in source_rows.iter().enumerate() {
            work.charge_relation_clone(source)?;
            let (reduced, reductions) = reduce_by_pivots(
                context,
                source.clone(),
                &pivots,
                &mut work,
                limits.arithmetic,
                Some(retained_bytes),
            )?;
            if reduced.is_zero() {
                continue;
            }
            check_limit("parametric pivots", pivots.len() + 1, limits.max_pivots)?;
            let pivot = hardest_shift(&reduced, &column_rank)?;
            let divisor = reduced.terms().get(pivot).ok_or_else(|| {
                ParametricEliminationError::InternalReplayFailure {
                    detail: "chosen pivot is absent from its reduced row".to_owned(),
                }
            })?;
            let next_retained_terms = checked_count_add(
                retained_terms,
                reduced.terms().len(),
                "retained parametric pivot terms",
            )?;
            check_limit(
                "retained parametric pivot terms",
                next_retained_terms,
                limits.max_retained_terms,
            )?;
            // A nonzero scalar normalization preserves support. Charge every
            // output slot before constructing the normalized relation.
            work.charge_updates(reduced.terms().len())?;
            // The certificate will deep-clone the divisor. Census that copy
            // before either its GMP payload or the pivot shift is cloned.
            work.charge_coefficient_observation(divisor)?;
            work.charge_context_constant_constructor(context, ContextConstant::One)?;
            let one = context.one();
            work.charge_guarded_division_pending_operation(&one, divisor)?;
            let pending_inverse = context
                .checked_div_guarded_pending_normalization_with_limits_and_origin_limit(
                    &one,
                    divisor,
                    limits.arithmetic.exact_algebra,
                    limits.arithmetic.max_guard_origins,
                )?;
            work.charge_guarded_division_final_normalization(
                pending_inverse.value_before_final_normalization(),
            )?;
            let inverse = context
                .finish_guarded_division_normalization_with_limits_and_origin_limit(
                    pending_inverse,
                    limits.arithmetic.exact_algebra,
                    limits.arithmetic.max_guard_origins,
                )?;
            work.charge_coefficient_observation(&inverse.value)?;
            let (prospective_guards, prospective_origins) =
                guarded_normalization_payload_upper_bound(&reduced, &inverse)?;
            check_limit(
                "guards in one normalized pivot",
                prospective_guards,
                limits.max_retained_guards,
            )?;
            check_limit(
                "guard origins in one normalized pivot",
                prospective_origins,
                limits.max_retained_guard_origins,
            )?;
            check_limit(
                "prospective retained parametric pivot guards",
                checked_count_add(
                    retained_guards,
                    prospective_guards,
                    "prospective retained parametric pivot guards",
                )?,
                limits.max_retained_guards,
            )?;
            check_limit(
                "prospective retained parametric pivot guard origins",
                checked_count_add(
                    retained_guard_origins,
                    prospective_origins,
                    "prospective retained parametric pivot guard origins",
                )?,
                limits.max_retained_guard_origins,
            )?;
            let ordinal = pivots.len();
            let prospective_pivot_bytes = prospective_normalized_pivot_retained_byte_bound(
                source,
                pivot,
                &reduced,
                &inverse,
                &reductions,
                divisor,
                ordinal,
                limits.arithmetic.exact_algebra.max_polynomial_terms,
                &mut work,
            )?;
            let prospective_total_retained_bytes = checked_count_add(
                retained_bytes,
                prospective_pivot_bytes,
                "retained parametric elimination bytes",
            )?;
            check_limit(
                "retained parametric elimination bytes",
                prospective_total_retained_bytes,
                limits.max_retained_bytes,
            )?;
            work.charge_scaled_relation_operation_into_empty(&reduced, &inverse)?;
            // All certificate-owned clones below are dominated by the checked
            // envelope. The complete possible pivot-vector buffer was
            // fallibly reserved and charged before the first pivot.
            let mut unit_relation = ParametricRelation::new(
                source.family_fingerprint(),
                pivot_row_id(ordinal),
                context,
            );
            unit_relation.add_scaled_guarded_with_limits(
                context,
                &reduced,
                inverse,
                limits.arithmetic,
            )?;
            verify_unit_pivot(context, &unit_relation, pivot, &mut work, limits.arithmetic)?;
            observe_relation(&unit_relation, &mut work)?;
            retained_terms = next_retained_terms;
            retained_guards = checked_count_add(
                retained_guards,
                unit_relation.guarded_nonzero_conditions().len(),
                "retained parametric pivot guards",
            )?;
            check_limit(
                "retained parametric pivot guards",
                retained_guards,
                limits.max_retained_guards,
            )?;
            retained_guard_origins = checked_count_add(
                retained_guard_origins,
                guard_origin_count(&unit_relation)?,
                "retained parametric pivot guard origins",
            )?;
            check_limit(
                "retained parametric pivot guard origins",
                retained_guard_origins,
                limits.max_retained_guard_origins,
            )?;
            let divisor_copy_shape = work.coefficient_shape(divisor)?;
            work.charge_coefficient_clone(&divisor_copy_shape)?;
            let equation = ParametricPivotEquation {
                ordinal,
                pivot: pivot.clone(),
                unit_relation,
                trace: ParametricEliminationTrace {
                    base_source_row_index: source_row_index,
                    reductions,
                    divisor: divisor.clone(),
                },
            };
            let actual_pivot_bytes = pivot_retained_byte_bound(&equation, &mut work)?
                .checked_sub(size_of::<ParametricPivotEquation>())
                .ok_or(ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                })?;
            if actual_pivot_bytes > prospective_pivot_bytes {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail: format!(
                        "normalized pivot retained {actual_pivot_bytes} bytes beyond its {prospective_pivot_bytes}-byte prospective envelope"
                    ),
                });
            }
            // Persist the deterministic conservative envelope, not an
            // allocator-specific claim of exact physical bytes.
            retained_bytes = prospective_total_retained_bytes;
            pivots.push(equation);
        }

        let pivot_set = pivots
            .iter()
            .map(|pivot| pivot.pivot.clone())
            .collect::<BTreeSet<_>>();
        let mut prospective_free_column_bytes = 0usize;
        for shift in input
            .columns_easiest_first
            .iter()
            .filter(|shift| !pivot_set.contains(*shift))
        {
            prospective_free_column_bytes = checked_count_add(
                prospective_free_column_bytes,
                size_of::<IndexShift>(),
                "retained parametric elimination bytes",
            )?;
            prospective_free_column_bytes = checked_count_add(
                prospective_free_column_bytes,
                shift.owned_retained_byte_bound().ok_or(
                    ParametricEliminationError::ResourceCountOverflow {
                        resource: "retained parametric elimination bytes",
                    },
                )?,
                "retained parametric elimination bytes",
            )?;
        }
        check_limit(
            "retained parametric elimination bytes",
            checked_count_add(
                retained_bytes,
                prospective_free_column_bytes,
                "retained parametric elimination bytes",
            )?,
            limits.max_retained_bytes,
        )?;
        let free_column_count = input.column_count - pivot_set.len();
        let mut free_columns = Vec::new();
        free_columns
            .try_reserve_exact(free_column_count)
            .map_err(|_| ParametricEliminationError::ResourceLimit {
                resource: "free-column vector allocation",
                requested: free_column_count,
                limit: limits.max_columns,
            })?;
        for shift in input
            .columns_easiest_first
            .iter()
            .filter(|shift| !pivot_set.contains(*shift))
        {
            free_columns.push(shift.clone());
        }
        if free_columns.len() != free_column_count {
            return Err(ParametricEliminationError::InternalReplayFailure {
                detail: "free-column preflight count differs from collection".to_owned(),
            });
        }
        retained_bytes = checked_count_add(
            retained_bytes,
            prospective_free_column_bytes,
            "retained parametric elimination bytes",
        )?;
        check_limit(
            "retained parametric elimination bytes",
            retained_bytes,
            limits.max_retained_bytes,
        )?;
        let family_fingerprint: Arc<str> = Arc::from(source_rows[0].family_fingerprint());
        let mut result = Self {
            family_fingerprint,
            context_fingerprint: Arc::from(context.fingerprint()),
            source_manifest: Arc::from(source_manifest),
            limits,
            columns_easiest_first: input.columns_easiest_first,
            pivots,
            free_columns,
            stats: ParametricEliminationStats {
                source_rows: source_rows.len(),
                columns: input.column_count,
                input_terms: input.input_terms,
                input_guards: input.input_guards,
                input_guard_origins: input.input_guard_origins,
                rank: pivot_set.len(),
                free_columns: input.column_count - pivot_set.len(),
                construction_reductions: work.reductions,
                construction_updates: work.updates,
                construction_coefficient_algebra_work: work.coefficient_algebra_work,
                construction_coefficient_exponent_entry_work: work.coefficient_exponent_entry_work,
                construction_coefficient_integer_bit_work: work.coefficient_integer_bit_work,
                retained_terms,
                retained_guards,
                retained_guard_origins,
                retained_bytes,
                maximum_row_width: work.maximum_row_width,
                replay_reductions: 0,
                replay_updates: 0,
                replay_coefficient_algebra_work: 0,
                replay_coefficient_exponent_entry_work: 0,
                replay_coefficient_integer_bit_work: 0,
            },
        };
        let replay = result.replay_internal(context, source_rows)?;
        result.stats.replay_reductions = replay.reductions;
        result.stats.replay_updates = replay.updates;
        result.stats.replay_coefficient_algebra_work = replay.coefficient_algebra_work;
        result.stats.replay_coefficient_exponent_entry_work =
            replay.coefficient_exponent_entry_work;
        result.stats.replay_coefficient_integer_bit_work = replay.coefficient_integer_bit_work;
        result.stats.maximum_row_width =
            result.stats.maximum_row_width.max(replay.maximum_row_width);
        Ok(result)
    }

    fn replay_internal(
        &self,
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
    ) -> Result<WorkBudget, ParametricEliminationError> {
        validate_certificate_scope_header(self, context, source_rows)?;
        let mut work = WorkBudget::replay(self.limits, 0);
        validate_source_rows(context, source_rows, self.limits, &mut work)?;
        validate_certificate_source_manifest(self, source_rows, &mut work)?;

        for pivot in &self.pivots {
            let source = source_rows
                .get(pivot.trace.base_source_row_index)
                .ok_or_else(|| ParametricEliminationError::InternalReplayFailure {
                    detail: format!(
                        "pivot {} refers to missing source row {}",
                        pivot.ordinal, pivot.trace.base_source_row_index
                    ),
                })?;
            work.charge_relation_clone(source)?;
            let mut reduced = source.clone();
            for reduction in &pivot.trace.reductions {
                let prior = self
                    .pivots
                    .get(reduction.prior_pivot_ordinal)
                    .filter(|prior| prior.ordinal < pivot.ordinal)
                    .ok_or_else(|| ParametricEliminationError::InternalReplayFailure {
                        detail: format!(
                            "pivot {} has invalid prior pivot {}",
                            pivot.ordinal, reduction.prior_pivot_ordinal
                        ),
                    })?;
                let actual = if let Some(actual) = reduced.terms().get(&prior.pivot) {
                    let shape = work.coefficient_shape(actual)?;
                    work.charge_coefficient_clone(&shape)?;
                    actual.clone()
                } else {
                    work.charge_context_constant_constructor(context, ContextConstant::Zero)?;
                    context.zero()
                };
                work.charge_binary_coefficient_operation(
                    CoefficientOperation::Subtract,
                    &actual,
                    &reduction.factor,
                )?;
                let delta = context.sub_with_limits(
                    &actual,
                    &reduction.factor,
                    self.limits.arithmetic.exact_algebra,
                )?;
                work.charge_coefficient_observation(&delta)?;
                if !delta.is_zero() {
                    return Err(ParametricEliminationError::InternalReplayFailure {
                        detail: format!(
                            "pivot {} trace factor differs at prior pivot {}",
                            pivot.ordinal, prior.ordinal
                        ),
                    });
                }
                eliminate_one(
                    context,
                    &mut reduced,
                    prior,
                    &reduction.factor,
                    &mut work,
                    self.limits.arithmetic,
                )?;
            }
            let actual_divisor = reduced.terms().get(&pivot.pivot).ok_or_else(|| {
                ParametricEliminationError::InternalReplayFailure {
                    detail: format!("pivot {} disappeared during replay", pivot.ordinal),
                }
            })?;
            work.charge_binary_coefficient_operation(
                CoefficientOperation::Subtract,
                actual_divisor,
                &pivot.trace.divisor,
            )?;
            let divisor_delta = context.sub_with_limits(
                actual_divisor,
                &pivot.trace.divisor,
                self.limits.arithmetic.exact_algebra,
            )?;
            work.charge_coefficient_observation(&divisor_delta)?;
            if !divisor_delta.is_zero() {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail: format!("pivot {} divisor differs during replay", pivot.ordinal),
                });
            }
            work.charge_context_constant_constructor(context, ContextConstant::One)?;
            let one = context.one();
            work.charge_guarded_division_pending_operation(&one, actual_divisor)?;
            let pending_inverse = context
                .checked_div_guarded_pending_normalization_with_limits_and_origin_limit(
                    &one,
                    actual_divisor,
                    self.limits.arithmetic.exact_algebra,
                    self.limits.arithmetic.max_guard_origins,
                )?;
            work.charge_guarded_division_final_normalization(
                pending_inverse.value_before_final_normalization(),
            )?;
            let inverse = context
                .finish_guarded_division_normalization_with_limits_and_origin_limit(
                    pending_inverse,
                    self.limits.arithmetic.exact_algebra,
                    self.limits.arithmetic.max_guard_origins,
                )?;
            work.charge_coefficient_observation(&inverse.value)?;
            let (prospective_guards, prospective_origins) =
                guarded_normalization_payload_upper_bound(&reduced, &inverse)?;
            check_limit(
                "guards in one replayed pivot",
                prospective_guards,
                self.limits.max_retained_guards,
            )?;
            check_limit(
                "guard origins in one replayed pivot",
                prospective_origins,
                self.limits.max_retained_guard_origins,
            )?;
            work.charge_updates(reduced.terms().len())?;
            work.charge_scaled_relation_operation_into_empty(&reduced, &inverse)?;
            let mut replayed = ParametricRelation::new(
                source.family_fingerprint(),
                pivot_row_id(pivot.ordinal),
                context,
            );
            replayed.add_scaled_guarded_with_limits(
                context,
                &reduced,
                inverse,
                self.limits.arithmetic,
            )?;
            observe_relation(&replayed, &mut work)?;
            work.charge_relation_equality(&replayed, &pivot.unit_relation)?;
            if !replayed.has_identical_guard_provenance(&pivot.unit_relation) {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail: format!(
                        "pivot {} relation or guard provenance differs during replay",
                        pivot.ordinal
                    ),
                });
            }
        }

        for (source_row_index, source) in source_rows.iter().enumerate() {
            work.charge_relation_clone(source)?;
            let (reduced, _) = reduce_by_pivots(
                context,
                source.clone(),
                &self.pivots,
                &mut work,
                self.limits.arithmetic,
                None,
            )?;
            if !reduced.is_zero() {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail: format!(
                        "source row {source_row_index} retains {} terms after all pivots",
                        reduced.terms().len()
                    ),
                });
            }
        }
        Ok(work)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricEliminationError {
    EmptyIndexSpace,
    EmptySourceRows,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    WrongFamily {
        row: usize,
    },
    WrongContext {
        row: usize,
    },
    IndexOverflow {
        position: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    EmptyOrderingIdentity,
    DuplicateColumn {
        first_position: usize,
        duplicate_position: usize,
        shift: Box<[i64]>,
    },
    MissingColumn {
        shift: Box<[i64]>,
    },
    UnexpectedColumn {
        position: usize,
        shift: Box<[i64]>,
    },
    OrderingIdentityMismatch,
    ColumnOrderMismatch,
    InvalidSourceGuard {
        row: usize,
        condition: usize,
    },
    InternalReplayFailure {
        detail: String,
    },
    Sector(SectorFoundationError),
    Coefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
}

impl fmt::Display for ParametricEliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("an elimination anchor cannot be empty"),
            Self::EmptySourceRows => {
                formatter.write_str("parametric elimination needs source rows")
            }
            Self::WrongArity { expected, actual } => {
                write!(
                    formatter,
                    "parametric elimination arity is {actual}, expected {expected}"
                )
            }
            Self::WrongFamily { row } => {
                write!(formatter, "source row {row} belongs to another family")
            }
            Self::WrongContext { row } => {
                write!(
                    formatter,
                    "source row {row} belongs to another K(n) context"
                )
            }
            Self::IndexOverflow { position } => {
                write!(
                    formatter,
                    "anchor plus shift overflowed at index {position}"
                )
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "parametric {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::EmptyOrderingIdentity => formatter.write_str(
                "a preordered parametric elimination needs an authenticated ordering identity",
            ),
            Self::DuplicateColumn {
                first_position,
                duplicate_position,
                shift,
            } => write!(
                formatter,
                "parametric shift {shift:?} occurs twice in the preordered column manifest at positions {first_position} and {duplicate_position}"
            ),
            Self::MissingColumn { shift } => {
                write!(
                    formatter,
                    "parametric shift {shift:?} is missing from the column manifest"
                )
            }
            Self::UnexpectedColumn { position, shift } => write!(
                formatter,
                "preordered column {position} with shift {shift:?} is absent from the exact source support"
            ),
            Self::OrderingIdentityMismatch => formatter.write_str(
                "the authenticated preordered-column identity differs from the elimination certificate",
            ),
            Self::ColumnOrderMismatch => formatter.write_str(
                "the authenticated preordered-column list differs from the elimination certificate",
            ),
            Self::InvalidSourceGuard { row, condition } => write!(
                formatter,
                "source row {row} has an invalid guard at position {condition}"
            ),
            Self::InternalReplayFailure { detail } => {
                write!(formatter, "parametric elimination replay failed: {detail}")
            }
            Self::Sector(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricEliminationError {}

impl From<ParametricCoefficientError> for ParametricEliminationError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<ParametricRelationError> for ParametricEliminationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<SectorFoundationError> for ParametricEliminationError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

struct ValidatedInput {
    columns_easiest_first: Vec<IndexShift>,
    column_count: usize,
    input_terms: usize,
    input_guards: usize,
    input_guard_origins: usize,
    wrapper_retained_bytes: usize,
}

struct ValidatedSource {
    columns: BTreeSet<IndexShift>,
    input_terms: usize,
    input_guards: usize,
    input_guard_origins: usize,
}

fn validate_source(
    context: &ParametricCoefficientContext,
    source_rows: &[ParametricRelation],
    ordering: &ParametricEliminationOrdering,
    wrapper_retained_bytes: usize,
    limits: ParametricEliminationLimits,
    work: &mut WorkBudget,
) -> Result<ValidatedInput, ParametricEliminationError> {
    validate_source_header(source_rows, limits)?;
    if ordering.anchor.len() != context.index_count() {
        return Err(ParametricEliminationError::WrongArity {
            expected: context.index_count(),
            actual: ordering.anchor.len(),
        });
    }
    let source = validate_source_rows(context, source_rows, limits, work)?;
    let mut decorated = source
        .columns
        .into_iter()
        .map(|shift| {
            let indices = ordering.shifted_indices(&shift)?;
            let key = ordering.policy.complexity_key(&indices)?;
            Ok((key, shift))
        })
        .collect::<Result<Vec<_>, ParametricEliminationError>>()?;
    decorated.sort_by(|(left_key, left_shift), (right_key, right_shift)| {
        left_key
            .cmp(right_key)
            .then_with(|| left_shift.cmp(right_shift))
    });
    let columns_easiest_first = decorated
        .into_iter()
        .map(|(_, shift)| shift)
        .collect::<Vec<_>>();
    Ok(ValidatedInput {
        column_count: columns_easiest_first.len(),
        columns_easiest_first,
        input_terms: source.input_terms,
        input_guards: source.input_guards,
        input_guard_origins: source.input_guard_origins,
        wrapper_retained_bytes,
    })
}

fn validate_source_preordered(
    context: &ParametricCoefficientContext,
    source_rows: &[ParametricRelation],
    columns_easiest_first: Vec<IndexShift>,
    wrapper_retained_bytes: usize,
    limits: ParametricEliminationLimits,
    work: &mut WorkBudget,
) -> Result<ValidatedInput, ParametricEliminationError> {
    validate_source_header(source_rows, limits)?;
    let source = validate_source_rows(context, source_rows, limits, work)?;
    check_limit(
        "parametric columns",
        columns_easiest_first.len(),
        limits.max_columns,
    )?;
    let mut positions = BTreeMap::new();
    for (position, shift) in columns_easiest_first.iter().enumerate() {
        if shift.arity() != context.index_count() {
            return Err(ParametricEliminationError::WrongArity {
                expected: context.index_count(),
                actual: shift.arity(),
            });
        }
        if let Some(&first_position) = positions.get(shift) {
            return Err(ParametricEliminationError::DuplicateColumn {
                first_position,
                duplicate_position: position,
                shift: shift.values().to_vec().into_boxed_slice(),
            });
        }
        if !source.columns.contains(shift) {
            return Err(ParametricEliminationError::UnexpectedColumn {
                position,
                shift: shift.values().to_vec().into_boxed_slice(),
            });
        }
        positions.insert(shift.clone(), position);
    }
    for shift in &source.columns {
        if !positions.contains_key(shift) {
            return Err(ParametricEliminationError::MissingColumn {
                shift: shift.values().to_vec().into_boxed_slice(),
            });
        }
    }
    Ok(ValidatedInput {
        column_count: columns_easiest_first.len(),
        columns_easiest_first,
        input_terms: source.input_terms,
        input_guards: source.input_guards,
        input_guard_origins: source.input_guard_origins,
        wrapper_retained_bytes,
    })
}

fn validate_ordering_identity(
    identity: &str,
    limit: usize,
) -> Result<(), ParametricEliminationError> {
    if identity.is_empty() {
        return Err(ParametricEliminationError::EmptyOrderingIdentity);
    }
    check_limit("preordered ordering identity bytes", identity.len(), limit)
}

fn validate_source_header(
    source_rows: &[ParametricRelation],
    limits: ParametricEliminationLimits,
) -> Result<(), ParametricEliminationError> {
    if source_rows.is_empty() {
        return Err(ParametricEliminationError::EmptySourceRows);
    }
    check_limit("source rows", source_rows.len(), limits.max_source_rows)?;
    Ok(())
}

fn validate_source_rows(
    context: &ParametricCoefficientContext,
    source_rows: &[ParametricRelation],
    limits: ParametricEliminationLimits,
    work: &mut WorkBudget,
) -> Result<ValidatedSource, ParametricEliminationError> {
    let family = source_rows[0].family_fingerprint();
    let mut columns = BTreeSet::new();
    let mut input_terms = 0usize;
    let mut input_guards = 0usize;
    let mut input_guard_origins = 0usize;

    // Admit every cheap structural property before the first coefficient or
    // guard polynomial is traversed.  Besides giving the named source limits
    // precedence over algebra-work limits, this keeps a late oversized row
    // from forcing authentication of all earlier sparse payloads first.
    for (row, relation) in source_rows.iter().enumerate() {
        if relation.family_fingerprint() != family {
            return Err(ParametricEliminationError::WrongFamily { row });
        }
        if relation.context_fingerprint() != context.fingerprint() {
            return Err(ParametricEliminationError::WrongContext { row });
        }
        if relation.arity() != context.index_count() {
            return Err(ParametricEliminationError::WrongArity {
                expected: context.index_count(),
                actual: relation.arity(),
            });
        }

        input_terms =
            checked_count_add(input_terms, relation.terms().len(), "input relation terms")?;
        check_limit("input relation terms", input_terms, limits.max_input_terms)?;
        work.maximum_row_width = work.maximum_row_width.max(relation.terms().len());
        for shift in relation.terms().keys() {
            if !columns.contains(shift) {
                let requested = checked_count_add(columns.len(), 1, "parametric columns")?;
                check_limit("parametric columns", requested, limits.max_columns)?;
                columns.insert(shift.clone());
            }
        }

        census_source_guard_metadata(
            relation.guarded_nonzero_conditions(),
            limits,
            &mut input_guards,
            &mut input_guard_origins,
        )?;
    }

    // Only structurally admitted sources reach the deep authentication pass.
    // The same cumulative WorkBudget then continues into manifests,
    // elimination, and retained-payload censuses.
    for (row, relation) in source_rows.iter().enumerate() {
        for coefficient in relation.terms().values() {
            work.authenticate_coefficient(context, coefficient, limits.arithmetic.exact_algebra)?;
        }
        for (condition_index, condition) in relation.guarded_nonzero_conditions().iter().enumerate()
        {
            if condition.polynomial().is_zero() || condition.origins().is_empty() {
                return Err(ParametricEliminationError::InvalidSourceGuard {
                    row,
                    condition: condition_index,
                });
            }
            work.authenticate_polynomial(
                context,
                condition.polynomial(),
                limits.arithmetic.exact_algebra,
            )?;
        }
    }
    Ok(ValidatedSource {
        columns,
        input_terms,
        input_guards,
        input_guard_origins,
    })
}

fn census_source_guard_metadata(
    conditions: &[ParametricNonZeroCondition],
    limits: ParametricEliminationLimits,
    input_guards: &mut usize,
    input_guard_origins: &mut usize,
) -> Result<(), ParametricEliminationError> {
    let next_guards = checked_count_add(*input_guards, conditions.len(), "input relation guards")?;
    check_limit(
        "input relation guards",
        next_guards,
        limits.max_input_guards,
    )?;
    *input_guards = next_guards;

    for condition in conditions {
        check_limit(
            "origins in one source guard",
            condition.origins().len(),
            limits.arithmetic.max_guard_origins,
        )?;
        let next_origins = checked_count_add(
            *input_guard_origins,
            condition.origins().len(),
            "input relation guard origins",
        )?;
        check_limit(
            "input relation guard origins",
            next_origins,
            limits.max_input_guard_origins,
        )?;
        *input_guard_origins = next_origins;
    }
    Ok(())
}

fn validate_certificate_scope_header(
    certificate: &ParametricEliminationKernel,
    context: &ParametricCoefficientContext,
    source_rows: &[ParametricRelation],
) -> Result<(), ParametricEliminationError> {
    if certificate.context_fingerprint.as_ref() != context.fingerprint() {
        return Err(ParametricEliminationError::WrongContext { row: 0 });
    }
    if source_rows.is_empty() {
        return Err(ParametricEliminationError::EmptySourceRows);
    }
    check_limit(
        "source rows",
        source_rows.len(),
        certificate.limits.max_source_rows,
    )?;
    if source_rows.len() != certificate.stats.source_rows {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: format!(
                "source row count is {}, certificate was built from {}",
                source_rows.len(),
                certificate.stats.source_rows
            ),
        });
    }
    // Reject cheap scope mismatches before counting or allocating a complete
    // caller-controlled source manifest.
    for (row, source) in source_rows.iter().enumerate() {
        if source.family_fingerprint() != certificate.family_fingerprint.as_ref() {
            return Err(ParametricEliminationError::WrongFamily { row });
        }
        if source.context_fingerprint() != certificate.context_fingerprint.as_ref() {
            return Err(ParametricEliminationError::WrongContext { row });
        }
        if source.arity() != context.index_count() {
            return Err(ParametricEliminationError::WrongArity {
                expected: context.index_count(),
                actual: source.arity(),
            });
        }
    }
    Ok(())
}

fn validate_certificate_source_manifest(
    certificate: &ParametricEliminationKernel,
    source_rows: &[ParametricRelation],
    work: &mut WorkBudget,
) -> Result<(), ParametricEliminationError> {
    let replay_manifest = source_manifest(
        source_rows,
        certificate.limits.max_source_manifest_bytes,
        work,
    )?;
    if replay_manifest != certificate.source_manifest.as_ref() {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "ordered source-row manifest differs from the certificate".to_owned(),
        });
    }
    Ok(())
}

fn source_manifest(
    source_rows: &[ParametricRelation],
    limit: usize,
    work: &mut WorkBudget,
) -> Result<String, ParametricEliminationError> {
    let exact = source_manifest_byte_len(source_rows, limit, work)?;
    source_manifest_with_exact_len(source_rows, limit, exact, work)
}

/// Count the collision-free aggregate manifest before allocating either the
/// aggregate String or any row String.
fn source_manifest_byte_len(
    source_rows: &[ParametricRelation],
    limit: usize,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    let mut bytes = PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA.len();
    bytes = checked_count_add(bytes, "|rows=".len(), "source manifest bytes")?;
    bytes = checked_count_add(
        bytes,
        source_rows.len().to_string().len(),
        "source manifest bytes",
    )?;
    check_limit("source manifest bytes", bytes, limit)?;
    for row in source_rows {
        // The relation encoder length-prefixes each polynomial/coefficient:
        // one nested counting traversal and one outer counting traversal.
        // Extract each shape once, then precharge both encoder passes from the
        // cached shape before entering it.
        work.charge_relation_payload_traversals(row, 2)?;
        let remaining =
            limit
                .checked_sub(bytes)
                .ok_or(ParametricEliminationError::ResourceLimit {
                    resource: "source manifest bytes",
                    requested: bytes,
                    limit,
                })?;
        let row_bytes = row
            .stable_manifest_byte_len_with_limit(remaining)
            .map_err(ParametricEliminationError::Relation)?;
        bytes = checked_count_add(bytes, 1, "source manifest bytes")?;
        bytes = checked_count_add(bytes, row_bytes.to_string().len(), "source manifest bytes")?;
        bytes = checked_count_add(bytes, 1, "source manifest bytes")?;
        bytes = checked_count_add(bytes, row_bytes, "source manifest bytes")?;
        check_limit("source manifest bytes", bytes, limit)?;
    }
    Ok(bytes)
}

fn source_manifest_with_exact_len(
    source_rows: &[ParametricRelation],
    limit: usize,
    exact_bytes: usize,
    work: &mut WorkBudget,
) -> Result<String, ParametricEliminationError> {
    check_limit("source manifest bytes", exact_bytes, limit)?;
    let mut output = String::new();
    output.try_reserve_exact(exact_bytes).map_err(|_| {
        ParametricEliminationError::ResourceLimit {
            resource: "source manifest allocation bytes",
            requested: exact_bytes,
            limit,
        }
    })?;
    output.push_str(PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA);
    output.push_str("|rows=");
    output.push_str(&source_rows.len().to_string());
    for row in source_rows {
        // `stable_manifest_with_limit` repeats its two payload traversals for
        // an exact byte count and then for the single reserved output buffer.
        work.charge_relation_payload_traversals(row, 4)?;
        let remaining =
            limit
                .checked_sub(output.len())
                .ok_or(ParametricEliminationError::ResourceLimit {
                    resource: "source manifest bytes",
                    requested: output.len(),
                    limit,
                })?;
        let row = row
            .stable_manifest_with_limit(remaining)
            .map_err(ParametricEliminationError::Relation)?;
        let requested = output
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(row.len().to_string().len()))
            .and_then(|length| length.checked_add(1))
            .and_then(|length| length.checked_add(row.len()))
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "source manifest bytes",
            })?;
        check_limit("source manifest bytes", requested, limit)?;
        output.push('|');
        output.push_str(&row.len().to_string());
        output.push(':');
        output.push_str(&row);
    }
    if output.len() != exact_bytes {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: format!(
                "source manifest preflight counted {exact_bytes} bytes but encoded {}",
                output.len()
            ),
        });
    }
    Ok(output)
}

fn reduce_by_pivots(
    context: &ParametricCoefficientContext,
    mut row: ParametricRelation,
    pivots: &[ParametricPivotEquation],
    work: &mut WorkBudget,
    arithmetic: ParametricArithmeticLimits,
    prospective_certificate_base_bytes: Option<usize>,
) -> Result<(ParametricRelation, Vec<ParametricEliminationReduction>), ParametricEliminationError> {
    let mut reductions = Vec::new();
    let reduction_slots = pivots
        .len()
        .checked_mul(size_of::<ParametricEliminationReduction>())
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
    if let Some(base) = prospective_certificate_base_bytes {
        check_limit(
            "retained parametric elimination bytes",
            checked_count_add(
                base,
                reduction_slots,
                "retained parametric elimination bytes",
            )?,
            work.limits.max_retained_bytes,
        )?;
    }
    reductions.try_reserve_exact(pivots.len()).map_err(|_| {
        ParametricEliminationError::ResourceLimit {
            resource: "parametric elimination reduction trace allocation",
            requested: pivots.len(),
            limit: pivots.len(),
        }
    })?;
    let mut reduction_factor_bytes = 0usize;
    for pivot in pivots {
        let Some(factor) = row.terms().get(&pivot.pivot) else {
            continue;
        };
        work.charge_coefficient_observation(factor)?;
        reduction_factor_bytes = checked_count_add(
            reduction_factor_bytes,
            coefficient_deep_retained_byte_bound(factor, work)?,
            "retained parametric elimination bytes",
        )?;
        if let Some(base) = prospective_certificate_base_bytes {
            check_limit(
                "retained parametric elimination bytes",
                checked_count_add(
                    checked_count_add(
                        base,
                        reduction_slots,
                        "retained parametric elimination bytes",
                    )?,
                    reduction_factor_bytes,
                    "retained parametric elimination bytes",
                )?,
                work.limits.max_retained_bytes,
            )?;
        }
        let factor_shape = work.coefficient_shape(factor)?;
        work.charge_coefficient_clone(&factor_shape)?;
        let factor = factor.clone();
        eliminate_one(context, &mut row, pivot, &factor, work, arithmetic)?;
        reductions.push(ParametricEliminationReduction {
            prior_pivot_ordinal: pivot.ordinal,
            factor,
        });
    }
    Ok((row, reductions))
}

fn eliminate_one(
    context: &ParametricCoefficientContext,
    row: &mut ParametricRelation,
    pivot: &ParametricPivotEquation,
    factor: &ParametricCoefficient,
    work: &mut WorkBudget,
    arithmetic: ParametricArithmeticLimits,
) -> Result<(), ParametricEliminationError> {
    work.charge_reduction()?;
    let expected_updates = row
        .terms()
        .len()
        .checked_add(pivot.unit_relation.terms().len())
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "parametric sparse updates",
        })?;
    work.charge_updates(expected_updates)?;
    work.charge_unary_coefficient_operation(CoefficientOperation::Negate, factor)?;
    let negative = context.neg_with_limits(factor, arithmetic.exact_algebra)?;
    work.charge_coefficient_observation(&negative)?;
    // `add_scaled_with_limits` carries every source guard and may add the
    // scale-factor denominator.  Bound that retained payload before the
    // transactional relation clone/addition allocates it.
    let predicted_guards = checked_count_add(
        row.guarded_nonzero_conditions().len(),
        pivot.unit_relation.guarded_nonzero_conditions().len(),
        "guards in one elimination row",
    )?;
    let predicted_guards = checked_count_add(predicted_guards, 1, "guards in one elimination row")?;
    let predicted_guards = checked_count_add(
        predicted_guards,
        pivot.unit_relation.terms().len(),
        "guards in one elimination row",
    )?;
    let collisions = pivot
        .unit_relation
        .terms()
        .keys()
        .filter(|shift| row.terms().contains_key(*shift))
        .count();
    let predicted_guards = checked_count_add(
        predicted_guards,
        collisions,
        "guards in one elimination row",
    )?;
    check_limit(
        "guards in one elimination row",
        predicted_guards,
        work.limits.max_retained_guards,
    )?;
    let predicted_origins = checked_count_add(
        guard_origin_count(row)?,
        guard_origin_count(&pivot.unit_relation)?,
        "guard origins in one elimination row",
    )?;
    let predicted_origins = checked_count_add(
        predicted_origins,
        pivot.unit_relation.guarded_nonzero_conditions().len(),
        "guard origins in one elimination row",
    )?;
    // One origin creates the scale-factor condition and one records its
    // attachment to the target relation.
    let predicted_origins =
        checked_count_add(predicted_origins, 2, "guard origins in one elimination row")?;
    let term_condition_count = checked_count_add(
        pivot.unit_relation.terms().len(),
        collisions,
        "guard origins in one elimination row",
    )?;
    let term_origins = term_condition_count.checked_mul(2).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "guard origins in one elimination row",
        },
    )?;
    let predicted_origins = checked_count_add(
        predicted_origins,
        term_origins,
        "guard origins in one elimination row",
    )?;
    check_limit(
        "guard origins in one elimination row",
        predicted_origins,
        work.limits.max_retained_guard_origins,
    )?;
    work.charge_scaled_relation_operation(row, &pivot.unit_relation, &negative)?;
    row.add_scaled_with_limits(context, &pivot.unit_relation, &negative, arithmetic)?;
    if row.terms().contains_key(&pivot.pivot) {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: format!("pivot {} did not eliminate exactly", pivot.ordinal),
        });
    }
    observe_relation(row, work)
}

fn verify_unit_pivot(
    context: &ParametricCoefficientContext,
    relation: &ParametricRelation,
    pivot: &IndexShift,
    work: &mut WorkBudget,
    arithmetic: ParametricArithmeticLimits,
) -> Result<(), ParametricEliminationError> {
    let coefficient = relation.terms().get(pivot).ok_or_else(|| {
        ParametricEliminationError::InternalReplayFailure {
            detail: "normalized pivot is absent".to_owned(),
        }
    })?;
    work.charge_context_constant_constructor(context, ContextConstant::One)?;
    let one = context.one();
    work.charge_binary_coefficient_operation(CoefficientOperation::Subtract, coefficient, &one)?;
    let delta = context.sub_with_limits(coefficient, &one, arithmetic.exact_algebra)?;
    work.charge_coefficient_observation(&delta)?;
    if delta.is_zero() {
        Ok(())
    } else {
        Err(ParametricEliminationError::InternalReplayFailure {
            detail: "normalized pivot coefficient is not one".to_owned(),
        })
    }
}

fn hardest_shift<'a>(
    relation: &'a ParametricRelation,
    column_rank: &BTreeMap<IndexShift, usize>,
) -> Result<&'a IndexShift, ParametricEliminationError> {
    let mut hardest: Option<(usize, &IndexShift)> = None;
    for shift in relation.terms().keys() {
        let rank = column_rank.get(shift).copied().ok_or_else(|| {
            ParametricEliminationError::MissingColumn {
                shift: shift.values().to_vec().into_boxed_slice(),
            }
        })?;
        if hardest.is_none_or(|(current, _)| rank > current) {
            hardest = Some((rank, shift));
        }
    }
    hardest.map(|(_, shift)| shift).ok_or_else(|| {
        ParametricEliminationError::InternalReplayFailure {
            detail: "cannot choose a pivot from a zero row".to_owned(),
        }
    })
}

fn observe_relation(
    relation: &ParametricRelation,
    work: &mut WorkBudget,
) -> Result<(), ParametricEliminationError> {
    work.charge_relation_observation(relation)?;
    work.maximum_row_width = work.maximum_row_width.max(relation.terms().len());
    check_limit(
        "parametric row width",
        relation.terms().len(),
        work.limits.max_columns,
    )?;
    check_limit(
        "guards in one parametric row",
        relation.guarded_nonzero_conditions().len(),
        work.limits.max_retained_guards,
    )?;
    check_limit(
        "guard origins in one parametric row",
        guard_origin_count(relation)?,
        work.limits.max_retained_guard_origins,
    )
}

fn pivot_row_id(ordinal: usize) -> ParametricRowId {
    ParametricRowId::Derived {
        label: Arc::from(format!("parametric-elimination-pivot-{ordinal}")),
    }
}

#[derive(Clone, Copy)]
enum WorkKind {
    Construction,
    Replay,
}

#[derive(Clone, Copy)]
struct WorkBudget {
    kind: WorkKind,
    limits: ParametricEliminationLimits,
    reductions: usize,
    updates: usize,
    coefficient_algebra_work: usize,
    coefficient_exponent_entry_work: usize,
    coefficient_integer_bit_work: usize,
    maximum_row_width: usize,
}

#[derive(Clone, Copy)]
enum ContextConstant {
    Zero,
    One,
}

impl WorkBudget {
    const fn construction(limits: ParametricEliminationLimits, maximum_row_width: usize) -> Self {
        Self {
            kind: WorkKind::Construction,
            limits,
            reductions: 0,
            updates: 0,
            coefficient_algebra_work: 0,
            coefficient_exponent_entry_work: 0,
            coefficient_integer_bit_work: 0,
            maximum_row_width,
        }
    }

    const fn replay(limits: ParametricEliminationLimits, maximum_row_width: usize) -> Self {
        Self {
            kind: WorkKind::Replay,
            limits,
            reductions: 0,
            updates: 0,
            coefficient_algebra_work: 0,
            coefficient_exponent_entry_work: 0,
            coefficient_integer_bit_work: 0,
            maximum_row_width,
        }
    }

    fn charge_reduction(&mut self) -> Result<(), ParametricEliminationError> {
        self.reductions = self.reductions.checked_add(1).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "parametric reductions",
            },
        )?;
        let limit = match self.kind {
            WorkKind::Construction => self.limits.max_reductions,
            WorkKind::Replay => self.limits.max_replay_reductions,
        };
        check_limit("reductions", self.reductions, limit)
    }

    fn charge_updates(&mut self, count: usize) -> Result<(), ParametricEliminationError> {
        self.updates = self.updates.checked_add(count).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "parametric sparse updates",
            },
        )?;
        let limit = match self.kind {
            WorkKind::Construction => self.limits.max_sparse_updates,
            WorkKind::Replay => self.limits.max_replay_updates,
        };
        check_limit("sparse updates", self.updates, limit)
    }

    fn charge_coefficient_work(
        &mut self,
        algebra_work: usize,
        exponent_entry_work: usize,
        integer_bit_work: usize,
    ) -> Result<(), ParametricEliminationError> {
        let next_algebra = checked_count_add(
            self.coefficient_algebra_work,
            algebra_work,
            "coefficient algebra work",
        )?;
        let next_integer_bits = checked_count_add(
            self.coefficient_integer_bit_work,
            integer_bit_work,
            "coefficient integer-bit work",
        )?;
        let next_exponent_entries = checked_count_add(
            self.coefficient_exponent_entry_work,
            exponent_entry_work,
            "coefficient exponent-entry work",
        )?;
        let (
            algebra_resource,
            algebra_limit,
            exponent_resource,
            exponent_limit,
            integer_resource,
            integer_limit,
        ) = match self.kind {
            WorkKind::Construction => (
                "construction coefficient algebra work",
                self.limits.max_construction_coefficient_algebra_work,
                "construction coefficient exponent-entry work",
                self.limits.max_construction_coefficient_exponent_entry_work,
                "construction coefficient integer-bit work",
                self.limits.max_construction_coefficient_integer_bit_work,
            ),
            WorkKind::Replay => (
                "replay coefficient algebra work",
                self.limits.max_replay_coefficient_algebra_work,
                "replay coefficient exponent-entry work",
                self.limits.max_replay_coefficient_exponent_entry_work,
                "replay coefficient integer-bit work",
                self.limits.max_replay_coefficient_integer_bit_work,
            ),
        };
        // Both prospective totals are checked before either counter is
        // committed, preserving the transactional budget on failure.
        check_limit(algebra_resource, next_algebra, algebra_limit)?;
        check_limit(exponent_resource, next_exponent_entries, exponent_limit)?;
        check_limit(integer_resource, next_integer_bits, integer_limit)?;
        self.coefficient_algebra_work = next_algebra;
        self.coefficient_exponent_entry_work = next_exponent_entries;
        self.coefficient_integer_bit_work = next_integer_bits;
        Ok(())
    }

    /// Charge construction of an elimination-internal zero or one before the
    /// context allocates it. `ParametricCoefficientContext::{zero, one}` both
    /// build fresh sparse polynomials and then call `wrap_unchecked`, whose
    /// debug assertion authenticates the complete fraction. Charging that
    /// assertion unconditionally keeps certificate counters independent of
    /// the Rust build profile.
    fn charge_context_constant_constructor(
        &mut self,
        context: &ParametricCoefficientContext,
        constant: ContextConstant,
    ) -> Result<CoefficientWorkShape, ParametricEliminationError> {
        let variable_count = checked_count_add(
            context.base().parameter_names().len(),
            context.index_count(),
            "coefficient constant variable count",
        )?;
        let shape = context_constant_coefficient_shape(variable_count, constant);
        self.charge_coefficient_shape_envelope(&shape)?;
        // Conversion from the freshly constructed numerator polynomial into
        // a RationalPolynomial creates the unit denominator and tests it for
        // one in `from_num_den(..., false)`.
        self.charge_polynomial_shape_envelope(&shape.denominator)?;
        self.charge_coefficient_validation(&shape)?;
        Ok(shape)
    }

    fn charge_binary_coefficient_operation(
        &mut self,
        operation: CoefficientOperation,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        let left_shape = self.coefficient_shape(left)?;
        let right_shape = self.coefficient_shape(right)?;
        let equal_denominator = if matches!(
            operation,
            CoefficientOperation::Add | CoefficientOperation::Subtract
        ) {
            // The prospective branch decision is itself one complete
            // polynomial equality. The lower checked sum repeats it and is
            // charged by `charge_checked_binary_from_shapes` below.
            self.charge_polynomial_equality(&left_shape.denominator, &right_shape.denominator)?;
            left.raw().denominator == right.raw().denominator
        } else {
            false
        };
        self.charge_checked_binary_from_shapes(
            operation,
            &left_shape,
            &right_shape,
            equal_denominator,
        )
        .map(|_| ())
    }

    fn charge_unary_coefficient_operation(
        &mut self,
        operation: CoefficientOperation,
        value: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        if !matches!(operation, CoefficientOperation::Negate) {
            return Err(ParametricEliminationError::InternalReplayFailure {
                detail: "a binary coefficient operation was sent to the unary work preflight"
                    .to_owned(),
            });
        }
        let shape = self.coefficient_shape(value)?;
        // `ParametricCoefficientContext::neg_with_limits` authenticates its
        // input, the lower checked negation authenticates it again, then
        // `-value.clone()` copies the complete fraction and visits the
        // numerator once more to negate its integer coefficients.
        self.charge_coefficient_validation(&shape)?;
        self.charge_coefficient_validation(&shape)?;
        self.charge_coefficient_clone(&shape)?;
        self.charge_polynomial_shape_envelope(&shape.numerator)?;
        // The lower result check, wrapper check, and debug-only unchecked-wrap
        // assertion are all charged. Charging the last pass in release builds
        // intentionally keeps certificates independent of build profile.
        for _ in 0..3 {
            self.charge_coefficient_validation(&shape)?;
        }
        Ok(())
    }

    /// Charge one complete checked binary call after its operand shapes have
    /// already been censused. This mirrors the public parametric wrapper, the
    /// lower checked exact-algebra API, its degree preflight, raw sparse
    /// arithmetic envelope, and every accepted-result validation.
    fn charge_checked_binary_from_shapes(
        &mut self,
        operation: CoefficientOperation,
        left: &CoefficientWorkShape,
        right: &CoefficientWorkShape,
        equal_denominator: bool,
    ) -> Result<CoefficientWorkShape, ParametricEliminationError> {
        if matches!(operation, CoefficientOperation::Negate) {
            return Err(ParametricEliminationError::InternalReplayFailure {
                detail: "a unary coefficient operation was sent to the binary work preflight"
                    .to_owned(),
            });
        }

        // Parametric wrapper validation, followed by the lower checked API's
        // independent binary-input validation.
        for _ in 0..2 {
            self.charge_coefficient_validation(left)?;
            self.charge_coefficient_validation(right)?;
        }

        if matches!(
            operation,
            CoefficientOperation::Add | CoefficientOperation::Subtract
        ) {
            // `checked_coefficient_sum_on_map` repeats the denominator
            // equality used to select its fast path.
            self.charge_polynomial_equality(&left.denominator, &right.denominator)?;
        }
        if matches!(
            operation,
            CoefficientOperation::Multiply | CoefficientOperation::Divide
        ) || (matches!(
            operation,
            CoefficientOperation::Add | CoefficientOperation::Subtract
        ) && !equal_denominator)
        {
            // Product/division preflights and the cross-denominator sum
            // preflight each compute every per-variable degree once.
            self.charge_coefficient_degree_scan(left)?;
            self.charge_coefficient_degree_scan(right)?;
        }

        // The raw operators are Symbolica `RationalPolynomial` operations,
        // not the simpler cross-product formulas used by RustRed's term-cap
        // preflight. Account for every sparse traversal and intermediate
        // operation surrounding Symbolica's native polynomial GCD calls.
        // The internal workspace of those GCD calls remains the sole opaque
        // native seam.
        self.charge_symbolica_rational_binary_surroundings(
            operation,
            left,
            right,
            equal_denominator,
        )?;

        let max_polynomial_terms = self.limits.arithmetic.exact_algebra.max_polynomial_terms;
        let estimate = coefficient_operation_estimate_from_shapes(
            self,
            operation,
            left,
            right,
            equal_denominator,
            max_polynomial_terms,
        )?;
        let (algebra_work, exponent_entry_work, integer_bit_work) =
            coefficient_operation_arithmetic_only(&estimate, left, right)?;
        self.charge_coefficient_work(algebra_work, exponent_entry_work, integer_bit_work)?;

        // Lower result authentication, public wrapper authentication, and
        // the debug-only unchecked-wrap assertion.
        for _ in 0..3 {
            self.charge_coefficient_validation(&estimate.output)?;
        }
        Ok(estimate.output)
    }

    fn charge_symbolica_rational_binary_surroundings(
        &mut self,
        operation: CoefficientOperation,
        left: &CoefficientWorkShape,
        right: &CoefficientWorkShape,
        equal_denominator: bool,
    ) -> Result<(), ParametricEliminationError> {
        match operation {
            CoefficientOperation::Add => {
                self.charge_symbolica_rational_add_surroundings(left, right, equal_denominator)?;
            }
            CoefficientOperation::Subtract => {
                // Symbolica's borrowed subtraction is literally
                // `self.add(&other.clone().neg())`.
                self.charge_coefficient_clone(right)?;
                self.charge_polynomial_shape_envelope(&right.numerator)?;
                self.charge_symbolica_rational_add_surroundings(left, right, equal_denominator)?;
            }
            CoefficientOperation::Multiply => {
                self.charge_symbolica_rational_multiply_surroundings(left, right)?;
            }
            CoefficientOperation::Divide => {
                // Symbolica implements borrowed division as multiplication
                // by a cloned, in-place inverted right-hand fraction.
                self.charge_coefficient_clone(right)?;
                self.charge_symbolica_rational_inversion_surroundings(right)?;
                let inverted_numerator = copy_polynomial_work_shape(self, &right.denominator)?;
                let inverted_denominator = copy_polynomial_work_shape(self, &right.numerator)?;
                let inverted_right = CoefficientWorkShape {
                    numerator: inverted_numerator,
                    denominator: inverted_denominator,
                };
                self.charge_symbolica_rational_multiply_surroundings(left, &inverted_right)?;
            }
            CoefficientOperation::Negate => {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail:
                        "a unary coefficient operation reached Symbolica's binary work envelope"
                            .to_owned(),
                });
            }
        }
        Ok(())
    }

    fn charge_symbolica_rational_inversion_surroundings(
        &mut self,
        value: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        // `inv` tests the old numerator for zero. `from_num_den(..., false)`
        // then uses it as the new denominator, tests it for one, inspects its
        // leading sign, and may negate it. The old denominator may likewise
        // be negated as the new numerator. Charge the longest legal branch;
        // the caller separately charges the full RHS clone.
        for _ in 0..4 {
            self.charge_polynomial_shape_envelope(&value.numerator)?;
        }
        self.charge_polynomial_shape_envelope(&value.denominator)
    }

    fn charge_symbolica_rational_final_normalization_surroundings(
        &mut self,
        value: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        // `from_num_den(..., true)` first tests the denominator for one. On
        // the longest legal branch it then materializes a polynomial GCD,
        // tests that GCD for one, exactly divides both sides, inspects the
        // normalized denominator's leading sign, and negates both quotients.
        // Native GCD workspace is opaque, but every surrounding traversal and
        // exact-division intermediate must be admitted before entering
        // Symbolica.
        self.charge_polynomial_shape_envelope(&value.denominator)?;
        let gcd =
            self.charge_polynomial_gcd_surroundings(&value.numerator, &value.denominator, false)?;
        let numerator =
            self.charge_polynomial_exact_division_surroundings(&value.numerator, &gcd, None)?;
        let denominator =
            self.charge_polynomial_exact_division_surroundings(&value.denominator, &gcd, None)?;
        self.charge_polynomial_shape_envelope(&denominator)?;
        self.charge_polynomial_shape_envelope(&numerator)?;
        self.charge_polynomial_shape_envelope(&denominator)
    }

    fn charge_symbolica_rational_add_surroundings(
        &mut self,
        left: &CoefficientWorkShape,
        right: &CoefficientWorkShape,
        equal_denominator: bool,
    ) -> Result<(), ParametricEliminationError> {
        let denominator_gcd = self.charge_polynomial_gcd_surroundings(
            &left.denominator,
            &right.denominator,
            equal_denominator,
        )?;
        let one = constant_one_polynomial_shape(left.denominator.variable_count);
        let left_reduced = self.charge_polynomial_exact_division_surroundings(
            &left.denominator,
            &denominator_gcd,
            equal_denominator.then_some(&one),
        )?;
        let right_reduced = self.charge_polynomial_exact_division_surroundings(
            &right.denominator,
            &denominator_gcd,
            equal_denominator.then_some(&one),
        )?;

        let left_numerator =
            self.charge_polynomial_product_surroundings(&left.numerator, &right_reduced)?;
        let right_numerator =
            self.charge_polynomial_product_surroundings(&right.numerator, &left_reduced)?;
        let numerator =
            self.charge_polynomial_sum_surroundings(&left_numerator, &right_numerator)?;

        // Symbolica selects one of these algebraically equal denominator
        // products based on sparse term counts. Charge both possible branches
        // and retain their componentwise upper bound for final cancellation.
        let denominator_left =
            self.charge_polynomial_product_surroundings(&right_reduced, &left.denominator)?;
        let denominator_right =
            self.charge_polynomial_product_surroundings(&left_reduced, &right.denominator)?;
        let denominator = polynomial_shape_componentwise_upper_bound(
            self,
            &denominator_left,
            &denominator_right,
        )?;

        let final_gcd =
            self.charge_polynomial_gcd_surroundings(&numerator, &denominator_gcd, false)?;
        self.charge_polynomial_exact_division_surroundings(&numerator, &final_gcd, None)?;
        self.charge_polynomial_exact_division_surroundings(&denominator, &final_gcd, None)?;
        Ok(())
    }

    fn charge_symbolica_rational_multiply_surroundings(
        &mut self,
        left: &CoefficientWorkShape,
        right: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        let numerator_gcd =
            self.charge_polynomial_gcd_surroundings(&left.numerator, &right.denominator, false)?;
        let denominator_gcd =
            self.charge_polynomial_gcd_surroundings(&left.denominator, &right.numerator, false)?;

        // Each division is conditional on a non-unit cross GCD in Symbolica.
        // Charging all four legal reductions covers every branch without an
        // unbudgeted prospective native GCD.
        let left_numerator = self.charge_polynomial_exact_division_surroundings(
            &left.numerator,
            &numerator_gcd,
            None,
        )?;
        let right_denominator = self.charge_polynomial_exact_division_surroundings(
            &right.denominator,
            &numerator_gcd,
            None,
        )?;
        let left_denominator = self.charge_polynomial_exact_division_surroundings(
            &left.denominator,
            &denominator_gcd,
            None,
        )?;
        let right_numerator = self.charge_polynomial_exact_division_surroundings(
            &right.numerator,
            &denominator_gcd,
            None,
        )?;

        self.charge_polynomial_product_surroundings(&left_numerator, &right_numerator)?;
        self.charge_polynomial_product_surroundings(&left_denominator, &right_denominator)?;
        Ok(())
    }

    fn charge_polynomial_gcd_surroundings(
        &mut self,
        left: &PolynomialWorkShape,
        right: &PolynomialWorkShape,
        known_equal: bool,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        self.charge_polynomial_shape_envelope(left)?;
        self.charge_polynomial_shape_envelope(right)?;
        let output = if known_equal {
            polynomial_shape_componentwise_upper_bound(self, left, right)?
        } else {
            let max_polynomial_terms = self.limits.arithmetic.exact_algebra.max_polynomial_terms;
            polynomial_common_factor_shape_bound(self, left, right, max_polynomial_terms)?
        };
        // Materialized GCD output and the following `is_one` branch test.
        self.charge_polynomial_shape_envelope(&output)?;
        self.charge_polynomial_shape_envelope(&output)?;
        Ok(output)
    }

    fn charge_polynomial_exact_division_surroundings(
        &mut self,
        dividend: &PolynomialWorkShape,
        divisor: &PolynomialWorkShape,
        known_output: Option<&PolynomialWorkShape>,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        self.charge_polynomial_shape_envelope(dividend)?;
        self.charge_polynomial_shape_envelope(divisor)?;
        let output = match known_output {
            Some(output) => copy_polynomial_work_shape(self, output)?,
            None => {
                let max_polynomial_terms =
                    self.limits.arithmetic.exact_algebra.max_polynomial_terms;
                canonical_polynomial_factor_shape_bound(self, dividend, max_polynomial_terms)?
            }
        };
        // Exact sparse division is enveloped by reconstructing the dividend
        // from the quotient/divisor term pairs. This is separate from opaque
        // native GCD workspace.
        let reconstruction = polynomial_product_work(self, &output, divisor)?;
        self.charge_coefficient_work(
            reconstruction.algebra_work,
            reconstruction.exponent_entry_work,
            reconstruction.integer_bit_work,
        )?;
        self.charge_polynomial_shape_envelope(&output)?;
        Ok(output)
    }

    fn charge_polynomial_product_surroundings(
        &mut self,
        left: &PolynomialWorkShape,
        right: &PolynomialWorkShape,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        self.charge_polynomial_shape_envelope(left)?;
        self.charge_polynomial_shape_envelope(right)?;
        let estimate = polynomial_product_work(self, left, right)?;
        self.charge_coefficient_work(
            estimate.algebra_work,
            estimate.exponent_entry_work,
            estimate.integer_bit_work,
        )?;
        self.charge_polynomial_shape_envelope(&estimate.output)?;
        Ok(estimate.output)
    }

    fn charge_polynomial_sum_surroundings(
        &mut self,
        left: &PolynomialWorkShape,
        right: &PolynomialWorkShape,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        self.charge_polynomial_shape_envelope(left)?;
        self.charge_polynomial_shape_envelope(right)?;
        let estimate = polynomial_sum_work(self, left, right)?;
        self.charge_coefficient_work(
            estimate.algebra_work,
            estimate.exponent_entry_work,
            estimate.integer_bit_work,
        )?;
        self.charge_polynomial_shape_envelope(&estimate.output)?;
        Ok(estimate.output)
    }

    fn charge_coefficient_observation(
        &mut self,
        coefficient: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        self.coefficient_shape(coefficient).map(|_| ())
    }

    fn charge_relation_observation(
        &mut self,
        relation: &ParametricRelation,
    ) -> Result<(), ParametricEliminationError> {
        for coefficient in relation.terms().values() {
            self.charge_coefficient_observation(coefficient)?;
        }
        for condition in relation.guarded_nonzero_conditions() {
            self.polynomial_shape(condition.polynomial().raw())?;
        }
        Ok(())
    }

    fn charge_relation_equality(
        &mut self,
        left: &ParametricRelation,
        right: &ParametricRelation,
    ) -> Result<(), ParametricEliminationError> {
        // `has_identical_guard_provenance` first compares mathematical terms
        // plus the compatibility guard vector, then compares the complete
        // provenance-bearing guard vector. A mismatch may occur at the final
        // payload, so both relations are charged in full.
        for relation in [left, right] {
            for coefficient in relation.terms().values() {
                let shape = self.coefficient_shape(coefficient)?;
                self.charge_coefficient_shape_envelope(&shape)?;
            }
            for polynomial in relation.nonzero_conditions() {
                let shape = self.polynomial_shape(polynomial.raw())?;
                self.charge_polynomial_shape_envelope(&shape)?;
            }
            for condition in relation.guarded_nonzero_conditions() {
                let shape = self.polynomial_shape(condition.polynomial().raw())?;
                self.charge_polynomial_shape_envelope(&shape)?;
            }
        }
        Ok(())
    }

    fn charge_relation_payload_traversals(
        &mut self,
        relation: &ParametricRelation,
        traversals_after_shape_scan: usize,
    ) -> Result<(), ParametricEliminationError> {
        for coefficient in relation.terms().values() {
            let shape = self.coefficient_shape(coefficient)?;
            for _ in 0..traversals_after_shape_scan {
                self.charge_coefficient_shape_envelope(&shape)?;
            }
        }
        for condition in relation.guarded_nonzero_conditions() {
            let shape = self.polynomial_shape(condition.polynomial().raw())?;
            for _ in 0..traversals_after_shape_scan {
                self.charge_polynomial_shape_envelope(&shape)?;
            }
        }
        Ok(())
    }

    fn charge_relation_retained_bound_traversal(
        &mut self,
        relation: &ParametricRelation,
    ) -> Result<(), ParametricEliminationError> {
        for coefficient in relation.terms().values() {
            let shape = self.coefficient_shape(coefficient)?;
            self.charge_coefficient_shape_envelope(&shape)?;
        }
        // The relation's retained-byte routine walks both the compatibility
        // polynomial vector and the provenance-bearing guard vector.
        for polynomial in relation.nonzero_conditions() {
            let shape = self.polynomial_shape(polynomial.raw())?;
            self.charge_polynomial_shape_envelope(&shape)?;
        }
        for condition in relation.guarded_nonzero_conditions() {
            let shape = self.polynomial_shape(condition.polynomial().raw())?;
            self.charge_polynomial_shape_envelope(&shape)?;
        }
        Ok(())
    }

    fn charge_polynomial_retained_bound_traversal(
        &mut self,
        polynomial: &ParametricPolynomial,
    ) -> Result<(), ParametricEliminationError> {
        let shape = self.polynomial_shape(polynomial.raw())?;
        self.charge_polynomial_shape_envelope(&shape)
    }

    /// Charge the complete first phase of guarded division before any of its
    /// three condition polynomials is cloned or its checked quotient enters
    /// Symbolica. Native GCD workspace remains the documented opaque seam;
    /// every Rust-visible validation, clone, property scan, and condition
    /// equality surrounding it is included here.
    fn charge_guarded_division_pending_operation(
        &mut self,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        let dividend_shape = self.coefficient_shape(dividend)?;
        let divisor_shape = self.coefficient_shape(divisor)?;

        // Outer guarded-division authentication.
        self.charge_coefficient_validation(&dividend_shape)?;
        self.charge_coefficient_validation(&divisor_shape)?;

        let candidates = [
            &dividend_shape.denominator,
            &divisor_shape.denominator,
            &divisor_shape.numerator,
        ];
        self.charge_guarded_division_condition_candidates(candidates)?;

        // `checked_div_with_limits` is the ordinary wrapper/lower checked
        // binary path nested at the end of guarded pending construction.
        self.charge_checked_binary_from_shapes(
            CoefficientOperation::Divide,
            &dividend_shape,
            &divisor_shape,
            false,
        )?;
        Ok(())
    }

    fn charge_guarded_division_condition_candidates(
        &mut self,
        candidates: [&PolynomialWorkShape; 3],
    ) -> Result<(), ParametricEliminationError> {
        // The candidate array clones all three polynomials before processing
        // the first condition.
        for candidate in candidates {
            self.charge_polynomial_clone(candidate)?;
        }

        let mut prior_nonconstant: Vec<&PolynomialWorkShape> = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            // `is_constant` may inspect the full dense exponent payload.
            self.charge_polynomial_shape_envelope(candidate)?;
            if candidate.is_constant()? {
                // The debug assertion that a skipped constant is nonzero is
                // charged in every build profile.
                self.charge_polynomial_shape_envelope(candidate)?;
                continue;
            }
            // Explicit candidate validation and the independent validation in
            // `nonzero_condition_with_origins_and_limits`.
            self.charge_polynomial_validation(candidate)?;
            self.charge_polynomial_validation(candidate)?;
            // Insertion compares with retained earlier conditions. A matched
            // duplicate then enters `merge_origins_from`, whose debug
            // assertion repeats the polynomial equality. Charge every
            // possible match position so first and last duplicates are both
            // covered without performing an unbudgeted prospective search.
            for previous in &prior_nonconstant {
                self.charge_polynomial_equality(candidate, *previous)?;
            }
            for previous in &prior_nonconstant {
                self.charge_polynomial_equality(candidate, *previous)?;
            }
            prior_nonconstant.push(candidate);
        }
        Ok(())
    }

    /// Authenticate a source coefficient under the same cumulative budget
    /// used by manifest construction and elimination. Cheap vector lengths
    /// are charged before integer-size or exponent scans; the repeated
    /// lower-layer validator pass and complete dense-layout census are
    /// precharged from the cached shape before semantic validation.
    fn authenticate_coefficient(
        &mut self,
        context: &ParametricCoefficientContext,
        coefficient: &ParametricCoefficient,
        limits: crate::algebra::ExactAlgebraLimits,
    ) -> Result<CoefficientWorkShape, ParametricEliminationError> {
        let numerator = self.preflight_polynomial_shape(&coefficient.raw().numerator)?;
        let denominator = self.preflight_polynomial_shape(&coefficient.raw().denominator)?;
        self.charge_pending_polynomial_validation(numerator)?;
        self.charge_pending_polynomial_validation(denominator)?;
        let numerator_shape =
            complete_polynomial_shape(self, &coefficient.raw().numerator, numerator)?;
        let denominator_shape =
            complete_polynomial_shape(self, &coefficient.raw().denominator, denominator)?;
        context.validate_with_limits(coefficient, limits)?;
        Ok(CoefficientWorkShape {
            numerator: numerator_shape,
            denominator: denominator_shape,
        })
    }

    fn authenticate_polynomial(
        &mut self,
        context: &ParametricCoefficientContext,
        polynomial: &ParametricPolynomial,
        limits: crate::algebra::ExactAlgebraLimits,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        let pending = self.preflight_polynomial_shape(polynomial.raw())?;
        self.charge_pending_polynomial_validation(pending)?;
        context.validate_polynomial_with_limits(polynomial, limits)?;
        complete_polynomial_shape(self, polynomial.raw(), pending)
    }

    fn coefficient_shape(
        &mut self,
        coefficient: &ParametricCoefficient,
    ) -> Result<CoefficientWorkShape, ParametricEliminationError> {
        Ok(CoefficientWorkShape {
            numerator: self.polynomial_shape(&coefficient.raw().numerator)?,
            denominator: self.polynomial_shape(&coefficient.raw().denominator)?,
        })
    }

    fn polynomial_shape(
        &mut self,
        polynomial: &crate::CoefficientPolynomial,
    ) -> Result<PolynomialWorkShape, ParametricEliminationError> {
        let pending = self.preflight_polynomial_shape(polynomial)?;
        let expected = pending.layout.expected_exponent_entries()?;
        if polynomial.exponents.len() != expected {
            return Err(ParametricEliminationError::InternalReplayFailure {
                detail: format!(
                    "authenticated polynomial shape has {} terms, {} exponents, and {} variables",
                    pending.layout.terms,
                    polynomial.exponents.len(),
                    pending.layout.variable_count
                ),
            });
        }
        complete_polynomial_shape(self, polynomial, pending)
    }

    fn preflight_polynomial_shape(
        &mut self,
        polynomial: &crate::CoefficientPolynomial,
    ) -> Result<PendingPolynomialWorkShape, ParametricEliminationError> {
        let layout = PolynomialWorkLayout::new(polynomial);
        // This cheap admission happens before either the GMP magnitude census
        // or the dense degree scan.
        self.charge_coefficient_work(layout.terms, layout.exponent_entries, 0)?;
        let mut maximum_integer_bits = 0usize;
        let mut total_integer_bits = 0usize;
        for coefficient in &polynomial.coefficients {
            let bits = integer_magnitude_bits_usize(coefficient)?;
            self.charge_coefficient_work(0, 0, bits)?;
            maximum_integer_bits = maximum_integer_bits.max(bits);
            total_integer_bits =
                checked_count_add(total_integer_bits, bits, "coefficient integer-bit work")?;
        }
        Ok(PendingPolynomialWorkShape {
            layout,
            maximum_integer_bits,
            total_integer_bits,
        })
    }

    fn charge_pending_polynomial_validation(
        &mut self,
        pending: PendingPolynomialWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        // One validator visits every coefficient and exponent, then compares
        // each exponent row after the first with its predecessor to prove
        // canonical monomial order. Charge a conservative bit envelope as
        // well, even though integer zero tests normally inspect only a sign
        // word rather than every GMP limb.
        let ordering_entries = pending
            .layout
            .terms
            .saturating_sub(1)
            .checked_mul(pending.layout.variable_count)
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient exponent-entry work",
            })?;
        self.charge_coefficient_work(
            pending.layout.terms,
            checked_count_add(
                pending.layout.exponent_entries,
                ordering_entries,
                "coefficient exponent-entry work",
            )?,
            pending.total_integer_bits,
        )
    }

    fn charge_polynomial_validation(
        &mut self,
        shape: &PolynomialWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        let ordering_entries = shape
            .terms
            .saturating_sub(1)
            .checked_mul(shape.variable_count)
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient exponent-entry work",
            })?;
        self.charge_coefficient_work(
            shape.terms,
            checked_count_add(
                shape.exponent_entries,
                ordering_entries,
                "coefficient exponent-entry work",
            )?,
            shape.total_integer_bits,
        )
    }

    fn charge_coefficient_validation(
        &mut self,
        shape: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_polynomial_validation(&shape.numerator)?;
        self.charge_polynomial_validation(&shape.denominator)
    }

    fn charge_polynomial_clone(
        &mut self,
        shape: &PolynomialWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_polynomial_shape_envelope(shape)
    }

    fn charge_coefficient_clone(
        &mut self,
        shape: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_coefficient_shape_envelope(shape)
    }

    fn charge_polynomial_equality(
        &mut self,
        left: &PolynomialWorkShape,
        right: &PolynomialWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        // A mismatch may occur only at the final sparse entry, so both sides
        // are included in the deterministic worst-case envelope.
        self.charge_polynomial_shape_envelope(left)?;
        self.charge_polynomial_shape_envelope(right)
    }

    fn charge_coefficient_degree_scan(
        &mut self,
        shape: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        // `degree(variable)` scans one exponent from every term. Called for
        // every variable, this visits each dense exponent entry once.
        self.charge_coefficient_work(0, shape.total_exponent_entries()?, 0)
    }

    fn charge_polynomial_shape_envelope(
        &mut self,
        shape: &PolynomialWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_coefficient_work(
            shape.terms,
            shape.exponent_entries,
            shape.total_integer_bits,
        )
    }

    fn charge_coefficient_shape_envelope(
        &mut self,
        shape: &CoefficientWorkShape,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_coefficient_work(
            shape.total_terms()?,
            shape.total_exponent_entries()?,
            shape.total_integer_bits()?,
        )
    }

    /// Charge RustRed-visible work of the explicit second normalization in
    /// guarded division. This runs after the first checked quotient has
    /// returned, but before the second native GCD call, so the input term
    /// pair and dense exponent arrays are actual rather than guessed.
    fn charge_guarded_division_final_normalization(
        &mut self,
        value: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        let shape = self.coefficient_shape(value)?;
        self.charge_coefficient_validation(&shape)?;
        self.charge_symbolica_rational_final_normalization_surroundings(&shape)?;
        let max_polynomial_terms = self.limits.arithmetic.exact_algebra.max_polynomial_terms;
        let estimate = coefficient_final_normalization_estimate_from_shape(
            self,
            &shape,
            max_polynomial_terms,
        )?;
        let (algebra_work, exponent_entry_work, integer_bit_work) =
            coefficient_operation_arithmetic_only_unary(&estimate, &shape)?;
        self.charge_coefficient_work(algebra_work, exponent_entry_work, integer_bit_work)?;
        // `wrap_checked_with_limits` authenticates the canonical result and
        // its unchecked wrapper repeats that proof in debug builds. Charge
        // both unconditionally so persisted counters are profile-independent.
        self.charge_coefficient_validation(&estimate.output)?;
        self.charge_coefficient_validation(&estimate.output)
    }

    /// Charge every rational multiplication and possible coefficient
    /// collection performed by `target += factor * source`, including the
    /// transactional target clone and every denominator-guard adapter pass.
    fn charge_scaled_relation_operation(
        &mut self,
        target: &ParametricRelation,
        source: &ParametricRelation,
        factor: &ParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        self.charge_relation_clone(target)?;
        let factor_shape = self.coefficient_shape(factor)?;

        let collisions = source
            .terms()
            .keys()
            .filter(|shift| target.terms().contains_key(*shift))
            .count();
        let prospective_conditions = target
            .guarded_nonzero_conditions()
            .len()
            .checked_add(source.guarded_nonzero_conditions().len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_add(source.terms().len()))
            .and_then(|count| count.checked_add(collisions))
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "guards in one elimination row",
            })?;
        check_limit(
            "guards in one elimination row",
            prospective_conditions,
            self.limits.max_retained_guards,
        )?;
        let mut attached_conditions = Vec::new();
        attached_conditions
            .try_reserve_exact(prospective_conditions)
            .map_err(|_| ParametricEliminationError::ResourceLimit {
                resource: "guard work-shape allocation",
                requested: prospective_conditions,
                limit: self.limits.max_retained_guards,
            })?;
        for condition in target.guarded_nonzero_conditions() {
            attached_conditions.push(self.polynomial_shape(condition.polynomial().raw())?);
        }
        self.charge_add_scaled_in_place(
            Some(target),
            source,
            &factor_shape,
            &mut attached_conditions,
        )
    }

    fn charge_scaled_relation_operation_into_empty(
        &mut self,
        source: &ParametricRelation,
        factor: &GuardedParametricCoefficient,
    ) -> Result<(), ParametricEliminationError> {
        let factor_shape = self.coefficient_shape(&factor.value)?;
        let prospective_conditions = factor
            .nonzero
            .len()
            .checked_add(source.guarded_nonzero_conditions().len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_add(source.terms().len()))
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "guards in one normalized pivot",
            })?;
        check_limit(
            "guards in one normalized pivot",
            prospective_conditions,
            self.limits.max_retained_guards,
        )?;
        let mut attached_conditions = Vec::new();
        attached_conditions
            .try_reserve_exact(prospective_conditions)
            .map_err(|_| ParametricEliminationError::ResourceLimit {
                resource: "guard work-shape allocation",
                requested: prospective_conditions,
                limit: self.limits.max_retained_guards,
            })?;

        // `add_scaled_guarded_with_limits` validates the guarded factor before
        // moving its conditions into the cloned empty target.
        self.charge_coefficient_validation(&factor_shape)?;
        for condition in &factor.nonzero {
            let shape = self.polynomial_shape(condition.polynomial().raw())?;
            self.charge_condition_attachment(shape, &mut attached_conditions)?;
        }
        self.charge_add_scaled_in_place(None, source, &factor_shape, &mut attached_conditions)
    }

    fn charge_relation_clone(
        &mut self,
        relation: &ParametricRelation,
    ) -> Result<(), ParametricEliminationError> {
        for coefficient in relation.terms().values() {
            let shape = self.coefficient_shape(coefficient)?;
            self.charge_coefficient_clone(&shape)?;
        }
        // The compatibility and provenance-bearing vectors each own their
        // own polynomial clone.
        for polynomial in relation.nonzero_conditions() {
            let shape = self.polynomial_shape(polynomial.raw())?;
            self.charge_polynomial_clone(&shape)?;
        }
        for condition in relation.guarded_nonzero_conditions() {
            let shape = self.polynomial_shape(condition.polynomial().raw())?;
            self.charge_polynomial_clone(&shape)?;
        }
        Ok(())
    }

    fn charge_condition_attachment(
        &mut self,
        candidate: PolynomialWorkShape,
        attached: &mut Vec<PolynomialWorkShape>,
    ) -> Result<(), ParametricEliminationError> {
        // Context containment and the explicit bounded validator are separate
        // passes, followed by zero/constant classification.
        self.charge_polynomial_validation(&candidate)?;
        self.charge_polynomial_validation(&candidate)?;
        self.charge_polynomial_shape_envelope(&candidate)?;

        // `is_new` and `insert_parametric_condition` independently search the
        // existing condition vector. Assume no early match for a conservative
        // deterministic envelope.
        for existing in attached.iter() {
            self.charge_polynomial_equality(&candidate, existing)?;
        }
        // The compatibility polynomial-only view is cloned even when the
        // later insertion discovers a duplicate and drops this copy.
        self.charge_polynomial_clone(&candidate)?;
        for existing in attached.iter() {
            self.charge_polynomial_equality(&candidate, existing)?;
        }
        // A duplicate enters `ParametricNonZeroCondition::merge_origins_from`,
        // whose debug assertion repeats equality against the matched
        // polynomial. Charge all possible match positions so the envelope is
        // build-profile independent and covers first as well as last matches.
        for existing in attached.iter() {
            self.charge_polynomial_equality(&candidate, existing)?;
        }
        attached.push(candidate);
        Ok(())
    }

    fn charge_denominator_guard_from_shape(
        &mut self,
        coefficient: &CoefficientWorkShape,
        attached: &mut Vec<PolynomialWorkShape>,
    ) -> Result<(), ParametricEliminationError> {
        // `denominator_condition_with_limits` validates the complete
        // coefficient before cloning its denominator.
        self.charge_coefficient_validation(coefficient)?;
        self.charge_polynomial_clone(&coefficient.denominator)?;
        // Condition construction authenticates that cloned polynomial once;
        // relation attachment performs two further validations.
        self.charge_polynomial_validation(&coefficient.denominator)?;
        let denominator = copy_polynomial_work_shape(self, &coefficient.denominator)?;
        self.charge_condition_attachment(denominator, attached)
    }

    fn charge_add_scaled_in_place(
        &mut self,
        target: Option<&ParametricRelation>,
        source: &ParametricRelation,
        factor: &CoefficientWorkShape,
        attached: &mut Vec<PolynomialWorkShape>,
    ) -> Result<(), ParametricEliminationError> {
        // `add_scaled_in_place` authenticates the factor independently of any
        // guarded-wrapper validation.
        self.charge_coefficient_validation(factor)?;
        for condition in source.guarded_nonzero_conditions() {
            let shape = self.polynomial_shape(condition.polynomial().raw())?;
            // The source condition is cloned before attachment.
            self.charge_polynomial_clone(&shape)?;
            self.charge_condition_attachment(shape, attached)?;
        }
        self.charge_denominator_guard_from_shape(factor, attached)?;

        for (shift, coefficient) in source.terms() {
            let source_shape = self.coefficient_shape(coefficient)?;
            let scaled = self.charge_checked_binary_from_shapes(
                CoefficientOperation::Multiply,
                &source_shape,
                factor,
                false,
            )?;

            // `add_term_in_place` authenticates the incoming coefficient and
            // always discovers its pre-zero-test denominator condition.
            self.charge_coefficient_validation(&scaled)?;
            self.charge_denominator_guard_from_shape(&scaled, attached)?;

            if let Some(current) = target.and_then(|target| target.terms().get(shift)) {
                let current_shape = self.coefficient_shape(current)?;
                // Assuming unequal denominators selects the larger safe raw
                // cross-product envelope; the lower equality scan is still
                // charged by the checked binary helper.
                let sum = self.charge_checked_binary_from_shapes(
                    CoefficientOperation::Add,
                    &current_shape,
                    &scaled,
                    false,
                )?;
                // A zero collected sum skips this path. Charging the complete
                // nonzero denominator adapter remains conservative.
                self.charge_denominator_guard_from_shape(&sum, attached)?;
            }
        }
        Ok(())
    }
}

/// Construction or replay vocabulary used by the narrow coefficient-work
/// facade. The facade deliberately keeps the estimator's private shape model
/// and operation enum out of database-facing modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParametricCoefficientWorkPhase {
    Construction,
    Replay,
}

/// Independent ceilings for one sequence of exact coefficient operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricCoefficientWorkLedgerLimits {
    pub(crate) arithmetic: ParametricArithmeticLimits,
    pub(crate) max_algebra_work: usize,
    pub(crate) max_exponent_entry_work: usize,
    pub(crate) max_integer_bit_work: usize,
    /// Maximum number of denominator-attachment attempts retained by this
    /// ledger. Duplicate and constant candidates remain in this history
    /// because their comparison/validation work is still observable.
    pub(crate) max_guard_history: usize,
}

impl Default for ParametricCoefficientWorkLedgerLimits {
    fn default() -> Self {
        let limits = ParametricEliminationLimits::default();
        Self {
            arithmetic: limits.arithmetic,
            max_algebra_work: limits.max_construction_coefficient_algebra_work,
            max_exponent_entry_work: limits.max_construction_coefficient_exponent_entry_work,
            max_integer_bit_work: limits.max_construction_coefficient_integer_bit_work,
            max_guard_history: limits.max_retained_guards,
        }
    }
}

/// Immutable cumulative counters exported by the coefficient-work facade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricCoefficientWorkStats {
    algebra_work: usize,
    exponent_entry_work: usize,
    integer_bit_work: usize,
}

impl ParametricCoefficientWorkStats {
    pub(crate) const fn algebra_work(self) -> usize {
        self.algebra_work
    }

    pub(crate) const fn exponent_entry_work(self) -> usize {
        self.exponent_entry_work
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }
}

/// Failure of one transactionally charged coefficient operation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParametricCoefficientWorkError {
    Elimination(ParametricEliminationError),
    SparsePayloadAllocation { resource: &'static str },
}

impl fmt::Display for ParametricCoefficientWorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Elimination(error) => error.fmt(formatter),
            Self::SparsePayloadAllocation { resource } => {
                write!(formatter, "failed to reserve {resource}")
            }
        }
    }
}

impl std::error::Error for ParametricCoefficientWorkError {}

impl From<ParametricEliminationError> for ParametricCoefficientWorkError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

impl From<ParametricCoefficientError> for ParametricCoefficientWorkError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Elimination(value.into())
    }
}

/// Charge-and-execute facade over the private elimination coefficient-work
/// estimator. Callers select semantic operations rather than constructing a
/// shape estimate themselves, so the charged operation cannot diverge from
/// the Symbolica operation that actually runs.
///
/// Every method executes against a copied budget and commits its counters only
/// after the lower exact operation succeeds. A rejected or invalid operation
/// therefore leaves [`Self::stats`] unchanged.
pub(crate) struct ParametricCoefficientWorkLedger {
    work: WorkBudget,
    arithmetic: ParametricArithmeticLimits,
    guard_history: Vec<PolynomialWorkShape>,
    max_guard_history: usize,
}

impl ParametricCoefficientWorkLedger {
    pub(crate) fn new(
        phase: ParametricCoefficientWorkPhase,
        limits: ParametricCoefficientWorkLedgerLimits,
    ) -> Self {
        let mut elimination_limits = ParametricEliminationLimits::default();
        elimination_limits.arithmetic = limits.arithmetic;
        elimination_limits.max_construction_coefficient_algebra_work = limits.max_algebra_work;
        elimination_limits.max_construction_coefficient_exponent_entry_work =
            limits.max_exponent_entry_work;
        elimination_limits.max_construction_coefficient_integer_bit_work =
            limits.max_integer_bit_work;
        elimination_limits.max_replay_coefficient_algebra_work = limits.max_algebra_work;
        elimination_limits.max_replay_coefficient_exponent_entry_work =
            limits.max_exponent_entry_work;
        elimination_limits.max_replay_coefficient_integer_bit_work = limits.max_integer_bit_work;
        elimination_limits.max_retained_guards = limits.max_guard_history;
        let work = match phase {
            ParametricCoefficientWorkPhase::Construction => {
                WorkBudget::construction(elimination_limits, 0)
            }
            ParametricCoefficientWorkPhase::Replay => WorkBudget::replay(elimination_limits, 0),
        };
        Self {
            work,
            arithmetic: limits.arithmetic,
            guard_history: Vec::new(),
            max_guard_history: limits.max_guard_history,
        }
    }

    pub(crate) const fn stats(&self) -> ParametricCoefficientWorkStats {
        ParametricCoefficientWorkStats {
            algebra_work: self.work.coefficient_algebra_work,
            exponent_entry_work: self.work.coefficient_exponent_entry_work,
            integer_bit_work: self.work.coefficient_integer_bit_work,
        }
    }

    pub(crate) fn try_one(
        &mut self,
        context: &ParametricCoefficientContext,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        trial.charge_context_constant_constructor(context, ContextConstant::One)?;
        let value = context.one();
        self.work = trial;
        Ok(value)
    }

    pub(crate) fn try_copy_authenticated(
        &mut self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        let shape = trial.coefficient_shape(value)?;
        trial.charge_coefficient_clone(&shape)?;
        let copy = value
            .try_copy_authenticated_sparse_payload()
            .map_err(
                |resource| ParametricCoefficientWorkError::SparsePayloadAllocation { resource },
            )?;
        self.work = trial;
        Ok(copy)
    }

    pub(crate) fn try_neg(
        &mut self,
        context: &ParametricCoefficientContext,
        value: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        trial.charge_unary_coefficient_operation(CoefficientOperation::Negate, value)?;
        let output = context.neg_with_limits(value, self.arithmetic.exact_algebra)?;
        self.work = trial;
        Ok(output)
    }

    pub(crate) fn try_add(
        &mut self,
        context: &ParametricCoefficientContext,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        self.try_binary(context, CoefficientOperation::Add, left, right)
    }

    pub(crate) fn try_sub(
        &mut self,
        context: &ParametricCoefficientContext,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        self.try_binary(context, CoefficientOperation::Subtract, left, right)
    }

    pub(crate) fn try_mul(
        &mut self,
        context: &ParametricCoefficientContext,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        self.try_binary(context, CoefficientOperation::Multiply, left, right)
    }

    /// Execute one unguarded field quotient for Symbolica's native sparse-row
    /// reducer adapter.
    ///
    /// This is deliberately not a rule-construction division seam.  The
    /// reducer's temporary `L`/`U` state is discarded, and the caller must
    /// replay the returned `L` transcript through the guarded provenance path
    /// before accepting any persistent row.  Keeping this operation in the
    /// work ledger nevertheless gives the native field adapter the same exact
    /// arithmetic limits and transactional counters as every other scalar
    /// operation.
    pub(crate) fn try_native_field_division(
        &mut self,
        context: &ParametricCoefficientContext,
        numerator: &ParametricCoefficient,
        denominator: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        self.try_binary(
            context,
            CoefficientOperation::Divide,
            numerator,
            denominator,
        )
    }

    fn try_binary(
        &mut self,
        context: &ParametricCoefficientContext,
        operation: CoefficientOperation,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        trial.charge_binary_coefficient_operation(operation, left, right)?;
        let output = match operation {
            CoefficientOperation::Add => {
                context.add_with_limits(left, right, self.arithmetic.exact_algebra)?
            }
            CoefficientOperation::Subtract => {
                context.sub_with_limits(left, right, self.arithmetic.exact_algebra)?
            }
            CoefficientOperation::Multiply => {
                context.mul_with_limits(left, right, self.arithmetic.exact_algebra)?
            }
            CoefficientOperation::Divide => {
                context.checked_div_with_limits(left, right, self.arithmetic.exact_algebra)?
            }
            CoefficientOperation::Negate => {
                return Err(ParametricEliminationError::InternalReplayFailure {
                    detail: "a non-binary coefficient operation reached the work facade".to_owned(),
                }
                .into());
            }
        };
        self.work = trial;
        Ok(output)
    }

    pub(crate) fn try_guarded_division_pending(
        &mut self,
        context: &ParametricCoefficientContext,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
    ) -> Result<PendingGuardedParametricDivision, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        trial.charge_guarded_division_pending_operation(dividend, divisor)?;
        let pending = context
            .checked_div_guarded_pending_normalization_with_limits_and_origin_limit(
                dividend,
                divisor,
                self.arithmetic.exact_algebra,
                self.arithmetic.max_guard_origins,
            )?;
        self.work = trial;
        Ok(pending)
    }

    pub(crate) fn try_finish_guarded_division(
        &mut self,
        context: &ParametricCoefficientContext,
        pending: PendingGuardedParametricDivision,
    ) -> Result<GuardedParametricCoefficient, ParametricCoefficientWorkError> {
        let mut trial = self.work;
        trial.charge_guarded_division_final_normalization(
            pending.value_before_final_normalization(),
        )?;
        let output = context.finish_guarded_division_normalization_with_limits_and_origin_limit(
            pending,
            self.arithmetic.exact_algebra,
            self.arithmetic.max_guard_origins,
        )?;
        self.work = trial;
        Ok(output)
    }

    /// Discover, authenticate, and attach one coefficient denominator guard
    /// with caller-supplied locator provenance. The estimator history is an
    /// operation history rather than the deduplicated guard vector: repeated
    /// coefficients remain visible because both lower insertion searches run.
    pub(crate) fn try_insert_denominator_guard(
        &mut self,
        context: &ParametricCoefficientContext,
        guards: &mut Vec<ParametricNonZeroCondition>,
        coefficient: &ParametricCoefficient,
        origin: GuardOrigin,
    ) -> Result<(), ParametricCoefficientWorkError> {
        let requested = self.guard_history.len().checked_add(1).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient guard attachment history",
            },
        )?;
        check_limit(
            "coefficient guard attachment history",
            requested,
            self.max_guard_history,
        )?;
        self.guard_history.try_reserve_exact(1).map_err(|_| {
            ParametricCoefficientWorkError::SparsePayloadAllocation {
                resource: "coefficient guard work-shape history",
            }
        })?;
        guards.try_reserve_exact(1).map_err(|_| {
            ParametricCoefficientWorkError::SparsePayloadAllocation {
                resource: "coefficient denominator guard vector",
            }
        })?;

        let mut trial = self.work;
        let shape = trial.coefficient_shape(coefficient)?;
        trial.charge_denominator_guard_from_shape(&shape, &mut self.guard_history)?;

        let result = (|| -> Result<(), ParametricCoefficientWorkError> {
            let polynomial = context
                .denominator_condition_with_limits(coefficient, self.arithmetic.exact_algebra)?;
            if polynomial.raw().is_constant() {
                return Ok(());
            }
            let condition = context.nonzero_condition_with_origins_and_origin_limit(
                polynomial,
                [origin],
                self.arithmetic.exact_algebra,
                self.arithmetic.max_guard_origins,
            )?;
            insert_parametric_condition(guards, condition, self.arithmetic.max_guard_origins)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.work = trial;
                Ok(())
            }
            Err(error) => {
                let removed = self.guard_history.pop();
                debug_assert!(removed.is_some());
                Err(error)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CoefficientOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

#[derive(Debug, Default)]
struct PolynomialWorkShape {
    terms: usize,
    variable_count: usize,
    exponent_entries: usize,
    /// Componentwise degree upper bounds in the authenticated variable order.
    /// They are exact for freshly observed polynomials, but sums, branch
    /// unions, and canonical-factor envelopes may retain conservative bounds.
    /// An empty vector is the canonical all-zero sentinel. A nonempty vector
    /// has exactly `variable_count` entries. Nontrivial vectors are built only
    /// after their metadata work is charged and use fallible exact reservation.
    degree_bounds: Vec<usize>,
    maximum_integer_bits: usize,
    total_integer_bits: usize,
}

impl PolynomialWorkShape {
    fn is_constant(&self) -> Result<bool, ParametricEliminationError> {
        validate_degree_bound_arity(self)?;
        Ok(self.degree_bounds.iter().all(|&degree| degree == 0))
    }

    /// Product of `(degree_i + 1)`, hence an upper bound on both the
    /// monomial count and the collision-free Kronecker degree plus one.
    /// Keep this derived rather than propagated: scalar box multiplication
    /// loses which variables carry degree and grows exponentially under
    /// repeated sums of polynomials with the same support.
    #[cfg(test)]
    fn monomial_box_bound(&self) -> Result<usize, ParametricEliminationError> {
        validate_degree_bound_arity(self)?;
        monomial_box_bound_from_degrees(&self.degree_bounds)
    }
}

fn monomial_box_bound_from_degrees(
    degree_bounds: &[usize],
) -> Result<usize, ParametricEliminationError> {
    degree_bounds.iter().try_fold(1usize, |bound, &degree| {
        bound
            .checked_mul(degree.checked_add(1).ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "coefficient normalization monomial box",
                },
            )?)
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient normalization monomial box",
            })
    })
}

fn charged_monomial_box_bound_from_degrees(
    work: &mut WorkBudget,
    degree_bounds: &[usize],
) -> Result<usize, ParametricEliminationError> {
    // Deriving the Kronecker box is a dense metadata traversal. Precharge the
    // complete scan so an exact one-below limit fails before inspecting a
    // private degree entry.
    work.charge_coefficient_work(0, degree_bounds.len(), 0)?;
    monomial_box_bound_from_degrees(degree_bounds)
}

#[derive(Clone, Copy, Debug, Default)]
struct PolynomialWorkLayout {
    terms: usize,
    variable_count: usize,
    exponent_entries: usize,
}

impl PolynomialWorkLayout {
    fn new(polynomial: &crate::CoefficientPolynomial) -> Self {
        Self {
            terms: polynomial.coefficients.len(),
            variable_count: polynomial.variables.len(),
            exponent_entries: polynomial.exponents.len(),
        }
    }

    fn expected_exponent_entries(self) -> Result<usize, ParametricEliminationError> {
        self.terms.checked_mul(self.variable_count).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient exponent-entry work",
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PendingPolynomialWorkShape {
    layout: PolynomialWorkLayout,
    maximum_integer_bits: usize,
    total_integer_bits: usize,
}

#[derive(Debug, Default)]
struct CoefficientWorkShape {
    numerator: PolynomialWorkShape,
    denominator: PolynomialWorkShape,
}

impl CoefficientWorkShape {
    fn total_terms(&self) -> Result<usize, ParametricEliminationError> {
        checked_count_add(
            self.numerator.terms,
            self.denominator.terms,
            "coefficient algebra work",
        )
    }

    fn total_exponent_entries(&self) -> Result<usize, ParametricEliminationError> {
        checked_count_add(
            self.numerator.exponent_entries,
            self.denominator.exponent_entries,
            "coefficient exponent-entry work",
        )
    }

    fn total_integer_bits(&self) -> Result<usize, ParametricEliminationError> {
        checked_count_add(
            self.numerator.total_integer_bits,
            self.denominator.total_integer_bits,
            "coefficient integer-bit work",
        )
    }
}

fn context_constant_coefficient_shape(
    variable_count: usize,
    constant: ContextConstant,
) -> CoefficientWorkShape {
    CoefficientWorkShape {
        numerator: match constant {
            ContextConstant::Zero => zero_polynomial_work_shape(variable_count),
            ContextConstant::One => constant_one_polynomial_shape(variable_count),
        },
        denominator: constant_one_polynomial_shape(variable_count),
    }
}

fn zero_polynomial_work_shape(variable_count: usize) -> PolynomialWorkShape {
    PolynomialWorkShape {
        terms: 0,
        variable_count,
        exponent_entries: 0,
        degree_bounds: Vec::new(),
        maximum_integer_bits: 0,
        total_integer_bits: 0,
    }
}

fn constant_one_polynomial_shape(variable_count: usize) -> PolynomialWorkShape {
    PolynomialWorkShape {
        terms: 1,
        variable_count,
        exponent_entries: variable_count,
        degree_bounds: Vec::new(),
        maximum_integer_bits: 1,
        total_integer_bits: 1,
    }
}

fn allocate_degree_bounds(
    work: &mut WorkBudget,
    variable_count: usize,
    metadata_entry_work: usize,
) -> Result<Vec<usize>, ParametricEliminationError> {
    work.charge_coefficient_work(0, metadata_entry_work, 0)?;
    let allocation_bytes = variable_count.checked_mul(size_of::<usize>()).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient degree-bound allocation bytes",
        },
    )?;
    let mut bounds = Vec::new();
    bounds.try_reserve_exact(variable_count).map_err(|_| {
        ParametricEliminationError::ResourceLimit {
            resource: "coefficient degree-bound allocation bytes",
            requested: allocation_bytes,
            limit: allocation_bytes,
        }
    })?;
    bounds.resize(variable_count, 0);
    Ok(bounds)
}

fn copy_polynomial_work_shape(
    work: &mut WorkBudget,
    source: &PolynomialWorkShape,
) -> Result<PolynomialWorkShape, ParametricEliminationError> {
    validate_degree_bound_arity(source)?;
    let degree_bounds = if source.degree_bounds.is_empty() {
        Vec::new()
    } else {
        // One zero-initialization write, then one source read and one
        // destination write for every copied degree entry.
        let metadata_entry_work = source.variable_count.checked_mul(3).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient degree-bound metadata work",
            },
        )?;
        let mut degree_bounds =
            allocate_degree_bounds(work, source.variable_count, metadata_entry_work)?;
        degree_bounds.copy_from_slice(&source.degree_bounds);
        degree_bounds
    };
    Ok(PolynomialWorkShape {
        terms: source.terms,
        variable_count: source.variable_count,
        exponent_entries: source.exponent_entries,
        degree_bounds,
        maximum_integer_bits: source.maximum_integer_bits,
        total_integer_bits: source.total_integer_bits,
    })
}

fn validate_degree_bound_arity(
    shape: &PolynomialWorkShape,
) -> Result<(), ParametricEliminationError> {
    if !shape.degree_bounds.is_empty() && shape.degree_bounds.len() != shape.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "polynomial degree-bound shape has the wrong authenticated arity".to_owned(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DegreeBoundCombination {
    Minimum,
    Maximum,
    Sum,
}

fn combine_polynomial_degree_bounds(
    work: &mut WorkBudget,
    left: &PolynomialWorkShape,
    right: &PolynomialWorkShape,
    combination: DegreeBoundCombination,
) -> Result<Vec<usize>, ParametricEliminationError> {
    if left.variable_count != right.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "polynomial degree bounds crossed authenticated variable maps".to_owned(),
        });
    }
    validate_degree_bound_arity(left)?;
    validate_degree_bound_arity(right)?;
    if matches!(combination, DegreeBoundCombination::Minimum)
        && (left.degree_bounds.is_empty() || right.degree_bounds.is_empty())
    {
        return Ok(Vec::new());
    }
    if left.degree_bounds.is_empty() || right.degree_bounds.is_empty() {
        let source = if left.degree_bounds.is_empty() {
            right
        } else {
            left
        };
        if source.degree_bounds.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(copy_polynomial_work_shape(work, source)?.degree_bounds);
    }

    if matches!(combination, DegreeBoundCombination::Sum) {
        let preflight_entry_work = left.variable_count.checked_mul(2).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient degree-bound metadata work",
            },
        )?;
        work.charge_coefficient_work(0, preflight_entry_work, 0)?;
        for (&left_degree, &right_degree) in
            left.degree_bounds.iter().zip(right.degree_bounds.iter())
        {
            left_degree.checked_add(right_degree).ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "coefficient polynomial degree bound",
                },
            )?;
        }
    }
    // The remaining phase performs one zero-initialization write, then a
    // read/read/write combine, followed by the all-zero sentinel scan.
    let metadata_entry_work = left.variable_count.checked_mul(5).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient degree-bound metadata work",
        },
    )?;
    let mut bounds = allocate_degree_bounds(work, left.variable_count, metadata_entry_work)?;
    for ((bound, &left_degree), &right_degree) in bounds
        .iter_mut()
        .zip(left.degree_bounds.iter())
        .zip(right.degree_bounds.iter())
    {
        *bound = match combination {
            DegreeBoundCombination::Minimum => left_degree.min(right_degree),
            DegreeBoundCombination::Maximum => left_degree.max(right_degree),
            DegreeBoundCombination::Sum => left_degree.checked_add(right_degree).ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "coefficient polynomial degree bound",
                },
            )?,
        };
    }
    if bounds.iter().all(|&degree| degree == 0) {
        Ok(Vec::new())
    } else {
        Ok(bounds)
    }
}

fn polynomial_shape_componentwise_upper_bound(
    work: &mut WorkBudget,
    left: &PolynomialWorkShape,
    right: &PolynomialWorkShape,
) -> Result<PolynomialWorkShape, ParametricEliminationError> {
    if left.variable_count != right.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "polynomial work bound crossed authenticated variable maps".to_owned(),
        });
    }
    let terms = left.terms.max(right.terms);
    let maximum_integer_bits = left.maximum_integer_bits.max(right.maximum_integer_bits);
    Ok(PolynomialWorkShape {
        terms,
        variable_count: left.variable_count,
        exponent_entries: terms.checked_mul(left.variable_count).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient exponent-entry work",
            },
        )?,
        degree_bounds: combine_polynomial_degree_bounds(
            work,
            left,
            right,
            DegreeBoundCombination::Maximum,
        )?,
        maximum_integer_bits,
        total_integer_bits: terms.checked_mul(maximum_integer_bits).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient integer-bit work",
            },
        )?,
    })
}

fn polynomial_common_factor_shape_bound(
    work: &mut WorkBudget,
    left: &PolynomialWorkShape,
    right: &PolynomialWorkShape,
    max_polynomial_terms: usize,
) -> Result<PolynomialWorkShape, ParametricEliminationError> {
    if left.variable_count != right.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "polynomial GCD work bound crossed authenticated variable maps".to_owned(),
        });
    }
    validate_degree_bound_arity(left)?;
    validate_degree_bound_arity(right)?;
    if left.terms == 0 && right.terms == 0 {
        return Ok(zero_polynomial_work_shape(left.variable_count));
    }
    if left.terms == 0 {
        return canonical_polynomial_factor_shape_bound(work, right, max_polynomial_terms);
    }
    if right.terms == 0 {
        return canonical_polynomial_factor_shape_bound(work, left, max_polynomial_terms);
    }
    let left_bound = canonical_polynomial_factor_shape_bound(work, left, max_polynomial_terms)?;
    let right_bound = canonical_polynomial_factor_shape_bound(work, right, max_polynomial_terms)?;
    let degree_bounds = combine_polynomial_degree_bounds(
        work,
        &left_bound,
        &right_bound,
        DegreeBoundCombination::Minimum,
    )?;
    let common_box = charged_monomial_box_bound_from_degrees(work, &degree_bounds)?;
    let terms = left_bound.terms.min(right_bound.terms).min(common_box);
    let maximum_integer_bits = left_bound
        .maximum_integer_bits
        .min(right_bound.maximum_integer_bits);
    Ok(PolynomialWorkShape {
        terms,
        variable_count: left.variable_count,
        exponent_entries: terms.checked_mul(left.variable_count).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient exponent-entry work",
            },
        )?,
        degree_bounds,
        maximum_integer_bits,
        total_integer_bits: terms.checked_mul(maximum_integer_bits).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient integer-bit work",
            },
        )?,
    })
}

#[derive(Debug, Default)]
struct CoefficientOperationEstimate {
    output: CoefficientWorkShape,
    algebra_work: usize,
    exponent_entry_work: usize,
    integer_bit_work: usize,
}

fn coefficient_operation_arithmetic_only(
    estimate: &CoefficientOperationEstimate,
    left: &CoefficientWorkShape,
    right: &CoefficientWorkShape,
) -> Result<(usize, usize, usize), ParametricEliminationError> {
    let input_terms = checked_count_add(
        left.total_terms()?,
        right.total_terms()?,
        "coefficient algebra work",
    )?;
    let input_exponents = checked_count_add(
        left.total_exponent_entries()?,
        right.total_exponent_entries()?,
        "coefficient exponent-entry work",
    )?;
    let input_bits = checked_count_add(
        left.total_integer_bits()?,
        right.total_integer_bits()?,
        "coefficient integer-bit work",
    )?;
    coefficient_operation_arithmetic_only_from_input(
        estimate,
        input_terms,
        input_exponents,
        input_bits,
    )
}

fn coefficient_operation_arithmetic_only_unary(
    estimate: &CoefficientOperationEstimate,
    input: &CoefficientWorkShape,
) -> Result<(usize, usize, usize), ParametricEliminationError> {
    coefficient_operation_arithmetic_only_from_input(
        estimate,
        input.total_terms()?,
        input.total_exponent_entries()?,
        input.total_integer_bits()?,
    )
}

fn coefficient_operation_arithmetic_only_from_input(
    estimate: &CoefficientOperationEstimate,
    input_terms: usize,
    input_exponents: usize,
    input_bits: usize,
) -> Result<(usize, usize, usize), ParametricEliminationError> {
    let subtract_io = |total: usize,
                       input: usize,
                       output: usize,
                       resource: &'static str|
     -> Result<usize, ParametricEliminationError> {
        total
            .checked_sub(input)
            .and_then(|remaining| remaining.checked_sub(output))
            .ok_or_else(|| ParametricEliminationError::InternalReplayFailure {
                detail: format!("{resource} estimate is smaller than its input/output census"),
            })
    };
    Ok((
        subtract_io(
            estimate.algebra_work,
            input_terms,
            estimate.output.total_terms()?,
            "coefficient algebra work",
        )?,
        subtract_io(
            estimate.exponent_entry_work,
            input_exponents,
            estimate.output.total_exponent_entries()?,
            "coefficient exponent-entry work",
        )?,
        subtract_io(
            estimate.integer_bit_work,
            input_bits,
            estimate.output.total_integer_bits()?,
            "coefficient integer-bit work",
        )?,
    ))
}

fn complete_polynomial_shape(
    work: &mut WorkBudget,
    polynomial: &crate::CoefficientPolynomial,
    pending: PendingPolynomialWorkShape,
) -> Result<PolynomialWorkShape, ParametricEliminationError> {
    let expected = pending.layout.expected_exponent_entries()?;
    if polynomial.exponents.len() != expected {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: format!(
                "authenticated polynomial shape has {} terms, {} exponents, and {} variables",
                pending.layout.terms,
                polynomial.exponents.len(),
                pending.layout.variable_count
            ),
        });
    }
    // Discover whether the all-zero sentinel suffices before allocating a
    // dense bound vector. This scan and the possible fill scan are charged
    // independently and before the corresponding work begins.
    work.charge_coefficient_work(0, pending.layout.exponent_entries, 0)?;
    if polynomial.exponents.iter().all(|&exponent| exponent == 0) {
        return Ok(PolynomialWorkShape {
            terms: pending.layout.terms,
            variable_count: pending.layout.variable_count,
            exponent_entries: pending.layout.exponent_entries,
            degree_bounds: Vec::new(),
            maximum_integer_bits: pending.maximum_integer_bits,
            total_integer_bits: pending.total_integer_bits,
        });
    }
    let fill_entry_work = pending.layout.exponent_entries.checked_mul(3).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient degree-bound metadata work",
        },
    )?;
    let metadata_entry_work = checked_count_add(
        fill_entry_work,
        pending.layout.variable_count,
        "coefficient degree-bound metadata work",
    )?;
    let mut degree_bounds =
        allocate_degree_bounds(work, pending.layout.variable_count, metadata_entry_work)?;
    for exponents in polynomial
        .exponents
        .chunks_exact(pending.layout.variable_count)
    {
        for (bound, &exponent) in degree_bounds.iter_mut().zip(exponents) {
            *bound = (*bound).max(usize::from(exponent));
        }
    }
    Ok(PolynomialWorkShape {
        terms: pending.layout.terms,
        variable_count: pending.layout.variable_count,
        exponent_entries: pending.layout.exponent_entries,
        degree_bounds,
        maximum_integer_bits: pending.maximum_integer_bits,
        total_integer_bits: pending.total_integer_bits,
    })
}

fn coefficient_final_normalization_estimate_from_shape(
    work: &mut WorkBudget,
    shape: &CoefficientWorkShape,
    max_polynomial_terms: usize,
) -> Result<CoefficientOperationEstimate, ParametricEliminationError> {
    if shape.numerator.variable_count != shape.denominator.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "normalization input crossed authenticated variable maps".to_owned(),
        });
    }
    let term_pairs = shape
        .numerator
        .terms
        .checked_mul(shape.denominator.terms)
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient algebra work",
        })?;
    let pair_exponents = term_pairs
        .checked_mul(shape.numerator.variable_count)
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        })?;
    let pair_integer_bits = term_pairs
        .checked_mul(checked_count_add(
            shape.numerator.maximum_integer_bits,
            shape.denominator.maximum_integer_bits,
            "coefficient integer-bit work",
        )?)
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient integer-bit work",
        })?;
    // A second normalization can expose a dense quotient even when the
    // pending numerator/denominator are sparse (for example division by a
    // sparse polynomial factor). Bound its accepted canonical output with the
    // same Kronecker/Mignotte envelope used by ordinary field operations.
    let output = canonical_coefficient_shape_bound(work, shape, max_polynomial_terms)?;
    let output_terms = output.total_terms()?;
    let output_exponent_entries = output.total_exponent_entries()?;
    let output_integer_bits = output.total_integer_bits()?;
    Ok(CoefficientOperationEstimate {
        output,
        algebra_work: checked_count_add(
            checked_count_add(shape.total_terms()?, term_pairs, "coefficient algebra work")?,
            output_terms,
            "coefficient algebra work",
        )?,
        exponent_entry_work: checked_count_add(
            checked_count_add(
                shape.total_exponent_entries()?,
                pair_exponents,
                "coefficient exponent-entry work",
            )?,
            output_exponent_entries,
            "coefficient exponent-entry work",
        )?,
        integer_bit_work: checked_count_add(
            checked_count_add(
                shape.total_integer_bits()?,
                pair_integer_bits,
                "coefficient integer-bit work",
            )?,
            output_integer_bits,
            "coefficient integer-bit work",
        )?,
    })
}

fn coefficient_operation_estimate_from_shapes(
    work: &mut WorkBudget,
    operation: CoefficientOperation,
    left: &CoefficientWorkShape,
    right: &CoefficientWorkShape,
    equal_denominator: bool,
    max_polynomial_terms: usize,
) -> Result<CoefficientOperationEstimate, ParametricEliminationError> {
    if matches!(operation, CoefficientOperation::Negate) {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "a unary coefficient operation was sent to the binary work preflight"
                .to_owned(),
        });
    }
    let input_terms = checked_count_add(
        left.total_terms()?,
        right.total_terms()?,
        "coefficient algebra work",
    )?;
    let input_exponent_entries = checked_count_add(
        left.total_exponent_entries()?,
        right.total_exponent_entries()?,
        "coefficient exponent-entry work",
    )?;
    let input_bits = checked_count_add(
        left.total_integer_bits()?,
        right.total_integer_bits()?,
        "coefficient integer-bit work",
    )?;

    let (output, arithmetic_terms, arithmetic_exponents, arithmetic_bits) = match operation {
        CoefficientOperation::Multiply => {
            let numerator = polynomial_product_work(work, &left.numerator, &right.numerator)?;
            let denominator = polynomial_product_work(work, &left.denominator, &right.denominator)?;
            (
                CoefficientWorkShape {
                    numerator: numerator.output,
                    denominator: denominator.output,
                },
                checked_count_add(
                    numerator.algebra_work,
                    denominator.algebra_work,
                    "coefficient algebra work",
                )?,
                checked_count_add(
                    numerator.exponent_entry_work,
                    denominator.exponent_entry_work,
                    "coefficient exponent-entry work",
                )?,
                checked_count_add(
                    numerator.integer_bit_work,
                    denominator.integer_bit_work,
                    "coefficient integer-bit work",
                )?,
            )
        }
        CoefficientOperation::Divide => {
            let numerator = polynomial_product_work(work, &left.numerator, &right.denominator)?;
            let denominator = polynomial_product_work(work, &left.denominator, &right.numerator)?;
            (
                CoefficientWorkShape {
                    numerator: numerator.output,
                    denominator: denominator.output,
                },
                checked_count_add(
                    numerator.algebra_work,
                    denominator.algebra_work,
                    "coefficient algebra work",
                )?,
                checked_count_add(
                    numerator.exponent_entry_work,
                    denominator.exponent_entry_work,
                    "coefficient exponent-entry work",
                )?,
                checked_count_add(
                    numerator.integer_bit_work,
                    denominator.integer_bit_work,
                    "coefficient integer-bit work",
                )?,
            )
        }
        CoefficientOperation::Add | CoefficientOperation::Subtract if equal_denominator => {
            let numerator = polynomial_sum_work(work, &left.numerator, &right.numerator)?;
            (
                CoefficientWorkShape {
                    numerator: numerator.output,
                    denominator: copy_polynomial_work_shape(work, &left.denominator)?,
                },
                numerator.algebra_work,
                numerator.exponent_entry_work,
                numerator.integer_bit_work,
            )
        }
        CoefficientOperation::Add | CoefficientOperation::Subtract => {
            let left_cross = polynomial_product_work(work, &left.numerator, &right.denominator)?;
            let right_cross = polynomial_product_work(work, &right.numerator, &left.denominator)?;
            let numerator = polynomial_sum_work(work, &left_cross.output, &right_cross.output)?;
            let denominator = polynomial_product_work(work, &left.denominator, &right.denominator)?;
            let algebra = checked_count_add(
                checked_count_add(
                    left_cross.algebra_work,
                    right_cross.algebra_work,
                    "coefficient algebra work",
                )?,
                checked_count_add(
                    numerator.algebra_work,
                    denominator.algebra_work,
                    "coefficient algebra work",
                )?,
                "coefficient algebra work",
            )?;
            let bits = checked_count_add(
                checked_count_add(
                    left_cross.integer_bit_work,
                    right_cross.integer_bit_work,
                    "coefficient integer-bit work",
                )?,
                checked_count_add(
                    numerator.integer_bit_work,
                    denominator.integer_bit_work,
                    "coefficient integer-bit work",
                )?,
                "coefficient integer-bit work",
            )?;
            let exponents = checked_count_add(
                checked_count_add(
                    left_cross.exponent_entry_work,
                    right_cross.exponent_entry_work,
                    "coefficient exponent-entry work",
                )?,
                checked_count_add(
                    numerator.exponent_entry_work,
                    denominator.exponent_entry_work,
                    "coefficient exponent-entry work",
                )?,
                "coefficient exponent-entry work",
            )?;
            (
                CoefficientWorkShape {
                    numerator: numerator.output,
                    denominator: denominator.output,
                },
                algebra,
                exponents,
                bits,
            )
        }
        CoefficientOperation::Negate => unreachable!(),
    };
    let output = canonical_coefficient_shape_bound(work, &output, max_polynomial_terms)?;
    let output_terms = output.total_terms()?;
    let output_bits = output.total_integer_bits()?;
    let output_exponent_entries = output.total_exponent_entries()?;
    Ok(CoefficientOperationEstimate {
        output,
        algebra_work: checked_count_add(
            checked_count_add(input_terms, arithmetic_terms, "coefficient algebra work")?,
            output_terms,
            "coefficient algebra work",
        )?,
        exponent_entry_work: checked_count_add(
            checked_count_add(
                input_exponent_entries,
                arithmetic_exponents,
                "coefficient exponent-entry work",
            )?,
            output_exponent_entries,
            "coefficient exponent-entry work",
        )?,
        integer_bit_work: checked_count_add(
            checked_count_add(input_bits, arithmetic_bits, "coefficient integer-bit work")?,
            output_bits,
            "coefficient integer-bit work",
        )?,
    })
}

/// Bound the visible canonical quotient after a native polynomial GCD.
///
/// A quotient can be much denser than its sparse dividend. Map each
/// multivariate polynomial collision-free into one Kronecker variable using
/// the per-variable degree box. Every normalized numerator/denominator is an
/// integer factor of its raw counterpart, so Mignotte's factor-height bound
/// gives `bits(factor coefficient) <= bits(raw height) + degree + log2(terms)`.
/// The exact-algebra term cap bounds RustRed's acceptance and retention of the
/// returned sparse vectors immediately after the native call. It does not
/// bound allocations made inside Symbolica while computing that quotient.
fn canonical_coefficient_shape_bound(
    work: &mut WorkBudget,
    raw: &CoefficientWorkShape,
    max_polynomial_terms: usize,
) -> Result<CoefficientWorkShape, ParametricEliminationError> {
    Ok(CoefficientWorkShape {
        numerator: canonical_polynomial_factor_shape_bound(
            work,
            &raw.numerator,
            max_polynomial_terms,
        )?,
        denominator: canonical_polynomial_factor_shape_bound(
            work,
            &raw.denominator,
            max_polynomial_terms,
        )?,
    })
}

fn canonical_polynomial_factor_shape_bound(
    work: &mut WorkBudget,
    raw: &PolynomialWorkShape,
    max_polynomial_terms: usize,
) -> Result<PolynomialWorkShape, ParametricEliminationError> {
    validate_degree_bound_arity(raw)?;
    if raw.terms == 0 {
        return Ok(zero_polynomial_work_shape(raw.variable_count));
    }
    let monomial_box_bound = charged_monomial_box_bound_from_degrees(work, &raw.degree_bounds)?;
    let terms = monomial_box_bound.min(max_polynomial_terms);
    let exponent_entries = terms.checked_mul(raw.variable_count).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        },
    )?;
    let kronecker_degree = monomial_box_bound.checked_sub(1).ok_or(
        ParametricEliminationError::InternalReplayFailure {
            detail: "nonzero polynomial has an empty Kronecker monomial box".to_owned(),
        },
    )?;
    let maximum_integer_bits = checked_count_add(
        checked_count_add(
            raw.maximum_integer_bits,
            kronecker_degree,
            "coefficient integer-bit work",
        )?,
        checked_count_add(
            ceil_log2_usize(raw.terms.max(1)),
            1,
            "coefficient integer-bit work",
        )?,
        "coefficient integer-bit work",
    )?;
    let total_integer_bits = terms.checked_mul(maximum_integer_bits).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient integer-bit work",
        },
    )?;
    Ok(PolynomialWorkShape {
        terms,
        variable_count: raw.variable_count,
        exponent_entries,
        degree_bounds: copy_polynomial_work_shape(work, raw)?.degree_bounds,
        maximum_integer_bits,
        total_integer_bits,
    })
}

#[derive(Debug, Default)]
struct PolynomialOperationEstimate {
    output: PolynomialWorkShape,
    algebra_work: usize,
    exponent_entry_work: usize,
    integer_bit_work: usize,
}

fn polynomial_product_work(
    work: &mut WorkBudget,
    left: &PolynomialWorkShape,
    right: &PolynomialWorkShape,
) -> Result<PolynomialOperationEstimate, ParametricEliminationError> {
    if left.variable_count != right.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "coefficient operation crossed authenticated variable maps".to_owned(),
        });
    }
    validate_degree_bound_arity(left)?;
    validate_degree_bound_arity(right)?;
    if left.terms == 0 || right.terms == 0 {
        return Ok(PolynomialOperationEstimate {
            output: zero_polynomial_work_shape(left.variable_count),
            ..PolynomialOperationEstimate::default()
        });
    }
    let term_pairs = left.terms.checked_mul(right.terms).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient algebra work",
        },
    )?;
    let collision_bits = ceil_log2_usize(term_pairs.max(1));
    let maximum_integer_bits = checked_count_add(
        checked_count_add(
            left.maximum_integer_bits,
            right.maximum_integer_bits,
            "coefficient integer-bit work",
        )?,
        collision_bits,
        "coefficient integer-bit work",
    )?;
    let arithmetic_exponent_entries = term_pairs.checked_mul(left.variable_count).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        },
    )?;
    let degree_bounds =
        combine_polynomial_degree_bounds(work, left, right, DegreeBoundCombination::Sum)?;
    let terms = term_pairs.min(charged_monomial_box_bound_from_degrees(
        work,
        &degree_bounds,
    )?);
    let total_integer_bits = terms.checked_mul(maximum_integer_bits).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient integer-bit work",
        },
    )?;
    let exponent_entries = terms.checked_mul(left.variable_count).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        },
    )?;
    // One coefficient multiplication and one conservative collection charge
    // per raw term pair.
    let integer_bit_work = checked_count_add(
        total_integer_bits,
        term_pairs
            .checked_mul(checked_count_add(
                left.maximum_integer_bits,
                right.maximum_integer_bits,
                "coefficient integer-bit work",
            )?)
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient integer-bit work",
            })?,
        "coefficient integer-bit work",
    )?;
    Ok(PolynomialOperationEstimate {
        output: PolynomialWorkShape {
            terms,
            variable_count: left.variable_count,
            exponent_entries,
            degree_bounds,
            maximum_integer_bits,
            total_integer_bits,
        },
        algebra_work: term_pairs,
        // Every raw term pair constructs or compares one dense exponent row.
        exponent_entry_work: arithmetic_exponent_entries,
        integer_bit_work,
    })
}

fn polynomial_sum_work(
    work: &mut WorkBudget,
    left: &PolynomialWorkShape,
    right: &PolynomialWorkShape,
) -> Result<PolynomialOperationEstimate, ParametricEliminationError> {
    if left.variable_count != right.variable_count {
        return Err(ParametricEliminationError::InternalReplayFailure {
            detail: "coefficient operation crossed authenticated variable maps".to_owned(),
        });
    }
    let term_visits = checked_count_add(left.terms, right.terms, "coefficient algebra work")?;
    let degree_bounds =
        combine_polynomial_degree_bounds(work, left, right, DegreeBoundCombination::Maximum)?;
    let terms = term_visits.min(charged_monomial_box_bound_from_degrees(
        work,
        &degree_bounds,
    )?);
    let exponent_entries = terms.checked_mul(left.variable_count).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        },
    )?;
    let arithmetic_exponent_entries = term_visits.checked_mul(left.variable_count).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient exponent-entry work",
        },
    )?;
    let maximum_integer_bits = checked_count_add(
        left.maximum_integer_bits.max(right.maximum_integer_bits),
        checked_count_add(
            ceil_log2_usize(term_visits.max(1)),
            1,
            "coefficient integer-bit work",
        )?,
        "coefficient integer-bit work",
    )?;
    let total_integer_bits = terms.checked_mul(maximum_integer_bits).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "coefficient integer-bit work",
        },
    )?;
    Ok(PolynomialOperationEstimate {
        output: PolynomialWorkShape {
            terms,
            variable_count: left.variable_count,
            exponent_entries,
            degree_bounds,
            maximum_integer_bits,
            total_integer_bits,
        },
        algebra_work: term_visits,
        exponent_entry_work: arithmetic_exponent_entries,
        integer_bit_work: term_visits.checked_mul(maximum_integer_bits).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient integer-bit work",
            },
        )?,
    })
}

fn ceil_log2_usize(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn integer_magnitude_bits_usize(value: &Integer) -> Result<usize, ParametricEliminationError> {
    let bits = match value {
        Integer::Single(value) => i64::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Double(value) => i128::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Large(value) => value.significant_bits(),
    };
    usize::try_from(bits).map_err(|_| ParametricEliminationError::ResourceCountOverflow {
        resource: "coefficient integer-bit work",
    })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricEliminationError> {
    if requested > limit {
        Err(ParametricEliminationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_count_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ParametricEliminationError> {
    left.checked_add(right)
        .ok_or(ParametricEliminationError::ResourceCountOverflow { resource })
}

fn guard_origin_count(relation: &ParametricRelation) -> Result<usize, ParametricEliminationError> {
    relation
        .guarded_nonzero_conditions()
        .iter()
        .try_fold(0usize, |count, condition| {
            checked_count_add(count, condition.origins().len(), "parametric guard origins")
        })
}

/// Conservative payload created when an empty target relation is assigned
/// `factor * relation` through the guarded relation adapter.  The adapter
/// records one attachment origin per inherited condition and may add a
/// two-origin scale-factor denominator condition.  Bounding this before the
/// adapter call prevents an untrusted guard manifest from causing an
/// unbounded transactional clone/allocation.
fn guarded_normalization_payload_upper_bound(
    relation: &ParametricRelation,
    factor: &GuardedParametricCoefficient,
) -> Result<(usize, usize), ParametricEliminationError> {
    let inherited_guards = relation.guarded_nonzero_conditions().len();
    let factor_guards = factor.nonzero.len();
    let guards = checked_count_add(
        checked_count_add(
            inherited_guards,
            factor_guards,
            "prospective normalized pivot guards",
        )?,
        1,
        "prospective normalized pivot guards",
    )?;
    // `add_term_in_place` discovers one denominator condition for every
    // scaled source term, even before testing whether its numerator is zero.
    let guards = checked_count_add(
        guards,
        relation.terms().len(),
        "prospective normalized pivot guards",
    )?;
    let factor_origins = factor.nonzero.iter().try_fold(0usize, |count, condition| {
        checked_count_add(
            count,
            condition.origins().len(),
            "prospective normalized pivot guard origins",
        )
    })?;
    let origins = checked_count_add(
        guard_origin_count(relation)?,
        factor_origins,
        "prospective normalized pivot guard origins",
    )?;
    let origins = checked_count_add(
        origins,
        inherited_guards,
        "prospective normalized pivot guard origins",
    )?;
    let origins = checked_count_add(
        origins,
        factor_guards,
        "prospective normalized pivot guard origins",
    )?;
    let origins = checked_count_add(origins, 2, "prospective normalized pivot guard origins")?;
    let term_origins = relation.terms().len().checked_mul(2).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "prospective normalized pivot guard origins",
        },
    )?;
    let origins = checked_count_add(
        origins,
        term_origins,
        "prospective normalized pivot guard origins",
    )?;
    Ok((guards, origins))
}

fn elimination_base_retained_byte_bound(
    wrapper_retained_bytes: usize,
    context: &ParametricCoefficientContext,
    family_fingerprint: &str,
    source_manifest_bytes: usize,
    columns_easiest_first: &Vec<IndexShift>,
) -> Result<usize, ParametricEliminationError> {
    let mut bytes = wrapper_retained_bytes;
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(family_fingerprint.len())?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(context.fingerprint().len())?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(source_manifest_bytes)?,
        "retained parametric elimination bytes",
    )?;
    checked_count_add(
        bytes,
        shift_vec_retained_byte_bound(columns_easiest_first)?,
        "retained parametric elimination bytes",
    )
}

fn pivot_retained_byte_bound(
    pivot: &ParametricPivotEquation,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    let mut bytes = size_of::<ParametricPivotEquation>();
    bytes = checked_count_add(
        bytes,
        pivot.pivot.owned_retained_byte_bound().ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "retained parametric elimination bytes",
            },
        )?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        relation_certificate_retained_byte_bound(&pivot.unit_relation, work)?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        pivot
            .trace
            .reductions
            .capacity()
            .checked_mul(size_of::<ParametricEliminationReduction>())
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "retained parametric elimination bytes",
            })?,
        "retained parametric elimination bytes",
    )?;
    for reduction in &pivot.trace.reductions {
        bytes = checked_count_add(
            bytes,
            coefficient_deep_retained_byte_bound(&reduction.factor, work)?,
            "retained parametric elimination bytes",
        )?;
    }
    checked_count_add(
        bytes,
        coefficient_deep_retained_byte_bound(&pivot.trace.divisor, work)?,
        "retained parametric elimination bytes",
    )
}

/// Deterministic conservative envelope checked before any complete normalized
/// pivot relation, pivot shift, divisor, or trace payload is retained.
///
/// Coefficient products use their raw sparse-product shapes. As documented at
/// module level, Symbolica's native canonicalization/GCD workspace is opaque;
/// the canonical result is checked against this envelope immediately after
/// construction and cannot be published if it exceeds it.
fn prospective_normalized_pivot_retained_byte_bound(
    source: &ParametricRelation,
    pivot: &IndexShift,
    reduced: &ParametricRelation,
    inverse: &GuardedParametricCoefficient,
    reductions: &Vec<ParametricEliminationReduction>,
    divisor: &ParametricCoefficient,
    ordinal: usize,
    max_polynomial_terms: usize,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    // The pivot Vec's complete possible slot buffer is charged before the
    // loop, so this envelope deliberately excludes the equation's inline
    // slot and includes only its deep payload.
    let mut bytes = pivot.owned_retained_byte_bound().ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        },
    )?;
    bytes = checked_count_add(
        bytes,
        reductions
            .capacity()
            .checked_mul(size_of::<ParametricEliminationReduction>())
            .ok_or(ParametricEliminationError::ResourceCountOverflow {
                resource: "retained parametric elimination bytes",
            })?,
        "retained parametric elimination bytes",
    )?;
    for reduction in reductions {
        bytes = checked_count_add(
            bytes,
            coefficient_deep_retained_byte_bound(&reduction.factor, work)?,
            "retained parametric elimination bytes",
        )?;
    }
    bytes = checked_count_add(
        bytes,
        coefficient_deep_retained_byte_bound(divisor, work)?,
        "retained parametric elimination bytes",
    )?;
    checked_count_add(
        bytes,
        prospective_normalized_relation_retained_byte_bound(
            source,
            reduced,
            inverse,
            ordinal,
            max_polynomial_terms,
            work,
        )?,
        "retained parametric elimination bytes",
    )
}

fn prospective_normalized_relation_retained_byte_bound(
    source: &ParametricRelation,
    reduced: &ParametricRelation,
    inverse: &GuardedParametricCoefficient,
    ordinal: usize,
    max_polynomial_terms: usize,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    let row_id = pivot_row_id(ordinal);
    let target_row = row_id.guard_identity();
    let source_row = source.row_id().guard_identity();
    let mut bytes = size_of::<ParametricRelation>();
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(source.family_fingerprint().len())?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(source.context_fingerprint().len())?,
        "retained parametric elimination bytes",
    )?;
    if let ParametricRowId::Derived { label } = &row_id {
        bytes = checked_count_add(
            bytes,
            arc_str_allocation_bound(label.len())?,
            "retained parametric elimination bytes",
        )?;
    }

    let btree_node_bound = size_of::<(IndexShift, ParametricCoefficient)>()
        .checked_mul(16)
        .and_then(|value| value.checked_add(32usize.checked_mul(size_of::<usize>())?))
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
    let inverse_shape = work.coefficient_shape(&inverse.value)?;
    for (shift, coefficient) in reduced.terms() {
        let coefficient_shape = work.coefficient_shape(coefficient)?;
        let output = coefficient_operation_estimate_from_shapes(
            work,
            CoefficientOperation::Multiply,
            &coefficient_shape,
            &inverse_shape,
            false,
            max_polynomial_terms,
        )?
        .output;
        bytes = checked_count_add(
            bytes,
            btree_node_bound,
            "retained parametric elimination bytes",
        )?;
        bytes = checked_count_add(
            bytes,
            shift.owned_retained_byte_bound().ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                },
            )?,
            "retained parametric elimination bytes",
        )?;
        bytes = checked_count_add(
            bytes,
            coefficient_shape_retained_byte_bound(&output)?,
            "retained parametric elimination bytes",
        )?;
    }

    let attached_origin_bytes = GuardOrigin::relation_attached_retained_byte_bound(&target_row)
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
    for condition in reduced
        .guarded_nonzero_conditions()
        .iter()
        .chain(inverse.nonzero.iter())
    {
        work.charge_polynomial_retained_bound_traversal(condition.polynomial())?;
        bytes = checked_count_add(
            bytes,
            condition.owned_retained_byte_bound().ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                },
            )?,
            "retained parametric elimination bytes",
        )?;
        work.charge_polynomial_retained_bound_traversal(condition.polynomial())?;
        bytes = checked_count_add(
            bytes,
            condition.polynomial().owned_retained_byte_bound().ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                },
            )?,
            "retained parametric elimination bytes",
        )?;
        bytes = checked_count_add(
            bytes,
            attached_origin_bytes,
            "retained parametric elimination bytes",
        )?;
    }

    let scale_origin = GuardOrigin::RelationScaleFactorDenominator {
        target_row: target_row.clone(),
        source_row,
    };
    let scale_origin_bytes = scale_origin.retained_byte_bound().ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        },
    )?;
    bytes = checked_count_add(
        bytes,
        prospective_polynomial_condition_retained_byte_bound(
            &inverse_shape.denominator,
            checked_count_add(
                scale_origin_bytes,
                attached_origin_bytes,
                "retained parametric elimination bytes",
            )?,
        )?,
        "retained parametric elimination bytes",
    )?;
    for (shift, coefficient) in reduced.terms() {
        let coefficient_shape = work.coefficient_shape(coefficient)?;
        let denominator = coefficient_operation_estimate_from_shapes(
            work,
            CoefficientOperation::Multiply,
            &coefficient_shape,
            &inverse_shape,
            false,
            max_polynomial_terms,
        )?
        .output
        .denominator;
        let input_origin = GuardOrigin::relation_input_term_denominator_retained_byte_bound(
            &target_row,
            shift.arity(),
        )
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
        bytes = checked_count_add(
            bytes,
            prospective_polynomial_condition_retained_byte_bound(
                &denominator,
                checked_count_add(
                    input_origin,
                    attached_origin_bytes,
                    "retained parametric elimination bytes",
                )?,
            )?,
            "retained parametric elimination bytes",
        )?;
    }
    // Symbolica and libstd may retain spare capacity beyond exact sparse
    // lengths. Deterministic slack keeps this an allocation envelope without
    // claiming physical allocator-byte exactness.
    bytes
        .checked_mul(4)
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })
}

fn prospective_polynomial_condition_retained_byte_bound(
    polynomial: &PolynomialWorkShape,
    origin_bytes: usize,
) -> Result<usize, ParametricEliminationError> {
    let polynomial_bytes = polynomial_shape_retained_byte_bound(polynomial)?;
    let mut bytes = size_of::<ParametricNonZeroCondition>();
    bytes = checked_count_add(
        bytes,
        polynomial_bytes,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(bytes, origin_bytes, "retained parametric elimination bytes")?;
    bytes = checked_count_add(
        bytes,
        size_of::<ParametricPolynomial>(),
        "retained parametric elimination bytes",
    )?;
    checked_count_add(
        bytes,
        polynomial_bytes,
        "retained parametric elimination bytes",
    )
}

fn coefficient_shape_retained_byte_bound(
    shape: &CoefficientWorkShape,
) -> Result<usize, ParametricEliminationError> {
    checked_count_add(
        size_of::<ParametricCoefficient>(),
        checked_count_add(
            polynomial_shape_retained_byte_bound(&shape.numerator)?,
            polynomial_shape_retained_byte_bound(&shape.denominator)?,
            "retained parametric elimination bytes",
        )?,
        "retained parametric elimination bytes",
    )
}

fn polynomial_shape_retained_byte_bound(
    shape: &PolynomialWorkShape,
) -> Result<usize, ParametricEliminationError> {
    let coefficient_slots = shape.terms.checked_mul(size_of::<Integer>()).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        },
    )?;
    let exponent_bytes = shape.exponent_entries.checked_mul(size_of::<u16>()).ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        },
    )?;
    let integer_payload = shape
        .total_integer_bits
        .checked_add(7)
        .and_then(|bits| bits.checked_div(8))
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
    checked_count_add(
        checked_count_add(
            coefficient_slots,
            exponent_bytes,
            "retained parametric elimination bytes",
        )?,
        integer_payload,
        "retained parametric elimination bytes",
    )
}

fn relation_certificate_retained_byte_bound(
    relation: &ParametricRelation,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    work.charge_relation_retained_bound_traversal(relation)?;
    let mut bytes = relation.owned_retained_byte_bound().ok_or(
        ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        },
    )?;
    // `ParametricRelation::owned_retained_byte_bound` deliberately treats
    // identity Arcs as external sharing seams. Unit pivots create fresh Arcs,
    // so the certificate must charge their allocations explicitly.
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(relation.family_fingerprint().len())?,
        "retained parametric elimination bytes",
    )?;
    bytes = checked_count_add(
        bytes,
        arc_str_allocation_bound(relation.context_fingerprint().len())?,
        "retained parametric elimination bytes",
    )?;
    if let ParametricRowId::Derived { label } = relation.row_id() {
        bytes = checked_count_add(
            bytes,
            arc_str_allocation_bound(label.len())?,
            "retained parametric elimination bytes",
        )?;
    }
    Ok(bytes)
}

fn coefficient_deep_retained_byte_bound(
    coefficient: &ParametricCoefficient,
    work: &mut WorkBudget,
) -> Result<usize, ParametricEliminationError> {
    let shape = work.coefficient_shape(coefficient)?;
    work.charge_coefficient_shape_envelope(&shape)?;
    coefficient
        .owned_retained_byte_bound()
        .and_then(|bytes| bytes.checked_sub(size_of::<ParametricCoefficient>()))
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })
}

fn shift_vec_retained_byte_bound(
    shifts: &Vec<IndexShift>,
) -> Result<usize, ParametricEliminationError> {
    let mut bytes = shifts
        .capacity()
        .checked_mul(size_of::<IndexShift>())
        .ok_or(ParametricEliminationError::ResourceCountOverflow {
            resource: "retained parametric elimination bytes",
        })?;
    for shift in shifts {
        bytes = checked_count_add(
            bytes,
            shift.owned_retained_byte_bound().ok_or(
                ParametricEliminationError::ResourceCountOverflow {
                    resource: "retained parametric elimination bytes",
                },
            )?,
            "retained parametric elimination bytes",
        )?;
    }
    Ok(bytes)
}

fn arc_str_allocation_bound(length: usize) -> Result<usize, ParametricEliminationError> {
    // Two strong/weak counters plus one word of conservative alignment/header
    // slack precede the dynamically sized `str` payload.
    checked_count_add(
        length,
        3usize.checked_mul(size_of::<usize>()).ok_or(
            ParametricEliminationError::ResourceCountOverflow {
                resource: "retained parametric elimination bytes",
            },
        )?,
        "retained parametric elimination bytes",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GuardOrigin, algebra::CoefficientContext, algebra::ExactAlgebraError,
        algebra::ExactAlgebraLimits,
    };

    fn synthetic_context() -> (CoefficientContext, ParametricCoefficientContext) {
        let base = CoefficientContext::new(["d"]);
        let parametric =
            ParametricCoefficientContext::try_new(&base, "parametric-elimination-tests", 1)
                .unwrap();
        (base, parametric)
    }

    fn recurrence(
        context: &ParametricCoefficientContext,
        family: &str,
        label: &str,
    ) -> ParametricRelation {
        let space = IndexSpace::try_new(1).unwrap();
        let mut row = ParametricRelation::new(
            family,
            ParametricRowId::Derived {
                label: Arc::from(label),
            },
            context,
        );
        // n J(n+1) - J(n) = 0.
        row.add_term(
            context,
            space.unit(0, 1).unwrap(),
            context.index(0).unwrap(),
        )
        .unwrap();
        row.add_term(context, space.zero(), context.integer(-1))
            .unwrap();
        row
    }

    fn two_row_system(
        context: &ParametricCoefficientContext,
    ) -> (ParametricRelation, ParametricRelation) {
        let space = IndexSpace::try_new(1).unwrap();
        let plus = space.unit(0, 1).unwrap();
        let zero = space.zero();
        let minus = space.unit(0, -1).unwrap();
        let n = context.index(0).unwrap();

        let mut first = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("resource-first"),
            },
            context,
        );
        first.add_term(context, plus.clone(), n).unwrap();
        first.add_term(context, zero, context.integer(-1)).unwrap();

        let mut second = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("resource-second"),
            },
            context,
        );
        second.add_term(context, plus, context.one()).unwrap();
        second.add_term(context, minus, context.one()).unwrap();
        (first, second)
    }

    fn resource_fixture(
        context: &ParametricCoefficientContext,
        limits: ParametricEliminationLimits,
    ) -> Result<ParametricElimination, ParametricEliminationError> {
        let (first, second) = two_row_system(context);
        ParametricElimination::build(
            context,
            &[first, second],
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            limits,
        )
    }

    fn coefficient_power(
        context: &ParametricCoefficientContext,
        base: &ParametricCoefficient,
        exponent: usize,
    ) -> ParametricCoefficient {
        let mut output = context.one();
        for _ in 0..exponent {
            output = context.mul(&output, base).unwrap();
        }
        output
    }

    fn test_polynomial_work_shape(
        terms: usize,
        degree_bounds: &[usize],
        maximum_integer_bits: usize,
    ) -> PolynomialWorkShape {
        let variable_count = degree_bounds.len();
        PolynomialWorkShape {
            terms,
            variable_count,
            exponent_entries: terms.checked_mul(variable_count).unwrap(),
            degree_bounds: if degree_bounds.iter().all(|&degree| degree == 0) {
                Vec::new()
            } else {
                degree_bounds.to_vec()
            },
            maximum_integer_bits,
            total_integer_bits: terms.checked_mul(maximum_integer_bits).unwrap(),
        }
    }

    fn assert_polynomial_shape_fits(actual: &PolynomialWorkShape, envelope: &PolynomialWorkShape) {
        validate_degree_bound_arity(actual).unwrap();
        validate_degree_bound_arity(envelope).unwrap();
        assert_eq!(actual.variable_count, envelope.variable_count);
        for variable in 0..actual.variable_count {
            let actual_degree = actual.degree_bounds.get(variable).copied().unwrap_or(0);
            let envelope_degree = envelope.degree_bounds.get(variable).copied().unwrap_or(0);
            assert!(actual_degree <= envelope_degree);
        }
        assert!(actual.terms <= envelope.terms);
        assert!(actual.exponent_entries <= envelope.exponent_entries);
        assert!(actual.maximum_integer_bits <= envelope.maximum_integer_bits);
        assert!(actual.total_integer_bits <= envelope.total_integer_bits);
    }

    fn assert_shape_fits(actual: &CoefficientWorkShape, envelope: &CoefficientWorkShape) {
        assert_polynomial_shape_fits(&actual.numerator, &envelope.numerator);
        assert_polynomial_shape_fits(&actual.denominator, &envelope.denominator);
    }

    fn coefficient_work_counters(work: &WorkBudget) -> (usize, usize, usize) {
        (
            work.coefficient_algebra_work,
            work.coefficient_exponent_entry_work,
            work.coefficient_integer_bit_work,
        )
    }

    fn counter_difference(
        larger: (usize, usize, usize),
        smaller: (usize, usize, usize),
    ) -> (usize, usize, usize) {
        (
            larger.0.checked_sub(smaller.0).unwrap(),
            larger.1.checked_sub(smaller.1).unwrap(),
            larger.2.checked_sub(smaller.2).unwrap(),
        )
    }

    fn polynomial_envelope_counters(shape: &PolynomialWorkShape) -> (usize, usize, usize) {
        (
            shape.terms,
            shape.exponent_entries,
            shape.total_integer_bits,
        )
    }

    fn coefficient_envelope_counters(shape: &CoefficientWorkShape) -> (usize, usize, usize) {
        (
            shape.total_terms().unwrap(),
            shape.total_exponent_entries().unwrap(),
            shape.total_integer_bits().unwrap(),
        )
    }

    fn add_counter_tuples(
        left: (usize, usize, usize),
        right: (usize, usize, usize),
    ) -> (usize, usize, usize) {
        (
            left.0.checked_add(right.0).unwrap(),
            left.1.checked_add(right.1).unwrap(),
            left.2.checked_add(right.2).unwrap(),
        )
    }

    fn scale_counter_tuple(value: (usize, usize, usize), factor: usize) -> (usize, usize, usize) {
        (
            value.0.checked_mul(factor).unwrap(),
            value.1.checked_mul(factor).unwrap(),
            value.2.checked_mul(factor).unwrap(),
        )
    }

    #[test]
    fn coefficient_work_ledger_delegates_to_private_estimator_and_symbolica_operation() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "coefficient-work-facade", 2).unwrap();
        let left = context
            .add(&context.index(0).unwrap(), &context.integer(2))
            .unwrap();
        let right = context
            .sub(&context.index(1).unwrap(), &context.integer(3))
            .unwrap();

        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            ParametricCoefficientWorkLedgerLimits::default(),
        );
        let actual = ledger.try_mul(&context, &left, &right).unwrap();
        assert_eq!(actual, context.mul(&left, &right).unwrap());

        let mut direct = WorkBudget::construction(ParametricEliminationLimits::default(), 0);
        direct
            .charge_binary_coefficient_operation(CoefficientOperation::Multiply, &left, &right)
            .unwrap();
        let expected = coefficient_work_counters(&direct);
        let stats = ledger.stats();
        assert_eq!(
            (
                stats.algebra_work(),
                stats.exponent_entry_work(),
                stats.integer_bit_work(),
            ),
            expected
        );
    }

    #[test]
    fn coefficient_work_ledger_executes_complete_exact_field_surface() {
        let base = CoefficientContext::new(["d", "m2"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "coefficient-work-complete-surface", 2)
                .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let denominator = context.add(&n1, &context.one()).unwrap();
        let fraction = context.checked_div(&n0, &denominator).unwrap();
        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Replay,
            ParametricCoefficientWorkLedgerLimits::default(),
        );

        let one = ledger.try_one(&context).unwrap();
        assert_eq!(one, context.one());
        let copied = ledger.try_copy_authenticated(&fraction).unwrap();
        assert_eq!(copied, fraction);
        let negated = ledger.try_neg(&context, &copied).unwrap();
        assert_eq!(negated, context.neg(&copied).unwrap());
        let sum = ledger.try_add(&context, &copied, &one).unwrap();
        assert_eq!(sum, context.add(&copied, &one).unwrap());
        let difference = ledger.try_sub(&context, &sum, &one).unwrap();
        assert_eq!(difference, copied);
        let product = ledger.try_mul(&context, &difference, &denominator).unwrap();
        assert_eq!(product, n0);
        let pending = ledger
            .try_guarded_division_pending(&context, &one, &denominator)
            .unwrap();
        let guarded = ledger
            .try_finish_guarded_division(&context, pending)
            .unwrap();
        assert_eq!(
            guarded.value,
            context.checked_div(&one, &denominator).unwrap()
        );
        assert_eq!(guarded.nonzero.len(), 1);
        assert!(ledger.stats().algebra_work() > 0);
        assert!(ledger.stats().exponent_entry_work() > 0);
        assert!(ledger.stats().integer_bit_work() > 0);
    }

    #[test]
    fn coefficient_work_ledger_enforces_guarded_division_origin_limits_transactionally() {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(
            &base,
            "coefficient-work-guarded-division-origin-limit",
            1,
        )
        .unwrap();
        let one = context.one();
        let locus = context.add(&context.index(0).unwrap(), &one).unwrap();

        let limits = |max_guard_origins| {
            let mut arithmetic = ParametricArithmeticLimits::default();
            arithmetic.max_guard_origins = max_guard_origins;
            ParametricCoefficientWorkLedgerLimits {
                arithmetic,
                ..ParametricCoefficientWorkLedgerLimits::default()
            }
        };

        // One nonconstant divisor numerator needs exactly one origin.
        let mut exact_one = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits(1),
        );
        let pending = exact_one
            .try_guarded_division_pending(&context, &one, &locus)
            .unwrap();
        let guarded = exact_one
            .try_finish_guarded_division(&context, pending)
            .unwrap();
        assert_eq!(guarded.nonzero.len(), 1);
        assert_eq!(guarded.nonzero[0].origins().len(), 1);
        assert!(
            guarded.nonzero[0]
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDivisorNumerator)
        );

        let mut one_below_zero = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits(0),
        );
        one_below_zero.try_one(&context).unwrap();
        let committed_zero = one_below_zero.stats();
        assert!(matches!(
            one_below_zero.try_guarded_division_pending(&context, &one, &locus),
            Err(ParametricCoefficientWorkError::Elimination(
                ParametricEliminationError::Coefficient(
                    ParametricCoefficientError::ResourceLimit {
                        resource: "parametric guard origin inputs",
                        requested: 1,
                        limit: 0,
                    }
                )
            ))
        ));
        assert_eq!(one_below_zero.stats(), committed_zero);

        // `(1/locus) / locus` discovers the same locus once as the dividend
        // denominator and once as the divisor numerator. Exact admission is
        // two merged origins; one below must roll back all ledger counters.
        let reciprocal = context.checked_div(&one, &locus).unwrap();
        let mut exact_two = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits(2),
        );
        let pending = exact_two
            .try_guarded_division_pending(&context, &reciprocal, &locus)
            .unwrap();
        let guarded = exact_two
            .try_finish_guarded_division(&context, pending)
            .unwrap();
        assert_eq!(guarded.nonzero.len(), 1);
        assert_eq!(guarded.nonzero[0].origins().len(), 2);
        assert!(
            guarded.nonzero[0]
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDividendDenominator)
        );
        assert!(
            guarded.nonzero[0]
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDivisorNumerator)
        );

        let mut one_below_one = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits(1),
        );
        one_below_one.try_one(&context).unwrap();
        let committed_one = one_below_one.stats();
        assert!(matches!(
            one_below_one.try_guarded_division_pending(&context, &reciprocal, &locus),
            Err(ParametricCoefficientWorkError::Elimination(
                ParametricEliminationError::Coefficient(
                    ParametricCoefficientError::ResourceLimit {
                        resource: "parametric guard origins",
                        requested: 2,
                        limit: 1,
                    }
                )
            ))
        ));
        assert_eq!(one_below_one.stats(), committed_one);

        // The finish phase is independently authenticated: a caller cannot
        // construct a two-origin pending value through the compatibility
        // default and smuggle it into a stricter ledger.
        let default_pending = context
            .checked_div_guarded_pending_normalization_with_limits(
                &reciprocal,
                &locus,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let mut strict_finish = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits(1),
        );
        strict_finish.try_one(&context).unwrap();
        let committed_finish = strict_finish.stats();
        assert!(matches!(
            strict_finish.try_finish_guarded_division(&context, default_pending),
            Err(ParametricCoefficientWorkError::Elimination(
                ParametricEliminationError::Coefficient(
                    ParametricCoefficientError::ResourceLimit {
                        resource: "parametric guard origins",
                        requested: 2,
                        limit: 1,
                    }
                )
            ))
        ));
        assert_eq!(strict_finish.stats(), committed_finish);
    }

    #[test]
    fn elimination_construction_and_replay_use_the_configured_guard_origin_limit() {
        let (_, context) = synthetic_context();
        let source = recurrence(
            &context,
            "family",
            "guarded-division-production-origin-limit",
        );
        assert!(source.guarded_nonzero_conditions().is_empty());
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();

        let mut strict = ParametricEliminationLimits::default();
        strict.arithmetic.max_guard_origins = 0;
        assert!(matches!(
            ParametricElimination::build(&context, &[source.clone()], ordering.clone(), strict,),
            Err(ParametricEliminationError::Coefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "parametric guard origin inputs",
                    requested: 1,
                    limit: 0,
                }
            ))
        ));

        let mut certificate = ParametricElimination::build(
            &context,
            &[source.clone()],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        certificate.kernel.limits.arithmetic.max_guard_origins = 0;
        assert!(matches!(
            certificate.replay(&context, &[source]),
            Err(ParametricEliminationError::Coefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "parametric guard origin inputs",
                    requested: 1,
                    limit: 0,
                }
            ))
        ));
    }

    #[test]
    fn coefficient_work_ledger_limit_failure_is_transactional() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "coefficient-work-transactional-limit", 2)
                .unwrap();
        let left = context
            .add(&context.index(0).unwrap(), &context.integer(2))
            .unwrap();
        let right = context
            .sub(&context.index(1).unwrap(), &context.integer(3))
            .unwrap();
        let mut exact = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            ParametricCoefficientWorkLedgerLimits::default(),
        );
        exact.try_mul(&context, &left, &right).unwrap();
        let demand = exact.stats();

        for one_below in [
            ParametricCoefficientWorkLedgerLimits {
                max_algebra_work: demand.algebra_work() - 1,
                ..ParametricCoefficientWorkLedgerLimits::default()
            },
            ParametricCoefficientWorkLedgerLimits {
                max_exponent_entry_work: demand.exponent_entry_work() - 1,
                ..ParametricCoefficientWorkLedgerLimits::default()
            },
            ParametricCoefficientWorkLedgerLimits {
                max_integer_bit_work: demand.integer_bit_work() - 1,
                ..ParametricCoefficientWorkLedgerLimits::default()
            },
        ] {
            let mut ledger = ParametricCoefficientWorkLedger::new(
                ParametricCoefficientWorkPhase::Construction,
                one_below,
            );
            assert!(matches!(
                ledger.try_mul(&context, &left, &right),
                Err(ParametricCoefficientWorkError::Elimination(
                    ParametricEliminationError::ResourceLimit { .. }
                ))
            ));
            assert_eq!(ledger.stats(), ParametricCoefficientWorkStats::default());
        }
    }

    #[test]
    fn per_variable_degree_upper_bounds_remain_tight_under_sums_products_and_branch_unions() {
        let limits = ParametricEliminationLimits::default();
        let mut work = WorkBudget::construction(limits, 0);

        let mut repeated = test_polynomial_work_shape(2, &[8, 0], 3);
        for _ in 0..64 {
            repeated = polynomial_sum_work(&mut work, &repeated, &repeated)
                .unwrap()
                .output;
            assert_eq!(repeated.degree_bounds, [8, 0]);
            assert_eq!(repeated.monomial_box_bound().unwrap(), 9);
            assert!(repeated.terms <= 9);
        }

        let x_only = test_polynomial_work_shape(11, &[10, 0], 5);
        let y_only = test_polynomial_work_shape(11, &[0, 10], 7);
        let sum = polynomial_sum_work(&mut work, &x_only, &y_only).unwrap();
        assert_eq!(sum.output.degree_bounds, [10, 10]);
        assert_eq!(sum.output.monomial_box_bound().unwrap(), 121);
        assert_eq!(sum.output.terms, 22);

        let branch_union =
            polynomial_shape_componentwise_upper_bound(&mut work, &x_only, &y_only).unwrap();
        assert_eq!(branch_union.degree_bounds, [10, 10]);
        assert_eq!(branch_union.monomial_box_bound().unwrap(), 121);
        assert_eq!(branch_union.terms, 11);

        let left = test_polynomial_work_shape(3, &[2, 0], 5);
        let right = test_polynomial_work_shape(20, &[3, 4], 7);
        let product = polynomial_product_work(&mut work, &left, &right).unwrap();
        assert_eq!(product.output.degree_bounds, [5, 4]);
        assert_eq!(product.output.monomial_box_bound().unwrap(), 30);
        assert_eq!(product.output.terms, 30);
    }

    #[test]
    fn per_variable_degree_bounds_cover_zero_canonicalization_and_gcd_edges() {
        let limits = ParametricEliminationLimits::default();
        let mut work = WorkBudget::construction(limits, 0);
        let zero = zero_polynomial_work_shape(2);
        let polynomial = test_polynomial_work_shape(20, &[10, 2], 11);

        let product = polynomial_product_work(&mut work, &zero, &polynomial).unwrap();
        assert_eq!(product.output.terms, 0);
        assert!(product.output.degree_bounds.is_empty());
        assert_eq!(product.output.monomial_box_bound().unwrap(), 1);

        let canonical_zero = canonical_polynomial_factor_shape_bound(
            &mut work,
            &zero,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        assert_eq!(canonical_zero.terms, 0);
        assert!(canonical_zero.degree_bounds.is_empty());

        let other = test_polynomial_work_shape(20, &[3, 7], 13);
        let gcd = polynomial_common_factor_shape_bound(
            &mut work,
            &polynomial,
            &other,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        assert_eq!(gcd.degree_bounds, [3, 2]);
        assert_eq!(gcd.monomial_box_bound().unwrap(), 12);
        assert_eq!(gcd.terms, 12);

        let zero_gcd = polynomial_common_factor_shape_bound(
            &mut work,
            &zero,
            &polynomial,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        assert_eq!(zero_gcd.degree_bounds, [10, 2]);
        assert_eq!(zero_gcd.monomial_box_bound().unwrap(), 33);
    }

    #[test]
    fn degree_bound_metadata_work_has_exact_limit_and_overflow_boundaries() {
        fn combine_with_limits(
            limits: ParametricEliminationLimits,
        ) -> Result<(Vec<usize>, usize), ParametricEliminationError> {
            let left = test_polynomial_work_shape(3, &[2, 1], 3);
            let right = test_polynomial_work_shape(4, &[1, 4], 3);
            let mut work = WorkBudget::construction(limits, 0);
            let bounds = combine_polynomial_degree_bounds(
                &mut work,
                &left,
                &right,
                DegreeBoundCombination::Sum,
            )?;
            Ok((bounds, work.coefficient_exponent_entry_work))
        }

        let defaults = ParametricEliminationLimits::default();
        let (bounds, exact_work) = combine_with_limits(defaults).unwrap();
        assert_eq!(bounds, [3, 5]);
        assert_eq!(exact_work, 14);

        let mut exact = defaults;
        exact.max_construction_coefficient_exponent_entry_work = exact_work;
        assert_eq!(combine_with_limits(exact).unwrap().1, exact_work);

        let mut one_below = defaults;
        one_below.max_construction_coefficient_exponent_entry_work = exact_work - 1;
        assert!(matches!(
            combine_with_limits(one_below),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == exact_work && limit + 1 == requested
        ));

        fn box_with_limits(
            limits: ParametricEliminationLimits,
        ) -> Result<(usize, usize), ParametricEliminationError> {
            let shape = test_polynomial_work_shape(4, &[2, 3], 3);
            let mut work = WorkBudget::construction(limits, 0);
            let bound = charged_monomial_box_bound_from_degrees(&mut work, &shape.degree_bounds)?;
            Ok((bound, work.coefficient_exponent_entry_work))
        }

        let (box_bound, box_work) = box_with_limits(defaults).unwrap();
        assert_eq!(box_bound, 12);
        assert_eq!(box_work, 2);

        let mut exact_box = defaults;
        exact_box.max_construction_coefficient_exponent_entry_work = box_work;
        assert_eq!(box_with_limits(exact_box).unwrap(), (box_bound, box_work));

        let mut one_below_box = defaults;
        one_below_box.max_construction_coefficient_exponent_entry_work = box_work - 1;
        assert!(matches!(
            box_with_limits(one_below_box),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == box_work && limit + 1 == requested
        ));

        let oversized_box = test_polynomial_work_shape(1, &[usize::MAX], 1);
        assert!(matches!(
            oversized_box.monomial_box_bound(),
            Err(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient normalization monomial box",
            })
        ));

        let one = test_polynomial_work_shape(1, &[1], 1);
        let mut overflow_work = WorkBudget::construction(defaults, 0);
        assert!(matches!(
            polynomial_product_work(&mut overflow_work, &oversized_box, &one),
            Err(ParametricEliminationError::ResourceCountOverflow {
                resource: "coefficient polynomial degree bound",
            })
        ));

        let malformed = PolynomialWorkShape {
            variable_count: 2,
            degree_bounds: vec![1],
            ..test_polynomial_work_shape(1, &[0, 0], 1)
        };
        assert!(matches!(
            malformed.monomial_box_bound(),
            Err(ParametricEliminationError::InternalReplayFailure { .. })
        ));
    }

    #[test]
    fn degree_bound_copy_and_observation_have_exact_limit_boundaries() {
        fn copy_with_limits(
            limits: ParametricEliminationLimits,
        ) -> Result<(PolynomialWorkShape, usize), ParametricEliminationError> {
            let source = test_polynomial_work_shape(4, &[2, 3], 3);
            let mut work = WorkBudget::construction(limits, 0);
            let copy = copy_polynomial_work_shape(&mut work, &source)?;
            Ok((copy, work.coefficient_exponent_entry_work))
        }

        let defaults = ParametricEliminationLimits::default();
        let (copy, copy_work) = copy_with_limits(defaults).unwrap();
        assert_eq!(copy.degree_bounds, [2, 3]);
        assert_eq!(copy_work, 6);

        let mut exact_copy = defaults;
        exact_copy.max_construction_coefficient_exponent_entry_work = copy_work;
        assert_eq!(copy_with_limits(exact_copy).unwrap().1, copy_work);

        let mut one_below_copy = defaults;
        one_below_copy.max_construction_coefficient_exponent_entry_work = copy_work - 1;
        assert!(matches!(
            copy_with_limits(one_below_copy),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == copy_work && limit + 1 == requested
        ));

        let base = CoefficientContext::new(std::iter::empty::<&str>());
        let context = ParametricCoefficientContext::try_new(
            &base,
            "parametric-elimination-degree-observation-tests",
            2,
        )
        .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let nonconstant = context.add(&n0, &n1).unwrap();
        let constant = context.one();

        fn observe_with_limits(
            polynomial: &crate::CoefficientPolynomial,
            limits: ParametricEliminationLimits,
        ) -> Result<(PolynomialWorkShape, usize), ParametricEliminationError> {
            let layout = PolynomialWorkLayout::new(polynomial);
            let pending = PendingPolynomialWorkShape {
                layout,
                maximum_integer_bits: 1,
                total_integer_bits: layout.terms,
            };
            let mut work = WorkBudget::construction(limits, 0);
            let shape = complete_polynomial_shape(&mut work, polynomial, pending)?;
            Ok((shape, work.coefficient_exponent_entry_work))
        }

        let nonconstant_polynomial = &nonconstant.raw().numerator;
        let (observed, observation_work) =
            observe_with_limits(nonconstant_polynomial, defaults).unwrap();
        let exponent_entries = nonconstant_polynomial.exponents.len();
        let variable_count = nonconstant_polynomial.variables.len();
        assert_eq!(observation_work, 4 * exponent_entries + variable_count);
        assert_eq!(observed.degree_bounds, [1, 1]);

        let mut exact_observation = defaults;
        exact_observation.max_construction_coefficient_exponent_entry_work = observation_work;
        assert_eq!(
            observe_with_limits(nonconstant_polynomial, exact_observation)
                .unwrap()
                .1,
            observation_work
        );

        let mut one_below_observation = defaults;
        one_below_observation.max_construction_coefficient_exponent_entry_work =
            observation_work - 1;
        assert!(matches!(
            observe_with_limits(nonconstant_polynomial, one_below_observation),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == observation_work && limit + 1 == requested
        ));

        let constant_polynomial = &constant.raw().numerator;
        let (observed_constant, constant_work) =
            observe_with_limits(constant_polynomial, defaults).unwrap();
        assert_eq!(constant_work, constant_polynomial.exponents.len());
        assert!(observed_constant.degree_bounds.is_empty());

        let mut one_below_constant = defaults;
        one_below_constant.max_construction_coefficient_exponent_entry_work = constant_work - 1;
        assert!(matches!(
            observe_with_limits(constant_polynomial, one_below_constant),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == constant_work && limit + 1 == requested
        ));
    }

    fn higher_arity_resource_fixture(
        limits: ParametricEliminationLimits,
    ) -> Result<ParametricElimination, ParametricEliminationError> {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(
            &base,
            "parametric-elimination-higher-arity-tests",
            5,
        )
        .unwrap();
        let space = IndexSpace::try_new(5).unwrap();
        let n0 = context.index(0).unwrap();
        let n4 = context.index(4).unwrap();
        let coefficient = context.mul(&n0, &n4).unwrap();
        let mut row = ParametricRelation::new(
            "higher-arity-family",
            ParametricRowId::Derived {
                label: Arc::from("higher-arity-resource-row"),
            },
            &context,
        );
        row.add_term(&context, space.unit(0, 1).unwrap(), coefficient)
            .unwrap();
        row.add_term(&context, space.zero(), context.integer(-1))
            .unwrap();
        ParametricElimination::build(
            &context,
            &[row],
            ParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                [1, 1, 1, 1, 1],
            )
            .unwrap(),
            limits,
        )
    }

    #[test]
    fn normalizes_centers_and_replays_a_symbolic_recurrence() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "source");
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        let elimination = ParametricElimination::build(
            &context,
            &[source.clone()],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        assert_eq!(elimination.stats().rank(), 1);
        assert_eq!(elimination.free_columns().len(), 1);
        elimination.replay(&context, &[source]).unwrap();

        let pivot = &elimination.pivots()[0];
        assert_eq!(pivot.pivot().values(), &[1]);
        assert_eq!(
            pivot.unit_relation().terms().get(pivot.pivot()).unwrap(),
            &context.one()
        );
        assert!(
            pivot
                .unit_relation()
                .guarded_nonzero_conditions()
                .iter()
                .any(|condition| condition
                    .origins()
                    .contains(&GuardOrigin::GuardedDivisionDivisorNumerator))
        );

        let centered = pivot
            .centered_relation(&context, ParametricArithmeticLimits::default())
            .unwrap();
        let zero = IndexSpace::try_new(1).unwrap().zero();
        assert_eq!(centered.terms().get(&zero), Some(&context.one()));
        assert!(
            centered
                .guarded_nonzero_conditions()
                .iter()
                .any(|condition| !condition.polynomial().is_nonzero_constant())
        );
    }

    #[test]
    fn dependent_rows_do_not_become_hardcoded_extra_pivots() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "source");
        let mut doubled = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("dependent"),
            },
            &context,
        );
        doubled
            .add_scaled(&context, &source, &context.integer(2))
            .unwrap();
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        let elimination = ParametricElimination::build(
            &context,
            &[source, doubled],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        assert_eq!(elimination.pivots().len(), 1);
        assert_eq!(elimination.stats().rank(), 1);
        assert!(elimination.stats().construction_reductions() >= 1);
    }

    #[test]
    fn authenticated_preordered_mode_reuses_the_anchor_algebra_and_replay_kernel() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "source");
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        let anchor = ParametricElimination::build(
            &context,
            &[source.clone()],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let columns = anchor.columns_easiest_first().to_vec();
        let preordered = PreorderedParametricElimination::build(
            &context,
            &[source.clone()],
            columns.clone(),
            "authenticated-test-ordering-v1",
            ParametricEliminationLimits::default(),
        )
        .unwrap();

        assert_eq!(preordered.family_fingerprint(), anchor.family_fingerprint());
        assert_eq!(
            preordered.context_fingerprint(),
            anchor.context_fingerprint()
        );
        assert_eq!(preordered.source_manifest(), anchor.source_manifest());
        assert_eq!(preordered.limits(), anchor.limits());
        assert_eq!(preordered.columns_easiest_first(), columns);
        assert_eq!(preordered.free_columns(), anchor.free_columns());
        let mut preordered_kernel_stats = preordered.stats();
        let mut anchor_kernel_stats = anchor.stats();
        // Algebra is shared. The complete retained-byte census intentionally
        // differs because one wrapper owns an authenticated identity string
        // while the public wrapper owns a concrete anchor allocation.
        preordered_kernel_stats.retained_bytes = 0;
        anchor_kernel_stats.retained_bytes = 0;
        assert_eq!(preordered_kernel_stats, anchor_kernel_stats);
        assert_ne!(
            preordered.stats().retained_bytes(),
            anchor.stats().retained_bytes()
        );
        assert_eq!(preordered.pivots().len(), anchor.pivots().len());
        for (left, right) in preordered.pivots().iter().zip(anchor.pivots()) {
            assert_eq!(left.ordinal(), right.ordinal());
            assert_eq!(left.pivot(), right.pivot());
            assert_eq!(left.trace(), right.trace());
            assert!(
                left.unit_relation()
                    .has_identical_guard_provenance(right.unit_relation())
            );
        }
        assert_eq!(
            preordered.ordering_identity(),
            "authenticated-test-ordering-v1"
        );
        preordered
            .replay(
                &context,
                &[source.clone()],
                &columns,
                "authenticated-test-ordering-v1",
            )
            .unwrap();
        assert_eq!(
            preordered
                .replay(&context, &[source.clone()], &columns, "another-ordering")
                .unwrap_err(),
            ParametricEliminationError::OrderingIdentityMismatch
        );
        let mut reversed = columns.clone();
        reversed.reverse();
        assert_eq!(
            preordered
                .replay(
                    &context,
                    &[source],
                    &reversed,
                    "authenticated-test-ordering-v1",
                )
                .unwrap_err(),
            ParametricEliminationError::ColumnOrderMismatch
        );
    }

    #[test]
    fn authenticated_preordered_mode_rejects_duplicate_missing_and_foreign_arity_columns() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "source");
        let space = IndexSpace::try_new(1).unwrap();
        let zero = space.zero();
        let raised = space.unit(0, 1).unwrap();
        let limits = ParametricEliminationLimits::default();

        assert!(matches!(
            PreorderedParametricElimination::build(
                &context,
                &[source.clone()],
                vec![zero.clone(), zero, raised.clone()],
                "authenticated-test-ordering-v1",
                limits,
            ),
            Err(ParametricEliminationError::DuplicateColumn {
                first_position: 0,
                duplicate_position: 1,
                ..
            })
        ));

        assert!(matches!(
            PreorderedParametricElimination::build(
                &context,
                &[source.clone()],
                vec![raised.clone()],
                "authenticated-test-ordering-v1",
                limits,
            ),
            Err(ParametricEliminationError::MissingColumn { .. })
        ));

        assert_eq!(
            PreorderedParametricElimination::build(
                &context,
                &[source.clone()],
                vec![raised, IndexSpace::try_new(2).unwrap().zero()],
                "authenticated-test-ordering-v1",
                limits,
            )
            .unwrap_err(),
            ParametricEliminationError::WrongArity {
                expected: 1,
                actual: 2,
            }
        );

        assert!(matches!(
            PreorderedParametricElimination::build(
                &context,
                &[source.clone()],
                vec![space.zero(), space.unit(0, 2).unwrap()],
                "authenticated-test-ordering-v1",
                limits,
            ),
            Err(ParametricEliminationError::UnexpectedColumn { position: 1, .. })
        ));

        assert_eq!(
            PreorderedParametricElimination::build(
                &context,
                &[source],
                vec![space.zero(), space.unit(0, 1).unwrap()],
                "",
                limits,
            )
            .unwrap_err(),
            ParametricEliminationError::EmptyOrderingIdentity
        );
    }

    #[test]
    fn rejects_foreign_families_and_resource_exhaustion() {
        let (_, context) = synthetic_context();
        let first = recurrence(&context, "family-a", "first");
        let second = recurrence(&context, "family-b", "second");
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        assert_eq!(
            ParametricElimination::build(
                &context,
                &[first.clone(), second],
                ordering.clone(),
                ParametricEliminationLimits::default(),
            )
            .unwrap_err(),
            ParametricEliminationError::WrongFamily { row: 1 }
        );

        let mut strict = ParametricEliminationLimits::default();
        strict.max_input_terms = 1;
        assert!(matches!(
            ParametricElimination::build(&context, &[first], ordering, strict),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "input relation terms",
                ..
            })
        ));
    }

    #[test]
    fn cheap_source_caps_precede_every_deep_authentication_pass() {
        let (_, context) = synthetic_context();
        let mut guarded = recurrence(&context, "family", "guarded-source-cap-precedence");
        let n = context.index(0).unwrap();
        guarded
            .add_nonzero_condition(&context, context.numerator_condition(&n).unwrap())
            .unwrap();
        assert!(!guarded.guarded_nonzero_conditions().is_empty());

        let cases = [
            ("input relation terms", 0usize),
            ("parametric columns", 1),
            ("input relation guards", 2),
            ("input relation guard origins", 3),
            ("origins in one source guard", 4),
        ];
        for (expected_resource, case) in cases {
            let mut limits = ParametricEliminationLimits::default();
            limits.max_construction_coefficient_algebra_work = 0;
            limits.max_construction_coefficient_exponent_entry_work = 0;
            limits.max_construction_coefficient_integer_bit_work = 0;
            match case {
                0 => limits.max_input_terms = guarded.terms().len() - 1,
                1 => limits.max_columns = 0,
                2 => limits.max_input_guards = 0,
                3 => limits.max_input_guard_origins = 0,
                4 => limits.arithmetic.max_guard_origins = 0,
                _ => unreachable!(),
            }
            let mut work = WorkBudget::construction(limits, 0);
            assert!(matches!(
                validate_source_rows(&context, &[guarded.clone()], limits, &mut work),
                Err(ParametricEliminationError::ResourceLimit {
                    resource,
                    ..
                }) if resource == expected_resource
            ));
            assert_eq!(work.coefficient_algebra_work, 0);
            assert_eq!(work.coefficient_exponent_entry_work, 0);
            assert_eq!(work.coefficient_integer_bit_work, 0);
        }

        let poisoned_zero = context.zero_nonzero_condition_for_test();
        assert!(poisoned_zero.polynomial().is_zero());
        assert!(!poisoned_zero.origins().is_empty());
        let mut limits = ParametricEliminationLimits::default();
        limits.max_input_guard_origins = 0;
        let mut guards = 0;
        let mut origins = 0;
        assert!(matches!(
            census_source_guard_metadata(
                std::slice::from_ref(&poisoned_zero),
                limits,
                &mut guards,
                &mut origins,
            ),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "input relation guard origins",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn replay_source_caps_precede_every_deep_authentication_pass() {
        let (_, context) = synthetic_context();
        let mut source = recurrence(&context, "family", "replay-source-cap-precedence");
        let n = context.index(0).unwrap();
        source
            .add_nonzero_condition(&context, context.numerator_condition(&n).unwrap())
            .unwrap();
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        let reference = ParametricElimination::build(
            &context,
            &[source.clone()],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();

        for (expected_resource, case) in [
            ("input relation terms", 0usize),
            ("parametric columns", 1),
            ("input relation guards", 2),
            ("input relation guard origins", 3),
            ("origins in one source guard", 4),
        ] {
            let mut certificate = reference.clone();
            let limits = &mut certificate.kernel.limits;
            limits.max_replay_coefficient_algebra_work = 0;
            limits.max_replay_coefficient_exponent_entry_work = 0;
            limits.max_replay_coefficient_integer_bit_work = 0;
            match case {
                0 => limits.max_input_terms = source.terms().len() - 1,
                1 => limits.max_columns = 0,
                2 => limits.max_input_guards = 0,
                3 => limits.max_input_guard_origins = 0,
                4 => limits.arithmetic.max_guard_origins = 0,
                _ => unreachable!(),
            }
            assert!(matches!(
                certificate.replay(&context, &[source.clone()]),
                Err(ParametricEliminationError::ResourceLimit {
                    resource,
                    ..
                }) if resource == expected_resource
            ));
        }
    }

    #[test]
    fn exact_coefficient_work_and_retained_byte_census_limits_rebuild() {
        let (_, context) = synthetic_context();
        let reference = resource_fixture(&context, ParametricEliminationLimits::default()).unwrap();
        let stats = reference.stats();
        assert_eq!(reference.pivots().len(), 2);
        assert_eq!(reference.pivots()[1].trace().reductions().len(), 1);
        assert!(!reference.free_columns().is_empty());
        assert!(
            reference
                .source_manifest()
                .contains(PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA)
        );
        assert!(matches!(
            reference.pivots()[0].unit_relation().row_id(),
            ParametricRowId::Derived { label }
                if label.starts_with("parametric-elimination-pivot-")
        ));
        assert!(!reference.pivots()[0].trace().divisor().is_zero());
        assert!(
            !reference.pivots()[1].trace().reductions()[0]
                .factor()
                .is_zero()
        );
        assert!(stats.construction_coefficient_algebra_work() > 0);
        assert!(stats.construction_coefficient_exponent_entry_work() > 0);
        assert!(stats.construction_coefficient_integer_bit_work() > 0);
        assert!(stats.replay_coefficient_algebra_work() > 0);
        assert!(stats.replay_coefficient_exponent_entry_work() > 0);
        assert!(stats.replay_coefficient_integer_bit_work() > 0);
        assert!(stats.retained_bytes() > 0);
        assert!(stats.retained_guards() > 0);
        assert!(stats.retained_guard_origins() >= stats.retained_guards());

        let mut exact = ParametricEliminationLimits::default();
        exact.max_construction_coefficient_algebra_work =
            stats.construction_coefficient_algebra_work();
        exact.max_construction_coefficient_exponent_entry_work =
            stats.construction_coefficient_exponent_entry_work();
        exact.max_construction_coefficient_integer_bit_work =
            stats.construction_coefficient_integer_bit_work();
        exact.max_replay_coefficient_algebra_work = stats.replay_coefficient_algebra_work();
        exact.max_replay_coefficient_exponent_entry_work =
            stats.replay_coefficient_exponent_entry_work();
        exact.max_replay_coefficient_integer_bit_work = stats.replay_coefficient_integer_bit_work();
        exact.max_retained_bytes = stats.retained_bytes();
        let rebuilt = resource_fixture(&context, exact).unwrap();
        assert_eq!(rebuilt.stats(), stats);
    }

    #[test]
    fn every_new_elimination_resource_limit_rejects_one_below_its_exact_census() {
        let (_, context) = synthetic_context();
        let stats = resource_fixture(&context, ParametricEliminationLimits::default())
            .unwrap()
            .stats();

        let cases = [
            (
                "construction coefficient algebra work",
                stats.construction_coefficient_algebra_work(),
            ),
            (
                "construction coefficient integer-bit work",
                stats.construction_coefficient_integer_bit_work(),
            ),
            (
                "construction coefficient exponent-entry work",
                stats.construction_coefficient_exponent_entry_work(),
            ),
            (
                "replay coefficient algebra work",
                stats.replay_coefficient_algebra_work(),
            ),
            (
                "replay coefficient integer-bit work",
                stats.replay_coefficient_integer_bit_work(),
            ),
            (
                "replay coefficient exponent-entry work",
                stats.replay_coefficient_exponent_entry_work(),
            ),
            ("reductions", stats.replay_reductions()),
            ("sparse updates", stats.replay_updates()),
            (
                "retained parametric elimination bytes",
                stats.retained_bytes(),
            ),
        ];

        for (resource, exact_value) in cases {
            assert!(exact_value > 0);
            let mut limits = ParametricEliminationLimits::default();
            match resource {
                "construction coefficient algebra work" => {
                    limits.max_construction_coefficient_algebra_work = exact_value - 1;
                }
                "construction coefficient integer-bit work" => {
                    limits.max_construction_coefficient_integer_bit_work = exact_value - 1;
                }
                "construction coefficient exponent-entry work" => {
                    limits.max_construction_coefficient_exponent_entry_work = exact_value - 1;
                }
                "replay coefficient algebra work" => {
                    limits.max_replay_coefficient_algebra_work = exact_value - 1;
                }
                "replay coefficient integer-bit work" => {
                    limits.max_replay_coefficient_integer_bit_work = exact_value - 1;
                }
                "replay coefficient exponent-entry work" => {
                    limits.max_replay_coefficient_exponent_entry_work = exact_value - 1;
                }
                "reductions" => {
                    limits.max_replay_reductions = exact_value - 1;
                }
                "sparse updates" => {
                    limits.max_replay_updates = exact_value - 1;
                }
                "retained parametric elimination bytes" => {
                    limits.max_retained_bytes = exact_value - 1;
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                resource_fixture(&context, limits),
                Err(ParametricEliminationError::ResourceLimit {
                    resource: actual_resource,
                    requested,
                    limit,
                }) if actual_resource == resource && requested == exact_value && limit + 1 == exact_value
            ));
        }
    }

    #[test]
    fn dense_exponent_entry_work_is_exact_and_arity_sensitive() {
        let reference =
            higher_arity_resource_fixture(ParametricEliminationLimits::default()).unwrap();
        let stats = reference.stats();
        assert!(
            stats.construction_coefficient_exponent_entry_work()
                > stats.construction_coefficient_algebra_work()
        );
        assert!(
            stats.replay_coefficient_exponent_entry_work()
                > stats.replay_coefficient_algebra_work()
        );

        let mut exact = ParametricEliminationLimits::default();
        exact.max_construction_coefficient_exponent_entry_work =
            stats.construction_coefficient_exponent_entry_work();
        exact.max_replay_coefficient_exponent_entry_work =
            stats.replay_coefficient_exponent_entry_work();
        assert_eq!(higher_arity_resource_fixture(exact).unwrap().stats(), stats);

        let mut construction_one_below = ParametricEliminationLimits::default();
        construction_one_below.max_construction_coefficient_exponent_entry_work =
            stats.construction_coefficient_exponent_entry_work() - 1;
        assert!(matches!(
            higher_arity_resource_fixture(construction_one_below),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == stats.construction_coefficient_exponent_entry_work()
                && limit + 1 == requested
        ));

        let mut replay_one_below = ParametricEliminationLimits::default();
        replay_one_below.max_replay_coefficient_exponent_entry_work =
            stats.replay_coefficient_exponent_entry_work() - 1;
        assert!(matches!(
            higher_arity_resource_fixture(replay_one_below),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "replay coefficient exponent-entry work",
                requested,
                limit,
            }) if requested == stats.replay_coefficient_exponent_entry_work()
                && limit + 1 == requested
        ));
    }

    #[test]
    fn replay_rejects_tampered_stored_work_census() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "replay-work-source");
        let mut elimination = ParametricElimination::build(
            &context,
            &[source.clone()],
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        elimination
            .kernel
            .stats
            .replay_coefficient_exponent_entry_work += 1;
        assert!(matches!(
            elimination.replay(&context, &[source]),
            Err(ParametricEliminationError::InternalReplayFailure { detail })
                if detail.contains("stored certificate")
        ));
    }

    #[test]
    fn replay_rechecks_stored_source_row_limit_before_deep_authentication() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "replay-source-row-limit");
        let mut elimination = ParametricElimination::build(
            &context,
            &[source.clone()],
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        elimination.kernel.limits.max_source_rows = 0;
        assert!(matches!(
            elimination.replay(&context, &[source]),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "source rows",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn guarded_division_second_normalization_charges_actual_dense_input() {
        let base = CoefficientContext::new(["d", "m"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "guarded-division-normalization-work", 3)
                .unwrap();
        let divisor = context
            .add(&context.index(0).unwrap(), &context.index(2).unwrap())
            .unwrap();
        let pending = context
            .checked_div_guarded_pending_normalization_with_limits(
                &context.one(),
                &divisor,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let mut work = WorkBudget::construction(ParametricEliminationLimits::default(), 0);
        let shape = work
            .coefficient_shape(pending.value_before_final_normalization())
            .unwrap();
        let estimate = coefficient_final_normalization_estimate_from_shape(
            &mut work,
            &shape,
            ExactAlgebraLimits::default().max_polynomial_terms,
        )
        .unwrap();
        let term_pairs = shape.numerator.terms * shape.denominator.terms;
        let expected_pair_exponents = term_pairs * shape.numerator.variable_count;
        assert!(expected_pair_exponents > term_pairs);
        assert_eq!(
            estimate.exponent_entry_work,
            shape.total_exponent_entries().unwrap()
                + expected_pair_exponents
                + estimate.output.total_exponent_entries().unwrap()
        );
        context
            .finish_guarded_division_normalization_with_limits(
                pending,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
    }

    #[test]
    fn second_normalization_surroundings_match_independent_longest_branch_envelope() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "second-normalization-surroundings", 1)
                .unwrap();
        let n = context.index(0).unwrap();
        let numerator = context
            .numerator_condition(
                &context
                    .mul(&n, &context.add(&n, &context.one()).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let denominator = context.numerator_condition(&n).unwrap();
        let pending = context
            .noncanonical_pending_fraction_for_test(
                &numerator,
                &denominator,
                ExactAlgebraLimits::default(),
            )
            .unwrap();

        let limits = ParametricEliminationLimits::default();
        let mut shape_work = WorkBudget::construction(limits, 0);
        let shape = shape_work
            .coefficient_shape(pending.value_before_final_normalization())
            .unwrap();
        let shape_observation = coefficient_work_counters(&shape_work);
        let gcd = polynomial_common_factor_shape_bound(
            &mut shape_work,
            &shape.numerator,
            &shape.denominator,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        let numerator_quotient = canonical_polynomial_factor_shape_bound(
            &mut shape_work,
            &shape.numerator,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        let denominator_quotient = canonical_polynomial_factor_shape_bound(
            &mut shape_work,
            &shape.denominator,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        let numerator_reconstruction =
            polynomial_product_work(&mut shape_work, &numerator_quotient, &gcd).unwrap();
        let denominator_reconstruction =
            polynomial_product_work(&mut shape_work, &denominator_quotient, &gcd).unwrap();
        let bound_metadata =
            counter_difference(coefficient_work_counters(&shape_work), shape_observation);

        // Independently enumerate the longest legal Symbolica branch:
        // 2*N + 3*D + 4*G + 2*(N/G) + 3*(D/G), plus the two exact-division
        // reconstruction envelopes and the degree-box/copy metadata required
        // to construct those conservative shapes. This is intentionally not
        // derived from the charging helper under test.
        let mut expected = add_counter_tuples(
            bound_metadata,
            scale_counter_tuple(polynomial_envelope_counters(&shape.numerator), 2),
        );
        for contribution in [
            scale_counter_tuple(polynomial_envelope_counters(&shape.denominator), 3),
            scale_counter_tuple(polynomial_envelope_counters(&gcd), 4),
            scale_counter_tuple(polynomial_envelope_counters(&numerator_quotient), 2),
            scale_counter_tuple(polynomial_envelope_counters(&denominator_quotient), 3),
            (
                numerator_reconstruction.algebra_work,
                numerator_reconstruction.exponent_entry_work,
                numerator_reconstruction.integer_bit_work,
            ),
            (
                denominator_reconstruction.algebra_work,
                denominator_reconstruction.exponent_entry_work,
                denominator_reconstruction.integer_bit_work,
            ),
        ] {
            expected = add_counter_tuples(expected, contribution);
        }

        let run = |limits| {
            let mut work = WorkBudget::construction(limits, 0);
            work.charge_symbolica_rational_final_normalization_surroundings(&shape)?;
            Ok::<_, ParametricEliminationError>(coefficient_work_counters(&work))
        };
        assert_eq!(run(limits).unwrap(), expected);

        for (resource, exact) in [
            ("construction coefficient algebra work", expected.0),
            ("construction coefficient exponent-entry work", expected.1),
            ("construction coefficient integer-bit work", expected.2),
        ] {
            let mut one_below = limits;
            match resource {
                "construction coefficient algebra work" => {
                    one_below.max_construction_coefficient_algebra_work = exact - 1;
                }
                "construction coefficient exponent-entry work" => {
                    one_below.max_construction_coefficient_exponent_entry_work = exact - 1;
                }
                "construction coefficient integer-bit work" => {
                    one_below.max_construction_coefficient_integer_bit_work = exact - 1;
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                run(one_below),
                Err(ParametricEliminationError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == exact && limit + 1 == exact
            ));
        }
    }

    #[test]
    fn noncanonical_second_normalization_changes_value_and_has_exact_work_boundaries() {
        fn pending_n_times_n_plus_one_over_n(
            context: &ParametricCoefficientContext,
        ) -> Result<
            crate::parametric_coefficient::PendingGuardedParametricDivision,
            ParametricEliminationError,
        > {
            let n = context.index(0).unwrap();
            let n_plus_one = context.add(&n, &context.one())?;
            let numerator = context.mul(&n, &n_plus_one)?;
            let numerator = context.numerator_condition(&numerator)?;
            let denominator = context.numerator_condition(&n)?;
            context
                .noncanonical_pending_fraction_for_test(
                    &numerator,
                    &denominator,
                    ExactAlgebraLimits::default(),
                )
                .map_err(ParametricEliminationError::Coefficient)
        }

        fn run(
            context: &ParametricCoefficientContext,
            limits: ParametricEliminationLimits,
        ) -> Result<(GuardedParametricCoefficient, WorkBudget), ParametricEliminationError>
        {
            let pending = pending_n_times_n_plus_one_over_n(context)?;
            let mut work = WorkBudget::construction(limits, 0);
            work.charge_guarded_division_final_normalization(
                pending.value_before_final_normalization(),
            )?;
            let finished = context.finish_guarded_division_normalization_with_limits(
                pending,
                limits.arithmetic.exact_algebra,
            )?;
            work.charge_coefficient_observation(&finished.value)?;
            Ok((finished, work))
        }

        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "noncanonical-second-gcd", 1).unwrap();
        let n = context.index(0).unwrap();
        let expected = context.add(&n, &context.one()).unwrap();
        let pending = pending_n_times_n_plus_one_over_n(&context).unwrap();
        let before = pending.value_before_final_normalization().clone();
        assert_eq!(before.raw().denominator, n.raw().numerator);
        assert_ne!(before.raw().denominator, context.one().raw().denominator);
        assert_ne!(before, expected);
        let finished = context
            .finish_guarded_division_normalization_with_limits(
                pending,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert_eq!(finished.value, expected);

        let (_, reference_work) = run(&context, ParametricEliminationLimits::default()).unwrap();
        let exact = (
            reference_work.coefficient_algebra_work,
            reference_work.coefficient_exponent_entry_work,
            reference_work.coefficient_integer_bit_work,
        );
        assert!(exact.0 > 0 && exact.1 > 0 && exact.2 > 0);

        let mut exact_limits = ParametricEliminationLimits::default();
        exact_limits.max_construction_coefficient_algebra_work = exact.0;
        exact_limits.max_construction_coefficient_exponent_entry_work = exact.1;
        exact_limits.max_construction_coefficient_integer_bit_work = exact.2;
        let (exact_finished, exact_work) = run(&context, exact_limits).unwrap();
        assert_eq!(exact_finished.value, expected);
        assert_eq!(
            (
                exact_work.coefficient_algebra_work,
                exact_work.coefficient_exponent_entry_work,
                exact_work.coefficient_integer_bit_work,
            ),
            exact
        );

        for (resource, exact_value) in [
            ("construction coefficient algebra work", exact.0),
            ("construction coefficient exponent-entry work", exact.1),
            ("construction coefficient integer-bit work", exact.2),
        ] {
            let mut limits = ParametricEliminationLimits::default();
            match resource {
                "construction coefficient algebra work" => {
                    limits.max_construction_coefficient_algebra_work = exact_value - 1;
                }
                "construction coefficient exponent-entry work" => {
                    limits.max_construction_coefficient_exponent_entry_work = exact_value - 1;
                }
                "construction coefficient integer-bit work" => {
                    limits.max_construction_coefficient_integer_bit_work = exact_value - 1;
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                run(&context, limits),
                Err(ParametricEliminationError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == exact_value && limit + 1 == requested
            ));
        }
    }

    #[test]
    fn normalization_envelope_covers_sparse_dividend_dense_quotient() {
        let (_, context) = synthetic_context();
        let n = context.index(0).unwrap();
        let mut n_to_eight = context.one();
        for _ in 0..8 {
            n_to_eight = context.mul(&n_to_eight, &n).unwrap();
        }
        let sparse = context.sub(&n_to_eight, &context.one()).unwrap();
        let divisor = context.sub(&n, &context.one()).unwrap();
        assert_eq!(sparse.raw().numerator.nterms(), 2);

        let mut work = WorkBudget::construction(ParametricEliminationLimits::default(), 0);
        let sparse_shape = work.coefficient_shape(&sparse).unwrap();
        let divisor_shape = work.coefficient_shape(&divisor).unwrap();
        let envelope = coefficient_operation_estimate_from_shapes(
            &mut work,
            CoefficientOperation::Divide,
            &sparse_shape,
            &divisor_shape,
            sparse.raw().denominator == divisor.raw().denominator,
            100,
        )
        .unwrap();
        let quotient = context.checked_div(&sparse, &divisor).unwrap();
        assert!(quotient.raw().numerator.nterms() > sparse.raw().numerator.nterms());
        assert!(
            quotient.raw().numerator.nterms() <= envelope.output.numerator.terms,
            "dense quotient must fit the pre-normalization factor envelope"
        );
        assert!(
            quotient.raw().numerator.exponents.len() <= envelope.output.numerator.exponent_entries
        );
        assert!(
            coefficient_deep_retained_byte_bound(&quotient, &mut work).unwrap()
                <= coefficient_shape_retained_byte_bound(&envelope.output).unwrap()
        );
    }

    #[test]
    fn source_authentication_layout_limits_are_exact_and_precede_late_semantic_failure() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "source-layout-census");
        let default_limits = ParametricEliminationLimits::default();
        let mut reference_work = WorkBudget::construction(default_limits, 0);
        validate_source_rows(
            &context,
            &[source.clone()],
            default_limits,
            &mut reference_work,
        )
        .unwrap();
        let exact = (
            reference_work.coefficient_algebra_work,
            reference_work.coefficient_exponent_entry_work,
            reference_work.coefficient_integer_bit_work,
        );
        assert!(exact.0 > 0 && exact.1 > 0 && exact.2 > 0);

        let mut exact_limits = default_limits;
        exact_limits.max_construction_coefficient_algebra_work = exact.0;
        exact_limits.max_construction_coefficient_exponent_entry_work = exact.1;
        exact_limits.max_construction_coefficient_integer_bit_work = exact.2;
        let mut exact_work = WorkBudget::construction(exact_limits, 0);
        validate_source_rows(&context, &[source], exact_limits, &mut exact_work).unwrap();
        assert_eq!(
            (
                exact_work.coefficient_algebra_work,
                exact_work.coefficient_exponent_entry_work,
                exact_work.coefficient_integer_bit_work,
            ),
            exact
        );

        for (resource, exact_value) in [
            ("construction coefficient algebra work", exact.0),
            ("construction coefficient exponent-entry work", exact.1),
            ("construction coefficient integer-bit work", exact.2),
        ] {
            let mut limits = default_limits;
            match resource {
                "construction coefficient algebra work" => {
                    limits.max_construction_coefficient_algebra_work = exact_value - 1;
                }
                "construction coefficient exponent-entry work" => {
                    limits.max_construction_coefficient_exponent_entry_work = exact_value - 1;
                }
                "construction coefficient integer-bit work" => {
                    limits.max_construction_coefficient_integer_bit_work = exact_value - 1;
                }
                _ => unreachable!(),
            }
            let mut work = WorkBudget::construction(limits, 0);
            let source = recurrence(&context, "family", "source-layout-one-below");
            assert!(matches!(
                validate_source_rows(&context, &[source], limits, &mut work),
                Err(ParametricEliminationError::ResourceLimit {
                    resource: actual_resource,
                    requested,
                    limit,
                }) if actual_resource == resource
                    && requested == exact_value
                    && limit + 1 == exact_value
            ));
        }

        let valid = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let mut valid_work = WorkBudget::construction(default_limits, 0);
        valid_work
            .authenticate_coefficient(&context, &valid, default_limits.arithmetic.exact_algebra)
            .unwrap();
        let valid_exact = (
            valid_work.coefficient_algebra_work,
            valid_work.coefficient_exponent_entry_work,
            valid_work.coefficient_integer_bit_work,
        );
        let mut malformed = valid;
        let variables = malformed.raw().numerator.variables.len();
        let last = malformed.raw().numerator.exponents.len() - 1;
        // Duplicate the later monomial onto the earlier one while preserving
        // the nonzero support of the valid source.  Mutating the later
        // monomial instead would collapse this two-term fixture onto the
        // all-zero sentinel path, whose degree census is intentionally
        // smaller and therefore cannot exercise the same exact boundary.
        let duplicate = malformed.raw().numerator.exponents[last];
        malformed.overwrite_numerator_exponent_for_test(last - variables, duplicate);

        for (resource, exact_value) in [
            ("construction coefficient algebra work", valid_exact.0),
            (
                "construction coefficient exponent-entry work",
                valid_exact.1,
            ),
            ("construction coefficient integer-bit work", valid_exact.2),
        ] {
            let mut admitted = default_limits;
            let mut rejected = default_limits;
            match resource {
                "construction coefficient algebra work" => {
                    admitted.max_construction_coefficient_algebra_work = exact_value;
                    rejected.max_construction_coefficient_algebra_work = exact_value - 1;
                }
                "construction coefficient exponent-entry work" => {
                    admitted.max_construction_coefficient_exponent_entry_work = exact_value;
                    rejected.max_construction_coefficient_exponent_entry_work = exact_value - 1;
                }
                "construction coefficient integer-bit work" => {
                    admitted.max_construction_coefficient_integer_bit_work = exact_value;
                    rejected.max_construction_coefficient_integer_bit_work = exact_value - 1;
                }
                _ => unreachable!(),
            }
            let mut admitted_work = WorkBudget::construction(admitted, 0);
            assert!(matches!(
                admitted_work.authenticate_coefficient(
                    &context,
                    &malformed,
                    admitted.arithmetic.exact_algebra,
                ),
                Err(ParametricEliminationError::Coefficient(
                    crate::ParametricCoefficientError::ExactAlgebra(
                        ExactAlgebraError::NonCanonicalMonomialOrder { .. }
                    )
                ))
            ));
            let mut rejected_work = WorkBudget::construction(rejected, 0);
            assert!(matches!(
                rejected_work.authenticate_coefficient(
                    &context,
                    &malformed,
                    rejected.arithmetic.exact_algebra,
                ),
                Err(ParametricEliminationError::ResourceLimit {
                    resource: actual_resource,
                    requested,
                    limit,
                }) if actual_resource == resource
                    && requested == exact_value
                    && limit + 1 == exact_value
            ));
        }
    }

    #[test]
    fn primitive_sparse_traversal_charges_include_order_clone_equality_and_degree_passes() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "primitive-work-formulas", 2).unwrap();
        let x = context.index(0).unwrap();
        let y = context.index(1).unwrap();
        let x2 = context.mul(&x, &x).unwrap();
        let xy = context.mul(&x, &y).unwrap();
        let y2 = context.mul(&y, &y).unwrap();
        let polynomial_coefficient = context.add(&context.add(&x2, &xy).unwrap(), &y2).unwrap();
        let polynomial = context
            .numerator_condition(&polynomial_coefficient)
            .unwrap();

        let mut work = WorkBudget::construction(ParametricEliminationLimits::default(), 0);
        let shape = work.polynomial_shape(polynomial.raw()).unwrap();
        assert_eq!(shape.terms, 3);
        assert_eq!(shape.exponent_entries, shape.terms * shape.variable_count);

        let before = (
            work.coefficient_algebra_work,
            work.coefficient_exponent_entry_work,
            work.coefficient_integer_bit_work,
        );
        work.charge_polynomial_validation(&shape).unwrap();
        assert_eq!(work.coefficient_algebra_work - before.0, shape.terms);
        assert_eq!(
            work.coefficient_exponent_entry_work - before.1,
            shape.exponent_entries + (shape.terms - 1) * shape.variable_count
        );
        assert_eq!(
            work.coefficient_integer_bit_work - before.2,
            shape.total_integer_bits
        );

        let before = (
            work.coefficient_algebra_work,
            work.coefficient_exponent_entry_work,
            work.coefficient_integer_bit_work,
        );
        work.charge_polynomial_clone(&shape).unwrap();
        assert_eq!(work.coefficient_algebra_work - before.0, shape.terms);
        assert_eq!(
            work.coefficient_exponent_entry_work - before.1,
            shape.exponent_entries
        );
        assert_eq!(
            work.coefficient_integer_bit_work - before.2,
            shape.total_integer_bits
        );

        let before = (
            work.coefficient_algebra_work,
            work.coefficient_exponent_entry_work,
            work.coefficient_integer_bit_work,
        );
        work.charge_polynomial_equality(&shape, &shape).unwrap();
        assert_eq!(work.coefficient_algebra_work - before.0, 2 * shape.terms);
        assert_eq!(
            work.coefficient_exponent_entry_work - before.1,
            2 * shape.exponent_entries
        );
        assert_eq!(
            work.coefficient_integer_bit_work - before.2,
            2 * shape.total_integer_bits
        );

        let coefficient_shape = work.coefficient_shape(&polynomial_coefficient).unwrap();
        let before_exponents = work.coefficient_exponent_entry_work;
        work.charge_coefficient_degree_scan(&coefficient_shape)
            .unwrap();
        assert_eq!(
            work.coefficient_exponent_entry_work - before_exponents,
            coefficient_shape.total_exponent_entries().unwrap()
        );
    }

    #[test]
    fn symbolica_rational_binary_paths_charge_equal_denominator_gcd_and_clone_deltas() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "symbolica-binary-path-work", 2).unwrap();
        let x = context.index(0).unwrap();
        let y = context.index(1).unwrap();
        let denominator = context.add(&y, &context.one()).unwrap();
        let left = context.checked_div(&x, &denominator).unwrap();
        let right = context.checked_div(&context.one(), &denominator).unwrap();
        assert_eq!(left.raw().denominator, right.raw().denominator);
        assert!(left.raw().denominator.nterms() > 1);

        let limits = ParametricEliminationLimits::default();
        let mut shape_work = WorkBudget::construction(limits, 0);
        let left_shape = shape_work.coefficient_shape(&left).unwrap();
        let right_shape = shape_work.coefficient_shape(&right).unwrap();

        let run = |operation, left_shape, right_shape, equal_denominator| {
            let mut work = WorkBudget::construction(limits, 0);
            work.charge_checked_binary_from_shapes(
                operation,
                left_shape,
                right_shape,
                equal_denominator,
            )
            .unwrap();
            coefficient_work_counters(&work)
        };

        let addition = run(CoefficientOperation::Add, &left_shape, &right_shape, true);
        let subtraction = run(
            CoefficientOperation::Subtract,
            &left_shape,
            &right_shape,
            true,
        );
        let subtraction_clone_and_negation = add_counter_tuples(
            coefficient_envelope_counters(&right_shape),
            polynomial_envelope_counters(&right_shape.numerator),
        );
        assert_eq!(
            counter_difference(subtraction, addition),
            subtraction_clone_and_negation,
            "Symbolica subtraction must charge a full RHS clone plus numerator negation"
        );

        // Even the equal-denominator path enters Symbolica's denominator GCD.
        // Its two inputs, materialized output, and `is_one` scan alone require
        // four complete denominator traversals. The complete addition census
        // must strictly exceed this independent mandatory lower bound because
        // it also includes denominator reductions, numerator products, the
        // denominator product, and final cancellation.
        let denominator_gcd_floor =
            scale_counter_tuple(polynomial_envelope_counters(&left_shape.denominator), 4);
        assert!(addition.0 > denominator_gcd_floor.0);
        assert!(addition.1 > denominator_gcd_floor.1);
        assert!(addition.2 > denominator_gcd_floor.2);

        let mut inversion_copy_work = WorkBudget::construction(limits, 0);
        let inverted_right = CoefficientWorkShape {
            numerator: copy_polynomial_work_shape(
                &mut inversion_copy_work,
                &right_shape.denominator,
            )
            .unwrap(),
            denominator: copy_polynomial_work_shape(
                &mut inversion_copy_work,
                &right_shape.numerator,
            )
            .unwrap(),
        };
        let inversion_metadata = coefficient_work_counters(&inversion_copy_work);
        let multiplication_by_inverse = run(
            CoefficientOperation::Multiply,
            &left_shape,
            &inverted_right,
            false,
        );
        let division = run(
            CoefficientOperation::Divide,
            &left_shape,
            &right_shape,
            false,
        );
        let division_clone_and_inversion = add_counter_tuples(
            coefficient_envelope_counters(&right_shape),
            add_counter_tuples(
                add_counter_tuples(
                    scale_counter_tuple(polynomial_envelope_counters(&right_shape.numerator), 4),
                    polynomial_envelope_counters(&right_shape.denominator),
                ),
                inversion_metadata,
            ),
        );
        assert_eq!(
            counter_difference(division, multiplication_by_inverse),
            division_clone_and_inversion,
            "Symbolica division must charge the complete RHS clone/inversion before multiply"
        );
    }

    #[test]
    fn internal_context_constant_constructors_have_profile_independent_exact_counters() {
        let base = CoefficientContext::new(["d", "m"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "constant-constructor-work", 3).unwrap();
        let variable_count = context.base().parameter_names().len() + context.index_count();
        assert_eq!(variable_count, 5);

        for (constant, expected) in [
            (ContextConstant::Zero, (3, 3 * variable_count, 3)),
            (ContextConstant::One, (5, 5 * variable_count, 5)),
        ] {
            let mut work = WorkBudget::construction(ParametricEliminationLimits::default(), 0);
            let shape = work
                .charge_context_constant_constructor(&context, constant)
                .unwrap();
            assert_eq!(coefficient_work_counters(&work), expected);
            match constant {
                ContextConstant::Zero => {
                    assert_eq!(shape.numerator.terms, 0);
                    assert_eq!(shape.denominator.terms, 1);
                }
                ContextConstant::One => {
                    assert_eq!(shape.numerator.terms, 1);
                    assert_eq!(shape.denominator.terms, 1);
                }
            }
        }
    }

    #[test]
    fn guarded_pending_duplicate_condition_charges_search_and_debug_merge_equality() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "guarded-duplicate-work", 1).unwrap();
        let n = context.index(0).unwrap();
        let guard = context.add(&n, &context.one()).unwrap();
        let dividend = context.checked_div(&context.one(), &guard).unwrap();
        let divisor = guard;
        let raw_candidates = [
            &dividend.raw().denominator,
            &divisor.raw().denominator,
            &divisor.raw().numerator,
        ];
        assert_eq!(raw_candidates[0], raw_candidates[2]);
        assert!(raw_candidates[1].is_constant());

        let pending = context
            .checked_div_guarded_pending_normalization_with_limits(
                &dividend,
                &divisor,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let guarded = context
            .finish_guarded_division_normalization_with_limits(
                pending,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert_eq!(guarded.nonzero.len(), 1);
        assert_eq!(guarded.nonzero[0].origins().len(), 2);

        let limits = ParametricEliminationLimits::default();
        let mut shape_work = WorkBudget::construction(limits, 0);
        let candidates = [
            shape_work.polynomial_shape(raw_candidates[0]).unwrap(),
            shape_work.polynomial_shape(raw_candidates[1]).unwrap(),
            shape_work.polynomial_shape(raw_candidates[2]).unwrap(),
        ];
        let candidate_refs = [&candidates[0], &candidates[1], &candidates[2]];
        let mut actual = WorkBudget::construction(limits, 0);
        actual
            .charge_guarded_division_condition_candidates(candidate_refs)
            .unwrap();

        let mut expected = WorkBudget::construction(limits, 0);
        for candidate in candidate_refs {
            expected.charge_polynomial_clone(candidate).unwrap();
        }
        for (position, candidate) in candidate_refs.into_iter().enumerate() {
            expected
                .charge_polynomial_shape_envelope(candidate)
                .unwrap();
            if position == 1 {
                expected
                    .charge_polynomial_shape_envelope(candidate)
                    .unwrap();
                continue;
            }
            expected.charge_polynomial_validation(candidate).unwrap();
            expected.charge_polynomial_validation(candidate).unwrap();
            if position == 2 {
                // The insertion search finds the first candidate, then the
                // debug assertion in `merge_origins_from` compares it again.
                expected
                    .charge_polynomial_equality(candidate, candidate_refs[0])
                    .unwrap();
                expected
                    .charge_polynomial_equality(candidate, candidate_refs[0])
                    .unwrap();
            }
        }
        assert_eq!(
            coefficient_work_counters(&actual),
            coefficient_work_counters(&expected)
        );
    }

    #[test]
    fn relation_condition_attachment_charges_first_and_last_duplicate_debug_equality() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "relation-duplicate-work", 2).unwrap();
        let first_coefficient = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let last_coefficient = context
            .add(&context.index(1).unwrap(), &context.one())
            .unwrap();
        let first = context.numerator_condition(&first_coefficient).unwrap();
        let last = context.numerator_condition(&last_coefficient).unwrap();
        assert_ne!(first, last);

        let limits = ParametricEliminationLimits::default();
        for (label, candidate_polynomial) in [("first", &first), ("last", &last)] {
            let matching_position = if label == "first" { 0 } else { 1 };
            assert_eq!(
                candidate_polynomial,
                if matching_position == 0 {
                    &first
                } else {
                    &last
                }
            );

            let mut setup = WorkBudget::construction(limits, 0);
            let mut actual_attached = vec![
                setup.polynomial_shape(first.raw()).unwrap(),
                setup.polynomial_shape(last.raw()).unwrap(),
            ];
            let actual_candidate = setup.polynomial_shape(candidate_polynomial.raw()).unwrap();
            let mut actual = WorkBudget::construction(limits, 0);
            actual
                .charge_condition_attachment(actual_candidate, &mut actual_attached)
                .unwrap();

            let mut expected_setup = WorkBudget::construction(limits, 0);
            let expected_attached = [
                expected_setup.polynomial_shape(first.raw()).unwrap(),
                expected_setup.polynomial_shape(last.raw()).unwrap(),
            ];
            let candidate = expected_setup
                .polynomial_shape(candidate_polynomial.raw())
                .unwrap();
            let mut expected = WorkBudget::construction(limits, 0);
            expected.charge_polynomial_validation(&candidate).unwrap();
            expected.charge_polynomial_validation(&candidate).unwrap();
            expected
                .charge_polynomial_shape_envelope(&candidate)
                .unwrap();
            for existing in &expected_attached {
                expected
                    .charge_polynomial_equality(&candidate, existing)
                    .unwrap();
            }
            expected.charge_polynomial_clone(&candidate).unwrap();
            for existing in &expected_attached {
                expected
                    .charge_polynomial_equality(&candidate, existing)
                    .unwrap();
            }
            for existing in &expected_attached {
                expected
                    .charge_polynomial_equality(&candidate, existing)
                    .unwrap();
            }
            assert_eq!(
                coefficient_work_counters(&actual),
                coefficient_work_counters(&expected),
                "{label} duplicate must include the merge debug equality"
            );
        }
    }

    #[test]
    fn dense_multivariate_and_final_normalization_envelopes_cover_real_quotients() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "dense-final-normalization", 2).unwrap();
        let x = context.index(0).unwrap();
        let y = context.index(1).unwrap();
        let x_plus_y = context.add(&x, &y).unwrap();
        let x_minus_y = context.sub(&x, &y).unwrap();
        let x8 = coefficient_power(&context, &x, 8);
        let y8 = coefficient_power(&context, &y, 8);
        let sparse = context.sub(&x8, &y8).unwrap();

        let limits = ParametricEliminationLimits::default();
        let mut work = WorkBudget::construction(limits, 0);
        let sparse_shape = work.coefficient_shape(&sparse).unwrap();
        let divisor_shape = work.coefficient_shape(&x_minus_y).unwrap();
        let quotient_envelope = coefficient_operation_estimate_from_shapes(
            &mut work,
            CoefficientOperation::Divide,
            &sparse_shape,
            &divisor_shape,
            sparse.raw().denominator == x_minus_y.raw().denominator,
            limits.arithmetic.exact_algebra.max_polynomial_terms,
        )
        .unwrap();
        let dense_quotient = context.checked_div(&sparse, &x_minus_y).unwrap();
        assert_eq!(dense_quotient.raw().numerator.nterms(), 8);
        let dense_shape = work.coefficient_shape(&dense_quotient).unwrap();
        assert_shape_fits(&dense_shape, &quotient_envelope.output);

        let two = context.integer(2);
        let content_dividend = context.mul(&two, &x_plus_y).unwrap();
        let content_divisor = context.mul(&two, &x_minus_y).unwrap();
        let negative = context.neg(&x_plus_y).unwrap();
        let x2_minus_y2 = context
            .sub(
                &coefficient_power(&context, &x, 2),
                &coefficient_power(&context, &y, 2),
            )
            .unwrap();
        let cases = [
            ("dense", sparse, x_minus_y.clone()),
            ("integer-content", content_dividend, content_divisor),
            ("signed-leading-term", negative, x_plus_y.clone()),
            ("first-quotient-already-cancelled", x2_minus_y2, x_minus_y),
        ];
        for (label, dividend, divisor) in cases {
            let pending = context
                .checked_div_guarded_pending_normalization_with_limits(
                    &dividend,
                    &divisor,
                    limits.arithmetic.exact_algebra,
                )
                .unwrap();
            let pending_shape = work
                .coefficient_shape(pending.value_before_final_normalization())
                .unwrap();
            let envelope = coefficient_final_normalization_estimate_from_shape(
                &mut work,
                &pending_shape,
                limits.arithmetic.exact_algebra.max_polynomial_terms,
            )
            .unwrap();
            let finished = context
                .finish_guarded_division_normalization_with_limits(
                    pending,
                    limits.arithmetic.exact_algebra,
                )
                .unwrap();
            let actual = work.coefficient_shape(&finished.value).unwrap();
            assert_shape_fits(&actual, &envelope.output);
            context
                .validate_with_limits(&finished.value, limits.arithmetic.exact_algebra)
                .unwrap_or_else(|error| panic!("{label} final normalization failed: {error}"));
        }
    }

    #[test]
    fn source_shape_work_is_rejected_before_manifest_generation() {
        let (_, context) = synthetic_context();
        let source = recurrence(&context, "family", "manifest-precedence-source");
        let ordering =
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap();
        let mut limits = ParametricEliminationLimits::default();
        limits.max_construction_coefficient_exponent_entry_work = 0;
        limits.max_source_manifest_bytes = 0;
        assert!(matches!(
            ParametricElimination::build(&context, &[source.clone()], ordering.clone(), limits),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "construction coefficient exponent-entry work",
                ..
            })
        ));

        let mut elimination = ParametricElimination::build(
            &context,
            &[source.clone()],
            ordering,
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        elimination
            .kernel
            .limits
            .max_replay_coefficient_exponent_entry_work = 0;
        elimination.kernel.limits.max_source_manifest_bytes = 0;
        assert!(matches!(
            elimination.replay(&context, &[source]),
            Err(ParametricEliminationError::ResourceLimit {
                resource: "replay coefficient exponent-entry work",
                ..
            })
        ));
    }
}
