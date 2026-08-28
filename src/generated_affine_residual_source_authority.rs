//! Sealed source authority for generated residual-affine epochs.
//!
//! The initial epoch consumes the global live-leaf queue through one sealed
//! `Arc` handle. It exposes only common scope metadata plus deterministic
//! replay; semantic source views remain a separate, narrower boundary.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::generated_affine_initial_global_affine_terminal::{
    GeneratedAffineInitialGlobalAffineBoundTerminal, GeneratedAffineInitialGlobalAffineTerminal,
    GeneratedAffineInitialGlobalAffineTerminalSourceView,
};
use crate::product_locus_boolean_cover::ResidualProductLocusBooleanReplaySession;
use crate::{
    COORDINATE_EQUALITY_LOCUS_V1_SCHEMA, CoordinateEqualityLeafStatus,
    GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA, GeneratedSectorLiveLeafOutcome,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorQueuedSourceDisposition, IntegralFamily, IntegralOrderingPolicy,
    PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA, ParametricCoefficientContext, ParametricPolynomial,
    ParametricRelation, ParametricSectorCoverageError, ParametricSectorLeafDisposition,
    ParametricSectorProductZeroDecomposition, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemLimits,
    ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanCoverStats,
    ResidualProductLocusBooleanNodeOutcome, SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA, SectorMask,
    SymbolicPolynomialPredicate, SymbolicPolynomialPredicateKind, SymbolicSectorCase,
    SymbolicSectorCaseId,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-source-authority-v1";

/// Which authenticated residual source feeds one generated affine epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualSourceAuthorityKind {
    InitialGlobal,
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
/// initial-global authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourceNavigationLimits {
    pub(crate) max_source_view_resolutions: usize,
    pub(crate) max_initial_case_lookup_comparisons: usize,
    pub(crate) max_initial_disposition_candidate_comparisons: usize,
}

impl Default for GeneratedAffineResidualSourceNavigationLimits {
    fn default() -> Self {
        Self {
            max_source_view_resolutions: 1,
            max_initial_case_lookup_comparisons: usize::BITS as usize + 1,
            max_initial_disposition_candidate_comparisons: 1_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourceNavigationStats {
    source_view_resolutions: usize,
    initial_case_lookup_comparisons: usize,
    initial_disposition_candidate_comparisons: usize,
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
}

/// Aggregate bounds for one exact point lookup through the initial-global
/// source. Classification evaluates the frozen global case partition and then
/// performs one complete, uniqueness-checking queue scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSourcePointLimits {
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

/// Exact outer work performed by one successful initial-global point lookup.
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
            Self::SourceView(_) => formatter.write_str("residual source point navigation failed"),
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked during residual source point classification"),
        }
    }
}

// Nested proof diagnostics remain redacted at this source-version boundary.
impl std::error::Error for GeneratedAffineResidualSourcePointError {}

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

/// Source-neutral lifetime-bound input for one generated affine epoch.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedAffineResidualSourceView<'source> {
    InitialGlobal(GeneratedAffineInitialGlobalSourceView<'source>),
}

impl GeneratedAffineResidualSourceView<'_> {
    pub(crate) const fn work_item_ordinal(self) -> usize {
        match self {
            Self::InitialGlobal(view) => view.terminal().work_item_ordinal(),
        }
    }
}

/// One sealed, version-preserving source allocation for an affine epoch.
///
/// The concrete source variant is module-private. Sibling modules may create
/// an authority through the typed constructors and use its common operations,
/// but cannot pattern-match it to recover the retained raw source `Arc`. No source
/// fabricates a queue or copies predicates, affine maps, guards, or relations.
/// Cloning this authority clones only its single retained `Arc` handle.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualSourceAuthority {
    inner: GeneratedAffineResidualSourceAuthorityInner,
}

#[derive(Clone)]
enum GeneratedAffineResidualSourceAuthorityInner {
    InitialGlobal(Arc<GeneratedSectorLiveLeafQueueCertificate>),
}

/// One successful replay of the exact source authority for a complete affine
/// compilation batch.
///
/// Sessions retain the unforgeable V1 queue-replay token needed by positional
/// child compilation.
pub(crate) struct GeneratedAffineResidualSourceReplaySession<'scope> {
    authority: &'scope GeneratedAffineResidualSourceAuthority,
    inner: GeneratedAffineResidualSourceReplaySessionInner<'scope>,
}

enum GeneratedAffineResidualSourceReplaySessionInner<'scope> {
    InitialGlobal(ResidualProductLocusBooleanReplaySession<'scope>),
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

    pub(crate) const fn schema(&self) -> &'static str {
        GENERATED_AFFINE_RESIDUAL_SOURCE_AUTHORITY_V1_SCHEMA
    }

    pub(crate) const fn kind(&self) -> GeneratedAffineResidualSourceAuthorityKind {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(_) => {
                GeneratedAffineResidualSourceAuthorityKind::InitialGlobal
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

    /// Number of generated parametric source rows behind this source.
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
        };
        Ok(GeneratedAffineResidualSourceReplaySession {
            authority: self,
            inner,
        })
    }

    /// Exact allocation identity without exposing the retained source.
    pub(crate) fn same_source_allocation(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(left),
                GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(right),
            ) => Arc::ptr_eq(left, right),
        }
    }

    /// Resolve one source-ordered ordinal through the exact retained variant.
    ///
    /// This accepts no caller-created locator.  The returned references are
    /// tied to this authority borrow and does not expose its owning
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
            }
            let view = self
                .authenticated_source_view(work_item_ordinal)
                .map_err(GeneratedAffineResidualSourcePointError::SourceView)?;
            if view.work_item_ordinal() != work_item_ordinal {
                return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
            }
            let GeneratedAffineResidualSourceView::InitialGlobal(initial) = view;
            let terminal = initial.terminal();
            if terminal.case_lookup_comparisons() > stats.initial_case_lookup_comparisons
                || terminal.source_disposition_candidate_comparisons()
                    != stats.initial_disposition_candidate_comparisons
            {
                return Err(GeneratedAffineResidualSourcePointError::AuthorityMismatch);
            }
            Ok((view, stats))
        }))
        .map_err(|_| GeneratedAffineResidualSourcePointError::SymbolicaPanic)?
    }

    /// Replay the retained source allocation without reconstructing it.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualSourceAuthorityError> {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => source
                .replay(family, context)
                .map_err(GeneratedAffineResidualSourceAuthorityError::InitialGlobal),
        }
    }

    /// The source scope is the exact retained initial-global queue.
    fn initial_scope(&self) -> &GeneratedSectorLiveLeafQueueCertificate {
        match &self.inner {
            GeneratedAffineResidualSourceAuthorityInner::InitialGlobal(source) => source.as_ref(),
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

/// Redacted authentication failure at the sealed source dispatch seam.
pub(crate) enum GeneratedAffineResidualSourceViewError {
    InitialGlobalSchemaMismatch,
    InitialGlobalWorkItemOutOfRange,
    InitialGlobalAuthorityMismatch,
    ReplaySessionMismatch,
}

impl fmt::Debug for GeneratedAffineResidualSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::InitialGlobalSchemaMismatch => "InitialGlobalSchemaMismatch",
            Self::InitialGlobalWorkItemOutOfRange => "InitialGlobalWorkItemOutOfRange",
            Self::InitialGlobalAuthorityMismatch => "InitialGlobalAuthorityMismatch",
            Self::ReplaySessionMismatch => "ReplaySessionMismatch",
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
    BindingCensusMismatch,
    BindingMismatch,
    ReplaySessionRequired,
    ReplaySessionMismatch,
    TerminalNotReady,
    AffineBranch,
    AffineTerminal,
    ResourceCountOverflow { resource: &'static str },
    V1Cover(ResidualProductLocusBooleanCoverError),
}

impl fmt::Debug for GeneratedAffineInitialGlobalBooleanCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::BindingCensusMismatch => "BindingCensusMismatch",
            Self::BindingMismatch => "BindingMismatch",
            Self::ReplaySessionRequired => "ReplaySessionRequired",
            Self::ReplaySessionMismatch => "ReplaySessionMismatch",
            Self::TerminalNotReady => "TerminalNotReady",
            Self::AffineBranch => "AffineBranch",
            Self::AffineTerminal => "AffineTerminal",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
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
}

impl fmt::Debug for GeneratedAffineResidualSourceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::InitialGlobal(_) => "InitialGlobal",
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
        }
    }
}

// Deliberately do not delegate `Error::source`: the wrapped V1 error formats
// operational detail, while this seam promises redacted diagnostics.
impl std::error::Error for GeneratedAffineResidualSourceAuthorityError {}
