//! Synthetic formulas for ownership mechanics only; no IBP provenance claim.
use super::*;
use crate::family::{IntegralFamily, IntegralKey};
use crate::identity::ParametricIbpGenerator;
use crate::reduction::ReductionLimits;
use crate::sector::OrderingPolicy;
use crate::solver::{
    CoordinateCase, ExceptionalConditions, Integral, RuleCandidate, SearchStats, SectorRule,
    SectorSolution, SectorStats, Term,
};
use std::sync::Arc;

pub(super) fn key<const N: usize>(powers: [i64; N]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
pub(super) fn rule<const N: usize>(
    family: &IntegralFamily,
    target: [i16; N],
    rhs: &[([i16; N], i64)],
) -> SectorRule<N> {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    SectorRule {
        candidate: RuleCandidate {
            case: CoordinateCase::new(target.map(Some)).unwrap().into(),
            target: Integral::numeric(target).unwrap(),
            rhs: rhs
                .iter()
                .map(|(key, factor)| Term {
                    integral: Integral::numeric(*key).unwrap(),
                    coefficient: context.integer(*factor).raw().clone(),
                })
                .collect(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    }
}
pub(super) fn input<const N: usize>(
    sector: [bool; N],
    rank: Option<u32>,
    rules: Vec<SectorRule<N>>,
    terminals: &[[i16; N]],
) -> CandidateOwnerInput<N> {
    CandidateOwnerInput {
        sector,
        saved_root: [true; N],
        ordering: OrderingPolicy::default(),
        solution: SectorSolution {
            max_numerator_rank: rank,
            finite_case_policy: Default::default(),
            rules,
            finite_residuals: terminals
                .iter()
                .map(|key| Integral::numeric(*key).unwrap())
                .collect(),
            stats: SectorStats::default(),
        },
    }
}
pub(super) fn context<const N: usize>(
    family: Arc<IntegralFamily>,
    rank: Option<u32>,
    limits: ReductionLimits,
) -> Arc<CandidateOwnerContext<N>> {
    Arc::new(
        CandidateOwnerContext::try_new(
            family,
            CandidateOwnerScope {
                max_numerator_rank: rank,
                finite_case_policy: Default::default(),
            },
            vec![],
            limits,
        )
        .unwrap(),
    )
}
pub(super) fn programs<const N: usize>(
    family: Arc<IntegralFamily>,
    rank: Option<u32>,
    inputs: Vec<CandidateOwnerInput<N>>,
    limits: ReductionLimits,
) -> Arc<CandidateOwnerPrograms<N>> {
    Arc::new(CandidateOwnerPrograms::try_new(context(family, rank, limits), inputs).unwrap())
}
