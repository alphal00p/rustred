//! Ordered immutable batches share the existing exact rule evaluator.
use super::super::model::{CandidateReductionError, PreparedRule};
use super::model::{PreparedOwner, PreparedOwnerBatch};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::solver::candidate_reduction) enum OwnerStep {
    Terminal,
    Applied(BTreeMap<IntegralKey, Coefficient>),
    Uncovered,
}

impl<const N: usize> PreparedOwnerBatch<N> {
    pub(super) fn new(
        rules: Vec<PreparedRule<N>>,
        terminals: BTreeSet<IntegralKey>,
        overlay: Option<super::feedback::OwnerOverlayMetadata<N>>,
    ) -> Self {
        let coalescing_bound = rules
            .iter()
            .map(|rule| {
                let unique: BTreeSet<_> = rule.rhs.iter().map(|term| &term.shift).collect();
                rule.rhs.len() - unique.len()
            })
            .max()
            .unwrap_or(0);
        Self {
            rules,
            terminals,
            coalescing_bound,
            overlay,
        }
    }
}

impl<const N: usize> PreparedOwner<N> {
    /// A later terminal must never shadow an earlier applicable formula.
    /// The callback wraps the one existing evaluator, optionally with aggregate
    /// reservation accounting. Only exact Uncovered advances to the next batch.
    pub(in crate::solver::candidate_reduction) fn evaluate_step<E>(
        &self,
        key: &IntegralKey,
        mut evaluate: impl FnMut(
            &PreparedOwnerBatch<N>,
        ) -> Result<
            Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError>,
            E,
        >,
    ) -> Result<OwnerStep, E>
    where
        E: From<CandidateReductionError>,
    {
        for batch in &self.batches {
            if batch.terminals.contains(key) {
                return Ok(OwnerStep::Terminal);
            }
            match evaluate(batch)? {
                Ok(terms) => return Ok(OwnerStep::Applied(terms)),
                Err(CandidateReductionError::Uncovered { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(OwnerStep::Uncovered)
    }
}
