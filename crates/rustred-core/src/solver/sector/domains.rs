//! Directed exceptional-domain search for a shared campaign's missing work.
//!
//! This reuses the ordinary sector queue, not a new algebra or proof engine.

use super::{SectorRule, SectorSolution, SectorSolveError, SectorSolveOptions, SectorStats};
use crate::solver::{Case, FiniteCasePolicy, Integral, SectorEvent, SectorSolver};

/// Rules found for explicitly requested domains, NOT a completed whole sector.
///
/// All exceptional children of the requested cases are visited within the
/// stated optional rank scope. Recursive successor coverage is separate. In
/// particular, this type cannot be passed to the complete-sector encoder.
#[derive(Debug)]
pub struct SectorDomainSolution<const N: usize> {
    pub requested_cases: Vec<Case<N>>,
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
    pub rules: Vec<SectorRule<N>>,
    pub finite_residuals: Vec<Integral<N>>,
    pub stats: SectorStats,
}

impl<const N: usize> SectorSolver<'_, N> {
    /// Search only nominated missing domains, including their exceptional
    /// branches. Positive powers need not be fixed: e.g. fixing just negative
    /// indices preserves complete parametric denominator rays.
    ///
    /// A campaign should reuse its installed rules before nominating work.
    /// Arbitrary borrowed rules are deliberately not accepted here: a
    /// `SectorRule` alone does not bind source family, ordering and snapshot.
    /// To handle descendants above an input rank, pass the actual descendant
    /// scope (or no rank bound), never clip it to the original request's rank.
    pub fn solve_domains(
        &self,
        cases: Vec<Case<N>>,
        options: SectorSolveOptions,
    ) -> Result<SectorDomainSolution<N>, SectorSolveError<N>> {
        self.solve_domains_with_observer(cases, options, |_| {})
    }

    pub fn solve_domains_with_observer(
        &self,
        cases: Vec<Case<N>>,
        options: SectorSolveOptions,
        observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorDomainSolution<N>, SectorSolveError<N>> {
        // Preflight every input before the first case starts. Invalid later
        // input cannot leave an earlier rule looking like a completed job.
        for case in &cases {
            self.validate_case(case)
                .map_err(|source| SectorSolveError::Search {
                    case: case.clone(),
                    source,
                })?;
        }
        let mut admitted = Vec::new();
        for case in &cases {
            let refined = case
                .intersect_in_rank(
                    &[],
                    self.system.index_variables(),
                    self.ordering().sector(),
                    options.max_numerator_rank,
                )
                .map_err(|source| SectorSolveError::Geometry {
                    case: case.clone(),
                    source,
                })?;
            if let Some(refined) = refined {
                if super::prune_subsumed(&mut admitted, &refined).map_err(|source| {
                    SectorSolveError::Geometry {
                        case: refined.clone(),
                        source,
                    }
                })? {
                    admitted.push(refined);
                }
            }
        }
        // Broader symbolic domains precede numerical leaves, so duplicate or
        // subsumed initial nominations cannot change finite-seed chronology.
        admitted.sort_unstable_by(Case::queue_cmp);
        let SectorSolution {
            max_numerator_rank,
            finite_case_policy,
            rules,
            finite_residuals,
            stats,
        } = self.solve_case_queue(admitted, options, observe)?;
        Ok(SectorDomainSolution {
            requested_cases: cases,
            max_numerator_rank,
            finite_case_policy,
            rules,
            finite_residuals,
            stats,
        })
    }
}

#[cfg(test)]
mod tests;
