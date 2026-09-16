//! Symbolica-backed semi-numerical target lifting.
//!
//! SpIReD discovers a dependency over a finite field and reconstructs the
//! coefficient functions from black-box evaluations.  This module keeps that
//! separation explicit: every callback runs the native finite-field GPLU on
//! the unchanged selected source trace, while Symbolica owns interpolation,
//! CRT/rational lifting and verification.  No interpolation or reconstruction
//! arithmetic is implemented in RustRed.

use std::collections::HashMap;
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, SelfRing};
use symbolica::poly::reconstruction::{
    ReconstructionMethod, ReconstructionOptions, reconstruct_rational_function_over_q,
};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::variables::FrameVariables;
use super::{ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term};

type Fp = FiniteFieldElement<u64>;
type CacheKey = (u64, Vec<u64>);

pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    max_degree: u16,
    max_probes: usize,
    max_attempts: usize,
    max_primes: usize,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    if rows.is_empty() || columns.is_empty() || target_column >= columns.len() {
        return Err(MaterializationError::TargetAbsent);
    }
    // A constant frame is already cheap in the exact sparse backend and cannot
    // be passed to Symbolica's multivariate reconstruction API.
    if variables.active_len() == 0 {
        return Err(MaterializationError::SemiNumericalReconstruction(
            "the semi-numerical route requires at least one coefficient variable".into(),
        ));
    }
    let reconstruction_variables = variables.active_variables();
    let options = ReconstructionOptions {
        max_degree,
        max_probes,
        max_attempts,
        verification_points: 3,
        ..Default::default()
    };
    if max_primes < 2 {
        return Err(MaterializationError::SemiNumericalReconstruction(
            "max_primes must be at least two for an independent verification image".into(),
        ));
    }

    observe(MaterializationEvent::SemiNumericalStarted {
        rows: rows.len(),
        columns: columns.len(),
        variables: reconstruction_variables.len(),
    });

    // All target-row evaluations at one point are shared by every coefficient
    // reconstruction. This is essential: the Symbolica API is scalar-valued,
    // while one GPLU evaluation yields the complete sparse row.
    let mut cache: HashMap<CacheKey, Option<Vec<(u32, Fp)>>> = HashMap::new();
    let mut output = Vec::new();
    for (column, integral) in columns.iter().enumerate() {
        let mut black_box = |field: &Zp64, point: &[Fp]| {
            let key = (
                field.get_prime(),
                point.iter().map(|value| *value.inner()).collect::<Vec<_>>(),
            );
            let row = cache.entry(key).or_insert_with(|| {
                target_row_at_point(rows, columns, order, target_column, variables, field, point)
            });
            row.as_ref().map(|terms| {
                terms
                    .iter()
                    .find(|(id, _)| *id as usize == column)
                    .map_or_else(|| field.to_element(0), |(_, value)| *value)
            })
        };
        let (reconstructed, stats) = reconstruct_rational_function_over_q(
            reconstruction_variables.clone(),
            &mut black_box,
            ReconstructionMethod::Automatic,
            &options,
            max_primes,
        )
        .map_err(|error| {
            reconstruction_error(
                error,
                column,
                target_column,
                rows.len(),
                columns.len(),
                reconstruction_variables.len(),
            )
        })?;
        observe(MaterializationEvent::SemiNumericalCoefficient {
            column,
            probes: stats.probes,
            primes: stats.primes,
        });
        let restored = variables
            .restore_coefficient(&reconstructed)
            .map_err(|error| {
                MaterializationError::SemiNumericalReconstruction(error.to_string())
            })?;
        if !restored.is_zero() {
            output.push(Term {
                integral: *integral,
                coefficient: restored,
            });
        }
    }
    // The black-box row is monic by construction. Keep the same target-column
    // and ordering contract as exact GPLU before ordinary canonicalization.
    if output
        .iter()
        .find(|term| term.integral == columns[target_column])
        .is_none_or(|term| !term.coefficient.is_one())
    {
        return Err(MaterializationError::SemiNumericalReconstruction(
            "reconstructed target coefficient is not one".into(),
        ));
    }
    observe(MaterializationEvent::SemiNumericalFinished {
        output_terms: output.len(),
    });
    Ok(output)
}

/// Add bounded structural context only after the native reconstruction fails.
/// No coefficient formatting, additional probe or retry happens on this path.
fn reconstruction_error(
    cause: impl std::fmt::Display,
    column: usize,
    target_column: usize,
    rows: usize,
    columns: usize,
    active_variables: usize,
) -> MaterializationError {
    MaterializationError::SemiNumericalReconstruction(format!(
        "coefficient column {column} (zero-based; target column {target_column}; frame rows={rows}, columns={columns}, active variables={active_variables}): {cause}"
    ))
}

fn target_row_at_point<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    field: &Zp64,
    point: &[Fp],
) -> Option<Vec<(u32, Fp)>> {
    let mut reducer = SparseRowReducer::new(
        u32::try_from(columns.len()).ok()?.checked_add(1)?,
        field.clone(),
        LuLMode::None,
    );
    let mut values = Vec::new();
    let mut ids = Vec::new();
    for row in rows {
        values.clear();
        ids.clear();
        for term in row {
            let column = columns
                .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                .ok()?;
            let coefficient = variables.map_coefficient(&term.coefficient).ok()?;
            let numerator = coefficient.numerator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            let denominator = coefficient.denominator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            if field.is_zero(&denominator) {
                return None;
            }
            let value = field.div(&numerator, &denominator);
            if !field.is_zero(&value) {
                ids.push(column as u32);
                values.push(value);
            }
        }
        if let Some(pivot) = reducer.add_row(&values, &ids) {
            if pivot == target_column as u32 {
                let upper = reducer.u();
                let row = upper.nrows() as usize - 1;
                let start = upper.row_ptrs()[row];
                let end = upper.row_ptrs()[row + 1];
                return Some(
                    upper.col_idcs()[start..end]
                        .iter()
                        .copied()
                        .zip(upper.values()[start..end].iter().copied())
                        .collect(),
                );
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::{CoefficientVariableOrder, IntegralOrder};
    use super::*;
    use crate::algebra::CoefficientContext;

    fn integral(shift: i16) -> Integral<1> {
        Integral::symbolic([shift]).unwrap()
    }

    #[test]
    fn native_reconstruction_matches_exact_sparse_target_row() {
        let context = CoefficientContext::new(["a", "b"]);
        let rows = vec![
            vec![
                Term {
                    integral: integral(3),
                    coefficient: context.coefficient_fixture("a"),
                },
                Term {
                    integral: integral(2),
                    coefficient: context.coefficient_fixture("1"),
                },
            ],
            vec![
                Term {
                    integral: integral(3),
                    coefficient: context.coefficient_fixture("2*a"),
                },
                Term {
                    integral: integral(1),
                    coefficient: context.coefficient_fixture("b"),
                },
            ],
        ];
        let order = IntegralOrder::new([true], [false]);
        let mut columns = vec![integral(3), integral(2), integral(1)];
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        let actual = materialize(
            &rows,
            &columns,
            &order,
            0,
            &variables,
            4,
            2000,
            3,
            4,
            |_| {},
        )
        .unwrap();
        assert_eq!(actual[0].integral, integral(3));
        assert!(actual[0].coefficient.is_one());
        assert!(actual.iter().any(|term| term.integral == integral(2)));
    }

    #[test]
    fn failure_context_preserves_native_prime_budget_cause() {
        let cause = symbolica::poly::reconstruction::ReconstructionError::PrimeLimit;
        let expected_cause = cause.to_string();
        let error = reconstruction_error(cause, 7, 3, 11, 19, 4);
        let MaterializationError::SemiNumericalReconstruction(detail) = error else {
            panic!("native reconstruction failure changed its error variant");
        };
        assert_eq!(
            detail,
            format!(
                "coefficient column 7 (zero-based; target column 3; frame rows=11, columns=19, active variables=4): {expected_cause}"
            )
        );
    }

    #[test]
    fn native_invalid_options_gain_context_without_success_events_or_retry() {
        let context = CoefficientContext::new(["a"]);
        let rows = vec![vec![Term {
            integral: integral(1),
            coefficient: context.coefficient_fixture("a"),
        }]];
        let columns = vec![integral(1)];
        let order = IntegralOrder::new([true], [false]);
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        let mut events = Vec::new();
        // A zero degree bound is rejected by Symbolica before probing. This
        // exercises the actual error boundary without changing native policy.
        let error = materialize(
            &rows,
            &columns,
            &order,
            0,
            &variables,
            0,
            10,
            1,
            2,
            |event| events.push(event),
        )
        .unwrap_err();
        let MaterializationError::SemiNumericalReconstruction(detail) = error else {
            panic!("native reconstruction failure changed its error variant");
        };
        assert_eq!(
            detail,
            format!(
                "coefficient column 0 (zero-based; target column 0; frame rows=1, columns=1, active variables=1): {}",
                symbolica::poly::reconstruction::ReconstructionError::InvalidOptions
            )
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            MaterializationEvent::SemiNumericalStarted {
                rows: 1,
                columns: 1,
                variables: 1,
            }
        ));
    }
}
