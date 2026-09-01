//! Cold, non-owning exact endpoint materialization.
//!
//! An expansion in this module is only an exact structural identity.  It does
//! not enter an artifact, reducer, owner ledger, or closure cover.  Wider
//! affine powers are delegated to Symbolica's native sparse-polynomial power;
//! RustRed supplies checked support admission, exponent-to-key routing, exact
//! coefficient authentication, and deterministic coalescing.
//!
//! The current native-expansion boundary deliberately admits only
//! parameter-independent affine coefficients.  Symbolica's outer sparse
//! polynomial then has one-term rational constants as coefficients, so the
//! structural operation cap is an auditable pre-native envelope.  Supporting
//! parameter-dependent coefficients requires a separate native coefficient
//! term/output admission proof and is intentionally not inferred here.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{
    IntegerRing, MultivariatePolynomial, PolyVariable, RationalPolynomialField, Ring, Z,
};

use crate::algebra::{
    Coefficient, CoefficientContext, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::symmetry::{Canonicalizer, RoutingCoefficient};

use super::error::FactorizedNumeratorLiftError;
use super::limits::FactorizedNumeratorLiftExpansionLimits;
use super::model::{
    CompiledFactorizationRouting, FactorizedNumeratorLiftAction, FactorizedNumeratorLiftEndpoint,
    FactorizedNumeratorLiftExpansion, FactorizedNumeratorLiftStart,
};

type EndpointPolynomial = MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u32>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RetainedCoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

struct RetainedCoefficientCensus {
    current: RetainedCoefficientWeight,
    max_terms: usize,
    max_clone_owned_bytes: usize,
}

impl RetainedCoefficientCensus {
    fn empty(limits: FactorizedNumeratorLiftExpansionLimits) -> Self {
        Self {
            current: RetainedCoefficientWeight::default(),
            max_terms: limits.max_retained_endpoint_coefficient_terms,
            max_clone_owned_bytes: limits.max_retained_endpoint_coefficient_clone_owned_bytes,
        }
    }

    /// Admit coefficients that remain live while a second endpoint collection
    /// is constructed.  This is used both for Symbolica's native polynomial
    /// output and, critically, for canonicalization's borrowed input
    /// expansion; subsequent insertions therefore bound the live input+output
    /// clone peak rather than only the returned map.
    fn with_live_inputs<'coefficient>(
        coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
        limits: FactorizedNumeratorLiftExpansionLimits,
    ) -> Result<Self, FactorizedNumeratorLiftError> {
        let mut census = Self::empty(limits);
        for coefficient in coefficients {
            census.replace(
                RetainedCoefficientWeight::default(),
                retained_coefficient_weight(coefficient)?,
            )?;
        }
        Ok(census)
    }

    /// Check a temporary result while the old retained coefficient is still
    /// live.  Native arithmetic scratch remains governed by exact-algebra and
    /// structural-operation policies; this bound covers the returned exact
    /// coefficient object before transactional replacement.
    fn admit_additional(
        &self,
        additional: RetainedCoefficientWeight,
    ) -> Result<(), FactorizedNumeratorLiftError> {
        let prospective = self.current.checked_add(additional)?;
        self.admit(prospective)
    }

    fn replace(
        &mut self,
        old: RetainedCoefficientWeight,
        new: RetainedCoefficientWeight,
    ) -> Result<(), FactorizedNumeratorLiftError> {
        let prospective = self.current.checked_sub(old)?.checked_add(new)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    fn admit(
        &self,
        prospective: RetainedCoefficientWeight,
    ) -> Result<(), FactorizedNumeratorLiftError> {
        admit_limit(
            "factorized numerator retained endpoint coefficient terms",
            prospective.terms,
            self.max_terms,
        )?;
        admit_limit(
            "factorized numerator retained endpoint coefficient clone-owned bytes",
            prospective.clone_owned_bytes,
            self.max_clone_owned_bytes,
        )
    }
}

impl RetainedCoefficientWeight {
    fn checked_add(self, other: Self) -> Result<Self, FactorizedNumeratorLiftError> {
        Ok(Self {
            terms: self.terms.checked_add(other.terms).ok_or(
                FactorizedNumeratorLiftError::ResourceCountOverflow {
                    resource: "factorized numerator retained endpoint coefficient terms",
                },
            )?,
            clone_owned_bytes: self.clone_owned_bytes.checked_add(other.clone_owned_bytes).ok_or(
                FactorizedNumeratorLiftError::ResourceCountOverflow {
                    resource:
                        "factorized numerator retained endpoint coefficient clone-owned bytes",
                },
            )?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, FactorizedNumeratorLiftError> {
        Ok(Self {
            terms: self.terms.checked_sub(other.terms).ok_or(
                FactorizedNumeratorLiftError::Invariant {
                    detail: "retained endpoint coefficient-term census underflowed",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_sub(other.clone_owned_bytes)
                .ok_or(FactorizedNumeratorLiftError::Invariant {
                    detail: "retained endpoint coefficient-byte census underflowed",
                })?,
        })
    }
}

fn retained_coefficient_weight(
    coefficient: &Coefficient,
) -> Result<RetainedCoefficientWeight, FactorizedNumeratorLiftError> {
    let terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator retained endpoint coefficient terms",
        })?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator retained endpoint coefficient clone-owned bytes",
        },
    )?;
    Ok(RetainedCoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

fn admit_single_retained_coefficient(
    coefficient: &Coefficient,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<(), FactorizedNumeratorLiftError> {
    RetainedCoefficientCensus::empty(limits).admit(retained_coefficient_weight(coefficient)?)
}

impl CompiledFactorizationRouting {
    /// Materialize an exact pure route as a one-endpoint structural identity.
    pub(crate) fn try_expand_pure_routing(
        &self,
        family: &IntegralFamily,
        source: &IntegralKey,
        limits: FactorizedNumeratorLiftExpansionLimits,
    ) -> Result<FactorizedNumeratorLiftExpansion, FactorizedNumeratorLiftError> {
        authenticate_family(self.family_fingerprint(), family)?;
        admit_limit("factorized numerator endpoints", 1, limits.max_endpoints)?;
        preflight_endpoint_key_storage(1, family.denominator_count(), limits)?;
        let coefficient = family.coefficient_context().one();
        family
            .coefficient_context()
            .validate_with_limits(&coefficient, limits.exact_algebra)?;
        admit_single_retained_coefficient(&coefficient, limits)?;
        let endpoint = FactorizedNumeratorLiftEndpoint {
            key: self.try_route_key(source)?,
            coefficient,
        };
        Ok(FactorizedNumeratorLiftExpansion {
            family_fingerprint: family.fingerprint_owner(),
            routing_identity: self.identity.clone(),
            source: try_clone_integral_key(source)?,
            endpoints: Box::new([endpoint]),
        })
    }
}

impl FactorizedNumeratorLiftAction {
    /// Materialize all exact endpoints of this authenticated action.
    ///
    /// The result remains a cold structural relation.  In particular, this
    /// method never recursively dispatches an emitted key through the action.
    pub(crate) fn try_expand_endpoints(
        &self,
        family: &IntegralFamily,
        source: &IntegralKey,
        limits: FactorizedNumeratorLiftExpansionLimits,
    ) -> Result<FactorizedNumeratorLiftExpansion, FactorizedNumeratorLiftError> {
        authenticate_family(self.routing().family_fingerprint(), family)?;
        admit_limit("factorized numerator endpoints", 1, limits.max_endpoints)?;
        preflight_endpoint_key_storage(1, family.denominator_count(), limits)?;
        match self.try_start(source)? {
            FactorizedNumeratorLiftStart::Routed(key) => {
                let coefficient = family.coefficient_context().one();
                family
                    .coefficient_context()
                    .validate_with_limits(&coefficient, limits.exact_algebra)?;
                admit_single_retained_coefficient(&coefficient, limits)?;
                Ok(FactorizedNumeratorLiftExpansion {
                    family_fingerprint: family.fingerprint_owner(),
                    routing_identity: self.routing.identity.clone(),
                    source: try_clone_integral_key(source)?,
                    endpoints: Box::new([FactorizedNumeratorLiftEndpoint { key, coefficient }]),
                })
            }
            FactorizedNumeratorLiftStart::Auxiliary(state) => {
                let power = state.measure().remaining_power();
                let relation = self.affine_relation();
                let mut denominator_positions = Vec::new();
                denominator_positions
                    .try_reserve_exact(relation.denominator_coefficients().len())
                    .map_err(|_| FactorizedNumeratorLiftError::AllocationFailure {
                        resource: "factorized numerator denominator positions",
                        requested: relation.denominator_coefficients().len(),
                    })?;
                denominator_positions.extend(
                    relation
                        .denominator_coefficients()
                        .iter()
                        .enumerate()
                        .filter_map(|(position, coefficient)| {
                            (!coefficient.is_zero()).then_some(position)
                        }),
                );
                let has_constant = !relation.constant().is_zero();
                let width = denominator_positions
                    .len()
                    .checked_add(usize::from(has_constant))
                    .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
                        resource: "factorized numerator expansion branches",
                    })?;
                if width != self.branch_width() || width == 0 {
                    return Err(FactorizedNumeratorLiftError::Invariant {
                        detail: "compiled action width disagrees with its affine relation",
                    });
                }

                let support = preflight_expansion(
                    power,
                    width,
                    denominator_positions.len(),
                    family.denominator_count(),
                    limits,
                )?;
                preflight_routed_extrema(state.routed_powers(), &denominator_positions, power)?;
                let context = family.coefficient_context();
                let endpoints = if width == 1 {
                    expand_width_one(
                        context,
                        state.routed_powers(),
                        relation.constant(),
                        relation.denominator_coefficients(),
                        &denominator_positions,
                        power,
                        limits,
                    )?
                } else {
                    expand_symbolica_power(
                        context,
                        state.routed_powers(),
                        relation.constant(),
                        relation.denominator_coefficients(),
                        &denominator_positions,
                        has_constant,
                        power,
                        support,
                        limits,
                    )?
                };
                Ok(FactorizedNumeratorLiftExpansion {
                    family_fingerprint: family.fingerprint_owner(),
                    routing_identity: self.routing.identity.clone(),
                    source: try_clone_integral_key(source)?,
                    endpoints,
                })
            }
        }
    }
}

impl FactorizedNumeratorLiftExpansion {
    /// Canonicalize endpoint keys under one authenticated family action and
    /// exactly coalesce coincident images.  The source and compiled-routing
    /// capability remain unchanged; canonicalization is deliberately a
    /// caller-boundary operation rather than an implicit part of raw
    /// expansion.
    pub(crate) fn try_canonicalize_endpoints(
        &self,
        family: &IntegralFamily,
        canonicalizer: &Canonicalizer,
        limits: FactorizedNumeratorLiftExpansionLimits,
    ) -> Result<Self, FactorizedNumeratorLiftError> {
        authenticate_family(self.family_fingerprint(), family)?;
        if canonicalizer.family_fingerprint() != self.family_fingerprint()
            || canonicalizer.arity() != self.source.powers().len()
        {
            return Err(FactorizedNumeratorLiftError::WrongCanonicalizerFamily);
        }
        preflight_endpoint_key_storage(self.endpoints.len(), canonicalizer.arity(), limits)?;
        admit_product_limit(
            "factorized numerator canonicalization routes",
            &[self.endpoints.len(), canonicalizer.group_order()],
            limits.max_canonicalization_routes,
        )?;
        admit_product_limit(
            "factorized numerator canonicalization transported power entries",
            &[
                self.endpoints.len(),
                canonicalizer.group_order(),
                canonicalizer.arity(),
            ],
            limits.max_canonicalization_power_entries,
        )?;
        let context = family.coefficient_context();
        let mut coefficient_census = RetainedCoefficientCensus::with_live_inputs(
            self.endpoints
                .iter()
                .map(FactorizedNumeratorLiftEndpoint::coefficient),
            limits,
        )?;
        let mut coalesced = BTreeMap::new();
        let mut additions = 0_usize;
        for endpoint in &self.endpoints {
            let canonicalization = canonicalizer.canonicalize(&endpoint.key)?;
            let coefficient = match canonicalization.route().coefficient() {
                RoutingCoefficient::One => &endpoint.coefficient,
            };
            let canonical = try_clone_integral_key(canonicalization.canonical())?;
            try_insert_endpoint(
                &mut coalesced,
                canonical,
                coefficient,
                context,
                limits,
                &mut additions,
                &mut coefficient_census,
            )?;
        }
        Ok(Self {
            family_fingerprint: self.family_fingerprint.clone(),
            routing_identity: self.routing_identity.clone(),
            source: try_clone_integral_key(&self.source)?,
            endpoints: finish_endpoints(coalesced)?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn expand_symbolica_power(
    context: &CoefficientContext,
    routed_base: &[i64],
    constant: &Coefficient,
    denominator_coefficients: &[Coefficient],
    denominator_positions: &[usize],
    has_constant: bool,
    power: u64,
    expected_support: usize,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<Box<[FactorizedNumeratorLiftEndpoint]>, FactorizedNumeratorLiftError> {
    let native_power = usize::try_from(power).map_err(|_| {
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator native power",
        }
    })?;
    let native_limit = i32::MAX as u32;
    if power > u64::from(native_limit) {
        return Err(FactorizedNumeratorLiftError::NativeExpansionExponentLimit {
            requested: power,
            limit: native_limit,
        });
    }
    if has_constant {
        authenticate_constant_expansion_coefficient(context, constant, power, limits)?;
    }
    for &position in denominator_positions {
        authenticate_constant_expansion_coefficient(
            context,
            &denominator_coefficients[position],
            power,
            limits,
        )?;
    }

    let mut variables = Vec::new();
    variables
        .try_reserve_exact(denominator_positions.len())
        .map_err(|_| FactorizedNumeratorLiftError::AllocationFailure {
            resource: "factorized numerator Symbolica variables",
            requested: denominator_positions.len(),
        })?;
    variables.extend((0..denominator_positions.len()).map(PolyVariable::Temporary));
    let variables = Arc::new(variables);
    let field = RationalPolynomialField::new(Z);
    let template = EndpointPolynomial::new(&field, None, variables);
    let polynomial = catch_unwind(AssertUnwindSafe(|| {
        let mut affine = template.zero();
        if has_constant {
            affine = &affine + &template.constant(constant.clone());
        }
        for (variable, &position) in denominator_positions.iter().enumerate() {
            let mut exponents = Vec::new();
            exponents
                .try_reserve_exact(denominator_positions.len())
                .map_err(|_| FactorizedNumeratorLiftError::AllocationFailure {
                    resource: "factorized numerator Symbolica monomial exponents",
                    requested: denominator_positions.len(),
                })?;
            exponents.resize(denominator_positions.len(), 0_u32);
            exponents[variable] = 1;
            let term = template.monomial(denominator_coefficients[position].clone(), exponents);
            affine = &affine + &term;
        }
        Ok::<_, FactorizedNumeratorLiftError>(affine.pow(native_power))
    }))
    .map_err(|_| FactorizedNumeratorLiftError::NativeExpansionPanic)??;

    if polynomial.nterms() != expected_support {
        return Err(
            FactorizedNumeratorLiftError::NativeExpansionSupportMismatch {
                expected: expected_support,
                actual: polynomial.nterms(),
            },
        );
    }
    let mut coefficient_census =
        RetainedCoefficientCensus::with_live_inputs(polynomial.coefficients.iter(), limits)?;
    let mut coalesced = BTreeMap::new();
    let mut additions = 0_usize;
    for (term, (coefficient, exponents)) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
        .enumerate()
    {
        let routed_degree = exponents.iter().try_fold(0_u64, |total, &exponent| {
            total.checked_add(u64::from(exponent))
        });
        let Some(routed_degree) = routed_degree else {
            return Err(FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "factorized numerator routed monomial degree",
            });
        };
        if routed_degree > power || (!has_constant && routed_degree != power) {
            return Err(FactorizedNumeratorLiftError::NativeExpansionTermDegree {
                term,
                requested: power,
                actual: routed_degree,
            });
        }
        let key = route_exponents(routed_base, denominator_positions, exponents)?;
        try_insert_endpoint(
            &mut coalesced,
            key,
            coefficient,
            context,
            limits,
            &mut additions,
            &mut coefficient_census,
        )?;
    }
    let endpoints = finish_endpoints(coalesced)?;
    if endpoints.len() != expected_support {
        return Err(
            FactorizedNumeratorLiftError::NativeExpansionSupportMismatch {
                expected: expected_support,
                actual: endpoints.len(),
            },
        );
    }
    Ok(endpoints)
}

fn expand_width_one(
    context: &CoefficientContext,
    routed_base: &[i64],
    constant: &Coefficient,
    denominator_coefficients: &[Coefficient],
    denominator_positions: &[usize],
    power: u64,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<Box<[FactorizedNumeratorLiftEndpoint]>, FactorizedNumeratorLiftError> {
    let (coefficient, key) = match denominator_positions {
        [] => (
            symbolica_coefficient_power(context, constant, power, limits)?,
            IntegralKey::try_new(routed_base.iter().copied())?,
        ),
        [position] => (
            symbolica_coefficient_power(
                context,
                &denominator_coefficients[*position],
                power,
                limits,
            )?,
            route_single_decrement(routed_base, *position, power)?,
        ),
        _ => {
            return Err(FactorizedNumeratorLiftError::Invariant {
                detail: "width-one expansion has multiple denominator branches",
            });
        }
    };
    if coefficient.is_zero() {
        Ok(Box::new([]))
    } else {
        admit_single_retained_coefficient(&coefficient, limits)?;
        Ok(Box::new([FactorizedNumeratorLiftEndpoint {
            key,
            coefficient,
        }]))
    }
}

fn symbolica_coefficient_power(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    power: u64,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<Coefficient, FactorizedNumeratorLiftError> {
    authenticate_constant_expansion_coefficient(context, coefficient, power, limits)?;
    if power == 0 || coefficient == &context.one() {
        return Ok(context.one());
    }
    let minus_one = context.integer(-1);
    if coefficient == &minus_one {
        return Ok(if power % 2 == 0 {
            context.one()
        } else {
            minus_one
        });
    }
    let native_limit = u32::MAX;
    if power > u64::from(native_limit) {
        return Err(
            FactorizedNumeratorLiftError::NativeDirectCoefficientPowerExponentLimit {
                requested: power,
                limit: native_limit,
            },
        );
    }
    let requested = usize::try_from(power).map_err(|_| {
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator direct coefficient power",
        }
    })?;
    admit_limit(
        "factorized numerator direct coefficient power",
        requested,
        limits.max_direct_coefficient_power,
    )?;
    let field = RationalPolynomialField::new(Z);
    let powered = catch_unwind(AssertUnwindSafe(|| field.pow(coefficient, power)))
        .map_err(|_| FactorizedNumeratorLiftError::NativeExpansionPanic)?;
    context.validate_with_limits(&powered, limits.exact_algebra)?;
    Ok(powered)
}

fn authenticate_constant_expansion_coefficient(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    power: u64,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<(), FactorizedNumeratorLiftError> {
    context.preflight_power_with_limits(coefficient, power, limits.exact_algebra)?;
    if coefficient.is_constant() {
        Ok(())
    } else {
        Err(FactorizedNumeratorLiftError::NonconstantExpansionCoefficient)
    }
}

fn preflight_expansion(
    power: u64,
    width: usize,
    exponent_width: usize,
    family_arity: usize,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<usize, FactorizedNumeratorLiftError> {
    let support = multinomial_support(power, width, limits.max_endpoints)?;
    preflight_endpoint_key_storage(support, family_arity, limits)?;
    admit_product_limit(
        "factorized numerator exponent entries",
        &[support, exponent_width],
        limits.max_exponent_entries,
    )?;
    if width > 1 {
        let power = usize::try_from(power).map_err(|_| {
            FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "factorized numerator structural term operations",
            }
        })?;
        admit_product_limit(
            "factorized numerator structural term operations",
            &[support, width, power],
            limits.max_structural_term_operations,
        )?;
    }
    Ok(support)
}

fn multinomial_support(
    power: u64,
    width: usize,
    limit: usize,
) -> Result<usize, FactorizedNumeratorLiftError> {
    if width == 0 {
        return Err(FactorizedNumeratorLiftError::Invariant {
            detail: "an expansion has no nonzero affine branches",
        });
    }
    if power == 0 || width == 1 {
        admit_limit("factorized numerator endpoints", 1, limit)?;
        return Ok(1);
    }
    let mut support = 1_u128;
    for index in 1..width {
        let factor = u128::from(power).checked_add(index as u128).ok_or(
            FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "factorized numerator multinomial support",
            },
        )?;
        support = support.checked_mul(factor).ok_or(
            FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "factorized numerator multinomial support",
            },
        )? / index as u128;
        if support > limit as u128 {
            return Err(FactorizedNumeratorLiftError::ResourceLimit {
                resource: "factorized numerator endpoints",
                requested: usize::try_from(support).unwrap_or(usize::MAX),
                limit,
            });
        }
    }
    usize::try_from(support).map_err(|_| FactorizedNumeratorLiftError::ResourceCountOverflow {
        resource: "factorized numerator multinomial support",
    })
}

fn preflight_routed_extrema(
    routed_base: &[i64],
    denominator_positions: &[usize],
    power: u64,
) -> Result<(), FactorizedNumeratorLiftError> {
    for &position in denominator_positions {
        let current = routed_base[position];
        let minimum = i128::from(current) - i128::from(power);
        if minimum < i128::from(i64::MIN) {
            return Err(FactorizedNumeratorLiftError::RoutedPowerShiftUnderflow {
                position,
                power: current,
                decrement: power,
            });
        }
    }
    Ok(())
}

fn route_exponents(
    routed_base: &[i64],
    denominator_positions: &[usize],
    exponents: &[u32],
) -> Result<IntegralKey, FactorizedNumeratorLiftError> {
    if denominator_positions.len() != exponents.len() {
        return Err(FactorizedNumeratorLiftError::Invariant {
            detail: "Symbolica endpoint monomial has the wrong exponent width",
        });
    }
    let mut powers = try_clone_powers(routed_base)?;
    for (&position, &exponent) in denominator_positions.iter().zip(exponents) {
        let decrement = u64::from(exponent);
        powers[position] = checked_lower_power(position, powers[position], decrement)?;
    }
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn route_single_decrement(
    routed_base: &[i64],
    position: usize,
    decrement: u64,
) -> Result<IntegralKey, FactorizedNumeratorLiftError> {
    let mut powers = try_clone_powers(routed_base)?;
    powers[position] = checked_lower_power(position, powers[position], decrement)?;
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn checked_lower_power(
    position: usize,
    power: i64,
    decrement: u64,
) -> Result<i64, FactorizedNumeratorLiftError> {
    let shifted = i128::from(power) - i128::from(decrement);
    i64::try_from(shifted).map_err(
        |_| FactorizedNumeratorLiftError::RoutedPowerShiftUnderflow {
            position,
            power,
            decrement,
        },
    )
}

fn try_clone_powers(powers: &[i64]) -> Result<Vec<i64>, FactorizedNumeratorLiftError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(powers.len()).map_err(|_| {
        FactorizedNumeratorLiftError::AllocationFailure {
            resource: "factorized numerator endpoint powers",
            requested: powers.len(),
        }
    })?;
    cloned.extend_from_slice(powers);
    Ok(cloned)
}

fn try_clone_integral_key(key: &IntegralKey) -> Result<IntegralKey, FactorizedNumeratorLiftError> {
    Ok(IntegralKey::try_from_preallocated(try_clone_powers(
        key.powers(),
    )?)?)
}

fn try_insert_endpoint(
    endpoints: &mut BTreeMap<IntegralKey, Coefficient>,
    key: IntegralKey,
    coefficient: &Coefficient,
    context: &CoefficientContext,
    limits: FactorizedNumeratorLiftExpansionLimits,
    additions: &mut usize,
    coefficient_census: &mut RetainedCoefficientCensus,
) -> Result<(), FactorizedNumeratorLiftError> {
    use std::collections::btree_map::Entry;

    context.validate_with_limits(coefficient, limits.exact_algebra)?;
    if coefficient.is_zero() {
        return Ok(());
    }
    let requested = endpoints.len().checked_add(1).ok_or(
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator coalesced endpoints",
        },
    )?;
    match endpoints.entry(key) {
        Entry::Vacant(entry) => {
            admit_limit(
                "factorized numerator coalesced endpoints",
                requested,
                limits.max_endpoints,
            )?;
            // The source coefficient remains live while its output clone is
            // allocated. Admit its conservative clone-owned bound first,
            // then retain the clone's exact census weight transactionally.
            let clone_bound = retained_coefficient_weight(coefficient)?;
            coefficient_census.admit_additional(clone_bound)?;
            let retained = coefficient.clone();
            let retained_weight = retained_coefficient_weight(&retained)?;
            coefficient_census.replace(RetainedCoefficientWeight::default(), retained_weight)?;
            entry.insert(retained);
        }
        Entry::Occupied(mut entry) => {
            *additions = additions.checked_add(1).ok_or(
                FactorizedNumeratorLiftError::ResourceCountOverflow {
                    resource: "factorized numerator coefficient additions",
                },
            )?;
            admit_limit(
                "factorized numerator coefficient additions",
                *additions,
                limits.max_structural_term_operations,
            )?;
            let old_weight = retained_coefficient_weight(entry.get())?;
            let sum = context.try_add(entry.get(), coefficient, limits.exact_algebra)?;
            let sum_weight = retained_coefficient_weight(&sum)?;
            coefficient_census.admit_additional(sum_weight)?;
            if sum.is_zero() {
                coefficient_census.replace(old_weight, RetainedCoefficientWeight::default())?;
                entry.remove();
            } else {
                coefficient_census.replace(old_weight, sum_weight)?;
                *entry.get_mut() = sum;
            }
        }
    }
    Ok(())
}

fn finish_endpoints(
    endpoints: BTreeMap<IntegralKey, Coefficient>,
) -> Result<Box<[FactorizedNumeratorLiftEndpoint]>, FactorizedNumeratorLiftError> {
    let requested = endpoints.len();
    let mut output = Vec::new();
    output.try_reserve_exact(requested).map_err(|_| {
        FactorizedNumeratorLiftError::AllocationFailure {
            resource: "factorized numerator endpoints",
            requested,
        }
    })?;
    output.extend(
        endpoints
            .into_iter()
            .map(|(key, coefficient)| FactorizedNumeratorLiftEndpoint { key, coefficient }),
    );
    Ok(output.into_boxed_slice())
}

fn preflight_endpoint_key_storage(
    endpoint_count: usize,
    family_arity: usize,
    limits: FactorizedNumeratorLiftExpansionLimits,
) -> Result<(), FactorizedNumeratorLiftError> {
    admit_product_limit(
        "factorized numerator endpoint power entries",
        &[endpoint_count, family_arity],
        limits.max_endpoint_power_entries,
    )?;

    let payload_bytes = endpoint_count
        .checked_mul(family_arity)
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<i64>()));
    let owner_bytes = endpoint_count.checked_mul(std::mem::size_of::<IntegralKey>());
    let retained_bytes = payload_bytes
        .zip(owner_bytes)
        .and_then(|(payload, owners)| payload.checked_add(owners))
        .ok_or(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator retained endpoint key bytes",
            requested: usize::MAX,
            limit: limits.max_retained_endpoint_key_bytes,
        })?;
    admit_limit(
        "factorized numerator retained endpoint key bytes",
        retained_bytes,
        limits.max_retained_endpoint_key_bytes,
    )
}

fn authenticate_family(
    expected_fingerprint: &str,
    family: &IntegralFamily,
) -> Result<(), FactorizedNumeratorLiftError> {
    if family.fingerprint() == expected_fingerprint {
        Ok(())
    } else {
        Err(FactorizedNumeratorLiftError::WrongExpansionFamily)
    }
}

fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FactorizedNumeratorLiftError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn admit_product_limit(
    resource: &'static str,
    factors: &[usize],
    limit: usize,
) -> Result<(), FactorizedNumeratorLiftError> {
    let mut product = 1_usize;
    for &factor in factors {
        let Some(next) = product.checked_mul(factor) else {
            return Err(FactorizedNumeratorLiftError::ResourceLimit {
                resource,
                requested: usize::MAX,
                limit,
            });
        };
        product = next;
        if product > limit {
            return Err(FactorizedNumeratorLiftError::ResourceLimit {
                resource,
                requested: product,
                limit,
            });
        }
    }
    Ok(())
}
