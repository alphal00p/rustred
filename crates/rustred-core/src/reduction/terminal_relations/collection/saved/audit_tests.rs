//! Independent adversarial tests of the saved-session composition boundary.
use super::*;
use crate::family::AffineDenominator;
use crate::reduction::terminal_relations::TerminalEquation;
use std::sync::atomic::AtomicBool;

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn tadpole(name: &str) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![AffineDenominator::new(c.integer(-1), vec![c.one()])],
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    )
}

fn sunset(name: &str) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        c.integer(-1),
                        row.into_iter().map(|n| c.integer(n)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![c.zero(); 3],
        )
        .unwrap(),
    )
}

fn session(family: Arc<IntegralFamily>, keys: &[&[i64]]) -> TerminalRelationSession {
    TerminalRelationSession::new(
        family,
        keys.iter().map(|p| key(p)).collect(),
        0,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn explicit_predecessor_guard_survives_cross_family_composition_and_cold_load() {
    let family = tadpole("saved-collection-audit-guard");
    let c = family.coefficient_context();
    let guard = c
        .try_sub(
            &c.parameter("d").unwrap(),
            &c.integer(7),
            Default::default(),
        )
        .unwrap();
    let leading = c
        .try_sub(
            &c.parameter("d").unwrap(),
            &c.integer(2),
            Default::default(),
        )
        .unwrap();
    let mut source = session(family.clone(), &[&[1], &[2]]);
    source
        .enable_assistance("audit-explicit-provider-domain".into())
        .unwrap();
    for _ in 0..100 {
        if source.is_complete() {
            break;
        }
        source
            .step_with_provider(&AtomicBool::new(false), |target| {
                Ok(if target == &key(&[1]) {
                    vec![TerminalEquation {
                        terms: BTreeMap::from([
                            (key(&[1]), leading.clone()),
                            (key(&[2]), c.integer(-2)),
                        ]),
                        nonzero_conditions: vec![guard.clone()],
                    }]
                } else {
                    vec![]
                })
            })
            .unwrap();
    }
    assert!(source.is_complete());
    let peer = session(tadpole("saved-collection-audit-peer"), &[&[1]]);
    let sessions = [&source, &peer];
    let plan = TerminalCollectionPlan::prepare(&sessions, Default::default()).unwrap();
    assert!(
        plan.apply(&family, &key(&[2]))
            .unwrap()
            .nonzero_conditions()
            .contains(&guard)
    );
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &bytes,
        &sessions,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        cold.apply(&family, &key(&[2])).unwrap().terms(),
        plan.apply(&family, &key(&[2])).unwrap().terms()
    );
    assert!(
        cold.apply(&family, &key(&[2]))
            .unwrap()
            .nonzero_conditions()
            .contains(&guard)
    );
    assert_eq!(
        cold.predecessors[family.fingerprint()]
            .assistance_binding
            .as_deref(),
        Some("audit-explicit-provider-domain")
    );
}

#[test]
fn unresolved_numerator_survives_scalar_collection_and_outputs_are_declared() {
    let family = sunset("saved-collection-audit-numerator");
    let source = session(family.clone(), &[&[1, 1, 1], &[2, 1, 1], &[1, 1, -2]]);
    let plan = TerminalCollectionPlan::prepare(&[&source], Default::default()).unwrap();
    assert_eq!(plan.statistics().passthrough_terminals, 1);
    let numerator = key(&[1, 1, -2]);
    let row = plan.apply(&family, &numerator).unwrap();
    assert_eq!(
        row.terms(),
        &BTreeMap::from([(
            VacuumIntegralKey::from_family(&family, numerator.clone()),
            family.coefficient_context().one()
        )])
    );
    assert!(
        plan.remaining_terminals()
            .contains(&VacuumIntegralKey::from_family(&family, numerator))
    );
    for target in source.raw_terminals() {
        assert!(
            plan.apply(&family, target)
                .unwrap()
                .terms()
                .keys()
                .all(|output| plan.raw_terminals().contains(output))
        );
    }
    assert!(matches!(
        plan.apply(&sunset("saved-collection-audit-foreign"), &key(&[1, 1, 1])),
        Err(VacuumCollectionError::WrongFamily)
    ));
    assert!(matches!(
        plan.apply(&family, &key(&[1, 2, 1])),
        Err(VacuumCollectionError::UndeclaredTerminal)
    ));
}

#[test]
fn external_passthrough_does_not_claim_sole_mass_homogeneity() {
    let c = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "saved-collection-audit-external",
            vec!["k".into()],
            vec!["p".into()],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero()]),
                AffineDenominator::new(c.zero(), vec![c.one(), c.integer(2)]),
            ],
            vec![vec![c.one()]],
            vec![c.zero(); 2],
        )
        .unwrap(),
    );
    let source = session(family.clone(), &[&[1, 1]]);
    let vacuum = session(
        tadpole("saved-collection-audit-mixed-vacuum"),
        &[&[1], &[2]],
    );
    let plan = TerminalCollectionPlan::prepare(&[&source, &vacuum], Default::default()).unwrap();
    let target = key(&[1, 1]);
    let output = VacuumIntegralKey::from_family(&family, target.clone());
    assert_eq!(
        plan.apply(&family, &target).unwrap().terms(),
        &BTreeMap::from([(output.clone(), c.one())])
    );
    assert_eq!(plan.statistics().collection_groups, 1);
    assert!(matches!(
        plan.common_mass_squared_power(&family, &target, &output),
        Err(VacuumCollectionError::Unsupported { .. })
    ));
}

#[test]
fn native_decode_rejects_forged_map_even_with_valid_native_atom_encoding() {
    let family = tadpole("saved-collection-audit-mutation");
    let source = session(family.clone(), &[&[1], &[2]]);
    let mut plan = TerminalCollectionPlan::prepare(&[&source], Default::default()).unwrap();
    let row = plan
        .reductions
        .get_mut(family.fingerprint())
        .unwrap()
        .get_mut(&key(&[2]))
        .unwrap();
    *row.terms.values_mut().next().unwrap() = family.coefficient_context().integer(123);
    let forged = plan.to_native_bytes(Default::default()).unwrap();
    assert!(
        TerminalCollectionPlan::from_native_bytes(
            &forged,
            &[&source],
            Default::default(),
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn native_decode_binds_inherited_source_conditions_not_only_family_and_raw_keys() {
    let family = tadpole("saved-collection-audit-source-binding");
    let mut source = session(family.clone(), &[&[1], &[2]]);
    let plan = TerminalCollectionPlan::prepare(&[&source], Default::default()).unwrap();
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let c = family.coefficient_context();
    source.conditions.push(
        c.try_sub(
            &c.parameter("d").unwrap(),
            &c.integer(9),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(
        TerminalCollectionPlan::from_native_bytes(
            &bytes,
            &[&source],
            Default::default(),
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn numerator_mass_is_part_of_sole_mass_homogeneity_admission() {
    for numerator_case in [0, 1, 2] {
        let c = CoefficientContext::new(["d", "mu"]);
        let numerator_constant = match numerator_case {
            0 => c.zero(),
            1 => c.integer(-1),
            _ => c.parameter("mu").unwrap(),
        };
        let family = Arc::new(
            IntegralFamily::new(
                format!("saved-collection-audit-numerator-mass-{numerator_case}"),
                vec!["k1".into(), "k2".into()],
                vec![],
                c.clone(),
                c.parameter("d").unwrap(),
                [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                    .into_iter()
                    .enumerate()
                    .map(|(axis, row)| {
                        AffineDenominator::new(
                            if axis == 2 {
                                numerator_constant.clone()
                            } else {
                                c.integer(-1)
                            },
                            row.into_iter().map(|n| c.integer(n)).collect(),
                        )
                    })
                    .collect(),
                vec![],
                vec![c.zero(); 3],
            )
            .unwrap(),
        );
        let source = session(family.clone(), &[&[1, 1, -2]]);
        let plan = TerminalCollectionPlan::prepare(&[&source], Default::default()).unwrap();
        let target = key(&[1, 1, -2]);
        let output = VacuumIntegralKey::from_family(&family, target.clone());
        assert_eq!(
            plan.apply(&family, &target).unwrap().terms(),
            &BTreeMap::from([(output.clone(), c.one())])
        );
        let power = plan.common_mass_squared_power(&family, &target, &output);
        if numerator_case == 2 {
            assert!(matches!(
                power,
                Err(VacuumCollectionError::Unsupported { .. })
            ));
        } else {
            assert_eq!(power.unwrap(), 0);
        }
    }
}

#[test]
fn retained_old_output_names_are_identity_terminals_after_a_local_basis_change() {
    let mut a = session(tadpole("saved-collection-audit-alternative-a"), &[&[2]]);
    let mut b = session(tadpole("saved-collection-audit-alternative-b"), &[&[2]]);
    let old = TerminalCollectionPlan::prepare(&[&a, &b], Default::default()).unwrap();
    assert_eq!(old.remaining_terminals().len(), 1);
    let retained = old.remaining_terminals().first().unwrap();

    // The first collection aliases A(2) to B(2), with B selected canonically.
    // Now enlarge B alone to include B(1), and let native ordinary sources
    // reduce B(2)=(d-2)/2*B(1). Retaining the old B(2) name for A must not
    // simultaneously advertise B(2) as a recursively reducible final master.
    let changed = if a.family_owner().fingerprint() == retained.family_fingerprint() {
        &mut a
    } else {
        &mut b
    };
    changed.extend(&BTreeSet::from([key(&[1])]), 0).unwrap();
    for _ in 0..100 {
        if changed.is_complete() {
            break;
        }
        changed.step(&AtomicBool::new(false)).unwrap();
    }
    assert!(changed.is_complete());
    assert_eq!(changed.remaining_terminals(), BTreeSet::from([key(&[1])]));
    let sources = [&a, &b];
    let rebound = old.rebind(&sources, Default::default()).unwrap();
    assert_eq!(rebound.proofs().len(), old.proofs().len());
    for output in rebound.remaining_terminals() {
        if rebound.raw_terminals().contains(output) {
            let family = rebound.family(output.family_fingerprint()).unwrap();
            assert_eq!(
                rebound.apply(family, output.integral()).unwrap().terms(),
                &BTreeMap::from([(output.clone(), family.coefficient_context().one())]),
            );
        }
    }
    let bytes = rebound.to_native_bytes(Default::default()).unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &bytes,
        &sources,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(cold.remaining_terminals(), rebound.remaining_terminals());
    for source in sources {
        for input in source.raw_terminals() {
            assert_eq!(
                cold.apply(source.family_owner(), input).unwrap().terms(),
                rebound.apply(source.family_owner(), input).unwrap().terms()
            );
        }
    }
}
