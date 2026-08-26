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
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;

use crate::canonical_parametric_locus_table::{
    CanonicalLocusTableCopyLimits, CanonicalLocusTableError, CanonicalLocusTableOwner,
};
use crate::direct_bad_formula::{
    DirectBadFormulaClause, DirectBadFormulaRoute, DirectBadFormulaTruth, route_direct_bad_formula,
};
use crate::direct_bad_formula_arbitrary::{
    ArbitraryDirectBadFormula, ArbitraryDirectBadFormulaError, ArbitraryDirectBadFormulaLimits,
    ArbitraryDirectBadFormulaRoute, ArbitraryDirectBadFormulaTruth,
};
use crate::parametric_coefficient::ParametricPolynomialAssociateLimits;
use crate::{
    ExactAlgebraLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial, SymbolicPolynomialPredicateKind,
};

#[cfg(test)]
thread_local! {
    static ARBITRARY_PARTITION_COMPILE_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_REPLAY_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_CONTEXT_COPY_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_CANONICAL_COPY_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_INHERITED_VALIDATION_RESERVE_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_INHERITED_COPY_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_FORMULA_BOX_RESERVE_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_DIVISIBILITY_CACHE_RESERVE_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static ARBITRARY_PARTITION_REPLAY_PROBLEM_COPY_STAGE_FOR_TEST: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static AUTHENTICATED_ARBITRARY_PARTITION_POST_VALIDATION_PANIC_FOR_TEST:
        std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AUTHENTICATED_ARBITRARY_PARTITION_LINEAR_VALIDATIONS_FOR_TEST:
        std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn inject_arbitrary_partition_compile_panic_for_test() {
    ARBITRARY_PARTITION_COMPILE_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn inject_arbitrary_partition_replay_panic_for_test() {
    ARBITRARY_PARTITION_REPLAY_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn inject_authenticated_arbitrary_partition_post_validation_panic_for_test() {
    AUTHENTICATED_ARBITRARY_PARTITION_POST_VALIDATION_PANIC_FOR_TEST
        .with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_authenticated_arbitrary_partition_post_validation_panic_for_test() {
    AUTHENTICATED_ARBITRARY_PARTITION_POST_VALIDATION_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected authenticated arbitrary relative partition post-validation panic");
        }
    });
}

#[cfg(test)]
fn reset_authenticated_arbitrary_partition_linear_validations_for_test() {
    AUTHENTICATED_ARBITRARY_PARTITION_LINEAR_VALIDATIONS_FOR_TEST
        .with(|validations| validations.set(0));
}

#[cfg(test)]
fn authenticated_arbitrary_partition_linear_validations_for_test() -> usize {
    AUTHENTICATED_ARBITRARY_PARTITION_LINEAR_VALIDATIONS_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn mark_authenticated_arbitrary_partition_linear_validation_for_test() {
    AUTHENTICATED_ARBITRARY_PARTITION_LINEAR_VALIDATIONS_FOR_TEST.with(|validations| {
        validations.set(validations.get().checked_add(1).unwrap_or(usize::MAX));
    });
}

#[cfg(test)]
fn maybe_inject_arbitrary_partition_compile_panic_for_test() {
    ARBITRARY_PARTITION_COMPILE_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected arbitrary relative partition compile panic");
        }
    });
}

#[cfg(test)]
fn maybe_inject_arbitrary_partition_replay_panic_for_test() {
    ARBITRARY_PARTITION_REPLAY_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected arbitrary relative partition replay panic");
        }
    });
}

#[cfg(test)]
fn reset_arbitrary_partition_reserve_observations_for_test() {
    ARBITRARY_PARTITION_CONTEXT_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
    ARBITRARY_PARTITION_CANONICAL_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
    ARBITRARY_PARTITION_INHERITED_VALIDATION_RESERVE_OBSERVED_FOR_TEST
        .with(|observed| observed.set(false));
    ARBITRARY_PARTITION_INHERITED_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
    ARBITRARY_PARTITION_FORMULA_BOX_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
    ARBITRARY_PARTITION_DIVISIBILITY_CACHE_RESERVE_OBSERVED_FOR_TEST
        .with(|observed| observed.set(false));
}

#[cfg(test)]
fn arbitrary_partition_reserve_observations_for_test() -> (bool, bool, bool, bool, bool, bool) {
    (
        ARBITRARY_PARTITION_CONTEXT_COPY_OBSERVED_FOR_TEST.with(std::cell::Cell::get),
        ARBITRARY_PARTITION_CANONICAL_COPY_OBSERVED_FOR_TEST.with(std::cell::Cell::get),
        ARBITRARY_PARTITION_INHERITED_VALIDATION_RESERVE_OBSERVED_FOR_TEST
            .with(std::cell::Cell::get),
        ARBITRARY_PARTITION_INHERITED_COPY_OBSERVED_FOR_TEST.with(std::cell::Cell::get),
        ARBITRARY_PARTITION_FORMULA_BOX_RESERVE_OBSERVED_FOR_TEST.with(std::cell::Cell::get),
        ARBITRARY_PARTITION_DIVISIBILITY_CACHE_RESERVE_OBSERVED_FOR_TEST.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
fn mark_arbitrary_partition_context_copy_observed_for_test() {
    ARBITRARY_PARTITION_CONTEXT_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(test)]
fn mark_arbitrary_partition_canonical_copy_observed_for_test() {
    ARBITRARY_PARTITION_CANONICAL_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(test)]
fn mark_arbitrary_partition_inherited_validation_reserve_observed_for_test() {
    ARBITRARY_PARTITION_INHERITED_VALIDATION_RESERVE_OBSERVED_FOR_TEST
        .with(|observed| observed.set(true));
}

#[cfg(test)]
fn mark_arbitrary_partition_inherited_copy_observed_for_test() {
    ARBITRARY_PARTITION_INHERITED_COPY_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(test)]
fn mark_arbitrary_partition_formula_box_reserve_observed_for_test() {
    ARBITRARY_PARTITION_FORMULA_BOX_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(test)]
fn mark_arbitrary_partition_divisibility_cache_reserve_observed_for_test() {
    ARBITRARY_PARTITION_DIVISIBILITY_CACHE_RESERVE_OBSERVED_FOR_TEST
        .with(|observed| observed.set(true));
}

#[cfg(test)]
fn reset_arbitrary_partition_replay_problem_copy_stage_for_test() {
    ARBITRARY_PARTITION_REPLAY_PROBLEM_COPY_STAGE_FOR_TEST.with(|stage| stage.set(0));
}

#[cfg(test)]
fn arbitrary_partition_replay_problem_copy_stage_for_test() -> usize {
    ARBITRARY_PARTITION_REPLAY_PROBLEM_COPY_STAGE_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn mark_arbitrary_partition_replay_problem_copy_stage_for_test(stage: usize) {
    ARBITRARY_PARTITION_REPLAY_PROBLEM_COPY_STAGE_FOR_TEST.with(|observed| {
        debug_assert_eq!(observed.get() + 1, stage);
        observed.set(stage);
    });
}

#[cfg(not(test))]
fn maybe_inject_arbitrary_partition_compile_panic_for_test() {}

#[cfg(not(test))]
fn maybe_inject_arbitrary_partition_replay_panic_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_context_copy_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_canonical_copy_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_inherited_validation_reserve_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_inherited_copy_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_formula_box_reserve_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_divisibility_cache_reserve_observed_for_test() {}

#[cfg(not(test))]
fn mark_arbitrary_partition_replay_problem_copy_stage_for_test(_stage: usize) {}

#[cfg(not(test))]
fn reset_arbitrary_partition_replay_problem_copy_stage_for_test() {}

#[cfg(not(test))]
fn maybe_inject_authenticated_arbitrary_partition_post_validation_panic_for_test() {}

#[cfg(not(test))]
fn reset_authenticated_arbitrary_partition_linear_validations_for_test() {}

#[cfg(not(test))]
fn mark_authenticated_arbitrary_partition_linear_validation_for_test() {}

/// Stable schema for the target-relative structural partition core.
pub const AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA: &str =
    "rustred-affine-when-bad-relative-partition-v1";

/// Stable schema for the owner-neutral arbitrary-width partition seam.
pub(crate) const AFFINE_WHEN_BAD_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA: &str =
    "rustred-affine-when-bad-arbitrary-relative-partition-v1";

/// Stable schema for the arbitrary-width seam when pairwise locus
/// canonicality is carried by an opaque outer owner.
///
/// Keeping this distinct from the raw-input schema prevents a certificate
/// from silently changing authority kind while retaining a V1 label.  The
/// raw V1 compiler and its resource transcript remain byte-for-byte
/// unchanged.
pub(crate) const AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA: &str =
    "rustred-affine-when-bad-authenticated-arbitrary-relative-partition-v1";

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

/// Additional enforceable resources for the crate-private arbitrary seam.
/// Exported V1 limits remain byte-for-byte unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeLimits {
    pub(crate) relative: AffineWhenBadRelativeCaseLimits,
    pub(crate) max_source_problem_owned_logical_bytes: usize,
    pub(crate) max_formula_retained_owned_logical_bytes: usize,
    pub(crate) max_formula_compilation_owned_logical_peak_upper_bound: usize,
    pub(crate) max_work_owned_logical_peak_upper_bound: usize,
    pub(crate) max_compiler_owned_logical_peak_upper_bound: usize,
    pub(crate) max_compilation_owned_logical_peak_upper_bound: usize,
}

impl Default for AffineWhenBadArbitraryRelativeLimits {
    fn default() -> Self {
        Self {
            relative: AffineWhenBadRelativeCaseLimits::default(),
            max_source_problem_owned_logical_bytes: usize::MAX,
            max_formula_retained_owned_logical_bytes: usize::MAX,
            max_formula_compilation_owned_logical_peak_upper_bound: usize::MAX,
            max_work_owned_logical_peak_upper_bound: usize::MAX,
            max_compiler_owned_logical_peak_upper_bound: usize::MAX,
            max_compilation_owned_logical_peak_upper_bound: usize::MAX,
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

/// Exact source-neutral occurrence which caused one arbitrary-width split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeSplitTrigger {
    clause_ordinal: usize,
    clause_atom_ordinal: usize,
    atom_ordinal: usize,
    atom: AffineWhenBadAtom,
}

impl AffineWhenBadArbitraryRelativeSplitTrigger {
    pub(crate) const fn clause_ordinal(self) -> usize {
        self.clause_ordinal
    }

    pub(crate) const fn clause_atom_ordinal(self) -> usize {
        self.clause_atom_ordinal
    }

    pub(crate) const fn atom_ordinal(self) -> usize {
        self.atom_ordinal
    }

    pub(crate) const fn atom(self) -> AffineWhenBadAtom {
        self.atom
    }
}

/// One deterministic complementary refinement in the arbitrary-width core.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeSplit {
    ordinal: usize,
    parent: AffineWhenBadRelativeCaseId,
    trigger: AffineWhenBadArbitraryRelativeSplitTrigger,
    equal_zero_child: AffineWhenBadRelativeCaseId,
    nonzero_child: AffineWhenBadRelativeCaseId,
}

impl AffineWhenBadArbitraryRelativeSplit {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn parent(&self) -> AffineWhenBadRelativeCaseId {
        self.parent
    }

    pub(crate) const fn trigger(&self) -> AffineWhenBadArbitraryRelativeSplitTrigger {
        self.trigger
    }

    pub(crate) const fn equal_zero_child(&self) -> AffineWhenBadRelativeCaseId {
        self.equal_zero_child
    }

    pub(crate) const fn nonzero_child(&self) -> AffineWhenBadRelativeCaseId {
        self.nonzero_child
    }
}

/// One table-indexed predicate retained by the arbitrary-width core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativePredicate {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
}

impl AffineWhenBadArbitraryRelativePredicate {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }
}

/// One final, table-indexed conjunction in the arbitrary-width core.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeCase {
    id: AffineWhenBadRelativeCaseId,
    predicates: Vec<AffineWhenBadArbitraryRelativePredicate>,
}

impl AffineWhenBadArbitraryRelativeCase {
    pub(crate) const fn id(&self) -> AffineWhenBadRelativeCaseId {
        self.id
    }

    pub(crate) fn predicates(&self) -> &[AffineWhenBadArbitraryRelativePredicate] {
        &self.predicates
    }

    pub(crate) fn predicate_capacity(&self) -> usize {
        self.predicates.capacity()
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

/// Provenance-blind disposition of one leaf in the arbitrary-width core.
///
/// `None` means the bad formula is false on the leaf.  `Some(i)` means clause
/// `i` is the first decisive true clause under deterministic formula order.
/// The authenticated outer owner, not this core, interprets that ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeLeafClassification {
    case: AffineWhenBadRelativeCaseId,
    decisive_clause_ordinal: Option<usize>,
}

impl AffineWhenBadArbitraryRelativeLeafClassification {
    pub(crate) const fn case(&self) -> AffineWhenBadRelativeCaseId {
        self.case
    }

    pub(crate) const fn decisive_clause_ordinal(&self) -> Option<usize> {
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

/// Deterministic compilation-memory census for the arbitrary-width seam.
///
/// This is separate from the exported V1 stats so the compatibility schema
/// and its exact resource transcript remain unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AffineWhenBadArbitraryRelativeCompilationStats {
    source_problem_owned_logical_byte_envelope: usize,
    formula_retained_owned_logical_bytes: usize,
    formula_compilation_owned_logical_peak_upper_bound: usize,
    work_owned_logical_peak_upper_bound: usize,
    compiler_owned_logical_peak_upper_bound: usize,
}

impl AffineWhenBadArbitraryRelativeCompilationStats {
    pub(crate) const fn source_problem_owned_logical_byte_envelope(self) -> usize {
        self.source_problem_owned_logical_byte_envelope
    }

    pub(crate) const fn formula_retained_owned_logical_bytes(self) -> usize {
        self.formula_retained_owned_logical_bytes
    }

    pub(crate) const fn formula_compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.formula_compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn work_owned_logical_peak_upper_bound(self) -> usize {
        self.work_owned_logical_peak_upper_bound
    }

    /// Peak newly owned by the compiler, excluding the caller-owned source
    /// problem returned by `retained_owned_logical_byte_bound`.
    pub(crate) const fn compiler_owned_logical_peak_upper_bound(self) -> usize {
        self.compiler_owned_logical_peak_upper_bound
    }
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

/// Replayable, provenance-neutral arbitrary-width partition transcript.
///
/// This certificate owns only canonical loci, inherited truths, the flattened
/// OR-of-AND formula, and structural routing results.  Semantic source
/// locators remain exclusively owned by the caller.
enum AffineWhenBadArbitraryCanonicalLoci {
    /// Defensive source-neutral input. Replay must repeat the complete
    /// pairwise equality/associate validation.
    Raw(Vec<ParametricPolynomial>),
    /// Opaque proof that a bounded outer canonicalizer has already completed
    /// that pairwise scan. Replay preserves this authority and performs only
    /// linear authentication, census, and a bounded sparse-payload copy.
    Authenticated {
        expected_schema: &'static str,
        owner: CanonicalLocusTableOwner,
    },
}

impl AffineWhenBadArbitraryCanonicalLoci {
    fn loci(&self) -> &[ParametricPolynomial] {
        match self {
            Self::Raw(loci) => loci,
            Self::Authenticated { owner, .. } => owner.loci(),
        }
    }

    const fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    fn semantic_payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Raw(left), Self::Raw(right)) => left == right,
            (
                Self::Authenticated {
                    expected_schema: left_schema,
                    owner: left,
                },
                Self::Authenticated {
                    expected_schema: right_schema,
                    owner: right,
                },
            ) => left_schema == right_schema && left.loci() == right.loci(),
            _ => false,
        }
    }
}

pub(crate) struct AffineWhenBadArbitraryRelativePartitionCertificate {
    schema: &'static str,
    context_fingerprint: String,
    canonical_loci: AffineWhenBadArbitraryCanonicalLoci,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    formula: ArbitraryDirectBadFormula<AffineWhenBadAtom>,
    splits: Vec<AffineWhenBadArbitraryRelativeSplit>,
    cases: Vec<AffineWhenBadArbitraryRelativeCase>,
    classifications: Vec<AffineWhenBadArbitraryRelativeLeafClassification>,
    limits: AffineWhenBadArbitraryRelativeLimits,
    stats: AffineWhenBadRelativeCaseStats,
    compilation_stats: AffineWhenBadArbitraryRelativeCompilationStats,
}

impl AffineWhenBadArbitraryRelativePartitionCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub(crate) fn structural_loci(&self) -> &[ParametricPolynomial] {
        self.canonical_loci.loci()
    }

    pub(crate) fn structural_loci_capacity(&self) -> usize {
        match &self.canonical_loci {
            AffineWhenBadArbitraryCanonicalLoci::Raw(loci) => loci.capacity(),
            AffineWhenBadArbitraryCanonicalLoci::Authenticated { owner, .. } => {
                owner.loci_capacity()
            }
        }
    }

    pub(crate) fn inherited_truths(&self) -> &[AffineWhenBadInheritedTruth] {
        &self.inherited_truths
    }

    pub(crate) fn atoms(&self) -> &[AffineWhenBadAtom] {
        self.formula.atoms()
    }

    pub(crate) fn clause_count(&self) -> usize {
        self.formula.clause_count()
    }

    pub(crate) fn clause_range(&self, clause_ordinal: usize) -> Option<Range<usize>> {
        self.formula.clause_range(clause_ordinal)
    }

    pub(crate) fn splits(&self) -> &[AffineWhenBadArbitraryRelativeSplit] {
        &self.splits
    }

    pub(crate) fn cases(&self) -> &[AffineWhenBadArbitraryRelativeCase] {
        &self.cases
    }

    pub(crate) fn cases_capacity(&self) -> usize {
        self.cases.capacity()
    }

    pub(crate) fn classifications(&self) -> &[AffineWhenBadArbitraryRelativeLeafClassification] {
        &self.classifications
    }

    pub(crate) const fn limits(&self) -> AffineWhenBadArbitraryRelativeLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> AffineWhenBadRelativeCaseStats {
        self.stats
    }

    pub(crate) const fn compilation_stats(&self) -> AffineWhenBadArbitraryRelativeCompilationStats {
        self.compilation_stats
    }

    pub(crate) fn case(
        &self,
        id: AffineWhenBadRelativeCaseId,
    ) -> Option<&AffineWhenBadArbitraryRelativeCase> {
        self.cases.iter().find(|case| case.id == id)
    }

    /// Discard derivation-only formula, split, and replay state while moving
    /// the canonical loci and final cases into the compact application owner.
    pub(crate) fn into_application_parts(
        self,
    ) -> (
        Vec<ParametricPolynomial>,
        Vec<AffineWhenBadArbitraryRelativeCase>,
    ) {
        let loci = match self.canonical_loci {
            AffineWhenBadArbitraryCanonicalLoci::Raw(loci) => loci,
            AffineWhenBadArbitraryCanonicalLoci::Authenticated { owner, .. } => owner.into_loci(),
        };
        (loci, self.cases)
    }

    pub(crate) fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineWhenBadRelativeCaseError> {
        catch_unwind(AssertUnwindSafe(|| self.replay_inner(context))).map_err(|_| {
            AffineWhenBadRelativeCaseError::SymbolicaPanic {
                stage: "arbitrary relative partition replay",
            }
        })?
    }

    fn replay_inner(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineWhenBadRelativeCaseError> {
        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        maybe_inject_arbitrary_partition_replay_panic_for_test();
        let expected_certificate_schema = match &self.canonical_loci {
            AffineWhenBadArbitraryCanonicalLoci::Raw(_) => {
                AFFINE_WHEN_BAD_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA
            }
            AffineWhenBadArbitraryCanonicalLoci::Authenticated {
                expected_schema,
                owner,
            } => {
                if owner.schema() != *expected_schema {
                    return Err(AffineWhenBadRelativeCaseError::SchemaMismatch);
                }
                if owner.context_fingerprint() != context.fingerprint() {
                    return Err(AffineWhenBadRelativeCaseError::ContextMismatch);
                }
                AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA
            }
        };
        if self.schema != expected_certificate_schema {
            return Err(AffineWhenBadRelativeCaseError::SchemaMismatch);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(AffineWhenBadRelativeCaseError::ContextMismatch);
        }
        self.formula
            .validate_payload()
            .map_err(map_arbitrary_formula_error)?;
        preflight_arbitrary_payload_comparison(self, self.limits.relative)?;
        let replay_source_problem_owned_logical_byte_envelope = match &self.canonical_loci {
            AffineWhenBadArbitraryCanonicalLoci::Raw(_) => {
                arbitrary_replay_source_problem_owned_logical_byte_envelope(
                    self.structural_loci(),
                    self.inherited_truths.len(),
                    self.formula.atoms().len(),
                    self.formula.clause_count(),
                )?
            }
            AffineWhenBadArbitraryCanonicalLoci::Authenticated {
                expected_schema: _,
                owner,
            } => {
                authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope_from_parts(
                    owner,
                    self.inherited_truths.len(),
                    self.formula.atoms().len(),
                    self.formula.clause_count(),
                )?
            }
        };
        if replay_source_problem_owned_logical_byte_envelope
            != self
                .compilation_stats
                .source_problem_owned_logical_byte_envelope
        {
            return Err(AffineWhenBadRelativeCaseError::ReplayMismatch);
        }
        check_limit(
            "affine WhenBad arbitrary source problem owned logical bytes",
            replay_source_problem_owned_logical_byte_envelope,
            self.limits.max_source_problem_owned_logical_bytes,
        )?;
        check_limit(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            self.compilation_stats
                .compiler_owned_logical_peak_upper_bound,
            self.limits.max_compiler_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            checked_add(
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                replay_source_problem_owned_logical_byte_envelope,
                self.compilation_stats
                    .compiler_owned_logical_peak_upper_bound,
            )?,
            self.limits.max_compilation_owned_logical_peak_upper_bound,
        )?;
        let rebuilt = match &self.canonical_loci {
            AffineWhenBadArbitraryCanonicalLoci::Raw(_) => {
                let problem =
                    self.try_copy_raw_problem(replay_source_problem_owned_logical_byte_envelope)?;
                AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                    context,
                    problem,
                    self.limits,
                )?
            }
            AffineWhenBadArbitraryCanonicalLoci::Authenticated { .. } => {
                let problem = self.try_copy_authenticated_problem(
                    context,
                    replay_source_problem_owned_logical_byte_envelope,
                )?;
                match AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
                    context,
                    problem,
                    self.limits,
                ) {
                    Ok(certificate) => certificate,
                    Err(failure) => return Err(failure.into_parts().0),
                }
            }
        };
        preflight_arbitrary_payload_comparison(&rebuilt, self.limits.relative)?;
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        }
    }

    fn try_copy_raw_problem(
        &self,
        admitted_owned_logical_byte_envelope: usize,
    ) -> Result<AffineWhenBadArbitraryRelativeProblem, AffineWhenBadRelativeCaseError> {
        let AffineWhenBadArbitraryCanonicalLoci::Raw(source_structural_loci) = &self.canonical_loci
        else {
            return Err(AffineWhenBadRelativeCaseError::ReplayMismatch);
        };
        let mut replay_copy_census = AffineWhenBadRelativeCaseStats::default();
        replay_copy_census.retained_bytes = capacity_byte_envelope(
            source_structural_loci.len(),
            size_of::<ParametricPolynomial>(),
        )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            replay_copy_census.retained_bytes,
            self.limits.relative.max_retained_bytes,
        )?;
        for polynomial in source_structural_loci {
            charge_retained_polynomial(polynomial, &mut replay_copy_census, self.limits.relative)?;
        }
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(1);
        let structural_loci = try_canonicalize_structural_loci(source_structural_loci)?;
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(2);
        let inherited_truths = try_canonicalize_inherited_truths(&self.inherited_truths)?;
        let mut atoms = Vec::new();
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(3);
        try_reserve_exact(
            "affine WhenBad arbitrary replay atoms",
            &mut atoms,
            self.formula.atoms().len(),
        )?;
        atoms.extend_from_slice(self.formula.atoms());
        let mut clause_ranges = Vec::new();
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(4);
        try_reserve_exact(
            "affine WhenBad arbitrary replay clause ranges",
            &mut clause_ranges,
            self.formula.clause_count(),
        )?;
        for clause_ordinal in 0..self.formula.clause_count() {
            clause_ranges.push(
                self.formula
                    .clause_range(clause_ordinal)
                    .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?,
            );
        }
        let problem = AffineWhenBadArbitraryRelativeProblem::from_preallocated(
            structural_loci,
            inherited_truths,
            atoms,
            clause_ranges,
        );
        let observed = problem.retained_owned_logical_byte_bound()?;
        if observed > admitted_owned_logical_byte_envelope {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed,
                    admitted: admitted_owned_logical_byte_envelope,
                },
            );
        }
        Ok(problem)
    }

    fn try_copy_authenticated_problem(
        &self,
        context: &ParametricCoefficientContext,
        admitted_owned_logical_byte_envelope: usize,
    ) -> Result<AffineWhenBadAuthenticatedArbitraryRelativeProblem, AffineWhenBadRelativeCaseError>
    {
        let AffineWhenBadArbitraryCanonicalLoci::Authenticated {
            expected_schema,
            owner,
        } = &self.canonical_loci
        else {
            return Err(AffineWhenBadRelativeCaseError::ReplayMismatch);
        };
        let projected_problem_owned_logical_byte_envelope =
            authenticated_arbitrary_projected_replay_problem_owned_logical_byte_envelope_from_parts(
                owner,
                self.inherited_truths.len(),
                self.formula.atoms().len(),
                self.formula.clause_count(),
            )?;
        if projected_problem_owned_logical_byte_envelope > admitted_owned_logical_byte_envelope {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed: projected_problem_owned_logical_byte_envelope,
                    admitted: admitted_owned_logical_byte_envelope,
                },
            );
        }
        let canonical_loci =
            try_copy_authenticated_canonical_owner(owner, context, expected_schema, self.limits)?;
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(1);
        let inherited_truths = try_canonicalize_inherited_truths(&self.inherited_truths)?;
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(2);
        let mut atoms = Vec::new();
        try_reserve_exact(
            "affine WhenBad authenticated arbitrary replay atoms",
            &mut atoms,
            self.formula.atoms().len(),
        )?;
        atoms.extend_from_slice(self.formula.atoms());
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(3);
        let mut clause_ranges = Vec::new();
        try_reserve_exact(
            "affine WhenBad authenticated arbitrary replay clause ranges",
            &mut clause_ranges,
            self.formula.clause_count(),
        )?;
        for clause_ordinal in 0..self.formula.clause_count() {
            clause_ranges.push(
                self.formula
                    .clause_range(clause_ordinal)
                    .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?,
            );
        }
        mark_arbitrary_partition_replay_problem_copy_stage_for_test(4);
        let problem = AffineWhenBadAuthenticatedArbitraryRelativeProblem::from_preallocated(
            canonical_loci,
            expected_schema,
            inherited_truths,
            atoms,
            clause_ranges,
        );
        let observed = problem.retained_owned_logical_byte_bound()?;
        if observed > admitted_owned_logical_byte_envelope {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed,
                    admitted: admitted_owned_logical_byte_envelope,
                },
            );
        }
        Ok(problem)
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.context_fingerprint == other.context_fingerprint
            && self
                .canonical_loci
                .semantic_payload_eq(&other.canonical_loci)
            && self.inherited_truths == other.inherited_truths
            && self.formula.payload_eq(&other.formula)
            && self.splits == other.splits
            && self.cases == other.cases
            && self.classifications == other.classifications
            && self.limits == other.limits
            && self.stats == other.stats
            && self.compilation_stats == other.compilation_stats
    }
}

impl fmt::Debug for AffineWhenBadArbitraryRelativePartitionCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AffineWhenBadArbitraryRelativePartitionCertificate")
            .field("schema", &self.schema)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("structural_locus_count", &self.structural_loci().len())
            .field(
                "authenticated_canonical_loci",
                &self.canonical_loci.is_authenticated(),
            )
            .field("inherited_truth_count", &self.inherited_truths.len())
            .field("formula", &self.formula)
            .field("split_count", &self.splits.len())
            .field("case_count", &self.cases.len())
            .field("classification_count", &self.classifications.len())
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .field("compilation_stats", &self.compilation_stats)
            .finish()
    }
}

impl PartialEq for AffineWhenBadArbitraryRelativePartitionCertificate {
    fn eq(&self, other: &Self) -> bool {
        self.payload_eq(other)
    }
}

impl Eq for AffineWhenBadArbitraryRelativePartitionCertificate {}

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
pub(crate) struct AffineWhenBadArbitraryRelativeProblem {
    structural_loci: Vec<ParametricPolynomial>,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    atoms: Vec<AffineWhenBadAtom>,
    clause_ranges: Vec<Range<usize>>,
}

impl AffineWhenBadArbitraryRelativeProblem {
    pub(crate) fn from_preallocated(
        structural_loci: Vec<ParametricPolynomial>,
        inherited_truths: Vec<AffineWhenBadInheritedTruth>,
        atoms: Vec<AffineWhenBadAtom>,
        clause_ranges: Vec<Range<usize>>,
    ) -> Self {
        Self {
            structural_loci,
            inherited_truths,
            atoms,
            clause_ranges,
        }
    }

    /// Complete owned-byte bound for the caller-owned source payload before
    /// it is moved into the compiler. This includes spare vector capacity and
    /// every authenticated sparse-polynomial allocation.
    pub(crate) fn retained_owned_logical_byte_bound(
        &self,
    ) -> Result<usize, AffineWhenBadRelativeCaseError> {
        let mut bytes = size_of::<Self>();
        for allocation in [
            checked_mul(
                "affine WhenBad arbitrary source problem retained bytes",
                self.structural_loci.capacity(),
                size_of::<ParametricPolynomial>(),
            )?,
            checked_mul(
                "affine WhenBad arbitrary source problem retained bytes",
                self.inherited_truths.capacity(),
                size_of::<AffineWhenBadInheritedTruth>(),
            )?,
            checked_mul(
                "affine WhenBad arbitrary source problem retained bytes",
                self.atoms.capacity(),
                size_of::<AffineWhenBadAtom>(),
            )?,
            checked_mul(
                "affine WhenBad arbitrary source problem retained bytes",
                self.clause_ranges.capacity(),
                size_of::<Range<usize>>(),
            )?,
        ] {
            bytes = checked_add(
                "affine WhenBad arbitrary source problem retained bytes",
                bytes,
                allocation,
            )?;
        }
        for polynomial in &self.structural_loci {
            bytes = checked_add(
                "affine WhenBad arbitrary source problem retained bytes",
                bytes,
                polynomial.owned_retained_byte_bound().ok_or(
                    AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                        resource: "affine WhenBad arbitrary source problem retained bytes",
                    },
                )?,
            )?;
        }
        Ok(bytes)
    }
}

/// Internal arbitrary-width input carrying the opaque proof that its locus
/// table was canonicalized once by the bounded outer owner.  The owner is not
/// cloneable and there is no raw constructor for authenticated authority.
pub(crate) struct AffineWhenBadAuthenticatedArbitraryRelativeProblem {
    canonical_loci: CanonicalLocusTableOwner,
    expected_canonical_locus_schema: &'static str,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    atoms: Vec<AffineWhenBadAtom>,
    clause_ranges: Vec<Range<usize>>,
}

impl AffineWhenBadAuthenticatedArbitraryRelativeProblem {
    pub(crate) fn from_preallocated(
        canonical_loci: CanonicalLocusTableOwner,
        expected_canonical_locus_schema: &'static str,
        inherited_truths: Vec<AffineWhenBadInheritedTruth>,
        atoms: Vec<AffineWhenBadAtom>,
        clause_ranges: Vec<Range<usize>>,
    ) -> Self {
        Self {
            canonical_loci,
            expected_canonical_locus_schema,
            inherited_truths,
            atoms,
            clause_ranges,
        }
    }

    pub(crate) const fn canonical_loci(&self) -> &CanonicalLocusTableOwner {
        &self.canonical_loci
    }

    pub(crate) fn retained_owned_logical_byte_bound(
        &self,
    ) -> Result<usize, AffineWhenBadRelativeCaseError> {
        let resource = "affine WhenBad authenticated arbitrary source problem retained bytes";
        let mut bytes = size_of::<Self>();
        for allocation in [
            canonical_owner_retained_owned_logical_bytes(&self.canonical_loci)?,
            checked_mul(
                resource,
                self.inherited_truths.capacity(),
                size_of::<AffineWhenBadInheritedTruth>(),
            )?,
            checked_mul(
                resource,
                self.atoms.capacity(),
                size_of::<AffineWhenBadAtom>(),
            )?,
            checked_mul(
                resource,
                self.clause_ranges.capacity(),
                size_of::<Range<usize>>(),
            )?,
        ] {
            bytes = checked_add(resource, bytes, allocation)?;
        }
        Ok(bytes)
    }
}

/// Recoverable authenticated compilation failure. All fallible compilation
/// borrows the problem, so resource errors and caught panics return the exact
/// non-Clone canonical owner for a deterministic retry.
pub(crate) struct AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {
    error: AffineWhenBadRelativeCaseError,
    problem: AffineWhenBadAuthenticatedArbitraryRelativeProblem,
}

impl AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {
    pub(crate) const fn error(&self) -> &AffineWhenBadRelativeCaseError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        AffineWhenBadRelativeCaseError,
        AffineWhenBadAuthenticatedArbitraryRelativeProblem,
    ) {
        (self.error, self.problem)
    }
}

impl fmt::Debug for AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure")
            .field("error", &self.error)
            .field("private_problem", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {}

fn arbitrary_replay_source_problem_owned_logical_byte_envelope(
    structural_loci: &[ParametricPolynomial],
    inherited_truth_count: usize,
    atom_count: usize,
    clause_count: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let resource = "affine WhenBad arbitrary source problem owned logical bytes";
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativeProblem>();
    for allocation in [
        capacity_byte_envelope(structural_loci.len(), size_of::<ParametricPolynomial>())?,
        capacity_byte_envelope(
            inherited_truth_count,
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(atom_count, size_of::<AffineWhenBadAtom>())?,
        capacity_byte_envelope(clause_count, size_of::<Range<usize>>())?,
    ] {
        bytes = checked_add(resource, bytes, allocation)?;
    }
    for polynomial in structural_loci {
        bytes = checked_add(
            resource,
            bytes,
            deterministic_polynomial_owned_byte_envelope(polynomial)?,
        )?;
    }
    Ok(bytes)
}

fn canonical_owner_retained_owned_logical_bytes(
    owner: &CanonicalLocusTableOwner,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    owner
        .retained_owned_logical_byte_bound()
        .map_err(map_canonical_locus_table_error)?
        .checked_sub(size_of::<CanonicalLocusTableOwner>())
        .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow {
            resource: "affine WhenBad authenticated canonical owner retained bytes",
        })
}

fn canonical_owner_projected_compact_owned_logical_bytes(
    owner: &CanonicalLocusTableOwner,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    owner
        .projected_compact_copy_owned_logical_byte_bound()
        .map_err(map_canonical_locus_table_error)
}

fn canonical_owner_projected_compact_dynamic_owned_logical_bytes(
    owner: &CanonicalLocusTableOwner,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    canonical_owner_projected_compact_owned_logical_bytes(owner)?
        .checked_sub(size_of::<CanonicalLocusTableOwner>())
        .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow {
            resource: "affine WhenBad authenticated canonical owner retained bytes",
        })
}

fn canonical_owner_projected_compact_container_owned_logical_bytes(
    owner: &CanonicalLocusTableOwner,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let resource = "affine WhenBad authenticated canonical owner retained bytes";
    let mut dynamic = canonical_owner_projected_compact_dynamic_owned_logical_bytes(owner)?;
    for polynomial in owner.loci() {
        dynamic = dynamic
            .checked_sub(
                polynomial
                    .owned_retained_byte_bound()
                    .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource })?,
            )
            .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource })?;
    }
    Ok(dynamic)
}

fn canonical_owner_container_owned_logical_bytes(
    owner: &CanonicalLocusTableOwner,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut dynamic = canonical_owner_retained_owned_logical_bytes(owner)?;
    for polynomial in owner.loci() {
        dynamic = dynamic
            .checked_sub(polynomial.owned_retained_byte_bound().ok_or(
                AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                    resource: "affine WhenBad authenticated canonical owner retained bytes",
                },
            )?)
            .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                resource: "affine WhenBad authenticated canonical owner retained bytes",
            })?;
    }
    Ok(dynamic)
}

fn authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope_from_parts(
    owner: &CanonicalLocusTableOwner,
    inherited_truth_count: usize,
    atom_count: usize,
    clause_count: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let resource = "affine WhenBad arbitrary source problem owned logical bytes";
    let mut bytes = size_of::<AffineWhenBadAuthenticatedArbitraryRelativeProblem>();
    for allocation in [
        canonical_owner_retained_owned_logical_bytes(owner)?,
        capacity_byte_envelope(
            inherited_truth_count,
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(atom_count, size_of::<AffineWhenBadAtom>())?,
        capacity_byte_envelope(clause_count, size_of::<Range<usize>>())?,
    ] {
        bytes = checked_add(resource, bytes, allocation)?;
    }
    Ok(bytes)
}

fn authenticated_arbitrary_projected_replay_problem_owned_logical_byte_envelope_from_parts(
    owner: &CanonicalLocusTableOwner,
    inherited_truth_count: usize,
    atom_count: usize,
    clause_count: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let resource = "affine WhenBad arbitrary source problem owned logical bytes";
    let mut bytes = size_of::<AffineWhenBadAuthenticatedArbitraryRelativeProblem>();
    for allocation in [
        canonical_owner_projected_compact_dynamic_owned_logical_bytes(owner)?,
        capacity_byte_envelope(
            inherited_truth_count,
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(atom_count, size_of::<AffineWhenBadAtom>())?,
        capacity_byte_envelope(clause_count, size_of::<Range<usize>>())?,
    ] {
        bytes = checked_add(resource, bytes, allocation)?;
    }
    Ok(bytes)
}

fn authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope(
    problem: &AffineWhenBadAuthenticatedArbitraryRelativeProblem,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope_from_parts(
        &problem.canonical_loci,
        problem.inherited_truths.len(),
        problem.atoms.len(),
        problem.clause_ranges.len(),
    )
}

fn map_canonical_locus_table_error(
    error: CanonicalLocusTableError,
) -> AffineWhenBadRelativeCaseError {
    match error {
        CanonicalLocusTableError::SchemaMismatch => AffineWhenBadRelativeCaseError::SchemaMismatch,
        CanonicalLocusTableError::ContextMismatch => {
            AffineWhenBadRelativeCaseError::ContextMismatch
        }
        CanonicalLocusTableError::IdenticallyZeroLocus
        | CanonicalLocusTableError::CoefficientFieldLocus => {
            AffineWhenBadRelativeCaseError::ReplayMismatch
        }
        CanonicalLocusTableError::ReservedCapacityExhausted {
            requested,
            reserved,
        } => AffineWhenBadRelativeCaseError::ResourceLimit {
            resource: "canonical locus table reserved capacity",
            requested,
            limit: reserved,
        },
        CanonicalLocusTableError::ResourceLimit {
            resource,
            requested,
            limit,
        } => AffineWhenBadRelativeCaseError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        CanonicalLocusTableError::ResourceCountOverflow { resource } => {
            AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource }
        }
        CanonicalLocusTableError::AllocationFailure {
            resource,
            requested,
        } => AffineWhenBadRelativeCaseError::AllocationFailure {
            resource,
            requested,
        },
        CanonicalLocusTableError::RetainedByteEnvelopeExceeded { observed, admitted } => {
            AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded { observed, admitted }
        }
        CanonicalLocusTableError::SymbolicaPanic { stage } => {
            AffineWhenBadRelativeCaseError::SymbolicaPanic { stage }
        }
        CanonicalLocusTableError::ParametricCoefficient(error) => {
            AffineWhenBadRelativeCaseError::ParametricCoefficient(error)
        }
    }
}

fn try_copy_authenticated_canonical_owner(
    owner: &CanonicalLocusTableOwner,
    context: &ParametricCoefficientContext,
    expected_schema: &'static str,
    limits: AffineWhenBadArbitraryRelativeLimits,
) -> Result<CanonicalLocusTableOwner, AffineWhenBadRelativeCaseError> {
    let source_owner_owned_logical_bytes = owner
        .retained_owned_logical_byte_bound()
        .map_err(map_canonical_locus_table_error)?;
    let projected_destination_owned_logical_bytes =
        canonical_owner_projected_compact_owned_logical_bytes(owner)?;
    let max_copy_owned_logical_peak_upper_bound = checked_add(
        "canonical locus authenticated copy owned logical peak upper bound",
        source_owner_owned_logical_bytes,
        projected_destination_owned_logical_bytes,
    )?;
    let copy_limits = CanonicalLocusTableCopyLimits {
        exact_algebra: limits.relative.exact_algebra,
        max_context_fingerprint_bytes: limits.relative.max_context_fingerprint_bytes,
        max_structural_loci: limits.relative.max_structural_loci,
        max_retained_polynomial_terms: limits.relative.max_retained_polynomial_terms,
        max_retained_polynomial_exponent_entries: limits
            .relative
            .max_retained_polynomial_exponent_entries,
        max_retained_polynomial_integer_bits: limits.relative.max_retained_polynomial_integer_bits,
        max_retained_owned_logical_bytes: projected_destination_owned_logical_bytes,
        max_copy_owned_logical_peak_upper_bound,
    };
    owner
        .try_copy_authenticated(context, expected_schema, copy_limits)
        .map_err(map_canonical_locus_table_error)
}

/// Compatibility input for the original fixed-width, provenance-owning API.
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
    case: AffineWhenBadArbitraryRelativeCase,
    decisions: Vec<Option<SymbolicPolynomialPredicateKind>>,
}

#[derive(Clone, Copy)]
enum RelativePartitionRetainedLayout {
    LegacyPolynomialRich,
    ArbitraryTableIndexed {
        max_work_owned_logical_peak_upper_bound: usize,
        max_compiler_owned_logical_peak_upper_bound: usize,
        source_problem_owned_logical_bytes: usize,
        max_compilation_owned_logical_peak_upper_bound: usize,
    },
}

#[derive(Clone, Copy)]
enum RelativeDirectFormulaView<'a> {
    Legacy(&'a AffineWhenBadDirectFormula),
    Arbitrary(&'a ArbitraryDirectBadFormula<AffineWhenBadAtom>),
}

impl RelativeDirectFormulaView<'_> {
    fn clause_visit_bound(self) -> usize {
        match self {
            Self::Legacy(formula) => formula.clauses.len(),
            Self::Arbitrary(formula) => formula.stats().route_clause_visit_bound(),
        }
    }

    fn atom_query_bound(self) -> usize {
        match self {
            Self::Legacy(formula) => formula.atom_count,
            Self::Arbitrary(formula) => formula.stats().route_atom_query_bound(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelativePartitionRoute {
    Bad {
        clause_ordinal: usize,
    },
    Good,
    Split {
        clause_ordinal: usize,
        clause_atom_ordinal: usize,
        atom_ordinal: usize,
        atom: AffineWhenBadAtom,
    },
}

#[derive(Clone, Copy)]
enum RelativePartitionTruth {
    False,
    True,
    Unknown,
}

#[derive(Clone, Copy)]
struct LocusDivisibilityCacheEntry {
    divisor: usize,
    dividend: usize,
    result: bool,
}

struct ArbitraryAssociatePeakAdmission<'a> {
    limits: AffineWhenBadArbitraryRelativeLimits,
    source_problem_owned_logical_bytes: usize,
    compiler_owned_logical_peak_upper_bound: &'a mut usize,
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
        validate_structural_loci(context, &source_structural_loci, limits, &mut stats, None)?;
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

        let (kernel_splits, kernel_cases, kernel_classifications) = build_partition_kernel(
            context,
            &structural_loci,
            &inherited_truths,
            RelativeDirectFormulaView::Legacy(&formula),
            RelativePartitionRetainedLayout::LegacyPolynomialRich,
            limits,
            &mut stats,
        )?;
        let (splits, cases, classifications) = materialize_legacy_partition(
            &structural_loci,
            &formula,
            kernel_splits,
            kernel_cases,
            kernel_classifications,
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

/// Crate-private compiler for the arbitrary-width, provenance-neutral seam.
pub(crate) struct AffineWhenBadArbitraryRelativePartitionCompiler;

impl AffineWhenBadArbitraryRelativePartitionCompiler {
    pub(crate) fn compile(
        context: &ParametricCoefficientContext,
        problem: AffineWhenBadArbitraryRelativeProblem,
        limits: AffineWhenBadArbitraryRelativeLimits,
    ) -> Result<AffineWhenBadArbitraryRelativePartitionCertificate, AffineWhenBadRelativeCaseError>
    {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(context, problem, limits)
        }))
        .map_err(|_| AffineWhenBadRelativeCaseError::SymbolicaPanic {
            stage: "arbitrary relative partition compilation",
        })?
    }

    pub(crate) fn compile_authenticated(
        context: &ParametricCoefficientContext,
        problem: AffineWhenBadAuthenticatedArbitraryRelativeProblem,
        limits: AffineWhenBadArbitraryRelativeLimits,
    ) -> Result<
        AffineWhenBadArbitraryRelativePartitionCertificate,
        AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            Self::compile_authenticated_inner(context, &problem, limits)
        }));
        match prepared {
            Ok(Ok(certificate)) => Ok(certificate),
            Ok(Err(error)) => {
                Err(AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure { error, problem })
            }
            Err(_) => Err(
                AffineWhenBadAuthenticatedArbitraryRelativePartitionFailure {
                    error: AffineWhenBadRelativeCaseError::SymbolicaPanic {
                        stage: "authenticated arbitrary relative partition compilation",
                    },
                    problem,
                },
            ),
        }
    }

    fn compile_authenticated_inner(
        context: &ParametricCoefficientContext,
        problem: &AffineWhenBadAuthenticatedArbitraryRelativeProblem,
        arbitrary_limits: AffineWhenBadArbitraryRelativeLimits,
    ) -> Result<AffineWhenBadArbitraryRelativePartitionCertificate, AffineWhenBadRelativeCaseError>
    {
        maybe_inject_arbitrary_partition_compile_panic_for_test();
        let observed_source_problem_owned_logical_bytes =
            problem.retained_owned_logical_byte_bound()?;
        let replay_source_problem_owned_logical_byte_envelope =
            authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope(problem)?;
        let source_problem_owned_logical_bytes = observed_source_problem_owned_logical_bytes
            .max(replay_source_problem_owned_logical_byte_envelope);
        check_limit(
            "affine WhenBad arbitrary source problem owned logical bytes",
            source_problem_owned_logical_bytes,
            arbitrary_limits.max_source_problem_owned_logical_bytes,
        )?;
        let limits = arbitrary_limits.relative;
        let mut stats = AffineWhenBadRelativeCaseStats::default();
        stats.context_fingerprint_bytes = context.fingerprint().len();
        check_limit(
            "affine WhenBad relative context fingerprint bytes",
            stats.context_fingerprint_bytes,
            limits.max_context_fingerprint_bytes,
        )?;
        let projected_initial_retained_bytes =
            authenticated_arbitrary_projected_initial_retained_byte_envelope(
                context.fingerprint().len(),
                &problem.canonical_loci,
                problem.inherited_truths.len(),
            )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            projected_initial_retained_bytes,
            limits.max_retained_bytes,
        )?;
        let mut pre_partition_compiler_owned_logical_peak_upper_bound =
            check_arbitrary_owned_peak_limits(
                projected_initial_retained_bytes,
                0,
                source_problem_owned_logical_bytes,
                arbitrary_limits,
            )?;
        mark_arbitrary_partition_context_copy_observed_for_test();
        let context_fingerprint = try_copy_string(
            context.fingerprint(),
            "affine WhenBad authenticated arbitrary relative context fingerprint",
        )?;

        mark_arbitrary_partition_canonical_copy_observed_for_test();
        let canonical_loci = try_copy_authenticated_canonical_owner(
            &problem.canonical_loci,
            context,
            problem.expected_canonical_locus_schema,
            arbitrary_limits,
        )?;
        let owner_copy_phase_retained_bytes = checked_add(
            "affine WhenBad relative retained bytes",
            checked_add(
                "affine WhenBad relative retained bytes",
                size_of::<AffineWhenBadArbitraryRelativePartitionCertificate>(),
                capacity_byte_envelope(context.fingerprint().len(), size_of::<u8>())?,
            )?,
            canonical_owner_retained_owned_logical_bytes(&canonical_loci)?,
        )?;
        if owner_copy_phase_retained_bytes > projected_initial_retained_bytes {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed: owner_copy_phase_retained_bytes,
                    admitted: projected_initial_retained_bytes,
                },
            );
        }
        // From this point onward every durable census is derived from the
        // compact owner actually retained by the certificate, never from the
        // caller's potentially over-reserved construction owner.
        stats.retained_bytes = authenticated_arbitrary_initial_retained_byte_envelope(
            context.fingerprint().len(),
            &canonical_loci,
            &problem.inherited_truths,
        )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        )?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    0,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        let source_structural_loci = canonical_loci.loci();
        reset_authenticated_arbitrary_partition_linear_validations_for_test();
        validate_authenticated_structural_loci(
            context,
            source_structural_loci,
            limits,
            &mut stats,
        )?;
        if stats.retained_bytes > projected_initial_retained_bytes {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed: stats.retained_bytes,
                    admitted: projected_initial_retained_bytes,
                },
            );
        }
        maybe_inject_authenticated_arbitrary_partition_post_validation_panic_for_test();
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    0,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        let inherited_validation_work_owned_logical_peak_upper_bound =
            capacity_byte_envelope(source_structural_loci.len(), size_of::<bool>())?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    inherited_validation_work_owned_logical_peak_upper_bound,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        check_limit(
            "affine WhenBad relative inherited truths",
            problem.inherited_truths.len(),
            limits.max_inherited_truths,
        )?;
        mark_arbitrary_partition_inherited_validation_reserve_observed_for_test();
        validate_inherited_truths(
            &problem.inherited_truths,
            source_structural_loci.len(),
            limits,
            &mut stats,
        )?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    0,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        mark_arbitrary_partition_inherited_copy_observed_for_test();
        let inherited_truths = try_canonicalize_inherited_truths(&problem.inherited_truths)?;
        let formula = validate_and_compile_arbitrary_formula(
            &problem.atoms,
            &problem.clause_ranges,
            source_structural_loci.len(),
            limits,
            &mut stats,
            true,
            arbitrary_limits,
            source_problem_owned_logical_bytes,
            &mut pre_partition_compiler_owned_logical_peak_upper_bound,
        )?;

        let (splits, cases, classifications) = build_partition_kernel(
            context,
            source_structural_loci,
            &inherited_truths,
            RelativeDirectFormulaView::Arbitrary(&formula),
            RelativePartitionRetainedLayout::ArbitraryTableIndexed {
                max_work_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_work_owned_logical_peak_upper_bound,
                max_compiler_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_compiler_owned_logical_peak_upper_bound,
                source_problem_owned_logical_bytes,
                max_compilation_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_compilation_owned_logical_peak_upper_bound,
            },
            limits,
            &mut stats,
        )?;
        let (payload_units, payload_bytes, payload_integer_bits) =
            authenticated_arbitrary_payload_census(
                AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA,
                problem.expected_canonical_locus_schema,
                &context_fingerprint,
                source_structural_loci,
                &inherited_truths,
                &formula,
                &splits,
                &cases,
                &classifications,
            )?;
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

        let partition_work_owned_logical_peak_upper_bound =
            arbitrary_work_owned_logical_peak_upper_bound(stats)?;
        let work_owned_logical_peak_upper_bound = partition_work_owned_logical_peak_upper_bound
            .max(inherited_validation_work_owned_logical_peak_upper_bound);
        check_limit(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            work_owned_logical_peak_upper_bound,
            arbitrary_limits.max_work_owned_logical_peak_upper_bound,
        )?;
        let partition_compiler_owned_logical_peak_upper_bound = checked_add(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            stats.retained_bytes,
            partition_work_owned_logical_peak_upper_bound,
        )?;
        let compiler_owned_logical_peak_upper_bound =
            partition_compiler_owned_logical_peak_upper_bound
                .max(pre_partition_compiler_owned_logical_peak_upper_bound);
        check_limit(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            compiler_owned_logical_peak_upper_bound,
            arbitrary_limits.max_compiler_owned_logical_peak_upper_bound,
        )?;
        let compilation_owned_logical_peak_upper_bound = checked_add(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            source_problem_owned_logical_bytes,
            compiler_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            compilation_owned_logical_peak_upper_bound,
            arbitrary_limits.max_compilation_owned_logical_peak_upper_bound,
        )?;
        // The caller-owned problem may retain a much larger reserved locus
        // capacity than the compact authenticated owner copied into the
        // certificate (notably after duplicate-heavy first-seen interning).
        // Keep the larger `source_problem_owned_logical_bytes` above for the
        // honest construction peak, but persist the deterministic envelope of
        // the compact replay source. Replaying then reconstructs the same
        // compact owner without re-running any pairwise canonicality proof.
        let certificate_replay_source_problem_owned_logical_byte_envelope =
            authenticated_arbitrary_replay_source_problem_owned_logical_byte_envelope_from_parts(
                &canonical_loci,
                inherited_truths.len(),
                formula.atoms().len(),
                formula.clause_count(),
            )?;
        let compilation_stats = AffineWhenBadArbitraryRelativeCompilationStats {
            source_problem_owned_logical_byte_envelope:
                certificate_replay_source_problem_owned_logical_byte_envelope,
            formula_retained_owned_logical_bytes: formula.stats().retained_owned_logical_bytes(),
            formula_compilation_owned_logical_peak_upper_bound: formula
                .stats()
                .compilation_owned_logical_peak_upper_bound(),
            work_owned_logical_peak_upper_bound,
            compiler_owned_logical_peak_upper_bound,
        };

        let certificate = AffineWhenBadArbitraryRelativePartitionCertificate {
            schema: AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA,
            context_fingerprint,
            canonical_loci: AffineWhenBadArbitraryCanonicalLoci::Authenticated {
                expected_schema: problem.expected_canonical_locus_schema,
                owner: canonical_loci,
            },
            inherited_truths,
            formula,
            splits,
            cases,
            classifications,
            limits: arbitrary_limits,
            stats,
            compilation_stats,
        };
        let observed = observed_arbitrary_certificate_owned_byte_bound(&certificate)?;
        if observed > certificate.stats.retained_bytes {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed,
                    admitted: certificate.stats.retained_bytes,
                },
            );
        }
        Ok(certificate)
    }

    fn compile_inner(
        context: &ParametricCoefficientContext,
        problem: AffineWhenBadArbitraryRelativeProblem,
        arbitrary_limits: AffineWhenBadArbitraryRelativeLimits,
    ) -> Result<AffineWhenBadArbitraryRelativePartitionCertificate, AffineWhenBadRelativeCaseError>
    {
        maybe_inject_arbitrary_partition_compile_panic_for_test();
        let observed_source_problem_owned_logical_bytes =
            problem.retained_owned_logical_byte_bound()?;
        let replay_source_problem_owned_logical_byte_envelope =
            arbitrary_replay_source_problem_owned_logical_byte_envelope(
                &problem.structural_loci,
                problem.inherited_truths.len(),
                problem.atoms.len(),
                problem.clause_ranges.len(),
            )?;
        let source_problem_owned_logical_bytes = observed_source_problem_owned_logical_bytes
            .max(replay_source_problem_owned_logical_byte_envelope);
        check_limit(
            "affine WhenBad arbitrary source problem owned logical bytes",
            source_problem_owned_logical_bytes,
            arbitrary_limits.max_source_problem_owned_logical_bytes,
        )?;
        let limits = arbitrary_limits.relative;
        let mut stats = AffineWhenBadRelativeCaseStats::default();
        stats.context_fingerprint_bytes = context.fingerprint().len();
        check_limit(
            "affine WhenBad relative context fingerprint bytes",
            stats.context_fingerprint_bytes,
            limits.max_context_fingerprint_bytes,
        )?;
        stats.retained_bytes = arbitrary_initial_retained_byte_envelope(
            context.fingerprint().len(),
            &problem.structural_loci,
            &problem.inherited_truths,
        )?;
        check_limit(
            "affine WhenBad relative retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        )?;
        let mut pre_partition_compiler_owned_logical_peak_upper_bound =
            check_arbitrary_owned_peak_limits(
                stats.retained_bytes,
                0,
                source_problem_owned_logical_bytes,
                arbitrary_limits,
            )?;
        mark_arbitrary_partition_context_copy_observed_for_test();
        let context_fingerprint = try_copy_string(
            context.fingerprint(),
            "affine WhenBad arbitrary relative context fingerprint",
        )?;

        let AffineWhenBadArbitraryRelativeProblem {
            structural_loci: source_structural_loci,
            inherited_truths: source_inherited_truths,
            atoms,
            clause_ranges,
        } = problem;
        validate_structural_loci(
            context,
            &source_structural_loci,
            limits,
            &mut stats,
            Some(ArbitraryAssociatePeakAdmission {
                limits: arbitrary_limits,
                source_problem_owned_logical_bytes,
                compiler_owned_logical_peak_upper_bound:
                    &mut pre_partition_compiler_owned_logical_peak_upper_bound,
            }),
        )?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    0,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        mark_arbitrary_partition_canonical_copy_observed_for_test();
        let structural_loci = try_canonicalize_structural_loci(&source_structural_loci)?;

        let inherited_validation_work_owned_logical_peak_upper_bound =
            capacity_byte_envelope(structural_loci.len(), size_of::<bool>())?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    inherited_validation_work_owned_logical_peak_upper_bound,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        check_limit(
            "affine WhenBad relative inherited truths",
            source_inherited_truths.len(),
            limits.max_inherited_truths,
        )?;
        mark_arbitrary_partition_inherited_validation_reserve_observed_for_test();
        validate_inherited_truths(
            &source_inherited_truths,
            structural_loci.len(),
            limits,
            &mut stats,
        )?;
        pre_partition_compiler_owned_logical_peak_upper_bound =
            pre_partition_compiler_owned_logical_peak_upper_bound.max(
                check_arbitrary_owned_peak_limits(
                    stats.retained_bytes,
                    0,
                    source_problem_owned_logical_bytes,
                    arbitrary_limits,
                )?,
            );
        mark_arbitrary_partition_inherited_copy_observed_for_test();
        let inherited_truths = try_canonicalize_inherited_truths(&source_inherited_truths)?;
        let formula = validate_and_compile_arbitrary_formula(
            &atoms,
            &clause_ranges,
            structural_loci.len(),
            limits,
            &mut stats,
            true,
            arbitrary_limits,
            source_problem_owned_logical_bytes,
            &mut pre_partition_compiler_owned_logical_peak_upper_bound,
        )?;

        let (splits, cases, classifications) = build_partition_kernel(
            context,
            &structural_loci,
            &inherited_truths,
            RelativeDirectFormulaView::Arbitrary(&formula),
            RelativePartitionRetainedLayout::ArbitraryTableIndexed {
                max_work_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_work_owned_logical_peak_upper_bound,
                max_compiler_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_compiler_owned_logical_peak_upper_bound,
                source_problem_owned_logical_bytes,
                max_compilation_owned_logical_peak_upper_bound: arbitrary_limits
                    .max_compilation_owned_logical_peak_upper_bound,
            },
            limits,
            &mut stats,
        )?;
        let (payload_units, payload_bytes, payload_integer_bits) = arbitrary_payload_census(
            &context_fingerprint,
            &structural_loci,
            &inherited_truths,
            &formula,
            &splits,
            &cases,
            &classifications,
        )?;
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

        let partition_work_owned_logical_peak_upper_bound =
            arbitrary_work_owned_logical_peak_upper_bound(stats)?;
        let work_owned_logical_peak_upper_bound = partition_work_owned_logical_peak_upper_bound
            .max(inherited_validation_work_owned_logical_peak_upper_bound);
        check_limit(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            work_owned_logical_peak_upper_bound,
            arbitrary_limits.max_work_owned_logical_peak_upper_bound,
        )?;
        let partition_compiler_owned_logical_peak_upper_bound = checked_add(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            stats.retained_bytes,
            partition_work_owned_logical_peak_upper_bound,
        )?;
        let compiler_owned_logical_peak_upper_bound =
            partition_compiler_owned_logical_peak_upper_bound
                .max(pre_partition_compiler_owned_logical_peak_upper_bound);
        check_limit(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            compiler_owned_logical_peak_upper_bound,
            arbitrary_limits.max_compiler_owned_logical_peak_upper_bound,
        )?;
        let compilation_owned_logical_peak_upper_bound = checked_add(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            source_problem_owned_logical_bytes,
            compiler_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            compilation_owned_logical_peak_upper_bound,
            arbitrary_limits.max_compilation_owned_logical_peak_upper_bound,
        )?;
        let compilation_stats = AffineWhenBadArbitraryRelativeCompilationStats {
            source_problem_owned_logical_byte_envelope:
                replay_source_problem_owned_logical_byte_envelope,
            formula_retained_owned_logical_bytes: formula.stats().retained_owned_logical_bytes(),
            formula_compilation_owned_logical_peak_upper_bound: formula
                .stats()
                .compilation_owned_logical_peak_upper_bound(),
            work_owned_logical_peak_upper_bound,
            compiler_owned_logical_peak_upper_bound,
        };

        let certificate = AffineWhenBadArbitraryRelativePartitionCertificate {
            schema: AFFINE_WHEN_BAD_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA,
            context_fingerprint,
            canonical_loci: AffineWhenBadArbitraryCanonicalLoci::Raw(structural_loci),
            inherited_truths,
            formula,
            splits,
            cases,
            classifications,
            limits: arbitrary_limits,
            stats,
            compilation_stats,
        };
        let observed = observed_arbitrary_certificate_owned_byte_bound(&certificate)?;
        if observed > certificate.stats.retained_bytes {
            return Err(
                AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                    observed,
                    admitted: certificate.stats.retained_bytes,
                },
            );
        }
        Ok(certificate)
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
    mut arbitrary_admission: Option<ArbitraryAssociatePeakAdmission<'_>>,
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
            let associated = if let Some(admission) = arbitrary_admission.as_mut() {
                let compiler_live = stats.retained_bytes;
                let compiler_remaining = remaining_limit(
                    "affine WhenBad arbitrary compiler owned logical peak upper bound",
                    admission.limits.max_compiler_owned_logical_peak_upper_bound,
                    compiler_live,
                )?;
                let compilation_live = checked_add(
                    "affine WhenBad arbitrary compilation owned logical peak upper bound",
                    admission.source_problem_owned_logical_bytes,
                    compiler_live,
                )?;
                let compilation_remaining = remaining_limit(
                    "affine WhenBad arbitrary compilation owned logical peak upper bound",
                    admission
                        .limits
                        .max_compilation_owned_logical_peak_upper_bound,
                    compilation_live,
                )?;
                let child_limits = ParametricPolynomialAssociateLimits {
                    exact_algebra: limits.exact_algebra,
                    max_combined_temporary_byte_envelope: compiler_remaining
                        .min(compilation_remaining),
                    ..ParametricPolynomialAssociateLimits::default()
                };
                let result = match context.polynomial_loci_are_associates_with_census(
                    first,
                    polynomial,
                    child_limits,
                ) {
                    Ok(result) => result,
                    Err(ParametricCoefficientError::ResourceLimit {
                        resource: "polynomial-associate combined temporary byte envelope",
                        requested,
                        ..
                    }) => {
                        let (resource, live, limit) = if compiler_remaining <= compilation_remaining
                        {
                            (
                                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                                compiler_live,
                                admission.limits.max_compiler_owned_logical_peak_upper_bound,
                            )
                        } else {
                            (
                                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                                compilation_live,
                                admission
                                    .limits
                                    .max_compilation_owned_logical_peak_upper_bound,
                            )
                        };
                        return Err(AffineWhenBadRelativeCaseError::ResourceLimit {
                            resource,
                            requested: checked_add(resource, live, requested)?,
                            limit,
                        });
                    }
                    Err(error) => return Err(error.into()),
                };
                let child = result.stats();
                let scratch = checked_add(
                    "affine WhenBad arbitrary associate combined temporary byte envelope",
                    child.rustred_visible_temporary_byte_envelope(),
                    child.native_workspace_byte_envelope(),
                )?;
                let compiler_peak = checked_add(
                    "affine WhenBad arbitrary compiler owned logical peak upper bound",
                    compiler_live,
                    scratch,
                )?;
                check_arbitrary_compiler_and_global_peak_limits(
                    compiler_peak,
                    admission.source_problem_owned_logical_bytes,
                    admission.limits.max_compiler_owned_logical_peak_upper_bound,
                    admission
                        .limits
                        .max_compilation_owned_logical_peak_upper_bound,
                )?;
                *admission.compiler_owned_logical_peak_upper_bound =
                    (*admission.compiler_owned_logical_peak_upper_bound).max(compiler_peak);
                result.associated()
            } else {
                context.polynomial_loci_are_associates_with_limits(
                    first,
                    polynomial,
                    limits.exact_algebra,
                )?
            };
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

/// Validate a locus table whose pairwise canonicality is carried by an opaque
/// sealed owner. This deliberately retains every O(N) authentication and
/// payload census from the raw path while performing no equality comparison,
/// coefficient-field associate proof, or native Symbolica projection.
fn validate_authenticated_structural_loci(
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
        mark_authenticated_arbitrary_partition_linear_validation_for_test();
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
        charge_retained_polynomial(polynomial, stats, limits)?;
    }
    stats.structural_loci = loci.len();
    debug_assert_eq!(stats.structural_locus_equality_comparisons, 0);
    debug_assert_eq!(stats.structural_locus_associate_comparisons, 0);
    debug_assert_eq!(stats.structural_locus_associate_term_pairs, 0);
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

fn validate_and_compile_arbitrary_formula(
    atoms: &[AffineWhenBadAtom],
    ranges: &[Range<usize>],
    locus_count: usize,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
    retain_formula_storage: bool,
    arbitrary_limits: AffineWhenBadArbitraryRelativeLimits,
    source_problem_owned_logical_bytes: usize,
    pre_partition_compiler_owned_logical_peak_upper_bound: &mut usize,
) -> Result<ArbitraryDirectBadFormula<AffineWhenBadAtom>, AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad relative bad clauses",
        ranges.len(),
        limits.max_bad_clauses,
    )?;
    check_limit(
        "affine WhenBad relative bad atoms",
        atoms.len(),
        limits.max_bad_atoms,
    )?;
    for atom in atoms {
        if atom.locus_ordinal >= locus_count {
            return Err(AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                locus_ordinal: atom.locus_ordinal,
            });
        }
    }

    let formula_limits = arbitrary_formula_limits(limits);
    let formula_preflight = ArbitraryDirectBadFormula::<AffineWhenBadAtom>::preflight_compile(
        atoms,
        ranges,
        formula_limits,
    )
    .map_err(map_arbitrary_formula_error)?;
    let atom_storage = formula_preflight.atom_storage_bytes();
    let clause_storage = formula_preflight.clause_storage_bytes();
    let formula_storage = checked_add(
        "affine WhenBad relative retained bytes",
        atom_storage,
        clause_storage,
    )?;
    check_limit(
        "affine WhenBad arbitrary formula retained owned logical bytes",
        formula_preflight.retained_owned_logical_bytes(),
        arbitrary_limits.max_formula_retained_owned_logical_bytes,
    )?;
    check_limit(
        "affine WhenBad arbitrary formula compilation owned logical peak upper bound",
        formula_preflight.compilation_owned_logical_peak_upper_bound(),
        arbitrary_limits.max_formula_compilation_owned_logical_peak_upper_bound,
    )?;
    if retain_formula_storage {
        stats.retained_bytes = checked_bounded_add(
            "affine WhenBad relative retained bytes",
            stats.retained_bytes,
            formula_storage,
            limits.max_retained_bytes,
        )?;
    }
    let formula_compilation_extra = formula_preflight
        .compilation_owned_logical_peak_upper_bound()
        .checked_sub(formula_preflight.retained_owned_logical_bytes())
        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
    let formula_phase_retained_and_temporary = checked_add(
        "affine WhenBad arbitrary compiler owned logical peak upper bound",
        stats.retained_bytes,
        formula_compilation_extra,
    )?;
    *pre_partition_compiler_owned_logical_peak_upper_bound =
        (*pre_partition_compiler_owned_logical_peak_upper_bound).max(
            check_arbitrary_owned_peak_limits(
                formula_phase_retained_and_temporary,
                0,
                source_problem_owned_logical_bytes,
                arbitrary_limits,
            )?,
        );
    mark_arbitrary_partition_formula_box_reserve_observed_for_test();
    let formula = ArbitraryDirectBadFormula::compile(atoms, ranges, formula_limits)
        .map_err(map_arbitrary_formula_error)?;
    if formula.stats().atoms() != atoms.len()
        || formula.stats().clauses() != ranges.len()
        || formula.stats().atom_storage_bytes() != atom_storage
        || formula.stats().clause_storage_bytes() != clause_storage
    {
        return Err(AffineWhenBadRelativeCaseError::CaseStateMismatch);
    }
    stats.bad_clauses = ranges.len();
    stats.bad_atoms = atoms.len();
    Ok(formula)
}

fn arbitrary_formula_limits(
    limits: AffineWhenBadRelativeCaseLimits,
) -> ArbitraryDirectBadFormulaLimits {
    ArbitraryDirectBadFormulaLimits {
        max_atoms: limits.max_bad_atoms,
        max_clauses: limits.max_bad_clauses,
        max_atom_storage_bytes: usize::MAX,
        max_clause_storage_bytes: usize::MAX,
        max_retained_owned_logical_bytes: usize::MAX,
        max_compilation_owned_logical_peak_upper_bound: usize::MAX,
        // These are cumulative partition limits, charged before every route.
        max_route_clause_visits: usize::MAX,
        max_route_atom_queries: usize::MAX,
    }
}

fn map_arbitrary_formula_error(
    error: ArbitraryDirectBadFormulaError,
) -> AffineWhenBadRelativeCaseError {
    match error {
        ArbitraryDirectBadFormulaError::MalformedClauseRange { clause_ordinal, .. } => {
            AffineWhenBadRelativeCaseError::MalformedFormulaClause { clause_ordinal }
        }
        ArbitraryDirectBadFormulaError::UncoveredAtomTail { .. } => {
            AffineWhenBadRelativeCaseError::MalformedFormulaClause {
                clause_ordinal: usize::MAX,
            }
        }
        ArbitraryDirectBadFormulaError::ResourceLimit {
            resource,
            requested,
            limit,
        } => AffineWhenBadRelativeCaseError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        ArbitraryDirectBadFormulaError::ResourceCountOverflow { resource } => {
            AffineWhenBadRelativeCaseError::ResourceCountOverflow { resource }
        }
        ArbitraryDirectBadFormulaError::AllocationFailure {
            resource,
            requested,
        }
        | ArbitraryDirectBadFormulaError::NonExactAllocation {
            resource,
            requested,
            ..
        } => AffineWhenBadRelativeCaseError::AllocationFailure {
            resource,
            requested,
        },
        ArbitraryDirectBadFormulaError::SchemaMismatch
        | ArbitraryDirectBadFormulaError::PayloadMismatch => {
            AffineWhenBadRelativeCaseError::CaseStateMismatch
        }
        ArbitraryDirectBadFormulaError::ReplayMismatch => {
            AffineWhenBadRelativeCaseError::ReplayMismatch
        }
    }
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

fn build_partition_kernel(
    context: &ParametricCoefficientContext,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
    formula: RelativeDirectFormulaView<'_>,
    retained_layout: RelativePartitionRetainedLayout,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: &mut AffineWhenBadRelativeCaseStats,
) -> Result<
    (
        Vec<AffineWhenBadArbitraryRelativeSplit>,
        Vec<AffineWhenBadArbitraryRelativeCase>,
        Vec<AffineWhenBadArbitraryRelativeLeafClassification>,
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
    if let RelativePartitionRetainedLayout::ArbitraryTableIndexed {
        max_work_owned_logical_peak_upper_bound,
        max_compiler_owned_logical_peak_upper_bound,
        source_problem_owned_logical_bytes,
        max_compilation_owned_logical_peak_upper_bound,
    } = retained_layout
    {
        let mut root_stats = *stats;
        root_stats.live_leaves = 1;
        root_stats.case_ids = 1;
        root_stats.work_decision_cells = structural_loci.len();
        let work_peak = arbitrary_work_owned_logical_peak_upper_bound(root_stats)?;
        check_limit(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            work_peak,
            max_work_owned_logical_peak_upper_bound,
        )?;
        let compiler_peak = checked_add(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            root_stats.retained_bytes,
            work_peak,
        )?;
        check_limit(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            compiler_peak,
            max_compiler_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            checked_add(
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                source_problem_owned_logical_bytes,
                compiler_peak,
            )?,
            max_compilation_owned_logical_peak_upper_bound,
        )?;
    }

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
        case: AffineWhenBadArbitraryRelativeCase {
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
            route_arbitrary_formula(
                context,
                structural_loci,
                formula,
                retained_layout,
                &work_case.decisions,
                &mut divisibility_cache,
                stats,
                limits,
            )?
        };
        match route {
            RelativePartitionRoute::Bad { clause_ordinal } => {
                retain_classification(
                    &mut disposition_slots,
                    case_index,
                    AffineWhenBadArbitraryRelativeLeafClassification {
                        case: case_id,
                        decisive_clause_ordinal: Some(clause_ordinal),
                    },
                    stats,
                    limits,
                )?;
            }
            RelativePartitionRoute::Good => {
                retain_classification(
                    &mut disposition_slots,
                    case_index,
                    AffineWhenBadArbitraryRelativeLeafClassification {
                        case: case_id,
                        decisive_clause_ordinal: None,
                    },
                    stats,
                    limits,
                )?;
            }
            RelativePartitionRoute::Split {
                clause_ordinal,
                clause_atom_ordinal,
                atom_ordinal,
                atom,
            } => split_work_case(
                structural_loci,
                &mut slots,
                &mut disposition_slots,
                &mut work,
                &mut splits,
                case_id,
                AffineWhenBadArbitraryRelativeSplitTrigger {
                    clause_ordinal,
                    clause_atom_ordinal,
                    atom_ordinal,
                    atom,
                },
                retained_layout,
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

fn route_arbitrary_formula(
    context: &ParametricCoefficientContext,
    structural_loci: &[ParametricPolynomial],
    formula: RelativeDirectFormulaView<'_>,
    retained_layout: RelativePartitionRetainedLayout,
    decisions: &[Option<SymbolicPolynomialPredicateKind>],
    divisibility_cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<RelativePartitionRoute, AffineWhenBadRelativeCaseError> {
    let evaluations = checked_bounded_add(
        "affine WhenBad relative direct bad-formula evaluations",
        stats.direct_bad_formula_evaluations,
        1,
        limits.max_direct_bad_formula_evaluations,
    )?;
    let clause_visits = checked_bounded_add(
        "affine WhenBad relative direct bad-formula clause visits",
        stats.direct_bad_formula_clause_visits,
        formula.clause_visit_bound(),
        limits.max_direct_bad_formula_clause_visits,
    )?;
    let atom_queries = checked_bounded_add(
        "affine WhenBad relative direct bad-formula atom truth queries",
        stats.direct_bad_formula_atom_truth_queries,
        formula.atom_query_bound(),
        limits.max_direct_bad_formula_atom_truth_queries,
    )?;
    stats.direct_bad_formula_evaluations = evaluations;
    stats.direct_bad_formula_clause_visits = clause_visits;
    stats.direct_bad_formula_atom_truth_queries = atom_queries;

    let mut atom_truth = |atom: AffineWhenBadAtom| {
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
                retained_layout,
                divisibility_cache,
                stats,
                limits,
            )?,
        };
        Ok::<_, AffineWhenBadRelativeCaseError>(match decided {
            Some(kind) if kind == atom.kind => RelativePartitionTruth::True,
            Some(_) => RelativePartitionTruth::False,
            None => RelativePartitionTruth::Unknown,
        })
    };

    match formula {
        RelativeDirectFormulaView::Legacy(formula) => {
            let route = route_direct_bad_formula(
                formula
                    .clauses
                    .iter()
                    .map(AffineWhenBadFormulaClause::direct),
                |atom| -> Result<DirectBadFormulaTruth, AffineWhenBadRelativeCaseError> {
                    Ok(match atom_truth(atom)? {
                        RelativePartitionTruth::False => DirectBadFormulaTruth::False,
                        RelativePartitionTruth::True => DirectBadFormulaTruth::True,
                        RelativePartitionTruth::Unknown => DirectBadFormulaTruth::Unknown,
                    })
                },
            )?;
            match route {
                DirectBadFormulaRoute::Bad { clause_ordinal } => {
                    Ok(RelativePartitionRoute::Bad { clause_ordinal })
                }
                DirectBadFormulaRoute::Good => Ok(RelativePartitionRoute::Good),
                DirectBadFormulaRoute::Split {
                    clause_ordinal,
                    atom,
                } => {
                    let clause = formula
                        .clauses
                        .get(clause_ordinal)
                        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
                    let clause_atom_ordinal = clause
                        .atoms()
                        .position(|candidate| candidate == atom)
                        .ok_or(AffineWhenBadRelativeCaseError::CaseStateMismatch)?;
                    let atom_ordinal = formula.clauses[..clause_ordinal]
                        .iter()
                        .try_fold(0usize, |total, clause| {
                            checked_add(
                                "affine WhenBad relative bad atoms",
                                total,
                                clause.direct().atom_count(),
                            )
                        })?
                        .checked_add(clause_atom_ordinal)
                        .ok_or(AffineWhenBadRelativeCaseError::ResourceCountOverflow {
                            resource: "affine WhenBad relative bad atoms",
                        })?;
                    Ok(RelativePartitionRoute::Split {
                        clause_ordinal,
                        clause_atom_ordinal,
                        atom_ordinal,
                        atom,
                    })
                }
            }
        }
        RelativeDirectFormulaView::Arbitrary(formula) => Ok(
            match formula.route(
                |atom| -> Result<ArbitraryDirectBadFormulaTruth, AffineWhenBadRelativeCaseError> {
                    Ok(match atom_truth(atom)? {
                        RelativePartitionTruth::False => ArbitraryDirectBadFormulaTruth::False,
                        RelativePartitionTruth::True => ArbitraryDirectBadFormulaTruth::True,
                        RelativePartitionTruth::Unknown => ArbitraryDirectBadFormulaTruth::Unknown,
                    })
                },
            )? {
                ArbitraryDirectBadFormulaRoute::Bad { clause_ordinal } => {
                    RelativePartitionRoute::Bad { clause_ordinal }
                }
                ArbitraryDirectBadFormulaRoute::Good => RelativePartitionRoute::Good,
                ArbitraryDirectBadFormulaRoute::Split {
                    clause_ordinal,
                    clause_atom_ordinal,
                    atom_ordinal,
                    atom,
                } => RelativePartitionRoute::Split {
                    clause_ordinal,
                    clause_atom_ordinal,
                    atom_ordinal,
                    atom,
                },
            },
        ),
    }
}

#[cfg(test)]
fn route_formula(
    context: &ParametricCoefficientContext,
    structural_loci: &[ParametricPolynomial],
    formula: &AffineWhenBadDirectFormula,
    decisions: &[Option<SymbolicPolynomialPredicateKind>],
    divisibility_cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<DirectBadFormulaRoute<AffineWhenBadAtom>, AffineWhenBadRelativeCaseError> {
    Ok(
        match route_arbitrary_formula(
            context,
            structural_loci,
            RelativeDirectFormulaView::Legacy(formula),
            RelativePartitionRetainedLayout::LegacyPolynomialRich,
            decisions,
            divisibility_cache,
            stats,
            limits,
        )? {
            RelativePartitionRoute::Bad { clause_ordinal } => {
                DirectBadFormulaRoute::Bad { clause_ordinal }
            }
            RelativePartitionRoute::Good => DirectBadFormulaRoute::Good,
            RelativePartitionRoute::Split {
                clause_ordinal,
                atom,
                ..
            } => DirectBadFormulaRoute::Split {
                clause_ordinal,
                atom,
            },
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
    retained_layout: RelativePartitionRetainedLayout,
    cache: &mut Vec<LocusDivisibilityCacheEntry>,
    stats: &mut AffineWhenBadRelativeCaseStats,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<Option<SymbolicPolynomialPredicateKind>, AffineWhenBadRelativeCaseError> {
    if matches!(
        retained_layout,
        RelativePartitionRetainedLayout::ArbitraryTableIndexed { .. }
    ) {
        // Symbolica owns the K[n] divisibility algebra, but its current public
        // quotient API exposes no pre-allocation native GCD/quotient workspace
        // census. The arbitrary compiler promises an aggregate owned-memory
        // ceiling, so it must not enter that unbounded optimization. Exact
        // splitting remains complete; only implication-based pruning is
        // deferred. The public V1 compatibility path retains its historical
        // behavior and resource contract.
        return Ok(None);
    }
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
                retained_layout,
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
                retained_layout,
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
    retained_layout: RelativePartitionRetainedLayout,
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
    if let RelativePartitionRetainedLayout::ArbitraryTableIndexed {
        max_work_owned_logical_peak_upper_bound,
        max_compiler_owned_logical_peak_upper_bound,
        source_problem_owned_logical_bytes,
        max_compilation_owned_logical_peak_upper_bound,
    } = retained_layout
    {
        let mut staged = *stats;
        staged.locus_divisibility_cache_entries = prospective_entries;
        let work_owned_logical_peak_upper_bound =
            arbitrary_work_owned_logical_peak_upper_bound(staged)?;
        check_limit(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            work_owned_logical_peak_upper_bound,
            max_work_owned_logical_peak_upper_bound,
        )?;
        check_arbitrary_compiler_and_global_peak_limits(
            checked_add(
                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                staged.retained_bytes,
                work_owned_logical_peak_upper_bound,
            )?,
            source_problem_owned_logical_bytes,
            max_compiler_owned_logical_peak_upper_bound,
            max_compilation_owned_logical_peak_upper_bound,
        )?;
        mark_arbitrary_partition_divisibility_cache_reserve_observed_for_test();
    }
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

fn materialize_legacy_partition(
    structural_loci: &[ParametricPolynomial],
    formula: &AffineWhenBadDirectFormula,
    kernel_splits: Vec<AffineWhenBadArbitraryRelativeSplit>,
    kernel_cases: Vec<AffineWhenBadArbitraryRelativeCase>,
    kernel_classifications: Vec<AffineWhenBadArbitraryRelativeLeafClassification>,
) -> Result<
    (
        Vec<AffineWhenBadRelativeSplit>,
        Vec<AffineWhenBadRelativeCase>,
        Vec<AffineWhenBadRelativeLeafClassification>,
    ),
    AffineWhenBadRelativeCaseError,
> {
    let mut splits = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative compatibility splits",
        &mut splits,
        kernel_splits.len(),
    )?;
    for split in kernel_splits {
        let source = structural_loci
            .get(split.trigger.atom.locus_ordinal)
            .ok_or(AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                locus_ordinal: split.trigger.atom.locus_ordinal,
            })?;
        splits.push(AffineWhenBadRelativeSplit {
            ordinal: split.ordinal,
            parent: split.parent,
            trigger: AffineWhenBadRelativeSplitTrigger {
                clause_ordinal: split.trigger.clause_ordinal,
                atom: split.trigger.atom,
            },
            polynomial: try_copy_polynomial(
                source,
                "affine WhenBad relative compatibility split polynomial",
            )?,
            equal_zero_child: split.equal_zero_child,
            nonzero_child: split.nonzero_child,
        });
    }

    let mut cases = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative compatibility cases",
        &mut cases,
        kernel_cases.len(),
    )?;
    for case in kernel_cases {
        let mut predicates = Vec::new();
        try_reserve_exact(
            "affine WhenBad relative compatibility case predicates",
            &mut predicates,
            case.predicates.len(),
        )?;
        for predicate in case.predicates {
            let source = structural_loci.get(predicate.locus_ordinal).ok_or(
                AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                    locus_ordinal: predicate.locus_ordinal,
                },
            )?;
            predicates.push(AffineWhenBadRelativePredicate {
                locus_ordinal: predicate.locus_ordinal,
                kind: predicate.kind,
                polynomial: try_copy_polynomial(
                    source,
                    "affine WhenBad relative compatibility predicate polynomial",
                )?,
            });
        }
        cases.push(AffineWhenBadRelativeCase {
            id: case.id,
            predicates,
        });
    }

    let mut classifications = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative compatibility classifications",
        &mut classifications,
        kernel_classifications.len(),
    )?;
    for classification in kernel_classifications {
        let disposition = match classification.decisive_clause_ordinal {
            Some(clause_ordinal) => disposition_for_bad_clause(formula, clause_ordinal)?,
            None => AffineWhenBadRelativeLeafDisposition::Applicable,
        };
        classifications.push(AffineWhenBadRelativeLeafClassification {
            case: classification.case,
            disposition,
            decisive_clause_ordinal: classification.decisive_clause_ordinal,
        });
    }
    Ok((splits, cases, classifications))
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
    disposition_slots: &mut [Option<AffineWhenBadArbitraryRelativeLeafClassification>],
    case_index: usize,
    classification: AffineWhenBadArbitraryRelativeLeafClassification,
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
    disposition_slots: &mut Vec<Option<AffineWhenBadArbitraryRelativeLeafClassification>>,
    work: &mut Vec<AffineWhenBadRelativeCaseId>,
    splits: &mut Vec<AffineWhenBadArbitraryRelativeSplit>,
    parent_id: AffineWhenBadRelativeCaseId,
    trigger: AffineWhenBadArbitraryRelativeSplitTrigger,
    retained_layout: RelativePartitionRetainedLayout,
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
    let (split_size, case_size, classification_size, predicate_size) = match retained_layout {
        RelativePartitionRetainedLayout::LegacyPolynomialRich => (
            size_of::<AffineWhenBadRelativeSplit>(),
            size_of::<AffineWhenBadRelativeCase>(),
            size_of::<AffineWhenBadRelativeLeafClassification>(),
            size_of::<AffineWhenBadRelativePredicate>(),
        ),
        RelativePartitionRetainedLayout::ArbitraryTableIndexed { .. } => (
            size_of::<AffineWhenBadArbitraryRelativeSplit>(),
            size_of::<AffineWhenBadArbitraryRelativeCase>(),
            size_of::<AffineWhenBadArbitraryRelativeLeafClassification>(),
            size_of::<AffineWhenBadArbitraryRelativePredicate>(),
        ),
    };
    let retained_container_delta = checked_add(
        "affine WhenBad relative retained bytes",
        capacity_byte_envelope(1, split_size)?,
        checked_add(
            "affine WhenBad relative retained bytes",
            capacity_byte_envelope(1, case_size)?,
            checked_add(
                "affine WhenBad relative retained bytes",
                capacity_byte_envelope(1, classification_size)?,
                capacity_byte_envelope(predicate_delta, predicate_size)?,
            )?,
        )?,
    )?;
    staged.retained_bytes = checked_bounded_add(
        "affine WhenBad relative retained bytes",
        staged.retained_bytes,
        retained_container_delta,
        limits.max_retained_bytes,
    )?;

    if matches!(
        retained_layout,
        RelativePartitionRetainedLayout::LegacyPolynomialRich
    ) {
        // The V1 compatibility certificate owns one split polynomial, every
        // final case predicate polynomial, and no table-indexed aliases.
        // Charge the exact copies before mutating the kernel transcript; the
        // compatibility adapter performs only the already-admitted copies.
        for predicate in &parent.case.predicates {
            let source = structural_loci.get(predicate.locus_ordinal).ok_or(
                AffineWhenBadRelativeCaseError::StructuralLocusOutOfRange {
                    locus_ordinal: predicate.locus_ordinal,
                },
            )?;
            charge_retained_polynomial(source, &mut staged, limits)?;
        }
        charge_retained_polynomial(polynomial, &mut staged, limits)?;
        charge_retained_polynomial(polynomial, &mut staged, limits)?;
        charge_retained_polynomial(polynomial, &mut staged, limits)?;
    }
    if let RelativePartitionRetainedLayout::ArbitraryTableIndexed {
        max_work_owned_logical_peak_upper_bound,
        max_compiler_owned_logical_peak_upper_bound,
        source_problem_owned_logical_bytes,
        max_compilation_owned_logical_peak_upper_bound,
    } = retained_layout
    {
        let work_peak = arbitrary_work_owned_logical_peak_upper_bound(staged)?;
        check_limit(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            work_peak,
            max_work_owned_logical_peak_upper_bound,
        )?;
        let compiler_peak = checked_add(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            staged.retained_bytes,
            work_peak,
        )?;
        check_limit(
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
            compiler_peak,
            max_compiler_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
            checked_add(
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                source_problem_owned_logical_bytes,
                compiler_peak,
            )?,
            max_compilation_owned_logical_peak_upper_bound,
        )?;
    }

    let mut equal_predicates = Vec::new();
    try_reserve_exact(
        "affine WhenBad relative equal-zero child predicates",
        &mut equal_predicates,
        child_depth,
    )?;
    for predicate in &parent.case.predicates {
        equal_predicates.push(*predicate);
    }
    equal_predicates.push(AffineWhenBadArbitraryRelativePredicate {
        locus_ordinal: trigger.atom.locus_ordinal,
        kind: SymbolicPolynomialPredicateKind::EqualZero,
    });

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
    parent
        .case
        .predicates
        .push(AffineWhenBadArbitraryRelativePredicate {
            locus_ordinal: trigger.atom.locus_ordinal,
            kind: SymbolicPolynomialPredicateKind::NonZero,
        });
    parent.case.id = nonzero_id;

    let equal = WorkCase {
        case: AffineWhenBadArbitraryRelativeCase {
            id: equal_id,
            predicates: equal_predicates,
        },
        decisions: equal_decisions,
    };
    slots.push(Some(equal));
    slots.push(Some(parent));
    disposition_slots.push(None);
    disposition_slots.push(None);
    splits.push(AffineWhenBadArbitraryRelativeSplit {
        ordinal: splits.len(),
        parent: parent_id,
        trigger,
        equal_zero_child: equal_id,
        nonzero_child: nonzero_id,
    });
    // Stack order is reversed so the equality child is evaluated first.
    work.push(nonzero_id);
    work.push(equal_id);
    *stats = staged;
    Ok(())
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

fn arbitrary_payload_census(
    context_fingerprint: &str,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
    formula: &ArbitraryDirectBadFormula<AffineWhenBadAtom>,
    splits: &[AffineWhenBadArbitraryRelativeSplit],
    cases: &[AffineWhenBadArbitraryRelativeCase],
    classifications: &[AffineWhenBadArbitraryRelativeLeafClassification],
) -> Result<(usize, usize, usize), AffineWhenBadRelativeCaseError> {
    formula
        .validate_payload()
        .map_err(map_arbitrary_formula_error)?;
    let mut units =
        scalar_representation_units::<AffineWhenBadArbitraryRelativePartitionCertificate>();
    for count in [
        scalar_representation_units::<AffineWhenBadRelativeCaseLimits>(),
        scalar_representation_units::<AffineWhenBadRelativeCaseStats>(),
        checked_mul(
            "affine WhenBad relative payload comparison units",
            inherited_truths.len(),
            scalar_representation_units::<AffineWhenBadInheritedTruth>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            formula.atoms().len(),
            scalar_representation_units::<AffineWhenBadAtom>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            formula.clause_count(),
            2,
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            splits.len(),
            scalar_representation_units::<AffineWhenBadArbitraryRelativeSplit>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            cases.len(),
            scalar_representation_units::<AffineWhenBadArbitraryRelativeCase>(),
        )?,
        checked_mul(
            "affine WhenBad relative payload comparison units",
            classifications.len(),
            scalar_representation_units::<AffineWhenBadArbitraryRelativeLeafClassification>(),
        )?,
    ] {
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            count,
        )?;
    }
    for case in cases {
        units = checked_add(
            "affine WhenBad relative payload comparison units",
            units,
            checked_mul(
                "affine WhenBad relative payload comparison units",
                case.predicates.len(),
                scalar_representation_units::<AffineWhenBadArbitraryRelativePredicate>(),
            )?,
        )?;
    }

    let mut bytes = context_fingerprint.len();
    let mut integer_bits = 0usize;
    for polynomial in structural_loci {
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
    Ok((units, bytes, integer_bits))
}

/// Extend the raw arbitrary payload census with the authority-bearing fields
/// that are compared only by authenticated certificates.
///
/// The raw V1 census intentionally remains unchanged.  An authenticated
/// payload additionally compares its authority discriminant, its distinct
/// certificate schema, and the expected schema of the opaque canonical-locus
/// owner.  Polynomial payload is still charged exactly once by
/// `arbitrary_payload_census`.
#[allow(clippy::too_many_arguments)]
fn authenticated_arbitrary_payload_census(
    certificate_schema: &'static str,
    expected_canonical_locus_schema: &'static str,
    context_fingerprint: &str,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
    formula: &ArbitraryDirectBadFormula<AffineWhenBadAtom>,
    splits: &[AffineWhenBadArbitraryRelativeSplit],
    cases: &[AffineWhenBadArbitraryRelativeCase],
    classifications: &[AffineWhenBadArbitraryRelativeLeafClassification],
) -> Result<(usize, usize, usize), AffineWhenBadRelativeCaseError> {
    let (mut units, mut bytes, integer_bits) = arbitrary_payload_census(
        context_fingerprint,
        structural_loci,
        inherited_truths,
        formula,
        splits,
        cases,
        classifications,
    )?;
    units = checked_add(
        "affine WhenBad relative payload comparison units",
        units,
        checked_add(
            "affine WhenBad relative payload comparison units",
            scalar_representation_units::<bool>(),
            checked_mul(
                "affine WhenBad relative payload comparison units",
                2,
                scalar_representation_units::<&'static str>(),
            )?,
        )?,
    )?;
    bytes = checked_add(
        "affine WhenBad relative payload comparison bytes",
        bytes,
        checked_add(
            "affine WhenBad relative payload comparison bytes",
            certificate_schema.len(),
            expected_canonical_locus_schema.len(),
        )?,
    )?;
    Ok((units, bytes, integer_bits))
}

fn arbitrary_initial_retained_byte_envelope(
    context_fingerprint_bytes: usize,
    structural_loci: &[ParametricPolynomial],
    inherited_truths: &[AffineWhenBadInheritedTruth],
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativePartitionCertificate>();
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
        capacity_byte_envelope(1, size_of::<AffineWhenBadArbitraryRelativeCase>())?,
        capacity_byte_envelope(
            1,
            size_of::<AffineWhenBadArbitraryRelativeLeafClassification>(),
        )?,
    ] {
        bytes = checked_add("affine WhenBad relative retained bytes", bytes, allocation)?;
    }
    Ok(bytes)
}

fn authenticated_arbitrary_initial_retained_byte_envelope(
    context_fingerprint_bytes: usize,
    owner: &CanonicalLocusTableOwner,
    inherited_truths: &[AffineWhenBadInheritedTruth],
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativePartitionCertificate>();
    bytes = checked_add(
        "affine WhenBad relative retained bytes",
        bytes,
        capacity_byte_envelope(context_fingerprint_bytes, size_of::<u8>())?,
    )?;
    for allocation in [
        canonical_owner_container_owned_logical_bytes(owner)?,
        capacity_byte_envelope(
            inherited_truths.len(),
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(1, size_of::<AffineWhenBadArbitraryRelativeCase>())?,
        capacity_byte_envelope(
            1,
            size_of::<AffineWhenBadArbitraryRelativeLeafClassification>(),
        )?,
    ] {
        bytes = checked_add("affine WhenBad relative retained bytes", bytes, allocation)?;
    }
    Ok(bytes)
}

/// Full no-allocation projection of the authenticated certificate storage
/// known before the canonical owner is copied. This includes the compact
/// destination owner and every retained sparse-polynomial payload, plus the
/// inherited and mandatory base-case arrays. Later formula and partition
/// growth is charged independently as it is preflighted.
fn authenticated_arbitrary_projected_initial_retained_byte_envelope(
    context_fingerprint_bytes: usize,
    owner: &CanonicalLocusTableOwner,
    inherited_truth_count: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let resource = "affine WhenBad relative retained bytes";
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativePartitionCertificate>();
    for allocation in [
        capacity_byte_envelope(context_fingerprint_bytes, size_of::<u8>())?,
        canonical_owner_projected_compact_container_owned_logical_bytes(owner)?,
        capacity_byte_envelope(
            inherited_truth_count,
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        capacity_byte_envelope(1, size_of::<AffineWhenBadArbitraryRelativeCase>())?,
        capacity_byte_envelope(
            1,
            size_of::<AffineWhenBadArbitraryRelativeLeafClassification>(),
        )?,
    ] {
        bytes = checked_add(resource, bytes, allocation)?;
    }
    for polynomial in owner.loci() {
        bytes = checked_add(
            resource,
            bytes,
            deterministic_polynomial_owned_byte_envelope(polynomial)?,
        )?;
    }
    Ok(bytes)
}

fn arbitrary_work_owned_logical_peak_upper_bound(
    stats: AffineWhenBadRelativeCaseStats,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = checked_mul(
        "affine WhenBad arbitrary work owned logical peak upper bound",
        stats.work_decision_cells,
        size_of::<Option<SymbolicPolynomialPredicateKind>>(),
    )?;
    for allocation in [
        capacity_byte_envelope(stats.case_ids, size_of::<Option<WorkCase>>())?,
        capacity_byte_envelope(
            stats.case_ids,
            size_of::<Option<AffineWhenBadArbitraryRelativeLeafClassification>>(),
        )?,
        capacity_byte_envelope(stats.case_ids, size_of::<AffineWhenBadRelativeCaseId>())?,
        capacity_byte_envelope(
            stats.locus_divisibility_cache_entries,
            size_of::<LocusDivisibilityCacheEntry>(),
        )?,
    ] {
        bytes = checked_add(
            "affine WhenBad arbitrary work owned logical peak upper bound",
            bytes,
            allocation,
        )?;
    }
    Ok(bytes)
}

fn check_arbitrary_owned_peak_limits(
    retained_bytes: usize,
    work_owned_logical_peak_upper_bound: usize,
    source_problem_owned_logical_bytes: usize,
    limits: AffineWhenBadArbitraryRelativeLimits,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad arbitrary work owned logical peak upper bound",
        work_owned_logical_peak_upper_bound,
        limits.max_work_owned_logical_peak_upper_bound,
    )?;
    let compiler_peak = checked_add(
        "affine WhenBad arbitrary compiler owned logical peak upper bound",
        retained_bytes,
        work_owned_logical_peak_upper_bound,
    )?;
    check_arbitrary_compiler_and_global_peak_limits(
        compiler_peak,
        source_problem_owned_logical_bytes,
        limits.max_compiler_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;
    Ok(compiler_peak)
}

fn check_arbitrary_compiler_and_global_peak_limits(
    compiler_owned_logical_peak_upper_bound: usize,
    source_problem_owned_logical_bytes: usize,
    max_compiler_owned_logical_peak_upper_bound: usize,
    max_compilation_owned_logical_peak_upper_bound: usize,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    check_limit(
        "affine WhenBad arbitrary compiler owned logical peak upper bound",
        compiler_owned_logical_peak_upper_bound,
        max_compiler_owned_logical_peak_upper_bound,
    )?;
    let global_peak = checked_add(
        "affine WhenBad arbitrary compilation owned logical peak upper bound",
        source_problem_owned_logical_bytes,
        compiler_owned_logical_peak_upper_bound,
    )?;
    check_limit(
        "affine WhenBad arbitrary compilation owned logical peak upper bound",
        global_peak,
        max_compilation_owned_logical_peak_upper_bound,
    )
}

fn observed_arbitrary_certificate_owned_byte_bound(
    certificate: &AffineWhenBadArbitraryRelativePartitionCertificate,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    let mut bytes = size_of::<AffineWhenBadArbitraryRelativePartitionCertificate>();
    for allocation in [
        certificate.context_fingerprint.capacity(),
        checked_mul(
            "affine WhenBad relative retained bytes",
            certificate.inherited_truths.capacity(),
            size_of::<AffineWhenBadInheritedTruth>(),
        )?,
        certificate.formula.stats().atom_storage_bytes(),
        certificate.formula.stats().clause_storage_bytes(),
        checked_mul(
            "affine WhenBad relative retained bytes",
            certificate.splits.capacity(),
            size_of::<AffineWhenBadArbitraryRelativeSplit>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            certificate.cases.capacity(),
            size_of::<AffineWhenBadArbitraryRelativeCase>(),
        )?,
        checked_mul(
            "affine WhenBad relative retained bytes",
            certificate.classifications.capacity(),
            size_of::<AffineWhenBadArbitraryRelativeLeafClassification>(),
        )?,
    ] {
        bytes = checked_add("affine WhenBad relative retained bytes", bytes, allocation)?;
    }
    match &certificate.canonical_loci {
        AffineWhenBadArbitraryCanonicalLoci::Raw(loci) => {
            bytes = checked_add(
                "affine WhenBad relative retained bytes",
                bytes,
                checked_mul(
                    "affine WhenBad relative retained bytes",
                    loci.capacity(),
                    size_of::<ParametricPolynomial>(),
                )?,
            )?;
            for polynomial in loci {
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
        }
        AffineWhenBadArbitraryCanonicalLoci::Authenticated { owner, .. } => {
            bytes = checked_add(
                "affine WhenBad relative retained bytes",
                bytes,
                canonical_owner_retained_owned_logical_bytes(owner)?,
            )?;
        }
    }
    for case in &certificate.cases {
        bytes = checked_add(
            "affine WhenBad relative retained bytes",
            bytes,
            checked_mul(
                "affine WhenBad relative retained bytes",
                case.predicates.capacity(),
                size_of::<AffineWhenBadArbitraryRelativePredicate>(),
            )?,
        )?;
    }
    Ok(bytes)
}

fn preflight_arbitrary_payload_comparison(
    certificate: &AffineWhenBadArbitraryRelativePartitionCertificate,
    limits: AffineWhenBadRelativeCaseLimits,
) -> Result<(), AffineWhenBadRelativeCaseError> {
    let (units, bytes, integer_bits) = match &certificate.canonical_loci {
        AffineWhenBadArbitraryCanonicalLoci::Raw(_) => arbitrary_payload_census(
            &certificate.context_fingerprint,
            certificate.structural_loci(),
            &certificate.inherited_truths,
            &certificate.formula,
            &certificate.splits,
            &certificate.cases,
            &certificate.classifications,
        )?,
        AffineWhenBadArbitraryCanonicalLoci::Authenticated {
            expected_schema,
            owner: _,
        } => authenticated_arbitrary_payload_census(
            certificate.schema,
            expected_schema,
            &certificate.context_fingerprint,
            certificate.structural_loci(),
            &certificate.inherited_truths,
            &certificate.formula,
            &certificate.splits,
            &certificate.cases,
            &certificate.classifications,
        )?,
    };
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
    let observed = observed_arbitrary_certificate_owned_byte_bound(certificate)?;
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

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    already_used: usize,
) -> Result<usize, AffineWhenBadRelativeCaseError> {
    check_limit(resource, already_used, limit)?;
    Ok(limit - already_used)
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
    use crate::canonical_parametric_locus_table::{
        CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA, CanonicalLocusTableBuilder,
        CanonicalLocusTableLimits,
    };
    use crate::parametric_coefficient::{
        polynomial_associate_native_boundary_calls_for_test,
        reset_polynomial_associate_native_boundary_calls_for_test,
    };

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

    fn indexed_loci(
        context: &ParametricCoefficientContext,
        count: usize,
    ) -> Vec<ParametricPolynomial> {
        (0..count)
            .map(|position| index_polynomial(context, position))
            .collect()
    }

    fn compile_arbitrary(
        context: &ParametricCoefficientContext,
        structural_loci: Vec<ParametricPolynomial>,
        inherited_truths: Vec<AffineWhenBadInheritedTruth>,
        atoms: Vec<AffineWhenBadAtom>,
        clause_ranges: Vec<Range<usize>>,
        limits: AffineWhenBadArbitraryRelativeLimits,
    ) -> Result<AffineWhenBadArbitraryRelativePartitionCertificate, AffineWhenBadRelativeCaseError>
    {
        AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            context,
            AffineWhenBadArbitraryRelativeProblem::from_preallocated(
                structural_loci,
                inherited_truths,
                atoms,
                clause_ranges,
            ),
            limits,
        )
    }

    fn canonical_locus_owner(
        context: &ParametricCoefficientContext,
        inputs: &[ParametricPolynomial],
        reserved_slots: usize,
    ) -> CanonicalLocusTableOwner {
        let mut builder = CanonicalLocusTableBuilder::try_new(
            context,
            CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA,
            reserved_slots,
            CanonicalLocusTableLimits::default(),
        )
        .unwrap();
        for polynomial in inputs {
            builder.try_intern(context, polynomial).unwrap();
        }
        builder.seal().unwrap()
    }

    fn authenticated_problem(
        context: &ParametricCoefficientContext,
        inputs: &[ParametricPolynomial],
        reserved_slots: usize,
        inherited_truths: Vec<AffineWhenBadInheritedTruth>,
        atoms: Vec<AffineWhenBadAtom>,
        clause_ranges: Vec<Range<usize>>,
    ) -> AffineWhenBadAuthenticatedArbitraryRelativeProblem {
        AffineWhenBadAuthenticatedArbitraryRelativeProblem::from_preallocated(
            canonical_locus_owner(context, inputs, reserved_slots),
            CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA,
            inherited_truths,
            atoms,
            clause_ranges,
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
    fn arbitrary_three_atom_conjunction_has_table_indexed_equality_first_transcript() {
        let context = context("affine-relative-arbitrary-width-three", 3);
        let atoms = vec![
            AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
            AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::NonZero),
            AffineWhenBadAtom::new(2, SymbolicPolynomialPredicateKind::EqualZero),
        ];
        let certificate = compile_arbitrary(
            &context,
            indexed_loci(&context, 3),
            Vec::new(),
            atoms,
            vec![0..3],
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        certificate.replay(&context).unwrap();

        assert_eq!(certificate.splits().len(), 3);
        assert_eq!(
            certificate
                .splits()
                .iter()
                .map(|split| {
                    let trigger = split.trigger();
                    (
                        split.parent().value(),
                        trigger.clause_ordinal(),
                        trigger.clause_atom_ordinal(),
                        trigger.atom_ordinal(),
                        trigger.atom().locus_ordinal(),
                        split.equal_zero_child().value(),
                        split.nonzero_child().value(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                (0, 0, 0, 0, 0, 1, 2),
                (1, 0, 1, 1, 1, 3, 4),
                (4, 0, 2, 2, 2, 5, 6),
            ],
        );
        assert_eq!(
            certificate
                .cases()
                .iter()
                .map(|case| case.id().value())
                .collect::<Vec<_>>(),
            [2, 3, 5, 6],
        );
        assert_eq!(
            certificate
                .classifications()
                .iter()
                .map(|leaf| (leaf.case().value(), leaf.decisive_clause_ordinal()))
                .collect::<Vec<_>>(),
            [(2, None), (3, None), (5, Some(0)), (6, None)],
        );
        assert_eq!(
            certificate.cases()[2]
                .predicates()
                .iter()
                .map(|predicate| (predicate.locus_ordinal(), predicate.kind()))
                .collect::<Vec<_>>(),
            [
                (0, SymbolicPolynomialPredicateKind::EqualZero),
                (1, SymbolicPolynomialPredicateKind::NonZero),
                (2, SymbolicPolynomialPredicateKind::EqualZero),
            ],
        );
    }

    #[test]
    fn arbitrary_empty_formula_empty_conjunction_and_later_true_clause_route_exactly() {
        let empty_context = context("affine-relative-arbitrary-empty-formula", 1);
        let empty_formula = compile_arbitrary(
            &empty_context,
            indexed_loci(&empty_context, 1),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert!(empty_formula.splits().is_empty());
        assert_eq!(
            empty_formula.classifications()[0].decisive_clause_ordinal(),
            None
        );

        let empty_clause = compile_arbitrary(
            &empty_context,
            indexed_loci(&empty_context, 1),
            Vec::new(),
            Vec::new(),
            vec![0..0],
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert!(empty_clause.splits().is_empty());
        assert_eq!(
            empty_clause.classifications()[0].decisive_clause_ordinal(),
            Some(0)
        );

        let context = context("affine-relative-arbitrary-later-true", 2);
        let later_true = compile_arbitrary(
            &context,
            indexed_loci(&context, 2),
            vec![AffineWhenBadInheritedTruth::new(
                1,
                SymbolicPolynomialPredicateKind::EqualZero,
            )],
            vec![
                AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
                AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::EqualZero),
            ],
            vec![0..1, 1..2],
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert!(later_true.splits().is_empty());
        assert_eq!(
            later_true.classifications()[0].decisive_clause_ordinal(),
            Some(1)
        );
    }

    #[test]
    fn arbitrary_formula_preserves_repeated_occurrences_and_rejects_malformed_ranges() {
        let context = context("affine-relative-arbitrary-occurrences", 1);
        let repeated = AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero);
        let certificate = compile_arbitrary(
            &context,
            indexed_loci(&context, 1),
            Vec::new(),
            vec![repeated, repeated, repeated],
            vec![0..3],
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.atoms(), [repeated, repeated, repeated]);
        assert_eq!(certificate.clause_range(0), Some(0..3));
        assert_eq!(certificate.splits()[0].trigger().atom_ordinal(), 0);

        assert!(matches!(
            compile_arbitrary(
                &context,
                indexed_loci(&context, 1),
                Vec::new(),
                vec![repeated],
                vec![0..0],
                AffineWhenBadArbitraryRelativeLimits::default(),
            ),
            Err(AffineWhenBadRelativeCaseError::MalformedFormulaClause { .. })
        ));
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

    fn arbitrary_three_atom_problem(
        context: &ParametricCoefficientContext,
    ) -> AffineWhenBadArbitraryRelativeProblem {
        AffineWhenBadArbitraryRelativeProblem::from_preallocated(
            indexed_loci(context, 3),
            Vec::new(),
            vec![
                AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
                AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::NonZero),
                AffineWhenBadAtom::new(2, SymbolicPolynomialPredicateKind::EqualZero),
            ],
            vec![0..3],
        )
    }

    fn arbitrary_single_locus_formula_problem(
        context: &ParametricCoefficientContext,
    ) -> AffineWhenBadArbitraryRelativeProblem {
        AffineWhenBadArbitraryRelativeProblem::from_preallocated(
            indexed_loci(context, 1),
            Vec::new(),
            vec![
                AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
                AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::NonZero),
                AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
            ],
            vec![0..3],
        )
    }

    fn arbitrary_divisibility_problem(
        context: &ParametricCoefficientContext,
    ) -> AffineWhenBadArbitraryRelativeProblem {
        AffineWhenBadArbitraryRelativeProblem::from_preallocated(
            product_loci(context),
            vec![AffineWhenBadInheritedTruth::new(
                0,
                SymbolicPolynomialPredicateKind::EqualZero,
            )],
            vec![AffineWhenBadAtom::new(
                1,
                SymbolicPolynomialPredicateKind::EqualZero,
            )],
            vec![0..1],
        )
    }

    fn arbitrary_post_structural_validation_stats(
        context: &ParametricCoefficientContext,
        problem: &AffineWhenBadArbitraryRelativeProblem,
    ) -> AffineWhenBadRelativeCaseStats {
        let mut stats = AffineWhenBadRelativeCaseStats::default();
        stats.context_fingerprint_bytes = context.fingerprint().len();
        stats.retained_bytes = arbitrary_initial_retained_byte_envelope(
            context.fingerprint().len(),
            &problem.structural_loci,
            &problem.inherited_truths,
        )
        .unwrap();
        validate_structural_loci(
            context,
            &problem.structural_loci,
            AffineWhenBadRelativeCaseLimits::default(),
            &mut stats,
            None,
        )
        .unwrap();
        stats
    }

    #[test]
    fn arbitrary_exact_resource_limits_replay_and_every_new_one_below_limit_rejects() {
        let context = context("affine-relative-arbitrary-exact-limits", 3);
        let baseline = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_three_atom_problem(&context),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        let compilation = baseline.compilation_stats();
        let source_bytes = compilation.source_problem_owned_logical_byte_envelope();
        let overall_peak = source_bytes
            .checked_add(compilation.compiler_owned_logical_peak_upper_bound())
            .unwrap();
        let exact = AffineWhenBadArbitraryRelativeLimits {
            relative: exact_limits(baseline.stats()),
            max_source_problem_owned_logical_bytes: source_bytes,
            max_formula_retained_owned_logical_bytes: compilation
                .formula_retained_owned_logical_bytes(),
            max_formula_compilation_owned_logical_peak_upper_bound: compilation
                .formula_compilation_owned_logical_peak_upper_bound(),
            max_work_owned_logical_peak_upper_bound: compilation
                .work_owned_logical_peak_upper_bound(),
            max_compiler_owned_logical_peak_upper_bound: compilation
                .compiler_owned_logical_peak_upper_bound(),
            max_compilation_owned_logical_peak_upper_bound: overall_peak,
        };
        let bounded = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_three_atom_problem(&context),
            exact,
        )
        .unwrap();
        bounded.replay(&context).unwrap();

        type Setter = fn(&mut AffineWhenBadArbitraryRelativeLimits, usize);
        let probes: [(&str, usize, Setter); 6] = [
            (
                "affine WhenBad arbitrary source problem owned logical bytes",
                source_bytes,
                |limits, value| limits.max_source_problem_owned_logical_bytes = value,
            ),
            (
                "affine WhenBad arbitrary formula retained owned logical bytes",
                compilation.formula_retained_owned_logical_bytes(),
                |limits, value| limits.max_formula_retained_owned_logical_bytes = value,
            ),
            (
                "affine WhenBad arbitrary formula compilation owned logical peak upper bound",
                compilation.formula_compilation_owned_logical_peak_upper_bound(),
                |limits, value| {
                    limits.max_formula_compilation_owned_logical_peak_upper_bound = value
                },
            ),
            (
                "affine WhenBad arbitrary work owned logical peak upper bound",
                compilation.work_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_work_owned_logical_peak_upper_bound = value,
            ),
            (
                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                compilation.compiler_owned_logical_peak_upper_bound(),
                |limits, value| limits.max_compiler_owned_logical_peak_upper_bound = value,
            ),
            (
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                overall_peak,
                |limits, value| limits.max_compilation_owned_logical_peak_upper_bound = value,
            ),
        ];
        for (resource, observed, set) in probes {
            assert!(observed > 0, "fixture must exercise {resource}");
            let mut one_below = exact;
            set(&mut one_below, observed - 1);
            assert_resource(
                AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                    &context,
                    arbitrary_three_atom_problem(&context),
                    one_below,
                )
                .unwrap_err(),
                resource,
            );
        }
    }

    #[test]
    fn arbitrary_replay_copy_limits_reject_before_every_problem_allocation() {
        let context = context("affine-relative-arbitrary-replay-copy-limits", 3);
        let compile = || {
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                AffineWhenBadArbitraryRelativeLimits::default(),
            )
            .unwrap()
        };
        let baseline = compile();
        let source = baseline
            .compilation_stats()
            .source_problem_owned_logical_byte_envelope();
        let compiler = baseline
            .compilation_stats()
            .compiler_owned_logical_peak_upper_bound();
        assert!(source > 0 && compiler > 0);

        let mut source_one_below = compile();
        source_one_below
            .limits
            .max_source_problem_owned_logical_bytes = source - 1;
        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        assert_resource(
            source_one_below.replay(&context).unwrap_err(),
            "affine WhenBad arbitrary source problem owned logical bytes",
        );
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 0);

        let mut global_one_below = compile();
        global_one_below
            .limits
            .max_compilation_owned_logical_peak_upper_bound =
            source.checked_add(compiler).unwrap() - 1;
        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        assert_resource(
            global_one_below.replay(&context).unwrap_err(),
            "affine WhenBad arbitrary compilation owned logical peak upper bound",
        );
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 0);

        let mut exact_limits = AffineWhenBadArbitraryRelativeLimits::default();
        exact_limits.max_source_problem_owned_logical_bytes = source;
        exact_limits.max_compiler_owned_logical_peak_upper_bound = compiler;
        exact_limits.max_compilation_owned_logical_peak_upper_bound =
            source.checked_add(compiler).unwrap();
        let exact = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_three_atom_problem(&context),
            exact_limits,
        )
        .unwrap();
        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        exact.replay(&context).unwrap();
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 4);
    }

    #[test]
    fn arbitrary_replay_copy_envelope_covers_large_gmp_polynomials() {
        let context = context("affine-relative-arbitrary-replay-large-gmp", 2);
        let mut huge = context.integer(2);
        for _ in 0..12 {
            huge = context.mul(&huge, &huge).unwrap();
        }
        let shifted = context.add(&context.index(0).unwrap(), &huge).unwrap();
        let loci = vec![
            context.numerator_condition(&shifted).unwrap(),
            index_polynomial(&context, 1),
        ];
        let problem = AffineWhenBadArbitraryRelativeProblem::from_preallocated(
            loci,
            vec![AffineWhenBadInheritedTruth::new(
                1,
                SymbolicPolynomialPredicateKind::NonZero,
            )],
            vec![AffineWhenBadAtom::new(
                0,
                SymbolicPolynomialPredicateKind::EqualZero,
            )],
            vec![0..1],
        );
        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert!(
            certificate.structural_loci()[0]
                .raw()
                .coefficients
                .iter()
                .any(|coefficient| matches!(coefficient, Integer::Large(_)))
        );
        assert!(
            certificate
                .compilation_stats()
                .source_problem_owned_logical_byte_envelope()
                > 0
        );
        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        certificate.replay(&context).unwrap();
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 4);
    }

    #[test]
    fn arbitrary_owned_peak_limits_reject_before_each_risky_reserve_seam() {
        let context = context("affine-relative-arbitrary-pre-reserve-limits", 3);
        let problem = arbitrary_three_atom_problem(&context);
        let initial_retained = arbitrary_initial_retained_byte_envelope(
            context.fingerprint().len(),
            &problem.structural_loci,
            &problem.inherited_truths,
        )
        .unwrap();
        assert!(initial_retained > 0);

        let mut before_context = AffineWhenBadArbitraryRelativeLimits::default();
        before_context.max_compiler_owned_logical_peak_upper_bound = initial_retained - 1;
        reset_arbitrary_partition_reserve_observations_for_test();
        assert_resource(
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                before_context,
            )
            .unwrap_err(),
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
        );
        assert_eq!(
            arbitrary_partition_reserve_observations_for_test(),
            (false, false, false, false, false, false),
        );

        let post_structural =
            arbitrary_post_structural_validation_stats(&context, &problem).retained_bytes;
        assert!(post_structural > initial_retained);
        let mut before_canonical = AffineWhenBadArbitraryRelativeLimits::default();
        before_canonical.max_compiler_owned_logical_peak_upper_bound = post_structural - 1;
        reset_arbitrary_partition_reserve_observations_for_test();
        assert_resource(
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                before_canonical,
            )
            .unwrap_err(),
            "affine WhenBad arbitrary compiler owned logical peak upper bound",
        );
        assert_eq!(
            arbitrary_partition_reserve_observations_for_test(),
            (true, false, false, false, false, false),
        );

        let inherited_validation_work =
            capacity_byte_envelope(problem.structural_loci.len(), size_of::<bool>()).unwrap();
        assert!(inherited_validation_work > 0);
        let mut before_seen = AffineWhenBadArbitraryRelativeLimits::default();
        before_seen.max_work_owned_logical_peak_upper_bound = inherited_validation_work - 1;
        reset_arbitrary_partition_reserve_observations_for_test();
        assert_resource(
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                before_seen,
            )
            .unwrap_err(),
            "affine WhenBad arbitrary work owned logical peak upper bound",
        );
        assert_eq!(
            arbitrary_partition_reserve_observations_for_test(),
            (true, true, false, false, false, false),
        );

        let baseline = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_three_atom_problem(&context),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        let mut before_formula = AffineWhenBadArbitraryRelativeLimits::default();
        before_formula.max_formula_compilation_owned_logical_peak_upper_bound = baseline
            .compilation_stats()
            .formula_compilation_owned_logical_peak_upper_bound()
            - 1;
        reset_arbitrary_partition_reserve_observations_for_test();
        assert_resource(
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                before_formula,
            )
            .unwrap_err(),
            "affine WhenBad arbitrary formula compilation owned logical peak upper bound",
        );
        assert_eq!(
            arbitrary_partition_reserve_observations_for_test(),
            (true, true, true, true, false, false),
        );

        let formula_problem = arbitrary_single_locus_formula_problem(&context);
        let formula_post_structural =
            arbitrary_post_structural_validation_stats(&context, &formula_problem).retained_bytes;
        let formula_source_bytes = formula_problem
            .retained_owned_logical_byte_bound()
            .unwrap()
            .max(
                arbitrary_replay_source_problem_owned_logical_byte_envelope(
                    &formula_problem.structural_loci,
                    formula_problem.inherited_truths.len(),
                    formula_problem.atoms.len(),
                    formula_problem.clause_ranges.len(),
                )
                .unwrap(),
            );
        let formula_preflight = ArbitraryDirectBadFormula::<AffineWhenBadAtom>::preflight_compile(
            &formula_problem.atoms,
            &formula_problem.clause_ranges,
            arbitrary_formula_limits(AffineWhenBadRelativeCaseLimits::default()),
        )
        .unwrap();
        let formula_storage = formula_preflight
            .atom_storage_bytes()
            .checked_add(formula_preflight.clause_storage_bytes())
            .unwrap();
        let formula_compilation_extra = formula_preflight
            .compilation_owned_logical_peak_upper_bound()
            .checked_sub(formula_preflight.retained_owned_logical_bytes())
            .unwrap();
        let formula_phase_compiler_peak = formula_post_structural
            .checked_add(formula_storage)
            .and_then(|value| value.checked_add(formula_compilation_extra))
            .unwrap();
        assert!(formula_phase_compiler_peak > formula_post_structural);
        for (resource, limits) in [
            (
                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                {
                    let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
                    limits.max_compiler_owned_logical_peak_upper_bound =
                        formula_phase_compiler_peak - 1;
                    limits
                },
            ),
            (
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                {
                    let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
                    limits.max_compilation_owned_logical_peak_upper_bound = formula_source_bytes
                        .checked_add(formula_phase_compiler_peak)
                        .unwrap()
                        - 1;
                    limits
                },
            ),
        ] {
            reset_arbitrary_partition_reserve_observations_for_test();
            assert_resource(
                AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                    &context,
                    arbitrary_single_locus_formula_problem(&context),
                    limits,
                )
                .unwrap_err(),
                resource,
            );
            assert_eq!(
                arbitrary_partition_reserve_observations_for_test(),
                (true, true, true, true, false, false),
            );
        }

        reset_arbitrary_partition_reserve_observations_for_test();
        let divisibility = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_divisibility_problem(&context),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert_eq!(divisibility.stats().locus_divisibility_checks(), 0);
        assert_eq!(divisibility.stats().locus_divisibility_cache_entries(), 0);
        assert_eq!(
            arbitrary_partition_reserve_observations_for_test(),
            (true, true, true, true, true, false),
            "the resource-bounded arbitrary path must not enter Symbolica's uninstrumented quotient optimization",
        );
    }

    #[test]
    fn arbitrary_compile_and_replay_panics_are_caught_at_typed_boundaries() {
        let context = context("affine-relative-arbitrary-panic", 3);
        inject_arbitrary_partition_compile_panic_for_test();
        assert!(matches!(
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                AffineWhenBadArbitraryRelativeLimits::default(),
            ),
            Err(AffineWhenBadRelativeCaseError::SymbolicaPanic {
                stage: "arbitrary relative partition compilation",
            })
        ));

        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            arbitrary_three_atom_problem(&context),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        inject_arbitrary_partition_replay_panic_for_test();
        assert!(matches!(
            certificate.replay(&context),
            Err(AffineWhenBadRelativeCaseError::SymbolicaPanic {
                stage: "arbitrary relative partition replay",
            })
        ));
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn authenticated_arbitrary_matches_raw_partition_without_pairwise_or_native_revalidation() {
        let context = context("affine-relative-authenticated-vs-raw", 3);
        let loci = indexed_loci(&context, 3);
        let atoms = vec![
            AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
            AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::NonZero),
            AffineWhenBadAtom::new(2, SymbolicPolynomialPredicateKind::EqualZero),
        ];
        // Mint authority before measuring either compiler. Pairwise work is
        // expected exactly once in this outer construction step.
        let authenticated_problem = authenticated_problem(
            &context,
            &loci,
            loci.len(),
            Vec::new(),
            atoms.clone(),
            vec![0..3],
        );

        reset_polynomial_associate_native_boundary_calls_for_test();
        let raw = AffineWhenBadArbitraryRelativePartitionCompiler::compile(
            &context,
            AffineWhenBadArbitraryRelativeProblem::from_preallocated(
                loci,
                Vec::new(),
                atoms,
                vec![0..3],
            ),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        let raw_native_calls = polynomial_associate_native_boundary_calls_for_test();
        assert_eq!(raw_native_calls, 3);
        assert_eq!(raw.stats().structural_locus_equality_comparisons(), 3);
        assert_eq!(raw.stats().structural_locus_associate_comparisons(), 3);
        assert_eq!(
            raw.schema(),
            AFFINE_WHEN_BAD_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA
        );

        reset_polynomial_associate_native_boundary_calls_for_test();
        let authenticated = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            authenticated_problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert_eq!(polynomial_associate_native_boundary_calls_for_test(), 0);
        assert_eq!(
            authenticated_arbitrary_partition_linear_validations_for_test(),
            3
        );
        assert_eq!(
            authenticated
                .stats()
                .structural_locus_equality_comparisons(),
            0
        );
        assert_eq!(
            authenticated
                .stats()
                .structural_locus_associate_comparisons(),
            0
        );
        assert_eq!(
            authenticated.schema(),
            AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA
        );

        assert_eq!(authenticated.structural_loci(), raw.structural_loci());
        assert_eq!(authenticated.inherited_truths(), raw.inherited_truths());
        assert_eq!(authenticated.atoms(), raw.atoms());
        assert_eq!(authenticated.clause_count(), raw.clause_count());
        assert_eq!(authenticated.splits(), raw.splits());
        assert_eq!(authenticated.cases(), raw.cases());
        assert_eq!(authenticated.classifications(), raw.classifications());

        let authority_unit_delta = checked_add(
            "test authenticated authority unit delta",
            scalar_representation_units::<bool>(),
            checked_mul(
                "test authenticated authority unit delta",
                2,
                scalar_representation_units::<&'static str>(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            authenticated.stats().payload_comparison_units(),
            raw.stats().payload_comparison_units() + authority_unit_delta
        );
        assert_eq!(
            authenticated.stats().payload_comparison_bytes(),
            raw.stats().payload_comparison_bytes()
                + AFFINE_WHEN_BAD_AUTHENTICATED_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA.len()
                + CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA.len()
        );

        reset_polynomial_associate_native_boundary_calls_for_test();
        authenticated.replay(&context).unwrap();
        assert_eq!(polynomial_associate_native_boundary_calls_for_test(), 0);
        assert_eq!(
            authenticated_arbitrary_partition_linear_validations_for_test(),
            3
        );
    }

    #[test]
    fn authenticated_arbitrary_failure_and_panic_return_the_exact_owner_for_retry() {
        let context = context("affine-relative-authenticated-recovery", 2);
        let loci = indexed_loci(&context, 2);
        let atoms = vec![AffineWhenBadAtom::new(
            0,
            SymbolicPolynomialPredicateKind::EqualZero,
        )];
        let problem =
            authenticated_problem(&context, &loci, loci.len(), Vec::new(), atoms, vec![0..1]);
        let mut too_small = AffineWhenBadArbitraryRelativeLimits::default();
        too_small.relative.max_structural_loci = 1;
        let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context, problem, too_small,
        )
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            AffineWhenBadRelativeCaseError::ResourceLimit {
                resource: "canonical locus authenticated copy structural loci",
                requested: 2,
                limit: 1,
            }
        ));
        let (_, problem) = failure.into_parts();
        assert_eq!(problem.canonical_loci().loci(), loci);

        inject_authenticated_arbitrary_partition_post_validation_panic_for_test();
        let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            AffineWhenBadRelativeCaseError::SymbolicaPanic {
                stage: "authenticated arbitrary relative partition compilation",
            }
        ));
        let debug = format!("{failure:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(context.fingerprint()));
        assert!(!debug.contains(&format!("{}", loci[0].raw())));

        let (_, problem) = failure.into_parts();
        assert_eq!(problem.canonical_loci().loci(), loci);
        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn authenticated_arbitrary_rejects_schema_and_context_mismatch_without_losing_authority() {
        let ctx = context("affine-relative-authenticated-identity", 1);
        let loci = indexed_loci(&ctx, 1);
        let owner = canonical_locus_owner(&ctx, &loci, 1);
        let wrong_schema_problem =
            AffineWhenBadAuthenticatedArbitraryRelativeProblem::from_preallocated(
                owner,
                "wrong-canonical-locus-schema",
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &ctx,
            wrong_schema_problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &AffineWhenBadRelativeCaseError::SchemaMismatch
        );
        let (_, recovered) = failure.into_parts();
        assert_eq!(
            recovered.canonical_loci().schema(),
            CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA
        );

        let AffineWhenBadAuthenticatedArbitraryRelativeProblem {
            canonical_loci,
            expected_canonical_locus_schema: _,
            inherited_truths,
            atoms,
            clause_ranges,
        } = recovered;
        let corrected = AffineWhenBadAuthenticatedArbitraryRelativeProblem::from_preallocated(
            canonical_loci,
            CANONICAL_PARAMETRIC_LOCUS_TABLE_V1_SCHEMA,
            inherited_truths,
            atoms,
            clause_ranges,
        );
        let foreign = context("affine-relative-authenticated-identity-foreign", 1);
        let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &foreign,
            corrected,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &AffineWhenBadRelativeCaseError::ContextMismatch
        );
        let (_, recovered) = failure.into_parts();
        let mut certificate =
            AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
                &ctx,
                recovered,
                AffineWhenBadArbitraryRelativeLimits::default(),
            )
            .unwrap();
        certificate.schema = AFFINE_WHEN_BAD_ARBITRARY_RELATIVE_PARTITION_V1_SCHEMA;
        assert_eq!(
            certificate.replay(&ctx),
            Err(AffineWhenBadRelativeCaseError::SchemaMismatch)
        );
    }

    #[test]
    fn authenticated_authority_and_schema_census_have_exact_one_below_boundaries() {
        let context = context("affine-relative-authenticated-authority-census", 1);
        let loci = indexed_loci(&context, 1);
        let make_problem = || {
            authenticated_problem(
                &context,
                &loci,
                1,
                Vec::new(),
                vec![AffineWhenBadAtom::new(
                    0,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )],
                vec![0..1],
            )
        };
        let baseline = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            make_problem(),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        for resource in [
            "affine WhenBad relative payload comparison units",
            "affine WhenBad relative payload comparison bytes",
        ] {
            let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
            if resource == "affine WhenBad relative payload comparison units" {
                limits.relative.max_payload_comparison_units =
                    baseline.stats().payload_comparison_units() - 1;
            } else {
                limits.relative.max_payload_comparison_bytes =
                    baseline.stats().payload_comparison_bytes() - 1;
            }
            let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
                &context,
                make_problem(),
                limits,
            )
            .unwrap_err();
            assert_resource(failure.into_parts().0, resource);
        }
    }

    #[test]
    fn authenticated_initial_certificate_is_preflighted_before_any_copy() {
        let context = context("affine-relative-authenticated-initial-preflight", 1);
        let loci = indexed_loci(&context, 1);
        let make_problem =
            || authenticated_problem(&context, &loci, 1, Vec::new(), Vec::new(), Vec::new());
        let probe = make_problem();
        let projected = authenticated_arbitrary_projected_initial_retained_byte_envelope(
            context.fingerprint().len(),
            probe.canonical_loci(),
            probe.inherited_truths.len(),
        )
        .unwrap();
        let source = probe.retained_owned_logical_byte_bound().unwrap();
        drop(probe);

        let mut exact = AffineWhenBadArbitraryRelativeLimits::default();
        exact.relative.max_retained_bytes = projected;
        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            make_problem(),
            exact,
        )
        .unwrap();
        assert!(certificate.stats().retained_bytes() <= projected);

        for (resource, limits) in [
            ("affine WhenBad relative retained bytes", {
                let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
                limits.relative.max_retained_bytes = projected - 1;
                limits
            }),
            (
                "affine WhenBad arbitrary compiler owned logical peak upper bound",
                {
                    let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
                    limits.max_compiler_owned_logical_peak_upper_bound = projected - 1;
                    limits
                },
            ),
            (
                "affine WhenBad arbitrary compilation owned logical peak upper bound",
                {
                    let mut limits = AffineWhenBadArbitraryRelativeLimits::default();
                    limits.max_compilation_owned_logical_peak_upper_bound =
                        source.checked_add(projected).unwrap() - 1;
                    limits
                },
            ),
        ] {
            reset_arbitrary_partition_reserve_observations_for_test();
            let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
                &context,
                make_problem(),
                limits,
            )
            .unwrap_err();
            assert_resource(failure.into_parts().0, resource);
            assert_eq!(
                arbitrary_partition_reserve_observations_for_test(),
                (false, false, false, false, false, false),
                "the full authenticated initial certificate must be rejected before allocation",
            );
        }
    }

    #[test]
    fn authenticated_replay_owner_copy_has_exact_problem_envelope_boundaries() {
        let context = context("affine-relative-authenticated-replay-copy-boundary", 2);
        let loci = indexed_loci(&context, 2);
        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            authenticated_problem(
                &context,
                &loci,
                loci.len(),
                Vec::new(),
                vec![AffineWhenBadAtom::new(
                    0,
                    SymbolicPolynomialPredicateKind::EqualZero,
                )],
                vec![0..1],
            ),
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        let admitted = certificate
            .compilation_stats()
            .source_problem_owned_logical_byte_envelope();
        let AffineWhenBadArbitraryCanonicalLoci::Authenticated { owner, .. } =
            &certificate.canonical_loci
        else {
            unreachable!();
        };
        assert_eq!(
            authenticated_arbitrary_projected_replay_problem_owned_logical_byte_envelope_from_parts(
                owner,
                certificate.inherited_truths.len(),
                certificate.formula.atoms().len(),
                certificate.formula.clause_count(),
            )
            .unwrap(),
            admitted
        );

        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        let exact = certificate
            .try_copy_authenticated_problem(&context, admitted)
            .unwrap();
        assert!(exact.retained_owned_logical_byte_bound().unwrap() <= admitted);
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 4);

        reset_arbitrary_partition_replay_problem_copy_stage_for_test();
        assert!(matches!(
            certificate.try_copy_authenticated_problem(&context, admitted - 1),
            Err(AffineWhenBadRelativeCaseError::RetainedByteEnvelopeExceeded {
                observed,
                admitted: limit,
            }) if observed == admitted && limit == admitted - 1
        ));
        assert_eq!(arbitrary_partition_replay_problem_copy_stage_for_test(), 0);
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn duplicate_heavy_authenticated_owner_replays_compactly_but_charges_source_peak() {
        let context = context("affine-relative-authenticated-duplicate-heavy", 2);
        let unique = indexed_loci(&context, 2);
        let mut duplicate_heavy = Vec::new();
        for _ in 0..24 {
            duplicate_heavy.push(unique[0].clone());
            duplicate_heavy.push(unique[1].clone());
        }
        let make_problem = || {
            authenticated_problem(
                &context,
                &duplicate_heavy,
                64,
                Vec::new(),
                vec![
                    AffineWhenBadAtom::new(0, SymbolicPolynomialPredicateKind::EqualZero),
                    AffineWhenBadAtom::new(1, SymbolicPolynomialPredicateKind::NonZero),
                ],
                vec![0..2],
            )
        };
        let source_problem = make_problem();
        assert_eq!(source_problem.canonical_loci().loci(), unique);
        let noncompact_source_bytes = source_problem.retained_owned_logical_byte_bound().unwrap();

        reset_polynomial_associate_native_boundary_calls_for_test();
        let certificate = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            source_problem,
            AffineWhenBadArbitraryRelativeLimits::default(),
        )
        .unwrap();
        assert_eq!(polynomial_associate_native_boundary_calls_for_test(), 0);
        assert!(
            certificate
                .compilation_stats()
                .source_problem_owned_logical_byte_envelope()
                < noncompact_source_bytes
        );
        reset_polynomial_associate_native_boundary_calls_for_test();
        certificate.replay(&context).unwrap();
        assert_eq!(polynomial_associate_native_boundary_calls_for_test(), 0);

        let mut one_below = AffineWhenBadArbitraryRelativeLimits::default();
        one_below.max_source_problem_owned_logical_bytes = noncompact_source_bytes - 1;
        let failure = AffineWhenBadArbitraryRelativePartitionCompiler::compile_authenticated(
            &context,
            make_problem(),
            one_below,
        )
        .unwrap_err();
        assert_resource(
            failure.into_parts().0,
            "affine WhenBad arbitrary source problem owned logical bytes",
        );
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

    #[test]
    fn tampered_arbitrary_occurrence_leaf_and_stats_do_not_replay() {
        let context = context("affine-relative-arbitrary-tamper", 3);
        let compile = || {
            AffineWhenBadArbitraryRelativePartitionCompiler::compile(
                &context,
                arbitrary_three_atom_problem(&context),
                AffineWhenBadArbitraryRelativeLimits::default(),
            )
            .unwrap()
        };

        let mut occurrence = compile();
        occurrence.splits[0].trigger.atom_ordinal = usize::MAX;
        assert!(matches!(
            occurrence.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));

        let mut leaf = compile();
        leaf.classifications[0].decisive_clause_ordinal = Some(usize::MAX);
        assert!(matches!(
            leaf.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));

        let mut stats = compile();
        stats.stats.payload_comparison_units -= 1;
        assert!(matches!(
            stats.replay(&context),
            Err(AffineWhenBadRelativeCaseError::ReplayMismatch)
        ));
    }
}
