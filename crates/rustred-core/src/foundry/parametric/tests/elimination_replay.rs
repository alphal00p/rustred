use std::sync::Arc;

use crate::identity::RowId;
use crate::sector::OrderingPolicy;

use super::super::prepare::{PreparedSourceRow, prepare_problem};
use super::super::replay::verify_exact_source_replay;
use super::super::sparse::reduce_rows;
use super::super::{ParametricRuleError, ParametricRuleLimits};
use super::support::{sunset_sources, tadpole_sources};

#[test]
fn indexed_elimination_retains_the_full_recursive_pivot_chain() {
    let (_, context, relations) = sunset_sources();
    let limits = ParametricRuleLimits::default();
    let mut problem = prepare_problem(
        &context,
        &relations[1..=1],
        &[2, 3, 4],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    problem.sources = vec![
        PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from("first"),
            },
            entries: vec![(0, context.integer(2))],
            guards: Vec::new(),
        },
        PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from("second"),
            },
            entries: vec![
                (0, context.integer(4)),
                (1, context.integer(10)),
                (2, context.integer(6)),
            ],
            guards: Vec::new(),
        },
    ];
    let reduced = reduce_rows(&context, &problem, limits).unwrap();

    assert_eq!(reduced.pivot_column, 1);
    assert_eq!(
        reduced
            .pivot_guards
            .iter()
            .map(|guard| guard.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        reduced
            .pivot_guards
            .iter()
            .map(|guard| guard.pivot_shift().values())
            .collect::<Vec<_>>(),
        vec![
            problem.columns[0].shift.values(),
            problem.columns[1].shift.values(),
        ]
    );
    let minus_one_fifth = context
        .div(&context.integer(-1), &context.integer(5))
        .unwrap();
    let one_tenth = context.div(&context.one(), &context.integer(10)).unwrap();
    assert!(
        context
            .sub(
                reduced.source_combination[0].coefficient(),
                &minus_one_fifth,
            )
            .unwrap()
            .is_zero()
    );
    assert!(
        context
            .sub(reduced.source_combination[1].coefficient(), &one_tenth)
            .unwrap()
            .is_zero()
    );
    let replay = verify_exact_source_replay(&context, &problem, &reduced, limits).unwrap();
    assert_eq!(replay.source_rows_used(), 2);
    assert_eq!(replay.shift_columns_checked(), 7);

    let one_below = ParametricRuleLimits {
        max_elimination_pivot_dependency_entries: 2,
        ..limits
    };
    assert_eq!(
        reduce_rows(&context, &problem, one_below).err(),
        Some(ParametricRuleError::ResourceLimit {
            resource: "aggregate parametric elimination pivot dependencies",
            requested: 3,
            limit: 2,
        })
    );
}

#[test]
fn exact_indexed_replay_rejects_a_tampered_candidate() {
    let (_, context, relations, _) = tadpole_sources();
    let limits = ParametricRuleLimits::default();
    let problem = prepare_problem(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let mut reduced = reduce_rows(&context, &problem, limits).unwrap();
    reduced.shift_entries[1].1 = context.zero();
    assert_eq!(
        verify_exact_source_replay(&context, &problem, &reduced, limits),
        Err(ParametricRuleError::ReplayMismatch { shift_column: 1 })
    );
}
