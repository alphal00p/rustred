//! Exact composition of one correlated closed block with independent K1 blocks.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{
    IntegerRing, MultivariatePolynomial, PolyVariable, RationalPolynomialField, Z,
};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{DenominatorExpansion, IntegralKey, ScalarProductCoordinate};
use crate::reduction::Reducer;

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::evaluate::{
    ProductPolynomial, clone_i64_vec, clone_key, clone_u32_as_u64, multiset_support,
    validate_native_coefficients, zero_parent_key,
};
use super::limits::FactorizedProductMomentLimits;
use super::model::{
    CorrelatedProductBlock, FactorizedProductMomentChart, ProductMomentExpansion,
    ProductMomentSource, ProductMomentStatistics, SingletonProductBlock,
};
use super::partial_angular::{PartialAngularEvaluator, PartialMomentKey};
use super::radial::RadialEvaluator;
use super::resources::{
    CoefficientBudget, OutputKeyBudget, accumulate_coefficient, admit_exponent_payload,
    admit_output_key_payload, constant_rational_magnitude_bits, release_map_resources,
};

type CorrelatedPolynomial = MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u32>;

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_correlated_product(
    chart: &FactorizedProductMomentChart<'_>,
    active_powers: Box<[i64]>,
    polynomial: ProductPolynomial,
    source: ProductMomentSource,
    correlated: &CorrelatedProductBlock,
    singletons: &[SingletonProductBlock],
    limits: FactorizedProductMomentLimits,
) -> Result<ProductMomentExpansion, FactorizedProductMomentError> {
    let family = chart.authority.family();
    let context = family.coefficient_context();
    let loop_count = family.loop_count();
    let variable_count = loop_count.checked_add(chart.edges.len()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product polynomial variables",
        },
    )?;
    admit_exponent_payload(polynomial.nterms(), variable_count, limits)?;
    let dependency = chart
        .authority
        .dependencies()
        .get(correlated.dependency_ordinal)
        .ok_or(FactorizedProductMomentError::MissingDependency {
            ordinal: correlated.dependency_ordinal,
        })?;
    let mut correlated_reducer = Reducer::with_limits(dependency, limits.dependency_reduction)?;
    let mut radial = RadialEvaluator::try_new(chart, singletons, limits)?;
    let mut angular = PartialAngularEvaluator::try_new(
        context,
        family.dimension(),
        loop_count,
        &chart.edges,
        singletons,
        limits,
    )?;
    let mut budget = CoefficientBudget::new(limits);
    chart.retain_chart_inputs(&mut budget)?;
    for coefficient in &polynomial.coefficients {
        context.validate_with_limits(coefficient, limits.exact_algebra)?;
        budget.retain(coefficient)?;
    }
    let mut key_budget = OutputKeyBudget::new(limits);
    if let ProductMomentSource::Parent(parent) = &source {
        key_budget.retain(parent)?;
    }
    let mut output = BTreeMap::new();
    let mut coalescing_additions = 0_usize;
    let mut correlated_requests = 0_usize;

    for (polynomial_coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        if exponents.len() != variable_count {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a Symbolica product monomial has the wrong exponent width",
            });
        }
        let radial_powers =
            clone_u32_as_u64(&exponents[..loop_count], "correlated radial exponent row")?;
        let cross_powers =
            clone_u32_as_u64(&exponents[loop_count..], "correlated cross exponent row")?;
        let partial = angular.evaluate(
            &radial_powers,
            &cross_powers,
            &mut budget,
            &mut coalescing_additions,
        )?;
        for (moment, angular_coefficient) in partial {
            let weighted = context.try_mul(
                polynomial_coefficient,
                &angular_coefficient,
                limits.exact_algebra,
            )?;
            let normalized =
                context.try_mul(&weighted, &chart.normalization, limits.exact_algebra)?;
            budget.admit_temporaries([&weighted, &normalized])?;
            drop(weighted);
            budget.retain(&normalized)?;
            let correlated_polynomial =
                correlated_denominator_polynomial(chart, correlated, &moment, &mut budget, limits)?;
            for coefficient in &correlated_polynomial.coefficients {
                context.validate_with_limits(coefficient, limits.exact_algebra)?;
            }
            let active_end = correlated
                .active_power_start
                .checked_add(correlated.parent_positions.len())
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "correlated active-power range",
                })?;
            let correlated_active = active_powers
                .get(correlated.active_power_start..active_end)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the correlated block active-power range is absent",
                })?;
            let correlated_masters = reduce_correlated_polynomial(
                dependency.family().coefficient_context(),
                correlated_active,
                &correlated_polynomial,
                &mut correlated_reducer,
                &mut correlated_requests,
                radial.request_count(),
                &mut budget,
                &mut key_budget,
                &mut coalescing_additions,
                limits,
            )?;
            let mut products = BTreeMap::new();
            for (master, dependency_coefficient) in &correlated_masters {
                let raw = inject_dependency_master(
                    &zero_parent_key(family.denominator_count())?,
                    &correlated.parent_positions,
                    master,
                )?;
                let coefficient =
                    context.try_mul(&normalized, dependency_coefficient, limits.exact_algebra)?;
                budget.admit_temporaries([&coefficient])?;
                accumulate_coefficient(
                    context,
                    &mut products,
                    raw,
                    coefficient,
                    &mut budget,
                    &mut key_budget,
                    limits,
                    family.denominator_count(),
                    &mut coalescing_additions,
                )?;
            }
            budget.release(&normalized)?;
            release_map_resources(&correlated_masters, &mut budget, &mut key_budget)?;
            for coefficient in &correlated_polynomial.coefficients {
                budget.release(coefficient)?;
            }

            for block in singletons {
                let radial_expansion = radial.evaluate(
                    block.dependency_ordinal,
                    active_powers[block.active_power_ordinal],
                    moment.radial_powers(loop_count)[block.transformed_vector],
                    correlated_requests,
                    &mut budget,
                    &mut key_budget,
                    &mut coalescing_additions,
                )?;
                products = convolve_singleton(
                    chart,
                    products,
                    block,
                    &radial_expansion,
                    &mut budget,
                    &mut key_budget,
                    &mut coalescing_additions,
                    limits,
                )?;
                radial.release_returned(&radial_expansion, &mut budget, &mut key_budget)?;
            }
            for (raw_master, coefficient) in products {
                let terminal = chart
                    .rule()
                    .parent_terminal_for(&raw_master)
                    .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?;
                budget.release(&coefficient)?;
                key_budget.release(&raw_master)?;
                accumulate_coefficient(
                    context,
                    &mut output,
                    clone_key(terminal)?,
                    coefficient,
                    &mut budget,
                    &mut key_budget,
                    limits,
                    family.denominator_count(),
                    &mut coalescing_additions,
                )?;
            }
            budget.release(&angular_coefficient)?;
        }
    }

    let correlated_statistics = correlated_reducer.statistics();
    let (radial_rule_applications, radial_cache_hits) = radial.dependency_statistics()?;
    let dependency_requests = correlated_requests
        .checked_add(radial.request_count())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product dependency requests",
        })?;
    let dependency_rule_applications = correlated_statistics
        .rule_applications()
        .checked_add(radial_rule_applications)
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product dependency rule applications",
        })?;
    let dependency_cache_hits = correlated_statistics
        .cache_hits()
        .checked_add(radial_cache_hits)
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product dependency cache hits",
        })?;
    let statistics = ProductMomentStatistics {
        numerator_polynomial_terms: polynomial.nterms(),
        angular_states: angular.state_count(),
        angular_transitions: angular.transition_count(),
        radial_states: radial.state_count(),
        radial_summands: radial.summand_count(),
        dependency_requests,
        dependency_rule_applications,
        dependency_cache_hits,
        coalescing_additions,
    };
    radial.finish(&mut budget, &mut key_budget)?;
    let guards = angular.finish()?;
    for coefficient in &polynomial.coefficients {
        budget.release(coefficient)?;
    }
    admit_output_key_payload(output.len(), family.denominator_count(), limits)?;
    Ok(ProductMomentExpansion::new(
        family.fingerprint_owner(),
        chart.identity.clone(),
        source,
        output,
        guards,
        statistics,
    ))
}

#[allow(clippy::too_many_arguments)]
fn reduce_correlated_polynomial(
    context: &CoefficientContext,
    active_powers: &[i64],
    polynomial: &CorrelatedPolynomial,
    reducer: &mut Reducer<'_>,
    requests: &mut usize,
    request_offset: usize,
    budget: &mut CoefficientBudget,
    key_budget: &mut OutputKeyBudget,
    coalescing_additions: &mut usize,
    limits: FactorizedProductMomentLimits,
) -> Result<BTreeMap<IntegralKey, Coefficient>, FactorizedProductMomentError> {
    if polynomial.nvars() != active_powers.len() {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the correlated scalar polynomial has the wrong denominator width",
        });
    }
    let prospective_requests = requests.checked_add(polynomial.nterms()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product dependency requests",
        },
    )?;
    let aggregate_requests = request_offset.checked_add(prospective_requests).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product dependency requests",
        },
    )?;
    admit_limit(
        "product dependency requests",
        aggregate_requests,
        limits.max_dependency_requests,
    )?;
    let mut output = BTreeMap::new();
    for (polynomial_coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        let mut powers = clone_i64_vec(active_powers, "correlated dependency target")?;
        for (power, &shift) in powers.iter_mut().zip(exponents) {
            *power = power.checked_sub(i64::from(shift)).ok_or(
                FactorizedProductMomentError::RadialShiftOverflow {
                    denominator_power: *power,
                    shift: u64::from(shift),
                },
            )?;
        }
        let target = IntegralKey::try_new(powers)?;
        let decomposition = reducer.reduce_unit_mass(&target)?;
        for (master, dependency_coefficient) in decomposition.terms() {
            key_budget.retain(master)?;
            budget.retain(dependency_coefficient)?;
        }
        for (master, dependency_coefficient) in decomposition.terms() {
            let coefficient = context.try_mul(
                polynomial_coefficient,
                dependency_coefficient,
                limits.exact_algebra,
            )?;
            budget.admit_temporaries([&coefficient])?;
            accumulate_coefficient(
                context,
                &mut output,
                clone_key(master)?,
                coefficient,
                budget,
                key_budget,
                limits,
                active_powers.len(),
                coalescing_additions,
            )?;
        }
        for (master, dependency_coefficient) in decomposition.terms() {
            key_budget.release(master)?;
            budget.release(dependency_coefficient)?;
        }
    }
    *requests = prospective_requests;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn convolve_singleton(
    chart: &FactorizedProductMomentChart<'_>,
    products: BTreeMap<IntegralKey, Coefficient>,
    block: &SingletonProductBlock,
    dependency: &BTreeMap<IntegralKey, Coefficient>,
    budget: &mut CoefficientBudget,
    key_budget: &mut OutputKeyBudget,
    coalescing_additions: &mut usize,
    limits: FactorizedProductMomentLimits,
) -> Result<BTreeMap<IntegralKey, Coefficient>, FactorizedProductMomentError> {
    let context = chart.authority.family().coefficient_context();
    let prospective = products.len().checked_mul(dependency.len()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product convolution terms",
        },
    )?;
    admit_limit(
        "product convolution terms",
        prospective,
        limits.max_output_terms,
    )?;
    admit_output_key_payload(
        prospective,
        chart.authority.family().denominator_count(),
        limits,
    )?;
    let mut output = BTreeMap::new();
    for (partial_key, partial_coefficient) in &products {
        for (master, dependency_coefficient) in dependency {
            let raw = inject_dependency_master(
                partial_key,
                std::slice::from_ref(&block.parent_position),
                master,
            )?;
            let coefficient = context.try_mul(
                partial_coefficient,
                dependency_coefficient,
                limits.exact_algebra,
            )?;
            budget.admit_temporaries([&coefficient])?;
            accumulate_coefficient(
                context,
                &mut output,
                raw,
                coefficient,
                budget,
                key_budget,
                limits,
                chart.authority.family().denominator_count(),
                coalescing_additions,
            )?;
        }
    }
    release_map_resources(&products, budget, key_budget)?;
    Ok(output)
}

fn inject_dependency_master(
    parent: &IntegralKey,
    positions: &[usize],
    master: &IntegralKey,
) -> Result<IntegralKey, FactorizedProductMomentError> {
    if positions.len() != master.powers().len() {
        return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
    }
    let mut powers = clone_i64_vec(parent.powers(), "product dependency-master injection")?;
    for (&position, &power) in positions.iter().zip(master.powers()) {
        let slot = powers
            .get_mut(position)
            .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?;
        if *slot != 0 {
            return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
        }
        *slot = power;
    }
    Ok(IntegralKey::try_new(powers)?)
}

fn correlated_denominator_polynomial(
    chart: &FactorizedProductMomentChart<'_>,
    block: &CorrelatedProductBlock,
    moment: &PartialMomentKey,
    budget: &mut CoefficientBudget,
    limits: FactorizedProductMomentLimits,
) -> Result<CorrelatedPolynomial, FactorizedProductMomentError> {
    let dependency = &chart.authority.dependencies()[block.dependency_ordinal];
    let family = dependency.family();
    let context = family.coefficient_context();
    let arity = family.denominator_count();
    let coordinate_powers = correlated_coordinate_powers(chart, block, moment)?;
    let mut expansions = Vec::new();
    expansions
        .try_reserve_exact(coordinate_powers.len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "correlated scalar-product expansions",
            requested: coordinate_powers.len(),
        })?;
    let mut support_bound = 1_usize;
    let mut operation_bound = 0_usize;
    let mut exponent_rows_peak = 1_usize;
    let mut numerator_bits = 1_usize;
    let mut denominator_bits = 1_usize;
    let one = context.one();
    budget.retain(&one)?;
    for (coordinate, &(power, sign)) in coordinate_powers.iter().enumerate() {
        if power == 0 {
            continue;
        }
        let native_limit = i32::MAX as u32;
        if power > u64::from(native_limit) {
            return Err(
                FactorizedProductMomentError::NativePolynomialExponentLimit {
                    requested: power,
                    limit: native_limit,
                },
            );
        }
        let expansion = family.scalar_product_expansion(coordinate)?;
        for coefficient in
            std::iter::once(expansion.constant()).chain(expansion.denominator_coefficients())
        {
            context.validate_with_limits(coefficient, limits.exact_algebra)?;
            budget.retain(coefficient)?;
        }
        let (width, affine_num_bits, affine_den_bits) = rational_affine_profile(&expansion)?;
        let power_size = usize::try_from(power).map_err(|_| {
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated scalar-product power",
            }
        })?;
        numerator_bits = numerator_bits
            .checked_add(power_size.checked_mul(affine_num_bits).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "correlated projected numerator bits",
                },
            )?)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated projected numerator bits",
            })?;
        denominator_bits = denominator_bits
            .checked_add(power_size.checked_mul(affine_den_bits).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "correlated projected denominator bits",
                },
            )?)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated projected denominator bits",
            })?;
        let factor_support = multiset_support(power, width)?;
        let prior_support = support_bound;
        support_bound = support_bound.checked_mul(factor_support).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated polynomial support",
            },
        )?;
        admit_limit(
            "correlated polynomial support",
            support_bound,
            limits.max_native_polynomial_terms,
        )?;
        operation_bound = operation_bound
            .checked_add(
                factor_support
                    .checked_mul(width)
                    .and_then(|value| value.checked_mul(power_size.max(1)))
                    .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "correlated native polynomial operations",
                    })?,
            )
            .and_then(|value| value.checked_add(prior_support.checked_mul(factor_support)?))
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated native polynomial operations",
            })?;
        admit_limit(
            "correlated native polynomial operations",
            operation_bound,
            limits.max_native_polynomial_operations,
        )?;
        exponent_rows_peak = exponent_rows_peak.max(
            prior_support
                .checked_add(width)
                .and_then(|value| value.checked_add(factor_support))
                .and_then(|value| value.checked_add(support_bound))
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "correlated native exponent rows",
                })?,
        );
        expansions.push((power_size, sign, expansion));
    }
    admit_exponent_payload(exponent_rows_peak, arity, limits)?;
    budget.admit_native_rational_envelope(
        exponent_rows_peak,
        numerator_bits,
        denominator_bits,
        &one,
    )?;
    let template = polynomial_template(arity)?;
    let mut polynomial = template.constant(one.clone());
    validate_native_coefficients(context, &polynomial, limits)?;
    for coefficient in &polynomial.coefficients {
        budget.retain(coefficient)?;
    }
    for (power, sign, expansion) in expansions {
        let affine = affine_polynomial(context, &template, &expansion, sign, limits)?;
        for coefficient in &affine.coefficients {
            budget.retain(coefficient)?;
        }
        for coefficient in
            std::iter::once(expansion.constant()).chain(expansion.denominator_coefficients())
        {
            budget.release(coefficient)?;
        }
        drop(expansion);
        let powered = catch_unwind(AssertUnwindSafe(|| affine.pow(power)))
            .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
        validate_native_coefficients(context, &powered, limits)?;
        for coefficient in &powered.coefficients {
            budget.retain(coefficient)?;
        }
        let next = catch_unwind(AssertUnwindSafe(|| &polynomial * &powered))
            .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
        validate_native_coefficients(context, &next, limits)?;
        if next.nterms() > limits.max_native_polynomial_terms {
            return Err(
                FactorizedProductMomentError::NativePolynomialSupportExceeded {
                    actual: next.nterms(),
                    limit: limits.max_native_polynomial_terms,
                },
            );
        }
        for coefficient in &next.coefficients {
            budget.retain(coefficient)?;
        }
        for coefficient in &polynomial.coefficients {
            budget.release(coefficient)?;
        }
        for coefficient in &affine.coefficients {
            budget.release(coefficient)?;
        }
        for coefficient in &powered.coefficients {
            budget.release(coefficient)?;
        }
        polynomial = next;
    }
    // The returned polynomial remains charged to the caller's aggregate
    // budget until its reduction is complete.
    budget.release(&one)?;
    Ok(polynomial)
}

fn correlated_coordinate_powers(
    chart: &FactorizedProductMomentChart<'_>,
    block: &CorrelatedProductBlock,
    moment: &PartialMomentKey,
) -> Result<Box<[(u64, i64)]>, FactorizedProductMomentError> {
    let family = chart.authority.dependencies()[block.dependency_ordinal].family();
    let radial = moment.radial_powers(chart.authority.family().loop_count());
    let cross = moment.cross_powers(chart.authority.family().loop_count());
    let mut output = Vec::new();
    output
        .try_reserve_exact(family.coordinates().len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "correlated scalar-product powers",
            requested: family.coordinates().len(),
        })?;
    for coordinate in family.coordinates() {
        let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
            return Err(
                FactorizedProductMomentError::UnsupportedDependencySemantic {
                    ordinal: block.dependency_ordinal,
                },
            );
        };
        let global_left = block.transformed_vectors[left];
        let global_right = block.transformed_vectors[right];
        let power = if left == right {
            radial[global_left]
        } else {
            let edge = edge_slot(&chart.edges, global_left, global_right)?;
            cross[edge]
        };
        let sign = block.vector_signs[left]
            .checked_mul(block.vector_signs[right])
            .ok_or(FactorizedProductMomentError::Invariant {
                detail: "a correlated coordinate sign overflowed i64",
            })?;
        output.push((power, sign));
    }
    Ok(output.into_boxed_slice())
}

fn affine_polynomial(
    context: &CoefficientContext,
    template: &CorrelatedPolynomial,
    expansion: &DenominatorExpansion,
    sign: i64,
    limits: FactorizedProductMomentLimits,
) -> Result<CorrelatedPolynomial, FactorizedProductMomentError> {
    if sign != 1 && sign != -1 {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "a correlated coordinate sign is not unit",
        });
    }
    let signed = |coefficient: &Coefficient| -> Result<Coefficient, FactorizedProductMomentError> {
        if sign == 1 {
            Ok(coefficient.clone())
        } else {
            Ok(context.try_neg(coefficient, limits.exact_algebra)?)
        }
    };
    let mut coefficients = Vec::new();
    let coefficient_count = expansion
        .denominator_coefficients()
        .len()
        .checked_add(1)
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "correlated affine coefficients",
        })?;
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "correlated affine coefficients",
            requested: coefficient_count,
        })?;
    coefficients.push(signed(expansion.constant())?);
    for coefficient in expansion.denominator_coefficients() {
        coefficients.push(signed(coefficient)?);
    }
    let exponent_count = expansion.denominator_coefficients().len();
    let mut monomials = Vec::new();
    monomials.try_reserve_exact(exponent_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "correlated affine monomials",
            requested: exponent_count,
        }
    })?;
    for (variable, coefficient) in coefficients[1..].iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        let mut exponents = Vec::new();
        exponents.try_reserve_exact(exponent_count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "correlated affine exponent row",
                requested: exponent_count,
            }
        })?;
        exponents.resize(exponent_count, 0_u32);
        exponents[variable] = 1;
        monomials.push((coefficient.clone(), exponents));
    }
    let polynomial = catch_unwind(AssertUnwindSafe(|| {
        let mut polynomial = template.constant(coefficients[0].clone());
        for (coefficient, exponents) in monomials {
            polynomial = &polynomial + &template.monomial(coefficient.clone(), exponents);
        }
        polynomial
    }))
    .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
    validate_native_coefficients(context, &polynomial, limits)?;
    Ok(polynomial)
}

fn rational_affine_profile(
    expansion: &DenominatorExpansion,
) -> Result<(usize, usize, usize), FactorizedProductMomentError> {
    let mut width = 0_usize;
    let mut max_numerator_bits = 0_usize;
    let mut denominator_sum_bits = 0_usize;
    for coefficient in
        std::iter::once(expansion.constant()).chain(expansion.denominator_coefficients())
    {
        if coefficient.is_zero() {
            continue;
        }
        width =
            width
                .checked_add(1)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "correlated affine width",
                })?;
        let (numerator, denominator) = constant_rational_magnitude_bits(coefficient).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "an authenticated correlated affine coefficient stopped being rational",
            },
        )?;
        max_numerator_bits = max_numerator_bits.max(numerator);
        denominator_sum_bits = denominator_sum_bits.checked_add(denominator).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated affine denominator bits",
            },
        )?;
    }
    let numerator_bits = max_numerator_bits
        .checked_add(denominator_sum_bits)
        .and_then(|value| value.checked_add(ceil_log2(width)))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "correlated affine numerator bits",
        })?;
    Ok((width, numerator_bits, denominator_sum_bits.max(1)))
}

fn polynomial_template(
    variable_count: usize,
) -> Result<CorrelatedPolynomial, FactorizedProductMomentError> {
    let mut variables = Vec::new();
    variables.try_reserve_exact(variable_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "correlated Symbolica variables",
            requested: variable_count,
        }
    })?;
    variables.extend((0..variable_count).map(PolyVariable::Temporary));
    Ok(CorrelatedPolynomial::new(
        &RationalPolynomialField::new(Z),
        None,
        Arc::new(variables),
    ))
}

fn edge_slot(
    edges: &[(usize, usize)],
    left: usize,
    right: usize,
) -> Result<usize, FactorizedProductMomentError> {
    let pair = (left.min(right), left.max(right));
    edges
        .iter()
        .position(|&edge| edge == pair)
        .ok_or(FactorizedProductMomentError::Invariant {
            detail: "a correlated cross coordinate is absent from the parent chart",
        })
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}
