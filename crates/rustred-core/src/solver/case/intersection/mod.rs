//! Exact OR-of-affine-cases intersection of one polynomial AND conjunction.
//!
//! This cold service uses existing integer case admission and native Symbolica
//! factorization/ideal normalization. It does not approximate irreducible
//! geometry by a bounded integer search, create rules, or establish closure.

mod definite_quadratic;
mod diagnostic;
mod engine;
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
        engine::intersect(self, conjunction, indices, sector, limits)
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
