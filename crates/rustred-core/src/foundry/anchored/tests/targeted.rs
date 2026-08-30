use std::sync::Arc;

use symbolica::domains::SelfRing;

use crate::family::IntegralKey;
use crate::identity::RowId;
use crate::sector::OrderingPolicy;

use super::super::derive::derive_strictly_descending_rule_for_target;
use super::super::error::AnchoredRuleError;
use super::super::limits::AnchoredRuleLimits;
use super::super::prepare::{OrderedIntegral, PreparedSourceRow, prepare_problem};
use super::super::sparse::reduce_rows_for_target;
use super::tadpole_sources;

#[test]
fn targeted_anchored_rule_is_exact_and_lookup_failures_are_typed() {
    let (_, context, relations, _) = tadpole_sources();
    let rule = derive_strictly_descending_rule_for_target(
        &context,
        &relations,
        &[1],
        &[2],
        OrderingPolicy::default(),
        AnchoredRuleLimits::default(),
    )
    .unwrap();
    assert_eq!(rule.pivot().powers(), &[2]);
    assert_eq!(rule.replay().source_rows_used(), 1);

    assert_eq!(
        derive_strictly_descending_rule_for_target(
            &context,
            &relations,
            &[1],
            &[2, 3],
            OrderingPolicy::default(),
            AnchoredRuleLimits::default(),
        ),
        Err(AnchoredRuleError::WrongTargetIntegralArity {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(
        derive_strictly_descending_rule_for_target(
            &context,
            &relations,
            &[1],
            &[99],
            OrderingPolicy::default(),
            AnchoredRuleLimits::default(),
        ),
        Err(AnchoredRuleError::TargetIntegralAbsent)
    );
    assert_eq!(
        derive_strictly_descending_rule_for_target(
            &context,
            &relations,
            &[1],
            &[1],
            OrderingPolicy::default(),
            AnchoredRuleLimits::default(),
        ),
        Err(AnchoredRuleError::TargetIntegralNotPivot)
    );
}

#[test]
fn targeted_anchored_back_substitution_budgets_have_exact_boundaries() {
    let (_, context, relations, _) = tadpole_sources();
    let defaults = AnchoredRuleLimits::default();
    let exact = AnchoredRuleLimits {
        max_back_substitution_output_nonzero_entries: 3,
        max_back_substitution_live_nonzero_entries: 10,
        ..defaults
    };
    derive_strictly_descending_rule_for_target(
        &context,
        &relations,
        &[1],
        &[2],
        OrderingPolicy::default(),
        exact,
    )
    .unwrap();

    assert_eq!(
        derive_strictly_descending_rule_for_target(
            &context,
            &relations,
            &[1],
            &[2],
            OrderingPolicy::default(),
            AnchoredRuleLimits {
                max_back_substitution_output_nonzero_entries: 2,
                ..exact
            },
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "Symbolica target back-substitution output nonzero entries",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(
        derive_strictly_descending_rule_for_target(
            &context,
            &relations,
            &[1],
            &[2],
            OrderingPolicy::default(),
            AnchoredRuleLimits {
                max_back_substitution_live_nonzero_entries: 9,
                ..exact
            },
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "Symbolica target back-substitution live nonzero entries",
            requested: 10,
            limit: 9,
        })
    );
}

#[test]
fn targeted_anchored_path_keeps_duplicate_provenance_columns_free() {
    let (_, context, relations, _) = tadpole_sources();
    let limits = AnchoredRuleLimits::default();
    let mut problem = prepare_problem(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    problem.sources = ["first", "dependent"]
        .into_iter()
        .map(|label| PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from(label),
            },
            entries: vec![(0, context.base().one()), (1, context.base().one())],
            guards: Vec::new(),
        })
        .collect();

    let reduced = reduce_rows_for_target(&context, &problem, 0, limits).unwrap();
    assert_eq!(reduced.pivot_column, 0);
    assert_eq!(
        reduced
            .integral_entries
            .iter()
            .map(|(column, coefficient)| (*column, coefficient.is_one()))
            .collect::<Vec<_>>(),
        [(0, true), (1, true)]
    );
    assert_eq!(reduced.source_combination.len(), 1);
    assert_eq!(reduced.source_combination[0].source_ordinal(), 0);
    assert_eq!(
        reduced.source_combination[0].row_id(),
        &RowId::Derived {
            label: Arc::from("first")
        }
    );
    assert!(reduced.source_combination[0].coefficient().is_one());
    assert_eq!(reduced.pivot_guards.len(), 1);
    assert_eq!(reduced.pivot_guards[0].source_ordinal(), 0);
    assert_eq!(
        reduced.pivot_guards[0].row_id(),
        &RowId::Derived {
            label: Arc::from("first")
        }
    );
    assert_eq!(reduced.pivot_guards[0].pivot_column(), 0);
    assert!(reduced.pivot_guards[0].coefficient().is_one());

    problem.sources = vec![PreparedSourceRow {
        row_id: RowId::Derived {
            label: Arc::from("target-only"),
        },
        entries: vec![(0, context.base().one())],
        guards: Vec::new(),
    }];
    assert_eq!(
        reduce_rows_for_target(&context, &problem, 0, limits).err(),
        Some(AnchoredRuleError::TargetHasNoStrictlyDescendingRule)
    );
}

#[test]
fn targeted_anchored_path_remaps_a_later_physical_pivot_across_a_provenance_row() {
    let (base, context, relations, _) = tadpole_sources();
    let limits = AnchoredRuleLimits::default();
    let mut problem = prepare_problem(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    problem.columns = [3, 2, 1]
        .into_iter()
        .map(|power| {
            let key = IntegralKey::try_new([power]).unwrap();
            OrderedIntegral {
                complexity: problem.ordering.complexity_key(key.powers()).unwrap(),
                key,
            }
        })
        .collect();
    problem.sources = vec![
        PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from("first-physical"),
            },
            entries: vec![(0, base.integer(2))],
            guards: Vec::new(),
        },
        PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from("dependent-provenance"),
            },
            entries: vec![(0, base.integer(4))],
            guards: Vec::new(),
        },
        PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from("later-physical"),
            },
            entries: vec![
                (0, base.integer(6)),
                (1, base.integer(3)),
                (2, base.integer(3)),
            ],
            guards: Vec::new(),
        },
    ];

    // The requested pivot starts as reducer row 2, becomes physical-CSR row 1,
    // and becomes reverse-RREF row 0. Looking it up through the remapped pivot
    // map is therefore required to recover this exact row.
    let reduced = reduce_rows_for_target(&context, &problem, 1, limits).unwrap();
    assert_eq!(reduced.pivot_column, 1);
    assert_eq!(
        reduced
            .integral_entries
            .iter()
            .map(|(column, coefficient)| (*column, coefficient.is_one()))
            .collect::<Vec<_>>(),
        [(1, true), (2, true)]
    );

    let minus_one = base.integer(-1);
    let one_third = base
        .try_div(&base.one(), &base.integer(3), Default::default())
        .unwrap();
    assert_eq!(reduced.source_combination.len(), 2);
    assert_eq!(reduced.source_combination[0].source_ordinal(), 0);
    assert_eq!(
        reduced.source_combination[0].row_id(),
        &RowId::Derived {
            label: Arc::from("first-physical")
        }
    );
    assert_eq!(reduced.source_combination[0].coefficient(), &minus_one);
    assert_eq!(reduced.source_combination[1].source_ordinal(), 2);
    assert_eq!(
        reduced.source_combination[1].row_id(),
        &RowId::Derived {
            label: Arc::from("later-physical")
        }
    );
    assert_eq!(reduced.source_combination[1].coefficient(), &one_third);

    assert_eq!(reduced.pivot_guards.len(), 2);
    assert_eq!(reduced.pivot_guards[0].source_ordinal(), 0);
    assert_eq!(
        reduced.pivot_guards[0].row_id(),
        &RowId::Derived {
            label: Arc::from("first-physical")
        }
    );
    assert_eq!(reduced.pivot_guards[0].pivot_column(), 0);
    assert_eq!(reduced.pivot_guards[0].coefficient(), &base.integer(2));
    assert_eq!(reduced.pivot_guards[1].source_ordinal(), 2);
    assert_eq!(
        reduced.pivot_guards[1].row_id(),
        &RowId::Derived {
            label: Arc::from("later-physical")
        }
    );
    assert_eq!(reduced.pivot_guards[1].pivot_column(), 1);
    assert_eq!(reduced.pivot_guards[1].coefficient(), &base.integer(3));
}
