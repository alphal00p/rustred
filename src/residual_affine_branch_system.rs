//! Family-scoped affine-system compilation for one residual Boolean terminal.
//!
//! This layer is the authenticated bridge between construction-provenance
//! product branching and simultaneous integer-affine elimination.  It binds
//! every recognized row to the exact Coverage V4 structural-locus ordinal
//! which supplied it, compiles *all* equal-zero atoms before choosing an
//! outcome, and retains nonzero atoms as their original exact guards.
//!
//! A successful [`ResidualAffineBranchSystemOutcome::GuardedAffineMap`] is
//! deliberately not a proof that the nonzero guards have been discharged or
//! composed through the affine map.  Callers must use the exact original
//! Boolean-terminal predicate or a separately authenticated
//! [`crate::ResidualAffineBranchGuardCompositionCertificate`] before applying
//! the map at a concrete integer point.

use std::fmt;
use std::mem::{align_of, size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{Integer, PolyVariable};

use crate::residual_affine_atom_rows::{
    ResidualAffineAtomRowAttemptLogicalMemoryCensus, ResidualAffineAtomRowFreshCompilationAttempt,
    residual_affine_atom_row_attempt_logical_memory_census,
};
use crate::residual_affine_integer_system::{
    ResidualAffineIntegerSystemFreshCompilationAttempt,
    ResidualAffineIntegerSystemFreshPlanAuthorization,
    ResidualAffineIntegerSystemRawTransientCensus,
    residual_affine_integer_system_compilation_owned_logical_peak_from_census,
    residual_affine_integer_system_memory_envelope_from_limits,
};
use crate::{
    IntegralFamily, ParametricCoefficientContext, RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA,
    ResidualAffineAtomRowCertificate, ResidualAffineAtomRowError, ResidualAffineAtomRowLimits,
    ResidualAffineAtomRowOutcome, ResidualAffineAtomRowUnsupported,
    ResidualAffineIntegerEmptyWitness, ResidualAffineIntegerMap,
    ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemError,
    ResidualAffineIntegerSystemInputError, ResidualAffineIntegerSystemInputRow,
    ResidualAffineIntegerSystemLimits, ResidualAffineIntegerSystemOutcome,
    ResidualAffineIntegerSystemUnsupported, ResidualAffinePrimitiveRow,
    ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError,
    ResidualProductLocusBooleanNode, ResidualProductLocusBooleanNodeOutcome,
};

/// Stable schema for a family-scoped affine compilation of one disjoint
/// product-locus Boolean terminal.
pub const RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA: &str =
    "rustred-residual-affine-branch-system-v1";

#[cfg(test)]
thread_local! {
    static RESIDUAL_AFFINE_BRANCH_LOCAL_COMPARISON_CENSUS_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static RESIDUAL_AFFINE_BRANCH_MEMORY_CENSUS_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static RESIDUAL_AFFINE_BRANCH_SCOPE_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_branch_local_comparison_census_scans_for_test() {
    RESIDUAL_AFFINE_BRANCH_LOCAL_COMPARISON_CENSUS_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_branch_local_comparison_census_scans_for_test() -> usize {
    RESIDUAL_AFFINE_BRANCH_LOCAL_COMPARISON_CENSUS_SCANS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_branch_memory_census_scans_for_test() {
    RESIDUAL_AFFINE_BRANCH_MEMORY_CENSUS_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_branch_memory_census_scans_for_test() -> usize {
    RESIDUAL_AFFINE_BRANCH_MEMORY_CENSUS_SCANS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_branch_scope_scans_for_test() {
    RESIDUAL_AFFINE_BRANCH_SCOPE_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_branch_scope_scans_for_test() -> usize {
    RESIDUAL_AFFINE_BRANCH_SCOPE_SCANS.with(std::cell::Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualAffineBranchSystemLimits {
    pub atom_row: ResidualAffineAtomRowLimits,
    pub integer_system: ResidualAffineIntegerSystemLimits,
    /// Bounds use and retention of the family identity already cached by an
    /// authenticated [`IntegralFamily`]. It does not govern construction of
    /// that family.
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    /// Total bytes read across both operands before any scope-fingerprint
    /// equality comparison is entered.
    pub max_scope_fingerprint_comparison_bytes: usize,
    pub max_source_identity_bytes_referenced: usize,
    pub max_zero_atoms: usize,
    pub max_nonzero_guards: usize,
    pub max_structural_atom_lookups: usize,
    pub max_referenced_atom_terms: usize,
    pub max_zero_atom_source_terms: usize,
    pub max_zero_atom_exponent_entries: usize,
    pub max_zero_atom_coefficient_bits: usize,
    pub max_potential_block_witnesses: usize,
    pub max_potential_block_exponent_entries: usize,
    pub max_potential_retained_integer_bits: usize,
    pub max_retained_atom_context_fingerprint_bytes: usize,
    pub max_potential_row_components: usize,
    pub max_system_input_rows: usize,
    pub max_system_lineage_ordinals: usize,
    pub max_manifest_entries: usize,
    pub max_manifest_bytes: usize,
    pub max_manifest_validation_comparisons: usize,
    pub max_unsupported_reasons: usize,
    /// Conservative equal-payload census recomputed from actual branch-owned
    /// operands. The source cover and integer system enforce their own fresh
    /// checked phases.
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
}

impl Default for ResidualAffineBranchSystemLimits {
    fn default() -> Self {
        Self {
            atom_row: ResidualAffineAtomRowLimits::default(),
            integer_system: ResidualAffineIntegerSystemLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_scope_fingerprint_comparison_bytes: 4 * 1024 * 1024,
            max_source_identity_bytes_referenced: 1024 * 1024 * 1024,
            max_zero_atoms: 16_000_000,
            max_nonzero_guards: 64_000_000,
            max_structural_atom_lookups: 80_000_000,
            max_referenced_atom_terms: 512_000_000,
            max_zero_atom_source_terms: 256_000_000,
            max_zero_atom_exponent_entries: 16_000_000_000,
            max_zero_atom_coefficient_bits: 1_000_000_000_000_000,
            max_potential_block_witnesses: 256_000_000,
            max_potential_block_exponent_entries: 16_000_000_000,
            max_potential_retained_integer_bits: 4_000_000_000_000_000,
            max_retained_atom_context_fingerprint_bytes: 16 * 1024 * 1024 * 1024,
            max_potential_row_components: 16_000_000_000,
            max_system_input_rows: 16_000_000,
            max_system_lineage_ordinals: 16_000_000,
            max_manifest_entries: 80_000_000,
            max_manifest_bytes: 8 * 1024 * 1024 * 1024,
            max_manifest_validation_comparisons: 256_000_000,
            max_unsupported_reasons: 16_000_001,
            max_payload_comparison_units: 64_000_000_000,
            max_payload_comparison_bytes: 64 * 1024 * 1024 * 1024,
            max_payload_comparison_integer_bits: 8_000_000_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualAffineBranchSystemStats {
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    scope_fingerprint_comparison_bytes: usize,
    source_identity_bytes_referenced: usize,
    zero_atoms: usize,
    nonzero_guards: usize,
    structural_atom_lookups: usize,
    referenced_atom_terms: usize,
    zero_atom_source_terms: usize,
    zero_atom_exponent_entries: usize,
    zero_atom_coefficient_bits: usize,
    potential_block_witnesses: usize,
    potential_block_exponent_entries: usize,
    potential_retained_integer_bits: usize,
    retained_atom_context_fingerprint_bytes: usize,
    potential_row_components: usize,
    manifest_entries: usize,
    manifest_bytes: usize,
    manifest_validation_comparisons: usize,
    atom_recognitions_attempted: usize,
    recognized_rows: usize,
    redundant_zero_polynomials: usize,
    inconsistent_nonzero_constants: usize,
    unsupported_atoms: usize,
    system_input_rows: usize,
    system_lineage_ordinals: usize,
    retained_row_components: usize,
    retained_nonzero_guard_ordinals: usize,
    general_congruence_boundaries: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ResidualAffineBranchSystemStats {
    stats_getters!(
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        scope_fingerprint_comparison_bytes,
        source_identity_bytes_referenced,
        zero_atoms,
        nonzero_guards,
        structural_atom_lookups,
        referenced_atom_terms,
        zero_atom_source_terms,
        zero_atom_exponent_entries,
        zero_atom_coefficient_bits,
        potential_block_witnesses,
        potential_block_exponent_entries,
        potential_retained_integer_bits,
        retained_atom_context_fingerprint_bytes,
        potential_row_components,
        manifest_entries,
        manifest_bytes,
        manifest_validation_comparisons,
        atom_recognitions_attempted,
        recognized_rows,
        redundant_zero_polynomials,
        inconsistent_nonzero_constants,
        unsupported_atoms,
        system_input_rows,
        system_lineage_ordinals,
        retained_row_components,
        retained_nonzero_guard_ordinals,
        general_congruence_boundaries,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
    );
}

/// The complete recognition result for one exact equal-zero structural atom.
/// Unsupported recognition is retained as a completeness boundary, not
/// silently discarded.
#[derive(Clone, Debug)]
pub enum ResidualAffineBranchZeroAtomOutcome {
    Row(ResidualAffineAtomRowCertificate),
    RedundantZeroPolynomial(ResidualAffineAtomRowCertificate),
    InconsistentNonzeroConstant(ResidualAffineAtomRowCertificate),
    Unsupported(ResidualAffineAtomRowUnsupported),
}

impl ResidualAffineBranchZeroAtomOutcome {
    pub const fn certificate(&self) -> Option<&ResidualAffineAtomRowCertificate> {
        match self {
            Self::Row(certificate)
            | Self::RedundantZeroPolynomial(certificate)
            | Self::InconsistentNonzeroConstant(certificate) => Some(certificate),
            Self::Unsupported(_) => None,
        }
    }

    pub const fn unsupported_reason(&self) -> Option<&ResidualAffineAtomRowUnsupported> {
        match self {
            Self::Unsupported(reason) => Some(reason),
            _ => None,
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Row(left), Self::Row(right))
            | (Self::RedundantZeroPolynomial(left), Self::RedundantZeroPolynomial(right))
            | (Self::InconsistentNonzeroConstant(left), Self::InconsistentNonzeroConstant(right)) => {
                left.payload_eq(right)
            }
            (Self::Unsupported(left), Self::Unsupported(right)) => left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResidualAffineBranchZeroAtomRecognition {
    structural_locus_ordinal: usize,
    source_terms: usize,
    outcome: ResidualAffineBranchZeroAtomOutcome,
}

impl ResidualAffineBranchZeroAtomRecognition {
    pub const fn structural_locus_ordinal(&self) -> usize {
        self.structural_locus_ordinal
    }

    pub const fn source_terms(&self) -> usize {
        self.source_terms
    }

    pub const fn outcome(&self) -> &ResidualAffineBranchZeroAtomOutcome {
        &self.outcome
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.structural_locus_ordinal == other.structural_locus_ordinal
            && self.source_terms == other.source_terms
            && self.outcome.payload_eq(&other.outcome)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchUnsupportedReason {
    Atom {
        structural_locus_ordinal: usize,
        reason: ResidualAffineAtomRowUnsupported,
    },
    GeneralCongruence(ResidualAffineIntegerSystemUnsupported),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchEmptyReason {
    InconsistentNonzeroConstant { structural_locus_ordinal: usize },
    IntegerSystem,
}

/// `GuardedAffineMap` still carries the original, uncomposed nonzero guard
/// ordinals. It is the only outcome which may feed affine ordering, and even
/// then only together with an exact guard-applicability check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchSystemOutcome {
    GuardedAffineMap,
    ProvedEmpty(ResidualAffineBranchEmptyReason),
    Unsupported {
        reasons: Vec<ResidualAffineBranchUnsupportedReason>,
    },
}

#[derive(Clone, Debug)]
pub struct ResidualAffineBranchSystemCertificate {
    schema: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    source_partition_identity: Arc<str>,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    ready_terminal_ordinal: usize,
    // Keep the fallibly reserved vectors: converting user-sized payloads into
    // boxed slices may perform a second, infallible proportional allocation.
    zero_atom_recognitions: Vec<ResidualAffineBranchZeroAtomRecognition>,
    nonzero_guard_locus_ordinals: Vec<usize>,
    integer_system: Option<Arc<ResidualAffineIntegerSystemCertificate>>,
    outcome: ResidualAffineBranchSystemOutcome,
    limits: ResidualAffineBranchSystemLimits,
    stats: ResidualAffineBranchSystemStats,
}

/// Allocation-independent logical memory retained and reached while the
/// no-replay V2 branch adapter is compiling one ready Boolean terminal.
///
/// This value is adjacent authenticated state rather than part of the frozen
/// V1 certificate schema.  The shared Boolean cover is deliberately excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSystemLogicalMemoryCensus {
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    atom_attempt_owned_logical_peak_upper_bound: usize,
    integer_system_compilation_owned_logical_peak_upper_bound: usize,
    integer_system_raw_transient_census: Option<ResidualAffineIntegerSystemRawTransientCensus>,
}

/// Authenticated scalar cost of comparing two equal branch payloads, including
/// the exact nested integer-system comparison when one is retained.  No
/// branch child, lineage, integer-system allocation, or source locator crosses
/// the seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSystemPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

impl ResidualAffineBranchSystemPayloadComparisonCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }

    pub(crate) const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

/// Allocation-independent V2 parent preflight derived only from the frozen
/// V1 branch and nested integer-system limits. Shared cover and source-
/// identity payloads are excluded exactly as in the adjacent concrete census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSystemMemoryEnvelope {
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineBranchSystemMemoryEnvelope {
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

impl ResidualAffineBranchSystemLogicalMemoryCensus {
    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn atom_attempt_owned_logical_peak_upper_bound(self) -> usize {
        self.atom_attempt_owned_logical_peak_upper_bound
    }

    pub(crate) const fn integer_system_compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.integer_system_compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn integer_system_raw_transient_census(
        self,
    ) -> Option<ResidualAffineIntegerSystemRawTransientCensus> {
        self.integer_system_raw_transient_census
    }

    #[cfg(test)]
    pub(crate) fn tamper_retained_and_peak_coherently_for_test(&mut self) {
        self.retained_owned_logical_bytes = self.retained_owned_logical_bytes.saturating_add(1);
        self.compilation_owned_logical_peak_upper_bound = self
            .compilation_owned_logical_peak_upper_bound
            .saturating_add(1);
    }
}

/// The only V2 branch outcome allowed to carry a future plan authorization.
/// The type and all of its fields are crate-private and non-`Clone`.
pub(crate) struct ResidualAffineBranchSystemFreshGuarded {
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    authorization: ResidualAffineBranchSystemFreshGuardAuthorization,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    payload_comparison_census: BranchPayloadComparisonCensus,
}

/// Empty and unsupported outcomes are terminal at this seam and physically
/// contain no plan authorization, including unsupported branches which keep a
/// diagnostic integer-system map in their frozen V1 payload.
pub(crate) struct ResidualAffineBranchSystemFreshTerminal {
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    payload_comparison_census: BranchPayloadComparisonCensus,
}

pub(crate) enum ResidualAffineBranchSystemFreshCompilation {
    Guarded(ResidualAffineBranchSystemFreshGuarded),
    Terminal(ResidualAffineBranchSystemFreshTerminal),
}

/// Single-use authorization consumed only by the V2 sealed guard compiler.
/// It binds the exact cover, branch, and integer-system allocations together
/// with the adjacent logical-memory and comparison censes.
pub(crate) struct ResidualAffineBranchSystemFreshGuardAuthorization {
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    integer_plan_authorization: ResidualAffineIntegerSystemFreshPlanAuthorization,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    payload_comparison_census: BranchPayloadComparisonCensus,
    sealed_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    sealed_payload_comparison_census: BranchPayloadComparisonCensus,
}

pub(crate) enum ResidualAffineBranchSystemFreshAuthenticatedParts {
    Guarded {
        branch: Arc<ResidualAffineBranchSystemCertificate>,
        authorization: ResidualAffineBranchSystemFreshGuardAuthorization,
        memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    },
    Terminal {
        branch: Arc<ResidualAffineBranchSystemCertificate>,
        memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    },
}

impl fmt::Debug for ResidualAffineBranchSystemFreshGuarded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSystemFreshGuarded")
            .field("private_branch", &"<redacted>")
            .field("memory", &self.memory)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ResidualAffineBranchSystemFreshTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSystemFreshTerminal")
            .field("private_branch", &"<redacted>")
            .field("memory", &self.memory)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ResidualAffineBranchSystemFreshGuardAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSystemFreshGuardAuthorization")
            .field("private_sources", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ResidualAffineBranchSystemFreshCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Guarded(value) => value.fmt(formatter),
            Self::Terminal(value) => value.fmt(formatter),
        }
    }
}

impl ResidualAffineBranchSystemCertificate {
    /// Compile one exact ready Boolean terminal. The fresh seam retains the
    /// exact caller-supplied `Arc`; no deep-cloned cover may be substituted.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
        ready_terminal_ordinal: usize,
        limits: ResidualAffineBranchSystemLimits,
    ) -> Result<Self, ResidualAffineBranchSystemError> {
        catch_unwind(AssertUnwindSafe(|| {
            preflight_source(
                family,
                context,
                &source_cover,
                ready_terminal_ordinal,
                limits,
            )?;
            source_cover.replay(family, context)?;
            let supplied = source_cover.clone();
            let certificate = compile_replayed(
                family,
                context,
                source_cover,
                ready_terminal_ordinal,
                limits,
            )?;
            if !Arc::ptr_eq(&certificate.source_cover, &supplied) {
                return Err(ResidualAffineBranchSystemError::FreshSourceCoverAllocationMismatch);
            }
            Ok(certificate)
        }))
        .map_err(|_| ResidualAffineBranchSystemError::SymbolicaPanic)?
    }

    /// V2-only positional adapter used after an enclosing Boolean authority
    /// has replayed the complete source exactly once.  This method performs no
    /// cover replay and returns a non-forgeable, single-use fresh bundle.
    pub(crate) fn compile_fresh_replayed(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
        ready_terminal_ordinal: usize,
        limits: ResidualAffineBranchSystemLimits,
    ) -> Result<ResidualAffineBranchSystemFreshCompilation, ResidualAffineBranchSystemError> {
        catch_unwind(AssertUnwindSafe(|| {
            compile_fresh_replayed_inner(
                family,
                context,
                source_cover,
                ready_terminal_ordinal,
                limits,
            )
        }))
        .map_err(|_| ResidualAffineBranchSystemError::SymbolicaPanic)?
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn source_partition_identity(&self) -> &str {
        &self.source_partition_identity
    }

    pub const fn source_cover(&self) -> &Arc<ResidualProductLocusBooleanCoverCertificate> {
        &self.source_cover
    }

    pub const fn ready_terminal_ordinal(&self) -> usize {
        self.ready_terminal_ordinal
    }

    pub fn ready_terminal(&self) -> Option<&ResidualProductLocusBooleanNode> {
        self.source_cover.nodes().get(self.ready_terminal_ordinal)
    }

    pub fn zero_atom_recognitions(&self) -> &[ResidualAffineBranchZeroAtomRecognition] {
        &self.zero_atom_recognitions
    }

    /// Original Coverage V4 atom ordinals. They have not yet been composed
    /// through or discharged against the affine map.
    pub fn nonzero_guard_locus_ordinals(&self) -> &[usize] {
        &self.nonzero_guard_locus_ordinals
    }

    /// Return the simultaneous integer system only when it is itself a sound
    /// certificate for this complete branch.  In particular, a system solved
    /// from the recognized subset of an `Unsupported` branch is retained for
    /// replay diagnostics but is never exposed as actionable public data.
    pub fn integer_system(&self) -> Option<&ResidualAffineIntegerSystemCertificate> {
        certified_integer_system_for_outcome(&self.outcome, &self.integer_system)
    }

    /// Exact retained integer-system allocation for source-neutral affine
    /// composition. The outcome gate is identical to [`Self::integer_system`].
    pub fn integer_system_arc(&self) -> Option<&Arc<ResidualAffineIntegerSystemCertificate>> {
        certified_integer_system_arc_for_outcome(&self.outcome, &self.integer_system)
    }

    /// Exact private payload compared by [`Self::payload_eq_checked_inner`].
    ///
    /// This deliberately does not apply the public outcome gate: an
    /// `Unsupported` branch can retain a diagnostic integer system compiled
    /// from its recognized subset, and replay equality must authenticate that
    /// retained payload even though it is not actionable semantic data.
    pub(crate) const fn retained_integer_system_for_payload_comparison(
        &self,
    ) -> &Option<Arc<ResidualAffineIntegerSystemCertificate>> {
        &self.integer_system
    }

    pub fn affine_map(&self) -> Option<&ResidualAffineIntegerMap> {
        matches!(
            self.outcome,
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        )
        .then_some(())
        .and_then(|_| self.integer_system.as_deref())
        .and_then(ResidualAffineIntegerSystemCertificate::affine_map)
    }

    pub fn empty_witness(&self) -> Option<&ResidualAffineIntegerEmptyWitness> {
        self.integer_system
            .as_deref()
            .and_then(ResidualAffineIntegerSystemCertificate::empty_witness)
    }

    pub const fn outcome(&self) -> &ResidualAffineBranchSystemOutcome {
        &self.outcome
    }

    pub const fn limits(&self) -> ResidualAffineBranchSystemLimits {
        self.limits
    }

    pub const fn stats(&self) -> ResidualAffineBranchSystemStats {
        self.stats
    }

    /// Recompute and authenticate the retained equal-payload comparison
    /// census, returning scalars only for an enclosing V2 resource preflight.
    pub(crate) fn authenticated_payload_comparison_census(
        &self,
    ) -> Result<ResidualAffineBranchSystemPayloadComparisonCensus, ResidualAffineBranchSystemError>
    {
        let local = branch_equal_payload_comparison_census(self)?;
        self.recursive_payload_comparison_census_from_local(local)
    }

    /// Authenticate the fresh adjacent memory and local comparison census in
    /// one pass, then add the exact nested integer-system equality census once.
    pub(crate) fn authenticate_fresh_memory_and_payload_comparison_census(
        &self,
        context: &ParametricCoefficientContext,
        memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> Result<ResidualAffineBranchSystemPayloadComparisonCensus, ResidualAffineBranchSystemError>
    {
        let local = authenticate_fresh_branch_adjacent_census(context, self, memory, None)?;
        self.recursive_payload_comparison_census_from_local(local)
    }

    fn recursive_payload_comparison_census_from_local(
        &self,
        local: BranchPayloadComparisonCensus,
    ) -> Result<ResidualAffineBranchSystemPayloadComparisonCensus, ResidualAffineBranchSystemError>
    {
        let nested = match self.retained_integer_system_for_payload_comparison() {
            Some(system) => Some(
                system
                    .recompute_payload_comparison_census()
                    .map_err(ResidualAffineBranchSystemError::IntegerSystem)?,
            ),
            None => None,
        };
        Ok(ResidualAffineBranchSystemPayloadComparisonCensus {
            units: checked_add(
                "branch recursive payload comparison units",
                local.units,
                nested.map(|value| value.units()).unwrap_or(0),
            )?,
            bytes: checked_add(
                "branch recursive payload comparison bytes",
                local.bytes,
                nested.map(|value| value.bytes()).unwrap_or(0),
            )?,
            integer_bits: checked_add(
                "branch recursive payload comparison integer bits",
                local.integer_bits,
                nested.map(|value| value.integer_bits()).unwrap_or(0),
            )?,
        })
    }

    /// Evaluate the exact original Boolean path at one integer point. This is
    /// useful even for unsupported/empty outcomes and does not pretend that
    /// the retained guards have been transformed into affine coordinates.
    pub fn matches_original_boolean_terminal_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<bool, ResidualAffineBranchSystemError> {
        let comparison_bytes = preflight_fingerprint_pair(
            "context fingerprint bytes",
            &self.context_fingerprint,
            context.fingerprint(),
            self.limits.max_context_fingerprint_bytes,
        )?;
        check_limit(
            "scope fingerprint comparison bytes",
            comparison_bytes,
            self.limits.max_scope_fingerprint_comparison_bytes,
        )?;
        if self.context_fingerprint != context.fingerprint() {
            return Err(ResidualAffineBranchSystemError::WrongContext);
        }
        Ok(self
            .source_cover
            .ready_terminal_for_indices(context, indices)?
            .is_some_and(|terminal| terminal.ordinal() == self.ready_terminal_ordinal))
    }

    /// Exact concrete applicability at an already-mapped point in the original
    /// index coordinates `n`.
    ///
    /// This method does **not** accept affine parameters `t` and does not
    /// evaluate composed guards `G(F(t))`.  Use
    /// [`crate::ResidualAffineBranchGuardCompositionCertificate`] for that
    /// separately authenticated operation. Unsupported and proved-empty
    /// outcomes are never actionable.
    pub fn guarded_affine_map_applies_at_original_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<bool, ResidualAffineBranchSystemError> {
        if !matches!(
            self.outcome,
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ) {
            return Ok(false);
        }
        self.matches_original_boolean_terminal_for_indices(context, indices)
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ResidualAffineBranchSystemError> {
        self.replay_with_cover(family, context, self.source_cover.clone())
    }

    /// Persisted replay accepts an independently allocated cover-owned payload.
    /// The cover certificate still enforces its own explicit identity boundary
    /// for nested discovery/extraction proof allocations.
    pub fn replay_with_cover(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) -> Result<(), ResidualAffineBranchSystemError> {
        catch_unwind(AssertUnwindSafe(|| {
            validate_scope(
                self.schema,
                &self.family_fingerprint,
                &self.context_fingerprint,
                family,
                context,
                self.limits,
            )?;
            preflight_source(
                family,
                context,
                &source_cover,
                self.ready_terminal_ordinal,
                self.limits,
            )?;
            source_cover.replay(family, context)?;
            if !Arc::ptr_eq(&self.source_cover, &source_cover)
                && !self.source_cover.payload_eq_checked(&source_cover)?
            {
                return Err(ResidualAffineBranchSystemError::SourceCoverMismatch);
            }
            let rebuilt = compile_replayed(
                family,
                context,
                source_cover,
                self.ready_terminal_ordinal,
                self.limits,
            )?;
            if self.payload_eq_checked_inner(&rebuilt, true)? {
                Ok(())
            } else {
                Err(ResidualAffineBranchSystemError::ReplayMismatch)
            }
        }))
        .map_err(|_| ResidualAffineBranchSystemError::SymbolicaPanic)?
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.payload_eq_checked(other).unwrap_or(false)
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, ResidualAffineBranchSystemError> {
        // This is a directional authentication comparison. The receiver is the
        // retained authority, so its persisted limits cap the complete census
        // of both operands before branch-owned deep equality is entered.
        self.payload_eq_checked_inner(other, false)
    }

    fn payload_eq_checked_inner(
        &self,
        other: &Self,
        source_cover_already_equal: bool,
    ) -> Result<bool, ResidualAffineBranchSystemError> {
        if std::ptr::eq(self, other) {
            return Ok(true);
        }
        preflight_branch_payload_comparison(self, other)?;
        let branch_owned_equal = self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.source_partition_identity == other.source_partition_identity
            && self.ready_terminal_ordinal == other.ready_terminal_ordinal
            && self.zero_atom_recognitions.len() == other.zero_atom_recognitions.len()
            && self
                .zero_atom_recognitions
                .iter()
                .zip(other.zero_atom_recognitions.iter())
                .all(|(left, right)| left.payload_eq(right))
            && self.nonzero_guard_locus_ordinals == other.nonzero_guard_locus_ordinals
            && self.outcome == other.outcome
            && self.limits == other.limits
            && self.stats == other.stats;
        if !branch_owned_equal {
            return Ok(false);
        }
        let source_cover_equal = source_cover_already_equal
            || Arc::ptr_eq(&self.source_cover, &other.source_cover)
            || self.source_cover.payload_eq_checked(&other.source_cover)?;
        if !source_cover_equal {
            return Ok(false);
        }
        option_system_payload_eq_checked(&self.integer_system, &other.integer_system)
    }
}

#[cfg(test)]
impl ResidualAffineBranchSystemCertificate {
    pub(crate) fn tamper_ready_terminal_ordinal_for_test(&mut self) {
        self.ready_terminal_ordinal = usize::MAX;
    }

    pub(crate) fn set_ready_terminal_ordinal_for_test(&mut self, ordinal: usize) {
        self.ready_terminal_ordinal = ordinal;
    }

    pub(crate) fn tamper_first_zero_atom_ordinal_for_test(&mut self) {
        if let Some(first) = self.zero_atom_recognitions.first_mut() {
            first.structural_locus_ordinal = usize::MAX;
        }
    }

    pub(crate) fn tamper_first_guard_ordinal_for_test(&mut self) {
        if let Some(first) = self.nonzero_guard_locus_ordinals.first_mut() {
            *first = usize::MAX;
        }
    }

    pub(crate) fn tamper_stats_for_test(&mut self) {
        self.stats.atom_recognitions_attempted =
            self.stats.atom_recognitions_attempted.saturating_add(1);
    }

    pub(crate) fn tamper_schema_for_test(&mut self) {
        self.schema = "tampered-residual-affine-branch-schema";
    }

    pub(crate) fn tamper_family_fingerprint_for_test(&mut self) {
        self.family_fingerprint.push_str("|tampered-family");
    }

    pub(crate) fn tamper_context_fingerprint_for_test(&mut self) {
        self.context_fingerprint.push_str("|tampered-context");
    }

    pub(crate) fn tamper_source_identity_for_test(&mut self) {
        self.source_partition_identity = Arc::from("tampered-source-partition-identity");
    }

    pub(crate) fn tamper_first_zero_atom_source_terms_for_test(&mut self) {
        if let Some(first) = self.zero_atom_recognitions.first_mut() {
            first.source_terms = first.source_terms.saturating_add(1);
        }
    }

    pub(crate) fn tamper_first_zero_atom_outcome_for_test(&mut self) {
        if let Some(first) = self.zero_atom_recognitions.first_mut() {
            first.outcome = ResidualAffineBranchZeroAtomOutcome::Unsupported(
                ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial {
                    term_ordinal: usize::MAX,
                },
            );
        }
    }

    pub(crate) fn tamper_outcome_for_test(&mut self) {
        self.outcome = ResidualAffineBranchSystemOutcome::Unsupported {
            reasons: Vec::new(),
        };
    }

    pub(crate) fn tamper_integer_system_for_test(&mut self) {
        self.integer_system = None;
    }

    pub(crate) fn tamper_limits_for_test(&mut self) {
        self.limits.max_zero_atoms = 0;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchSystemError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    SourceCoverMismatch,
    FreshSourceCoverAllocationMismatch,
    FreshSourceBranchAllocationMismatch,
    FreshIntegerSystemAllocationMismatch,
    FreshAuthorizationOutcomeInvariant,
    FreshAdjacentCensusMismatch,
    MemoryCensusInvariant,
    ReadyTerminalNotFound {
        ordinal: usize,
    },
    ReadyTerminalOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    TerminalNotReadyForAffineRecognition {
        ordinal: usize,
    },
    SourceTerminalManifestMismatch,
    StructuralLocusOrdinalOutOfRange {
        ordinal: usize,
    },
    AtomRow {
        structural_locus_ordinal: usize,
        source: ResidualAffineAtomRowError,
    },
    AtomRowOutcomeInvariant {
        structural_locus_ordinal: usize,
    },
    SystemInput {
        structural_locus_ordinal: usize,
        source: ResidualAffineIntegerSystemInputError,
    },
    IntegerSystem(ResidualAffineIntegerSystemError),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    ReplayMismatch,
    SymbolicaPanic,
    BooleanCover(ResidualProductLocusBooleanCoverError),
}

impl fmt::Display for ResidualAffineBranchSystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual affine-branch schema mismatch"),
            Self::WrongFamily => formatter.write_str("residual affine branch belongs to another family"),
            Self::WrongContext => formatter.write_str("residual affine branch belongs to another K(n) context"),
            Self::SourceCoverMismatch => formatter.write_str("residual affine branch source Boolean cover differs"),
            Self::FreshSourceCoverAllocationMismatch => formatter.write_str("fresh residual affine branch did not retain the exact supplied Boolean-cover allocation"),
            Self::FreshSourceBranchAllocationMismatch => formatter.write_str("fresh residual affine branch authorization did not retain the exact supplied branch allocation"),
            Self::FreshIntegerSystemAllocationMismatch => formatter.write_str("fresh residual affine branch authorization did not retain the exact branch integer-system allocation"),
            Self::FreshAuthorizationOutcomeInvariant => formatter.write_str("only a guarded affine branch may retain fresh guard-plan authorization"),
            Self::FreshAdjacentCensusMismatch => formatter.write_str("fresh residual affine branch adjacent logical-memory or comparison census differs"),
            Self::MemoryCensusInvariant => formatter.write_str("fresh residual affine branch logical-memory census invariant failed"),
            Self::ReadyTerminalNotFound { ordinal } => write!(formatter, "Boolean node {ordinal} was not found"),
            Self::ReadyTerminalOrdinalMismatch { expected, actual } => write!(formatter, "Boolean node at position {expected} carries ordinal {actual}"),
            Self::TerminalNotReadyForAffineRecognition { ordinal } => write!(formatter, "Boolean node {ordinal} is not ready for affine recognition"),
            Self::SourceTerminalManifestMismatch => formatter.write_str("Boolean terminal zero/nonzero atom manifests are not canonical and disjoint"),
            Self::StructuralLocusOrdinalOutOfRange { ordinal } => write!(formatter, "Coverage V4 structural locus ordinal {ordinal} is out of range"),
            Self::AtomRow { structural_locus_ordinal, source } => write!(formatter, "affine recognition for structural locus {structural_locus_ordinal} failed: {source}"),
            Self::AtomRowOutcomeInvariant { structural_locus_ordinal } => write!(formatter, "affine recognition for structural locus {structural_locus_ordinal} reported a row outcome without retaining that row"),
            Self::SystemInput { structural_locus_ordinal, source } => write!(formatter, "affine-system input for structural locus {structural_locus_ordinal} failed: {source}"),
            Self::IntegerSystem(source) => source.fmt(formatter),
            Self::ResourceLimit { resource, requested, limit } => write!(formatter, "residual affine-branch {resource} requested {requested}, configured limit is {limit}"),
            Self::ResourceCountOverflow { resource } => write!(formatter, "residual affine-branch {resource} count overflowed usize"),
            Self::AllocationFailure { resource } => write!(formatter, "could not reserve bounded residual affine-branch storage for {resource}"),
            Self::ReplayMismatch => formatter.write_str("residual affine branch did not replay"),
            Self::SymbolicaPanic => formatter.write_str("Symbolica panicked during residual affine-branch compilation"),
            Self::BooleanCover(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ResidualAffineBranchSystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AtomRow { source, .. } => Some(source),
            Self::SystemInput { source, .. } => Some(source),
            Self::IntegerSystem(source) => Some(source),
            Self::BooleanCover(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ResidualProductLocusBooleanCoverError> for ResidualAffineBranchSystemError {
    fn from(value: ResidualProductLocusBooleanCoverError) -> Self {
        Self::BooleanCover(value)
    }
}

struct SourcePreflight<'a> {
    terminal: &'a ResidualProductLocusBooleanNode,
    stats: ResidualAffineBranchSystemStats,
}

fn preflight_source<'a>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: &'a ResidualProductLocusBooleanCoverCertificate,
    ready_terminal_ordinal: usize,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<SourcePreflight<'a>, ResidualAffineBranchSystemError> {
    if source_cover.schema() != RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA {
        return Err(ResidualAffineBranchSystemError::SchemaMismatch);
    }
    let family_fingerprint_comparison_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        source_cover.family_fingerprint(),
        family.fingerprint_ref(),
        limits.max_family_fingerprint_bytes,
    )?;
    let context_fingerprint_comparison_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        source_cover.context_fingerprint(),
        context.fingerprint(),
        limits.max_context_fingerprint_bytes,
    )?;
    let scope_fingerprint_comparison_bytes = checked_add(
        "scope fingerprint comparison bytes",
        family_fingerprint_comparison_bytes,
        context_fingerprint_comparison_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        scope_fingerprint_comparison_bytes,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if source_cover.family_fingerprint() != family.fingerprint_ref() {
        return Err(ResidualAffineBranchSystemError::WrongFamily);
    }
    if source_cover.context_fingerprint() != context.fingerprint() {
        return Err(ResidualAffineBranchSystemError::WrongContext);
    }
    let source_identity_bytes = source_cover
        .source_extraction()
        .source_partition()
        .source_identity()
        .len();
    check_limit(
        "source identity bytes referenced",
        source_identity_bytes,
        limits.max_source_identity_bytes_referenced,
    )?;

    let terminal = source_cover.nodes().get(ready_terminal_ordinal).ok_or(
        ResidualAffineBranchSystemError::ReadyTerminalNotFound {
            ordinal: ready_terminal_ordinal,
        },
    )?;
    if terminal.ordinal() != ready_terminal_ordinal {
        return Err(
            ResidualAffineBranchSystemError::ReadyTerminalOrdinalMismatch {
                expected: ready_terminal_ordinal,
                actual: terminal.ordinal(),
            },
        );
    }
    if !matches!(
        terminal.outcome(),
        ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
    ) {
        return Err(
            ResidualAffineBranchSystemError::TerminalNotReadyForAffineRecognition {
                ordinal: ready_terminal_ordinal,
            },
        );
    }
    let zero_atoms = terminal.equal_zero_atoms().len();
    let nonzero_guards = terminal.nonzero_atoms().len();
    let all_atoms = checked_add("structural atom lookups", zero_atoms, nonzero_guards)?;
    check_limit("zero atoms", zero_atoms, limits.max_zero_atoms)?;
    check_limit("nonzero guards", nonzero_guards, limits.max_nonzero_guards)?;
    check_limit(
        "structural atom lookups",
        all_atoms,
        limits.max_structural_atom_lookups,
    )?;
    check_limit(
        "system input rows",
        zero_atoms,
        limits.max_system_input_rows,
    )?;
    check_limit(
        "system lineage ordinals",
        zero_atoms,
        limits.max_system_lineage_ordinals,
    )?;
    check_limit(
        "unsupported reasons",
        zero_atoms,
        limits.max_unsupported_reasons,
    )?;

    let manifest_entries = all_atoms;
    check_limit(
        "manifest entries",
        manifest_entries,
        limits.max_manifest_entries,
    )?;
    let zero_manifest_bytes = checked_mul(
        "manifest bytes",
        zero_atoms,
        size_of::<ResidualAffineBranchZeroAtomRecognition>(),
    )?;
    let guard_manifest_bytes = checked_mul("manifest bytes", nonzero_guards, size_of::<usize>())?;
    let manifest_bytes = checked_add("manifest bytes", zero_manifest_bytes, guard_manifest_bytes)?;
    check_limit("manifest bytes", manifest_bytes, limits.max_manifest_bytes)?;

    let row_components = checked_add("potential row components", context.index_count(), 1)?;
    // Every successful row is retained once by its atom certificate and once
    // by the independently owned integer-system input. Census both copies
    // before either fallible deep-copy seam is entered.
    let retained_row_copies = checked_mul("potential row components", row_components, 2)?;
    let potential_row_components =
        checked_mul("potential row components", zero_atoms, retained_row_copies)?;
    check_limit(
        "potential row components",
        potential_row_components,
        limits.max_potential_row_components,
    )?;
    let retained_atom_context_fingerprint_bytes = checked_mul(
        "retained atom context fingerprint bytes",
        zero_atoms,
        context.fingerprint().len(),
    )?;
    check_limit(
        "retained atom context fingerprint bytes",
        retained_atom_context_fingerprint_bytes,
        limits.max_retained_atom_context_fingerprint_bytes,
    )?;

    let within_zero = zero_atoms.saturating_sub(1);
    let within_nonzero = nonzero_guards.saturating_sub(1);
    let cross = all_atoms;
    let maximum_manifest_validation_comparisons = checked_add(
        "manifest validation comparisons",
        checked_add(
            "manifest validation comparisons",
            within_zero,
            within_nonzero,
        )?,
        cross,
    )?;
    check_limit(
        "manifest validation comparisons",
        maximum_manifest_validation_comparisons,
        limits.max_manifest_validation_comparisons,
    )?;
    let (manifest_is_canonical, observed_manifest_validation_comparisons) =
        validate_sorted_disjoint_manifest(terminal.equal_zero_atoms(), terminal.nonzero_atoms());
    debug_assert!(
        observed_manifest_validation_comparisons <= maximum_manifest_validation_comparisons
    );
    if !manifest_is_canonical {
        return Err(ResidualAffineBranchSystemError::SourceTerminalManifestMismatch);
    }
    // The full prospective maximum was charged before entering validation, so
    // successful stats must report that same authenticated charge rather than
    // only the comparisons reached by this particular canonical manifest.
    let manifest_validation_comparisons = maximum_manifest_validation_comparisons;

    let coverage = source_cover.source_queue().discovery().coverage();
    let mut referenced_atom_terms = 0usize;
    let mut zero_atom_source_terms = 0usize;
    let mut zero_atom_exponent_entries = 0usize;
    let mut zero_atom_coefficient_bits = 0usize;
    let mut potential_block_witnesses = 0usize;
    let mut potential_block_exponent_entries = 0usize;
    let mut potential_retained_integer_bits = 0usize;
    let base_variables = context.base().variables().len();
    for (is_zero, &ordinal) in terminal
        .equal_zero_atoms()
        .iter()
        .map(|ordinal| (true, ordinal))
        .chain(
            terminal
                .nonzero_atoms()
                .iter()
                .map(|ordinal| (false, ordinal)),
        )
    {
        let polynomial = coverage
            .structural_locus(ordinal)
            .ok_or(ResidualAffineBranchSystemError::StructuralLocusOrdinalOutOfRange { ordinal })?;
        referenced_atom_terms = bounded_add(
            "referenced atom terms",
            referenced_atom_terms,
            polynomial.raw().nterms(),
            limits.max_referenced_atom_terms,
        )?;
        if is_zero {
            let source_terms = polynomial.raw().nterms();
            zero_atom_source_terms = bounded_add(
                "zero-atom source terms",
                zero_atom_source_terms,
                source_terms,
                limits.max_zero_atom_source_terms,
            )?;
            let exponent_entries = checked_mul(
                "zero-atom exponent entries",
                source_terms,
                polynomial.raw().variables.len(),
            )?;
            zero_atom_exponent_entries = bounded_add(
                "zero-atom exponent entries",
                zero_atom_exponent_entries,
                exponent_entries,
                limits.max_zero_atom_exponent_entries,
            )?;
            potential_block_witnesses = bounded_add(
                "potential block witnesses",
                potential_block_witnesses,
                source_terms,
                limits.max_potential_block_witnesses,
            )?;
            let block_exponent_entries = checked_mul(
                "potential block exponent entries",
                source_terms,
                base_variables,
            )?;
            potential_block_exponent_entries = bounded_add(
                "potential block exponent entries",
                potential_block_exponent_entries,
                block_exponent_entries,
                limits.max_potential_block_exponent_entries,
            )?;

            let mut source_coefficient_bits = 0usize;
            let mut largest_source_coefficient_bits = 0usize;
            for coefficient in &polynomial.raw().coefficients {
                let bits = integer_magnitude_bits(coefficient)?;
                check_limit(
                    "zero-atom integer coefficient bits",
                    bits,
                    limits.atom_row.max_integer_coefficient_bits,
                )?;
                largest_source_coefficient_bits = largest_source_coefficient_bits.max(bits);
                source_coefficient_bits =
                    checked_add("zero-atom coefficient bits", source_coefficient_bits, bits)?;
            }
            zero_atom_coefficient_bits = bounded_add(
                "zero-atom coefficient bits",
                zero_atom_coefficient_bits,
                source_coefficient_bits,
                limits.max_zero_atom_coefficient_bits,
            )?;
            // A retained certificate owns the source coefficients, at most one
            // signed-scalar witness per source term, and one primitive row.
            // Witness quotients cannot have greater magnitude than the source
            // integer coefficients from which exact gcd division constructs
            // them. This deliberately conservative preflight also charges
            // unsupported attempts which ultimately retain no child payload.
            let source_and_witness_bits = checked_mul(
                "potential retained integer bits",
                source_coefficient_bits,
                2,
            )?;
            let primitive_bits = checked_mul(
                "potential retained integer bits",
                largest_source_coefficient_bits,
                row_components,
            )?;
            // The atom certificate retains one primitive row and the branch
            // constructs one independently owned integer-system input row.
            // Charge both before either fallible copy seam is entered.
            let primitive_and_system_input_bits =
                checked_mul("potential retained integer bits", primitive_bits, 2)?;
            potential_retained_integer_bits = bounded_add(
                "potential retained integer bits",
                potential_retained_integer_bits,
                checked_add(
                    "potential retained integer bits",
                    source_and_witness_bits,
                    primitive_and_system_input_bits,
                )?,
                limits.max_potential_retained_integer_bits,
            )?;
        }
    }

    Ok(SourcePreflight {
        terminal,
        stats: ResidualAffineBranchSystemStats {
            family_fingerprint_bytes: family.fingerprint_ref().len(),
            context_fingerprint_bytes: context.fingerprint().len(),
            scope_fingerprint_comparison_bytes,
            source_identity_bytes_referenced: source_identity_bytes,
            zero_atoms,
            nonzero_guards,
            structural_atom_lookups: all_atoms,
            referenced_atom_terms,
            zero_atom_source_terms,
            zero_atom_exponent_entries,
            zero_atom_coefficient_bits,
            potential_block_witnesses,
            potential_block_exponent_entries,
            potential_retained_integer_bits,
            retained_atom_context_fingerprint_bytes,
            potential_row_components,
            manifest_entries,
            manifest_bytes,
            manifest_validation_comparisons,
            retained_nonzero_guard_ordinals: nonzero_guards,
            ..ResidualAffineBranchSystemStats::default()
        },
    })
}

fn compile_replayed(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    ready_terminal_ordinal: usize,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError> {
    let preflight = preflight_source(
        family,
        context,
        &source_cover,
        ready_terminal_ordinal,
        limits,
    )?;
    let zero_ordinals = preflight.terminal.equal_zero_atoms();
    let guard_ordinals = preflight.terminal.nonzero_atoms();
    let mut stats = preflight.stats;

    // Cover replay has authenticated this exact identity against the supplied
    // family. Its byte length was bounded by `preflight_source`; retain a
    // separately owned certificate copy through a fallible allocation seam.
    let family_fingerprint =
        try_copy_string(source_cover.family_fingerprint(), "family fingerprint")?;
    let context_fingerprint = try_copy_string(context.fingerprint(), "context fingerprint")?;
    let source_partition_identity = source_cover
        .source_extraction()
        .source_partition()
        .source_identity()
        .clone();

    let mut nonzero_guards = Vec::new();
    nonzero_guards
        .try_reserve_exact(guard_ordinals.len())
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "nonzero guard ordinals",
        })?;
    nonzero_guards.extend_from_slice(guard_ordinals);

    let coverage = source_cover.source_queue().discovery().coverage();
    let payload = compile_zero_atom_payload(
        context,
        zero_ordinals.len(),
        zero_ordinals.iter().map(|&ordinal| {
            coverage
                .structural_locus(ordinal)
                .map(|polynomial| (ordinal, polynomial))
                .ok_or(
                    ResidualAffineBranchSystemError::StructuralLocusOrdinalOutOfRange { ordinal },
                )
        }),
        limits,
        &mut stats,
    )?;

    let mut certificate = ResidualAffineBranchSystemCertificate {
        schema: RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA,
        family_fingerprint,
        context_fingerprint,
        source_partition_identity,
        source_cover,
        ready_terminal_ordinal,
        zero_atom_recognitions: payload.zero_atom_recognitions,
        nonzero_guard_locus_ordinals: nonzero_guards,
        integer_system: payload.integer_system,
        outcome: payload.outcome,
        limits,
        stats,
    };
    authenticate_branch_payload_comparison_stats(&mut certificate)?;
    Ok(certificate)
}

fn compile_fresh_replayed_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    ready_terminal_ordinal: usize,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<ResidualAffineBranchSystemFreshCompilation, ResidualAffineBranchSystemError> {
    // Derive the complete parent-visible envelope before entering any fresh
    // atom or integer-system allocation. This is intentionally independent of
    // the concrete terminal shape.
    let _memory_envelope = residual_affine_branch_system_memory_envelope_from_limits(limits)?;
    let preflight = preflight_source(
        family,
        context,
        &source_cover,
        ready_terminal_ordinal,
        limits,
    )?;
    let zero_ordinals = preflight.terminal.equal_zero_atoms();
    let guard_ordinals = preflight.terminal.nonzero_atoms();
    let mut stats = preflight.stats;

    let family_fingerprint =
        try_copy_string(source_cover.family_fingerprint(), "family fingerprint")?;
    let context_fingerprint = try_copy_string(context.fingerprint(), "context fingerprint")?;
    let source_partition_identity = source_cover
        .source_extraction()
        .source_partition()
        .source_identity()
        .clone();

    let mut nonzero_guards = Vec::new();
    nonzero_guards
        .try_reserve_exact(guard_ordinals.len())
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "nonzero guard ordinals",
        })?;
    nonzero_guards.extend_from_slice(guard_ordinals);

    let coverage = source_cover.source_queue().discovery().coverage();
    let payload = compile_zero_atom_payload_fresh(
        context,
        zero_ordinals.len(),
        zero_ordinals.iter().map(|&ordinal| {
            coverage
                .structural_locus(ordinal)
                .map(|polynomial| (ordinal, polynomial))
                .ok_or(
                    ResidualAffineBranchSystemError::StructuralLocusOrdinalOutOfRange { ordinal },
                )
        }),
        limits,
        &mut stats,
    )?;

    let FreshCompiledZeroAtomPayload {
        zero_atom_recognitions,
        integer_system,
        integer_plan_authorization,
        outcome,
        standalone_atom_attempt_owned_logical_peak_upper_bound,
        integer_system_compilation_owned_logical_peak_upper_bound,
        integer_system_raw_transient_census,
    } = payload;
    let mut certificate = ResidualAffineBranchSystemCertificate {
        schema: RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA,
        family_fingerprint,
        context_fingerprint,
        source_partition_identity,
        source_cover,
        ready_terminal_ordinal,
        zero_atom_recognitions,
        nonzero_guard_locus_ordinals: nonzero_guards,
        integer_system,
        outcome,
        limits,
        stats,
    };
    let payload_comparison_census = authenticate_branch_payload_comparison_stats(&mut certificate)?;
    let certificate = Arc::new(certificate);
    let memory = recompute_fresh_branch_memory_census(
        context,
        &certificate,
        Some(standalone_atom_attempt_owned_logical_peak_upper_bound),
        integer_system_compilation_owned_logical_peak_upper_bound,
        integer_system_raw_transient_census,
    )?;

    match integer_plan_authorization {
        Some(integer_plan_authorization) => {
            if !matches!(
                certificate.outcome(),
                ResidualAffineBranchSystemOutcome::GuardedAffineMap
            ) {
                return Err(ResidualAffineBranchSystemError::FreshAuthorizationOutcomeInvariant);
            }
            let retained_branch = Arc::clone(&certificate);
            let authorization = ResidualAffineBranchSystemFreshGuardAuthorization {
                source_cover: Arc::clone(certificate.source_cover()),
                branch: certificate,
                integer_plan_authorization,
                memory,
                payload_comparison_census,
                sealed_memory: memory,
                sealed_payload_comparison_census: payload_comparison_census,
            };
            if !Arc::ptr_eq(&retained_branch, &authorization.branch) {
                return Err(ResidualAffineBranchSystemError::FreshSourceBranchAllocationMismatch);
            }
            Ok(ResidualAffineBranchSystemFreshCompilation::Guarded(
                ResidualAffineBranchSystemFreshGuarded {
                    branch: retained_branch,
                    authorization,
                    memory,
                    payload_comparison_census,
                },
            ))
        }
        None => {
            if matches!(
                certificate.outcome(),
                ResidualAffineBranchSystemOutcome::GuardedAffineMap
            ) {
                return Err(ResidualAffineBranchSystemError::FreshAuthorizationOutcomeInvariant);
            }
            Ok(ResidualAffineBranchSystemFreshCompilation::Terminal(
                ResidualAffineBranchSystemFreshTerminal {
                    branch: certificate,
                    memory,
                    payload_comparison_census,
                },
            ))
        }
    }
}

struct FreshCompiledZeroAtomPayload {
    zero_atom_recognitions: Vec<ResidualAffineBranchZeroAtomRecognition>,
    integer_system: Option<Arc<ResidualAffineIntegerSystemCertificate>>,
    integer_plan_authorization: Option<ResidualAffineIntegerSystemFreshPlanAuthorization>,
    outcome: ResidualAffineBranchSystemOutcome,
    standalone_atom_attempt_owned_logical_peak_upper_bound: usize,
    integer_system_compilation_owned_logical_peak_upper_bound: usize,
    integer_system_raw_transient_census: Option<ResidualAffineIntegerSystemRawTransientCensus>,
}

fn compile_zero_atom_payload_fresh<'a>(
    context: &ParametricCoefficientContext,
    zero_atom_count: usize,
    sources: impl IntoIterator<
        Item = Result<(usize, &'a crate::ParametricPolynomial), ResidualAffineBranchSystemError>,
    >,
    limits: ResidualAffineBranchSystemLimits,
    stats: &mut ResidualAffineBranchSystemStats,
) -> Result<FreshCompiledZeroAtomPayload, ResidualAffineBranchSystemError> {
    let mut zero_atom_recognitions = Vec::new();
    zero_atom_recognitions
        .try_reserve_exact(zero_atom_count)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "zero-atom recognition manifest",
        })?;
    let mut system_inputs = Vec::new();
    system_inputs
        .try_reserve_exact(zero_atom_count)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "affine-system inputs",
        })?;
    check_limit(
        "unsupported reasons",
        zero_atom_count,
        limits.max_unsupported_reasons,
    )?;
    let mut unsupported_reasons = Vec::new();
    unsupported_reasons
        .try_reserve_exact(zero_atom_count)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "unsupported reasons",
        })?;

    let mut first_inconsistent_constant = None;
    let mut standalone_atom_attempt_owned_logical_peak_upper_bound = 0usize;
    for source in sources {
        let (ordinal, polynomial) = source?;
        let source_terms = polynomial.raw().nterms();
        stats.atom_recognitions_attempted = checked_add(
            "atom recognitions attempted",
            stats.atom_recognitions_attempted,
            1,
        )?;
        preflight_zero_atom_source_copy(context, polynomial, ordinal, limits)?;
        let source_copy = polynomial
            .try_copy_authenticated_sparse_payload()
            .map_err(|resource| ResidualAffineBranchSystemError::AllocationFailure { resource })?;
        let fresh_attempt =
            ResidualAffineAtomRowCertificate::compile_fresh(context, source_copy, limits.atom_row)
                .map_err(|source| ResidualAffineBranchSystemError::AtomRow {
                    structural_locus_ordinal: ordinal,
                    source,
                })?;
        let attempt_memory = fresh_attempt.logical_memory_census();
        standalone_atom_attempt_owned_logical_peak_upper_bound =
            standalone_atom_attempt_owned_logical_peak_upper_bound
                .max(attempt_memory.owned_logical_peak_upper_bound());
        let outcome = match fresh_attempt {
            ResidualAffineAtomRowFreshCompilationAttempt::Complete {
                certificate,
                logical_memory_census: _,
            } => match certificate.outcome() {
                ResidualAffineAtomRowOutcome::Row => {
                    let row = certificate.row().ok_or(
                        ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
                            structural_locus_ordinal: ordinal,
                        },
                    )?;
                    let row = try_copy_primitive_row(row, ordinal, limits)?;
                    let mut lineage = Vec::new();
                    lineage.try_reserve_exact(1).map_err(|_| {
                        ResidualAffineBranchSystemError::AllocationFailure {
                            resource: "affine-system input lineage",
                        }
                    })?;
                    lineage.push(ordinal);
                    let input = ResidualAffineIntegerSystemInputRow::try_new(
                        row,
                        lineage,
                        limits.max_system_lineage_ordinals,
                    )
                    .map_err(|source| {
                        ResidualAffineBranchSystemError::SystemInput {
                            structural_locus_ordinal: ordinal,
                            source,
                        }
                    })?;
                    stats.recognized_rows =
                        checked_add("recognized rows", stats.recognized_rows, 1)?;
                    stats.system_input_rows =
                        checked_add("system input rows", stats.system_input_rows, 1)?;
                    stats.system_lineage_ordinals = checked_add(
                        "system lineage ordinals",
                        stats.system_lineage_ordinals,
                        input.structural_locus_ordinals().len(),
                    )?;
                    stats.retained_row_components = checked_add(
                        "retained row components",
                        stats.retained_row_components,
                        input.row().components().len(),
                    )?;
                    system_inputs.push(input);
                    ResidualAffineBranchZeroAtomOutcome::Row(certificate)
                }
                ResidualAffineAtomRowOutcome::RedundantZeroPolynomial => {
                    stats.redundant_zero_polynomials = checked_add(
                        "redundant zero polynomials",
                        stats.redundant_zero_polynomials,
                        1,
                    )?;
                    ResidualAffineBranchZeroAtomOutcome::RedundantZeroPolynomial(certificate)
                }
                ResidualAffineAtomRowOutcome::InconsistentNonzeroConstant => {
                    stats.inconsistent_nonzero_constants = checked_add(
                        "inconsistent nonzero constants",
                        stats.inconsistent_nonzero_constants,
                        1,
                    )?;
                    first_inconsistent_constant.get_or_insert(ordinal);
                    ResidualAffineBranchZeroAtomOutcome::InconsistentNonzeroConstant(certificate)
                }
            },
            ResidualAffineAtomRowFreshCompilationAttempt::Unsupported {
                reason,
                logical_memory_census: _,
            } => {
                stats.unsupported_atoms =
                    checked_add("unsupported atoms", stats.unsupported_atoms, 1)?;
                unsupported_reasons.push(ResidualAffineBranchUnsupportedReason::Atom {
                    structural_locus_ordinal: ordinal,
                    reason: reason.clone(),
                });
                ResidualAffineBranchZeroAtomOutcome::Unsupported(reason)
            }
        };
        zero_atom_recognitions.push(ResidualAffineBranchZeroAtomRecognition {
            structural_locus_ordinal: ordinal,
            source_terms,
            outcome,
        });
    }

    let mut integer_plan_authorization = None;
    let mut integer_system_compilation_owned_logical_peak_upper_bound = 0usize;
    let mut integer_system_raw_transient_census = None;
    let (integer_system, outcome) = if let Some(structural_locus_ordinal) =
        first_inconsistent_constant
    {
        (
            None,
            ResidualAffineBranchSystemOutcome::ProvedEmpty(
                ResidualAffineBranchEmptyReason::InconsistentNonzeroConstant {
                    structural_locus_ordinal,
                },
            ),
        )
    } else {
        match ResidualAffineIntegerSystemCertificate::compile_fresh(
            context.index_count(),
            &system_inputs,
            limits.integer_system,
        ) {
            Ok(ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh)) => {
                integer_system_compilation_owned_logical_peak_upper_bound =
                    fresh.compilation_owned_logical_peak_upper_bound();
                integer_system_raw_transient_census = Some(fresh.raw_transient_census());
                let (system, authorization) = fresh
                    .into_certificate_and_plan_authorization()
                    .map_err(ResidualAffineBranchSystemError::IntegerSystem)?;
                match system.outcome() {
                    ResidualAffineIntegerSystemOutcome::ProvedEmpty => (
                        Some(system),
                        ResidualAffineBranchSystemOutcome::ProvedEmpty(
                            ResidualAffineBranchEmptyReason::IntegerSystem,
                        ),
                    ),
                    ResidualAffineIntegerSystemOutcome::AffineMap => {
                        if unsupported_reasons.is_empty() {
                            integer_plan_authorization = Some(authorization);
                            (
                                Some(system),
                                ResidualAffineBranchSystemOutcome::GuardedAffineMap,
                            )
                        } else {
                            // The recognized subset may be useful for replay
                            // diagnostics, but its authorization is destroyed
                            // at this boundary and cannot reach guard planning.
                            drop(authorization);
                            (
                                Some(system),
                                ResidualAffineBranchSystemOutcome::Unsupported {
                                    reasons: unsupported_reasons,
                                },
                            )
                        }
                    }
                }
            }
            Ok(ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(unsupported)) => {
                integer_system_compilation_owned_logical_peak_upper_bound =
                    unsupported.compilation_owned_logical_peak_upper_bound();
                integer_system_raw_transient_census = Some(unsupported.raw_transient_census());
                stats.general_congruence_boundaries = checked_add(
                    "general congruence boundaries",
                    stats.general_congruence_boundaries,
                    1,
                )?;
                unsupported_reasons.push(ResidualAffineBranchUnsupportedReason::GeneralCongruence(
                    unsupported.reason(),
                ));
                (
                    None,
                    ResidualAffineBranchSystemOutcome::Unsupported {
                        reasons: unsupported_reasons,
                    },
                )
            }
            Err(source) => return Err(ResidualAffineBranchSystemError::IntegerSystem(source)),
        }
    };

    Ok(FreshCompiledZeroAtomPayload {
        zero_atom_recognitions,
        integer_system,
        integer_plan_authorization,
        outcome,
        standalone_atom_attempt_owned_logical_peak_upper_bound,
        integer_system_compilation_owned_logical_peak_upper_bound,
        integer_system_raw_transient_census,
    })
}

impl ResidualAffineBranchSystemFreshCompilation {
    /// Consume the private fresh proof using only allocation identities,
    /// outcome tags, and already-seeded scalar seals. The sole
    /// independent deep validation is deliberately deferred to the enclosing
    /// persisted terminal's final adjacent authentication.
    pub(crate) fn into_authenticated_parts(
        self,
        context: &ParametricCoefficientContext,
    ) -> Result<ResidualAffineBranchSystemFreshAuthenticatedParts, ResidualAffineBranchSystemError>
    {
        match self {
            Self::Guarded(fresh) => {
                authenticate_fresh_branch_proof_scalars(
                    context,
                    &fresh.branch,
                    fresh.memory,
                    fresh.payload_comparison_census,
                )?;
                if !Arc::ptr_eq(&fresh.branch, &fresh.authorization.branch)
                    || fresh.memory != fresh.authorization.memory
                    || fresh.payload_comparison_census
                        != fresh.authorization.payload_comparison_census
                    || fresh.authorization.memory != fresh.authorization.sealed_memory
                    || fresh.authorization.payload_comparison_census
                        != fresh.authorization.sealed_payload_comparison_census
                {
                    return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
                }
                Ok(ResidualAffineBranchSystemFreshAuthenticatedParts::Guarded {
                    branch: fresh.branch,
                    authorization: fresh.authorization,
                    memory: fresh.memory,
                })
            }
            Self::Terminal(fresh) => {
                authenticate_fresh_branch_proof_scalars(
                    context,
                    &fresh.branch,
                    fresh.memory,
                    fresh.payload_comparison_census,
                )?;
                if matches!(
                    fresh.branch.outcome(),
                    ResidualAffineBranchSystemOutcome::GuardedAffineMap
                ) {
                    return Err(
                        ResidualAffineBranchSystemError::FreshAuthorizationOutcomeInvariant,
                    );
                }
                Ok(
                    ResidualAffineBranchSystemFreshAuthenticatedParts::Terminal {
                        branch: fresh.branch,
                        memory: fresh.memory,
                    },
                )
            }
        }
    }
}

impl ResidualAffineBranchSystemFreshGuardAuthorization {
    pub(crate) fn into_authenticated_guard_sources(
        self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    ) -> Result<ResidualAffineBranchSystemFreshGuardSources, ResidualAffineBranchSystemError> {
        if !Arc::ptr_eq(expected_branch, &self.branch) {
            return Err(ResidualAffineBranchSystemError::FreshSourceBranchAllocationMismatch);
        }
        if !Arc::ptr_eq(self.branch.source_cover(), &self.source_cover) {
            return Err(ResidualAffineBranchSystemError::FreshSourceCoverAllocationMismatch);
        }
        if !matches!(
            self.branch.outcome(),
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ) {
            return Err(ResidualAffineBranchSystemError::FreshAuthorizationOutcomeInvariant);
        }
        let integer_system = self
            .branch
            .integer_system_arc()
            .ok_or(ResidualAffineBranchSystemError::FreshAuthorizationOutcomeInvariant)?;
        if !self
            .integer_plan_authorization
            .authenticates_certificate_allocation(integer_system)
        {
            return Err(ResidualAffineBranchSystemError::FreshIntegerSystemAllocationMismatch);
        }
        if self.memory != self.sealed_memory
            || self.payload_comparison_census != self.sealed_payload_comparison_census
        {
            return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
        }
        authenticate_fresh_branch_proof_scalars(
            context,
            &self.branch,
            self.memory,
            self.payload_comparison_census,
        )?;
        Ok(ResidualAffineBranchSystemFreshGuardSources {
            source_cover: self.source_cover,
            branch: self.branch,
            integer_plan_authorization: self.integer_plan_authorization,
            memory: self.memory,
        })
    }

    #[cfg(test)]
    pub(crate) fn tamper_memory_retained_for_test(&mut self) {
        self.memory.retained_owned_logical_bytes =
            self.memory.retained_owned_logical_bytes.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_memory_peak_for_test(&mut self) {
        self.memory.compilation_owned_logical_peak_upper_bound = self
            .memory
            .compilation_owned_logical_peak_upper_bound
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_nested_integer_raw_census_for_test(&mut self) {
        self.memory.integer_system_raw_transient_census = None;
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_units_for_test(&mut self) {
        self.payload_comparison_census.units =
            self.payload_comparison_census.units.saturating_add(1);
    }
}

pub(crate) struct ResidualAffineBranchSystemFreshGuardSources {
    pub(crate) source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    pub(crate) branch: Arc<ResidualAffineBranchSystemCertificate>,
    pub(crate) integer_plan_authorization: ResidualAffineIntegerSystemFreshPlanAuthorization,
    pub(crate) memory: ResidualAffineBranchSystemLogicalMemoryCensus,
}

fn authenticate_fresh_branch_proof_scalars(
    _context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    payload_comparison_census: BranchPayloadComparisonCensus,
) -> Result<(), ResidualAffineBranchSystemError> {
    if certificate.schema != RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    let stats_census = BranchPayloadComparisonCensus {
        units: certificate.stats.payload_comparison_units,
        bytes: certificate.stats.payload_comparison_bytes,
        integer_bits: certificate.stats.payload_comparison_integer_bits,
    };
    let envelope = residual_affine_branch_system_memory_envelope_from_limits(certificate.limits)?;
    if payload_comparison_census != stats_census
        || memory.retained_owned_logical_bytes() == 0
        || memory.compilation_owned_logical_peak_upper_bound()
            < memory.retained_owned_logical_bytes()
        || memory.retained_owned_logical_bytes()
            > envelope.retained_owned_logical_bytes_upper_bound()
        || memory.compilation_owned_logical_peak_upper_bound()
            > envelope.compilation_owned_logical_peak_upper_bound()
    {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    Ok(())
}

fn recompute_fresh_branch_memory_census(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
    expected_standalone_atom_attempt_owned_logical_peak_upper_bound: Option<usize>,
    integer_system_compilation_owned_logical_peak_upper_bound: usize,
    integer_system_raw_transient_census: Option<ResidualAffineIntegerSystemRawTransientCensus>,
) -> Result<ResidualAffineBranchSystemLogicalMemoryCensus, ResidualAffineBranchSystemError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_BRANCH_MEMORY_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let retained_owned_logical_bytes = branch_retained_owned_logical_bytes(certificate)?;
    let integer_retained = certificate
        .retained_integer_system_for_payload_comparison()
        .as_deref()
        .map(|system| {
            system
                .recompute_retained_owned_logical_bytes_upper_bound()
                .map_err(ResidualAffineBranchSystemError::IntegerSystem)
        })
        .transpose()?
        .unwrap_or(0);
    let local_retained = retained_owned_logical_bytes
        .checked_sub(integer_retained)
        .ok_or(ResidualAffineBranchSystemError::MemoryCensusInvariant)?;
    let (
        atom_attempt_owned_logical_peak_upper_bound,
        standalone_atom_attempt_owned_logical_peak_upper_bound,
    ) = branch_ordered_atom_attempt_peak(context, certificate)?;
    if expected_standalone_atom_attempt_owned_logical_peak_upper_bound
        .is_some_and(|expected| expected != standalone_atom_attempt_owned_logical_peak_upper_bound)
    {
        return Err(ResidualAffineBranchSystemError::MemoryCensusInvariant);
    }
    let derived_integer_peak = match integer_system_raw_transient_census {
        Some(raw) => residual_affine_integer_system_compilation_owned_logical_peak_from_census(
            raw,
            integer_retained,
        )
        .map_err(ResidualAffineBranchSystemError::IntegerSystem)?,
        None => 0,
    };
    if derived_integer_peak != integer_system_compilation_owned_logical_peak_upper_bound {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    let input_temporary = branch_system_input_temporary_logical_bytes(certificate)?;
    let integer_overlap = checked_add(
        "fresh integer-system owned logical peak",
        checked_add(
            "fresh integer-system owned logical peak",
            local_retained,
            input_temporary,
        )?,
        integer_system_compilation_owned_logical_peak_upper_bound,
    )?;
    let compilation_owned_logical_peak_upper_bound = retained_owned_logical_bytes
        .max(atom_attempt_owned_logical_peak_upper_bound)
        .max(integer_overlap);
    let memory = ResidualAffineBranchSystemLogicalMemoryCensus {
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
        atom_attempt_owned_logical_peak_upper_bound,
        integer_system_compilation_owned_logical_peak_upper_bound,
        integer_system_raw_transient_census,
    };
    Ok(memory)
}

fn authenticate_fresh_branch_adjacent_census(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    retained_payload_comparison_census: Option<BranchPayloadComparisonCensus>,
) -> Result<BranchPayloadComparisonCensus, ResidualAffineBranchSystemError> {
    authenticate_fresh_branch_scope(context, certificate)?;
    let envelope = residual_affine_branch_system_memory_envelope_from_limits(certificate.limits)?;
    if memory.retained_owned_logical_bytes() > envelope.retained_owned_logical_bytes_upper_bound()
        || memory.compilation_owned_logical_peak_upper_bound()
            > envelope.compilation_owned_logical_peak_upper_bound()
    {
        return Err(ResidualAffineBranchSystemError::MemoryCensusInvariant);
    }
    let expected = recompute_fresh_branch_memory_census(
        context,
        certificate,
        None,
        memory.integer_system_compilation_owned_logical_peak_upper_bound,
        memory.integer_system_raw_transient_census,
    )?;
    let expected_payload = branch_equal_payload_comparison_census(certificate)?;
    if expected != memory
        || retained_payload_comparison_census.is_some_and(|retained| retained != expected_payload)
    {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    Ok(expected_payload)
}

fn authenticate_fresh_branch_scope(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
) -> Result<(), ResidualAffineBranchSystemError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_BRANCH_SCOPE_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    if certificate.schema != RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA {
        return Err(ResidualAffineBranchSystemError::SchemaMismatch);
    }
    let family_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        &certificate.family_fingerprint,
        certificate.source_cover.family_fingerprint(),
        certificate.limits.max_family_fingerprint_bytes,
    )?;
    let context_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        &certificate.context_fingerprint,
        context.fingerprint(),
        certificate.limits.max_context_fingerprint_bytes,
    )?;
    let scope_bytes = checked_add(
        "scope fingerprint comparison bytes",
        family_bytes,
        context_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        scope_bytes,
        certificate.limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if scope_bytes != certificate.stats.scope_fingerprint_comparison_bytes {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    if certificate.family_fingerprint != certificate.source_cover.family_fingerprint() {
        return Err(ResidualAffineBranchSystemError::WrongFamily);
    }
    if certificate.context_fingerprint != context.fingerprint() {
        return Err(ResidualAffineBranchSystemError::WrongContext);
    }
    Ok(())
}

pub(crate) fn authenticate_residual_affine_branch_fresh_memory_census(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
    memory: ResidualAffineBranchSystemLogicalMemoryCensus,
) -> Result<(), ResidualAffineBranchSystemError> {
    authenticate_fresh_branch_adjacent_census(context, certificate, memory, None).map(|_| ())
}

/// Conservative prospective branch memory from the existing V1 hard limits.
/// Every product and sum is checked; allocator capacity and shared child-cover
/// payloads never enter the result.
pub(crate) fn residual_affine_branch_system_memory_envelope_from_limits(
    limits: ResidualAffineBranchSystemLimits,
) -> Result<ResidualAffineBranchSystemMemoryEnvelope, ResidualAffineBranchSystemError> {
    let resource = "branch logical memory envelope";
    let word = size_of::<usize>();
    let zero_atoms = limits.max_zero_atoms;
    let guards = limits.max_nonzero_guards;
    let source_coefficients = limits.max_zero_atom_source_terms;
    let source_exponents = limits.max_zero_atom_exponent_entries;
    let witnesses = limits.max_potential_block_witnesses;
    let witness_exponents = limits.max_potential_block_exponent_entries;
    let atom_fingerprint_bytes = limits.max_retained_atom_context_fingerprint_bytes;
    // `max_potential_row_components` charges the atom-certificate primitive
    // row and its independently owned integer-system input copy.
    let retained_row_components = limits.max_potential_row_components / 2;
    let unsupported_reasons = limits.max_unsupported_reasons;
    let retained_integer_bits = limits.max_potential_retained_integer_bits;
    let retained_integer_entries = checked_add(
        resource,
        checked_add(resource, source_coefficients, retained_row_components)?,
        witnesses,
    )?;
    let retained_gmp =
        branch_gmp_logical_bytes_upper_bound(retained_integer_entries, retained_integer_bits)?;
    let local_retained = [
        branch_arc_owned_logical_bytes::<ResidualAffineBranchSystemCertificate>()?,
        limits.max_family_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
        checked_mul(
            resource,
            zero_atoms,
            size_of::<ResidualAffineBranchZeroAtomRecognition>(),
        )?,
        checked_mul(resource, guards, word)?,
        atom_fingerprint_bytes,
        checked_mul(resource, source_coefficients, size_of::<Integer>())?,
        checked_mul(resource, source_exponents, size_of::<u16>())?,
        checked_mul(resource, retained_row_components, size_of::<Integer>())?,
        checked_mul(
            resource,
            witnesses,
            size_of::<crate::ResidualAffineBaseBlockWitness>(),
        )?,
        checked_mul(resource, witness_exponents, size_of::<u16>())?,
        checked_mul(
            resource,
            unsupported_reasons,
            size_of::<ResidualAffineBranchUnsupportedReason>(),
        )?,
        retained_gmp,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;

    let integer_system =
        residual_affine_integer_system_memory_envelope_from_limits(limits.integer_system)
            .map_err(ResidualAffineBranchSystemError::IntegerSystem)?;
    let retained_owned_logical_bytes_upper_bound = checked_add(
        resource,
        local_retained,
        integer_system.retained_owned_logical_bytes_upper_bound(),
    )?;

    let system_input_rows = zero_atoms.min(limits.max_system_input_rows);
    let system_lineage_ordinals = system_input_rows.min(limits.max_system_lineage_ordinals);
    let input_temporary = [
        size_of::<Vec<ResidualAffineIntegerSystemInputRow>>(),
        checked_mul(
            resource,
            system_input_rows,
            size_of::<ResidualAffineIntegerSystemInputRow>(),
        )?,
        checked_mul(resource, retained_row_components, size_of::<Integer>())?,
        checked_mul(resource, system_lineage_ordinals, word)?,
        branch_gmp_logical_bytes_upper_bound(retained_row_components, retained_integer_bits)?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;

    let atom_extra = if zero_atoms == 0 {
        0
    } else {
        let components = limits.atom_row.max_primitive_row_components;
        let base_variables = limits.atom_row.max_base_variables;
        let coefficient_bits = limits.atom_row.max_integer_coefficient_bits;
        let temporary_integer_entries = checked_add(resource, components, 4)?;
        let temporary_integer_bits =
            checked_mul(resource, temporary_integer_entries, coefficient_bits)?;
        [
            size_of::<ResidualAffineAtomRowFreshCompilationAttempt>(),
            size_of::<ResidualAffineAtomRowAttemptLogicalMemoryCensus>(),
            size_of::<Vec<Option<usize>>>(),
            checked_mul(resource, components, size_of::<Option<usize>>())?,
            size_of::<Vec<crate::ResidualAffineBaseBlockWitness>>(),
            size_of::<Option<ResidualAffinePrimitiveRow>>(),
            size_of::<ResidualAffinePrimitiveRow>(),
            checked_mul(resource, components, size_of::<Integer>())?,
            size_of::<Vec<u16>>(),
            checked_mul(resource, base_variables, size_of::<u16>())?,
            checked_mul(resource, 4, size_of::<Integer>())?,
            branch_gmp_logical_bytes_upper_bound(
                temporary_integer_entries,
                temporary_integer_bits,
            )?,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?
    };
    let atom_phase = checked_add(
        resource,
        checked_add(resource, local_retained, input_temporary)?,
        atom_extra,
    )?;
    let integer_phase = checked_add(
        resource,
        checked_add(resource, local_retained, input_temporary)?,
        integer_system.compilation_owned_logical_peak_upper_bound(),
    )?;
    let compilation_owned_logical_peak_upper_bound = retained_owned_logical_bytes_upper_bound
        .max(atom_phase)
        .max(integer_phase);
    Ok(ResidualAffineBranchSystemMemoryEnvelope {
        retained_owned_logical_bytes_upper_bound,
        compilation_owned_logical_peak_upper_bound,
    })
}

fn branch_gmp_logical_bytes_upper_bound(
    integer_entries: usize,
    total_integer_bits: usize,
) -> Result<usize, ResidualAffineBranchSystemError> {
    checked_add(
        "branch GMP logical bytes",
        logical_bytes_for_bits(total_integer_bits),
        checked_add(
            "branch GMP logical bytes",
            checked_mul(
                "branch GMP logical bytes",
                integer_entries,
                size_of::<usize>(),
            )?,
            integer_entries.saturating_sub(1),
        )?,
    )
}

fn branch_ordered_atom_attempt_peak(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchSystemCertificate,
) -> Result<(usize, usize), ResidualAffineBranchSystemError> {
    // At attempt i, only recognitions and integer-system inputs from attempts
    // [0, i) are live. The standalone atom helper already includes the exact
    // in-progress source clone, partial witnesses/common/current rows, and its
    // scratch, so that payload is not charged again in the prefix.
    let mut outer_prefix =
        branch_arc_owned_logical_bytes::<ResidualAffineBranchSystemCertificate>()?;
    outer_prefix = checked_add(
        "fresh atom-attempt prefix bytes",
        outer_prefix,
        certificate.family_fingerprint.len(),
    )?;
    outer_prefix = checked_add(
        "fresh atom-attempt prefix bytes",
        outer_prefix,
        certificate.context_fingerprint.len(),
    )?;
    outer_prefix = checked_add(
        "fresh atom-attempt prefix bytes",
        outer_prefix,
        checked_mul(
            "fresh atom-attempt guard bytes",
            certificate.nonzero_guard_locus_ordinals.len(),
            size_of::<usize>(),
        )?,
    )?;
    let coverage = certificate
        .source_cover
        .source_queue()
        .discovery()
        .coverage();
    let mut retained_prefix = 0usize;
    let mut input_prefix = 0usize;
    let mut peak = outer_prefix;
    let mut standalone_peak = 0usize;
    for recognition in &certificate.zero_atom_recognitions {
        let ordinal = recognition.structural_locus_ordinal;
        let source = coverage
            .structural_locus(ordinal)
            .ok_or(ResidualAffineBranchSystemError::StructuralLocusOrdinalOutOfRange { ordinal })?;
        let standalone = residual_affine_atom_row_attempt_logical_memory_census(
            context,
            source,
            certificate.limits.atom_row,
        )
        .map_err(|source| ResidualAffineBranchSystemError::AtomRow {
            structural_locus_ordinal: ordinal,
            source,
        })?
        .owned_logical_peak_upper_bound();
        standalone_peak = standalone_peak.max(standalone);
        let mut current_outer_overlap = size_of::<ResidualAffineBranchZeroAtomRecognition>();
        match &recognition.outcome {
            ResidualAffineBranchZeroAtomOutcome::Row(atom) => {
                let row =
                    atom.row()
                        .ok_or(ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
                            structural_locus_ordinal: ordinal,
                        })?;
                current_outer_overlap = checked_add(
                    "fresh atom-attempt current input overlap",
                    current_outer_overlap,
                    system_input_row_logical_bytes(row)?,
                )?;
            }
            ResidualAffineBranchZeroAtomOutcome::Unsupported(_) => {
                current_outer_overlap = checked_add(
                    "fresh atom-attempt current unsupported overlap",
                    current_outer_overlap,
                    size_of::<ResidualAffineBranchUnsupportedReason>(),
                )?;
            }
            ResidualAffineBranchZeroAtomOutcome::RedundantZeroPolynomial(_)
            | ResidualAffineBranchZeroAtomOutcome::InconsistentNonzeroConstant(_) => {}
        }
        let candidate = checked_add(
            "fresh atom-attempt ordered peak",
            checked_add(
                "fresh atom-attempt ordered peak",
                outer_prefix,
                retained_prefix,
            )?,
            checked_add(
                "fresh atom-attempt ordered peak",
                input_prefix,
                checked_add(
                    "fresh atom-attempt ordered peak",
                    standalone,
                    current_outer_overlap,
                )?,
            )?,
        )?;
        peak = peak.max(candidate);

        retained_prefix = checked_add(
            "fresh atom-attempt retained prefix",
            retained_prefix,
            size_of::<ResidualAffineBranchZeroAtomRecognition>(),
        )?;
        if let Some(atom) = recognition.outcome.certificate() {
            retained_prefix = checked_add(
                "fresh atom-attempt retained prefix",
                retained_prefix,
                atom_certificate_dynamic_logical_bytes(atom)?,
            )?;
        } else {
            retained_prefix = checked_add(
                "fresh atom-attempt retained prefix",
                retained_prefix,
                size_of::<ResidualAffineBranchUnsupportedReason>(),
            )?;
        }
        if let ResidualAffineBranchZeroAtomOutcome::Row(atom) = &recognition.outcome {
            let row =
                atom.row()
                    .ok_or(ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
                        structural_locus_ordinal: ordinal,
                    })?;
            input_prefix = checked_add(
                "fresh atom-attempt input prefix",
                input_prefix,
                system_input_row_logical_bytes(row)?,
            )?;
        }
    }
    Ok((peak, standalone_peak))
}

fn branch_equal_payload_comparison_census(
    certificate: &ResidualAffineBranchSystemCertificate,
) -> Result<BranchPayloadComparisonCensus, ResidualAffineBranchSystemError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_BRANCH_LOCAL_COMPARISON_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let mut budget = BranchPayloadComparisonBudget::new(certificate.limits);
    branch_payload_operand_census(certificate, &mut budget)?;
    branch_payload_operand_census(certificate, &mut budget)?;
    let census = budget.census;
    if census.units != certificate.stats.payload_comparison_units
        || census.bytes != certificate.stats.payload_comparison_bytes
        || census.integer_bits != certificate.stats.payload_comparison_integer_bits
    {
        return Err(ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch);
    }
    Ok(census)
}

fn branch_retained_owned_logical_bytes(
    certificate: &ResidualAffineBranchSystemCertificate,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let mut bytes = branch_arc_owned_logical_bytes::<ResidualAffineBranchSystemCertificate>()?;
    bytes = checked_add(
        "branch retained owned logical bytes",
        bytes,
        certificate.family_fingerprint.len(),
    )?;
    bytes = checked_add(
        "branch retained owned logical bytes",
        bytes,
        certificate.context_fingerprint.len(),
    )?;
    bytes = checked_add(
        "branch retained owned logical bytes",
        bytes,
        checked_mul(
            "branch retained recognition bytes",
            certificate.zero_atom_recognitions.len(),
            size_of::<ResidualAffineBranchZeroAtomRecognition>(),
        )?,
    )?;
    bytes = checked_add(
        "branch retained owned logical bytes",
        bytes,
        checked_mul(
            "branch retained guard ordinal bytes",
            certificate.nonzero_guard_locus_ordinals.len(),
            size_of::<usize>(),
        )?,
    )?;
    for recognition in &certificate.zero_atom_recognitions {
        if let Some(atom) = recognition.outcome.certificate() {
            bytes = checked_add(
                "branch retained owned logical bytes",
                bytes,
                atom_certificate_dynamic_logical_bytes(atom)?,
            )?;
        }
    }
    if let ResidualAffineBranchSystemOutcome::Unsupported { reasons } = &certificate.outcome {
        bytes = checked_add(
            "branch retained owned logical bytes",
            bytes,
            checked_mul(
                "branch retained unsupported reason bytes",
                reasons.len(),
                size_of::<ResidualAffineBranchUnsupportedReason>(),
            )?,
        )?;
    }
    if let Some(system) = certificate
        .retained_integer_system_for_payload_comparison()
        .as_deref()
    {
        bytes = checked_add(
            "branch retained owned logical bytes",
            bytes,
            system
                .recompute_retained_owned_logical_bytes_upper_bound()
                .map_err(ResidualAffineBranchSystemError::IntegerSystem)?,
        )?;
    }
    Ok(bytes)
}

fn branch_system_input_temporary_logical_bytes(
    certificate: &ResidualAffineBranchSystemCertificate,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let row_count = certificate
        .zero_atom_recognitions
        .iter()
        .filter(|recognition| {
            matches!(
                recognition.outcome,
                ResidualAffineBranchZeroAtomOutcome::Row(_)
            )
        })
        .count();
    let mut bytes = checked_mul(
        "branch temporary system-input bytes",
        row_count,
        size_of::<ResidualAffineIntegerSystemInputRow>(),
    )?;
    for recognition in &certificate.zero_atom_recognitions {
        let ResidualAffineBranchZeroAtomOutcome::Row(atom) = &recognition.outcome else {
            continue;
        };
        let row = atom
            .row()
            .ok_or(ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
                structural_locus_ordinal: recognition.structural_locus_ordinal,
            })?;
        bytes = checked_add(
            "branch temporary system-input bytes",
            bytes,
            system_input_row_dynamic_logical_bytes(row)?,
        )?;
    }
    Ok(bytes)
}

fn system_input_row_logical_bytes(
    row: &ResidualAffinePrimitiveRow,
) -> Result<usize, ResidualAffineBranchSystemError> {
    checked_add(
        "branch temporary system-input row bytes",
        size_of::<ResidualAffineIntegerSystemInputRow>(),
        system_input_row_dynamic_logical_bytes(row)?,
    )
}

fn system_input_row_dynamic_logical_bytes(
    row: &ResidualAffinePrimitiveRow,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let mut bytes = checked_add(
        "branch temporary system-input row bytes",
        checked_mul(
            "branch temporary system-input components",
            row.components().len(),
            size_of::<Integer>(),
        )?,
        size_of::<usize>(),
    )?;
    for value in row.components() {
        bytes = checked_add(
            "branch temporary system-input row bytes",
            bytes,
            large_integer_dynamic_logical_bytes(value)?,
        )?;
    }
    Ok(bytes)
}

fn atom_certificate_dynamic_logical_bytes(
    certificate: &ResidualAffineAtomRowCertificate,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let mut bytes = certificate.context_fingerprint().len();
    bytes = checked_add(
        "branch retained atom bytes",
        bytes,
        polynomial_dynamic_logical_bytes(certificate.source())?,
    )?;
    if let Some(row) = certificate.primitive_row() {
        bytes = checked_add(
            "branch retained atom bytes",
            bytes,
            checked_mul(
                "branch retained atom row bytes",
                row.components().len(),
                size_of::<Integer>(),
            )?,
        )?;
        for value in row.components() {
            bytes = checked_add(
                "branch retained atom bytes",
                bytes,
                large_integer_dynamic_logical_bytes(value)?,
            )?;
        }
    }
    bytes = checked_add(
        "branch retained atom bytes",
        bytes,
        checked_mul(
            "branch retained block-witness bytes",
            certificate.block_witnesses().len(),
            size_of::<crate::ResidualAffineBaseBlockWitness>(),
        )?,
    )?;
    for witness in certificate.block_witnesses() {
        bytes = checked_add(
            "branch retained atom bytes",
            bytes,
            checked_mul(
                "branch retained block-exponent bytes",
                witness.base_exponents().len(),
                size_of::<u16>(),
            )?,
        )?;
        bytes = checked_add(
            "branch retained atom bytes",
            bytes,
            large_integer_dynamic_logical_bytes(witness.signed_scalar())?,
        )?;
    }
    Ok(bytes)
}

fn polynomial_dynamic_logical_bytes(
    polynomial: &crate::ParametricPolynomial,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let raw = polynomial.raw();
    let mut bytes = checked_add(
        "branch retained polynomial bytes",
        checked_mul(
            "branch retained polynomial coefficient bytes",
            raw.coefficients.len(),
            size_of::<Integer>(),
        )?,
        checked_mul(
            "branch retained polynomial exponent bytes",
            raw.exponents.len(),
            size_of::<u16>(),
        )?,
    )?;
    for value in &raw.coefficients {
        bytes = checked_add(
            "branch retained polynomial bytes",
            bytes,
            large_integer_dynamic_logical_bytes(value)?,
        )?;
    }
    Ok(bytes)
}

fn large_integer_dynamic_logical_bytes(
    value: &Integer,
) -> Result<usize, ResidualAffineBranchSystemError> {
    if !matches!(value, Integer::Large(_)) {
        return Ok(0);
    }
    checked_add(
        "branch retained large-integer bytes",
        logical_bytes_for_bits(integer_magnitude_bits(value)?),
        size_of::<usize>(),
    )
}

fn branch_arc_owned_logical_bytes<T>() -> Result<usize, ResidualAffineBranchSystemError> {
    checked_add(
        "branch Arc owned logical bytes",
        checked_add(
            "branch Arc owned logical bytes",
            checked_mul("branch Arc owned logical bytes", 2, size_of::<usize>())?,
            align_of::<T>().saturating_sub(1),
        )?,
        size_of::<T>(),
    )
}

fn logical_bytes_for_bits(bits: usize) -> usize {
    bits / u8::BITS as usize + usize::from(bits % u8::BITS as usize != 0)
}

struct CompiledZeroAtomPayload {
    zero_atom_recognitions: Vec<ResidualAffineBranchZeroAtomRecognition>,
    integer_system: Option<Arc<ResidualAffineIntegerSystemCertificate>>,
    outcome: ResidualAffineBranchSystemOutcome,
}

fn compile_zero_atom_payload<'a>(
    context: &ParametricCoefficientContext,
    zero_atom_count: usize,
    sources: impl IntoIterator<
        Item = Result<(usize, &'a crate::ParametricPolynomial), ResidualAffineBranchSystemError>,
    >,
    limits: ResidualAffineBranchSystemLimits,
    stats: &mut ResidualAffineBranchSystemStats,
) -> Result<CompiledZeroAtomPayload, ResidualAffineBranchSystemError> {
    let mut zero_atom_recognitions = Vec::new();
    zero_atom_recognitions
        .try_reserve_exact(zero_atom_count)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "zero-atom recognition manifest",
        })?;
    let mut system_inputs = Vec::new();
    system_inputs
        .try_reserve_exact(zero_atom_count)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "affine-system inputs",
        })?;
    let unsupported_capacity = zero_atom_count;
    check_limit(
        "unsupported reasons",
        unsupported_capacity,
        limits.max_unsupported_reasons,
    )?;
    let mut unsupported_reasons = Vec::new();
    unsupported_reasons
        .try_reserve_exact(unsupported_capacity)
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "unsupported reasons",
        })?;

    let mut first_inconsistent_constant = None;
    for source in sources {
        let (ordinal, polynomial) = source?;
        let source_terms = polynomial.raw().nterms();
        stats.atom_recognitions_attempted += 1;
        preflight_zero_atom_source_copy(context, polynomial, ordinal, limits)?;
        let source_copy = polynomial
            .try_copy_authenticated_sparse_payload()
            .map_err(|resource| ResidualAffineBranchSystemError::AllocationFailure { resource })?;
        let outcome = match ResidualAffineAtomRowCertificate::compile(
            context,
            source_copy,
            limits.atom_row,
        ) {
            Ok(certificate) => match certificate.outcome() {
                ResidualAffineAtomRowOutcome::Row => {
                    let row = certificate.row().ok_or(
                        ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
                            structural_locus_ordinal: ordinal,
                        },
                    )?;
                    let row = try_copy_primitive_row(row, ordinal, limits)?;
                    let mut lineage = Vec::new();
                    lineage.try_reserve_exact(1).map_err(|_| {
                        ResidualAffineBranchSystemError::AllocationFailure {
                            resource: "affine-system input lineage",
                        }
                    })?;
                    lineage.push(ordinal);
                    let input = ResidualAffineIntegerSystemInputRow::try_new(
                        row,
                        lineage,
                        limits.max_system_lineage_ordinals,
                    )
                    .map_err(|source| {
                        ResidualAffineBranchSystemError::SystemInput {
                            structural_locus_ordinal: ordinal,
                            source,
                        }
                    })?;
                    stats.recognized_rows += 1;
                    stats.system_input_rows += 1;
                    stats.system_lineage_ordinals += input.structural_locus_ordinals().len();
                    stats.retained_row_components = checked_add(
                        "retained row components",
                        stats.retained_row_components,
                        input.row().components().len(),
                    )?;
                    system_inputs.push(input);
                    ResidualAffineBranchZeroAtomOutcome::Row(certificate)
                }
                ResidualAffineAtomRowOutcome::RedundantZeroPolynomial => {
                    stats.redundant_zero_polynomials += 1;
                    ResidualAffineBranchZeroAtomOutcome::RedundantZeroPolynomial(certificate)
                }
                ResidualAffineAtomRowOutcome::InconsistentNonzeroConstant => {
                    stats.inconsistent_nonzero_constants += 1;
                    first_inconsistent_constant.get_or_insert(ordinal);
                    ResidualAffineBranchZeroAtomOutcome::InconsistentNonzeroConstant(certificate)
                }
            },
            Err(ResidualAffineAtomRowError::Unsupported { reason }) => {
                stats.unsupported_atoms += 1;
                unsupported_reasons.push(ResidualAffineBranchUnsupportedReason::Atom {
                    structural_locus_ordinal: ordinal,
                    reason: reason.clone(),
                });
                ResidualAffineBranchZeroAtomOutcome::Unsupported(reason)
            }
            Err(source) => {
                return Err(ResidualAffineBranchSystemError::AtomRow {
                    structural_locus_ordinal: ordinal,
                    source,
                });
            }
        };
        zero_atom_recognitions.push(ResidualAffineBranchZeroAtomRecognition {
            structural_locus_ordinal: ordinal,
            source_terms,
            outcome,
        });
    }

    let (integer_system, outcome) =
        if let Some(structural_locus_ordinal) = first_inconsistent_constant {
            // Every zero atom above was still recognized. A single authenticated
            // nonzero constant is already a complete inconsistency proof, so no
            // weaker unsupported boundary may override it.
            (
                None,
                ResidualAffineBranchSystemOutcome::ProvedEmpty(
                    ResidualAffineBranchEmptyReason::InconsistentNonzeroConstant {
                        structural_locus_ordinal,
                    },
                ),
            )
        } else {
            match ResidualAffineIntegerSystemCertificate::compile(
                context.index_count(),
                &system_inputs,
                limits.integer_system,
            ) {
                Ok(system) => {
                    let system = Arc::new(system);
                    match system.outcome() {
                        ResidualAffineIntegerSystemOutcome::ProvedEmpty => (
                            Some(system),
                            ResidualAffineBranchSystemOutcome::ProvedEmpty(
                                ResidualAffineBranchEmptyReason::IntegerSystem,
                            ),
                        ),
                        ResidualAffineIntegerSystemOutcome::AffineMap => {
                            if unsupported_reasons.is_empty() {
                                (
                                    Some(system),
                                    ResidualAffineBranchSystemOutcome::GuardedAffineMap,
                                )
                            } else {
                                (
                                    Some(system),
                                    ResidualAffineBranchSystemOutcome::Unsupported {
                                        reasons: unsupported_reasons,
                                    },
                                )
                            }
                        }
                    }
                }
                Err(ResidualAffineIntegerSystemError::Unsupported(reason)) => {
                    stats.general_congruence_boundaries += 1;
                    unsupported_reasons.push(
                        ResidualAffineBranchUnsupportedReason::GeneralCongruence(reason),
                    );
                    (
                        None,
                        ResidualAffineBranchSystemOutcome::Unsupported {
                            reasons: unsupported_reasons,
                        },
                    )
                }
                Err(source) => return Err(ResidualAffineBranchSystemError::IntegerSystem(source)),
            }
        };

    Ok(CompiledZeroAtomPayload {
        zero_atom_recognitions,
        integer_system,
        outcome,
    })
}

fn option_system_payload_eq_checked(
    left: &Option<Arc<ResidualAffineIntegerSystemCertificate>>,
    right: &Option<Arc<ResidualAffineIntegerSystemCertificate>>,
) -> Result<bool, ResidualAffineBranchSystemError> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if Arc::ptr_eq(left, right) {
                Ok(true)
            } else {
                left.payload_eq_checked(right)
                    .map_err(ResidualAffineBranchSystemError::IntegerSystem)
            }
        }
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn preflight_branch_payload_comparison(
    retained: &ResidualAffineBranchSystemCertificate,
    supplied: &ResidualAffineBranchSystemCertificate,
) -> Result<(), ResidualAffineBranchSystemError> {
    let limits = retained.limits;
    let family_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        &retained.family_fingerprint,
        &supplied.family_fingerprint,
        limits.max_family_fingerprint_bytes,
    )?;
    let context_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        &retained.context_fingerprint,
        &supplied.context_fingerprint,
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        checked_add(
            "scope fingerprint comparison bytes",
            family_bytes,
            context_bytes,
        )?,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    for identity in [
        retained.source_partition_identity.as_ref(),
        supplied.source_partition_identity.as_ref(),
    ] {
        check_limit(
            "source identity bytes referenced",
            identity.len(),
            limits.max_source_identity_bytes_referenced,
        )?;
    }
    for certificate in [retained, supplied] {
        check_limit(
            "zero atoms",
            certificate.zero_atom_recognitions.len(),
            limits.max_zero_atoms,
        )?;
        check_limit(
            "nonzero guards",
            certificate.nonzero_guard_locus_ordinals.len(),
            limits.max_nonzero_guards,
        )?;
    }

    // Recompute both actual retained shapes without allocation before any
    // branch-owned deep equality. Stored stats are authenticated payload, not
    // trusted as the budget source. One live budget spans both operands, so an
    // oversized independently supplied payload is rejected while it is being
    // censused rather than after a complete adversarial traversal.
    let mut comparison_budget = BranchPayloadComparisonBudget::new(limits);
    branch_payload_operand_census(retained, &mut comparison_budget)?;
    branch_payload_operand_census(supplied, &mut comparison_budget)
}

fn preflight_zero_atom_source_copy(
    context: &ParametricCoefficientContext,
    source: &crate::ParametricPolynomial,
    structural_locus_ordinal: usize,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<(), ResidualAffineBranchSystemError> {
    context
        .validate_polynomial_with_limits(source, limits.atom_row.exact_algebra)
        .map_err(|source| ResidualAffineBranchSystemError::AtomRow {
            structural_locus_ordinal,
            source: source.into(),
        })?;
    check_limit(
        "zero-atom source-copy terms",
        source.raw().nterms(),
        limits.atom_row.max_source_terms,
    )?;
    let exponent_entries = checked_mul(
        "zero-atom source-copy exponent entries",
        source.raw().nterms(),
        source.raw().variables.len(),
    )?;
    check_limit(
        "zero-atom source-copy exponent entries",
        exponent_entries,
        limits.atom_row.max_exponent_entries_inspected,
    )?;
    let mut bit_payload = 0usize;
    for coefficient in &source.raw().coefficients {
        let bits = integer_magnitude_bits(coefficient)?;
        check_limit(
            "zero-atom source-copy coefficient bits",
            bits,
            limits.atom_row.max_integer_coefficient_bits,
        )?;
        bit_payload = bounded_add(
            "zero-atom source-copy integer bits",
            bit_payload,
            bits,
            limits.max_potential_retained_integer_bits,
        )?;
    }
    Ok(())
}

fn certified_integer_system_for_outcome<'a>(
    outcome: &ResidualAffineBranchSystemOutcome,
    system: &'a Option<Arc<ResidualAffineIntegerSystemCertificate>>,
) -> Option<&'a ResidualAffineIntegerSystemCertificate> {
    certified_integer_system_arc_for_outcome(outcome, system).map(Arc::as_ref)
}

fn certified_integer_system_arc_for_outcome<'a>(
    outcome: &ResidualAffineBranchSystemOutcome,
    system: &'a Option<Arc<ResidualAffineIntegerSystemCertificate>>,
) -> Option<&'a Arc<ResidualAffineIntegerSystemCertificate>> {
    matches!(
        outcome,
        ResidualAffineBranchSystemOutcome::GuardedAffineMap
            | ResidualAffineBranchSystemOutcome::ProvedEmpty(
                ResidualAffineBranchEmptyReason::IntegerSystem
            )
    )
    .then_some(())
    .and_then(|_| system.as_ref())
}

fn try_copy_primitive_row(
    source: &ResidualAffinePrimitiveRow,
    structural_locus_ordinal: usize,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<ResidualAffinePrimitiveRow, ResidualAffineBranchSystemError> {
    check_limit(
        "integer-system input row components",
        source.components().len(),
        limits.atom_row.max_primitive_row_components,
    )?;
    let mut bit_payload = 0usize;
    for component in source.components() {
        let bits = integer_magnitude_bits(component)?;
        check_limit(
            "integer-system input coefficient bits",
            bits,
            limits.atom_row.max_integer_coefficient_bits,
        )?;
        bit_payload = bounded_add(
            "integer-system input integer bits",
            bit_payload,
            bits,
            limits.max_potential_retained_integer_bits,
        )?;
    }

    let mut components = Vec::new();
    components
        .try_reserve_exact(source.components().len())
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure {
            resource: "integer-system input row components",
        })?;
    components.extend(source.components().iter().cloned());
    ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
        components,
        limits.atom_row.max_primitive_row_components,
        limits.atom_row.max_integer_coefficient_bits,
        limits.atom_row.max_integer_bit_work,
    )
    .map_err(
        |_| ResidualAffineBranchSystemError::AtomRowOutcomeInvariant {
            structural_locus_ordinal,
        },
    )
}

fn validate_scope(
    schema: &'static str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    limits: ResidualAffineBranchSystemLimits,
) -> Result<(), ResidualAffineBranchSystemError> {
    if schema != RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA {
        return Err(ResidualAffineBranchSystemError::SchemaMismatch);
    }
    let family_comparison_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        family_fingerprint,
        family.fingerprint_ref(),
        limits.max_family_fingerprint_bytes,
    )?;
    let context_comparison_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        context_fingerprint,
        context.fingerprint(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        checked_add(
            "scope fingerprint comparison bytes",
            family_comparison_bytes,
            context_comparison_bytes,
        )?,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if family_fingerprint != family.fingerprint_ref() {
        return Err(ResidualAffineBranchSystemError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint() {
        return Err(ResidualAffineBranchSystemError::WrongContext);
    }
    Ok(())
}

fn preflight_fingerprint_pair(
    resource: &'static str,
    retained: &str,
    supplied: &str,
    one_fingerprint_limit: usize,
) -> Result<usize, ResidualAffineBranchSystemError> {
    check_limit(resource, retained.len(), one_fingerprint_limit)?;
    check_limit(resource, supplied.len(), one_fingerprint_limit)?;
    checked_add(
        "scope fingerprint comparison bytes",
        retained.len(),
        supplied.len(),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BranchPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

impl BranchPayloadComparisonCensus {
    fn checked_add(self, other: Self) -> Result<Self, ResidualAffineBranchSystemError> {
        Ok(Self {
            units: checked_add("payload comparison units", self.units, other.units)?,
            bytes: checked_add("payload comparison bytes", self.bytes, other.bytes)?,
            integer_bits: checked_add(
                "payload comparison integer bits",
                self.integer_bits,
                other.integer_bits,
            )?,
        })
    }
}

/// Live, directional comparison budget derived from the retained certificate.
///
/// Every charge is checked before it is committed. `preflight_repeated` uses
/// only a slice's already-retained length and representation size, allowing an
/// oversized nested operand to fail before its first element is visited. Exact
/// integer magnitudes are then charged one at a time before traversal can
/// continue to the next value.
struct BranchPayloadComparisonBudget {
    limits: ResidualAffineBranchSystemLimits,
    census: BranchPayloadComparisonCensus,
}

impl BranchPayloadComparisonBudget {
    fn new(limits: ResidualAffineBranchSystemLimits) -> Self {
        Self {
            limits,
            census: BranchPayloadComparisonCensus::default(),
        }
    }

    fn preflight(
        &self,
        additional: BranchPayloadComparisonCensus,
    ) -> Result<BranchPayloadComparisonCensus, ResidualAffineBranchSystemError> {
        let requested = self.census.checked_add(additional)?;
        check_limit(
            "payload comparison units",
            requested.units,
            self.limits.max_payload_comparison_units,
        )?;
        check_limit(
            "payload comparison bytes",
            requested.bytes,
            self.limits.max_payload_comparison_bytes,
        )?;
        check_limit(
            "payload comparison integer bits",
            requested.integer_bits,
            self.limits.max_payload_comparison_integer_bits,
        )?;
        Ok(requested)
    }

    fn charge(
        &mut self,
        additional: BranchPayloadComparisonCensus,
    ) -> Result<(), ResidualAffineBranchSystemError> {
        self.census = self.preflight(additional)?;
        Ok(())
    }

    fn add_units(&mut self, additional: usize) -> Result<(), ResidualAffineBranchSystemError> {
        self.charge(BranchPayloadComparisonCensus {
            units: additional,
            ..BranchPayloadComparisonCensus::default()
        })
    }

    fn add_bytes(&mut self, additional: usize) -> Result<(), ResidualAffineBranchSystemError> {
        self.charge(BranchPayloadComparisonCensus {
            bytes: additional,
            ..BranchPayloadComparisonCensus::default()
        })
    }

    fn add_string(&mut self, value: &str) -> Result<(), ResidualAffineBranchSystemError> {
        self.charge(BranchPayloadComparisonCensus {
            units: 1,
            bytes: value.len(),
            integer_bits: 0,
        })
    }

    fn add_integer(&mut self, value: &Integer) -> Result<(), ResidualAffineBranchSystemError> {
        self.charge(BranchPayloadComparisonCensus {
            units: 1,
            bytes: size_of::<Integer>(),
            integer_bits: integer_magnitude_bits(value)?,
        })
    }

    /// Reject a nested sequence from its length metadata before entering its
    /// element loop. This is a lower-bound check only; exact per-element
    /// charges below still produce the authenticated complete census.
    fn preflight_repeated(
        &self,
        entries: usize,
        minimum_units_per_entry: usize,
        representation_bytes: usize,
    ) -> Result<(), ResidualAffineBranchSystemError> {
        self.preflight(BranchPayloadComparisonCensus {
            units: checked_mul("payload comparison units", entries, minimum_units_per_entry)?,
            bytes: representation_bytes,
            integer_bits: 0,
        })?;
        Ok(())
    }
}

/// Count representation words rather than relying on a fragile hand-written
/// constant for nested limit/stat structures. The structures censused through
/// this helper contain only scalar numeric leaves (with possible padding), so
/// representation words are a conservative upper bound on scalar equality
/// work and automatically include newly added leaves.
fn scalar_representation_units<T>() -> usize {
    let bytes = size_of::<T>();
    let word = size_of::<usize>();
    bytes / word + usize::from(bytes % word != 0)
}

/// Recompute the complete branch-owned operand shape without allocating.
///
/// `source_cover` and `integer_system` are deliberately stopped at their
/// pointer/option plus checked-comparator seams: their deep payloads are
/// governed by fresh resource phases in those certificates. Everything that
/// branch equality itself traverses is counted here, including both string
/// payloads and arbitrary-precision integer magnitudes.
fn branch_payload_operand_census(
    certificate: &ResidualAffineBranchSystemCertificate,
    budget: &mut BranchPayloadComparisonBudget,
) -> Result<(), ResidualAffineBranchSystemError> {
    // The fixed representation owns all scalar fields and all container
    // handles. Heap-backed bytes traversed by equality are added below.
    budget.add_bytes(size_of::<ResidualAffineBranchSystemCertificate>())?;
    budget.add_units(1)?; // top-level allocation pointer fast path
    budget.add_string(certificate.schema)?;
    budget.add_string(&certificate.family_fingerprint)?;
    budget.add_string(&certificate.context_fingerprint)?;
    // Arc identity/payload comparison plus the referenced string bytes.
    budget.add_units(1)?;
    budget.add_string(&certificate.source_partition_identity)?;
    // Arc::ptr_eq followed, when necessary, by the cover's checked equality.
    budget.add_units(2)?;
    budget.add_units(1)?; // ready_terminal_ordinal

    // Both manifest container-length comparisons are explicit branch work.
    budget.add_units(1)?;
    budget.preflight_repeated(
        certificate.zero_atom_recognitions.len(),
        3,
        size_of_val(certificate.zero_atom_recognitions.as_slice()),
    )?;
    for recognition in &certificate.zero_atom_recognitions {
        budget.add_bytes(size_of::<ResidualAffineBranchZeroAtomRecognition>())?;
        census_zero_atom_recognition(budget, recognition)?;
    }
    budget.add_units(1)?;
    budget.preflight_repeated(
        certificate.nonzero_guard_locus_ordinals.len(),
        1,
        size_of_val(certificate.nonzero_guard_locus_ordinals.as_slice()),
    )?;
    for ordinal in &certificate.nonzero_guard_locus_ordinals {
        budget.add_units(1)?;
        budget.add_bytes(size_of_val(ordinal))?;
    }

    // Option discriminant plus the integer-system checked-comparator seam.
    // Its independently bounded deep payload is intentionally not duplicated.
    budget.add_units(2)?;
    census_branch_outcome(budget, &certificate.outcome)?;

    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchSystemLimits,
    >())?;
    budget.add_units(scalar_representation_units::<ResidualAffineBranchSystemStats>())?;
    Ok(())
}

fn census_zero_atom_recognition(
    budget: &mut BranchPayloadComparisonBudget,
    recognition: &ResidualAffineBranchZeroAtomRecognition,
) -> Result<(), ResidualAffineBranchSystemError> {
    budget.add_units(3)?; // structural ordinal, source terms, outcome discriminant
    match &recognition.outcome {
        ResidualAffineBranchZeroAtomOutcome::Row(certificate)
        | ResidualAffineBranchZeroAtomOutcome::RedundantZeroPolynomial(certificate)
        | ResidualAffineBranchZeroAtomOutcome::InconsistentNonzeroConstant(certificate) => {
            census_atom_row_certificate(budget, certificate)
        }
        ResidualAffineBranchZeroAtomOutcome::Unsupported(reason) => {
            census_atom_unsupported(budget, reason)
        }
    }
}

fn census_atom_row_certificate(
    budget: &mut BranchPayloadComparisonBudget,
    certificate: &ResidualAffineAtomRowCertificate,
) -> Result<(), ResidualAffineBranchSystemError> {
    budget.add_string(certificate.schema())?;
    budget.add_string(certificate.context_fingerprint())?;
    census_parametric_polynomial(
        budget,
        certificate.source(),
        certificate.context_fingerprint(),
    )?;
    budget.add_units(1)?; // atom-row outcome discriminant

    budget.add_units(1)?; // primitive-row Option discriminant
    if let Some(primitive) = certificate.primitive_row() {
        budget.add_units(1)?; // component Vec length
        budget.preflight_repeated(
            primitive.components().len(),
            1,
            size_of_val(primitive.components()),
        )?;
        for component in primitive.components() {
            budget.add_integer(component)?;
        }
    }

    budget.add_units(1)?; // block-witness Vec length
    budget.preflight_repeated(
        certificate.block_witnesses().len(),
        2,
        checked_add(
            "payload comparison bytes",
            size_of_val(certificate.block_witnesses()),
            checked_mul(
                "payload comparison bytes",
                certificate.block_witnesses().len(),
                size_of::<Integer>(),
            )?,
        )?,
    )?;
    for witness in certificate.block_witnesses() {
        budget.add_bytes(size_of_val(witness))?;
        budget.add_units(1)?; // base-exponent Vec length
        budget.preflight_repeated(
            witness.base_exponents().len(),
            1,
            size_of_val(witness.base_exponents()),
        )?;
        for exponent in witness.base_exponents() {
            budget.add_units(1)?;
            budget.add_bytes(size_of_val(exponent))?;
        }
        budget.add_integer(witness.signed_scalar())?;
    }

    budget.add_units(scalar_representation_units::<ResidualAffineAtomRowLimits>())?;
    budget.add_units(scalar_representation_units::<
        crate::ResidualAffineAtomRowStats,
    >())?;
    Ok(())
}

fn census_parametric_polynomial(
    budget: &mut BranchPayloadComparisonBudget,
    source: &crate::ParametricPolynomial,
    authenticated_context_fingerprint: &str,
) -> Result<(), ResidualAffineBranchSystemError> {
    let raw = source.raw();
    // Ring identity; variable Arc/length; coefficient and exponent lengths;
    // and the conservative constant/zero shape probes used when maps differ.
    budget.add_units(7)?;
    budget.add_bytes(size_of::<Vec<PolyVariable>>())?;

    budget.preflight_repeated(
        raw.coefficients.len(),
        1,
        size_of_val(raw.coefficients.as_slice()),
    )?;
    for coefficient in &raw.coefficients {
        budget.add_integer(coefficient)?;
    }
    budget.preflight_repeated(
        raw.exponents.len(),
        1,
        size_of_val(raw.exponents.as_slice()),
    )?;
    for exponent in &raw.exponents {
        budget.add_units(1)?;
        budget.add_bytes(size_of_val(exponent))?;
    }
    budget.preflight_repeated(
        raw.variables.len(),
        1,
        checked_mul(
            "payload comparison bytes",
            raw.variables.len(),
            size_of::<PolyVariable>(),
        )?,
    )?;
    for _variable in raw.variables.iter() {
        budget.add_units(1)?;
        budget.add_bytes(size_of::<PolyVariable>())?;
    }

    // ParametricPolynomial's private Arc<str> is authenticated to this exact
    // context at atom construction. Count both the Arc seam and its payload;
    // this is separate from the atom certificate's owned context String.
    budget.add_units(1)?;
    budget.add_string(authenticated_context_fingerprint)
}

fn census_atom_unsupported(
    budget: &mut BranchPayloadComparisonBudget,
    reason: &ResidualAffineAtomRowUnsupported,
) -> Result<(), ResidualAffineBranchSystemError> {
    budget.add_units(1)?; // unsupported-reason discriminant
    match reason {
        ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial { term_ordinal }
        | ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock {
            block_ordinal: term_ordinal,
        } => {
            budget.add_units(1)?;
            budget.add_bytes(size_of_val(term_ordinal))
        }
    }
}

fn census_branch_outcome(
    budget: &mut BranchPayloadComparisonBudget,
    outcome: &ResidualAffineBranchSystemOutcome,
) -> Result<(), ResidualAffineBranchSystemError> {
    budget.add_units(1)?; // branch-outcome discriminant
    match outcome {
        ResidualAffineBranchSystemOutcome::GuardedAffineMap => Ok(()),
        ResidualAffineBranchSystemOutcome::ProvedEmpty(reason) => {
            budget.add_units(1)?; // empty-reason discriminant
            if let ResidualAffineBranchEmptyReason::InconsistentNonzeroConstant {
                structural_locus_ordinal,
            } = reason
            {
                budget.add_units(1)?;
                budget.add_bytes(size_of_val(structural_locus_ordinal))?;
            }
            Ok(())
        }
        ResidualAffineBranchSystemOutcome::Unsupported { reasons } => {
            budget.add_units(1)?; // unsupported-reason Vec length
            budget.preflight_repeated(
                reasons.len(),
                3,
                checked_add(
                    "payload comparison bytes",
                    size_of_val(reasons.as_slice()),
                    checked_mul(
                        "payload comparison bytes",
                        reasons.len(),
                        size_of::<usize>(),
                    )?,
                )?,
            )?;
            for reason in reasons {
                budget.add_bytes(size_of::<ResidualAffineBranchUnsupportedReason>())?;
                budget.add_units(1)?; // branch unsupported-reason discriminant
                match reason {
                    ResidualAffineBranchUnsupportedReason::Atom {
                        structural_locus_ordinal,
                        reason,
                    } => {
                        budget.add_units(1)?;
                        budget.add_bytes(size_of_val(structural_locus_ordinal))?;
                        census_atom_unsupported(budget, reason)?;
                    }
                    ResidualAffineBranchUnsupportedReason::GeneralCongruence(reason) => {
                        budget.add_units(1)?; // integer-system unsupported discriminant
                        match reason {
                            ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
                                remaining_equations,
                            } => {
                                budget.add_units(1)?;
                                budget.add_bytes(size_of_val(remaining_equations))?;
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

fn authenticate_branch_payload_comparison_stats(
    certificate: &mut ResidualAffineBranchSystemCertificate,
) -> Result<BranchPayloadComparisonCensus, ResidualAffineBranchSystemError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_BRANCH_LOCAL_COMPARISON_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let mut comparison_budget = BranchPayloadComparisonBudget::new(certificate.limits);
    branch_payload_operand_census(certificate, &mut comparison_budget)?;
    branch_payload_operand_census(certificate, &mut comparison_budget)?;
    let comparison = comparison_budget.census;
    certificate.stats.payload_comparison_units = comparison.units;
    certificate.stats.payload_comparison_bytes = comparison.bytes;
    certificate.stats.payload_comparison_integer_bits = comparison.integer_bits;
    Ok(comparison)
}

fn validate_sorted_disjoint_manifest(left: &[usize], right: &[usize]) -> (bool, usize) {
    let mut comparisons = 0usize;
    for pair in left.windows(2).chain(right.windows(2)) {
        comparisons += 1;
        if pair[0] >= pair[1] {
            return (false, comparisons);
        }
    }
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        comparisons += 1;
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => return (false, comparisons),
        }
    }
    (true, comparisons)
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, ResidualAffineBranchSystemError> {
    let mut target = String::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| ResidualAffineBranchSystemError::AllocationFailure { resource })?;
    target.push_str(source);
    Ok(target)
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ResidualAffineBranchSystemError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ResidualAffineBranchSystemError::ResourceCountOverflow {
        resource: "integer coefficient bits",
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineBranchSystemError> {
    left.checked_add(right)
        .ok_or(ResidualAffineBranchSystemError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineBranchSystemError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineBranchSystemError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ResidualAffineBranchSystemError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineBranchSystemError> {
    if requested > limit {
        Err(ResidualAffineBranchSystemError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_affine_initial_global_affine_terminal::{
        GeneratedAffineInitialGlobalAffineTerminal,
        GeneratedAffineInitialGlobalAffineTerminalError,
        GeneratedAffineInitialGlobalAffineTerminalOutcome,
    };
    use crate::residual_affine_branch_guard_composition::{
        ResidualAffineBranchGuardCompositionError, ResidualAffineBranchGuardCompositionLimits,
        ResidualAffineBranchSealedGuardBundle, guard_gmp_logical_bytes_upper_bound,
        sealed_guard_memory_envelope_parts_from_limits,
    };
    use crate::{
        AffineDenominator, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        IntegralOrderingPolicy, ParametricCoefficient, ParametricIbpGenerator,
        ParametricPolynomial, ResidualProductLocusBooleanCoverCompiler,
        ResidualProductLocusBooleanCoverLimits, SectorMask, algebra::CoefficientContext,
    };

    fn context(scope: &str, arity: usize) -> ParametricCoefficientContext {
        let base = CoefficientContext::new(std::iter::empty::<&str>());
        ParametricCoefficientContext::try_new(&base, scope, arity).unwrap()
    }

    fn polynomial(
        context: &ParametricCoefficientContext,
        value: &ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    fn sunset_family(name: &str) -> IntegralFamily {
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

    fn generated_sunset_cover_for_sector(
        sector: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) {
        let family = sunset_family(&format!("residual-affine-branch-unit-sunset-{sector}"));
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string(sector).unwrap(),
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
        let cover = Arc::new(
            ResidualProductLocusBooleanCoverCompiler::compile(
                &family,
                &context,
                queue,
                0,
                ResidualProductLocusBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        (family, context, cover)
    }

    fn generated_sunset_cover() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) {
        generated_sunset_cover_for_sector("111")
    }

    fn find_fresh_guarded(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        require_zero_guards: bool,
    ) -> Option<(usize, ResidualAffineBranchSystemFreshGuarded)> {
        for node in cover.nodes() {
            if !matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            ) || require_zero_guards && !node.nonzero_atoms().is_empty()
            {
                continue;
            }
            let fresh = ResidualAffineBranchSystemCertificate::compile_fresh_replayed(
                family,
                context,
                Arc::clone(cover),
                node.ordinal(),
                ResidualAffineBranchSystemLimits::default(),
            )
            .unwrap();
            if let ResidualAffineBranchSystemFreshCompilation::Guarded(guarded) = fresh {
                return Some((node.ordinal(), guarded));
            }
        }
        None
    }

    fn first_fresh_guarded(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        require_zero_guards: bool,
    ) -> (usize, ResidualAffineBranchSystemFreshGuarded) {
        find_fresh_guarded(family, context, cover, require_zero_guards)
            .expect("validation cover has no requested fresh guarded branch")
    }

    fn first_fresh_guarded_with_nonzero_guards(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) -> (usize, ResidualAffineBranchSystemFreshGuarded) {
        for node in cover.nodes() {
            if !matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            ) || node.nonzero_atoms().is_empty()
            {
                continue;
            }
            if let ResidualAffineBranchSystemFreshCompilation::Guarded(guarded) =
                ResidualAffineBranchSystemCertificate::compile_fresh_replayed(
                    family,
                    context,
                    Arc::clone(cover),
                    node.ordinal(),
                    ResidualAffineBranchSystemLimits::default(),
                )
                .unwrap()
            {
                return (node.ordinal(), guarded);
            }
        }
        panic!("validation cover has no guarded branch with nonzero guards")
    }

    fn synthetic_payload(
        context: &ParametricCoefficientContext,
        sources: &[(usize, ParametricPolynomial)],
    ) -> (CompiledZeroAtomPayload, ResidualAffineBranchSystemStats) {
        synthetic_payload_with_limits(
            context,
            sources,
            ResidualAffineBranchSystemLimits::default(),
        )
    }

    fn synthetic_payload_with_limits(
        context: &ParametricCoefficientContext,
        sources: &[(usize, ParametricPolynomial)],
        limits: ResidualAffineBranchSystemLimits,
    ) -> (CompiledZeroAtomPayload, ResidualAffineBranchSystemStats) {
        let mut stats = ResidualAffineBranchSystemStats::default();
        let payload = compile_zero_atom_payload(
            context,
            sources.len(),
            sources
                .iter()
                .map(|(ordinal, polynomial)| Ok((*ordinal, polynomial))),
            limits,
            &mut stats,
        )
        .unwrap();
        (payload, stats)
    }

    #[test]
    fn system_inconsistency_wins_after_every_atom_including_unsupported_was_compiled() {
        let context = context("affine-branch-synthetic-empty", 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let nonlinear = polynomial(&context, &context.mul(&n0, &n1).unwrap());
        let n0_zero = polynomial(&context, &n0);
        let n0_one = polynomial(&context, &context.sub(&n0, &context.one()).unwrap());
        let (payload, stats) =
            synthetic_payload(&context, &[(2, nonlinear), (3, n0_zero), (5, n0_one)]);

        assert_eq!(stats.atom_recognitions_attempted(), 3);
        assert_eq!(stats.unsupported_atoms(), 1);
        assert_eq!(stats.recognized_rows(), 2);
        assert!(matches!(
            payload.zero_atom_recognitions[0].outcome(),
            ResidualAffineBranchZeroAtomOutcome::Unsupported(
                ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial { .. }
            )
        ));
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::ProvedEmpty(
                ResidualAffineBranchEmptyReason::IntegerSystem
            )
        ));
        let system = payload.integer_system.unwrap();
        assert_eq!(
            system.outcome(),
            ResidualAffineIntegerSystemOutcome::ProvedEmpty
        );
        assert!(system.empty_witness().is_some());
    }

    #[test]
    fn a_nonzero_constant_wins_but_does_not_short_circuit_other_atom_recognition() {
        let context = context("affine-branch-synthetic-constant-empty", 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let nonlinear = polynomial(&context, &context.mul(&n0, &n1).unwrap());
        let constant = polynomial(&context, &context.integer(7));
        let (payload, stats) = synthetic_payload(&context, &[(11, constant), (13, nonlinear)]);

        assert_eq!(stats.atom_recognitions_attempted(), 2);
        assert_eq!(stats.inconsistent_nonzero_constants(), 1);
        assert_eq!(stats.unsupported_atoms(), 1);
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::ProvedEmpty(
                ResidualAffineBranchEmptyReason::InconsistentNonzeroConstant {
                    structural_locus_ordinal: 11
                }
            )
        ));
        assert!(payload.integer_system.is_none());
    }

    #[test]
    fn genuine_congruence_parameterization_is_a_typed_unsupported_boundary() {
        let context = context("affine-branch-synthetic-congruence", 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let two_n0 = context.mul(&context.integer(2), &n0).unwrap();
        let three_n1 = context.mul(&context.integer(3), &n1).unwrap();
        let row = polynomial(&context, &context.add(&two_n0, &three_n1).unwrap());
        let (payload, stats) = synthetic_payload(&context, &[(17, row)]);

        assert_eq!(stats.atom_recognitions_attempted(), 1);
        assert_eq!(stats.recognized_rows(), 1);
        assert_eq!(stats.general_congruence_boundaries(), 1);
        assert!(payload.integer_system.is_none());
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::Unsupported { ref reasons }
                if matches!(
                    reasons.as_slice(),
                    [ResidualAffineBranchUnsupportedReason::GeneralCongruence(
                        ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported { .. }
                    )]
                )
        ));
    }

    #[test]
    fn unsupported_branch_cannot_expose_the_consistent_recognized_subset_map() {
        let context = context("affine-branch-synthetic-partial-map", 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let nonlinear = polynomial(&context, &context.mul(&n0, &n1).unwrap());
        let row = polynomial(
            &context,
            &context
                .sub(&context.add(&n0, &n1).unwrap(), &context.integer(3))
                .unwrap(),
        );
        let (payload, stats) = synthetic_payload(&context, &[(23, nonlinear), (29, row)]);

        assert_eq!(stats.unsupported_atoms(), 1);
        assert_eq!(stats.recognized_rows(), 1);
        assert!(matches!(
            &payload.outcome,
            ResidualAffineBranchSystemOutcome::Unsupported { .. }
        ));
        assert!(
            payload
                .integer_system
                .as_deref()
                .and_then(ResidualAffineIntegerSystemCertificate::affine_map)
                .is_some(),
            "the recognized subset has a diagnostic map"
        );
        assert!(
            certified_integer_system_for_outcome(&payload.outcome, &payload.integer_system)
                .is_none(),
            "an unsupported branch must not expose that partial map as certified"
        );
    }

    #[test]
    fn fresh_late_atom_unsupported_destroys_diagnostic_map_authorization() {
        let base = CoefficientContext::new(["theta"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "fresh-late-atom-unsupported", 2).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let first = polynomial(&context, &context.sub(&n0, &context.one()).unwrap());
        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let theta_row = context
            .mul(&theta, &context.add(&n0, &context.one()).unwrap())
            .unwrap();
        let late_nonassociate = polynomial(
            &context,
            &context
                .add(&theta_row, &context.add(&n1, &context.one()).unwrap())
                .unwrap(),
        );
        let sources = [(3usize, first), (11usize, late_nonassociate)];
        let mut stats = ResidualAffineBranchSystemStats::default();
        let payload = compile_zero_atom_payload_fresh(
            &context,
            sources.len(),
            sources
                .iter()
                .map(|(ordinal, source)| Ok((*ordinal, source))),
            ResidualAffineBranchSystemLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert_eq!(stats.atom_recognitions_attempted(), 2);
        assert!(matches!(
            payload.zero_atom_recognitions[1].outcome(),
            ResidualAffineBranchZeroAtomOutcome::Unsupported(
                ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock { .. }
            )
        ));
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::Unsupported { .. }
        ));
        assert!(
            payload.integer_system.is_some(),
            "diagnostic subset map retained"
        );
        assert!(
            payload.integer_plan_authorization.is_none(),
            "unsupported branch must physically lose plan authorization"
        );
        assert!(payload.standalone_atom_attempt_owned_logical_peak_upper_bound > 0);
        assert!(payload.integer_system_raw_transient_census.is_some());
    }

    #[test]
    fn fresh_empty_and_general_congruence_outcomes_physically_lack_authorization() {
        let context = context("fresh-terminal-auth-absence", 2);
        let constant = polynomial(&context, &context.integer(9));
        let mut stats = ResidualAffineBranchSystemStats::default();
        let empty_sources = [(5usize, constant)];
        let empty = compile_zero_atom_payload_fresh(
            &context,
            empty_sources.len(),
            empty_sources
                .iter()
                .map(|(ordinal, source)| Ok((*ordinal, source))),
            ResidualAffineBranchSystemLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert!(matches!(
            empty.outcome,
            ResidualAffineBranchSystemOutcome::ProvedEmpty(
                ResidualAffineBranchEmptyReason::InconsistentNonzeroConstant { .. }
            )
        ));
        assert!(empty.integer_plan_authorization.is_none());
        assert!(empty.integer_system_raw_transient_census.is_none());

        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let congruence = polynomial(
            &context,
            &context
                .add(
                    &context.mul(&context.integer(2), &n0).unwrap(),
                    &context.mul(&context.integer(3), &n1).unwrap(),
                )
                .unwrap(),
        );
        let mut stats = ResidualAffineBranchSystemStats::default();
        let congruence_sources = [(7usize, congruence)];
        let unsupported = compile_zero_atom_payload_fresh(
            &context,
            congruence_sources.len(),
            congruence_sources
                .iter()
                .map(|(ordinal, source)| Ok((*ordinal, source))),
            ResidualAffineBranchSystemLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert!(matches!(
            unsupported.outcome,
            ResidualAffineBranchSystemOutcome::Unsupported { .. }
        ));
        assert!(unsupported.integer_plan_authorization.is_none());
        assert!(unsupported.integer_system.is_none());
        assert!(unsupported.integer_system_raw_transient_census.is_some());
        assert!(unsupported.integer_system_compilation_owned_logical_peak_upper_bound > 0);
    }

    #[test]
    fn affine_map_is_exposed_only_for_the_guarded_success_outcome() {
        let context = context("affine-branch-synthetic-map", 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let row = polynomial(
            &context,
            &context
                .sub(&context.add(&n0, &n1).unwrap(), &context.integer(3))
                .unwrap(),
        );
        let (payload, _) = synthetic_payload(&context, &[(19, row)]);
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ));
        assert!(
            payload
                .integer_system
                .as_deref()
                .and_then(ResidualAffineIntegerSystemCertificate::affine_map)
                .is_some()
        );
    }

    #[test]
    fn empty_zero_atom_manifest_is_the_guarded_identity_map() {
        let context = context("affine-branch-synthetic-identity", 3);
        let mut limits = ResidualAffineBranchSystemLimits::default();
        limits.max_unsupported_reasons = 0;
        let (payload, stats) = synthetic_payload_with_limits(&context, &[], limits);
        assert_eq!(stats.atom_recognitions_attempted(), 0);
        assert!(matches!(
            payload.outcome,
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ));
        let map = payload
            .integer_system
            .as_deref()
            .and_then(ResidualAffineIntegerSystemCertificate::affine_map)
            .unwrap();
        assert!(map.pivot_positions().is_empty());
        assert_eq!(map.free_positions(), &[0, 1, 2]);
        for row in 0..3 {
            assert_eq!(map.constant(row), Some(&Integer::from(0)));
            for column in 0..3 {
                assert_eq!(
                    map.linear_coefficient(row, column),
                    Some(&Integer::from(usize::from(row == column)))
                );
            }
        }
    }

    #[test]
    fn asymmetric_payload_comparison_preflights_large_supplied_guard_manifest() {
        let (family, context, cover) = generated_sunset_cover();
        let terminal_ordinal = cover
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
            })
            .max_by_key(|node| node.equal_zero_atoms().len() + node.nonzero_atoms().len())
            .expect("sunset cover has a ready affine terminal")
            .ordinal();
        let certificate = ResidualAffineBranchSystemCertificate::compile(
            &family,
            &context,
            cover,
            terminal_ordinal,
            ResidualAffineBranchSystemLimits::default(),
        )
        .unwrap();
        let exact_equal_pair_units = certificate.stats().payload_comparison_units();
        assert!(exact_equal_pair_units > 0);

        // The small retained receiver authorizes exactly its authenticated
        // equal-pair census. Grow only the independently supplied operand. The
        // extra count itself is larger than the complete baseline allowance,
        // so a bulk length preflight jumps past limit+1. A per-element walk
        // would instead stop only after incrementing to exactly limit+1.
        let mut retained = certificate;
        retained.limits.max_payload_comparison_units = exact_equal_pair_units;
        let mut supplied = retained.clone();
        let additional_guards = exact_equal_pair_units.checked_add(2).unwrap();
        let enlarged_guard_count = supplied
            .nonzero_guard_locus_ordinals
            .len()
            .checked_add(additional_guards)
            .unwrap();
        assert!(enlarged_guard_count <= retained.limits.max_nonzero_guards);
        supplied
            .nonzero_guard_locus_ordinals
            .resize(enlarged_guard_count, usize::MAX);

        assert!(matches!(
            retained.payload_eq_checked(&supplied),
            Err(ResidualAffineBranchSystemError::ResourceLimit {
                resource: "payload comparison units",
                requested,
                limit,
            }) if limit == exact_equal_pair_units
                && requested > exact_equal_pair_units.checked_add(1).unwrap()
        ));
    }

    #[test]
    fn family_cover_source_replay_tamper_and_every_outer_preflight_budget_are_exact() {
        let (family, context, cover) = generated_sunset_cover();
        let terminal = cover
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                ) && !node.equal_zero_atoms().is_empty()
                    && !node.nonzero_atoms().is_empty()
            })
            .max_by_key(|node| node.equal_zero_atoms().len() + node.nonzero_atoms().len())
            .expect("sunset cover has a guarded affine terminal");
        let certificate = ResidualAffineBranchSystemCertificate::compile(
            &family,
            &context,
            cover.clone(),
            terminal.ordinal(),
            ResidualAffineBranchSystemLimits::default(),
        )
        .unwrap();
        certificate.replay(&family, &context).unwrap();
        assert!(Arc::ptr_eq(certificate.source_cover(), &cover));
        assert_eq!(family.fingerprint_ref(), family.fingerprint().as_str());
        assert_eq!(certificate.family_fingerprint(), family.fingerprint_ref());

        let structurally_equal_cover = Arc::new((*cover).clone());
        assert!(!Arc::ptr_eq(&structurally_equal_cover, &cover));
        certificate
            .replay_with_cover(&family, &context, structurally_equal_cover)
            .unwrap();
        let freshly_compiled_equal_cover = Arc::new(
            ResidualProductLocusBooleanCoverCompiler::compile(
                &family,
                &context,
                cover.source_queue().clone(),
                cover.source_work_item_ordinal(),
                cover.limits(),
            )
            .unwrap(),
        );
        assert!(!Arc::ptr_eq(&freshly_compiled_equal_cover, &cover));
        certificate
            .replay_with_cover(&family, &context, freshly_compiled_equal_cover)
            .unwrap();

        let wrong_family = sunset_family("residual-affine-branch-unit-wrong-family");
        assert!(matches!(
            certificate.replay(&wrong_family, &context),
            Err(ResidualAffineBranchSystemError::WrongFamily)
        ));
        let wrong_context = ParametricCoefficientContext::try_new(
            context.base(),
            "residual-affine-branch-unit-wrong-context",
            context.index_count(),
        )
        .unwrap();
        assert!(matches!(
            certificate.replay(&family, &wrong_context),
            Err(ResidualAffineBranchSystemError::WrongContext)
        ));
        let mut bounded_scope = certificate.clone();
        bounded_scope.limits.max_context_fingerprint_bytes = context.fingerprint().len();
        let oversized_scope = "x".repeat(context.fingerprint().len() + 1);
        let oversized_context = ParametricCoefficientContext::try_new(
            context.base(),
            &oversized_scope,
            context.index_count(),
        )
        .unwrap();
        assert!(oversized_context.fingerprint().len() > context.fingerprint().len());
        assert!(matches!(
            bounded_scope.replay(&family, &oversized_context),
            Err(ResidualAffineBranchSystemError::ResourceLimit {
                resource: "context fingerprint bytes",
                requested,
                limit,
            }) if requested == oversized_context.fingerprint().len()
                && limit == context.fingerprint().len()
        ));

        let mut oversized_retained = certificate.clone();
        oversized_retained.limits.max_family_fingerprint_bytes = family.fingerprint_ref().len();
        oversized_retained.family_fingerprint.push('x');
        assert!(matches!(
            oversized_retained.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ResourceLimit {
                resource: "family fingerprint bytes",
                requested,
                limit,
            }) if requested == family.fingerprint_ref().len() + 1
                && limit == family.fingerprint_ref().len()
        ));

        assert!(certificate.integer_system().is_some());
        assert!(certificate.affine_map().is_some());
        let mut accessor_gated = certificate.clone();
        accessor_gated.tamper_outcome_for_test();
        assert!(
            accessor_gated
                .integer_system
                .as_deref()
                .and_then(ResidualAffineIntegerSystemCertificate::affine_map)
                .is_some(),
            "the compiled certificate still retains its diagnostic child map"
        );
        assert!(accessor_gated.integer_system().is_none());
        assert!(accessor_gated.affine_map().is_none());
        assert!(
            !accessor_gated
                .guarded_affine_map_applies_at_original_indices(
                    &context,
                    &vec![0; context.index_count()],
                )
                .unwrap()
        );

        let mut other_cover_limits = cover.limits();
        other_cover_limits.max_atoms += 1;
        let other_cover = Arc::new(
            ResidualProductLocusBooleanCoverCompiler::compile(
                &family,
                &context,
                cover.source_queue().clone(),
                cover.source_work_item_ordinal(),
                other_cover_limits,
            )
            .unwrap(),
        );
        assert!(matches!(
            certificate.replay_with_cover(&family, &context, other_cover),
            Err(ResidualAffineBranchSystemError::SourceCoverMismatch)
        ));

        let mut tampered = certificate.clone();
        tampered.tamper_schema_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::SchemaMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_family_fingerprint_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::WrongFamily)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_context_fingerprint_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::WrongContext)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_source_identity_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_stats_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_first_zero_atom_ordinal_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_first_zero_atom_source_terms_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_first_zero_atom_outcome_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_first_guard_ordinal_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_ready_terminal_ordinal_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReadyTerminalNotFound { .. })
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_outcome_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_integer_system_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_limits_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchSystemError::ResourceLimit {
                resource: "zero atoms",
                ..
            })
        ));

        let stats = certificate.stats();
        macro_rules! checked_late_difference_budget {
            ($field:ident, $getter:ident, $resource:literal) => {{
                let requested = stats.$getter();
                assert!(requested > 0, $resource);
                let mut exact = certificate.clone();
                exact.limits.$field = requested;
                let mut late_difference = exact.clone();
                late_difference.tamper_stats_for_test();
                assert_eq!(
                    exact.payload_eq_checked(&late_difference),
                    Ok(false),
                    "exact comparison budget {}",
                    $resource
                );

                let mut one_below = exact;
                one_below.limits.$field = requested - 1;
                let mut late_difference = one_below.clone();
                late_difference.tamper_stats_for_test();
                assert!(matches!(
                    one_below.payload_eq_checked(&late_difference),
                    Err(ResidualAffineBranchSystemError::ResourceLimit {
                        resource: actual,
                        requested: actual_requested,
                        limit,
                    }) if actual == $resource
                        && actual_requested == requested
                        && limit == requested - 1
                ));
            }};
        }
        checked_late_difference_budget!(
            max_payload_comparison_units,
            payload_comparison_units,
            "payload comparison units"
        );
        checked_late_difference_budget!(
            max_payload_comparison_bytes,
            payload_comparison_bytes,
            "payload comparison bytes"
        );
        checked_late_difference_budget!(
            max_payload_comparison_integer_bits,
            payload_comparison_integer_bits,
            "payload comparison integer bits"
        );

        let mut checks: Vec<(&str, ResidualAffineBranchSystemLimits)> = Vec::new();
        let base = certificate.limits();
        let mut exact_manifest_limits = base;
        exact_manifest_limits.max_manifest_validation_comparisons =
            stats.manifest_validation_comparisons();
        assert!(
            preflight_source(
                &family,
                &context,
                &cover,
                terminal.ordinal(),
                exact_manifest_limits,
            )
            .is_ok(),
            "exact manifest validation comparison budget"
        );
        macro_rules! below {
            ($field:ident, $getter:ident, $resource:literal) => {{
                let requested = stats.$getter();
                assert!(requested > 0, $resource);
                let mut limits = base;
                limits.$field = requested - 1;
                checks.push(($resource, limits));
            }};
        }
        below!(
            max_family_fingerprint_bytes,
            family_fingerprint_bytes,
            "family fingerprint bytes"
        );
        below!(
            max_context_fingerprint_bytes,
            context_fingerprint_bytes,
            "context fingerprint bytes"
        );
        below!(
            max_scope_fingerprint_comparison_bytes,
            scope_fingerprint_comparison_bytes,
            "scope fingerprint comparison bytes"
        );
        below!(
            max_source_identity_bytes_referenced,
            source_identity_bytes_referenced,
            "source identity bytes referenced"
        );
        below!(max_zero_atoms, zero_atoms, "zero atoms");
        below!(max_nonzero_guards, nonzero_guards, "nonzero guards");
        below!(
            max_structural_atom_lookups,
            structural_atom_lookups,
            "structural atom lookups"
        );
        below!(
            max_referenced_atom_terms,
            referenced_atom_terms,
            "referenced atom terms"
        );
        below!(
            max_zero_atom_source_terms,
            zero_atom_source_terms,
            "zero-atom source terms"
        );
        below!(
            max_zero_atom_exponent_entries,
            zero_atom_exponent_entries,
            "zero-atom exponent entries"
        );
        below!(
            max_zero_atom_coefficient_bits,
            zero_atom_coefficient_bits,
            "zero-atom coefficient bits"
        );
        below!(
            max_potential_block_witnesses,
            potential_block_witnesses,
            "potential block witnesses"
        );
        below!(
            max_potential_block_exponent_entries,
            potential_block_exponent_entries,
            "potential block exponent entries"
        );
        below!(
            max_potential_retained_integer_bits,
            potential_retained_integer_bits,
            "potential retained integer bits"
        );
        below!(
            max_retained_atom_context_fingerprint_bytes,
            retained_atom_context_fingerprint_bytes,
            "retained atom context fingerprint bytes"
        );
        below!(
            max_potential_row_components,
            potential_row_components,
            "potential row components"
        );
        below!(max_system_input_rows, zero_atoms, "system input rows");
        below!(
            max_system_lineage_ordinals,
            zero_atoms,
            "system lineage ordinals"
        );
        below!(max_manifest_entries, manifest_entries, "manifest entries");
        below!(max_manifest_bytes, manifest_bytes, "manifest bytes");
        below!(
            max_manifest_validation_comparisons,
            manifest_validation_comparisons,
            "manifest validation comparisons"
        );
        below!(max_unsupported_reasons, zero_atoms, "unsupported reasons");

        for (resource, limits) in checks {
            assert!(
                matches!(
                    preflight_source(&family, &context, &cover, terminal.ordinal(), limits),
                    Err(ResidualAffineBranchSystemError::ResourceLimit {
                        resource: actual,
                        ..
                    }) if actual == resource
                ),
                "one-below budget {resource}"
            );
        }
    }

    #[test]
    fn fresh_guarded_branch_consumes_zero_guard_authorization_and_seals_origins() {
        let mut selected = None;
        for sector in ["011", "101", "110", "111"] {
            let (family, context, cover) = generated_sunset_cover_for_sector(sector);
            cover.replay(&family, &context).unwrap();
            if let Some((terminal_ordinal, guarded)) =
                find_fresh_guarded(&family, &context, &cover, true)
            {
                selected = Some((family, context, cover, terminal_ordinal, guarded));
                break;
            }
        }
        let (family, context, cover, terminal_ordinal, guarded) =
            selected.expect("validation sectors have no fresh guarded branch without guards");
        cover.replay(&family, &context).unwrap();
        let branch = Arc::clone(&guarded.branch);
        let integer_system = branch.integer_system_arc().unwrap().clone();
        let memory = guarded.memory;
        assert!(memory.retained_owned_logical_bytes() > 0);
        assert!(
            memory.compilation_owned_logical_peak_upper_bound()
                >= memory.retained_owned_logical_bytes()
        );
        let bundle = ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
            &context,
            Arc::clone(&branch),
            guarded.authorization,
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
        .unwrap();
        assert_eq!(bundle.guard_count(), 0);
        assert!(!bundle.has_contradiction());
        assert!(bundle.memory().plan_retained_owned_logical_bytes() > 0);
        assert!(
            bundle
                .memory()
                .plan_compilation_owned_logical_peak_upper_bound()
                >= bundle.memory().plan_retained_owned_logical_bytes()
        );
        assert!(bundle.every_origin_is_generated_affine_sealed_for_test());
        assert!(bundle.allocations_match_for_test(&cover, &branch, &integer_system));
        assert_eq!(terminal_ordinal, branch.ready_terminal_ordinal());
        let (dense_ready_position, sparse_ready_ordinal) = cover
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
            })
            .enumerate()
            .find_map(|(dense, node)| (dense != node.ordinal()).then_some((dense, node.ordinal())))
            .expect("validation cover has a sparse ready-node ordinal");
        assert_ne!(dense_ready_position, sparse_ready_ordinal);
        ResidualAffineBranchSystemCertificate::compile_fresh_replayed(
            &family,
            &context,
            Arc::clone(&cover),
            sparse_ready_ordinal,
            ResidualAffineBranchSystemLimits::default(),
        )
        .unwrap();
        let debug = format!("{bundle:?}");
        for forbidden in [
            "source_case",
            "source_work_item_ordinal",
            "ready_terminal_ordinal",
            "ResidualAffineBranchNonzeroGuardSubstitution",
        ] {
            assert!(
                !debug.contains(forbidden),
                "debug leaked {forbidden}: {debug}"
            );
        }
    }

    #[test]
    fn fresh_guard_authorization_rejects_equal_distinct_allocations_and_census_tampering() {
        let (family, context, cover) = generated_sunset_cover();
        cover.replay(&family, &context).unwrap();
        let (terminal_ordinal, guarded) = first_fresh_guarded(&family, &context, &cover, false);
        let independently_equal_branch = Arc::new((*guarded.branch).clone());
        assert!(!Arc::ptr_eq(&independently_equal_branch, &guarded.branch));
        assert!(matches!(
            ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                &context,
                independently_equal_branch,
                guarded.authorization,
                ResidualAffineBranchGuardCompositionLimits::default(),
            ),
            Err(ResidualAffineBranchGuardCompositionError::Branch(
                ResidualAffineBranchSystemError::FreshSourceBranchAllocationMismatch
            ))
        ));

        let (_, mut guarded) = first_fresh_guarded(&family, &context, &cover, false);
        guarded.authorization.source_cover = Arc::new((*cover).clone());
        assert!(matches!(
            ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                &context,
                Arc::clone(&guarded.branch),
                guarded.authorization,
                ResidualAffineBranchGuardCompositionLimits::default(),
            ),
            Err(ResidualAffineBranchGuardCompositionError::Branch(
                ResidualAffineBranchSystemError::FreshSourceCoverAllocationMismatch
            ))
        ));

        let (_, mut guarded) = first_fresh_guarded(&family, &context, &cover, false);
        let old_integer = guarded.branch.integer_system_arc().unwrap().clone();
        let mut independently_equal_payload = (*guarded.branch).clone();
        independently_equal_payload.integer_system = Some(Arc::new((*old_integer).clone()));
        let independently_equal_branch = Arc::new(independently_equal_payload);
        guarded.authorization.branch = Arc::clone(&independently_equal_branch);
        assert!(matches!(
            ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                &context,
                independently_equal_branch,
                guarded.authorization,
                ResidualAffineBranchGuardCompositionLimits::default(),
            ),
            Err(ResidualAffineBranchGuardCompositionError::Branch(
                ResidualAffineBranchSystemError::FreshIntegerSystemAllocationMismatch
            ))
        ));

        for tamper in [
            ResidualAffineBranchSystemFreshGuardAuthorization::tamper_memory_retained_for_test,
            ResidualAffineBranchSystemFreshGuardAuthorization::tamper_memory_peak_for_test,
            ResidualAffineBranchSystemFreshGuardAuthorization::tamper_nested_integer_raw_census_for_test,
            ResidualAffineBranchSystemFreshGuardAuthorization::tamper_payload_units_for_test,
        ] {
            let (_, mut guarded) = first_fresh_guarded(&family, &context, &cover, false);
            tamper(&mut guarded.authorization);
            assert!(matches!(
                ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                    &context,
                    Arc::clone(&guarded.branch),
                    guarded.authorization,
                    ResidualAffineBranchGuardCompositionLimits::default(),
                ),
                Err(ResidualAffineBranchGuardCompositionError::Branch(
                    ResidualAffineBranchSystemError::FreshAdjacentCensusMismatch
                        | ResidualAffineBranchSystemError::IntegerSystem(_)
                ))
            ));
        }

        for tamper in [
            ResidualAffineBranchSealedGuardBundle::tamper_memory_for_test,
            ResidualAffineBranchSealedGuardBundle::tamper_coherent_plan_memory_for_test,
            ResidualAffineBranchSealedGuardBundle::tamper_payload_units_for_test,
        ] {
            let (_, guarded) = first_fresh_guarded(&family, &context, &cover, false);
            let branch = Arc::clone(&guarded.branch);
            let branch_memory = guarded.memory;
            let mut bundle = ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                &context,
                Arc::clone(&branch),
                guarded.authorization,
                ResidualAffineBranchGuardCompositionLimits::default(),
            )
            .unwrap();
            tamper(&mut bundle);
            assert!(matches!(
                bundle.authenticate_with_branch_memory(&context, &branch, branch_memory),
                Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch)
            ));
        }
        assert!(cover.nodes().iter().any(|node| {
            matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            ) && node.ordinal() == terminal_ordinal
        }));
    }

    #[test]
    fn opaque_initial_terminal_authenticates_locator_outcome_memory_and_private_branch_identity() {
        let (family, context, cover) = generated_sunset_cover();
        cover.replay(&family, &context).unwrap();
        let make = || {
            let (terminal_ordinal, guarded) = first_fresh_guarded(&family, &context, &cover, false);
            let independently_equal_branch = Arc::new((*guarded.branch).clone());
            let fresh = ResidualAffineBranchSystemFreshCompilation::Guarded(guarded);
            let terminal = GeneratedAffineInitialGlobalAffineTerminal::compile_from_fresh_branch(
                &context,
                cover.source_work_item_ordinal(),
                terminal_ordinal,
                fresh,
                ResidualAffineBranchGuardCompositionLimits::default(),
            )
            .unwrap();
            (terminal, independently_equal_branch)
        };

        let (terminal, _) = make();
        terminal.reauthenticate_for_test(&context).unwrap();
        assert_eq!(
            terminal.source_work_item_ordinal(),
            cover.source_work_item_ordinal()
        );
        assert!(terminal.memory().retained_owned_logical_bytes() > 0);
        assert!(
            terminal
                .memory()
                .compilation_owned_logical_peak_upper_bound()
                >= terminal.memory().retained_owned_logical_bytes()
        );
        let debug = format!("{terminal:?}");
        for forbidden in [
            "source_case",
            "ResidualAffineBranchSystemCertificate",
            "ResidualAffineBranchGuardCompositionEntry",
            "ResidualAffineBranchNonzeroGuardSubstitution",
        ] {
            assert!(
                !debug.contains(forbidden),
                "debug leaked {forbidden}: {debug}"
            );
        }

        let (mut terminal, _) = make();
        terminal.tamper_source_work_item_ordinal_for_test();
        assert!(matches!(
            terminal.reauthenticate_for_test(&context),
            Err(GeneratedAffineInitialGlobalAffineTerminalError::LocatorMismatch)
        ));
        let (mut terminal, _) = make();
        terminal.tamper_local_terminal_ordinal_for_test();
        assert!(matches!(
            terminal.reauthenticate_for_test(&context),
            Err(GeneratedAffineInitialGlobalAffineTerminalError::LocatorMismatch)
        ));
        let (mut terminal, _) = make();
        terminal.tamper_outcome_for_test();
        assert!(matches!(
            terminal.reauthenticate_for_test(&context),
            Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch)
        ));
        let (mut terminal, _) = make();
        terminal.tamper_guard_memory_for_test();
        assert!(matches!(
            terminal.reauthenticate_for_test(&context),
            Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch)
        ));
        let (mut terminal, _) = make();
        terminal.tamper_branch_memory_for_test();
        assert!(terminal.reauthenticate_for_test(&context).is_err());
        let (mut terminal, independently_equal_branch) = make();
        terminal.tamper_guard_source_branch_for_test(independently_equal_branch);
        assert!(matches!(
            terminal.reauthenticate_for_test(&context),
            Err(GeneratedAffineInitialGlobalAffineTerminalError::Guard(
                ResidualAffineBranchGuardCompositionError::FreshSourceBranchAllocationMismatch
            ))
        ));
    }

    #[test]
    fn opaque_initial_terminals_authenticate_natural_actionable_and_zero_guard_paths() {
        let mut ready = 0usize;
        let mut observed = [0usize; 4];
        let mut actionable_zero_guard = 0usize;
        for sector in ["011", "101", "110", "111"] {
            let (family, context, cover) = generated_sunset_cover_for_sector(sector);
            // The V2 adapter is entered only after this one complete replay.
            // None of the per-terminal calls invokes a public V1 replay.
            cover.replay(&family, &context).unwrap();
            for node in cover.nodes() {
                if !matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                ) {
                    continue;
                }
                ready += 1;
                let fresh = ResidualAffineBranchSystemCertificate::compile_fresh_replayed(
                    &family,
                    &context,
                    Arc::clone(&cover),
                    node.ordinal(),
                    ResidualAffineBranchSystemLimits::default(),
                )
                .unwrap();
                let terminal =
                    GeneratedAffineInitialGlobalAffineTerminal::compile_from_fresh_branch(
                        &context,
                        cover.source_work_item_ordinal(),
                        node.ordinal(),
                        fresh,
                        ResidualAffineBranchGuardCompositionLimits::default(),
                    )
                    .unwrap();
                terminal.reauthenticate_for_test(&context).unwrap();
                match terminal.outcome() {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty => {
                        observed[0] += 1;
                        assert_eq!(terminal.guard_count(), 0);
                    }
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported => {
                        observed[1] += 1;
                        assert_eq!(terminal.guard_count(), 0);
                    }
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction => {
                        observed[2] += 1;
                        assert!(terminal.guard_count() > 0);
                    }
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable => {
                        observed[3] += 1;
                        actionable_zero_guard += usize::from(terminal.guard_count() == 0);
                    }
                }
            }
        }
        assert!(ready > 0);
        assert_eq!(observed.into_iter().sum::<usize>(), ready);
        assert_eq!(
            &observed[..3],
            &[0, 0, 0],
            "these concrete generated sectors are the bounded natural actionable oracle"
        );
        assert!(observed[3] > 0, "fixture must exercise Actionable");
        assert!(
            actionable_zero_guard > 0,
            "fixture must exercise a guarded Actionable branch with zero guards"
        );
    }

    #[test]
    fn sealed_guard_v2_pair_census_is_exact_and_one_below_on_all_axes() {
        let (family, context, cover) = generated_sunset_cover();
        cover.replay(&family, &context).unwrap();
        let (_, guarded) = first_fresh_guarded_with_nonzero_guards(&family, &context, &cover);
        let baseline = ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
            &context,
            Arc::clone(&guarded.branch),
            guarded.authorization,
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
        .unwrap();
        let memory_envelope = sealed_guard_memory_envelope_parts_from_limits(
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
        .unwrap();
        assert!(
            baseline.memory().retained_owned_logical_bytes()
                <= memory_envelope.guard_retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            baseline.memory().plan_retained_owned_logical_bytes()
                <= memory_envelope.plan_retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            baseline
                .memory()
                .plan_compilation_owned_logical_peak_upper_bound()
                <= memory_envelope.plan_compilation_owned_logical_peak_upper_bound()
        );
        let (units, bytes, integer_bits) = baseline.payload_comparison_census_for_test();
        assert!(units > 0 && bytes > 0 && integer_bits > 0);

        for (resource, exact_value, set_limit) in [
            ("payload comparison units", units, 0usize),
            ("payload comparison bytes", bytes, 1usize),
            ("payload comparison integer bits", integer_bits, 2usize),
        ] {
            let configure = |value: usize| {
                let mut limits = ResidualAffineBranchGuardCompositionLimits::default();
                match set_limit {
                    0 => limits.max_payload_comparison_units = value,
                    1 => limits.max_payload_comparison_bytes = value,
                    2 => limits.max_payload_comparison_integer_bits = value,
                    _ => unreachable!(),
                }
                limits
            };
            let (_, guarded) = first_fresh_guarded_with_nonzero_guards(&family, &context, &cover);
            ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                &context,
                Arc::clone(&guarded.branch),
                guarded.authorization,
                configure(exact_value),
            )
            .unwrap();
            let (_, guarded) = first_fresh_guarded_with_nonzero_guards(&family, &context, &cover);
            assert!(matches!(
                ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                    &context,
                    Arc::clone(&guarded.branch),
                    guarded.authorization,
                    configure(exact_value - 1),
                ),
                Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == exact_value && limit == exact_value - 1
            ));
        }
    }

    #[test]
    fn sealed_guard_gmp_rounding_is_checked() {
        assert_eq!(guard_gmp_logical_bytes_upper_bound(2, 129).unwrap(), 34);
        assert!(guard_gmp_logical_bytes_upper_bound(usize::MAX, usize::MAX).is_err());
    }

    #[test]
    fn branch_limit_memory_envelope_is_checked_exact_and_dominates_fresh_results() {
        assert_eq!(branch_gmp_logical_bytes_upper_bound(2, 129).unwrap(), 34);
        assert!(matches!(
            branch_gmp_logical_bytes_upper_bound(usize::MAX, 0),
            Err(ResidualAffineBranchSystemError::ResourceCountOverflow {
                resource: "branch GMP logical bytes"
            })
        ));

        let mut zero = ResidualAffineBranchSystemLimits::default();
        zero.max_family_fingerprint_bytes = 0;
        zero.max_context_fingerprint_bytes = 0;
        zero.max_zero_atoms = 0;
        zero.max_nonzero_guards = 0;
        zero.max_zero_atom_source_terms = 0;
        zero.max_zero_atom_exponent_entries = 0;
        zero.max_potential_block_witnesses = 0;
        zero.max_potential_block_exponent_entries = 0;
        zero.max_potential_retained_integer_bits = 0;
        zero.max_retained_atom_context_fingerprint_bytes = 0;
        zero.max_potential_row_components = 0;
        zero.max_system_input_rows = 0;
        zero.max_system_lineage_ordinals = 0;
        zero.max_unsupported_reasons = 0;
        let nested =
            residual_affine_integer_system_memory_envelope_from_limits(zero.integer_system)
                .unwrap();
        let fixed =
            branch_arc_owned_logical_bytes::<ResidualAffineBranchSystemCertificate>().unwrap();
        let envelope = residual_affine_branch_system_memory_envelope_from_limits(zero).unwrap();
        assert_eq!(
            envelope.retained_owned_logical_bytes_upper_bound(),
            fixed + nested.retained_owned_logical_bytes_upper_bound()
        );
        assert_eq!(
            envelope.compilation_owned_logical_peak_upper_bound(),
            (fixed + nested.retained_owned_logical_bytes_upper_bound())
                .max(fixed + size_of::<Vec<ResidualAffineIntegerSystemInputRow>>())
                .max(
                    fixed
                        + size_of::<Vec<ResidualAffineIntegerSystemInputRow>>()
                        + nested.compilation_owned_logical_peak_upper_bound()
                )
        );

        let defaults = residual_affine_branch_system_memory_envelope_from_limits(
            ResidualAffineBranchSystemLimits::default(),
        )
        .unwrap();
        assert!(
            defaults.compilation_owned_logical_peak_upper_bound()
                >= defaults.retained_owned_logical_bytes_upper_bound()
        );
        let (family, context, cover) = generated_sunset_cover();
        cover.replay(&family, &context).unwrap();
        let (_, guarded) = first_fresh_guarded(&family, &context, &cover, false);
        assert!(
            guarded.memory.retained_owned_logical_bytes()
                <= defaults.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            guarded.memory.compilation_owned_logical_peak_upper_bound()
                <= defaults.compilation_owned_logical_peak_upper_bound()
        );

        let mut overflow = ResidualAffineBranchSystemLimits::default();
        overflow.max_zero_atoms = usize::MAX;
        assert!(matches!(
            residual_affine_branch_system_memory_envelope_from_limits(overflow),
            Err(ResidualAffineBranchSystemError::ResourceCountOverflow {
                resource: "branch logical memory envelope"
            })
        ));
    }
}
