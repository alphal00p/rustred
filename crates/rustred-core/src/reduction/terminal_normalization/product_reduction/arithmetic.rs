use super::model::{
    Terms, VacuumProductReductionError as Error, VacuumProductReductionLimits,
    VacuumProductReductionStatistics,
};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::reduction::{ReductionError, ReductionRequest};
use std::collections::{BTreeMap, HashSet, btree_map::Entry};

pub(super) fn charge(
    value: &mut usize,
    addition: usize,
    limit: Option<usize>,
    _resource: &'static str,
) -> Result<(), Error> {
    symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
    let requested = value.saturating_add(addition);
    if let Some(limit) = limit {
        if requested > limit {
            return Err(Error::ResourceLimit {
                resource: _resource,
                requested,
                limit,
            });
        }
    }
    *value = requested;
    Ok(())
}

#[derive(Default)]
pub(super) struct Conditions {
    values: Vec<Coefficient>,
    seen: HashSet<Coefficient>,
}
impl Conditions {
    pub(super) fn values(&self) -> &[Coefficient] {
        &self.values
    }
    pub fn extend(
        &mut self,
        context: &CoefficientContext,
        values: impl IntoIterator<Item = Coefficient>,
        limits: VacuumProductReductionLimits,
    ) -> Result<(), Error> {
        self.extend_with_admission(context, values, limits, |_, _| Ok(()))
    }
    pub(super) fn extend_with_admission(
        &mut self,
        context: &CoefficientContext,
        values: impl IntoIterator<Item = Coefficient>,
        limits: VacuumProductReductionLimits,
        mut admit: impl FnMut(&[Coefficient], &Coefficient) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for value in values {
            context.validate_with_limits(&value, limits.reduction.exact_algebra)?;
            if value.is_zero() {
                return Err(Error::InvalidInput("a nonzero condition vanished".into()));
            }
            if !value.denominator.is_constant() {
                self.insert_with_admission(value.denominator.clone().into(), limits, &mut admit)?;
            }
            self.insert_with_admission(value, limits, &mut admit)?;
        }
        Ok(())
    }
    pub fn denominator(
        &mut self,
        context: &CoefficientContext,
        value: &Coefficient,
        limits: VacuumProductReductionLimits,
    ) -> Result<(), Error> {
        context.validate_with_limits(value, limits.reduction.exact_algebra)?;
        if !value.denominator.is_constant() {
            self.insert(value.denominator.clone().into(), limits)?;
        }
        Ok(())
    }
    fn insert(
        &mut self,
        value: Coefficient,
        limits: VacuumProductReductionLimits,
    ) -> Result<(), Error> {
        self.insert_with_admission(value, limits, &mut |_, _| Ok(()))
    }
    fn insert_with_admission(
        &mut self,
        value: Coefficient,
        limits: VacuumProductReductionLimits,
        admit: &mut impl FnMut(&[Coefficient], &Coefficient) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if !self.seen.contains(&value) {
            let mut count = self.values.len();
            charge(
                &mut count,
                1,
                limits.diagnostic_limit(limits.max_nonzero_conditions),
                "nonzero conditions",
            )?;
            admit(&self.values, &value)?;
            self.seen.insert(value.clone());
            self.values.push(value);
        }
        Ok(())
    }
    pub fn into_values(self) -> Vec<Coefficient> {
        self.values
    }
}

pub(super) fn accumulate<K: Ord>(
    context: &CoefficientContext,
    output: &mut BTreeMap<K, Coefficient>,
    product: K,
    coefficient: Coefficient,
    limits: VacuumProductReductionLimits,
    request: &mut ReductionRequest,
    stats: &mut VacuumProductReductionStatistics,
) -> Result<(), Error> {
    accumulate_with_admission(
        context,
        output,
        product,
        coefficient,
        limits,
        request,
        stats,
        |_, _| Ok(()),
    )
}

pub(super) fn accumulate_with_admission<K: Ord>(
    context: &CoefficientContext,
    output: &mut BTreeMap<K, Coefficient>,
    product: K,
    coefficient: Coefficient,
    limits: VacuumProductReductionLimits,
    request: &mut ReductionRequest,
    stats: &mut VacuumProductReductionStatistics,
    mut admit: impl FnMut(Option<&Coefficient>, &Coefficient) -> Result<(), Error>,
) -> Result<(), Error> {
    if coefficient.is_zero() {
        return Ok(());
    }
    match output.entry(product) {
        Entry::Occupied(mut entry) => {
            stats.coalescing_additions = stats.coalescing_additions.saturating_add(1);
            request.record_coalescing_additions(
                1,
                limits
                    .reduction
                    .diagnostic_limit(limits.reduction.max_coalescing_additions),
            )?;
            let sum = context.try_add(entry.get(), &coefficient, limits.reduction.exact_algebra)?;
            if sum.is_zero() {
                entry.remove();
            } else {
                admit(Some(entry.get()), &sum)?;
                *entry.get_mut() = sum;
            }
        }
        Entry::Vacant(entry) => {
            admit(None, &coefficient)?;
            entry.insert(coefficient);
        }
    }
    if limits.reduction.diagnostics_enabled
        && output.len() > limits.reduction.max_factorization_terms
    {
        return Err(ReductionError::FactorizationTermLimit {
            requested: output.len(),
            limit: limits.reduction.max_factorization_terms,
        }
        .into());
    }
    Ok(())
}

/// The existing factorization convolution embeds IntegralKeys into parent
/// slots. Here its same checked Cartesian bound and exact operations apply to
/// sorted family-qualified products, which have no shared parent coordinate.
pub(super) fn convolve(
    context: &CoefficientContext,
    left: &Terms,
    right: &Terms,
    limits: VacuumProductReductionLimits,
    request: &mut ReductionRequest,
    stats: &mut VacuumProductReductionStatistics,
) -> Result<Terms, Error> {
    let requested = left.len().saturating_mul(right.len());
    if limits.reduction.diagnostics_enabled && requested > limits.reduction.max_factorization_terms
    {
        return Err(ReductionError::FactorizationTermLimit {
            requested,
            limit: limits.reduction.max_factorization_terms,
        }
        .into());
    }
    let mut output = BTreeMap::new();
    for (a, ca) in left {
        for (b, cb) in right {
            let mut count = a.len();
            charge(
                &mut count,
                b.len(),
                limits.diagnostic_limit(limits.max_factors_per_product),
                "factors per product",
            )?;
            let mut factors = a.clone();
            factors.extend(b.iter().cloned());
            factors.sort();
            let coefficient = context.try_mul(ca, cb, limits.reduction.exact_algebra)?;
            accumulate(
                context,
                &mut output,
                factors,
                coefficient,
                limits,
                request,
                stats,
            )?;
        }
    }
    Ok(output)
}
