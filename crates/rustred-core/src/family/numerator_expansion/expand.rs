//! Symbolica-backed sparse expansion and checked endpoint routing.
//!
//! The native outer-polynomial coefficient field is deliberately restricted
//! to authenticated rational constants. Symbolica owns every polynomial
//! power, product, collision, and cancellation; RustRed supplies only the
//! prospective structural envelope and typed endpoint routing.

use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{MultivariatePolynomial, PolyVariable, Q, Rational};

use crate::algebra::{
    Coefficient, CoefficientContext, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::{IntegralFamily, IntegralKey};

use super::error::MultiAffineNumeratorExpansionError;
use super::limits::MultiAffineNumeratorExpansionLimits;
use super::model::{MultiAffineNumeratorEndpoint, MultiAffineNumeratorFactor};

#[path = "native_rational.rs"]
mod native_rational;
use native_rational::{authenticated_rational, contextual_coefficient, rational_weight};
#[path = "support.rs"]
mod support;
use support::PrefixSupport;
#[path = "visit.rs"]
mod visit;
pub(crate) use visit::{AdmittedSupport, try_expand_multi_affine_support_with_usage};

// Input admission below already restricts every scalar to Q. Keeping that
// native field here avoids polynomial variable-map unification and polynomial
// GCDs for each scalar operation during sparse powers/products.
type EndpointPolynomial = MultivariatePolynomial<Q, u32>;

#[derive(Clone, Copy, Debug, Default)]
struct CoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

impl CoefficientWeight {
    fn checked_add(self, other: Self) -> Result<Self, MultiAffineNumeratorExpansionError> {
        Ok(Self {
            terms: self.terms.checked_add(other.terms).ok_or(
                MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                    resource: "multi-affine retained coefficient terms",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_add(other.clone_owned_bytes)
                .ok_or(MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                    resource: "multi-affine retained coefficient clone-owned bytes",
                })?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct FactorPlan {
    ordinal: usize,
    native_power: usize,
    support_bound: usize,
    prefix_support_bound: usize,
}

/// Expand a fixed product of affine parent-denominator numerators exactly.
///
/// A monomial `prod_i D_i^e_i` lowers the corresponding base-key powers by
/// `e_i`. Returned endpoints are sorted by [`IntegralKey`] and exactly
/// coalesced. This function owns no rule or artifact semantics.
#[cfg(test)]
pub(crate) fn try_expand_multi_affine_numerator(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<Box<[MultiAffineNumeratorEndpoint]>, MultiAffineNumeratorExpansionError> {
    try_expand_multi_affine_numerator_with_usage(family, base, factors, limits, |_| Ok(()))
}

/// Prospective structural work, not a measured count of native operations.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ExpansionUsage {
    pub operations: usize,
    pub endpoints: usize,
}

pub(crate) fn try_expand_multi_affine_numerator_with_usage(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
    reserve: impl FnOnce(ExpansionUsage) -> Result<(), MultiAffineNumeratorExpansionError>,
) -> Result<Box<[MultiAffineNumeratorEndpoint]>, MultiAffineNumeratorExpansionError> {
    let Some(expanded) = expand_native(family, base, factors, limits, reserve)? else {
        return Ok(Box::new([]));
    };
    materialize_endpoints(
        family.coefficient_context(),
        base,
        &expanded.polynomial,
        expanded.input_weight,
        expanded.constant_wrapper_bytes,
        limits,
    )
}

/// Expand with parameter-dependent affine constants, e.g. a deformed mass.
///
/// Rational-constant inputs take the unchanged native-Q lane above and return
/// identical endpoints. Otherwise every parameter-dependent constant `c_a`
/// becomes one virtual Temporary axis `X_a`, so the complete native
/// expansion of `prod_a (X_a + sum_i c_ai D_i)^n_a` stays over Q with the
/// unchanged prospective support/operation/exponent admission on the lifted
/// arity. Each lifted monomial `q X^e D^f` is then substituted exactly as
/// `q prod_a c_a^e_a` in the family coefficient context and coalesced per
/// endpoint key; exact zero sums are dropped. Linear denominator coefficients
/// must still be rational constants. This substitution is the separate
/// coefficient-work boundary: one checked context product per lifted
/// monomial and virtual axis is admitted against the native operation limit,
/// and every retained output coefficient against the retained-coefficient
/// limits.
pub(crate) fn try_expand_parametric_multi_affine_numerator_with_usage(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
    reserve: impl FnOnce(ExpansionUsage) -> Result<(), MultiAffineNumeratorExpansionError>,
) -> Result<Box<[MultiAffineNumeratorEndpoint]>, MultiAffineNumeratorExpansionError> {
    let context = family.coefficient_context();
    let arity = family.denominator_count();
    if base.powers().len() != arity {
        return Err(MultiAffineNumeratorExpansionError::WrongBaseArity {
            expected: arity,
            actual: base.powers().len(),
        });
    }
    admit_limit(
        "multi-affine factors",
        factors.len(),
        limits.max_factors,
        limits.diagnostics_enabled,
    )?;
    let mut parametric = 0_usize;
    for factor in factors {
        context.validate_with_limits(factor.constant(), limits.exact_algebra)?;
        if !factor.constant().is_constant() {
            parametric += 1;
            // The substitution power table reaches c_a^n_a exactly once per
            // virtual axis; admit its exponent growth before any work.
            context.preflight_power_with_limits(
                factor.constant(),
                factor.power(),
                limits.exact_algebra,
            )?;
        }
    }
    if parametric == 0 {
        return try_expand_multi_affine_numerator_with_usage(
            family, base, factors, limits, reserve,
        );
    }
    // Shared budgets also see the substitution boundary: at most one checked
    // context product per lifted monomial and virtual axis, plus one
    // coalescing sum, prospectively charged on the projected lifted support.
    // As elsewhere, overflow is a diagnostic; ordinary execution saturates.
    const SUBSTITUTION: &str = "multi-affine parametric constant operations";
    let diagnostics = limits.diagnostics_enabled;
    let substitution_per_term = estimate(parametric.checked_add(1), SUBSTITUTION, diagnostics)?;
    let reserve = move |usage: ExpansionUsage| {
        let substitution = estimate(
            usage.endpoints.checked_mul(substitution_per_term),
            SUBSTITUTION,
            diagnostics,
        )?;
        reserve(ExpansionUsage {
            operations: estimate(
                usage.operations.checked_add(substitution),
                SUBSTITUTION,
                diagnostics,
            )?,
            endpoints: usage.endpoints,
        })
    };
    let lifted_arity = arity.checked_add(parametric).ok_or(
        MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine parametric lifted arity",
        },
    )?;
    admit_product_limit(
        "multi-affine relation coefficient entries",
        factors.len(),
        lifted_arity,
        limits.max_relation_coefficient_entries,
        limits.diagnostics_enabled,
    )?;
    let mut constants = Vec::new();
    constants.try_reserve_exact(parametric).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine parametric constants",
            requested: parametric,
        }
    })?;
    let mut lifted = Vec::new();
    lifted.try_reserve_exact(factors.len()).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine lifted factors",
            requested: factors.len(),
        }
    })?;
    for (ordinal, factor) in factors.iter().enumerate() {
        if factor.denominator_coefficients().len() != arity {
            return Err(MultiAffineNumeratorExpansionError::WrongRelationArity {
                factor: ordinal,
                expected: arity,
                actual: factor.denominator_coefficients().len(),
            });
        }
        let virtual_axis = (!factor.constant().is_constant()).then(|| arity + constants.len());
        let coefficients =
            factor
                .denominator_coefficients()
                .iter()
                .cloned()
                .chain((arity..lifted_arity).map(|axis| {
                    if Some(axis) == virtual_axis {
                        context.one()
                    } else {
                        context.zero()
                    }
                }));
        let constant = if virtual_axis.is_some() {
            constants.push(factor.constant().clone());
            context.zero()
        } else {
            factor.constant().clone()
        };
        lifted.push(MultiAffineNumeratorFactor::try_new(
            constant,
            coefficients,
            factor.power(),
        )?);
    }
    let mut lifted_base = try_clone_powers(base.powers())?;
    lifted_base.try_reserve_exact(parametric).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine lifted base powers",
            requested: parametric,
        }
    })?;
    lifted_base.resize(lifted_arity, 0);
    let lifted_base = IntegralKey::try_from_preallocated(lifted_base)?;
    let Some(expanded) = expand_native_on(
        context,
        lifted_arity,
        &lifted_base,
        &lifted,
        limits,
        reserve,
    )?
    else {
        return Ok(Box::new([]));
    };
    substitute_parametric_constants(context, base, &constants, &expanded, limits)
}

fn substitute_parametric_constants(
    context: &CoefficientContext,
    base: &IntegralKey,
    constants: &[Coefficient],
    expanded: &NativeExpansion,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<Box<[MultiAffineNumeratorEndpoint]>, MultiAffineNumeratorExpansionError> {
    let polynomial = &expanded.polynomial;
    let arity = base.powers().len();
    let lifted_arity = arity + constants.len();
    admit_limit(
        "multi-affine endpoints",
        polynomial.nterms(),
        limits.max_endpoints,
        limits.diagnostics_enabled,
    )?;
    preflight_endpoint_key_storage(polynomial.nterms(), arity, limits)?;
    let operations = estimate(
        constants
            .len()
            .checked_add(1)
            .and_then(|per_term| polynomial.nterms().checked_mul(per_term)),
        "multi-affine parametric constant operations",
        limits.diagnostics_enabled,
    )?;
    admit_limit(
        "multi-affine parametric constant operations",
        operations,
        limits.max_native_polynomial_operations,
        limits.diagnostics_enabled,
    )?;
    let live_base = expanded.input_weight.checked_add(polynomial_weight(
        polynomial,
        expanded.constant_wrapper_bytes,
    )?)?;
    admit_live_coefficients(live_base, limits)?;
    // powers[a][e] = c_a^e, extended on demand up to the lifted degree.
    let mut powers: Vec<Vec<Coefficient>> = constants.iter().map(|_| vec![context.one()]).collect();
    let mut endpoints = std::collections::BTreeMap::<IntegralKey, Coefficient>::new();
    for term in 0..polynomial.nterms() {
        let exponents = polynomial.exponents(term);
        if exponents.len() != lifted_arity {
            return Err(MultiAffineNumeratorExpansionError::NativeExponentWidth {
                expected: lifted_arity,
                actual: exponents.len(),
            });
        }
        let mut key = try_clone_powers(base.powers())?;
        for (position, &exponent) in exponents[..arity].iter().enumerate() {
            key[position] = checked_lower_power(position, key[position], u64::from(exponent))?;
        }
        let mut value = contextual_coefficient(context, &polynomial.coefficients[term]);
        for (axis, &exponent) in exponents[arity..].iter().enumerate() {
            let exponent = usize::try_from(exponent).map_err(|_| {
                MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                    resource: "multi-affine parametric constant power",
                }
            })?;
            if exponent == 0 {
                continue;
            }
            while powers[axis].len() <= exponent {
                let next = context.try_mul(
                    powers[axis].last().expect("power table starts at one"),
                    &constants[axis],
                    limits.exact_algebra,
                )?;
                powers[axis].push(next);
            }
            value = context.try_mul(&value, &powers[axis][exponent], limits.exact_algebra)?;
        }
        let key = IntegralKey::try_from_preallocated(key)?;
        match endpoints.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let sum = context.try_add(entry.get(), &value, limits.exact_algebra)?;
                if sum.is_zero() {
                    entry.remove();
                } else {
                    *entry.get_mut() = sum;
                }
            }
        }
    }
    let mut output = Vec::new();
    output.try_reserve_exact(endpoints.len()).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine endpoints",
            requested: endpoints.len(),
        }
    })?;
    let mut output_weight = CoefficientWeight::default();
    for (key, coefficient) in endpoints {
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;
        output_weight = output_weight.checked_add(coefficient_weight(&coefficient)?)?;
        admit_live_coefficients(live_base.checked_add(output_weight)?, limits)?;
        output.push(MultiAffineNumeratorEndpoint { key, coefficient });
    }
    Ok(output.into_boxed_slice())
}

/// Shared native algebra, before choosing coefficient output or trace support.
struct NativeExpansion {
    polynomial: EndpointPolynomial,
    input_weight: CoefficientWeight,
    constant_wrapper_bytes: usize,
}

fn expand_native(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
    reserve: impl FnOnce(ExpansionUsage) -> Result<(), MultiAffineNumeratorExpansionError>,
) -> Result<Option<NativeExpansion>, MultiAffineNumeratorExpansionError> {
    expand_native_on(
        family.coefficient_context(),
        family.denominator_count(),
        base,
        factors,
        limits,
        reserve,
    )
}

/// Native expansion over `arity` Temporary variables of one coefficient
/// context. The parametric-constant lane calls this with virtual axes.
fn expand_native_on(
    context: &CoefficientContext,
    arity: usize,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
    reserve: impl FnOnce(ExpansionUsage) -> Result<(), MultiAffineNumeratorExpansionError>,
) -> Result<Option<NativeExpansion>, MultiAffineNumeratorExpansionError> {
    if base.powers().len() != arity {
        return Err(MultiAffineNumeratorExpansionError::WrongBaseArity {
            expected: arity,
            actual: base.powers().len(),
        });
    }
    admit_limit(
        "multi-affine factors",
        factors.len(),
        limits.max_factors,
        limits.diagnostics_enabled,
    )?;
    admit_product_limit(
        "multi-affine relation coefficient entries",
        factors.len(),
        arity,
        limits.max_relation_coefficient_entries,
        limits.diagnostics_enabled,
    )?;

    let total_power = factors.iter().try_fold(0_u64, |total, factor| {
        total.checked_add(factor.power()).ok_or(
            MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine total power",
            },
        )
    })?;
    if limits.diagnostics_enabled && total_power > limits.max_total_power {
        return Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine total power",
            requested: usize::try_from(total_power).unwrap_or(usize::MAX),
            limit: usize::try_from(limits.max_total_power).unwrap_or(usize::MAX),
        });
    }

    let (plans, projected_support, zero_product, input_weight, operation_bound) =
        preflight_factors(context, factors, arity, total_power, limits)?;
    reserve(ExpansionUsage {
        operations: operation_bound,
        endpoints: if zero_product { 0 } else { projected_support },
    })?;
    if zero_product {
        return Ok(None);
    }
    preflight_power_shifts(base, factors)?;
    admit_limit(
        "multi-affine endpoints",
        projected_support,
        limits.max_endpoints,
        limits.diagnostics_enabled,
    )?;
    preflight_endpoint_key_storage(projected_support, arity, limits)?;

    let variables = try_temporary_variables(arity)?;
    // Preserve the original constant-coefficient authentication and a
    // conservative legacy-wrapper charge, without retaining that wrapper in
    // every native arithmetic operation. This temporary is dropped here.
    let constant_wrapper_bytes = {
        let one = context.one();
        context.validate_with_limits(&one, limits.exact_algebra)?;
        coefficient_weight(&one)?
            .clone_owned_bytes
            .max(size_of::<Rational>())
    };
    let template = EndpointPolynomial::new(&Q, None, Arc::new(variables));
    let mut polynomial = template.constant(Rational::one());
    validate_polynomial(&polynomial, limits)?;
    admit_live_coefficients(
        input_weight.checked_add(polynomial_weight(&polynomial, constant_wrapper_bytes)?)?,
        limits,
    )?;

    for plan in plans {
        let factor = &factors[plan.ordinal];
        let affine = affine_polynomial(&template, factor, arity)?;
        validate_polynomial(&affine, limits)?;
        let powered = catch_unwind(AssertUnwindSafe(|| affine.pow(plan.native_power)))
            .map_err(|_| MultiAffineNumeratorExpansionError::NativePolynomialPanic)?;
        validate_polynomial(&powered, limits)?;
        if powered.nterms() > plan.support_bound {
            return Err(
                MultiAffineNumeratorExpansionError::NativePolynomialSupportExceeded {
                    actual: powered.nterms(),
                    limit: plan.support_bound,
                },
            );
        }
        let next = catch_unwind(AssertUnwindSafe(|| &polynomial * &powered))
            .map_err(|_| MultiAffineNumeratorExpansionError::NativePolynomialPanic)?;
        validate_polynomial(&next, limits)?;
        if next.nterms() > plan.prefix_support_bound
            || (limits.diagnostics_enabled && next.nterms() > limits.max_native_polynomial_terms)
        {
            return Err(
                MultiAffineNumeratorExpansionError::NativePolynomialSupportExceeded {
                    actual: next.nterms(),
                    limit: plan
                        .prefix_support_bound
                        .min(limits.max_native_polynomial_terms),
                },
            );
        }
        let live_weight = input_weight
            .checked_add(polynomial_weight(&polynomial, constant_wrapper_bytes)?)?
            .checked_add(polynomial_weight(&affine, constant_wrapper_bytes)?)?
            .checked_add(polynomial_weight(&powered, constant_wrapper_bytes)?)?
            .checked_add(polynomial_weight(&next, constant_wrapper_bytes)?)?;
        admit_live_coefficients(live_weight, limits)?;
        polynomial = next;
    }

    Ok(Some(NativeExpansion {
        polynomial,
        input_weight,
        constant_wrapper_bytes,
    }))
}

fn preflight_factors(
    context: &CoefficientContext,
    factors: &[MultiAffineNumeratorFactor],
    arity: usize,
    total_power: u64,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<
    (Vec<FactorPlan>, usize, bool, CoefficientWeight, usize),
    MultiAffineNumeratorExpansionError,
> {
    let mut plans = Vec::new();
    plans.try_reserve_exact(factors.len()).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine factor plans",
            requested: factors.len(),
        }
    })?;
    let mut projected_support = 1_usize;
    let mut prefix = PrefixSupport::try_new(arity)?;
    let mut operation_bound = 0_usize;
    let mut exponent_rows_peak = 1_usize;
    let mut zero_product = false;
    let mut input_weight = CoefficientWeight::default();
    for (ordinal, factor) in factors.iter().enumerate() {
        if factor.denominator_coefficients().len() != arity {
            return Err(MultiAffineNumeratorExpansionError::WrongRelationArity {
                factor: ordinal,
                expected: arity,
                actual: factor.denominator_coefficients().len(),
            });
        }
        for (coefficient_ordinal, coefficient) in std::iter::once(factor.constant())
            .chain(factor.denominator_coefficients())
            .enumerate()
        {
            // The total product power is a conservative authentication
            // envelope. This Stage-1 native boundary then rejects every
            // parameter-dependent outer coefficient before arithmetic.
            context.preflight_power_with_limits(coefficient, total_power, limits.exact_algebra)?;
            if !coefficient.is_constant() {
                return Err(
                    MultiAffineNumeratorExpansionError::NonconstantExpansionCoefficient {
                        factor: ordinal,
                        coefficient: coefficient_ordinal,
                    },
                );
            }
            input_weight = input_weight.checked_add(coefficient_weight(coefficient)?)?;
        }
        if factor.power() == 0 {
            continue;
        }
        let native_limit = i32::MAX as u32;
        if factor.power() > u64::from(native_limit) {
            return Err(MultiAffineNumeratorExpansionError::NativeExponentLimit {
                factor: ordinal,
                requested: factor.power(),
                limit: native_limit,
            });
        }
        let native_power = usize::try_from(factor.power()).map_err(|_| {
            MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine native factor power",
            }
        })?;
        let width = usize::from(!factor.constant().is_zero())
            .checked_add(
                factor
                    .denominator_coefficients()
                    .iter()
                    .filter(|coefficient| !coefficient.is_zero())
                    .count(),
            )
            .ok_or(MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine factor width",
            })?;
        if width == 0 {
            zero_product = true;
            continue;
        }
        let support_bound = match multiset_support(factor.power(), width) {
            Ok(bound) => bound,
            Err(error) if limits.diagnostics_enabled => return Err(error),
            Err(_) => usize::MAX, // Saturated diagnostic estimate, not an allocation request.
        };
        admit_limit(
            "multi-affine factor support",
            support_bound,
            limits.max_native_polynomial_terms,
            limits.diagnostics_enabled,
        )?;
        let prior_support = projected_support;
        // Every pair may contribute native multiplication work, even when
        // many pairs collide into the same output monomial. Never replace
        // this operation bound with the smaller output-support envelope.
        let multiply_operations = estimate(
            prior_support.checked_mul(support_bound),
            "multi-affine native polynomial operations",
            limits.diagnostics_enabled,
        )?;
        prefix.include(factor)?;
        projected_support = prefix.refine(multiply_operations);
        admit_limit(
            "multi-affine projected polynomial support",
            projected_support,
            limits.max_native_polynomial_terms,
            limits.diagnostics_enabled,
        )?;
        let power_operations = estimate(
            support_bound
                .checked_mul(width)
                .and_then(|value| value.checked_mul(native_power.max(1))),
            "multi-affine native polynomial operations",
            limits.diagnostics_enabled,
        )?;
        operation_bound = estimate(
            operation_bound
                .checked_add(power_operations)
                .and_then(|value| value.checked_add(multiply_operations)),
            "multi-affine native polynomial operations",
            limits.diagnostics_enabled,
        )?;
        admit_limit(
            "multi-affine native polynomial operations",
            operation_bound,
            limits.max_native_polynomial_operations,
            limits.diagnostics_enabled,
        )?;
        let live_rows = estimate(
            prior_support
                .checked_add(width)
                .and_then(|value| value.checked_add(support_bound))
                .and_then(|value| value.checked_add(projected_support)),
            "multi-affine native exponent rows",
            limits.diagnostics_enabled,
        )?;
        exponent_rows_peak = exponent_rows_peak.max(live_rows);
        plans.push(FactorPlan {
            ordinal,
            native_power,
            support_bound,
            prefix_support_bound: projected_support,
        });
    }
    admit_product_limit(
        "multi-affine native exponent entries",
        exponent_rows_peak,
        arity,
        limits.max_native_exponent_entries,
        limits.diagnostics_enabled,
    )?;
    admit_live_coefficients(input_weight, limits)?;
    Ok((
        plans,
        projected_support,
        zero_product,
        input_weight,
        operation_bound,
    ))
}

fn affine_polynomial(
    template: &EndpointPolynomial,
    factor: &MultiAffineNumeratorFactor,
    arity: usize,
) -> Result<EndpointPolynomial, MultiAffineNumeratorExpansionError> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut affine = template.zero();
        if !factor.constant().is_zero() {
            affine = &affine + &template.constant(authenticated_rational(factor.constant()));
        }
        for (position, coefficient) in factor.denominator_coefficients().iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            let mut exponents = Vec::new();
            exponents.try_reserve_exact(arity).map_err(|_| {
                MultiAffineNumeratorExpansionError::AllocationFailure {
                    resource: "multi-affine monomial exponents",
                    requested: arity,
                }
            })?;
            exponents.resize(arity, 0_u32);
            exponents[position] = 1;
            affine = &affine + &template.monomial(authenticated_rational(coefficient), exponents);
        }
        Ok::<_, MultiAffineNumeratorExpansionError>(affine)
    }))
    .map_err(|_| MultiAffineNumeratorExpansionError::NativePolynomialPanic)?
}

fn materialize_endpoints(
    context: &CoefficientContext,
    base: &IntegralKey,
    polynomial: &EndpointPolynomial,
    input_weight: CoefficientWeight,
    constant_wrapper_bytes: usize,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<Box<[MultiAffineNumeratorEndpoint]>, MultiAffineNumeratorExpansionError> {
    let arity = base.powers().len();
    admit_limit(
        "multi-affine endpoints",
        polynomial.nterms(),
        limits.max_endpoints,
        limits.diagnostics_enabled,
    )?;
    preflight_endpoint_key_storage(polynomial.nterms(), arity, limits)?;
    // Symbolica's sparse polynomial is already expanded, coalesced, and free
    // of zero coefficients. The exponent-vector-to-key map `e -> base-e` is
    // injective, so no second coefficient-addition layer is needed. Keep the
    // native polynomial in the live census while cloning output coefficients.
    let native_weight = polynomial_weight(polynomial, constant_wrapper_bytes)?;
    let live_base = input_weight.checked_add(native_weight)?;
    admit_live_coefficients(live_base, limits)?;
    let requested = polynomial.nterms();
    let mut endpoints = Vec::new();
    endpoints.try_reserve_exact(requested).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine endpoints",
            requested,
        }
    })?;
    let mut output_weight = CoefficientWeight::default();
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        if exponents.len() != arity {
            return Err(MultiAffineNumeratorExpansionError::NativeExponentWidth {
                expected: arity,
                actual: exponents.len(),
            });
        }
        if coefficient.is_zero() {
            continue;
        }
        let mut powers = try_clone_powers(base.powers())?;
        for (position, &exponent) in exponents.iter().enumerate() {
            let decrement = u64::from(exponent);
            powers[position] = checked_lower_power(position, powers[position], decrement)?;
        }
        let retained = rational_weight(coefficient, constant_wrapper_bytes)?;
        let prospective = live_base
            .checked_add(output_weight)?
            .checked_add(retained)?;
        admit_live_coefficients(prospective, limits)?;
        // Native Q owns normalization. Its numerator and denominator are
        // already coprime; reconstruct only the public output wrapper on the
        // ORIGINAL family map, after admitting its prospective clone storage.
        let coefficient = contextual_coefficient(context, coefficient);
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;
        let actual = coefficient_weight(&coefficient)?;
        if actual.terms > retained.terms || actual.clone_owned_bytes > retained.clone_owned_bytes {
            return Err(MultiAffineNumeratorExpansionError::Invariant {
                detail: "constant rational wrapper exceeded its prospective ownership bound",
            });
        }
        endpoints.push(MultiAffineNumeratorEndpoint {
            key: IntegralKey::try_from_preallocated(powers)?,
            coefficient,
        });
        output_weight = output_weight.checked_add(retained)?;
    }
    endpoints.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    if endpoints.windows(2).any(|pair| pair[0].key >= pair[1].key) {
        return Err(MultiAffineNumeratorExpansionError::Invariant {
            detail: "Symbolica returned duplicate or unordered endpoint monomials",
        });
    }
    admit_limit(
        "multi-affine endpoints",
        endpoints.len(),
        limits.max_endpoints,
        limits.diagnostics_enabled,
    )?;
    preflight_endpoint_key_storage(endpoints.len(), arity, limits)?;
    Ok(endpoints.into_boxed_slice())
}

fn validate_polynomial(
    polynomial: &EndpointPolynomial,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    symbolica::utils::resources::checkpoint()
        .map_err(crate::algebra::ExactAlgebraError::Resource)?;
    if limits.diagnostics_enabled && polynomial.nterms() > limits.max_native_polynomial_terms {
        return Err(
            MultiAffineNumeratorExpansionError::NativePolynomialSupportExceeded {
                actual: polynomial.nterms(),
                limit: limits.max_native_polynomial_terms,
            },
        );
    }
    for coefficient in &polynomial.coefficients {
        if coefficient.is_zero()
            || coefficient.denominator_ref().is_zero()
            || coefficient.denominator_ref().is_negative()
        {
            return Err(MultiAffineNumeratorExpansionError::Invariant {
                detail: "native Q returned a zero or noncanonical sparse coefficient",
            });
        }
    }
    Ok(())
}

fn polynomial_weight(
    polynomial: &EndpointPolynomial,
    constant_wrapper_bytes: usize,
) -> Result<CoefficientWeight, MultiAffineNumeratorExpansionError> {
    polynomial
        .coefficients
        .iter()
        .try_fold(CoefficientWeight::default(), |weight, coefficient| {
            weight.checked_add(rational_weight(coefficient, constant_wrapper_bytes)?)
        })
}

fn coefficient_weight(
    coefficient: &Coefficient,
) -> Result<CoefficientWeight, MultiAffineNumeratorExpansionError> {
    let terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine retained coefficient terms",
        })?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine retained coefficient clone-owned bytes",
        },
    )?;
    Ok(CoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

/// Admit borrowed ingress before callers clone the selected coefficient rows.
/// Uses the same cumulative ownership accounting as native expansion itself.
pub(crate) fn preflight_coefficient_clones<'a>(
    coefficients: impl IntoIterator<Item = &'a Coefficient>,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    let weight = coefficients
        .into_iter()
        .try_fold(CoefficientWeight::default(), |weight, coefficient| {
            weight.checked_add(coefficient_weight(coefficient)?)
        })?;
    admit_live_coefficients(weight, limits)
}

fn admit_live_coefficients(
    weight: CoefficientWeight,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    admit_limit(
        "multi-affine retained coefficient terms",
        weight.terms,
        limits.max_retained_coefficient_terms,
        limits.diagnostics_enabled,
    )?;
    admit_limit(
        "multi-affine retained coefficient clone-owned bytes",
        weight.clone_owned_bytes,
        limits.max_retained_coefficient_clone_owned_bytes,
        limits.diagnostics_enabled,
    )
}

fn multiset_support(power: u64, width: usize) -> Result<usize, MultiAffineNumeratorExpansionError> {
    if power == 0 || width == 1 {
        return Ok(1);
    }
    let mut support = 1_u128;
    for index in 1..width {
        let factor = u128::from(power).checked_add(index as u128).ok_or(
            MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine factor support",
            },
        )?;
        support = support.checked_mul(factor).ok_or(
            MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                resource: "multi-affine factor support",
            },
        )? / index as u128;
    }
    usize::try_from(support).map_err(|_| {
        MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine factor support",
        }
    })
}

fn checked_lower_power(
    position: usize,
    power: i64,
    decrement: u64,
) -> Result<i64, MultiAffineNumeratorExpansionError> {
    let shifted = i128::from(power) - i128::from(decrement);
    i64::try_from(shifted).map_err(
        |_| MultiAffineNumeratorExpansionError::PowerShiftUnderflow {
            position,
            power,
            decrement,
        },
    )
}

fn preflight_power_shifts(
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
) -> Result<(), MultiAffineNumeratorExpansionError> {
    for position in 0..base.powers().len() {
        let decrement = factors.iter().try_fold(0_u64, |degree, factor| {
            if factor.denominator_coefficients()[position].is_zero() {
                Ok(degree)
            } else {
                degree.checked_add(factor.power()).ok_or(
                    MultiAffineNumeratorExpansionError::NativeExponentDegreeOverflow { position },
                )
            }
        })?;
        if decrement > i32::MAX as u64 {
            return Err(
                MultiAffineNumeratorExpansionError::NativeExponentDegreeOverflow { position },
            );
        }
        checked_lower_power(position, base.powers()[position], decrement)?;
    }
    Ok(())
}

fn try_clone_powers(powers: &[i64]) -> Result<Vec<i64>, MultiAffineNumeratorExpansionError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(powers.len()).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine endpoint powers",
            requested: powers.len(),
        }
    })?;
    cloned.extend_from_slice(powers);
    Ok(cloned)
}

fn try_temporary_variables(
    arity: usize,
) -> Result<Vec<PolyVariable>, MultiAffineNumeratorExpansionError> {
    let mut variables = Vec::new();
    variables.try_reserve_exact(arity).map_err(|_| {
        MultiAffineNumeratorExpansionError::AllocationFailure {
            resource: "multi-affine Symbolica variables",
            requested: arity,
        }
    })?;
    variables.extend((0..arity).map(PolyVariable::Temporary));
    Ok(variables)
}

fn preflight_endpoint_key_storage(
    endpoint_count: usize,
    arity: usize,
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    admit_product_limit(
        "multi-affine endpoint power entries",
        endpoint_count,
        arity,
        limits.max_endpoint_power_entries,
        limits.diagnostics_enabled,
    )?;
    let retained_bytes = estimate(
        endpoint_count
            .checked_mul(arity)
            .and_then(|entries| entries.checked_mul(size_of::<i64>()))
            .and_then(|payload| {
                endpoint_count
                    .checked_mul(size_of::<IntegralKey>())
                    .and_then(|owners| payload.checked_add(owners))
            }),
        "multi-affine retained endpoint key bytes",
        limits.diagnostics_enabled,
    )?;
    admit_limit(
        "multi-affine retained endpoint key bytes",
        retained_bytes,
        limits.max_retained_endpoint_key_bytes,
        limits.diagnostics_enabled,
    )
}

fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
    diagnostics_enabled: bool,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    symbolica::utils::resources::checkpoint()
        .map_err(crate::algebra::ExactAlgebraError::Resource)?;
    if !diagnostics_enabled || requested <= limit {
        Ok(())
    } else {
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn admit_product_limit(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
    diagnostics_enabled: bool,
) -> Result<(), MultiAffineNumeratorExpansionError> {
    let requested = estimate(left.checked_mul(right), resource, diagnostics_enabled)?;
    admit_limit(resource, requested, limit, diagnostics_enabled)
}

#[cfg(test)]
#[path = "rational_tests.rs"]
mod rational_tests;

#[cfg(test)]
#[path = "support_tests.rs"]
mod support_tests;

#[cfg(test)]
#[path = "resource_policy_tests.rs"]
mod resource_policy_tests;

/// Estimates do not allocate. Overflow remains an explicit diagnostic; ordinary
/// execution saturates the census and relies on the shared RAM/time budget.
fn estimate(
    value: Option<usize>,
    resource: &'static str,
    diagnostics_enabled: bool,
) -> Result<usize, MultiAffineNumeratorExpansionError> {
    match value {
        Some(value) => Ok(value),
        None if diagnostics_enabled => {
            Err(MultiAffineNumeratorExpansionError::ResourceCountOverflow { resource })
        }
        None => Ok(usize::MAX),
    }
}
