//! Target-relative structural case splitting for generated affine `WhenBad`.
//!
//! This is the relation-free stage of the generated residual-affine
//! `WhenBad` compiler.  It owns only a canonical table of already
//! authenticated structural loci, inherited truth facts, and a direct bad
//! formula supplied by the future matcher-bound outer compiler.  The root is
//! therefore deliberately abstract here: the future outer certificate binds
//! it to one exact residual-affine inventory case.
//!
//! There is no public builder and no entry point accepting a relation or a
//! reduction-rule candidate.  Public types expose only the replayable
//! structural transcript that the authenticated outer compiler will embed.

use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;

use crate::direct_bad_formula::{
    DirectBadFormulaClause, DirectBadFormulaRoute, DirectBadFormulaTruth, route_direct_bad_formula,
};
use crate::{
    ExactAlgebraLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial, SymbolicPolynomialPredicateKind,
};

/// Stable schema for the target-relative structural partition core.
pub const AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA: &str =
    "rustred-affine-when-bad-relative-partition-v1";

/// Checked work and retained-transcript budgets for one target-relative root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativeCaseLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_structural_loci: usize,
    pub max_structural_locus_equality_comparisons: usize,
    pub max_structural_locus_associate_comparisons: usize,
    pub max_structural_locus_associate_term_pairs: usize,
    pub max_inherited_truths: usize,
    pub max_bad_clauses: usize,
    pub max_bad_atoms: usize,
    pub max_direct_bad_formula_evaluations: usize,
    pub max_direct_bad_formula_clause_visits: usize,
    pub max_direct_bad_formula_atom_truth_queries: usize,
    pub max_splits: usize,
    pub max_live_leaves: usize,
    pub max_case_ids: usize,
    pub max_predicates_per_case: usize,
    pub max_predicate_instances: usize,
    pub max_leaf_classifications: usize,
    /// `live_leaves * structural_loci` cells in the allocation-fallible
    /// internal truth-state table.
    pub max_work_decision_cells: usize,
    pub max_locus_divisibility_checks: usize,
    pub max_locus_divisibility_term_pairs: usize,
    pub max_locus_divisibility_cache_entries: usize,
    pub max_retained_polynomial_terms: usize,
    pub max_retained_polynomial_exponent_entries: usize,
    pub max_retained_polynomial_integer_bits: usize,
    pub max_retained_polynomial_display_bytes: usize,
    pub max_retained_bytes: usize,
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
}

impl Default for AffineWhenBadRelativeCaseLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_structural_loci: 4_000_000,
            max_structural_locus_equality_comparisons: 32_000_000,
            max_structural_locus_associate_comparisons: 32_000_000,
            max_structural_locus_associate_term_pairs: 512_000_000,
            max_inherited_truths: 4_000_000,
            max_bad_clauses: 16_000_000,
            max_bad_atoms: 32_000_000,
            max_direct_bad_formula_evaluations: 16_000_000,
            max_direct_bad_formula_clause_visits: 512_000_000,
            max_direct_bad_formula_atom_truth_queries: 512_000_000,
            max_splits: 4_000_000,
            max_live_leaves: 4_000_001,
            max_case_ids: 8_000_001,
            max_predicates_per_case: 4096,
            max_predicate_instances: 32_000_000,
            max_leaf_classifications: 4_000_001,
            max_work_decision_cells: usize::try_from(16_000_000_000_u64).unwrap_or(usize::MAX),
            max_locus_divisibility_checks: 32_000_000,
            max_locus_divisibility_term_pairs: 512_000_000,
            max_locus_divisibility_cache_entries: 32_000_000,
            max_retained_polynomial_terms: 64_000_000,
            max_retained_polynomial_exponent_entries: 1_000_000_000,
            max_retained_polynomial_integer_bits: 4_000_000_000,
            max_retained_polynomial_display_bytes: 2 * 1024 * 1024 * 1024,
            max_retained_bytes: usize::try_from(4_294_967_296_u64).unwrap_or(usize::MAX),
            max_payload_comparison_units: 1_000_000_000,
            max_payload_comparison_bytes: 2 * 1024 * 1024 * 1024,
            max_payload_comparison_integer_bits: 4_000_000_000,
        }
    }
}

/// Stable identifier allocated monotonically in split-transcript order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AffineWhenBadRelativeCaseId(u64);

impl AffineWhenBadRelativeCaseId {
    pub const ROOT: Self = Self(0);

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for AffineWhenBadRelativeCaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One direct-formula atom over the canonical structural-locus table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineWhenBadAtom {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
}

impl AffineWhenBadAtom {
    pub const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn new(locus_ordinal: usize, kind: SymbolicPolynomialPredicateKind) -> Self {
        Self {
            locus_ordinal,
            kind,
        }
    }
}

/// One truth supplied by the selected target's authenticated premises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineWhenBadInheritedTruth {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
}

impl AffineWhenBadInheritedTruth {
    pub const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn new(locus_ordinal: usize, kind: SymbolicPolynomialPredicateKind) -> Self {
        Self {
            locus_ordinal,
            kind,
        }
    }
}

/// One retained predicate relative to the selected target root.
#[derive(Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativePredicate {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: ParametricPolynomial,
}

impl AffineWhenBadRelativePredicate {
    pub const fn locus_ordinal(&self) -> usize {
        self.locus_ordinal
    }

    pub const fn kind(&self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub const fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }
}

/// Provenance of one direct bad-formula clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffineWhenBadClauseProvenance {
    CandidateRequiredGuardZero { condition_ordinal: usize },
    CoefficientFieldLeakBoundaryZero { pullback_ordinal: usize },
    FreeIndexLeak { pullback_ordinal: usize },
    WholeTargetFreeIndexLeak { pullback_ordinal: usize },
}

/// Exact unresolved atom and owner-clause which caused one refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativeSplitTrigger {
    clause_ordinal: usize,
    atom: AffineWhenBadAtom,
}

impl AffineWhenBadRelativeSplitTrigger {
    pub const fn clause_ordinal(self) -> usize {
        self.clause_ordinal
    }

    pub const fn atom(self) -> AffineWhenBadAtom {
        self.atom
    }
}

/// One deterministic complementary refinement of a live relative case.
#[derive(Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativeSplit {
    ordinal: usize,
    parent: AffineWhenBadRelativeCaseId,
    trigger: AffineWhenBadRelativeSplitTrigger,
    polynomial: ParametricPolynomial,
    equal_zero_child: AffineWhenBadRelativeCaseId,
    nonzero_child: AffineWhenBadRelativeCaseId,
}

impl AffineWhenBadRelativeSplit {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn parent(&self) -> AffineWhenBadRelativeCaseId {
        self.parent
    }

    pub const fn trigger(&self) -> AffineWhenBadRelativeSplitTrigger {
        self.trigger
    }

    pub const fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub const fn equal_zero_child(&self) -> AffineWhenBadRelativeCaseId {
        self.equal_zero_child
    }

    pub const fn nonzero_child(&self) -> AffineWhenBadRelativeCaseId {
        self.nonzero_child
    }
}

/// One final conjunction relative to the future authenticated target root.
#[derive(Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativeCase {
    id: AffineWhenBadRelativeCaseId,
    predicates: Vec<AffineWhenBadRelativePredicate>,
}

impl AffineWhenBadRelativeCase {
    pub const fn id(&self) -> AffineWhenBadRelativeCaseId {
        self.id
    }

    pub fn predicates(&self) -> &[AffineWhenBadRelativePredicate] {
        &self.predicates
    }
}

/// Final semantic disposition of one structurally retained relative leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffineWhenBadRelativeLeafDisposition {
    Applicable,
    ExceptionalDomain { condition_ordinal: usize },
    ExceptionalLeak { pullback_ordinal: usize },
}

/// One leaf disposition and its decisive direct-formula clause, if bad.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativeLeafClassification {
    case: AffineWhenBadRelativeCaseId,
    disposition: AffineWhenBadRelativeLeafDisposition,
    decisive_clause_ordinal: Option<usize>,
}

impl AffineWhenBadRelativeLeafClassification {
    pub const fn case(&self) -> AffineWhenBadRelativeCaseId {
        self.case
    }

    pub const fn disposition(&self) -> AffineWhenBadRelativeLeafDisposition {
        self.disposition
    }

    pub const fn decisive_clause_ordinal(&self) -> Option<usize> {
        self.decisive_clause_ordinal
    }
}

/// Complete checked work and retained-payload census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffineWhenBadRelativeCaseStats {
    context_fingerprint_bytes: usize,
    structural_loci: usize,
    structural_locus_equality_comparisons: usize,
    structural_locus_associate_comparisons: usize,
    structural_locus_associate_term_pairs: usize,
    inherited_truths: usize,
    bad_clauses: usize,
    bad_atoms: usize,
    direct_bad_formula_evaluations: usize,
    direct_bad_formula_clause_visits: usize,
    direct_bad_formula_atom_truth_queries: usize,
    splits: usize,
    live_leaves: usize,
    case_ids: usize,
    maximum_predicates_per_case: usize,
    predicate_instances: usize,
    leaf_classifications: usize,
    work_decision_cells: usize,
    locus_divisibility_checks: usize,
    locus_divisibility_term_pairs: usize,
    locus_divisibility_cache_entries: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_polynomial_display_bytes: usize,
    retained_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
}

macro_rules! affine_relative_stats_getters {
    ($($name:ident),* $(,)?) => {$ (
        pub const fn $name(self) -> usize { self.$name }
    )* };
}

impl AffineWhenBadRelativeCaseStats {
    affine_relative_stats_getters!(
        context_fingerprint_bytes,
        structural_loci,
        structural_locus_equality_comparisons,
        structural_locus_associate_comparisons,
        structural_locus_associate_term_pairs,
        inherited_truths,
        bad_clauses,
        bad_atoms,
        direct_bad_formula_evaluations,
        direct_bad_formula_clause_visits,
        direct_bad_formula_atom_truth_queries,
        splits,
        live_leaves,
        case_ids,
        maximum_predicates_per_case,
        predicate_instances,
        leaf_classifications,
        work_decision_cells,
        locus_divisibility_checks,
        locus_divisibility_term_pairs,
        locus_divisibility_cache_entries,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        retained_polynomial_display_bytes,
        retained_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
    );
}

/// Replayable target-relative structure.  It proves no target authority by
/// itself and cannot publish or apply a reduction rule.
#[derive(Debug, PartialEq, Eq)]
pub struct AffineWhenBadRelativePartitionCertificate {
    schema: &'static str,
    context_fingerprint: String,
    structural_loci: Vec<ParametricPolynomial>,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    formula: AffineWhenBadDirectFormula,
    splits: Vec<AffineWhenBadRelativeSplit>,
    cases: Vec<AffineWhenBadRelativeCase>,
    classifications: Vec<AffineWhenBadRelativeLeafClassification>,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: AffineWhenBadRelativeCaseStats,
}

impl AffineWhenBadRelativePartitionCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn structural_loci(&self) -> &[ParametricPolynomial] {
        &self.structural_loci
    }

    pub fn inherited_truths(&self) -> &[AffineWhenBadInheritedTruth] {
        &self.inherited_truths
    }

    pub const fn bad_clause_count(&self) -> usize {
        self.formula.clauses.len()
    }

    pub const fn bad_atom_count(&self) -> usize {
        self.formula.atom_count
    }

    pub fn clause_provenance(
        &self,
        clause_ordinal: usize,
    ) -> Option<AffineWhenBadClauseProvenance> {
        self.formula
            .clauses
            .get(clause_ordinal)
            .map(AffineWhenBadFormulaClause::provenance)
    }

    pub fn splits(&self) -> &[AffineWhenBadRelativeSplit] {
        &self.splits
    }

    pub fn cases(&self) -> &[AffineWhenBadRelativeCase] {
        &self.cases
    }

    pub fn classifications(&self) -> &[AffineWhenBadRelativeLeafClassification] {
        &self.classifications
    }

    pub const fn limits(&self) -> AffineWhenBadRelativeCaseLimits {
        self.limits
    }

    pub const fn stats(&self) -> AffineWhenBadRelativeCaseStats {
        self.stats
    }

    pub fn case(&self, id: AffineWhenBadRelativeCaseId) -> Option<&AffineWhenBadRelativeCase> {
        self.cases.iter().find(|case| case.id == id)
    }

    /// Rebuild the complete relative formula overlay and compare every field.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineWhenBadRelativeCaseError> {
        catch_unwind(AssertUnwindSafe(|| self.replay_inner(context))).map_err(|_| {
            AffineWhenBadRelativeCaseError::SymbolicaPanic {
                stage: "relative partition replay",
            }
        })?
    }

    fn replay_inner(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineWhenBadRelativeCaseError> {
        if self.schema != AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA {
            return Err(AffineWhenBadRelativeCaseError::SchemaMismatch);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(AffineWhenBadRelativeCaseError::ContextMismatch);
        }
        preflight_payload_comparison(self, self.limits)?;
        let problem = self.try_copy_problem()?;
        let rebuilt =
            AffineWhenBadRelativePartitionCompiler::compile(context, problem, self.limits)?;
        preflight_payload_comparison(&rebuilt, self.limits)?;
        if self == &rebuilt {
            Ok(())
        } else {
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        }
    }

    fn try_copy_problem(
        &self,
    ) -> Result<AffineWhenBadRelativeProblem, AffineWhenBadRelativeCaseError> {
        // Census the complete source payload, including GMP magnitudes, before
        // reserving a replay vector or copying the first sparse polynomial.
        let mut replay_copy_census = AffineWhenBadRelativeCaseStats::default();
        replay_copy_census.retained_bytes = capacity_byte_envelope(
            self.structural_loci.len(),
            size_of::<ParametricPolynomial>(),
        )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            replay_copy_census.retained_bytes,
            self.limits.max_retained_bytes,
        )?;
        for polynomial in &self.structural_loci {
            charge_retained_polynomial(polynomial, &mut replay_copy_census, self.limits)?;
        }
        let structural_loci = try_canonicalize_structural_loci(&self.structural_loci)?;
        let inherited_truths = try_canonicalize_inherited_truths(&self.inherited_truths)?;
        let clauses = try_canonicalize_formula(&self.formula)?.clauses;
        Ok(AffineWhenBadRelativeProblem::from_preallocated(
            structural_loci,
            inherited_truths,
            clauses,
        ))
    }
}

/// Typed failure of the target-relative structural core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineWhenBadRelativeCaseError {
    SchemaMismatch,
    ContextMismatch,
    IdenticallyZeroStructuralLocus {
        locus_ordinal: usize,
    },
    CoefficientFieldStructuralLocus {
        locus_ordinal: usize,
    },
    DuplicateStructuralLocus {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
    AssociatedStructuralLocusRequiresCanonicalization {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
    StructuralLocusOutOfRange {
        locus_ordinal: usize,
    },
    DuplicateInheritedTruth {
        locus_ordinal: usize,
    },
    MalformedFormulaClause {
        clause_ordinal: usize,
    },
    CaseIdOverflow,
    CaseStateMismatch,
    ReplayMismatch,
    RetainedByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
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
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for AffineWhenBadRelativeCaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("affine WhenBad relative partition schema mismatch")
            }
            Self::ContextMismatch => formatter
                .write_str("affine WhenBad relative partition belongs to another K(n) context"),
            Self::IdenticallyZeroStructuralLocus { locus_ordinal } => write!(
                formatter,
                "affine WhenBad structural locus {locus_ordinal} is identically zero"
            ),
            Self::CoefficientFieldStructuralLocus { locus_ordinal } => write!(
                formatter,
                "affine WhenBad structural locus {locus_ordinal} is a coefficient-field unit"
            ),
            Self::DuplicateStructuralLocus {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "affine WhenBad structural loci {first_ordinal} and {duplicate_ordinal} are equal"
            ),
            Self::AssociatedStructuralLocusRequiresCanonicalization {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "affine WhenBad structural loci {first_ordinal} and {duplicate_ordinal} are K-associates and must be canonicalized by the authenticated owner"
            ),
            Self::StructuralLocusOutOfRange { locus_ordinal } => write!(
                formatter,
                "affine WhenBad formula references missing structural locus {locus_ordinal}"
            ),
            Self::DuplicateInheritedTruth { locus_ordinal } => write!(
                formatter,
                "affine WhenBad root decides structural locus {locus_ordinal} more than once"
            ),
            Self::MalformedFormulaClause { clause_ordinal } => write!(
                formatter,
                "affine WhenBad direct formula clause {clause_ordinal} has malformed atom kinds"
            ),
            Self::CaseIdOverflow => {
                formatter.write_str("affine WhenBad relative case identifier overflow")
            }
            Self::CaseStateMismatch => {
                formatter.write_str("affine WhenBad relative live-case state is inconsistent")
            }
            Self::ReplayMismatch => {
                formatter.write_str("affine WhenBad relative partition did not replay exactly")
            }
            Self::RetainedByteEnvelopeExceeded { observed, admitted } => write!(
                formatter,
                "affine WhenBad relative retained payload observed {observed} bytes after an admitted envelope of {admitted} bytes"
            ),
            Self::SymbolicaPanic { stage } => {
                write!(
                    formatter,
                    "Symbolica panicked during affine WhenBad {stage}"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight"
            ),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AffineWhenBadRelativeCaseError {}

impl From<ParametricCoefficientError> for AffineWhenBadRelativeCaseError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Internal input assembled only after the future outer compiler has
/// authenticated the selected target, conditions, and pullbacks.
pub(crate) struct AffineWhenBadRelativeProblem {
    structural_loci: Vec<ParametricPolynomial>,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    clauses: Vec<AffineWhenBadFormulaClause>,
}

impl AffineWhenBadRelativeProblem {
    pub(crate) fn from_preallocated(
        structural_loci: Vec<ParametricPolynomial>,
        inherited_truths: Vec<AffineWhenBadInheritedTruth>,
        clauses: Vec<AffineWhenBadFormulaClause>,
    ) -> Self {
        Self {
            structural_loci,
            inherited_truths,
            clauses,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineWhenBadFormulaClause {
    CandidateRequiredGuardZero {
        condition_ordinal: usize,
        guard_zero: AffineWhenBadAtom,
    },
    CoefficientFieldLeakBoundaryZero {
        pullback_ordinal: usize,
        boundary_zero: AffineWhenBadAtom,
    },
    FreeIndexLeak {
        pullback_ordinal: usize,
        boundary_zero: AffineWhenBadAtom,
        numerator_nonzero: AffineWhenBadAtom,
    },
    WholeTargetFreeIndexLeak {
        pullback_ordinal: usize,
        numerator_nonzero: AffineWhenBadAtom,
    },
}

impl AffineWhenBadFormulaClause {
    pub(crate) const fn candidate_required_guard_zero(
        condition_ordinal: usize,
        locus_ordinal: usize,
    ) -> Self {
        Self::CandidateRequiredGuardZero {
            condition_ordinal,
            guard_zero: AffineWhenBadAtom::new(
                locus_ordinal,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        }
    }

    pub(crate) const fn coefficient_field_leak_boundary_zero(
        pullback_ordinal: usize,
        boundary_locus_ordinal: usize,
    ) -> Self {
        Self::CoefficientFieldLeakBoundaryZero {
            pullback_ordinal,
            boundary_zero: AffineWhenBadAtom::new(
                boundary_locus_ordinal,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        }
    }

    pub(crate) const fn free_index_leak(
        pullback_ordinal: usize,
        boundary_locus_ordinal: usize,
        numerator_locus_ordinal: usize,
    ) -> Self {
        Self::FreeIndexLeak {
            pullback_ordinal,
            boundary_zero: AffineWhenBadAtom::new(
                boundary_locus_ordinal,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            numerator_nonzero: AffineWhenBadAtom::new(
                numerator_locus_ordinal,
                SymbolicPolynomialPredicateKind::NonZero,
            ),
        }
    }

    pub(crate) const fn whole_target_free_index_leak(
        pullback_ordinal: usize,
        numerator_locus_ordinal: usize,
    ) -> Self {
        Self::WholeTargetFreeIndexLeak {
            pullback_ordinal,
            numerator_nonzero: AffineWhenBadAtom::new(
                numerator_locus_ordinal,
                SymbolicPolynomialPredicateKind::NonZero,
            ),
        }
    }

    const fn provenance(&self) -> AffineWhenBadClauseProvenance {
        match *self {
            Self::CandidateRequiredGuardZero {
                condition_ordinal, ..
            } => AffineWhenBadClauseProvenance::CandidateRequiredGuardZero { condition_ordinal },
            Self::CoefficientFieldLeakBoundaryZero {
                pullback_ordinal, ..
            } => {
                AffineWhenBadClauseProvenance::CoefficientFieldLeakBoundaryZero { pullback_ordinal }
            }
            Self::FreeIndexLeak {
                pullback_ordinal, ..
            } => AffineWhenBadClauseProvenance::FreeIndexLeak { pullback_ordinal },
            Self::WholeTargetFreeIndexLeak {
                pullback_ordinal, ..
            } => AffineWhenBadClauseProvenance::WholeTargetFreeIndexLeak { pullback_ordinal },
        }
    }

    const fn direct(&self) -> DirectBadFormulaClause<AffineWhenBadAtom> {
        match *self {
            Self::CandidateRequiredGuardZero { guard_zero, .. } => {
                DirectBadFormulaClause::Atom(guard_zero)
            }
            Self::CoefficientFieldLeakBoundaryZero { boundary_zero, .. } => {
                DirectBadFormulaClause::Atom(boundary_zero)
            }
            Self::FreeIndexLeak {
                boundary_zero,
                numerator_nonzero,
                ..
            } => DirectBadFormulaClause::Conjunction(boundary_zero, numerator_nonzero),
            Self::WholeTargetFreeIndexLeak {
                numerator_nonzero, ..
            } => DirectBadFormulaClause::Atom(numerator_nonzero),
        }
    }

    fn atoms(self) -> impl Iterator<Item = AffineWhenBadAtom> {
        let (first, second) = match self.direct() {
            DirectBadFormulaClause::Atom(atom) => (atom, None),
            DirectBadFormulaClause::Conjunction(left, right) => (left, Some(right)),
        };
        std::iter::once(first).chain(second)
    }

    const fn has_well_formed_atom_kinds(self) -> bool {
        match self {
            Self::CandidateRequiredGuardZero { guard_zero, .. } => {
                matches!(guard_zero.kind, SymbolicPolynomialPredicateKind::EqualZero)
            }
            Self::CoefficientFieldLeakBoundaryZero { boundary_zero, .. } => {
                matches!(
                    boundary_zero.kind,
                    SymbolicPolynomialPredicateKind::EqualZero
                )
            }
            Self::FreeIndexLeak {
                boundary_zero,
                numerator_nonzero,
                ..
            } => {
                matches!(
                    boundary_zero.kind,
                    SymbolicPolynomialPredicateKind::EqualZero
                ) && matches!(
                    numerator_nonzero.kind,
                    SymbolicPolynomialPredicateKind::NonZero
                )
            }
            Self::WholeTargetFreeIndexLeak {
                numerator_nonzero, ..
            } => matches!(
                numerator_nonzero.kind,
                SymbolicPolynomialPredicateKind::NonZero
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AffineWhenBadDirectFormula {
    clauses: Vec<AffineWhenBadFormulaClause>,
    atom_count: usize,
}

struct WorkCase {
    case: AffineWhenBadRelativeCase,
    decisions: Vec<Option<SymbolicPolynomialPredicateKind>>,
}

#[derive(Clone, Copy)]
struct LocusDivisibilityCacheEntry {
    divisor: usize,
    dividend: usize,
    result: bool,
}

/// Crate-private by design: only the future matcher-bound compiler may feed
/// this structural core in production.
pub(crate) struct AffineWhenBadRelativePartitionCompiler;

impl AffineWhenBadRelativePartitionCompiler {
    pub(crate) fn compile(
        context: &ParametricCoefficientContext,
        problem: AffineWhenBadRelativeProblem,
        limits: AffineWhenBadRelativeCaseLimits,
    ) -> Result<AffineWhenBadRelativePartitionCertificate, AffineWhenBadRelativeCaseError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(context, problem, limits)
        }))
        .map_err(|_| AffineWhenBadRelativeCaseError::SymbolicaPanic {
            stage: "relative partition compilation",
        })?
    }

    fn compile_inner(
        context: &ParametricCoefficientContext,
        problem: AffineWhenBadRelativeProblem,
        limits: AffineWhenBadRelativeCaseLimits,
    ) -> Result<AffineWhenBadRelativePartitionCertificate, AffineWhenBadRelativeCaseError> {
        let mut stats = AffineWhenBadRelativeCaseStats::default();
        stats.context_fingerprint_bytes = context.fingerprint().len();
        check_limit(
            "affine WhenBad relative context fingerprint bytes",
            stats.context_fingerprint_bytes,
            limits.max_context_fingerprint_bytes,
        )?;
        stats.retained_bytes = initial_retained_byte_envelope(
            context.fingerprint().len(),
            &problem.structural_loci,
            &problem.inherited_truths,
            &problem.clauses,
        )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        )?;
        let context_fingerprint = try_copy_string(
            context.fingerprint(),
            "affine WhenBad relative context fingerprint",
        )?;

        let AffineWhenBadRelativeProblem {
            structural_loci: source_structural_loci,
            inherited_truths: source_inherited_truths,
            clauses,
        } = problem;
        validate_structural_loci(context, &source_structural_loci, limits, &mut stats)?;
        let structural_loci = try_canonicalize_structural_loci(&source_structural_loci)?;
        let source_formula = validate_formula(clauses, structural_loci.len(), limits, &mut stats)?;
        let formula = try_canonicalize_formula(&source_formula)?;
        validate_inherited_truths(
            &source_inherited_truths,
            structural_loci.len(),
            limits,
            &mut stats,
        )?;
        let inherited_truths = try_canonicalize_inherited_truths(&source_inherited_truths)?;

        let (splits, cases, classifications) = build_partition(
            context,
            &structural_loci,
            &inherited_truths,
            &formula,
            limits,
            &mut stats,
        )?;
        let (payload_units, payload_bytes, payload_integer_bits) = payload_census(
            &context_fingerprint,
            &structural_loci,
            &inherited_truths,
            &formula,
            &splits,
            &cases,
            &classifications,
        )?;
        let observed_retained_bytes = observed_certificate_owned_byte_bound(
            &context_fingerprint,
            &structural_loci,
            &inherited_truths,
            &formula,
            &splits,
            &cases,
            &classifications,
        )?;
        if observed_retained_bytes > stats.retained_bytes {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed: observed_retained_bytes,
                    admitted: stats.retained_bytes,
                },
            );
        }
        check_limit(
            "affine WhenBad relative payload comparison units",
            payload_units,
            limits.max_payload_comparison_units,
        )?;
        check_limit(
            "affine WhenBad relative payload comparison bytes",
            payload_bytes,
            limits.max_payload_comparison_bytes,
        )?;
        check_limit(
            "affine WhenBad relative payload comparison integer bits",
            payload_integer_bits,
            limits.max_payload_comparison_integer_bits,
        )?;
        stats.payload_comparison_units = payload_units;
        stats.payload_comparison_bytes = payload_bytes;
        stats.payload_comparison_integer_bits = payload_integer_bits;

        Ok(AffineWhenBadRelativePartitionCertificate {
            schema: AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA,
            context_fingerprint,
            structural_loci,
            inherited_truths,
            formula,
            splits,
            cases,
            classifications,
            limits,
            stats,
        })
    }
}

fn try_canonicalize_structural_loci(
    source: &[ParametricPolynomial],
) -> Result<Vec<ParametricPolynomial>, AffineWhenBadRelativeCaseError> {
    let mut canonical = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative canonical structural loci",
        &mut canonical,
        source.len(),
    )?;
    for polynomial in source {
        let admitted = deterministic_polynomial_owned_byte_envelope(polynomial)?;
        let copied = try_copy_polynomial(
            polynomial,
            "affine WhenBad relative canonical structural locus",
        )?;
        let observed = copied.owned_retained_byte_bound().ok_or(
            AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                resource: "affine WhenBad relative retained bytes",
            },
        )?;
        if observed > admitted {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded { observed, admitted },
            );
        }
        canonical.push(copied);
    }
    Ok(canonical)
}

fn try_canonicalize_inherited_truths(
    source: &[AffineWhenBadInheritedTruth],
) -> Result<Vec<AffineWhenBadInheritedTruth>, AffineWhenBadRelativeCaseError> {
    let mut canonical = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative canonical inherited truths",
        &mut canonical,
        source.len(),
    )?;
    canonical.extend_from_slice(source);
    Ok(canonical)
}

fn try_canonicalize_formula(
    source: &AffineWhenBadDirectFormula,
) -> Result<AffineWhenBadDirectFormula, AffineWhenBadRelativeCaseError> {
    let mut clauses = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative canonical formula clauses",
        &mut clauses,
        source.clauses.len(),
    )?;
    clauses.extend_from_slice(&source.clauses);
    Ok(AffineWhenBadDirectFormula {
        clauses,
        atom_count: source.atom_count,
    })
}

fn validate_structural_loci(
    context: &ParametricCoefficientContext,
    loci: &[ParametricPolynomial],
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad relative structural loci",
        loci.len(),
        limits.max_structural_loci,
    )?;
    for (ordinal, polynomial) in loci.iter().enumerate() {
        context.validate_polynomial_with_limits(polynomial, limits.exact_algebra)?;
        if polynomial.is_zero() {
            return Err(
                AffineWhenBadRelativeCaseError::IdenticallyZeroStructuralLocus {
                    locus_ordinal: ordinal,
                },
            );
        }
        if !context.polynomial_depends_on_indices_with_limits(polynomial, limits.exact_algebra)? {
            return Err(
                AffineWhenBadRelativeCaseError::CoefficientFieldStructuralLocus {
                    locus_ordinal: ordinal,
                },
            );
        }
        // Admit the complete sparse payload before any associate proof can
        // construct its temporary coefficient copies.
        charge_retained_polynomial(polynomial, stats, limits)?;
        for (first_ordinal, first) in loci[..ordinal].iter().enumerate() {
            stats.structural_locus_equality_comparisons = checked_bounded_add(
                "affine WhenBad relative structural locus equality comparisons",
                stats.structural_locus_equality_comparisons,
                1,
                limits.max_structural_locus_equality_comparisons,
            )?;
            if first == polynomial {
                return Err(AffineWhenBadRelativeCaseError::DuplicateStructuralLocus {
                    first_ordinal,
                    duplicate_ordinal: ordinal,
                });
            }
            let term_pairs = checked_mul(
                "affine WhenBad relative structural locus associate term pairs",
                first.term_count(),
                polynomial.term_count(),
            )?;
            let prospective_comparisons = checked_bounded_add(
                "affine WhenBad relative structural locus associate comparisons",
                stats.structural_locus_associate_comparisons,
                1,
                limits.max_structural_locus_associate_comparisons,
            )?;
            let prospective_term_pairs = checked_bounded_add(
                "affine WhenBad relative structural locus associate term pairs",
                stats.structural_locus_associate_term_pairs,
                term_pairs,
                limits.max_structural_locus_associate_term_pairs,
            )?;
            let associated = context.polynomial_loci_are_associates_with_limits(
                first,
                polynomial,
                limits.exact_algebra,
            )?;
            stats.structural_locus_associate_comparisons = prospective_comparisons;
            stats.structural_locus_associate_term_pairs = prospective_term_pairs;
            if associated {
                return Err(
                    AffineWhenBadRelativeCaseError::AssociatedStructuralLocusRequiresCanonicalization {
                        first_ordinal,
                        duplicate_ordinal: ordinal,
                    },
                );
            }
        }
    }
    stats.structural_loci = loci.len();
    Ok(())
}

fn validate_formula(
    clauses: Vec<AffineWhenBadFormulaClause>,
    locus_count: usize,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
) -> Result<AffineWhenBadDirectFormula, AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad relative bad clauses",
        clauses.len(),
        limits.max_bad_clauses,
    )?;
    let mut atom_count = 0usize;
    for (clause_ordinal, clause) in clauses.iter().copied().enumerate() {
        if !clause.has_well_formed_atom_kinds() {
            return Err(AffineWhenBadRelativeCaseError::MalformedFormulaClause { clause_ordinal });
        }
        for atom in clause.atoms() {
            if atom.locus_ordinal >= locus_count {
                return Err(AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                    locus_ordinal: atom.locus_ordinal,
                });
            }
        }
        atom_count = checked_add(
            "affine WhenBad relative bad atoms",
            atom_count,
            clause.direct().atom_count(),
        )?;
    }
    check_limit(
        "affine WhenBad relative bad atoms",
        atom_count,
        limits.max_bad_atoms,
    )?;
    stats.bad_clauses = clauses.len();
    stats.bad_atoms = atom_count;
    Ok(AffineWhenBadDirectFormula {
        clauses,
        atom_count,
    })
}

fn validate_inherited_truths(
    inherited_truths: &[AffineWhenBadInheritedTruth],
    locus_count: usize,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad relative inherited truths",
        inherited_truths.len(),
        limits.max_inherited_truths,
    )?;
    let mut seen = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative inherited truth decisions",
        &mut seen,
        locus_count,
    )?;
    seen.resize(locus_count, false);
    for truth in inherited_truths {
        let Some(slot) = seen.get_mut(truth.locus_ordinal) else {
            return Err(AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                locus_ordinal: truth.locus_ordinal,
            });
        };
        if *slot {
            return Err(AffineWhenBadRelativeCaseError::DuplicateInheritedTruth {
                locus_ordinal: truth.locus_ordinal,
            });
        }
        *slot = true;
    }
    stats.inherited_truths = inherited_truths.len();
    Ok(())
}

fn build_partition(
    context: &ParametricCoefficientContext,
    structural_loci: &Vec<ParametricPolynomial>,
    inherited_truths: &[AffineWhenBadInheritedTruth],
    formula: &AffineWhenBadDirectFormula,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
) -> Result<
    (
        Vec<AffineWhenBadRelativeSplit>,
        Vec<AffineWhenBadRelativeCase>,
        Vec<AffineWhenBadRelativeLeafClassification>,
    ),
    AffineWhenBadRelativeCaseError,
> {
    check_limit(
        "affine WhenBad relative live leaves",
        1,
        limits.max_live_leaves,
    )?;
    check_limit(
        "affine WhenBad relative case identifiers",
        1,
        limits.max_case_ids,
    )?;
    check_limit(
        "affine WhenBad relative leaf classifications",
        1,
        limits.max_leaf_classifications,
    )?;
    check_limit(
        "affine WhenBad relative work decision cells",
        structural_loci.len(),
        limits.max_work_decision_cells,
    )?;

    let mut root_decisions = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative root decisions",
        &mut root_decisions,
        structural_loci.len(),
    )?;
    root_decisions.resize(structural_loci.len(), None);
    for truth in inherited_truths {
        let decision = root_decisions.get_mut(truth.locus_ordinal).ok_or(
            AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                locus_ordinal: truth.locus_ordinal,
            },
        )?;
        if decision.replace(truth.kind).is_some() {
            return Err(AffineWhenBadRelativeCaseError::DuplicateInheritedTruth {
                locus_ordinal: truth.locus_ordinal,
            });
        }
    }

    let root = WorkCase {
        case: AffineWhenBadRelativeCase {
            id: AffineWhenBadRelativeCaseId::ROOT,
            predicates: Vec::new(),
        },
        decisions: root_decisions,
    };
    let mut slots = Vec::new();
    try_reserve_exact("affine WhenBad relative case slots", &mut slots, 1)?;
    slots.push(Some(root));
    let mut disposition_slots = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative disposition slots",
        &mut disposition_slots,
        1,
    )?;
    disposition_slots.push(None);
    let mut work = Vec::new();
    try_reserve_exact("affine WhenBad relative work queue", &mut work, 1)?;
    work.push(AffineWhenBadRelativeCaseId::ROOT);
    let mut splits = Vec::new();
    let mut divisibility_cache = Vec::new();

    stats.live_leaves = 1;
    stats.case_ids = 1;
    stats.work_decision_cells = structural_loci.len();

    while let Some(case_id) = work.pop() {
        let case_index = case_index(case_id)?;
        let route = {
            let work_case = slots
                .get(case_index)
                .and_then(Option::as_ref)
                .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
            route_formula(
                context,
                structural_loci,
                formula,
                &work_case.decisions,
                &mut divisibility_cache,
                stats,
                limits,
            )?
        };
        match route {
            DirectBadFormulaRoute::Bad { clause_ordinal } => {
                let disposition = disposition_for_bad_clause(formula, clause_ordinal)?;
                retain_classification(
                    &mut disposition_slots,
                    case_index,
                    AffineWhenBadRelativeLeafClassification {
                        case: case_id,
                        disposition,
                        decisive_clause_ordinal: Some(clause_ordinal),
                    },
                    stats,
                    limits,
                )?;
            }
            DirectBadFormulaRoute::Good => {
                retain_classification(
                    &mut disposition_slots,
                    case_index,
                    AffineWhenBadRelativeLeafClassification {
                        case: case_id,
                        disposition: AffineWhenBadRelativeLeafDisposition::Applicable,
                        decisive_clause_ordinal: None,
                    },
                    stats,
                    limits,
                )?;
            }
            DirectBadFormulaRoute::Split {
                clause_ordinal,
                atom,
            } => split_work_case(
                structural_loci,
                &mut slots,
                &mut disposition_slots,
                &mut work,
                &mut splits,
                case_id,
                AffineWhenBadRelativeSplitTrigger {
                    clause_ordinal,
                    atom,
                },
                stats,
                limits,
            )?,
        }
    }

    if slots.len() != disposition_slots.len()
        || stats.leaf_classifications != stats.live_leaves
        || stats.splits.checked_add(1) != Some(stats.live_leaves)
    {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    let mut cases = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative final cases",
        &mut cases,
        stats.live_leaves,
    )?;
    let mut classifications = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative final classifications",
        &mut classifications,
        stats.leaf_classifications,
    )?;
    for (slot_ordinal, (slot, disposition)) in slots
        .into_iter()
        .zip(disposition_slots.into_iter())
        .enumerate()
    {
        match (slot, disposition) {
            (None, None) => {}
            (Some(work_case), Some(classification)) => {
                if usize::try_from(work_case.case.id.0).ok() != Some(slot_ordinal)
                    || classification.case != work_case.case.id
                {
                    return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
                }
                cases.push(work_case.case);
                classifications.push(classification);
            }
            _ => return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch),
        }
    }
    if cases.len() != stats.live_leaves
        || classifications.len() != stats.leaf_classifications
        || !cases
            .iter()
            .map(|case| case.id)
            .eq(classifications.iter().map(|entry| entry.case))
    {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    Ok((splits, cases, classifications))
}

fn route_formula(
    context: &ParametricCoefficientContext,
    structural_loci: &[ParametricPolynomial],
    formula: &AffineWhenBadDirectFormula,
    decisions: &[Option<SymbolicPolynomialPredicateKind>],
    divisibility_cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<DirectBadFormulaRoute<AffineWhenBadAtom>, AffineWhenBadRelativeCaseError> {
    let evaluations = checked_bounded_add(
        "affine WhenBad relative direct bad-formula evaluations",
        stats.direct_bad_formula_evaluations,
        1,
        limits.max_direct_bad_formula_evaluations,
    )?;
    let clause_visits = checked_bounded_add(
        "affine WhenBad relative direct bad-formula clause visits",
        stats.direct_bad_formula_clause_visits,
        formula.clauses.len(),
        limits.max_direct_bad_formula_clause_visits,
    )?;
    let atom_queries = checked_bounded_add(
        "affine WhenBad relative direct bad-formula atom truth queries",
        stats.direct_bad_formula_atom_truth_queries,
        formula.atom_count,
        limits.max_direct_bad_formula_atom_truth_queries,
    )?;
    stats.direct_bad_formula_evaluations = evaluations;
    stats.direct_bad_formula_clause_visits = clause_visits;
    stats.direct_bad_formula_atom_truth_queries = atom_queries;

    route_direct_bad_formula(
        formula
            .clauses
            .iter()
            .map(AffineWhenBadFormulaClause::direct),
        |atom| {
            let exact = decisions.get(atom.locus_ordinal).copied().ok_or(
                AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                    locus_ordinal: atom.locus_ordinal,
                },
            )?;
            let decided = match exact {
                Some(kind) => Some(kind),
                None => implied_locus_decision(
                    context,
                    atom.locus_ordinal,
                    decisions,
                    structural_loci,
                    divisibility_cache,
                    stats,
                    limits,
                )?,
            };
            Ok(match decided {
                Some(kind) if kind == atom.kind => DirectBadFormulaTruth::True,
                Some(_) => DirectBadFormulaTruth::False,
                None => DirectBadFormulaTruth::Unknown,
            })
        },
    )
}

/// Sound principal-divisibility implications in the integral domain `K[n]`:
/// if `p | q`, then `p=0 => q=0` and `q!=0 => p!=0`.
fn implied_locus_decision(
    context: &ParametricCoefficientContext,
    requested_locus: usize,
    decisions: &[Option<SymbolicPolynomialPredicateKind>],
    structural_loci: &[ParametricPolynomial],
    cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<Option<SymbolicPolynomialPredicateKind>, AffineWhenBadRelativeCaseError> {
    let requested = structural_loci.get(requested_locus).ok_or(
        AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
            locus_ordinal: requested_locus,
        },
    )?;
    for (known_locus, known_kind) in decisions
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(ordinal, kind)| kind.map(|kind| (ordinal, kind)))
    {
        let known = structural_loci.get(known_locus).ok_or(
            AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                locus_ordinal: known_locus,
            },
        )?;
        let implied = match known_kind {
            SymbolicPolynomialPredicateKind::EqualZero => cached_locus_divisibility(
                context,
                known_locus,
                requested_locus,
                known,
                requested,
                cache,
                stats,
                limits,
            )?
            .then_some(SymbolicPolynomialPredicateKind::EqualZero),
            SymbolicPolynomialPredicateKind::NonZero => cached_locus_divisibility(
                context,
                requested_locus,
                known_locus,
                requested,
                known,
                cache,
                stats,
                limits,
            )?
            .then_some(SymbolicPolynomialPredicateKind::NonZero),
        };
        if implied.is_some() {
            return Ok(implied);
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
    cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<bool, AffineWhenBadRelativeCaseError> {
    if let Some(entry) = cache
        .iter()
        .find(|entry| entry.divisor == divisor_ordinal && entry.dividend == dividend_ordinal)
    {
        return Ok(entry.result);
    }
    let term_pairs = checked_mul(
        "affine WhenBad relative locus divisibility term pairs",
        divisor.term_count(),
        dividend.term_count(),
    )?;
    let prospective_checks = checked_bounded_add(
        "affine WhenBad relative locus divisibility checks",
        stats.locus_divisibility_checks,
        1,
        limits.max_locus_divisibility_checks,
    )?;
    let prospective_term_pairs = checked_bounded_add(
        "affine WhenBad relative locus divisibility term pairs",
        stats.locus_divisibility_term_pairs,
        term_pairs,
        limits.max_locus_divisibility_term_pairs,
    )?;
    let prospective_entries = checked_bounded_add(
        "affine WhenBad relative locus divisibility cache entries",
        cache.len(),
        1,
        limits.max_locus_divisibility_cache_entries,
    )?;
    try_reserve_exact("affine WhenBad relative locus divisibility cache", cache, 1)?;
    let result = context.polynomial_divides_with_limits(divisor, dividend, limits.exact_algebra)?;
    cache.push(LocusDivisibilityCacheEntry {
        divisor: divisor_ordinal,
        dividend: dividend_ordinal,
        result,
    });
    stats.locus_divisibility_checks = prospective_checks;
    stats.locus_divisibility_term_pairs = prospective_term_pairs;
    stats.locus_divisibility_cache_entries = prospective_entries;
    Ok(result)
}

fn disposition_for_bad_clause(
    formula: &AffineWhenBadDirectFormula,
    clause_ordinal: usize,
) -> Result<AffineWhenBadRelativeLeafDisposition, AffineWhenBadRelativeCaseError> {
    let clause = formula
        .clauses
        .get(clause_ordinal)
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
    Ok(match clause.provenance() {
        AffineWhenBadClauseProvenance::CandidateRequiredGuardZero { condition_ordinal } => {
            AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal }
        }
        AffineWhenBadClauseProvenance::CoefficientFieldLeakBoundaryZero { pullback_ordinal }
        | AffineWhenBadClauseProvenance::FreeIndexLeak { pullback_ordinal }
        | AffineWhenBadClauseProvenance::WholeTargetFreeIndexLeak { pullback_ordinal } => {
            AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal }
        }
    })
}

fn retain_classification(
    disposition_slots: &mut [Option<AffineWhenBadRelativeLeafClassification>],
    case_index: usize,
    classification: AffineWhenBadRelativeLeafClassification,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    let prospective = checked_bounded_add(
        "affine WhenBad relative leaf classifications",
        stats.leaf_classifications,
        1,
        limits.max_leaf_classifications,
    )?;
    let slot = disposition_slots
        .get_mut(case_index)
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
    if slot.replace(classification).is_some() {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    stats.leaf_classifications = prospective;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn split_work_case(
    structural_loci: &[ParametricPolynomial],
    slots: &mut Vec<Option<WorkCase>>,
    disposition_slots: &mut Vec<Option<AffineWhenBadRelativeLeafClassification>>,
    work: &mut Vec<AffineWhenBadRelativeCaseId>,
    splits: &mut Vec<AffineWhenBadRelativeSplit>,
    parent_id: AffineWhenBadRelativeCaseId,
    trigger: AffineWhenBadRelativeSplitTrigger,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    let parent_index = case_index(parent_id)?;
    let parent = slots
        .get(parent_index)
        .and_then(Option::as_ref)
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
    if disposition_slots
        .get(parent_index)
        .and_then(Option::as_ref)
        .is_some()
    {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    let polynomial = structural_loci.get(trigger.atom.locus_ordinal).ok_or(
        AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
            locus_ordinal: trigger.atom.locus_ordinal,
        },
    )?;
    if parent
        .decisions
        .get(trigger.atom.locus_ordinal)
        .copied()
        .flatten()
        .is_some()
    {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    let parent_depth = parent.case.predicates.len();
    let child_depth = checked_add(
        "affine WhenBad relative predicates per case",
        parent_depth,
        1,
    )?;

    let mut staged = *stats;
    staged.splits = checked_bounded_add(
        "affine WhenBad relative splits",
        staged.splits,
        1,
        limits.max_splits,
    )?;
    staged.live_leaves = checked_bounded_add(
        "affine WhenBad relative live leaves",
        staged.live_leaves,
        1,
        limits.max_live_leaves,
    )?;
    check_limit(
        "affine WhenBad relative leaf classifications",
        staged.live_leaves,
        limits.max_leaf_classifications,
    )?;
    staged.case_ids = checked_bounded_add(
        "affine WhenBad relative case identifiers",
        staged.case_ids,
        2,
        limits.max_case_ids,
    )?;
    check_limit(
        "affine WhenBad relative predicates per case",
        child_depth,
        limits.max_predicates_per_case,
    )?;
    staged.maximum_predicates_per_case = staged.maximum_predicates_per_case.max(child_depth);
    let predicate_delta = checked_add(
        "affine WhenBad relative predicate instances",
        parent_depth,
        2,
    )?;
    staged.predicate_instances = checked_bounded_add(
        "affine WhenBad relative predicate instances",
        staged.predicate_instances,
        predicate_delta,
        limits.max_predicate_instances,
    )?;
    staged.work_decision_cells = checked_mul(
        "affine WhenBad relative work decision cells",
        staged.live_leaves,
        structural_loci.len(),
    )?;
    check_limit(
        "affine WhenBad relative work decision cells",
        staged.work_decision_cells,
        limits.max_work_decision_cells,
    )?;
    let retained_container_delta = checked_add(
        "affine WhenBad relative retained bytes",
        capacity_byte_envelope(1, size_of::<AffineWhenBadRelativeSplit>())?,
        checked_add(
            "affine WhenBad relative retained bytes",
            capacity_byte_envelope(1, size_of::<AffineWhenBadRelativeCase>())?,
            checked_add(
                "affine WhenBad relative retained bytes",
                capacity_byte_envelope(1, size_of::<AffineWhenBadRelativeLeafClassification>())?,
                capacity_byte_envelope(
                    predicate_delta,
                    size_of::<AffineWhenBadRelativePredicate>(),
                )?,
            )?,
        )?,
    )?;
    staged.retained_bytes = checked_bounded_add(
        "affine WhenBad relative retained bytes",
        staged.retained_bytes,
        retained_container_delta,
        limits.max_retained_bytes,
    )?;

    let mut equal_predicates = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative equal-zero child predicates",
        &mut equal_predicates,
        child_depth,
    )?;
    for predicate in &parent.case.predicates {
        equal_predicates.push(try_copy_predicate(predicate, &mut staged, limits)?);
    }
    equal_predicates.push(AffineWhenBadRelativePredicate {
        locus_ordinal: trigger.atom.locus_ordinal,
        kind: SymbolicPolynomialPredicateKind::EqualZero,
        polynomial: try_copy_and_charge_polynomial(polynomial, &mut staged, limits)?,
    });
    let nonzero_polynomial = try_copy_and_charge_polynomial(polynomial, &mut staged, limits)?;
    let transcript_polynomial = try_copy_and_charge_polynomial(polynomial, &mut staged, limits)?;

    let mut equal_decisions = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative equal-zero child decisions",
        &mut equal_decisions,
        parent.decisions.len(),
    )?;
    equal_decisions.extend_from_slice(&parent.decisions);
    equal_decisions[trigger.atom.locus_ordinal] = Some(SymbolicPolynomialPredicateKind::EqualZero);

    try_reserve_exact("affine WhenBad relative case slots", slots, 2)?;
    try_reserve_exact(
        "affine WhenBad relative disposition slots",
        disposition_slots,
        2,
    )?;
    try_reserve_exact("affine WhenBad relative work queue", work, 2)?;
    try_reserve_exact("affine WhenBad relative split transcript", splits, 1)?;
    slots
        .get_mut(parent_index)
        .and_then(Option::as_mut)
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?
        .case
        .predicates
        .try_reserve_exact(1)
        .map_err(|_| AffineWhenBadRelativeCaseError::AllocationFailure {
            resource: "affine WhenBad relative nonzero child predicates",
            requested: child_depth,
        })?;

    let equal_raw =
        u64::try_from(slots.len()).map_err(|_| AffineWhenBadRelativeCaseError::CaseIdOverflow)?;
    let nonzero_raw = equal_raw
        .checked_add(1)
        .ok_or(AffineWhenBadRelativeCaseError::CaseIdOverflow)?;
    let equal_id = AffineWhenBadRelativeCaseId(equal_raw);
    let nonzero_id = AffineWhenBadRelativeCaseId(nonzero_raw);

    let mut parent = slots[parent_index]
        .take()
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
    parent.decisions[trigger.atom.locus_ordinal] = Some(SymbolicPolynomialPredicateKind::NonZero);
    parent.case.predicates.push(AffineWhenBadRelativePredicate {
        locus_ordinal: trigger.atom.locus_ordinal,
        kind: SymbolicPolynomialPredicateKind::NonZero,
        polynomial: nonzero_polynomial,
    });
    parent.case.id = nonzero_id;

    let equal = WorkCase {
        case: AffineWhenBadRelativeCase {
            id: equal_id,
            predicates: equal_predicates,
        },
        decisions: equal_decisions,
    };
    slots.push(Some(equal));
    slots.push(Some(parent));
    disposition_slots.push(None);
    disposition_slots.push(None);
    splits.push(AffineWhenBadRelativeSplit {
        ordinal: splits.len(),
        parent: parent_id,
        trigger,
        polynomial: transcript_polynomial,
        equal_zero_child: equal_id,
        nonzero_child: nonzero_id,
    });
    // Stack order is reversed so the equality child is evaluated first.
    work.push(nonzero_id);
    work.push(equal_id);
    *stats = staged;
    Ok(())
}

fn try_copy_predicate(
    source: &AffineWhenBadRelativePredicate,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<AffineWhenBadRelativePredicate, AffineWhenBadRelativeCaseError> {
    Ok(AffineWhenBadRelativePredicate {
        locus_ordinal: source.locus_ordinal,
        kind: source.kind,
        polynomial: try_copy_and_charge_polynomial(&source.polynomial, stats, limits)?,
    })
}

fn try_copy_and_charge_polynomial(
    source: &ParametricPolynomial,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<ParametricPolynomial, AffineWhenBadRelativeCaseError> {
    let before_bytes = stats.retained_bytes;
    let mut staged = *stats;
    charge_retained_polynomial(source, &mut staged, limits)?;
    let copied = try_copy_polynomial(
        source,
        "affine WhenBad relative retained polynomial payload",
    )?;
    let observed = copied.owned_retained_byte_bound().ok_or(
        AffineWhenBadRelativeCaseError::ResourceCountOverflow {
            resource: "affine WhenBad relative retained bytes",
        },
    )?;
    let admitted = staged.retained_bytes.checked_sub(before_bytes).ok_or(
        AffineWhenBadRelativeCaseError::ResourceCountOverflow {
            resource: "affine WhenBad relative retained bytes",
        },
    )?;
    if observed > admitted {
        return Err(
            AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded { observed, admitted },
        );
    }
    *stats = staged;
    Ok(copied)
}

fn charge_retained_polynomial(
    polynomial: &ParametricPolynomial,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    let terms = polynomial.term_count();
    let exponent_entries = checked_mul(
        "affine WhenBad relative retained polynomial exponent entries",
        terms,
        polynomial.raw().variables.len(),
    )?;
    let integer_bits =
        polynomial
            .raw()
            .coefficients
            .iter()
            .try_fold(0usize, |total, coefficient| {
                checked_add(
                    "affine WhenBad relative retained polynomial integer bits",
                    total,
                    integer_magnitude_bits(coefficient)?,
                )
            })?;
    let remaining_display_bytes = limits
        .max_retained_polynomial_display_bytes
        .checked_sub(stats.retained_polynomial_display_bytes)
        .ok_or(AffineWhenBadRelativeCaseError::ResourceLimit {
            resource: "affine WhenBad relative retained polynomial display bytes",
            requested: stats.retained_polynomial_display_bytes,
            limit: limits.max_retained_polynomial_display_bytes,
        })?;
    let local_display_bytes = bounded_polynomial_display_bytes(polynomial, remaining_display_bytes)
        .map_err(|requested| AffineWhenBadRelativeCaseError::ResourceLimit {
            resource: "affine WhenBad relative retained polynomial display bytes",
            requested: stats
                .retained_polynomial_display_bytes
                .checked_add(requested)
                .unwrap_or(usize::MAX),
            limit: limits.max_retained_polynomial_display_bytes,
        })?;

    let staged_terms = checked_bounded_add(
        "affine WhenBad relative retained polynomial terms",
        stats.retained_polynomial_terms,
        terms,
        limits.max_retained_polynomial_terms,
    )?;
    let staged_exponents = checked_bounded_add(
        "affine WhenBad relative retained polynomial exponent entries",
        stats.retained_polynomial_exponent_entries,
        exponent_entries,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    let staged_integer_bits = checked_bounded_add(
        "affine WhenBad relative retained polynomial integer bits",
        stats.retained_polynomial_integer_bits,
        integer_bits,
        limits.max_retained_polynomial_integer_bits,
    )?;
    let staged_display_bytes = checked_bounded_add(
        "affine WhenBad relative retained polynomial display bytes",
        stats.retained_polynomial_display_bytes,
        local_display_bytes,
        limits.max_retained_polynomial_display_bytes,
    )?;
    let owned_bytes = deterministic_polynomial_owned_byte_envelope(polynomial)?;
    let staged_retained_bytes = checked_bounded_add(
        "affine WhenBad relative retained bytes",
        stats.retained_bytes,
        owned_bytes,
        limits.max_retained_bytes,
    )?;
    stats.retained_polynomial_terms = staged_terms;
    stats.retained_polynomial_exponent_entries = staged_exponents;
    stats.retained_polynomial_integer_bits = staged_integer_bits;
    stats.retained_polynomial_display_bytes = staged_display_bytes;
    stats.retained_bytes = staged_retained_bytes;
    Ok(())
}

/// Allocator-independent upper envelope for a freshly canonicalized sparse
/// polynomial copy. Vector storage gets a factor-two capacity allowance;
/// `Integer::Large` storage is its significant-byte length plus one machine
/// word of limb-rounding slack. Actual retained capacity is checked separately
/// after every copy and again for the completed certificate.
fn deterministic_polynomial_owned_byte_envelope(
    polynomial: &ParametricPolynomial,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<ParametricPolynomial>();
    bytes = checked_add(
        "affine WhenBad relative retained bytes",
        bytes,
        capacity_byte_envelope(polynomial.raw().coefficients.len(), size_of::<Integer>())?,
    )?;
    bytes = checked_add(
        "affine WhenBad relative retained bytes",
        bytes,
        capacity_byte_envelope(polynomial.raw().exponents.len(), size_of::<u16>())?,
    )?;
    for coefficient in &polynomial.raw().coefficients {
        if matches!(coefficient, Integer::Large(_)) {
            let magnitude_bytes = integer_magnitude_bits(coefficient)?.checked_add(7).ok_or(
                AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                    resource: "affine WhenBad relative retained bytes",
                },
            )? / 8;
            bytes = checked_add(
                "affine WhenBad relative retained bytes",
                bytes,
                checked_add(
                    "affine WhenBad relative retained bytes",
                    magnitude_bytes,
                    size_of::<usize>(),
                )?,
            )?;
        }
    }
    Ok(bytes)
}

fn payload_census(
    context_fingerprint: &str,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
    formula: &AffineWhenBadDirectFormula,
    splits: &[AffineWhenBadRelativeSplit],
    cases: &[AffineWhenBadRelativeCase],
    classifications: &[AffineWhenBadRelativeLeafClassification],
) -> Result<(usize, usize, usize), AffineWhenBadRelativeCaseError> {
    let mut units = scalar_representation_units::<AffineWhenBadRelativePartitionCertificate>();
    for count in [
        scalar_representation_units::<AffineWhenBadRelativeCaseLimits>(),
        scalar_representation_units::<AffineWhenBadRelativeCaseStats>(),
        scalar_representation_units::<AffineWhenBadDirectFormula>(),
        checked_mul(
            "affine WhenBad relative payload comparison units",
            inherited_truths.len(),
            scalar_representation_units::<AffineWhenBadInheritedTruth>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            formula.clauses.len(),
            scalar_representation_units::<AffineWhenBadFormulaClause>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            splits.len(),
            scalar_representation_units::<AffineWhenBadRelativeSplit>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            cases.len(),
            scalar_representation_units::<AffineWhenBadRelativeCase>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            classifications.len(),
            scalar_representation_units::<AffineWhenBadRelativeLeafClassification>(),
        )?,
    ] {
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            count,
        )?;
    }
    let mut bytes = context_fingerprint.len();
    let mut integer_bits = 0usize;
    for polynomial in structural_loci
        .iter()
        .chain(splits.iter().map(|split| &split.polynomial))
        .chain(cases.iter().flat_map(|case| {
            case.predicates
                .iter()
                .map(|predicate| &predicate.polynomial)
        }))
    {
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            scalar_representation_units::<ParametricPolynomial>(),
        )?;
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            polynomial.term_count(),
        )?;
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            polynomial.raw().exponents.len(),
        )?;
        bytes = checked_add(
            "affine WhenBad relative payload comparison bytes",
            bytes,
            bounded_polynomial_display_bytes(polynomial, usize::MAX).map_err(|_| {
                AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                    resource: "affine WhenBad relative payload comparison bytes",
                }
            })?,
        )?;
        for coefficient in &polynomial.raw().coefficients {
            integer_bits = checked_add(
                "affine WhenBad relative payload comparison integer bits",
                integer_bits,
                integer_magnitude_bits(coefficient)?,
            )?;
        }
    }
    for case in cases {
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            checked_mul(
                "affine WhenBad relative payload comparison units",
                case.predicates.len(),
                scalar_representation_units::<AffineWhenBadRelativePredicate>(),
            )?,
        )?;
    }
    Ok((units, bytes, integer_bits))
}

fn initial_retained_byte_envelope(
    context_fingerprint_bytes: usize,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
    clauses: &[AffineWhenBadFormulaClause],
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<AffineWhenBadRelativePartitionCertificate>();
    bytes = checked_add(
        "affine WhenBad relative retained bytes",
        bytes,
        capacity_byte_envelope(context_fingerprint_bytes, size_of::<u8>())?,
    )?;
    for allocation in [
        capacity_byte_envelope(structural_loci.len(), size_of::<ParametricPolynomial>())?,
        capacity_byte_envelope(
            inherited_truths.len(),
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(clauses.len(), size_of::<AffineWhenBadFormulaClause>())?,
        capacity_byte_envelope(1, size_of::<AffineWhenBadRelativeCase>())?,
        capacity_byte_envelope(1, size_of::<AffineWhenBadRelativeLeafClassification>())?,
    ] {
        bytes = checked_add("affine WhenBad relative retained bytes", bytes, allocation)?;
    }
    Ok(bytes)
}

fn observed_certificate_owned_byte_bound(
    context_fingerprint: &String,
    structural_loci: &Vec<ParametricPolynomial>,
    inherited_truths: &Vec<AffineWhenBadInheritedTruth>,
    formula: &AffineWhenBadDirectFormula,
    splits: &Vec<AffineWhenBadRelativeSplit>,
    cases: &Vec<AffineWhenBadRelativeCase>,
    classifications: &Vec<AffineWhenBadRelativeLeafClassification>,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<AffineWhenBadRelativePartitionCertificate>();
    for allocation in [
        context_fingerprint.capacity(),
        checked_mul(
            "affine WhenBad relative retained bytes",
            structural_loci.capacity(),
            size_of::<ParametricPolynomial>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            inherited_truths.capacity(),
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            formula.clauses.capacity(),
            size_of::<AffineWhenBadFormulaClause>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            splits.capacity(),
            size_of::<AffineWhenBadRelativeSplit>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            cases.capacity(),
            size_of::<AffineWhenBadRelativeCase>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            classifications.capacity(),
            size_of::<AffineWhenBadRelativeLeafClassification>(),
        )?,
    ] {
        bytes = checked_add("affine WhenBad relative retained bytes", bytes, allocation)?;
    }
    for polynomial in structural_loci {
        bytes = checked_add(
            "affine WhenBad relative retained bytes",
            bytes,
            polynomial.owned_retained_byte_bound().ok_or(
                AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                    resource: "affine WhenBad relative retained bytes",
                },
            )?,
        )?;
    }
    for split in splits {
        bytes = checked_add(
            "affine WhenBad relative retained bytes",
            bytes,
            split.polynomial.owned_retained_byte_bound().ok_or(
                AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                    resource: "affine WhenBad relative retained bytes",
                },
            )?,
        )?;
    }
    for case in cases {
        bytes = checked_add(
            "affine WhenBad relative retained bytes",
            bytes,
            checked_mul(
                "affine WhenBad relative retained bytes",
                case.predicates.capacity(),
                size_of::<AffineWhenBadRelativePredicate>(),
            )?,
        )?;
        for predicate in &case.predicates {
            bytes = checked_add(
                "affine WhenBad relative retained bytes",
                bytes,
                predicate.polynomial.owned_retained_byte_bound().ok_or(
                    AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                        resource: "affine WhenBad relative retained bytes",
                    },
                )?,
            )?;
        }
    }
    Ok(bytes)
}

fn capacity_byte_envelope(
    entries: usize,
    element_size: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    checked_mul(
        "affine WhenBad relative retained bytes",
        checked_mul(
            "affine WhenBad relative retained bytes",
            entries,
            element_size,
        )?,
        2,
    )
}

fn preflight_payload_comparison(
    certificate: &AffineWhenBadRelativePartitionCertificate,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    let (units, bytes, integer_bits) = payload_census(
        &certificate.context_fingerprint,
        &certificate.structural_loci,
        &certificate.inherited_truths,
        &certificate.formula,
        &certificate.splits,
        &certificate.cases,
        &certificate.classifications,
    )?;
    check_limit(
        "affine WhenBad relative payload comparison units",
        units,
        limits.max_payload_comparison_units,
    )?;
    check_limit(
        "affine WhenBad relative payload comparison bytes",
        bytes,
        limits.max_payload_comparison_bytes,
    )?;
    check_limit(
        "affine WhenBad relative payload comparison integer bits",
        integer_bits,
        limits.max_payload_comparison_integer_bits,
    )?;
    if (units, bytes, integer_bits)
        != (
            certificate.stats.payload_comparison_units,
            certificate.stats.payload_comparison_bytes,
            certificate.stats.payload_comparison_integer_bits,
        )
    {
        return Err(AffineWhenBadRelativeCaseError::ReplayMismatch);
    }
    let observed = observed_certificate_owned_byte_bound(
        &certificate.context_fingerprint,
        &certificate.structural_loci,
        &certificate.inherited_truths,
        &certificate.formula,
        &certificate.splits,
        &certificate.cases,
        &certificate.classifications,
    )?;
    check_limit(
        "affine WhenBad relative retained bytes",
        certificate.stats.retained_bytes,
        limits.max_retained_bytes,
    )?;
    if observed > certificate.stats.retained_bytes {
        return Err(
            AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                observed,
                admitted: certificate.stats.retained_bytes,
            },
        );
    }
    Ok(())
}

fn case_index(id: AffineWhenBadRelativeCaseId) -> Result<usize, AffineWhenBadRelativeCaseError> {
    usize::try_from(id.0).map_err(|_| AffineWhenBadRelativeCaseError::CaseIdOverflow)
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| AffineWhenBadRelativeCaseError::ResourceCountOverflow {
        resource: "affine WhenBad relative retained polynomial integer bits",
    })
}

fn bounded_polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, usize> {
    let mut writer = BoundedByteCounter {
        bytes: 0,
        limit,
        overflowed: false,
    };
    if write!(&mut writer, "{}", polynomial.raw()).is_err() {
        return Err(if writer.overflowed {
            usize::MAX
        } else {
            writer.bytes.max(limit.saturating_add(1))
        });
    }
    Ok(writer.bytes)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
    overflowed: bool,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.bytes.checked_add(value.len()) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        self.bytes = requested;
        if requested > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn try_copy_polynomial(
    source: &ParametricPolynomial,
    resource: &'static str,
) -> Result<ParametricPolynomial, AffineWhenBadRelativeCaseError> {
    source.try_copy_authenticated_sparse_payload().map_err(|_| {
        AffineWhenBadRelativeCaseError::AllocationFailure {
            resource,
            requested: source.term_count(),
        }
    })
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, AffineWhenBadRelativeCaseError> {
    let mut output = String::new();
    output.try_reserve_exact(source.len()).map_err(|_| {
        AffineWhenBadRelativeCaseError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    output.push_str(source);
    Ok(output)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    target.try_reserve_exact(additional).map_err(|_| {
        AffineWhenBadRelativeCaseError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    left.checked_add(right)
        .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    left.checked_mul(right)
        .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    if requested > limit {
        Err(AffineWhenBadRelativeCaseError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn scalar_representation_units<T>() -> usize {
    let bytes = size_of::<T>();
    let word = size_of::<usize>();
    bytes / word + usize::from(bytes % word != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoefficientContext;

    fn context(name: &str, index_count: usize) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            name,
            index_count,
        )
        .unwrap()
    }

    fn index_polynomial(
        context: &ParametricCoefficientContext,
        position: usize,
    ) -> ParametricPolynomial {
        context
            .numerator_condition(&context.index(position).unwrap())
            .unwrap()
    }

    fn independent_loci(context: &ParametricCoefficientContext) -> Vec<ParametricPolynomial> {
        vec![index_polynomial(context, 0), index_polynomial(context, 1)]
    }

    fn conjunction_problem(context: &ParametricCoefficientContext) -> AffineWhenBadRelativeProblem {
        AffineWhenBadRelativeProblem::from_preallocated(
            independent_loci(context),
            Vec::new(),
            vec![AffineWhenBadFormulaClause::free_index_leak(17, 0, 1)],
        )
    }

    fn compile_conjunction(
        context: &ParametricCoefficientContext,
        limits: AffineWhenBadRelativeCaseLimits,
    ) -> Result<AffineWhenBadRelativePartitionCertificate, AffineWhenBadRelativeCaseError> {
        AffineWhenBadRelativePartitionCompiler::compile(
            context,
            conjunction_problem(context),
            limits,
        )
    }

    fn opposite(kind: SymbolicPolynomialPredicateKind) -> SymbolicPolynomialPredicateKind {
        match kind {
            SymbolicPolynomialPredicateKind::EqualZero => SymbolicPolynomialPredicateKind::NonZero,
            SymbolicPolynomialPredicateKind::NonZero => SymbolicPolynomialPredicateKind::EqualZero,
        }
    }

    fn assigned(
        truth: DirectBadFormulaTruth,
        atom_kind: SymbolicPolynomialPredicateKind,
    ) -> Option<SymbolicPolynomialPredicateKind> {
        match truth {
            DirectBadFormulaTruth::True => Some(atom_kind),
            DirectBadFormulaTruth::False => Some(opposite(atom_kind)),
            DirectBadFormulaTruth::Unknown => None,
        }
    }

    fn conjunction_route(
        left: DirectBadFormulaTruth,
        right: DirectBadFormulaTruth,
    ) -> DirectBadFormulaRoute<AffineWhenBadAtom> {
        let context = context("affine-relative-truth-table", 2);
        let loci = independent_loci(&context);
        let clause = AffineWhenBadFormulaClause::free_index_leak(5, 0, 1);
        let formula = AffineWhenBadDirectFormula {
            clauses: vec![clause],
            atom_count: 2,
        };
        let mut decisions = vec![None; 2];
        decisions[0] = assigned(left, SymbolicPolynomialPredicateKind::EqualZero);
        decisions[1] = assigned(right, SymbolicPolynomialPredicateKind::NonZero);
        route_formula(
            &context,
            &loci,
            &formula,
            &decisions,
            &mut Vec::new(),
            &mut AffineWhenBadRelativeCaseStats::default(),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn owner_formula_has_the_complete_three_valued_conjunction_table() {
        use DirectBadFormulaRoute::{Bad, Good, Split};
        use DirectBadFormulaTruth::{False, True, Unknown};

        let boundary = AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero);
        let numerator = AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::NonZero);
        for (left, right, expected) in [
            (False, False, Good),
            (False, True, Good),
            (False, Unknown, Good),
            (True, False, Good),
            (True, True, Bad { clause_ordinal: 0 }),
            (
                True,
                Unknown,
                Split {
                    clause_ordinal: 0,
                    atom: numerator,
                },
            ),
            (Unknown, False, Good),
            (
                Unknown,
                True,
                Split {
                    clause_ordinal: 0,
                    atom: boundary,
                },
            ),
            (
                Unknown,
                Unknown,
                Split {
                    clause_ordinal: 0,
                    atom: boundary,
                },
            ),
        ] {
            assert_eq!(conjunction_route(left, right), expected);
        }
    }

    #[test]
    fn later_true_clause_dominates_an_earlier_unknown_with_exact_provenance() {
        let context = context("affine-relative-later-true", 2);
        let loci = independent_loci(&context);
        let formula = AffineWhenBadDirectFormula {
            clauses: vec![
                AffineWhenBadFormulaClause::candidate_required_guard_zero(4, 0),
                AffineWhenBadFormulaClause::coefficient_field_leak_boundary_zero(9, 1),
            ],
            atom_count: 2,
        };
        let decisions = vec![None, Some(SymbolicPolynomialPredicateKind::EqualZero)];
        assert_eq!(
            route_formula(
                &context,
                &loci,
                &formula,
                &decisions,
                &mut Vec::new(),
                &mut AffineWhenBadRelativeCaseStats::default(),
                AffineWhenBadRelativeCaseLimits::default(),
            )
            .unwrap(),
            DirectBadFormulaRoute::Bad { clause_ordinal: 1 },
        );
    }

    #[test]
    fn free_boundary_and_free_numerator_have_exact_three_branch_transcript() {
        let context = context("affine-relative-b-n", 2);
        let certificate =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        certificate.replay(&context).unwrap();

        assert_eq!(certificate.splits().len(), 2);
        let boundary_split = &certificate.splits()[0];
        assert_eq!(boundary_split.parent(), AffineWhenBadRelativeCaseId::ROOT);
        assert_eq!(boundary_split.equal_zero_child().value(), 1);
        assert_eq!(boundary_split.nonzero_child().value(), 2);
        assert_eq!(boundary_split.trigger().clause_ordinal(), 0);
        assert_eq!(boundary_split.trigger().atom().locus_ordinal(), 0);
        let numerator_split = &certificate.splits()[1];
        assert_eq!(numerator_split.parent().value(), 1);
        assert_eq!(numerator_split.equal_zero_child().value(), 3);
        assert_eq!(numerator_split.nonzero_child().value(), 4);
        assert_eq!(numerator_split.trigger().clause_ordinal(), 0);
        assert_eq!(numerator_split.trigger().atom().locus_ordinal(), 1);

        assert_eq!(
            certificate
                .cases()
                .iter()
                .map(|case| case.id().value())
                .collect::<Vec<_>>(),
            [2, 3, 4],
        );
        assert_eq!(
            certificate
                .classifications()
                .iter()
                .map(|entry| (entry.case().value(), entry.disposition()))
                .collect::<Vec<_>>(),
            [
                (2, AffineWhenBadRelativeLeafDisposition::Applicable),
                (3, AffineWhenBadRelativeLeafDisposition::Applicable),
                (
                    4,
                    AffineWhenBadRelativeLeafDisposition::ExceptionalLeak {
                        pullback_ordinal: 17,
                    },
                ),
            ],
        );
        assert_eq!(
            certificate.classifications()[2].decisive_clause_ordinal(),
            Some(0)
        );
        assert_eq!(
            certificate.clause_provenance(0),
            Some(AffineWhenBadClauseProvenance::FreeIndexLeak {
                pullback_ordinal: 17,
            })
        );
    }

    #[test]
    fn atomic_and_whole_target_numerator_gate_forms_route_exactly() {
        let context = context("affine-relative-atomic-gates", 2);
        let coefficient_boundary = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                independent_loci(&context),
                Vec::new(),
                vec![AffineWhenBadFormulaClause::coefficient_field_leak_boundary_zero(3, 0)],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(coefficient_boundary.splits().len(), 1);
        assert_eq!(
            coefficient_boundary.classifications()[0].disposition(),
            AffineWhenBadRelativeLeafDisposition::ExceptionalLeak {
                pullback_ordinal: 3,
            }
        );

        let whole_target = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                independent_loci(&context),
                Vec::new(),
                vec![AffineWhenBadFormulaClause::whole_target_free_index_leak(
                    8, 1,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(whole_target.splits().len(), 1);
        assert_eq!(
            whole_target.clause_provenance(0),
            Some(AffineWhenBadClauseProvenance::WholeTargetFreeIndexLeak {
                pullback_ordinal: 8,
            })
        );
    }

    #[test]
    fn inherited_target_truths_seed_the_root_without_new_predicates() {
        let context = context("affine-relative-inherited", 2);
        let certificate = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                independent_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    0,
                    SymbolicPolynomialPredicateKind::NonZero,
                )],
                vec![AffineWhenBadFormulaClause::candidate_required_guard_zero(
                    11, 0,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        certificate.replay(&context).unwrap();
        assert!(certificate.splits().is_empty());
        assert_eq!(certificate.cases().len(), 1);
        assert!(certificate.cases()[0].predicates().is_empty());
        assert_eq!(
            certificate.classifications()[0].disposition(),
            AffineWhenBadRelativeLeafDisposition::Applicable,
        );
    }

    fn product_loci(context: &ParametricCoefficientContext) -> Vec<ParametricPolynomial> {
        let p = context.index(0).unwrap();
        let q = context.mul(&p, &context.index(1).unwrap()).unwrap();
        vec![
            context.numerator_condition(&p).unwrap(),
            context.numerator_condition(&q).unwrap(),
        ]
    }

    #[test]
    fn principal_divisibility_implications_include_inherited_target_facts() {
        let context = context("affine-relative-divisibility", 2);
        // p=0 and p|q imply q=0, so the candidate-domain failure is true at
        // the root and no redundant q split is retained.
        let zero_implication = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                product_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    0,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )],
                vec![AffineWhenBadFormulaClause::candidate_required_guard_zero(
                    2, 1,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert!(zero_implication.splits().is_empty());
        assert_eq!(
            zero_implication.classifications()[0].disposition(),
            AffineWhenBadRelativeLeafDisposition::ExceptionalDomain {
                condition_ordinal: 2,
            }
        );

        // q!=0 and p|q imply p!=0.
        let nonzero_implication = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                product_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    1,
                    SymbolicPolynomialPredicateKind::NonZero,
                )],
                vec![AffineWhenBadFormulaClause::whole_target_free_index_leak(
                    7, 0,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert!(nonzero_implication.splits().is_empty());
        assert_eq!(
            nonzero_implication.classifications()[0].disposition(),
            AffineWhenBadRelativeLeafDisposition::ExceptionalLeak {
                pullback_ordinal: 7,
            }
        );
        assert!(nonzero_implication.stats().locus_divisibility_checks() > 0);
    }

    #[test]
    fn principal_divisibility_does_not_apply_invalid_converses() {
        let context = context("affine-relative-divisibility-converse", 2);
        let q_zero_does_not_force_p_zero = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                product_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    1,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )],
                vec![AffineWhenBadFormulaClause::candidate_required_guard_zero(
                    0, 0,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(q_zero_does_not_force_p_zero.splits().len(), 1);

        let p_nonzero_does_not_force_q_nonzero = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(
                product_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    0,
                    SymbolicPolynomialPredicateKind::NonZero,
                )],
                vec![AffineWhenBadFormulaClause::whole_target_free_index_leak(
                    0, 1,
                )],
            ),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(p_nonzero_does_not_force_q_nonzero.splits().len(), 1);
    }

    #[test]
    fn associate_loci_fail_closed_at_a_typed_canonicalization_seam() {
        let context = context("affine-relative-associates", 1);
        let p = context.index(0).unwrap();
        let twice_p = context.mul(&context.integer(2), &p).unwrap();
        let problem = || {
            AffineWhenBadRelativeProblem::from_preallocated(
                vec![
                    context.numerator_condition(&p).unwrap(),
                    context.numerator_condition(&twice_p).unwrap(),
                ],
                Vec::new(),
                Vec::new(),
            )
        };
        assert!(matches!(
            AffineWhenBadRelativePartitionCompiler::compile(
                &context,
                problem(),
                AffineWhenBadRelativeCaseLimits::default(),
            ),
            Err(
                AffineWhenBadRelativeCaseError::AssociatedStructuralLocusRequiresCanonicalization {
                    first_ordinal: 0,
                    duplicate_ordinal: 1,
                }
            )
        ));

        let mut no_comparisons = AffineWhenBadRelativeCaseLimits::default();
        no_comparisons.max_structural_locus_associate_comparisons = 0;
        assert_resource(
            AffineWhenBadRelativePartitionCompiler::compile(&context, problem(), no_comparisons)
                .unwrap_err(),
            "affine WhenBad relative structural locus associate comparisons",
        );
    }

    #[test]
    fn monotone_ids_and_complementary_children_conserve_the_relative_root() {
        let context = context("affine-relative-conservation", 2);
        let first =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        let second = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            first.try_copy_problem().unwrap(),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cases().len(), first.splits().len() + 1);
        for split in first.splits() {
            assert!(split.equal_zero_child().value() < split.nonzero_child().value());
            assert_eq!(
                split.nonzero_child().value(),
                split.equal_zero_child().value() + 1
            );
            assert!(first.cases().iter().all(|case| case.id() != split.parent()));
        }
    }

    #[test]
    fn retained_envelope_is_replay_deterministic_for_spare_capacity_and_large_integers() {
        let context = context("affine-relative-capacity-determinism", 2);
        let mut huge = context.integer(2);
        for _ in 0..12 {
            huge = context.mul(&huge, &huge).unwrap();
        }
        let shifted = context.add(&context.index(0).unwrap(), &huge).unwrap();

        let mut loci = Vec::with_capacity(128);
        loci.push(context.numerator_condition(&shifted).unwrap());
        loci.push(index_polynomial(&context, 1));
        let mut inherited = Vec::with_capacity(64);
        inherited.push(AffineWhenBadInheritedTruth::new(
            1,
            SymbolicPolynomialPredicateKind::NonZero,
        ));
        let mut clauses = Vec::with_capacity(96);
        clauses.push(AffineWhenBadFormulaClause::candidate_required_guard_zero(
            3, 0,
        ));
        let certificate = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadRelativeProblem::from_preallocated(loci, inherited, clauses),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(
            certificate.structural_loci()[0]
                .raw()
                .coefficients
                .iter()
                .filter(|coefficient| matches!(coefficient, Integer::Large(_)))
                .count(),
            1,
        );
        certificate.replay(&context).unwrap();
        let rebuilt = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            certificate.try_copy_problem().unwrap(),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(
            certificate.stats().retained_bytes(),
            rebuilt.stats().retained_bytes()
        );
        assert_eq!(certificate, rebuilt);
    }

    #[test]
    fn leaf_classification_obligation_is_admitted_before_root_or_split_mutation() {
        let context = context("affine-relative-leaf-obligation", 2);
        let mut no_root = AffineWhenBadRelativeCaseLimits::default();
        no_root.max_leaf_classifications = 0;
        assert_resource(
            compile_conjunction(&context, no_root).unwrap_err(),
            "affine WhenBad relative leaf classifications",
        );

        let mut no_split_child = AffineWhenBadRelativeCaseLimits::default();
        no_split_child.max_leaf_classifications = 1;
        assert_resource(
            compile_conjunction(&context, no_split_child).unwrap_err(),
            "affine WhenBad relative leaf classifications",
        );
    }

    fn exact_limits(stats: AffineWhenBadRelativeCaseStats) -> AffineWhenBadRelativeCaseLimits {
        let mut limits = AffineWhenBadRelativeCaseLimits::default();
        limits.max_context_fingerprint_bytes = stats.context_fingerprint_bytes();
        limits.max_structural_loci = stats.structural_loci();
        limits.max_structural_locus_equality_comparisons =
            stats.structural_locus_equality_comparisons();
        limits.max_structural_locus_associate_comparisons =
            stats.structural_locus_associate_comparisons();
        limits.max_structural_locus_associate_term_pairs =
            stats.structural_locus_associate_term_pairs();
        limits.max_inherited_truths = stats.inherited_truths();
        limits.max_bad_clauses = stats.bad_clauses();
        limits.max_bad_atoms = stats.bad_atoms();
        limits.max_direct_bad_formula_evaluations = stats.direct_bad_formula_evaluations();
        limits.max_direct_bad_formula_clause_visits = stats.direct_bad_formula_clause_visits();
        limits.max_direct_bad_formula_atom_truth_queries =
            stats.direct_bad_formula_atom_truth_queries();
        limits.max_splits = stats.splits();
        limits.max_live_leaves = stats.live_leaves();
        limits.max_case_ids = stats.case_ids();
        limits.max_predicates_per_case = stats.maximum_predicates_per_case();
        limits.max_predicate_instances = stats.predicate_instances();
        limits.max_leaf_classifications = stats.leaf_classifications();
        limits.max_work_decision_cells = stats.work_decision_cells();
        limits.max_locus_divisibility_checks = stats.locus_divisibility_checks();
        limits.max_locus_divisibility_term_pairs = stats.locus_divisibility_term_pairs();
        limits.max_locus_divisibility_cache_entries = stats.locus_divisibility_cache_entries();
        limits.max_retained_polynomial_terms = stats.retained_polynomial_terms();
        limits.max_retained_polynomial_exponent_entries =
            stats.retained_polynomial_exponent_entries();
        limits.max_retained_polynomial_integer_bits = stats.retained_polynomial_integer_bits();
        limits.max_retained_polynomial_display_bytes = stats.retained_polynomial_display_bytes();
        limits.max_retained_bytes = stats.retained_bytes();
        limits.max_payload_comparison_units = stats.payload_comparison_units();
        limits.max_payload_comparison_bytes = stats.payload_comparison_bytes();
        limits.max_payload_comparison_integer_bits = stats.payload_comparison_integer_bits();
        limits
    }

    fn assert_resource(error: AffineWhenBadRelativeCaseError, resource: &'static str) {
        assert!(
            matches!(
                error,
                AffineWhenBadRelativeCaseError::ResourceLimit {
                    resource: actual,
                    ..
                } if actual == resource
            ),
            "expected resource {resource}, got {error:?}"
        );
    }

    #[test]
    fn exact_measured_limits_replay_and_representative_one_below_limits_reject() {
        let context = context("affine-relative-exact-limits", 2);
        let baseline =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        let exact = exact_limits(baseline.stats());
        let bounded = compile_conjunction(&context, exact).unwrap();
        bounded.replay(&context).unwrap();

        type Setter = fn(&mut AffineWhenBadRelativeCaseLimits, usize);
        let probes: [(&str, usize, Setter); 25] = [
            (
                "affine WhenBad relative context fingerprint bytes",
                baseline.stats().context_fingerprint_bytes(),
                |limits, value| limits.max_context_fingerprint_bytes = value,
            ),
            (
                "affine WhenBad relative structural loci",
                baseline.stats().structural_loci(),
                |limits, value| limits.max_structural_loci = value,
            ),
            (
                "affine WhenBad relative structural locus equality comparisons",
                baseline.stats().structural_locus_equality_comparisons(),
                |limits, value| limits.max_structural_locus_equality_comparisons = value,
            ),
            (
                "affine WhenBad relative structural locus associate comparisons",
                baseline.stats().structural_locus_associate_comparisons(),
                |limits, value| limits.max_structural_locus_associate_comparisons = value,
            ),
            (
                "affine WhenBad relative structural locus associate term pairs",
                baseline.stats().structural_locus_associate_term_pairs(),
                |limits, value| limits.max_structural_locus_associate_term_pairs = value,
            ),
            (
                "affine WhenBad relative bad clauses",
                baseline.stats().bad_clauses(),
                |limits, value| limits.max_bad_clauses = value,
            ),
            (
                "affine WhenBad relative bad atoms",
                baseline.stats().bad_atoms(),
                |limits, value| limits.max_bad_atoms = value,
            ),
            (
                "affine WhenBad relative direct bad-formula evaluations",
                baseline.stats().direct_bad_formula_evaluations(),
                |limits, value| limits.max_direct_bad_formula_evaluations = value,
            ),
            (
                "affine WhenBad relative direct bad-formula clause visits",
                baseline.stats().direct_bad_formula_clause_visits(),
                |limits, value| limits.max_direct_bad_formula_clause_visits = value,
            ),
            (
                "affine WhenBad relative direct bad-formula atom truth queries",
                baseline.stats().direct_bad_formula_atom_truth_queries(),
                |limits, value| limits.max_direct_bad_formula_atom_truth_queries = value,
            ),
            (
                "affine WhenBad relative splits",
                baseline.stats().splits(),
                |limits, value| limits.max_splits = value,
            ),
            (
                "affine WhenBad relative live leaves",
                baseline.stats().live_leaves(),
                |limits, value| limits.max_live_leaves = value,
            ),
            (
                "affine WhenBad relative case identifiers",
                baseline.stats().case_ids(),
                |limits, value| limits.max_case_ids = value,
            ),
            (
                "affine WhenBad relative predicates per case",
                baseline.stats().maximum_predicates_per_case(),
                |limits, value| limits.max_predicates_per_case = value,
            ),
            (
                "affine WhenBad relative predicate instances",
                baseline.stats().predicate_instances(),
                |limits, value| limits.max_predicate_instances = value,
            ),
            (
                "affine WhenBad relative leaf classifications",
                baseline.stats().leaf_classifications(),
                |limits, value| limits.max_leaf_classifications = value,
            ),
            (
                "affine WhenBad relative work decision cells",
                baseline.stats().work_decision_cells(),
                |limits, value| limits.max_work_decision_cells = value,
            ),
            (
                "affine WhenBad relative retained polynomial terms",
                baseline.stats().retained_polynomial_terms(),
                |limits, value| limits.max_retained_polynomial_terms = value,
            ),
            (
                "affine WhenBad relative retained polynomial exponent entries",
                baseline.stats().retained_polynomial_exponent_entries(),
                |limits, value| limits.max_retained_polynomial_exponent_entries = value,
            ),
            (
                "affine WhenBad relative retained polynomial integer bits",
                baseline.stats().retained_polynomial_integer_bits(),
                |limits, value| limits.max_retained_polynomial_integer_bits = value,
            ),
            (
                "affine WhenBad relative retained polynomial display bytes",
                baseline.stats().retained_polynomial_display_bytes(),
                |limits, value| limits.max_retained_polynomial_display_bytes = value,
            ),
            (
                "affine WhenBad relative retained bytes",
                baseline.stats().retained_bytes(),
                |limits, value| limits.max_retained_bytes = value,
            ),
            (
                "affine WhenBad relative payload comparison units",
                baseline.stats().payload_comparison_units(),
                |limits, value| limits.max_payload_comparison_units = value,
            ),
            (
                "affine WhenBad relative payload comparison bytes",
                baseline.stats().payload_comparison_bytes(),
                |limits, value| limits.max_payload_comparison_bytes = value,
            ),
            (
                "affine WhenBad relative payload comparison integer bits",
                baseline.stats().payload_comparison_integer_bits(),
                |limits, value| limits.max_payload_comparison_integer_bits = value,
            ),
        ];
        for (resource, observed, set) in probes {
            assert!(observed > 0, "fixture must exercise {resource}");
            let mut one_below = exact;
            set(&mut one_below, observed - 1);
            assert_resource(
                compile_conjunction(&context, one_below).unwrap_err(),
                resource,
            );
        }
    }

    #[test]
    fn divisibility_limits_are_transactional_and_exact() {
        let context = context("affine-relative-divisibility-limits", 2);
        let problem = || {
            AffineWhenBadRelativeProblem::from_preallocated(
                product_loci(&context),
                vec![AffineWhenBadInheritedTruth::new(
                    0,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )],
                vec![AffineWhenBadFormulaClause::candidate_required_guard_zero(
                    0, 1,
                )],
            )
        };
        let baseline = AffineWhenBadRelativePartitionCompiler::compile(
            &context,
            problem(),
            AffineWhenBadRelativeCaseLimits::default(),
        )
        .unwrap();
        assert_eq!(baseline.stats().locus_divisibility_checks(), 1);
        assert_eq!(baseline.stats().locus_divisibility_cache_entries(), 1);

        for (resource, set) in [
            (
                "affine WhenBad relative locus divisibility checks",
                (|limits: &mut AffineWhenBadRelativeCaseLimits| {
                    limits.max_locus_divisibility_checks = 0
                }) as fn(&mut AffineWhenBadRelativeCaseLimits),
            ),
            (
                "affine WhenBad relative locus divisibility term pairs",
                |limits: &mut AffineWhenBadRelativeCaseLimits| {
                    limits.max_locus_divisibility_term_pairs = 0
                },
            ),
            (
                "affine WhenBad relative locus divisibility cache entries",
                |limits: &mut AffineWhenBadRelativeCaseLimits| {
                    limits.max_locus_divisibility_cache_entries = 0
                },
            ),
        ] {
            let mut limits = AffineWhenBadRelativeCaseLimits::default();
            set(&mut limits);
            assert_resource(
                AffineWhenBadRelativePartitionCompiler::compile(&context, problem(), limits)
                    .unwrap_err(),
                resource,
            );
        }
    }

    #[test]
    fn tampered_clause_and_split_provenance_do_not_replay() {
        let context = context("affine-relative-tamper", 2);
        let mut certificate =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        certificate.splits[0].trigger.clause_ordinal = usize::MAX;
        assert!(matches!(
            certificate.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));

        let mut clause =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        clause.formula.clauses[0] =
            AffineWhenBadFormulaClause::candidate_required_guard_zero(99, 0);
        assert!(matches!(
            clause.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));

        let mut understated_census =
            compile_conjunction(&context, AffineWhenBadRelativeCaseLimits::default()).unwrap();
        understated_census.stats.payload_comparison_units -= 1;
        assert!(matches!(
            understated_census.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));
    }
}
