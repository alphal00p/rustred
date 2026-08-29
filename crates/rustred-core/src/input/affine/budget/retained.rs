use std::collections::BTreeSet;
use std::sync::Arc;

use symbolica::prelude::{Integer, PolyVariable};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::CompiledSymbolicaAffineDenominator;
use super::super::work::CoefficientCensus;

pub(in crate::input::affine) fn integer_magnitude_bits(
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

pub(in crate::input::affine) fn signed_i64_magnitude_bits(value: i64) -> usize {
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

pub(in crate::input::affine) fn polynomial_census(
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

pub(in crate::input::affine) fn coefficient_census(
    coefficient: &Coefficient,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = polynomial_census(&coefficient.numerator)?;
    census.checked_add_assign(
        polynomial_census(&coefficient.denominator)?,
        "coefficient census",
    )?;
    Ok(census)
}

pub(in crate::input::affine) fn conservative_owned_capacity_bytes(
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

pub(in crate::input::affine) fn retained_variable_map_arc_bytes<'a>(
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

pub(in crate::input::affine) fn compiled_retained_byte_bound(
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
    // capacity crate-private. Charge a two-times growth policy plus one word;
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

pub(in crate::input::affine) fn multiply_census(
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

pub(in crate::input::affine) fn planned_polynomial_clone_census(
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

pub(in crate::input::affine) fn planned_coefficient_clone_census(
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

pub(in crate::input::affine) fn planned_unit_coefficient_census(
    variables: usize,
) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
    let mut census = super::planned_operation_polynomial_census(1, variables, 1)?;
    census.checked_add_assign(
        super::planned_operation_polynomial_census(1, variables, 1)?,
        "planned unit coefficient census",
    )?;
    Ok(census)
}
