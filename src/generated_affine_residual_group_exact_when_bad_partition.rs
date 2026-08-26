//! Owning exact relative `WhenBad` partition for the current generated-affine lineage.
//!
//! This phase consumes only the move-only materialization produced by
//! `generated_affine_residual_group_exact_when_bad_materialization`.  All
//! fallible work borrows that owner.  Only after materialization replay,
//! deterministic condition/formula assembly, and source-neutral relative
//! partition compilation have succeeded is the untouched owner moved into a
//! non-publishing terminal.  Operational failure (including a caught panic)
//! returns the exact input materialization.
//!
//! The module is deliberately topology-neutral.  It knows only the authenticated
//! source schedule, physical-parameter identity projections, and exact boundary
//! events already retained by the materializer.  Polynomial validation,
//! coefficient-field associate proofs, arbitrary-width formula routing, and
//! relative-domain divisibility inference are delegated to Symbolica-backed
//! source-neutral APIs.

use std::fmt;
use std::mem::size_of;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;

use crate::generated_affine_residual_group_exact_session::GeneratedAffineResidualGroupExactSession;
use crate::generated_affine_residual_group_exact_when_bad_conditions::{
    GeneratedAffineResidualGroupExactConditionHazardLocator,
    GeneratedAffineResidualGroupExactConditionSourceLocator,
};
use crate::generated_affine_residual_group_exact_when_bad_materialization::{
    GeneratedAffineResidualGroupExactBoundaryDisposition,
    GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    GeneratedAffineResidualGroupExactMappedConditionClass,
    GeneratedAffineResidualGroupExactMappedSource,
    GeneratedAffineResidualGroupExactWhenBadIdenticallyBad as MaterializedIdenticallyBad,
    GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason as MaterializedIdenticallyBadReason,
    GeneratedAffineResidualGroupExactWhenBadMaterialization,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
    GeneratedAffineResidualGroupExactWhenBadReadyForPartition as MaterializedReadyForPartition,
};
use crate::generated_residual_affine_when_bad::{
    AffineWhenBadArbitraryRelativeLimits, AffineWhenBadArbitraryRelativePartitionCertificate,
    AffineWhenBadArbitraryRelativePartitionCompiler, AffineWhenBadArbitraryRelativeProblem,
    AffineWhenBadAtom, AffineWhenBadInheritedTruth, AffineWhenBadRelativeCaseError,
    AffineWhenBadRelativeCaseLimits,
};
use crate::parametric_coefficient::{
    ParametricParameterIdentityClass, ParametricPolynomialAssociateLimits,
};
use crate::{
    ExactAlgebraLimits, IntegralFamily, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial, SymbolicPolynomialPredicateKind,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-when-bad-partition-v1";

#[cfg(test)]
std::thread_local! {
    static PARTITION_PREPARE_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static PARTITION_RESERVE_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn inject_partition_prepare_panic_for_test() {
    PARTITION_PREPARE_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_partition_prepare_panic_for_test() {
    PARTITION_PREPARE_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected exact WhenBad partition preparation panic");
        }
    });
}

#[cfg(test)]
fn reset_partition_reserve_observed_for_test() {
    PARTITION_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
}

#[cfg(test)]
fn partition_reserve_was_observed_for_test() -> bool {
    PARTITION_RESERVE_OBSERVED_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn mark_partition_reserve_observed_for_test() {
    PARTITION_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(not(test))]
fn maybe_inject_partition_prepare_panic_for_test() {}

#[cfg(not(test))]
fn mark_partition_reserve_observed_for_test() {}

/// Limits for one current-lineage partition attempt.
///
/// `associate` bounds every individual Symbolica associate proof.  The outer
/// aggregate call and native-work limits prevent that allowance from being
/// reset for an unbounded source stream.  `relative` is intersected with the
/// owner-wide remaining retained allowance before the source-neutral compiler
/// is entered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadPartitionLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) associate: ParametricPolynomialAssociateLimits,
    pub(crate) relative: AffineWhenBadRelativeCaseLimits,
    pub(crate) max_source_transcripts: usize,
    pub(crate) max_boundary_transcripts: usize,
    pub(crate) max_locus_inputs: usize,
    pub(crate) max_structural_loci: usize,
    pub(crate) max_inherited_truth_occurrences: usize,
    pub(crate) max_inherited_truths: usize,
    pub(crate) max_bad_atoms: usize,
    pub(crate) max_bad_clauses: usize,
    pub(crate) max_locus_origins: usize,
    pub(crate) max_inherited_provenances: usize,
    pub(crate) max_atom_provenances: usize,
    pub(crate) max_clause_provenances: usize,
    pub(crate) max_intern_equality_comparisons: usize,
    pub(crate) max_intern_equality_term_pairs: usize,
    pub(crate) max_intern_associate_comparisons: usize,
    pub(crate) max_intern_associate_native_cross_term_pairs: usize,
    pub(crate) max_intern_associate_rustred_visible_temporary_byte_peak: usize,
    pub(crate) max_intern_associate_native_workspace_byte_peak: usize,
    pub(crate) max_retained_polynomial_terms: usize,
    pub(crate) max_retained_polynomial_exponent_entries: usize,
    pub(crate) max_retained_polynomial_integer_bits: usize,
    pub(crate) max_transcript_retained_owned_logical_bytes: usize,
    pub(crate) max_source_problem_retained_owned_logical_bytes: usize,
    pub(crate) max_partition_formula_retained_owned_logical_bytes: usize,
    pub(crate) max_partition_formula_compilation_owned_logical_peak_upper_bound: usize,
    pub(crate) max_partition_work_owned_logical_peak_upper_bound: usize,
    pub(crate) max_partition_compiler_owned_logical_peak_upper_bound: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_compilation_owned_logical_peak_upper_bound: usize,
    pub(crate) max_payload_comparison_units: usize,
    pub(crate) max_payload_comparison_integer_bits: usize,
}

impl Default for GeneratedAffineResidualGroupExactWhenBadPartitionLimits {
    fn default() -> Self {
        const LARGE: usize = 256_000_000;
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            associate: ParametricPolynomialAssociateLimits::default(),
            relative: AffineWhenBadRelativeCaseLimits::default(),
            max_source_transcripts: 48_000_000,
            max_boundary_transcripts: LARGE,
            max_locus_inputs: 512_000_000,
            max_structural_loci: LARGE,
            max_inherited_truth_occurrences: 32_000_000,
            max_inherited_truths: 32_000_000,
            max_bad_atoms: 512_000_000,
            max_bad_clauses: LARGE,
            max_locus_origins: 512_000_000,
            max_inherited_provenances: 32_000_000,
            max_atom_provenances: 512_000_000,
            max_clause_provenances: LARGE,
            max_intern_equality_comparisons: 1_000_000_000,
            max_intern_equality_term_pairs: usize::MAX,
            max_intern_associate_comparisons: 1_000_000_000,
            max_intern_associate_native_cross_term_pairs: usize::MAX,
            max_intern_associate_rustred_visible_temporary_byte_peak: usize::MAX,
            max_intern_associate_native_workspace_byte_peak: usize::MAX,
            max_retained_polynomial_terms: usize::MAX,
            max_retained_polynomial_exponent_entries: usize::MAX,
            max_retained_polynomial_integer_bits: usize::MAX,
            max_transcript_retained_owned_logical_bytes: usize::MAX,
            max_source_problem_retained_owned_logical_bytes: usize::MAX,
            max_partition_formula_retained_owned_logical_bytes: usize::MAX,
            max_partition_formula_compilation_owned_logical_peak_upper_bound: usize::MAX,
            max_partition_work_owned_logical_peak_upper_bound: usize::MAX,
            max_partition_compiler_owned_logical_peak_upper_bound: usize::MAX,
            max_retained_owned_logical_bytes: usize::MAX,
            max_compilation_owned_logical_peak_upper_bound: usize::MAX,
            max_payload_comparison_units: usize::MAX,
            max_payload_comparison_integer_bits: usize::MAX,
        }
    }
}

/// Exact semantic source of one structural-locus occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadLocusOrigin {
    TargetPremise {
        source_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    RecenteredRowGuard {
        source_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    DenominatorIdentityCoefficient {
        source_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
        kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
        coefficient_locus_ordinal: usize,
    },
    RetainedBoundary {
        boundary_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionHazardLocator,
    },
}

/// Every source occurrence is retained even when equality/associate interning
/// maps several occurrences to the same first-seen structural locus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord {
    ordinal: usize,
    locus_ordinal: usize,
    origin: GeneratedAffineResidualGroupExactWhenBadLocusOrigin,
}

impl GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn origin(self) -> GeneratedAffineResidualGroupExactWhenBadLocusOrigin {
        self.origin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadConditionPartitionDisposition {
    Discharged,
    InheritedNonzero { locus_ordinal: usize },
    CandidateBadClause { clause_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition {
    NeverIdentityZero,
    ConditionalBadClause {
        clause_ordinal: usize,
        first_atom_ordinal: usize,
        atom_count: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript {
    kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    disposition: GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition,
}

impl GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript {
    pub(crate) const fn kind(self) -> GeneratedAffineResidualGroupExactDenominatorIdentityKind {
        self.kind
    }

    pub(crate) const fn disposition(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition {
        self.disposition
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind {
    Condition {
        class: GeneratedAffineResidualGroupExactMappedConditionClass,
        disposition: GeneratedAffineResidualGroupExactWhenBadConditionPartitionDisposition,
    },
    Coefficient {
        denominators: [GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript; 2],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadSourceTranscript {
    ordinal: usize,
    source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    kind: GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind,
}

impl GeneratedAffineResidualGroupExactWhenBadSourceTranscript {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn source(self) -> GeneratedAffineResidualGroupExactConditionSourceLocator {
        self.source
    }

    pub(crate) const fn kind(self) -> GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition {
    Empty,
    SuppressedByNumerator,
    RetainedBadClause { clause_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript {
    ordinal: usize,
    source: GeneratedAffineResidualGroupExactConditionHazardLocator,
    disposition: GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition,
}

impl GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn source(self) -> GeneratedAffineResidualGroupExactConditionHazardLocator {
        self.source
    }

    pub(crate) const fn disposition(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition {
        self.disposition
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadInheritedProvenance {
    ordinal: usize,
    locus_ordinal: usize,
    origin_record_ordinal: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadInheritedProvenance {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn origin_record_ordinal(self) -> usize {
        self.origin_record_ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadAtomProvenance {
    atom_ordinal: usize,
    clause_ordinal: usize,
    clause_atom_ordinal: usize,
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    origin_record_ordinal: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadAtomProvenance {
    pub(crate) const fn atom_ordinal(self) -> usize {
        self.atom_ordinal
    }

    pub(crate) const fn clause_ordinal(self) -> usize {
        self.clause_ordinal
    }

    pub(crate) const fn clause_atom_ordinal(self) -> usize {
        self.clause_atom_ordinal
    }

    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn origin_record_ordinal(self) -> usize {
        self.origin_record_ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadClauseSource {
    RecenteredRowGuard {
        source_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    DenominatorIdentity {
        source_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
        kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    },
    RetainedBoundary {
        boundary_ordinal: usize,
        source: GeneratedAffineResidualGroupExactConditionHazardLocator,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadClauseProvenance {
    ordinal: usize,
    first_atom_ordinal: usize,
    atom_count: usize,
    source: GeneratedAffineResidualGroupExactWhenBadClauseSource,
}

impl GeneratedAffineResidualGroupExactWhenBadClauseProvenance {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn first_atom_ordinal(self) -> usize {
        self.first_atom_ordinal
    }

    pub(crate) const fn atom_count(self) -> usize {
        self.atom_count
    }

    pub(crate) const fn source(self) -> GeneratedAffineResidualGroupExactWhenBadClauseSource {
        self.source
    }
}

/// Allocation-independent counters plus admitted polynomial and native work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadPartitionStats {
    source_transcripts: usize,
    boundary_transcripts: usize,
    locus_inputs: usize,
    structural_loci: usize,
    inherited_truth_occurrences: usize,
    inherited_truths: usize,
    bad_atoms: usize,
    bad_clauses: usize,
    locus_origins: usize,
    inherited_provenances: usize,
    atom_provenances: usize,
    clause_provenances: usize,
    intern_equality_comparisons: usize,
    intern_equality_term_pairs: usize,
    intern_associate_comparisons: usize,
    intern_associate_native_cross_term_pairs: usize,
    intern_associate_rustred_visible_temporary_byte_peak: usize,
    intern_associate_native_workspace_byte_peak: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    applicable_leaves: usize,
    exceptional_leaves: usize,
    transcript_retained_owned_logical_bytes: usize,
    source_problem_retained_owned_logical_bytes: usize,
    partition_formula_retained_owned_logical_bytes: usize,
    partition_formula_compilation_owned_logical_peak_upper_bound: usize,
    partition_work_owned_logical_peak_upper_bound: usize,
    partition_compiler_owned_logical_peak_upper_bound: usize,
    partition_retained_owned_logical_bytes: usize,
    materialization_retained_owned_logical_bytes: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    payload_comparison_units: usize,
    payload_comparison_integer_bits: usize,
}

macro_rules! partition_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupExactWhenBadPartitionStats {
    partition_stats_getters!(
        source_transcripts,
        boundary_transcripts,
        locus_inputs,
        structural_loci,
        inherited_truth_occurrences,
        inherited_truths,
        bad_atoms,
        bad_clauses,
        locus_origins,
        inherited_provenances,
        atom_provenances,
        clause_provenances,
        intern_equality_comparisons,
        intern_equality_term_pairs,
        intern_associate_comparisons,
        intern_associate_native_cross_term_pairs,
        intern_associate_rustred_visible_temporary_byte_peak,
        intern_associate_native_workspace_byte_peak,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        applicable_leaves,
        exceptional_leaves,
        transcript_retained_owned_logical_bytes,
        source_problem_retained_owned_logical_bytes,
        partition_formula_retained_owned_logical_bytes,
        partition_formula_compilation_owned_logical_peak_upper_bound,
        partition_work_owned_logical_peak_upper_bound,
        partition_compiler_owned_logical_peak_upper_bound,
        partition_retained_owned_logical_bytes,
        materialization_retained_owned_logical_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
        payload_comparison_units,
        payload_comparison_integer_bits,
    );
}

/// Provenance-only outer transcript.  Structural polynomials and formula atoms
/// are owned exactly once by `partition` below.
struct GeneratedAffineResidualGroupExactWhenBadPartitionTranscript {
    source_transcripts: Vec<GeneratedAffineResidualGroupExactWhenBadSourceTranscript>,
    boundary_transcripts: Vec<GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript>,
    locus_origins: Vec<GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord>,
    inherited_provenance: Vec<GeneratedAffineResidualGroupExactWhenBadInheritedProvenance>,
    atom_provenance: Vec<GeneratedAffineResidualGroupExactWhenBadAtomProvenance>,
    clause_provenance: Vec<GeneratedAffineResidualGroupExactWhenBadClauseProvenance>,
}

impl PartialEq for GeneratedAffineResidualGroupExactWhenBadPartitionTranscript {
    fn eq(&self, other: &Self) -> bool {
        self.source_transcripts == other.source_transcripts
            && self.boundary_transcripts == other.boundary_transcripts
            && self.locus_origins == other.locus_origins
            && self.inherited_provenance == other.inherited_provenance
            && self.atom_provenance == other.atom_provenance
            && self.clause_provenance == other.clause_provenance
    }
}

impl Eq for GeneratedAffineResidualGroupExactWhenBadPartitionTranscript {}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadPartitionTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadPartitionTranscript")
            .field("source_transcripts", &self.source_transcripts)
            .field("boundary_transcripts", &self.boundary_transcripts)
            .field("locus_origin_count", &self.locus_origins.len())
            .field("inherited_count", &self.inherited_provenance.len())
            .field("atom_count", &self.atom_provenance.len())
            .field("clause_provenance", &self.clause_provenance)
            .finish()
    }
}

/// A relative partition with at least one applicable leaf, awaiting the later
/// atomic publication phase.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadReadyForPublication {
    schema: &'static str,
    owner: MaterializedReadyForPartition,
    transcript: GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
    partition: AffineWhenBadArbitraryRelativePartitionCertificate,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
}

impl GeneratedAffineResidualGroupExactWhenBadReadyForPublication {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactWhenBadPartitionLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        self.stats
    }

    pub(crate) fn source_transcripts(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadSourceTranscript] {
        &self.transcript.source_transcripts
    }

    pub(crate) fn boundary_transcripts(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript] {
        &self.transcript.boundary_transcripts
    }

    pub(crate) fn locus_origins(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord] {
        &self.transcript.locus_origins
    }

    pub(crate) fn atom_provenance(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadAtomProvenance] {
        &self.transcript.atom_provenance
    }

    pub(crate) fn inherited_provenance(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadInheritedProvenance] {
        &self.transcript.inherited_provenance
    }

    pub(crate) fn clause_provenance(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactWhenBadClauseProvenance] {
        &self.transcript.clause_provenance
    }

    pub(crate) const fn partition(&self) -> &AffineWhenBadArbitraryRelativePartitionCertificate {
        &self.partition
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
        replay_partition_terminal(
            self.schema,
            &self.owner,
            &self.transcript,
            &self.partition,
            self.limits,
            self.stats,
            family,
            context,
            session,
            true,
        )
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadReadyForPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadReadyForPublication")
            .field("schema", &self.schema)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("private_owner", &"<redacted>")
            .field("private_partition", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason {
    Upstream(MaterializedIdenticallyBadReason),
    NoStructurallyApplicableRelativeLeaf,
}

enum GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload {
    Upstream(MaterializedIdenticallyBad),
    AllExceptional {
        owner: MaterializedReadyForPartition,
        transcript: GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
        partition: AffineWhenBadArbitraryRelativePartitionCertificate,
    },
}

/// Replayable proof that the materialized candidate is bad on the complete
/// authenticated target domain.  It publishes and consumes nothing.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad {
    schema: &'static str,
    reason: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason,
    payload: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
}

impl GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad {
    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason {
        self.reason
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA {
                return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::SchemaMismatch);
            }
            match &self.payload {
                GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::Upstream(
                    owner,
                ) => {
                    owner.replay(family, context, session)?;
                    if self.reason
                        != GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason::Upstream(
                            owner.reason(),
                        )
                    {
                        return Err(
                            GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch,
                        );
                    }
                    let rebuilt_stats = prepare_upstream_identically_bad_stats(owner, self.limits)?;
                    if rebuilt_stats != self.stats {
                        return Err(
                            GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch,
                        );
                    }
                    Ok(())
                }
                GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::AllExceptional {
                    owner,
                    transcript,
                    partition,
                } => replay_partition_terminal(
                    self.schema,
                    owner,
                    transcript,
                    partition,
                    self.limits,
                    self.stats,
                    family,
                    context,
                    session,
                    false,
                ),
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactWhenBadPartitionError::SymbolicaPanic)?
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad")
            .field("schema", &self.schema)
            .field("reason", &self.reason)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

pub(crate) enum GeneratedAffineResidualGroupExactWhenBadPartitionCompilation {
    ReadyForPublication(GeneratedAffineResidualGroupExactWhenBadReadyForPublication),
    IdenticallyBad(GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad),
}

impl GeneratedAffineResidualGroupExactWhenBadPartitionCompilation {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        match self {
            Self::ReadyForPublication(value) => value.stats,
            Self::IdenticallyBad(value) => value.stats,
        }
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
        match self {
            Self::ReadyForPublication(value) => value.replay(family, context, session),
            Self::IdenticallyBad(value) => value.replay(family, context, session),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadPartitionCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadyForPublication(value) => formatter
                .debug_tuple("ReadyForPublication")
                .field(value)
                .finish(),
            Self::IdenticallyBad(value) => formatter
                .debug_tuple("IdenticallyBad")
                .field(value)
                .finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadPartitionError {
    Materialization(GeneratedAffineResidualGroupExactWhenBadMaterializationError),
    Relative(AffineWhenBadRelativeCaseError),
    ParametricCoefficient(ParametricCoefficientError),
    SchemaMismatch,
    MalformedMaterialization,
    ReplayMismatch,
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
        requested: usize,
    },
    SymbolicaPanic,
}

impl fmt::Display for GeneratedAffineResidualGroupExactWhenBadPartitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Materialization(error) => error.fmt(formatter),
            Self::Relative(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::SchemaMismatch => formatter.write_str("exact WhenBad partition schema mismatch"),
            Self::MalformedMaterialization => {
                formatter.write_str("exact WhenBad materialization is malformed for partitioning")
            }
            Self::ReplayMismatch => formatter.write_str("exact WhenBad partition replay mismatch"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight",
            ),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during exact WhenBad partitioning")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactWhenBadPartitionError {}

impl From<GeneratedAffineResidualGroupExactWhenBadMaterializationError>
    for GeneratedAffineResidualGroupExactWhenBadPartitionError
{
    fn from(value: GeneratedAffineResidualGroupExactWhenBadMaterializationError) -> Self {
        Self::Materialization(value)
    }
}

impl From<AffineWhenBadRelativeCaseError>
    for GeneratedAffineResidualGroupExactWhenBadPartitionError
{
    fn from(value: AffineWhenBadRelativeCaseError) -> Self {
        Self::Relative(value)
    }
}

impl From<ParametricCoefficientError> for GeneratedAffineResidualGroupExactWhenBadPartitionError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Recoverable operational failure retaining the exact non-Clone input owner.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
    error: GeneratedAffineResidualGroupExactWhenBadPartitionError,
    owner: GeneratedAffineResidualGroupExactWhenBadMaterialization,
}

impl GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
    pub(crate) const fn error(&self) -> &GeneratedAffineResidualGroupExactWhenBadPartitionError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactWhenBadPartitionError,
        GeneratedAffineResidualGroupExactWhenBadMaterialization,
    ) {
        (self.error, self.owner)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadPartitionFailure")
            .field("error", &self.error)
            .field("private_owner", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactWhenBadPartitionFailure {}

pub(crate) struct GeneratedAffineResidualGroupExactWhenBadPartitionCompiler;

impl GeneratedAffineResidualGroupExactWhenBadPartitionCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
        owner: GeneratedAffineResidualGroupExactWhenBadMaterialization,
        limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
        GeneratedAffineResidualGroupExactWhenBadPartitionFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            prepare_partition_terminal(family, context, session, &owner, limits)
        }));
        match (prepared, owner) {
            (
                Ok(Ok(PreparedPartitionTerminal::UpstreamIdenticallyBad { stats })),
                GeneratedAffineResidualGroupExactWhenBadMaterialization::IdenticallyBad(owner),
            ) => {
                let reason =
                    GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason::Upstream(
                        owner.reason(),
                    );
                Ok(GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::IdenticallyBad(
                    GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad {
                        schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA,
                        reason,
                        payload: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::Upstream(owner),
                        limits,
                        stats,
                    },
                ))
            }
            (
                Ok(Ok(PreparedPartitionTerminal::Partitioned {
                    transcript,
                    partition,
                    stats,
                    applicable,
                })),
                GeneratedAffineResidualGroupExactWhenBadMaterialization::ReadyForPartition(owner),
            ) => {
                // Today every locally assembled bad clause is a nonempty
                // conjunction of `= 0` atoms and every inherited premise is
                // `!= 0`, so the all-nonzero assignment supplies an applicable
                // leaf.  Keep the all-exceptional terminal explicit and
                // replayable because the source-neutral arbitrary seam also
                // supports future formula forms for which that invariant need
                // not hold.
                if applicable {
                    Ok(GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(
                        GeneratedAffineResidualGroupExactWhenBadReadyForPublication {
                            schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA,
                            owner,
                            transcript,
                            partition,
                            limits,
                            stats,
                        },
                    ))
                } else {
                    Ok(GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::IdenticallyBad(
                        GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad {
                            schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA,
                            reason: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason::NoStructurallyApplicableRelativeLeaf,
                            payload: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::AllExceptional {
                                owner,
                                transcript,
                                partition,
                            },
                            limits,
                            stats,
                        },
                    ))
                }
            }
            (Ok(Ok(_)), owner) => Err(GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
                error: GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch,
                owner,
            }),
            (Ok(Err(error)), owner) => {
                Err(GeneratedAffineResidualGroupExactWhenBadPartitionFailure { error, owner })
            }
            (Err(_), owner) => Err(GeneratedAffineResidualGroupExactWhenBadPartitionFailure {
                error: GeneratedAffineResidualGroupExactWhenBadPartitionError::SymbolicaPanic,
                owner,
            }),
        }
    }
}

enum PreparedPartitionTerminal {
    UpstreamIdenticallyBad {
        stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    },
    Partitioned {
        transcript: GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
        partition: AffineWhenBadArbitraryRelativePartitionCertificate,
        stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
        applicable: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PartitionAssemblyAdmission {
    source_transcripts: usize,
    boundary_transcripts: usize,
    locus_inputs: usize,
    inherited_truth_occurrences: usize,
    bad_atoms: usize,
    bad_clauses: usize,
    transcript_retained_owned_logical_bytes: usize,
    source_problem_container_owned_logical_bytes: usize,
    assembly_container_owned_logical_bytes: usize,
}

struct PreparedPartitionAssembly {
    structural_loci: Vec<ParametricPolynomial>,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    atoms: Vec<AffineWhenBadAtom>,
    clause_ranges: Vec<Range<usize>>,
    transcript: GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
}

fn prepare_partition_terminal(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    owner: &GeneratedAffineResidualGroupExactWhenBadMaterialization,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<PreparedPartitionTerminal, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    owner.replay(family, context, session)?;
    match owner {
        GeneratedAffineResidualGroupExactWhenBadMaterialization::IdenticallyBad(owner) => {
            let stats = prepare_upstream_identically_bad_stats(owner, limits)?;
            Ok(PreparedPartitionTerminal::UpstreamIdenticallyBad { stats })
        }
        GeneratedAffineResidualGroupExactWhenBadMaterialization::ReadyForPartition(owner) => {
            prepare_ready_partition(context, owner, limits)
        }
    }
}

fn prepare_upstream_identically_bad_stats(
    owner: &MaterializedIdenticallyBad,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<
    GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    GeneratedAffineResidualGroupExactWhenBadPartitionError,
> {
    let mut stats = GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        materialization_retained_owned_logical_bytes: owner.stats().retained_owned_logical_bytes(),
        ..GeneratedAffineResidualGroupExactWhenBadPartitionStats::default()
    };
    stats.retained_owned_logical_bytes = checked_add(
        "exact WhenBad partition retained owned logical bytes",
        size_of::<GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad>(),
        stats.materialization_retained_owned_logical_bytes,
    )?;
    stats.compilation_owned_logical_peak_upper_bound = stats.retained_owned_logical_bytes;
    authenticate_final_stats(stats, limits)?;
    Ok(stats)
}

fn prepare_ready_partition(
    context: &ParametricCoefficientContext,
    owner: &MaterializedReadyForPartition,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<PreparedPartitionTerminal, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let admission = preflight_partition_assembly(owner, limits)?;
    let prepared = assemble_partition_problem(context, owner, admission, limits)?;
    maybe_inject_partition_prepare_panic_for_test();

    let PreparedPartitionAssembly {
        structural_loci,
        inherited_truths,
        atoms,
        clause_ranges,
        transcript,
        mut stats,
    } = prepared;

    let materialization_retained = owner.stats().retained_owned_logical_bytes();
    let outer_before_partition = checked_add(
        "exact WhenBad partition retained owned logical bytes",
        materialization_retained,
        checked_add(
            "exact WhenBad partition retained owned logical bytes",
            transcript_retained_bytes(&transcript)?,
            partitioned_terminal_header_byte_bound(),
        )?,
    )?;
    check_limit(
        "exact WhenBad partition retained owned logical bytes",
        outer_before_partition,
        limits.max_retained_owned_logical_bytes,
    )?;

    let mut relative_limits = limits.relative;
    relative_limits.exact_algebra =
        intersect_exact_limits(relative_limits.exact_algebra, limits.exact_algebra);
    relative_limits.max_structural_loci = relative_limits
        .max_structural_loci
        .min(limits.max_structural_loci);
    relative_limits.max_inherited_truths = relative_limits
        .max_inherited_truths
        .min(limits.max_inherited_truths);
    relative_limits.max_bad_atoms = relative_limits.max_bad_atoms.min(limits.max_bad_atoms);
    relative_limits.max_bad_clauses = relative_limits.max_bad_clauses.min(limits.max_bad_clauses);
    relative_limits.max_retained_bytes = relative_limits.max_retained_bytes.min(remaining_limit(
        "exact WhenBad partition retained owned logical bytes",
        limits.max_retained_owned_logical_bytes,
        outer_before_partition,
    )?);
    relative_limits.max_payload_comparison_units = relative_limits
        .max_payload_comparison_units
        .min(remaining_limit(
            "exact WhenBad partition payload comparison units",
            limits.max_payload_comparison_units,
            transcript_payload_units(&transcript)?,
        )?);
    relative_limits.max_payload_comparison_integer_bits = relative_limits
        .max_payload_comparison_integer_bits
        .min(limits.max_payload_comparison_integer_bits);

    let problem = AffineWhenBadArbitraryRelativeProblem::from_preallocated(
        structural_loci,
        inherited_truths,
        atoms,
        clause_ranges,
    );
    let source_problem_retained = problem.retained_owned_logical_byte_bound()?;
    if source_problem_retained > stats.source_problem_retained_owned_logical_bytes {
        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
    }
    check_limit(
        "exact WhenBad partition source problem retained owned logical bytes",
        source_problem_retained,
        limits.max_source_problem_retained_owned_logical_bytes,
    )?;
    stats.source_problem_retained_owned_logical_bytes = source_problem_retained;
    let core_compilation_peak_allowance = remaining_limit(
        "exact WhenBad partition compilation owned logical peak upper bound",
        limits.max_compilation_owned_logical_peak_upper_bound,
        outer_before_partition,
    )?;
    let arbitrary_limits = AffineWhenBadArbitraryRelativeLimits {
        relative: relative_limits,
        // Keep every child component ceiling authoritative.  The aggregate
        // outer allowance is represented independently below, so clamping a
        // component to it would mislabel an outer-global failure as a child
        // source/formula/work/compiler failure.
        max_source_problem_owned_logical_bytes: limits
            .max_source_problem_retained_owned_logical_bytes,
        max_formula_retained_owned_logical_bytes: limits
            .max_partition_formula_retained_owned_logical_bytes,
        max_formula_compilation_owned_logical_peak_upper_bound: limits
            .max_partition_formula_compilation_owned_logical_peak_upper_bound,
        max_work_owned_logical_peak_upper_bound: limits
            .max_partition_work_owned_logical_peak_upper_bound,
        max_compiler_owned_logical_peak_upper_bound: limits
            .max_partition_compiler_owned_logical_peak_upper_bound,
        max_compilation_owned_logical_peak_upper_bound: core_compilation_peak_allowance,
    };
    let partition = match AffineWhenBadArbitraryRelativePartitionCompiler::compile(
        context,
        problem,
        arbitrary_limits,
    ) {
        Ok(partition) => partition,
        Err(AffineWhenBadRelativeCaseError::ResourceLimit {
            resource: "affine WhenBad arbitrary compilation owned logical peak upper bound",
            requested,
            ..
        }) => {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                    requested: checked_add(
                        "exact WhenBad partition compilation owned logical peak upper bound",
                        outer_before_partition,
                        requested,
                    )?,
                    limit: limits.max_compilation_owned_logical_peak_upper_bound,
                },
            );
        }
        Err(AffineWhenBadRelativeCaseError::ResourceCountOverflow {
            resource: "affine WhenBad arbitrary compilation owned logical peak upper bound",
        }) => {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                },
            );
        }
        Err(error) => return Err(error.into()),
    };

    let applicable_leaves = partition
        .classifications()
        .iter()
        .filter(|classification| classification.decisive_clause_ordinal().is_none())
        .count();
    let exceptional_leaves = partition
        .classifications()
        .len()
        .checked_sub(applicable_leaves)
        .ok_or(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)?;
    if partition.classifications().is_empty() {
        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
    }

    let partition_stats = partition.stats();
    let partition_compilation_stats = partition.compilation_stats();
    stats.source_problem_retained_owned_logical_bytes = source_problem_retained
        .max(partition_compilation_stats.source_problem_owned_logical_byte_envelope());
    check_limit(
        "exact WhenBad partition source problem retained owned logical bytes",
        stats.source_problem_retained_owned_logical_bytes,
        limits.max_source_problem_retained_owned_logical_bytes,
    )?;
    stats.applicable_leaves = applicable_leaves;
    stats.exceptional_leaves = exceptional_leaves;
    stats.transcript_retained_owned_logical_bytes = transcript_retained_bytes(&transcript)?;
    stats.partition_retained_owned_logical_bytes = partition_stats.retained_bytes();
    stats.partition_formula_retained_owned_logical_bytes =
        partition_compilation_stats.formula_retained_owned_logical_bytes();
    stats.partition_formula_compilation_owned_logical_peak_upper_bound =
        partition_compilation_stats.formula_compilation_owned_logical_peak_upper_bound();
    stats.partition_work_owned_logical_peak_upper_bound =
        partition_compilation_stats.work_owned_logical_peak_upper_bound();
    stats.partition_compiler_owned_logical_peak_upper_bound =
        partition_compilation_stats.compiler_owned_logical_peak_upper_bound();
    stats.materialization_retained_owned_logical_bytes = materialization_retained;
    stats.retained_owned_logical_bytes = checked_add(
        "exact WhenBad partition retained owned logical bytes",
        outer_before_partition,
        stats.partition_retained_owned_logical_bytes,
    )?;
    stats.payload_comparison_units = checked_add(
        "exact WhenBad partition payload comparison units",
        transcript_payload_units(&transcript)?,
        partition_stats.payload_comparison_units(),
    )?;
    stats.payload_comparison_integer_bits = partition_stats.payload_comparison_integer_bits();
    let core_compilation_peak = checked_add(
        "exact WhenBad partition compilation owned logical peak upper bound",
        stats.source_problem_retained_owned_logical_bytes,
        stats.partition_compiler_owned_logical_peak_upper_bound,
    )?;
    let outer_core_compilation_peak = checked_add(
        "exact WhenBad partition compilation owned logical peak upper bound",
        outer_before_partition,
        core_compilation_peak,
    )?;
    stats.compilation_owned_logical_peak_upper_bound = stats
        .compilation_owned_logical_peak_upper_bound
        .max(stats.retained_owned_logical_bytes)
        .max(outer_core_compilation_peak);
    authenticate_final_stats(stats, limits)?;

    Ok(PreparedPartitionTerminal::Partitioned {
        transcript,
        partition,
        stats,
        applicable: applicable_leaves != 0,
    })
}

fn preflight_partition_assembly(
    owner: &MaterializedReadyForPartition,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<PartitionAssemblyAdmission, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut admission = PartitionAssemblyAdmission {
        source_transcripts: owner.sources().len(),
        boundary_transcripts: owner.boundaries().len(),
        ..PartitionAssemblyAdmission::default()
    };

    for source in owner.sources() {
        match source {
            GeneratedAffineResidualGroupExactMappedSource::Condition(condition) => {
                if !matches!(
                    condition.source(),
                    GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise { .. }
                        | GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard { .. }
                ) {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                if condition.class()
                    != GeneratedAffineResidualGroupExactMappedConditionClass::IndexDependent
                {
                    continue;
                }
                admission.locus_inputs = checked_add(
                    "exact WhenBad partition locus inputs",
                    admission.locus_inputs,
                    1,
                )?;
                match condition.source() {
                    GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise { .. } => {
                        admission.inherited_truth_occurrences = checked_add(
                            "exact WhenBad partition inherited truth occurrences",
                            admission.inherited_truth_occurrences,
                            1,
                        )?;
                    }
                    GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard { .. } => {
                        admission.bad_atoms = checked_add(
                            "exact WhenBad partition bad atoms",
                            admission.bad_atoms,
                            1,
                        )?;
                        admission.bad_clauses = checked_add(
                            "exact WhenBad partition bad clauses",
                            admission.bad_clauses,
                            1,
                        )?;
                    }
                    GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient { .. }
                    | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient { .. } => {
                        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                    }
                }
            }
            GeneratedAffineResidualGroupExactMappedSource::Coefficient(coefficient) => {
                if !matches!(
                    coefficient.source(),
                    GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient { .. }
                        | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient { .. }
                ) {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                let identities = coefficient.denominator_identities();
                if identities[0].kind()
                    != GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator
                    || identities[1].kind()
                        != GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator
                {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                for identity in identities {
                    match identity.projection().class() {
                        ParametricParameterIdentityClass::AlwaysIdentityZero => {
                            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                        }
                        ParametricParameterIdentityClass::NeverIdentityZero { .. } => {}
                        ParametricParameterIdentityClass::Conditional { coefficient_loci } => {
                            if coefficient_loci.is_empty() {
                                return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                            }
                            admission.locus_inputs = checked_add(
                                "exact WhenBad partition locus inputs",
                                admission.locus_inputs,
                                coefficient_loci.len(),
                            )?;
                            admission.bad_atoms = checked_add(
                                "exact WhenBad partition bad atoms",
                                admission.bad_atoms,
                                coefficient_loci.len(),
                            )?;
                            admission.bad_clauses = checked_add(
                                "exact WhenBad partition bad clauses",
                                admission.bad_clauses,
                                1,
                            )?;
                        }
                    }
                }
            }
        }
    }

    for event in owner.boundaries() {
        match event.disposition() {
            GeneratedAffineResidualGroupExactBoundaryDisposition::Empty
            | GeneratedAffineResidualGroupExactBoundaryDisposition::SuppressedByNumerator => {}
            GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary => {
                if event.boundary().is_none() {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                admission.locus_inputs = checked_add(
                    "exact WhenBad partition locus inputs",
                    admission.locus_inputs,
                    1,
                )?;
                admission.bad_atoms =
                    checked_add("exact WhenBad partition bad atoms", admission.bad_atoms, 1)?;
                admission.bad_clauses = checked_add(
                    "exact WhenBad partition bad clauses",
                    admission.bad_clauses,
                    1,
                )?;
            }
            GeneratedAffineResidualGroupExactBoundaryDisposition::WholeTarget => {
                return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
            }
        }
    }

    for (resource, requested, limit) in [
        (
            "exact WhenBad partition source transcripts",
            admission.source_transcripts,
            limits.max_source_transcripts,
        ),
        (
            "exact WhenBad partition boundary transcripts",
            admission.boundary_transcripts,
            limits.max_boundary_transcripts,
        ),
        (
            "exact WhenBad partition locus inputs",
            admission.locus_inputs,
            limits.max_locus_inputs,
        ),
        (
            "exact WhenBad partition inherited truth occurrences",
            admission.inherited_truth_occurrences,
            limits.max_inherited_truth_occurrences,
        ),
        (
            "exact WhenBad partition bad atoms",
            admission.bad_atoms,
            limits.max_bad_atoms,
        ),
        (
            "exact WhenBad partition bad clauses",
            admission.bad_clauses,
            limits.max_bad_clauses,
        ),
        (
            "exact WhenBad partition locus origins",
            admission.locus_inputs,
            limits.max_locus_origins,
        ),
        (
            "exact WhenBad partition inherited provenances",
            admission.inherited_truth_occurrences,
            limits.max_inherited_provenances,
        ),
        (
            "exact WhenBad partition atom provenances",
            admission.bad_atoms,
            limits.max_atom_provenances,
        ),
        (
            "exact WhenBad partition clause provenances",
            admission.bad_clauses,
            limits.max_clause_provenances,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }

    admission.transcript_retained_owned_logical_bytes =
        admitted_transcript_retained_bytes(admission)?;
    check_limit(
        "exact WhenBad partition transcript retained owned logical bytes",
        admission.transcript_retained_owned_logical_bytes,
        limits.max_transcript_retained_owned_logical_bytes,
    )?;
    admission.source_problem_container_owned_logical_bytes =
        admitted_source_problem_container_bytes(admission)?;
    check_limit(
        "exact WhenBad partition source problem retained owned logical bytes",
        admission.source_problem_container_owned_logical_bytes,
        limits.max_source_problem_retained_owned_logical_bytes,
    )?;
    admission.assembly_container_owned_logical_bytes =
        admitted_assembly_container_bytes(admission)?;

    let materialization_retained = owner.stats().retained_owned_logical_bytes();
    let prospective_outer_retained = checked_add(
        "exact WhenBad partition retained owned logical bytes",
        checked_add(
            "exact WhenBad partition retained owned logical bytes",
            materialization_retained,
            admission.transcript_retained_owned_logical_bytes,
        )?,
        partitioned_terminal_header_byte_bound(),
    )?;
    check_limit(
        "exact WhenBad partition retained owned logical bytes",
        prospective_outer_retained,
        limits.max_retained_owned_logical_bytes,
    )?;
    let prospective_assembly_peak = checked_add(
        "exact WhenBad partition compilation owned logical peak upper bound",
        materialization_retained,
        admission.assembly_container_owned_logical_bytes,
    )?;
    check_limit(
        "exact WhenBad partition compilation owned logical peak upper bound",
        prospective_assembly_peak.max(prospective_outer_retained),
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    Ok(admission)
}

fn assemble_partition_problem(
    context: &ParametricCoefficientContext,
    owner: &MaterializedReadyForPartition,
    admission: PartitionAssemblyAdmission,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<PreparedPartitionAssembly, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut structural_loci = try_vec_with_exact_capacity(
        "exact WhenBad partition structural loci",
        admission.locus_inputs,
    )?;
    let mut inherited_truths = try_vec_with_exact_capacity(
        "exact WhenBad partition inherited truths",
        admission.inherited_truth_occurrences,
    )?;
    let mut inherited_truth_seeded = try_vec_with_exact_capacity(
        "exact WhenBad partition inherited truth seed map",
        admission.locus_inputs,
    )?;
    let mut atoms =
        try_vec_with_exact_capacity("exact WhenBad partition bad atoms", admission.bad_atoms)?;
    let mut clause_ranges = try_vec_with_exact_capacity(
        "exact WhenBad partition bad clause ranges",
        admission.bad_clauses,
    )?;
    let mut transcript = GeneratedAffineResidualGroupExactWhenBadPartitionTranscript {
        source_transcripts: try_vec_with_exact_capacity(
            "exact WhenBad partition source transcripts",
            admission.source_transcripts,
        )?,
        boundary_transcripts: try_vec_with_exact_capacity(
            "exact WhenBad partition boundary transcripts",
            admission.boundary_transcripts,
        )?,
        locus_origins: try_vec_with_exact_capacity(
            "exact WhenBad partition locus origins",
            admission.locus_inputs,
        )?,
        inherited_provenance: try_vec_with_exact_capacity(
            "exact WhenBad partition inherited provenance",
            admission.inherited_truth_occurrences,
        )?,
        atom_provenance: try_vec_with_exact_capacity(
            "exact WhenBad partition atom provenance",
            admission.bad_atoms,
        )?,
        clause_provenance: try_vec_with_exact_capacity(
            "exact WhenBad partition clause provenance",
            admission.bad_clauses,
        )?,
    };
    let assembly_nonproblem_owned_logical_bytes = checked_add(
        "exact WhenBad partition assembly owned logical bytes",
        owner.stats().retained_owned_logical_bytes(),
        checked_add(
            "exact WhenBad partition assembly owned logical bytes",
            admission.transcript_retained_owned_logical_bytes,
            checked_add(
                "exact WhenBad partition assembly owned logical bytes",
                size_of::<Vec<u8>>(),
                admission.locus_inputs,
            )?,
        )?,
    )?;
    let mut stats = GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        transcript_retained_owned_logical_bytes: admission.transcript_retained_owned_logical_bytes,
        source_problem_retained_owned_logical_bytes: admission
            .source_problem_container_owned_logical_bytes,
        materialization_retained_owned_logical_bytes: owner.stats().retained_owned_logical_bytes(),
        compilation_owned_logical_peak_upper_bound: checked_add(
            "exact WhenBad partition compilation owned logical peak upper bound",
            assembly_nonproblem_owned_logical_bytes,
            admission.source_problem_container_owned_logical_bytes,
        )?,
        ..GeneratedAffineResidualGroupExactWhenBadPartitionStats::default()
    };

    for (source_ordinal, source) in owner.sources().iter().enumerate() {
        let source_locator = source.source();
        let kind = match source {
            GeneratedAffineResidualGroupExactMappedSource::Condition(condition) => {
                if !matches!(
                    source_locator,
                    GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise { .. }
                        | GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard { .. }
                ) {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                let disposition = if condition.class()
                    == GeneratedAffineResidualGroupExactMappedConditionClass::IndexDependent
                {
                    match source_locator {
                        GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise { .. } => {
                            let (locus_ordinal, origin_record_ordinal) = intern_locus(
                                context,
                                condition.polynomial(),
                                GeneratedAffineResidualGroupExactWhenBadLocusOrigin::TargetPremise {
                                    source_ordinal,
                                    source: source_locator,
                                },
                                &mut structural_loci,
                                &mut transcript.locus_origins,
                                limits,
                                &mut stats,
                                assembly_nonproblem_owned_logical_bytes,
                            )?;
                            while inherited_truth_seeded.len() < structural_loci.len() {
                                inherited_truth_seeded.push(0);
                            }
                            let seeded = inherited_truth_seeded.get_mut(locus_ordinal).ok_or(
                                GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch,
                            )?;
                            if *seeded == 0 {
                                let requested = checked_add(
                                    "exact WhenBad partition inherited truths",
                                    inherited_truths.len(),
                                    1,
                                )?;
                                check_limit(
                                    "exact WhenBad partition inherited truths",
                                    requested,
                                    limits.max_inherited_truths,
                                )?;
                                inherited_truths.push(AffineWhenBadInheritedTruth::new(
                                    locus_ordinal,
                                    SymbolicPolynomialPredicateKind::NonZero,
                                ));
                                *seeded = 1;
                            }
                            transcript.inherited_provenance.push(
                                GeneratedAffineResidualGroupExactWhenBadInheritedProvenance {
                                    ordinal: transcript.inherited_provenance.len(),
                                    locus_ordinal,
                                    origin_record_ordinal,
                                },
                            );
                            GeneratedAffineResidualGroupExactWhenBadConditionPartitionDisposition::InheritedNonzero {
                                locus_ordinal,
                            }
                        }
                        GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard { .. } => {
                            let clause_ordinal = transcript.clause_provenance.len();
                            let first_atom_ordinal = atoms.len();
                            let (locus_ordinal, origin_record_ordinal) = intern_locus(
                                context,
                                condition.polynomial(),
                                GeneratedAffineResidualGroupExactWhenBadLocusOrigin::RecenteredRowGuard {
                                    source_ordinal,
                                    source: source_locator,
                                },
                                &mut structural_loci,
                                &mut transcript.locus_origins,
                                limits,
                                &mut stats,
                                assembly_nonproblem_owned_logical_bytes,
                            )?;
                            push_bad_atom(
                                locus_ordinal,
                                origin_record_ordinal,
                                clause_ordinal,
                                0,
                                &mut atoms,
                                &mut transcript.atom_provenance,
                            );
                            finish_clause(
                                first_atom_ordinal,
                                GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard {
                                    source_ordinal,
                                    source: source_locator,
                                },
                                &atoms,
                                &mut clause_ranges,
                                &mut transcript.clause_provenance,
                            )?;
                            GeneratedAffineResidualGroupExactWhenBadConditionPartitionDisposition::CandidateBadClause {
                                clause_ordinal,
                            }
                        }
                        GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient { .. }
                        | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient { .. } => {
                            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                        }
                    }
                } else {
                    GeneratedAffineResidualGroupExactWhenBadConditionPartitionDisposition::Discharged
                };
                GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind::Condition {
                    class: condition.class(),
                    disposition,
                }
            }
            GeneratedAffineResidualGroupExactMappedSource::Coefficient(coefficient) => {
                if !matches!(
                    source_locator,
                    GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient { .. }
                        | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient { .. }
                ) {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                let identities = coefficient.denominator_identities();
                if identities[0].kind()
                    != GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator
                    || identities[1].kind()
                        != GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator
                {
                    return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                }
                let mut denominator_transcripts = [
                    GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript {
                        kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator,
                        disposition: GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition::NeverIdentityZero,
                    },
                    GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript {
                        kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator,
                        disposition: GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition::NeverIdentityZero,
                    },
                ];
                for (identity_ordinal, identity) in identities.iter().enumerate() {
                    let disposition = match identity.projection().class() {
                        ParametricParameterIdentityClass::AlwaysIdentityZero => {
                            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
                        }
                        ParametricParameterIdentityClass::NeverIdentityZero { .. } => {
                            GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition::NeverIdentityZero
                        }
                        ParametricParameterIdentityClass::Conditional { coefficient_loci } => {
                            let clause_ordinal = transcript.clause_provenance.len();
                            let first_atom_ordinal = atoms.len();
                            for (coefficient_locus_ordinal, locus) in
                                coefficient_loci.iter().enumerate()
                            {
                                let (locus_ordinal, origin_record_ordinal) = intern_locus(
                                    context,
                                    locus.polynomial(),
                                    GeneratedAffineResidualGroupExactWhenBadLocusOrigin::DenominatorIdentityCoefficient {
                                        source_ordinal,
                                        source: source_locator,
                                        kind: identity.kind(),
                                        coefficient_locus_ordinal,
                                    },
                                    &mut structural_loci,
                                    &mut transcript.locus_origins,
                                    limits,
                                    &mut stats,
                                    assembly_nonproblem_owned_logical_bytes,
                                )?;
                                push_bad_atom(
                                    locus_ordinal,
                                    origin_record_ordinal,
                                    clause_ordinal,
                                    coefficient_locus_ordinal,
                                    &mut atoms,
                                    &mut transcript.atom_provenance,
                                );
                            }
                            finish_clause(
                                first_atom_ordinal,
                                GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity {
                                    source_ordinal,
                                    source: source_locator,
                                    kind: identity.kind(),
                                },
                                &atoms,
                                &mut clause_ranges,
                                &mut transcript.clause_provenance,
                            )?;
                            GeneratedAffineResidualGroupExactWhenBadDenominatorPartitionDisposition::ConditionalBadClause {
                                clause_ordinal,
                                first_atom_ordinal,
                                atom_count: atoms.len() - first_atom_ordinal,
                            }
                        }
                    };
                    let slot = denominator_transcripts.get_mut(identity_ordinal).ok_or(
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization,
                    )?;
                    *slot = GeneratedAffineResidualGroupExactWhenBadDenominatorTranscript {
                        kind: identity.kind(),
                        disposition,
                    };
                }
                GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind::Coefficient {
                    denominators: denominator_transcripts,
                }
            }
        };
        transcript.source_transcripts.push(
            GeneratedAffineResidualGroupExactWhenBadSourceTranscript {
                ordinal: source_ordinal,
                source: source_locator,
                kind,
            },
        );
    }

    for event in owner.boundaries() {
        let disposition = match event.disposition() {
            GeneratedAffineResidualGroupExactBoundaryDisposition::Empty => {
                GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition::Empty
            }
            GeneratedAffineResidualGroupExactBoundaryDisposition::SuppressedByNumerator => {
                GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition::SuppressedByNumerator
            }
            GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary => {
                let polynomial = event.boundary().ok_or(
                    GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization,
                )?;
                let clause_ordinal = transcript.clause_provenance.len();
                let first_atom_ordinal = atoms.len();
                let (locus_ordinal, origin_record_ordinal) = intern_locus(
                    context,
                    polynomial,
                    GeneratedAffineResidualGroupExactWhenBadLocusOrigin::RetainedBoundary {
                        boundary_ordinal: event.ordinal(),
                        source: event.source(),
                    },
                    &mut structural_loci,
                    &mut transcript.locus_origins,
                    limits,
                    &mut stats,
                    assembly_nonproblem_owned_logical_bytes,
                )?;
                push_bad_atom(
                    locus_ordinal,
                    origin_record_ordinal,
                    clause_ordinal,
                    0,
                    &mut atoms,
                    &mut transcript.atom_provenance,
                );
                finish_clause(
                    first_atom_ordinal,
                    GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary {
                        boundary_ordinal: event.ordinal(),
                        source: event.source(),
                    },
                    &atoms,
                    &mut clause_ranges,
                    &mut transcript.clause_provenance,
                )?;
                GeneratedAffineResidualGroupExactWhenBadBoundaryPartitionDisposition::RetainedBadClause {
                    clause_ordinal,
                }
            }
            GeneratedAffineResidualGroupExactBoundaryDisposition::WholeTarget => {
                return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization);
            }
        };
        transcript.boundary_transcripts.push(
            GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript {
                ordinal: event.ordinal(),
                source: event.source(),
                disposition,
            },
        );
    }

    if structural_loci.len() > admission.locus_inputs
        || transcript.inherited_provenance.len() != admission.inherited_truth_occurrences
        || atoms.len() != admission.bad_atoms
        || clause_ranges.len() != admission.bad_clauses
        || transcript.source_transcripts.len() != admission.source_transcripts
        || transcript.boundary_transcripts.len() != admission.boundary_transcripts
        || transcript.locus_origins.len() != admission.locus_inputs
        || transcript.atom_provenance.len() != admission.bad_atoms
        || transcript.clause_provenance.len() != admission.bad_clauses
    {
        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
    }

    stats.source_transcripts = transcript.source_transcripts.len();
    stats.boundary_transcripts = transcript.boundary_transcripts.len();
    stats.locus_inputs = transcript.locus_origins.len();
    stats.structural_loci = structural_loci.len();
    stats.inherited_truth_occurrences = transcript.inherited_provenance.len();
    stats.inherited_truths = inherited_truths.len();
    stats.bad_atoms = atoms.len();
    stats.bad_clauses = clause_ranges.len();
    stats.locus_origins = transcript.locus_origins.len();
    stats.inherited_provenances = transcript.inherited_provenance.len();
    stats.atom_provenances = transcript.atom_provenance.len();
    stats.clause_provenances = transcript.clause_provenance.len();

    Ok(PreparedPartitionAssembly {
        structural_loci,
        inherited_truths,
        atoms,
        clause_ranges,
        transcript,
        stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn intern_locus(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    origin: GeneratedAffineResidualGroupExactWhenBadLocusOrigin,
    structural_loci: &mut Vec<ParametricPolynomial>,
    origins: &mut Vec<GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord>,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    assembly_nonproblem_owned_logical_bytes: usize,
) -> Result<(usize, usize), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    context.validate_polynomial_with_limits(polynomial, limits.exact_algebra)?;
    if polynomial.is_zero()
        || !context.polynomial_depends_on_indices_with_limits(polynomial, limits.exact_algebra)?
    {
        return Err(
            GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization,
        );
    }

    let mut exact_match = None;
    for (ordinal, retained) in structural_loci.iter().enumerate() {
        let term_pairs = checked_mul(
            "exact WhenBad partition intern equality term pairs",
            polynomial.term_count(),
            retained.term_count(),
        )?;
        stats.intern_equality_comparisons = checked_bounded_add(
            "exact WhenBad partition intern equality comparisons",
            stats.intern_equality_comparisons,
            1,
            limits.max_intern_equality_comparisons,
        )?;
        stats.intern_equality_term_pairs = checked_bounded_add(
            "exact WhenBad partition intern equality term pairs",
            stats.intern_equality_term_pairs,
            term_pairs,
            limits.max_intern_equality_term_pairs,
        )?;
        if retained == polynomial {
            exact_match = Some(ordinal);
            break;
        }
    }

    let locus_ordinal = if let Some(ordinal) = exact_match {
        ordinal
    } else {
        let mut associate_match = None;
        for (ordinal, retained) in structural_loci.iter().enumerate() {
            stats.intern_associate_comparisons = checked_bounded_add(
                "exact WhenBad partition intern associate comparisons",
                stats.intern_associate_comparisons,
                1,
                limits.max_intern_associate_comparisons,
            )?;
            let mut child = limits.associate;
            child.exact_algebra = intersect_exact_limits(child.exact_algebra, limits.exact_algebra);
            let remaining_cross_pairs = remaining_limit(
                "exact WhenBad partition intern associate native cross term pairs",
                limits.max_intern_associate_native_cross_term_pairs,
                stats.intern_associate_native_cross_term_pairs,
            )?;
            child.max_native_cross_term_pairs =
                child.max_native_cross_term_pairs.min(remaining_cross_pairs);
            child.max_peak_native_cross_term_pairs = child
                .max_peak_native_cross_term_pairs
                .min(remaining_cross_pairs);
            child.max_rustred_visible_temporary_byte_envelope = child
                .max_rustred_visible_temporary_byte_envelope
                .min(limits.max_intern_associate_rustred_visible_temporary_byte_peak);
            child.max_native_workspace_byte_envelope = child
                .max_native_workspace_byte_envelope
                .min(limits.max_intern_associate_native_workspace_byte_peak);
            let configured_visible_limit = child.max_rustred_visible_temporary_byte_envelope;
            let configured_native_limit = child.max_native_workspace_byte_envelope;
            let configured_combined_limit = child.max_combined_temporary_byte_envelope;
            let assembly_live = checked_add(
                "exact WhenBad partition compilation owned logical peak upper bound",
                assembly_nonproblem_owned_logical_bytes,
                stats.source_problem_retained_owned_logical_bytes,
            )?;
            let remaining_compilation_bytes = remaining_limit(
                "exact WhenBad partition compilation owned logical peak upper bound",
                limits.max_compilation_owned_logical_peak_upper_bound,
                assembly_live,
            )?;
            let outer_global_owns_combined =
                remaining_compilation_bytes <= configured_combined_limit;
            // If the configured combined ceiling is stricter, satisfying it
            // necessarily satisfies the looser outer aggregate.  Preserve
            // that child ownership instead of letting a per-component clamp
            // fire first under `check_associate_stats`' component-first order.
            if outer_global_owns_combined {
                child.max_rustred_visible_temporary_byte_envelope = child
                    .max_rustred_visible_temporary_byte_envelope
                    .min(remaining_compilation_bytes);
                child.max_native_workspace_byte_envelope = child
                    .max_native_workspace_byte_envelope
                    .min(remaining_compilation_bytes);
            }
            child.max_combined_temporary_byte_envelope =
                configured_combined_limit.min(remaining_compilation_bytes);
            let result = match context
                .polynomial_loci_are_associates_with_census(retained, polynomial, child)
            {
                Ok(result) => result,
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate combined temporary byte envelope",
                    requested,
                    ..
                }) if outer_global_owns_combined => {
                    return Err(map_intern_associate_resource_error(
                        requested,
                        assembly_live,
                        limits.max_compilation_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate RustRed-visible temporary byte envelope",
                    requested,
                    ..
                }) if outer_global_owns_combined
                    && remaining_compilation_bytes <= configured_visible_limit =>
                {
                    return Err(map_intern_associate_resource_error(
                        requested,
                        assembly_live,
                        limits.max_compilation_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate native workspace byte envelope",
                    requested,
                    ..
                }) if outer_global_owns_combined
                    && remaining_compilation_bytes <= configured_native_limit =>
                {
                    return Err(map_intern_associate_resource_error(
                        requested,
                        assembly_live,
                        limits.max_compilation_owned_logical_peak_upper_bound,
                    ));
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate combined temporary byte envelope",
                }) if outer_global_owns_combined => {
                    return Err(
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                            resource: "exact WhenBad partition compilation owned logical peak upper bound",
                        },
                    );
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate RustRed-visible temporary byte envelope",
                }) if outer_global_owns_combined
                    && remaining_compilation_bytes <= configured_visible_limit =>
                {
                    return Err(
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                            resource: "exact WhenBad partition compilation owned logical peak upper bound",
                        },
                    );
                }
                Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate native workspace byte envelope",
                }) if outer_global_owns_combined
                    && remaining_compilation_bytes <= configured_native_limit =>
                {
                    return Err(
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                            resource: "exact WhenBad partition compilation owned logical peak upper bound",
                        },
                    );
                }
                Err(error) => return Err(error.into()),
            };
            let child_stats = result.stats();
            stats.intern_associate_native_cross_term_pairs = checked_bounded_add(
                "exact WhenBad partition intern associate native cross term pairs",
                stats.intern_associate_native_cross_term_pairs,
                child_stats.native_cross_term_pairs(),
                limits.max_intern_associate_native_cross_term_pairs,
            )?;
            stats.intern_associate_rustred_visible_temporary_byte_peak = stats
                .intern_associate_rustred_visible_temporary_byte_peak
                .max(child_stats.rustred_visible_temporary_byte_envelope());
            stats.intern_associate_native_workspace_byte_peak = stats
                .intern_associate_native_workspace_byte_peak
                .max(child_stats.native_workspace_byte_envelope());
            // Rust-visible projections overlap native product work, so the
            // APIs' separate conservative envelopes cannot soundly be joined
            // by `max`. Their checked sum is conservative (not necessarily
            // tight), and the child preflights the same sum before native
            // allocation through its combined limit.
            let associate_scratch = checked_add(
                "exact WhenBad partition intern associate combined temporary byte envelope",
                child_stats.rustred_visible_temporary_byte_envelope(),
                child_stats.native_workspace_byte_envelope(),
            )?;
            let associate_peak = checked_add(
                "exact WhenBad partition compilation owned logical peak upper bound",
                assembly_live,
                associate_scratch,
            )?;
            check_limit(
                "exact WhenBad partition compilation owned logical peak upper bound",
                associate_peak,
                limits.max_compilation_owned_logical_peak_upper_bound,
            )?;
            stats.compilation_owned_logical_peak_upper_bound = stats
                .compilation_owned_logical_peak_upper_bound
                .max(associate_peak);
            if result.associated() {
                associate_match = Some(ordinal);
                break;
            }
        }
        if let Some(ordinal) = associate_match {
            ordinal
        } else {
            let requested = checked_add(
                "exact WhenBad partition structural loci",
                structural_loci.len(),
                1,
            )?;
            check_limit(
                "exact WhenBad partition structural loci",
                requested,
                limits.max_structural_loci,
            )?;
            let polynomial_terms = polynomial.term_count();
            let polynomial_exponents = polynomial.raw().exponents.len();
            let polynomial_bits = polynomial_integer_bits(polynomial)?;
            let polynomial_owned = polynomial.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                    resource: "exact WhenBad partition source problem retained owned logical bytes",
                },
            )?;
            let prospective_source_problem = checked_add(
                "exact WhenBad partition source problem retained owned logical bytes",
                stats.source_problem_retained_owned_logical_bytes,
                polynomial_owned,
            )?;
            check_limit(
                "exact WhenBad partition source problem retained owned logical bytes",
                prospective_source_problem,
                limits.max_source_problem_retained_owned_logical_bytes,
            )?;
            let prospective_copy_peak = checked_add(
                "exact WhenBad partition compilation owned logical peak upper bound",
                assembly_nonproblem_owned_logical_bytes,
                prospective_source_problem,
            )?;
            check_limit(
                "exact WhenBad partition compilation owned logical peak upper bound",
                prospective_copy_peak,
                limits.max_compilation_owned_logical_peak_upper_bound,
            )?;
            stats.retained_polynomial_terms = checked_bounded_add(
                "exact WhenBad partition retained polynomial terms",
                stats.retained_polynomial_terms,
                polynomial_terms,
                limits.max_retained_polynomial_terms,
            )?;
            stats.retained_polynomial_exponent_entries = checked_bounded_add(
                "exact WhenBad partition retained polynomial exponent entries",
                stats.retained_polynomial_exponent_entries,
                polynomial_exponents,
                limits.max_retained_polynomial_exponent_entries,
            )?;
            stats.retained_polynomial_integer_bits = checked_bounded_add(
                "exact WhenBad partition retained polynomial integer bits",
                stats.retained_polynomial_integer_bits,
                polynomial_bits,
                limits.max_retained_polynomial_integer_bits,
            )?;
            let copied =
                polynomial
                    .try_copy_authenticated_sparse_payload()
                    .map_err(|resource| {
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
                            resource,
                            requested: polynomial_terms.max(polynomial_exponents),
                        }
                    })?;
            structural_loci.push(copied);
            stats.source_problem_retained_owned_logical_bytes = prospective_source_problem;
            stats.compilation_owned_logical_peak_upper_bound = stats
                .compilation_owned_logical_peak_upper_bound
                .max(prospective_copy_peak);
            requested - 1
        }
    };

    let origin_record_ordinal = origins.len();
    origins.push(GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord {
        ordinal: origin_record_ordinal,
        locus_ordinal,
        origin,
    });
    Ok((locus_ordinal, origin_record_ordinal))
}

fn map_intern_associate_resource_error(
    child_requested: usize,
    assembly_live: usize,
    compilation_limit: usize,
) -> GeneratedAffineResidualGroupExactWhenBadPartitionError {
    let resource = "exact WhenBad partition compilation owned logical peak upper bound";
    let Some(requested) = assembly_live.checked_add(child_requested) else {
        return GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
            resource,
        };
    };
    GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
        resource,
        requested,
        limit: compilation_limit,
    }
}

fn push_bad_atom(
    locus_ordinal: usize,
    origin_record_ordinal: usize,
    clause_ordinal: usize,
    clause_atom_ordinal: usize,
    atoms: &mut Vec<AffineWhenBadAtom>,
    provenance: &mut Vec<GeneratedAffineResidualGroupExactWhenBadAtomProvenance>,
) {
    let atom_ordinal = atoms.len();
    atoms.push(AffineWhenBadAtom::new(
        locus_ordinal,
        SymbolicPolynomialPredicateKind::EqualZero,
    ));
    provenance.push(GeneratedAffineResidualGroupExactWhenBadAtomProvenance {
        atom_ordinal,
        clause_ordinal,
        clause_atom_ordinal,
        locus_ordinal,
        kind: SymbolicPolynomialPredicateKind::EqualZero,
        origin_record_ordinal,
    });
}

fn finish_clause(
    first_atom_ordinal: usize,
    source: GeneratedAffineResidualGroupExactWhenBadClauseSource,
    atoms: &[AffineWhenBadAtom],
    ranges: &mut Vec<Range<usize>>,
    provenance: &mut Vec<GeneratedAffineResidualGroupExactWhenBadClauseProvenance>,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let atom_count = atoms
        .len()
        .checked_sub(first_atom_ordinal)
        .ok_or(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)?;
    if atom_count == 0 {
        return Err(
            GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization,
        );
    }
    let ordinal = ranges.len();
    if provenance.len() != ordinal {
        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
    }
    ranges.push(first_atom_ordinal..atoms.len());
    provenance.push(GeneratedAffineResidualGroupExactWhenBadClauseProvenance {
        ordinal,
        first_atom_ordinal,
        atom_count,
        source,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_partition_terminal(
    schema: &'static str,
    owner: &MaterializedReadyForPartition,
    transcript: &GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
    partition: &AffineWhenBadArbitraryRelativePartitionCertificate,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    expect_applicable: bool,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    catch_unwind(AssertUnwindSafe(|| {
        if schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA {
            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::SchemaMismatch);
        }
        owner.replay(family, context, session)?;
        let rebuilt = prepare_ready_partition(context, owner, limits)?;
        let PreparedPartitionTerminal::Partitioned {
            transcript: rebuilt_transcript,
            partition: rebuilt_partition,
            stats: rebuilt_stats,
            applicable,
        } = rebuilt
        else {
            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
        };
        if applicable != expect_applicable
            || transcript != &rebuilt_transcript
            || stats != rebuilt_stats
            || !arbitrary_partition_payload_eq(partition, &rebuilt_partition)
        {
            return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
        }
        Ok(())
    }))
    .map_err(|_| GeneratedAffineResidualGroupExactWhenBadPartitionError::SymbolicaPanic)?
}

fn arbitrary_partition_payload_eq(
    left: &AffineWhenBadArbitraryRelativePartitionCertificate,
    right: &AffineWhenBadArbitraryRelativePartitionCertificate,
) -> bool {
    left.schema() == right.schema()
        && left.context_fingerprint() == right.context_fingerprint()
        && left.structural_loci() == right.structural_loci()
        && left.inherited_truths() == right.inherited_truths()
        && left.atoms() == right.atoms()
        && left.clause_count() == right.clause_count()
        && (0..left.clause_count())
            .all(|ordinal| left.clause_range(ordinal) == right.clause_range(ordinal))
        && left.splits() == right.splits()
        && left.cases() == right.cases()
        && left.classifications() == right.classifications()
        && left.limits() == right.limits()
        && left.stats() == right.stats()
        && left.compilation_stats() == right.compilation_stats()
}

fn authenticate_final_stats(
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    for (resource, requested, limit) in [
        (
            "exact WhenBad partition source transcripts",
            stats.source_transcripts,
            limits.max_source_transcripts,
        ),
        (
            "exact WhenBad partition boundary transcripts",
            stats.boundary_transcripts,
            limits.max_boundary_transcripts,
        ),
        (
            "exact WhenBad partition locus inputs",
            stats.locus_inputs,
            limits.max_locus_inputs,
        ),
        (
            "exact WhenBad partition structural loci",
            stats.structural_loci,
            limits.max_structural_loci,
        ),
        (
            "exact WhenBad partition inherited truth occurrences",
            stats.inherited_truth_occurrences,
            limits.max_inherited_truth_occurrences,
        ),
        (
            "exact WhenBad partition inherited truths",
            stats.inherited_truths,
            limits.max_inherited_truths,
        ),
        (
            "exact WhenBad partition bad atoms",
            stats.bad_atoms,
            limits.max_bad_atoms,
        ),
        (
            "exact WhenBad partition bad clauses",
            stats.bad_clauses,
            limits.max_bad_clauses,
        ),
        (
            "exact WhenBad partition locus origins",
            stats.locus_origins,
            limits.max_locus_origins,
        ),
        (
            "exact WhenBad partition inherited provenances",
            stats.inherited_provenances,
            limits.max_inherited_provenances,
        ),
        (
            "exact WhenBad partition atom provenances",
            stats.atom_provenances,
            limits.max_atom_provenances,
        ),
        (
            "exact WhenBad partition clause provenances",
            stats.clause_provenances,
            limits.max_clause_provenances,
        ),
        (
            "exact WhenBad partition intern equality comparisons",
            stats.intern_equality_comparisons,
            limits.max_intern_equality_comparisons,
        ),
        (
            "exact WhenBad partition intern equality term pairs",
            stats.intern_equality_term_pairs,
            limits.max_intern_equality_term_pairs,
        ),
        (
            "exact WhenBad partition intern associate comparisons",
            stats.intern_associate_comparisons,
            limits.max_intern_associate_comparisons,
        ),
        (
            "exact WhenBad partition intern associate native cross term pairs",
            stats.intern_associate_native_cross_term_pairs,
            limits.max_intern_associate_native_cross_term_pairs,
        ),
        (
            "exact WhenBad partition intern associate RustRed-visible temporary byte peak",
            stats.intern_associate_rustred_visible_temporary_byte_peak,
            limits.max_intern_associate_rustred_visible_temporary_byte_peak,
        ),
        (
            "exact WhenBad partition intern associate native workspace byte peak",
            stats.intern_associate_native_workspace_byte_peak,
            limits.max_intern_associate_native_workspace_byte_peak,
        ),
        (
            "exact WhenBad partition retained polynomial terms",
            stats.retained_polynomial_terms,
            limits.max_retained_polynomial_terms,
        ),
        (
            "exact WhenBad partition retained polynomial exponent entries",
            stats.retained_polynomial_exponent_entries,
            limits.max_retained_polynomial_exponent_entries,
        ),
        (
            "exact WhenBad partition retained polynomial integer bits",
            stats.retained_polynomial_integer_bits,
            limits.max_retained_polynomial_integer_bits,
        ),
        (
            "exact WhenBad partition transcript retained owned logical bytes",
            stats.transcript_retained_owned_logical_bytes,
            limits.max_transcript_retained_owned_logical_bytes,
        ),
        (
            "exact WhenBad partition source problem retained owned logical bytes",
            stats.source_problem_retained_owned_logical_bytes,
            limits.max_source_problem_retained_owned_logical_bytes,
        ),
        (
            "exact WhenBad partition formula retained owned logical bytes",
            stats.partition_formula_retained_owned_logical_bytes,
            limits.max_partition_formula_retained_owned_logical_bytes,
        ),
        (
            "exact WhenBad partition formula compilation owned logical peak upper bound",
            stats.partition_formula_compilation_owned_logical_peak_upper_bound,
            limits.max_partition_formula_compilation_owned_logical_peak_upper_bound,
        ),
        (
            "exact WhenBad partition work owned logical peak upper bound",
            stats.partition_work_owned_logical_peak_upper_bound,
            limits.max_partition_work_owned_logical_peak_upper_bound,
        ),
        (
            "exact WhenBad partition compiler owned logical peak upper bound",
            stats.partition_compiler_owned_logical_peak_upper_bound,
            limits.max_partition_compiler_owned_logical_peak_upper_bound,
        ),
        (
            "exact WhenBad partition retained owned logical bytes",
            stats.retained_owned_logical_bytes,
            limits.max_retained_owned_logical_bytes,
        ),
        (
            "exact WhenBad partition compilation owned logical peak upper bound",
            stats.compilation_owned_logical_peak_upper_bound,
            limits.max_compilation_owned_logical_peak_upper_bound,
        ),
        (
            "exact WhenBad partition payload comparison units",
            stats.payload_comparison_units,
            limits.max_payload_comparison_units,
        ),
        (
            "exact WhenBad partition payload comparison integer bits",
            stats.payload_comparison_integer_bits,
            limits.max_payload_comparison_integer_bits,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn transcript_retained_bytes(
    transcript: &GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactWhenBadPartitionTranscript>();
    for (capacity, element_size) in [
        (
            transcript.source_transcripts.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadSourceTranscript>(),
        ),
        (
            transcript.boundary_transcripts.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript>(),
        ),
        (
            transcript.locus_origins.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord>(),
        ),
        (
            transcript.inherited_provenance.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadInheritedProvenance>(),
        ),
        (
            transcript.atom_provenance.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadAtomProvenance>(),
        ),
        (
            transcript.clause_provenance.capacity(),
            size_of::<GeneratedAffineResidualGroupExactWhenBadClauseProvenance>(),
        ),
    ] {
        bytes = checked_add(
            "exact WhenBad partition transcript retained owned logical bytes",
            bytes,
            checked_mul(
                "exact WhenBad partition transcript retained owned logical bytes",
                capacity,
                element_size,
            )?,
        )?;
    }
    Ok(bytes)
}

fn partitioned_terminal_header_byte_bound() -> usize {
    size_of::<GeneratedAffineResidualGroupExactWhenBadReadyForPublication>().max(size_of::<
        GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad,
    >())
}

fn admitted_transcript_retained_bytes(
    admission: PartitionAssemblyAdmission,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactWhenBadPartitionTranscript>();
    for (count, element_size) in [
        (
            admission.source_transcripts,
            size_of::<GeneratedAffineResidualGroupExactWhenBadSourceTranscript>(),
        ),
        (
            admission.boundary_transcripts,
            size_of::<GeneratedAffineResidualGroupExactWhenBadBoundaryTranscript>(),
        ),
        (
            admission.locus_inputs,
            size_of::<GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord>(),
        ),
        (
            admission.inherited_truth_occurrences,
            size_of::<GeneratedAffineResidualGroupExactWhenBadInheritedProvenance>(),
        ),
        (
            admission.bad_atoms,
            size_of::<GeneratedAffineResidualGroupExactWhenBadAtomProvenance>(),
        ),
        (
            admission.bad_clauses,
            size_of::<GeneratedAffineResidualGroupExactWhenBadClauseProvenance>(),
        ),
    ] {
        bytes = checked_add(
            "exact WhenBad partition transcript retained owned logical bytes",
            bytes,
            checked_mul(
                "exact WhenBad partition transcript retained owned logical bytes",
                count,
                element_size,
            )?,
        )?;
    }
    Ok(bytes)
}

fn admitted_source_problem_container_bytes(
    admission: PartitionAssemblyAdmission,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativeProblem>();
    for (count, element_size) in [
        (admission.locus_inputs, size_of::<ParametricPolynomial>()),
        (
            admission.inherited_truth_occurrences,
            size_of::<AffineWhenBadInheritedTruth>(),
        ),
        (admission.bad_atoms, size_of::<AffineWhenBadAtom>()),
        (admission.bad_clauses, size_of::<Range<usize>>()),
    ] {
        bytes = checked_add(
            "exact WhenBad partition source problem retained owned logical bytes",
            bytes,
            checked_mul(
                "exact WhenBad partition source problem retained owned logical bytes",
                count,
                element_size,
            )?,
        )?;
    }
    Ok(bytes)
}

fn admitted_assembly_container_bytes(
    admission: PartitionAssemblyAdmission,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    checked_add(
        "exact WhenBad partition assembly container owned logical bytes",
        checked_add(
            "exact WhenBad partition assembly container owned logical bytes",
            admission.transcript_retained_owned_logical_bytes,
            admission.source_problem_container_owned_logical_bytes,
        )?,
        checked_add(
            "exact WhenBad partition assembly container owned logical bytes",
            size_of::<Vec<u8>>(),
            admission.locus_inputs,
        )?,
    )
}

fn transcript_payload_units(
    transcript: &GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    [
        transcript.source_transcripts.len(),
        transcript.boundary_transcripts.len(),
        transcript.locus_origins.len(),
        transcript.inherited_provenance.len(),
        transcript.atom_provenance.len(),
        transcript.clause_provenance.len(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add(
            "exact WhenBad partition payload comparison units",
            sum,
            value,
        )
    })
}

fn polynomial_integer_bits(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    polynomial
        .raw()
        .coefficients
        .iter()
        .try_fold(0usize, |sum, value| {
            checked_add(
                "exact WhenBad partition retained polynomial integer bits",
                sum,
                integer_magnitude_bits(value)?,
            )
        })
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
            resource: "exact WhenBad partition retained polynomial integer bits",
        }
    })
}

fn intersect_exact_limits(
    left: ExactAlgebraLimits,
    right: ExactAlgebraLimits,
) -> ExactAlgebraLimits {
    ExactAlgebraLimits {
        max_exponent: left.max_exponent.min(right.max_exponent),
        max_polynomial_terms: left.max_polynomial_terms.min(right.max_polynomial_terms),
        max_term_operations: left.max_term_operations.min(right.max_term_operations),
    }
}

fn try_vec_with_exact_capacity<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut values = Vec::new();
    mark_partition_reserve_observed_for_test();
    values.try_reserve_exact(requested).map_err(|_| {
        GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    if size_of::<T>() != 0 && values.capacity() != requested {
        return Err(
            GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
                resource,
                requested,
            },
        );
    }
    Ok(values)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow { resource },
    )
}

fn checked_bounded_add(
    resource: &'static str,
    current: usize,
    addition: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let requested = checked_add(resource, current, addition)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    limit.checked_sub(consumed).ok_or(
        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_affine_residual_group_exact_session::tests::{
        ExactConditionPlanTestFixture, exact_condition_plan_test_fixture_in_sector,
    };
    use crate::generated_affine_residual_group_exact_when_bad_conditions::{
        GeneratedAffineResidualGroupExactConditionPlanCompiler,
        GeneratedAffineResidualGroupExactConditionPlanLimits,
    };
    use crate::generated_affine_residual_group_exact_when_bad_materialization::{
        GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler,
        GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    };

    fn materialization(
        name: &str,
        sector_bits: &str,
        compact: bool,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedAffineResidualGroupExactSession,
        GeneratedAffineResidualGroupExactWhenBadMaterialization,
    ) {
        let ExactConditionPlanTestFixture {
            family,
            context,
            session,
            ready,
        } = exact_condition_plan_test_fixture_in_sector(name, sector_bits, compact);
        let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family,
            &context,
            &session,
            ready,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        let owner = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
            &family,
            &context,
            &session,
            plan,
            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
        )
        .unwrap();
        (family, context, session, owner)
    }

    fn compile_sector_011(
        name: &str,
        limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    ) -> Result<
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
        ),
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadPartitionFailure,
        ),
    > {
        let (family, context, session, owner) = materialization(name, "011", false);
        match GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
            &family, &context, &session, owner, limits,
        ) {
            Ok(compilation) => Ok((family, context, session, compilation)),
            Err(failure) => Err((family, context, session, failure)),
        }
    }

    fn compile_sector_011_error(
        name: &str,
        limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionError {
        let (family, context, session, failure) = compile_sector_011(name, limits).unwrap_err();
        let (error, owner) = failure.into_parts();
        owner.replay(&family, &context, &session).unwrap();
        error
    }

    fn associate_intern_fixture(
        name: &str,
    ) -> (
        ParametricCoefficientContext,
        ParametricPolynomial,
        ParametricPolynomial,
    ) {
        let context = ParametricCoefficientContext::try_new(
            &crate::CoefficientContext::new(["theta"]),
            name,
            2,
        )
        .unwrap();
        let left_value = context
            .add(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let right_value = context.mul(&context.integer(2), &left_value).unwrap();
        let left = context.numerator_condition(&left_value).unwrap();
        let right = context.numerator_condition(&right_value).unwrap();
        assert_ne!(left, right);
        (context, left, right)
    }

    fn run_associate_intern(
        context: &ParametricCoefficientContext,
        retained: &ParametricPolynomial,
        candidate: &ParametricPolynomial,
        limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
        assembly_nonproblem_owned_logical_bytes: usize,
    ) -> Result<
        GeneratedAffineResidualGroupExactWhenBadPartitionStats,
        GeneratedAffineResidualGroupExactWhenBadPartitionError,
    > {
        let mut structural_loci = vec![retained.clone()];
        let mut origins = Vec::with_capacity(1);
        let mut stats = GeneratedAffineResidualGroupExactWhenBadPartitionStats::default();
        stats.source_problem_retained_owned_logical_bytes =
            retained.owned_retained_byte_bound().unwrap();
        let (locus_ordinal, origin_ordinal) = intern_locus(
            context,
            candidate,
            GeneratedAffineResidualGroupExactWhenBadLocusOrigin::TargetPremise {
                source_ordinal: 0,
                source: GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise {
                    premise_ordinal: 0,
                },
            },
            &mut structural_loci,
            &mut origins,
            limits,
            &mut stats,
            assembly_nonproblem_owned_logical_bytes,
        )?;
        assert_eq!((locus_ordinal, origin_ordinal), (0, 0));
        assert_eq!(structural_loci.len(), 1);
        assert_eq!(origins.len(), 1);
        Ok(stats)
    }

    #[test]
    fn sector_011_materialization_builds_generic_formula_partition_and_replays() {
        let (family, context, session, compilation) = compile_sector_011(
            "exact-when-bad-partition-sector-011",
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        compilation.replay(&family, &context, &session).unwrap();
        assert_eq!(compilation.targets_consumed(), 0);
        assert!(!compilation.publishes_rule());

        let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(
            ready,
        ) = &compilation
        else {
            panic!("sector-011 retained formula unexpectedly has no applicable leaf");
        };
        assert_eq!(
            ready.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA
        );
        assert_eq!(ready.source_transcripts().len(), 7);
        assert_eq!(ready.boundary_transcripts().len(), 5);
        assert_eq!(
            ready.partition().clause_count(),
            ready.clause_provenance().len()
        );
        assert_eq!(
            ready.partition().atoms().len(),
            ready.atom_provenance().len()
        );
        assert_eq!(ready.locus_origins().len(), ready.stats().locus_inputs());
        assert_eq!(ready.stats().inherited_truth_occurrences(), 0);
        assert_eq!(ready.stats().inherited_truths(), 0);
        assert!(ready.stats().applicable_leaves() > 0);
        assert!(ready.stats().bad_atoms() >= 8);
        assert!(ready.stats().bad_clauses() >= 8);
        assert!(ready.stats().structural_loci() <= ready.stats().locus_inputs());
        assert!(
            ready.stats().compilation_owned_logical_peak_upper_bound()
                >= ready.stats().retained_owned_logical_bytes()
        );

        let mut coefficient_sources = 0usize;
        for source in ready.source_transcripts() {
            if let GeneratedAffineResidualGroupExactWhenBadSourceTranscriptKind::Coefficient {
                denominators,
            } = source.kind()
            {
                coefficient_sources += 1;
                assert_eq!(
                    denominators[0].kind,
                    GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator,
                );
                assert_eq!(
                    denominators[1].kind,
                    GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator,
                );
            }
        }
        assert_eq!(coefficient_sources, 5);
        for clause in ready.clause_provenance() {
            assert_eq!(
                ready.partition().clause_range(clause.ordinal()),
                Some(
                    clause.first_atom_ordinal()..clause.first_atom_ordinal() + clause.atom_count()
                ),
            );
            for (local, provenance) in ready.atom_provenance()
                [clause.first_atom_ordinal()..clause.first_atom_ordinal() + clause.atom_count()]
                .iter()
                .enumerate()
            {
                assert_eq!(provenance.clause_ordinal, clause.ordinal());
                assert_eq!(provenance.clause_atom_ordinal(), local);
            }
        }
    }

    #[test]
    fn pre_reserve_limit_and_panic_return_replayable_owner_for_retry() {
        let fixture_name = "exact-when-bad-partition-owner-recovery";
        let (family, context, session, owner) = materialization(fixture_name, "011", false);
        reset_partition_reserve_observed_for_test();
        let mut limits = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        limits.max_transcript_retained_owned_logical_bytes = 0;
        let failure = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
            &family, &context, &session, owner, limits,
        )
        .unwrap_err();
        assert!(!partition_reserve_was_observed_for_test());
        let (error, owner) = failure.into_parts();
        assert!(matches!(
            error,
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource: "exact WhenBad partition transcript retained owned logical bytes",
                ..
            }
        ));
        owner.replay(&family, &context, &session).unwrap();

        inject_partition_prepare_panic_for_test();
        let failure = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
            &family,
            &context,
            &session,
            owner,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &GeneratedAffineResidualGroupExactWhenBadPartitionError::SymbolicaPanic
        );
        let (_, owner) = failure.into_parts();
        owner.replay(&family, &context, &session).unwrap();
        let compilation = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
            &family,
            &context,
            &session,
            owner,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        compilation.replay(&family, &context, &session).unwrap();
    }

    type LimitSetter = fn(&mut GeneratedAffineResidualGroupExactWhenBadPartitionLimits, usize);
    type StatGetter = fn(GeneratedAffineResidualGroupExactWhenBadPartitionStats) -> usize;

    #[test]
    fn exact_and_one_below_outer_memory_limits_are_transactional() {
        let fixture_name = "exact-when-bad-partition-memory-limits";
        let (_, _, _, baseline) = compile_sector_011(
            fixture_name,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        let stats = baseline.stats();
        let probes: &[(&str, StatGetter, LimitSetter)] = &[
            (
                "transcript retained",
                |stats| stats.transcript_retained_owned_logical_bytes(),
                |limits, value| limits.max_transcript_retained_owned_logical_bytes = value,
            ),
            (
                "source problem retained",
                |stats| stats.source_problem_retained_owned_logical_bytes(),
                |limits, value| limits.max_source_problem_retained_owned_logical_bytes = value,
            ),
            (
                "associate RustRed-visible temporary peak",
                |stats| stats.intern_associate_rustred_visible_temporary_byte_peak(),
                |limits, value| {
                    limits.max_intern_associate_rustred_visible_temporary_byte_peak = value
                },
            ),
            (
                "associate native workspace peak",
                |stats| stats.intern_associate_native_workspace_byte_peak(),
                |limits, value| limits.max_intern_associate_native_workspace_byte_peak = value,
            ),
            (
                "formula retained",
                |stats| stats.partition_formula_retained_owned_logical_bytes(),
                |limits, value| limits.max_partition_formula_retained_owned_logical_bytes = value,
            ),
            (
                "formula compilation peak",
                |stats| stats.partition_formula_compilation_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_formula_compilation_owned_logical_peak_upper_bound = value
                },
            ),
            (
                "partition work peak",
                |stats| stats.partition_work_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_partition_work_owned_logical_peak_upper_bound = value,
            ),
            (
                "partition compiler peak",
                |stats| stats.partition_compiler_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_compiler_owned_logical_peak_upper_bound = value
                },
            ),
            (
                "retained",
                |stats| stats.retained_owned_logical_bytes(),
                |limits, value| limits.max_retained_owned_logical_bytes = value,
            ),
            (
                "compilation peak",
                |stats| stats.compilation_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_compilation_owned_logical_peak_upper_bound = value,
            ),
        ];

        for (name, getter, setter) in probes {
            let observed = getter(stats);
            assert!(observed > 0, "{name} probe is vacuous");
            let (family, context, session, owner) = materialization(fixture_name, "011", false);
            let mut one_below = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            setter(&mut one_below, observed - 1);
            let failure = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
                &family, &context, &session, owner, one_below,
            )
            .unwrap_err();
            if *name == "compilation peak" {
                assert!(
                    matches!(
                        failure.error(),
                        GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                            resource: "exact WhenBad partition compilation owned logical peak upper bound",
                            ..
                        }
                    ),
                    "unexpected compilation-peak error: {:?}",
                    failure.error()
                );
            }
            let (_, owner) = failure.into_parts();
            owner.replay(&family, &context, &session).unwrap();

            let mut exact = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            setter(&mut exact, observed);
            let compilation = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
                &family, &context, &session, owner, exact,
            )
            .unwrap_or_else(|failure| panic!("exact {name} failed: {failure}"));
            assert_eq!(getter(compilation.stats()), observed, "{name}");
            compilation.replay(&family, &context, &session).unwrap();
        }
    }

    #[test]
    fn associate_component_and_combined_limits_preserve_typed_ownership() {
        let (context, retained, candidate) =
            associate_intern_fixture("exact-when-bad-partition-associate-resource-mapping");
        let assembly_nonproblem = 257usize;
        let source_problem = retained.owned_retained_byte_bound().unwrap();
        let assembly_live = assembly_nonproblem.checked_add(source_problem).unwrap();
        let baseline = run_associate_intern(
            &context,
            &retained,
            &candidate,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
            assembly_nonproblem,
        )
        .unwrap();
        let visible = baseline.intern_associate_rustred_visible_temporary_byte_peak();
        let native = baseline.intern_associate_native_workspace_byte_peak();
        let combined = visible.checked_add(native).unwrap();
        assert!(visible > 0 && native > 0);
        assert_eq!(
            baseline.compilation_owned_logical_peak_upper_bound(),
            assembly_live.checked_add(combined).unwrap(),
        );

        // A deep outer-global clamp is allowed to trip either component first;
        // whichever exact child preflight wins must still report the outer
        // aggregate resource, and its lower-bound request must exceed it.
        let deep_remaining = visible.min(native) - 1;
        let mut deep_global = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        deep_global.max_compilation_owned_logical_peak_upper_bound =
            assembly_live.checked_add(deep_remaining).unwrap();
        assert!(matches!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                deep_global,
                assembly_nonproblem,
            ),
            Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource: "exact WhenBad partition compilation owned logical peak upper bound",
                requested,
                limit,
            }) if requested > limit
        ));

        // At the exact aggregate boundary the combined child envelope passes;
        // one below is owned by, and therefore named as, the outer limit.
        let mut exact_global = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        exact_global.max_compilation_owned_logical_peak_upper_bound =
            assembly_live.checked_add(combined).unwrap();
        run_associate_intern(
            &context,
            &retained,
            &candidate,
            exact_global,
            assembly_nonproblem,
        )
        .unwrap();
        let mut one_below_global = exact_global;
        one_below_global.max_compilation_owned_logical_peak_upper_bound -= 1;
        assert!(matches!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                one_below_global,
                assembly_nonproblem,
            ),
            Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource: "exact WhenBad partition compilation owned logical peak upper bound",
                requested,
                limit,
            }) if requested > limit
        ));

        let mut exact_visible = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        exact_visible.max_intern_associate_rustred_visible_temporary_byte_peak = visible;
        run_associate_intern(
            &context,
            &retained,
            &candidate,
            exact_visible,
            assembly_nonproblem,
        )
        .unwrap();
        let mut one_below_visible = exact_visible;
        one_below_visible.max_intern_associate_rustred_visible_temporary_byte_peak -= 1;
        assert_eq!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                one_below_visible,
                assembly_nonproblem,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate RustRed-visible temporary byte envelope",
                    requested: visible,
                    limit: visible - 1,
                },
            ),
        );

        let mut exact_native = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        exact_native.max_intern_associate_native_workspace_byte_peak = native;
        run_associate_intern(
            &context,
            &retained,
            &candidate,
            exact_native,
            assembly_nonproblem,
        )
        .unwrap();
        let mut one_below_native = exact_native;
        one_below_native.max_intern_associate_native_workspace_byte_peak -= 1;
        assert_eq!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                one_below_native,
                assembly_nonproblem,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate native workspace byte envelope",
                    requested: native,
                    limit: native - 1,
                },
            ),
        );

        let mut exact_combined = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        exact_combined
            .associate
            .max_combined_temporary_byte_envelope = combined;
        run_associate_intern(
            &context,
            &retained,
            &candidate,
            exact_combined,
            assembly_nonproblem,
        )
        .unwrap();
        let mut one_below_combined = exact_combined;
        one_below_combined
            .associate
            .max_combined_temporary_byte_envelope -= 1;
        assert_eq!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                one_below_combined,
                assembly_nonproblem,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate combined temporary byte envelope",
                    requested: combined,
                    limit: combined - 1,
                },
            ),
        );

        assert!(matches!(
            map_intern_associate_resource_error(usize::MAX, 1, usize::MAX,),
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                resource: "exact WhenBad partition compilation owned logical peak upper bound",
            }
        ));
    }

    #[test]
    fn associate_colliding_component_and_global_limits_keep_the_stricter_owner() {
        let (context, retained, candidate) =
            associate_intern_fixture("exact-when-bad-partition-associate-collisions");
        let assembly_nonproblem = 257usize;
        let source_problem = retained.owned_retained_byte_bound().unwrap();
        let assembly_live = assembly_nonproblem.checked_add(source_problem).unwrap();
        let baseline = run_associate_intern(
            &context,
            &retained,
            &candidate,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
            assembly_nonproblem,
        )
        .unwrap();
        let visible = baseline.intern_associate_rustred_visible_temporary_byte_peak();
        let native = baseline.intern_associate_native_workspace_byte_peak();
        let combined = visible.checked_add(native).unwrap();
        assert!(visible > 2 && native > 2 && combined > 2);

        for (component_resource, configure, component_requested) in [
            (
                "polynomial-associate RustRed-visible temporary byte envelope",
                0usize,
                visible,
            ),
            (
                "polynomial-associate native workspace byte envelope",
                1usize,
                native,
            ),
        ] {
            // Both ceilings would reject the first phase with positive
            // scratch.  The component check is earlier within that same
            // child preflight and therefore remains child-owned.
            let component_limit = 0;
            let mut component_owned =
                GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            if configure == 0 {
                component_owned.max_intern_associate_rustred_visible_temporary_byte_peak =
                    component_limit;
            } else {
                component_owned.max_intern_associate_native_workspace_byte_peak = component_limit;
            }
            component_owned.max_compilation_owned_logical_peak_upper_bound =
                assembly_live.checked_add(combined - 1).unwrap();
            assert!(matches!(
                run_associate_intern(
                    &context,
                    &retained,
                    &candidate,
                    component_owned,
                    assembly_nonproblem,
                )
                .unwrap_err(),
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ParametricCoefficient(
                    ParametricCoefficientError::ResourceLimit {
                        resource: actual,
                        requested,
                        limit: 0,
                    },
                ) if actual == component_resource && requested > 0
            ));

            // Conversely, a deep aggregate clamp rejects an earlier phase
            // before the later near-exact component ceiling is reached.
            let mut global_owned =
                GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            if configure == 0 {
                global_owned.max_intern_associate_rustred_visible_temporary_byte_peak =
                    component_requested;
            } else {
                global_owned.max_intern_associate_native_workspace_byte_peak = component_requested;
            }
            global_owned.max_compilation_owned_logical_peak_upper_bound =
                assembly_live.checked_add(1).unwrap();
            assert!(matches!(
                run_associate_intern(
                    &context,
                    &retained,
                    &candidate,
                    global_owned,
                    assembly_nonproblem,
                ),
                Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                    requested,
                    limit,
                }) if requested > limit
            ));
        }

        // The combined child ceiling can be stricter than a global clamp
        // which is itself below one visible component.  Component-first child
        // checks must not steal ownership from that stricter combined ceiling.
        let mut combined_owned = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        combined_owned
            .associate
            .max_combined_temporary_byte_envelope = visible - 2;
        combined_owned.max_compilation_owned_logical_peak_upper_bound =
            assembly_live.checked_add(visible - 1).unwrap();
        let combined_error = run_associate_intern(
            &context,
            &retained,
            &candidate,
            combined_owned,
            assembly_nonproblem,
        )
        .unwrap_err();
        assert!(matches!(
            combined_error,
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate combined temporary byte envelope",
                    requested,
                    limit,
                },
            ) if requested > limit && limit == visible - 2
        ));

        let mut global_owned = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        global_owned.associate.max_combined_temporary_byte_envelope = combined - 1;
        global_owned.max_compilation_owned_logical_peak_upper_bound =
            assembly_live.checked_add(combined - 1).unwrap();
        assert!(matches!(
            run_associate_intern(
                &context,
                &retained,
                &candidate,
                global_owned,
                assembly_nonproblem,
            ),
            Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource: "exact WhenBad partition compilation owned logical peak upper bound",
                requested,
                limit,
            }) if requested > limit
        ));
    }

    #[test]
    fn arbitrary_core_component_limits_are_not_relabelled_as_outer_global() {
        let fixture_name = "exact-when-bad-partition-core-component-ownership";
        let (_, _, _, baseline) = compile_sector_011(
            fixture_name,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        let stats = baseline.stats();
        let component_probes: &[(&str, StatGetter, LimitSetter)] = &[
            (
                "affine WhenBad arbitrary source problem owned logical bytes",
                |stats| stats.source_problem_retained_owned_logical_bytes(),
                |limits, value| limits.max_source_problem_retained_owned_logical_bytes = value,
            ),
            (
                "affine WhenBad arbitrary formula retained owned logical bytes",
                |stats| stats.partition_formula_retained_owned_logical_bytes(),
                |limits, value| limits.max_partition_formula_retained_owned_logical_bytes = value,
            ),
            (
                "affine WhenBad arbitrary formula compilation owned logical peak upper bound",
                |stats| stats.partition_formula_compilation_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_formula_compilation_owned_logical_peak_upper_bound = value
                },
            ),
            (
                "affine WhenBad arbitrary work owned logical peak upper bound",
                |stats| stats.partition_work_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_partition_work_owned_logical_peak_upper_bound = value,
            ),
            (
                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                |stats| stats.partition_compiler_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_compiler_owned_logical_peak_upper_bound = value
                },
            ),
        ];

        for (resource, getter, setter) in component_probes {
            let observed = getter(stats);
            assert!(observed > 0, "{resource} probe is vacuous");
            let mut limits = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            setter(&mut limits, observed - 1);
            let error = compile_sector_011_error(fixture_name, limits);
            assert!(
                matches!(
                    error,
                    GeneratedAffineResidualGroupExactWhenBadPartitionError::Relative(
                        AffineWhenBadRelativeCaseError::ResourceLimit {
                            resource: actual,
                            requested,
                            limit,
                        },
                    ) if actual == *resource && requested == observed && limit == observed - 1
                ),
                "unexpected {resource} component error: {error:?}",
            );
        }
    }

    #[test]
    fn outer_global_precedes_later_arbitrary_core_component_failures() {
        let fixture_name = "exact-when-bad-partition-core-global-ownership";
        let (_, _, _, baseline) = compile_sector_011(
            fixture_name,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        let stats = baseline.stats();
        let outer_before_partition = stats
            .retained_owned_logical_bytes()
            .checked_sub(stats.partition_retained_owned_logical_bytes())
            .unwrap();
        let source_boundary = outer_before_partition
            .checked_add(stats.source_problem_retained_owned_logical_bytes())
            .unwrap();
        assert!(source_boundary > 0);

        let later_components: &[(StatGetter, LimitSetter)] = &[
            (
                |stats| stats.partition_formula_retained_owned_logical_bytes(),
                |limits, value| limits.max_partition_formula_retained_owned_logical_bytes = value,
            ),
            (
                |stats| stats.partition_formula_compilation_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_formula_compilation_owned_logical_peak_upper_bound = value
                },
            ),
            (
                |stats| stats.partition_work_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_partition_work_owned_logical_peak_upper_bound = value,
            ),
            (
                |stats| stats.partition_compiler_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_partition_compiler_owned_logical_peak_upper_bound = value
                },
            ),
        ];
        for (getter, setter) in later_components {
            let observed = getter(stats);
            assert!(observed > 0);
            let mut limits = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
            setter(&mut limits, observed - 1);
            limits.max_compilation_owned_logical_peak_upper_bound = source_boundary - 1;
            assert!(matches!(
                compile_sector_011_error(fixture_name, limits),
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                    requested,
                    limit,
                } if requested > limit
            ));
        }

        let mut source_exact = GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default();
        source_exact.max_source_problem_retained_owned_logical_bytes =
            stats.source_problem_retained_owned_logical_bytes();
        source_exact.max_compilation_owned_logical_peak_upper_bound = source_boundary - 1;
        assert!(matches!(
            compile_sector_011_error(fixture_name, source_exact),
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                resource: "exact WhenBad partition compilation owned logical peak upper bound",
                requested,
                limit,
            } if requested > limit
        ));
    }

    #[test]
    fn transcript_and_foreign_partition_tampering_are_rejected() {
        let fixture_name = "exact-when-bad-partition-tamper";
        let (family, context, session, mut compilation) = compile_sector_011(
            fixture_name,
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(
            ready,
        ) = &mut compilation
        else {
            panic!("sector-011 fixture unexpectedly became identically bad");
        };
        let original = ready.transcript.atom_provenance[0];
        ready.transcript.atom_provenance[0].clause_atom_ordinal += 1;
        assert!(matches!(
            ready.replay(&family, &context, &session),
            Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)
        ));
        ready.transcript.atom_provenance[0] = original;
        ready.replay(&family, &context, &session).unwrap();

        let (_, _, _, foreign) = compile_sector_011(
            "exact-when-bad-partition-foreign",
            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(
            foreign,
        ) = foreign
        else {
            panic!("foreign sector-011 fixture unexpectedly became identically bad");
        };
        let original_partition = std::mem::replace(&mut ready.partition, foreign.partition);
        assert!(matches!(
            ready.replay(&family, &context, &session),
            Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)
        ));
        ready.partition = original_partition;
        ready.replay(&family, &context, &session).unwrap();
    }
}
