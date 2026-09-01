//! Exact `q_i^2 = D_i + 1` expansion with sealed dependency feedback.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{
    IntegerRing, MultivariatePolynomial, PolyVariable, RationalPolynomialField, Z,
};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralKey, IntegralKeyError};
use crate::reduction::Reducer;

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::FactorizedProductMomentChart;
use super::resources::{
    CoefficientBudget, OutputKeyBudget, accumulate_coefficient, admit_exponent_payload,
    admit_state_key_payload, release_map_resources,
};

type RadialPolynomial = MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u32>;

pub(super) struct RadialEvaluator<'authority> {
    context: &'authority CoefficientContext,
    reducers: BTreeMap<usize, Reducer<'authority>>,
    cache: BTreeMap<(usize, i64, u64), BTreeMap<IntegralKey, Coefficient>>,
    requests: usize,
    summands: usize,
    limits: FactorizedProductMomentLimits,
}

impl<'authority> RadialEvaluator<'authority> {
    pub(super) fn try_new(
        chart: &FactorizedProductMomentChart<'authority>,
        limits: FactorizedProductMomentLimits,
    ) -> Result<Self, FactorizedProductMomentError> {
        let mut ordinals = BTreeSet::new();
        ordinals.extend(chart.dependency_by_vector.iter().copied());
        let mut reducers = BTreeMap::new();
        for ordinal in ordinals {
            let dependency = chart
                .authority
                .dependencies()
                .get(ordinal)
                .ok_or(FactorizedProductMomentError::MissingDependency { ordinal })?;
            reducers.insert(
                ordinal,
                Reducer::with_limits(dependency, limits.dependency_reduction)?,
            );
        }
        Ok(Self {
            context: chart.authority.family().coefficient_context(),
            reducers,
            cache: BTreeMap::new(),
            requests: 0,
            summands: 0,
            limits,
        })
    }

    pub(super) fn evaluate(
        &mut self,
        dependency_ordinal: usize,
        denominator_power: i64,
        radial_power: u64,
        budget: &mut CoefficientBudget,
        key_budget: &mut OutputKeyBudget,
        coalescing_additions: &mut usize,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, FactorizedProductMomentError> {
        if denominator_power < 1 {
            return Err(FactorizedProductMomentError::NonpositiveActivePower {
                vector: dependency_ordinal,
                power: denominator_power,
            });
        }
        let radial_size = usize::try_from(radial_power).map_err(|_| {
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "radial power",
            }
        })?;
        admit_limit("radial power", radial_size, self.limits.max_radial_power)?;
        let cache_key = (dependency_ordinal, denominator_power, radial_power);
        if let Some(cached) = self.cache.get(&cache_key) {
            return retain_map_clone(cached, budget, key_budget);
        }
        if !self.reducers.contains_key(&dependency_ordinal) {
            return Err(FactorizedProductMomentError::MissingDependency {
                ordinal: dependency_ordinal,
            });
        }

        let requested_states = self.cache.len().checked_add(1).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "radial states",
            },
        )?;
        admit_limit(
            "radial states",
            requested_states,
            self.limits.max_radial_states,
        )?;
        admit_state_key_payload(requested_states, 3, self.limits)?;

        let support = radial_size.checked_add(1).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "radial summands",
            },
        )?;
        let prospective_summands = self.summands.checked_add(support).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "radial summands",
            },
        )?;
        admit_limit(
            "radial summands",
            prospective_summands,
            self.limits.max_radial_summands,
        )?;
        let prospective_requests = self.requests.checked_add(support).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product dependency requests",
            },
        )?;
        admit_limit(
            "product dependency requests",
            prospective_requests,
            self.limits.max_dependency_requests,
        )?;
        let polynomial = radial_polynomial(self.context, radial_power, support, self.limits)?;
        admit_exponent_payload(polynomial.nterms(), 1, self.limits)?;
        for coefficient in &polynomial.coefficients {
            budget.retain(coefficient)?;
        }

        let mut output = BTreeMap::new();
        for (radial_coefficient, exponents) in polynomial
            .coefficients
            .iter()
            .zip(polynomial.exponents_iter())
        {
            let shift = u64::from(exponents[0]);
            let shift_i64 = i64::from(exponents[0]);
            let shifted_power = denominator_power.checked_sub(shift_i64).ok_or(
                FactorizedProductMomentError::RadialShiftOverflow {
                    denominator_power,
                    shift,
                },
            )?;
            let target = IntegralKey::try_new([shifted_power])?;
            let decomposition = self
                .reducers
                .get_mut(&dependency_ordinal)
                .ok_or(FactorizedProductMomentError::MissingDependency {
                    ordinal: dependency_ordinal,
                })?
                .reduce_unit_mass(&target)?;
            for (key, coefficient) in decomposition.terms() {
                budget.retain(coefficient)?;
                key_budget.retain(key)?;
            }
            for (master, dependency_coefficient) in decomposition.terms() {
                let contribution = self.context.try_mul(
                    radial_coefficient,
                    dependency_coefficient,
                    self.limits.exact_algebra,
                )?;
                budget.admit_temporaries([&contribution])?;
                accumulate_coefficient(
                    self.context,
                    &mut output,
                    clone_key(master)?,
                    contribution,
                    budget,
                    key_budget,
                    self.limits,
                    1,
                    coalescing_additions,
                )?;
            }
            for (key, coefficient) in decomposition.terms() {
                budget.release(coefficient)?;
                key_budget.release(key)?;
            }
        }
        for coefficient in &polynomial.coefficients {
            budget.release(coefficient)?;
        }
        self.requests = prospective_requests;
        self.summands = prospective_summands;

        let returned = retain_map_clone(&output, budget, key_budget)?;
        self.cache.insert(cache_key, output);
        Ok(returned)
    }

    pub(super) fn release_returned(
        &self,
        expansion: &BTreeMap<IntegralKey, Coefficient>,
        budget: &mut CoefficientBudget,
        key_budget: &mut OutputKeyBudget,
    ) -> Result<(), FactorizedProductMomentError> {
        release_map_resources(expansion, budget, key_budget)
    }

    pub(super) fn state_count(&self) -> usize {
        self.cache.len()
    }

    pub(super) fn request_count(&self) -> usize {
        self.requests
    }

    pub(super) fn summand_count(&self) -> usize {
        self.summands
    }

    pub(super) fn dependency_statistics(&self) -> (usize, usize) {
        self.reducers.values().fold((0, 0), |aggregate, reducer| {
            let statistics = reducer.statistics();
            (
                aggregate.0.saturating_add(statistics.rule_applications()),
                aggregate.1.saturating_add(statistics.cache_hits()),
            )
        })
    }

    pub(super) fn finish(
        mut self,
        budget: &mut CoefficientBudget,
        key_budget: &mut OutputKeyBudget,
    ) -> Result<(), FactorizedProductMomentError> {
        for expansion in self.cache.values() {
            release_map_resources(expansion, budget, key_budget)?;
        }
        self.cache.clear();
        Ok(())
    }
}

fn radial_polynomial(
    context: &CoefficientContext,
    radial_power: u64,
    expected_support: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<RadialPolynomial, FactorizedProductMomentError> {
    let native_limit = i32::MAX as u32;
    if radial_power > u64::from(native_limit) {
        return Err(
            FactorizedProductMomentError::NativePolynomialExponentLimit {
                requested: radial_power,
                limit: native_limit,
            },
        );
    }
    let native_power = usize::try_from(radial_power).map_err(|_| {
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "radial native power",
        }
    })?;
    admit_limit(
        "radial native polynomial terms",
        expected_support,
        limits.max_native_polynomial_terms,
    )?;
    admit_exponent_payload(expected_support, 1, limits)?;
    let operation_bound = expected_support
        .checked_mul(2)
        .and_then(|value| value.checked_mul(native_power.max(1)))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "radial native polynomial operations",
        })?;
    admit_limit(
        "radial native polynomial operations",
        operation_bound,
        limits.max_native_polynomial_operations,
    )?;
    let retained_rows = expected_support.checked_add(2).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "radial projected native coefficient rows",
        },
    )?;
    let integer_bits =
        native_power
            .checked_add(1)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "radial projected native coefficient bits",
            })?;
    let mut budget = CoefficientBudget::new(limits);
    let context_unit = context.one();
    budget.retain(&context_unit)?;
    // The affine polynomial has two unit coefficients and `(1+D)^r` has
    // `r+1` integer coefficients, each bounded in magnitude by `2^r`.
    // Admit their complete live output peak before asking Symbolica to
    // allocate either sparse polynomial.
    budget.admit_native_integer_envelope(retained_rows, integer_bits, &context_unit)?;
    let mut variables = Vec::new();
    variables.try_reserve_exact(1).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "radial Symbolica variables",
            requested: 1,
        }
    })?;
    variables.push(PolyVariable::Temporary(0));
    let variables = Arc::new(variables);
    let field = RationalPolynomialField::new(Z);
    let template = RadialPolynomial::new(&field, None, variables);
    let mut exponent = Vec::new();
    exponent
        .try_reserve_exact(1)
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "radial Symbolica monomial exponent",
            requested: 1,
        })?;
    exponent.push(1_u32);
    let affine = catch_unwind(AssertUnwindSafe(|| {
        &template.constant(context_unit.clone())
            + &template.monomial(context_unit.clone(), exponent)
    }))
    .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
    validate_native_coefficients(context, &affine, limits)?;
    for coefficient in &affine.coefficients {
        budget.retain(coefficient)?;
    }
    let polynomial = if native_power == 0 {
        template.constant(context_unit.clone())
    } else {
        catch_unwind(AssertUnwindSafe(|| affine.pow(native_power)))
            .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?
    };
    validate_native_coefficients(context, &polynomial, limits)?;
    for coefficient in &polynomial.coefficients {
        budget.retain(coefficient)?;
    }
    if polynomial.nterms() != expected_support {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "Symbolica returned the wrong support for (D+1)^r",
        });
    }
    for coefficient in &affine.coefficients {
        budget.release(coefficient)?;
    }
    for coefficient in &polynomial.coefficients {
        budget.release(coefficient)?;
    }
    budget.release(&context_unit)?;
    Ok(polynomial)
}

fn validate_native_coefficients(
    context: &CoefficientContext,
    polynomial: &RadialPolynomial,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for coefficient in &polynomial.coefficients {
        context.validate_with_limits(coefficient, limits.exact_algebra)?;
    }
    Ok(())
}

fn retain_map_clone(
    input: &BTreeMap<IntegralKey, Coefficient>,
    budget: &mut CoefficientBudget,
    key_budget: &mut OutputKeyBudget,
) -> Result<BTreeMap<IntegralKey, Coefficient>, FactorizedProductMomentError> {
    let mut output = BTreeMap::new();
    for (key, coefficient) in input {
        let coefficient = coefficient.clone();
        budget.retain(&coefficient)?;
        let key = clone_key(key)?;
        key_budget.retain(&key)?;
        output.insert(key, coefficient);
    }
    Ok(output)
}

fn clone_key(key: &IntegralKey) -> Result<IntegralKey, IntegralKeyError> {
    IntegralKey::try_new(key.powers().iter().copied())
}
