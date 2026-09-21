use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::ReductionLimits;
use crate::sector::OrderingPolicy;
use crate::solver::{FiniteCasePolicy, SectorSolution};

use super::super::model::PreparedRule;
use super::super::preparation::shared::PreparedFamily;

/// Common generation scope. R bounds public entries, never internal successors.
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
    pub rules: Vec<PreparedRule<N>>,
    pub terminals: BTreeSet<IntegralKey>,
}

/// Atomically admitted immutable owner collection in one common family.
#[derive(Debug)]
pub struct CandidateOwnerPrograms<const N: usize> {
    pub(in crate::solver::candidate_reduction) context: Arc<CandidateOwnerContext<N>>,
    pub(in crate::solver::candidate_reduction) owners: BTreeMap<[bool; N], PreparedOwner<N>>,
}

impl<const N: usize> CandidateOwnerPrograms<N> {
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
            .map(|owner| owner.terminals.len())
            .sum()
    }
}
