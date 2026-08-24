//! Sealed source authority for generated residual-affine epochs.
//!
//! The initial epoch consumes the global live-leaf queue.  Every later epoch
//! consumes only the exact residual queue retained by the preceding effective
//! affine owner.  This sealed wrapper owns one `Arc` handle in either case and exposes
//! only common scope metadata plus deterministic replay; semantic source
//! views remain a separate, narrower boundary.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::generated_affine_initial_global_affine_terminal::{
    GeneratedAffineInitialGlobalAffineBoundTerminal, GeneratedAffineInitialGlobalAffineTerminal,
    GeneratedAffineInitialGlobalAffineTerminalSourceView,
};
use crate::generated_sector_affine_effective_coverage::GeneratedSectorAffinePointStats;
use crate::generated_sector_affine_effective_residual_queue::{
    GeneratedSectorAffineEffectiveResidualAtomPolarity,
    GeneratedSectorAffineEffectiveResidualExceptionalSourceView,
    GeneratedSectorAffineEffectiveResidualQueueCertificate,
    GeneratedSectorAffineEffectiveResidualQueueError,
    GeneratedSectorAffineEffectiveResidualQueuePointDisposition,
    GeneratedSectorAffineEffectiveResidualQueuePointLimits,
    GeneratedSectorAffineEffectiveResidualSourceView,
    GeneratedSectorAffineEffectiveResidualSourceViewError,
    GeneratedSectorAffineEffectiveResidualTargetSourceView,
    GeneratedSectorAffineEffectiveResidualTerminalSourceView,
    GeneratedSectorAffineEffectiveResidualUnsupportedSourceView,
};
use crate::product_locus_boolean_cover::ResidualProductLocusBooleanReplaySession;
use crate::{
    COORDINATE_EQUALITY_LOCUS_V1_SCHEMA, CoordinateEqualityLeafStatus,
    GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA, GeneratedSectorLiveLeafOutcome,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorQueuedSourceDisposition, IntegralFamily, IntegralOrderingPolicy,
    PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA, ParametricCoefficientContext, ParametricPolynomial,
    ParametricRelation, ParametricSectorCoverageError, ParametricSectorLeafDisposition,
    ParametricSectorProductZeroDecomposition, ResidualAffineBranchGuardCompositionClass,
    ResidualAffineBranchGuardCompositionEntry, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemLimits,
    ResidualAffineBranchUnsupportedReason, ResidualAffineIntegerMap,
    ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanCoverStats,
    ResidualProductLocusBooleanNodeOutcome, ResidualUnitAffinePolynomialCompositionStats,
    SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA, SectorMask, SymbolicPolynomialPredicate,
    SymbolicPolynomialPredicateKind, SymbolicSectorCase, SymbolicSectorCaseId,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-source-authority-v1";

/// Which authenticated residual source feeds one generated affine epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualSourceAuthorityKind {
    InitialGlobal,
    PriorEffective,
}

const fn source_point_portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

/// Query-wide, allocation-free admission envelope for a batch of exact
/// `K(n) -> K` polynomial specializations.  The authenticated source retains
/// the per-specialization arithmetic policy; these limits bound the complete
/// aggregate work before the first Symbolica/GMP-producing execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualPointSpecializationLimits {
    pub(crate) max_source_terms: usize,
    pub(crate) max_source_exponent_entries: usize,
    pub(crate) max_preflight_validation_source_term_scan_bound: usize,
    pub(crate) max_preflight_validation_source_exponent_entry_scan_bound: usize,
    pub(crate) max_output_term_bound: usize,
    pub(crate) max_output_exponent_entry_bound: usize,
    pub(crate) max_power_operation_bound: usize,
    pub(crate) max_largest_output_integer_bit_bound: usize,
    pub(crate) max_integer_bit_work_bound: usize,
    pub(crate) max_retained_output_term_bound: usize,
    pub(crate) max_retained_output_byte_bound: usize,
}

impl Default for GeneratedAffineResidualPointSpecializationLimits {
    fn default() -> Self {
        Self {
            max_source_terms: 1_000_000_000,
            max_source_exponent_entries: source_point_portable_usize(64_000_000_000),
            max_preflight_validation_source_term_scan_bound: source_point_portable_usize(
                8_000_000_000,
            ),
            max_preflight_validation_source_exponent_entry_scan_bound: source_point_portable_usize(
                640_000_000_000,
            ),
            max_output_term_bound: 4_000_000_000,
            max_output_exponent_entry_bound: source_point_portable_usize(256_000_000_000),
            max_power_operation_bound: source_point_portable_usize(64_000_000_000),
            max_largest_output_integer_bit_bound: 64_000_000,
            max_integer_bit_work_bound: source_point_portable_usize(64_000_000_000_000),
            max_retained_output_term_bound: 4_000_000_000,
            max_retained_output_byte_bound: source_point_portable_usize(256 * 1024 * 1024 * 1024),
        }
    }
}

/// Prospective aggregate specialization work authenticated before execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualPointSpecializationStats {
    source_terms: usize,
    source_exponent_entries: usize,
    preflight_validation_source_term_scan_bound: usize,
    preflight_validation_source_exponent_entry_scan_bound: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    largest_output_integer_bit_bound: usize,
    integer_bit_work_bound: usize,
    retained_output_term_bound: usize,
    retained_output_byte_bound: usize,
}

macro_rules! residual_point_specialization_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualPointSpecializationStats {
    residual_point_specialization_stats_getters!(
        source_terms,
        source_exponent_entries,
        preflight_validation_source_term_scan_bound,
        preflight_validation_source_exponent_entry_scan_bound,
        output_term_bound,
        output_exponent_entry_bound,
        power_operation_bound,
        largest_output_integer_bit_bound,
        integer_bit_work_bound,
        retained_output_term_bound,
        retained_output_byte_bound,
    );
}

/// Prospective work for resolving one source ordinal through its retained
/// authority.  Prior-effective counts are conservative authenticated batch
/// bounds sealed by the prior queue; initial counts are exact except for the
/// logarithmic binary-search ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourceNavigationLimits {
    pub(crate) max_source_view_resolutions: usize,
    pub(crate) max_initial_case_lookup_comparisons: usize,
    pub(crate) max_initial_disposition_candidate_comparisons: usize,
    pub(crate) max_prior_authority_index_comparison_bound: usize,
    pub(crate) max_prior_projection_payload_comparison_bound: usize,
}

impl Default for GeneratedAffineResidualSourceNavigationLimits {
    fn default() -> Self {
        Self {
            max_source_view_resolutions: 1,
            max_initial_case_lookup_comparisons: usize::BITS as usize + 1,
            max_initial_disposition_candidate_comparisons: 1_000_000_000,
            max_prior_authority_index_comparison_bound: source_point_portable_usize(64_000_000_000),
            max_prior_projection_payload_comparison_bound: source_point_portable_usize(
                64_000_000_000,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourceNavigationStats {
    source_view_resolutions: usize,
    initial_case_lookup_comparisons: usize,
    initial_disposition_candidate_comparisons: usize,
    prior_authority_index_comparison_bound: usize,
    prior_projection_payload_comparison_bound: usize,
}

impl GeneratedAffineResidualSourceNavigationStats {
    pub(crate) const fn source_view_resolutions(self) -> usize {
        self.source_view_resolutions
    }
    pub(crate) const fn initial_case_lookup_comparisons(self) -> usize {
        self.initial_case_lookup_comparisons
    }
    pub(crate) const fn initial_disposition_candidate_comparisons(self) -> usize {
        self.initial_disposition_candidate_comparisons
    }
    pub(crate) const fn prior_authority_index_comparison_bound(self) -> usize {
        self.prior_authority_index_comparison_bound
    }
    pub(crate) const fn prior_projection_payload_comparison_bound(self) -> usize {
        self.prior_projection_payload_comparison_bound
    }
}

/// Aggregate bounds for one exact point lookup through either retained source
/// version.  Initial-global classification evaluates the frozen global case
/// partition and then performs one complete, uniqueness-checking queue scan.
/// Prior-effective classification delegates to the already bounded exact
/// residual-queue classifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourcePointLimits {
    pub(crate) prior_effective: GeneratedSectorAffineEffectiveResidualQueuePointLimits,
    pub(crate) initial_specialization: GeneratedAffineResidualPointSpecializationLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_index_entries: usize,
    pub(crate) max_initial_orthant_index_scans: usize,
    pub(crate) max_initial_case_scans: usize,
    pub(crate) max_initial_classification_scans: usize,
    pub(crate) max_initial_predicate_scans: usize,
    pub(crate) max_initial_predicate_evaluations: usize,
    pub(crate) max_initial_work_item_scans: usize,
    pub(crate) max_initial_disposition_candidate_comparisons: usize,
}

impl Default for GeneratedAffineResidualSourcePointLimits {
    fn default() -> Self {
        Self {
            prior_effective: GeneratedSectorAffineEffectiveResidualQueuePointLimits::default(),
            initial_specialization: GeneratedAffineResidualPointSpecializationLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_index_entries: 1_000_000,
            max_initial_orthant_index_scans: 2_000_000,
            max_initial_case_scans: 2_000_000_000,
            max_initial_classification_scans: 2_000_000_000,
            max_initial_predicate_scans: 32_000_000_000usize,
            max_initial_predicate_evaluations: 16_000_000_000usize,
            max_initial_work_item_scans: 1_000_000_000,
            max_initial_disposition_candidate_comparisons: 1_000_000_000,
        }
    }
}

/// Exact outer work performed by one successful source-neutral point lookup.
/// The delegated prior owner keeps its more detailed arithmetic census behind
/// its sealed certificate; this layer records the exact residual queue scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourcePointStats {
    kind: Option<GeneratedAffineResidualSourceAuthorityKind>,
    scope_comparison_bytes: usize,
    index_entries: usize,
    initial_orthant_index_scans: usize,
    initial_case_scans: usize,
    initial_classification_scans: usize,
    initial_predicate_scans: usize,
    initial_predicate_evaluations: usize,
    initial_specialization: GeneratedAffineResidualPointSpecializationStats,
    prior_effective_owner: Option<GeneratedSectorAffinePointStats>,
    work_item_scans: usize,
    initial_disposition_candidate_comparisons: usize,
}

impl GeneratedAffineResidualSourcePointStats {
    pub(crate) const fn kind(self) -> Option<GeneratedAffineResidualSourceAuthorityKind> {
        self.kind
    }
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn index_entries(self) -> usize {
        self.index_entries
    }
    pub(crate) const fn initial_orthant_index_scans(self) -> usize {
        self.initial_orthant_index_scans
    }
    pub(crate) const fn initial_case_scans(self) -> usize {
        self.initial_case_scans
    }
    pub(crate) const fn initial_classification_scans(self) -> usize {
        self.initial_classification_scans
    }
    pub(crate) const fn initial_predicate_scans(self) -> usize {
        self.initial_predicate_scans
    }
    pub(crate) const fn initial_predicate_evaluations(self) -> usize {
        self.initial_predicate_evaluations
    }
    pub(crate) const fn initial_specialization(
        self,
    ) -> GeneratedAffineResidualPointSpecializationStats {
        self.initial_specialization
    }
    pub(crate) const fn prior_effective_owner(self) -> Option<GeneratedSectorAffinePointStats> {
        self.prior_effective_owner
    }
    pub(crate) const fn work_item_scans(self) -> usize {
        self.work_item_scans
    }
    pub(crate) const fn initial_disposition_candidate_comparisons(self) -> usize {
        self.initial_disposition_candidate_comparisons
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualSourcePointDisposition {
    Excluded,
    Work { work_item_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourcePointClassification {
    disposition: GeneratedAffineResidualSourcePointDisposition,
    stats: GeneratedAffineResidualSourcePointStats,
}

impl GeneratedAffineResidualSourcePointClassification {
    pub(crate) const fn disposition(self) -> GeneratedAffineResidualSourcePointDisposition {
        self.disposition
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualSourcePointStats {
        self.stats
    }
}

pub(crate) enum GeneratedAffineResidualSourcePointError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    AuthorityMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    InitialCoverage(ParametricSectorCoverageError),
    PriorEffective(GeneratedSectorAffineEffectiveResidualQueueError),
    SourceView(GeneratedAffineResidualSourceViewError),
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffineResidualSourcePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::AuthorityMismatch => "AuthorityMismatch",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::InitialCoverage(_) => "InitialCoverage",
            Self::PriorEffective(_) => "PriorEffective",
            Self::SourceView(_) => "SourceView",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedAffineResidualSourcePointError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualSourcePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual source point schema mismatch"),
            Self::WrongFamily => formatter.write_str("residual source point family mismatch"),
            Self::WrongContext => formatter.write_str("residual source point context mismatch"),
            Self::WrongArity => formatter.write_str("residual source point arity mismatch"),
            Self::AuthorityMismatch => {
                formatter.write_str("residual source point authority mismatch")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("residual source point resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("residual source point resource limit exceeded")
            }
            Self::InitialCoverage(_) => {
                formatter.write_str("initial residual source point classification failed")
            }
            Self::PriorEffective(_) => {
                formatter.write_str("prior residual source point classification failed")
            }
            Self::SourceView(_) => formatter.write_str("residual source point navigation failed"),
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked during residual source point classification"),
        }
    }
}

// Nested proof diagnostics remain redacted at this source-version boundary.
impl std::error::Error for GeneratedAffineResidualSourcePointError {}

/// Source-wide conservative navigation work which is not represented by the
/// exact scalar fields returned on individual initial-global views.
///
/// Prior-effective resolution follows sealed owner indices and projection
/// authentication paths. Its queue has already computed complete batch bounds
/// for those operations; a later collection compiler must admit these bounds
/// before resolving any prior source item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourceBatchNavigationCensus {
    prior_authority_index_comparison_bound: usize,
    prior_projection_payload_comparison_bound: usize,
}

impl GeneratedAffineResidualSourceBatchNavigationCensus {
    pub(crate) const fn prior_authority_index_comparison_bound(self) -> usize {
        self.prior_authority_index_comparison_bound
    }

    pub(crate) const fn prior_projection_payload_comparison_bound(self) -> usize {
        self.prior_projection_payload_comparison_bound
    }
}

/// Scalar identity shared by both initial-global outcomes.
///
/// The lifetime prevents this value from outliving the exact retained source
/// authority.  No queue, extraction, partition, relation, or owning `Arc`
/// crosses this seam.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalTerminalSourceView<'source> {
    work_item_ordinal: usize,
    source_case_position: usize,
    source_identity_bytes: usize,
    case_lookup_comparisons: usize,
    source_disposition_candidate_comparisons: usize,
    lifetime: std::marker::PhantomData<&'source ()>,
}

impl GeneratedAffineInitialGlobalTerminalSourceView<'_> {
    pub(crate) const fn work_item_ordinal(self) -> usize {
        self.work_item_ordinal
    }

    /// Bytes in the retained canonical partition identity.  This is scalar
    /// budgeting provenance, not access to that private identity.
    pub(crate) const fn source_identity_bytes(self) -> usize {
        self.source_identity_bytes
    }

    /// Exact comparisons made by the bounded binary source-case lookup.
    pub(crate) const fn case_lookup_comparisons(self) -> usize {
        self.case_lookup_comparisons
    }

    /// Exact candidate-ordinal comparisons used to bind an unsupported
    /// queue disposition to its retained global classification.
    pub(crate) const fn source_disposition_candidate_comparisons(self) -> usize {
        self.source_disposition_candidate_comparisons
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalTerminalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalTerminalSourceView")
            .field("work_item_ordinal", &self.work_item_ordinal)
            .field("source_identity_bytes", &self.source_identity_bytes)
            .field("case_lookup_comparisons", &self.case_lookup_comparisons)
            .field(
                "source_disposition_candidate_comparisons",
                &self.source_disposition_candidate_comparisons,
            )
            .field("private_terminal_authority", &"<redacted>")
            .finish()
    }
}

/// Explicit per-predicate bounds for the only non-O(1) lookup retained by the
/// initial-global view.  A later compiler can pass its remaining aggregate
/// budget and accumulate the returned exact statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalPredicateLookupLimits {
    max_structural_locus_comparisons: usize,
    max_product_decomposition_comparisons: usize,
    max_factor_locus_checks: usize,
}

impl GeneratedAffineInitialGlobalPredicateLookupLimits {
    pub(crate) const fn new(
        max_structural_locus_comparisons: usize,
        max_product_decomposition_comparisons: usize,
        max_factor_locus_checks: usize,
    ) -> Self {
        Self {
            max_structural_locus_comparisons,
            max_product_decomposition_comparisons,
            max_factor_locus_checks,
        }
    }
}

/// Exact work performed while resolving one source predicate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalPredicateLookupStats {
    structural_locus_comparisons: usize,
    product_decomposition_comparisons: usize,
    factor_locus_checks: usize,
}

impl GeneratedAffineInitialGlobalPredicateLookupStats {
    pub(crate) const fn structural_locus_comparisons(self) -> usize {
        self.structural_locus_comparisons
    }

    pub(crate) const fn product_decomposition_comparisons(self) -> usize {
        self.product_decomposition_comparisons
    }

    pub(crate) const fn factor_locus_checks(self) -> usize {
        self.factor_locus_checks
    }
}

#[derive(Clone, Copy)]
enum GeneratedAffineInitialGlobalPredicateAtoms<'source> {
    Singleton(usize),
    CanonicalFactors(&'source [usize]),
}

/// One authenticated source predicate and only the canonical atoms derived
/// from that predicate.  Unrelated global structural loci remain unreachable.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalPredicateSourceView<'source> {
    predicate_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'source ParametricPolynomial,
    structural_locus_ordinal: usize,
    atoms: GeneratedAffineInitialGlobalPredicateAtoms<'source>,
    structural_loci: &'source [ParametricPolynomial],
    stats: GeneratedAffineInitialGlobalPredicateLookupStats,
}

impl<'source> GeneratedAffineInitialGlobalPredicateSourceView<'source> {
    pub(crate) const fn predicate_ordinal(self) -> usize {
        self.predicate_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn atom_count(self) -> usize {
        match self.atoms {
            GeneratedAffineInitialGlobalPredicateAtoms::Singleton(_) => 1,
            GeneratedAffineInitialGlobalPredicateAtoms::CanonicalFactors(factors) => factors.len(),
        }
    }

    pub(crate) fn atom_locus_ordinal(self, atom_position: usize) -> Option<usize> {
        match self.atoms {
            GeneratedAffineInitialGlobalPredicateAtoms::Singleton(ordinal) => {
                (atom_position == 0).then_some(ordinal)
            }
            GeneratedAffineInitialGlobalPredicateAtoms::CanonicalFactors(factors) => {
                factors.get(atom_position).copied()
            }
        }
    }

    /// Resolve only an atom authenticated through this exact source
    /// predicate's canonical decomposition.
    pub(crate) fn atom_polynomial(
        self,
        atom_position: usize,
    ) -> Option<&'source ParametricPolynomial> {
        self.structural_loci
            .get(self.atom_locus_ordinal(atom_position)?)
    }

    pub(crate) const fn stats(self) -> GeneratedAffineInitialGlobalPredicateLookupStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalPredicateSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalPredicateSourceView")
            .field("predicate_ordinal", &self.predicate_ordinal)
            .field("kind", &self.kind)
            .field("structural_locus_ordinal", &self.structural_locus_ordinal)
            .field("atom_count", &self.atom_count())
            .field("stats", &self.stats)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Nonempty initial source.  Its global locus tables are private navigation
/// authority; callers can resolve only one retained source predicate at a
/// time through an explicitly bounded operation.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalReadySourceView<'source> {
    terminal: GeneratedAffineInitialGlobalTerminalSourceView<'source>,
    source_queue: &'source Arc<GeneratedSectorLiveLeafQueueCertificate>,
    boolean_replay_session: Option<&'source ResidualProductLocusBooleanReplaySession<'source>>,
    source_predicates: &'source [SymbolicPolynomialPredicate],
    structural_loci: &'source [ParametricPolynomial],
    product_zero_decompositions: &'source [ParametricSectorProductZeroDecomposition],
}

impl<'source> GeneratedAffineInitialGlobalReadySourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineInitialGlobalTerminalSourceView<'source> {
        self.terminal
    }

    pub(crate) const fn source_predicate_count(self) -> usize {
        self.source_predicates.len()
    }

    pub(crate) fn authenticated_predicate_view(
        self,
        predicate_ordinal: usize,
        limits: GeneratedAffineInitialGlobalPredicateLookupLimits,
    ) -> Result<
        GeneratedAffineInitialGlobalPredicateSourceView<'source>,
        GeneratedAffineInitialGlobalPredicateSourceViewError,
    > {
        authenticated_initial_global_predicate_view(self, predicate_ordinal, limits)
    }

    /// Scalar census which must be aggregate-charged before the sealed V1
    /// child performs scope binding and predicate-to-locus comparisons.
    pub(crate) fn boolean_binding_census(
        self,
    ) -> Result<
        GeneratedAffineInitialGlobalBooleanBindingCensus,
        GeneratedAffineInitialGlobalBooleanCoverError,
    > {
        let coverage = self.source_queue.discovery().coverage();
        let partition = coverage.partition();
        let coverage_stats = coverage.stats();
        let structural_operand_factor = coverage.structural_loci().len().checked_add(1).ok_or(
            GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                resource: "structural polynomial equality operands",
            },
        )?;
        let structural_comparison_factor = self
            .source_predicates
            .len()
            .checked_mul(structural_operand_factor)
            .ok_or(
                GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                    resource: "structural polynomial equality comparisons",
                },
            )?;
        let structural_polynomial_equality_term_work = structural_comparison_factor
            .checked_mul(coverage_stats.retained_structural_locus_terms())
            .ok_or(
                GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                    resource: "structural polynomial equality term work",
                },
            )?;
        let structural_polynomial_equality_byte_work = structural_comparison_factor
            .checked_mul(coverage_stats.retained_structural_locus_bytes())
            .ok_or(
                GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                    resource: "structural polynomial equality byte work",
                },
            )?;
        let scope_operand_bytes = self
            .source_queue
            .family_fingerprint()
            .len()
            .checked_add(self.source_queue.context_fingerprint().len())
            .and_then(|value| value.checked_add(partition.context_fingerprint().len()))
            .ok_or(
                GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                    resource: "Boolean source scope comparison bytes",
                },
            )?;
        let scope_comparison_bytes = scope_operand_bytes.checked_mul(2).ok_or(
            GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                resource: "Boolean source scope comparison bytes",
            },
        )?;
        // Extraction-vs-coverage and queue-vs-discovery each compare two
        // immutable sector masks.
        let sector_entry_comparisons = self.source_queue.sector().arity().checked_mul(4).ok_or(
            GeneratedAffineInitialGlobalBooleanCoverError::ResourceCountOverflow {
                resource: "Boolean source sector entry comparisons",
            },
        )?;
        Ok(GeneratedAffineInitialGlobalBooleanBindingCensus {
            source_identity_pointer_comparisons: 1,
            source_identity_bytes: partition.source_identity().len(),
            scope_comparison_bytes,
            sector_entry_comparisons,
            structural_polynomial_equality_term_work,
            structural_polynomial_equality_byte_work,
        })
    }

    /// Compile the exact retained V1 Boolean child after the authority-wide
    /// caller has replayed the source once and charged `binding_census`.
    pub(crate) fn compile_boolean_cover_replayed(
        self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        binding_census: GeneratedAffineInitialGlobalBooleanBindingCensus,
        limits: ResidualProductLocusBooleanCoverLimits,
    ) -> Result<
        GeneratedAffineInitialGlobalBooleanCover,
        GeneratedAffineInitialGlobalBooleanCoverError,
    > {
        if self.boolean_binding_census()? != binding_census {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingCensusMismatch);
        }
        let work_item_ordinal = self.terminal.work_item_ordinal;
        let replay_session = self
            .boolean_replay_session
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::ReplaySessionRequired)?;
        if !replay_session.authenticates_queue(self.source_queue)
            || replay_session.family_fingerprint() != family.fingerprint_ref()
            || replay_session.context_fingerprint() != context.fingerprint()
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::ReplaySessionMismatch);
        }
        let compiled = replay_session
            .compile_replayed_at_case_position_with_census(
                work_item_ordinal,
                self.terminal.source_case_position,
                limits,
            )
            .map_err(GeneratedAffineInitialGlobalBooleanCoverError::V1Cover)?;
        let (
            cover,
            retained_owned_logical_bytes_upper_bound,
            compilation_owned_logical_peak_upper_bound,
        ) = compiled.into_parts();
        let retained_item = self
            .source_queue
            .work_items()
            .get(work_item_ordinal)
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch)?;
        if cover.source_work_item_ordinal() != work_item_ordinal
            || !Arc::ptr_eq(cover.source_queue(), self.source_queue)
            || !Arc::ptr_eq(cover.source_extraction(), retained_item.extraction_arc())
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch);
        }
        Ok(GeneratedAffineInitialGlobalBooleanCover {
            cover: Arc::new(cover),
            retained_owned_logical_bytes_upper_bound,
            compilation_owned_logical_peak_upper_bound,
        })
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalReadySourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalReadySourceView")
            .field("terminal", &self.terminal)
            .field("source_predicate_count", &self.source_predicates.len())
            .field("private_global_locus_authority", &"<redacted>")
            .finish()
    }
}

/// The two semantically distinct initial-global outcomes.  Empty leaves have
/// no predicate access at the type level.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedAffineInitialGlobalSourceView<'source> {
    CoordinateLeafProvedEmpty(GeneratedAffineInitialGlobalTerminalSourceView<'source>),
    ReadyForBooleanCover(GeneratedAffineInitialGlobalReadySourceView<'source>),
}

impl<'source> GeneratedAffineInitialGlobalSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineInitialGlobalTerminalSourceView<'source> {
        match self {
            Self::CoordinateLeafProvedEmpty(terminal) => terminal,
            Self::ReadyForBooleanCover(ready) => ready.terminal(),
        }
    }
}

/// Terminal semantics exposed by the sealed initial-global V1 Boolean cover.
///
/// Branching nodes never cross this seam.  The detailed contradiction witness
/// remains owned by the V1 certificate because the next affine inventory only
/// needs the exhaustive empty/ready distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineInitialGlobalBooleanTerminalOutcome {
    ProvedEmpty,
    ReadyForAffineRecognition,
}

/// Which exact terminal fact selects one authenticated atom reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineInitialGlobalBooleanAtomPolarity {
    EqualZero,
    NonZero,
}

/// One predicate-restricted atom borrowed from the exact sealed V1 cover.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalBooleanAtomSourceView<'cover> {
    locus_ordinal: usize,
    polynomial: &'cover ParametricPolynomial,
}

impl<'cover> GeneratedAffineInitialGlobalBooleanAtomSourceView<'cover> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn polynomial(self) -> &'cover ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanAtomSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalBooleanAtomSourceView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Lifetime-bound terminal summary from one sealed initial-global V1 cover.
///
/// Atom resolution is positional inside this exact terminal's equal-zero or
/// nonzero slice.  A caller cannot use an unrelated structural-locus ordinal
/// to access the private global table.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalBooleanTerminalSourceView<'cover> {
    ordinal: usize,
    equal_zero_atoms: &'cover [usize],
    nonzero_atoms: &'cover [usize],
    outcome: GeneratedAffineInitialGlobalBooleanTerminalOutcome,
    cover: &'cover ResidualProductLocusBooleanCoverCertificate,
}

impl<'cover> GeneratedAffineInitialGlobalBooleanTerminalSourceView<'cover> {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn outcome(self) -> GeneratedAffineInitialGlobalBooleanTerminalOutcome {
        self.outcome
    }

    pub(crate) const fn equal_zero_atom_count(self) -> usize {
        self.equal_zero_atoms.len()
    }

    pub(crate) const fn nonzero_atom_count(self) -> usize {
        self.nonzero_atoms.len()
    }

    /// Resolve an atom only by its position in this exact terminal fact slice.
    pub(crate) fn atom(
        self,
        polarity: GeneratedAffineInitialGlobalBooleanAtomPolarity,
        position: usize,
    ) -> Option<GeneratedAffineInitialGlobalBooleanAtomSourceView<'cover>> {
        let locus_ordinal = match polarity {
            GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero => {
                self.equal_zero_atoms.get(position).copied()?
            }
            GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero => {
                self.nonzero_atoms.get(position).copied()?
            }
        };
        let polynomial = self
            .cover
            .source_queue()
            .discovery()
            .coverage()
            .structural_locus(locus_ordinal)?;
        Some(GeneratedAffineInitialGlobalBooleanAtomSourceView {
            locus_ordinal,
            polynomial,
        })
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanTerminalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalBooleanTerminalSourceView")
            .field("ordinal", &self.ordinal)
            .field("equal_zero_atom_count", &self.equal_zero_atoms.len())
            .field("nonzero_atom_count", &self.nonzero_atoms.len())
            .field("outcome", &self.outcome)
            .field("private_cover", &"<redacted>")
            .finish()
    }
}

/// Sealed ownership of one actual V1 Boolean cover.
///
/// The raw V1 certificate is intentionally unreachable: its ordinary public
/// getters expose the source queue, extraction, and source-case identifiers.
/// Only narrow terminal summaries, scalar resource censes, and checked replay
/// comparison cross into the V2 source-neutral compiler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalBooleanPointLimits {
    pub(crate) specialization: GeneratedAffineResidualPointSpecializationLimits,
    pub(crate) max_context_comparison_bytes: usize,
    pub(crate) max_sector_index_scans: usize,
    pub(crate) max_node_scans: usize,
    pub(crate) max_terminal_scans: usize,
    pub(crate) max_ready_terminal_scans: usize,
    pub(crate) max_atom_scans: usize,
    pub(crate) max_atom_evaluations: usize,
}

impl Default for GeneratedAffineInitialGlobalBooleanPointLimits {
    fn default() -> Self {
        Self {
            specialization: GeneratedAffineResidualPointSpecializationLimits::default(),
            max_context_comparison_bytes: 64 * 1024 * 1024,
            max_sector_index_scans: 2_000_000,
            max_node_scans: 3_000_000_000,
            max_terminal_scans: 3_000_000_000,
            max_ready_terminal_scans: 3_000_000_000,
            max_atom_scans: 32_000_000_000usize,
            max_atom_evaluations: 16_000_000_000usize,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalBooleanPointStats {
    context_comparison_bytes: usize,
    sector_index_scans: usize,
    node_scans: usize,
    terminal_scans: usize,
    ready_terminal_scans: usize,
    atom_scans: usize,
    atom_evaluations: usize,
    specialization: GeneratedAffineResidualPointSpecializationStats,
}

impl GeneratedAffineInitialGlobalBooleanPointStats {
    pub(crate) const fn context_comparison_bytes(self) -> usize {
        self.context_comparison_bytes
    }
    pub(crate) const fn sector_index_scans(self) -> usize {
        self.sector_index_scans
    }
    pub(crate) const fn node_scans(self) -> usize {
        self.node_scans
    }
    pub(crate) const fn terminal_scans(self) -> usize {
        self.terminal_scans
    }
    pub(crate) const fn ready_terminal_scans(self) -> usize {
        self.ready_terminal_scans
    }
    pub(crate) const fn atom_scans(self) -> usize {
        self.atom_scans
    }
    pub(crate) const fn atom_evaluations(self) -> usize {
        self.atom_evaluations
    }
    pub(crate) const fn specialization(self) -> GeneratedAffineResidualPointSpecializationStats {
        self.specialization
    }
}

pub(crate) enum GeneratedAffineInitialGlobalBooleanPointError {
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Cover(ResidualProductLocusBooleanCoverError),
    Specialization(GeneratedAffineResidualSourcePointError),
    AuthorityMismatch,
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::Cover(_) => "Cover",
            Self::Specialization(_) => "Specialization",
            Self::AuthorityMismatch => "AuthorityMismatch",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedAffineInitialGlobalBooleanPointError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineInitialGlobalBooleanPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("initial Boolean point resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("initial Boolean point resource limit exceeded")
            }
            Self::Cover(_) => formatter.write_str("initial Boolean point evaluation failed"),
            Self::Specialization(_) => {
                formatter.write_str("initial Boolean point specialization preflight failed")
            }
            Self::AuthorityMismatch => {
                formatter.write_str("initial Boolean point authority mismatch")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during initial Boolean point evaluation")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineInitialGlobalBooleanPointError {}

pub(crate) struct GeneratedAffineInitialGlobalBooleanCover {
    cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl GeneratedAffineInitialGlobalBooleanCover {
    pub(crate) fn source_work_item_ordinal(&self) -> usize {
        self.cover.source_work_item_ordinal()
    }

    pub(crate) fn node_count(&self) -> usize {
        self.cover.nodes().len()
    }

    pub(crate) fn terminal_count(&self) -> usize {
        self.cover.stats().ready_terminals() + self.cover.stats().proved_empty_terminals()
    }

    /// Complete V1 scalar census.  Returning this copy cannot reveal the
    /// queue, extraction, source case, predicates, or structural loci.
    pub(crate) fn v1_stats(&self) -> ResidualProductLocusBooleanCoverStats {
        self.cover.stats()
    }

    /// Conservative logical retained-byte envelope for the sealed child Arc,
    /// excluding recursively shared queue/extraction payloads.
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(&self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    /// V2-path conservative child compilation peak, including raw root
    /// construction coordinates which canonical V1 statistics discard.
    pub(crate) const fn compilation_owned_logical_peak_upper_bound(&self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    /// Authenticate the adjacent retained scalar against a fresh census of
    /// the sealed raw V1 payload without exposing that payload.
    pub(crate) fn authenticated_retained_owned_logical_bytes_upper_bound(
        &self,
    ) -> Result<usize, GeneratedAffineInitialGlobalBooleanCoverError> {
        let recomputed = self
            .cover
            .recompute_retained_owned_logical_bytes_upper_bound()
            .map_err(GeneratedAffineInitialGlobalBooleanCoverError::V1Cover)?;
        if recomputed != self.retained_owned_logical_bytes_upper_bound {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingCensusMismatch);
        }
        Ok(recomputed)
    }

    /// Authenticate the V1 comparison census against the sealed raw payload.
    pub(crate) fn authenticated_v1_payload_comparison_census(
        &self,
    ) -> Result<(usize, usize), GeneratedAffineInitialGlobalBooleanCoverError> {
        let recomputed = self
            .cover
            .recompute_payload_comparison_census()
            .map_err(GeneratedAffineInitialGlobalBooleanCoverError::V1Cover)?;
        if recomputed
            != (
                self.cover.stats().payload_comparison_units(),
                self.cover.stats().payload_comparison_bytes(),
            )
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingCensusMismatch);
        }
        Ok(recomputed)
    }

    pub(crate) fn terminal_view(
        &self,
        node_ordinal: usize,
    ) -> Option<GeneratedAffineInitialGlobalBooleanTerminalSourceView<'_>> {
        let node = self.cover.nodes().get(node_ordinal)?;
        if node.ordinal() != node_ordinal || !node.is_terminal() {
            return None;
        }
        let outcome = match node.outcome() {
            ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(_) => {
                GeneratedAffineInitialGlobalBooleanTerminalOutcome::ProvedEmpty
            }
            ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition => {
                GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
            }
            ResidualProductLocusBooleanNodeOutcome::Branched { .. } => return None,
        };
        Some(GeneratedAffineInitialGlobalBooleanTerminalSourceView {
            ordinal: node_ordinal,
            equal_zero_atoms: node.equal_zero_atoms(),
            nonzero_atoms: node.nonzero_atoms(),
            outcome,
            cover: self.cover.as_ref(),
        })
    }

    pub(crate) fn terminal_views(
        &self,
    ) -> impl Iterator<Item = GeneratedAffineInitialGlobalBooleanTerminalSourceView<'_>> {
        self.cover
            .terminals()
            .filter_map(|node| self.terminal_view(node.ordinal()))
    }

    /// Resolve the unique nonempty Boolean terminal for one exact point while
    /// keeping the raw cover and its terminal nodes sealed.  The complete
    /// prospective terminal/atom census is admitted before Symbolica creates
    /// any specialization temporary.
    pub(crate) fn ready_terminal_ordinal_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedAffineInitialGlobalBooleanPointLimits,
    ) -> Result<
        (Option<usize>, GeneratedAffineInitialGlobalBooleanPointStats),
        GeneratedAffineInitialGlobalBooleanPointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let initial_context_comparison_bytes = initial_boolean_point_checked_add(
                "context comparison bytes",
                self.cover.context_fingerprint().len(),
                context.fingerprint().len(),
            )?;
            initial_boolean_point_check_limit(
                "context comparison bytes",
                initial_context_comparison_bytes,
                limits.max_context_comparison_bytes,
            )?;
            if self.cover.context_fingerprint() != context.fingerprint() {
                return Err(GeneratedAffineInitialGlobalBooleanPointError::Cover(
                    ResidualProductLocusBooleanCoverError::WrongContext,
                ));
            }
            let initial_sector_index_scan = indices.len();
            initial_boolean_point_check_limit(
                "sector index scans",
                initial_sector_index_scan,
                limits.max_sector_index_scans,
            )?;
            if !self
                .cover
                .sector()
                .contains_indices(indices)
                .map_err(|error| {
                    GeneratedAffineInitialGlobalBooleanPointError::Cover(error.into())
                })?
            {
                return Ok((
                    None,
                    GeneratedAffineInitialGlobalBooleanPointStats {
                        context_comparison_bytes: initial_context_comparison_bytes,
                        sector_index_scans: initial_sector_index_scan,
                        ..GeneratedAffineInitialGlobalBooleanPointStats::default()
                    },
                ));
            }
            let context_comparison_bytes = initial_boolean_point_checked_mul(
                "context comparison bytes",
                initial_context_comparison_bytes,
                2,
            )?;
            initial_boolean_point_check_limit(
                "context comparison bytes",
                context_comparison_bytes,
                limits.max_context_comparison_bytes,
            )?;
            let sector_index_scans = initial_boolean_point_checked_mul(
                "sector index scans",
                indices.len(),
                2,
            )?;
            initial_boolean_point_check_limit(
                "sector index scans",
                sector_index_scans,
                limits.max_sector_index_scans,
            )?;

            // First pass: authenticate and admit the complete actual node and
            // atom shape without trusting the adjacent V1 statistics.
            let nodes = self.cover.nodes();
            let node_scans = initial_boolean_point_checked_mul("node scans", nodes.len(), 3)?;
            initial_boolean_point_check_limit(
                "node scans",
                node_scans,
                limits.max_node_scans,
            )?;
            let mut terminal_count = 0usize;
            let mut ready_terminal_count = 0usize;
            let mut atom_evaluations = 0usize;
            for (node_ordinal, node) in nodes.iter().enumerate() {
                if node.ordinal() != node_ordinal {
                    return Err(GeneratedAffineInitialGlobalBooleanPointError::AuthorityMismatch);
                }
                if !node.is_terminal() {
                    continue;
                }
                terminal_count = initial_boolean_point_checked_add(
                    "terminal count",
                    terminal_count,
                    1,
                )?;
                if !matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                ) {
                    continue;
                }
                ready_terminal_count = initial_boolean_point_checked_add(
                    "ready terminal count",
                    ready_terminal_count,
                    1,
                )?;
                atom_evaluations = initial_boolean_point_checked_add(
                    "atom evaluations",
                    atom_evaluations,
                    node.equal_zero_atoms().len(),
                )?;
                atom_evaluations = initial_boolean_point_checked_add(
                    "atom evaluations",
                    atom_evaluations,
                    node.nonzero_atoms().len(),
                )?;
            }
            let terminal_scans =
                initial_boolean_point_checked_mul("terminal scans", terminal_count, 3)?;
            let ready_terminal_scans = initial_boolean_point_checked_mul(
                "ready terminal scans",
                ready_terminal_count,
                3,
            )?;
            let atom_scans =
                initial_boolean_point_checked_mul("atom scans", atom_evaluations, 2)?;
            initial_boolean_point_check_limit(
                "terminal scans",
                terminal_scans,
                limits.max_terminal_scans,
            )?;
            initial_boolean_point_check_limit(
                "ready terminal scans",
                ready_terminal_scans,
                limits.max_ready_terminal_scans,
            )?;
            initial_boolean_point_check_limit(
                "atom scans",
                atom_scans,
                limits.max_atom_scans,
            )?;
            initial_boolean_point_check_limit(
                "atom evaluations",
                atom_evaluations,
                limits.max_atom_evaluations,
            )?;

            // Second pass: preflight all legacy specializations.  Only after
            // the aggregate succeeds may the third, executing V1 pass run.
            let coverage = self.cover.source_queue().discovery().coverage();
            let arithmetic = self
                .cover
                .source_queue()
                .discovery()
                .limits()
                .coverage
                .generated_when_bad
                .when_bad
                .arithmetic;
            let mut specialization =
                GeneratedAffineResidualPointSpecializationStats::default();
            for node in nodes {
                if !node.is_terminal()
                    || !matches!(
                        node.outcome(),
                        ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                    )
                {
                    continue;
                }
                for &locus_ordinal in node
                    .equal_zero_atoms()
                    .iter()
                    .chain(node.nonzero_atoms())
                {
                    let polynomial = coverage.structural_locus(locus_ordinal).ok_or(
                        GeneratedAffineInitialGlobalBooleanPointError::AuthorityMismatch,
                    )?;
                    let preflight = context
                        .preflight_specialize_polynomial(polynomial, indices, arithmetic)
                        .map_err(|error| {
                            GeneratedAffineInitialGlobalBooleanPointError::Cover(error.into())
                        })?;
                    accumulate_residual_point_specialization(
                        &mut specialization,
                        preflight,
                        limits.specialization,
                    )
                    .map_err(GeneratedAffineInitialGlobalBooleanPointError::Specialization)?;
                }
            }
            let ordinal = self
                .cover
                .ready_terminal_for_indices(context, indices)
                .map_err(GeneratedAffineInitialGlobalBooleanPointError::Cover)?
                .map(|node| node.ordinal());
            if ordinal.is_some_and(|ordinal| {
                self.terminal_view(ordinal).is_none_or(|terminal| {
                    terminal.outcome()
                        != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
                })
            }) {
                return Err(GeneratedAffineInitialGlobalBooleanPointError::AuthorityMismatch);
            }
            Ok((
                ordinal,
                GeneratedAffineInitialGlobalBooleanPointStats {
                    context_comparison_bytes,
                    sector_index_scans,
                    node_scans,
                    terminal_scans,
                    ready_terminal_scans,
                    atom_scans,
                    atom_evaluations,
                    specialization,
                },
            ))
        }))
        .map_err(|_| GeneratedAffineInitialGlobalBooleanPointError::SymbolicaPanic)?
    }

    /// Compile one ready terminal from this exact retained cover without
    /// replaying that cover.  The raw V1 owner and fresh branch never cross
    /// this source-neutral adapter: the returned value is already sealed by
    /// the opaque initial-affine terminal owner.
    pub(crate) fn compile_ready_affine_terminal_replayed(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        branch_limits: ResidualAffineBranchSystemLimits,
        guard_limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineBoundTerminal,
        GeneratedAffineInitialGlobalBooleanCoverError,
    > {
        if self.source_work_item_ordinal() != source_work_item_ordinal {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch);
        }
        let terminal = self
            .terminal_view(local_terminal_ordinal)
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch)?;
        if terminal.outcome()
            != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::TerminalNotReady);
        }

        let exact_cover = Arc::clone(&self.cover);
        let fresh_branch = ResidualAffineBranchSystemCertificate::compile_fresh_replayed(
            family,
            context,
            Arc::clone(&exact_cover),
            local_terminal_ordinal,
            branch_limits,
        )
        .map_err(|_| GeneratedAffineInitialGlobalBooleanCoverError::AffineBranch)?;
        GeneratedAffineInitialGlobalAffineBoundTerminal::compile_and_bind(
            context,
            source_work_item_ordinal,
            local_terminal_ordinal,
            fresh_branch,
            guard_limits,
            &exact_cover,
            terminal.equal_zero_atoms,
            terminal.nonzero_atoms,
        )
        .map_err(|_| GeneratedAffineInitialGlobalBooleanCoverError::AffineTerminal)
    }

    /// Reauthenticate a previously sealed child against this exact private V1
    /// cover allocation.  Only a Boolean parent retaining the original cover
    /// can make this check succeed.
    pub(crate) fn authenticate_affine_terminal_allocation(
        &self,
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        terminal: &GeneratedAffineInitialGlobalAffineTerminal,
    ) -> Result<(), GeneratedAffineInitialGlobalBooleanCoverError> {
        let source_terminal = self
            .terminal_view(local_terminal_ordinal)
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch)?;
        if self.source_work_item_ordinal() != source_work_item_ordinal
            || terminal.source_work_item_ordinal() != source_work_item_ordinal
            || terminal.local_terminal_ordinal() != local_terminal_ordinal
            || source_terminal.outcome()
                != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch);
        }
        terminal
            .authenticate_source_cover_allocation_and_boolean_manifests(
                context,
                &self.cover,
                source_terminal.equal_zero_atoms,
                source_terminal.nonzero_atoms,
            )
            .map_err(|_| GeneratedAffineInitialGlobalBooleanCoverError::AffineTerminal)
    }

    /// Resolve the private Ready node and authenticate/project its opaque
    /// affine child in one traversal.  The exact V1 cover allocation and both
    /// ordered Boolean manifests remain private to this wrapper.
    pub(crate) fn authenticated_affine_terminal_source_view<'terminal>(
        &self,
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        terminal: &'terminal GeneratedAffineInitialGlobalAffineTerminal,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalSourceView<'terminal>,
        GeneratedAffineInitialGlobalBooleanCoverError,
    > {
        let source_terminal = self
            .terminal_view(local_terminal_ordinal)
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch)?;
        if self.source_work_item_ordinal() != source_work_item_ordinal
            || terminal.source_work_item_ordinal() != source_work_item_ordinal
            || terminal.local_terminal_ordinal() != local_terminal_ordinal
            || source_terminal.outcome()
                != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch);
        }
        terminal
            .authenticated_source_view_for_boolean_binding(
                context,
                &self.cover,
                source_terminal.equal_zero_atoms,
                source_terminal.nonzero_atoms,
            )
            .map_err(|_| GeneratedAffineInitialGlobalBooleanCoverError::AffineTerminal)
    }

    /// Bool-only checked comparison of two moved opaque affine children.  The
    /// exact raw V1 cover allocation remains private to this sealed wrapper;
    /// neither child nor caller can substitute an independently equal cover.
    pub(crate) fn authenticate_affine_terminal_pair_payload(
        &self,
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        left: &GeneratedAffineInitialGlobalAffineTerminal,
        right: &GeneratedAffineInitialGlobalAffineTerminal,
    ) -> Result<bool, GeneratedAffineInitialGlobalBooleanCoverError> {
        let source_terminal = self
            .terminal_view(local_terminal_ordinal)
            .ok_or(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch)?;
        if self.source_work_item_ordinal() != source_work_item_ordinal
            || left.source_work_item_ordinal() != source_work_item_ordinal
            || right.source_work_item_ordinal() != source_work_item_ordinal
            || left.local_terminal_ordinal() != local_terminal_ordinal
            || right.local_terminal_ordinal() != local_terminal_ordinal
            || source_terminal.outcome()
                != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingMismatch);
        }
        left.payload_eq_checked(
            right,
            context,
            &self.cover,
            source_terminal.equal_zero_atoms,
            source_terminal.nonzero_atoms,
        )
        .map_err(|_| GeneratedAffineInitialGlobalBooleanCoverError::AffineTerminal)
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedAffineInitialGlobalBooleanCoverError> {
        if self.retained_owned_logical_bytes_upper_bound
            != other.retained_owned_logical_bytes_upper_bound
            || self.compilation_owned_logical_peak_upper_bound
                != other.compilation_owned_logical_peak_upper_bound
        {
            return Ok(false);
        }
        self.cover
            .payload_eq_checked(other.cover.as_ref())
            .map_err(GeneratedAffineInitialGlobalBooleanCoverError::V1Cover)
    }

    #[cfg(test)]
    pub(crate) fn tamper_resource_census_for_test(&mut self) {
        self.retained_owned_logical_bytes_upper_bound = self
            .retained_owned_logical_bytes_upper_bound
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_compilation_peak_census_for_test(&mut self) {
        self.compilation_owned_logical_peak_upper_bound = self
            .compilation_owned_logical_peak_upper_bound
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_v1_payload_comparison_census_for_test(&mut self) {
        Arc::make_mut(&mut self.cover).tamper_payload_comparison_stats_for_test();
    }

    #[cfg(test)]
    pub(crate) fn tamper_v1_general_stats_for_test(&mut self) {
        Arc::make_mut(&mut self.cover).tamper_stats_for_test();
    }
}

fn initial_boolean_point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineInitialGlobalBooleanPointError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineInitialGlobalBooleanPointError::ResourceCountOverflow { resource })
}

fn initial_boolean_point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineInitialGlobalBooleanPointError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineInitialGlobalBooleanPointError::ResourceCountOverflow { resource })
}

fn initial_boolean_point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineInitialGlobalBooleanPointError> {
    if requested > limit {
        Err(
            GeneratedAffineInitialGlobalBooleanPointError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanCover {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalBooleanCover")
            .field("source_work_item_ordinal", &self.source_work_item_ordinal())
            .field("node_count", &self.node_count())
            .field("terminal_count", &self.terminal_count())
            .field("private_v1_cover", &"<redacted>")
            .finish()
    }
}

/// Conservative scalar charge for one positional no-replay V1 child.
///
/// The positional seam authenticates the already replayed immutable partition
/// by exact shared source-identity allocation rather than repeating derived
/// partition equality.  Scope/sector comparisons and the remaining V1
/// predicate-to-structural-locus equality work are still charged here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalBooleanBindingCensus {
    source_identity_pointer_comparisons: usize,
    source_identity_bytes: usize,
    scope_comparison_bytes: usize,
    sector_entry_comparisons: usize,
    structural_polynomial_equality_term_work: usize,
    structural_polynomial_equality_byte_work: usize,
}

impl GeneratedAffineInitialGlobalBooleanBindingCensus {
    pub(crate) const fn source_identity_pointer_comparisons(self) -> usize {
        self.source_identity_pointer_comparisons
    }

    pub(crate) const fn source_identity_bytes(self) -> usize {
        self.source_identity_bytes
    }

    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }

    pub(crate) const fn sector_entry_comparisons(self) -> usize {
        self.sector_entry_comparisons
    }

    pub(crate) const fn structural_polynomial_equality_term_work(self) -> usize {
        self.structural_polynomial_equality_term_work
    }

    pub(crate) const fn structural_polynomial_equality_byte_work(self) -> usize {
        self.structural_polynomial_equality_byte_work
    }
}

/// Source-neutral scalar identity for one terminal inherited from a prior
/// affine epoch.
///
/// The current-owner view also carries a V1 inventory locator and outcome.
/// Neither value is needed by the next epoch, so this projection deliberately
/// retains only the ordinal in the exact source authority.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorTerminalSourceView<'source> {
    work_item_ordinal: usize,
    lifetime: std::marker::PhantomData<&'source ()>,
}

impl GeneratedAffineResidualPriorTerminalSourceView<'_> {
    pub(crate) const fn work_item_ordinal(self) -> usize {
        self.work_item_ordinal
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorTerminalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorTerminalSourceView")
            .field("work_item_ordinal", &self.work_item_ordinal)
            .field("private_prior_terminal_authority", &"<redacted>")
            .finish()
    }
}

/// Which retained Boolean fact selects one positional unsupported-source atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualPriorAtomPolarity {
    EqualZero,
    NonZero,
}

const fn prior_atom_polarity_to_current_owner(
    polarity: GeneratedAffineResidualPriorAtomPolarity,
) -> GeneratedSectorAffineEffectiveResidualAtomPolarity {
    match polarity {
        GeneratedAffineResidualPriorAtomPolarity::EqualZero => {
            GeneratedSectorAffineEffectiveResidualAtomPolarity::EqualZero
        }
        GeneratedAffineResidualPriorAtomPolarity::NonZero => {
            GeneratedSectorAffineEffectiveResidualAtomPolarity::NonZero
        }
    }
}

/// One source-neutral atom borrowed through the exact prior authority.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorAtomSourceView<'source> {
    locus_ordinal: usize,
    polynomial: &'source ParametricPolynomial,
}

impl<'source> GeneratedAffineResidualPriorAtomSourceView<'source> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorAtomSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorAtomSourceView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Unsupported prior terminal projected without its V1 case locator, source
/// case, Boolean cover, branch certificate, or owning source allocation.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorUnsupportedSourceView<'source> {
    terminal: GeneratedAffineResidualPriorTerminalSourceView<'source>,
    // This current-owner view is sealed inside this module. Its broad methods
    // never cross the authority API; only positional atoms and reasons do.
    inner: GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'source>,
}

impl<'source> GeneratedAffineResidualPriorUnsupportedSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
        self.terminal
    }

    pub(crate) const fn atom_count(
        self,
        polarity: GeneratedAffineResidualPriorAtomPolarity,
    ) -> usize {
        self.inner
            .atom_count(prior_atom_polarity_to_current_owner(polarity))
    }

    pub(crate) fn atom(
        self,
        polarity: GeneratedAffineResidualPriorAtomPolarity,
        position: usize,
    ) -> Option<GeneratedAffineResidualPriorAtomSourceView<'source>> {
        let atom = self
            .inner
            .atom(prior_atom_polarity_to_current_owner(polarity), position)?;
        Some(GeneratedAffineResidualPriorAtomSourceView {
            locus_ordinal: atom.locus_ordinal(),
            polynomial: atom.polynomial(),
        })
    }

    pub(crate) const fn unsupported_reason_count(self) -> usize {
        self.inner.unsupported_reason_count()
    }

    pub(crate) fn unsupported_reason(
        self,
        position: usize,
    ) -> Option<&'source ResidualAffineBranchUnsupportedReason> {
        self.inner.unsupported_reason(position)
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorUnsupportedSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorUnsupportedSourceView")
            .field("terminal", &self.terminal)
            .field(
                "equal_zero_atom_count",
                &self.atom_count(GeneratedAffineResidualPriorAtomPolarity::EqualZero),
            )
            .field(
                "nonzero_atom_count",
                &self.atom_count(GeneratedAffineResidualPriorAtomPolarity::NonZero),
            )
            .field("unsupported_reason_count", &self.unsupported_reason_count())
            .field("private_prior_unsupported_authority", &"<redacted>")
            .finish()
    }
}

/// Source-neutral projection of a mapped prior guard class.
///
/// Base and free-index conditions expose only their exact polynomial. The
/// `ParametricNonZeroCondition` and its origin set remain sealed in the prior
/// owner graph.
#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualPriorGuardClassSourceView<'source> {
    Contradiction,
    DischargedNonzeroIntegerConstant,
    BaseAssumption {
        condition_polynomial: &'source ParametricPolynomial,
    },
    FreeIndexDependent {
        condition_polynomial: &'source ParametricPolynomial,
    },
}

impl<'source> GeneratedAffineResidualPriorGuardClassSourceView<'source> {
    pub(crate) const fn condition_polynomial(self) -> Option<&'source ParametricPolynomial> {
        match self {
            Self::BaseAssumption {
                condition_polynomial,
            }
            | Self::FreeIndexDependent {
                condition_polynomial,
            } => Some(condition_polynomial),
            Self::Contradiction | Self::DischargedNonzeroIntegerConstant => None,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorGuardClassSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Contradiction => "Contradiction",
            Self::DischargedNonzeroIntegerConstant => "DischargedNonzeroIntegerConstant",
            Self::BaseAssumption { .. } => "BaseAssumption",
            Self::FreeIndexDependent { .. } => "FreeIndexDependent",
        };
        formatter
            .debug_struct("GeneratedAffineResidualPriorGuardClassSourceView")
            .field("kind", &kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// One positional prior guard projected at the unified authority boundary.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorGuardSourceView<'source> {
    structural_locus_ordinal: usize,
    mapped_polynomial: &'source ParametricPolynomial,
    composition_stats: ResidualUnitAffinePolynomialCompositionStats,
    class: GeneratedAffineResidualPriorGuardClassSourceView<'source>,
}

impl<'source> GeneratedAffineResidualPriorGuardSourceView<'source> {
    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn mapped_polynomial(self) -> &'source ParametricPolynomial {
        self.mapped_polynomial
    }

    pub(crate) const fn composition_stats(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.composition_stats
    }

    pub(crate) const fn class(self) -> GeneratedAffineResidualPriorGuardClassSourceView<'source> {
        self.class
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorGuardSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorGuardSourceView")
            .field("structural_locus_ordinal", &self.structural_locus_ordinal)
            .field("composition_stats", &self.composition_stats)
            .field("class", &self.class)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

fn project_prior_guard_entry<'source>(
    entry: &'source ResidualAffineBranchGuardCompositionEntry,
) -> GeneratedAffineResidualPriorGuardSourceView<'source> {
    let class = match entry.class() {
        ResidualAffineBranchGuardCompositionClass::Contradiction => {
            GeneratedAffineResidualPriorGuardClassSourceView::Contradiction
        }
        ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant => {
            GeneratedAffineResidualPriorGuardClassSourceView::DischargedNonzeroIntegerConstant
        }
        ResidualAffineBranchGuardCompositionClass::BaseAssumption(condition) => {
            GeneratedAffineResidualPriorGuardClassSourceView::BaseAssumption {
                condition_polynomial: condition.polynomial(),
            }
        }
        ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(condition) => {
            GeneratedAffineResidualPriorGuardClassSourceView::FreeIndexDependent {
                condition_polynomial: condition.polynomial(),
            }
        }
    };
    GeneratedAffineResidualPriorGuardSourceView {
        structural_locus_ordinal: entry.structural_locus_ordinal(),
        mapped_polynomial: entry.mapped_polynomial(),
        composition_stats: entry.composition_stats(),
        class,
    }
}

/// Shared projected target payload. This carries no ordinary-actionable
/// binding and is therefore also sound for an exceptional child's consumed
/// target.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorTargetSourceView<'source> {
    terminal: GeneratedAffineResidualPriorTerminalSourceView<'source>,
    inner: GeneratedSectorAffineEffectiveResidualTargetSourceView<'source>,
}

impl<'source> GeneratedAffineResidualPriorTargetSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
        self.terminal
    }

    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        self.inner.affine_map()
    }

    pub(crate) const fn guard_entry_count(self) -> usize {
        self.inner.guard_entry_count()
    }

    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualPriorGuardSourceView<'source>> {
        self.inner
            .guard_entry(position)
            .map(project_prior_guard_entry)
    }

    pub(crate) const fn constant_count(self) -> usize {
        self.inner.constant_count()
    }

    pub(crate) fn constant(self, position: usize) -> Option<&'source Integer> {
        self.inner.constant(position)
    }

    pub(crate) const fn free_position_count(self) -> usize {
        self.inner.free_position_count()
    }

    pub(crate) fn free_position(self, position: usize) -> Option<usize> {
        self.inner.free_position(position)
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorTargetSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorTargetSourceView")
            .field("terminal", &self.terminal)
            .field("guard_entry_count", &self.guard_entry_count())
            .field("constant_count", &self.constant_count())
            .field("free_position_count", &self.free_position_count())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// One actionable prior target projected without old inventory/group/case
/// locators or raw guard-entry/class payloads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GeneratedAffineResidualPriorActionableBinding {
    Unprocessed,
    Unconsumed,
}

/// Opaque binding retained by the Boolean certificate so replay can prove
/// that a coalesced actionable view still denotes the same owner outcome.
/// The discriminator has no accessor and its Debug output is redacted.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualPriorActionableBindingSeal {
    binding: GeneratedAffineResidualPriorActionableBinding,
}

impl fmt::Debug for GeneratedAffineResidualPriorActionableBindingSeal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorActionableBindingSeal")
            .field("private_binding", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
impl GeneratedAffineResidualPriorActionableBindingSeal {
    pub(crate) fn tamper_for_test(&mut self) {
        self.binding = match self.binding {
            GeneratedAffineResidualPriorActionableBinding::Unprocessed => {
                GeneratedAffineResidualPriorActionableBinding::Unconsumed
            }
            GeneratedAffineResidualPriorActionableBinding::Unconsumed => {
                GeneratedAffineResidualPriorActionableBinding::Unprocessed
            }
        };
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorActionableSourceView<'source> {
    target: GeneratedAffineResidualPriorTargetSourceView<'source>,
    binding: GeneratedAffineResidualPriorActionableBindingSeal,
}

impl<'source> GeneratedAffineResidualPriorActionableSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
        self.target.terminal()
    }

    pub(crate) const fn binding_seal(self) -> GeneratedAffineResidualPriorActionableBindingSeal {
        self.binding
    }

    pub(crate) const fn target(self) -> GeneratedAffineResidualPriorTargetSourceView<'source> {
        self.target
    }

    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        self.target.affine_map()
    }

    pub(crate) const fn guard_entry_count(self) -> usize {
        self.target.guard_entry_count()
    }

    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualPriorGuardSourceView<'source>> {
        self.target.guard_entry(position)
    }

    pub(crate) const fn constant_count(self) -> usize {
        self.target.constant_count()
    }

    pub(crate) fn constant(self, position: usize) -> Option<&'source Integer> {
        self.target.constant(position)
    }

    pub(crate) const fn free_position_count(self) -> usize {
        self.target.free_position_count()
    }

    pub(crate) fn free_position(self, position: usize) -> Option<usize> {
        self.target.free_position(position)
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorActionableSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorActionableSourceView")
            .field("terminal", &self.terminal())
            .field("guard_entry_count", &self.guard_entry_count())
            .field("constant_count", &self.constant_count())
            .field("free_position_count", &self.free_position_count())
            .field("private_prior_actionable_authority", &"<redacted>")
            .finish()
    }
}

/// One exceptional predicate projected without its relative case or
/// partition.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorExceptionalPredicateSourceView<'source> {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'source ParametricPolynomial,
}

impl<'source> GeneratedAffineResidualPriorExceptionalPredicateSourceView<'source> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorExceptionalPredicateSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorExceptionalPredicateSourceView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("kind", &self.kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// One exceptional prior child. The inherited target and exact predicate
/// sequence are projected, while the relative case/partition and leak/domain
/// owner payload remain sealed.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualPriorExceptionalSourceView<'source> {
    target: GeneratedAffineResidualPriorTargetSourceView<'source>,
    inner: GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'source>,
}

impl<'source> GeneratedAffineResidualPriorExceptionalSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
        self.target.terminal()
    }

    pub(crate) const fn target(self) -> GeneratedAffineResidualPriorTargetSourceView<'source> {
        self.target
    }

    pub(crate) const fn predicate_count(self) -> usize {
        self.inner.predicate_count()
    }

    pub(crate) fn predicate(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualPriorExceptionalPredicateSourceView<'source>> {
        let predicate = self.inner.predicate(position)?;
        Some(GeneratedAffineResidualPriorExceptionalPredicateSourceView {
            locus_ordinal: predicate.locus_ordinal(),
            kind: predicate.kind(),
            polynomial: predicate.polynomial(),
        })
    }
}

impl fmt::Debug for GeneratedAffineResidualPriorExceptionalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualPriorExceptionalSourceView")
            .field("target", &self.target)
            .field("predicate_count", &self.predicate_count())
            .field("private_exceptional_authority", &"<redacted>")
            .finish()
    }
}

/// Lifetime-bound, source-neutral prior-effective source. The two actionable
/// owner dispositions are coalesced; their distinction survives only in the
/// opaque binding seal retained for Boolean replay.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedAffineResidualPriorSourceView<'source> {
    Unsupported(GeneratedAffineResidualPriorUnsupportedSourceView<'source>),
    Actionable(GeneratedAffineResidualPriorActionableSourceView<'source>),
    ExceptionalDomain(GeneratedAffineResidualPriorExceptionalSourceView<'source>),
    ExceptionalLeak(GeneratedAffineResidualPriorExceptionalSourceView<'source>),
}

impl<'source> GeneratedAffineResidualPriorSourceView<'source> {
    pub(crate) const fn terminal(self) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
        match self {
            Self::Unsupported(view) => view.terminal(),
            Self::Actionable(view) => view.terminal(),
            Self::ExceptionalDomain(view) | Self::ExceptionalLeak(view) => view.terminal(),
        }
    }
}

fn project_prior_terminal<'source>(
    terminal: GeneratedSectorAffineEffectiveResidualTerminalSourceView<'source>,
) -> GeneratedAffineResidualPriorTerminalSourceView<'source> {
    GeneratedAffineResidualPriorTerminalSourceView {
        work_item_ordinal: terminal.work_item_ordinal(),
        lifetime: std::marker::PhantomData,
    }
}

fn project_prior_target<'source>(
    inner: GeneratedSectorAffineEffectiveResidualTargetSourceView<'source>,
) -> GeneratedAffineResidualPriorTargetSourceView<'source> {
    GeneratedAffineResidualPriorTargetSourceView {
        terminal: project_prior_terminal(inner.terminal()),
        inner,
    }
}

fn project_prior_actionable<'source>(
    inner: GeneratedSectorAffineEffectiveResidualTargetSourceView<'source>,
    binding: GeneratedAffineResidualPriorActionableBinding,
) -> GeneratedAffineResidualPriorActionableSourceView<'source> {
    GeneratedAffineResidualPriorActionableSourceView {
        target: project_prior_target(inner),
        binding: GeneratedAffineResidualPriorActionableBindingSeal { binding },
    }
}

fn project_prior_exceptional<'source>(
    inner: GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'source>,
) -> GeneratedAffineResidualPriorExceptionalSourceView<'source> {
    GeneratedAffineResidualPriorExceptionalSourceView {
        target: project_prior_target(inner.target()),
        inner,
    }
}

fn project_prior_source_view<'source>(
    inner: GeneratedSectorAffineEffectiveResidualSourceView<'source>,
) -> GeneratedAffineResidualPriorSourceView<'source> {
    match inner {
        GeneratedSectorAffineEffectiveResidualSourceView::UnsupportedInventoryTerminal(inner) => {
            GeneratedAffineResidualPriorSourceView::Unsupported(
                GeneratedAffineResidualPriorUnsupportedSourceView {
                    terminal: project_prior_terminal(inner.terminal()),
                    inner,
                },
            )
        }
        GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(inner) => {
            GeneratedAffineResidualPriorSourceView::Actionable(project_prior_actionable(
                inner,
                GeneratedAffineResidualPriorActionableBinding::Unprocessed,
            ))
        }
        GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(inner) => {
            GeneratedAffineResidualPriorSourceView::Actionable(project_prior_actionable(
                inner,
                GeneratedAffineResidualPriorActionableBinding::Unconsumed,
            ))
        }
        GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalDomain(inner) => {
            GeneratedAffineResidualPriorSourceView::ExceptionalDomain(project_prior_exceptional(
                inner,
            ))
        }
        GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalLeak(inner) => {
            GeneratedAffineResidualPriorSourceView::ExceptionalLeak(project_prior_exceptional(
                inner,
            ))
        }
    }
}

/// Source-neutral lifetime-bound input for one generated affine epoch.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedAffineResidualSourceView<'source> {
    InitialGlobal(GeneratedAffineInitialGlobalSourceView<'source>),
    PriorEffective(GeneratedAffineResidualPriorSourceView<'source>),
}

impl GeneratedAffineResidualSourceView<'_> {
    pub(crate) const fn work_item_ordinal(self) -> usize {
        match self {
            Self::InitialGlobal(view) => view.terminal().work_item_ordinal(),
            Self::PriorEffective(view) => view.terminal().work_item_ordinal(),
        }
    }
}

/// One sealed, version-preserving source allocation for an affine epoch.
///
/// The concrete source variant is module-private. Sibling modules may create
/// an authority through the typed constructors and use its common operations,
/// but cannot pattern-match it to recover either raw source `Arc`. No source
/// fabricates a queue or copies predicates, affine maps, guards, or relations.
/// Cloning this authority clones only its single retained `Arc` handle.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualSourceAuthority {
    inner: GeneratedAffineResidualSourceAuthorityInner,
}

#[derive(Clone)]
enum GeneratedAffineResidualSourceAuthorityInner {
    InitialGlobal(Arc<GeneratedSectorLiveLeafQueueCertificate>),
    PriorEffective(Arc<GeneratedSectorAffineEffectiveResidualQueueCertificate>),
}

/// One successful replay of the exact source authority for a complete affine
/// compilation batch.
///
/// Initial-global sessions retain the unforgeable V1 queue-replay token needed
/// by positional child compilation.  Prior-effective sessions need no child
/// token because their residual leaves are already disjoint terminals.
pub(crate) struct GeneratedAffineResidualSourceReplaySession<'scope> {
    authority: &'scope GeneratedAffineResidualSourceAuthority,
    inner: GeneratedAffineResidualSourceReplaySessionInner<'scope>,
}

enum GeneratedAffineResidualSourceReplaySessionInner<'scope> {
    InitialGlobal(ResidualProductLocusBooleanReplaySession<'scope>),
    PriorEffective,
}

impl<'scope> GeneratedAffineResidualSourceReplaySession<'scope> {
    pub(crate) fn authenticated_source_view<'view>(
        &'view self,
        work_item_ordinal: usize,
    ) -> Result<GeneratedAffineResidualSourceView<'view>, GeneratedAffineResidualSourceViewError>
    where
        'scope: 'view,
    {
        match (&self.authority.inner, &self.inner) {
            (
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source),
                GeneratedAffineResidualSourceReplaySessionInner::InitialGlobal(replay),
            ) => authenticated_initial_global_source_view(source, Some(replay), work_item_ordinal)
                .map(GeneratedAffineResidualSourceView::InitialGlobal),
            (
                GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source),
                GeneratedAffineResidualSourceReplaySessionInner::PriorEffective,
            ) => source
                .authenticated_source_view(work_item_ordinal)
                .map(project_prior_source_view)
                .map(GeneratedAffineResidualSourceView::PriorEffective)
                .map_err(GeneratedAffineResidualSourceViewError::PriorEffective),
            _ => Err(GeneratedAffineResidualSourceViewError::ReplaySessionMismatch),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualSourceReplaySession<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualSourceReplaySession")
            .field("kind", &self.authority.kind())
            .field("private_replayed_source", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualSourceAuthority {
    pub(crate) fn initial_global(source: Arc<GeneratedSectorLiveLeafQueueCertificate>) -> Self {
        Self {
            inner: GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source),
        }
    }

    pub(crate) fn prior_effective(
        source: Arc<GeneratedSectorAffineEffectiveResidualQueueCertificate>,
    ) -> Self {
        Self {
            inner: GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source),
        }
    }

    pub(crate) const fn schema(&self) -> &'static str {
        GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA
    }

    pub(crate) const fn kind(&self) -> GeneratedAffineResidualSourceAuthorityKind {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(_) => {
                GeneratedAffineResidualSourceAuthorityKind::InitialGlobal
            }
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(_) => {
                GeneratedAffineResidualSourceAuthorityKind::PriorEffective
            }
        }
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.initial_scope().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.initial_scope().context_fingerprint()
    }

    pub(crate) fn sector(&self) -> &SectorMask {
        self.initial_scope().sector()
    }

    pub(crate) fn ordering(&self) -> IntegralOrderingPolicy {
        self.initial_scope().ordering()
    }

    pub(crate) fn arity(&self) -> usize {
        self.sector().arity()
    }

    /// Number of source-ordered residual items in this exact epoch input.
    pub(crate) fn len(&self) -> usize {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => {
                source.work_items().len()
            }
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => source.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Classify one concrete integer point through this exact retained source
    /// allocation.  Global rules and proved-empty leaves are outside the
    /// residual union.  A residual point is returned only after a complete
    /// uniqueness-checking scan binds it to one authenticated source item.
    pub(crate) fn classification_for_indices(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedAffineResidualSourcePointLimits,
    ) -> Result<
        GeneratedAffineResidualSourcePointClassification,
        GeneratedAffineResidualSourcePointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            classify_source_point_inner(self, family, context, indices, limits)
        }))
        .map_err(|_| GeneratedAffineResidualSourcePointError::SymbolicaPanic)?
    }

    /// Number of generated parametric source rows behind either source
    /// version.  Later epochs inherit the same exact initial row-span
    /// allocation through their owner graph.
    pub(crate) fn source_row_count(&self) -> usize {
        self.initial_scope().discovery().row_span().rows().len()
    }

    /// Lifetime-bound access to one certificate-owned source row.  No row-span
    /// `Arc` and no caller-supplied relation crosses this seam.
    pub(crate) fn source_row(&self, source_row_ordinal: usize) -> Option<&ParametricRelation> {
        self.initial_scope()
            .discovery()
            .row_span()
            .rows()
            .get(source_row_ordinal)
    }

    pub(crate) fn source_batch_navigation_census(
        &self,
    ) -> GeneratedAffineResidualSourceBatchNavigationCensus {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(_) => {
                GeneratedAffineResidualSourceBatchNavigationCensus::default()
            }
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => {
                let stats = source.stats();
                GeneratedAffineResidualSourceBatchNavigationCensus {
                    prior_authority_index_comparison_bound: stats
                        .authority_index_comparison_bound(),
                    prior_projection_payload_comparison_bound: stats
                        .projection_payload_comparison_bound(),
                }
            }
        }
    }

    /// Replay this exact source once and mint the unforgeable session used by
    /// an authority-wide Boolean compilation.
    pub(crate) fn replay_session<'scope>(
        &'scope self,
        family: &'scope IntegralFamily,
        context: &'scope ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualSourceReplaySession<'scope>,
        GeneratedAffineResidualSourceAuthorityError,
    > {
        let inner = match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => {
                GeneratedAffineResidualSourceReplaySessionInner::InitialGlobal(
                    ResidualProductLocusBooleanReplaySession::replay(family, context, source)
                        .map_err(GeneratedAffineResidualSourceAuthorityError::InitialGlobal)?,
                )
            }
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => {
                source
                    .replay(family, context)
                    .map_err(GeneratedAffineResidualSourceAuthorityError::PriorEffective)?;
                GeneratedAffineResidualSourceReplaySessionInner::PriorEffective
            }
        };
        Ok(GeneratedAffineResidualSourceReplaySession {
            authority: self,
            inner,
        })
    }

    /// Exact allocation identity without exposing either retained source.
    pub(crate) fn same_source_allocation(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(left),
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(right),
            ) => Arc::ptr_eq(left, right),
            (
                GeneratedAffineResidualSourceAuthorityInner::PriorEffective(left),
                GeneratedAffineResidualSourceAuthorityInner::PriorEffective(right),
            ) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    /// Resolve one source-ordered ordinal through the exact retained variant.
    ///
    /// This accepts no caller-created locator.  The returned references are
    /// tied to this authority borrow, and neither branch exposes its owning
    /// `Arc` or a broad source certificate.  Callers compiling a complete
    /// epoch must use [`Self::replay_session`] and resolve views through that
    /// session; this ordinary inspection path cannot mint a compilable
    /// initial-global Boolean view.  Resolution performs only local sealed-
    /// authority checks plus a logarithmic, exactly counted lookup in the
    /// frozen initial partition.
    pub(crate) fn authenticated_source_view(
        &self,
        work_item_ordinal: usize,
    ) -> Result<GeneratedAffineResidualSourceView<'_>, GeneratedAffineResidualSourceViewError> {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => {
                authenticated_initial_global_source_view(source, None, work_item_ordinal)
                    .map(GeneratedAffineResidualSourceView::InitialGlobal)
            }
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => source
                .authenticated_source_view(work_item_ordinal)
                .map(project_prior_source_view)
                .map(GeneratedAffineResidualSourceView::PriorEffective)
                .map_err(GeneratedAffineResidualSourceViewError::PriorEffective),
        }
    }

    /// Resolve one source item only after admitting the complete navigation
    /// envelope.  This is the point-query seam used by Boolean and inventory
    /// routing; it returns a lifetime-bound view and no owning source handle.
    pub(crate) fn authenticated_source_view_with_limits(
        &self,
        work_item_ordinal: usize,
        limits: GeneratedAffineResidualSourceNavigationLimits,
    ) -> Result<
        (
            GeneratedAffineResidualSourceView<'_>,
            GeneratedAffineResidualSourceNavigationStats,
        ),
        GeneratedAffineResidualSourcePointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let mut stats = GeneratedAffineResidualSourceNavigationStats {
                source_view_resolutions: 1,
                ..GeneratedAffineResidualSourceNavigationStats::default()
            };
            source_point_check_limit(
                "source view resolutions",
                stats.source_view_resolutions,
                limits.max_source_view_resolutions,
            )?;
            match &self.inner {
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => {
                    let item =
                        source
                            .work_items()
                            .get(work_item_ordinal)
                            .ok_or(GeneratedAffineResidualSourcePointError::SourceView(
                            GeneratedAffineResidualSourceViewError::InitialGlobalWorkItemOutOfRange,
                        ))?;
                    stats.initial_case_lookup_comparisons =
                        binary_source_case_lookup_comparison_bound(
                            source.discovery().coverage().partition().cases().len(),
                        );
                    stats.initial_disposition_candidate_comparisons =
                        match item.source_disposition() {
                            GeneratedSectorQueuedSourceDisposition::Uncovered => 0,
                            GeneratedSectorQueuedSourceDisposition::Unsupported {
                                candidate_ordinals,
                            } => candidate_ordinals.len(),
                        };
                    source_point_check_limit(
                        "initial source case lookup comparisons",
                        stats.initial_case_lookup_comparisons,
                        limits.max_initial_case_lookup_comparisons,
                    )?;
                    source_point_check_limit(
                        "initial source disposition candidate comparisons",
                        stats.initial_disposition_candidate_comparisons,
                        limits.max_initial_disposition_candidate_comparisons,
                    )?;
                }
                GeneratedAffineResidualSourceAuthorityInner::PriorEffective(_) => {
                    let census = self.source_batch_navigation_census();
                    stats.prior_authority_index_comparison_bound =
                        census.prior_authority_index_comparison_bound();
                    stats.prior_projection_payload_comparison_bound =
                        census.prior_projection_payload_comparison_bound();
                    source_point_check_limit(
                        "prior authority index comparison bound",
                        stats.prior_authority_index_comparison_bound,
                        limits.max_prior_authority_index_comparison_bound,
                    )?;
                    source_point_check_limit(
                        "prior projection payload comparison bound",
                        stats.prior_projection_payload_comparison_bound,
                        limits.max_prior_projection_payload_comparison_bound,
                    )?;
                }
            }
            let view = self
                .authenticated_source_view(work_item_ordinal)
                .map_err(GeneratedAffineResidualSourcePointError::SourceView)?;
            if view.work_item_ordinal() != work_item_ordinal {
                return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
            }
            if let GeneratedAffineResidualSourceView::InitialGlobal(initial) = view {
                let terminal = initial.terminal();
                if terminal.case_lookup_comparisons() > stats.initial_case_lookup_comparisons
                    || terminal.source_disposition_candidate_comparisons()
                        != stats.initial_disposition_candidate_comparisons
                {
                    return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
                }
            }
            Ok((view, stats))
        }))
        .map_err(|_| GeneratedAffineResidualSourcePointError::SymbolicaPanic)?
    }

    /// Replay the retained source allocation without reconstructing or
    /// converting it to the other source version.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualSourceAuthorityError> {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => source
                .replay(family, context)
                .map_err(GeneratedAffineResidualSourceAuthorityError::InitialGlobal),
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => source
                .replay(family, context)
                .map_err(GeneratedAffineResidualSourceAuthorityError::PriorEffective),
        }
    }

    /// Both variants inherit their family/context/sector/order scope from the
    /// exact initial global queue retained in their authority graph.
    fn initial_scope(&self) -> &GeneratedSectorLiveLeafQueueCertificate {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => source.as_ref(),
            GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => {
                source.owner().source_queue().as_ref()
            }
        }
    }
}

fn classify_source_point_inner(
    authority: &GeneratedAffineResidualSourceAuthority,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedAffineResidualSourcePointLimits,
) -> Result<GeneratedAffineResidualSourcePointClassification, GeneratedAffineResidualSourcePointError>
{
    if authority.schema() != GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA {
        return Err(GeneratedAffineResidualSourcePointError::SchemaMismatch);
    }
    let scope_comparison_bytes = source_point_checked_sum(
        "scope comparison bytes",
        [
            authority.family_fingerprint().len(),
            family.fingerprint().len(),
            authority.context_fingerprint().len(),
            context.fingerprint().len(),
        ],
    )?;
    source_point_check_limit(
        "scope comparison bytes",
        scope_comparison_bytes,
        limits.max_scope_comparison_bytes,
    )?;
    if authority.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedAffineResidualSourcePointError::WrongFamily);
    }
    if authority.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedAffineResidualSourcePointError::WrongContext);
    }
    source_point_check_limit("index entries", indices.len(), limits.max_index_entries)?;
    if indices.len() != authority.arity() || indices.len() != context.index_count() {
        return Err(GeneratedAffineResidualSourcePointError::WrongArity);
    }

    let mut stats = GeneratedAffineResidualSourcePointStats {
        kind: Some(authority.kind()),
        scope_comparison_bytes,
        index_entries: indices.len(),
        ..GeneratedAffineResidualSourcePointStats::default()
    };
    let disposition = match &authority.inner {
        GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => {
            let coverage = source.discovery().coverage();
            if source.schema() != GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA
                || coverage.schema() != PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA
            {
                return Err(GeneratedAffineResidualSourcePointError::SchemaMismatch);
            }
            // Reject an outside-orthant point before traversing the retained
            // partition or any polynomial payload.  The legacy classifier
            // repeats this scan during execution, so admit both passes now.
            let initial_orthant_scan = indices.len();
            source_point_check_limit(
                "initial orthant index scans",
                initial_orthant_scan,
                limits.max_initial_orthant_index_scans,
            )?;
            if !coverage
                .partition()
                .orthant()
                .contains_integer_point(indices)
                .map_err(|error| {
                    GeneratedAffineResidualSourcePointError::InitialCoverage(
                        ParametricSectorCoverageError::SectorCase(error),
                    )
                })?
            {
                stats.initial_orthant_index_scans = initial_orthant_scan;
                return Ok(GeneratedAffineResidualSourcePointClassification {
                    disposition: GeneratedAffineResidualSourcePointDisposition::Excluded,
                    stats,
                });
            }
            let orthant_index_scans =
                source_point_checked_mul("initial orthant index scans", indices.len(), 2)?;
            source_point_check_limit(
                "initial orthant index scans",
                orthant_index_scans,
                limits.max_initial_orthant_index_scans,
            )?;
            stats.initial_orthant_index_scans = orthant_index_scans;
            let cases = coverage.partition().cases();
            let classifications = coverage.classifications();
            // The point path visits every case once for the predicate census,
            // once for the allocation-free specialization preflight, and once
            // in the retained V1 classifier.  Admit all three complete passes
            // before the first case is inspected.
            let case_scans = source_point_checked_mul("initial case scans", cases.len(), 3)?;
            source_point_check_limit(
                "initial case scans",
                case_scans,
                limits.max_initial_case_scans,
            )?;
            let classification_scans =
                source_point_checked_mul("initial classification scans", classifications.len(), 2)?;
            source_point_check_limit(
                "initial classification scans",
                classification_scans,
                limits.max_initial_classification_scans,
            )?;
            if cases.len() != classifications.len() {
                return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
            }
            let predicate_evaluations = cases.iter().try_fold(0usize, |total, case| {
                source_point_checked_add(
                    "initial predicate evaluations",
                    total,
                    case.predicates().len(),
                )
            })?;
            source_point_check_limit(
                "initial predicate evaluations",
                predicate_evaluations,
                limits.max_initial_predicate_evaluations,
            )?;
            let predicate_scans =
                source_point_checked_mul("initial predicate scans", predicate_evaluations, 2)?;
            source_point_check_limit(
                "initial predicate scans",
                predicate_scans,
                limits.max_initial_predicate_scans,
            )?;

            // Complete the allocation-free arithmetic census for every
            // prospective execution before the legacy classifier creates its
            // first specialized polynomial.
            let arithmetic = source
                .discovery()
                .limits()
                .coverage
                .generated_when_bad
                .when_bad
                .arithmetic;
            let mut specialization = GeneratedAffineResidualPointSpecializationStats::default();
            for (case, classification) in cases.iter().zip(classifications) {
                if case.id() != classification.case() {
                    return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
                }
                for predicate in case.predicates() {
                    let preflight = context
                        .preflight_specialize_polynomial(
                            predicate.polynomial(),
                            indices,
                            arithmetic,
                        )
                        .map_err(|error| {
                            GeneratedAffineResidualSourcePointError::InitialCoverage(
                                ParametricSectorCoverageError::ParametricCoefficient(error),
                            )
                        })?;
                    accumulate_residual_point_specialization(
                        &mut specialization,
                        preflight,
                        limits.initial_specialization,
                    )?;
                }
            }
            stats.initial_case_scans = case_scans;
            stats.initial_classification_scans = classification_scans;
            stats.initial_predicate_scans = predicate_scans;
            stats.initial_predicate_evaluations = predicate_evaluations;
            stats.initial_specialization = specialization;
            let classification = coverage
                .classification_for_indices(context, indices)
                .map_err(GeneratedAffineResidualSourcePointError::InitialCoverage)?;
            let Some(classification) = classification else {
                return Ok(GeneratedAffineResidualSourcePointClassification {
                    disposition: GeneratedAffineResidualSourcePointDisposition::Excluded,
                    stats,
                });
            };
            match classification.disposition() {
                ParametricSectorLeafDisposition::DescendingRule { .. }
                | ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => {
                    GeneratedAffineResidualSourcePointDisposition::Excluded
                }
                global @ (ParametricSectorLeafDisposition::Uncovered
                | ParametricSectorLeafDisposition::Unsupported { .. }) => {
                    source_point_check_limit(
                        "initial work item scans",
                        source.work_items().len(),
                        limits.max_initial_work_item_scans,
                    )?;
                    stats.work_item_scans = source.work_items().len();
                    let mut matched = None;
                    let mut matches = 0usize;
                    for (ordinal, item) in source.work_items().iter().enumerate() {
                        if item.ordinal() != ordinal {
                            return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
                        }
                        if item.source_case() != classification.case() {
                            continue;
                        }
                        matches =
                            source_point_checked_add("initial work item matches", matches, 1)?;
                        matched = Some(ordinal);
                    }
                    if matches != 1 {
                        return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
                    }
                    let work_item_ordinal = matched
                        .ok_or(GeneratedAffineResidualSourcePointError::AuthorityMismatch)?;
                    let item = source
                        .work_items()
                        .get(work_item_ordinal)
                        .ok_or(GeneratedAffineResidualSourcePointError::AuthorityMismatch)?;
                    let prospective_candidate_comparisons =
                        prospective_sealed_initial_disposition_candidate_comparisons(
                            item.source_disposition(),
                            global,
                            source.limits().max_unsupported_candidate_references,
                        )
                        .ok_or(GeneratedAffineResidualSourcePointError::AuthorityMismatch)?;
                    stats.initial_disposition_candidate_comparisons = source_point_bounded_add(
                        "initial disposition candidate comparisons",
                        stats.initial_disposition_candidate_comparisons,
                        prospective_candidate_comparisons,
                        limits.max_initial_disposition_candidate_comparisons,
                    )?;
                    if authenticate_sealed_initial_disposition(
                        item.source_disposition(),
                        global,
                        source.limits().max_unsupported_candidate_references,
                    ) != Some(prospective_candidate_comparisons)
                    {
                        return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
                    }
                    GeneratedAffineResidualSourcePointDisposition::Work { work_item_ordinal }
                }
            }
        }
        GeneratedAffineResidualSourceAuthorityInner::PriorEffective(source) => {
            let classified = source
                .classification_for_indices(family, context, indices, limits.prior_effective)
                .map_err(GeneratedAffineResidualSourcePointError::PriorEffective)?;
            stats.prior_effective_owner = Some(classified.owner_stats());
            stats.work_item_scans = classified.work_item_scans();
            match classified.disposition() {
                GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Excluded => {
                    GeneratedAffineResidualSourcePointDisposition::Excluded
                }
                GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Work {
                    work_item_ordinal,
                    ..
                } => GeneratedAffineResidualSourcePointDisposition::Work { work_item_ordinal },
            }
        }
    };
    Ok(GeneratedAffineResidualSourcePointClassification { disposition, stats })
}

fn source_point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualSourcePointError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualSourcePointError::ResourceCountOverflow { resource })
}

fn source_point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualSourcePointError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualSourcePointError::ResourceCountOverflow { resource })
}

fn source_point_checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualSourcePointError> {
    values.into_iter().try_fold(0usize, |total, value| {
        source_point_checked_add(resource, total, value)
    })
}

fn source_point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualSourcePointError> {
    if requested > limit {
        Err(GeneratedAffineResidualSourcePointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

const RESIDUAL_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER: usize = 8;
const RESIDUAL_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER: usize = 10;

pub(crate) fn accumulate_residual_point_specialization(
    stats: &mut GeneratedAffineResidualPointSpecializationStats,
    preflight: crate::parametric_coefficient::ParametricPolynomialSpecializationPreflight,
    limits: GeneratedAffineResidualPointSpecializationLimits,
) -> Result<(), GeneratedAffineResidualSourcePointError> {
    stats.source_terms = source_point_bounded_add(
        "point specialization source terms",
        stats.source_terms,
        preflight.source_terms(),
        limits.max_source_terms,
    )?;
    stats.source_exponent_entries = source_point_bounded_add(
        "point specialization source exponent entries",
        stats.source_exponent_entries,
        preflight.source_exponent_entries(),
        limits.max_source_exponent_entries,
    )?;
    stats.preflight_validation_source_term_scan_bound = source_point_bounded_add(
        "point preflight/validation source-term scan bound",
        stats.preflight_validation_source_term_scan_bound,
        source_point_checked_mul(
            "point preflight/validation source-term scan bound",
            preflight.source_terms(),
            RESIDUAL_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER,
        )?,
        limits.max_preflight_validation_source_term_scan_bound,
    )?;
    stats.preflight_validation_source_exponent_entry_scan_bound = source_point_bounded_add(
        "point preflight/validation source exponent-entry scan bound",
        stats.preflight_validation_source_exponent_entry_scan_bound,
        source_point_checked_mul(
            "point preflight/validation source exponent-entry scan bound",
            preflight.source_exponent_entries(),
            RESIDUAL_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER,
        )?,
        limits.max_preflight_validation_source_exponent_entry_scan_bound,
    )?;
    stats.output_term_bound = source_point_bounded_add(
        "point specialization output term bound",
        stats.output_term_bound,
        preflight.output_term_bound(),
        limits.max_output_term_bound,
    )?;
    stats.output_exponent_entry_bound = source_point_bounded_add(
        "point specialization output exponent-entry bound",
        stats.output_exponent_entry_bound,
        preflight.output_exponent_entry_bound(),
        limits.max_output_exponent_entry_bound,
    )?;
    stats.power_operation_bound = source_point_bounded_add(
        "point specialization power-operation bound",
        stats.power_operation_bound,
        preflight.power_operation_bound(),
        limits.max_power_operation_bound,
    )?;
    stats.largest_output_integer_bit_bound = stats
        .largest_output_integer_bit_bound
        .max(preflight.largest_output_integer_bit_bound());
    source_point_check_limit(
        "point specialization largest output integer-bit bound",
        stats.largest_output_integer_bit_bound,
        limits.max_largest_output_integer_bit_bound,
    )?;
    stats.integer_bit_work_bound = source_point_bounded_add(
        "point specialization integer-bit work bound",
        stats.integer_bit_work_bound,
        preflight.integer_bit_work_bound(),
        limits.max_integer_bit_work_bound,
    )?;
    stats.retained_output_term_bound = source_point_bounded_add(
        "point specialization retained output-term bound",
        stats.retained_output_term_bound,
        preflight.retained_output_term_bound(),
        limits.max_retained_output_term_bound,
    )?;
    stats.retained_output_byte_bound = source_point_bounded_add(
        "point specialization retained output-byte bound",
        stats.retained_output_byte_bound,
        preflight.retained_output_byte_bound(),
        limits.max_retained_output_byte_bound,
    )?;
    Ok(())
}

fn source_point_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualSourcePointError> {
    let requested = source_point_checked_add(resource, left, right)?;
    source_point_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn authenticated_initial_global_source_view<'source>(
    source: &'source Arc<GeneratedSectorLiveLeafQueueCertificate>,
    boolean_replay_session: Option<&'source ResidualProductLocusBooleanReplaySession<'source>>,
    work_item_ordinal: usize,
) -> Result<GeneratedAffineInitialGlobalSourceView<'source>, GeneratedAffineResidualSourceViewError>
{
    if boolean_replay_session.is_some_and(|replay| !replay.authenticates_queue(source)) {
        return Err(GeneratedAffineResidualSourceViewError::ReplaySessionMismatch);
    }
    let discovery = source.discovery();
    let coverage = discovery.coverage();
    let partition = coverage.partition();
    if source.schema() != GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA
        || coverage.schema() != PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA
        || partition.schema() != SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA
    {
        return Err(GeneratedAffineResidualSourceViewError::InitialGlobalSchemaMismatch);
    }

    let item = source
        .work_items()
        .get(work_item_ordinal)
        .ok_or(GeneratedAffineResidualSourceViewError::InitialGlobalWorkItemOutOfRange)?;
    if item.ordinal() != work_item_ordinal
        || item.extraction().schema() != COORDINATE_EQUALITY_LOCUS_V1_SCHEMA
        || item.extraction().source_partition().schema() != SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA
        || item.extraction().source_case() != item.source_case()
        || !Arc::ptr_eq(
            item.extraction().source_partition().source_identity(),
            partition.source_identity(),
        )
        || partition.cases().len() != coverage.classifications().len()
    {
        return Err(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch);
    }

    let (source_case_position, case_lookup_comparisons) =
        binary_source_case_position(partition.cases(), item.source_case())
            .ok_or(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch)?;
    let source_case = partition
        .cases()
        .get(source_case_position)
        .ok_or(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch)?;
    let classification = coverage
        .classifications()
        .get(source_case_position)
        .ok_or(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch)?;
    let Some(source_disposition_candidate_comparisons) = authenticate_sealed_initial_disposition(
        item.source_disposition(),
        classification.disposition(),
        source.limits().max_unsupported_candidate_references,
    ) else {
        return Err(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch);
    };
    if source_case.id() != item.source_case() || classification.case() != item.source_case() {
        return Err(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch);
    }

    let terminal = GeneratedAffineInitialGlobalTerminalSourceView {
        work_item_ordinal,
        source_case_position,
        source_identity_bytes: partition.source_identity().len(),
        case_lookup_comparisons,
        source_disposition_candidate_comparisons,
        lifetime: std::marker::PhantomData,
    };
    match authenticate_initial_global_semantic_outcome(item.outcome(), item.extraction().status())
        .ok_or(GeneratedAffineResidualSourceViewError::InitialGlobalAuthorityMismatch)?
    {
        GeneratedAffineInitialGlobalSemanticOutcome::CoordinateLeafProvedEmpty => {
            Ok(GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(terminal))
        }
        GeneratedAffineInitialGlobalSemanticOutcome::ReadyForBooleanCover => Ok(
            GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(
                GeneratedAffineInitialGlobalReadySourceView {
                    terminal,
                    source_queue: source,
                    boolean_replay_session,
                    source_predicates: source_case.predicates(),
                    structural_loci: coverage.structural_loci(),
                    product_zero_decompositions: coverage.product_zero_decompositions(),
                },
            ),
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedAffineInitialGlobalSemanticOutcome {
    CoordinateLeafProvedEmpty,
    ReadyForBooleanCover,
}

fn authenticate_initial_global_semantic_outcome(
    outcome: &GeneratedSectorLiveLeafOutcome,
    status: &CoordinateEqualityLeafStatus,
) -> Option<GeneratedAffineInitialGlobalSemanticOutcome> {
    match (outcome, status) {
        (
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
            CoordinateEqualityLeafStatus::ProvedEmpty(_),
        ) => Some(GeneratedAffineInitialGlobalSemanticOutcome::CoordinateLeafProvedEmpty),
        (
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment
            | GeneratedSectorLiveLeafOutcome::PartialReelimination { .. }
            | GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary { .. },
            CoordinateEqualityLeafStatus::NotProvedEmpty,
        ) => Some(GeneratedAffineInitialGlobalSemanticOutcome::ReadyForBooleanCover),
        _ => None,
    }
}

/// Cases are frozen in increasing stable-id order.  The comparison count is
/// returned with the view rather than hidden in an accessor; its logarithmic
/// bound follows from binary search over a `usize`-bounded slice.
fn binary_source_case_position(
    cases: &[SymbolicSectorCase],
    source_case: SymbolicSectorCaseId,
) -> Option<(usize, usize)> {
    let mut low = 0usize;
    let mut high = cases.len();
    let mut comparisons = 0usize;
    while low < high {
        let middle = low + (high - low) / 2;
        comparisons += 1;
        match cases[middle].id().cmp(&source_case) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Some((middle, comparisons)),
        }
    }
    None
}

/// Worst-case successful comparison count for the binary search above.
/// Computing this ceiling is allocation-free and does not inspect case IDs.
fn binary_source_case_lookup_comparison_bound(mut case_count: usize) -> usize {
    let mut comparisons = 0usize;
    while case_count != 0 {
        comparisons += 1;
        case_count /= 2;
    }
    comparisons
}

/// Allocation-free prospective count for the only linear part of initial
/// disposition authentication.  The retained queue ceiling is checked here so
/// callers can admit the point-local count before comparing candidate entries.
fn prospective_sealed_initial_disposition_candidate_comparisons(
    queued: &GeneratedSectorQueuedSourceDisposition,
    classified: &ParametricSectorLeafDisposition,
    max_candidate_comparisons: usize,
) -> Option<usize> {
    match (queued, classified) {
        (
            GeneratedSectorQueuedSourceDisposition::Uncovered,
            ParametricSectorLeafDisposition::Uncovered,
        ) => Some(0),
        (
            GeneratedSectorQueuedSourceDisposition::Unsupported {
                candidate_ordinals: queued,
            },
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals: classified,
            },
        ) if queued.len() == classified.len() && queued.len() <= max_candidate_comparisons => {
            Some(queued.len())
        }
        _ => None,
    }
}

fn authenticate_sealed_initial_disposition(
    queued: &GeneratedSectorQueuedSourceDisposition,
    classified: &ParametricSectorLeafDisposition,
    max_candidate_comparisons: usize,
) -> Option<usize> {
    let comparisons = prospective_sealed_initial_disposition_candidate_comparisons(
        queued,
        classified,
        max_candidate_comparisons,
    )?;
    match (queued, classified) {
        (
            GeneratedSectorQueuedSourceDisposition::Uncovered,
            ParametricSectorLeafDisposition::Uncovered,
        ) => Some(0),
        (
            GeneratedSectorQueuedSourceDisposition::Unsupported {
                candidate_ordinals: queued,
            },
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals: classified,
            },
        ) => queued
            .iter()
            .zip(classified.iter())
            .all(|(&queued, &classified)| queued == classified)
            .then_some(comparisons),
        _ => None,
    }
}

fn authenticated_initial_global_predicate_view<'source>(
    source: GeneratedAffineInitialGlobalReadySourceView<'source>,
    predicate_ordinal: usize,
    limits: GeneratedAffineInitialGlobalPredicateLookupLimits,
) -> Result<
    GeneratedAffineInitialGlobalPredicateSourceView<'source>,
    GeneratedAffineInitialGlobalPredicateSourceViewError,
> {
    let predicate = source
        .source_predicates
        .get(predicate_ordinal)
        .ok_or(GeneratedAffineInitialGlobalPredicateSourceViewError::PredicateOutOfRange)?;
    let mut stats = GeneratedAffineInitialGlobalPredicateLookupStats::default();
    let mut structural_locus_ordinal = None;
    for (ordinal, retained) in source.structural_loci.iter().enumerate() {
        if stats.structural_locus_comparisons >= limits.max_structural_locus_comparisons {
            return Err(
                GeneratedAffineInitialGlobalPredicateSourceViewError::StructuralLocusComparisonLimit,
            );
        }
        stats.structural_locus_comparisons += 1;
        if retained == predicate.polynomial() {
            structural_locus_ordinal = Some(ordinal);
            break;
        }
    }
    let structural_locus_ordinal = structural_locus_ordinal
        .ok_or(GeneratedAffineInitialGlobalPredicateSourceViewError::StructuralLocusNotFound)?;

    let decomposition = bounded_canonical_product_decomposition(
        source.product_zero_decompositions,
        structural_locus_ordinal,
        &mut stats,
        limits,
    )?;
    let atoms = if let Some(decomposition) = decomposition {
        let factors = decomposition.factor_locus_ordinals();
        if factors.len() < 2 {
            return Err(GeneratedAffineInitialGlobalPredicateSourceViewError::MalformedAuthority);
        }
        let mut previous = None;
        for &factor in factors {
            if stats.factor_locus_checks >= limits.max_factor_locus_checks {
                return Err(
                    GeneratedAffineInitialGlobalPredicateSourceViewError::FactorLocusCheckLimit,
                );
            }
            stats.factor_locus_checks += 1;
            if factor >= source.structural_loci.len()
                || previous.is_some_and(|previous| previous >= factor)
            {
                return Err(
                    GeneratedAffineInitialGlobalPredicateSourceViewError::MalformedAuthority,
                );
            }
            previous = Some(factor);
        }
        GeneratedAffineInitialGlobalPredicateAtoms::CanonicalFactors(factors)
    } else {
        GeneratedAffineInitialGlobalPredicateAtoms::Singleton(structural_locus_ordinal)
    };
    Ok(GeneratedAffineInitialGlobalPredicateSourceView {
        predicate_ordinal,
        kind: predicate.kind(),
        polynomial: predicate.polynomial(),
        structural_locus_ordinal,
        atoms,
        structural_loci: source.structural_loci,
        stats,
    })
}

fn bounded_canonical_product_decomposition<'source>(
    decompositions: &'source [ParametricSectorProductZeroDecomposition],
    product_locus_ordinal: usize,
    stats: &mut GeneratedAffineInitialGlobalPredicateLookupStats,
    limits: GeneratedAffineInitialGlobalPredicateLookupLimits,
) -> Result<
    Option<&'source ParametricSectorProductZeroDecomposition>,
    GeneratedAffineInitialGlobalPredicateSourceViewError,
> {
    let mut low = 0usize;
    let mut high = decompositions.len();
    while low < high {
        if stats.product_decomposition_comparisons >= limits.max_product_decomposition_comparisons {
            return Err(
                GeneratedAffineInitialGlobalPredicateSourceViewError::ProductDecompositionComparisonLimit,
            );
        }
        stats.product_decomposition_comparisons += 1;
        let middle = low + (high - low) / 2;
        if decompositions[middle].product_locus_ordinal() < product_locus_ordinal {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    let Some(candidate) = decompositions.get(low) else {
        return Ok(None);
    };
    if stats.product_decomposition_comparisons >= limits.max_product_decomposition_comparisons {
        return Err(
            GeneratedAffineInitialGlobalPredicateSourceViewError::ProductDecompositionComparisonLimit,
        );
    }
    stats.product_decomposition_comparisons += 1;
    Ok((candidate.product_locus_ordinal() == product_locus_ordinal).then_some(candidate))
}

/// Redacted authentication failure at the source-neutral dispatch seam.
pub(crate) enum GeneratedAffineResidualSourceViewError {
    InitialGlobalSchemaMismatch,
    InitialGlobalWorkItemOutOfRange,
    InitialGlobalAuthorityMismatch,
    ReplaySessionMismatch,
    PriorEffective(GeneratedSectorAffineEffectiveResidualSourceViewError),
}

impl fmt::Debug for GeneratedAffineResidualSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::InitialGlobalSchemaMismatch => "InitialGlobalSchemaMismatch",
            Self::InitialGlobalWorkItemOutOfRange => "InitialGlobalWorkItemOutOfRange",
            Self::InitialGlobalAuthorityMismatch => "InitialGlobalAuthorityMismatch",
            Self::ReplaySessionMismatch => "ReplaySessionMismatch",
            Self::PriorEffective(_) => "PriorEffective",
        };
        formatter
            .debug_struct("GeneratedAffineResidualSourceViewError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialGlobalSchemaMismatch => {
                formatter.write_str("initial global residual source schema mismatch")
            }
            Self::InitialGlobalWorkItemOutOfRange => {
                formatter.write_str("initial global residual source item is out of range")
            }
            Self::InitialGlobalAuthorityMismatch => {
                formatter.write_str("initial global residual source authority mismatch")
            }
            Self::ReplaySessionMismatch => {
                formatter.write_str("residual source replay-session authority mismatch")
            }
            Self::PriorEffective(_) => {
                formatter.write_str("prior effective residual source authentication failed")
            }
        }
    }
}

// Deliberately do not delegate `Error::source`: neither nested proof payloads
// nor lower-layer authentication diagnostics cross this privacy boundary.
impl std::error::Error for GeneratedAffineResidualSourceViewError {}

/// Redacted, bounded lookup failure for one initial source predicate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineInitialGlobalPredicateSourceViewError {
    PredicateOutOfRange,
    StructuralLocusComparisonLimit,
    StructuralLocusNotFound,
    ProductDecompositionComparisonLimit,
    FactorLocusCheckLimit,
    MalformedAuthority,
}

impl fmt::Debug for GeneratedAffineInitialGlobalPredicateSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::PredicateOutOfRange => "PredicateOutOfRange",
            Self::StructuralLocusComparisonLimit => "StructuralLocusComparisonLimit",
            Self::StructuralLocusNotFound => "StructuralLocusNotFound",
            Self::ProductDecompositionComparisonLimit => "ProductDecompositionComparisonLimit",
            Self::FactorLocusCheckLimit => "FactorLocusCheckLimit",
            Self::MalformedAuthority => "MalformedAuthority",
        };
        formatter
            .debug_struct("GeneratedAffineInitialGlobalPredicateSourceViewError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineInitialGlobalPredicateSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PredicateOutOfRange => {
                formatter.write_str("initial global source predicate is out of range")
            }
            Self::StructuralLocusComparisonLimit => {
                formatter.write_str("initial global structural-locus comparison limit exceeded")
            }
            Self::StructuralLocusNotFound => {
                formatter.write_str("initial global source predicate locus was not found")
            }
            Self::ProductDecompositionComparisonLimit => formatter
                .write_str("initial global product-decomposition comparison limit exceeded"),
            Self::FactorLocusCheckLimit => {
                formatter.write_str("initial global factor-locus check limit exceeded")
            }
            Self::MalformedAuthority => {
                formatter.write_str("initial global predicate authority is malformed")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineInitialGlobalPredicateSourceViewError {}

/// Redacted failure while sealing one actual initial-global V1 Boolean cover.
pub(crate) enum GeneratedAffineInitialGlobalBooleanCoverError {
    WrongSourceKind,
    SourceProvedEmpty,
    BindingCensusMismatch,
    BindingMismatch,
    ReplaySessionRequired,
    ReplaySessionMismatch,
    TerminalNotReady,
    AffineBranch,
    AffineTerminal,
    ResourceCountOverflow { resource: &'static str },
    SourceView(GeneratedAffineResidualSourceViewError),
    V1Cover(ResidualProductLocusBooleanCoverError),
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::WrongSourceKind => "WrongSourceKind",
            Self::SourceProvedEmpty => "SourceProvedEmpty",
            Self::BindingCensusMismatch => "BindingCensusMismatch",
            Self::BindingMismatch => "BindingMismatch",
            Self::ReplaySessionRequired => "ReplaySessionRequired",
            Self::ReplaySessionMismatch => "ReplaySessionMismatch",
            Self::TerminalNotReady => "TerminalNotReady",
            Self::AffineBranch => "AffineBranch",
            Self::AffineTerminal => "AffineTerminal",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::SourceView(_) => "SourceView",
            Self::V1Cover(_) => "V1Cover",
        };
        formatter
            .debug_struct("GeneratedAffineInitialGlobalBooleanCoverError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineInitialGlobalBooleanCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceKind => {
                formatter.write_str("initial-global Boolean cover requested from another source")
            }
            Self::SourceProvedEmpty => {
                formatter.write_str("initial-global Boolean source is already proved empty")
            }
            Self::BindingCensusMismatch | Self::BindingMismatch => {
                formatter.write_str("initial-global Boolean source binding mismatch")
            }
            Self::ReplaySessionRequired => {
                formatter.write_str("initial-global Boolean replay session is required")
            }
            Self::ReplaySessionMismatch => {
                formatter.write_str("initial-global Boolean replay session mismatch")
            }
            Self::TerminalNotReady => formatter
                .write_str("initial-global Boolean terminal is not ready for affine recognition"),
            Self::AffineBranch => formatter
                .write_str("initial-global source-neutral affine branch compilation failed"),
            Self::AffineTerminal => {
                formatter.write_str("initial-global opaque affine terminal authentication failed")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("initial-global Boolean source resource count overflow")
            }
            Self::SourceView(_) => {
                formatter.write_str("initial-global Boolean source authentication failed")
            }
            Self::V1Cover(_) => {
                formatter.write_str("initial-global sealed V1 Boolean compilation failed")
            }
        }
    }
}

// Deliberately redact nested V1/source diagnostics at the sealed bridge.
impl std::error::Error for GeneratedAffineInitialGlobalBooleanCoverError {}

impl fmt::Debug for GeneratedAffineResidualSourceAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualSourceAuthority")
            .field("schema", &self.schema())
            .field("kind", &self.kind())
            .field("arity", &self.arity())
            .field("residual_source_count", &self.len())
            .field("private_source", &"<redacted>")
            .finish()
    }
}

pub(crate) enum GeneratedAffineResidualSourceAuthorityError {
    InitialGlobal(GeneratedSectorLiveLeafQueueError),
    PriorEffective(GeneratedSectorAffineEffectiveResidualQueueError),
}

impl fmt::Debug for GeneratedAffineResidualSourceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::InitialGlobal(_) => "InitialGlobal",
            Self::PriorEffective(_) => "PriorEffective",
        };
        formatter
            .debug_struct("GeneratedAffineResidualSourceAuthorityError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualSourceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialGlobal(_) => {
                formatter.write_str("initial global residual source replay failed")
            }
            Self::PriorEffective(_) => {
                formatter.write_str("prior effective residual source replay failed")
            }
        }
    }
}

// Deliberately do not delegate `Error::source`: the wrapped V1 error formats
// operational detail, while this seam promises redacted diagnostics.
impl std::error::Error for GeneratedAffineResidualSourceAuthorityError {}

/// Shared test-only construction of a genuinely exact delegated owner
/// envelope.  Keeping this next to the source-neutral adapter lets both its
/// direct tests and the composed Boolean tests exercise the same full chain.
#[cfg(test)]
pub(crate) mod point_test_support {
    use crate::generated_residual_affine_when_bad_compilation::{
        GeneratedResidualAffineWhenBadPointLimits, GeneratedResidualAffineWhenBadPointStats,
    };
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffinePointLimits, GeneratedSectorAffinePointSpecializationLimits,
        GeneratedSectorAffinePointSpecializationStats, GeneratedSectorAffinePointStats,
    };
    use crate::residual_affine_integer_system::{
        ResidualAffineIntegerMapPointLimits, ResidualAffineIntegerMapPointStats,
    };

    fn exact_specialization_limits(
        stats: GeneratedSectorAffinePointSpecializationStats,
    ) -> GeneratedSectorAffinePointSpecializationLimits {
        GeneratedSectorAffinePointSpecializationLimits {
            max_source_terms: stats.source_terms(),
            max_source_exponent_entries: stats.source_exponent_entries(),
            max_preflight_validation_source_term_scan_bound: stats
                .preflight_validation_source_term_scan_bound(),
            max_preflight_validation_source_exponent_entry_scan_bound: stats
                .preflight_validation_source_exponent_entry_scan_bound(),
            max_output_term_bound: stats.output_term_bound(),
            max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
            max_power_operation_bound: stats.power_operation_bound(),
            max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
            max_integer_bit_work_bound: stats.integer_bit_work_bound(),
            max_retained_output_term_bound: stats.retained_output_term_bound(),
            max_retained_output_byte_bound: stats.retained_output_byte_bound(),
        }
    }

    fn exact_map_limits(
        stats: ResidualAffineIntegerMapPointStats,
    ) -> ResidualAffineIntegerMapPointLimits {
        ResidualAffineIntegerMapPointLimits {
            max_ambient_arity: stats.ambient_arity(),
            max_matrix_entries_inspected: stats.matrix_entries_inspected(),
            max_nonzero_multiplications: stats.nonzero_multiplications(),
            max_additions: stats.additions(),
            max_fixed_point_comparisons: stats.fixed_point_comparisons(),
            max_peak_temporary_bytes: stats.peak_temporary_bytes(),
            max_integer_bits: stats.largest_integer_bits(),
            max_integer_bit_work: stats.integer_bit_work(),
        }
    }

    fn exact_relative_limits(
        stats: GeneratedResidualAffineWhenBadPointStats,
    ) -> GeneratedResidualAffineWhenBadPointLimits {
        GeneratedResidualAffineWhenBadPointLimits {
            max_context_fingerprint_comparison_bytes: stats.context_fingerprint_comparison_bytes(),
            max_index_entries: stats.index_entries(),
            max_cases: stats.cases(),
            max_classifications: stats.classifications(),
            max_predicates: stats.predicates(),
            max_source_terms: stats.source_terms(),
            max_source_exponent_entries: stats.source_exponent_entries(),
            max_preflight_validation_source_term_scan_bound: stats
                .preflight_validation_source_term_scan_bound(),
            max_preflight_validation_source_exponent_entry_scan_bound: stats
                .preflight_validation_source_exponent_entry_scan_bound(),
            max_output_term_bound: stats.output_term_bound(),
            max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
            max_power_operation_bound: stats.power_operation_bound(),
            max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
            max_integer_bit_work_bound: stats.integer_bit_work_bound(),
            max_retained_output_term_bound: stats.retained_output_term_bound(),
            max_retained_output_byte_bound: stats.retained_output_byte_bound(),
        }
    }

    pub(crate) fn exact_owner_limits(
        stats: GeneratedSectorAffinePointStats,
    ) -> GeneratedSectorAffinePointLimits {
        GeneratedSectorAffinePointLimits {
            map: exact_map_limits(stats.map().unwrap_or_default()),
            relative: exact_relative_limits(stats.relative().unwrap_or_default()),
            global_specialization: exact_specialization_limits(stats.global_specialization()),
            boolean_specialization: exact_specialization_limits(stats.boolean_specialization()),
            max_family_fingerprint_comparison_bytes: stats.family_fingerprint_comparison_bytes(),
            max_context_fingerprint_comparison_bytes: stats.context_fingerprint_comparison_bytes(),
            max_index_entries: stats.index_entries(),
            max_global_cases: stats.global_cases(),
            max_global_classifications: stats.global_classifications(),
            max_global_predicates: stats.global_predicates(),
            max_work_items_scanned: stats.work_items_scanned(),
            max_inventory_terminal_scans: stats.inventory_terminal_scans(),
            max_boolean_nodes_scanned: stats.boolean_nodes_scanned(),
            max_boolean_ready_terminals: stats.boolean_ready_terminals(),
            max_boolean_predicates: stats.boolean_predicates(),
            max_owner_terminal_record_scans: stats.owner_terminal_record_scans(),
            max_inventory_case_lookups: stats.inventory_case_lookups(),
            max_group_pass_scans: stats.group_pass_scans(),
            max_group_case_references_scanned: stats.group_case_references_scanned(),
            max_target_disposition_scans: stats.target_disposition_scans(),
            max_attempt_scans: stats.attempt_scans(),
            max_child_output_lookups: stats.child_output_lookups(),
            max_sealed_rule_scans: stats.sealed_rule_scans(),
            max_residual_work_scans: stats.residual_work_scans(),
            max_child_offset_arithmetic: stats.child_offset_arithmetic(),
            max_child_offset_comparisons: stats.child_offset_comparisons(),
            max_child_authority_comparisons: stats.child_authority_comparisons(),
        }
    }

    /// Visit every positive field in the delegated owner envelope with exactly
    /// that field reduced by one.  Both source and Boolean composition tests use
    /// this single exhaustive field list.
    pub(crate) fn for_each_positive_owner_one_below(
        stats: GeneratedSectorAffinePointStats,
        mut visit: impl FnMut(&'static str, GeneratedSectorAffinePointLimits, usize),
    ) {
        let exact = exact_owner_limits(stats);
        macro_rules! visit_one_below {
            ($requested:expr; $($path:ident).+) => {{
                let requested = $requested;
                if requested > 0 {
                    let mut one_below = exact;
                    one_below.$($path).+ = requested - 1;
                    visit(stringify!($($path).+), one_below, requested);
                }
            }};
        }
        macro_rules! outer {
            ($field:ident, $getter:ident) => {
                visit_one_below!(stats.$getter(); $field);
            };
        }
        outer!(
            max_family_fingerprint_comparison_bytes,
            family_fingerprint_comparison_bytes
        );
        outer!(
            max_context_fingerprint_comparison_bytes,
            context_fingerprint_comparison_bytes
        );
        outer!(max_index_entries, index_entries);
        outer!(max_global_cases, global_cases);
        outer!(max_global_classifications, global_classifications);
        outer!(max_global_predicates, global_predicates);
        outer!(max_work_items_scanned, work_items_scanned);
        outer!(max_inventory_terminal_scans, inventory_terminal_scans);
        outer!(max_boolean_nodes_scanned, boolean_nodes_scanned);
        outer!(max_boolean_ready_terminals, boolean_ready_terminals);
        outer!(max_boolean_predicates, boolean_predicates);
        outer!(max_owner_terminal_record_scans, owner_terminal_record_scans);
        outer!(max_inventory_case_lookups, inventory_case_lookups);
        outer!(max_group_pass_scans, group_pass_scans);
        outer!(
            max_group_case_references_scanned,
            group_case_references_scanned
        );
        outer!(max_target_disposition_scans, target_disposition_scans);
        outer!(max_attempt_scans, attempt_scans);
        outer!(max_child_output_lookups, child_output_lookups);
        outer!(max_sealed_rule_scans, sealed_rule_scans);
        outer!(max_residual_work_scans, residual_work_scans);
        outer!(max_child_offset_arithmetic, child_offset_arithmetic);
        outer!(max_child_offset_comparisons, child_offset_comparisons);
        outer!(max_child_authority_comparisons, child_authority_comparisons);

        macro_rules! specialization {
            ($stage:ident, $stage_stats:expr, $field:ident, $getter:ident) => {
                visit_one_below!($stage_stats.$getter(); $stage.$field);
            };
        }
        macro_rules! all_specialization {
            ($stage:ident, $stage_stats:expr) => {
                specialization!($stage, $stage_stats, max_source_terms, source_terms);
                specialization!(
                    $stage,
                    $stage_stats,
                    max_source_exponent_entries,
                    source_exponent_entries
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_preflight_validation_source_term_scan_bound,
                    preflight_validation_source_term_scan_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_preflight_validation_source_exponent_entry_scan_bound,
                    preflight_validation_source_exponent_entry_scan_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_output_term_bound,
                    output_term_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_output_exponent_entry_bound,
                    output_exponent_entry_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_power_operation_bound,
                    power_operation_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_largest_output_integer_bit_bound,
                    largest_output_integer_bit_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_integer_bit_work_bound,
                    integer_bit_work_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_retained_output_term_bound,
                    retained_output_term_bound
                );
                specialization!(
                    $stage,
                    $stage_stats,
                    max_retained_output_byte_bound,
                    retained_output_byte_bound
                );
            };
        }
        all_specialization!(global_specialization, stats.global_specialization());
        all_specialization!(boolean_specialization, stats.boolean_specialization());

        if let Some(map) = stats.map() {
            macro_rules! map_field {
                ($field:ident, $getter:ident) => {
                    visit_one_below!(map.$getter(); map.$field);
                };
            }
            map_field!(max_ambient_arity, ambient_arity);
            map_field!(max_matrix_entries_inspected, matrix_entries_inspected);
            map_field!(max_nonzero_multiplications, nonzero_multiplications);
            map_field!(max_additions, additions);
            map_field!(max_fixed_point_comparisons, fixed_point_comparisons);
            map_field!(max_peak_temporary_bytes, peak_temporary_bytes);
            map_field!(max_integer_bits, largest_integer_bits);
            map_field!(max_integer_bit_work, integer_bit_work);
        }

        if let Some(relative) = stats.relative() {
            macro_rules! relative_field {
                ($field:ident, $getter:ident) => {
                    visit_one_below!(relative.$getter(); relative.$field);
                };
            }
            relative_field!(
                max_context_fingerprint_comparison_bytes,
                context_fingerprint_comparison_bytes
            );
            relative_field!(max_index_entries, index_entries);
            relative_field!(max_cases, cases);
            relative_field!(max_classifications, classifications);
            relative_field!(max_predicates, predicates);
            relative_field!(max_source_terms, source_terms);
            relative_field!(max_source_exponent_entries, source_exponent_entries);
            relative_field!(
                max_preflight_validation_source_term_scan_bound,
                preflight_validation_source_term_scan_bound
            );
            relative_field!(
                max_preflight_validation_source_exponent_entry_scan_bound,
                preflight_validation_source_exponent_entry_scan_bound
            );
            relative_field!(max_output_term_bound, output_term_bound);
            relative_field!(max_output_exponent_entry_bound, output_exponent_entry_bound);
            relative_field!(max_power_operation_bound, power_operation_bound);
            relative_field!(
                max_largest_output_integer_bit_bound,
                largest_output_integer_bit_bound
            );
            relative_field!(max_integer_bit_work_bound, integer_bit_work_bound);
            relative_field!(max_retained_output_term_bound, retained_output_term_bound);
            relative_field!(max_retained_output_byte_bound, retained_output_byte_bound);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA,
        GeneratedAffineInitialGlobalBooleanAtomPolarity,
        GeneratedAffineInitialGlobalBooleanCoverError,
        GeneratedAffineInitialGlobalPredicateLookupLimits,
        GeneratedAffineInitialGlobalPredicateSourceViewError,
        GeneratedAffineInitialGlobalReadySourceView, GeneratedAffineInitialGlobalSemanticOutcome,
        GeneratedAffineInitialGlobalSourceView, GeneratedAffineInitialGlobalTerminalSourceView,
        GeneratedAffineResidualPointSpecializationLimits,
        GeneratedAffineResidualPointSpecializationStats,
        GeneratedAffineResidualPriorActionableSourceView, GeneratedAffineResidualPriorAtomPolarity,
        GeneratedAffineResidualPriorExceptionalSourceView,
        GeneratedAffineResidualPriorGuardClassSourceView, GeneratedAffineResidualPriorSourceView,
        GeneratedAffineResidualPriorTargetSourceView,
        GeneratedAffineResidualPriorUnsupportedSourceView, GeneratedAffineResidualSourceAuthority,
        GeneratedAffineResidualSourceAuthorityError, GeneratedAffineResidualSourceAuthorityKind,
        GeneratedAffineResidualSourcePointDisposition, GeneratedAffineResidualSourcePointError,
        GeneratedAffineResidualSourcePointLimits, GeneratedAffineResidualSourceView,
        GeneratedAffineResidualSourceViewError, authenticate_initial_global_semantic_outcome,
    };
    use crate::generated_residual_affine_when_bad_compilation::GeneratedResidualAffineWhenBadPointError;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCertificate,
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
        GeneratedSectorAffinePointError,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCertificate,
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueError,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
        GeneratedSectorAffineEffectiveResidualSourceView,
        GeneratedSectorAffineEffectiveResidualSourceViewError,
        GeneratedSectorAffineEffectiveResidualTargetSourceView,
    };
    use crate::residual_affine_integer_system::ResidualAffineIntegerMapPointError;
    use crate::{
        AffineDenominator, CoefficientContext, CoordinateEqualityEmptyReason,
        CoordinateEqualityLeafStatus, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafOutcome,
        GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueError, GeneratedSectorLiveLeafQueueLimits,
        GeneratedSectorQueuedSourceDisposition, IntegralFamily, IntegralOrderingPolicy,
        ParametricCoefficientContext, ParametricIbpGenerator,
        ResidualAffineBranchGuardCompositionClass, ResidualProductLocusBooleanCoverError,
        ResidualProductLocusBooleanCoverLimits, SectorMask, SectorOrthantSide,
    };

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

    fn max_guard_root_tadpole_family(name: &str) -> IntegralFamily {
        // Retain one inert base-field symbol because Symbolica's current
        // zero-variable polynomial formatter does not support this path.
        let coefficients = CoefficientContext::new(["unused"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parse("18446744073709551614").unwrap(),
            vec![AffineDenominator::new(
                coefficients.zero(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn one_loop_initial_global_fixture(
        family: IntegralFamily,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) {
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            GeneratedSectorDiscoveryLimits::default(),
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 1;
        let queue = GeneratedSectorLiveLeafQueueCompiler::compile(
            &family,
            &context,
            &discovery,
            queue_limits,
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn initial_global_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) {
        initial_global_sector_fixture(name, "001")
    }

    fn initial_global_sector_fixture(
        name: &str,
        sector: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorLiveLeafQueueCertificate>,
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
            SectorMask::try_from_bit_string(sector).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = GeneratedSectorLiveLeafQueueCompiler::compile(
            &family,
            &context,
            &discovery,
            queue_limits,
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn prior_effective_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorAffineEffectiveResidualQueueCertificate>,
    ) {
        let (family, context, source_queue) = initial_global_fixture(name);
        let inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                source_queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate> = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        let queue = GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
            &family,
            &context,
            owner,
            GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn wrong_context(
        context: &ParametricCoefficientContext,
        scope: &str,
    ) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(context.base(), scope, context.index_count()).unwrap()
    }

    fn exact_point_specialization_limits(
        stats: GeneratedAffineResidualPointSpecializationStats,
    ) -> GeneratedAffineResidualPointSpecializationLimits {
        GeneratedAffineResidualPointSpecializationLimits {
            max_source_terms: stats.source_terms(),
            max_source_exponent_entries: stats.source_exponent_entries(),
            max_preflight_validation_source_term_scan_bound: stats
                .preflight_validation_source_term_scan_bound(),
            max_preflight_validation_source_exponent_entry_scan_bound: stats
                .preflight_validation_source_exponent_entry_scan_bound(),
            max_output_term_bound: stats.output_term_bound(),
            max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
            max_power_operation_bound: stats.power_operation_bound(),
            max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
            max_integer_bit_work_bound: stats.integer_bit_work_bound(),
            max_retained_output_term_bound: stats.retained_output_term_bound(),
            max_retained_output_byte_bound: stats.retained_output_byte_bound(),
        }
    }

    fn exact_source_point_limits(
        stats: super::GeneratedAffineResidualSourcePointStats,
    ) -> GeneratedAffineResidualSourcePointLimits {
        let initial_work_item_scans = (stats.kind()
            == Some(GeneratedAffineResidualSourceAuthorityKind::InitialGlobal))
        .then_some(stats.work_item_scans())
        .unwrap_or(0);
        let prior_effective = stats.prior_effective_owner().map_or_else(
            Default::default,
            |owner| {
                crate::generated_sector_affine_effective_residual_queue::GeneratedSectorAffineEffectiveResidualQueuePointLimits {
                    owner: super::point_test_support::exact_owner_limits(owner),
                    max_work_item_scans: stats.work_item_scans(),
                }
            },
        );
        GeneratedAffineResidualSourcePointLimits {
            prior_effective,
            initial_specialization: exact_point_specialization_limits(
                stats.initial_specialization(),
            ),
            max_scope_comparison_bytes: stats.scope_comparison_bytes(),
            max_index_entries: stats.index_entries(),
            max_initial_orthant_index_scans: stats.initial_orthant_index_scans(),
            max_initial_case_scans: stats.initial_case_scans(),
            max_initial_classification_scans: stats.initial_classification_scans(),
            max_initial_predicate_scans: stats.initial_predicate_scans(),
            max_initial_predicate_evaluations: stats.initial_predicate_evaluations(),
            max_initial_work_item_scans: initial_work_item_scans,
            max_initial_disposition_candidate_comparisons: stats
                .initial_disposition_candidate_comparisons(),
        }
    }

    fn is_source_point_resource_limit(error: &GeneratedAffineResidualSourcePointError) -> bool {
        matches!(
            error,
            GeneratedAffineResidualSourcePointError::ResourceLimit { .. }
                | GeneratedAffineResidualSourcePointError::PriorEffective(
                    GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit { .. }
                )
        ) || matches!(
            error,
            GeneratedAffineResidualSourcePointError::PriorEffective(
                GeneratedSectorAffineEffectiveResidualQueueError::Point(
                    GeneratedSectorAffinePointError::ResourceLimit { .. }
                        | GeneratedSectorAffinePointError::AffineMap(
                            ResidualAffineIntegerMapPointError::ResourceLimit { .. }
                        )
                        | GeneratedSectorAffinePointError::RelativePoint(
                            GeneratedResidualAffineWhenBadPointError::ResourceLimit { .. }
                        )
                )
            )
        )
    }

    fn assert_prior_source_exact_and_every_positive_one_below(
        authority: &GeneratedAffineResidualSourceAuthority,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        point: &[i64],
        baseline: super::GeneratedAffineResidualSourcePointClassification,
    ) {
        let stats = baseline.stats();
        let owner = stats
            .prior_effective_owner()
            .expect("PriorEffective Work must retain its delegated owner census");
        let exact = exact_source_point_limits(stats);
        let exact_classified = authority
            .classification_for_indices(family, context, point, exact)
            .unwrap();
        assert_eq!(exact_classified.disposition(), baseline.disposition());
        assert_eq!(exact_classified.stats(), stats);

        let mut tested_positive_limits = 0usize;
        macro_rules! reject_one_below {
            ($requested:expr; $($path:ident).+) => {{
                let requested = $requested;
                if requested > 0 {
                    tested_positive_limits += 1;
                    let mut one_below = exact;
                    one_below.$($path).+ = requested - 1;
                    let error = authority
                        .classification_for_indices(family, context, point, one_below)
                        .unwrap_err();
                    assert!(
                        is_source_point_resource_limit(&error),
                        "{} prior one-below returned {error:?}",
                        stringify!($($path).+),
                    );
                }
            }};
        }

        reject_one_below!(stats.scope_comparison_bytes(); max_scope_comparison_bytes);
        reject_one_below!(stats.index_entries(); max_index_entries);
        reject_one_below!(stats.work_item_scans(); prior_effective.max_work_item_scans);

        macro_rules! owner_outer {
            ($field:ident, $getter:ident) => {
                reject_one_below!(owner.$getter(); prior_effective.owner.$field);
            };
        }
        owner_outer!(
            max_family_fingerprint_comparison_bytes,
            family_fingerprint_comparison_bytes
        );
        owner_outer!(
            max_context_fingerprint_comparison_bytes,
            context_fingerprint_comparison_bytes
        );
        owner_outer!(max_index_entries, index_entries);
        owner_outer!(max_global_cases, global_cases);
        owner_outer!(max_global_classifications, global_classifications);
        owner_outer!(max_global_predicates, global_predicates);
        owner_outer!(max_work_items_scanned, work_items_scanned);
        owner_outer!(max_inventory_terminal_scans, inventory_terminal_scans);
        owner_outer!(max_boolean_nodes_scanned, boolean_nodes_scanned);
        owner_outer!(max_boolean_ready_terminals, boolean_ready_terminals);
        owner_outer!(max_boolean_predicates, boolean_predicates);
        owner_outer!(max_owner_terminal_record_scans, owner_terminal_record_scans);
        owner_outer!(max_inventory_case_lookups, inventory_case_lookups);
        owner_outer!(max_group_pass_scans, group_pass_scans);
        owner_outer!(
            max_group_case_references_scanned,
            group_case_references_scanned
        );
        owner_outer!(max_target_disposition_scans, target_disposition_scans);
        owner_outer!(max_attempt_scans, attempt_scans);
        owner_outer!(max_child_output_lookups, child_output_lookups);
        owner_outer!(max_sealed_rule_scans, sealed_rule_scans);
        owner_outer!(max_residual_work_scans, residual_work_scans);
        owner_outer!(max_child_offset_arithmetic, child_offset_arithmetic);
        owner_outer!(max_child_offset_comparisons, child_offset_comparisons);
        owner_outer!(max_child_authority_comparisons, child_authority_comparisons);

        macro_rules! specialization_one_below {
            ($stage:ident, $stage_stats:expr, $field:ident, $getter:ident) => {
                reject_one_below!(
                    $stage_stats.$getter();
                    prior_effective.owner.$stage.$field
                );
            };
        }
        macro_rules! all_specialization_one_below {
            ($stage:ident, $stage_stats:expr) => {
                specialization_one_below!($stage, $stage_stats, max_source_terms, source_terms);
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_source_exponent_entries,
                    source_exponent_entries
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_preflight_validation_source_term_scan_bound,
                    preflight_validation_source_term_scan_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_preflight_validation_source_exponent_entry_scan_bound,
                    preflight_validation_source_exponent_entry_scan_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_output_term_bound,
                    output_term_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_output_exponent_entry_bound,
                    output_exponent_entry_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_power_operation_bound,
                    power_operation_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_largest_output_integer_bit_bound,
                    largest_output_integer_bit_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_integer_bit_work_bound,
                    integer_bit_work_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_retained_output_term_bound,
                    retained_output_term_bound
                );
                specialization_one_below!(
                    $stage,
                    $stage_stats,
                    max_retained_output_byte_bound,
                    retained_output_byte_bound
                );
            };
        }
        all_specialization_one_below!(global_specialization, owner.global_specialization());
        all_specialization_one_below!(boolean_specialization, owner.boolean_specialization());

        if let Some(map) = owner.map() {
            macro_rules! map_one_below {
                ($field:ident, $getter:ident) => {
                    reject_one_below!(map.$getter(); prior_effective.owner.map.$field);
                };
            }
            map_one_below!(max_ambient_arity, ambient_arity);
            map_one_below!(max_matrix_entries_inspected, matrix_entries_inspected);
            map_one_below!(max_nonzero_multiplications, nonzero_multiplications);
            map_one_below!(max_additions, additions);
            map_one_below!(max_fixed_point_comparisons, fixed_point_comparisons);
            map_one_below!(max_peak_temporary_bytes, peak_temporary_bytes);
            map_one_below!(max_integer_bits, largest_integer_bits);
            map_one_below!(max_integer_bit_work, integer_bit_work);
        }

        if let Some(relative) = owner.relative() {
            macro_rules! relative_one_below {
                ($field:ident, $getter:ident) => {
                    reject_one_below!(relative.$getter(); prior_effective.owner.relative.$field);
                };
            }
            relative_one_below!(
                max_context_fingerprint_comparison_bytes,
                context_fingerprint_comparison_bytes
            );
            relative_one_below!(max_index_entries, index_entries);
            relative_one_below!(max_cases, cases);
            relative_one_below!(max_classifications, classifications);
            relative_one_below!(max_predicates, predicates);
            relative_one_below!(max_source_terms, source_terms);
            relative_one_below!(max_source_exponent_entries, source_exponent_entries);
            relative_one_below!(
                max_preflight_validation_source_term_scan_bound,
                preflight_validation_source_term_scan_bound
            );
            relative_one_below!(
                max_preflight_validation_source_exponent_entry_scan_bound,
                preflight_validation_source_exponent_entry_scan_bound
            );
            relative_one_below!(max_output_term_bound, output_term_bound);
            relative_one_below!(max_output_exponent_entry_bound, output_exponent_entry_bound);
            relative_one_below!(max_power_operation_bound, power_operation_bound);
            relative_one_below!(
                max_largest_output_integer_bit_bound,
                largest_output_integer_bit_bound
            );
            relative_one_below!(max_integer_bit_work_bound, integer_bit_work_bound);
            relative_one_below!(max_retained_output_term_bound, retained_output_term_bound);
            relative_one_below!(max_retained_output_byte_bound, retained_output_byte_bound);
        }
        assert!(tested_positive_limits > 0);
    }

    fn assert_authority_debug_is_redacted(
        authority: &GeneratedAffineResidualSourceAuthority,
        family_fingerprint: &str,
        context_fingerprint: &str,
    ) {
        let rendered = format!("{authority:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family_fingerprint));
        assert!(!rendered.contains(context_fingerprint));
        for private in [
            "ParametricRelation",
            "private_predicate",
            "split_recentered_relation",
            "source_queue",
            "ordered_child_outputs",
        ] {
            assert!(!rendered.contains(private), "leaked {private}: {rendered}");
        }
    }

    fn assert_replay_error_is_redacted(
        error: &GeneratedAffineResidualSourceAuthorityError,
        forbidden: &[&str],
    ) {
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("<redacted>"));
        for private in forbidden {
            assert!(!rendered.contains(private), "leaked {private}: {rendered}");
        }
        assert!(std::error::Error::source(error).is_none());
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn sealed_authority_and_error_remain_thread_safe() {
        assert_send_sync::<GeneratedAffineResidualSourceAuthority>();
        assert_send_sync::<GeneratedAffineResidualSourceAuthorityError>();
        assert_send_sync::<GeneratedAffineResidualSourceView<'static>>();
        assert_send_sync::<GeneratedAffineResidualSourceViewError>();
        assert_send_sync::<GeneratedAffineInitialGlobalPredicateSourceViewError>();
    }

    #[test]
    fn initial_global_unified_views_are_lifetime_bound_narrow_and_explicitly_bounded() {
        let (family, context, source) = one_loop_initial_global_fixture(massive_tadpole_family(
            "authority-initial-unified-view-private-family",
        ));
        let source_strong_count = Arc::strong_count(&source);
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        authority.replay(&family, &context).unwrap();
        assert_eq!(Arc::strong_count(&source), source_strong_count + 1);
        assert!(!authority.is_empty());

        let coverage = source.discovery().coverage();
        let mut ready_sources = 0usize;
        let mut resolved_predicates = 0usize;
        for ordinal in 0..authority.len() {
            let strong_before = Arc::strong_count(&source);
            let view = authority.authenticated_source_view(ordinal).unwrap();
            assert_eq!(Arc::strong_count(&source), strong_before);
            assert_eq!(view.work_item_ordinal(), ordinal);
            let GeneratedAffineResidualSourceView::InitialGlobal(view) = view else {
                panic!("initial authority returned a prior-effective view")
            };
            let terminal = view.terminal();
            assert_eq!(terminal.work_item_ordinal(), ordinal);
            assert_eq!(
                terminal.source_identity_bytes(),
                coverage.partition().source_identity().len()
            );
            assert!(terminal.case_lookup_comparisons() > 0);
            assert!(
                terminal.case_lookup_comparisons() <= usize::BITS as usize + 1,
                "binary lookup exceeded its word-sized logarithmic bound"
            );
            let expected_disposition_comparisons =
                match source.work_items()[ordinal].source_disposition() {
                    GeneratedSectorQueuedSourceDisposition::Uncovered => 0,
                    GeneratedSectorQueuedSourceDisposition::Unsupported { candidate_ordinals } => {
                        candidate_ordinals.len()
                    }
                };
            assert_eq!(
                terminal.source_disposition_candidate_comparisons(),
                expected_disposition_comparisons
            );

            let rendered = format!("{view:?}");
            assert!(rendered.contains("<redacted>"));
            for private in [
                "source_case",
                "source_queue",
                "source_extraction",
                "structural_loci",
                "product_zero_decompositions",
                "PartialReelimination",
                family.fingerprint_ref(),
                context.fingerprint(),
            ] {
                assert!(!rendered.contains(private), "leaked {private}: {rendered}");
            }

            let GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(ready) = view else {
                continue;
            };
            ready_sources += 1;
            let item = &source.work_items()[ordinal];
            let source_case = coverage
                .partition()
                .case(item.source_case())
                .expect("replayed source case");
            assert_eq!(
                ready.source_predicate_count(),
                source_case.predicates().len()
            );

            for predicate_ordinal in 0..ready.source_predicate_count() {
                let predicate = ready
                    .authenticated_predicate_view(
                        predicate_ordinal,
                        GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                            usize::MAX,
                            usize::MAX,
                            usize::MAX,
                        ),
                    )
                    .unwrap();
                resolved_predicates += 1;
                let expected = &source_case.predicates()[predicate_ordinal];
                assert_eq!(predicate.predicate_ordinal(), predicate_ordinal);
                assert_eq!(predicate.kind(), expected.kind());
                assert_eq!(predicate.polynomial(), expected.polynomial());
                assert_eq!(
                    predicate.atom_polynomial(predicate.atom_count()),
                    None,
                    "unrelated global loci must not be addressable"
                );
                for atom_position in 0..predicate.atom_count() {
                    let locus = predicate.atom_locus_ordinal(atom_position).unwrap();
                    assert_eq!(
                        predicate.atom_polynomial(atom_position),
                        coverage.structural_locus(locus)
                    );
                }

                let stats = predicate.stats();
                assert!(stats.structural_locus_comparisons() > 0);
                let structural_too_small = ready
                    .authenticated_predicate_view(
                        predicate_ordinal,
                        GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                            stats.structural_locus_comparisons() - 1,
                            usize::MAX,
                            usize::MAX,
                        ),
                    )
                    .unwrap_err();
                assert_eq!(
                    structural_too_small,
                    GeneratedAffineInitialGlobalPredicateSourceViewError::StructuralLocusComparisonLimit
                );
                if stats.product_decomposition_comparisons() > 0 {
                    let product_too_small = ready
                        .authenticated_predicate_view(
                            predicate_ordinal,
                            GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                                usize::MAX,
                                stats.product_decomposition_comparisons() - 1,
                                usize::MAX,
                            ),
                        )
                        .unwrap_err();
                    assert_eq!(
                        product_too_small,
                        GeneratedAffineInitialGlobalPredicateSourceViewError::ProductDecompositionComparisonLimit
                    );
                }
                if stats.factor_locus_checks() > 0 {
                    let factor_too_small = ready
                        .authenticated_predicate_view(
                            predicate_ordinal,
                            GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                                usize::MAX,
                                usize::MAX,
                                stats.factor_locus_checks() - 1,
                            ),
                        )
                        .unwrap_err();
                    assert_eq!(
                        factor_too_small,
                        GeneratedAffineInitialGlobalPredicateSourceViewError::FactorLocusCheckLimit
                    );
                }
            }

            let out_of_range = ready
                .authenticated_predicate_view(
                    ready.source_predicate_count(),
                    GeneratedAffineInitialGlobalPredicateLookupLimits::new(0, 0, 0),
                )
                .unwrap_err();
            assert_eq!(
                out_of_range,
                GeneratedAffineInitialGlobalPredicateSourceViewError::PredicateOutOfRange
            );
            let rendered = format!("{out_of_range} {out_of_range:?}");
            assert!(rendered.contains("<redacted>"));
            assert!(std::error::Error::source(&out_of_range).is_none());
        }
        assert!(ready_sources > 0, "fixture must retain a nonempty source");
        assert!(
            resolved_predicates > 0,
            "fixture must exercise bounded predicate-to-locus resolution"
        );

        let error = authority
            .authenticated_source_view(authority.len())
            .unwrap_err();
        assert!(matches!(
            error,
            GeneratedAffineResidualSourceViewError::InitialGlobalWorkItemOutOfRange
        ));
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn replay_session_seals_exact_v1_cover_atoms_payload_and_resource_boundaries() {
        let (family, context, source) =
            initial_global_sector_fixture("authority-sealed-boolean-cover-private-family", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        let session = authority.replay_session(&family, &context).unwrap();
        let (work_item_ordinal, ready) = (0..authority.len())
            .find_map(
                |ordinal| match session.authenticated_source_view(ordinal).unwrap() {
                    GeneratedAffineResidualSourceView::InitialGlobal(
                        GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(ready),
                    ) => Some((ordinal, ready)),
                    _ => None,
                },
            )
            .expect("natural generated 011 source has a Ready Boolean leaf");
        let census = ready.boolean_binding_census().unwrap();
        assert_eq!(census.source_identity_pointer_comparisons(), 1);
        assert!(census.source_identity_bytes() > 0);
        assert!(census.scope_comparison_bytes() > 0);
        assert!(census.sector_entry_comparisons() > 0);
        assert!(census.structural_polynomial_equality_term_work() > 0);
        assert!(census.structural_polynomial_equality_byte_work() > 0);

        let strong_before = Arc::strong_count(&source);
        let first = ready
            .compile_boolean_cover_replayed(
                &family,
                &context,
                census,
                ResidualProductLocusBooleanCoverLimits::default(),
            )
            .unwrap();
        assert_eq!(Arc::strong_count(&source), strong_before + 1);
        assert_eq!(first.source_work_item_ordinal(), work_item_ordinal);
        assert!(first.node_count() >= first.terminal_count());
        assert!(first.retained_owned_logical_bytes_upper_bound() > 0);
        assert!(
            first.compilation_owned_logical_peak_upper_bound()
                >= first.retained_owned_logical_bytes_upper_bound()
        );
        let preflight = crate::product_locus_boolean_cover::residual_product_locus_boolean_memory_envelope_from_limits(
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap();
        assert!(
            first.retained_owned_logical_bytes_upper_bound()
                <= preflight.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            first.compilation_owned_logical_peak_upper_bound()
                <= preflight.compilation_owned_logical_peak_upper_bound()
        );
        assert_eq!(
            first.terminal_count(),
            first.v1_stats().ready_terminals() + first.v1_stats().proved_empty_terminals(),
        );

        let mut terminal_count = 0usize;
        let mut atom_count = 0usize;
        for terminal in first.terminal_views() {
            terminal_count += 1;
            for polarity in [
                GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero,
                GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero,
            ] {
                let count = match polarity {
                    GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero => {
                        terminal.equal_zero_atom_count()
                    }
                    GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero => {
                        terminal.nonzero_atom_count()
                    }
                };
                for position in 0..count {
                    let atom = terminal.atom(polarity, position).unwrap();
                    assert!(atom.polynomial().term_count() > 0);
                    assert!(format!("{atom:?}").contains("<redacted>"));
                    atom_count += 1;
                }
                assert!(terminal.atom(polarity, count).is_none());
            }
        }
        assert_eq!(terminal_count, first.terminal_count());
        assert!(
            atom_count > 0,
            "natural 011 cover must expose authenticated atoms"
        );

        let mut second = ready
            .compile_boolean_cover_replayed(
                &family,
                &context,
                census,
                ResidualProductLocusBooleanCoverLimits::default(),
            )
            .unwrap();
        assert!(first.payload_eq_checked(&second).unwrap());
        second.tamper_resource_census_for_test();
        assert!(!first.payload_eq_checked(&second).unwrap());
        assert_eq!(Arc::strong_count(&source), strong_before + 2);
        let rendered = format!("{first:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(!rendered.contains(context.fingerprint()));

        let normal_ready = match authority
            .authenticated_source_view(work_item_ordinal)
            .unwrap()
        {
            GeneratedAffineResidualSourceView::InitialGlobal(
                GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(ready),
            ) => ready,
            _ => panic!("session and ordinary source views disagree"),
        };
        assert!(matches!(
            normal_ready.compile_boolean_cover_replayed(
                &family,
                &context,
                census,
                ResidualProductLocusBooleanCoverLimits::default(),
            ),
            Err(GeneratedAffineInitialGlobalBooleanCoverError::ReplaySessionRequired)
        ));

        let mut wrong_census = census;
        wrong_census.source_identity_bytes = wrong_census.source_identity_bytes.saturating_add(1);
        assert!(matches!(
            ready.compile_boolean_cover_replayed(
                &family,
                &context,
                wrong_census,
                ResidualProductLocusBooleanCoverLimits::default(),
            ),
            Err(GeneratedAffineInitialGlobalBooleanCoverError::BindingCensusMismatch)
        ));

        let observed = first.v1_stats();
        assert!(observed.payload_comparison_units() > 0);
        let mut exact = ResidualProductLocusBooleanCoverLimits::default();
        exact.max_payload_comparison_units = observed.payload_comparison_units();
        ready
            .compile_boolean_cover_replayed(&family, &context, census, exact)
            .unwrap();
        exact.max_payload_comparison_units = observed.payload_comparison_units() - 1;
        assert!(matches!(
            ready.compile_boolean_cover_replayed(&family, &context, census, exact),
            Err(GeneratedAffineInitialGlobalBooleanCoverError::V1Cover(
                ResidualProductLocusBooleanCoverError::ResourceLimit {
                    resource: "payload comparison units",
                    requested,
                    limit,
                }
            )) if requested == observed.payload_comparison_units() && limit + 1 == requested
        ));
    }

    #[test]
    fn natural_product_source_exposes_only_its_canonical_factor_atoms_and_enforces_bounds() {
        let (family, context, source) =
            initial_global_sector_fixture("authority-initial-product-private-family", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        authority.replay(&family, &context).unwrap();
        assert!(
            !source
                .discovery()
                .coverage()
                .product_zero_decompositions()
                .is_empty(),
            "natural sunset sector must retain product provenance"
        );

        let mut found_product_predicate = false;
        'items: for ordinal in 0..authority.len() {
            let GeneratedAffineResidualSourceView::InitialGlobal(
                GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(ready),
            ) = authority.authenticated_source_view(ordinal).unwrap()
            else {
                continue;
            };
            for predicate_ordinal in 0..ready.source_predicate_count() {
                let predicate = ready
                    .authenticated_predicate_view(
                        predicate_ordinal,
                        GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                            usize::MAX,
                            usize::MAX,
                            usize::MAX,
                        ),
                    )
                    .unwrap();
                let stats = predicate.stats();
                if stats.factor_locus_checks() == 0 {
                    continue;
                }
                found_product_predicate = true;
                assert!(predicate.atom_count() >= 2);
                assert_eq!(predicate.atom_count(), stats.factor_locus_checks());
                for atom_position in 0..predicate.atom_count() {
                    assert!(predicate.atom_locus_ordinal(atom_position).is_some());
                    assert!(predicate.atom_polynomial(atom_position).is_some());
                }
                assert!(
                    predicate
                        .atom_locus_ordinal(predicate.atom_count())
                        .is_none()
                );
                assert!(predicate.atom_polynomial(predicate.atom_count()).is_none());

                let factor_limit = ready
                    .authenticated_predicate_view(
                        predicate_ordinal,
                        GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                            usize::MAX,
                            usize::MAX,
                            stats.factor_locus_checks() - 1,
                        ),
                    )
                    .unwrap_err();
                assert_eq!(
                    factor_limit,
                    GeneratedAffineInitialGlobalPredicateSourceViewError::FactorLocusCheckLimit
                );
                assert!(stats.product_decomposition_comparisons() > 0);
                let product_limit = ready
                    .authenticated_predicate_view(
                        predicate_ordinal,
                        GeneratedAffineInitialGlobalPredicateLookupLimits::new(
                            usize::MAX,
                            stats.product_decomposition_comparisons() - 1,
                            usize::MAX,
                        ),
                    )
                    .unwrap_err();
                assert_eq!(
                    product_limit,
                    GeneratedAffineInitialGlobalPredicateSourceViewError::ProductDecompositionComparisonLimit
                );
                break 'items;
            }
        }
        assert!(
            found_product_predicate,
            "natural sunset sector 011 must force canonical factor resolution"
        );
    }

    #[test]
    fn every_retained_nonempty_initial_outcome_collapses_to_ready_without_payload_exposure() {
        let preserved = initial_global_fixture("authority-ready-preserved-private-family");
        let partial = one_loop_initial_global_fixture(massive_tadpole_family(
            "authority-ready-partial-private-family",
        ));
        let boundary = one_loop_initial_global_fixture(max_guard_root_tadpole_family(
            "authority-ready-boundary-private-family",
        ));
        let mut saw_preserved = false;
        let mut saw_partial = false;
        let mut saw_boundary = false;

        for (family, context, source) in [preserved, partial, boundary] {
            let authority =
                GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
            authority.replay(&family, &context).unwrap();
            for (ordinal, item) in source.work_items().iter().enumerate() {
                let unified = authority.authenticated_source_view(ordinal).unwrap();
                let GeneratedAffineResidualSourceView::InitialGlobal(unified) = unified else {
                    panic!("initial source dispatched to prior-effective view")
                };
                match item.outcome() {
                    crate::GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty => {
                        assert!(matches!(
                            unified,
                            GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(_)
                        ));
                    }
                    crate::GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment => {
                        saw_preserved = true;
                        assert!(matches!(
                            unified,
                            GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(_)
                        ));
                    }
                    crate::GeneratedSectorLiveLeafOutcome::PartialReelimination { .. } => {
                        saw_partial = true;
                        assert!(matches!(
                            unified,
                            GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(_)
                        ));
                    }
                    crate::GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary { .. } => {
                        saw_boundary = true;
                        assert!(matches!(
                            unified,
                            GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(_)
                        ));
                    }
                }
                let rendered = format!("{unified:?}");
                for private in [
                    "PartialReelimination",
                    "PreservedIndexBoundary",
                    "compilation",
                    "witness",
                    "assignment",
                ] {
                    assert!(!rendered.contains(private), "leaked {private}: {rendered}");
                }
            }
        }

        assert!(
            saw_preserved,
            "fixture must cover preserved-without-assignment"
        );
        assert!(saw_partial, "fixture must cover partial re-elimination");
        assert!(
            saw_boundary,
            "fixture must cover the checked-index boundary"
        );
    }

    #[test]
    fn defensive_empty_outcome_is_distinct_from_a_ready_empty_conjunction() {
        // V4 global coverage prunes all three coordinate-empty reason classes
        // before live-queue construction, so no natural V2 queue fixture can
        // reach this frozen defensive outcome. Test the pure typed binding
        // directly: an authenticated coordinate-empty leaf is no Boolean
        // cover, whereas a nonempty source with zero predicates is one empty
        // conjunction ready for Boolean compilation.
        let proved_empty = CoordinateEqualityLeafStatus::ProvedEmpty(
            CoordinateEqualityEmptyReason::OrthantViolation {
                index: 0,
                value: 0,
                equality_predicate_ordinals: vec![0].into_boxed_slice(),
                side: SectorOrthantSide::AtLeastOne,
            },
        );
        assert_eq!(
            authenticate_initial_global_semantic_outcome(
                &GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
                &proved_empty,
            ),
            Some(GeneratedAffineInitialGlobalSemanticOutcome::CoordinateLeafProvedEmpty)
        );
        assert_eq!(
            authenticate_initial_global_semantic_outcome(
                &GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment,
                &CoordinateEqualityLeafStatus::NotProvedEmpty,
            ),
            Some(GeneratedAffineInitialGlobalSemanticOutcome::ReadyForBooleanCover)
        );
        assert_eq!(
            authenticate_initial_global_semantic_outcome(
                &GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
                &CoordinateEqualityLeafStatus::NotProvedEmpty,
            ),
            None,
            "outcome/status disagreement must not authenticate"
        );

        let terminal = GeneratedAffineInitialGlobalTerminalSourceView {
            work_item_ordinal: 0,
            source_case_position: 0,
            source_identity_bytes: 0,
            case_lookup_comparisons: 0,
            source_disposition_candidate_comparisons: 0,
            lifetime: std::marker::PhantomData,
        };
        let empty = GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(terminal);
        assert!(matches!(
            empty,
            GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(_)
        ));
        assert_eq!(
            authenticate_initial_global_semantic_outcome(
                &GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment,
                &CoordinateEqualityLeafStatus::NotProvedEmpty,
            ),
            Some(GeneratedAffineInitialGlobalSemanticOutcome::ReadyForBooleanCover)
        );
    }

    fn assert_target_projection_matches_current_owner(
        expected: GeneratedSectorAffineEffectiveResidualTargetSourceView<'_>,
        actual: GeneratedAffineResidualPriorTargetSourceView<'_>,
    ) {
        assert_eq!(
            expected.terminal().work_item_ordinal(),
            actual.terminal().work_item_ordinal()
        );
        assert!(std::ptr::eq(expected.affine_map(), actual.affine_map()));
        assert_eq!(expected.guard_entry_count(), actual.guard_entry_count());
        for position in 0..expected.guard_entry_count() {
            let expected = expected.guard_entry(position).unwrap();
            let actual = actual.guard_entry(position).unwrap();
            assert_eq!(
                expected.structural_locus_ordinal(),
                actual.structural_locus_ordinal()
            );
            assert!(std::ptr::eq(
                expected.mapped_polynomial(),
                actual.mapped_polynomial()
            ));
            assert_eq!(expected.composition_stats(), actual.composition_stats());
            match (expected.class(), actual.class()) {
                (
                    ResidualAffineBranchGuardCompositionClass::Contradiction,
                    GeneratedAffineResidualPriorGuardClassSourceView::Contradiction,
                )
                | (
                    ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant,
                    GeneratedAffineResidualPriorGuardClassSourceView::DischargedNonzeroIntegerConstant,
                ) => {}
                (
                    ResidualAffineBranchGuardCompositionClass::BaseAssumption(condition),
                    GeneratedAffineResidualPriorGuardClassSourceView::BaseAssumption {
                        condition_polynomial,
                    },
                )
                | (
                    ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(condition),
                    GeneratedAffineResidualPriorGuardClassSourceView::FreeIndexDependent {
                        condition_polynomial,
                    },
                ) => assert!(std::ptr::eq(condition.polynomial(), condition_polynomial)),
                _ => panic!("prior guard projection changed its semantic class"),
            }
            let rendered = format!("{actual:?}");
            assert!(rendered.contains("<redacted>"));
            assert!(!rendered.contains("origin"));
            assert!(!rendered.contains("polynomial:"));
        }
        assert!(actual.guard_entry(actual.guard_entry_count()).is_none());
        assert_eq!(expected.constant_count(), actual.constant_count());
        for position in 0..expected.constant_count() {
            assert!(std::ptr::eq(
                expected.constant(position).unwrap(),
                actual.constant(position).unwrap()
            ));
        }
        assert!(actual.constant(actual.constant_count()).is_none());
        assert_eq!(expected.free_position_count(), actual.free_position_count());
        for position in 0..expected.free_position_count() {
            assert_eq!(
                expected.free_position(position),
                actual.free_position(position)
            );
        }
        assert!(actual.free_position(actual.free_position_count()).is_none());
    }

    fn assert_unsupported_projection_matches_current_owner(
        expected: crate::generated_sector_affine_effective_residual_queue::GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'_>,
        actual: GeneratedAffineResidualPriorUnsupportedSourceView<'_>,
    ) {
        for (expected_polarity, actual_polarity) in [
            (
                crate::generated_sector_affine_effective_residual_queue::GeneratedSectorAffineEffectiveResidualAtomPolarity::EqualZero,
                GeneratedAffineResidualPriorAtomPolarity::EqualZero,
            ),
            (
                crate::generated_sector_affine_effective_residual_queue::GeneratedSectorAffineEffectiveResidualAtomPolarity::NonZero,
                GeneratedAffineResidualPriorAtomPolarity::NonZero,
            ),
        ] {
            assert_eq!(
                expected.atom_count(expected_polarity),
                actual.atom_count(actual_polarity)
            );
            for position in 0..expected.atom_count(expected_polarity) {
                let expected = expected.atom(expected_polarity, position).unwrap();
                let actual = actual.atom(actual_polarity, position).unwrap();
                assert_eq!(expected.locus_ordinal(), actual.locus_ordinal());
                assert!(std::ptr::eq(expected.polynomial(), actual.polynomial()));
            }
        }
        assert_eq!(
            expected.unsupported_reason_count(),
            actual.unsupported_reason_count()
        );
        for position in 0..expected.unsupported_reason_count() {
            assert!(std::ptr::eq(
                expected.unsupported_reason(position).unwrap(),
                actual.unsupported_reason(position).unwrap()
            ));
        }
    }

    fn assert_prior_projection_matches_current_owner(
        expected: GeneratedSectorAffineEffectiveResidualSourceView<'_>,
        actual: GeneratedAffineResidualPriorSourceView<'_>,
    ) {
        match (expected, actual) {
            (
                GeneratedSectorAffineEffectiveResidualSourceView::UnsupportedInventoryTerminal(
                    expected,
                ),
                GeneratedAffineResidualPriorSourceView::Unsupported(actual),
            ) => assert_unsupported_projection_matches_current_owner(expected, actual),
            (
                GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(
                    expected,
                )
                | GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(expected),
                GeneratedAffineResidualPriorSourceView::Actionable(actual),
            ) => assert_target_projection_matches_current_owner(expected, actual.target()),
            (
                GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalDomain(expected),
                GeneratedAffineResidualPriorSourceView::ExceptionalDomain(actual),
            )
            | (
                GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalLeak(expected),
                GeneratedAffineResidualPriorSourceView::ExceptionalLeak(actual),
            ) => {
                assert_target_projection_matches_current_owner(expected.target(), actual.target());
                assert_eq!(expected.predicate_count(), actual.predicate_count());
                for position in 0..expected.predicate_count() {
                    let expected = expected.predicate(position).unwrap();
                    let actual = actual.predicate(position).unwrap();
                    assert_eq!(expected.locus_ordinal(), actual.locus_ordinal());
                    assert_eq!(expected.kind(), actual.kind());
                    assert!(std::ptr::eq(expected.polynomial(), actual.polynomial()));
                }
            }
            _ => panic!("prior source projection changed its semantic outcome"),
        }
    }

    #[test]
    fn prior_effective_unified_views_dispatch_exactly_and_reject_bad_ordinals_and_authority() {
        let (family, context, source) =
            prior_effective_fixture("authority-prior-unified-view-private-family");
        assert!(!source.is_empty(), "fixture must retain residual work");
        let authority =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::clone(&source));
        authority.replay(&family, &context).unwrap();

        for ordinal in 0..source.len() {
            let strong_before = Arc::strong_count(&source);
            let expected = source.authenticated_source_view(ordinal).unwrap();
            let actual = authority.authenticated_source_view(ordinal).unwrap();
            assert_eq!(Arc::strong_count(&source), strong_before);
            assert_eq!(actual.work_item_ordinal(), ordinal);
            let GeneratedAffineResidualSourceView::PriorEffective(actual) = actual else {
                panic!("prior authority returned an initial-global view")
            };
            assert_eq!(
                effective_source_view_kind(actual),
                effective_source_view_kind(expected)
            );
            assert_eq!(actual.terminal().work_item_ordinal(), ordinal);
            assert_prior_projection_matches_current_owner(expected, actual);
            let rendered = format!("{actual:?}");
            assert!(rendered.contains("<redacted>"));
            assert!(!rendered.contains("polynomial:"));
            assert!(!rendered.contains("origin"));
            assert!(!rendered.contains(family.fingerprint_ref()));
            assert!(!rendered.contains(context.fingerprint()));
        }

        let out_of_range = authority
            .authenticated_source_view(authority.len())
            .unwrap_err();
        assert!(matches!(
            out_of_range,
            GeneratedAffineResidualSourceViewError::PriorEffective(
                GeneratedSectorAffineEffectiveResidualSourceViewError::WorkItemOutOfRange
            )
        ));
        let rendered = format!("{out_of_range} {out_of_range:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(std::error::Error::source(&out_of_range).is_none());

        let mut corrupted = source.as_ref().clone();
        assert!(corrupted.test_only_corrupt_first_authority());
        let corrupted =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::new(corrupted));
        let error = corrupted.authenticated_source_view(0).unwrap_err();
        assert!(matches!(
            error,
            GeneratedAffineResidualSourceViewError::PriorEffective(
                GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch
                    | GeneratedSectorAffineEffectiveResidualSourceViewError::ExceptionalAuthenticationFailed
            )
        ));
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(std::error::Error::source(&error).is_none());
    }

    fn effective_source_view_kind<T>(view: T) -> &'static str
    where
        T: EffectiveSourceViewKind,
    {
        view.kind()
    }

    trait EffectiveSourceViewKind {
        fn kind(self) -> &'static str;
    }

    impl EffectiveSourceViewKind for GeneratedSectorAffineEffectiveResidualSourceView<'_> {
        fn kind(self) -> &'static str {
            match self {
                GeneratedSectorAffineEffectiveResidualSourceView::UnsupportedInventoryTerminal(
                    _,
                ) => "unsupported",
                GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(_) => {
                    "actionable"
                }
                GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(_) => {
                    "actionable"
                }
                GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalDomain(_) => "domain",
                GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalLeak(_) => "leak",
            }
        }
    }

    impl EffectiveSourceViewKind for super::GeneratedAffineResidualPriorSourceView<'_> {
        fn kind(self) -> &'static str {
            match self {
                super::GeneratedAffineResidualPriorSourceView::Unsupported(_) => "unsupported",
                super::GeneratedAffineResidualPriorSourceView::Actionable(_) => "actionable",
                super::GeneratedAffineResidualPriorSourceView::ExceptionalDomain(_) => "domain",
                super::GeneratedAffineResidualPriorSourceView::ExceptionalLeak(_) => "leak",
            }
        }
    }

    #[test]
    fn initial_global_authority_preserves_metadata_replay_and_exact_arc_lifetime() {
        const WRONG_CONTEXT_SENTINEL: &str = "authority-initial-wrong-context-private";
        let (family, context, source) =
            initial_global_fixture("authority-initial-global-private-family");
        let expected_family_fingerprint = family.fingerprint_ref().to_owned();
        let expected_context_fingerprint = context.fingerprint().to_owned();
        let expected_sector = source.sector().clone();
        let expected_ordering = source.ordering();
        let expected_len = source.work_items().len();

        assert_eq!(Arc::strong_count(&source), 1);
        let weak = Arc::downgrade(&source);
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        assert_eq!(weak.strong_count(), 2);
        let clone = authority.clone();
        assert_eq!(weak.strong_count(), 3);
        drop(clone);
        assert_eq!(weak.strong_count(), 2);

        assert_eq!(
            authority.schema(),
            GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA
        );
        assert_eq!(
            authority.kind(),
            GeneratedAffineResidualSourceAuthorityKind::InitialGlobal
        );
        assert_eq!(authority.family_fingerprint(), expected_family_fingerprint);
        assert_eq!(
            authority.context_fingerprint(),
            expected_context_fingerprint
        );
        assert_eq!(authority.sector(), &expected_sector);
        assert_eq!(authority.ordering(), expected_ordering);
        assert_eq!(authority.arity(), expected_sector.arity());
        assert_eq!(authority.len(), expected_len);
        assert_eq!(authority.is_empty(), expected_len == 0);
        authority.replay(&family, &context).unwrap();
        assert_authority_debug_is_redacted(
            &authority,
            &expected_family_fingerprint,
            &expected_context_fingerprint,
        );

        drop(source);
        assert_eq!(weak.strong_count(), 1);
        authority.replay(&family, &context).unwrap();

        let wrong_family = equal_mass_two_loop_family("authority-initial-wrong-family-private");
        let wrong_family_error = authority.replay(&wrong_family, &context).unwrap_err();
        assert!(matches!(
            wrong_family_error,
            GeneratedAffineResidualSourceAuthorityError::InitialGlobal(_)
        ));
        assert_replay_error_is_redacted(
            &wrong_family_error,
            &[
                wrong_family.fingerprint_ref(),
                expected_family_fingerprint.as_str(),
            ],
        );
        let wrong_context = wrong_context(&context, WRONG_CONTEXT_SENTINEL);
        let wrong_context_error = authority.replay(&family, &wrong_context).unwrap_err();
        assert!(matches!(
            wrong_context_error,
            GeneratedAffineResidualSourceAuthorityError::InitialGlobal(_)
        ));
        assert_replay_error_is_redacted(
            &wrong_context_error,
            &[WRONG_CONTEXT_SENTINEL, wrong_context.fingerprint()],
        );

        drop(authority);
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn source_point_classification_metering_rejects_checked_count_overflow() {
        assert!(matches!(
            super::source_point_checked_mul("case scans", usize::MAX, 3),
            Err(
                GeneratedAffineResidualSourcePointError::ResourceCountOverflow {
                    resource: "case scans"
                }
            )
        ));
        assert!(matches!(
            super::source_point_bounded_add("candidate comparisons", usize::MAX, 1, usize::MAX,),
            Err(
                GeneratedAffineResidualSourcePointError::ResourceCountOverflow {
                    resource: "candidate comparisons"
                }
            )
        ));
    }

    #[test]
    fn source_neutral_point_classification_is_exact_bounded_and_version_preserving() {
        let (family, context, source) =
            initial_global_sector_fixture("authority-initial-point-classification-private", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(source);
        let point = [0, 1, 2];
        let classified = authority
            .classification_for_indices(
                &family,
                &context,
                &point,
                GeneratedAffineResidualSourcePointLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            classified.disposition(),
            GeneratedAffineResidualSourcePointDisposition::Work { .. }
        ));
        assert_eq!(
            classified.stats().kind(),
            Some(GeneratedAffineResidualSourceAuthorityKind::InitialGlobal)
        );
        assert!(classified.stats().scope_comparison_bytes() > 0);
        assert_eq!(classified.stats().index_entries(), point.len());
        assert!(classified.stats().initial_orthant_index_scans() > 0);
        assert!(classified.stats().initial_case_scans() > 0);
        assert!(classified.stats().initial_classification_scans() > 0);
        assert!(classified.stats().initial_predicate_scans() > 0);
        assert!(classified.stats().initial_predicate_evaluations() > 0);
        assert!(classified.stats().initial_specialization().source_terms() > 0);
        assert!(classified.stats().work_item_scans() > 0);
        assert!(
            classified
                .stats()
                .initial_disposition_candidate_comparisons()
                > 0,
            "the natural unsupported Work fixture must exercise candidate authentication"
        );
        assert!(authority.source_row_count() > 0);
        assert_eq!(
            authority.source_row(0).unwrap().family_fingerprint(),
            family.fingerprint()
        );

        assert!(matches!(
            authority
                .classification_for_indices(
                    &family,
                    &context,
                    &[1, 1, 2],
                    GeneratedAffineResidualSourcePointLimits::default(),
                )
                .unwrap()
                .disposition(),
            GeneratedAffineResidualSourcePointDisposition::Excluded
        ));
        let exact = exact_source_point_limits(classified.stats());
        let exact_classified = authority
            .classification_for_indices(&family, &context, &point, exact)
            .unwrap();
        assert_eq!(exact_classified.disposition(), classified.disposition());
        assert_eq!(exact_classified.stats(), classified.stats());

        macro_rules! source_one_below {
            ($field:ident, $getter:ident) => {{
                let requested = classified.stats().$getter();
                if requested > 0 {
                    let mut one_below = exact;
                    one_below.$field = requested - 1;
                    assert!(matches!(
                        authority.classification_for_indices(&family, &context, &point, one_below,),
                        Err(GeneratedAffineResidualSourcePointError::ResourceLimit { .. })
                    ));
                }
            }};
        }
        source_one_below!(max_scope_comparison_bytes, scope_comparison_bytes);
        source_one_below!(max_index_entries, index_entries);
        source_one_below!(max_initial_orthant_index_scans, initial_orthant_index_scans);
        source_one_below!(max_initial_case_scans, initial_case_scans);
        source_one_below!(
            max_initial_classification_scans,
            initial_classification_scans
        );
        source_one_below!(max_initial_predicate_scans, initial_predicate_scans);
        source_one_below!(
            max_initial_predicate_evaluations,
            initial_predicate_evaluations
        );
        source_one_below!(max_initial_work_item_scans, work_item_scans);
        source_one_below!(
            max_initial_disposition_candidate_comparisons,
            initial_disposition_candidate_comparisons
        );

        macro_rules! specialization_one_below {
            ($field:ident, $getter:ident) => {{
                let requested = classified.stats().initial_specialization().$getter();
                if requested > 0 {
                    let mut one_below = exact;
                    one_below.initial_specialization.$field = requested - 1;
                    assert!(matches!(
                        authority.classification_for_indices(&family, &context, &point, one_below,),
                        Err(GeneratedAffineResidualSourcePointError::ResourceLimit { .. })
                    ));
                }
            }};
        }
        specialization_one_below!(max_source_terms, source_terms);
        specialization_one_below!(max_source_exponent_entries, source_exponent_entries);
        specialization_one_below!(
            max_preflight_validation_source_term_scan_bound,
            preflight_validation_source_term_scan_bound
        );
        specialization_one_below!(
            max_preflight_validation_source_exponent_entry_scan_bound,
            preflight_validation_source_exponent_entry_scan_bound
        );
        specialization_one_below!(max_output_term_bound, output_term_bound);
        specialization_one_below!(max_output_exponent_entry_bound, output_exponent_entry_bound);
        specialization_one_below!(max_power_operation_bound, power_operation_bound);
        specialization_one_below!(
            max_largest_output_integer_bit_bound,
            largest_output_integer_bit_bound
        );
        specialization_one_below!(max_integer_bit_work_bound, integer_bit_work_bound);
        specialization_one_below!(max_retained_output_term_bound, retained_output_term_bound);
        specialization_one_below!(max_retained_output_byte_bound, retained_output_byte_bound);

        let (prior_family, prior_context, prior_source) =
            prior_effective_fixture("authority-prior-point-classification-private");
        let prior = GeneratedAffineResidualSourceAuthority::prior_effective(prior_source);
        let prior_outside = prior
            .classification_for_indices(
                &prior_family,
                &prior_context,
                &[1, 0, 1],
                GeneratedAffineResidualSourcePointLimits::default(),
            )
            .unwrap();
        assert_eq!(
            prior_outside.stats().kind(),
            Some(GeneratedAffineResidualSourceAuthorityKind::PriorEffective)
        );
        assert!(matches!(
            prior_outside.disposition(),
            GeneratedAffineResidualSourcePointDisposition::Excluded
        ));
        assert!(prior_outside.stats().prior_effective_owner().is_some());

        let prior_work = prior
            .classification_for_indices(
                &prior_family,
                &prior_context,
                &[-4, -4, 1],
                GeneratedAffineResidualSourcePointLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            prior_work.disposition(),
            GeneratedAffineResidualSourcePointDisposition::Work { .. }
        ));
        assert!(prior_work.stats().prior_effective_owner().is_some());
        assert!(prior_work.stats().work_item_scans() > 0);
        assert_prior_source_exact_and_every_positive_one_below(
            &prior,
            &prior_family,
            &prior_context,
            &[-4, -4, 1],
            prior_work,
        );

        let wrong_context = wrong_context(&context, "authority-point-wrong-context-private");
        assert!(matches!(
            authority.classification_for_indices(
                &family,
                &wrong_context,
                &point,
                GeneratedAffineResidualSourcePointLimits::default(),
            ),
            Err(GeneratedAffineResidualSourcePointError::WrongContext)
        ));
        assert!(matches!(
            authority.classification_for_indices(
                &family,
                &context,
                &point[..point.len() - 1],
                GeneratedAffineResidualSourcePointLimits::default(),
            ),
            Err(GeneratedAffineResidualSourcePointError::WrongArity)
        ));

        let rendered = format!(
            "{:?}",
            authority
                .classification_for_indices(
                    &equal_mass_two_loop_family("authority-point-wrong-private"),
                    &context,
                    &point,
                    GeneratedAffineResidualSourcePointLimits::default(),
                )
                .unwrap_err()
        );
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("authority-point-wrong-private"));
    }

    #[test]
    fn prior_effective_authority_preserves_metadata_replay_and_exact_arc_lifetime() {
        const WRONG_CONTEXT_SENTINEL: &str = "authority-prior-wrong-context-private";
        let (family, context, source) =
            prior_effective_fixture("authority-prior-effective-private-family");
        let expected_family_fingerprint = family.fingerprint_ref().to_owned();
        let expected_context_fingerprint = context.fingerprint().to_owned();
        let expected_sector = source.owner().source_queue().sector().clone();
        let expected_ordering = source.owner().source_queue().ordering();
        let expected_len = source.len();

        assert_eq!(Arc::strong_count(&source), 1);
        let weak = Arc::downgrade(&source);
        let authority =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::clone(&source));
        assert_eq!(weak.strong_count(), 2);
        let clone = authority.clone();
        assert_eq!(weak.strong_count(), 3);
        drop(clone);
        assert_eq!(weak.strong_count(), 2);

        assert_eq!(
            authority.schema(),
            GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA
        );
        assert_eq!(
            authority.kind(),
            GeneratedAffineResidualSourceAuthorityKind::PriorEffective
        );
        assert_eq!(authority.family_fingerprint(), expected_family_fingerprint);
        assert_eq!(
            authority.context_fingerprint(),
            expected_context_fingerprint
        );
        assert_eq!(authority.sector(), &expected_sector);
        assert_eq!(authority.ordering(), expected_ordering);
        assert_eq!(authority.arity(), expected_sector.arity());
        assert_eq!(authority.len(), expected_len);
        assert_eq!(authority.is_empty(), expected_len == 0);
        authority.replay(&family, &context).unwrap();
        assert_authority_debug_is_redacted(
            &authority,
            &expected_family_fingerprint,
            &expected_context_fingerprint,
        );

        drop(source);
        assert_eq!(weak.strong_count(), 1);
        authority.replay(&family, &context).unwrap();

        let wrong_family = equal_mass_two_loop_family("authority-prior-wrong-family-private");
        let wrong_family_error = authority.replay(&wrong_family, &context).unwrap_err();
        assert!(matches!(
            wrong_family_error,
            GeneratedAffineResidualSourceAuthorityError::PriorEffective(_)
        ));
        assert_replay_error_is_redacted(
            &wrong_family_error,
            &[
                wrong_family.fingerprint_ref(),
                expected_family_fingerprint.as_str(),
            ],
        );
        let wrong_context = wrong_context(&context, WRONG_CONTEXT_SENTINEL);
        let wrong_context_error = authority.replay(&family, &wrong_context).unwrap_err();
        assert!(matches!(
            wrong_context_error,
            GeneratedAffineResidualSourceAuthorityError::PriorEffective(_)
        ));
        assert_replay_error_is_redacted(
            &wrong_context_error,
            &[WRONG_CONTEXT_SENTINEL, wrong_context.fingerprint()],
        );

        drop(authority);
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn replay_error_redacts_nested_operational_detail() {
        const SENTINEL: &str = "private-source-family-and-predicate-sentinel";
        let error = GeneratedAffineResidualSourceAuthorityError::InitialGlobal(
            GeneratedSectorLiveLeafQueueError::ReplayMismatch { detail: SENTINEL },
        );
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(SENTINEL));
        assert!(std::error::Error::source(&error).is_none());

        let prior = GeneratedAffineResidualSourceAuthorityError::PriorEffective(
            GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit {
                resource: SENTINEL,
                requested: 2,
                limit: 1,
            },
        );
        let rendered = format!("{prior} {prior:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(SENTINEL));
        assert!(std::error::Error::source(&prior).is_none());
    }
}
