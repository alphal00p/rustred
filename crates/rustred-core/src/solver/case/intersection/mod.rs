//! Exact OR-of-affine-cases intersection of one polynomial AND conjunction.
//!
//! This cold service uses existing integer case admission and native Symbolica
//! factorization/ideal normalization. It does not approximate irreducible
//! geometry by a bounded integer search, create rules, or establish closure.

mod bilinear_integer;
mod definite_quadratic;
mod diagnostic;
mod engine;
mod integer_divisors;
mod linear_resultant;
mod model;
mod modular;
mod native;

pub use model::{
    CaseIntersectionBudget, CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits,
    CaseIntersectionResult, CaseIntersectionStats, CaseRefinement, CaseRefinementStats,
    CaseResolution, CaseResolutionReason, DeferredRegion, GraphDependent, ModularCheck,
    StuckBranchChecks, UnsupportedBranchPolicy,
};

use crate::algebra::CoefficientPolynomial;

use super::Case;

impl<const N: usize> Case<N> {
    /// Intersect an AND conjunction and return its complete exact OR of cases.
    /// Output is deterministically sorted, deduplicated and conservatively
    /// pruned by exact containment. Any unsupported sibling fails the entire
    /// operation; an empty successful `cases` vector means every branch is
    /// proved empty. Counters describe geometry work, not solver sources,
    /// closure, or master counts.
    #[allow(clippy::result_large_err)]
    pub fn intersect_many(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(
            self,
            conjunction,
            indices,
            sector,
            limits,
            None,
            UnsupportedBranchPolicy::Refuse,
            CaseRefinement::default(),
        )
    }

    /// Exact exceptional geometry within `sum(max(-n_i,0)) <= maximum`.
    ///
    /// Unsupported nonlinear branches are refined into bounded inactive-
    /// coordinate slices. Small fully rank-finite scopes may use those same
    /// slices before symbolic elimination. Positive powers remain symbolic. Native
    /// affine algebra, factorization and substitution are unchanged. Forced
    /// negative coordinates are checked against the rank before compact-key
    /// conversion, and every refinement shares the same work budget. Unsupported positive-
    /// power geometry still fails atomically. Returned equality cases can
    /// over-cover outside the explicit rank scope; this is not family closure.
    ///
    /// guard-fix (helper Y3f): NOT an emptiness test outside the rank scope. Branches beyond
    /// `maximum` are not returned, so an `Ok` with empty `cases` does NOT prove that an exceptional branch misses
    /// the case: never use it for cover, admission or covering checks (use [`Self::intersect_many`]). It has no
    /// production caller; it is crate-private so that none appears without review.
    #[allow(clippy::result_large_err)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn intersect_many_with_max_numerator_rank(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        maximum: u32,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(
            self,
            conjunction,
            indices,
            sector,
            limits,
            Some(maximum),
            UnsupportedBranchPolicy::Refuse,
            CaseRefinement::default(),
        )
    }

    /// [`Self::intersect_many`] (`max_numerator_rank` `None`) or
    /// [`Self::intersect_many_with_max_numerator_rank`] with an explicit
    /// policy for branches that no exact step resolves. `Refuse` is exactly
    /// those methods. `Defer` (S4, opt-in) returns such branches in
    /// [`CaseIntersectionResult::deferred`] instead of failing: `cases` then
    /// does NOT cover them, and a caller must keep every rule away from them.
    /// All other failures (resources, budgets, admission, invalid input)
    /// remain atomic under both policies.
    #[allow(clippy::result_large_err)]
    pub fn intersect_many_with_policy(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        max_numerator_rank: Option<u32>,
        unsupported: UnsupportedBranchPolicy,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(
            self,
            conjunction,
            indices,
            sector,
            limits,
            max_numerator_rank,
            unsupported,
            CaseRefinement::default(),
        )
    }

    /// [`Self::intersect_many_with_policy`] with the opt-in exact refinement
    /// steps of [`CaseRefinement`] (factor-first, saturation, G-split). With
    /// `CaseRefinement::default()` (every step off) this is exactly
    /// [`Self::intersect_many_with_policy`]. Results stay exact: the OR of
    /// `cases` and `deferred` (with their exclusions) is the exceptional locus.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub fn intersect_many_refined(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        max_numerator_rank: Option<u32>,
        unsupported: UnsupportedBranchPolicy,
        refinement: CaseRefinement,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(
            self,
            conjunction,
            indices,
            sector,
            limits,
            max_numerator_rank,
            unsupported,
            refinement,
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fast_path_tests;

#[cfg(test)]
mod captured_tests;

#[cfg(test)]
mod univariate_tests;

#[cfg(test)]
mod rank_tests;

#[cfg(test)]
mod rank_overflow_tests;

#[cfg(test)]
mod lex_tests;

#[cfg(test)]
mod rank_priority_tests;

#[cfg(test)]
mod overflow_consistency_tests;

#[cfg(test)]
mod t167_captured_inputs;

#[cfg(test)]
mod lift_tests;

#[cfg(test)]
mod lift_review_tests;

#[cfg(test)]
mod t167_p1_b5_captured;

#[cfg(test)]
mod defer_tests;

#[cfg(test)]
mod t167_gsplit_captured;

#[cfg(test)]
mod gsplit_tests;

#[cfg(test)]
mod t167_p1_astra_box;

#[cfg(test)]
mod defer_review_tests;
