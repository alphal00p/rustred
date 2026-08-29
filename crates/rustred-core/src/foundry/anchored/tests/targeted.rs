use std::sync::Arc;

use crate::identity::RowId;
use crate::sector::OrderingPolicy;

use super::super::derive::derive_strictly_descending_rule_for_target;
use super::super::error::AnchoredRuleError;
use super::super::limits::AnchoredRuleLimits;
use super::super::prepare::{PreparedSourceRow, prepare_problem};
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
        max_back_substitution_live_nonzero_entries: 7,
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
                max_back_substitution_live_nonzero_entries: 6,
                ..exact
            },
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "Symbolica target back-substitution live nonzero entries",
            requested: 7,
            limit: 6,
        })
    );
}

#[test]
fn targeted_anchored_path_rejects_reachable_provenance_pivots() {
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
    let provenance_pivot = problem.columns.len();
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

    assert_eq!(
        reduce_rows_for_target(&context, &problem, 0, limits).err(),
        Some(
            AnchoredRuleError::TargetBackSubstitutionUsesProvenancePivot {
                source_ordinal: 1,
                pivot_column: provenance_pivot,
            }
        )
    );

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
