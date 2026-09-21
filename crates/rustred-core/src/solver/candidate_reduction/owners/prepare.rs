use std::collections::BTreeMap;
use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::reduction::{ReductionError, ReductionLimits};
use crate::sector::zero;
use crate::solver::FiniteCasePolicy;

use super::super::CandidateReductionError;
use super::super::preparation::shared::{prepare_family, prepare_records};
use super::model::*;

impl<const N: usize> CandidateOwnerContext<N> {
    pub fn try_new(
        family: Arc<IntegralFamily>,
        scope: CandidateOwnerScope,
        zero_certificates: Vec<zero::Certificate>,
        limits: ReductionLimits,
    ) -> Result<Self, CandidateReductionError> {
        if scope.finite_case_policy == FiniteCasePolicy::RetainRankFinite
            && scope.max_numerator_rank.is_none()
        {
            return Err(CandidateReductionError::InvalidInput(
                "retain-rank-finite owner scope requires an explicit numerator rank".into(),
            ));
        }
        let shared = prepare_family(&family, zero_certificates, limits)?;
        Ok(Self {
            family,
            shared,
            scope,
            limits,
        })
    }
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    pub fn try_new(
        context: Arc<CandidateOwnerContext<N>>,
        inputs: impl IntoIterator<Item = CandidateOwnerInput<N>>,
    ) -> Result<Self, CandidateReductionError> {
        let mut owners = BTreeMap::new();
        for input in inputs {
            if owners.contains_key(&input.sector) {
                return Err(CandidateReductionError::InvalidInput(
                    "duplicate candidate owner sector".into(),
                ));
            }
            if input
                .sector
                .iter()
                .zip(input.saved_root)
                .any(|(&active, root)| active && !root)
            {
                return Err(CandidateReductionError::InvalidInput(
                    "candidate owner lies outside its saved root".into(),
                ));
            }
            if input.solution.max_numerator_rank != context.scope.max_numerator_rank {
                return Err(CandidateReductionError::InconsistentNumeratorRank {
                    expected: context.scope.max_numerator_rank,
                    actual: input.solution.max_numerator_rank,
                });
            }
            if input.solution.finite_case_policy != context.scope.finite_case_policy {
                return Err(CandidateReductionError::InconsistentFiniteCasePolicy {
                    expected: context.scope.finite_case_policy,
                    actual: input.solution.finite_case_policy,
                });
            }
            input
                .ordering
                .compare(&[0; N], &[0; N])
                .map_err(ReductionError::Ordering)?;
            let mut prepared = prepare_records(
                &context.shared,
                BTreeMap::from([(input.sector, input.solution)]),
                context.limits,
            )?;
            let rules = prepared
                .rules
                .remove(&input.sector)
                .expect("one admitted sector");
            owners.insert(
                input.sector,
                PreparedOwner {
                    root: input.saved_root,
                    ordering: input.ordering,
                    rules,
                    terminals: prepared.terminals,
                },
            );
        }
        if owners.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "candidate owner collection is empty".into(),
            ));
        }
        Ok(Self { context, owners })
    }
}
