use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::super::{VacuumIntegralKey, VacuumProductLimits, VacuumProductPlan};
use super::arithmetic::{Conditions, accumulate, charge, convolve};
use super::model::*;
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::{
    CacheWeight, ReductionError, ReductionRequest, SharedCacheBudget, coefficient_cache_weight,
};
type Error = VacuumProductReductionError;

enum Row {
    Terminal(VacuumIntegralKey, Coefficient),
    Product(Vec<VacuumIntegralKey>, Coefficient),
}
enum Frame {
    Expand(VacuumIntegralKey),
    Combine {
        target: VacuumIntegralKey,
        rows: Vec<Row>,
        conditions: Conditions,
    },
}
#[derive(Default)]
struct Request {
    algebra: ReductionRequest,
    stats: VacuumProductReductionStatistics,
    active: BTreeSet<VacuumIntegralKey>,
    lower_families: BTreeSet<String>,
    stack: Vec<Frame>,
}
impl Request {
    fn push(&mut self, frame: Frame, limit: usize) -> Result<(), Error> {
        self.algebra.retain_pending_frame(limit)?;
        self.stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "product application stack",
                requested: self.stack.len().saturating_add(1),
            })?;
        self.stack.push(frame);
        Ok(())
    }
}

impl<P: VacuumFactorProvider> VacuumProductReducer<P> {
    pub fn try_new(
        provider: P,
        measure_family: Arc<IntegralFamily>,
        limits: VacuumProductReductionLimits,
    ) -> Result<Self, Error> {
        let mut reducer = Self {
            provider,
            context: measure_family.coefficient_context().clone(),
            anchor: measure_family.clone(),
            limits,
            families: BTreeMap::new(),
            cache: BTreeMap::new(),
            cache_weight: CacheWeight::default(),
            cache_budget: SharedCacheBudget::default(),
        };
        reducer.register_family(measure_family)?;
        Ok(reducer)
    }

    pub fn cache_statistics(&self) -> VacuumProductCacheStatistics {
        let census = self.cache_budget.snapshot();
        VacuumProductCacheStatistics {
            cached_factors: census.integrals,
            coefficient_terms: census.coefficient_terms,
            coefficient_bytes: census.coefficient_bytes,
            families: self.families.len(),
        }
    }
    pub fn clear_cache(&mut self) -> Result<(), Error> {
        self.cache_budget.replace(
            self.cache.len(),
            self.cache_weight,
            0,
            CacheWeight::default(),
            self.limits.reduction,
        )?;
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        self.families.clear();
        self.families
            .insert(self.anchor.fingerprint().to_owned(), self.anchor.clone());
        Ok(())
    }

    /// Normalize a linear combination of products. All input coefficients,
    /// families and keys are validated before cancellation or cached reuse.
    /// Empty factor lists represent scalar coefficients. Inherited conditions
    /// remain in the answer even when the entire expression cancels to zero.
    pub fn reduce_products(
        &mut self,
        products: impl IntoIterator<Item = (Vec<VacuumFactor>, Coefficient)>,
        nonzero_conditions: impl IntoIterator<Item = Coefficient>,
    ) -> Result<VacuumProductReduction, Error> {
        let mut request = Request::default();
        let mut conditions = Conditions::default();
        conditions.extend(&self.context, nonzero_conditions, self.limits)?;
        let mut inputs = BTreeMap::new();
        for (factors, coefficient) in products {
            charge(
                &mut request.stats.input_products,
                1,
                self.limits.max_input_products,
                "input products",
            )?;
            let mut length = 0;
            charge(
                &mut length,
                factors.len(),
                self.limits.max_factors_per_product,
                "factors per product",
            )?;
            conditions.denominator(&self.context, &coefficient, self.limits)?;
            let mut keys = Vec::new();
            for factor in factors {
                self.register_family(factor.family.clone())?;
                self.retain_family_conditions(&factor.family, &mut conditions)?;
                keys.push(self.qualify(&factor.family, factor.integral)?);
            }
            keys.sort();
            accumulate(
                &self.context,
                &mut inputs,
                keys,
                coefficient,
                self.limits,
                &mut request.algebra,
                &mut request.stats,
            )?;
        }
        let mut output = BTreeMap::new();
        for (factors, coefficient) in inputs {
            // Every surviving factor is resolved, including siblings of a
            // zero factor; unavailable provider programs never become masters.
            for factor in &factors {
                request.push(
                    Frame::Expand(factor.clone()),
                    self.limits.reduction.max_pending_frames,
                )?;
                self.run(&mut request)?;
            }
            let mut expansion = BTreeMap::from([(Vec::new(), coefficient)]);
            for factor in factors {
                let entry = self.entry(&factor)?.clone();
                conditions.extend(&self.context, entry.conditions.iter().cloned(), self.limits)?;
                expansion = convolve(
                    &self.context,
                    &expansion,
                    &entry.terms,
                    self.limits,
                    &mut request.algebra,
                    &mut request.stats,
                )?;
            }
            for (product, coefficient) in expansion {
                accumulate(
                    &self.context,
                    &mut output,
                    product,
                    coefficient,
                    self.limits,
                    &mut request.algebra,
                    &mut request.stats,
                )?;
            }
        }
        Ok(VacuumProductReduction {
            terms: output,
            nonzero_conditions: conditions.into_values(),
            families: self.families.clone(),
            statistics: request.stats,
        })
    }

    fn register_family(&mut self, family: Arc<IntegralFamily>) -> Result<(), Error> {
        if !self
            .context
            .has_same_variable_map(family.coefficient_context())
            || family.dimension() != self.anchor.dimension()
            || family.external_count() != 0
            || family.power_shifts().iter().any(|shift| !shift.is_zero())
        {
            return Err(Error::InvalidInput("factor family differs in coefficient map, dimension, vacuum kinematics or analytic shifts".into()));
        }
        if !self.families.contains_key(family.fingerprint()) {
            let mut count = self.families.len();
            charge(&mut count, 1, self.limits.max_families, "families")?;
            self.families
                .insert(family.fingerprint().to_owned(), family);
        }
        Ok(())
    }
    fn qualify(
        &self,
        family: &IntegralFamily,
        integral: IntegralKey,
    ) -> Result<VacuumIntegralKey, Error> {
        if integral.powers().len() != family.denominator_count() {
            return Err(ReductionError::WrongArity {
                expected: family.denominator_count(),
                actual: integral.powers().len(),
            }
            .into());
        }
        Ok(VacuumIntegralKey::from_family(family, integral))
    }

    fn retain_family_conditions(
        &self,
        family: &IntegralFamily,
        conditions: &mut Conditions,
    ) -> Result<(), Error> {
        conditions.extend(
            &self.context,
            family
                .domain()
                .conditions()
                .map(|condition| condition.polynomial().clone().into()),
            self.limits,
        )
    }
    fn entry(&self, key: &VacuumIntegralKey) -> Result<&Arc<CacheEntry>, Error> {
        self.cache.get(key).ok_or_else(|| {
            ReductionError::ReducerInvariant {
                detail: "factor child is absent at combine",
            }
            .into()
        })
    }

    fn run(&mut self, request: &mut Request) -> Result<(), Error> {
        while let Some(frame) = request.stack.pop() {
            request.algebra.release_pending_frame()?;
            match frame {
                Frame::Expand(target) => {
                    if request.active.contains(&target) {
                        return Err(Error::InvalidInput(
                            "cyclic product provider dependency".into(),
                        ));
                    }
                    if self.cache.contains_key(&target) {
                        request.stats.cache_hits = request.stats.cache_hits.saturating_add(1);
                        continue;
                    }
                    request.active.insert(target.clone());
                    let (rows, conditions, children) = self.expand(&target, request)?;
                    request.push(
                        Frame::Combine {
                            target,
                            rows,
                            conditions,
                        },
                        self.limits.reduction.max_pending_frames,
                    )?;
                    for child in children.into_iter().rev() {
                        request.push(
                            Frame::Expand(child),
                            self.limits.reduction.max_pending_frames,
                        )?;
                    }
                }
                Frame::Combine {
                    target,
                    rows,
                    mut conditions,
                } => {
                    let mut output = BTreeMap::new();
                    for row in rows {
                        match row {
                            Row::Terminal(terminal, coefficient) => accumulate(
                                &self.context,
                                &mut output,
                                vec![terminal],
                                coefficient,
                                self.limits,
                                &mut request.algebra,
                                &mut request.stats,
                            )?,
                            Row::Product(factors, coefficient) => {
                                let mut products = BTreeMap::from([(Vec::new(), coefficient)]);
                                for factor in factors {
                                    let entry = self.entry(&factor)?.clone();
                                    conditions.extend(
                                        &self.context,
                                        entry.conditions.iter().cloned(),
                                        self.limits,
                                    )?;
                                    products = convolve(
                                        &self.context,
                                        &products,
                                        &entry.terms,
                                        self.limits,
                                        &mut request.algebra,
                                        &mut request.stats,
                                    )?;
                                }
                                for (product, coefficient) in products {
                                    accumulate(
                                        &self.context,
                                        &mut output,
                                        product,
                                        coefficient,
                                        self.limits,
                                        &mut request.algebra,
                                        &mut request.stats,
                                    )?;
                                }
                            }
                        }
                    }
                    if !request.active.remove(&target) {
                        return Err(Error::InvalidInput(
                            "product combine target is not active".into(),
                        ));
                    }
                    let conditions = conditions.into_values();
                    let weight = coefficient_cache_weight(output.values().chain(&conditions))?;
                    let sum = self.cache_weight.checked_add(weight)?;
                    self.cache_budget.replace(
                        0,
                        CacheWeight::default(),
                        1,
                        weight,
                        self.limits.reduction,
                    )?;
                    self.cache.insert(
                        target,
                        Arc::new(CacheEntry {
                            terms: output,
                            conditions,
                        }),
                    );
                    self.cache_weight = sum;
                }
            }
        }
        Ok(())
    }

    fn expand(
        &mut self,
        target: &VacuumIntegralKey,
        request: &mut Request,
    ) -> Result<(Vec<Row>, Conditions, BTreeSet<VacuumIntegralKey>), Error> {
        let family = self
            .families
            .get(target.family_fingerprint())
            .cloned()
            .ok_or_else(|| Error::InvalidInput("factor family is missing".into()))?;
        charge(
            &mut request.stats.provider_calls,
            1,
            self.limits.max_provider_calls,
            "provider calls",
        )?;
        let census = self.cache_budget.snapshot();
        let remaining = crate::reduction::ReductionLimits {
            max_rule_applications: request
                .algebra
                .remaining_rule_applications(self.limits.reduction.max_rule_applications),
            max_coalescing_additions: request
                .algebra
                .remaining_coalescing_additions(self.limits.reduction.max_coalescing_additions),
            max_pending_frames: request
                .algebra
                .remaining_pending_frames(self.limits.reduction.max_pending_frames),
            max_cached_integrals: self
                .limits
                .reduction
                .max_cached_integrals
                .saturating_sub(census.integrals),
            max_cached_coefficient_terms: self
                .limits
                .reduction
                .max_cached_coefficient_terms
                .saturating_sub(census.coefficient_terms),
            max_cached_coefficient_bytes: self
                .limits
                .reduction
                .max_cached_coefficient_bytes
                .saturating_sub(census.coefficient_bytes),
            ..self.limits.reduction
        };
        let result = self
            .provider
            .reduce(family.clone(), target.integral(), remaining)
            .map_err(|error| Error::Provider {
                family: target.family_fingerprint().to_owned(),
                target: target.integral().clone(),
                message: error.to_string(),
            })?;
        request.stats.rule_applications = request
            .stats
            .rule_applications
            .saturating_add(result.work.rule_applications);
        request.stats.coalescing_additions = request
            .stats
            .coalescing_additions
            .saturating_add(result.work.coalescing_additions);
        request.stats.provider_cache_hits = request
            .stats
            .provider_cache_hits
            .saturating_add(result.work.cache_hits);
        request.algebra.record_rule_applications(
            result.work.rule_applications,
            self.limits.reduction.max_rule_applications,
        )?;
        request.algebra.record_coalescing_additions(
            result.work.coalescing_additions,
            self.limits.reduction.max_coalescing_additions,
        )?;
        if result.terms.len() > self.limits.reduction.max_factorization_terms {
            return Err(ReductionError::FactorizationTermLimit {
                requested: result.terms.len(),
                limit: self.limits.reduction.max_factorization_terms,
            }
            .into());
        }
        let mut conditions = Conditions::default();
        // The native family domain belongs to the cached input factor even
        // when its provider returns zero or a different declared family.
        self.retain_family_conditions(&family, &mut conditions)?;
        conditions.extend(&self.context, result.nonzero_conditions, self.limits)?;
        let mut terms = BTreeMap::new();
        for term in result.terms {
            self.register_family(term.family.clone())?;
            // Retain the returned family's domain before zero coefficients
            // or like-terminal cancellation can remove the original term.
            self.retain_family_conditions(&term.family, &mut conditions)?;
            if term.family.loop_count() != family.loop_count() {
                return Err(Error::InvalidInput(
                    "provider terminal changes loop count outside a product".into(),
                ));
            }
            let key = self.qualify(&term.family, term.integral)?;
            conditions.denominator(&self.context, &term.coefficient, self.limits)?;
            accumulate(
                &self.context,
                &mut terms,
                key,
                term.coefficient,
                self.limits,
                &mut request.algebra,
                &mut request.stats,
            )?;
        }
        let mut groups: BTreeMap<String, BTreeSet<IntegralKey>> = BTreeMap::new();
        for key in terms.keys() {
            groups
                .entry(key.family_fingerprint().to_owned())
                .or_default()
                .insert(key.integral().clone());
        }
        let mut plans = BTreeMap::new();
        for (fingerprint, keys) in groups {
            charge(
                &mut request.stats.product_terminals,
                keys.len(),
                self.limits.products.max_terminals,
                "product terminals",
            )?;
            let plan = VacuumProductPlan::prepare(
                &self.families[&fingerprint],
                &keys,
                VacuumProductLimits {
                    max_basis_candidates: self
                        .limits
                        .products
                        .max_basis_candidates
                        .saturating_sub(request.stats.basis_candidates),
                    max_canonical_candidates: self
                        .limits
                        .products
                        .max_canonical_candidates
                        .saturating_sub(request.stats.canonical_candidates),
                    ..self.limits.products
                },
            )?;
            charge(
                &mut request.stats.basis_candidates,
                plan.statistics().basis_candidates,
                self.limits.products.max_basis_candidates,
                "basis candidates",
            )?;
            charge(
                &mut request.stats.canonical_candidates,
                plan.statistics().canonical_candidates,
                self.limits.products.max_canonical_candidates,
                "canonical candidates",
            )?;
            for (fingerprint, lower) in plan.families() {
                if !request.lower_families.contains(fingerprint) {
                    let mut count = request.lower_families.len();
                    charge(
                        &mut count,
                        1,
                        self.limits.products.max_lower_families,
                        "lower families",
                    )?;
                    request.lower_families.insert(fingerprint.clone());
                }
                self.register_family(lower.clone())?;
            }
            plans.insert(fingerprint, plan);
        }
        let mut rows = Vec::new();
        let mut children = BTreeSet::new();
        for (terminal, coefficient) in terms {
            let plan = &plans[terminal.family_fingerprint()];
            if let Some(product) = plan.products().get(terminal.integral()) {
                let mut loops = 0usize;
                let mut factor_count = 0;
                charge(
                    &mut factor_count,
                    product.factors().len(),
                    self.limits.max_factors_per_product,
                    "factors per product",
                )?;
                for factor in product.factors() {
                    let child_loops = self.families[factor.family_fingerprint()].loop_count();
                    if child_loops >= family.loop_count() {
                        return Err(Error::InvalidInput(
                            "product recursion does not strictly lower loop count".into(),
                        ));
                    }
                    loops = loops
                        .checked_add(child_loops)
                        .ok_or_else(|| Error::InvalidInput("product loop count overflow".into()))?;
                    children.insert(factor.clone());
                }
                if loops != family.loop_count() {
                    return Err(Error::InvalidInput(
                        "product does not preserve total loop count".into(),
                    ));
                }
                let coefficient = self.context.try_mul(
                    &coefficient,
                    product.coefficient(),
                    self.limits.reduction.exact_algebra,
                )?;
                rows.push(Row::Product(product.factors().to_vec(), coefficient));
            } else {
                rows.push(Row::Terminal(terminal, coefficient));
            }
        }
        Ok((rows, conditions, children))
    }
}
