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
fn bounded_trace_matches_exact_trace_and_refuses_without_partial_output() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    discovery.add_row(&numerical(&field, &[(3, 1), (1, 1)]));
    discovery.add_row(&numerical(&field, &[(3, 1), (0, 1)]));
    let trace = discovery.trace(1);
    assert_eq!(trace.len(), 2);
    assert_eq!(discovery.trace_bounded(1, 2, 4, |_| 2).unwrap(), trace);
    assert_eq!(
        discovery.trace_bounded(1, 1, 4, |_| 2),
        Err(crate::solver::RuleTrialBudget::ExactTraceRows)
    );
    assert_eq!(
        discovery.trace_bounded(1, 2, 3, |_| 2),
        Err(crate::solver::RuleTrialBudget::ExactTraceTerms)
    );
    assert_eq!(discovery.trace(1), trace, "refusal cannot mutate discovery");
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

#[cfg(feature = "reconstruction")]
#[test]
fn sampled_strict_backend_is_explicit_and_legacy_keeps_its_threshold() {
    use crate::node::driver::parse_backend;
    assert_eq!(
        parse_backend("sampled-strict").unwrap(),
        SymbolicExactBackend::SampledStrict {
            threads: 1,
            seed: 0x5eed
        }
    );
    assert_eq!(
        parse_backend("sampled").unwrap(),
        SymbolicExactBackend::Sampled {
            threads: 1,
            seed: 0x5eed,
            min_rows: 600
        }
    );
    assert!(parse_backend("sampled_strict").is_err());
}

#[cfg(feature = "reconstruction")]
#[test]
fn sampled_strict_refuses_actual_dense_box_without_exact_fallback() {
    // Only four nonzero monomials, but a genuine native dense reconstruction
    // box > MAX_BOX. Drive the actual dispatcher/sampler, no injected failure.
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let rows = vec![vec![
        Term {
            integral: integral(1),
            coefficient: context.one(),
        },
        Term {
            integral: integral(0),
            coefficient: context.coefficient_fixture("a^16+b^16+c^16+d^16"),
        },
    ]];
    let run = |backend| {
        let mut events = Vec::new();
        let result = exact_materialize_using_with_observer(
            &rows,
            &order(),
            integral(1),
            backend,
            CoefficientVariableOrder::Original,
            &[],
            |event| events.push(event),
        );
        (result, events)
    };
    let assert_refused = |(result, events): (
        Result<ExactRow<1>, MaterializationError>,
        Vec<MaterializationEvent<1>>,
    )| {
        assert!(
            matches!(&result, Err(MaterializationError::SemiNumericalReconstruction(reason))
            if reason.contains("strict sampled refused: dense box")
                && reason.contains("fallback disabled")),
            "{result:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, MaterializationEvent::SemiNumericalStarted { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MaterializationEvent::TargetBlockStarted { .. }))
        );
    };
    let strict = SymbolicExactBackend::SampledStrict {
        threads: 1,
        seed: 0x5eed,
    };
    assert_refused(run(strict));
    // A pre-screen Sampled decision must not bypass the selected strict backend.
    crate::node::screen::with_screen(
        Some(crate::node::screen::LiftScreen {
            min_rows: 0,
            sampled_from_rows: Some(0),
            max_box: 6000,
            ..Default::default()
        }),
        || assert_refused(run(strict)),
    );
    crate::node::screen::take_reports();
    // Same real dense-box failure: legacy still enters TOF and matches Sparse.
    let (legacy, events) = run(SymbolicExactBackend::Sampled {
        threads: 1,
        seed: 0x5eed,
        min_rows: 0,
    });
    assert_eq!(
        legacy.unwrap(),
        exact_materialize(&rows, &order(), integral(1)).unwrap()
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, MaterializationEvent::TargetBlockStarted { .. }))
    );
}

/// Deterministic pseudo-random rows over a small symbolic column space.
fn random_rows(field: &Zp64, rows: usize, seed: u64) -> Vec<Vec<Term<1, NumericalCoefficient>>> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    (0..rows)
        .map(|_| {
            let mut shifts: Vec<i16> = (0..1 + next() % 5).map(|_| (next() % 24) as i16).collect();
            shifts.sort_unstable_by(|a, b| b.cmp(a));
            shifts.dedup();
            numerical(
                field,
                &shifts
                    .iter()
                    .map(|&shift| (shift, 1 + next() % 100))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

#[test]
fn batched_rows_reproduce_row_by_row_insertion() {
    let field = Zp64::new(101);
    for seed in 0..8 {
        let rows = random_rows(&field, 40, seed);
        let mut single = Discovery::new(order(), field.clone());
        let pivots: Vec<_> = rows.iter().map(|row| single.add_row(row)).collect();
        let mut batched = Discovery::new(order(), field.clone());
        let (first, second) = rows.split_at(17);
        let mut batch_pivots = batched.add_rows(first);
        batch_pivots.extend(batched.add_rows(second));
        assert_eq!(pivots, batch_pivots);
        assert_eq!(single.columns(), batched.columns());
        assert_eq!(single.stats(), batched.stats());
        // One native column registration per batch with new columns.
        assert!(batched.column_batches() <= 2);
        assert!(batched.column_batches() < single.column_batches());
        assert_eq!(
            single.pivot_columns().collect::<Vec<_>>(),
            batched.pivot_columns().collect::<Vec<_>>()
        );
        for row in 0..batched.basis_len() {
            assert_eq!(single.u_row_columns(row), batched.u_row_columns(row));
            assert_eq!(single.u_row_values(row), batched.u_row_values(row));
        }
        assert_eq!(single.u_nnz(), batched.u_nnz());
        assert_eq!(single.l_entries(), batched.l_entries());
    }
}

#[test]
fn single_walk_trace_union_equals_trace_many() {
    let field = Zp64::new(101);
    for seed in 0..8 {
        let mut discovery = Discovery::new(order(), field.clone());
        discovery.add_rows(&random_rows(&field, 60, 100 + seed));
        let basis = discovery.basis_len();
        let all: Vec<usize> = (0..basis).collect();
        assert_eq!(discovery.trace_union(&all), discovery.trace_many(&all));
        let some: Vec<usize> = (0..basis).filter(|row| row % 3 == 1).collect();
        assert_eq!(discovery.trace_union(&some), discovery.trace_many(&some));
        for root in 0..basis {
            assert_eq!(discovery.trace_union(&[root]), discovery.trace(root));
        }
    }
}

#[test]
fn registered_columns_have_no_pivot_until_a_row_reaches_them() {
    let field = Zp64::new(101);
    let mut discovery = Discovery::new(order(), field.clone());
    discovery.register(vec![integral(4), integral(2), integral(4)]);
    assert_eq!(discovery.columns(), [integral(4), integral(2)]);
    assert_eq!(discovery.pivot_row(0), None);
    assert_eq!(
        discovery.add_row(&numerical(&field, &[(4, 3), (3, 1)])),
        Some(integral(4))
    );
    assert_eq!(discovery.columns(), [integral(4), integral(3), integral(2)]);
    assert_eq!(discovery.pivot_row(0), Some(0));
    assert_eq!(discovery.pivot_row(2), None);
    assert_eq!(discovery.u_row_columns(0), [0, 1]);
}
