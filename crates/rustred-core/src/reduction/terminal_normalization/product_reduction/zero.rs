//! Native sufficient zero proofs before selected lower-program application.
//! Only the support decision is memoized; no Feynman polynomial engine or
//! topology-specific rank heuristic is duplicated here.
use std::sync::Arc;

use super::arithmetic::{Conditions, charge};
use super::model::*;
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::{
    CacheWeight, ReductionError, ReductionLimits, SharedCacheBudget, coefficient_cache_weight,
};
use crate::sector::{
    Mask,
    zero::{Analyzer, Decision},
};

type Error = VacuumProductReductionError;

impl<P: VacuumFactorProvider> VacuumProductReducer<P> {
    pub(super) fn proved_zero_conditions(
        &mut self,
        family: &IntegralFamily,
        integral: &IntegralKey,
        statistics: &mut VacuumProductReductionStatistics,
    ) -> Result<Option<Arc<Vec<Coefficient>>>, Error> {
        let support = Mask::try_from_indices(integral.powers())
            .map_err(|error| Error::InvalidInput(format!("factor support: {error}")))?;
        let key = self
            .memoize
            .then(|| (family.fingerprint().to_owned(), support.clone()));
        if let Some(key) = &key
            && let Some(decision) = self.zero_supports.get(key)
        {
            statistics.zero_cache_hits = statistics.zero_cache_hits.saturating_add(1);
            return Ok(decision.conditions(statistics));
        }
        // A support decision owns one shared cache slot, whether sufficient
        // or inconclusive. Admit that slot before invoking native analysis.
        if self.memoize {
            let census = self.cache_budget.snapshot();
            SharedCacheBudget::default().replace(
                0,
                CacheWeight::default(),
                census.integrals.checked_add(1).ok_or(
                    ReductionError::CacheResourceCountOverflow {
                        resource: "retained integrals",
                    },
                )?,
                CacheWeight {
                    coefficient_terms: census.coefficient_terms,
                    coefficient_bytes: census.coefficient_bytes,
                },
                self.limits.reduction,
            )?;
        }
        charge(
            &mut statistics.zero_analyses,
            1,
            self.limits.diagnostic_limit(self.limits.max_zero_analyses),
            "zero analyses",
        )?;
        let mut limits = self.limits.zero;
        limits.feynman.exact_algebra = self
            .limits
            .reduction
            .bounded_by(ReductionLimits {
                exact_algebra: limits.feynman.exact_algebra,
                ..self.limits.reduction
            })
            .exact_algebra;
        let analyzer = Analyzer::try_unrestricted_with_limits(family, limits)?;
        let decision = match analyzer.analyze(&support)? {
            Decision::ProvedZero(certificate) => {
                // Native construction already replayed its primitive integer
                // kernel through Symbolica. Bind the opaque certificate to
                // this exact unshifted family/support before trusting it.
                if certificate.family_fingerprint() != family.fingerprint()
                    || certificate.raw_sector() != &support
                    || certificate.effective_sector() != &support
                {
                    return Err(Error::InvalidInput(
                        "native zero certificate changed family or support".into(),
                    ));
                }
                let mut conditions = Conditions::default();
                conditions.extend(
                    &self.context,
                    certificate
                        .domain()
                        .conditions()
                        .iter()
                        .map(|condition| condition.polynomial().clone().into()),
                    self.limits,
                )?;
                ZeroSupport::ProvedZero(Arc::new(conditions.into_values()))
            }
            Decision::Inconclusive(_) => ZeroSupport::Inconclusive,
            Decision::Excluded(_) => {
                return Err(Error::InvalidInput(
                    "unrestricted factor zero analysis unexpectedly excluded a sector".into(),
                ));
            }
        };
        let result = decision.conditions(statistics);
        let Some(key) = key else {
            return Ok(result);
        };
        let weight = match &decision {
            ZeroSupport::Inconclusive => CacheWeight::default(),
            ZeroSupport::ProvedZero(conditions) => coefficient_cache_weight(conditions.iter())?,
        };
        let combined_weight = self.cache_weight.checked_add(weight)?;
        self.cache_budget
            .replace(0, CacheWeight::default(), 1, weight, self.limits.reduction)?;
        self.zero_supports.insert(key, decision);
        self.cache_weight = combined_weight;
        Ok(result)
    }
}

impl ZeroSupport {
    fn conditions(
        &self,
        statistics: &mut VacuumProductReductionStatistics,
    ) -> Option<Arc<Vec<Coefficient>>> {
        match self {
            Self::Inconclusive => None,
            Self::ProvedZero(conditions) => {
                statistics.proved_zero_factors = statistics.proved_zero_factors.saturating_add(1);
                Some(conditions.clone())
            }
        }
    }
}
