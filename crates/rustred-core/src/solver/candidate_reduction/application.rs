use super::CandidateReductionError;
use super::evaluator::{CandidateEvaluator, validate_entry_rank};
use super::reducer::{CandidateProgram, CandidateReducer};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::{ReductionLimits, ReductionRequest};
use std::collections::BTreeMap;

impl<const N: usize> CandidateReducer<N> {
    pub(super) fn validate_entry_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        self.validate_entry_with_limits(target, self.limits)
    }
    pub(super) fn validate_entry_with_limits(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<(), CandidateReductionError> {
        self.validate_target_with_limits(target, limits)?;
        if self.unresolved_policy == super::UnresolvedPolicy::Reject {
            validate_entry_rank(target, self.program.max_numerator_rank)?;
        }
        Ok(())
    }
    pub(super) fn validate_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        self.validate_target_with_limits(target, self.limits)
    }
    fn validate_target_with_limits(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<(), CandidateReductionError> {
        CandidateEvaluator {
            context: &self.program.context,
            root_sector: self.program.root_sector,
            ordering: &self.program.ordering,
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &self.program.source_conditions,
            zero_sectors: &self.program.zero_sectors,
            limits,
        }
        .validate_target(target)
    }
    pub(super) fn is_zero(&self, target: &IntegralKey) -> bool {
        self.program
            .zero_sectors
            .contains(&std::array::from_fn(|i| {
                target.powers().get(i).copied().unwrap_or(0) > 0
            }))
    }
    pub(super) fn apply_candidate(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        self.program
            .evaluator(target, self.limits)?
            .apply(target, request, &mut self.statistics)
    }
}

impl<const N: usize> CandidateProgram<N> {
    pub(super) fn evaluator(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<CandidateEvaluator<'_, N>, CandidateReductionError> {
        let sector = std::array::from_fn(|i| target.powers().get(i).copied().unwrap_or(0) > 0);
        let Some(rules) = self.rules.get(&sector) else {
            return Err(CandidateReductionError::Uncovered {
                target: target.clone(),
            });
        };
        Ok(CandidateEvaluator {
            context: &self.context,
            root_sector: self.root_sector,
            ordering: &self.ordering,
            rules,
            whole_piece_alternatives: self
                .whole_piece_alternatives
                .get(&sector)
                .map_or(&[], Vec::as_slice),
            source_conditions: &self.source_conditions,
            zero_sectors: &self.zero_sectors,
            limits,
        })
    }
}
