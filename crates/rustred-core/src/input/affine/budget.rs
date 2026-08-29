use std::collections::BTreeSet;
use std::sync::Arc;

use symbolica::prelude::{Integer, PolyVariable};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::CONSERVATIVE_GMP_CAPACITY_FACTOR;
use super::error::SymbolicaAffineDenominatorError;
use super::model::CompiledSymbolicaAffineDenominator;
use super::normalize::operation_dense_degree_boxes;
use super::work::{BinaryOperation, CoefficientCensus, ExactOperationAllocationEnvelope};

pub(super) fn integer_magnitude_bits(
    integer: &Integer,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    let bits = match integer {
        Integer::Single(value) => u64::from(u64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(u128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
        resource: "integer magnitude bits",
    })
}

pub(super) fn signed_i64_magnitude_bits(value: i64) -> usize {
    (u64::BITS - value.unsigned_abs().leading_zeros()) as usize
}

fn integer_owned_heap_bytes(integer: &Integer) -> Result<usize, SymbolicaAffineDenominatorError> {
    match integer {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => usize::try_from(value.capacity())
            .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "integer owned heap bytes",
            })?
            .checked_add(7)
            .map(|bits| bits / 8)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "integer owned heap bytes",
            }),
    }
}

pub(super) fn polynomial_census(
    polynomial: &CoefficientPolynomial,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let polynomial_terms = polynomial.nterms();
    let exponent_entries = polynomial_terms
        .checked_mul(polynomial.variables.len())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial census exponent entries",
        })?;
    if polynomial.exponents.len() != exponent_entries
        || polynomial.coefficients.len() != polynomial_terms
    {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "polynomial census found a malformed storage layout",
            },
        );
    }
    let integer_bits = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_magnitude_bits(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "polynomial census integer bits",
                },
            )
        })?;
    let integer_slots = polynomial
        .coefficients
        .capacity()
        .checked_mul(std::mem::size_of::<Integer>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    let exponent_bytes = polynomial
        .exponents
        .capacity()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    let limb_bytes = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_owned_heap_bytes(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "polynomial retained bytes",
                },
            )
        })?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(integer_slots)
        .and_then(|value| value.checked_add(exponent_bytes))
        .and_then(|value| value.checked_add(limb_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "polynomial retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

pub(super) fn coefficient_census(
    coefficient: &Coefficient,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = polynomial_census(&coefficient.numerator)?;
    census.checked_add_assign(
        polynomial_census(&coefficient.denominator)?,
        "coefficient census",
    )?;
    Ok(census)
}

fn conservative_owned_capacity_bytes(
    payload_bytes: usize,
    resource: &'static str,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if payload_bytes == 0 {
        return Ok(0);
    }
    payload_bytes
        .checked_add(std::mem::size_of::<usize>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })
}

pub(super) fn retained_variable_map_arc_bytes<'a>(
    coefficients: impl IntoIterator<Item = &'a Coefficient>,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    let mut distinct = BTreeSet::new();
    let mut bytes = 0usize;
    for coefficient in coefficients {
        for polynomial in [&coefficient.numerator, &coefficient.denominator] {
            let identity = Arc::as_ptr(&polynomial.variables) as usize;
            if distinct.insert(identity) {
                let arc_header = std::mem::size_of::<usize>().checked_mul(2).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    },
                )?;
                let variable_payload = polynomial
                    .variables
                    .capacity()
                    .checked_mul(std::mem::size_of::<PolyVariable>())
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    })?;
                let allocation = arc_header
                    .checked_add(std::mem::size_of::<Vec<PolyVariable>>())
                    .and_then(|value| value.checked_add(variable_payload))
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    })?;
                bytes = bytes.checked_add(allocation).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "compiled retained variable-map Arc bytes",
                    },
                )?;
            }
        }
    }
    Ok(bytes)
}

pub(super) fn compiled_retained_byte_bound(
    source_bytes: usize,
    normalized_expression_bytes: usize,
    projected_coefficient_bytes: usize,
    variable_map_arc_bytes: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    // The inline top-level structure owns both Atom handles and the affine row.
    // The additions below therefore charge only backing buffers and nested
    // coefficient allocations.
    let mut bytes = std::mem::size_of::<CompiledSymbolicaAffineDenominator>();
    let atom_payload = source_bytes
        .checked_add(normalized_expression_bytes)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained Atom bytes",
        })?;
    // Symbolica exposes an Atom's logical byte size but keeps RawAtom backing
    // capacity crate-private.  Charge a two-times growth policy plus one word;
    // this is intentionally a conservative retained-payload estimate, not an
    // exact observation of the private allocator capacity.
    bytes = bytes
        .checked_add(conservative_owned_capacity_bytes(
            atom_payload,
            "compiled retained Atom bytes",
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained bytes",
        })?;

    bytes = bytes
        .checked_add(conservative_owned_capacity_bytes(
            projected_coefficient_bytes,
            "compiled retained affine-coefficient bytes",
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained bytes",
        })?;
    bytes = bytes.checked_add(variable_map_arc_bytes).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "compiled retained variable-map Arc bytes",
        },
    )?;
    Ok(bytes)
}

pub(super) fn multiply_census(
    census: CoefficientCensus,
    count: usize,
    resource: &'static str,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    Ok(CoefficientCensus {
        polynomial_terms: census
            .polynomial_terms
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        exponent_entries: census
            .exponent_entries
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        integer_bits: census
            .integer_bits
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
        retained_bytes: census
            .retained_bytes
            .checked_mul(count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })?,
    })
}

pub(super) fn planned_polynomial_clone_census(
    polynomial: &CoefficientPolynomial,
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let terms = polynomial.nterms();
    let exponent_entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "planned polynomial clone exponent entries",
        },
    )?;
    let integer_bits = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_magnitude_bits(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "planned polynomial clone integer bits",
                },
            )
        })?;
    let limb_bytes = polynomial
        .coefficients
        .iter()
        .try_fold(0usize, |total, integer| {
            total.checked_add(integer_owned_heap_bytes(integer)?).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "planned polynomial clone retained bytes",
                },
            )
        })?;
    let retained_bytes = std::mem::size_of::<CoefficientPolynomial>()
        .checked_add(terms.checked_mul(std::mem::size_of::<Integer>()).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "planned polynomial clone retained bytes",
            },
        )?)
        .and_then(|value| {
            exponent_entries
                .checked_mul(std::mem::size_of::<u16>())
                .and_then(|bytes| value.checked_add(bytes))
        })
        .and_then(|value| value.checked_add(limb_bytes))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "planned polynomial clone retained bytes",
        })?;
    Ok(CoefficientCensus {
        polynomial_terms: terms,
        exponent_entries,
        integer_bits,
        retained_bytes,
    })
}

pub(super) fn planned_coefficient_clone_census(
    coefficient: &Coefficient,
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = planned_polynomial_clone_census(&coefficient.numerator, variables)?;
    census.checked_add_assign(
        planned_polynomial_clone_census(&coefficient.denominator, variables)?,
        "planned coefficient clone census",
    )?;
    Ok(census)
}

pub(super) fn planned_unit_coefficient_census(
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = planned_operation_polynomial_census(1, variables, 1)?;
    census.checked_add_assign(
        planned_operation_polynomial_census(1, variables, 1)?,
        "planned unit coefficient census",
    )?;
    Ok(census)
}

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

pub(super) fn planned_operation_polynomial_census(
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
    // GMP operations may retain spare limbs.  Charge one extra limb and a 2x
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

pub(super) fn verify_operation_result_envelope(
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
    // substitution induced by the componentwise degree box.  The substituted
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
        // product.  These sufficient unit tests prove that both GCDs are one.
        BinaryOperation::Multiply => {
            !((left.numerator.is_one() || right.denominator.is_one())
                && (left.denominator.is_one() || right.numerator.is_one()))
        }
        // Division is multiplication by the inverse.  These are the analogous
        // sufficient unit tests for its two cross-cancellation GCDs.
        BinaryOperation::Divide => {
            !((left.numerator.is_one() || right.numerator.is_one())
                && (left.denominator.is_one() || right.denominator.is_one()))
        }
    }
}

pub(super) fn exact_operation_allocation_envelope(
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
    // larger-looking one.  This is a conservative logical-result/retained
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
