//! Authority-bound public-safe premises for one generated affine case.
//!
//! An actionable inventory case can carry two different kinds of symbolic
//! facts.  Guard conditions and explicit `NonZero` case predicates are valid
//! nonzero premises.  An `EqualZero` case predicate is not: it requires a
//! later affine-equality refinement and must never be laundered into a
//! [`ParametricNonZeroCondition`].  This module makes that boundary explicit.
//!
//! Successful `Ready` certificates own the exact case-authority `Arc` and a
//! first-representative-ordered, category-sensitive associate-deduplicated
//! condition sequence. Base-only assumptions use `Q*`; index-dependent loci
//! use `Q(theta)*`; the classes never cross-merge.
//! The durable provenance of every condition is the single public marker
//! [`GuardOrigin::GeneratedAffineSealedCondition`].  Source work-item
//! locators, V1 guard locators, raw private predicates, and affine geometry do
//! not cross the boundary.  Exact geometry remains transitively owned by the
//! authority and is authenticated componentwise at construction and replay.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseGuardClassSourceView,
    GeneratedAffineResidualCaseSourceLocator, GeneratedAffineResidualCaseSourceRecordView,
    GeneratedAffineResidualCaseSourceView, GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::generated_residual_affine_condition_accumulator::{
    GeneratedResidualAffineConditionAccumulatorLimits,
    GeneratedResidualAffineConditionAccumulatorStats, GeneratedResidualAffineConditionInput,
    GeneratedResidualAffineConditionScope, GeneratedResidualAffineConditionSourceLocator,
    accumulate_generated_residual_affine_conditions,
};
use crate::{
    GuardOrigin, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricNonZeroCondition, ParametricPolynomial, SymbolicPolynomialPredicateKind,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_PREMISES_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-case-premises-v2";

#[cfg(test)]
thread_local! {
    static PREMISE_BOUNDARY_PANIC_FOR_TEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static READY_REPLAY_INPUT_ALLOCATION_ATTEMPTS_FOR_TEST: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn inject_premise_boundary_panic_for_test() {
    PREMISE_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_premise_boundary_panic_for_test() {
    PREMISE_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected generated affine premise boundary panic");
        }
    });
}

#[cfg(test)]
fn reset_ready_replay_input_allocation_attempts_for_test() {
    READY_REPLAY_INPUT_ALLOCATION_ATTEMPTS_FOR_TEST.with(|attempts| attempts.set(0));
}

#[cfg(test)]
fn note_ready_replay_input_allocation_attempt_for_test() {
    READY_REPLAY_INPUT_ALLOCATION_ATTEMPTS_FOR_TEST
        .with(|attempts| attempts.set(attempts.get().saturating_add(1)));
}

#[cfg(test)]
fn ready_replay_input_allocation_attempts_for_test() -> usize {
    READY_REPLAY_INPUT_ALLOCATION_ATTEMPTS_FOR_TEST.with(std::cell::Cell::get)
}

/// Aggregate construction and replay envelope.
///
/// `condition_accumulation` bounds the complete equality/category-sensitive
/// associate proof used to merge duplicate zero loci. The surrounding fields bound the authority
/// navigation, source scan, durable public projection, and overlap with that
/// temporary canonical table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePremisesLimits {
    pub(crate) condition_accumulation: GeneratedResidualAffineConditionAccumulatorLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_authority_allocation_comparisons: usize,
    pub(crate) max_authority_replays: usize,
    pub(crate) max_case_lookups: usize,
    pub(crate) max_group_lookups: usize,
    pub(crate) max_geometry_shape_comparisons: usize,
    pub(crate) max_geometry_component_comparisons: usize,
    pub(crate) max_geometry_integer_bits: usize,
    pub(crate) max_guard_scans: usize,
    pub(crate) max_predicate_scans: usize,
    pub(crate) max_condition_polynomials: usize,
    pub(crate) max_nonzero_condition_inputs: usize,
    pub(crate) max_equality_predicates: usize,
    pub(crate) max_input_polynomial_terms: usize,
    pub(crate) max_input_polynomial_exponent_entries: usize,
    pub(crate) max_input_polynomial_integer_bits: usize,
    pub(crate) max_retained_conditions: usize,
    pub(crate) max_retained_origins: usize,
    pub(crate) max_retained_polynomial_terms: usize,
    pub(crate) max_retained_polynomial_exponent_entries: usize,
    pub(crate) max_retained_polynomial_integer_bits: usize,
    pub(crate) max_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualCasePremisesLimits {
    fn default() -> Self {
        Self {
            condition_accumulation: GeneratedResidualAffineConditionAccumulatorLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_authority_allocation_comparisons: 1,
            max_authority_replays: 1,
            max_case_lookups: 1,
            max_group_lookups: 1,
            max_geometry_shape_comparisons: 16,
            max_geometry_component_comparisons: 1_000_000_000,
            max_geometry_integer_bits: portable_usize(16_000_000_000_000_000),
            max_guard_scans: 192_000_000,
            max_predicate_scans: 192_000_000,
            max_condition_polynomials: 384_000_000,
            max_nonzero_condition_inputs: 384_000_000,
            max_equality_predicates: 192_000_000,
            max_input_polynomial_terms: 2_000_000_000,
            max_input_polynomial_exponent_entries: portable_usize(64_000_000_000),
            max_input_polynomial_integer_bits: portable_usize(16_000_000_000_000_000),
            max_retained_conditions: 192_000_000,
            max_retained_origins: 192_000_000,
            max_retained_polynomial_terms: 2_000_000_000,
            max_retained_polynomial_exponent_entries: portable_usize(64_000_000_000),
            max_retained_polynomial_integer_bits: portable_usize(16_000_000_000_000_000),
            max_retained_bytes: portable_usize(32 * 1024 * 1024 * 1024),
            max_peak_scratch_bytes: portable_usize(64 * 1024 * 1024 * 1024),
        }
    }
}

/// Exact observed construction census.  The retained and peak byte fields are
/// conservative allocation-independent envelopes; all source payload counts
/// and child-canonicalizer counters are exact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePremisesStats {
    scope_comparison_bytes: usize,
    authority_allocation_comparisons: usize,
    authority_replays: usize,
    case_lookups: usize,
    group_lookups: usize,
    geometry_shape_comparisons: usize,
    geometry_component_comparisons: usize,
    geometry_integer_bits: usize,
    guard_scans: usize,
    predicate_scans: usize,
    condition_polynomials: usize,
    nonzero_condition_inputs: usize,
    equality_predicates: usize,
    input_polynomial_terms: usize,
    input_polynomial_exponent_entries: usize,
    input_polynomial_integer_bits: usize,
    retained_conditions: usize,
    retained_origins: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
    peak_scratch_byte_envelope: usize,
    condition_accumulation: GeneratedResidualAffineConditionAccumulatorStats,
}

macro_rules! premise_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCasePremisesStats {
    premise_stats_getters!(
        scope_comparison_bytes,
        authority_allocation_comparisons,
        authority_replays,
        case_lookups,
        group_lookups,
        geometry_shape_comparisons,
        geometry_component_comparisons,
        geometry_integer_bits,
        guard_scans,
        predicate_scans,
        condition_polynomials,
        nonzero_condition_inputs,
        equality_predicates,
        input_polynomial_terms,
        input_polynomial_exponent_entries,
        input_polynomial_integer_bits,
        retained_conditions,
        retained_origins,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        retained_byte_envelope,
        retained_bytes,
        peak_scratch_byte_envelope,
    );

    pub(crate) const fn condition_accumulation(
        self,
    ) -> GeneratedResidualAffineConditionAccumulatorStats {
        self.condition_accumulation
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCasePremisesError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongAuthorityAllocation,
    WrongCaseBinding,
    WrongGroupBinding,
    SourceBinding,
    ActionableGuardContradiction {
        guard_ordinal: usize,
    },
    ExpectedReady,
    ExpectedEqualityRefinement,
    PremiseMismatch,
    EqualityPredicateMismatch,
    ConditionCanonicalization,
    ConditionMaterialization,
    AllocationFailure {
        resource: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    RetainedByteEnvelopeExceeded,
    SymbolicaPanic,
}

impl GeneratedAffineResidualCasePremisesError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongAuthorityAllocation => "WrongAuthorityAllocation",
            Self::WrongCaseBinding => "WrongCaseBinding",
            Self::WrongGroupBinding => "WrongGroupBinding",
            Self::SourceBinding => "SourceBinding",
            Self::ActionableGuardContradiction { .. } => "ActionableGuardContradiction",
            Self::ExpectedReady => "ExpectedReady",
            Self::ExpectedEqualityRefinement => "ExpectedEqualityRefinement",
            Self::PremiseMismatch => "PremiseMismatch",
            Self::EqualityPredicateMismatch => "EqualityPredicateMismatch",
            Self::ConditionCanonicalization => "ConditionCanonicalization",
            Self::ConditionMaterialization => "ConditionMaterialization",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::RetainedByteEnvelopeExceeded => "RetainedByteEnvelopeExceeded",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCasePremisesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePremisesError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCasePremisesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("generated affine premise schema mismatch"),
            Self::WrongFamily => formatter.write_str("generated affine premise family mismatch"),
            Self::WrongContext => formatter.write_str("generated affine premise context mismatch"),
            Self::WrongArity => formatter.write_str("generated affine premise arity mismatch"),
            Self::WrongAuthorityAllocation => {
                formatter.write_str("generated affine premise authority allocation mismatch")
            }
            Self::WrongCaseBinding => {
                formatter.write_str("generated affine premise case binding mismatch")
            }
            Self::WrongGroupBinding => {
                formatter.write_str("generated affine premise group binding mismatch")
            }
            Self::SourceBinding => {
                formatter.write_str("generated affine premise source binding mismatch")
            }
            Self::ActionableGuardContradiction { .. } => formatter
                .write_str("generated affine actionable case contains a contradictory guard"),
            Self::ExpectedReady => formatter
                .write_str("generated affine premise case now requires equality refinement"),
            Self::ExpectedEqualityRefinement => {
                formatter.write_str("generated affine equality-refinement case is now ready")
            }
            Self::PremiseMismatch => {
                formatter.write_str("generated affine premise sequence mismatch")
            }
            Self::EqualityPredicateMismatch => {
                formatter.write_str("generated affine equality predicate sequence mismatch")
            }
            Self::ConditionCanonicalization => {
                formatter.write_str("generated affine premise canonicalization failed")
            }
            Self::ConditionMaterialization => {
                formatter.write_str("generated affine premise materialization failed")
            }
            Self::AllocationFailure { .. } => {
                formatter.write_str("generated affine premise bounded allocation failed")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("generated affine premise resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("generated affine premise resource limit exceeded")
            }
            Self::RetainedByteEnvelopeExceeded => {
                formatter.write_str("generated affine premise retained-byte envelope exceeded")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during generated affine premise operation")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualCasePremisesError {}

/// Metadata copied from the exact authority.  It is deliberately scalar or
/// scope metadata only: raw geometry and source predicates stay behind the
/// authority allocation.
struct CaseBinding {
    family_fingerprint: String,
    context_fingerprint: String,
    sector_bits: Vec<bool>,
    ordering: IntegralOrderingPolicy,
    case_ordinal: usize,
    source_locator: GeneratedAffineResidualCaseSourceLocator,
    group_ordinal: usize,
    ordinal_within_group: usize,
    ambient_arity: usize,
    free_position_count: usize,
    compact_matrix_entries: usize,
}

impl fmt::Debug for CaseBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CaseBinding")
            .field("case_ordinal", &self.case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("ambient_arity", &self.ambient_arity)
            .field("free_position_count", &self.free_position_count)
            .field("private_scope_and_geometry", &"<redacted>")
            .finish()
    }
}

/// A case for which every active condition is legitimately nonzero.
pub(crate) struct GeneratedAffineResidualCasePremisesCertificate {
    schema: &'static str,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    binding: CaseBinding,
    premises: Vec<ParametricNonZeroCondition>,
    limits: GeneratedAffineResidualCasePremisesLimits,
    stats: GeneratedAffineResidualCasePremisesStats,
}

impl fmt::Debug for GeneratedAffineResidualCasePremisesCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePremisesCertificate")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.binding.case_ordinal)
            .field("group_ordinal", &self.binding.group_ordinal)
            .field("premise_count", &self.premises.len())
            .field("private_authority", &"<redacted>")
            .field("private_premises", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCasePremisesCertificate {
    pub(crate) const fn case_ordinal(&self) -> usize {
        self.binding.case_ordinal
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.binding.group_ordinal
    }

    pub(crate) fn premises(&self) -> &[ParametricNonZeroCondition] {
        &self.premises
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCasePremisesLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCasePremisesStats {
        self.stats
    }

    /// Authenticated prospective owner envelope for this certificate and its
    /// private binding/premise payload. The authority pointee and this
    /// certificate's outer `Arc` control block are excluded and belong to the
    /// retaining parent graph's accounting boundary.
    pub(crate) const fn owner_retained_byte_envelope(&self) -> usize {
        self.stats.retained_byte_envelope
    }

    pub(crate) fn same_authority_allocation(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffineResidualCasePremisesError> {
        catch_unwind(AssertUnwindSafe(|| {
            replay_ready_inner(self, family, context, authority)
        }))
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SymbolicaPanic)?
    }
}

/// Typed boundary for an actionable inventory case that still carries one or
/// more equality predicates.  It owns no polynomial and no nonzero condition.
pub(crate) struct GeneratedAffineResidualCaseEqualityRefinementCertificate {
    schema: &'static str,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    binding: CaseBinding,
    equality_predicate_ordinals: Vec<usize>,
    limits: GeneratedAffineResidualCasePremisesLimits,
    stats: GeneratedAffineResidualCasePremisesStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseEqualityRefinementCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseEqualityRefinementCertificate")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.binding.case_ordinal)
            .field("group_ordinal", &self.binding.group_ordinal)
            .field(
                "equality_predicate_count",
                &self.equality_predicate_ordinals.len(),
            )
            .field("private_authority", &"<redacted>")
            .field("private_predicates", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseEqualityRefinementCertificate {
    pub(crate) const fn case_ordinal(&self) -> usize {
        self.binding.case_ordinal
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.binding.group_ordinal
    }

    pub(crate) fn equality_predicate_ordinals(&self) -> &[usize] {
        &self.equality_predicate_ordinals
    }

    #[cfg(test)]
    pub(crate) fn replace_equality_predicate_ordinal_for_test(
        &mut self,
        position: usize,
        source_ordinal: usize,
    ) -> bool {
        let Some(ordinal) = self.equality_predicate_ordinals.get_mut(position) else {
            return false;
        };
        *ordinal = source_ordinal;
        true
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCasePremisesLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCasePremisesStats {
        self.stats
    }

    pub(crate) fn same_authority_allocation(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
    }

    /// Narrow authority borrow for the adjacent unit-refinement owner.  The
    /// equality certificate remains the sole transitive owner; the adapter
    /// neither clones lineage handles nor exposes source payload.
    pub(crate) const fn bound_unit_equality_refinement_authority(
        &self,
    ) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        &self.authority
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffineResidualCasePremisesError> {
        catch_unwind(AssertUnwindSafe(|| {
            replay_equality_inner(self, family, context, authority)
        }))
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SymbolicaPanic)?
    }
}

pub(crate) enum GeneratedAffineResidualCasePremisesOutcome {
    Ready(GeneratedAffineResidualCasePremisesCertificate),
    RequiresAffineEqualityRefinement(GeneratedAffineResidualCaseEqualityRefinementCertificate),
}

impl fmt::Debug for GeneratedAffineResidualCasePremisesOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(certificate) => formatter.debug_tuple("Ready").field(certificate).finish(),
            Self::RequiresAffineEqualityRefinement(certificate) => formatter
                .debug_tuple("RequiresAffineEqualityRefinement")
                .field(certificate)
                .finish(),
        }
    }
}

/// Project one exact actionable inventory case into its public-safe premise
/// outcome.  Every allocation lies inside the panic boundary and follows an
/// allocation-free source census.
pub(crate) fn compile_generated_affine_residual_case_premises(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    limits: GeneratedAffineResidualCasePremisesLimits,
) -> Result<GeneratedAffineResidualCasePremisesOutcome, GeneratedAffineResidualCasePremisesError> {
    catch_unwind(AssertUnwindSafe(|| {
        compile_inner(family, context, authority, limits)
    }))
    .map_err(|_| GeneratedAffineResidualCasePremisesError::SymbolicaPanic)?
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SourceScan {
    guard_scans: usize,
    predicate_scans: usize,
    condition_polynomials: usize,
    nonzero_condition_inputs: usize,
    equality_predicates: usize,
    input_polynomial_terms: usize,
    input_polynomial_exponent_entries: usize,
    input_polynomial_integer_bits: usize,
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    limits: GeneratedAffineResidualCasePremisesLimits,
) -> Result<GeneratedAffineResidualCasePremisesOutcome, GeneratedAffineResidualCasePremisesError> {
    let mut stats = GeneratedAffineResidualCasePremisesStats::default();
    authenticate_scope_and_authority(family, context, authority.as_ref(), limits, &mut stats)?;
    let case = authority
        .authenticated_source_neutral_case_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    stats.case_lookups = bounded_add(
        "case lookups",
        stats.case_lookups,
        1,
        limits.max_case_lookups,
    )?;
    let group = authority
        .authenticated_source_neutral_group_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    stats.group_lookups = bounded_add(
        "group lookups",
        stats.group_lookups,
        1,
        limits.max_group_lookups,
    )?;
    authenticate_case_geometry(authority.as_ref(), case, group, limits, &mut stats)?;
    let source_scan = preflight_source(context, case.source(), limits)?;
    merge_source_scan(&mut stats, source_scan);

    #[cfg(test)]
    maybe_inject_premise_boundary_panic_for_test();

    if source_scan.equality_predicates != 0 {
        return compile_equality_outcome(Arc::clone(&authority), case, group, limits, stats);
    }

    let input_envelope = capacity_byte_envelope::<GeneratedResidualAffineConditionInput<'_>>(
        source_scan.nonzero_condition_inputs,
        "temporary condition inputs",
    )?;
    check_limit(
        "peak scratch bytes",
        input_envelope,
        limits.max_peak_scratch_bytes,
    )?;
    let inputs = collect_nonzero_inputs(case.source(), source_scan.nonzero_condition_inputs)?;
    if inputs.len() != source_scan.nonzero_condition_inputs {
        return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
    }

    let mut accumulator_limits = limits.condition_accumulation;
    accumulator_limits.max_retained_bytes = accumulator_limits.max_retained_bytes.min(
        limits
            .max_peak_scratch_bytes
            .checked_sub(input_envelope)
            .ok_or(GeneratedAffineResidualCasePremisesError::ResourceLimit {
                resource: "peak scratch bytes",
                requested: input_envelope,
                limit: limits.max_peak_scratch_bytes,
            })?,
    );
    let accumulation = accumulate_generated_residual_affine_conditions(
        context,
        group.free_positions(),
        inputs,
        accumulator_limits,
    )
    .map_err(|_| GeneratedAffineResidualCasePremisesError::ConditionCanonicalization)?;
    stats.condition_accumulation = accumulation.stats();
    preflight_retained_rows(context, accumulation.rows(), limits, &mut stats)?;

    let retained_envelope = ready_retained_byte_envelope(
        authority.as_ref(),
        accumulation.rows(),
        stats.retained_conditions,
    )?;
    check_limit(
        "retained bytes",
        retained_envelope,
        limits.max_retained_bytes,
    )?;
    stats.retained_byte_envelope = retained_envelope;
    stats.peak_scratch_byte_envelope = checked_sum(
        "peak scratch bytes",
        [
            input_envelope,
            accumulation.stats().retained_byte_envelope(),
            retained_envelope,
        ],
    )?;
    check_limit(
        "peak scratch bytes",
        stats.peak_scratch_byte_envelope,
        limits.max_peak_scratch_bytes,
    )?;

    let binding = build_binding(authority.as_ref(), case, group)?;
    let premises = materialize_premises(context, accumulation.rows(), limits)?;
    stats.retained_bytes = observed_ready_retained_bytes(&binding, &premises)?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    Ok(GeneratedAffineResidualCasePremisesOutcome::Ready(
        GeneratedAffineResidualCasePremisesCertificate {
            schema: GENERATED_AFFINE_RESIDUAL_CASE_PREMISES_V2_SCHEMA,
            authority,
            binding,
            premises,
            limits,
            stats,
        },
    ))
}

fn compile_equality_outcome(
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineResidualCasePremisesLimits,
    mut stats: GeneratedAffineResidualCasePremisesStats,
) -> Result<GeneratedAffineResidualCasePremisesOutcome, GeneratedAffineResidualCasePremisesError> {
    let equality_envelope =
        capacity_byte_envelope::<usize>(stats.equality_predicates, "equality predicate ordinals")?;
    let retained_envelope = equality_retained_byte_envelope(
        authority.as_ref(),
        stats.equality_predicates,
        equality_envelope,
    )?;
    check_limit(
        "retained bytes",
        retained_envelope,
        limits.max_retained_bytes,
    )?;
    let peak_scratch_byte_envelope =
        checked_add("peak scratch bytes", retained_envelope, equality_envelope)?;
    check_limit(
        "peak scratch bytes",
        peak_scratch_byte_envelope,
        limits.max_peak_scratch_bytes,
    )?;
    stats.retained_byte_envelope = retained_envelope;
    // Replay retains the certificate while rebuilding the bounded ordinal
    // vector, so construction admits that larger overlap as part of the
    // certificate contract too.
    stats.peak_scratch_byte_envelope = peak_scratch_byte_envelope;
    let equality_predicate_ordinals =
        collect_equality_predicate_ordinals(case.source(), stats.equality_predicates)?;
    let binding = build_binding(authority.as_ref(), case, group)?;
    stats.retained_bytes =
        observed_equality_retained_bytes(&binding, &equality_predicate_ordinals)?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    Ok(
        GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
            GeneratedAffineResidualCaseEqualityRefinementCertificate {
                schema: GENERATED_AFFINE_RESIDUAL_CASE_PREMISES_V2_SCHEMA,
                authority,
                binding,
                equality_predicate_ordinals,
                limits,
                stats,
            },
        ),
    )
}

fn authenticate_scope_and_authority(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &GeneratedAffineResidualCaseAuthority,
    limits: GeneratedAffineResidualCasePremisesLimits,
    stats: &mut GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    let family_fingerprint = family.fingerprint_ref();
    stats.scope_comparison_bytes = checked_sum(
        "scope comparison bytes",
        [
            family_fingerprint.len(),
            authority.family_fingerprint().len(),
            context.fingerprint().len(),
            authority.context_fingerprint().len(),
        ],
    )?;
    check_limit(
        "scope comparison bytes",
        stats.scope_comparison_bytes,
        limits.max_scope_comparison_bytes,
    )?;
    if family_fingerprint != authority.family_fingerprint() {
        return Err(GeneratedAffineResidualCasePremisesError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint() {
        return Err(GeneratedAffineResidualCasePremisesError::WrongContext);
    }
    if context.index_count() != authority.arity() || authority.sector().arity() != authority.arity()
    {
        return Err(GeneratedAffineResidualCasePremisesError::WrongArity);
    }
    stats.authority_replays = bounded_add(
        "authority replays",
        stats.authority_replays,
        1,
        limits.max_authority_replays,
    )?;
    authority
        .replay(family, context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)
}

const GEOMETRY_SHAPE_COMPARISONS: usize = 12;

fn authenticate_case_geometry(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineResidualCasePremisesLimits,
    stats: &mut GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    let geometry = case.source().geometry();
    let arity = authority.arity();
    let free_count = group.free_positions().len();
    let compact_entries = checked_mul("compact matrix entries", arity, free_count)?;
    stats.geometry_shape_comparisons = bounded_add(
        "geometry shape comparisons",
        stats.geometry_shape_comparisons,
        GEOMETRY_SHAPE_COMPARISONS,
        limits.max_geometry_shape_comparisons,
    )?;
    stats.geometry_component_comparisons = bounded_add(
        "geometry component comparisons",
        stats.geometry_component_comparisons,
        checked_sum(
            "geometry component comparisons",
            [arity, free_count, compact_entries, 1],
        )?,
        limits.max_geometry_component_comparisons,
    )?;
    if case.ordinal() != authority.case_ordinal()
        || case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
        || group.ambient_arity() != arity
        || geometry.ambient_arity() != arity
        || case.constants().len() != arity
        || group.free_positions().len() != geometry.free_positions().len()
        || group.compact_linear_coefficients().len() != compact_entries
        || group.anchor_offsets().len() != group.case_ordinals().len()
        || group
            .case_ordinals()
            .get(case.ordinal_within_group())
            .copied()
            != Some(case.ordinal())
        || group.free_positions() != geometry.free_positions()
    {
        return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
    }
    let mut integer_bits = 0usize;
    for position in 0..arity {
        let constant = geometry
            .constant(position)
            .ok_or(GeneratedAffineResidualCasePremisesError::SourceBinding)?;
        if case.constants().get(position) != Some(constant) {
            return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
        }
        integer_bits = bounded_add(
            "geometry integer bits",
            integer_bits,
            integer_magnitude_bits(constant)?,
            limits.max_geometry_integer_bits,
        )?;
    }
    let mut compact_position = 0usize;
    for row in 0..arity {
        for free_ordinal in 0..group.free_positions().len() {
            let coefficient = geometry
                .compact_linear_coefficient(row, free_ordinal)
                .ok_or(GeneratedAffineResidualCasePremisesError::SourceBinding)?;
            if group.compact_linear_coefficients().get(compact_position) != Some(coefficient) {
                return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
            }
            integer_bits = bounded_add(
                "geometry integer bits",
                integer_bits,
                integer_magnitude_bits(coefficient)?,
                limits.max_geometry_integer_bits,
            )?;
            compact_position = checked_add("compact matrix position", compact_position, 1)?;
        }
    }
    stats.geometry_integer_bits = integer_bits;
    Ok(())
}

fn preflight_source(
    context: &ParametricCoefficientContext,
    source: GeneratedAffineResidualCaseSourceView<'_>,
    limits: GeneratedAffineResidualCasePremisesLimits,
) -> Result<SourceScan, GeneratedAffineResidualCasePremisesError> {
    let mut scan = SourceScan {
        guard_scans: source.guard_count(),
        predicate_scans: source.exceptional_predicate_count(),
        ..SourceScan::default()
    };
    check_limit("guard scans", scan.guard_scans, limits.max_guard_scans)?;
    check_limit(
        "predicate scans",
        scan.predicate_scans,
        limits.max_predicate_scans,
    )?;

    visit_guard_conditions(source, |guard_ordinal, _, polynomial| {
        let Some(polynomial) = polynomial else {
            return Err(
                GeneratedAffineResidualCasePremisesError::ActionableGuardContradiction {
                    guard_ordinal,
                },
            );
        };
        scan.nonzero_condition_inputs = bounded_add(
            "nonzero condition inputs",
            scan.nonzero_condition_inputs,
            1,
            limits.max_nonzero_condition_inputs,
        )?;
        charge_polynomial(context, polynomial, limits, &mut scan)
    })?;
    visit_exceptional_predicates(source, |_, _, kind, polynomial| {
        match kind {
            SymbolicPolynomialPredicateKind::EqualZero => {
                scan.equality_predicates = bounded_add(
                    "equality predicates",
                    scan.equality_predicates,
                    1,
                    limits.max_equality_predicates,
                )?;
            }
            SymbolicPolynomialPredicateKind::NonZero => {
                scan.nonzero_condition_inputs = bounded_add(
                    "nonzero condition inputs",
                    scan.nonzero_condition_inputs,
                    1,
                    limits.max_nonzero_condition_inputs,
                )?;
            }
        }
        charge_polynomial(context, polynomial, limits, &mut scan)
    })?;
    Ok(scan)
}

fn charge_polynomial(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    limits: GeneratedAffineResidualCasePremisesLimits,
    scan: &mut SourceScan,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    scan.condition_polynomials = bounded_add(
        "condition polynomials",
        scan.condition_polynomials,
        1,
        limits.max_condition_polynomials,
    )?;
    let census = context
        .preflight_polynomial_validation_payload_with_limits(
            polynomial,
            limits.condition_accumulation.exact_algebra,
            remaining(
                "input polynomial terms",
                limits.max_input_polynomial_terms,
                scan.input_polynomial_terms,
            )?,
            remaining(
                "input polynomial exponent entries",
                limits.max_input_polynomial_exponent_entries,
                scan.input_polynomial_exponent_entries,
            )?,
            remaining(
                "input polynomial integer bits",
                limits.max_input_polynomial_integer_bits,
                scan.input_polynomial_integer_bits,
            )?,
        )
        .map_err(|_| GeneratedAffineResidualCasePremisesError::ConditionMaterialization)?;
    scan.input_polynomial_terms = checked_add(
        "input polynomial terms",
        scan.input_polynomial_terms,
        census.source_terms(),
    )?;
    scan.input_polynomial_exponent_entries = checked_add(
        "input polynomial exponent entries",
        scan.input_polynomial_exponent_entries,
        census.source_exponent_entries(),
    )?;
    scan.input_polynomial_integer_bits = checked_add(
        "input polynomial integer bits",
        scan.input_polynomial_integer_bits,
        census.source_integer_bits(),
    )?;
    Ok(())
}

fn merge_source_scan(stats: &mut GeneratedAffineResidualCasePremisesStats, scan: SourceScan) {
    stats.guard_scans = scan.guard_scans;
    stats.predicate_scans = scan.predicate_scans;
    stats.condition_polynomials = scan.condition_polynomials;
    stats.nonzero_condition_inputs = scan.nonzero_condition_inputs;
    stats.equality_predicates = scan.equality_predicates;
    stats.input_polynomial_terms = scan.input_polynomial_terms;
    stats.input_polynomial_exponent_entries = scan.input_polynomial_exponent_entries;
    stats.input_polynomial_integer_bits = scan.input_polynomial_integer_bits;
}

/// Visit symbolic guard conditions in positional order.  A `None` polynomial
/// identifies a contradiction; discharged nonzero integer constants are not
/// visited because they need no durable premise.
fn visit_guard_conditions<'source>(
    source: GeneratedAffineResidualCaseSourceView<'source>,
    mut visit: impl FnMut(
        usize,
        usize,
        Option<&'source ParametricPolynomial>,
    ) -> Result<(), GeneratedAffineResidualCasePremisesError>,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    for guard_ordinal in 0..source.guard_count() {
        let entry = source
            .guard_entry(guard_ordinal)
            .ok_or(GeneratedAffineResidualCasePremisesError::SourceBinding)?;
        if entry.entry_ordinal() != guard_ordinal {
            return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
        }
        match entry.class() {
            GeneratedAffineResidualCaseGuardClassSourceView::Contradiction => {
                visit(guard_ordinal, entry.structural_locus_ordinal(), None)?
            }
            GeneratedAffineResidualCaseGuardClassSourceView::DischargedNonzeroIntegerConstant => {}
            GeneratedAffineResidualCaseGuardClassSourceView::BaseAssumption(polynomial)
            | GeneratedAffineResidualCaseGuardClassSourceView::FreeIndexDependent(polynomial) => {
                visit(
                    guard_ordinal,
                    entry.structural_locus_ordinal(),
                    Some(polynomial),
                )?
            }
        }
    }
    Ok(())
}

fn visit_exceptional_predicates<'source>(
    source: GeneratedAffineResidualCaseSourceView<'source>,
    mut visit: impl FnMut(
        usize,
        usize,
        SymbolicPolynomialPredicateKind,
        &'source ParametricPolynomial,
    ) -> Result<(), GeneratedAffineResidualCasePremisesError>,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    for predicate_ordinal in 0..source.exceptional_predicate_count() {
        let predicate = source
            .exceptional_predicate(predicate_ordinal)
            .ok_or(GeneratedAffineResidualCasePremisesError::SourceBinding)?;
        if predicate.predicate_ordinal() != predicate_ordinal {
            return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
        }
        visit(
            predicate_ordinal,
            predicate.locus_ordinal(),
            predicate.kind(),
            predicate.polynomial(),
        )?;
    }
    Ok(())
}

fn collect_nonzero_inputs<'source>(
    source: GeneratedAffineResidualCaseSourceView<'source>,
    expected: usize,
) -> Result<
    Vec<GeneratedResidualAffineConditionInput<'source>>,
    GeneratedAffineResidualCasePremisesError,
> {
    let mut inputs = Vec::new();
    inputs.try_reserve_exact(expected).map_err(|_| {
        GeneratedAffineResidualCasePremisesError::AllocationFailure {
            resource: "temporary condition inputs",
        }
    })?;
    let mut encounter_ordinal = 0usize;
    visit_guard_conditions(source, |_, structural_locus_ordinal, polynomial| {
        let polynomial =
            polynomial.ok_or(GeneratedAffineResidualCasePremisesError::SourceBinding)?;
        inputs.push(GeneratedResidualAffineConditionInput::new(
            polynomial,
            GeneratedResidualAffineConditionScope::InheritedTargetPremise,
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal: encounter_ordinal,
                structural_locus_ordinal,
            },
            None,
        ));
        encounter_ordinal =
            checked_add("nonzero condition encounter ordinal", encounter_ordinal, 1)?;
        Ok(())
    })?;
    visit_exceptional_predicates(source, |_, locus, kind, polynomial| {
        if kind == SymbolicPolynomialPredicateKind::NonZero {
            inputs.push(GeneratedResidualAffineConditionInput::new(
                polynomial,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: encounter_ordinal,
                    structural_locus_ordinal: locus,
                },
                None,
            ));
            encounter_ordinal =
                checked_add("nonzero condition encounter ordinal", encounter_ordinal, 1)?;
        }
        Ok(())
    })?;
    if inputs.len() != expected {
        return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
    }
    Ok(inputs)
}

fn collect_equality_predicate_ordinals(
    source: GeneratedAffineResidualCaseSourceView<'_>,
    expected: usize,
) -> Result<Vec<usize>, GeneratedAffineResidualCasePremisesError> {
    let mut ordinals = Vec::new();
    ordinals.try_reserve_exact(expected).map_err(|_| {
        GeneratedAffineResidualCasePremisesError::AllocationFailure {
            resource: "equality predicate ordinals",
        }
    })?;
    visit_exceptional_predicates(source, |predicate_ordinal, _, kind, _| {
        if kind == SymbolicPolynomialPredicateKind::EqualZero {
            ordinals.push(predicate_ordinal);
        }
        Ok(())
    })?;
    if ordinals.len() != expected {
        return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
    }
    Ok(ordinals)
}

fn authenticate_equality_predicate_ordinals(
    source: GeneratedAffineResidualCaseSourceView<'_>,
    expected: &[usize],
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    let mut equality_position = 0usize;
    visit_exceptional_predicates(source, |predicate_ordinal, _, kind, _| {
        if kind == SymbolicPolynomialPredicateKind::EqualZero {
            if expected.get(equality_position).copied() != Some(predicate_ordinal) {
                return Err(GeneratedAffineResidualCasePremisesError::EqualityPredicateMismatch);
            }
            equality_position = checked_add(
                "equality predicate authentication position",
                equality_position,
                1,
            )?;
        }
        Ok(())
    })?;
    if equality_position != expected.len() {
        return Err(GeneratedAffineResidualCasePremisesError::EqualityPredicateMismatch);
    }
    Ok(())
}

fn preflight_retained_rows(
    context: &ParametricCoefficientContext,
    rows: &[crate::generated_residual_affine_condition_accumulator::GeneratedResidualAffineCanonicalConditionRow],
    limits: GeneratedAffineResidualCasePremisesLimits,
    stats: &mut GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    stats.retained_conditions = rows.len();
    check_limit(
        "retained conditions",
        stats.retained_conditions,
        limits.max_retained_conditions,
    )?;
    stats.retained_origins = rows.len();
    check_limit(
        "retained origins",
        stats.retained_origins,
        limits.max_retained_origins,
    )?;
    for row in rows {
        let census = context
            .preflight_polynomial_validation_payload_with_limits(
                row.polynomial(),
                limits.condition_accumulation.exact_algebra,
                remaining(
                    "retained polynomial terms",
                    limits.max_retained_polynomial_terms,
                    stats.retained_polynomial_terms,
                )?,
                remaining(
                    "retained polynomial exponent entries",
                    limits.max_retained_polynomial_exponent_entries,
                    stats.retained_polynomial_exponent_entries,
                )?,
                remaining(
                    "retained polynomial integer bits",
                    limits.max_retained_polynomial_integer_bits,
                    stats.retained_polynomial_integer_bits,
                )?,
            )
            .map_err(|_| GeneratedAffineResidualCasePremisesError::ConditionMaterialization)?;
        stats.retained_polynomial_terms = checked_add(
            "retained polynomial terms",
            stats.retained_polynomial_terms,
            census.source_terms(),
        )?;
        stats.retained_polynomial_exponent_entries = checked_add(
            "retained polynomial exponent entries",
            stats.retained_polynomial_exponent_entries,
            census.source_exponent_entries(),
        )?;
        stats.retained_polynomial_integer_bits = checked_add(
            "retained polynomial integer bits",
            stats.retained_polynomial_integer_bits,
            census.source_integer_bits(),
        )?;
    }
    Ok(())
}

fn materialize_premises(
    context: &ParametricCoefficientContext,
    rows: &[crate::generated_residual_affine_condition_accumulator::GeneratedResidualAffineCanonicalConditionRow],
    limits: GeneratedAffineResidualCasePremisesLimits,
) -> Result<Vec<ParametricNonZeroCondition>, GeneratedAffineResidualCasePremisesError> {
    let mut premises = Vec::new();
    premises.try_reserve_exact(rows.len()).map_err(|_| {
        GeneratedAffineResidualCasePremisesError::AllocationFailure {
            resource: "sealed generated affine premises",
        }
    })?;
    for row in rows {
        let polynomial = row
            .polynomial()
            .try_copy_authenticated_sparse_payload()
            .map_err(
                |_| GeneratedAffineResidualCasePremisesError::AllocationFailure {
                    resource: "sealed premise polynomial",
                },
            )?;
        let condition = context
            .nonzero_condition_with_origins_and_origin_limit(
                polynomial,
                [GuardOrigin::GeneratedAffineSealedCondition],
                limits.condition_accumulation.exact_algebra,
                1,
            )
            .map_err(|_| GeneratedAffineResidualCasePremisesError::ConditionMaterialization)?;
        if condition.origins().len() != 1
            || !condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        {
            return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
        }
        premises.push(condition);
    }
    Ok(premises)
}

fn build_binding(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<CaseBinding, GeneratedAffineResidualCasePremisesError> {
    Ok(CaseBinding {
        family_fingerprint: try_copy_string(authority.family_fingerprint(), "family fingerprint")?,
        context_fingerprint: try_copy_string(
            authority.context_fingerprint(),
            "context fingerprint",
        )?,
        sector_bits: try_copy_bool_slice(authority.sector().active_bits(), "sector bits")?,
        ordering: authority.ordering(),
        case_ordinal: case.ordinal(),
        source_locator: case.locator(),
        group_ordinal: case.group_ordinal(),
        ordinal_within_group: case.ordinal_within_group(),
        ambient_arity: group.ambient_arity(),
        free_position_count: group.free_positions().len(),
        compact_matrix_entries: group.compact_linear_coefficients().len(),
    })
}

fn binding_dynamic_byte_envelope(
    authority: &GeneratedAffineResidualCaseAuthority,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    checked_sum(
        "retained bytes",
        [
            capacity_byte_envelope::<u8>(
                authority.family_fingerprint().len(),
                "family fingerprint",
            )?,
            capacity_byte_envelope::<u8>(
                authority.context_fingerprint().len(),
                "context fingerprint",
            )?,
            capacity_byte_envelope::<bool>(authority.sector().arity(), "sector bits")?,
        ],
    )
}

fn ready_retained_byte_envelope(
    authority: &GeneratedAffineResidualCaseAuthority,
    rows: &[crate::generated_residual_affine_condition_accumulator::GeneratedResidualAffineCanonicalConditionRow],
    retained_conditions: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    let mut bytes = checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCasePremisesCertificate>(),
            binding_dynamic_byte_envelope(authority)?,
            capacity_byte_envelope::<ParametricNonZeroCondition>(
                retained_conditions,
                "sealed premises",
            )?,
        ],
    )?;
    let origin_bytes = GuardOrigin::GeneratedAffineSealedCondition
        .retained_byte_bound()
        .ok_or(
            GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                resource: "retained origin bytes",
            },
        )?;
    for row in rows {
        let polynomial_bytes = row.polynomial().owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                resource: "retained polynomial bytes",
            },
        )?;
        bytes = checked_sum("retained bytes", [bytes, polynomial_bytes, origin_bytes])?;
    }
    Ok(bytes)
}

fn equality_retained_byte_envelope(
    authority: &GeneratedAffineResidualCaseAuthority,
    _equality_predicates: usize,
    equality_envelope: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCaseEqualityRefinementCertificate>(),
            binding_dynamic_byte_envelope(authority)?,
            equality_envelope,
        ],
    )
}

fn observed_binding_dynamic_bytes(
    binding: &CaseBinding,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    checked_sum(
        "retained bytes",
        [
            binding.family_fingerprint.capacity(),
            binding.context_fingerprint.capacity(),
            checked_mul(
                "retained bytes",
                binding.sector_bits.capacity(),
                size_of::<bool>(),
            )?,
        ],
    )
}

fn observed_ready_retained_bytes(
    binding: &CaseBinding,
    premises: &Vec<ParametricNonZeroCondition>,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    let mut bytes = checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCasePremisesCertificate>(),
            observed_binding_dynamic_bytes(binding)?,
            checked_mul(
                "retained bytes",
                premises.capacity(),
                size_of::<ParametricNonZeroCondition>(),
            )?,
        ],
    )?;
    for condition in premises {
        let owned = condition.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                resource: "retained premise bytes",
            },
        )?;
        bytes = checked_add(
            "retained bytes",
            bytes,
            owned
                .checked_sub(size_of::<ParametricNonZeroCondition>())
                .ok_or(
                    GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                        resource: "retained premise bytes",
                    },
                )?,
        )?;
    }
    Ok(bytes)
}

fn ready_certificate_retained_byte_envelope(
    authority: &GeneratedAffineResidualCaseAuthority,
    premises: &[ParametricNonZeroCondition],
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    let mut bytes = checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCasePremisesCertificate>(),
            binding_dynamic_byte_envelope(authority)?,
            capacity_byte_envelope::<ParametricNonZeroCondition>(
                premises.len(),
                "sealed premises",
            )?,
        ],
    )?;
    let origin_bytes = GuardOrigin::GeneratedAffineSealedCondition
        .retained_byte_bound()
        .ok_or(
            GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                resource: "retained origin bytes",
            },
        )?;
    for condition in premises {
        if condition.origins().len() != 1
            || !condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        {
            return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
        }
        let polynomial_bytes = condition.polynomial().owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
                resource: "retained polynomial bytes",
            },
        )?;
        bytes = checked_sum("retained bytes", [bytes, polynomial_bytes, origin_bytes])?;
    }
    Ok(bytes)
}

fn observed_equality_retained_bytes(
    binding: &CaseBinding,
    equality_predicate_ordinals: &Vec<usize>,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCaseEqualityRefinementCertificate>(),
            observed_binding_dynamic_bytes(binding)?,
            checked_mul(
                "retained bytes",
                equality_predicate_ordinals.capacity(),
                size_of::<usize>(),
            )?,
        ],
    )
}

fn replay_ready_inner(
    certificate: &GeneratedAffineResidualCasePremisesCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_CASE_PREMISES_V2_SCHEMA {
        return Err(GeneratedAffineResidualCasePremisesError::SchemaMismatch);
    }
    authenticate_expected_authority(&certificate.authority, authority, certificate.limits)?;
    let mut replay_stats = GeneratedAffineResidualCasePremisesStats {
        authority_allocation_comparisons: 1,
        ..GeneratedAffineResidualCasePremisesStats::default()
    };
    authenticate_scope_and_authority(
        family,
        context,
        authority.as_ref(),
        certificate.limits,
        &mut replay_stats,
    )?;
    let case = authority
        .authenticated_source_neutral_case_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    replay_stats.case_lookups = bounded_add(
        "case lookups",
        replay_stats.case_lookups,
        1,
        certificate.limits.max_case_lookups,
    )?;
    let group = authority
        .authenticated_source_neutral_group_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    replay_stats.group_lookups = bounded_add(
        "group lookups",
        replay_stats.group_lookups,
        1,
        certificate.limits.max_group_lookups,
    )?;
    authenticate_case_geometry(
        authority.as_ref(),
        case,
        group,
        certificate.limits,
        &mut replay_stats,
    )?;
    authenticate_binding(&certificate.binding, authority.as_ref(), case, group)?;
    let source_scan = preflight_source(context, case.source(), certificate.limits)?;
    merge_source_scan(&mut replay_stats, source_scan);
    if source_scan.equality_predicates != 0 {
        return Err(GeneratedAffineResidualCasePremisesError::ExpectedReady);
    }
    authenticate_source_stats(certificate.stats, replay_stats)?;
    authenticate_compile_only_stats(certificate.stats)?;

    let input_envelope = capacity_byte_envelope::<GeneratedResidualAffineConditionInput<'_>>(
        source_scan.nonzero_condition_inputs,
        "temporary condition inputs",
    )?;
    if certificate.premises.len() != certificate.stats.retained_conditions
        || certificate.premises.len() != certificate.stats.retained_origins
    {
        return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
    }
    let retained_envelope =
        ready_certificate_retained_byte_envelope(authority.as_ref(), &certificate.premises)?;
    let retained_bytes =
        observed_ready_retained_bytes(&certificate.binding, &certificate.premises)?;
    check_limit(
        "retained bytes",
        retained_envelope,
        certificate.limits.max_retained_bytes,
    )?;
    if retained_envelope != certificate.stats.retained_byte_envelope
        || retained_bytes != certificate.stats.retained_bytes
        || retained_bytes > retained_envelope
    {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    let replay_peak_preflight = checked_sum(
        "peak scratch bytes",
        [
            retained_envelope,
            input_envelope,
            certificate
                .stats
                .condition_accumulation
                .retained_byte_envelope(),
        ],
    )?;
    if replay_peak_preflight != certificate.stats.peak_scratch_byte_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    check_limit(
        "peak scratch bytes",
        replay_peak_preflight,
        certificate.limits.max_peak_scratch_bytes,
    )?;
    let replay_baseline = checked_add("peak scratch bytes", retained_envelope, input_envelope)?;
    #[cfg(test)]
    note_ready_replay_input_allocation_attempt_for_test();
    let inputs = collect_nonzero_inputs(case.source(), source_scan.nonzero_condition_inputs)?;
    let mut accumulator_limits = certificate.limits.condition_accumulation;
    accumulator_limits.max_retained_bytes = accumulator_limits.max_retained_bytes.min(
        certificate
            .limits
            .max_peak_scratch_bytes
            .checked_sub(replay_baseline)
            .ok_or(GeneratedAffineResidualCasePremisesError::ResourceLimit {
                resource: "peak scratch bytes",
                requested: replay_baseline,
                limit: certificate.limits.max_peak_scratch_bytes,
            })?,
    );
    let accumulation = accumulate_generated_residual_affine_conditions(
        context,
        group.free_positions(),
        inputs,
        accumulator_limits,
    )
    .map_err(|_| GeneratedAffineResidualCasePremisesError::ConditionCanonicalization)?;
    if accumulation.rows().len() != certificate.premises.len() {
        return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
    }
    for (row, condition) in accumulation.rows().iter().zip(&certificate.premises) {
        if row.polynomial() != condition.polynomial()
            || condition.origins().len() != 1
            || !condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        {
            return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
        }
    }
    replay_stats.condition_accumulation = accumulation.stats();
    preflight_retained_rows(
        context,
        accumulation.rows(),
        certificate.limits,
        &mut replay_stats,
    )?;
    if replay_stats.condition_accumulation != certificate.stats.condition_accumulation
        || replay_stats.retained_conditions != certificate.stats.retained_conditions
        || replay_stats.retained_origins != certificate.stats.retained_origins
        || replay_stats.retained_polynomial_terms != certificate.stats.retained_polynomial_terms
        || replay_stats.retained_polynomial_exponent_entries
            != certificate.stats.retained_polynomial_exponent_entries
        || replay_stats.retained_polynomial_integer_bits
            != certificate.stats.retained_polynomial_integer_bits
    {
        return Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch);
    }
    let replay_retained_envelope = ready_retained_byte_envelope(
        authority.as_ref(),
        accumulation.rows(),
        certificate.premises.len(),
    )?;
    if replay_retained_envelope != retained_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    let replay_peak = checked_sum(
        "peak scratch bytes",
        [
            replay_retained_envelope,
            input_envelope,
            accumulation.stats().retained_byte_envelope(),
        ],
    )?;
    if replay_peak != certificate.stats.peak_scratch_byte_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    check_limit(
        "peak scratch bytes",
        replay_peak,
        certificate.limits.max_peak_scratch_bytes,
    )
}

fn replay_equality_inner(
    certificate: &GeneratedAffineResidualCaseEqualityRefinementCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_CASE_PREMISES_V2_SCHEMA {
        return Err(GeneratedAffineResidualCasePremisesError::SchemaMismatch);
    }
    authenticate_expected_authority(&certificate.authority, authority, certificate.limits)?;
    let mut replay_stats = GeneratedAffineResidualCasePremisesStats {
        authority_allocation_comparisons: 1,
        ..GeneratedAffineResidualCasePremisesStats::default()
    };
    authenticate_scope_and_authority(
        family,
        context,
        authority.as_ref(),
        certificate.limits,
        &mut replay_stats,
    )?;
    let case = authority
        .authenticated_source_neutral_case_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    replay_stats.case_lookups = bounded_add(
        "case lookups",
        replay_stats.case_lookups,
        1,
        certificate.limits.max_case_lookups,
    )?;
    let group = authority
        .authenticated_source_neutral_group_view(context)
        .map_err(|_| GeneratedAffineResidualCasePremisesError::SourceBinding)?;
    replay_stats.group_lookups = bounded_add(
        "group lookups",
        replay_stats.group_lookups,
        1,
        certificate.limits.max_group_lookups,
    )?;
    authenticate_case_geometry(
        authority.as_ref(),
        case,
        group,
        certificate.limits,
        &mut replay_stats,
    )?;
    authenticate_binding(&certificate.binding, authority.as_ref(), case, group)?;
    let source_scan = preflight_source(context, case.source(), certificate.limits)?;
    merge_source_scan(&mut replay_stats, source_scan);
    if source_scan.equality_predicates == 0 {
        return Err(GeneratedAffineResidualCasePremisesError::ExpectedEqualityRefinement);
    }
    authenticate_source_stats(certificate.stats, replay_stats)?;
    authenticate_compile_only_stats(certificate.stats)?;
    authenticate_equality_zero_stats(certificate.stats)?;
    if certificate.equality_predicate_ordinals.len() != source_scan.equality_predicates {
        return Err(GeneratedAffineResidualCasePremisesError::EqualityPredicateMismatch);
    }
    let equality_envelope = capacity_byte_envelope::<usize>(
        source_scan.equality_predicates,
        "equality predicate ordinals",
    )?;
    let retained_envelope = equality_retained_byte_envelope(
        authority.as_ref(),
        source_scan.equality_predicates,
        equality_envelope,
    )?;
    let retained_bytes = observed_equality_retained_bytes(
        &certificate.binding,
        &certificate.equality_predicate_ordinals,
    )?;
    check_limit(
        "retained bytes",
        retained_envelope,
        certificate.limits.max_retained_bytes,
    )?;
    if retained_envelope != certificate.stats.retained_byte_envelope
        || retained_bytes != certificate.stats.retained_bytes
        || retained_bytes > retained_envelope
    {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    let replay_peak = checked_add("peak scratch bytes", retained_envelope, equality_envelope)?;
    if replay_peak != certificate.stats.peak_scratch_byte_envelope {
        return Err(GeneratedAffineResidualCasePremisesError::RetainedByteEnvelopeExceeded);
    }
    check_limit(
        "peak scratch bytes",
        replay_peak,
        certificate.limits.max_peak_scratch_bytes,
    )?;
    authenticate_equality_predicate_ordinals(
        case.source(),
        &certificate.equality_predicate_ordinals,
    )
}

fn authenticate_expected_authority(
    retained: &Arc<GeneratedAffineResidualCaseAuthority>,
    expected: &Arc<GeneratedAffineResidualCaseAuthority>,
    limits: GeneratedAffineResidualCasePremisesLimits,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    check_limit(
        "authority allocation comparisons",
        1,
        limits.max_authority_allocation_comparisons,
    )?;
    if Arc::ptr_eq(retained, expected) {
        Ok(())
    } else {
        Err(GeneratedAffineResidualCasePremisesError::WrongAuthorityAllocation)
    }
}

fn authenticate_binding(
    binding: &CaseBinding,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if binding.family_fingerprint != authority.family_fingerprint()
        || binding.context_fingerprint != authority.context_fingerprint()
        || binding.sector_bits != authority.sector().active_bits()
        || binding.ordering != authority.ordering()
        || binding.ambient_arity != authority.arity()
        || binding.free_position_count != group.free_positions().len()
        || binding.compact_matrix_entries != group.compact_linear_coefficients().len()
    {
        return Err(GeneratedAffineResidualCasePremisesError::SourceBinding);
    }
    if binding.case_ordinal != authority.case_ordinal()
        || binding.case_ordinal != case.ordinal()
        || binding.source_locator != case.locator()
        || binding.ordinal_within_group != case.ordinal_within_group()
    {
        return Err(GeneratedAffineResidualCasePremisesError::WrongCaseBinding);
    }
    if binding.group_ordinal != authority.group_ordinal()
        || binding.group_ordinal != case.group_ordinal()
        || binding.group_ordinal != group.ordinal()
    {
        return Err(GeneratedAffineResidualCasePremisesError::WrongGroupBinding);
    }
    Ok(())
}

fn authenticate_source_stats(
    expected: GeneratedAffineResidualCasePremisesStats,
    actual: GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if expected.scope_comparison_bytes != actual.scope_comparison_bytes
        || expected.authority_replays != actual.authority_replays
        || expected.case_lookups != actual.case_lookups
        || expected.group_lookups != actual.group_lookups
        || expected.geometry_shape_comparisons != actual.geometry_shape_comparisons
        || expected.geometry_component_comparisons != actual.geometry_component_comparisons
        || expected.geometry_integer_bits != actual.geometry_integer_bits
        || expected.guard_scans != actual.guard_scans
        || expected.predicate_scans != actual.predicate_scans
        || expected.condition_polynomials != actual.condition_polynomials
        || expected.nonzero_condition_inputs != actual.nonzero_condition_inputs
        || expected.equality_predicates != actual.equality_predicates
        || expected.input_polynomial_terms != actual.input_polynomial_terms
        || expected.input_polynomial_exponent_entries != actual.input_polynomial_exponent_entries
        || expected.input_polynomial_integer_bits != actual.input_polynomial_integer_bits
    {
        Err(GeneratedAffineResidualCasePremisesError::SourceBinding)
    } else {
        Ok(())
    }
}

fn authenticate_compile_only_stats(
    expected: GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if expected.authority_allocation_comparisons != 0 {
        Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch)
    } else {
        Ok(())
    }
}

fn authenticate_equality_zero_stats(
    expected: GeneratedAffineResidualCasePremisesStats,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if expected.condition_accumulation
        != GeneratedResidualAffineConditionAccumulatorStats::default()
        || expected.retained_conditions != 0
        || expected.retained_origins != 0
        || expected.retained_polynomial_terms != 0
        || expected.retained_polynomial_exponent_entries != 0
        || expected.retained_polynomial_integer_bits != 0
    {
        Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch)
    } else {
        Ok(())
    }
}

fn try_copy_string(
    value: &str,
    resource: &'static str,
) -> Result<String, GeneratedAffineResidualCasePremisesError> {
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| GeneratedAffineResidualCasePremisesError::AllocationFailure { resource })?;
    copy.push_str(value);
    Ok(copy)
}

fn try_copy_bool_slice(
    values: &[bool],
    resource: &'static str,
) -> Result<Vec<bool>, GeneratedAffineResidualCasePremisesError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())
        .map_err(|_| GeneratedAffineResidualCasePremisesError::AllocationFailure { resource })?;
    copy.extend_from_slice(values);
    Ok(copy)
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualCasePremisesError::ResourceCountOverflow {
            resource: "integer magnitude bits",
        }
    })
}

fn capacity_byte_envelope<T>(
    entries: usize,
    resource: &'static str,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    checked_mul(resource, checked_mul(resource, entries, 2)?, size_of::<T>())
}

fn remaining(
    resource: &'static str,
    limit: usize,
    spent: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    limit
        .checked_sub(spent)
        .ok_or(GeneratedAffineResidualCasePremisesError::ResourceLimit {
            resource,
            requested: spent,
            limit,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCasePremisesError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualCasePremisesError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCasePremisesError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCasePremisesError> {
    if requested > limit {
        Err(GeneratedAffineResidualCasePremisesError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

const fn portable_usize(value: u64) -> usize {
    if usize::BITS >= u64::BITS {
        value as usize
    } else if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, ParametricIbpGenerator, SectorMask,
    };

    fn massive_tadpole_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn fixture(
        family: IntegralFamily,
        sector: SectorMask,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
    ) {
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            sector,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let source = GeneratedAffineResidualSourceAuthority::initial_global(queue);
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                source,
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory)
    }

    fn prior_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("001").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let old_inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let effective = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                old_inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        let prior_queue = Arc::new(
            GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
                &family,
                &context,
                effective,
                GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
            )
            .unwrap(),
        );
        let source = GeneratedAffineResidualSourceAuthority::prior_effective(prior_queue);
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                source,
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory)
    }

    fn authority(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        case_ordinal: usize,
    ) -> Arc<GeneratedAffineResidualCaseAuthority> {
        Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                family,
                context,
                Arc::clone(inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        )
    }

    fn ready_case(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        limits: GeneratedAffineResidualCasePremisesLimits,
    ) -> GeneratedAffineResidualCasePremisesCertificate {
        match compile_generated_affine_residual_case_premises(family, context, authority, limits)
            .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::Ready(certificate) => certificate,
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                panic!("natural ready fixture unexpectedly requires equality refinement")
            }
        }
    }

    fn assert_sealed_only(certificate: &GeneratedAffineResidualCasePremisesCertificate) {
        for condition in certificate.premises() {
            assert_eq!(condition.origins().len(), 1);
            assert!(
                condition
                    .origins()
                    .contains(&GuardOrigin::GeneratedAffineSealedCondition)
            );
            let stable = condition.origins().iter().next().unwrap().stable_string();
            assert!(!stable.contains("source-work"));
            assert!(!stable.contains("ready-terminal"));
            assert!(!stable.contains("residual-affine-branch-nonzero"));
        }
    }

    fn assert_ready_replay_rejected_before_input_allocation(
        certificate: &GeneratedAffineResidualCasePremisesCertificate,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) {
        reset_ready_replay_input_allocation_attempts_for_test();
        assert!(certificate.replay(family, context, authority).is_err());
        assert_eq!(ready_replay_input_allocation_attempts_for_test(), 0);
    }

    fn exact_limits(
        stats: GeneratedAffineResidualCasePremisesStats,
    ) -> GeneratedAffineResidualCasePremisesLimits {
        let mut limits = GeneratedAffineResidualCasePremisesLimits::default();
        limits.max_scope_comparison_bytes = stats.scope_comparison_bytes();
        limits.max_authority_allocation_comparisons = 1;
        limits.max_authority_replays = stats.authority_replays();
        limits.max_case_lookups = stats.case_lookups();
        limits.max_group_lookups = stats.group_lookups();
        limits.max_geometry_shape_comparisons = stats.geometry_shape_comparisons();
        limits.max_geometry_component_comparisons = stats.geometry_component_comparisons();
        limits.max_geometry_integer_bits = stats.geometry_integer_bits();
        limits.max_guard_scans = stats.guard_scans();
        limits.max_predicate_scans = stats.predicate_scans();
        limits.max_condition_polynomials = stats.condition_polynomials();
        limits.max_nonzero_condition_inputs = stats.nonzero_condition_inputs();
        limits.max_equality_predicates = stats.equality_predicates();
        limits.max_input_polynomial_terms = stats.input_polynomial_terms();
        limits.max_input_polynomial_exponent_entries = stats.input_polynomial_exponent_entries();
        limits.max_input_polynomial_integer_bits = stats.input_polynomial_integer_bits();
        limits.max_retained_conditions = stats.retained_conditions();
        limits.max_retained_origins = stats.retained_origins();
        limits.max_retained_polynomial_terms = stats.retained_polynomial_terms();
        limits.max_retained_polynomial_exponent_entries =
            stats.retained_polynomial_exponent_entries();
        limits.max_retained_polynomial_integer_bits = stats.retained_polynomial_integer_bits();
        limits.max_retained_bytes = stats.retained_byte_envelope();
        limits.max_peak_scratch_bytes = stats.peak_scratch_byte_envelope();
        limits
    }

    #[test]
    fn natural_zero_single_and_multiple_guard_cases_are_ready_and_sealed() {
        let (one_family, one_context, one_inventory) = fixture(
            massive_tadpole_family("premise-natural-zero-private"),
            SectorMask::try_new([true]).unwrap(),
        );
        assert_eq!(one_inventory.case_count(), 1);
        let one_authority = authority(&one_family, &one_context, &one_inventory, 0);
        let zero = ready_case(
            &one_family,
            &one_context,
            Arc::clone(&one_authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        assert_eq!(zero.stats().guard_scans(), 0);
        assert_eq!(zero.stats().condition_polynomials(), 0);
        assert!(zero.premises().is_empty());
        zero.replay(&one_family, &one_context, &one_authority)
            .unwrap();

        let (family, context, inventory) = fixture(
            equal_mass_two_loop_family("premise-natural-many-private"),
            SectorMask::try_from_bit_string("011").unwrap(),
        );
        assert!(inventory.case_count() >= 5);
        let single_authority = authority(&family, &context, &inventory, 1);
        let single = ready_case(
            &family,
            &context,
            Arc::clone(&single_authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        assert_eq!(single.stats().guard_scans(), 1);
        assert_eq!(single.stats().nonzero_condition_inputs(), 1);
        assert_eq!(single.premises().len(), 1);
        assert_sealed_only(&single);
        single.replay(&family, &context, &single_authority).unwrap();

        let discharged_authority = authority(&family, &context, &inventory, 3);
        let discharged = ready_case(
            &family,
            &context,
            Arc::clone(&discharged_authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        assert_eq!(discharged.stats().guard_scans(), 3);
        assert_eq!(discharged.stats().nonzero_condition_inputs(), 1);
        assert_eq!(discharged.premises().len(), 1);
        assert_sealed_only(&discharged);

        let multiple_authority = authority(&family, &context, &inventory, 4);
        let multiple = ready_case(
            &family,
            &context,
            Arc::clone(&multiple_authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        assert_eq!(multiple.stats().guard_scans(), 2);
        assert_eq!(multiple.stats().nonzero_condition_inputs(), 2);
        assert_eq!(multiple.premises().len(), 2);
        assert_ne!(
            multiple.premises()[0].polynomial(),
            multiple.premises()[1].polynomial()
        );
        assert_sealed_only(&multiple);
        multiple
            .replay(&family, &context, &multiple_authority)
            .unwrap();
    }

    #[test]
    fn duplicate_associate_inputs_merge_first_representative_and_seal_once() {
        let (family, context, _) = fixture(
            massive_tadpole_family("premise-associate-private"),
            SectorMask::try_new([true]).unwrap(),
        );
        let index = context.index(0).unwrap();
        let p = context.add(&index, &context.one()).unwrap();
        let q = context.mul(&p, &context.integer(-2)).unwrap();
        let p = context.numerator_condition(&p).unwrap();
        let q = context.numerator_condition(&q).unwrap();
        assert!(
            context
                .polynomial_loci_are_associates_with_limits(
                    &p,
                    &q,
                    crate::ExactAlgebraLimits::default(),
                )
                .unwrap()
        );
        let inputs = [
            GeneratedResidualAffineConditionInput::new(
                &p,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 0,
                    structural_locus_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &q,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                None,
            ),
        ];
        let accumulation = accumulate_generated_residual_affine_conditions(
            &context,
            &[0],
            inputs,
            GeneratedResidualAffineConditionAccumulatorLimits::default(),
        )
        .unwrap();
        assert_eq!(accumulation.rows().len(), 1);
        assert_eq!(accumulation.rows()[0].polynomial(), &p);
        assert_eq!(accumulation.rows()[0].source_input_ordinals(), &[0, 1]);
        assert_eq!(accumulation.stats().associate_checks(), 1);
        let premises = materialize_premises(
            &context,
            accumulation.rows(),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap();
        assert_eq!(premises.len(), 1);
        assert_eq!(premises[0].polynomial(), &p);
        assert_eq!(premises[0].origins().len(), 1);
        assert!(
            premises[0]
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        );
        drop(family);
    }

    #[test]
    fn prior_equal_zero_predicates_are_typed_deferred_not_nonzero_premises() {
        let (family, context, inventory) = prior_fixture("premise-equality-private");
        assert_eq!(inventory.case_count(), 3);
        let mut saw_nonzero_sibling_predicate = false;
        for case_ordinal in 0..inventory.case_count() {
            let authority = authority(&family, &context, &inventory, case_ordinal);
            let outcome = compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap();
            let GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
                certificate,
            ) = outcome
            else {
                panic!("an equality-bearing prior case must not become Ready")
            };
            assert!(!certificate.equality_predicate_ordinals().is_empty());
            assert_eq!(
                certificate.equality_predicate_ordinals().len(),
                certificate.stats().equality_predicates()
            );
            saw_nonzero_sibling_predicate |= certificate.stats().nonzero_condition_inputs() > 0;
            certificate.replay(&family, &context, &authority).unwrap();
            let debug = format!("{certificate:?}");
            assert!(debug.contains("<redacted>"));
            assert!(!debug.contains("m2"));
            assert!(!debug.contains("rustred::parametric"));
        }
        assert!(saw_nonzero_sibling_predicate);

        let authority = authority(&family, &context, &inventory, 0);
        let outcome = compile_generated_affine_residual_case_premises(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
            mut certificate,
        ) = outcome
        else {
            unreachable!()
        };
        let stats = certificate.stats();
        let exact = exact_limits(stats);
        let exact_outcome = compile_generated_affine_residual_case_premises(
            &family,
            &context,
            Arc::clone(&authority),
            exact,
        )
        .unwrap();
        let GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
            mut exact_certificate,
        ) = exact_outcome
        else {
            unreachable!()
        };
        assert_eq!(exact_certificate.stats(), stats);
        exact_certificate
            .replay(&family, &context, &authority)
            .unwrap();

        macro_rules! equality_zero_stat_tamper {
            ($field:ident) => {{
                assert_eq!(exact_certificate.stats.$field, 0);
                exact_certificate.stats.$field = 1;
                assert!(
                    exact_certificate
                        .replay(&family, &context, &authority)
                        .is_err(),
                    "{} tamper unexpectedly replayed",
                    stringify!($field),
                );
                exact_certificate.stats.$field = 0;
            }};
        }
        equality_zero_stat_tamper!(authority_allocation_comparisons);
        equality_zero_stat_tamper!(retained_conditions);
        equality_zero_stat_tamper!(retained_origins);
        equality_zero_stat_tamper!(retained_polynomial_terms);
        equality_zero_stat_tamper!(retained_polynomial_exponent_entries);
        equality_zero_stat_tamper!(retained_polynomial_integer_bits);

        let group = authority.authenticated_group_view(&context).unwrap();
        let nondefault_accumulation = accumulate_generated_residual_affine_conditions(
            &context,
            group.free_positions(),
            std::iter::empty::<GeneratedResidualAffineConditionInput<'_>>(),
            GeneratedResidualAffineConditionAccumulatorLimits::default(),
        )
        .unwrap()
        .stats();
        assert_ne!(
            nondefault_accumulation,
            GeneratedResidualAffineConditionAccumulatorStats::default()
        );
        exact_certificate.stats.condition_accumulation = nondefault_accumulation;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.stats.condition_accumulation =
            GeneratedResidualAffineConditionAccumulatorStats::default();

        let retained_envelope = exact_certificate.stats.retained_byte_envelope;
        exact_certificate.stats.retained_byte_envelope = retained_envelope + 1;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.stats.retained_byte_envelope = retained_envelope;
        let retained_bytes = exact_certificate.stats.retained_bytes;
        exact_certificate.stats.retained_bytes = retained_bytes + 1;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.stats.retained_bytes = retained_bytes;
        let peak = exact_certificate.stats.peak_scratch_byte_envelope;
        exact_certificate.stats.peak_scratch_byte_envelope = peak + 1;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.stats.peak_scratch_byte_envelope = peak;
        let max_retained_bytes = exact_certificate.limits.max_retained_bytes;
        exact_certificate.limits.max_retained_bytes = 0;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.limits.max_retained_bytes = max_retained_bytes;
        let max_peak_scratch_bytes = exact_certificate.limits.max_peak_scratch_bytes;
        exact_certificate.limits.max_peak_scratch_bytes = 0;
        assert!(
            exact_certificate
                .replay(&family, &context, &authority)
                .is_err()
        );
        exact_certificate.limits.max_peak_scratch_bytes = max_peak_scratch_bytes;
        exact_certificate
            .replay(&family, &context, &authority)
            .unwrap();

        assert!(stats.predicate_scans() > 0);
        let mut one_below_predicate_scans = exact;
        one_below_predicate_scans.max_predicate_scans = stats.predicate_scans() - 1;
        assert!(
            compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                one_below_predicate_scans,
            )
            .is_err()
        );
        assert!(stats.equality_predicates() > 0);
        let mut one_below_equality_predicates = exact;
        one_below_equality_predicates.max_equality_predicates = stats.equality_predicates() - 1;
        assert!(
            compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                one_below_equality_predicates,
            )
            .is_err()
        );

        certificate.equality_predicate_ordinals.pop();
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::EqualityPredicateMismatch)
        ));
    }

    #[test]
    fn exact_outer_limits_pass_and_every_positive_ready_demand_rejects_one_below() {
        let (family, context, inventory) = fixture(
            equal_mass_two_loop_family("premise-limits-private"),
            SectorMask::try_from_bit_string("011").unwrap(),
        );
        let authority = authority(&family, &context, &inventory, 4);
        let baseline = ready_case(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        let stats = baseline.stats();
        let exact = exact_limits(stats);
        let exact_certificate = ready_case(&family, &context, Arc::clone(&authority), exact);
        assert_eq!(exact_certificate.stats(), stats);
        exact_certificate
            .replay(&family, &context, &authority)
            .unwrap();

        macro_rules! one_below {
            ($field:ident, $demand:expr) => {{
                let demand = $demand;
                if demand > 0 {
                    let mut limits = exact;
                    limits.$field = demand - 1;
                    assert!(
                        compile_generated_affine_residual_case_premises(
                            &family,
                            &context,
                            Arc::clone(&authority),
                            limits,
                        )
                        .is_err(),
                        "{} one-below unexpectedly passed",
                        stringify!($field),
                    );
                }
            }};
        }
        one_below!(max_scope_comparison_bytes, stats.scope_comparison_bytes());
        one_below!(max_authority_replays, stats.authority_replays());
        one_below!(max_case_lookups, stats.case_lookups());
        one_below!(max_group_lookups, stats.group_lookups());
        one_below!(
            max_geometry_shape_comparisons,
            stats.geometry_shape_comparisons()
        );
        one_below!(
            max_geometry_component_comparisons,
            stats.geometry_component_comparisons()
        );
        one_below!(max_geometry_integer_bits, stats.geometry_integer_bits());
        one_below!(max_guard_scans, stats.guard_scans());
        one_below!(max_predicate_scans, stats.predicate_scans());
        one_below!(max_condition_polynomials, stats.condition_polynomials());
        one_below!(
            max_nonzero_condition_inputs,
            stats.nonzero_condition_inputs()
        );
        one_below!(max_equality_predicates, stats.equality_predicates());
        one_below!(max_input_polynomial_terms, stats.input_polynomial_terms());
        one_below!(
            max_input_polynomial_exponent_entries,
            stats.input_polynomial_exponent_entries()
        );
        one_below!(
            max_input_polynomial_integer_bits,
            stats.input_polynomial_integer_bits()
        );
        one_below!(max_retained_conditions, stats.retained_conditions());
        one_below!(max_retained_origins, stats.retained_origins());
        one_below!(
            max_retained_polynomial_terms,
            stats.retained_polynomial_terms()
        );
        one_below!(
            max_retained_polynomial_exponent_entries,
            stats.retained_polynomial_exponent_entries()
        );
        one_below!(
            max_retained_polynomial_integer_bits,
            stats.retained_polynomial_integer_bits()
        );
        one_below!(max_retained_bytes, stats.retained_byte_envelope());
        one_below!(max_peak_scratch_bytes, stats.peak_scratch_byte_envelope());

        let mut replay_zero = exact;
        replay_zero.max_authority_allocation_comparisons = 0;
        let certificate = ready_case(&family, &context, Arc::clone(&authority), replay_zero);
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::ResourceLimit {
                resource: "authority allocation comparisons",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn exact_authority_context_case_group_and_payload_tampering_are_rejected() {
        let (family, context, inventory) = fixture(
            equal_mass_two_loop_family("premise-binding-private"),
            SectorMask::try_from_bit_string("011").unwrap(),
        );
        let authority = authority(&family, &context, &inventory, 4);
        let independent = Arc::new((*authority).clone());
        let mut certificate = ready_case(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        assert!(certificate.same_authority_allocation(&authority));
        assert!(!certificate.same_authority_allocation(&independent));
        assert!(matches!(
            certificate.replay(&family, &context, &independent),
            Err(GeneratedAffineResidualCasePremisesError::WrongAuthorityAllocation)
        ));

        let wrong_family = equal_mass_two_loop_family("premise-binding-wrong-family-private");
        assert!(matches!(
            certificate.replay(&wrong_family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::WrongFamily)
        ));
        let wrong_context = ParametricCoefficientContext::try_new(
            family.coefficient_context(),
            "premise-binding-wrong-context-private",
            context.index_count(),
        )
        .unwrap();
        assert!(matches!(
            certificate.replay(&family, &wrong_context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::WrongContext)
        ));

        certificate.binding.case_ordinal += 1;
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::WrongCaseBinding)
        ));
        certificate.binding.case_ordinal -= 1;
        certificate.binding.group_ordinal += 1;
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::WrongGroupBinding)
        ));
        certificate.binding.group_ordinal -= 1;
        certificate.premises.swap(0, 1);
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::PremiseMismatch)
        ));
        certificate.premises.swap(0, 1);

        let authority_allocation_comparisons = certificate.stats.authority_allocation_comparisons;
        certificate.stats.authority_allocation_comparisons = 1;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.authority_allocation_comparisons = authority_allocation_comparisons;

        let retained_conditions = certificate.stats.retained_conditions;
        certificate.stats.retained_conditions = retained_conditions + 1;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.retained_conditions = retained_conditions;

        let retained_envelope = certificate.stats.retained_byte_envelope;
        certificate.stats.retained_byte_envelope = retained_envelope + 1;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.retained_byte_envelope = retained_envelope;

        let retained_bytes = certificate.stats.retained_bytes;
        certificate.stats.retained_bytes = retained_bytes + 1;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.retained_bytes = retained_bytes;

        let peak = certificate.stats.peak_scratch_byte_envelope;
        certificate.stats.peak_scratch_byte_envelope = peak + 1;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.peak_scratch_byte_envelope = peak;

        let accumulation = certificate.stats.condition_accumulation;
        certificate.stats.condition_accumulation =
            GeneratedResidualAffineConditionAccumulatorStats::default();
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.stats.condition_accumulation = accumulation;

        let max_retained_bytes = certificate.limits.max_retained_bytes;
        certificate.limits.max_retained_bytes = 0;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.limits.max_retained_bytes = max_retained_bytes;

        let max_peak_scratch_bytes = certificate.limits.max_peak_scratch_bytes;
        certificate.limits.max_peak_scratch_bytes = 0;
        assert_ready_replay_rejected_before_input_allocation(
            &certificate,
            &family,
            &context,
            &authority,
        );
        certificate.limits.max_peak_scratch_bytes = max_peak_scratch_bytes;

        reset_ready_replay_input_allocation_attempts_for_test();
        certificate.replay(&family, &context, &authority).unwrap();
        assert_eq!(ready_replay_input_allocation_attempts_for_test(), 1);
        certificate.schema = "tampered-generated-affine-premises";
        assert!(matches!(
            certificate.replay(&family, &context, &authority),
            Err(GeneratedAffineResidualCasePremisesError::SchemaMismatch)
        ));

        let debug = format!("{certificate:?}");
        let error = format!(
            "{:?}",
            GeneratedAffineResidualCasePremisesError::PremiseMismatch
        );
        assert!(debug.contains("<redacted>"));
        assert!(error.contains("<redacted>"));
        assert!(!debug.contains("m2"));
        assert!(!error.contains("m2"));
    }

    #[test]
    fn source_drop_exact_replay_concurrency_and_panic_boundary_hold() {
        let (family, context, inventory) = fixture(
            equal_mass_two_loop_family("premise-lifetime-private"),
            SectorMask::try_from_bit_string("011").unwrap(),
        );
        let weak_inventory = Arc::downgrade(&inventory);
        let authority = authority(&family, &context, &inventory, 4);
        let certificate = ready_case(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        drop(inventory);
        assert!(weak_inventory.upgrade().is_some());
        certificate.replay(&family, &context, &authority).unwrap();
        thread::scope(|scope| {
            for _ in 0..4 {
                let certificate = &certificate;
                let family = &family;
                let context = &context;
                let authority = &authority;
                scope.spawn(move || {
                    certificate.replay(family, context, authority).unwrap();
                });
            }
        });

        inject_premise_boundary_panic_for_test();
        assert!(matches!(
            compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            ),
            Err(GeneratedAffineResidualCasePremisesError::SymbolicaPanic)
        ));
        ready_case(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        );
        drop(certificate);
        drop(authority);
        assert!(weak_inventory.upgrade().is_none());
    }
}
