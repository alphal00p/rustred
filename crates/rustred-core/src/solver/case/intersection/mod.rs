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
mod native;

pub use model::{
    CaseIntersectionBudget, CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits,
    CaseIntersectionResult, CaseIntersectionStats,
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
    pub fn intersect_many(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(self, conjunction, indices, sector, limits, None)
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
    pub fn intersect_many_with_max_numerator_rank(
        &self,
        conjunction: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        maximum: u32,
    ) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
        engine::intersect(self, conjunction, indices, sector, limits, Some(maximum))
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
