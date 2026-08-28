//! Owning exact relative `WhenBad` partition for the current generated-affine lineage.
//!
//! This phase consumes only the move-only materialization produced by
//! the sibling `materialization` phase. All
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

use super::analysis::GeneratedAffineResidualGroupReadyPublicationAnalysisLimits;
use super::condition_plan::{
    GeneratedAffineResidualGroupExactConditionHazardLocator,
    GeneratedAffineResidualGroupExactConditionPlanLimits,
    GeneratedAffineResidualGroupExactConditionSourceLocator,
};
use super::materialization::{
    GeneratedAffineResidualGroupExactBoundaryDisposition,
    GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    GeneratedAffineResidualGroupExactMappedConditionClass,
    GeneratedAffineResidualGroupExactMappedSource,
    GeneratedAffineResidualGroupExactWhenBadIdenticallyBad as MaterializedIdenticallyBad,
    GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason as MaterializedIdenticallyBadReason,
    GeneratedAffineResidualGroupExactWhenBadMaterialization,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
    GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    GeneratedAffineResidualGroupExactWhenBadReadyForPartition as MaterializedReadyForPartition,
};
use crate::canonical_parametric_locus_table::{
    CanonicalLocusTableBuilder, CanonicalLocusTableError, CanonicalLocusTableLimits,
    CanonicalLocusTableOwner,
};
use crate::generated_residual_affine_when_bad::{
    AffineWhenBadArbitraryRelativeLimits, AffineWhenBadArbitraryRelativePartitionCertificate,
    AffineWhenBadArbitraryRelativePartitionCompiler, AffineWhenBadAtom,
    AffineWhenBadAuthenticatedArbitraryRelativeProblem, AffineWhenBadInheritedTruth,
    AffineWhenBadRelativeCaseError, AffineWhenBadRelativeCaseLimits,
};
use crate::parametric_coefficient::{
    ParametricParameterIdentityClass, ParametricPolynomialAssociateLimits,
};
use crate::solver::exact_session::{
    GeneratedAffineResidualGroupExactSession,
    GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    GeneratedAffineResidualGroupExactSessionRecenterReady,
    GeneratedAffineResidualGroupExactSessionRecenterStats,
};
use crate::{
    IntegralFamily, ParametricCoefficientContext, ParametricCoefficientError, ParametricPolynomial,
    SymbolicPolynomialPredicateKind, algebra::ExactAlgebraLimits,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_PARTITION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-when-bad-partition-v1";

const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_CANONICAL_LOCUS_TABLE_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-when-bad-canonical-locus-table-v1";

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

    pub(crate) fn pivot_term_ordinal(&self) -> usize {
        self.owner.condition_plan().ready().pivot_term_ordinal()
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    /// Strip the final derivation transcript after its classifications have
    /// been compiled into direct application routes.
    pub(crate) fn into_publication_parts(
        self,
    ) -> (
        MaterializedReadyForPartition,
        AffineWhenBadArbitraryRelativePartitionCertificate,
    ) {
        (self.owner, self.partition)
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

/// Replay-authenticated, compact ownership for committing one rejected
/// candidate without retaining the materialization/partition transcripts.
///
/// The token is non-forgeable outside this module. It retains the original
/// sealed Ready transaction and the exact terminal reason, so the session can
/// return both unchanged on every failed commit preflight.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadRejectedCandidate {
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
    replay_recipe: GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
}

/// Copy-only recipe required to independently rederive a committed rejection
/// from a freshly replayed raw Ready row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe {
    reason: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason,
    ready_analysis_limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    condition_plan_limits: GeneratedAffineResidualGroupExactConditionPlanLimits,
    materialization_limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    partition_limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    rederivation_owned_logical_peak_upper_bound: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe {
    pub(crate) const fn reason(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason {
        self.reason
    }

    pub(crate) const fn ready_analysis_limits(
        self,
    ) -> GeneratedAffineResidualGroupReadyPublicationAnalysisLimits {
        self.ready_analysis_limits
    }

    pub(crate) const fn condition_plan_limits(
        self,
    ) -> GeneratedAffineResidualGroupExactConditionPlanLimits {
        self.condition_plan_limits
    }

    pub(crate) const fn materialization_limits(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
        self.materialization_limits
    }

    pub(crate) const fn partition_limits(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionLimits {
        self.partition_limits
    }

    pub(crate) const fn rederivation_owned_logical_peak_upper_bound(self) -> usize {
        self.rederivation_owned_logical_peak_upper_bound
    }
}

fn rejected_candidate_rederivation_peak(
    raw_ready_retained: usize,
    ready_analysis_retained: usize,
    condition_plan_retained: usize,
    partition_retained: usize,
    phase_peaks: [usize; 4],
) -> usize {
    raw_ready_retained
        .saturating_add(ready_analysis_retained)
        .saturating_add(condition_plan_retained)
        .saturating_add(partition_retained)
        .saturating_add(phase_peaks.into_iter().max().unwrap_or(0))
}

impl GeneratedAffineResidualGroupExactWhenBadRejectedCandidate {
    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason {
        self.replay_recipe.reason
    }

    pub(crate) fn target_locator(
        &self,
    ) -> &crate::solver::exact_session::GeneratedAffineResidualGroupSolveTargetLocator {
        self.ready.target_locator()
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.ready.source_ordinal()
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.ready.pivot_ordinal()
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.ready.stats()
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    pub(crate) const fn emits_residual(&self) -> bool {
        false
    }

    pub(crate) const fn ready(&self) -> &GeneratedAffineResidualGroupExactSessionRecenterReady {
        &self.ready
    }

    pub(crate) fn into_parts_for_session(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactSessionRecenterReady,
        GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
    ) {
        (self.ready, self.replay_recipe)
    }

    pub(crate) const fn replay_recipe(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe {
        self.replay_recipe
    }

    pub(crate) fn from_parts_for_session(
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
        replay_recipe: GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
    ) -> Self {
        Self {
            ready,
            replay_recipe,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadRejectedCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadRejectedCandidate")
            .field("reason", &self.reason())
            .field("target_locator", self.target_locator())
            .field("source_ordinal", &self.source_ordinal())
            .field("pivot_ordinal", &self.pivot_ordinal())
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .field("emits_residual", &false)
            .field("private_ready", &"<redacted>")
            .finish()
    }
}

/// Failed replay-before-distillation attempt. The complete proof owner is
/// returned, so callers may inspect/retry without losing any transcript.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {
    error: GeneratedAffineResidualGroupExactWhenBadPartitionError,
    owner: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad,
}

impl GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {
    pub(crate) const fn error(&self) -> &GeneratedAffineResidualGroupExactWhenBadPartitionError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactWhenBadPartitionError,
        GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBad,
    ) {
        (self.error, self.owner)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure")
            .field("error", &self.error)
            .field("private_owner", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact WhenBad rejection proof failed before distillation")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {}

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

    /// Replay the complete terminal proof, then consume it into the smallest
    /// non-publishing session capability. No transcript is discarded until
    /// replay has succeeded; failure returns this exact owner.
    pub(crate) fn into_rejected_candidate(
        self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<
        GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
        GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure,
    > {
        if let Err(error) = self.replay(family, context, session) {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadRejectedCandidateFailure {
                    error,
                    owner: self,
                },
            );
        }
        let reason = self.reason;
        let partition_limits = self.limits;
        let partition_peak = self.stats.compilation_owned_logical_peak_upper_bound();
        let partition_retained = self.stats.retained_owned_logical_bytes();
        let (condition_plan, materialization_limits, materialization_peak) = match self.payload {
            GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::Upstream(
                owner,
            ) => {
                let limits = owner.limits();
                let peak = owner.stats().compilation_owned_logical_peak_upper_bound();
                (owner.into_condition_plan_for_rejection(), limits, peak)
            }
            GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadPayload::AllExceptional {
                owner,
                transcript: _,
                partition: _,
            } => {
                let limits = owner.limits();
                let peak = owner.stats().compilation_owned_logical_peak_upper_bound();
                (owner.into_condition_plan_for_publication(), limits, peak)
            }
        };
        let condition_plan_limits = condition_plan.limits();
        let condition_plan_peak = condition_plan
            .stats()
            .compilation_owned_logical_peak_upper_bound();
        let condition_plan_retained = condition_plan.stats().retained_owned_logical_bytes();
        let ready_for_conditions = condition_plan.into_ready();
        let ready_analysis_limits = ready_for_conditions.limits();
        let ready_analysis_peak = ready_for_conditions
            .stats()
            .peak_prospective_retained_bytes();
        let ready_analysis_retained = ready_for_conditions.stats().retained_bytes();
        let raw_ready_retained = ready_for_conditions
            .ready()
            .stats()
            .kernel()
            .owner_retained_bytes();
        let ready = ready_for_conditions.into_ready();
        Ok(GeneratedAffineResidualGroupExactWhenBadRejectedCandidate {
            ready,
            replay_recipe: GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe {
                reason,
                ready_analysis_limits,
                condition_plan_limits,
                materialization_limits,
                partition_limits,
                // The terminal retains the raw recentered Ready row R plus
                // nested A -> C -> M -> P owners. P's retained census already
                // includes M, while R, A, and C remain separately live.
                // Distillation replays the complete terminal while that graph
                // is retained, so add the largest phase-local compiler/replay
                // peak to the full nested retained graph.
                rederivation_owned_logical_peak_upper_bound: rejected_candidate_rederivation_peak(
                    raw_ready_retained,
                    ready_analysis_retained,
                    condition_plan_retained,
                    partition_retained,
                    [
                        ready_analysis_peak,
                        condition_plan_peak,
                        materialization_peak,
                        partition_peak,
                    ],
                ),
            },
        })
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
    canonical_locus_slots: usize,
    canonical_locus_context_fingerprint_bytes: usize,
    transcript_retained_owned_logical_bytes: usize,
    source_problem_container_owned_logical_bytes: usize,
    assembly_container_owned_logical_bytes: usize,
}

struct PreparedPartitionAssembly {
    canonical_loci: CanonicalLocusTableOwner,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    atoms: Vec<AffineWhenBadAtom>,
    clause_ranges: Vec<Range<usize>>,
    transcript: GeneratedAffineResidualGroupExactWhenBadPartitionTranscript,
    stats: GeneratedAffineResidualGroupExactWhenBadPartitionStats,
}

#[derive(Clone, Copy, Debug)]
struct CanonicalLocusAssemblyAccounting {
    source_problem_noncanonical_owned_logical_bytes: usize,
    assembly_nonproblem_owned_logical_bytes: usize,
    builder_owner_header_delta: usize,
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
    let admission = preflight_partition_assembly(context, owner, limits)?;
    let prepared = assemble_partition_problem(context, owner, admission, limits)?;

    let PreparedPartitionAssembly {
        canonical_loci,
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

    let core_source_problem_owned_logical_byte_envelope =
        authenticated_core_source_problem_owned_logical_byte_envelope(
            &canonical_loci,
            inherited_truths.len(),
            atoms.len(),
            clause_ranges.len(),
        )?;
    let problem = AffineWhenBadAuthenticatedArbitraryRelativeProblem::from_preallocated(
        canonical_loci,
        GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_CANONICAL_LOCUS_TABLE_V1_SCHEMA,
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
    let partition = match AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
        context,
        problem,
        arbitrary_limits,
    ) {
        Ok(partition) => partition,
        Err(failure) => match failure.into_parts().0 {
            AffineWhenBadRelativeCaseError::ResourceLimit {
                resource: "affine WhenBad arbitrary compilation owned logical peak upper bound",
                requested,
                ..
            } => {
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
            AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                resource: "affine WhenBad arbitrary compilation owned logical peak upper bound",
            } => {
                return Err(
                    GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                        resource: "exact WhenBad partition compilation owned logical peak upper bound",
                    },
                );
            }
            error => return Err(error.into()),
        },
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
        .max(core_source_problem_owned_logical_byte_envelope)
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
    context: &ParametricCoefficientContext,
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

    admission.canonical_locus_slots = admission.locus_inputs.min(limits.max_structural_loci);
    admission.canonical_locus_context_fingerprint_bytes = context.fingerprint().len();

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
    let source_problem_noncanonical_owned_logical_bytes =
        admitted_source_problem_noncanonical_bytes(admission)?;
    let builder_owner_header_delta = size_of::<CanonicalLocusTableBuilder>()
        .checked_sub(size_of::<CanonicalLocusTableOwner>())
        .ok_or(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)?;
    let accounting = CanonicalLocusAssemblyAccounting {
        source_problem_noncanonical_owned_logical_bytes,
        assembly_nonproblem_owned_logical_bytes,
        builder_owner_header_delta,
    };
    let owner_retained_allowance = remaining_limit(
        "exact WhenBad partition source problem retained owned logical bytes",
        limits.max_source_problem_retained_owned_logical_bytes,
        source_problem_noncanonical_owned_logical_bytes,
    )?;
    let builder_retained_allowance = owner_retained_allowance
        .checked_add(builder_owner_header_delta)
        .unwrap_or(usize::MAX);
    let assembly_builder_base = checked_add(
        "exact WhenBad partition compilation owned logical peak upper bound",
        assembly_nonproblem_owned_logical_bytes,
        source_problem_noncanonical_owned_logical_bytes,
    )?;
    let builder_construction_allowance = remaining_limit(
        "exact WhenBad partition compilation owned logical peak upper bound",
        limits.max_compilation_owned_logical_peak_upper_bound,
        assembly_builder_base,
    )?;
    let mut builder_associate = limits.associate;
    builder_associate.exact_algebra =
        intersect_exact_limits(builder_associate.exact_algebra, limits.exact_algebra);
    builder_associate.max_rustred_visible_temporary_byte_envelope = builder_associate
        .max_rustred_visible_temporary_byte_envelope
        .min(limits.max_intern_associate_rustred_visible_temporary_byte_peak);
    builder_associate.max_native_workspace_byte_envelope = builder_associate
        .max_native_workspace_byte_envelope
        .min(limits.max_intern_associate_native_workspace_byte_peak);
    let builder_limits = CanonicalLocusTableLimits {
        exact_algebra: limits.exact_algebra,
        associate: builder_associate,
        // The source-neutral compiler owns the public context-fingerprint
        // limit. This outer owner only authenticates the exact same context.
        max_context_fingerprint_bytes: usize::MAX,
        max_structural_loci: limits.max_structural_loci,
        max_equality_comparisons: limits.max_intern_equality_comparisons,
        max_equality_term_pairs: limits.max_intern_equality_term_pairs,
        max_associate_comparisons: limits.max_intern_associate_comparisons,
        // The established outer API has no separate structural term-pair
        // ceiling; the exact native proof remains bounded by its own limits.
        max_associate_term_pairs: usize::MAX,
        max_associate_native_cross_term_pairs: limits.max_intern_associate_native_cross_term_pairs,
        max_retained_polynomial_terms: limits.max_retained_polynomial_terms,
        max_retained_polynomial_exponent_entries: limits.max_retained_polynomial_exponent_entries,
        max_retained_polynomial_integer_bits: limits.max_retained_polynomial_integer_bits,
        max_retained_owned_logical_bytes: builder_retained_allowance,
        max_construction_owned_logical_peak_upper_bound: builder_construction_allowance,
    };
    let mut canonical_loci = CanonicalLocusTableBuilder::try_new(
        context,
        GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_CANONICAL_LOCUS_TABLE_V1_SCHEMA,
        admission.canonical_locus_slots,
        builder_limits,
    )
    .map_err(|error| map_canonical_locus_table_error(error, accounting, limits))?;
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
    let mut stats = GeneratedAffineResidualGroupExactWhenBadPartitionStats {
        transcript_retained_owned_logical_bytes: admission.transcript_retained_owned_logical_bytes,
        source_problem_retained_owned_logical_bytes: admission
            .source_problem_container_owned_logical_bytes,
        materialization_retained_owned_logical_bytes: owner.stats().retained_owned_logical_bytes(),
        compilation_owned_logical_peak_upper_bound: owner
            .stats()
            .retained_owned_logical_bytes()
            .checked_add(admission.assembly_container_owned_logical_bytes)
            .ok_or(
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                },
            )?,
        ..GeneratedAffineResidualGroupExactWhenBadPartitionStats::default()
    };
    synchronize_canonical_locus_stats(&canonical_loci, accounting, &mut stats)?;

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
                                &mut canonical_loci,
                                &mut transcript.locus_origins,
                                limits,
                                &mut stats,
                                accounting,
                            )?;
                            while inherited_truth_seeded.len() < canonical_loci.loci().len() {
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
                                &mut canonical_loci,
                                &mut transcript.locus_origins,
                                limits,
                                &mut stats,
                                accounting,
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
                                    &mut canonical_loci,
                                    &mut transcript.locus_origins,
                                    limits,
                                    &mut stats,
                                    accounting,
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
                    &mut canonical_loci,
                    &mut transcript.locus_origins,
                    limits,
                    &mut stats,
                    accounting,
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

    if canonical_loci.loci().len() > admission.locus_inputs
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
    stats.structural_loci = canonical_loci.loci().len();
    stats.inherited_truth_occurrences = transcript.inherited_provenance.len();
    stats.inherited_truths = inherited_truths.len();
    stats.bad_atoms = atoms.len();
    stats.bad_clauses = clause_ranges.len();
    stats.locus_origins = transcript.locus_origins.len();
    stats.inherited_provenances = transcript.inherited_provenance.len();
    stats.atom_provenances = transcript.atom_provenance.len();
    stats.clause_provenances = transcript.clause_provenance.len();

    let canonical_loci = canonical_loci
        .seal()
        .map_err(|error| map_canonical_locus_table_error(error, accounting, limits))?;
    let observed_source_problem = checked_add(
        "exact WhenBad partition source problem retained owned logical bytes",
        accounting.source_problem_noncanonical_owned_logical_bytes,
        canonical_loci
            .retained_owned_logical_byte_bound()
            .map_err(|error| map_canonical_locus_table_error(error, accounting, limits))?,
    )?;
    if observed_source_problem != stats.source_problem_retained_owned_logical_bytes {
        return Err(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch);
    }

    Ok(PreparedPartitionAssembly {
        canonical_loci,
        inherited_truths,
        atoms,
        clause_ranges,
        transcript,
        stats,
    })
}

fn canonical_builder_retained_as_owner(
    builder_retained: usize,
    accounting: CanonicalLocusAssemblyAccounting,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    builder_retained
        .checked_sub(accounting.builder_owner_header_delta)
        .ok_or(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)
}

fn canonical_builder_source_problem_bytes(
    builder_retained: usize,
    accounting: CanonicalLocusAssemblyAccounting,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    checked_add(
        "exact WhenBad partition source problem retained owned logical bytes",
        accounting.source_problem_noncanonical_owned_logical_bytes,
        canonical_builder_retained_as_owner(builder_retained, accounting)?,
    )
}

fn canonical_builder_compilation_peak(
    builder_peak: usize,
    accounting: CanonicalLocusAssemblyAccounting,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    checked_add(
        "exact WhenBad partition compilation owned logical peak upper bound",
        checked_add(
            "exact WhenBad partition compilation owned logical peak upper bound",
            accounting.assembly_nonproblem_owned_logical_bytes,
            accounting.source_problem_noncanonical_owned_logical_bytes,
        )?,
        builder_peak,
    )
}

fn synchronize_canonical_locus_stats(
    canonical_loci: &CanonicalLocusTableBuilder,
    accounting: CanonicalLocusAssemblyAccounting,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadPartitionStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let table = canonical_loci.stats();
    stats.structural_loci = table.structural_loci();
    stats.intern_equality_comparisons = table.equality_comparisons();
    stats.intern_equality_term_pairs = table.equality_term_pairs();
    stats.intern_associate_comparisons = table.associate_comparisons();
    stats.intern_associate_native_cross_term_pairs = table.associate_native_cross_term_pairs();
    stats.intern_associate_rustred_visible_temporary_byte_peak =
        table.associate_rustred_visible_temporary_byte_peak();
    stats.intern_associate_native_workspace_byte_peak =
        table.associate_native_workspace_byte_peak();
    stats.retained_polynomial_terms = table.retained_polynomial_terms();
    stats.retained_polynomial_exponent_entries = table.retained_polynomial_exponent_entries();
    stats.retained_polynomial_integer_bits = table.retained_polynomial_integer_bits();
    stats.source_problem_retained_owned_logical_bytes =
        canonical_builder_source_problem_bytes(table.retained_owned_logical_bytes(), accounting)?;
    stats.compilation_owned_logical_peak_upper_bound = stats
        .compilation_owned_logical_peak_upper_bound
        .max(canonical_builder_compilation_peak(
            table.construction_owned_logical_peak_upper_bound(),
            accounting,
        )?);
    Ok(())
}

fn map_canonical_locus_table_resource(resource: &'static str) -> &'static str {
    match resource {
        "canonical locus table structural loci" | "canonical locus table reserved capacity" => {
            "exact WhenBad partition structural loci"
        }
        "canonical locus table equality comparisons" => {
            "exact WhenBad partition intern equality comparisons"
        }
        "canonical locus table equality term pairs" => {
            "exact WhenBad partition intern equality term pairs"
        }
        "canonical locus table associate comparisons" => {
            "exact WhenBad partition intern associate comparisons"
        }
        "canonical locus table associate term pairs" => {
            "exact WhenBad partition intern associate term pairs"
        }
        "canonical locus table associate native cross term pairs" => {
            "exact WhenBad partition intern associate native cross term pairs"
        }
        "canonical locus table retained polynomial terms" => {
            "exact WhenBad partition retained polynomial terms"
        }
        "canonical locus table retained polynomial exponent entries" => {
            "exact WhenBad partition retained polynomial exponent entries"
        }
        "canonical locus table retained polynomial integer bits" => {
            "exact WhenBad partition retained polynomial integer bits"
        }
        "canonical locus table context fingerprint bytes" => {
            "exact WhenBad partition canonical locus context fingerprint bytes"
        }
        other => other,
    }
}

fn map_canonical_locus_table_error(
    error: CanonicalLocusTableError,
    accounting: CanonicalLocusAssemblyAccounting,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
) -> GeneratedAffineResidualGroupExactWhenBadPartitionError {
    match error {
        CanonicalLocusTableError::SchemaMismatch => {
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch
        }
        CanonicalLocusTableError::ContextMismatch
        | CanonicalLocusTableError::IdenticallyZeroLocus
        | CanonicalLocusTableError::CoefficientFieldLocus => {
            GeneratedAffineResidualGroupExactWhenBadPartitionError::MalformedMaterialization
        }
        CanonicalLocusTableError::ReservedCapacityExhausted {
            requested,
            reserved,
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
            resource: "exact WhenBad partition structural loci",
            requested,
            limit: reserved.min(limits.max_structural_loci),
        },
        CanonicalLocusTableError::ResourceLimit {
            resource: "canonical locus table retained owned logical bytes",
            requested,
            ..
        } => match canonical_builder_source_problem_bytes(requested, accounting) {
            Ok(requested) => {
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                    resource: "exact WhenBad partition source problem retained owned logical bytes",
                    requested,
                    limit: limits.max_source_problem_retained_owned_logical_bytes,
                }
            }
            Err(error) => error,
        },
        CanonicalLocusTableError::ResourceLimit {
            resource: "canonical locus table construction owned logical peak upper bound",
            requested,
            ..
        } => match canonical_builder_compilation_peak(requested, accounting) {
            Ok(requested) => {
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
                    resource: "exact WhenBad partition compilation owned logical peak upper bound",
                    requested,
                    limit: limits.max_compilation_owned_logical_peak_upper_bound,
                }
            }
            Err(error) => error,
        },
        CanonicalLocusTableError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceLimit {
            resource: map_canonical_locus_table_resource(resource),
            requested,
            limit,
        },
        CanonicalLocusTableError::ResourceCountOverflow {
            resource: "canonical locus table retained owned logical bytes",
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
            resource: "exact WhenBad partition source problem retained owned logical bytes",
        },
        CanonicalLocusTableError::ResourceCountOverflow {
            resource: "canonical locus table construction owned logical peak upper bound",
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
            resource: "exact WhenBad partition compilation owned logical peak upper bound",
        },
        CanonicalLocusTableError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                resource: map_canonical_locus_table_resource(resource),
            }
        }
        CanonicalLocusTableError::AllocationFailure {
            resource: "canonical locus table structural loci",
            requested,
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
            resource: "exact WhenBad partition structural loci",
            requested,
        },
        CanonicalLocusTableError::AllocationFailure {
            resource: "canonical locus table context fingerprint",
            requested,
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
            resource: "exact WhenBad partition canonical locus context fingerprint",
            requested,
        },
        CanonicalLocusTableError::AllocationFailure {
            resource,
            requested,
        } => GeneratedAffineResidualGroupExactWhenBadPartitionError::AllocationFailure {
            resource,
            requested,
        },
        CanonicalLocusTableError::RetainedByteEnvelopeExceeded { .. } => {
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch
        }
        CanonicalLocusTableError::SymbolicaPanic { .. } => {
            GeneratedAffineResidualGroupExactWhenBadPartitionError::SymbolicaPanic
        }
        CanonicalLocusTableError::ParametricCoefficient(error) => error.into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn intern_locus(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    origin: GeneratedAffineResidualGroupExactWhenBadLocusOrigin,
    canonical_loci: &mut CanonicalLocusTableBuilder,
    origins: &mut Vec<GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord>,
    limits: GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadPartitionStats,
    accounting: CanonicalLocusAssemblyAccounting,
) -> Result<(usize, usize), GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let outcome = canonical_loci
        .try_intern(context, polynomial)
        .map_err(|error| map_canonical_locus_table_error(error, accounting, limits))?;
    synchronize_canonical_locus_stats(canonical_loci, accounting, stats)?;

    let origin_record_ordinal = origins.len();
    origins.push(GeneratedAffineResidualGroupExactWhenBadLocusOriginRecord {
        ordinal: origin_record_ordinal,
        locus_ordinal: outcome.locus_ordinal(),
        origin,
    });
    Ok((outcome.locus_ordinal(), origin_record_ordinal))
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
    left == right
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
    let mut bytes = admitted_source_problem_noncanonical_bytes(admission)?;
    for (count, element_size) in [
        (1, size_of::<CanonicalLocusTableOwner>()),
        (
            admission.canonical_locus_context_fingerprint_bytes,
            size_of::<u8>(),
        ),
        (
            admission.canonical_locus_slots,
            size_of::<ParametricPolynomial>(),
        ),
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

fn admitted_source_problem_noncanonical_bytes(
    admission: PartitionAssemblyAdmission,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let mut bytes = size_of::<AffineWhenBadAuthenticatedArbitraryRelativeProblem>()
        .checked_sub(size_of::<CanonicalLocusTableOwner>())
        .ok_or(GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch)?;
    for (count, element_size) in [
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

fn authenticated_core_source_problem_owned_logical_byte_envelope(
    canonical_loci: &CanonicalLocusTableOwner,
    inherited_truths: usize,
    atoms: usize,
    clause_ranges: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let resource = "exact WhenBad partition source problem retained owned logical bytes";
    let canonical_dynamic = canonical_loci
        .retained_owned_logical_byte_bound()
        .map_err(|error| match error {
            CanonicalLocusTableError::ResourceCountOverflow { .. } => {
                GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                    resource,
                }
            }
            _ => GeneratedAffineResidualGroupExactWhenBadPartitionError::ReplayMismatch,
        })?
        .checked_sub(size_of::<CanonicalLocusTableOwner>())
        .ok_or(
            GeneratedAffineResidualGroupExactWhenBadPartitionError::ResourceCountOverflow {
                resource,
            },
        )?;
    let mut bytes = checked_add(
        resource,
        size_of::<AffineWhenBadAuthenticatedArbitraryRelativeProblem>(),
        canonical_dynamic,
    )?;
    for (count, element_size) in [
        (inherited_truths, size_of::<AffineWhenBadInheritedTruth>()),
        (atoms, size_of::<AffineWhenBadAtom>()),
        (clause_ranges, size_of::<Range<usize>>()),
    ] {
        bytes = checked_add(
            resource,
            bytes,
            checked_mul(resource, checked_mul(resource, count, element_size)?, 2)?,
        )?;
    }
    Ok(bytes)
}

fn admitted_assembly_container_bytes(
    admission: PartitionAssemblyAdmission,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadPartitionError> {
    let canonical_builder_bytes = checked_add(
        "exact WhenBad partition assembly container owned logical bytes",
        checked_add(
            "exact WhenBad partition assembly container owned logical bytes",
            size_of::<CanonicalLocusTableBuilder>(),
            admission.canonical_locus_context_fingerprint_bytes,
        )?,
        checked_mul(
            "exact WhenBad partition assembly container owned logical bytes",
            admission.canonical_locus_slots,
            size_of::<ParametricPolynomial>(),
        )?,
    )?;
    checked_add(
        "exact WhenBad partition assembly container owned logical bytes",
        checked_add(
            "exact WhenBad partition assembly container owned logical bytes",
            admission.transcript_retained_owned_logical_bytes,
            checked_add(
                "exact WhenBad partition assembly container owned logical bytes",
                admitted_source_problem_noncanonical_bytes(admission)?,
                canonical_builder_bytes,
            )?,
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
