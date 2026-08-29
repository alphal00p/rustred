use symbolica::prelude::Integer;

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::CONSERVATIVE_GMP_CAPACITY_FACTOR;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::normalize::operation_dense_degree_boxes;
use super::super::work::{BinaryOperation, CoefficientCensus, ExactOperationAllocationEnvelope};
use super::retained::{conservative_owned_capacity_bytes, integer_magnitude_bits};

fn polynomial_max_integer_bits(
    polynomial: &CoefficientPolynomial,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |maximum, integer| {
            Ok(maximum.max(integer_magnitude_bits(integer)?))
        })
}

fn ceil_log2_usize(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn product_coefficient_bit_bound(
    left_bits: usize,
    right_bits: usize,
    left_terms: usize,
    right_terms: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if left_terms == 0 || right_terms == 0 {
        return Ok(0);
    }
    left_bits
        .checked_add(right_bits)
        .and_then(|value| value.checked_add(ceil_log2_usize(left_terms.min(right_terms))))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation integer-bit envelope",
        })
}

pub(in crate::input::affine) fn planned_operation_polynomial_census(
    terms: usize,
    variables: usize,
    maximum_integer_bits: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let exponent_entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation exponent entries",
        },
    )?;
    let integer_bits = terms.checked_mul(maximum_integer_bits).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation integer bits",
        },
    )?;
    let rounded_bits = maximum_integer_bits
        .checked_add(usize::BITS as usize - 1)
        .map(|value| value / usize::BITS as usize * usize::BITS as usize)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    // GMP operations may retain spare limbs. Charge one extra limb and a 2x
    // allocator-growth envelope before entering the operation; the exact
    // post-operation census still uses `Integer::Large::capacity()`.
    let conservative_capacity_bits = rounded_bits
        .checked_add(usize::BITS as usize)
        .and_then(|value| value.checked_mul(CONSERVATIVE_GMP_CAPACITY_FACTOR))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    let heap_bytes = terms.checked_mul(conservative_capacity_bits / 8).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        },
    )?;
    let integer_slots = terms.checked_mul(std::mem::size_of::<Integer>()).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        },
    )?;
    let exponent_payload = exponent_entries
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    let integer_capacity = conservative_owned_capacity_bytes(
        integer_slots,
        "projected exact-operation retained bytes",
    )?;
    let exponent_capacity = conservative_owned_capacity_bytes(
        exponent_payload,
        "projected exact-operation retained bytes",
    )?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(integer_capacity)
        .and_then(|value| value.checked_add(exponent_capacity))
        .and_then(|value| value.checked_add(heap_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "projected exact-operation retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms: terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RawOperationPolynomialEnvelope {
    support_terms: usize,
    maximum_integer_bits: usize,
}

pub(in crate::input::affine) fn verify_operation_result_envelope(
    result: &Coefficient,
    actual: CoefficientCensus,
    planned: ExactOperationAllocationEnvelope,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if result.numerator.nterms() > planned.numerator_terms
        || result.denominator.nterms() > planned.denominator_terms
        || actual.polynomial_terms > planned.census.polynomial_terms
        || actual.exponent_entries > planned.census.exponent_entries
        || actual.integer_bits > planned.census.integer_bits
        || actual.retained_bytes > planned.census.retained_bytes
    {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "native exact-operation result exceeded its pre-operation envelope",
            },
        );
    }
    Ok(())
}

fn factor_coefficient_bit_bound(
    raw: RawOperationPolynomialEnvelope,
    dense_degree_box_terms: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if raw.support_terms == 0 {
        return Ok(0);
    }
    // Apply Landau--Mignotte after the injective mixed-radix Kronecker
    // substitution induced by the componentwise degree box. The substituted
    // polynomial has degree at most `dense_degree_box_terms - 1`, unchanged
    // coefficients, and at most `raw.support_terms` nonzero coefficients.
    // Every integral GCD quotient retained by Symbolica is an integral factor
    // of that raw polynomial, so this bounds its coefficient height before the
    // native normalization is entered.
    raw.maximum_integer_bits
        .checked_add(dense_degree_box_terms.saturating_sub(1))
        .and_then(|value| value.checked_add(ceil_log2_usize(raw.support_terms.max(1))))
        .and_then(|value| value.checked_add(2))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized exact-operation factor integer-bit envelope",
        })
}

fn normalization_may_divide(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
) -> bool {
    match operation {
        // If either denominator is one, denominator-GCD reduction and the
        // final numerator/denominator reduction in Symbolica are both units.
        BinaryOperation::Add => !left.denominator.is_one() && !right.denominator.is_one(),
        // Symbolica cross-cancels numerator/denominator pairs before the
        // product. These sufficient unit tests prove that both GCDs are one.
        BinaryOperation::Multiply => {
            !((left.numerator.is_one() || right.denominator.is_one())
                && (left.denominator.is_one() || right.numerator.is_one()))
        }
        // Division is multiplication by the inverse. These are the analogous
        // sufficient unit tests for its two cross-cancellation GCDs.
        BinaryOperation::Divide => {
            !((left.numerator.is_one() || right.numerator.is_one())
                && (left.denominator.is_one() || right.denominator.is_one()))
        }
    }
}

pub(in crate::input::affine) fn exact_operation_allocation_envelope(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
) -> Result<ExactOperationAllocationEnvelope, SymbolicaAffineDenominatorError> {
    let ln = left.numerator.nterms();
    let ld = left.denominator.nterms();
    let rn = right.numerator.nterms();
    let rd = right.denominator.nterms();
    let lnb = polynomial_max_integer_bits(&left.numerator)?;
    let ldb = polynomial_max_integer_bits(&left.denominator)?;
    let rnb = polynomial_max_integer_bits(&right.numerator)?;
    let rdb = polynomial_max_integer_bits(&right.denominator)?;
    let product_terms =
        |left: usize, right: usize| -> Result<usize, SymbolicaAffineDenominatorError> {
            left.checked_mul(right)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation term envelope",
                })
        };
    let (numerator_terms, numerator_bits, denominator_terms, denominator_bits) = match operation {
        BinaryOperation::Add if left.denominator == right.denominator => (
            ln.checked_add(rn)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation term envelope",
                })?,
            lnb.max(rnb).checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "projected exact-operation integer-bit envelope",
                },
            )?,
            ld,
            ldb,
        ),
        BinaryOperation::Add => {
            let left_cross_terms = product_terms(ln, rd)?;
            let right_cross_terms = product_terms(rn, ld)?;
            let left_cross_bits = product_coefficient_bit_bound(lnb, rdb, ln, rd)?;
            let right_cross_bits = product_coefficient_bit_bound(rnb, ldb, rn, ld)?;
            (
                left_cross_terms.checked_add(right_cross_terms).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected exact-operation term envelope",
                    },
                )?,
                left_cross_bits.max(right_cross_bits).checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected exact-operation integer-bit envelope",
                    },
                )?,
                product_terms(ld, rd)?,
                product_coefficient_bit_bound(ldb, rdb, ld, rd)?,
            )
        }
        BinaryOperation::Multiply => (
            product_terms(ln, rn)?,
            product_coefficient_bit_bound(lnb, rnb, ln, rn)?,
            product_terms(ld, rd)?,
            product_coefficient_bit_bound(ldb, rdb, ld, rd)?,
        ),
        BinaryOperation::Divide => (
            product_terms(ln, rd)?,
            product_coefficient_bit_bound(lnb, rdb, ln, rd)?,
            product_terms(ld, rn)?,
            product_coefficient_bit_bound(ldb, rnb, ld, rn)?,
        ),
    };
    let (numerator_box, denominator_box) =
        operation_dense_degree_boxes(left, right, operation, variables)?;
    let normalize = normalization_may_divide(left, right, operation);
    let numerator_raw = RawOperationPolynomialEnvelope {
        support_terms: numerator_terms.min(numerator_box),
        maximum_integer_bits: numerator_bits,
    };
    let denominator_raw = RawOperationPolynomialEnvelope {
        support_terms: denominator_terms.min(denominator_box),
        maximum_integer_bits: denominator_bits,
    };
    let (planned_numerator_terms, planned_numerator_bits) = if normalize {
        (
            if numerator_raw.support_terms == 0 {
                0
            } else {
                numerator_box
            },
            factor_coefficient_bit_bound(numerator_raw, numerator_box)?,
        )
    } else {
        (
            numerator_raw.support_terms,
            numerator_raw.maximum_integer_bits,
        )
    };
    let (planned_denominator_terms, planned_denominator_bits) = if normalize {
        (
            if denominator_raw.support_terms == 0 {
                0
            } else {
                denominator_box
            },
            factor_coefficient_bit_bound(denominator_raw, denominator_box)?,
        )
    } else {
        (
            denominator_raw.support_terms,
            denominator_raw.maximum_integer_bits,
        )
    };
    // The native operation may retain the raw cross-products while it builds
    // normalized GCD quotients, so charge both phases rather than only the
    // larger-looking one. This is a conservative logical-result/retained
    // allocation envelope; Symbolica's internal multivariate-GCD workspace is
    // not exposed by its API and is governed separately by the dense-box work
    // limits above.
    let mut census = planned_operation_polynomial_census(
        numerator_raw.support_terms,
        variables,
        numerator_raw.maximum_integer_bits,
    )?;
    census.checked_add_assign(
        planned_operation_polynomial_census(
            denominator_raw.support_terms,
            variables,
            denominator_raw.maximum_integer_bits,
        )?,
        "raw exact-operation allocation envelope",
    )?;
    if normalize {
        census.checked_add_assign(
            planned_operation_polynomial_census(
                planned_numerator_terms,
                variables,
                planned_numerator_bits,
            )?,
            "normalized exact-operation allocation envelope",
        )?;
        census.checked_add_assign(
            planned_operation_polynomial_census(
                planned_denominator_terms,
                variables,
                planned_denominator_bits,
            )?,
            "normalized exact-operation allocation envelope",
        )?;
    }
    Ok(ExactOperationAllocationEnvelope {
        census,
        numerator_terms: planned_numerator_terms,
        denominator_terms: planned_denominator_terms,
    })
}
