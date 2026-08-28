//! Exact finite composition of generated parametric rule domains over a sector.
//!
//! A single [`crate::GeneratedWhenBadCertificate`] proves where one candidate
//! obtained from freshly generated IBP/LI identities is admissible.  This
//! module combines several such candidates, in caller-persisted priority
//! order, into one finite structural partition of a [`crate::SectorMask`].
//! Every final leaf is explicitly classified as covered by a descending rule,
//! uncovered, or unsupported.  In particular, exhausting the supplied
//! candidates never manufactures a master integral.
//!
//! Composition follows LiteRed's ordered uncovered-case workflow.  The
//! authenticated bad-domain formula of each candidate is tri-evaluated on
//! every leaf not covered by an earlier candidate; a true bad clause keeps
//! the leaf open immediately, so irrelevant prefixes from a local `WhenBad`
//! decision tree cannot create a Cartesian product.  A predicate is reused
//! when bounded exact division proves that two polynomials differ by a unit
//! of the formal coefficient field `K = Q(theta)`, and the two sound
//! divisibility implications in `K[n]` are used while tri-evaluating atoms.
//! Contradictory coordinate branches remain in the final partition with
//! replayed `ProvedEmptyLocus` witnesses.  General polynomial contradictions
//! and radical-ideal equivalence deliberately remain unresolved.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::direct_bad_formula::{
    DirectBadFormulaClause, DirectBadFormulaRoute, DirectBadFormulaTruth, route_direct_bad_formula,
};
use crate::{
    CoordinateEqualityLocusError, CoordinateEqualityLocusLimits,
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanError, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
    GeneratedWhenBadError, GeneratedWhenBadLimits, IntegralFamily, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricPolynomial, ParametricReductionRuleCandidate, SectorMask,
    SymbolicPolynomialPredicateKind, SymbolicSectorCaseError, SymbolicSectorCaseId,
    SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
    SymbolicSectorCasePartitionCertificate, WhenBadLeakNumeratorGate,
};

/// Stable schema for exact finite candidate-domain composition.
pub const PARAMETRIC_SECTOR_COVERAGE_V1_SCHEMA: &str = "rustred-parametric-sector-coverage-v1";
/// Schema with ordered uncovered-domain composition and explicit retained
/// coordinate-contradiction leaves.
pub const PARAMETRIC_SECTOR_COVERAGE_V2_SCHEMA: &str = "rustred-parametric-sector-coverage-v2";
/// Schema with direct authenticated bad-formula composition.  A descending
/// leaf stores the candidate ordinal; concrete application replays that
/// candidate's local `WhenBad` classification instead of pretending that a
/// compact global leaf corresponds to one fixed local case.
pub const PARAMETRIC_SECTOR_COVERAGE_V3_SCHEMA: &str = "rustred-parametric-sector-coverage-v3";
/// Schema retaining the canonical structural-locus table and exact checked
/// product-zero decompositions used by bounded bad-domain compression.  The
/// same structure also records a limits-selected factored fallback through
/// ordinary structural predicates, its partition transcript, and its census.
pub const PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA: &str = "rustred-parametric-sector-coverage-v4";

/// Per-candidate and aggregate checked work/retention budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricSectorCoverageLimits {
    pub generated_when_bad: GeneratedWhenBadLimits,
    pub sector_cases: SymbolicSectorCaseLimits,
    /// Exact bounded recognizer used only to stop revisiting structurally
    /// retained leaves whose coordinate predicates prove them empty.
    pub coordinate_loci: CoordinateEqualityLocusLimits,
    pub max_candidates: usize,
    pub max_unique_predicates: usize,
    pub max_candidate_partition_leaves: usize,
    pub max_candidate_predicate_instances: usize,
    pub max_candidate_bad_clauses: usize,
    pub max_candidate_bad_atoms: usize,
    /// Cumulative tri-valued evaluations of retained direct bad-domain
    /// formulas across every visited global case.
    pub max_direct_bad_formula_evaluations: usize,
    /// Conservative cumulative atom-query charge: every evaluation is charged
    /// the formula's complete retained atom count before routing, independent
    /// of Boolean short-circuiting or the selected split.
    pub max_direct_bad_formula_atom_queries: usize,
    pub max_global_leaf_classifications: usize,
    pub max_candidate_leaf_match_attempts: usize,
    pub max_unsupported_references: usize,
    pub max_total_canonical_rows: usize,
    pub max_total_canonical_terms: usize,
    pub max_total_retained_source_rows: usize,
    pub max_total_retained_source_terms: usize,
    pub max_total_source_match_attempts: usize,
    pub max_total_candidate_binding_bytes: usize,
    pub max_total_condition_terms: usize,
    pub max_total_condition_bytes: usize,
    pub max_coordinate_pruning_checks: usize,
    pub max_locus_divisibility_checks: usize,
    /// Terms in the certificate-owned, first-seen canonical structural-locus
    /// table. Each locus is charged once even when several candidates reuse it.
    pub max_retained_structural_locus_terms: usize,
    /// Aggregate bounded canonical-display bytes of the structural-locus table.
    pub max_retained_structural_locus_bytes: usize,
    /// Distinct exact product-zero decomposition witnesses retained globally.
    pub max_product_zero_decompositions: usize,
    /// Aggregate factor-ordinal references in retained decomposition witnesses.
    pub max_product_zero_factor_references: usize,
    /// Checked Symbolica polynomial multiplications performed while creating
    /// distinct canonical product-zero compression witnesses. Identical
    /// canonical factor lists are deduplicated before multiplication.
    pub max_product_zero_multiplications: usize,
    /// Largest conservative whole-product support envelope which may be
    /// materialized as one concrete product-zero locus.
    ///
    /// A canonical factor list whose envelope exceeds this representation
    /// cutoff remains an ordered disjunction of its factor-zero atoms.  That
    /// exact fallback performs no Symbolica multiplication and retains no
    /// product/decomposition witness.  This cutoff is stored in the coverage
    /// certificate, so replay makes the same representation decision.
    pub max_materialized_product_zero_support_terms: usize,
    /// Aggregate canonical factors whose sparse supports may be inspected
    /// while deciding whether to materialize a product-zero locus.
    pub max_product_materialization_bound_factor_scans: usize,
    /// Aggregate flat exponent entries inspected by whole-product support
    /// bounds.  The bound computes every factor's componentwise degrees in one
    /// pass over this payload.
    pub max_product_materialization_bound_exponent_entries: usize,
    /// Candidate product-zero disjunctions retained in canonical factored
    /// form because their whole-product support exceeds the materialization
    /// cutoff.
    pub max_factored_product_zero_disjunctions: usize,
    /// Aggregate canonical factor atoms retained by factored product-zero
    /// disjunctions.  These are formula references, not decomposition-witness
    /// references.
    pub max_factored_product_zero_factor_references: usize,
    /// Aggregate candidate monomial pairs admitted before checked product
    /// reconstruction. This budget is consumed across all witnesses.
    pub max_product_reconstruction_term_pairs: usize,
    /// Per-multiplication conservative native-output support envelope.
    ///
    /// This is deliberately distinct from
    /// [`crate::algebra::ExactAlgebraLimits::max_polynomial_terms`]: the latter still
    /// bounds both authenticated inputs and the actual canonical output.  The
    /// transient envelope only admits a proved direct-polynomial support bound
    /// before Symbolica constructs an output which may canonicalize smaller.
    pub max_product_reconstruction_native_output_term_bound: usize,
    /// Aggregate sparse output terms produced by checked reconstruction.
    pub max_product_reconstruction_output_terms: usize,
    /// Aggregate dense exponent entries in checked reconstruction outputs.
    pub max_product_reconstruction_output_exponent_entries: usize,
    /// Aggregate integer-coefficient magnitude-bit payload in checked
    /// reconstruction outputs. A conservative collision bound is checked
    /// before native multiplication; this statistic records the exact output.
    pub max_product_reconstruction_output_coefficient_bits: usize,
    /// Aggregate exact K-unit-associate comparisons used to canonicalize the
    /// structural-locus table, including product representatives.
    pub max_structural_locus_associate_comparisons: usize,
    /// Aggregate checked division term-pair bound for non-identical
    /// structural-locus associate comparisons.
    pub max_structural_locus_associate_term_pairs: usize,
}

impl Default for ParametricSectorCoverageLimits {
    fn default() -> Self {
        Self {
            generated_when_bad: GeneratedWhenBadLimits::default(),
            sector_cases: SymbolicSectorCaseLimits::default(),
            coordinate_loci: CoordinateEqualityLocusLimits::default(),
            max_candidates: 4_096,
            max_unique_predicates: 4_096,
            max_candidate_partition_leaves: 4_000_000,
            max_candidate_predicate_instances: 16_000_000,
            max_candidate_bad_clauses: 16_000_000,
            max_candidate_bad_atoms: 32_000_000,
            max_direct_bad_formula_evaluations: 16_000_000,
            max_direct_bad_formula_atom_queries: 512_000_000,
            max_global_leaf_classifications: 4_000_000,
            max_candidate_leaf_match_attempts: 16_000_000,
            max_unsupported_references: 16_000_000,
            max_total_canonical_rows: 1_000_000,
            max_total_canonical_terms: 16_000_000,
            max_total_retained_source_rows: 1_000_000,
            max_total_retained_source_terms: 16_000_000,
            max_total_source_match_attempts: 16_000_000,
            max_total_candidate_binding_bytes: 512 * 1024 * 1024,
            max_total_condition_terms: 32_000_000,
            max_total_condition_bytes: 2 * 1024 * 1024 * 1024,
            max_coordinate_pruning_checks: 4_096,
            max_locus_divisibility_checks: 16_000_000,
            max_retained_structural_locus_terms: 32_000_000,
            max_retained_structural_locus_bytes: 2 * 1024 * 1024 * 1024,
            max_product_zero_decompositions: 16_000_000,
            max_product_zero_factor_references: 32_000_000,
            max_product_zero_multiplications: 32_000_000,
            max_materialized_product_zero_support_terms: 4_000_000,
            max_product_materialization_bound_factor_scans: 32_000_000,
            max_product_materialization_bound_exponent_entries: 4_000_000_000,
            max_factored_product_zero_disjunctions: 16_000_000,
            max_factored_product_zero_factor_references: 32_000_000,
            max_product_reconstruction_term_pairs: 256_000_000,
            max_product_reconstruction_native_output_term_bound: 1 << 22,
            max_product_reconstruction_output_terms: 256_000_000,
            max_product_reconstruction_output_exponent_entries: 4_000_000_000,
            max_product_reconstruction_output_coefficient_bits: 4_000_000_000,
            max_structural_locus_associate_comparisons: 32_000_000,
            max_structural_locus_associate_term_pairs: 512_000_000,
        }
    }
}

/// One generated-source-authenticated attempt in persisted priority order.
#[derive(Clone, Debug)]
pub struct SectorCoverageCandidateAttempt {
    ordinal: usize,
    compilation: GeneratedWhenBadCompilation,
}

impl SectorCoverageCandidateAttempt {
    pub(crate) const fn from_compilation(
        ordinal: usize,
        compilation: GeneratedWhenBadCompilation,
    ) -> Self {
        Self {
            ordinal,
            compilation,
        }
    }

    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn compilation(&self) -> &GeneratedWhenBadCompilation {
        &self.compilation
    }

    pub const fn is_certified(&self) -> bool {
        matches!(self.compilation, GeneratedWhenBadCompilation::Certified(_))
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal && self.compilation.payload_eq(&other.compilation)
    }
}

/// Terminal status of one global structural leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricSectorLeafDisposition {
    /// The first applicable candidate in persisted priority order.
    DescendingRule { candidate_ordinal: usize },
    /// This retained structural branch has no integer point in the sector.
    /// It is not an analytic zero-sector statement and does not classify any
    /// integral as zero.
    ProvedEmptyLocus {
        reason: ParametricSectorEmptyLocusReason,
    },
    /// No supplied certified candidate covers the leaf.
    Uncovered,
    /// No certified candidate covers the leaf and one or more authenticated
    /// candidates require a proof capability not implemented by `WhenBad`.
    Unsupported { candidate_ordinals: Box<[usize]> },
}

/// Exact witness that one retained conjunction is empty.  Coordinate reasons
/// use associates of `n_i-c`; the divisibility reason uses only the two valid
/// integral-domain implications for an exact quotient in `K[n]`. Predicate
/// ordinals refer to the final global case's predicate list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricSectorEmptyLocusReason {
    OrthantViolation {
        equality_predicate_ordinal: usize,
        index: usize,
        value: i64,
        side: crate::SectorOrthantSide,
    },
    ConflictingFixedValues {
        first_equality_predicate_ordinal: usize,
        second_equality_predicate_ordinal: usize,
        index: usize,
        first_value: i64,
        second_value: i64,
    },
    EqualityNonzeroContradiction {
        equality_predicate_ordinal: usize,
        nonzero_predicate_ordinal: usize,
        index: usize,
        value: i64,
    },
    /// `zero_polynomial | nonzero_polynomial` in the integral domain `K[n]`.
    PolynomialDivisibilityContradiction {
        zero_predicate_ordinal: usize,
        nonzero_predicate_ordinal: usize,
    },
}

/// One terminal classification, keyed by the global partition case id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricSectorLeafClassification {
    case: SymbolicSectorCaseId,
    disposition: ParametricSectorLeafDisposition,
}

impl ParametricSectorLeafClassification {
    pub const fn case(&self) -> SymbolicSectorCaseId {
        self.case
    }

    pub const fn disposition(&self) -> &ParametricSectorLeafDisposition {
        &self.disposition
    }
}

/// Exact provenance for one compiler-created product-zero compression.
///
/// All ordinals address [`ParametricSectorCoverageCertificate::structural_loci`].
/// Factors are strictly increasing, duplicate-free, and contain at least two
/// loci; one-factor no-op compressions are not retained. Base-field units are
/// omitted because their equality loci are empty over `K[n]`. Exact replay
/// multiplies these representatives and proves that the result is a nonzero
/// `K`-unit associate of the retained product representative; literal
/// polynomial equality is not required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricSectorProductZeroDecomposition {
    product_locus_ordinal: usize,
    factor_locus_ordinals: Box<[usize]>,
}

impl ParametricSectorProductZeroDecomposition {
    pub const fn product_locus_ordinal(&self) -> usize {
        self.product_locus_ordinal
    }

    pub fn factor_locus_ordinals(&self) -> &[usize] {
        &self.factor_locus_ordinals
    }
}

/// Aggregate checked work and retained-proof census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParametricSectorCoverageStats {
    shared_row_span_certificates: usize,
    shared_row_span_candidate_reuses: usize,
    candidates: usize,
    certified_candidates: usize,
    unsupported_candidates: usize,
    unique_predicates: usize,
    candidate_partition_leaves: usize,
    candidate_predicate_instances: usize,
    candidate_bad_clauses: usize,
    candidate_bad_atoms: usize,
    direct_bad_formula_evaluations: usize,
    direct_bad_formula_atom_queries: usize,
    global_leaves: usize,
    descending_leaves: usize,
    uncovered_leaves: usize,
    unsupported_leaves: usize,
    proved_empty_locus_leaves: usize,
    candidate_leaf_match_attempts: usize,
    unsupported_references: usize,
    canonical_rows: usize,
    canonical_terms: usize,
    retained_source_rows: usize,
    retained_source_terms: usize,
    source_match_attempts: usize,
    candidate_binding_bytes: usize,
    condition_terms: usize,
    condition_bytes: usize,
    coordinate_pruning_checks: usize,
    coordinate_pruned_leaves: usize,
    divisibility_pruned_leaves: usize,
    locus_divisibility_checks: usize,
    retained_structural_locus_terms: usize,
    retained_structural_locus_bytes: usize,
    product_zero_decompositions: usize,
    product_zero_factor_references: usize,
    product_zero_multiplications: usize,
    product_materialization_bound_factor_scans: usize,
    product_materialization_bound_exponent_entries: usize,
    factored_product_zero_disjunctions: usize,
    factored_product_zero_factor_references: usize,
    product_reconstruction_term_pairs: usize,
    product_reconstruction_output_terms: usize,
    product_reconstruction_output_exponent_entries: usize,
    product_reconstruction_output_coefficient_bits: usize,
    structural_locus_associate_comparisons: usize,
    structural_locus_associate_term_pairs: usize,
}

impl ParametricSectorCoverageStats {
    pub const fn shared_row_span_certificates(self) -> usize {
        self.shared_row_span_certificates
    }
    pub const fn shared_row_span_candidate_reuses(self) -> usize {
        self.shared_row_span_candidate_reuses
    }
    pub const fn candidates(self) -> usize {
        self.candidates
    }
    pub const fn certified_candidates(self) -> usize {
        self.certified_candidates
    }
    pub const fn unsupported_candidates(self) -> usize {
        self.unsupported_candidates
    }
    pub const fn unique_predicates(self) -> usize {
        self.unique_predicates
    }
    pub const fn candidate_partition_leaves(self) -> usize {
        self.candidate_partition_leaves
    }
    pub const fn candidate_predicate_instances(self) -> usize {
        self.candidate_predicate_instances
    }
    pub const fn candidate_bad_clauses(self) -> usize {
        self.candidate_bad_clauses
    }
    pub const fn candidate_bad_atoms(self) -> usize {
        self.candidate_bad_atoms
    }
    pub const fn direct_bad_formula_evaluations(self) -> usize {
        self.direct_bad_formula_evaluations
    }
    pub const fn direct_bad_formula_atom_queries(self) -> usize {
        self.direct_bad_formula_atom_queries
    }
    pub const fn global_leaves(self) -> usize {
        self.global_leaves
    }
    pub const fn descending_leaves(self) -> usize {
        self.descending_leaves
    }
    pub const fn uncovered_leaves(self) -> usize {
        self.uncovered_leaves
    }
    pub const fn unsupported_leaves(self) -> usize {
        self.unsupported_leaves
    }
    pub const fn proved_empty_locus_leaves(self) -> usize {
        self.proved_empty_locus_leaves
    }
    pub const fn candidate_leaf_match_attempts(self) -> usize {
        self.candidate_leaf_match_attempts
    }
    pub const fn unsupported_references(self) -> usize {
        self.unsupported_references
    }
    pub const fn canonical_rows(self) -> usize {
        self.canonical_rows
    }
    pub const fn canonical_terms(self) -> usize {
        self.canonical_terms
    }
    pub const fn retained_source_rows(self) -> usize {
        self.retained_source_rows
    }
    pub const fn retained_source_terms(self) -> usize {
        self.retained_source_terms
    }
    pub const fn source_match_attempts(self) -> usize {
        self.source_match_attempts
    }
    pub const fn candidate_binding_bytes(self) -> usize {
        self.candidate_binding_bytes
    }
    pub const fn condition_terms(self) -> usize {
        self.condition_terms
    }
    pub const fn condition_bytes(self) -> usize {
        self.condition_bytes
    }
    pub const fn coordinate_pruning_checks(self) -> usize {
        self.coordinate_pruning_checks
    }
    pub const fn coordinate_pruned_leaves(self) -> usize {
        self.coordinate_pruned_leaves
    }
    pub const fn divisibility_pruned_leaves(self) -> usize {
        self.divisibility_pruned_leaves
    }
    pub const fn locus_divisibility_checks(self) -> usize {
        self.locus_divisibility_checks
    }
    pub const fn retained_structural_locus_terms(self) -> usize {
        self.retained_structural_locus_terms
    }
    pub const fn retained_structural_locus_bytes(self) -> usize {
        self.retained_structural_locus_bytes
    }
    pub const fn product_zero_decompositions(self) -> usize {
        self.product_zero_decompositions
    }
    pub const fn product_zero_factor_references(self) -> usize {
        self.product_zero_factor_references
    }
    pub const fn product_zero_multiplications(self) -> usize {
        self.product_zero_multiplications
    }
    pub const fn product_materialization_bound_factor_scans(self) -> usize {
        self.product_materialization_bound_factor_scans
    }
    pub const fn product_materialization_bound_exponent_entries(self) -> usize {
        self.product_materialization_bound_exponent_entries
    }
    pub const fn factored_product_zero_disjunctions(self) -> usize {
        self.factored_product_zero_disjunctions
    }
    pub const fn factored_product_zero_factor_references(self) -> usize {
        self.factored_product_zero_factor_references
    }
    pub const fn product_reconstruction_term_pairs(self) -> usize {
        self.product_reconstruction_term_pairs
    }
    pub const fn product_reconstruction_output_terms(self) -> usize {
        self.product_reconstruction_output_terms
    }
    pub const fn product_reconstruction_output_exponent_entries(self) -> usize {
        self.product_reconstruction_output_exponent_entries
    }
    pub const fn product_reconstruction_output_coefficient_bits(self) -> usize {
        self.product_reconstruction_output_coefficient_bits
    }
    pub const fn structural_locus_associate_comparisons(self) -> usize {
        self.structural_locus_associate_comparisons
    }
    pub const fn structural_locus_associate_term_pairs(self) -> usize {
        self.structural_locus_associate_term_pairs
    }
}

/// Replayable, finite structural cover of one exact sector orthant.
#[derive(Clone, Debug)]
pub struct ParametricSectorCoverageCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    attempts: Box<[SectorCoverageCandidateAttempt]>,
    structural_loci: Box<[ParametricPolynomial]>,
    product_zero_decompositions: Box<[ParametricSectorProductZeroDecomposition]>,
    partition: SymbolicSectorCasePartitionCertificate,
    classifications: Box<[ParametricSectorLeafClassification]>,
    limits: ParametricSectorCoverageLimits,
    stats: ParametricSectorCoverageStats,
}

impl ParametricSectorCoverageCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.row_span.as_ref()
    }
    pub fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        &self.row_span
    }
    pub fn candidate_attempts(&self) -> &[SectorCoverageCandidateAttempt] {
        &self.attempts
    }
    /// Canonical first-seen representatives for every structural equality
    /// locus used by composition and product decomposition provenance.
    pub fn structural_loci(&self) -> &[ParametricPolynomial] {
        &self.structural_loci
    }
    pub fn structural_locus(&self, ordinal: usize) -> Option<&ParametricPolynomial> {
        self.structural_loci.get(ordinal)
    }
    /// Find the ordinal of an exact retained representative. Case predicates
    /// are cloned from this table, so no factoring or associate search is
    /// needed by downstream queue/start construction.
    pub fn structural_locus_ordinal_exact(
        &self,
        polynomial: &ParametricPolynomial,
    ) -> Option<usize> {
        self.structural_loci
            .iter()
            .position(|retained| retained == polynomial)
    }
    pub fn product_zero_decompositions(&self) -> &[ParametricSectorProductZeroDecomposition] {
        &self.product_zero_decompositions
    }
    pub fn product_zero_decompositions_for_locus(
        &self,
        product_locus_ordinal: usize,
    ) -> impl Iterator<Item = &ParametricSectorProductZeroDecomposition> {
        let start = self
            .product_zero_decompositions
            .partition_point(|witness| witness.product_locus_ordinal < product_locus_ordinal);
        let end = start
            + self.product_zero_decompositions[start..]
                .partition_point(|witness| witness.product_locus_ordinal == product_locus_ordinal);
        self.product_zero_decompositions[start..end].iter()
    }
    /// Deterministic preferred witness for callers that require one factor
    /// list. Witnesses are sorted by product ordinal and then lexicographically
    /// by factor ordinals, so this returns the lexicographically first list.
    pub fn canonical_product_zero_decomposition_for_locus(
        &self,
        product_locus_ordinal: usize,
    ) -> Option<&ParametricSectorProductZeroDecomposition> {
        let start = self
            .product_zero_decompositions
            .partition_point(|witness| witness.product_locus_ordinal < product_locus_ordinal);
        self.product_zero_decompositions
            .get(start)
            .filter(|witness| witness.product_locus_ordinal == product_locus_ordinal)
    }
    /// Resolve an exact case-predicate representative to the deterministic
    /// factor witness without polynomial factorization. This intentionally
    /// uses literal retained-representative equality: partition predicates are
    /// cloned from the structural table during compilation.
    pub fn canonical_product_zero_decomposition_for_exact_predicate(
        &self,
        polynomial: &ParametricPolynomial,
    ) -> Option<&ParametricSectorProductZeroDecomposition> {
        let ordinal = self.structural_locus_ordinal_exact(polynomial)?;
        self.canonical_product_zero_decomposition_for_locus(ordinal)
    }
    pub const fn partition(&self) -> &SymbolicSectorCasePartitionCertificate {
        &self.partition
    }
    pub fn classifications(&self) -> &[ParametricSectorLeafClassification] {
        &self.classifications
    }
    pub const fn stats(&self) -> ParametricSectorCoverageStats {
        self.stats
    }

    /// Regenerate every candidate's IBP/LI provenance, replay every `WhenBad`
    /// proof, and deterministically reconstruct the global composition.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorCoverageError> {
        self.validate_replay_scope(family, context)?;
        self.preflight_product_zero_payload(context)?;
        self.row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, self.row_span.clone())
    }

    /// Replay against one caller-shared row span, replaying that row-span
    /// certificate exactly once before the candidate batch.
    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), ParametricSectorCoverageError> {
        self.validate_replay_scope(family, context)?;
        self.preflight_product_zero_payload(context)?;
        row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), ParametricSectorCoverageError> {
        self.validate_replay_scope(family, context)?;
        if !Arc::ptr_eq(&self.row_span, &row_span) && !self.row_span.payload_eq(&row_span) {
            return Err(ParametricSectorCoverageError::SharedRowSpanCertificateMismatch);
        }
        self.preflight_product_zero_payload(context)?;
        let stored_compilations = self
            .attempts
            .iter()
            .enumerate()
            .map(|(ordinal, attempt)| {
                if attempt.ordinal != ordinal {
                    Err(ParametricSectorCoverageError::CandidateOrdinalMismatch {
                        expected: ordinal,
                        actual: attempt.ordinal,
                    })
                } else {
                    Ok(attempt.compilation.clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rebuilt =
            ParametricSectorCoverageCompiler::compose_authenticated_with_replayed_row_span(
                family,
                context,
                self.sector.clone(),
                stored_compilations,
                row_span,
                self.limits,
            )?;
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(ParametricSectorCoverageError::ReplayMismatch)
        }
    }

    fn validate_replay_scope(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorCoverageError> {
        if self.schema != PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA {
            return Err(ParametricSectorCoverageError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(ParametricSectorCoverageError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(ParametricSectorCoverageError::WrongContext);
        }
        Ok(())
    }

    fn preflight_product_zero_payload(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorCoverageError> {
        preflight_product_zero_payload(
            context,
            &self.structural_loci,
            &self.product_zero_decompositions,
            self.stats,
            self.limits,
        )
    }

    /// Locate the unique terminal global leaf for a concrete integer point.
    pub fn classification_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<Option<&ParametricSectorLeafClassification>, ParametricSectorCoverageError> {
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(ParametricSectorCoverageError::WrongContext);
        }
        if !self.partition.orthant().contains_integer_point(indices)? {
            return Ok(None);
        }
        let mut matched = None;
        for case in self.partition.cases() {
            let mut accepts = true;
            for predicate in case.predicates() {
                let specialized = context.specialize_polynomial(
                    predicate.polynomial(),
                    indices,
                    self.limits.generated_when_bad.when_bad.arithmetic,
                )?;
                accepts &= match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => specialized.is_zero(),
                    SymbolicPolynomialPredicateKind::NonZero => !specialized.is_zero(),
                };
            }
            if accepts {
                if matched.is_some() {
                    return Err(ParametricSectorCoverageError::PartitionEvaluationMismatch);
                }
                matched = self
                    .classifications
                    .iter()
                    .find(|classification| classification.case == case.id());
            }
        }
        matched
            .map(Some)
            .ok_or(ParametricSectorCoverageError::PartitionEvaluationMismatch)
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && (Arc::ptr_eq(&self.row_span, &other.row_span)
                || self.row_span.payload_eq(&other.row_span))
            && self.structural_loci == other.structural_loci
            && self.product_zero_decompositions == other.product_zero_decompositions
            && self.partition == other.partition
            && self.classifications == other.classifications
            && self.limits == other.limits
            && self.stats == other.stats
            && self.attempts.len() == other.attempts.len()
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                // The compilation objects are immutable and each side has
                // independently replayed its complete generated-source and
                // WhenBad payload. Compare all externally meaningful binding
                // and admissibility fields, not only the persisted ordinal.
                .all(attempt_payload_eq)
    }
}

/// Production compiler for a finite, ordered candidate set.
pub struct ParametricSectorCoverageCompiler;

/// Private proof token for one compiler-fresh generated-source authentication
/// and `WhenBad` compilation.  Arbitrary public or persisted compilations
/// cannot enter the no-replay composition path: they must first be replayed
/// and normalized by [`ParametricSectorCoverageCompiler::compose_authenticated`]
/// or [`ParametricSectorCoverageCertificate::replay`].
struct FreshGeneratedWhenBadCompilation(GeneratedWhenBadCompilation);

impl FreshGeneratedWhenBadCompilation {
    fn compile_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedWhenBadLimits,
    ) -> Result<Self, GeneratedWhenBadError> {
        Ok(Self(
            GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                family, context, candidate, row_span, limits,
            )?,
        ))
    }

    fn into_inner(self) -> GeneratedWhenBadCompilation {
        self.0
    }
}

impl ParametricSectorCoverageCompiler {
    /// Authenticate every raw elimination candidate against freshly generated
    /// IBP/LI identities, then compose the resulting domains.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        candidates: &[ParametricReductionRuleCandidate],
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        let row_span = Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
            family,
            context,
            limits.generated_when_bad.ibp,
            limits.generated_when_bad.row_span,
        )?);
        Self::compile_with_replayed_row_span(family, context, sector, candidates, row_span, limits)
    }

    /// Authenticate every candidate against one caller-supplied immutable
    /// generated row span.  The shared certificate is replayed exactly once,
    /// then reused for the complete candidate batch.
    pub fn compile_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        candidates: &[ParametricReductionRuleCandidate],
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        row_span.replay(family, context)?;
        Self::compile_with_replayed_row_span(family, context, sector, candidates, row_span, limits)
    }

    pub(crate) fn compile_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        candidates: &[ParametricReductionRuleCandidate],
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        validate_row_span_binding(family, context, &row_span, limits)?;
        check_limit(
            "sector-coverage candidates",
            candidates.len(),
            limits.max_candidates,
        )?;
        let mut compilations = Vec::with_capacity(candidates.len());
        for (ordinal, candidate) in candidates.iter().enumerate() {
            if candidate.family_fingerprint() != family.fingerprint() {
                return Err(ParametricSectorCoverageError::CandidateWrongFamily { ordinal });
            }
            if candidate.context_fingerprint() != context.fingerprint() {
                return Err(ParametricSectorCoverageError::CandidateWrongContext { ordinal });
            }
            if candidate.sector() != &sector {
                return Err(ParametricSectorCoverageError::CandidateWrongSector { ordinal });
            }
            compilations.push(
                FreshGeneratedWhenBadCompilation::compile_with_replayed_row_span(
                    family,
                    context,
                    candidate,
                    row_span.clone(),
                    limits.generated_when_bad,
                )?,
            );
        }
        Self::compose_fresh_authenticated_with_replayed_row_span(
            family,
            context,
            sector,
            compilations,
            row_span,
            limits,
        )
    }

    /// Compose only generated-source-authenticated candidate attempts.  Every
    /// supplied compilation is replayed before any global leaf is classified.
    pub fn compose_authenticated(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        check_limit(
            "sector-coverage candidates",
            compilations.len(),
            limits.max_candidates,
        )?;
        let row_span = if let Some(first) = compilations.first() {
            first.source_authentication().row_span_arc().clone()
        } else {
            Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                limits.generated_when_bad.ibp,
                limits.generated_when_bad.row_span,
            )?)
        };
        row_span.replay(family, context)?;
        Self::compose_authenticated_with_replayed_row_span(
            family,
            context,
            sector,
            compilations,
            row_span,
            limits,
        )
    }

    /// Normalize arbitrary authenticated attempts onto one caller-supplied,
    /// already-replayed row-span allocation.  Every input proof still takes
    /// its complete replay path.  Only the resulting compiler-fresh private
    /// tokens may enter the no-replay composition path.
    pub(crate) fn compose_authenticated_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        validate_row_span_binding(family, context, &row_span, limits)?;
        check_limit(
            "sector-coverage candidates",
            compilations.len(),
            limits.max_candidates,
        )?;

        let mut normalized = Vec::with_capacity(compilations.len());
        for compilation in compilations {
            // A payload-different certificate cannot replay against this
            // caller-owned proof.  Reject it before retained-elimination and
            // source-row authentication work.
            let compilation_row_span = compilation.source_authentication().row_span_arc();
            if !Arc::ptr_eq(compilation_row_span, &row_span)
                && !compilation_row_span.payload_eq(&row_span)
            {
                return Err(ParametricSectorCoverageError::SharedRowSpanCertificateMismatch);
            }
            compilation.replay_with_replayed_row_span(family, context, row_span.clone())?;
            normalized.push(
                FreshGeneratedWhenBadCompilation::compile_with_replayed_row_span(
                    family,
                    context,
                    compilation.candidate(),
                    row_span.clone(),
                    limits.generated_when_bad,
                )?,
            );
        }
        let coverage = Self::compose_fresh_authenticated_with_replayed_row_span(
            family,
            context,
            sector,
            normalized,
            row_span.clone(),
            limits,
        )?;
        if !Arc::ptr_eq(coverage.row_span_arc(), &row_span) {
            return Err(ParametricSectorCoverageError::SharedRowSpanCertificateMismatch);
        }
        Ok(coverage)
    }

    /// Compose only values returned immediately by the generated compiler.
    /// The private wrapper is the proof boundary: public and persisted values
    /// cannot call this path without first taking their full replay path.
    fn compose_fresh_authenticated_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<FreshGeneratedWhenBadCompilation>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<ParametricSectorCoverageCertificate, ParametricSectorCoverageError> {
        validate_coherent_limits(limits)?;
        validate_family_context(family, context)?;
        if sector.arity() != context.index_count() {
            return Err(ParametricSectorCoverageError::WrongArity {
                expected: context.index_count(),
                actual: sector.arity(),
            });
        }
        check_limit(
            "sector-coverage candidates",
            compilations.len(),
            limits.max_candidates,
        )?;

        let mut attempts = Vec::with_capacity(compilations.len());
        for (ordinal, fresh) in compilations.into_iter().enumerate() {
            let compilation = fresh.into_inner();
            if !Arc::ptr_eq(
                compilation.source_authentication().row_span_arc(),
                &row_span,
            ) {
                return Err(
                    ParametricSectorCoverageError::SharedRowSpanAllocationMismatch { ordinal },
                );
            }
            let (binding_family, binding_context, binding_sector) = match &compilation {
                GeneratedWhenBadCompilation::Certified(certificate) => {
                    let binding = certificate.admissibility().binding();
                    (
                        binding.family_fingerprint(),
                        binding.context_fingerprint(),
                        binding.sector(),
                    )
                }
                GeneratedWhenBadCompilation::Unsupported(unsupported) => {
                    let binding = unsupported.admissibility().binding();
                    (
                        binding.family_fingerprint(),
                        binding.context_fingerprint(),
                        binding.sector(),
                    )
                }
            };
            if binding_family != family.fingerprint() {
                return Err(ParametricSectorCoverageError::CandidateWrongFamily { ordinal });
            }
            if binding_context != context.fingerprint() {
                return Err(ParametricSectorCoverageError::CandidateWrongContext { ordinal });
            }
            if binding_sector != &sector {
                return Err(ParametricSectorCoverageError::CandidateWrongSector { ordinal });
            }
            attempts.push(SectorCoverageCandidateAttempt {
                ordinal,
                compilation,
            });
        }

        let (partition, classifications, structural_loci, product_zero_decompositions, stats) =
            compose_global_partition(context, &sector, &attempts, limits)?;
        let certificate = ParametricSectorCoverageCertificate {
            schema: PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            sector,
            row_span,
            attempts: attempts.into_boxed_slice(),
            structural_loci: structural_loci.into_boxed_slice(),
            product_zero_decompositions: product_zero_decompositions.into_boxed_slice(),
            partition,
            classifications: classifications.into_boxed_slice(),
            limits,
            stats,
        };
        // `SymbolicSectorCasePartitionBuilder::finish`, called by
        // `compose_global_partition`, already replayed the complete split
        // transcript.  Do not replay the identical partition a second time.
        Ok(certificate)
    }
}

fn compose_global_partition(
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    attempts: &[SectorCoverageCandidateAttempt],
    limits: ParametricSectorCoverageLimits,
) -> Result<
    (
        SymbolicSectorCasePartitionCertificate,
        Vec<ParametricSectorLeafClassification>,
        Vec<ParametricPolynomial>,
        Vec<ParametricSectorProductZeroDecomposition>,
        ParametricSectorCoverageStats,
    ),
    ParametricSectorCoverageError,
> {
    let mut stats = ParametricSectorCoverageStats {
        candidates: attempts.len(),
        shared_row_span_certificates: 1,
        shared_row_span_candidate_reuses: attempts.len(),
        ..ParametricSectorCoverageStats::default()
    };
    let mut unique_predicates = Vec::<ParametricPolynomial>::new();
    let mut product_zero_decompositions = Vec::<ParametricSectorProductZeroDecomposition>::new();
    let mut attempt_bad_formulas =
        Vec::<Option<CandidateBadFormula>>::with_capacity(attempts.len());
    let mut unsupported_ordinals = Vec::<usize>::new();

    for attempt in attempts {
        let (source_stats, binding_bytes) = match &attempt.compilation {
            GeneratedWhenBadCompilation::Certified(certificate) => {
                stats.certified_candidates = checked_add(
                    "certified sector-coverage candidates",
                    stats.certified_candidates,
                    1,
                )?;
                let admissibility = certificate.admissibility();
                let candidate_leaves = admissibility.partition().cases().len();
                stats.candidate_partition_leaves = checked_bounded_add(
                    "candidate partition leaves",
                    stats.candidate_partition_leaves,
                    candidate_leaves,
                    limits.max_candidate_partition_leaves,
                )?;
                for case in admissibility.partition().cases() {
                    stats.candidate_predicate_instances = checked_bounded_add(
                        "candidate predicate instances",
                        stats.candidate_predicate_instances,
                        case.predicates().len(),
                        limits.max_candidate_predicate_instances,
                    )?;
                }
                let bad_formula = CandidateBadFormula::try_new(
                    context,
                    admissibility,
                    &mut unique_predicates,
                    &mut product_zero_decompositions,
                    &mut stats,
                    limits,
                )?;
                stats.candidate_bad_clauses = checked_bounded_add(
                    "candidate bad-domain clauses",
                    stats.candidate_bad_clauses,
                    bad_formula.clauses.len(),
                    limits.max_candidate_bad_clauses,
                )?;
                stats.candidate_bad_atoms = checked_bounded_add(
                    "candidate bad-domain atoms",
                    stats.candidate_bad_atoms,
                    bad_formula.atom_count,
                    limits.max_candidate_bad_atoms,
                )?;
                attempt_bad_formulas.push(Some(bad_formula));
                let when_bad_stats = admissibility.stats();
                stats.condition_terms = checked_bounded_add(
                    "sector-coverage retained condition terms",
                    stats.condition_terms,
                    when_bad_stats.retained_condition_terms(),
                    limits.max_total_condition_terms,
                )?;
                stats.condition_bytes = checked_bounded_add(
                    "sector-coverage retained condition bytes",
                    stats.condition_bytes,
                    when_bad_stats.retained_condition_bytes(),
                    limits.max_total_condition_bytes,
                )?;
                (
                    certificate.source_authentication().stats(),
                    admissibility.binding().retained_bytes(),
                )
            }
            GeneratedWhenBadCompilation::Unsupported(unsupported) => {
                stats.unsupported_candidates = checked_add(
                    "unsupported sector-coverage candidates",
                    stats.unsupported_candidates,
                    1,
                )?;
                unsupported_ordinals.push(attempt.ordinal);
                attempt_bad_formulas.push(None);
                (
                    unsupported.source_authentication().stats(),
                    unsupported.admissibility().binding().retained_bytes(),
                )
            }
        };
        stats.canonical_rows = checked_bounded_add(
            "sector-coverage canonical rows",
            stats.canonical_rows,
            source_stats.canonical_rows(),
            limits.max_total_canonical_rows,
        )?;
        stats.canonical_terms = checked_bounded_add(
            "sector-coverage canonical terms",
            stats.canonical_terms,
            source_stats.canonical_terms(),
            limits.max_total_canonical_terms,
        )?;
        stats.retained_source_rows = checked_bounded_add(
            "sector-coverage retained source rows",
            stats.retained_source_rows,
            source_stats.retained_rows(),
            limits.max_total_retained_source_rows,
        )?;
        stats.retained_source_terms = checked_bounded_add(
            "sector-coverage retained source terms",
            stats.retained_source_terms,
            source_stats.retained_terms(),
            limits.max_total_retained_source_terms,
        )?;
        stats.source_match_attempts = checked_bounded_add(
            "sector-coverage source match attempts",
            stats.source_match_attempts,
            source_stats.match_attempts(),
            limits.max_total_source_match_attempts,
        )?;
        stats.candidate_binding_bytes = checked_bounded_add(
            "sector-coverage candidate binding bytes",
            stats.candidate_binding_bytes,
            binding_bytes,
            limits.max_total_candidate_binding_bytes,
        )?;
    }
    stats.unique_predicates = unique_predicates.len();
    canonicalize_product_zero_decompositions(&mut product_zero_decompositions);
    preflight_product_zero_payload(
        context,
        &unique_predicates,
        &product_zero_decompositions,
        stats,
        limits,
    )?;

    // Recognize every canonical structural locus once.  The result is only
    // used for exact empty-branch pruning; an unrecognized polynomial remains
    // a completely ordinary symbolic split and is never approximated.
    let mut coordinate_loci = Vec::with_capacity(unique_predicates.len());
    for polynomial in &unique_predicates {
        stats.coordinate_pruning_checks = checked_bounded_add(
            "sector-coverage coordinate pruning checks",
            stats.coordinate_pruning_checks,
            1,
            limits.max_coordinate_pruning_checks,
        )?;
        coordinate_loci.push(
            crate::coordinate_equality_loci::recognize_coordinate_locus_for_pruning(
                context,
                polynomial,
                limits.coordinate_loci,
            )?,
        );
    }

    let effective_limits = effective_sector_limits(limits);
    let mut builder =
        SymbolicSectorCasePartitionBuilder::try_new(context, sector.clone(), effective_limits)?;

    // LiteRed updates `noRules` after each accepted rule.  Mirror that
    // semantics exactly: replay a candidate's own decision tree only on the
    // global leaves not covered by any earlier candidate.  Covered leaves are
    // frozen, so unrelated later predicates cannot create a Cartesian
    // product across them.
    let root = builder.root_case();
    let mut open = BTreeMap::from([(root, GlobalCaseState::new(context.index_count()))]);
    let mut covered = BTreeMap::<SymbolicSectorCaseId, ParametricSectorLeafDisposition>::new();
    let mut coordinate_empty =
        BTreeMap::<SymbolicSectorCaseId, ParametricSectorEmptyLocusReason>::new();
    let mut divisibility_cache = BTreeMap::<(usize, usize), bool>::new();

    for (attempt, bad_formula) in attempts.iter().zip(&attempt_bad_formulas) {
        let GeneratedWhenBadCompilation::Certified(_) = &attempt.compilation else {
            continue;
        };
        if open.is_empty() {
            break;
        }
        let bad_formula = bad_formula
            .as_ref()
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        let current = std::mem::take(&mut open);
        for (global_case, state) in current {
            overlay_candidate_bad_formula(
                &mut builder,
                context,
                sector,
                global_case,
                state,
                attempt.ordinal,
                bad_formula,
                &unique_predicates,
                &coordinate_loci,
                &mut divisibility_cache,
                &mut open,
                &mut covered,
                &mut coordinate_empty,
                &mut stats,
                limits,
            )?;
        }
    }

    let partition = builder.finish(context)?;
    check_limit(
        "global sector-coverage leaf classifications",
        partition.cases().len(),
        limits.max_global_leaf_classifications,
    )?;

    let mut classifications = Vec::with_capacity(partition.cases().len());
    for global_case in partition.cases() {
        let disposition = match covered.remove(&global_case.id()) {
            Some(disposition) => {
                stats.descending_leaves = checked_add(
                    "descending sector-coverage leaves",
                    stats.descending_leaves,
                    1,
                )?;
                disposition
            }
            None if coordinate_empty.contains_key(&global_case.id()) => {
                let reason = coordinate_empty
                    .remove(&global_case.id())
                    .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
                validate_empty_locus_reason(
                    context,
                    sector,
                    global_case,
                    &reason,
                    limits.coordinate_loci,
                )?;
                stats.proved_empty_locus_leaves = checked_add(
                    "proved-empty sector-coverage loci",
                    stats.proved_empty_locus_leaves,
                    1,
                )?;
                ParametricSectorLeafDisposition::ProvedEmptyLocus { reason }
            }
            None if open.contains_key(&global_case.id()) => {
                if unsupported_ordinals.is_empty() {
                    stats.uncovered_leaves = checked_add(
                        "uncovered sector-coverage leaves",
                        stats.uncovered_leaves,
                        1,
                    )?;
                    ParametricSectorLeafDisposition::Uncovered
                } else {
                    stats.unsupported_leaves = checked_add(
                        "unsupported sector-coverage leaves",
                        stats.unsupported_leaves,
                        1,
                    )?;
                    stats.unsupported_references = checked_bounded_add(
                        "sector-coverage unsupported references",
                        stats.unsupported_references,
                        unsupported_ordinals.len(),
                        limits.max_unsupported_references,
                    )?;
                    ParametricSectorLeafDisposition::Unsupported {
                        candidate_ordinals: unsupported_ordinals.clone().into_boxed_slice(),
                    }
                }
            }
            None => {
                return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
            }
        };
        classifications.push(ParametricSectorLeafClassification {
            case: global_case.id(),
            disposition,
        });
    }
    if !covered.is_empty()
        || !coordinate_empty.is_empty()
        || classifications.len()
            != open.len() + stats.proved_empty_locus_leaves + stats.descending_leaves
    {
        return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
    }
    stats.global_leaves = classifications.len();
    Ok((
        partition,
        classifications,
        unique_predicates,
        product_zero_decompositions,
        stats,
    ))
}

#[derive(Clone, Debug)]
struct GlobalCaseState {
    locus_decisions: BTreeMap<usize, SymbolicPolynomialPredicateKind>,
    decision_predicate_ordinals: BTreeMap<usize, usize>,
    fixed_coordinates: Box<[Option<FixedCoordinate>]>,
    excluded_coordinates: BTreeMap<(usize, i64), usize>,
}

#[derive(Clone, Copy, Debug)]
struct FixedCoordinate {
    value: i64,
    predicate_ordinal: usize,
}

fn validate_empty_locus_reason(
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    case: &crate::SymbolicSectorCase,
    reason: &ParametricSectorEmptyLocusReason,
    limits: CoordinateEqualityLocusLimits,
) -> Result<(), ParametricSectorCoverageError> {
    let recognized = |ordinal: usize,
                      expected_kind: SymbolicPolynomialPredicateKind|
     -> Result<(usize, i64), ParametricSectorCoverageError> {
        let predicate = case
            .predicates()
            .get(ordinal)
            .ok_or(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)?;
        if predicate.kind() != expected_kind {
            return Err(ParametricSectorCoverageError::EmptyLocusWitnessMismatch);
        }
        crate::coordinate_equality_loci::recognize_coordinate_locus_for_pruning(
            context,
            predicate.polynomial(),
            limits,
        )?
        .ok_or(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)
    };

    let valid = match *reason {
        ParametricSectorEmptyLocusReason::OrthantViolation {
            equality_predicate_ordinal,
            index,
            value,
            side,
        } => {
            let active = sector
                .active_bits()
                .get(index)
                .copied()
                .ok_or(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)?;
            recognized(
                equality_predicate_ordinal,
                SymbolicPolynomialPredicateKind::EqualZero,
            )? == (index, value)
                && side
                    == if active {
                        crate::SectorOrthantSide::AtLeastOne
                    } else {
                        crate::SectorOrthantSide::AtMostZero
                    }
                && if active { value < 1 } else { value > 0 }
        }
        ParametricSectorEmptyLocusReason::ConflictingFixedValues {
            first_equality_predicate_ordinal,
            second_equality_predicate_ordinal,
            index,
            first_value,
            second_value,
        } => {
            first_value != second_value
                && recognized(
                    first_equality_predicate_ordinal,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )? == (index, first_value)
                && recognized(
                    second_equality_predicate_ordinal,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )? == (index, second_value)
        }
        ParametricSectorEmptyLocusReason::EqualityNonzeroContradiction {
            equality_predicate_ordinal,
            nonzero_predicate_ordinal,
            index,
            value,
        } => {
            recognized(
                equality_predicate_ordinal,
                SymbolicPolynomialPredicateKind::EqualZero,
            )? == (index, value)
                && recognized(
                    nonzero_predicate_ordinal,
                    SymbolicPolynomialPredicateKind::NonZero,
                )? == (index, value)
        }
        ParametricSectorEmptyLocusReason::PolynomialDivisibilityContradiction {
            zero_predicate_ordinal,
            nonzero_predicate_ordinal,
        } => {
            let zero = case
                .predicates()
                .get(zero_predicate_ordinal)
                .ok_or(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)?;
            let nonzero = case
                .predicates()
                .get(nonzero_predicate_ordinal)
                .ok_or(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)?;
            zero.kind() == SymbolicPolynomialPredicateKind::EqualZero
                && nonzero.kind() == SymbolicPolynomialPredicateKind::NonZero
                && context.polynomial_divides_with_limits(
                    zero.polynomial(),
                    nonzero.polynomial(),
                    limits.exact_algebra,
                )?
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ParametricSectorCoverageError::EmptyLocusWitnessMismatch)
    }
}

impl GlobalCaseState {
    fn new(index_count: usize) -> Self {
        Self {
            locus_decisions: BTreeMap::new(),
            decision_predicate_ordinals: BTreeMap::new(),
            fixed_coordinates: vec![None; index_count].into_boxed_slice(),
            excluded_coordinates: BTreeMap::new(),
        }
    }

    /// Add one exact coordinate equality or exclusion.  A returned witness
    /// means the conjunction is proved empty; unresolved polynomials never
    /// call this.
    fn apply_coordinate_decision(
        &mut self,
        sector: &SectorMask,
        locus: Option<(usize, i64)>,
        kind: SymbolicPolynomialPredicateKind,
        predicate_ordinal: usize,
    ) -> Result<Option<ParametricSectorEmptyLocusReason>, ParametricSectorCoverageError> {
        let Some((index, value)) = locus else {
            return Ok(None);
        };
        let Some(fixed) = self.fixed_coordinates.get_mut(index) else {
            return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
        };
        let active = *sector
            .active_bits()
            .get(index)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        match kind {
            SymbolicPolynomialPredicateKind::EqualZero => {
                if (active && value < 1) || (!active && value > 0) {
                    return Ok(Some(ParametricSectorEmptyLocusReason::OrthantViolation {
                        equality_predicate_ordinal: predicate_ordinal,
                        index,
                        value,
                        side: if active {
                            crate::SectorOrthantSide::AtLeastOne
                        } else {
                            crate::SectorOrthantSide::AtMostZero
                        },
                    }));
                }
                if let Some(existing) = *fixed
                    && existing.value != value
                {
                    return Ok(Some(
                        ParametricSectorEmptyLocusReason::ConflictingFixedValues {
                            first_equality_predicate_ordinal: existing.predicate_ordinal,
                            second_equality_predicate_ordinal: predicate_ordinal,
                            index,
                            first_value: existing.value,
                            second_value: value,
                        },
                    ));
                }
                if let Some(&nonzero_predicate_ordinal) =
                    self.excluded_coordinates.get(&(index, value))
                {
                    return Ok(Some(
                        ParametricSectorEmptyLocusReason::EqualityNonzeroContradiction {
                            equality_predicate_ordinal: predicate_ordinal,
                            nonzero_predicate_ordinal,
                            index,
                            value,
                        },
                    ));
                }
                *fixed = Some(FixedCoordinate {
                    value,
                    predicate_ordinal,
                });
            }
            SymbolicPolynomialPredicateKind::NonZero => {
                if let Some(existing) = *fixed
                    && existing.value == value
                {
                    return Ok(Some(
                        ParametricSectorEmptyLocusReason::EqualityNonzeroContradiction {
                            equality_predicate_ordinal: existing.predicate_ordinal,
                            nonzero_predicate_ordinal: predicate_ordinal,
                            index,
                            value,
                        },
                    ));
                }
                self.excluded_coordinates
                    .entry((index, value))
                    .or_insert(predicate_ordinal);
            }
        }
        Ok(None)
    }
}

/// Return a branch forced by exact divisibility in the integral domain
/// `K[n]`.  If `p | q`, then `p=0 => q=0`, while `q!=0 => p!=0`.
fn implied_locus_decision(
    context: &ParametricCoefficientContext,
    requested_locus: usize,
    state: &GlobalCaseState,
    polynomials: &[ParametricPolynomial],
    divisibility_cache: &mut BTreeMap<(usize, usize), bool>,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<Option<(SymbolicPolynomialPredicateKind, usize)>, ParametricSectorCoverageError> {
    let requested = polynomials
        .get(requested_locus)
        .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
    for (&known_locus, &known_kind) in &state.locus_decisions {
        let known = polynomials
            .get(known_locus)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        let implied = match known_kind {
            // known | requested and known=0 implies requested=0.
            SymbolicPolynomialPredicateKind::EqualZero => cached_locus_divisibility(
                context,
                known_locus,
                requested_locus,
                known,
                requested,
                divisibility_cache,
                stats,
                limits,
            )?
            .then_some(SymbolicPolynomialPredicateKind::EqualZero),
            // requested | known and known!=0 implies requested!=0.
            SymbolicPolynomialPredicateKind::NonZero => cached_locus_divisibility(
                context,
                requested_locus,
                known_locus,
                requested,
                known,
                divisibility_cache,
                stats,
                limits,
            )?
            .then_some(SymbolicPolynomialPredicateKind::NonZero),
        };
        if let Some(kind) = implied {
            return Ok(Some((kind, known_locus)));
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn cached_locus_divisibility(
    context: &ParametricCoefficientContext,
    divisor_ordinal: usize,
    dividend_ordinal: usize,
    divisor: &ParametricPolynomial,
    dividend: &ParametricPolynomial,
    cache: &mut BTreeMap<(usize, usize), bool>,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<bool, ParametricSectorCoverageError> {
    if let Some(&result) = cache.get(&(divisor_ordinal, dividend_ordinal)) {
        return Ok(result);
    }
    stats.locus_divisibility_checks = checked_bounded_add(
        "sector-coverage locus divisibility checks",
        stats.locus_divisibility_checks,
        1,
        limits.max_locus_divisibility_checks,
    )?;
    let result = context.polynomial_divides_with_limits(
        divisor,
        dividend,
        coverage_exact_algebra(limits),
    )?;
    cache.insert((divisor_ordinal, dividend_ordinal), result);
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CandidateBadAtom {
    locus: usize,
    kind: SymbolicPolynomialPredicateKind,
}

type CandidateBadClause = DirectBadFormulaClause<CandidateBadAtom>;

/// Direct Boolean form of the authenticated `WhenBad` domain:
///
/// Each leak contributes either `boundary=0` for a coefficient-field-nonzero
/// gate or `(boundary=0 AND gate!=0)` for a symbolic gate, never both:
///
/// `bad = OR(guard=0, leak_clause)`.
///
/// The local decision tree is one evaluation order for this formula, not part
/// of its semantics.  Retaining the formula lets global composition notice a
/// later clause that is already true before splitting irrelevant prefixes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CandidateBadFormula {
    clauses: Box<[CandidateBadClause]>,
    atom_count: usize,
}

impl CandidateBadFormula {
    fn try_new(
        context: &ParametricCoefficientContext,
        candidate: &crate::WhenBadCertificate,
        unique_predicates: &mut Vec<ParametricPolynomial>,
        product_zero_decompositions: &mut Vec<ParametricSectorProductZeroDecomposition>,
        stats: &mut ParametricSectorCoverageStats,
        limits: ParametricSectorCoverageLimits,
    ) -> Result<Self, ParametricSectorCoverageError> {
        let mut clauses = Vec::<CandidateBadClause>::new();
        let mut atom_count = 0usize;

        let mut insert_clause =
            |clause: CandidateBadClause| -> Result<(), ParametricSectorCoverageError> {
                if clauses.contains(&clause) {
                    return Ok(());
                }
                let requested_clauses =
                    checked_add("candidate bad-domain clauses", clauses.len(), 1)?;
                check_limit(
                    "candidate bad-domain clauses",
                    requested_clauses,
                    limits.max_candidate_bad_clauses,
                )?;
                atom_count = checked_bounded_add(
                    "candidate bad-domain atoms",
                    atom_count,
                    clause.atom_count(),
                    limits.max_candidate_bad_atoms,
                )?;
                clauses.push(clause);
                Ok(())
            };

        for condition in candidate.index_domain_guards() {
            let locus = insert_unique_structural_locus(
                context,
                unique_predicates,
                condition.polynomial(),
                limits.max_unique_predicates,
                coverage_exact_algebra(limits),
                stats,
                limits,
            )?;
            insert_clause(CandidateBadClause::Atom(CandidateBadAtom {
                locus,
                kind: SymbolicPolynomialPredicateKind::EqualZero,
            }))?;
        }

        for event in candidate.leak_events() {
            let boundary_locus = insert_unique_structural_locus(
                context,
                unique_predicates,
                event.boundary_polynomial(),
                limits.max_unique_predicates,
                coverage_exact_algebra(limits),
                stats,
                limits,
            )?;
            let boundary = CandidateBadAtom {
                locus: boundary_locus,
                kind: SymbolicPolynomialPredicateKind::EqualZero,
            };
            match event.numerator_gate() {
                WhenBadLeakNumeratorGate::CoefficientFieldNonzero(_) => {
                    insert_clause(CandidateBadClause::Atom(boundary))?;
                }
                WhenBadLeakNumeratorGate::Symbolic(gate) => {
                    let gate_locus = insert_unique_structural_locus(
                        context,
                        unique_predicates,
                        gate,
                        limits.max_unique_predicates,
                        coverage_exact_algebra(limits),
                        stats,
                        limits,
                    )?;
                    insert_clause(CandidateBadClause::Conjunction(
                        boundary,
                        CandidateBadAtom {
                            locus: gate_locus,
                            kind: SymbolicPolynomialPredicateKind::NonZero,
                        },
                    ))?;
                }
            }
        }

        // In the integral domain K[n], a finite disjunction of principal
        // zero loci is itself the zero locus of their product:
        //
        //     (p1=0 OR ... OR pk=0) <=> p1*...*pk=0.
        //
        // Prefer compressing the one-atom clauses before global overlay.  If
        // the conservative whole-product support fits the persisted cutoff,
        // one checked Symbolica product split replaces an arbitrary local
        // prefix.  Otherwise the exact, canonical factor disjunction remains
        // in the private formula and no product polynomial is constructed.
        // In both representations the factors remain in `unique_predicates`
        // for exact divisibility implications.
        // Preserve the exact pre-compression factor provenance. This is the
        // only sound source of the factor list: recovering it later by
        // factoring the product would not replay the compiler decision.
        // Every clause contributes at least one atom, so `clauses.len()` is a
        // checked upper bound on the staging allocation. Exact filtering below
        // may retain fewer references (conjunctions, duplicate loci, and
        // K-units). The selected representation charges either durable
        // decomposition references or private factored-formula references,
        // never both.
        preflight_candidate_atomic_locus_staging(clauses.len(), atom_count, limits)?;
        let mut atomic_loci = Vec::new();
        try_reserve_exact(
            "candidate atomic-locus ordinals",
            &mut atomic_loci,
            clauses.len(),
        )?;
        for clause in &clauses {
            if let CandidateBadClause::Atom(atom) = clause {
                atomic_loci.push(atom.locus);
            }
        }
        if !atomic_loci.is_empty() {
            let (routing, routed_stats) = route_candidate_product_zero_loci(
                context,
                unique_predicates,
                product_zero_decompositions,
                &atomic_loci,
                *stats,
                limits,
            )?;
            clauses.retain(|clause| !matches!(clause, CandidateBadClause::Atom(_)));
            match routing {
                CandidateProductZeroRouting::Omitted => {}
                CandidateProductZeroRouting::ConcreteLocus(locus) => clauses.insert(
                    0,
                    CandidateBadClause::Atom(CandidateBadAtom {
                        locus,
                        kind: SymbolicPolynomialPredicateKind::EqualZero,
                    }),
                ),
                CandidateProductZeroRouting::Factored(factors) => {
                    // `factors` is a duplicate-free subset of the original
                    // atomic clauses. After `retain`, the existing allocation
                    // therefore has enough capacity for an allocation-free
                    // canonical rewrite. This is the commit seam for the
                    // staged fallback census returned by the router.
                    clauses.extend(factors.iter().copied().map(|locus| {
                        CandidateBadClause::Atom(CandidateBadAtom {
                            locus,
                            kind: SymbolicPolynomialPredicateKind::EqualZero,
                        })
                    }));
                    clauses.rotate_right(factors.len());
                }
            }
            atom_count = clauses.iter().try_fold(0usize, |total, clause| {
                checked_add("candidate bad-domain atoms", total, clause.atom_count())
            })?;
            check_limit(
                "candidate bad-domain atoms",
                atom_count,
                limits.max_candidate_bad_atoms,
            )?;
            *stats = routed_stats;
        }

        Ok(Self {
            clauses: clauses.into_boxed_slice(),
            atom_count,
        })
    }
}

fn preflight_candidate_atomic_locus_staging(
    clause_count: usize,
    atom_count: usize,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    check_limit(
        "candidate bad-domain atoms",
        atom_count,
        limits.max_candidate_bad_atoms,
    )?;
    check_limit(
        "candidate atomic-locus ordinal staging",
        clause_count,
        atom_count,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateFormulaEvaluation {
    Bad,
    Covered,
    Split(CandidateBadAtom),
}

fn charge_direct_bad_formula_evaluation(
    formula: &CandidateBadFormula,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    // Stage both prospective counters before committing either one. A failed
    // atom-query charge must not leave an evaluation recorded without its
    // complete conservative query budget.
    let evaluations = checked_bounded_add(
        "sector-coverage direct bad-formula evaluations",
        stats.direct_bad_formula_evaluations,
        1,
        limits.max_direct_bad_formula_evaluations,
    )?;
    let atom_queries = checked_bounded_add(
        "sector-coverage direct bad-formula atom queries",
        stats.direct_bad_formula_atom_queries,
        formula.atom_count,
        limits.max_direct_bad_formula_atom_queries,
    )?;
    stats.direct_bad_formula_evaluations = evaluations;
    stats.direct_bad_formula_atom_queries = atom_queries;
    Ok(())
}

fn candidate_atom_truth(
    context: &ParametricCoefficientContext,
    atom: CandidateBadAtom,
    state: &GlobalCaseState,
    polynomials: &[ParametricPolynomial],
    divisibility_cache: &mut BTreeMap<(usize, usize), bool>,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<DirectBadFormulaTruth, ParametricSectorCoverageError> {
    let decided = if let Some(&kind) = state.locus_decisions.get(&atom.locus) {
        Some(kind)
    } else {
        implied_locus_decision(
            context,
            atom.locus,
            state,
            polynomials,
            divisibility_cache,
            stats,
            limits,
        )?
        .map(|(kind, _)| kind)
    };
    Ok(match decided {
        Some(kind) if kind == atom.kind => DirectBadFormulaTruth::True,
        Some(_) => DirectBadFormulaTruth::False,
        None => DirectBadFormulaTruth::Unknown,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_candidate_bad_formula(
    context: &ParametricCoefficientContext,
    formula: &CandidateBadFormula,
    state: &GlobalCaseState,
    polynomials: &[ParametricPolynomial],
    divisibility_cache: &mut BTreeMap<(usize, usize), bool>,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<CandidateFormulaEvaluation, ParametricSectorCoverageError> {
    charge_direct_bad_formula_evaluation(formula, stats, limits)?;
    let route = route_direct_bad_formula(formula.clauses.iter().copied(), |atom| {
        candidate_atom_truth(
            context,
            atom,
            state,
            polynomials,
            divisibility_cache,
            stats,
            limits,
        )
    })?;
    Ok(match route {
        DirectBadFormulaRoute::Bad { .. } => CandidateFormulaEvaluation::Bad,
        DirectBadFormulaRoute::Good => CandidateFormulaEvaluation::Covered,
        DirectBadFormulaRoute::Split { atom, .. } => CandidateFormulaEvaluation::Split(atom),
    })
}

#[allow(clippy::too_many_arguments)]
fn overlay_candidate_bad_formula(
    builder: &mut SymbolicSectorCasePartitionBuilder,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    global_root: SymbolicSectorCaseId,
    state: GlobalCaseState,
    candidate_ordinal: usize,
    formula: &CandidateBadFormula,
    unique_predicates: &[ParametricPolynomial],
    coordinate_loci: &[Option<(usize, i64)>],
    divisibility_cache: &mut BTreeMap<(usize, usize), bool>,
    open: &mut BTreeMap<SymbolicSectorCaseId, GlobalCaseState>,
    covered: &mut BTreeMap<SymbolicSectorCaseId, ParametricSectorLeafDisposition>,
    coordinate_empty: &mut BTreeMap<SymbolicSectorCaseId, ParametricSectorEmptyLocusReason>,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    let mut work = vec![(global_root, state)];
    while let Some((global_case, state)) = work.pop() {
        match evaluate_candidate_bad_formula(
            context,
            formula,
            &state,
            unique_predicates,
            divisibility_cache,
            stats,
            limits,
        )? {
            CandidateFormulaEvaluation::Bad => {
                stats.candidate_leaf_match_attempts = checked_bounded_add(
                    "sector-coverage candidate leaf match attempts",
                    stats.candidate_leaf_match_attempts,
                    1,
                    limits.max_candidate_leaf_match_attempts,
                )?;
                if open.insert(global_case, state).is_some() {
                    return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
                }
            }
            CandidateFormulaEvaluation::Covered => {
                stats.candidate_leaf_match_attempts = checked_bounded_add(
                    "sector-coverage candidate leaf match attempts",
                    stats.candidate_leaf_match_attempts,
                    1,
                    limits.max_candidate_leaf_match_attempts,
                )?;
                if covered
                    .insert(
                        global_case,
                        ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal },
                    )
                    .is_some()
                {
                    return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
                }
            }
            CandidateFormulaEvaluation::Split(atom) => {
                let polynomial = unique_predicates
                    .get(atom.locus)
                    .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
                let coordinate_locus = coordinate_loci
                    .get(atom.locus)
                    .copied()
                    .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
                let global_children = builder.split_on_bad_polynomial(
                    // Candidate replay already authenticated the associate;
                    // the canonical first-seen representative gives stable
                    // global transcripts across later candidates.  The
                    // builder validates the context again transactionally.
                    context,
                    global_case,
                    polynomial.clone(),
                )?;
                let predicate_ordinal = builder
                    .case(global_children.equal_zero_case())
                    .and_then(|case| case.predicates().len().checked_sub(1))
                    .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
                for (kind, global_child) in [
                    (
                        SymbolicPolynomialPredicateKind::NonZero,
                        global_children.nonzero_case(),
                    ),
                    (
                        SymbolicPolynomialPredicateKind::EqualZero,
                        global_children.equal_zero_case(),
                    ),
                ] {
                    let mut child_state = state.clone();
                    child_state.locus_decisions.insert(atom.locus, kind);
                    child_state
                        .decision_predicate_ordinals
                        .insert(atom.locus, predicate_ordinal);
                    if let Some(reason) = child_state.apply_coordinate_decision(
                        sector,
                        coordinate_locus,
                        kind,
                        predicate_ordinal,
                    )? {
                        stats.coordinate_pruned_leaves = checked_add(
                            "coordinate-pruned sector-coverage leaves",
                            stats.coordinate_pruned_leaves,
                            1,
                        )?;
                        if coordinate_empty.insert(global_child, reason).is_some() {
                            return Err(
                                ParametricSectorCoverageError::CandidateLeafMappingMismatch,
                            );
                        }
                    } else {
                        work.push((global_child, child_state));
                    }
                }
            }
        }
    }
    Ok(())
}

fn insert_unique_structural_locus(
    context: &ParametricCoefficientContext,
    unique: &mut Vec<ParametricPolynomial>,
    polynomial: &ParametricPolynomial,
    limit: usize,
    exact_algebra: crate::algebra::ExactAlgebraLimits,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<usize, ParametricSectorCoverageError> {
    let mut staged = *stats;
    if let Some(ordinal) = find_structural_locus(
        context,
        unique,
        polynomial,
        exact_algebra,
        &mut staged,
        limits,
    )? {
        *stats = staged;
        return Ok(ordinal);
    }
    let requested = checked_add("unique sector-coverage predicates", unique.len(), 1)?;
    check_limit("unique sector-coverage predicates", requested, limit)?;
    let charge = preflight_structural_locus_retention(polynomial, staged, limits)?;
    try_reserve_exact("retained structural loci", unique, 1)?;
    unique.push(polynomial.clone());
    staged.retained_structural_locus_terms = charge.terms;
    staged.retained_structural_locus_bytes = charge.bytes;
    *stats = staged;
    Ok(unique.len() - 1)
}

fn find_structural_locus(
    context: &ParametricCoefficientContext,
    unique: &[ParametricPolynomial],
    polynomial: &ParametricPolynomial,
    exact_algebra: crate::algebra::ExactAlgebraLimits,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<Option<usize>, ParametricSectorCoverageError> {
    for (ordinal, existing) in unique.iter().enumerate() {
        if bounded_same_structural_locus(
            context,
            existing,
            polynomial,
            exact_algebra,
            stats,
            limits,
        )? {
            return Ok(Some(ordinal));
        }
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct StructuralLocusRetentionCharge {
    terms: usize,
    bytes: usize,
}

fn preflight_structural_locus_retention(
    polynomial: &ParametricPolynomial,
    stats: ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<StructuralLocusRetentionCharge, ParametricSectorCoverageError> {
    let terms = checked_bounded_add(
        "sector-coverage retained structural locus terms",
        stats.retained_structural_locus_terms,
        polynomial.term_count(),
        limits.max_retained_structural_locus_terms,
    )?;
    let remaining = limits
        .max_retained_structural_locus_bytes
        .checked_sub(stats.retained_structural_locus_bytes)
        .ok_or(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage retained structural locus bytes",
            requested: stats.retained_structural_locus_bytes,
            limit: limits.max_retained_structural_locus_bytes,
        })?;
    let local_bytes = bounded_polynomial_display_bytes(polynomial, remaining).map_err(|local| {
        ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage retained structural locus bytes",
            requested: stats
                .retained_structural_locus_bytes
                .checked_add(local.requested)
                .unwrap_or(usize::MAX),
            limit: limits.max_retained_structural_locus_bytes,
        }
    })?;
    let bytes = checked_bounded_add(
        "sector-coverage retained structural locus bytes",
        stats.retained_structural_locus_bytes,
        local_bytes,
        limits.max_retained_structural_locus_bytes,
    )?;
    Ok(StructuralLocusRetentionCharge { terms, bytes })
}

/// Exact representation selected for the one-atom part of a candidate's bad
/// formula.  A factored route is already canonical: ordinals are strictly
/// increasing representatives from the structural-locus table.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CandidateProductZeroRouting {
    /// Every supplied atom was a nonzero base-field unit, hence its zero locus
    /// is empty and contributes nothing to the bad formula.
    Omitted,
    /// A singleton factor or a checked, retained concrete product locus.
    ConcreteLocus(usize),
    /// The exact disjunction of factor-zero atoms, retained without creating a
    /// concrete product or decomposition witness.
    Factored(Vec<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductSupportAtCutoff {
    Within(usize),
    Exceeds,
}

impl ProductSupportAtCutoff {
    fn checked_add(self, right: usize, cutoff: usize) -> Self {
        match self {
            Self::Within(left) => match left.checked_add(right) {
                Some(value) if value <= cutoff => Self::Within(value),
                _ => Self::Exceeds,
            },
            Self::Exceeds => Self::Exceeds,
        }
    }

    fn checked_mul(self, right: usize, cutoff: usize) -> Self {
        match self {
            Self::Within(left) => match left.checked_mul(right) {
                Some(value) if value <= cutoff => Self::Within(value),
                _ => Self::Exceeds,
            },
            Self::Exceeds => Self::Exceeds,
        }
    }

    fn minimum(self, other: Self) -> Self {
        match (self, other) {
            (Self::Within(left), Self::Within(right)) => Self::Within(left.min(right)),
            (Self::Within(value), Self::Exceeds) | (Self::Exceeds, Self::Within(value)) => {
                Self::Within(value)
            }
            (Self::Exceeds, Self::Exceeds) => Self::Exceeds,
        }
    }
}

/// Compute
///
/// `min(product_j terms(p_j), product_i(1 + sum_j degree_i(p_j)))`
///
/// while retaining exact values only through `cutoff`.  Both factors are
/// conservative support bounds for a nonzero product.  Arithmetic overflow
/// therefore proves that branch exceeds every representable cutoff; it is a
/// representation decision, not a compilation failure.  Exponents are
/// visited exactly once per factor, rather than once per variable.
/// This representation envelope does not weaken or replace the independent
/// exact exponent, retained-term, native-envelope, or aggregate reconstruction
/// checks on a product selected for materialization.
fn whole_product_support_at_cutoff(
    structural_loci: &[ParametricPolynomial],
    factors: &[usize],
    cutoff: usize,
) -> Result<ProductSupportAtCutoff, ParametricSectorCoverageError> {
    let Some(&first_ordinal) = factors.first() else {
        return Ok(ProductSupportAtCutoff::Within(1));
    };
    let first = structural_loci
        .get(first_ordinal)
        .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
    let variable_count = first.raw().variables.len();
    let mut sparse_tuples = ProductSupportAtCutoff::Within(1);
    let mut summed_degrees = Vec::new();
    try_reserve_exact(
        "product materialization-bound summed degrees",
        &mut summed_degrees,
        variable_count,
    )?;
    summed_degrees.resize(variable_count, ProductSupportAtCutoff::Within(0));
    let mut maxima = Vec::new();
    try_reserve_exact(
        "product materialization-bound maxima",
        &mut maxima,
        variable_count,
    )?;
    maxima.resize(variable_count, u16::MIN);

    for &ordinal in factors {
        let factor = structural_loci
            .get(ordinal)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        if factor.is_zero() || factor.raw().variables.len() != variable_count {
            return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
        }
        let term_count = factor.term_count();
        sparse_tuples = sparse_tuples.checked_mul(term_count, cutoff);

        let expected_exponents = term_count.checked_mul(variable_count).ok_or(
            ParametricSectorCoverageError::ResourceCountOverflow {
                resource: "product materialization-bound exponent layout",
            },
        )?;
        if factor.raw().exponents.len() != expected_exponents {
            return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
        }
        if variable_count == 0 {
            continue;
        }

        maxima.fill(u16::MIN);
        for (entry, &exponent) in factor.raw().exponents.iter().enumerate() {
            let variable = entry % variable_count;
            maxima[variable] = maxima[variable].max(exponent);
        }
        for variable in 0..variable_count {
            summed_degrees[variable] =
                summed_degrees[variable].checked_add(usize::from(maxima[variable]), cutoff);
        }
    }

    let mut degree_box = ProductSupportAtCutoff::Within(1);
    for degree in summed_degrees {
        let width = degree.checked_add(1, cutoff);
        degree_box = match width {
            ProductSupportAtCutoff::Within(width) => degree_box.checked_mul(width, cutoff),
            ProductSupportAtCutoff::Exceeds => ProductSupportAtCutoff::Exceeds,
        };
    }
    Ok(sparse_tuples.minimum(degree_box))
}

/// Canonicalize the compiler's original atomic loci, preflight one bounded
/// whole-product support scan, and either retain a concrete checked product or
/// route the formula through the exact factor disjunction.  All counters are
/// staged until the chosen route succeeds.  In particular, the factored route
/// cannot consume witness, multiplication, or reconstruction counters.
fn route_candidate_product_zero_loci(
    context: &ParametricCoefficientContext,
    structural_loci: &mut Vec<ParametricPolynomial>,
    decompositions: &mut Vec<ParametricSectorProductZeroDecomposition>,
    atomic_loci: &[usize],
    stats: ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<
    (CandidateProductZeroRouting, ParametricSectorCoverageStats),
    ParametricSectorCoverageError,
> {
    let mut factors = Vec::<usize>::new();
    try_reserve_exact(
        "candidate product-zero factor canonicalization",
        &mut factors,
        atomic_loci.len(),
    )?;
    for &locus in atomic_loci {
        let factor = structural_loci
            .get(locus)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        if factor.is_zero() {
            return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
        }
        if context
            .polynomial_depends_on_indices_with_limits(factor, coverage_exact_algebra(limits))?
        {
            factors.push(locus);
        }
    }
    factors.sort_unstable();
    factors.dedup();
    match factors.as_slice() {
        [] => return Ok((CandidateProductZeroRouting::Omitted, stats)),
        [single] => {
            return Ok((CandidateProductZeroRouting::ConcreteLocus(*single), stats));
        }
        _ => {}
    }

    let local_exponent_entries = factors.iter().try_fold(0usize, |total, &ordinal| {
        let factor = structural_loci
            .get(ordinal)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        checked_add(
            "sector-coverage product materialization-bound exponent entries",
            total,
            factor.raw().exponents.len(),
        )
    })?;
    let mut staged = stats;
    staged.product_materialization_bound_factor_scans = checked_bounded_add(
        "sector-coverage product materialization-bound factor scans",
        staged.product_materialization_bound_factor_scans,
        factors.len(),
        limits.max_product_materialization_bound_factor_scans,
    )?;
    staged.product_materialization_bound_exponent_entries = checked_bounded_add(
        "sector-coverage product materialization-bound exponent entries",
        staged.product_materialization_bound_exponent_entries,
        local_exponent_entries,
        limits.max_product_materialization_bound_exponent_entries,
    )?;

    let support = whole_product_support_at_cutoff(
        structural_loci,
        &factors,
        limits.max_materialized_product_zero_support_terms,
    )?;
    if support == ProductSupportAtCutoff::Exceeds {
        staged.factored_product_zero_disjunctions = checked_bounded_add(
            "sector-coverage factored product-zero disjunctions",
            staged.factored_product_zero_disjunctions,
            1,
            limits.max_factored_product_zero_disjunctions,
        )?;
        staged.factored_product_zero_factor_references = checked_bounded_add(
            "sector-coverage factored product-zero factor references",
            staged.factored_product_zero_factor_references,
            factors.len(),
            limits.max_factored_product_zero_factor_references,
        )?;
        return Ok((CandidateProductZeroRouting::Factored(factors), staged));
    }

    let product = retain_product_zero_decomposition(
        context,
        structural_loci,
        decompositions,
        &factors,
        &mut staged,
        limits,
    )?
    .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
    Ok((CandidateProductZeroRouting::ConcreteLocus(product), staged))
}

/// Build and retain one exact product witness from the compiler's original
/// atomic locus ordinals. No post-hoc polynomial factorization is performed.
fn retain_product_zero_decomposition(
    context: &ParametricCoefficientContext,
    structural_loci: &mut Vec<ParametricPolynomial>,
    decompositions: &mut Vec<ParametricSectorProductZeroDecomposition>,
    atomic_loci: &[usize],
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<Option<usize>, ParametricSectorCoverageError> {
    let remaining_factor_references = remaining_limit(
        "sector-coverage product-zero factor references",
        limits.max_product_zero_factor_references,
        stats.product_zero_factor_references,
    )?;
    if atomic_loci.len() > remaining_factor_references {
        return Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage product-zero factor references",
            requested: stats
                .product_zero_factor_references
                .checked_add(atomic_loci.len())
                .unwrap_or(usize::MAX),
            limit: limits.max_product_zero_factor_references,
        });
    }
    let mut factors = Vec::<usize>::new();
    try_reserve_exact(
        "product-zero factor ordinal staging",
        &mut factors,
        atomic_loci.len(),
    )?;
    for &locus in atomic_loci {
        let factor = structural_loci
            .get(locus)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        if factor.is_zero() {
            return Err(ParametricSectorCoverageError::CandidateLeafMappingMismatch);
        }
        // Nonzero elements of K are units, so their equality loci are empty
        // and they must not appear in the lattice factor witness.
        if context
            .polynomial_depends_on_indices_with_limits(factor, coverage_exact_algebra(limits))?
        {
            factors.push(locus);
        }
    }
    factors.sort_unstable();
    factors.dedup();
    let Some((&first, rest)) = factors.split_first() else {
        return Ok(None);
    };
    if rest.is_empty() {
        // Replacing one atom by itself is not a decomposition and retaining a
        // self-reference would invite accidental recursive expansion later.
        return Ok(Some(first));
    }

    // Equal canonical factor lists have the same checked construction. Reuse
    // the earlier witness before both native multiplication and retention
    // charging; this is the canonical decomposition deduplication seam.
    if let Some(existing) = decompositions
        .iter()
        .find(|witness| witness.factor_locus_ordinals.as_ref() == factors.as_slice())
    {
        return Ok(Some(existing.product_locus_ordinal));
    }

    let mut staged = *stats;

    // Preflight all witness-sized resources and the aggregate multiplication
    // census before doing Symbolica work. Whether the product needs a new
    // structural representative cannot soundly be decided yet: a different
    // factor list may multiply to a K-unit associate already in the table.
    let decomposition_count = checked_bounded_add(
        "sector-coverage product-zero decompositions",
        staged.product_zero_decompositions,
        1,
        limits.max_product_zero_decompositions,
    )?;
    let factor_references = checked_bounded_add(
        "sector-coverage product-zero factor references",
        staged.product_zero_factor_references,
        factors.len(),
        limits.max_product_zero_factor_references,
    )?;
    checked_bounded_add(
        "sector-coverage product-zero multiplications",
        staged.product_zero_multiplications,
        rest.len(),
        limits.max_product_zero_multiplications,
    )?;
    try_reserve_exact("product-zero decompositions", decompositions, 1)?;
    // Reserve the possible new representative before native multiplication;
    // exact associate lookup below may prove that this slot is unnecessary.
    try_reserve_exact("retained structural loci", structural_loci, 1)?;

    let mut product = structural_loci
        .get(first)
        .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?
        .clone();
    for &factor_ordinal in rest {
        let factor = structural_loci
            .get(factor_ordinal)
            .ok_or(ParametricSectorCoverageError::CandidateLeafMappingMismatch)?;
        product = bounded_product_reconstruction(context, &product, factor, &mut staged, limits)?;
    }

    let existing_product = find_structural_locus(
        context,
        structural_loci,
        &product,
        coverage_exact_algebra(limits),
        &mut staged,
        limits,
    )?;
    let product_locus_ordinal = existing_product.unwrap_or(structural_loci.len());
    // Only now can exact retained-table limits be evaluated without falsely
    // rejecting a product represented by an existing K-unit associate.
    let structural_charge = if existing_product.is_none() {
        let requested = checked_add(
            "unique sector-coverage predicates",
            structural_loci.len(),
            1,
        )?;
        check_limit(
            "unique sector-coverage predicates",
            requested,
            limits.max_unique_predicates,
        )?;
        Some(preflight_structural_locus_retention(
            &product, staged, limits,
        )?)
    } else {
        None
    };

    if let Some(charge) = structural_charge {
        structural_loci.push(product);
        staged.retained_structural_locus_terms = charge.terms;
        staged.retained_structural_locus_bytes = charge.bytes;
    }
    decompositions.push(ParametricSectorProductZeroDecomposition {
        product_locus_ordinal,
        factor_locus_ordinals: factors.into_boxed_slice(),
    });
    staged.product_zero_decompositions = decomposition_count;
    staged.product_zero_factor_references = factor_references;
    *stats = staged;
    Ok(Some(product_locus_ordinal))
}

/// Replay phase one: recensus every retained structural byte/count and reject
/// malformed ordinal structure before any Symbolica multiplication or exact
/// associate division can run. Algebraic validity is then established by the
/// deterministic compiler rebuild and final full-payload comparison.
fn preflight_product_zero_payload(
    _context: &ParametricCoefficientContext,
    structural_loci: &[ParametricPolynomial],
    decompositions: &[ParametricSectorProductZeroDecomposition],
    stats: ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    check_limit(
        "unique sector-coverage predicates",
        structural_loci.len(),
        limits.max_unique_predicates,
    )?;
    check_limit(
        "sector-coverage product-zero decompositions",
        decompositions.len(),
        limits.max_product_zero_decompositions,
    )?;

    let mut retained_terms = 0usize;
    let mut retained_bytes = 0usize;
    for polynomial in structural_loci {
        retained_terms = checked_bounded_add(
            "sector-coverage retained structural locus terms",
            retained_terms,
            polynomial.term_count(),
            limits.max_retained_structural_locus_terms,
        )?;
        let remaining = limits
            .max_retained_structural_locus_bytes
            .checked_sub(retained_bytes)
            .ok_or(ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage retained structural locus bytes",
                requested: retained_bytes,
                limit: limits.max_retained_structural_locus_bytes,
            })?;
        let local = bounded_polynomial_display_bytes(polynomial, remaining).map_err(|local| {
            ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage retained structural locus bytes",
                requested: retained_bytes
                    .checked_add(local.requested)
                    .unwrap_or(usize::MAX),
                limit: limits.max_retained_structural_locus_bytes,
            }
        })?;
        retained_bytes = checked_bounded_add(
            "sector-coverage retained structural locus bytes",
            retained_bytes,
            local,
            limits.max_retained_structural_locus_bytes,
        )?;
    }

    let mut factor_references = 0usize;
    let mut multiplication_count = 0usize;
    for (ordinal, witness) in decompositions.iter().enumerate() {
        if ordinal > 0
            && decomposition_cmp(&decompositions[ordinal - 1], witness) != std::cmp::Ordering::Less
        {
            return Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch);
        }
        if witness.product_locus_ordinal >= structural_loci.len()
            || witness.factor_locus_ordinals.len() < 2
        {
            return Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch);
        }
        factor_references = checked_bounded_add(
            "sector-coverage product-zero factor references",
            factor_references,
            witness.factor_locus_ordinals.len(),
            limits.max_product_zero_factor_references,
        )?;
        multiplication_count = checked_bounded_add(
            "sector-coverage product-zero multiplications",
            multiplication_count,
            witness.factor_locus_ordinals.len() - 1,
            limits.max_product_zero_multiplications,
        )?;
        let mut previous = None;
        for &factor_ordinal in witness.factor_locus_ordinals.iter() {
            if factor_ordinal >= structural_loci.len()
                || previous.is_some_and(|previous| factor_ordinal <= previous)
            {
                return Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch);
            }
            previous = Some(factor_ordinal);
        }
    }

    if stats.unique_predicates != structural_loci.len()
        || stats.retained_structural_locus_terms != retained_terms
        || stats.retained_structural_locus_bytes != retained_bytes
        || stats.product_zero_decompositions != decompositions.len()
        || stats.product_zero_factor_references != factor_references
        || stats.product_zero_multiplications != multiplication_count
    {
        return Err(ParametricSectorCoverageError::ProductZeroCensusMismatch);
    }
    let minimum_factored_references = checked_mul(
        "sector-coverage factored product-zero factor references",
        stats.factored_product_zero_disjunctions,
        2,
    )?;
    if stats.factored_product_zero_factor_references < minimum_factored_references
        || stats.product_materialization_bound_factor_scans
            < stats.factored_product_zero_factor_references
    {
        return Err(ParametricSectorCoverageError::ProductZeroCensusMismatch);
    }
    for (resource, requested, limit) in [
        (
            "sector-coverage product materialization-bound factor scans",
            stats.product_materialization_bound_factor_scans,
            limits.max_product_materialization_bound_factor_scans,
        ),
        (
            "sector-coverage product materialization-bound exponent entries",
            stats.product_materialization_bound_exponent_entries,
            limits.max_product_materialization_bound_exponent_entries,
        ),
        (
            "sector-coverage factored product-zero disjunctions",
            stats.factored_product_zero_disjunctions,
            limits.max_factored_product_zero_disjunctions,
        ),
        (
            "sector-coverage factored product-zero factor references",
            stats.factored_product_zero_factor_references,
            limits.max_factored_product_zero_factor_references,
        ),
        (
            "sector-coverage product reconstruction term pairs",
            stats.product_reconstruction_term_pairs,
            limits.max_product_reconstruction_term_pairs,
        ),
        (
            "sector-coverage product reconstruction output terms",
            stats.product_reconstruction_output_terms,
            limits.max_product_reconstruction_output_terms,
        ),
        (
            "sector-coverage product reconstruction output exponent entries",
            stats.product_reconstruction_output_exponent_entries,
            limits.max_product_reconstruction_output_exponent_entries,
        ),
        (
            "sector-coverage product reconstruction output coefficient bits",
            stats.product_reconstruction_output_coefficient_bits,
            limits.max_product_reconstruction_output_coefficient_bits,
        ),
        (
            "sector-coverage structural locus associate comparisons",
            stats.structural_locus_associate_comparisons,
            limits.max_structural_locus_associate_comparisons,
        ),
        (
            "sector-coverage structural locus associate term pairs",
            stats.structural_locus_associate_term_pairs,
            limits.max_structural_locus_associate_term_pairs,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn decomposition_cmp(
    left: &ParametricSectorProductZeroDecomposition,
    right: &ParametricSectorProductZeroDecomposition,
) -> std::cmp::Ordering {
    left.product_locus_ordinal
        .cmp(&right.product_locus_ordinal)
        .then_with(|| left.factor_locus_ordinals.cmp(&right.factor_locus_ordinals))
}

fn canonicalize_product_zero_decompositions(
    decompositions: &mut Vec<ParametricSectorProductZeroDecomposition>,
) {
    decompositions.sort_by(decomposition_cmp);
    decompositions.dedup();
}

fn bounded_polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, BoundedByteLimit> {
    let mut writer = BoundedByteCounter { bytes: 0, limit };
    if write!(&mut writer, "{}", polynomial.raw()).is_err() {
        return Err(BoundedByteLimit {
            requested: writer.bytes.max(limit.saturating_add(1)),
        });
    }
    Ok(writer.bytes)
}

struct BoundedByteLimit {
    requested: usize,
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn bounded_product_reconstruction(
    context: &ParametricCoefficientContext,
    left: &ParametricPolynomial,
    right: &ParametricPolynomial,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<ParametricPolynomial, ParametricSectorCoverageError> {
    let term_pairs = checked_mul(
        "sector-coverage product reconstruction term pairs",
        left.term_count(),
        right.term_count(),
    )?;
    let next_term_pairs = checked_bounded_add(
        "sector-coverage product reconstruction term pairs",
        stats.product_reconstruction_term_pairs,
        term_pairs,
        limits.max_product_reconstruction_term_pairs,
    )?;
    let next_multiplications = checked_bounded_add(
        "sector-coverage product-zero multiplications",
        stats.product_zero_multiplications,
        1,
        limits.max_product_zero_multiplications,
    )?;

    let remaining_term_pairs = remaining_limit(
        "sector-coverage product reconstruction term pairs",
        limits.max_product_reconstruction_term_pairs,
        stats.product_reconstruction_term_pairs,
    )?;
    let remaining_output_terms = remaining_limit(
        "sector-coverage product reconstruction output terms",
        limits.max_product_reconstruction_output_terms,
        stats.product_reconstruction_output_terms,
    )?;
    let remaining_output_exponents = remaining_limit(
        "sector-coverage product reconstruction output exponent entries",
        limits.max_product_reconstruction_output_exponent_entries,
        stats.product_reconstruction_output_exponent_entries,
    )?;
    let remaining_output_bits = remaining_limit(
        "sector-coverage product reconstruction output coefficient bits",
        limits.max_product_reconstruction_output_coefficient_bits,
        stats.product_reconstruction_output_coefficient_bits,
    )?;

    let variables = left.raw().variables.len();
    // Both authenticated factors are nonzero, and `Z[x]` is an integral
    // domain. The product therefore has at least one sparse term, one dense
    // exponent row, and one coefficient bit. Reject exhausted aggregate
    // output budgets with the coverage layer's typed resource error before
    // passing a zero ceiling into native exact algebra.
    checked_bounded_add(
        "sector-coverage product reconstruction output terms",
        stats.product_reconstruction_output_terms,
        1,
        limits.max_product_reconstruction_output_terms,
    )?;
    checked_bounded_add(
        "sector-coverage product reconstruction output exponent entries",
        stats.product_reconstruction_output_exponent_entries,
        variables,
        limits.max_product_reconstruction_output_exponent_entries,
    )?;
    let exponent_limited_terms = if variables == 0 {
        usize::MAX
    } else {
        remaining_output_exponents / variables
    };
    let coefficient_bit_bound = largest_polynomial_coefficient_bits(left)?
        .checked_add(largest_polynomial_coefficient_bits(right)?)
        .and_then(|bits| bits.checked_add(ceil_log2_usize(term_pairs)))
        .ok_or(ParametricSectorCoverageError::ResourceCountOverflow {
            resource: "sector-coverage product reconstruction coefficient bit bound",
        })?
        .max(1);
    check_limit(
        "sector-coverage product reconstruction output coefficient bits",
        coefficient_bit_bound,
        remaining_output_bits,
    )?;
    let coefficient_limited_terms = remaining_output_bits / coefficient_bit_bound;

    // Preserve the exact-algebra retained-term ceiling for both inputs and
    // the actual canonical output.  Admit the larger conservative native
    // support envelope independently, bounded by both its per-product ceiling
    // and every remaining aggregate output/exponent/bit budget.
    let mut exact = coverage_exact_algebra(limits);
    exact.max_term_operations = exact.max_term_operations.min(remaining_term_pairs);
    let max_native_output_term_bound = limits
        .max_product_reconstruction_native_output_term_bound
        .min(remaining_output_terms)
        .min(exponent_limited_terms)
        .min(coefficient_limited_terms);
    let product = context.multiply_polynomial_conditions_with_limits_and_native_output_bound(
        left,
        right,
        exact,
        max_native_output_term_bound,
    )?;

    let output_terms = product.term_count();
    let output_exponents = product.raw().exponents.len();
    let output_bits = polynomial_coefficient_bit_payload(&product)?;
    stats.product_reconstruction_term_pairs = next_term_pairs;
    stats.product_zero_multiplications = next_multiplications;
    stats.product_reconstruction_output_terms = checked_bounded_add(
        "sector-coverage product reconstruction output terms",
        stats.product_reconstruction_output_terms,
        output_terms,
        limits.max_product_reconstruction_output_terms,
    )?;
    stats.product_reconstruction_output_exponent_entries = checked_bounded_add(
        "sector-coverage product reconstruction output exponent entries",
        stats.product_reconstruction_output_exponent_entries,
        output_exponents,
        limits.max_product_reconstruction_output_exponent_entries,
    )?;
    stats.product_reconstruction_output_coefficient_bits = checked_bounded_add(
        "sector-coverage product reconstruction output coefficient bits",
        stats.product_reconstruction_output_coefficient_bits,
        output_bits,
        limits.max_product_reconstruction_output_coefficient_bits,
    )?;
    Ok(product)
}

fn bounded_same_structural_locus(
    context: &ParametricCoefficientContext,
    left: &ParametricPolynomial,
    right: &ParametricPolynomial,
    mut exact: crate::algebra::ExactAlgebraLimits,
    stats: &mut ParametricSectorCoverageStats,
    limits: ParametricSectorCoverageLimits,
) -> Result<bool, ParametricSectorCoverageError> {
    let comparisons = checked_bounded_add(
        "sector-coverage structural locus associate comparisons",
        stats.structural_locus_associate_comparisons,
        1,
        limits.max_structural_locus_associate_comparisons,
    )?;
    if left == right {
        stats.structural_locus_associate_comparisons = comparisons;
        return Ok(true);
    }

    // Polynomial associate division treats each input as a rational
    // polynomial with unit denominator. Its two checked product bounds are
    // exactly the left and right source term counts.
    let local_term_pairs = checked_add(
        "sector-coverage structural locus associate term pairs",
        left.term_count(),
        right.term_count(),
    )?;
    let term_pairs = checked_bounded_add(
        "sector-coverage structural locus associate term pairs",
        stats.structural_locus_associate_term_pairs,
        local_term_pairs,
        limits.max_structural_locus_associate_term_pairs,
    )?;
    let remaining = remaining_limit(
        "sector-coverage structural locus associate term pairs",
        limits.max_structural_locus_associate_term_pairs,
        stats.structural_locus_associate_term_pairs,
    )?;
    exact.max_term_operations = exact.max_term_operations.min(remaining);
    let result = context.polynomial_loci_are_associates_with_limits(left, right, exact)?;
    stats.structural_locus_associate_comparisons = comparisons;
    stats.structural_locus_associate_term_pairs = term_pairs;
    Ok(result)
}

fn largest_polynomial_coefficient_bits(
    polynomial: &ParametricPolynomial,
) -> Result<usize, ParametricSectorCoverageError> {
    polynomial
        .raw()
        .coefficients
        .iter()
        .try_fold(0usize, |largest, coefficient| {
            Ok(largest.max(integer_magnitude_bits(coefficient)?))
        })
}

fn polynomial_coefficient_bit_payload(
    polynomial: &ParametricPolynomial,
) -> Result<usize, ParametricSectorCoverageError> {
    polynomial
        .raw()
        .coefficients
        .iter()
        .try_fold(0usize, |total, coefficient| {
            checked_add(
                "sector-coverage product reconstruction output coefficient bits",
                total,
                integer_magnitude_bits(coefficient)?,
            )
        })
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ParametricSectorCoverageError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ParametricSectorCoverageError::ResourceCountOverflow {
        resource: "sector-coverage integer coefficient bits",
    })
}

fn ceil_log2_usize(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        (usize::BITS - (value - 1).leading_zeros()) as usize
    }
}

fn attempt_payload_eq(
    left: (
        &SectorCoverageCandidateAttempt,
        &SectorCoverageCandidateAttempt,
    ),
) -> bool {
    let (left, right) = left;
    left.ordinal == right.ordinal && left.compilation.payload_eq(&right.compilation)
}

fn effective_sector_limits(limits: ParametricSectorCoverageLimits) -> SymbolicSectorCaseLimits {
    let mut sector = limits.sector_cases;
    sector.max_live_cases = sector
        .max_live_cases
        .min(limits.max_global_leaf_classifications);
    sector
}

fn coverage_exact_algebra(
    limits: ParametricSectorCoverageLimits,
) -> crate::algebra::ExactAlgebraLimits {
    limits.generated_when_bad.when_bad.arithmetic.exact_algebra
}

pub(crate) fn validate_coherent_limits(
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    let arithmetic = coverage_exact_algebra(limits);
    if limits
        .generated_when_bad
        .when_bad
        .sector_cases
        .exact_algebra
        != arithmetic
    {
        return Err(ParametricSectorCoverageError::InconsistentLimits {
            first: "generated WhenBad arithmetic exact algebra",
            second: "generated WhenBad sector-case exact algebra",
        });
    }
    if limits.sector_cases.exact_algebra != arithmetic {
        return Err(ParametricSectorCoverageError::InconsistentLimits {
            first: "generated WhenBad arithmetic exact algebra",
            second: "global sector-case exact algebra",
        });
    }
    if limits.coordinate_loci.exact_algebra != arithmetic {
        return Err(ParametricSectorCoverageError::InconsistentLimits {
            first: "generated WhenBad arithmetic exact algebra",
            second: "coordinate-locus pruning exact algebra",
        });
    }
    Ok(())
}

pub(crate) fn validate_family_context(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), ParametricSectorCoverageError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(ParametricSectorCoverageError::WrongFamily);
    }
    if family.denominator_count() != context.index_count() {
        return Err(ParametricSectorCoverageError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    // The private index namespace is intentionally caller-owned: several
    // generated stages may need to share one exact K(n) map. For nonempty
    // coverage, every candidate is bound to this exact context fingerprint
    // below and GeneratedWhenBad freshly regenerates its IBP/LI sources in
    // that same context. Empty coverage carries no rule identity to bind and
    // proves only that the supplied candidate set leaves the orthant uncovered.
    Ok(())
}

pub(crate) fn validate_row_span_binding(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorCoverageError> {
    if row_span.family_fingerprint() != family.fingerprint() {
        return Err(ParametricSectorCoverageError::WrongFamily);
    }
    if row_span.context_fingerprint() != context.fingerprint() {
        return Err(ParametricSectorCoverageError::WrongContext);
    }
    if row_span.ibp_config() != limits.generated_when_bad.ibp {
        return Err(ParametricSectorCoverageError::GeneratedWhenBad(
            GeneratedWhenBadError::SharedRowSpanIbpConfigMismatch,
        ));
    }
    if row_span.config() != limits.generated_when_bad.row_span {
        return Err(ParametricSectorCoverageError::GeneratedWhenBad(
            GeneratedWhenBadError::SharedRowSpanConfigMismatch,
        ));
    }
    Ok(())
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorCoverageError> {
    left.checked_add(right)
        .ok_or(ParametricSectorCoverageError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorCoverageError> {
    left.checked_mul(right)
        .ok_or(ParametricSectorCoverageError::ResourceCountOverflow { resource })
}

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, ParametricSectorCoverageError> {
    limit
        .checked_sub(consumed)
        .ok_or(ParametricSectorCoverageError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        })
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ParametricSectorCoverageError> {
    values.try_reserve_exact(additional).map_err(|_| {
        ParametricSectorCoverageError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

fn checked_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, ParametricSectorCoverageError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorCoverageError> {
    if requested > limit {
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricSectorCoverageError {
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    CandidateWrongFamily {
        ordinal: usize,
    },
    CandidateWrongContext {
        ordinal: usize,
    },
    CandidateWrongSector {
        ordinal: usize,
    },
    CandidateOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    SharedRowSpanAllocationMismatch {
        ordinal: usize,
    },
    SharedRowSpanCertificateMismatch,
    SchemaMismatch,
    ReplayMismatch,
    CandidateLeafMappingMismatch,
    ProductZeroDecompositionMismatch,
    ProductZeroCensusMismatch,
    EmptyLocusWitnessMismatch,
    PartitionEvaluationMismatch,
    InconsistentLimits {
        first: &'static str,
        second: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    GeneratedWhenBad(GeneratedWhenBadError),
    SectorCase(SymbolicSectorCaseError),
    CoordinateLocus(CoordinateEqualityLocusError),
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for ParametricSectorCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter
                .write_str("sector coverage family and coefficient context fingerprints differ"),
            Self::WrongContext => {
                formatter.write_str("sector coverage belongs to a different coefficient context")
            }
            Self::WrongArity { expected, actual } => {
                write!(formatter, "sector arity is {actual}, expected {expected}")
            }
            Self::CandidateWrongFamily { ordinal } => {
                write!(
                    formatter,
                    "sector-coverage candidate {ordinal} belongs to another family"
                )
            }
            Self::CandidateWrongContext { ordinal } => write!(
                formatter,
                "sector-coverage candidate {ordinal} belongs to another coefficient context"
            ),
            Self::CandidateWrongSector { ordinal } => {
                write!(
                    formatter,
                    "sector-coverage candidate {ordinal} belongs to another sector"
                )
            }
            Self::CandidateOrdinalMismatch { expected, actual } => write!(
                formatter,
                "sector-coverage attempt ordinal is {actual}, expected {expected}"
            ),
            Self::SharedRowSpanAllocationMismatch { ordinal } => write!(
                formatter,
                "sector-coverage candidate {ordinal} does not retain the shared row-span allocation"
            ),
            Self::SharedRowSpanCertificateMismatch => formatter.write_str(
                "sector-coverage certificate is bound to another shared row-span allocation",
            ),
            Self::SchemaMismatch => formatter.write_str("sector-coverage schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("sector-coverage replay mismatch"),
            Self::CandidateLeafMappingMismatch => formatter
                .write_str("a global sector case did not map to exactly one local candidate case"),
            Self::ProductZeroDecompositionMismatch => {
                formatter.write_str("a retained product-zero decomposition does not exactly replay")
            }
            Self::ProductZeroCensusMismatch => formatter.write_str(
                "the retained product-zero structural census does not match its statistics",
            ),
            Self::EmptyLocusWitnessMismatch => formatter.write_str(
                "a proved-empty sector locus has an invalid coordinate contradiction witness",
            ),
            Self::PartitionEvaluationMismatch => formatter.write_str(
                "sector-coverage partition did not evaluate to exactly one terminal leaf",
            ),
            Self::InconsistentLimits { first, second } => write!(
                formatter,
                "sector-coverage limits are inconsistent between {first} and {second}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "sector coverage could not reserve {requested} entries for {resource}"
            ),
            Self::GeneratedWhenBad(error) => error.fmt(formatter),
            Self::SectorCase(error) => error.fmt(formatter),
            Self::CoordinateLocus(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricSectorCoverageError {}

impl From<GeneratedWhenBadError> for ParametricSectorCoverageError {
    fn from(value: GeneratedWhenBadError) -> Self {
        Self::GeneratedWhenBad(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for ParametricSectorCoverageError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::GeneratedWhenBad(GeneratedWhenBadError::RowSpan(value))
    }
}

impl From<SymbolicSectorCaseError> for ParametricSectorCoverageError {
    fn from(value: SymbolicSectorCaseError) -> Self {
        Self::SectorCase(value)
    }
}

impl From<CoordinateEqualityLocusError> for ParametricSectorCoverageError {
    fn from(value: CoordinateEqualityLocusError) -> Self {
        Self::CoordinateLocus(value)
    }
}

impl From<ParametricCoefficientError> for ParametricSectorCoverageError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

#[cfg(test)]
mod product_zero_decomposition_tests {
    use super::*;
    use crate::{
        AffineDenominator, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        IntegralOrderingPolicy, ParametricIbpGenerator, algebra::CoefficientContext,
    };

    fn polynomial_context() -> (
        ParametricCoefficientContext,
        ParametricPolynomial,
        ParametricPolynomial,
        ParametricPolynomial,
    ) {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "coverage-v4-products", 2).unwrap();
        let first = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let second = context
            .numerator_condition(&context.index(1).unwrap())
            .unwrap();
        let base_unit = context
            .numerator_condition(&context.lift(&base.parameter("d").unwrap()).unwrap())
            .unwrap();
        (context, first, second, base_unit)
    }

    fn retain_locus(
        context: &ParametricCoefficientContext,
        loci: &mut Vec<ParametricPolynomial>,
        polynomial: &ParametricPolynomial,
        stats: &mut ParametricSectorCoverageStats,
        limits: ParametricSectorCoverageLimits,
    ) -> usize {
        insert_unique_structural_locus(
            context,
            loci,
            polynomial,
            limits.max_unique_predicates,
            coverage_exact_algebra(limits),
            stats,
            limits,
        )
        .unwrap()
    }

    fn seeded_loci() -> (
        ParametricCoefficientContext,
        Vec<ParametricPolynomial>,
        ParametricSectorCoverageStats,
        [usize; 3],
    ) {
        let (context, first, second, base_unit) = polynomial_context();
        let limits = ParametricSectorCoverageLimits::default();
        let mut loci = Vec::new();
        let mut stats = ParametricSectorCoverageStats::default();
        let ordinals = [
            retain_locus(&context, &mut loci, &first, &mut stats, limits),
            retain_locus(&context, &mut loci, &second, &mut stats, limits),
            retain_locus(&context, &mut loci, &base_unit, &mut stats, limits),
        ];
        (context, loci, stats, ordinals)
    }

    #[test]
    fn exact_product_provenance_omits_units_and_canonicalizes_duplicates() {
        let (context, mut loci, mut stats, [first, second, base_unit]) = seeded_loci();
        let limits = ParametricSectorCoverageLimits::default();
        let mut decompositions = Vec::new();
        let product = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[second, first, first, base_unit],
            &mut stats,
            limits,
        )
        .unwrap()
        .unwrap();
        assert_eq!(decompositions.len(), 1);
        assert_eq!(decompositions[0].product_locus_ordinal(), product);
        assert_eq!(decompositions[0].factor_locus_ordinals(), [first, second]);
        assert_eq!(stats.product_zero_decompositions(), 1);
        assert_eq!(stats.product_zero_factor_references(), 2);
        assert_eq!(stats.product_zero_multiplications(), 1);

        let before = stats;
        let duplicate = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[first, base_unit, second],
            &mut stats,
            limits,
        )
        .unwrap();
        assert_eq!(duplicate, Some(product));
        assert_eq!(decompositions.len(), 1);
        assert_eq!(
            stats, before,
            "deduplication must happen before multiplication"
        );

        let singleton = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[first],
            &mut stats,
            limits,
        )
        .unwrap();
        assert_eq!(singleton, Some(first));
        assert_eq!(
            decompositions.len(),
            1,
            "one-factor no-ops are not witnesses"
        );
        stats.unique_predicates = loci.len();
        preflight_product_zero_payload(&context, &loci, &decompositions, stats, limits).unwrap();
    }

    #[test]
    fn distinct_factor_lists_share_one_exact_product_representative() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-v4-shared-products",
            3,
        )
        .unwrap();
        let limits = ParametricSectorCoverageLimits::default();
        let factors = (0..3)
            .map(|index| {
                context
                    .numerator_condition(&context.index(index).unwrap())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let composite = context
            .multiply_polynomial_conditions_with_limits(
                &factors[0],
                &factors[1],
                coverage_exact_algebra(limits),
            )
            .unwrap();
        let mut loci = Vec::new();
        let mut stats = ParametricSectorCoverageStats::default();
        let first = retain_locus(&context, &mut loci, &factors[0], &mut stats, limits);
        let second = retain_locus(&context, &mut loci, &factors[1], &mut stats, limits);
        let third = retain_locus(&context, &mut loci, &factors[2], &mut stats, limits);
        let composite = retain_locus(&context, &mut loci, &composite, &mut stats, limits);
        let mut decompositions = Vec::new();

        let long = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[first, second, third],
            &mut stats,
            limits,
        )
        .unwrap()
        .unwrap();
        let short = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[composite, third],
            &mut stats,
            limits,
        )
        .unwrap()
        .unwrap();
        assert_eq!(long, short);
        canonicalize_product_zero_decompositions(&mut decompositions);
        assert_eq!(decompositions.len(), 2);
        assert!(
            decompositions
                .iter()
                .all(|witness| witness.product_locus_ordinal() == long)
        );
        assert_ne!(
            decompositions[0].factor_locus_ordinals(),
            decompositions[1].factor_locus_ordinals()
        );
        stats.unique_predicates = loci.len();
        preflight_product_zero_payload(&context, &loci, &decompositions, stats, limits).unwrap();
    }

    #[test]
    fn product_reconstruction_reuses_a_nontrivial_base_field_associate() {
        let (context, mut loci, mut stats, [first, second, base_unit]) = seeded_loci();
        let limits = ParametricSectorCoverageLimits::default();
        let unscaled = context
            .multiply_polynomial_conditions_with_limits(
                &loci[first],
                &loci[second],
                coverage_exact_algebra(limits),
            )
            .unwrap();
        let scaled = context
            .multiply_polynomial_conditions_with_limits(
                &unscaled,
                &loci[base_unit],
                coverage_exact_algebra(limits),
            )
            .unwrap();
        let scaled_ordinal = retain_locus(&context, &mut loci, &scaled, &mut stats, limits);
        let comparisons_before = stats.structural_locus_associate_comparisons();
        let term_pairs_before = stats.structural_locus_associate_term_pairs();
        let mut decompositions = Vec::new();
        let product = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[first, second],
            &mut stats,
            limits,
        )
        .unwrap()
        .unwrap();

        assert_eq!(product, scaled_ordinal);
        assert_ne!(loci[product], unscaled);
        assert!(
            context
                .polynomial_loci_are_associates_with_limits(
                    &loci[product],
                    &unscaled,
                    coverage_exact_algebra(limits),
                )
                .unwrap()
        );
        assert!(stats.structural_locus_associate_comparisons() > comparisons_before);
        assert!(stats.structural_locus_associate_term_pairs() > term_pairs_before);
        stats.unique_predicates = loci.len();
        preflight_product_zero_payload(&context, &loci, &decompositions, stats, limits).unwrap();
    }

    #[test]
    fn strict_v4_budgets_fail_before_durable_retention() {
        for (resource, budget) in [
            ("sector-coverage product-zero decompositions", 0),
            ("sector-coverage product-zero factor references", 1),
            ("sector-coverage product-zero multiplications", 2),
        ] {
            let (context, mut loci, mut stats, [first, second, _]) = seeded_loci();
            let loci_before = loci.clone();
            let stats_before = stats;
            let mut decompositions = Vec::new();
            let mut limits = ParametricSectorCoverageLimits::default();
            match budget {
                0 => limits.max_product_zero_decompositions = 0,
                1 => limits.max_product_zero_factor_references = 1,
                2 => limits.max_product_zero_multiplications = 0,
                _ => unreachable!(),
            }
            assert!(matches!(
                retain_product_zero_decomposition(
                    &context,
                    &mut loci,
                    &mut decompositions,
                    &[first, second],
                    &mut stats,
                    limits,
                ),
                Err(ParametricSectorCoverageError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(loci, loci_before);
            assert!(decompositions.is_empty());
            assert_eq!(stats, stats_before);
        }

        for bytes in [false, true] {
            let (context, mut loci, mut stats, [first, second, _]) = seeded_loci();
            let loci_before = loci.clone();
            let stats_before = stats;
            let mut decompositions = Vec::new();
            let mut limits = ParametricSectorCoverageLimits::default();
            let expected = if bytes {
                limits.max_retained_structural_locus_bytes =
                    stats.retained_structural_locus_bytes();
                "sector-coverage retained structural locus bytes"
            } else {
                limits.max_retained_structural_locus_terms =
                    stats.retained_structural_locus_terms();
                "sector-coverage retained structural locus terms"
            };
            assert!(matches!(
                retain_product_zero_decomposition(
                    &context,
                    &mut loci,
                    &mut decompositions,
                    &[first, second],
                    &mut stats,
                    limits,
                ),
                Err(ParametricSectorCoverageError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == expected
            ));
            assert_eq!(loci, loci_before);
            assert!(decompositions.is_empty());
            assert_eq!(stats, stats_before);
        }
    }

    #[test]
    fn coverage_native_product_envelope_is_exact_transactional_and_censused() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-v4-native-product-envelope",
            1,
        )
        .unwrap();
        let index = context.index(0).unwrap();
        let index_squared = context.mul(&index, &index).unwrap();
        let one_plus_index = context
            .numerator_condition(&context.add(&context.one(), &index).unwrap())
            .unwrap();
        let left = context
            .numerator_condition(&context.add(&context.one(), &index_squared).unwrap())
            .unwrap();
        let right = context
            .numerator_condition(
                &context
                    .add(
                        &context.one(),
                        &context.mul(&context.integer(2), &index_squared).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let exact = crate::algebra::ExactAlgebraLimits {
            max_exponent: 4,
            max_polynomial_terms: 3,
            max_term_operations: 4,
        };
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.generated_when_bad.when_bad.arithmetic.exact_algebra = exact;
        limits
            .generated_when_bad
            .when_bad
            .sector_cases
            .exact_algebra = exact;
        limits.sector_cases.exact_algebra = exact;
        limits.coordinate_loci.exact_algebra = exact;
        limits.max_product_reconstruction_native_output_term_bound = 4;
        validate_coherent_limits(limits).unwrap();

        let mut loci = Vec::new();
        let mut stats = ParametricSectorCoverageStats::default();
        let left_ordinal = retain_locus(&context, &mut loci, &left, &mut stats, limits);
        let right_ordinal = retain_locus(&context, &mut loci, &right, &mut stats, limits);
        let mut decompositions = Vec::new();
        let product_ordinal = retain_product_zero_decomposition(
            &context,
            &mut loci,
            &mut decompositions,
            &[left_ordinal, right_ordinal],
            &mut stats,
            limits,
        )
        .unwrap()
        .unwrap();
        assert_eq!(loci[product_ordinal].term_count(), 3);
        assert_eq!(stats.product_reconstruction_term_pairs(), 4);
        assert_eq!(stats.product_reconstruction_output_terms(), 3);
        context
            .validate_polynomial_with_limits(&loci[product_ordinal], exact)
            .unwrap();

        canonicalize_product_zero_decompositions(&mut decompositions);
        stats.unique_predicates = loci.len();
        preflight_product_zero_payload(&context, &loci, &decompositions, stats, limits).unwrap();
        let stats_before_reuse = stats;
        assert_eq!(
            retain_product_zero_decomposition(
                &context,
                &mut loci,
                &mut decompositions,
                &[left_ordinal, right_ordinal],
                &mut stats,
                limits,
            )
            .unwrap(),
            Some(product_ordinal),
        );
        assert_eq!(stats, stats_before_reuse);

        let mut strict_limits = limits;
        strict_limits.max_product_reconstruction_native_output_term_bound = 3;
        let mut strict_loci = Vec::new();
        let mut strict_stats = ParametricSectorCoverageStats::default();
        let strict_left = retain_locus(
            &context,
            &mut strict_loci,
            &left,
            &mut strict_stats,
            strict_limits,
        );
        let strict_right = retain_locus(
            &context,
            &mut strict_loci,
            &right,
            &mut strict_stats,
            strict_limits,
        );
        let strict_loci_before = strict_loci.clone();
        let strict_stats_before = strict_stats;
        let mut strict_decompositions = Vec::new();
        assert_eq!(
            retain_product_zero_decomposition(
                &context,
                &mut strict_loci,
                &mut strict_decompositions,
                &[strict_left, strict_right],
                &mut strict_stats,
                strict_limits,
            )
            .unwrap_err(),
            ParametricSectorCoverageError::ParametricCoefficient(
                ParametricCoefficientError::ExactAlgebra(
                    crate::algebra::ExactAlgebraError::ResourceLimit {
                        resource: "exact polynomial multiplication output terms",
                        requested: 4,
                        limit: 3,
                    },
                ),
            ),
        );
        assert_eq!(strict_loci, strict_loci_before);
        assert!(strict_decompositions.is_empty());
        assert_eq!(strict_stats, strict_stats_before);

        // A wider native envelope never weakens the retained exact-algebra
        // ceiling.  These two sparse factors have a four-term envelope and a
        // four-term canonical product, so Symbolica completes multiplication
        // and postvalidation rejects the result transactionally.
        let mut retained_failure_loci = Vec::new();
        let mut retained_failure_stats = ParametricSectorCoverageStats::default();
        let retained_failure_left = retain_locus(
            &context,
            &mut retained_failure_loci,
            &one_plus_index,
            &mut retained_failure_stats,
            limits,
        );
        let retained_failure_right = retain_locus(
            &context,
            &mut retained_failure_loci,
            &left,
            &mut retained_failure_stats,
            limits,
        );
        let retained_failure_loci_before = retained_failure_loci.clone();
        let retained_failure_stats_before = retained_failure_stats;
        let mut retained_failure_decompositions = Vec::new();
        assert_eq!(
            retain_product_zero_decomposition(
                &context,
                &mut retained_failure_loci,
                &mut retained_failure_decompositions,
                &[retained_failure_left, retained_failure_right],
                &mut retained_failure_stats,
                limits,
            )
            .unwrap_err(),
            ParametricSectorCoverageError::ParametricCoefficient(
                ParametricCoefficientError::ExactAlgebra(
                    crate::algebra::ExactAlgebraError::ResourceLimit {
                        resource: "authenticated polynomial terms",
                        requested: 4,
                        limit: 3,
                    },
                ),
            ),
        );
        assert_eq!(retained_failure_loci, retained_failure_loci_before);
        assert!(retained_failure_decompositions.is_empty());
        assert_eq!(retained_failure_stats, retained_failure_stats_before);
    }

    #[test]
    fn candidate_atomic_locus_staging_does_not_charge_durable_witness_references() {
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.max_product_zero_factor_references = 0;
        assert_eq!(
            preflight_candidate_atomic_locus_staging(2, 2, limits),
            Ok(()),
            "the representation decision happens after staging, and a factored formula retains no decomposition witness",
        );

        assert_eq!(
            preflight_candidate_atomic_locus_staging(
                2,
                1,
                ParametricSectorCoverageLimits::default()
            ),
            Err(ParametricSectorCoverageError::ResourceLimit {
                resource: "candidate atomic-locus ordinal staging",
                requested: 2,
                limit: 1,
            })
        );
    }

    #[test]
    fn whole_product_support_bound_uses_sparse_and_multivariate_degree_envelopes() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-product-support-math",
            2,
        )
        .unwrap();
        let x = context.index(0).unwrap();
        let y = context.index(1).unwrap();
        let x2 = context.mul(&x, &x).unwrap();
        let x4 = context.mul(&x2, &x2).unwrap();

        let sparse_left = context
            .numerator_condition(&context.add(&context.one(), &x4).unwrap())
            .unwrap();
        let sparse_right = context
            .numerator_condition(
                &context
                    .add(
                        &context.one(),
                        &context.mul(&context.integer(2), &x4).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            whole_product_support_at_cutoff(&[sparse_left, sparse_right], &[0, 1], usize::MAX,)
                .unwrap(),
            ProductSupportAtCutoff::Within(4),
            "four sparse tuples are tighter than the nine-point degree interval",
        );

        let dense_left = context
            .numerator_condition(
                &context
                    .add(&context.add(&context.one(), &x).unwrap(), &x2)
                    .unwrap(),
            )
            .unwrap();
        let dense_right = context
            .numerator_condition(
                &context
                    .add(
                        &context
                            .add(
                                &context.one(),
                                &context.mul(&context.integer(2), &x).unwrap(),
                            )
                            .unwrap(),
                        &context.mul(&context.integer(3), &x2).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            whole_product_support_at_cutoff(&[dense_left, dense_right], &[0, 1], usize::MAX)
                .unwrap(),
            ProductSupportAtCutoff::Within(5),
            "the five-point degree interval is tighter than nine sparse tuples",
        );

        let x5 = context.mul(&x4, &x).unwrap();
        let x6 = context.mul(&x4, &x2).unwrap();
        let shifted_left = context
            .numerator_condition(&context.add(&context.add(&x4, &x5).unwrap(), &x6).unwrap())
            .unwrap();
        let shifted_right = context
            .numerator_condition(
                &context
                    .add(
                        &context
                            .add(&x4, &context.mul(&context.integer(2), &x5).unwrap())
                            .unwrap(),
                        &context.mul(&context.integer(3), &x6).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            whole_product_support_at_cutoff(&[shifted_left, shifted_right], &[0, 1], usize::MAX,)
                .unwrap(),
            ProductSupportAtCutoff::Within(9),
            "whole degrees intentionally dominate every sequential native degree-box preflight",
        );

        let xy = context.mul(&x, &y).unwrap();
        let rectangle_left = context
            .numerator_condition(
                &context
                    .add(
                        &context.add(&context.one(), &x).unwrap(),
                        &context.add(&y, &xy).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let rectangle_right = context
            .numerator_condition(
                &context
                    .add(
                        &context
                            .add(
                                &context.one(),
                                &context.mul(&context.integer(2), &x).unwrap(),
                            )
                            .unwrap(),
                        &context
                            .add(
                                &context.mul(&context.integer(3), &y).unwrap(),
                                &context.mul(&context.integer(4), &xy).unwrap(),
                            )
                            .unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            whole_product_support_at_cutoff(
                &[rectangle_left, rectangle_right],
                &[0, 1],
                usize::MAX,
            )
            .unwrap(),
            ProductSupportAtCutoff::Within(9),
            "the componentwise 3x3 box is tighter than sixteen sparse tuples",
        );
    }

    #[test]
    fn support_cutoff_math_is_exact_at_the_boundary_and_saturates_overflow() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-product-support-cutoff",
            1,
        )
        .unwrap();
        let x = context.index(0).unwrap();
        let x2 = context.mul(&x, &x).unwrap();
        let left = context
            .numerator_condition(&context.add(&context.one(), &x2).unwrap())
            .unwrap();
        let right = context
            .numerator_condition(
                &context
                    .add(
                        &context.one(),
                        &context.mul(&context.integer(2), &x2).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            whole_product_support_at_cutoff(&[left.clone(), right.clone()], &[0, 1], 4).unwrap(),
            ProductSupportAtCutoff::Within(4),
        );
        assert_eq!(
            whole_product_support_at_cutoff(&[left, right], &[0, 1], 3).unwrap(),
            ProductSupportAtCutoff::Exceeds,
        );
        assert_eq!(
            ProductSupportAtCutoff::Within(usize::MAX).checked_add(1, usize::MAX),
            ProductSupportAtCutoff::Exceeds,
        );
        assert_eq!(
            ProductSupportAtCutoff::Within(usize::MAX).checked_mul(2, usize::MAX),
            ProductSupportAtCutoff::Exceeds,
        );
        assert_eq!(
            ProductSupportAtCutoff::Exceeds.minimum(ProductSupportAtCutoff::Within(7)),
            ProductSupportAtCutoff::Within(7),
        );
    }

    #[test]
    fn routing_materializes_at_the_exact_support_cutoff_and_factors_one_below() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-product-routing-cutoff",
            1,
        )
        .unwrap();
        let x = context.index(0).unwrap();
        let x2 = context.mul(&x, &x).unwrap();
        let left = context
            .numerator_condition(&context.add(&context.one(), &x2).unwrap())
            .unwrap();
        let right = context
            .numerator_condition(
                &context
                    .add(
                        &context.one(),
                        &context.mul(&context.integer(2), &x2).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let base_limits = ParametricSectorCoverageLimits::default();
        let mut initial_loci = Vec::new();
        let mut initial_stats = ParametricSectorCoverageStats::default();
        let left_ordinal = retain_locus(
            &context,
            &mut initial_loci,
            &left,
            &mut initial_stats,
            base_limits,
        );
        let right_ordinal = retain_locus(
            &context,
            &mut initial_loci,
            &right,
            &mut initial_stats,
            base_limits,
        );

        let mut exact_limits = base_limits;
        exact_limits.max_materialized_product_zero_support_terms = 4;
        let mut exact_loci = initial_loci.clone();
        let mut exact_decompositions = Vec::new();
        let (exact_routing, exact_stats) = route_candidate_product_zero_loci(
            &context,
            &mut exact_loci,
            &mut exact_decompositions,
            &[left_ordinal, right_ordinal],
            initial_stats,
            exact_limits,
        )
        .unwrap();
        let CandidateProductZeroRouting::ConcreteLocus(product) = exact_routing else {
            panic!("the exact cutoff must select a concrete product")
        };
        assert_eq!(exact_loci[product].term_count(), 3);
        assert_eq!(exact_decompositions.len(), 1);
        assert_eq!(exact_stats.product_materialization_bound_factor_scans(), 2);
        assert_eq!(
            exact_stats.product_materialization_bound_exponent_entries(),
            4
        );
        assert_eq!(exact_stats.factored_product_zero_disjunctions(), 0);

        let mut below_limits = base_limits;
        below_limits.max_materialized_product_zero_support_terms = 3;
        let mut below_loci = initial_loci.clone();
        let mut below_decompositions = Vec::new();
        let (below_routing, below_stats) = route_candidate_product_zero_loci(
            &context,
            &mut below_loci,
            &mut below_decompositions,
            &[left_ordinal, right_ordinal],
            initial_stats,
            below_limits,
        )
        .unwrap();
        assert_eq!(
            below_routing,
            CandidateProductZeroRouting::Factored(vec![left_ordinal, right_ordinal]),
        );
        assert_eq!(below_loci, initial_loci);
        assert!(below_decompositions.is_empty());
        assert_eq!(below_stats.factored_product_zero_disjunctions(), 1);
        assert_eq!(below_stats.factored_product_zero_factor_references(), 2);
        assert_eq!(
            below_stats.product_zero_multiplications(),
            initial_stats.product_zero_multiplications(),
        );
        assert_eq!(
            below_stats.product_reconstruction_term_pairs(),
            initial_stats.product_reconstruction_term_pairs(),
        );
    }

    #[test]
    fn factored_routing_is_canonical_and_leaves_product_counters_untouched() {
        let (context, mut loci, mut stats, [first, second, base_unit]) = seeded_loci();
        let loci_before = loci.clone();
        let stats_before = stats;
        let mut decompositions = Vec::new();
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.max_materialized_product_zero_support_terms = 0;
        limits.max_product_zero_decompositions = 0;
        limits.max_product_zero_factor_references = 0;
        limits.max_product_zero_multiplications = 0;
        limits.max_product_reconstruction_term_pairs = 0;

        let (routing, routed_stats) = route_candidate_product_zero_loci(
            &context,
            &mut loci,
            &mut decompositions,
            &[second, first, first, base_unit],
            stats,
            limits,
        )
        .unwrap();
        assert_eq!(
            routing,
            CandidateProductZeroRouting::Factored(vec![first, second]),
        );
        stats = routed_stats;
        assert_eq!(loci, loci_before);
        assert!(decompositions.is_empty());
        assert_eq!(
            stats.product_zero_decompositions(),
            stats_before.product_zero_decompositions()
        );
        assert_eq!(
            stats.product_zero_factor_references(),
            stats_before.product_zero_factor_references()
        );
        assert_eq!(
            stats.product_zero_multiplications(),
            stats_before.product_zero_multiplications()
        );
        assert_eq!(
            stats.product_reconstruction_term_pairs(),
            stats_before.product_reconstruction_term_pairs()
        );
        assert_eq!(stats.product_materialization_bound_factor_scans(), 2);
        assert_eq!(
            stats.product_materialization_bound_exponent_entries(),
            loci[first].raw().exponents.len() + loci[second].raw().exponents.len(),
        );
        assert_eq!(stats.factored_product_zero_disjunctions(), 1);
        assert_eq!(stats.factored_product_zero_factor_references(), 2);

        stats.unique_predicates = loci.len();
        preflight_product_zero_payload(&context, &loci, &decompositions, stats, limits).unwrap();
    }

    #[test]
    fn new_representation_budgets_fail_transactionally() {
        for resource in [
            "sector-coverage product materialization-bound factor scans",
            "sector-coverage product materialization-bound exponent entries",
            "sector-coverage factored product-zero disjunctions",
            "sector-coverage factored product-zero factor references",
        ] {
            let (context, mut loci, stats, [first, second, _]) = seeded_loci();
            let loci_before = loci.clone();
            let stats_before = stats;
            let mut decompositions = Vec::new();
            let mut limits = ParametricSectorCoverageLimits::default();
            limits.max_materialized_product_zero_support_terms = 0;
            match resource {
                "sector-coverage product materialization-bound factor scans" => {
                    limits.max_product_materialization_bound_factor_scans = 1;
                }
                "sector-coverage product materialization-bound exponent entries" => {
                    limits.max_product_materialization_bound_exponent_entries =
                        loci[first].raw().exponents.len() + loci[second].raw().exponents.len() - 1;
                }
                "sector-coverage factored product-zero disjunctions" => {
                    limits.max_factored_product_zero_disjunctions = 0;
                }
                "sector-coverage factored product-zero factor references" => {
                    limits.max_factored_product_zero_factor_references = 1;
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                route_candidate_product_zero_loci(
                    &context,
                    &mut loci,
                    &mut decompositions,
                    &[first, second],
                    stats,
                    limits,
                ),
                Err(ParametricSectorCoverageError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(loci, loci_before);
            assert!(decompositions.is_empty());
            assert_eq!(stats, stats_before);
        }
    }

    #[test]
    fn reconstruction_and_associate_budgets_are_aggregate_and_transactional() {
        for resource in [
            "sector-coverage product reconstruction term pairs",
            "sector-coverage product reconstruction output terms",
            "sector-coverage product reconstruction output exponent entries",
            "sector-coverage product reconstruction output coefficient bits",
            "sector-coverage structural locus associate comparisons",
            "sector-coverage structural locus associate term pairs",
        ] {
            let (context, mut loci, mut stats, [first, second, _]) = seeded_loci();
            let loci_before = loci.clone();
            let stats_before = stats;
            let mut decompositions = Vec::new();
            let mut limits = ParametricSectorCoverageLimits::default();
            match resource {
                "sector-coverage product reconstruction term pairs" => {
                    limits.max_product_reconstruction_term_pairs = 0;
                }
                "sector-coverage product reconstruction output terms" => {
                    limits.max_product_reconstruction_output_terms = 0;
                }
                "sector-coverage product reconstruction output exponent entries" => {
                    limits.max_product_reconstruction_output_exponent_entries = 0;
                }
                "sector-coverage product reconstruction output coefficient bits" => {
                    limits.max_product_reconstruction_output_coefficient_bits = 0;
                }
                "sector-coverage structural locus associate comparisons" => {
                    limits.max_structural_locus_associate_comparisons =
                        stats.structural_locus_associate_comparisons();
                }
                "sector-coverage structural locus associate term pairs" => {
                    limits.max_structural_locus_associate_term_pairs =
                        stats.structural_locus_associate_term_pairs();
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                retain_product_zero_decomposition(
                    &context,
                    &mut loci,
                    &mut decompositions,
                    &[first, second],
                    &mut stats,
                    limits,
                ),
                Err(ParametricSectorCoverageError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(loci, loci_before);
            assert!(decompositions.is_empty());
            assert_eq!(stats, stats_before);
        }

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-v4-cumulative-product-work",
            3,
        )
        .unwrap();
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.max_product_reconstruction_term_pairs = 1;
        let mut loci = Vec::new();
        let mut stats = ParametricSectorCoverageStats::default();
        let ordinals = (0..3)
            .map(|index| {
                let factor = context
                    .numerator_condition(&context.index(index).unwrap())
                    .unwrap();
                retain_locus(&context, &mut loci, &factor, &mut stats, limits)
            })
            .collect::<Vec<_>>();
        let loci_before = loci.clone();
        let stats_before = stats;
        let mut decompositions = Vec::new();
        assert!(matches!(
            retain_product_zero_decomposition(
                &context,
                &mut loci,
                &mut decompositions,
                &ordinals,
                &mut stats,
                limits,
            ),
            Err(ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage product reconstruction term pairs",
                requested: 2,
                limit: 1,
            })
        ));
        assert_eq!(loci, loci_before);
        assert!(decompositions.is_empty());
        assert_eq!(stats, stats_before);
    }

    fn sunset_family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            "coverage-v4-sunset",
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

    fn sunset_coverage() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ParametricSectorCoverageCertificate,
    ) {
        sunset_coverage_with_product_cutoff(
            ParametricSectorCoverageLimits::default().max_materialized_product_zero_support_terms,
        )
    }

    fn sunset_coverage_with_product_cutoff(
        cutoff: usize,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ParametricSectorCoverageCertificate,
    ) {
        let family = sunset_family();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut limits = GeneratedSectorDiscoveryLimits::default();
        limits.adaptive.max_search_depth = 0;
        limits.coverage.max_materialized_product_zero_support_terms = cutoff;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            limits,
        )
        .unwrap();
        (family, context, discovery.coverage().clone())
    }

    #[test]
    fn coverage_factor_only_route_retains_no_product_witness_or_work_and_replays_cutoff() {
        let (family, context, coverage) = sunset_coverage_with_product_cutoff(0);
        assert!(coverage.product_zero_decompositions().is_empty());
        assert!(coverage.stats().factored_product_zero_disjunctions() > 0);
        assert!(coverage.stats().factored_product_zero_factor_references() > 0);
        assert_eq!(coverage.stats().product_zero_decompositions(), 0);
        assert_eq!(coverage.stats().product_zero_factor_references(), 0);
        assert_eq!(coverage.stats().product_zero_multiplications(), 0);
        assert_eq!(coverage.stats().product_reconstruction_term_pairs(), 0);
        assert_eq!(coverage.stats().product_reconstruction_output_terms(), 0);
        coverage.replay(&family, &context).unwrap();

        let mut cutoff_tamper = coverage.clone();
        cutoff_tamper
            .limits
            .max_materialized_product_zero_support_terms = usize::MAX;
        assert_eq!(
            cutoff_tamper.replay(&family, &context),
            Err(ParametricSectorCoverageError::ReplayMismatch),
        );

        let mut stats_tamper = coverage.clone();
        stats_tamper
            .stats
            .product_materialization_bound_factor_scans += 1;
        assert_eq!(
            stats_tamper.replay(&family, &context),
            Err(ParametricSectorCoverageError::ReplayMismatch),
        );
    }

    #[test]
    fn generated_product_compression_exposes_exact_factors_and_replay_rejects_tampering() {
        let (family, context, coverage) = sunset_coverage();
        assert_eq!(coverage.schema(), PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA);
        assert!(!coverage.product_zero_decompositions().is_empty());
        assert_eq!(
            coverage.stats().retained_structural_locus_terms(),
            coverage
                .structural_loci()
                .iter()
                .map(ParametricPolynomial::term_count)
                .sum::<usize>()
        );
        assert_eq!(
            coverage.stats().product_zero_decompositions(),
            coverage.product_zero_decompositions().len()
        );
        assert_eq!(
            coverage.stats().product_zero_factor_references(),
            coverage
                .product_zero_decompositions()
                .iter()
                .map(|witness| witness.factor_locus_ordinals().len())
                .sum::<usize>()
        );

        for witness in coverage.product_zero_decompositions() {
            assert!(witness.factor_locus_ordinals().len() >= 2);
            assert!(
                witness
                    .factor_locus_ordinals()
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            );
            assert!(witness.factor_locus_ordinals().iter().all(|&ordinal| {
                context
                    .polynomial_depends_on_indices_with_limits(
                        coverage.structural_locus(ordinal).unwrap(),
                        coverage_exact_algebra(ParametricSectorCoverageLimits::default()),
                    )
                    .unwrap()
            }));
            let product = coverage
                .structural_locus(witness.product_locus_ordinal())
                .unwrap();
            assert_eq!(
                coverage
                    .canonical_product_zero_decomposition_for_exact_predicate(product)
                    .unwrap()
                    .product_locus_ordinal(),
                witness.product_locus_ordinal()
            );
        }

        let mut resolved_partition_products = 0usize;
        for case in coverage.partition().cases() {
            for predicate in case.predicates() {
                if let Some(witness) = coverage
                    .canonical_product_zero_decomposition_for_exact_predicate(
                        predicate.polynomial(),
                    )
                {
                    assert_eq!(
                        coverage
                            .structural_locus(witness.product_locus_ordinal())
                            .unwrap(),
                        predicate.polynomial()
                    );
                    resolved_partition_products += 1;
                }
            }
        }
        assert!(resolved_partition_products > 0);
        coverage.replay(&family, &context).unwrap();

        let mut bad_factor = coverage.clone();
        bad_factor.product_zero_decompositions[0].factor_locus_ordinals[0] = usize::MAX;
        assert!(matches!(
            bad_factor.replay(&family, &context),
            Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch)
        ));

        let mut bad_singleton = coverage.clone();
        let singleton = bad_singleton.product_zero_decompositions[0].factor_locus_ordinals[0];
        bad_singleton.product_zero_decompositions[0].factor_locus_ordinals =
            vec![singleton].into_boxed_slice();
        assert!(matches!(
            bad_singleton.replay(&family, &context),
            Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch)
        ));

        let mut bad_cardinality = coverage.clone();
        let mut witnesses = bad_cardinality.product_zero_decompositions.to_vec();
        witnesses.push(witnesses[0].clone());
        witnesses.sort_by(decomposition_cmp);
        bad_cardinality.product_zero_decompositions = witnesses.into_boxed_slice();
        assert!(matches!(
            bad_cardinality.replay(&family, &context),
            Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch)
        ));

        let mut bad_stats = coverage.clone();
        bad_stats.stats.product_zero_factor_references += 1;
        assert!(matches!(
            bad_stats.replay(&family, &context),
            Err(ParametricSectorCoverageError::ProductZeroCensusMismatch)
        ));

        let mut bad_formula_stats = coverage.clone();
        bad_formula_stats.stats.direct_bad_formula_evaluations += 1;
        assert!(matches!(
            bad_formula_stats.replay(&family, &context),
            Err(ParametricSectorCoverageError::ReplayMismatch)
        ));

        let mut bad_formula_query_stats = coverage.clone();
        bad_formula_query_stats
            .stats
            .direct_bad_formula_atom_queries += 1;
        assert!(matches!(
            bad_formula_query_stats.replay(&family, &context),
            Err(ParametricSectorCoverageError::ReplayMismatch)
        ));

        let mut bad_table = coverage.clone();
        let product = bad_table.product_zero_decompositions[0].product_locus_ordinal;
        let factor = bad_table.product_zero_decompositions[0].factor_locus_ordinals[0];
        bad_table.structural_loci[product] = bad_table.structural_loci[factor].clone();
        assert!(matches!(
            bad_table.replay(&family, &context),
            Err(ParametricSectorCoverageError::ProductZeroCensusMismatch)
                | Err(ParametricSectorCoverageError::ProductZeroDecompositionMismatch)
                | Err(ParametricSectorCoverageError::ReplayMismatch)
        ));
    }

    #[test]
    fn sorted_product_witness_lookup_covers_first_middle_absent_and_ties() {
        let (_, _, mut coverage) = sunset_coverage();
        coverage.product_zero_decompositions = vec![
            ParametricSectorProductZeroDecomposition {
                product_locus_ordinal: 2,
                factor_locus_ordinals: vec![0, 1].into_boxed_slice(),
            },
            ParametricSectorProductZeroDecomposition {
                product_locus_ordinal: 5,
                factor_locus_ordinals: vec![0, 2].into_boxed_slice(),
            },
            ParametricSectorProductZeroDecomposition {
                product_locus_ordinal: 5,
                factor_locus_ordinals: vec![1, 2].into_boxed_slice(),
            },
            ParametricSectorProductZeroDecomposition {
                product_locus_ordinal: 9,
                factor_locus_ordinals: vec![0, 3].into_boxed_slice(),
            },
        ]
        .into_boxed_slice();

        assert_eq!(
            coverage
                .canonical_product_zero_decomposition_for_locus(2)
                .unwrap()
                .factor_locus_ordinals(),
            [0, 1]
        );
        assert_eq!(
            coverage
                .canonical_product_zero_decomposition_for_locus(5)
                .unwrap()
                .factor_locus_ordinals(),
            [0, 2],
            "the lower bound is the lexicographically first tied witness"
        );
        assert_eq!(coverage.product_zero_decompositions_for_locus(5).count(), 2);
        assert!(
            coverage
                .canonical_product_zero_decomposition_for_locus(4)
                .is_none()
        );
        assert!(
            coverage
                .canonical_product_zero_decomposition_for_locus(10)
                .is_none()
        );
        assert_eq!(
            coverage
                .canonical_product_zero_decomposition_for_locus(9)
                .unwrap()
                .factor_locus_ordinals(),
            [0, 3]
        );
    }
}

#[cfg(test)]
mod divisibility_implication_tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    fn product_loci() -> (ParametricCoefficientContext, Vec<ParametricPolynomial>) {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-divisibility-directions",
            2,
        )
        .unwrap();
        let p = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let product = context
            .mul(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let q = context.numerator_condition(&product).unwrap();
        (context, vec![p, q])
    }

    fn independent_loci() -> (ParametricCoefficientContext, Vec<ParametricPolynomial>) {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-formula-truth-table",
            2,
        )
        .unwrap();
        let p = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let q = context
            .numerator_condition(&context.index(1).unwrap())
            .unwrap();
        (context, vec![p, q])
    }

    fn production_bad_formula_loci() -> (ParametricCoefficientContext, Vec<ParametricPolynomial>) {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "coverage-production-bad-formula",
            3,
        )
        .unwrap();
        let product = context
            .mul(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let product = context.numerator_condition(&product).unwrap();
        let boundary = context
            .numerator_condition(&context.index(2).unwrap())
            .unwrap();
        let gate = context
            .numerator_condition(&context.index(1).unwrap())
            .unwrap();
        (context, vec![product, boundary, gate])
    }

    fn evaluate_assigned_formula(
        context: &ParametricCoefficientContext,
        polynomials: &[ParametricPolynomial],
        formula: &CandidateBadFormula,
        decisions: &[(usize, SymbolicPolynomialPredicateKind)],
    ) -> CandidateFormulaEvaluation {
        let mut state = GlobalCaseState::new(context.index_count());
        for (ordinal, &(locus, kind)) in decisions.iter().enumerate() {
            state.locus_decisions.insert(locus, kind);
            state.decision_predicate_ordinals.insert(locus, ordinal);
        }
        evaluate_candidate_bad_formula(
            context,
            formula,
            &state,
            polynomials,
            &mut BTreeMap::new(),
            &mut ParametricSectorCoverageStats::default(),
            ParametricSectorCoverageLimits::default(),
        )
        .unwrap()
    }

    fn implication(
        known_locus: usize,
        known_kind: SymbolicPolynomialPredicateKind,
        requested_locus: usize,
    ) -> Option<(SymbolicPolynomialPredicateKind, usize)> {
        let (context, polynomials) = product_loci();
        let mut state = GlobalCaseState::new(2);
        state.locus_decisions.insert(known_locus, known_kind);
        state.decision_predicate_ordinals.insert(known_locus, 0);
        implied_locus_decision(
            &context,
            requested_locus,
            &state,
            &polynomials,
            &mut BTreeMap::new(),
            &mut ParametricSectorCoverageStats::default(),
            ParametricSectorCoverageLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn factored_candidate_bad_formula_is_truth_equivalent_to_materialized_product_atom() {
        let (context, mut polynomials) = independent_loci();
        let product = context
            .multiply_polynomial_conditions_with_limits(
                &polynomials[0],
                &polynomials[1],
                crate::algebra::ExactAlgebraLimits::default(),
            )
            .unwrap();
        polynomials.push(product);
        let materialized = CandidateBadFormula {
            clauses: vec![CandidateBadClause::Atom(CandidateBadAtom {
                locus: 2,
                kind: SymbolicPolynomialPredicateKind::EqualZero,
            })]
            .into_boxed_slice(),
            atom_count: 1,
        };
        let factored = CandidateBadFormula {
            clauses: vec![
                CandidateBadClause::Atom(CandidateBadAtom {
                    locus: 0,
                    kind: SymbolicPolynomialPredicateKind::EqualZero,
                }),
                CandidateBadClause::Atom(CandidateBadAtom {
                    locus: 1,
                    kind: SymbolicPolynomialPredicateKind::EqualZero,
                }),
            ]
            .into_boxed_slice(),
            atom_count: 2,
        };

        for (left, right) in [
            (
                SymbolicPolynomialPredicateKind::EqualZero,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            (
                SymbolicPolynomialPredicateKind::EqualZero,
                SymbolicPolynomialPredicateKind::NonZero,
            ),
            (
                SymbolicPolynomialPredicateKind::NonZero,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            (
                SymbolicPolynomialPredicateKind::NonZero,
                SymbolicPolynomialPredicateKind::NonZero,
            ),
        ] {
            let product_kind = if left == SymbolicPolynomialPredicateKind::EqualZero
                || right == SymbolicPolynomialPredicateKind::EqualZero
            {
                SymbolicPolynomialPredicateKind::EqualZero
            } else {
                SymbolicPolynomialPredicateKind::NonZero
            };
            assert_eq!(
                evaluate_assigned_formula(
                    &context,
                    &polynomials,
                    &materialized,
                    &[(2, product_kind)],
                ),
                evaluate_assigned_formula(
                    &context,
                    &polynomials,
                    &factored,
                    &[(0, left), (1, right)],
                ),
            );
        }
    }

    #[test]
    fn exact_divisibility_uses_only_valid_integral_domain_directions() {
        // p | q: p=0 => q=0.
        assert_eq!(
            implication(0, SymbolicPolynomialPredicateKind::EqualZero, 1),
            Some((SymbolicPolynomialPredicateKind::EqualZero, 0))
        );
        // p | q: q!=0 => p!=0.
        assert_eq!(
            implication(1, SymbolicPolynomialPredicateKind::NonZero, 0),
            Some((SymbolicPolynomialPredicateKind::NonZero, 1))
        );

        // Invalid converse: q=0 does not imply p=0 (n1 may vanish).
        assert_eq!(
            implication(1, SymbolicPolynomialPredicateKind::EqualZero, 0),
            None
        );
        // Invalid converse: p!=0 does not imply q!=0 (n1 may vanish).
        assert_eq!(
            implication(0, SymbolicPolynomialPredicateKind::NonZero, 1),
            None
        );
    }

    #[test]
    fn divisibility_checks_fail_closed_before_symbolica_division() {
        let (context, polynomials) = product_loci();
        let mut state = GlobalCaseState::new(2);
        state
            .locus_decisions
            .insert(0, SymbolicPolynomialPredicateKind::EqualZero);
        state.decision_predicate_ordinals.insert(0, 0);
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.max_locus_divisibility_checks = 0;
        assert!(matches!(
            implied_locus_decision(
                &context,
                1,
                &state,
                &polynomials,
                &mut BTreeMap::new(),
                &mut ParametricSectorCoverageStats::default(),
                limits,
            ),
            Err(ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage locus divisibility checks",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn product_zero_locus_is_exactly_the_disjunction_of_factor_zero_loci() {
        let (context, polynomials) = independent_loci();
        let product = context
            .multiply_polynomial_conditions_with_limits(
                &polynomials[0],
                &polynomials[1],
                crate::algebra::ExactAlgebraLimits::default(),
            )
            .unwrap();
        for indices in [[0, 0], [0, 7], [-3, 0], [-3, 7]] {
            let specialized = context
                .specialize_polynomial(
                    &product,
                    &indices,
                    crate::ParametricArithmeticLimits::default(),
                )
                .unwrap();
            assert_eq!(specialized.is_zero(), indices[0] == 0 || indices[1] == 0);
        }

        let mut limits = crate::algebra::ExactAlgebraLimits::default();
        limits.max_term_operations = 0;
        assert!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &polynomials[0],
                    &polynomials[1],
                    limits,
                )
                .is_err(),
            "product compression must fail closed before unbudgeted Symbolica multiplication",
        );
    }

    #[test]
    fn conjunction_bad_clause_matches_its_full_boolean_truth_table() {
        let (context, polynomials) = independent_loci();
        let formula = CandidateBadFormula {
            clauses: vec![CandidateBadClause::Conjunction(
                CandidateBadAtom {
                    locus: 0,
                    kind: SymbolicPolynomialPredicateKind::EqualZero,
                },
                CandidateBadAtom {
                    locus: 1,
                    kind: SymbolicPolynomialPredicateKind::NonZero,
                },
            )]
            .into_boxed_slice(),
            atom_count: 2,
        };
        for (p_kind, q_kind, expected_bad) in [
            (
                SymbolicPolynomialPredicateKind::EqualZero,
                SymbolicPolynomialPredicateKind::EqualZero,
                false,
            ),
            (
                SymbolicPolynomialPredicateKind::EqualZero,
                SymbolicPolynomialPredicateKind::NonZero,
                true,
            ),
            (
                SymbolicPolynomialPredicateKind::NonZero,
                SymbolicPolynomialPredicateKind::EqualZero,
                false,
            ),
            (
                SymbolicPolynomialPredicateKind::NonZero,
                SymbolicPolynomialPredicateKind::NonZero,
                false,
            ),
        ] {
            let actual = evaluate_assigned_formula(
                &context,
                &polynomials,
                &formula,
                &[(0, p_kind), (1, q_kind)],
            );
            assert_eq!(
                actual,
                if expected_bad {
                    CandidateFormulaEvaluation::Bad
                } else {
                    CandidateFormulaEvaluation::Covered
                }
            );
        }
    }

    #[test]
    fn true_boundary_gate_clause_dominates_an_earlier_unknown_product_atom() {
        let (context, polynomials) = production_bad_formula_loci();
        let formula = CandidateBadFormula {
            clauses: vec![
                CandidateBadClause::Atom(CandidateBadAtom {
                    locus: 0,
                    kind: SymbolicPolynomialPredicateKind::EqualZero,
                }),
                CandidateBadClause::Conjunction(
                    CandidateBadAtom {
                        locus: 1,
                        kind: SymbolicPolynomialPredicateKind::EqualZero,
                    },
                    CandidateBadAtom {
                        locus: 2,
                        kind: SymbolicPolynomialPredicateKind::NonZero,
                    },
                ),
            ]
            .into_boxed_slice(),
            atom_count: 3,
        };
        assert_eq!(
            evaluate_assigned_formula(
                &context,
                &polynomials,
                &formula,
                &[
                    (1, SymbolicPolynomialPredicateKind::EqualZero),
                    (2, SymbolicPolynomialPredicateKind::NonZero),
                ],
            ),
            CandidateFormulaEvaluation::Bad,
        );
    }

    fn run_genuine_conjunction_split_chain(
        limits: ParametricSectorCoverageLimits,
        stats: &mut ParametricSectorCoverageStats,
    ) -> Result<(), ParametricSectorCoverageError> {
        let (context, polynomials) = independent_loci();
        let formula = CandidateBadFormula {
            clauses: vec![CandidateBadClause::Conjunction(
                CandidateBadAtom {
                    locus: 0,
                    kind: SymbolicPolynomialPredicateKind::EqualZero,
                },
                CandidateBadAtom {
                    locus: 1,
                    kind: SymbolicPolynomialPredicateKind::NonZero,
                },
            )]
            .into_boxed_slice(),
            atom_count: 2,
        };
        let sector = SectorMask::try_new([false, false]).unwrap();
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            &context,
            sector.clone(),
            effective_sector_limits(limits),
        )?;
        let root = builder.root_case();
        let coordinate_loci = vec![Some((0, 0)), Some((1, 0))];
        let mut divisibility_cache = BTreeMap::new();
        let mut open = BTreeMap::new();
        let mut covered = BTreeMap::new();
        let mut coordinate_empty = BTreeMap::new();
        overlay_candidate_bad_formula(
            &mut builder,
            &context,
            &sector,
            root,
            GlobalCaseState::new(context.index_count()),
            0,
            &formula,
            &polynomials,
            &coordinate_loci,
            &mut divisibility_cache,
            &mut open,
            &mut covered,
            &mut coordinate_empty,
            stats,
            limits,
        )?;
        assert!(
            coordinate_empty.is_empty(),
            "zero and nonzero branches at n0=0 and n1=0 are all valid in the inactive orthant",
        );
        builder.finish(&context)?;
        Ok(())
    }

    #[test]
    fn genuine_split_chain_has_exact_transactional_direct_formula_budgets() {
        let mut exact = ParametricSectorCoverageLimits::default();
        exact.max_direct_bad_formula_evaluations = 5;
        exact.max_direct_bad_formula_atom_queries = 10;
        let mut exact_stats = ParametricSectorCoverageStats::default();
        run_genuine_conjunction_split_chain(exact, &mut exact_stats).unwrap();
        assert_eq!(exact_stats.direct_bad_formula_evaluations(), 5);
        assert_eq!(exact_stats.direct_bad_formula_atom_queries(), 10);

        let mut one_below_evaluations = exact;
        one_below_evaluations.max_direct_bad_formula_evaluations = 4;
        let mut evaluation_stats = ParametricSectorCoverageStats::default();
        assert!(matches!(
            run_genuine_conjunction_split_chain(one_below_evaluations, &mut evaluation_stats),
            Err(ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage direct bad-formula evaluations",
                requested: 5,
                limit: 4,
            })
        ));
        assert_eq!(evaluation_stats.direct_bad_formula_evaluations(), 4);
        assert_eq!(evaluation_stats.direct_bad_formula_atom_queries(), 8);

        let mut one_below_queries = exact;
        one_below_queries.max_direct_bad_formula_atom_queries = 9;
        let mut query_stats = ParametricSectorCoverageStats::default();
        assert!(matches!(
            run_genuine_conjunction_split_chain(one_below_queries, &mut query_stats),
            Err(ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage direct bad-formula atom queries",
                requested: 10,
                limit: 9,
            })
        ));
        assert_eq!(query_stats.direct_bad_formula_evaluations(), 4);
        assert_eq!(query_stats.direct_bad_formula_atom_queries(), 8);
    }
}

#[cfg(test)]
mod authenticated_row_span_seam_tests {
    use super::*;
    use crate::{
        AffineDenominator, IntegralOrderingPolicy, ParametricElimination,
        ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
        ParametricRelation, ParametricRuleLimits, algebra::CoefficientContext,
    };

    fn family(name: &str) -> IntegralFamily {
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

    fn candidate(
        context: &ParametricCoefficientContext,
        rows: &[ParametricRelation],
        sector: SectorMask,
    ) -> ParametricReductionRuleCandidate {
        let elimination = ParametricElimination::build(
            context,
            rows,
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [2])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        ParametricReductionRuleCandidate::try_from_elimination_pivot(
            context,
            rows,
            &elimination,
            0,
            sector,
            ParametricRuleLimits::default(),
        )
        .unwrap()
    }

    fn row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        limits: ParametricSectorCoverageLimits,
    ) -> Arc<GeneratedSymbolicRowSpanCertificate> {
        Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                limits.generated_when_bad.ibp,
                limits.generated_when_bad.row_span,
            )
            .unwrap(),
        )
    }

    #[test]
    fn seam_normalizes_payload_equal_compilation_onto_exact_supplied_arc() {
        let family = family("coverage-seam-equal-row-span");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let sector = SectorMask::try_new([true]).unwrap();
        let limits = ParametricSectorCoverageLimits::default();
        let source = row_span(&family, &context, limits);
        let candidate = candidate(&context, source.rows(), sector.clone());
        let compilation = GeneratedWhenBadCompiler::compile_with_row_span(
            &family,
            &context,
            &candidate,
            source.clone(),
            limits.generated_when_bad,
        )
        .unwrap();
        let supplied = row_span(&family, &context, limits);
        assert!(!Arc::ptr_eq(&source, &supplied));
        assert!(source.payload_eq(&supplied));
        supplied.replay(&family, &context).unwrap();

        let coverage =
            ParametricSectorCoverageCompiler::compose_authenticated_with_replayed_row_span(
                &family,
                &context,
                sector,
                vec![compilation],
                supplied.clone(),
                limits,
            )
            .unwrap();

        assert!(Arc::ptr_eq(coverage.row_span_arc(), &supplied));
        assert!(
            coverage
                .candidate_attempts()
                .iter()
                .all(|attempt| Arc::ptr_eq(
                    attempt.compilation().source_authentication().row_span_arc(),
                    &supplied,
                ))
        );
        coverage
            .replay_with_row_span(&family, &context, supplied)
            .unwrap();
    }

    #[test]
    fn seam_rejects_compilation_authenticated_under_different_row_span_payload() {
        let family = family("coverage-seam-different-row-span");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let sector = SectorMask::try_new([true]).unwrap();
        let target_limits = ParametricSectorCoverageLimits::default();
        let mut source_limits = target_limits;
        source_limits
            .generated_when_bad
            .row_span
            .limits
            .max_augmented_rows += 1;
        let source = row_span(&family, &context, source_limits);
        let candidate = candidate(&context, source.rows(), sector.clone());
        let compilation = GeneratedWhenBadCompiler::compile_with_row_span(
            &family,
            &context,
            &candidate,
            source,
            source_limits.generated_when_bad,
        )
        .unwrap();
        let supplied = row_span(&family, &context, target_limits);
        supplied.replay(&family, &context).unwrap();

        assert!(matches!(
            ParametricSectorCoverageCompiler::compose_authenticated_with_replayed_row_span(
                &family,
                &context,
                sector,
                vec![compilation],
                supplied,
                target_limits,
            ),
            Err(ParametricSectorCoverageError::SharedRowSpanCertificateMismatch)
        ));
    }

    #[test]
    fn empty_batch_still_rejects_supplied_row_span_config_mismatch() {
        let family = family("coverage-seam-empty-config-mismatch");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let target_limits = ParametricSectorCoverageLimits::default();
        let mut source_limits = target_limits;
        source_limits
            .generated_when_bad
            .row_span
            .limits
            .max_augmented_rows += 1;
        let source = row_span(&family, &context, source_limits);

        assert!(matches!(
            ParametricSectorCoverageCompiler::compile_with_row_span(
                &family,
                &context,
                SectorMask::try_new([true]).unwrap(),
                &[],
                source,
                target_limits,
            ),
            Err(ParametricSectorCoverageError::GeneratedWhenBad(
                GeneratedWhenBadError::SharedRowSpanConfigMismatch
            ))
        ));
    }
}
