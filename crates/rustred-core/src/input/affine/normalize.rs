use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::budget::integer_magnitude_bits;
use super::construction::check_limit;
use super::error::SymbolicaAffineDenominatorError;
use super::limits::SymbolicaAffineDenominatorLimits;
use super::work::{BinaryOperation, ExactWorkBudget, NormalizedExpressionCensus};

fn polynomial_expression_census(
    polynomial: &CoefficientPolynomial,
) -> Result<NormalizedExpressionCensus, SymbolicaAffineDenominatorError> {
    if polynomial.is_zero() {
        return Ok(NormalizedExpressionCensus {
            nodes: 1,
            integer_bits: 0,
        });
    }
    let mut census = NormalizedExpressionCensus::default();
    for (integer, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        let mut term_nodes = 1usize; // retained integer coefficient
        let mut factors = 1usize;
        census.integer_bits = census
            .integer_bits
            .checked_add(integer_magnitude_bits(integer)?)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression integer bits",
            })?;
        for exponent in exponents.iter().copied().filter(|exponent| *exponent != 0) {
            factors = factors.checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "normalized expression nodes",
                },
            )?;
            if exponent == 1 {
                term_nodes = term_nodes.checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression nodes",
                    },
                )?;
            } else {
                // Power node, variable, and exact integer exponent.
                term_nodes = term_nodes.checked_add(3).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression nodes",
                    },
                )?;
                census.integer_bits = census
                    .integer_bits
                    .checked_add(
                        usize::try_from(u16::BITS - exponent.leading_zeros()).map_err(|_| {
                            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "normalized expression integer bits",
                            }
                        })?,
                    )
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "normalized expression integer bits",
                    })?;
            }
        }
        if factors > 1 {
            term_nodes = term_nodes.checked_add(1).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "normalized expression nodes",
                },
            )?;
        }
        census.nodes = census.nodes.checked_add(term_nodes).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            },
        )?;
    }
    if polynomial.nterms() > 1 {
        census.nodes = census.nodes.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            },
        )?;
    }
    Ok(census)
}

pub(super) fn normalized_expression_census(
    coefficient: &Coefficient,
) -> Result<NormalizedExpressionCensus, SymbolicaAffineDenominatorError> {
    let mut census = polynomial_expression_census(&coefficient.numerator)?;
    if !coefficient.denominator.is_one() {
        let denominator = polynomial_expression_census(&coefficient.denominator)?;
        census.nodes = census
            .nodes
            .checked_add(denominator.nodes)
            .and_then(|value| value.checked_add(3))
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression nodes",
            })?;
        census.integer_bits = census
            .integer_bits
            .checked_add(denominator.integer_bits)
            .and_then(|value| value.checked_add(1))
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "normalized expression integer bits",
            })?;
    }
    Ok(census)
}

pub(super) fn normalized_expression_render_byte_bound(
    census: NormalizedExpressionCensus,
    maximum_symbol_bytes: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    let bytes_per_node = maximum_symbol_bytes.checked_add(8).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized expression render bytes",
        },
    )?;
    census
        .nodes
        .checked_mul(bytes_per_node)
        .and_then(|value| value.checked_add(census.integer_bits))
        .and_then(|value| value.checked_add(16))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "normalized expression render bytes",
        })
}

fn polynomial_degrees(
    polynomial: &CoefficientPolynomial,
    expected_variables: usize,
) -> Result<Vec<u16>, SymbolicaAffineDenominatorError> {
    if polynomial.variables.len() != expected_variables {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "degree census found a polynomial on the wrong variable map",
            },
        );
    }
    let mut degrees = Vec::new();
    degrees.try_reserve_exact(expected_variables).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "componentwise degree census",
            requested: expected_variables,
        }
    })?;
    for variable in 0..expected_variables {
        degrees.push(polynomial.degree(variable));
    }
    Ok(degrees)
}

pub(super) fn operation_dense_degree_boxes(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let ln = polynomial_degrees(&left.numerator, variables)?;
    let ld = polynomial_degrees(&left.denominator, variables)?;
    let rn = polynomial_degrees(&right.numerator, variables)?;
    let rd = polynomial_degrees(&right.denominator, variables)?;
    let same_denominator = left.denominator == right.denominator;
    let mut numerator_box = 1usize;
    let mut denominator_box = 1usize;
    for variable in 0..variables {
        let sum = |left: u16,
                   right: u16,
                   resource: &'static str|
         -> Result<u32, SymbolicaAffineDenominatorError> {
            u32::from(left)
                .checked_add(u32::from(right))
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow { resource })
        };
        let (numerator_degree, denominator_degree) = match operation {
            BinaryOperation::Add if same_denominator => (
                u32::from(ln[variable].max(rn[variable])),
                u32::from(ld[variable]),
            ),
            BinaryOperation::Add => (
                sum(ln[variable], rd[variable], "addition numerator degree")?.max(sum(
                    rn[variable],
                    ld[variable],
                    "addition numerator degree",
                )?),
                sum(ld[variable], rd[variable], "addition denominator degree")?,
            ),
            BinaryOperation::Multiply => (
                sum(
                    ln[variable],
                    rn[variable],
                    "multiplication numerator degree",
                )?,
                sum(
                    ld[variable],
                    rd[variable],
                    "multiplication denominator degree",
                )?,
            ),
            BinaryOperation::Divide => (
                sum(ln[variable], rd[variable], "division numerator degree")?,
                sum(ld[variable], rn[variable], "division denominator degree")?,
            ),
        };
        let numerator_width = usize::try_from(numerator_degree.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense numerator degree box",
            },
        )?)
        .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense numerator degree box",
        })?;
        numerator_box = numerator_box.checked_mul(numerator_width).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense numerator degree box",
            },
        )?;
        let denominator_width = usize::try_from(denominator_degree.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense denominator degree box",
            },
        )?)
        .map_err(|_| SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense denominator degree box",
        })?;
        denominator_box = denominator_box.checked_mul(denominator_width).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "dense denominator degree box",
            },
        )?;
    }
    Ok((numerator_box, denominator_box))
}

pub(super) fn charge_dense_degree_box(
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
    variables: usize,
    limits: SymbolicaAffineDenominatorLimits,
    work: &mut ExactWorkBudget,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let (numerator_box, denominator_box) =
        operation_dense_degree_boxes(left, right, operation, variables)?;
    check_limit(
        "dense numerator degree-box terms",
        numerator_box,
        limits.max_dense_degree_box_terms,
    )?;
    check_limit(
        "dense denominator degree-box terms",
        denominator_box,
        limits.max_dense_degree_box_terms,
    )?;
    let terms = numerator_box.checked_add(denominator_box).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense degree-box terms",
        },
    )?;
    let entries = terms.checked_mul(variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "dense degree-box exponent entries",
        },
    )?;
    check_limit(
        "dense degree-box exponent entries",
        entries,
        limits.max_dense_degree_box_exponent_entries,
    )?;
    work.dense_degree_box_terms = work.dense_degree_box_terms.checked_add(terms).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "aggregate dense degree-box terms",
        },
    )?;
    check_limit(
        "aggregate dense degree-box terms",
        work.dense_degree_box_terms,
        limits.max_aggregate_dense_degree_box_terms,
    )?;
    work.dense_degree_box_exponent_entries = work
        .dense_degree_box_exponent_entries
        .checked_add(entries)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "aggregate dense degree-box exponent entries",
        })?;
    check_limit(
        "aggregate dense degree-box exponent entries",
        work.dense_degree_box_exponent_entries,
        limits.max_aggregate_dense_degree_box_exponent_entries,
    )?;
    Ok((numerator_box, denominator_box))
}
