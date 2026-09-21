use super::CandidateReductionError;
use super::evaluator::{CandidateEvaluator, validate_entry_rank};
use super::reducer::CandidateReducer;
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::ReductionRequest;
use std::collections::BTreeMap;

impl<const N: usize> CandidateReducer<N> {
    pub(super) fn validate_entry_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        self.validate_target(target)?;
        validate_entry_rank(target, self.max_numerator_rank)
    }
    pub(super) fn validate_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        CandidateEvaluator {
            context: &self.context,
            root_sector: self.root_sector,
            ordering: self.ordering,
            rules: &[],
            source_conditions: &self.source_conditions,
            zero_sectors: &self.zero_sectors,
            limits: self.limits,
        }
        .validate_target(target)
    }
    pub(super) fn is_zero(&self, target: &IntegralKey) -> bool {
        self.zero_sectors
            .contains(&std::array::from_fn(|i| target.powers()[i] > 0))
    }
    pub(super) fn apply_candidate(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        let sector = std::array::from_fn(|i| target.powers()[i] > 0);
        let Some(rules) = self.rules.get(&sector) else {
            return Err(CandidateReductionError::Uncovered {
                target: target.clone(),
            });
        };
        CandidateEvaluator {
            context: &self.context,
            root_sector: self.root_sector,
            ordering: self.ordering,
            rules,
            source_conditions: &self.source_conditions,
            zero_sectors: &self.zero_sectors,
            limits: self.limits,
        }
        .apply(target, request, &mut self.statistics)
    }
}
