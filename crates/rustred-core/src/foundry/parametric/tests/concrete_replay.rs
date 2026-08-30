use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::anchored::{AnchoredRuleLimits, derive_strictly_descending_rule_for_target};
use crate::identity::{IndexShift, ParametricRelation, RowId};
use crate::sector::OrderingPolicy;

use super::super::anchor::verify_concrete_specialization_replay;
use super::super::{
    ParametricNonZeroGuard, ParametricRuleError, ParametricRuleLimits, ParametricRuleTerm,
    ParametricSourceRowContribution, derive_sector_interior_rule,
    derive_sector_monotone_rule_for_target,
};
use super::support::tadpole_sources;

#[test]
fn concrete_replay_witness_counts_every_referenced_key_and_zero_term() {
    let (_, context, mut relations, _) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    let n_minus_one = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let zero_at_anchor = relations[0]
        .scaled_for_artifact_forgery_test(&context, &n_minus_one)
        .unwrap();
    relations.push(zero_at_anchor);

    let mut sources = rule.source_combination().to_vec();
    sources.push(ParametricSourceRowContribution::new(
        1,
        relations[1].row_id().clone(),
        context.one(),
    ));
    // Independently cover a retained source weight that itself specializes
    // to zero; its referenced row terms must still be checked and counted.
    sources.push(ParametricSourceRowContribution::new(
        0,
        relations[0].row_id().clone(),
        n_minus_one.clone(),
    ));
    // Add a nonzero pair which cancels exactly by integral key.
    sources.push(ParametricSourceRowContribution::new(
        0,
        relations[0].row_id().clone(),
        context.one(),
    ));
    sources.push(ParametricSourceRowContribution::new(
        0,
        relations[0].row_id().clone(),
        context
            .neg_with_limits(&context.one(), Default::default())
            .unwrap(),
    ));

    // A distinct RHS term whose coefficient specializes to zero must still
    // be specialized, keyed, and included in the deterministic census.
    let mut right_hand_side = rule.right_hand_side().to_vec();
    right_hand_side.push(ParametricRuleTerm::new(
        IndexShift::try_new([-1], 1).unwrap(),
        n_minus_one,
        rule.right_hand_side()[0].descent().clone(),
    ));
    let witness = verify_concrete_specialization_replay(
        &context,
        &relations,
        &[1],
        rule.pivot(),
        &right_hand_side,
        rule.nonzero_guards(),
        &sources,
        ParametricRuleLimits::default(),
    )
    .unwrap();

    assert_eq!(witness.source_contributions_checked(), 5);
    assert_eq!(witness.source_terms_checked(), 10);
    assert_eq!(witness.right_hand_side_terms_checked(), 2);
    assert_eq!(witness.integral_keys_checked(), 13);
    assert!(witness.exact_operations() > 0);
    assert!(witness.peak_retained_coefficient_terms() > 0);
}

#[test]
fn concrete_replay_reuses_sealed_payloads_without_full_operand_scans() {
    let (_, context, relations, _) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let (full_operand_scans_before, _) = context.authentication_scan_counts();
    verify_concrete_specialization_replay(
        &context,
        &relations,
        &[1],
        rule.pivot(),
        rule.right_hand_side(),
        rule.nonzero_guards(),
        rule.source_combination(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    // Native-result authentication may legitimately increase the second
    // counter. The first counter alone records redundant full operand scans.
    let (full_operand_scans_after, _) = context.authentication_scan_counts();
    assert_eq!(full_operand_scans_after, full_operand_scans_before);
}

#[test]
fn retained_payload_census_replaces_and_cancels_without_double_counting() {
    let (_, context, seed_relations, _) = tadpole_sources();
    let seed_rule = derive_sector_interior_rule(
        &context,
        &seed_relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let extra = IndexShift::try_new([-2], 1).unwrap();
    let rows = [
        ParametricRelation::from_terms_for_foundry_test(
            "concrete-replay-retained-payload",
            RowId::Derived {
                label: Arc::from("pivot-and-rhs"),
            },
            &context,
            [
                (seed_rule.pivot().clone(), context.one()),
                (
                    seed_rule.right_hand_side()[0].shift().clone(),
                    context.one(),
                ),
            ],
        )
        .unwrap(),
        ParametricRelation::from_terms_for_foundry_test(
            "concrete-replay-retained-payload",
            RowId::Derived {
                label: Arc::from("insert-extra"),
            },
            &context,
            [(extra.clone(), context.one())],
        )
        .unwrap(),
        ParametricRelation::from_terms_for_foundry_test(
            "concrete-replay-retained-payload",
            RowId::Derived {
                label: Arc::from("replace-extra"),
            },
            &context,
            [(extra.clone(), context.one())],
        )
        .unwrap(),
        ParametricRelation::from_terms_for_foundry_test(
            "concrete-replay-retained-payload",
            RowId::Derived {
                label: Arc::from("cancel-extra"),
            },
            &context,
            [(extra, context.integer(-2))],
        )
        .unwrap(),
    ];
    let sources = rows
        .iter()
        .enumerate()
        .map(|(source_ordinal, row)| {
            ParametricSourceRowContribution::new(
                source_ordinal,
                row.row_id().clone(),
                context.one(),
            )
        })
        .collect::<Vec<_>>();
    let rhs = [ParametricRuleTerm::new(
        seed_rule.right_hand_side()[0].shift().clone(),
        context
            .neg_with_limits(&context.one(), Default::default())
            .unwrap(),
        seed_rule.right_hand_side()[0].descent().clone(),
    )];
    let witness = verify_concrete_specialization_replay(
        &context,
        &rows,
        &[1],
        seed_rule.pivot(),
        &rhs,
        &[],
        &sources,
        ParametricRuleLimits::default(),
    )
    .unwrap();

    // Three retained constant rational coefficients own two polynomial terms
    // each. Replacing +1 by +2 and then cancelling it must not accumulate the
    // obsolete map values into the retained peak.
    assert_eq!(witness.peak_retained_coefficient_terms(), 6);
    assert_eq!(witness.source_contributions_checked(), 4);
    assert_eq!(witness.source_terms_checked(), 5);
    assert_eq!(witness.integral_keys_checked(), 7);
}

#[test]
fn concrete_replay_rejects_pivot_rhs_source_leftover_and_provenance_mutations() {
    let (_, context, relations, _) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let verify = |relations: &[ParametricRelation],
                  pivot: &IndexShift,
                  rhs: &[ParametricRuleTerm],
                  sources: &[ParametricSourceRowContribution]| {
        verify_concrete_specialization_replay(
            &context,
            relations,
            &[1],
            pivot,
            rhs,
            rule.nonzero_guards(),
            sources,
            ParametricRuleLimits::default(),
        )
    };

    assert_eq!(
        verify(
            &relations,
            rule.right_hand_side()[0].shift(),
            rule.right_hand_side(),
            rule.source_combination(),
        ),
        Err(ParametricRuleError::ConcreteReplayPivotMismatch)
    );

    let term = &rule.right_hand_side()[0];
    let wrong_rhs = [ParametricRuleTerm::new(
        term.shift().clone(),
        context.add(term.coefficient(), &context.one()).unwrap(),
        term.descent().clone(),
    )];
    assert_eq!(
        verify(
            &relations,
            rule.pivot(),
            &wrong_rhs,
            rule.source_combination(),
        ),
        Err(ParametricRuleError::ConcreteReplayRightHandSideMismatch {
            right_hand_side_ordinal: 0,
        })
    );

    assert_eq!(
        verify(&relations, rule.pivot(), &[], rule.source_combination(),),
        Err(ParametricRuleError::ConcreteReplayUnexpectedIntegral)
    );

    let doubled = [relations[0]
        .scaled_for_artifact_forgery_test(&context, &context.integer(2))
        .unwrap()];
    assert_eq!(
        verify(
            &doubled,
            rule.pivot(),
            rule.right_hand_side(),
            rule.source_combination(),
        ),
        Err(ParametricRuleError::ConcreteReplayPivotMismatch)
    );

    let original_source = &rule.source_combination()[0];
    let wrong_weight = [ParametricSourceRowContribution::new(
        original_source.source_ordinal(),
        original_source.row_id().clone(),
        context
            .mul(original_source.coefficient(), &context.integer(2))
            .unwrap(),
    )];
    assert_eq!(
        verify(
            &relations,
            rule.pivot(),
            rule.right_hand_side(),
            &wrong_weight,
        ),
        Err(ParametricRuleError::ConcreteReplayPivotMismatch)
    );

    let out_of_range = [ParametricSourceRowContribution::new(
        1,
        relations[0].row_id().clone(),
        context.one(),
    )];
    assert_eq!(
        verify(
            &relations,
            rule.pivot(),
            rule.right_hand_side(),
            &out_of_range,
        ),
        Err(ParametricRuleError::ConcreteReplaySourceOrdinalOutOfRange { source_ordinal: 1 })
    );

    let wrong_identity = [ParametricSourceRowContribution::new(
        0,
        RowId::Derived {
            label: Arc::from("wrong-row"),
        },
        context.one(),
    )];
    assert_eq!(
        verify(
            &relations,
            rule.pivot(),
            rule.right_hand_side(),
            &wrong_identity,
        ),
        Err(ParametricRuleError::ConcreteReplaySourceIdentityMismatch { source_ordinal: 0 })
    );
}

#[test]
fn concrete_replay_guard_overflow_and_resource_failures_are_typed() {
    let (_, context, relations, _) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    let mut guards = rule.nonzero_guards().to_vec();
    let zero_polynomial = context
        .numerator_condition_with_limits(&context.zero(), Default::default())
        .unwrap();
    guards[0] = ParametricNonZeroGuard {
        polynomial: zero_polynomial,
        origins: guards[0].origins().to_vec(),
    };
    assert_eq!(
        verify_concrete_specialization_replay(
            &context,
            &relations,
            &[1],
            rule.pivot(),
            rule.right_hand_side(),
            &guards,
            rule.source_combination(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal: 0 })
    );

    assert_eq!(
        verify_concrete_specialization_replay(
            &context,
            &relations,
            &[i64::MAX],
            rule.pivot(),
            rule.right_hand_side(),
            rule.nonzero_guards(),
            rule.source_combination(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::AnchorIndexOverflow { position: 0 })
    );

    let exact = rule.concrete_replay().exact_operations();
    let retained_terms = rule.concrete_replay().peak_retained_coefficient_terms();
    let exact_retained = ParametricRuleLimits {
        max_concrete_replay_retained_coefficient_terms: retained_terms,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        verify_concrete_specialization_replay(
            &context,
            &relations,
            &[1],
            rule.pivot(),
            rule.right_hand_side(),
            rule.nonzero_guards(),
            rule.source_combination(),
            exact_retained,
        )
        .unwrap()
        .peak_retained_coefficient_terms(),
        retained_terms
    );

    // Clone-owned byte evidence is intentionally runtime-only. Discover the
    // exact aggregate ceiling through typed failures, then pin exact and
    // one-below behavior without persisting a platform-dependent capacity.
    let mut retained_bytes = 0usize;
    loop {
        let limits = ParametricRuleLimits {
            max_concrete_replay_retained_coefficient_clone_owned_bytes: retained_bytes,
            ..ParametricRuleLimits::default()
        };
        match verify_concrete_specialization_replay(
            &context,
            &relations,
            &[1],
            rule.pivot(),
            rule.right_hand_side(),
            rule.nonzero_guards(),
            rule.source_combination(),
            limits,
        ) {
            Ok(_) => break,
            Err(ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay retained coefficient clone-owned bytes",
                requested,
                limit,
            }) if limit == retained_bytes && requested > retained_bytes => {
                retained_bytes = requested;
            }
            other => panic!("unexpected byte-census boundary result: {other:?}"),
        }
    }
    assert!(retained_bytes > 0);
    let one_below_retained_bytes = retained_bytes - 1;
    assert_eq!(
        verify_concrete_specialization_replay(
            &context,
            &relations,
            &[1],
            rule.pivot(),
            rule.right_hand_side(),
            rule.nonzero_guards(),
            rule.source_combination(),
            ParametricRuleLimits {
                max_concrete_replay_retained_coefficient_clone_owned_bytes:
                    one_below_retained_bytes,
                ..ParametricRuleLimits::default()
            },
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "concrete specialization replay retained coefficient clone-owned bytes",
            requested: retained_bytes,
            limit: one_below_retained_bytes,
        })
    );
    let resource_cases = [
        (
            ParametricRuleLimits {
                max_concrete_replay_terms: 3,
                ..ParametricRuleLimits::default()
            },
            ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay terms",
                requested: 4,
                limit: 3,
            },
        ),
        (
            ParametricRuleLimits {
                max_concrete_replay_integral_key_power_cells: 3,
                ..ParametricRuleLimits::default()
            },
            ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay integral-key power cells",
                requested: 5,
                limit: 3,
            },
        ),
        (
            ParametricRuleLimits {
                max_concrete_replay_integral_keys: 1,
                ..ParametricRuleLimits::default()
            },
            ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay distinct integral keys",
                requested: 2,
                limit: 1,
            },
        ),
        (
            ParametricRuleLimits {
                max_concrete_replay_retained_coefficient_terms: retained_terms - 1,
                ..ParametricRuleLimits::default()
            },
            ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay retained coefficient terms",
                requested: retained_terms,
                limit: retained_terms - 1,
            },
        ),
        (
            ParametricRuleLimits {
                max_concrete_replay_exact_operations: exact - 1,
                ..ParametricRuleLimits::default()
            },
            ParametricRuleError::ResourceLimit {
                resource: "concrete specialization replay exact operations",
                requested: exact,
                limit: exact - 1,
            },
        ),
    ];
    for (limits, expected) in resource_cases {
        assert_eq!(
            verify_concrete_specialization_replay(
                &context,
                &relations,
                &[1],
                rule.pivot(),
                rule.right_hand_side(),
                rule.nonzero_guards(),
                rule.source_combination(),
                limits,
            ),
            Err(expected)
        );
    }
}

#[test]
fn boundary_column_reordering_does_not_invalidate_an_exact_parametric_rule() {
    let base = CoefficientContext::new(["d"]);
    let context =
        IndexedCoefficientContext::try_new(&base, "concrete-replay-boundary-ordering", 2).unwrap();
    let pivot = IndexShift::try_new([2, 2], 2).unwrap();
    let boundary_column = IndexShift::try_new([-1, 4], 2).unwrap();
    let rhs = IndexShift::try_new([1, 1], 2).unwrap();
    let pivot_relation = ParametricRelation::from_terms_for_foundry_test(
        "synthetic-boundary-ordering-family",
        RowId::Derived {
            label: Arc::from("pivot-to-interior"),
        },
        &context,
        [(pivot.clone(), context.one()), (rhs.clone(), context.one())],
    )
    .unwrap();
    let boundary_relation = ParametricRelation::from_terms_for_foundry_test(
        "synthetic-boundary-ordering-family",
        RowId::Derived {
            label: Arc::from("boundary-to-interior"),
        },
        &context,
        [
            (boundary_column.clone(), context.one()),
            (rhs, context.one()),
        ],
    )
    .unwrap();
    let relations = [pivot_relation, boundary_relation];

    let rule = derive_sector_monotone_rule_for_target(
        &context,
        &relations,
        &[1, 1],
        pivot.values(),
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    assert_eq!(rule.pivot(), &pivot);
    assert_eq!(rule.right_hand_side()[0].shift().values(), [1, 1]);
    assert_eq!(rule.concrete_replay().anchor().powers(), [1, 1]);

    // The parametric order is pivot > boundary column > RHS. At n=(1,1),
    // however, the middle column pinches from two propagators to one, so the
    // concrete order is pivot > RHS > boundary column. Both exact target rows
    // are valid source-span elements, but their RREF normal forms differ.
    let anchored = derive_strictly_descending_rule_for_target(
        &context,
        &relations,
        &[1, 1],
        &[3, 3],
        OrderingPolicy::default(),
        AnchoredRuleLimits::default(),
    )
    .unwrap();
    assert_eq!(anchored.right_hand_side().len(), 1);
    assert_eq!(anchored.right_hand_side()[0].integral().powers(), [0, 5]);
    assert_ne!(anchored.right_hand_side()[0].integral().powers(), [2, 2]);
}
