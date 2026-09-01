//! Symbolica-backed numerator expansion and exact product composition.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{
    IntegerRing, MultivariatePolynomial, PolyVariable, RationalPolynomialField, Z,
};

use crate::family::IntegralKey;

use super::angular::{AngularEvaluator, cross_radial_powers};
use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::{
    FactorizedProductMomentChart, ProductBlockLayout, ProductMomentExpansion,
    ProductMomentMonomial, ProductMomentSource, ProductMomentStatistics, SingletonProductBlock,
};
use super::radial::RadialEvaluator;
use super::resources::{
    CoefficientBudget, OutputKeyBudget, accumulate_coefficient, admit_exponent_payload,
    admit_output_key_payload, constant_integer_magnitude_bits, release_map_resources,
};

pub(super) type ProductPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u32>;

impl FactorizedProductMomentChart<'_> {
    pub(crate) fn try_evaluate_parent(
        &self,
        source: &IntegralKey,
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductMomentExpansion, FactorizedProductMomentError> {
        let active_powers = self.validate_parent_source(source)?;
        self.preflight_retained_integral_keys(1, limits)?;
        let polynomial = self.parent_numerator_polynomial(source, limits)?;
        let source = ProductMomentSource::Parent(clone_key(source)?);
        self.evaluate_polynomial(active_powers, polynomial, source, limits)
    }

    pub(crate) fn try_evaluate_monomial(
        &self,
        monomial: &ProductMomentMonomial,
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductMomentExpansion, FactorizedProductMomentError> {
        self.validate_monomial(monomial, limits)?;
        self.preflight_retained_integral_keys(0, limits)?;
        let polynomial = self.monomial_polynomial(monomial, limits)?;
        self.evaluate_polynomial(
            clone_i64_box(monomial.active_powers(), "product active powers")?,
            polynomial,
            ProductMomentSource::Monomial(monomial.clone()),
            limits,
        )
    }

    fn evaluate_polynomial(
        &self,
        active_powers: Box<[i64]>,
        polynomial: ProductPolynomial,
        source: ProductMomentSource,
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductMomentExpansion, FactorizedProductMomentError> {
        match &self.layout {
            ProductBlockLayout::AllSingleton {
                singletons_by_vector,
            } => self.evaluate_all_singleton_polynomial(
                active_powers,
                polynomial,
                source,
                singletons_by_vector,
                limits,
            ),
            ProductBlockLayout::OneCorrelated {
                correlated,
                singletons_by_vector,
            } => super::correlated::evaluate_correlated_product(
                self,
                active_powers,
                polynomial,
                source,
                correlated,
                singletons_by_vector,
                limits,
            ),
        }
    }

    fn evaluate_all_singleton_polynomial(
        &self,
        active_powers: Box<[i64]>,
        polynomial: ProductPolynomial,
        source: ProductMomentSource,
        singletons_by_vector: &[SingletonProductBlock],
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductMomentExpansion, FactorizedProductMomentError> {
        let family = self.authority.family();
        let context = family.coefficient_context();
        let loop_count = self.loop_factor_count();
        let variable_count = loop_count.checked_add(self.edges.len()).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product polynomial variables",
            },
        )?;
        admit_exponent_payload(polynomial.nterms(), variable_count, limits)?;
        let mut budget = CoefficientBudget::new(limits);
        self.retain_chart_inputs(&mut budget)?;
        for coefficient in &polynomial.coefficients {
            context.validate_with_limits(coefficient, limits.exact_algebra)?;
            budget.retain(coefficient)?;
        }
        let mut key_budget = OutputKeyBudget::new(limits);
        let raw_master = self
            .sole_raw_master
            .as_ref()
            .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?;
        let terminal = self
            .sole_terminal
            .as_ref()
            .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?;
        key_budget.retain(raw_master)?;
        key_budget.retain(terminal)?;
        if let ProductMomentSource::Parent(parent) = &source {
            key_budget.retain(parent)?;
        }

        let mut angular =
            AngularEvaluator::new(context, family.dimension(), loop_count, &self.edges, limits);
        let mut radial = RadialEvaluator::try_new(self, singletons_by_vector, limits)?;
        let mut output = BTreeMap::new();
        let mut coalescing_additions = 0_usize;
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
                clone_u32_as_u64(&exponents[..loop_count], "product radial exponent row")?;
            let cross_powers =
                clone_u32_as_u64(&exponents[loop_count..], "product cross exponent row")?;
            let angular_coefficient = angular.evaluate(&cross_powers, &mut budget)?;
            budget.retain(&angular_coefficient)?;
            if angular_coefficient.is_zero() {
                budget.release(&angular_coefficient)?;
                continue;
            }
            let Some(angular_radial) = cross_radial_powers(loop_count, &self.edges, &cross_powers)?
            else {
                budget.release(&angular_coefficient)?;
                continue;
            };
            let mut total_radial = Vec::new();
            total_radial.try_reserve_exact(loop_count).map_err(|_| {
                FactorizedProductMomentError::AllocationFailure {
                    resource: "product total radial powers",
                    requested: loop_count,
                }
            })?;
            for (&direct, &angular_power) in radial_powers.iter().zip(&angular_radial) {
                let power = direct.checked_add(angular_power).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "product total radial power",
                    },
                )?;
                let power_size = usize::try_from(power).map_err(|_| {
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "product total radial power",
                    }
                })?;
                admit_limit("radial power", power_size, limits.max_radial_power)?;
                total_radial.push(power);
            }

            let angular_weighted = context.try_mul(
                polynomial_coefficient,
                &angular_coefficient,
                limits.exact_algebra,
            )?;
            budget.admit_temporaries([&angular_weighted])?;
            let normalized =
                context.try_mul(&angular_weighted, &self.normalization, limits.exact_algebra)?;
            budget.admit_temporaries([&angular_weighted, &normalized])?;
            // The weighted intermediate is no longer live when the normalized
            // value becomes retained by the first product map.
            drop(angular_weighted);
            let mut products = BTreeMap::new();
            accumulate_coefficient(
                context,
                &mut products,
                zero_parent_key(family.denominator_count())?,
                normalized,
                &mut budget,
                &mut key_budget,
                limits,
                family.denominator_count(),
                &mut coalescing_additions,
            )?;

            for block in singletons_by_vector {
                let dependency = radial.evaluate(
                    block.dependency_ordinal,
                    active_powers[block.active_power_ordinal],
                    total_radial[block.transformed_vector],
                    0,
                    &mut budget,
                    &mut key_budget,
                    &mut coalescing_additions,
                )?;
                let mut next = BTreeMap::new();
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
                admit_output_key_payload(prospective, family.denominator_count(), limits)?;
                for (partial_key, partial_coefficient) in &products {
                    for (dependency_master, dependency_coefficient) in &dependency {
                        let mut powers =
                            clone_i64_vec(partial_key.powers(), "product convolution key")?;
                        let parent = block.parent_position;
                        if powers[parent] != 0 || dependency_master.powers().len() != 1 {
                            return Err(FactorizedProductMomentError::Invariant {
                                detail: "a singleton dependency does not inject into one fresh parent slot",
                            });
                        }
                        powers[parent] = dependency_master.powers()[0];
                        let coefficient = context.try_mul(
                            partial_coefficient,
                            dependency_coefficient,
                            limits.exact_algebra,
                        )?;
                        budget.admit_temporaries([&coefficient])?;
                        accumulate_coefficient(
                            context,
                            &mut next,
                            IntegralKey::try_new(powers)?,
                            coefficient,
                            &mut budget,
                            &mut key_budget,
                            limits,
                            family.denominator_count(),
                            &mut coalescing_additions,
                        )?;
                    }
                }
                release_map_resources(&products, &mut budget, &mut key_budget)?;
                radial.release_returned(&dependency, &mut budget, &mut key_budget)?;
                products = next;
            }
            for (raw_master, coefficient) in products {
                if raw_master
                    != *self
                        .sole_raw_master
                        .as_ref()
                        .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?
                {
                    return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
                }
                let terminal = self
                    .rule()
                    .parent_terminal_for(&raw_master)
                    .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?;
                // `products` already owns and accounts for this moved entry.
                // Release that ownership before transferring its coefficient
                // to the independently keyed final-output map.
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

        let (dependency_rule_applications, dependency_cache_hits) =
            radial.dependency_statistics()?;
        let statistics = ProductMomentStatistics {
            numerator_polynomial_terms: polynomial.nterms(),
            angular_states: angular.state_count(),
            angular_transitions: angular.transition_count(),
            radial_states: radial.state_count(),
            radial_summands: radial.summand_count(),
            dependency_requests: radial.request_count(),
            dependency_rule_applications,
            dependency_cache_hits,
            coalescing_additions,
        };
        radial.finish(&mut budget, &mut key_budget)?;
        let guards = angular.finish(&mut budget)?;
        for coefficient in &polynomial.coefficients {
            budget.release(coefficient)?;
        }
        admit_output_key_payload(output.len(), family.denominator_count(), limits)?;
        Ok(ProductMomentExpansion::new(
            family.fingerprint_owner(),
            self.identity.clone(),
            source,
            output,
            guards,
            statistics,
        ))
    }

    fn validate_parent_source(
        &self,
        source: &IntegralKey,
    ) -> Result<Box<[i64]>, FactorizedProductMomentError> {
        let powers = source.powers();
        let active = self.rule().application_domain().sector().active_bits();
        if powers.len() != active.len() {
            return Err(FactorizedProductMomentError::WrongTargetArity {
                expected: active.len(),
                actual: powers.len(),
            });
        }
        for (position, (&power, &is_active)) in powers.iter().zip(active).enumerate() {
            if is_active != (power >= 1) {
                return Err(FactorizedProductMomentError::OutsideFactorizedSector {
                    position,
                    power,
                    active: is_active,
                });
            }
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.active_parent_positions.len())
            .map_err(|_| FactorizedProductMomentError::AllocationFailure {
                resource: "product active powers",
                requested: self.active_parent_positions.len(),
            })?;
        output.extend(
            self.active_parent_positions
                .iter()
                .map(|&parent| powers[parent]),
        );
        Ok(output.into_boxed_slice())
    }

    fn preflight_retained_integral_keys(
        &self,
        source_keys: usize,
        limits: FactorizedProductMomentLimits,
    ) -> Result<(), FactorizedProductMomentError> {
        let chart_keys = usize::from(self.sole_raw_master.is_some())
            .checked_add(usize::from(self.sole_terminal.is_some()))
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product retained integral keys",
            })?;
        let rows = chart_keys.checked_add(source_keys).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product retained integral keys",
            },
        )?;
        admit_output_key_payload(rows, self.authority.family().denominator_count(), limits)
    }

    fn validate_monomial(
        &self,
        monomial: &ProductMomentMonomial,
        limits: FactorizedProductMomentLimits,
    ) -> Result<(), FactorizedProductMomentError> {
        for (component, expected, actual) in [
            (
                "active powers",
                self.active_parent_positions.len(),
                monomial.active_powers().len(),
            ),
            (
                "radial powers",
                self.loop_factor_count(),
                monomial.radial_powers().len(),
            ),
            (
                "cross powers",
                self.edges.len(),
                monomial.cross_powers().len(),
            ),
        ] {
            if expected != actual {
                return Err(FactorizedProductMomentError::WrongMonomialWidth {
                    component,
                    expected,
                    actual,
                });
            }
        }
        for (vector, &power) in monomial.active_powers().iter().enumerate() {
            if power < 1 {
                return Err(FactorizedProductMomentError::NonpositiveActivePower { vector, power });
            }
        }
        let degree = monomial
            .radial_powers()
            .iter()
            .chain(monomial.cross_powers())
            .try_fold(0_u64, |total, &power| {
                total.checked_add(power).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "product monomial degree",
                    },
                )
            })?;
        let degree = usize::try_from(degree).map_err(|_| {
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product monomial degree",
            }
        })?;
        admit_limit(
            "product monomial degree",
            degree,
            limits.max_total_numerator_degree,
        )
    }

    fn parent_numerator_polynomial(
        &self,
        source: &IntegralKey,
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductPolynomial, FactorizedProductMomentError> {
        let variable_count = self
            .loop_factor_count()
            .checked_add(self.edges.len())
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product polynomial variables",
            })?;
        let mut total_degree = 0_u64;
        let mut support_bound = 1_usize;
        let mut operation_bound = 0_usize;
        let mut exponent_rows_peak = 1_usize;
        let mut integer_bit_bound = 1_usize;
        let routed = self.routing.routing().transformed_denominators();
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(source.powers().len())
            .map_err(|_| FactorizedProductMomentError::AllocationFailure {
                resource: "product native numerator plans",
                requested: source.powers().len(),
            })?;
        // Complete all exponent, support, and operation admission before
        // constructing the first native Symbolica polynomial.
        for (denominator, &power) in source.powers().iter().enumerate() {
            if power >= 0 {
                continue;
            }
            let degree = power.unsigned_abs();
            let native_limit = i32::MAX as u32;
            if degree > u64::from(native_limit) {
                return Err(
                    FactorizedProductMomentError::NativePolynomialExponentLimit {
                        requested: degree,
                        limit: native_limit,
                    },
                );
            }
            let native_degree = usize::try_from(degree).map_err(|_| {
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product native numerator power",
                }
            })?;
            let form = routed
                .get(denominator)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the routed denominator table is shorter than the family arity",
                })?;
            let (width, max_input_integer_bits) = affine_integer_profile(
                form,
                &self.radial_coordinate_positions,
                &self.cross_coordinate_positions,
            )?;
            let affine_l1_bits = max_input_integer_bits.checked_add(ceil_log2(width)).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient bits",
                },
            )?;
            let factor_bits = native_degree.checked_mul(affine_l1_bits).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient bits",
                },
            )?;
            integer_bit_bound = integer_bit_bound.checked_add(factor_bits).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient bits",
                },
            )?;
            let factor_support = multiset_support(degree, width)?;
            let prior_support = support_bound;
            support_bound = prior_support.checked_mul(factor_support).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product polynomial support",
                },
            )?;
            admit_limit(
                "product polynomial support",
                support_bound,
                limits.max_native_polynomial_terms,
            )?;
            let power_operations = factor_support
                .checked_mul(width)
                .and_then(|value| value.checked_mul(native_degree.max(1)))
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product native polynomial operations",
                })?;
            let multiplication_operations = prior_support.checked_mul(factor_support).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product native polynomial operations",
                },
            )?;
            operation_bound = operation_bound
                .checked_add(power_operations)
                .and_then(|value| value.checked_add(multiplication_operations))
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product native polynomial operations",
                })?;
            admit_limit(
                "product native polynomial operations",
                operation_bound,
                limits.max_native_polynomial_operations,
            )?;
            let live_rows = prior_support
                .checked_add(width)
                .and_then(|value| value.checked_add(factor_support))
                .and_then(|value| value.checked_add(support_bound))
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product native polynomial exponent rows",
                })?;
            exponent_rows_peak = exponent_rows_peak.max(live_rows);
            total_degree = total_degree.checked_add(degree).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product numerator degree",
                },
            )?;
            plans.push((denominator, native_degree));
        }
        let total_degree = usize::try_from(total_degree).map_err(|_| {
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product numerator degree",
            }
        })?;
        admit_limit(
            "product numerator degree",
            total_degree,
            limits.max_total_numerator_degree,
        )?;
        admit_limit(
            "product polynomial support",
            support_bound,
            limits.max_native_polynomial_terms,
        )?;
        admit_exponent_payload(exponent_rows_peak, variable_count, limits)?;

        let context = self.authority.family().coefficient_context();
        let mut native_budget = CoefficientBudget::new(limits);
        self.retain_chart_inputs(&mut native_budget)?;
        let context_unit = context.one();
        native_budget.retain(&context_unit)?;
        // All routed affine coefficients were authenticated as integers above.
        // The l1 norm bounds every Symbolica output coefficient, while the
        // row peak covers the simultaneously live prior/affine/power/product
        // sparse polynomials.  Admit that envelope before constructing the
        // first native polynomial.
        native_budget.admit_native_integer_envelope(
            exponent_rows_peak,
            integer_bit_bound,
            &context_unit,
        )?;
        let template = polynomial_template(variable_count)?;
        let mut polynomial = template.constant(context_unit.clone());
        validate_native_coefficients(context, &polynomial, limits)?;
        for coefficient in &polynomial.coefficients {
            native_budget.retain(coefficient)?;
        }
        for (denominator, native_degree) in plans {
            let affine = affine_polynomial(
                &template,
                &routed[denominator],
                &self.radial_coordinate_positions,
                &self.cross_coordinate_positions,
                context,
                limits,
            )?;
            for coefficient in &affine.coefficients {
                native_budget.retain(coefficient)?;
            }
            let powered = catch_unwind(AssertUnwindSafe(|| affine.pow(native_degree)))
                .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
            validate_native_coefficients(context, &powered, limits)?;
            for coefficient in &powered.coefficients {
                native_budget.retain(coefficient)?;
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
                native_budget.retain(coefficient)?;
            }
            for coefficient in &polynomial.coefficients {
                native_budget.release(coefficient)?;
            }
            for coefficient in &affine.coefficients {
                native_budget.release(coefficient)?;
            }
            for coefficient in &powered.coefficients {
                native_budget.release(coefficient)?;
            }
            polynomial = next;
        }
        for coefficient in &polynomial.coefficients {
            native_budget.release(coefficient)?;
        }
        native_budget.release(&context_unit)?;
        Ok(polynomial)
    }

    fn monomial_polynomial(
        &self,
        monomial: &ProductMomentMonomial,
        limits: FactorizedProductMomentLimits,
    ) -> Result<ProductPolynomial, FactorizedProductMomentError> {
        let variable_count = self
            .loop_factor_count()
            .checked_add(self.edges.len())
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product polynomial variables",
            })?;
        let mut exponents = Vec::new();
        exponents.try_reserve_exact(variable_count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "product monomial exponents",
                requested: variable_count,
            }
        })?;
        for &power in monomial
            .radial_powers()
            .iter()
            .chain(monomial.cross_powers())
        {
            let native_limit = i32::MAX as u32;
            if power > u64::from(native_limit) {
                return Err(
                    FactorizedProductMomentError::NativePolynomialExponentLimit {
                        requested: power,
                        limit: native_limit,
                    },
                );
            }
            exponents.push(u32::try_from(power).map_err(|_| {
                FactorizedProductMomentError::NativePolynomialExponentLimit {
                    requested: power,
                    limit: u32::MAX,
                }
            })?);
        }
        admit_limit(
            "product polynomial support",
            1,
            limits.max_native_polynomial_terms,
        )?;
        admit_exponent_payload(1, variable_count, limits)?;
        let context = self.authority.family().coefficient_context();
        let mut native_budget = CoefficientBudget::new(limits);
        self.retain_chart_inputs(&mut native_budget)?;
        let context_unit = context.one();
        native_budget.retain(&context_unit)?;
        native_budget.admit_native_integer_envelope(1, 1, &context_unit)?;
        let template = polynomial_template(variable_count)?;
        let polynomial = catch_unwind(AssertUnwindSafe(|| {
            template.monomial(context_unit.clone(), exponents)
        }))
        .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
        validate_native_coefficients(context, &polynomial, limits)?;
        native_budget.release(&context_unit)?;
        Ok(polynomial)
    }

    pub(super) fn retain_chart_inputs(
        &self,
        budget: &mut CoefficientBudget,
    ) -> Result<(), FactorizedProductMomentError> {
        budget.retain(&self.normalization)?;
        for form in self.routing.routing().transformed_denominators() {
            budget.retain(form.constant())?;
            for coefficient in form.scalar_coefficients() {
                budget.retain(coefficient)?;
            }
        }
        Ok(())
    }
}

pub(super) fn clone_u32_as_u64(
    values: &[u32],
    resource: &'static str,
) -> Result<Vec<u64>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: values.len(),
        }
    })?;
    for &value in values {
        output.push(u64::from(value));
    }
    Ok(output)
}

fn polynomial_template(
    variable_count: usize,
) -> Result<ProductPolynomial, FactorizedProductMomentError> {
    let mut variables = Vec::new();
    variables.try_reserve_exact(variable_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product Symbolica variables",
            requested: variable_count,
        }
    })?;
    variables.extend((0..variable_count).map(PolyVariable::Temporary));
    Ok(ProductPolynomial::new(
        &RationalPolynomialField::new(Z),
        None,
        Arc::new(variables),
    ))
}

fn affine_polynomial(
    template: &ProductPolynomial,
    form: &super::super::factorized_numerator_lift::RoutedAffineDenominator,
    radial_positions: &[usize],
    cross_positions: &[usize],
    context: &crate::algebra::CoefficientContext,
    limits: FactorizedProductMomentLimits,
) -> Result<ProductPolynomial, FactorizedProductMomentError> {
    let variable_count = radial_positions
        .len()
        .checked_add(cross_positions.len())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product affine polynomial variables",
        })?;
    let mut monomials = Vec::new();
    monomials.try_reserve_exact(variable_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product affine monomials",
            requested: variable_count,
        }
    })?;
    for (variable, &position) in radial_positions.iter().chain(cross_positions).enumerate() {
        let coefficient = &form.scalar_coefficients()[position];
        if coefficient.is_zero() {
            continue;
        }
        let mut exponents = Vec::new();
        exponents.try_reserve_exact(variable_count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "product affine monomial exponents",
                requested: variable_count,
            }
        })?;
        exponents.resize(variable_count, 0_u32);
        exponents[variable] = 1;
        monomials.push((coefficient, exponents));
    }
    let affine = catch_unwind(AssertUnwindSafe(|| {
        let mut affine = template.constant(context.zero());
        if !form.constant().is_zero() {
            affine = &affine + &template.constant(form.constant().clone());
        }
        for (coefficient, exponents) in monomials {
            affine = &affine + &template.monomial(coefficient.clone(), exponents);
        }
        affine
    }))
    .map_err(|_| FactorizedProductMomentError::NativePolynomialPanic)?;
    validate_native_coefficients(context, &affine, limits)?;
    Ok(affine)
}

pub(super) fn validate_native_coefficients(
    context: &crate::algebra::CoefficientContext,
    polynomial: &ProductPolynomial,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for coefficient in &polynomial.coefficients {
        context.validate_with_limits(coefficient, limits.exact_algebra)?;
    }
    Ok(())
}

fn affine_integer_profile(
    form: &super::super::factorized_numerator_lift::RoutedAffineDenominator,
    radial_positions: &[usize],
    cross_positions: &[usize],
) -> Result<(usize, usize), FactorizedProductMomentError> {
    let mut width = 0_usize;
    let mut max_bits = 0_usize;
    for coefficient in std::iter::once(form.constant()).chain(
        radial_positions
            .iter()
            .chain(cross_positions)
            .map(|&position| &form.scalar_coefficients()[position]),
    ) {
        if coefficient.is_zero() {
            continue;
        }
        width =
            width
                .checked_add(1)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product affine polynomial width",
                })?;
        let bits = constant_integer_magnitude_bits(coefficient).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "an authenticated routed affine coefficient stopped being an integer",
            },
        )?;
        max_bits = max_bits.max(bits);
    }
    Ok((width, max_bits))
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(super) fn multiset_support(
    power: u64,
    width: usize,
) -> Result<usize, FactorizedProductMomentError> {
    if power == 0 {
        return Ok(1);
    }
    if width == 0 {
        return Ok(0);
    }
    let power = u128::from(power);
    let width =
        u128::try_from(width).map_err(|_| FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product polynomial affine width",
        })?;
    let n = power.checked_add(width - 1).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product polynomial support",
        },
    )?;
    let k = (width - 1).min(power);
    let mut value = 1_u128;
    for step in 1..=k {
        value = value.checked_mul(n - k + step).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product polynomial support",
            },
        )? / step;
    }
    usize::try_from(value).map_err(|_| FactorizedProductMomentError::ResourceCountOverflow {
        resource: "product polynomial support",
    })
}

pub(super) fn zero_parent_key(arity: usize) -> Result<IntegralKey, FactorizedProductMomentError> {
    let mut powers = Vec::new();
    powers.try_reserve_exact(arity).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product zero parent key",
            requested: arity,
        }
    })?;
    powers.resize(arity, 0_i64);
    Ok(IntegralKey::try_new(powers)?)
}

pub(super) fn clone_key(key: &IntegralKey) -> Result<IntegralKey, FactorizedProductMomentError> {
    Ok(IntegralKey::try_new(key.powers().iter().copied())?)
}

fn clone_i64_box(
    values: &[i64],
    resource: &'static str,
) -> Result<Box<[i64]>, FactorizedProductMomentError> {
    Ok(clone_i64_vec(values, resource)?.into_boxed_slice())
}

pub(super) fn clone_i64_vec(
    values: &[i64],
    resource: &'static str,
) -> Result<Vec<i64>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: values.len(),
        }
    })?;
    output.extend_from_slice(values);
    Ok(output)
}
