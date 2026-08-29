use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::identity::RowId;
use crate::sector::{ComplexityComponent, OrderingPolicy, SectorMonotonePointClass};

use super::super::prepare::{PreparedSourceRow, prepare_problem};
use super::super::sparse::reduce_rows_for_target;
use super::super::{
    ParametricRuleError, ParametricRuleLimits, SectorMonotoneDependencyKind,
    derive_sector_interior_rule_for_target, derive_sector_monotone_rule_for_target,
};
use super::support::{sunset_sources, tadpole_sources};

#[test]
fn complete_sunset_span_yields_the_targeted_e1_rref_recurrence() {
    let (base, context, relations) = sunset_sources();
    let rule = derive_sector_interior_rule_for_target(
        &context,
        &relations,
        &[2, 2, 2],
        &[1, 0, 0],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    assert!(rule.sector_monotone_admission().is_none());
    assert_eq!(rule.pivot().values(), &[1, 0, 0]);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        vec![
            &[0, 1, -1][..],
            &[0, 0, 0][..],
            &[0, -1, 1][..],
            &[-1, 1, 0][..],
            &[-1, 0, 1][..],
        ]
    );
    assert!(
        rule.right_hand_side()
            .iter()
            .all(|term| term.descent().verify())
    );

    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let s = context.lift(&base.parameter("s").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let n2 = context.index(2).unwrap();
    let three = context.integer(3);
    let denominator = context.mul(&context.mul(&three, &s).unwrap(), &n0).unwrap();
    let expected_rhs = [
        context.div(&n1, &denominator).unwrap(),
        context
            .div(
                &context.sub(&d, &context.mul(&three, &n0).unwrap()).unwrap(),
                &denominator,
            )
            .unwrap(),
        context.div(&n2, &denominator).unwrap(),
        context
            .div(
                &context.mul(&context.integer(-1), &n1).unwrap(),
                &denominator,
            )
            .unwrap(),
        context
            .div(
                &context.mul(&context.integer(-1), &n2).unwrap(),
                &denominator,
            )
            .unwrap(),
    ];
    for (actual, expected) in rule.right_hand_side().iter().zip(&expected_rhs) {
        assert_indexed_equal(&context, actual.coefficient(), expected);
    }

    // The absent ordinal 2 pins its exact source weight to zero.
    assert_eq!(
        rule.source_combination()
            .iter()
            .map(|source| source.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
    let two_s_n0 = context
        .mul(&context.mul(&context.integer(2), &s).unwrap(), &n0)
        .unwrap();
    let three_s_n0 = denominator.clone();
    let six_s_n0 = context
        .mul(&context.mul(&context.integer(6), &s).unwrap(), &n0)
        .unwrap();
    let expected_sources = [
        context.div(&context.integer(-1), &two_s_n0).unwrap(),
        context.div(&context.one(), &three_s_n0).unwrap(),
        context.div(&context.one(), &six_s_n0).unwrap(),
    ];
    for (actual, expected) in rule.source_combination().iter().zip(&expected_sources) {
        assert_indexed_equal(&context, actual.coefficient(), expected);
    }

    assert_eq!(
        rule.elimination_pivot_guards()
            .iter()
            .map(|guard| guard.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );

    assert_eq!(rule.replay().source_rows_used(), 3);
    assert_eq!(rule.replay().shift_columns_checked(), 10);
    assert_eq!(
        rule.anchor_agreement().anchored_rule().pivot().powers(),
        &[3, 2, 2]
    );
    assert_eq!(rule.anchor_agreement().specialized_source_terms(), 3);

    let repeated = derive_sector_interior_rule_for_target(
        &context,
        &relations,
        &[2, 2, 2],
        &[1, 0, 0],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    assert_eq!(rule, repeated);
}

#[test]
fn complete_sunset_span_exposes_corner_pinch_dependencies() {
    let (_, context, relations) = sunset_sources();
    assert_eq!(
        derive_sector_interior_rule_for_target(
            &context,
            &relations,
            &[1, 1, 1],
            &[1, 0, 0],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::AnchorOutsideInterior)
    );

    let rule = derive_sector_monotone_rule_for_target(
        &context,
        &relations,
        &[1, 1, 1],
        &[1, 0, 0],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    assert!(!rule.domain().contains(rule.anchor().powers()).unwrap());
    assert_eq!(rule.sector().active_bits(), &[true, true, true]);
    assert_eq!(
        rule.anchor_agreement().anchored_rule().pivot().powers(),
        &[2, 1, 1]
    );

    let admission = rule.sector_monotone_admission().unwrap();
    assert!(admission.verify());
    assert_eq!(admission.parent_sector(), rule.sector());
    assert_eq!(
        admission
            .domain()
            .bounds()
            .iter()
            .map(|bounds| (bounds.lower(), bounds.upper()))
            .collect::<Vec<_>>(),
        vec![(1, i64::MAX - 1), (1, i64::MAX - 1), (1, i64::MAX - 1),]
    );
    assert_eq!(admission.dependencies().len(), 5);
    assert_eq!(
        admission
            .proper_subsector_dependency_count_at(rule.anchor().powers())
            .unwrap(),
        4
    );
    assert_eq!(
        admission
            .dependencies()
            .iter()
            .map(|dependency| dependency.shift().values())
            .collect::<Vec<_>>(),
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>()
    );
    let corner = admission.classify(rule.anchor().powers()).unwrap();
    assert_eq!(
        corner
            .iter()
            .map(|dependency| dependency.kind())
            .collect::<Vec<_>>(),
        vec![
            SectorMonotoneDependencyKind::ProperSubsector,
            SectorMonotoneDependencyKind::SameSector,
            SectorMonotoneDependencyKind::ProperSubsector,
            SectorMonotoneDependencyKind::ProperSubsector,
            SectorMonotoneDependencyKind::ProperSubsector,
        ]
    );
    assert_eq!(
        corner
            .iter()
            .map(|dependency| dependency.target_sector().active_bits())
            .collect::<Vec<_>>(),
        vec![
            &[true, true, false][..],
            &[true, true, true][..],
            &[true, false, true][..],
            &[false, true, true][..],
            &[false, true, true][..],
        ]
    );
    for dependency in &corner {
        assert!(dependency.verify());
        match dependency.kind() {
            SectorMonotoneDependencyKind::SameSector => {
                assert_ne!(
                    dependency.descent().decisive_component(),
                    ComplexityComponent::PropagatorCount
                );
                assert_eq!(dependency.pinched_positions().count(), 0);
            }
            SectorMonotoneDependencyKind::ProperSubsector => {
                assert_eq!(
                    dependency.descent().decisive_component(),
                    ComplexityComponent::PropagatorCount
                );
                assert!(dependency.pinched_positions().count() >= 1);
            }
        }
    }

    let thresholds = admission
        .dependencies()
        .iter()
        .map(|dependency| {
            dependency
                .descent()
                .thresholds()
                .iter()
                .map(|threshold| {
                    (
                        threshold.position(),
                        threshold.pinched_upper(),
                        threshold.same_sector_lower(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        thresholds,
        vec![
            vec![(2, 1, Some(2))],
            vec![],
            vec![(1, 1, Some(2))],
            vec![(0, 1, Some(2))],
            vec![(0, 1, Some(2))],
        ]
    );

    // Every point of this representative face/edge/corner cube belongs to
    // exactly one term-local cell and carries a concrete strict witness.
    for n0 in 1..=3 {
        for n1 in 1..=3 {
            for n2 in 1..=3 {
                let point = [n0, n1, n2];
                for dependency in admission.classify(&point).unwrap() {
                    assert!(dependency.verify());
                    match dependency.partition_class() {
                        SectorMonotonePointClass::SameSector => assert_eq!(
                            dependency.descent().source().sector(),
                            dependency.descent().target().sector()
                        ),
                        SectorMonotonePointClass::ProperSubsector {
                            cylinder_ordinal,
                            pinched_position,
                        } => {
                            assert_eq!(cylinder_ordinal, 0);
                            assert!(!dependency.target_sector().active_bits()[pinched_position]);
                            assert_eq!(
                                dependency.descent().decisive_component(),
                                ComplexityComponent::PropagatorCount
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sector_monotone_sunset_resource_boundaries_are_preflighted() {
    let (_, context, relations) = sunset_sources();
    let derive = |limits| {
        derive_sector_monotone_rule_for_target(
            &context,
            &relations,
            &[1, 1, 1],
            &[1, 0, 0],
            OrderingPolicy::default(),
            limits,
        )
    };
    let exact = ParametricRuleLimits {
        // Prepared interior + monotone box + five same-sector cells, each
        // retaining two endpoints for three coordinates.
        max_domain_bound_endpoint_cells: 42,
        max_sector_monotone_thresholds: 4,
        ..ParametricRuleLimits::default()
    };
    derive(exact).unwrap();

    assert_eq!(
        derive(ParametricRuleLimits {
            max_domain_bound_endpoint_cells: 41,
            ..exact
        }),
        Err(ParametricRuleError::ResourceLimit {
            resource: "live sector-monotone domain bound endpoint cells",
            requested: 42,
            limit: 41,
        })
    );
    assert_eq!(
        derive(ParametricRuleLimits {
            max_sector_monotone_thresholds: 3,
            ..exact
        }),
        Err(ParametricRuleError::ResourceLimit {
            resource: "sector-monotone active pinch thresholds",
            requested: 4,
            limit: 3,
        })
    );
}

#[test]
fn sector_monotone_target_rejects_inactive_line_activation_typed() {
    let (_, context, relations, _) = tadpole_sources();
    // The +1 term is present in K(n), so admitting it on the inactive-sector
    // boundary would require a coefficient-aware piecewise proof even though
    // its concrete coefficient happens to vanish at n=0.
    assert_eq!(
        derive_sector_monotone_rule_for_target(
            &context,
            &relations,
            &[0],
            &[0],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::ActivationLeakRequiresRefinement {
            right_hand_side_ordinal: 0,
            position: 0,
            shift: 1,
        })
    );
}

#[test]
fn targeted_parametric_lookup_failures_are_typed() {
    let (_, context, relations) = sunset_sources();
    let derive = |target: &[i64]| {
        derive_sector_interior_rule_for_target(
            &context,
            &relations,
            &[2, 2, 2],
            target,
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        )
    };

    assert_eq!(
        derive(&[1, 0]),
        Err(ParametricRuleError::WrongTargetShiftArity {
            expected: 3,
            actual: 2,
        })
    );
    assert_eq!(
        derive(&[9, 9, 9]),
        Err(ParametricRuleError::TargetShiftAbsent)
    );
    assert_eq!(
        derive(&[0, 0, 0]),
        Err(ParametricRuleError::TargetShiftNotPivot)
    );
}

#[test]
fn targeted_parametric_back_substitution_budgets_have_exact_boundaries() {
    let (_, context, relations, _) = tadpole_sources();
    let defaults = ParametricRuleLimits::default();
    let exact = ParametricRuleLimits {
        max_back_substitution_output_nonzero_entries: 3,
        max_back_substitution_live_nonzero_entries: 7,
        ..defaults
    };
    derive_sector_interior_rule_for_target(
        &context,
        &relations,
        &[1],
        &[1],
        OrderingPolicy::default(),
        exact,
    )
    .unwrap();

    assert_eq!(
        derive_sector_interior_rule_for_target(
            &context,
            &relations,
            &[1],
            &[1],
            OrderingPolicy::default(),
            ParametricRuleLimits {
                max_back_substitution_output_nonzero_entries: 2,
                ..exact
            },
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "Symbolica target back-substitution output nonzero entries",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(
        derive_sector_interior_rule_for_target(
            &context,
            &relations,
            &[1],
            &[1],
            OrderingPolicy::default(),
            ParametricRuleLimits {
                max_back_substitution_live_nonzero_entries: 6,
                ..exact
            },
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "Symbolica target back-substitution live nonzero entries",
            requested: 7,
            limit: 6,
        })
    );
}

#[test]
fn targeted_parametric_path_rejects_reachable_provenance_pivots() {
    let (_, context, relations) = sunset_sources();
    let limits = ParametricRuleLimits::default();
    let mut problem = prepare_problem(
        &context,
        &relations,
        &[2, 2, 2],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let provenance_pivot = problem.columns.len();
    problem.sources = duplicate_physical_rows(&context);

    assert_eq!(
        reduce_rows_for_target(&context, &problem, 0, limits).err(),
        Some(
            ParametricRuleError::TargetBackSubstitutionUsesProvenancePivot {
                source_ordinal: 1,
                pivot_column: provenance_pivot,
            }
        )
    );

    problem.sources = vec![PreparedSourceRow {
        row_id: RowId::Derived {
            label: Arc::from("target-only"),
        },
        entries: vec![(0, context.one())],
        guards: Vec::new(),
    }];
    assert_eq!(
        reduce_rows_for_target(&context, &problem, 0, limits).err(),
        Some(ParametricRuleError::TargetHasNoUniformlyDescendingRule)
    );
}

fn duplicate_physical_rows(context: &IndexedCoefficientContext) -> Vec<PreparedSourceRow> {
    ["first", "dependent"]
        .into_iter()
        .map(|label| PreparedSourceRow {
            row_id: RowId::Derived {
                label: Arc::from(label),
            },
            entries: vec![(0, context.one()), (1, context.one())],
            guards: Vec::new(),
        })
        .collect()
}

fn assert_indexed_equal(
    context: &IndexedCoefficientContext,
    actual: &IndexedCoefficient,
    expected: &IndexedCoefficient,
) {
    assert!(context.sub(actual, expected).unwrap().is_zero());
}
