use std::collections::BTreeMap;
use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::reduction::{ReductionError, ReductionLimits};
use crate::sector::zero;
use crate::solver::FiniteCasePolicy;

use super::super::CandidateReductionError;
use super::super::preparation::shared::{
    StreamedRule, prepare_batch_from, prepare_family, prepare_shifted_p_family, prepare_terms,
    term_interner_with, validate_solution_order,
};
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

    /// [`Self::try_new`] of the p library's Custom owners: an unshifted family
    /// passes the existing unit-scale propagator predicate, a family with eps
    /// lines the p-only admission. Every other consumer keeps refusing shifted
    /// families.
    pub(crate) fn try_new_p(
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
        let shared = prepare_shifted_p_family(&family, zero_certificates, limits)?;
        Ok(Self {
            family,
            shared,
            scope,
            limits,
        })
    }
}

impl<const N: usize> CandidateOwnerContext<N> {
    /// batch 12c: the prepared right-hand side of a lazily loaded rule with
    /// target `target`, exactly as the owner's eager preparation prepares it
    /// (same checks, same admission, this context's limits); `denominators`
    /// (the rule's prepared dispatch denominators) are shared.
    pub(crate) fn prepare_rule_body(
        &self,
        target: &crate::solver::Integral<N>,
        terms: crate::solver::ExactRow<N>,
        denominators: &[Arc<crate::algebra::IndexedPolynomial>],
    ) -> Result<Vec<crate::solver::PreparedTerm<N>>, CandidateReductionError> {
        let mut interner = term_interner_with(denominators);
        prepare_terms(
            &self.shared.context,
            target,
            terms,
            self.limits,
            &mut interner,
        )
    }
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    pub fn try_new(
        context: Arc<CandidateOwnerContext<N>>,
        inputs: impl IntoIterator<Item = CandidateOwnerInput<N>>,
    ) -> Result<Self, CandidateReductionError> {
        let mut owners = BTreeMap::new();
        for mut input in inputs {
            let rules = std::mem::take(&mut input.solution.rules);
            insert_owner(&mut owners, &context, input, rules.into_iter().map(Ok))?;
        }
        Self::from_owners(context, owners)
    }

    /// [`Self::try_new`] of ONE owner whose rules are restored one at a time
    /// (t162-export: a multi-GB payload is never held in restored form next
    /// to its prepared form). `input.solution.rules` must be empty; the rules
    /// come from `rules`, in order. Same checks, same prepared owner.
    pub(crate) fn try_new_streamed<R: Into<StreamedRule<N>>>(
        context: Arc<CandidateOwnerContext<N>>,
        input: CandidateOwnerInput<N>,
        rules: impl IntoIterator<Item = Result<R, CandidateReductionError>>,
    ) -> Result<Self, CandidateReductionError> {
        if !input.solution.rules.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "a streamed candidate owner carries its rules in the stream only".into(),
            ));
        }
        let mut owners = BTreeMap::new();
        insert_owner(&mut owners, &context, input, rules)?;
        Self::from_owners(context, owners)
    }

    fn from_owners(
        context: Arc<CandidateOwnerContext<N>>,
        owners: BTreeMap<[bool; N], Arc<PreparedOwner<N>>>,
    ) -> Result<Self, CandidateReductionError> {
        if owners.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "candidate owner collection is empty".into(),
            ));
        }
        Ok(Self {
            context,
            owners,
            lineage: Arc::new(()),
            overlay_usage: Default::default(),
        })
    }
}

/// The checks and preparation of one owner input (its rules from `rules`).
fn insert_owner<const N: usize, R: Into<StreamedRule<N>>>(
    owners: &mut BTreeMap<[bool; N], Arc<PreparedOwner<N>>>,
    context: &CandidateOwnerContext<N>,
    input: CandidateOwnerInput<N>,
    rules: impl IntoIterator<Item = Result<R, CandidateReductionError>>,
) -> Result<(), CandidateReductionError> {
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
        .compare(
            &vec![0; context.coefficient_context().index_count()],
            &vec![0; context.coefficient_context().index_count()],
        )
        .map_err(ReductionError::Ordering)?;
    // = prepare_records of the one sector, with the rules streamed.
    validate_solution_order(&input.sector, &input.ordering, &input.solution.order)?;
    let mut ordinal = 0_usize;
    let (rules, terminals) = prepare_batch_from(
        &context.shared,
        input.sector,
        rules,
        input.solution.finite_residuals,
        context.limits,
        &mut ordinal,
    )?;
    owners.insert(
        input.sector,
        Arc::new(PreparedOwner {
            root: input.saved_root,
            ordering: input.ordering,
            batches: vec![Arc::new(PreparedOwnerBatch::new(rules, terminals, None))],
        }),
    );
    Ok(())
}
