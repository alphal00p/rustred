use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::ReductionLimits;
use crate::sector::OrderingPolicy;
use crate::solver::{FiniteCasePolicy, SectorSolution};

use super::super::model::PreparedRule;
use super::super::preparation::shared::PreparedFamily;

/// Common generation scope. R bounds public entries by default, never internal
/// successors. Explicit finite entry admission may replace that default gate
/// without changing this stored generation scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateOwnerScope {
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
}

/// One original saved sector with its own root and ordering.
#[derive(Debug)]
pub struct CandidateOwnerInput<const N: usize> {
    pub sector: [bool; N],
    pub saved_root: [bool; N],
    pub ordering: OrderingPolicy,
    pub solution: SectorSolution<N>,
}

/// One native source/context preparation shared by all admitted owner records.
#[derive(Debug)]
pub struct CandidateOwnerContext<const N: usize> {
    pub(in crate::solver::candidate_reduction) family: Arc<IntegralFamily>,
    pub(in crate::solver::candidate_reduction) shared: PreparedFamily<N>,
    pub(in crate::solver::candidate_reduction) scope: CandidateOwnerScope,
    pub(in crate::solver::candidate_reduction) limits: ReductionLimits,
}

impl<const N: usize> CandidateOwnerContext<N> {
    pub fn family(&self) -> &IntegralFamily {
        &self.family
    }
    pub fn family_owner(&self) -> &Arc<IntegralFamily> {
        &self.family
    }
    pub fn coefficient_context(&self) -> &IndexedCoefficientContext {
        &self.shared.context
    }
    pub fn index_variables(&self) -> &[usize; N] {
        &self.shared.index_variables
    }
    pub fn scope(&self) -> CandidateOwnerScope {
        self.scope
    }
    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }
}

#[derive(Debug)]
pub(in crate::solver::candidate_reduction) struct PreparedOwner<const N: usize> {
    pub root: [bool; N],
    pub ordering: OrderingPolicy,
    pub batches: Vec<Arc<PreparedOwnerBatch<N>>>,
}

#[derive(Debug)]
pub(in crate::solver::candidate_reduction) struct PreparedOwnerBatch<const N: usize> {
    pub rules: Vec<PreparedRule<N>>,
    pub terminals: BTreeSet<IntegralKey>,
    pub coalescing_bound: usize,
    pub overlay: Option<super::feedback::OwnerOverlayMetadata<N>>,
}

/// Atomically admitted immutable owner collection in one common family.
#[derive(Debug)]
pub struct CandidateOwnerPrograms<const N: usize> {
    pub(in crate::solver::candidate_reduction) context: Arc<CandidateOwnerContext<N>>,
    pub(in crate::solver::candidate_reduction) owners: BTreeMap<[bool; N], Arc<PreparedOwner<N>>>,
    pub(super) lineage: Arc<()>,
    pub(super) overlay_usage: super::feedback::OverlayUsage,
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    pub(in crate::solver::candidate_reduction) fn extends(&self, previous: &Self) -> bool {
        Arc::ptr_eq(&self.lineage, &previous.lineage)
            && Arc::ptr_eq(&self.context, &previous.context)
            && self.owners.len() == previous.owners.len()
            && previous.owners.iter().all(|(sector, before)| {
                self.owners.get(sector).is_some_and(|after| {
                    before.root == after.root
                        && before.ordering == after.ordering
                        && after.batches.len() >= before.batches.len()
                        && before
                            .batches
                            .iter()
                            .zip(&after.batches)
                            .all(|(a, b)| Arc::ptr_eq(a, b))
                })
            })
    }
    pub fn context(&self) -> &Arc<CandidateOwnerContext<N>> {
        &self.context
    }
    pub fn owner_count(&self) -> usize {
        self.owners.len()
    }
    pub fn owner_sectors(&self) -> impl Iterator<Item = &[bool; N]> {
        self.owners.keys()
    }
    pub fn terminal_count(&self) -> usize {
        self.owners
            .values()
            .map(|owner| {
                let base = &owner.batches[0].terminals;
                if owner.batches.len() == 1 {
                    return base.len();
                }
                let additional: BTreeSet<_> = owner.batches[1..]
                    .iter()
                    .flat_map(|batch| &batch.terminals)
                    .filter(|key| !base.contains(*key))
                    .collect();
                base.len() + additional.len()
            })
            .sum()
    }
}
