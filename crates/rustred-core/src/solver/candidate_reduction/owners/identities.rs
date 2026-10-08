//! Saved candidate equations for joint elimination, without execution policy.
use super::{CandidateIntegralIdentity, CandidateOwnerPrograms};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::CandidateReductionError;
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Specialize every applicable saved rule belonging to this key's positive
    /// support, in saved batch/rule order. Missing owners or applicable rules
    /// return an empty list; no terminal or zero equation is inferred.
    ///
    /// Matching retains fixed/affine cases, whole exception conjunctions,
    /// source nonzero conditions, original denominators, and the saved root.
    /// As an internal-equation interface it imposes no entry numerator-rank
    /// bound. Terminal/deferred declarations, dispatch preference, and descent
    /// govern execution only and do not suppress these candidate equations.
    ///
    /// No source derivation is replayed. See [`CandidateIntegralIdentity`] for
    /// the authority boundary and sufficient generic-parameter conditions.
    pub fn applicable_identities(
        &self,
        target: &IntegralKey,
    ) -> Result<Vec<CandidateIntegralIdentity>, CandidateReductionError> {
        self.applicable_identities_in_request(
            target,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        )
    }

    /// Share arithmetic and rule budgets across a routed request's endpoints.
    pub(in crate::solver::candidate_reduction) fn applicable_identities_in_request(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<Vec<CandidateIntegralIdentity>, CandidateReductionError> {
        let arity = self.context.coefficient_context().index_count();
        if target.powers().len() != arity {
            return Err(ReductionError::WrongArity {
                expected: arity,
                actual: target.powers().len(),
            }
            .into());
        }
        let sector =
            std::array::from_fn(|axis| target.powers().get(axis).copied().unwrap_or(0) > 0);
        let Some(owner) = self.owners.get(&sector) else {
            return Ok(Vec::new());
        };
        let shared = &self.context.shared;
        let mut identities = Vec::new();
        for (batch_ordinal, batch) in owner.batches.iter().enumerate() {
            let evaluator = CandidateEvaluator {
                context: &shared.context,
                root_sector: owner.root,
                ordering: &owner.ordering,
                rules: &batch.rules,
                whole_piece_alternatives: &batch.whole_piece_alternatives,
                source_conditions: &shared.source_conditions,
                zero_sectors: &shared.zero_sectors,
                limits: self.context.limits,
            };
            for rule in &batch.rules {
                if let Some((terms, nonzero_conditions)) =
                    evaluator.applicable_identity(rule, target, request, statistics)?
                {
                    identities.push(CandidateIntegralIdentity {
                        terms,
                        nonzero_conditions,
                        owner: sector[..arity].to_vec(),
                        batch: batch_ordinal,
                        rule: rule.ordinal,
                    });
                }
            }
        }
        Ok(identities)
    }
}

#[cfg(test)]
mod tests;
