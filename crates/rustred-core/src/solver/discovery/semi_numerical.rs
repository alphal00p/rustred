//! Symbolica-backed semi-numerical target lifting.
//!
//! SpIReD discovers a dependency over a finite field and reconstructs the
//! coefficient functions from black-box evaluations.  This module keeps that
//! separation explicit: every callback runs the native finite-field GPLU on
//! the unchanged selected source trace, while Symbolica owns interpolation,
//! CRT/rational lifting and verification.  No interpolation or reconstruction
//! arithmetic is implemented in RustRed.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::SelfRing;
#[cfg(test)]
use symbolica::domains::finite_field::ToFiniteField;
use symbolica::domains::finite_field::{
    FiniteFieldCore, FiniteFieldElement, PrimeIteratorU64, Zp64,
};
#[cfg(test)]
use symbolica::domains::{Field, Ring};
use symbolica::poly::reconstruction::{
    ReconstructionMethod, ReconstructionOptions, reconstruct_rational_function_over_q,
};
#[cfg(test)]
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::variables::FrameVariables;
use super::{ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term};

mod frame;
use frame::ProbeFrame;

type Fp = FiniteFieldElement<u64>;
type CacheKey = (u64, Vec<u64>);

/// A finite-field image of the selected GPLU prefix.  The pivot trace is part
/// of the image because the row returned by forward elimination is a rational
/// function only while the same generic pivot branch is used.  Specializing
/// the source trace at a zero of a pivot minor can otherwise make the
/// black-box oracle piecewise, which is not a valid input to Symbolica's
/// rational-function reconstructor.
#[derive(Clone, Debug, PartialEq)]
struct TargetImage {
    row: Vec<(u32, Fp)>,
    // One outcome per original source row, including dependent inputs. Pivot
    // columns alone do not identify which sources supplied the accepted rows.
    pivots: Vec<Option<u32>>,
}

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
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    materialize_with_support(
        rows,
        columns,
        order,
        target_column,
        variables,
        max_degree,
        max_probes,
        max_attempts,
        max_primes,
        None,
        observe,
    )
}

/// Run reconstruction, optionally using an exact support set discovered by a
/// failed sampled pass.  The support retry is deliberately bounded to one
/// attempt and remains subject to Symbolica reconstruction and final exact
/// replay; it is not an exact-coefficient shortcut.
fn materialize_with_support<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    max_degree: u16,
    max_probes: usize,
    max_attempts: usize,
    max_primes: usize,
    support_override: Option<Vec<usize>>,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    let support_retry = support_override.is_some();
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

    // Column lookup and native variable remapping depend only on the selected
    // source trace, not on a prime or probe point. Prepare them once.
    let frame = ProbeFrame::new(rows, columns, order, variables)?;

    // All target-row evaluations at one point are shared by every coefficient
    // reconstruction. This is essential: the Symbolica API is scalar-valued,
    // while one GPLU evaluation yields the complete sparse row.
    let mut cache: HashMap<CacheKey, Option<TargetImage>> = HashMap::new();
    // The sampled majority (or first usable image during an exact-support
    // retry) proposes a GPLU branch. Reject other branches as unusable probes.
    // Exact replay must still authenticate the proposed branch and support.
    let mut generic_pivots: Option<Vec<Option<u32>>> = None;
    // Propose sparse support from independent finite-field images. This can
    // miss a nonzero coefficient and is never an authority gate. Native
    // reconstruction of an absent column may exhaust its attempt budget;
    // avoid those calls where possible and check the full exact row below.
    let (candidate_columns, sampled_pivots) = if let Some(support) = support_override {
        (support, Vec::new())
    } else {
        sampled_frame_support(&frame, target_column, variables.active_len())
            .unwrap_or_else(|| ((0..columns.len()).collect(), Vec::new()))
    };
    if !sampled_pivots.is_empty() {
        generic_pivots = Some(sampled_pivots);
    }
    let mut output = Vec::new();
    // A finite-field pivot can expose a column which is structurally zero in
    // characteristic zero (for example when a modular denominator vanishes
    // on one of the pivot paths).  Symbolica's reconstructor quite correctly
    // cannot certify such a zero from a black-box stream and may spend its
    // whole prime budget trying.  Keep the normal fast path, but consult the
    // exact sparse authority only after the first native reconstruction miss;
    // zero columns are then discarded, while a genuinely nonzero hard column
    // still fails closed with the original Symbolica diagnostic.
    let mut exact_support: Option<ExactRow<N>> = None;
    for &column in &candidate_columns {
        let integral = &columns[column];
        let mut black_box = |field: &Zp64, point: &[Fp]| {
            let key = (
                field.get_prime(),
                point.iter().map(|value| *value.inner()).collect::<Vec<_>>(),
            );
            let image = cache
                .entry(key)
                .or_insert_with(|| frame.target_row(target_column, field, point));
            let Some(image) = image.as_ref() else {
                return None;
            };
            if let Some(expected) = generic_pivots.as_ref() {
                if expected != &image.pivots {
                    return None;
                }
            } else {
                generic_pivots = Some(image.pivots.clone());
            }
            Some(
                image
                    .row
                    .iter()
                    .find(|(id, _)| *id as usize == column)
                    .map_or_else(|| field.to_element(0), |(_, value)| *value),
            )
        };
        let reconstruction = reconstruct_rational_function_over_q(
            reconstruction_variables.clone(),
            &mut black_box,
            ReconstructionMethod::Automatic,
            &options,
            max_primes,
        );
        let (reconstructed, stats) = match reconstruction {
            Ok(value) => value,
            Err(error) => {
                if exact_support.is_none() {
                    exact_support = Some(exact_replay(
                        rows,
                        columns,
                        order,
                        target_column,
                        variables,
                        true,
                        &mut observe,
                    )
                    .map_err(|replay| {
                        MaterializationError::SemiNumericalReconstruction(format!(
                            "exact support replay failed after Symbolica reconstruction miss: {replay}"
                        ))
                    })?);
                }
                if !exact_support
                    .as_ref()
                    .expect("support replay stored")
                    .iter()
                    .any(|term| term.integral == *integral && !term.coefficient.is_zero())
                {
                    continue;
                }
                return Err(reconstruction_error(
                    error,
                    column,
                    target_column,
                    rows.len(),
                    columns.len(),
                    reconstruction_variables.len(),
                ));
            }
        };
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
    // The sampled support is discovery evidence only. Re-run the selected
    // source trace over Symbolica's exact sparse field and compare the complete
    // row before handing anything back to the normal canonicalization and
    // publication gates. This makes a sampled-zero term fail closed rather
    // than silently becoming a published omission.
    // Call the already-selected sparse backend directly. Going back through
    // the backend dispatcher here would recursively instantiate its generic
    // observer type through the semi-numerical branch at higher-arity call
    // sites. The replay exposes phase boundaries, not its individual row
    // events, and remains the same characteristic-zero sparse GPLU authority.
    let exact = match exact_support {
        Some(exact) => exact,
        None => exact_replay(
            rows,
            columns,
            order,
            target_column,
            variables,
            false,
            &mut observe,
        )
        .map_err(|error| {
            MaterializationError::SemiNumericalReconstruction(format!(
                "exact characteristic-zero replay failed after reconstruction: {error}"
            ))
        })?,
    };
    if exact != output {
        if !support_retry {
            let exact_support_columns = exact
                .iter()
                .map(|term| {
                    columns
                        .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                        .expect("exact replay uses the common column registry")
                })
                .collect();
            return materialize_with_support(
                rows,
                columns,
                order,
                target_column,
                variables,
                max_degree,
                max_probes,
                max_attempts,
                max_primes,
                Some(exact_support_columns),
                observe,
            );
        }
        return Err(MaterializationError::SemiNumericalReconstruction(
            "sampled reconstruction disagrees with exact characteristic-zero replay".into(),
        ));
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

/// Keep the exact computation and error unchanged while making its cost
/// observable. Do not forward per-row events: one replay can contain a large
/// frame, and observers only need the boundaries to separate this work from
/// reconstruction. A successful replay is still compared with the proposed
/// row by the caller before the materializer can succeed.
fn exact_replay<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    support_recovery: bool,
    observe: &mut dyn FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    observe(MaterializationEvent::SemiNumericalExactReplayStarted { support_recovery });
    let result = super::sparse_materialize(rows, columns, order, target_column, variables, |_| {});
    observe(MaterializationEvent::SemiNumericalExactReplayFinished {
        output_terms: result.as_ref().ok().map(Vec::len),
    });
    result
}

/// Return a union of nonzero target-row columns from independent generic
/// specializations. `None` means no usable image was found and requests the
/// conservative all-column path. This is a structural sparsity screen, not a
/// correctness gate; exact source replay/publication remains authoritative.
fn sampled_frame_support(
    frame: &ProbeFrame,
    target_column: usize,
    dimensions: usize,
) -> Option<(Vec<usize>, Vec<Option<u32>>)> {
    let mut primes = PrimeIteratorU64::new(1 << 61);
    // Keep support by pivot branch and choose the most frequently observed
    // branch.  A union over all branches would reintroduce structurally
    // irrelevant columns and can make Symbolica try to reconstruct a zero
    // coefficient for a different GPLU branch.
    let mut branches: BTreeMap<Vec<Option<u32>>, (usize, BTreeSet<usize>)> = BTreeMap::new();
    let mut rng = StdRng::seed_from_u64(0x7375_7070_6f72_7421);
    for _ in 0..24 {
        let prime = primes.next()?;
        let field = Zp64::new(prime);
        let point: Vec<_> = (0..dimensions)
            .map(|_| field.to_element(rng.random_range(1..prime)))
            .collect();
        if let Some(image) = frame.target_row(target_column, &field, &point) {
            let entry = branches
                .entry(image.pivots)
                .or_insert_with(|| (0, BTreeSet::new()));
            entry.0 += 1;
            entry
                .1
                .extend(image.row.into_iter().map(|(column, _)| column as usize));
        }
    }
    let Some((selected_trace, (count, mut support))) =
        branches
            .into_iter()
            .max_by(|(left_trace, left), (right_trace, right)| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| right_trace.cmp(left_trace))
            })
    else {
        return None;
    };
    if count == 0 {
        return None;
    }
    support.insert(target_column);
    // The branch trace is returned alongside its support and is used as the
    // fixed generic branch for all subsequent Symbolica probes.
    Some((support.into_iter().collect(), selected_trace))
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

// The pre-preparation implementation remains a test-only differential oracle.
#[cfg(test)]
fn target_row_at_point<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    field: &Zp64,
    point: &[Fp],
) -> Option<TargetImage> {
    let mut reducer = SparseRowReducer::new(
        u32::try_from(columns.len()).ok()?.checked_add(1)?,
        field.clone(),
        LuLMode::None,
    );
    let mut values = Vec::new();
    let mut ids = Vec::new();
    let mut pivots = Vec::new();
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
        let pivot = reducer.add_row(&values, &ids);
        pivots.push(pivot);
        if let Some(pivot) = pivot {
            if pivot == target_column as u32 {
                let upper = reducer.u();
                let row = upper.nrows() as usize - 1;
                let start = upper.row_ptrs()[row];
                let end = upper.row_ptrs()[row + 1];
                return Some(TargetImage {
                    row: upper.col_idcs()[start..end]
                        .iter()
                        .copied()
                        .zip(upper.values()[start..end].iter().copied())
                        .collect(),
                    pivots,
                });
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
        let mut events = Vec::new();
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
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(actual[0].integral, integral(3));
        assert!(actual[0].coefficient.is_one());
        assert!(actual.iter().any(|term| term.integral == integral(2)));
        assert_eq!(
            actual,
            super::super::sparse_materialize(&rows, &columns, &order, 0, &variables, |_| {})
                .unwrap()
        );
        let replay = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    MaterializationEvent::SemiNumericalExactReplayStarted { .. }
                        | MaterializationEvent::SemiNumericalExactReplayFinished { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            replay,
            [
                MaterializationEvent::SemiNumericalExactReplayStarted {
                    support_recovery: false,
                },
                MaterializationEvent::SemiNumericalExactReplayFinished {
                    output_terms: Some(actual.len()),
                },
            ]
        );
        assert_eq!(
            events.last(),
            Some(&MaterializationEvent::SemiNumericalFinished {
                output_terms: actual.len(),
            })
        );
        assert!(!events.iter().any(|event| matches!(
            event,
            MaterializationEvent::RowStarted { .. } | MaterializationEvent::RowFinished { .. }
        )));
    }

    #[test]
    fn failed_exact_replay_emits_a_failed_boundary_without_success() {
        let context = CoefficientContext::new(["a"]);
        let rows = vec![vec![Term {
            integral: integral(2),
            coefficient: context.coefficient_fixture("a"),
        }]];
        let order = IntegralOrder::new([true], [false]);
        let mut columns = vec![integral(2), integral(1)];
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        let target = columns
            .iter()
            .position(|value| *value == integral(1))
            .unwrap();
        let mut events = Vec::new();
        let result = exact_replay(
            &rows,
            &columns,
            &order,
            target,
            &variables,
            false,
            &mut |event| events.push(event),
        );
        assert!(matches!(result, Err(MaterializationError::TargetNotPivot)));
        assert_eq!(
            events,
            [
                MaterializationEvent::SemiNumericalExactReplayStarted {
                    support_recovery: false,
                },
                MaterializationEvent::SemiNumericalExactReplayFinished { output_terms: None },
            ]
        );
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
        assert_eq!(
            events,
            [
                MaterializationEvent::SemiNumericalStarted {
                    rows: 1,
                    columns: 1,
                    variables: 1,
                },
                MaterializationEvent::SemiNumericalExactReplayStarted {
                    support_recovery: true,
                },
                MaterializationEvent::SemiNumericalExactReplayFinished {
                    output_terms: Some(1),
                },
            ]
        );
    }

    #[test]
    fn historical_affine_progression_support_samples_can_hide_a_nonzero_factor() {
        // The former support screen used x_i = 2 + 17*s + i.  These points
        // lie on an affine line, so a genuinely nonzero polynomial can vanish
        // on every sampled point.  Keep this as a regression guard against
        // reintroducing that sampling pattern.
        let values: Vec<_> = (0..6u64)
            .map(|sample| {
                let x0 = 2i64 + 17 * sample as i64;
                let x1 = 3i64 + 17 * sample as i64;
                x0 - x1 + 1
            })
            .collect();
        assert!(values.iter().all(|value| *value == 0));
        assert_ne!(2i64 - 4 + 1, 0);
    }

    #[test]
    fn pivot_trace_exposes_an_exceptional_first_probe() {
        let context = CoefficientContext::new(["a"]);
        let rows = vec![
            vec![
                Term {
                    integral: integral(2),
                    coefficient: context.coefficient_fixture("a"),
                },
                Term {
                    integral: integral(1),
                    coefficient: context.coefficient_fixture("1"),
                },
            ],
            vec![
                Term {
                    integral: integral(2),
                    coefficient: context.coefficient_fixture("1"),
                },
                Term {
                    integral: integral(0),
                    coefficient: context.coefficient_fixture("1"),
                },
            ],
        ];
        let order = IntegralOrder::new([true], [false]);
        let mut columns = vec![integral(2), integral(1), integral(0)];
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        let target_column = columns
            .binary_search_by(|column| order.compare(column, &integral(2)))
            .unwrap();
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        let field = Zp64::new(2_305_843_009_213_693_951);
        let generic = target_row_at_point(
            &rows,
            &columns,
            &order,
            target_column,
            &variables,
            &field,
            &[field.to_element(2)],
        )
        .unwrap();
        let exceptional = target_row_at_point(
            &rows,
            &columns,
            &order,
            target_column,
            &variables,
            &field,
            &[field.to_element(0)],
        )
        .unwrap();
        assert_ne!(generic.pivots, exceptional.pivots);
        assert_eq!(generic.pivots.len(), 1);
        assert_eq!(exceptional.pivots.len(), 2);
    }

    #[test]
    fn pivot_trace_preserves_dependent_source_positions() {
        let context = CoefficientContext::new(["a"]);
        let first = vec![Term {
            integral: integral(2),
            coefficient: context.coefficient_fixture("1"),
        }];
        let dependent = vec![Term {
            integral: integral(2),
            coefficient: context.coefficient_fixture("2"),
        }];
        let target = vec![Term {
            integral: integral(1),
            coefficient: context.coefficient_fixture("1"),
        }];
        let with_dependent = vec![first.clone(), dependent, target.clone()];
        let without_dependent = vec![first, target];
        let order = IntegralOrder::new([true], [false]);
        let mut columns = vec![integral(2), integral(1)];
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        let target_column = columns
            .binary_search_by(|column| order.compare(column, &integral(1)))
            .unwrap();
        let variables =
            FrameVariables::try_new(&with_dependent, CoefficientVariableOrder::Original, &[])
                .unwrap();
        let field = Zp64::new(2_305_843_009_213_693_951);
        let with_image = target_row_at_point(
            &with_dependent,
            &columns,
            &order,
            target_column,
            &variables,
            &field,
            &[],
        )
        .unwrap();
        let without_image = target_row_at_point(
            &without_dependent,
            &columns,
            &order,
            target_column,
            &variables,
            &field,
            &[],
        )
        .unwrap();
        assert_eq!(with_image.row, without_image.row);
        assert_ne!(with_image.pivots, without_image.pivots);
        assert_eq!(with_image.pivots, [Some(0), None, Some(1)]);
        assert_eq!(without_image.pivots, [Some(0), Some(1)]);
    }

    #[test]
    fn equal_pivot_columns_can_hide_different_source_chronology() {
        let context = CoefficientContext::new(["a"]);
        let term = |shift, coefficient: &str| Term {
            integral: integral(shift),
            coefficient: context.coefficient_fixture(coefficient),
        };
        let rows = vec![
            vec![term(3, "a"), term(2, "a")],
            vec![term(3, "1"), term(2, "2"), term(1, "1")],
            vec![term(2, "1"), term(1, "2")],
        ];
        let order = IntegralOrder::new([true], [false]);
        let columns = vec![integral(3), integral(2), integral(1)];
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        let field = Zp64::new(2_305_843_009_213_693_951);
        let generic = target_row_at_point(
            &rows,
            &columns,
            &order,
            1,
            &variables,
            &field,
            &[field.to_element(2)],
        )
        .unwrap();
        let exceptional = target_row_at_point(
            &rows,
            &columns,
            &order,
            1,
            &variables,
            &field,
            &[field.to_element(0)],
        )
        .unwrap();
        assert_eq!(
            generic.pivots.iter().flatten().collect::<Vec<_>>(),
            exceptional.pivots.iter().flatten().collect::<Vec<_>>(),
        );
        assert_eq!(generic.pivots, [Some(0), Some(1)]);
        assert_eq!(exceptional.pivots, [None, Some(0), Some(1)]);
        assert_ne!(generic.row, exceptional.row);
    }

    #[test]
    fn sampled_support_keeps_terms_hidden_on_the_historical_affine_line() {
        let context = CoefficientContext::new(["a", "b"]);
        let rows = vec![vec![
            Term {
                integral: integral(2),
                coefficient: context.coefficient_fixture("1"),
            },
            Term {
                integral: integral(1),
                coefficient: context.coefficient_fixture("a-b+1"),
            },
        ]];
        let order = IntegralOrder::new([true], [false]);
        let columns = vec![integral(2), integral(1)];
        let variables =
            FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
        assert_eq!(
            sampled_frame_support(
                &ProbeFrame::new(&rows, &columns, &order, &variables).unwrap(),
                0,
                variables.active_len(),
            )
            .unwrap(),
            (vec![0, 1], vec![Some(0)]),
        );
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
        assert_eq!(actual, rows[0]);
    }

    #[test]
    fn prepared_probes_match_direct_evaluation_across_primes_orders_and_poles() {
        let context = CoefficientContext::new(["unused", "a", "b"]);
        let term = |shift, value: &str| Term {
            integral: integral(shift),
            coefficient: context.coefficient_fixture(value),
        };
        let rows = vec![
            vec![term(3, "a/(b-1)"), term(2, "a")],
            vec![term(3, "2*a/(b-1)"), term(2, "2*a")],
            vec![term(3, "1"), term(2, "2"), term(1, "b")],
            vec![term(2, "1"), term(1, "a+b")],
        ];
        let columns = vec![integral(3), integral(2), integral(1)];
        let order = IntegralOrder::new([true], [false]);
        for variable_order in [
            CoefficientVariableOrder::Original,
            CoefficientVariableOrder::Reverse,
        ] {
            let variables = FrameVariables::try_new(&rows, variable_order, &[]).unwrap();
            assert_eq!(variables.active_len(), 2);
            let prepared = ProbeFrame::new(&rows, &columns, &order, &variables).unwrap();
            for prime in PrimeIteratorU64::new(1 << 61).take(3) {
                let field = Zp64::new(prime);
                for first in 0..4 {
                    for second in 0..4 {
                        let point = [field.to_element(first), field.to_element(second)];
                        for target in 0..columns.len() {
                            assert_eq!(
                                prepared.target_row(target, &field, &point),
                                target_row_at_point(
                                    &rows, &columns, &order, target, &variables, &field, &point,
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
}
