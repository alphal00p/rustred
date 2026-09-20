use crate::algebra::CoefficientContext;

use super::super::{
    CoefficientVariableOrder, SymbolicExactBackend, exact_materialize,
    exact_materialize_using_with_observer,
};
use super::*;

fn integral(shift: i16) -> Integral<1> {
    Integral::symbolic([shift]).unwrap()
}

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> ExactRow<1> {
    terms
        .iter()
        .map(|(shift, coefficient)| Term {
            integral: integral(*shift),
            coefficient: context.coefficient_fixture(coefficient),
        })
        .collect()
}

fn lift(rows: &[ExactRow<1>], target: i16) -> Result<ExactRow<1>, MaterializationError> {
    exact_materialize_using_with_observer(
        rows,
        &IntegralOrder::new([true], [false]),
        integral(target),
        SymbolicExactBackend::SparseTargetOnly,
        CoefficientVariableOrder::Original,
        &[],
        |_| {},
    )
}

#[test]
fn native_full_product_preserves_rational_coefficients_tail_variables_and_early_stop() {
    let context = CoefficientContext::new(["unused", "a", "b", "c"]);
    let rows = vec![
        row(&context, &[(4, "a/(b-1)"), (3, "1/(b-1)"), (1, "c/(b-1)")]),
        row(
            &context,
            &[(4, "-2*a/3"), (3, "-(a+2)/3"), (2, "-c/3"), (0, "-1/3")],
        ),
        // Would violate independent-F admission if processed after the hit.
        row(&context, &[(2, "1"), (0, "1")]),
    ];
    let expected = row(&context, &[(3, "1"), (2, "c/a"), (1, "-2*c/a"), (0, "1/a")]);
    let order = IntegralOrder::new([true], [false]);
    assert_eq!(
        exact_materialize(&rows, &order, integral(3)).unwrap(),
        expected
    );
    for policy in [
        CoefficientVariableOrder::Original,
        CoefficientVariableOrder::Reverse,
        CoefficientVariableOrder::IndicesFirst,
    ] {
        let mut events = Vec::new();
        let result = exact_materialize_using_with_observer(
            &rows,
            &order,
            integral(3),
            SymbolicExactBackend::SparseTargetOnly,
            policy,
            &[2, 1, 3, 0],
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(result, expected);
        assert!(
            result
                .iter()
                .all(|term| term.coefficient.get_variables() == context.one().get_variables())
        );
        assert!(matches!(
            events[1],
            MaterializationEvent::TargetBlockStarted { columns: 2 }
        ));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, MaterializationEvent::RowStarted { .. }))
                .count(),
            2
        );
        assert!(matches!(
            events[6],
            MaterializationEvent::TargetWeightsStarted {
                rows: 2,
                lower_nonzeros: 3
            }
        ));
        assert!(matches!(
            events[7],
            MaterializationEvent::TargetWeightsFinished { nonzero_weights: 2 }
        ));
        assert!(matches!(
            events[8],
            MaterializationEvent::TargetReconstructionStarted {
                rows: 2,
                columns: 5
            }
        ));
        assert!(matches!(
            events[9],
            MaterializationEvent::TargetReconstructionFinished { output_terms: 4 }
        ));
        assert_eq!(events.len(), 10);
    }
}

#[test]
fn insertion_order_may_differ_from_physical_pivot_order() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let rows = vec![
        row(&context, &[(3, "2"), (1, "a")]),
        row(&context, &[(4, "3"), (3, "4"), (1, "b")]),
        row(&context, &[(4, "6"), (3, "10"), (2, "5"), (0, "c")]),
    ];
    let expected = row(&context, &[(2, "1"), (1, "(-2*b-a)/5"), (0, "c/5")]);
    assert_eq!(lift(&rows, 2).unwrap(), expected);
    assert_eq!(
        exact_materialize(&rows, &IntegralOrder::new([true], [false]), integral(2)).unwrap(),
        expected
    );
}

#[test]
fn nonunit_unsorted_lower_factor_uses_native_normalization_and_pivot_mapping() {
    let context = CoefficientContext::new(["unused"]);
    let mut lower = SparseMatrix::new(0, 3, ExactField::new(Z));
    for (values, ids) in [
        (vec!["2"], vec![0]),
        (vec!["3", "5"], vec![0, 1]),
        (vec!["11", "7", "13"], vec![1, 0, 2]),
    ] {
        lower.add_row(
            values
                .iter()
                .map(|value| context.coefficient_fixture(value))
                .collect(),
            ids,
        );
    }
    let weights = solve_transposed_lower(&lower, 2, context.one(), false).unwrap();
    assert_eq!(weights.col_idcs(), &[0, 1, 2]);
    assert_eq!(
        weights.values(),
        &[
            context.coefficient_fixture("-1/65"),
            context.coefficient_fixture("-11/65"),
            context.coefficient_fixture("1/13"),
        ]
    );
}

#[test]
fn empty_dependent_and_tail_only_prefix_rows_are_typed_errors_not_dropped() {
    let context = CoefficientContext::new(["a"]);
    let target = row(&context, &[(2, "1"), (0, "a")]);
    for prefix in [
        vec![],
        row(&context, &[(3, "0")]),
        row(&context, &[(1, "a")]),
    ] {
        assert_eq!(
            lift(&[prefix, target.clone()], 2),
            Err(MaterializationError::TargetOnlyDependentPrefix { row: 1 })
        );
    }
    let rows = [
        row(&context, &[(3, "1"), (1, "a")]),
        // Independent in full A but dependent in the forbidden/target block.
        row(&context, &[(3, "2"), (0, "1")]),
        target,
    ];
    assert!(exact_materialize(&rows, &IntegralOrder::new([true], [false]), integral(2)).is_ok());
    assert_eq!(
        lift(&rows, 2),
        Err(MaterializationError::TargetOnlyDependentPrefix { row: 2 })
    );
}

#[test]
fn hidden_weight_poles_match_the_default_but_are_not_new_certificate_authority() {
    let context = CoefficientContext::new(["x"]);
    for terms in [
        vec![(2, "x"), (0, "x")],
        vec![(2, "x")],
        vec![(2, "1/x"), (0, "1/x")],
    ] {
        let rows = [row(&context, &terms)];
        let result = lift(&rows, 2).unwrap();
        assert_eq!(
            result,
            exact_materialize(&rows, &IntegralOrder::new([true], [false]), integral(2)).unwrap()
        );
        assert!(result.iter().all(|term| term.coefficient.is_one()));
        // Only a candidate ExactRow is returned. Original-source replay and
        // uncancelled source/weight guards still belong to the artifact owner.
    }
}

#[test]
fn constant_frames_and_denominator_only_variables_keep_the_original_map() {
    for parameters in [vec![], vec!["unused", "a"]] {
        let context = CoefficientContext::new(parameters);
        let rows = [row(&context, &[(2, "-2/3"), (0, "5/7")])];
        assert_eq!(
            lift(&rows, 2).unwrap(),
            row(&context, &[(2, "1"), (0, "-15/14")])
        );
    }
    let context = CoefficientContext::new(["unused", "a"]);
    let rows = [row(&context, &[(2, "-1/a"), (0, "1")])];
    assert_eq!(
        lift(&rows, 2).unwrap(),
        row(&context, &[(2, "1"), (0, "-a")])
    );
}

#[test]
fn malformed_lower_shapes_and_coordinates_fail_before_native_solving() {
    let context = CoefficientContext::new(["a"]);
    let mut missing = SparseMatrix::new(0, 2, ExactField::new(Z));
    missing.add_row(vec![context.one()], vec![0]);
    let mut above = missing.clone();
    above.add_row(vec![context.one()], vec![1]);
    // Rebuild with a forbidden upper entry while keeping a square shape.
    let mut upper = SparseMatrix::new(0, 2, ExactField::new(Z));
    upper.add_row(vec![context.one(), context.one()], vec![0, 1]);
    upper.add_row(vec![context.one()], vec![1]);
    let mut zero = SparseMatrix::new(0, 1, ExactField::new(Z));
    zero.add_row(vec![context.zero()], vec![0]);
    for (lower, target) in [(missing, 0), (above, 2), (upper, 1), (zero, 0)] {
        assert!(matches!(
            solve_transposed_lower(&lower, target, context.one(), false),
            Err(MaterializationError::TargetOnlyInvalidDecomposition(_))
        ));
    }
}

#[test]
fn native_full_product_rejects_wrong_weights_and_surviving_forbidden_terms() {
    let context = CoefficientContext::new(["a"]);
    let rows = [
        row(&context, &[(3, "1"), (2, "1"), (0, "a")]),
        row(&context, &[(3, "1"), (0, "1")]),
    ];
    let columns = [integral(3), integral(2), integral(0)];
    let variables =
        FrameVariables::try_new(&rows, CoefficientVariableOrder::Original, &[]).unwrap();
    for (ncols, terms) in [
        (1, vec![(0, "1")]),
        (2, vec![(0, "1")]),
        (2, vec![(0, "2"), (1, "-2")]),
        (2, vec![(1, "1")]),
    ] {
        let mut weights = SparseMatrix::new(0, ncols, ExactField::new(Z));
        weights.add_row(
            terms
                .iter()
                .map(|(_, value)| context.coefficient_fixture(value))
                .collect(),
            terms.iter().map(|(column, _)| *column).collect(),
        );
        assert!(matches!(
            reconstruct(
                &rows,
                &columns,
                &IntegralOrder::new([true], [false]),
                1,
                &|value| variables.map_coefficient(value),
                &|value| variables.restore_coefficient(value),
                weights,
                false,
            ),
            Err(MaterializationError::TargetOnlyInvalidDecomposition(_))
        ));
    }
}

fn experimental_lift<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
    factored: bool,
    policy: CoefficientVariableOrder,
    priority: &[usize],
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    let variables = FrameVariables::try_new(rows, policy, priority)?;
    let mut columns: Vec<_> = rows.iter().flatten().map(|term| term.integral).collect();
    columns.sort_unstable_by(|a, b| order.compare(a, b));
    columns.dedup();
    let target_column = columns
        .binary_search_by(|column| order.compare(column, &target))
        .map_err(|_| MaterializationError::TargetAbsent)?;
    if factored {
        materialize_factorized(rows, &columns, order, target_column, &variables, observe)
    } else {
        materialize(rows, &columns, order, target_column, &variables, observe)
    }
}

#[test]
fn factorized_block_preserves_full_row_both_maps_prefix_and_all_schedule_events() {
    let context = CoefficientContext::new(["unused", "a", "b", "tail"]);
    let rows = vec![
        row(
            &context,
            &[(4, "a/(b-1)^2"), (3, "1/(b-1)^2"), (1, "tail/(b-1)^2")],
        ),
        row(
            &context,
            &[(4, "-2*a/3"), (3, "-(a+2)/3"), (2, "-tail/3"), (0, "-1/3")],
        ),
        // This block-empty row must not be processed after the target hit.
        row(&context, &[(2, "1"), (0, "1")]),
    ];
    let order = IntegralOrder::new([true], [false]);
    let expected = exact_materialize(&rows, &order, integral(3)).unwrap();
    for policy in [
        CoefficientVariableOrder::Original,
        CoefficientVariableOrder::Reverse,
        CoefficientVariableOrder::IndicesFirst,
    ] {
        let mut observed = Vec::new();
        let ordinary = experimental_lift(
            &rows,
            &order,
            integral(3),
            false,
            policy,
            &[2, 1, 3, 0],
            |e| observed.push(e),
        )
        .unwrap();
        let mut events = Vec::new();
        let actual = experimental_lift(
            &rows,
            &order,
            integral(3),
            true,
            policy,
            &[2, 1, 3, 0],
            |e| events.push(e),
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual, ordinary);
        assert_eq!(events, observed);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, MaterializationEvent::RowStarted { .. }))
                .count(),
            2
        );
        for term in actual {
            assert_eq!(term.coefficient.numerator.variables(), context.variables());
            assert_eq!(
                term.coefficient.denominator.variables(),
                context.variables()
            );
        }
        assert_eq!(
            exact_materialize_using_with_observer(
                &rows,
                &order,
                integral(3),
                SymbolicExactBackend::SparseFactorized,
                policy,
                &[2, 1, 3, 0],
                |_| {}
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn factorized_block_rejects_empty_tail_only_and_dependent_prefixes() {
    let context = CoefficientContext::new(["a"]);
    let order = IntegralOrder::new([true], [false]);
    let target = row(&context, &[(2, "1"), (0, "a")]);
    for prefix in [
        vec![],
        row(&context, &[(3, "0")]),
        row(&context, &[(1, "a")]),
    ] {
        assert_eq!(
            experimental_lift(
                &[prefix, target.clone()],
                &order,
                integral(2),
                true,
                CoefficientVariableOrder::Original,
                &[],
                |_| {}
            ),
            Err(MaterializationError::TargetOnlyDependentPrefix { row: 1 })
        );
    }
    let rows = [
        row(&context, &[(3, "1"), (1, "a")]),
        row(&context, &[(3, "2"), (0, "1")]),
        target,
    ];
    assert!(exact_materialize(&rows, &order, integral(2)).is_ok());
    assert_eq!(
        experimental_lift(
            &rows,
            &order,
            integral(2),
            true,
            CoefficientVariableOrder::Original,
            &[],
            |_| {}
        ),
        Err(MaterializationError::TargetOnlyDependentPrefix { row: 2 })
    );
}

#[test]
fn factorized_native_triangular_solve_handles_unsorted_nonunit_lower_entries() {
    use symbolica::domains::factorized_rational_polynomial::FactorizedRationalPolynomialField;
    let context = CoefficientContext::new(["unused", "a"]);
    let field = FactorizedRationalPolynomialField::<_, u16>::new(Z, context.variables().clone());
    let mut lower = SparseMatrix::new(0, 3, field);
    for (values, ids) in [
        (vec!["2"], vec![0]),
        (vec!["3", "5"], vec![0, 1]),
        (vec!["11", "7", "13"], vec![1, 0, 2]),
    ] {
        lower.add_row(
            values
                .into_iter()
                .map(|v| {
                    factorized::factor(context.coefficient_fixture(v), context.variables()).unwrap()
                })
                .collect(),
            ids,
        );
    }
    let one = factorized::factor(context.one(), context.variables()).unwrap();
    let weights = solve_transposed_lower(&lower, 2, one, true).unwrap();
    assert_eq!(weights.col_idcs(), &[0, 1, 2]);
    for (actual, expected) in weights.values().iter().zip(["-1/65", "-11/65", "1/13"]) {
        let actual = factorized::ordinary(actual, context.variables()).unwrap();
        assert_eq!(actual, context.coefficient_fixture(expected));
        assert_eq!(actual.numerator.variables(), context.variables());
        assert_eq!(actual.denominator.variables(), context.variables());
    }
}

#[test]
fn full_frame_admission_checks_zero_tail_and_after_hit_denominators() {
    let context = CoefficientContext::new(["a"]);
    let order = IntegralOrder::new([true], [false]);
    for factored in [false, true] {
        for (after_hit, zero) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut bad = if zero { context.zero() } else { context.one() };
            bad.denominator = context.zero().denominator.zero();
            let bad = Term {
                integral: integral(0),
                coefficient: bad,
            };
            let mut rows = vec![row(&context, &[(2, "1"), (1, "a")])];
            if after_hit {
                rows.push(vec![bad]);
            } else {
                rows[0].push(bad);
            }
            let mut events = Vec::new();
            let result = experimental_lift(
                &rows,
                &order,
                integral(2),
                factored,
                CoefficientVariableOrder::Original,
                &[],
                |e| events.push(e),
            );
            assert_eq!(
                result,
                Err(MaterializationError::InvalidFactorizedCoefficient(
                    "zero input denominator"
                ))
            );
            assert!(events.is_empty());
        }
        let other = CoefficientContext::new(["foreign"]);
        let mut bad = context.zero();
        bad.denominator = other.one().denominator;
        let rows = [
            row(&context, &[(2, "1")]),
            vec![Term {
                integral: integral(0),
                coefficient: bad,
            }],
        ];
        assert_eq!(
            experimental_lift(
                &rows,
                &order,
                integral(2),
                factored,
                CoefficientVariableOrder::Original,
                &[],
                |_| {}
            ),
            Err(MaterializationError::CoefficientVariableMapMismatch)
        );
    }
}

#[test]
fn combined_field_is_arity_generic_and_preserves_constant_contexts_and_cancelled_poles() {
    fn check<const N: usize>() {
        for parameters in [vec![], vec!["unused", "a"]] {
            let context = CoefficientContext::new(parameters);
            let target = Integral::symbolic([2; N]).unwrap();
            let tail = Integral::symbolic([1; N]).unwrap();
            let order = IntegralOrder::new([true; N], [false; N]);
            for coefficient in [
                "-2/3",
                if context.variables().is_empty() {
                    "5/7"
                } else {
                    "1/(a-1)^2"
                },
            ] {
                let rows = [vec![
                    Term {
                        integral: target,
                        coefficient: context.coefficient_fixture(coefficient),
                    },
                    Term {
                        integral: tail,
                        coefficient: context.coefficient_fixture(coefficient),
                    },
                ]];
                let actual = experimental_lift(
                    &rows,
                    &order,
                    target,
                    true,
                    CoefficientVariableOrder::Original,
                    &[],
                    |_| {},
                )
                .unwrap();
                assert_eq!(actual, exact_materialize(&rows, &order, target).unwrap());
                for term in actual {
                    assert_eq!(term.coefficient, context.one());
                    assert_eq!(term.coefficient.numerator.variables(), context.variables());
                    assert_eq!(
                        term.coefficient.denominator.variables(),
                        context.variables()
                    );
                }
            }
        }
    }
    check::<2>();
    check::<7>();
}

#[test]
fn factorized_native_panics_are_typed_but_observer_panics_escape() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    assert!(matches!(
        native::<()>(true, "test operation", || panic!("native test panic")),
        Err(MaterializationError::FactorizedNativePanic {
            operation: "test operation"
        })
    ));
    assert!(
        catch_unwind(|| native::<()>(false, "test operation", || panic!("ordinary test panic")))
            .is_err()
    );
    let context = CoefficientContext::new(["a"]);
    let rows = [row(&context, &[(2, "1"), (0, "a")])];
    for factored in [false, true] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| experimental_lift(
                &rows,
                &IntegralOrder::new([true], [false]),
                integral(2),
                factored,
                CoefficientVariableOrder::Original,
                &[],
                |_| panic!("observer test panic")
            )))
            .is_err()
        );
    }
}
