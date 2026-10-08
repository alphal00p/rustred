use super::*;
use crate::family::AffineDenominator;
use crate::reduction::terminal_relations::TerminalRelationSession;

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}
fn tadpole(name: &str, scale_parameter: bool) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(if scale_parameter {
        vec!["d", "s"]
    } else {
        vec!["d"]
    });
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                c.integer(-1),
                vec![if scale_parameter {
                    c.parameter("s").unwrap()
                } else {
                    c.one()
                }],
            )],
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    )
}
fn sunset(name: &str, reverse: bool, inactive_massless: bool, shift: bool) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    let mut rows = vec![[1, 0, 0], [0, 0, 1], [1, 2, 1]];
    if reverse {
        rows.reverse();
    }
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            rows.into_iter()
                .enumerate()
                .map(|(i, row)| {
                    AffineDenominator::new(
                        c.integer(if inactive_massless && i == 2 { 0 } else { -1 }),
                        row.into_iter().map(|v| c.integer(v)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![c.integer(i64::from(shift)), c.zero(), c.zero()],
        )
        .unwrap(),
    )
}
fn input(
    family: Arc<IntegralFamily>,
    powers: &[&[i64]],
) -> (Arc<IntegralFamily>, BTreeSet<IntegralKey>) {
    (family, powers.iter().map(|p| key(p)).collect())
}
fn replay(plan: &VacuumDiagonalCollectionPlan) {
    let families: BTreeMap<_, _> = plan.families().map(|f| (f.fingerprint(), f)).collect();
    let context = plan.families().next().unwrap().coefficient_context();
    let limits = VacuumDiagonalCollectionLimits::default();
    // Re-form every diagonal sum from the retained original ordinary rows.
    for source in plan.sources() {
        let mut row = BTreeMap::new();
        for original in source.rows() {
            for (key, value) in &original.equation().terms {
                accumulate(&mut row, key.clone(), value, context, limits).unwrap();
            }
        }
        assert_eq!(row, source.sum().terms);
    }
    for equation in plan.equations() {
        let mut row = Row::new();
        for (&index, weight) in equation.source_weights() {
            let source = &plan.sources()[index];
            let family = families[source.family_fingerprint()];
            for (key, value) in &source.sum().terms {
                let target = plan.aliases().representative(family, key).unwrap().clone();
                let value = context.try_mul(weight, value, Default::default()).unwrap();
                accumulate(&mut row, target, &value, context, limits).unwrap();
            }
        }
        let mut expected = Row::new();
        for (key, value) in equation.terms() {
            let target = plan
                .aliases()
                .representative(families[key.family_fingerprint()], key.integral())
                .unwrap()
                .clone();
            accumulate(&mut expected, target, value, context, limits).unwrap();
        }
        assert_eq!(row, expected);
    }
}

#[test]
fn diagonal_collection_prefers_dots_and_flat_maps_restore_original_mass_power() {
    let a = input(tadpole("diagonal-a", false), &[&[1], &[2]]);
    let b = input(tadpole("diagonal-b", false), &[&[1], &[2]]);
    let plan =
        VacuumDiagonalCollectionPlan::prepare(&[a.clone(), b.clone()], Default::default()).unwrap();
    assert_eq!(plan.statistics().raw_terminals, 4);
    assert_eq!(plan.statistics().global_target_classes, 2);
    assert_eq!(plan.equations().len(), 1);
    assert_eq!(plan.remaining_terminals().len(), 1);
    assert!(plan.nonzero_conditions().is_empty());
    let c = a.0.coefficient_context();
    let expected = c
        .try_div(
            &c.try_sub(
                &c.parameter("d").unwrap(),
                &c.integer(2),
                Default::default(),
            )
            .unwrap(),
            &c.integer(2),
            Default::default(),
        )
        .unwrap();
    for (family, _) in [&a, &b] {
        let reduction = plan.apply(family, &key(&[2])).unwrap();
        assert_eq!(reduction.terms().len(), 1);
        let (output, value) = reduction.terms().first_key_value().unwrap();
        assert_eq!(output.integral(), &key(&[1]));
        assert_eq!(value, &expected);
        assert_eq!(
            plan.common_mass_squared_power(family, &key(&[2]), output)
                .unwrap(),
            -1
        );
        assert_eq!(
            plan.common_mass_squared_power(family, &key(&[1]), output)
                .unwrap(),
            0
        );
        assert!(reduction.nonzero_conditions().is_empty());
    }
    replay(&plan);
}

#[test]
fn diagonal_collection_regenerates_identically_from_cold_inputs_and_reversed_families() {
    let a = input(
        sunset("diagonal-sunset-a", false, false, false),
        &[&[1, 1, 1], &[2, 1, 1]],
    );
    let b = input(
        sunset("diagonal-sunset-b", true, false, false),
        &[&[1, 1, 1], &[1, 1, 2]],
    );
    let inputs = vec![a, b];
    let plan = VacuumDiagonalCollectionPlan::prepare(&inputs, Default::default()).unwrap();
    assert_eq!(plan.statistics().global_target_classes, 2);
    assert_eq!(plan.statistics().remaining_terminals, 1);
    let mut cold = Vec::new();
    for (family, raw) in &inputs {
        let session =
            TerminalRelationSession::new(Arc::clone(family), raw.clone(), 0, Default::default())
                .unwrap();
        let loaded = TerminalRelationSession::from_native_bytes(
            &session.to_native_bytes(Default::default()).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        cold.push((Arc::clone(loaded.family_owner()), raw.clone()));
    }
    cold.reverse();
    let regenerated = VacuumDiagonalCollectionPlan::prepare(&cold, Default::default()).unwrap();
    assert_eq!(plan.statistics(), regenerated.statistics());
    assert_eq!(
        plan.remaining_terminals(),
        regenerated.remaining_terminals()
    );
    for (family, raw) in &inputs {
        for key in raw {
            assert_eq!(
                plan.apply(family, key).unwrap().terms(),
                regenerated.apply(family, key).unwrap().terms()
            );
            assert_eq!(
                plan.apply(family, key).unwrap().nonzero_conditions(),
                regenerated.apply(family, key).unwrap().nonzero_conditions()
            );
            assert!(
                plan.apply(family, key)
                    .unwrap()
                    .terms()
                    .keys()
                    .all(|k| plan.remaining_terminals().contains(k))
            );
        }
    }
    assert!(
        plan.remaining_terminals()
            .iter()
            .all(|k| plan.raw_terminals().contains(k))
    );
    replay(&plan);
    replay(&regenerated);
}

#[test]
fn diagonal_collection_never_discards_unresolved_dot_children() {
    let a = input(sunset("diagonal-aux", false, false, false), &[&[1, 1, 1]]);
    let plan = VacuumDiagonalCollectionPlan::prepare(&[a.clone()], Default::default()).unwrap();
    assert!(plan.statistics().auxiliary_columns > 0);
    assert!(plan.equations().is_empty());
    assert_eq!(plan.remaining_terminals().len(), 1);
    assert_eq!(
        plan.apply(&a.0, &key(&[1, 1, 1]))
            .unwrap()
            .terms()
            .values()
            .next(),
        Some(&a.0.coefficient_context().one())
    );
}

#[test]
fn diagonal_collection_preserves_original_source_guards_even_after_cancellation() {
    let a = input(tadpole("diagonal-guard", true), &[&[1], &[2]]);
    let plan = VacuumDiagonalCollectionPlan::prepare(&[a.clone()], Default::default()).unwrap();
    let s = a.0.coefficient_context().parameter("s").unwrap();
    assert!(
        plan.sources()
            .iter()
            .flat_map(|s| s.rows())
            .flat_map(|r| &r.equation().nonzero_conditions)
            .any(|c| c == &s)
    );
    assert!(plan.nonzero_conditions().contains(&s));
    assert!(
        plan.apply(&a.0, &key(&[2]))
            .unwrap()
            .nonzero_conditions()
            .contains(&s)
    );
    replay(&plan);
}

#[test]
fn diagonal_collection_allows_massless_inactive_coordinates_but_rejects_unsupported_requests() {
    let family = sunset("diagonal-massless-isp", false, true, false);
    let plan = VacuumDiagonalCollectionPlan::prepare(
        &[input(Arc::clone(&family), &[&[1, 1, 0], &[2, 1, 0]])],
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.remaining_terminals().len(), 1);
    for raw in [&[1, 1, 1][..], &[-1, 1, 1][..]] {
        assert!(matches!(
            VacuumDiagonalCollectionPlan::prepare(
                &[input(Arc::clone(&family), &[raw])],
                Default::default()
            ),
            Err(VacuumCollectionError::Unsupported { .. })
        ));
    }
    let shifted = sunset("diagonal-shift", false, false, true);
    assert!(matches!(
        VacuumDiagonalCollectionPlan::prepare(&[input(shifted, &[&[1, 1, 1]])], Default::default()),
        Err(VacuumCollectionError::Unsupported { .. })
    ));
    assert!(matches!(
        plan.apply(&tadpole("foreign", false), &key(&[1])),
        Err(VacuumCollectionError::WrongFamily)
    ));
    assert!(matches!(
        plan.apply(&family, &key(&[1])),
        Err(VacuumCollectionError::WrongArity)
    ));
    assert!(matches!(
        plan.apply(&family, &key(&[3, 1, 0])),
        Err(VacuumCollectionError::UndeclaredTerminal)
    ));
}

#[test]
fn diagonal_collection_caps_fail_closed_and_empty_inventory_is_finite() {
    let a = input(tadpole("diagonal-caps", false), &[&[1], &[2]]);
    let defaults = VacuumDiagonalCollectionLimits::default();
    let limits = [
        VacuumDiagonalCollectionLimits {
            max_corner_seeds: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_source_rows: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_source_terms: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_columns: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_reducer_nonzeros: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_replay_operations: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_flat_map_terms: 0,
            ..defaults
        },
        VacuumDiagonalCollectionLimits {
            max_coefficient_terms: 0,
            ..defaults
        },
    ];
    for limit in limits {
        assert!(matches!(
            VacuumDiagonalCollectionPlan::prepare(&[a.clone()], limit),
            Err(VacuumCollectionError::Limit { .. })
        ));
    }
    let empty = VacuumDiagonalCollectionPlan::prepare(&[], defaults).unwrap();
    assert!(empty.raw_terminals().is_empty());
    assert!(empty.remaining_terminals().is_empty());
}
