use std::mem::size_of;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::{
    IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::identity::ParametricRelation;

use super::super::error::{check_limit, checked_add, checked_mul};
use super::super::{InvolutiveError, InvolutiveLimits};
use super::OrdinaryChartLiftLimits;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SymbolicCensus {
    pub(super) terms: usize,
    pub(super) exponent_cells: usize,
    pub(super) retained_bytes: usize,
    pub(super) max_integer_bits: usize,
}

impl SymbolicCensus {
    pub(super) fn try_add(
        self,
        right: Self,
        resource: &'static str,
    ) -> Result<Self, InvolutiveError> {
        Ok(Self {
            terms: checked_add(resource, self.terms, right.terms)?,
            exponent_cells: checked_add(resource, self.exponent_cells, right.exponent_cells)?,
            retained_bytes: checked_add(resource, self.retained_bytes, right.retained_bytes)?,
            max_integer_bits: self.max_integer_bits.max(right.max_integer_bits),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RelationInputCensus {
    pub(super) guards: SymbolicCensus,
    pub(super) all: SymbolicCensus,
}

pub(super) fn preflight_relation_symbolic_limits(
    relation: &ParametricRelation,
    census: RelationInputCensus,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    check_limit(
        "Ore AXPY input terms",
        relation.terms().len(),
        limits.max_axpy_input_terms,
    )?;
    check_limit(
        "Ore localization guards",
        relation.nonzero_conditions().len(),
        limits.max_localization_guards,
    )?;
    check_limit(
        "Ore localization guard terms",
        census.guards.terms,
        limits.max_localization_guard_terms,
    )?;
    check_limit(
        "Ore localization guard exponent cells",
        census.guards.exponent_cells,
        limits.max_localization_guard_exponent_cells,
    )?;
    check_limit(
        "Ore localization guard retained bytes",
        census.guards.retained_bytes,
        limits.max_localization_guard_retained_bytes,
    )?;
    check_limit(
        "ordinary chart-lift input integer bits",
        census.all.max_integer_bits,
        limits.indexed_algebra.max_specialization_integer_bits,
    )
}

pub(super) fn preflight_batch_symbolic_limits(
    guards: SymbolicCensus,
    all: SymbolicCensus,
    limits: OrdinaryChartLiftLimits,
) -> Result<(), InvolutiveError> {
    check_limit(
        "ordinary chart-lift input guard terms",
        guards.terms,
        limits.max_input_guard_terms,
    )?;
    check_limit(
        "ordinary chart-lift input guard exponent cells",
        guards.exponent_cells,
        limits.max_input_guard_exponent_cells,
    )?;
    check_limit(
        "ordinary chart-lift input guard retained bytes",
        guards.retained_bytes,
        limits.max_input_guard_retained_bytes,
    )?;
    check_limit(
        "ordinary chart-lift input symbolic terms",
        all.terms,
        limits.max_input_symbolic_terms,
    )?;
    check_limit(
        "ordinary chart-lift input symbolic exponent cells",
        all.exponent_cells,
        limits.max_input_symbolic_exponent_cells,
    )?;
    check_limit(
        "ordinary chart-lift input symbolic retained bytes",
        all.retained_bytes,
        limits.max_input_symbolic_retained_bytes,
    )
}

/// Authenticate every retained coefficient part and guard under the nested
/// exact limits while computing the batch-admission payload census.
pub(super) fn authenticate_input_symbolic_census(
    relation: &ParametricRelation,
    context: &IndexedCoefficientContext,
    limits: IndexedAlgebraLimits,
) -> Result<RelationInputCensus, InvolutiveError> {
    let mut coefficient_census = SymbolicCensus::default();
    for coefficient in relation.terms().values() {
        context.validate_with_limits(coefficient, limits.exact_algebra)?;
        coefficient_census = coefficient_census.try_add(
            coefficient_symbolic_census(coefficient)?,
            "ordinary chart-lift input coefficient payload",
        )?;
    }
    let mut guard_census = SymbolicCensus::default();
    for condition in relation.nonzero_conditions() {
        context.validate_polynomial_with_limits(condition.polynomial(), limits.exact_algebra)?;
        guard_census = guard_census.try_add(
            polynomial_symbolic_census(
                condition.polynomial(),
                checked_add(
                    "ordinary chart-lift input guard retained bytes",
                    size_of::<IndexedPolynomial>(),
                    size_of::<Arc<IndexedPolynomial>>(),
                )?,
                "ordinary chart-lift input guard retained bytes",
            )?,
            "ordinary chart-lift input guard payload",
        )?;
    }
    Ok(RelationInputCensus {
        guards: guard_census,
        all: coefficient_census
            .try_add(guard_census, "ordinary chart-lift input symbolic payload")?,
    })
}

fn coefficient_symbolic_census(
    coefficient: &IndexedCoefficient,
) -> Result<SymbolicCensus, InvolutiveError> {
    let raw = coefficient.raw();
    let numerator = raw_polynomial_symbolic_census(
        &raw.numerator,
        size_of::<IndexedCoefficient>(),
        "ordinary chart-lift input coefficient retained bytes",
    )?;
    let denominator = raw_polynomial_symbolic_census(
        &raw.denominator,
        0,
        "ordinary chart-lift input coefficient retained bytes",
    )?;
    numerator.try_add(denominator, "ordinary chart-lift input coefficient payload")
}

fn polynomial_symbolic_census(
    polynomial: &IndexedPolynomial,
    wrapper_bytes: usize,
    resource: &'static str,
) -> Result<SymbolicCensus, InvolutiveError> {
    raw_polynomial_symbolic_census(polynomial.raw(), wrapper_bytes, resource)
}

fn raw_polynomial_symbolic_census(
    polynomial: &crate::algebra::CoefficientPolynomial,
    wrapper_bytes: usize,
    resource: &'static str,
) -> Result<SymbolicCensus, InvolutiveError> {
    let terms = polynomial.coefficients.len();
    let exponent_cells = polynomial.exponents.len();
    let coefficient_slots = checked_mul(resource, terms, size_of::<Integer>())?;
    let exponent_bytes = checked_mul(resource, exponent_cells, size_of::<u16>())?;
    let mut retained_bytes = checked_add(
        resource,
        wrapper_bytes,
        checked_add(resource, coefficient_slots, exponent_bytes)?,
    )?;
    let mut max_integer_bits = 0usize;
    for coefficient in &polynomial.coefficients {
        let integer_bits = integer_magnitude_bits(coefficient, resource)?;
        max_integer_bits = max_integer_bits.max(integer_bits);
        let large_bits = match coefficient {
            Integer::Large(_) => integer_bits,
            Integer::Single(_) | Integer::Double(_) => 0,
        };
        let large_bytes = checked_add(resource, large_bits, 7)? / 8;
        retained_bytes = checked_add(resource, retained_bytes, large_bytes)?;
    }
    Ok(SymbolicCensus {
        terms,
        exponent_cells,
        retained_bytes,
        max_integer_bits,
    })
}

fn integer_magnitude_bits(
    value: &Integer,
    resource: &'static str,
) -> Result<usize, InvolutiveError> {
    let bits = match value {
        Integer::Single(value) => u64::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| InvolutiveError::ResourceCountOverflow { resource })
}
