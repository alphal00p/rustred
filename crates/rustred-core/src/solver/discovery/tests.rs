use symbolica::domains::finite_field::FiniteFieldCore;

use crate::algebra::CoefficientContext;

use super::*;

fn order() -> IntegralOrder<1> {
    IntegralOrder::new([true], [false])
}

fn integral(shift: i16) -> Integral<1> {
    Integral::symbolic([shift]).unwrap()
}

fn numerical(field: &Zp64, entries: &[(i16, u64)]) -> Vec<Term<1, NumericalCoefficient>> {
    entries
        .iter()
        .map(|&(shift, coefficient)| Term {
            integral: integral(shift),
            coefficient: field.to_element(coefficient),
        })
        .collect()
}

#[test]
fn dynamic_columns_keep_live_pivots_and_trace_in_integral_order() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(3, 1), (1, 1)])),
        Some(integral(3))
    );
    // Two insertions before the first old column, one between the old
    // columns, and one before the trailing sentinel all happen together.
    let row = [(5, 1), (4, 1), (3, 1), (2, 1), (1, 1), (0, 1)];
    assert_eq!(
        discovery.add_row(&numerical(&field, &row)),
        Some(integral(5))
    );
    assert_eq!(discovery.reducer.pivots()[2], Some(0));
    assert_eq!(
        discovery.add_row(&numerical(
            &field,
            &[(5, 1), (4, 1), (3, 1), (2, 2), (1, 1), (0, 1)],
        )),
        Some(integral(2))
    );
    assert_eq!(discovery.trace(2), [1, 2]);
    assert_eq!(discovery.trace(0), [0]);
    assert_eq!(discovery.trace_many(&[0, 2]), [1, 0, 2]);
    assert_eq!(discovery.trace_many(&[2, 0, 2]), [1, 0, 2]);
    assert!(discovery.trace_many(&[]).is_empty());
    assert_eq!(discovery.stats().columns, 6);
    assert_eq!(discovery.reducer.u().ncols(), 7);
}

#[test]
fn dependent_and_empty_inputs_do_not_shift_accepted_dependency_ids() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    let first = numerical(&field, &[(5, 1), (3, 1)]);
    assert_eq!(discovery.add_row(&first), Some(integral(5)));
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(5, 2), (3, 2)])),
        None
    );
    assert_eq!(discovery.add_row(&[]), None);
    let second = numerical(&field, &[(5, 1), (3, 2), (1, 1)]);
    assert_eq!(discovery.add_row(&second), Some(integral(3)));
    assert_eq!(discovery.add_row(&second), None);
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(3, 1), (1, 2)])),
        Some(integral(1))
    );
    assert_eq!(discovery.accepted_l_rows, [0, 2, 4]);
    assert_eq!(discovery.trace(2), [0, 1, 2]);
    assert_eq!(discovery.basis_len(), 3);
    assert_eq!(discovery.stats().rows_seen, 6);
    assert_eq!(discovery.stats().dependency_edges, 2);
    assert!(discovery.reducer.l().values().is_empty());
}

#[test]
fn full_physical_rank_and_structural_zero_columns_remain_extendable() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    let first = numerical(&field, &[(2, 1)]);
    assert_eq!(discovery.add_row(&first), Some(integral(2)));
    assert_eq!(discovery.add_row(&first), None);
    // Even at full physical rank the native reducer emitted the dependent
    // L slice, because the sentinel prevents its full-rank early return.
    assert_eq!(discovery.reducer.l().nrows(), 2);
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(4, 0), (3, 1), (2, 1)])),
        Some(integral(3))
    );
    assert_eq!(discovery.columns, [integral(4), integral(3), integral(2)]);
    assert_eq!(discovery.trace(1), [1]);
    assert_eq!(discovery.reducer.pivots()[0], None);
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(4, 1)])),
        Some(integral(4))
    );
}

#[test]
fn stats_expose_dependent_l_storage_growth_at_fixed_basis_size() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(3, 1), (1, 1)])),
        Some(integral(3))
    );
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(2, 1), (1, 1)])),
        Some(integral(2))
    );
    let dependent = numerical(&field, &[(3, 1), (2, 1), (1, 2)]);
    for repetitions in 1..=4 {
        assert_eq!(discovery.add_row(&dependent), None);
        let stats = discovery.stats();
        assert_eq!(stats.independent_rows, 2);
        assert_eq!(stats.reducer_nonzeros, 4);
        assert_eq!(stats.dependency_edges, 0);
        assert_eq!(stats.retained_l_rows, 2 + repetitions);
        assert_eq!(stats.retained_l_entries, 2 + 2 * repetitions);
    }
    // The retained patterns consume indices even though Pattern mode has
    // no values; nvalues() alone would incorrectly report zero storage.
    assert!(discovery.reducer.l().values().is_empty());
    assert_eq!(discovery.trace(0), [0]);
    assert_eq!(discovery.trace(1), [1]);
    let before_empty = discovery.stats();
    assert_eq!(discovery.add_row(&[]), None);
    assert_eq!(
        discovery.stats().retained_l_rows,
        before_empty.retained_l_rows
    );
    assert_eq!(
        discovery.stats().retained_l_entries,
        before_empty.retained_l_entries
    );
}

#[test]
fn target_trace_prunes_unrelated_rows_and_keeps_transitive_dependencies() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    for entries in [
        &[(6, 1), (-2, 1)][..],
        &[(4, 1), (2, 1)][..],
        &[(4, 1), (2, 2), (0, 1)][..],
        &[(2, 1), (0, 2)][..],
    ] {
        assert!(discovery.add_row(&numerical(&field, entries)).is_some());
    }
    assert_eq!(discovery.trace(3), [1, 2, 3]);
    assert_eq!(discovery.trace(1), [1]);
}

#[test]
fn rational_exact_replay_uses_pruned_originals_and_stops_at_the_target() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    for entries in [
        &[(5, 1), (0, 1)][..],
        &[(3, 2), (2, 3), (1, 1)][..],
        &[(3, 1), (2, 3), (1, 2)][..],
    ] {
        assert!(discovery.add_row(&numerical(&field, entries)).is_some());
    }
    let context = CoefficientContext::new(["x"]);
    let originals: Vec<ExactRow<1>> = [
        &[(5, "1"), (0, "1")][..],
        &[(3, "x/(x+1)"), (2, "1"), (1, "1/(x+1)")][..],
        &[(3, "1/(x+1)"), (2, "1"), (1, "2/(x+1)")][..],
    ]
    .iter()
    .map(|entries| {
        entries
            .iter()
            .map(|&(shift, coefficient)| Term {
                integral: integral(shift),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect()
    })
    .collect();
    let selected = discovery.trace(2);
    assert_eq!(selected, [1, 2]);
    let mut rows: Vec<_> = selected.iter().map(|&row| originals[row].clone()).collect();
    // A later exact pivot must not change the already produced target row.
    rows.push(vec![Term {
        integral: integral(1),
        coefficient: context.one(),
    }]);
    let result = exact_materialize(&rows, &order(), integral(2)).unwrap();
    let mut events = Vec::new();
    let observed =
        exact_materialize_with_observer(&rows, &order(), integral(2), |event| events.push(event))
            .unwrap();
    assert_eq!(observed, result);
    assert_eq!(
        events,
        [
            MaterializationEvent::FramePrepared {
                source_rows: 3,
                integral_columns: 3,
                target_column: 1,
                input_terms: 7,
                coefficient_variables: 1,
                active_variables: 1,
            },
            MaterializationEvent::RowStarted {
                row: 1,
                input_nonzeros: 3,
                reducer_rows: 0,
                reducer_nonzeros: 0,
            },
            MaterializationEvent::RowFinished {
                row: 1,
                pivot: Some(integral(3)),
                reducer_rows: 1,
                reducer_nonzeros: 3,
            },
            MaterializationEvent::RowStarted {
                row: 2,
                input_nonzeros: 3,
                reducer_rows: 1,
                reducer_nonzeros: 3,
            },
            MaterializationEvent::RowFinished {
                row: 2,
                pivot: Some(integral(2)),
                reducer_rows: 2,
                reducer_nonzeros: 5,
            },
        ]
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].integral, integral(2));
    assert_eq!(result[0].coefficient, context.one());
    assert_eq!(result[1].integral, integral(1));
    assert_eq!(
        result[1].coefficient,
        context.coefficient_fixture("(2*x-1)/((x+1)*(x-1))")
    );
}

#[test]
fn exact_replay_reports_absent_nonpivot_and_noncanonical_inputs() {
    let context = CoefficientContext::new(["x"]);
    let mut row = vec![
        Term {
            integral: integral(3),
            coefficient: context.one(),
        },
        Term {
            integral: integral(2),
            coefficient: context.one(),
        },
    ];
    assert_eq!(
        exact_materialize(&[row.clone()], &order(), integral(1)),
        Err(MaterializationError::TargetAbsent)
    );
    assert_eq!(
        exact_materialize(&[row.clone()], &order(), integral(2)),
        Err(MaterializationError::TargetNotPivot)
    );
    row.reverse();
    assert_eq!(
        exact_materialize(&[row], &order(), integral(2)),
        Err(MaterializationError::NonCanonicalRow { row: 0 })
    );
}

#[test]
fn exact_observer_reports_dependent_rows_without_advancing_native_rank() {
    let context = CoefficientContext::new(["x"]);
    let row = |entries: &[(i16, &str)]| {
        entries
            .iter()
            .map(|&(shift, coefficient)| Term {
                integral: integral(shift),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect()
    };
    let rows = vec![
        row(&[(3, "1"), (1, "1"), (0, "0")]),
        row(&[(3, "2"), (1, "2")]),
        row(&[(3, "1"), (2, "1")]),
    ];
    let baseline = exact_materialize(&rows, &order(), integral(2)).unwrap();
    let mut events = Vec::new();
    let observed =
        exact_materialize_with_observer(&rows, &order(), integral(2), |event| events.push(event))
            .unwrap();
    assert_eq!(observed, baseline);
    assert_eq!(events.len(), 7);
    assert_eq!(
        events[0],
        MaterializationEvent::FramePrepared {
            source_rows: 3,
            integral_columns: 4,
            target_column: 1,
            input_terms: 7,
            coefficient_variables: 1,
            active_variables: 0,
        }
    );
    assert_eq!(
        events[1],
        MaterializationEvent::RowStarted {
            row: 1,
            input_nonzeros: 2,
            reducer_rows: 0,
            reducer_nonzeros: 0,
        }
    );
    assert_eq!(
        events[4],
        MaterializationEvent::RowFinished {
            row: 2,
            pivot: None,
            reducer_rows: 1,
            reducer_nonzeros: 2,
        }
    );
}

#[test]
fn exact_gplu_compacts_fixed_axes_and_restores_the_full_context() {
    let context = CoefficientContext::new([
        "d", "x", "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
        "n13", "n14",
    ]);
    let row = |entries: &[(i16, &str)]| {
        entries
            .iter()
            .map(|&(shift, coefficient)| Term {
                integral: integral(shift),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect()
    };
    let rows = vec![
        row(&[(3, "x/(x+n11)"), (2, "1"), (1, "1/(x+n11)")]),
        row(&[(3, "1/(x+n11)"), (2, "1"), (1, "2/(x+n11)")]),
    ];
    let mut events = Vec::new();
    let result =
        exact_materialize_with_observer(&rows, &order(), integral(2), |event| events.push(event))
            .unwrap();
    assert_eq!(
        result,
        vec![
            Term {
                integral: integral(2),
                coefficient: context.one()
            },
            Term {
                integral: integral(1),
                coefficient: context.coefficient_fixture("(2*x-1)/((x+n11)*(x-1))")
            },
        ]
    );
    assert!(
        result
            .iter()
            .all(|term| term.coefficient.get_variables().len() == 17)
    );
    assert!(matches!(
        events[0],
        MaterializationEvent::FramePrepared {
            coefficient_variables: 17,
            active_variables: 2,
            ..
        }
    ));
}
