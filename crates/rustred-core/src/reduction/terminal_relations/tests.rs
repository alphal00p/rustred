use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

mod source_seeds;

fn tadpole() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "resumable-finite-ibp",
            vec!["q".into()],
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
fn key(n: i64) -> IntegralKey {
    IntegralKey::try_new([n]).unwrap()
}
fn finish(s: &mut TerminalRelationSession) {
    for _ in 0..1000 {
        if s.is_complete() {
            return;
        }
        s.step(&AtomicBool::new(false)).unwrap();
    }
    panic!("finite test work did not drain");
}

#[test]
fn finite_tadpole_relations_are_exact_and_do_not_invent_masters() {
    let family = tadpole();
    let mut s = TerminalRelationSession::new(
        family.clone(),
        BTreeSet::from([key(1), key(2)]),
        0,
        Default::default(),
    )
    .unwrap();
    finish(&mut s);
    assert_eq!(s.statistics().remaining_terminals, 1);
    assert_eq!(s.remaining_terminals(), BTreeSet::from([key(1)]));
    let terms = s.apply_terminal(&key(2)).unwrap();
    let c = family.coefficient_context();
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
    assert_eq!(terms, BTreeMap::from([(key(1), expected)]));
    assert!(s.apply_terminal(&key(3)).is_err());
}

#[test]
fn resume_and_higher_scope_reuse_sources_and_rebuild_basis_safely() {
    let family = tadpole();
    let mut s = TerminalRelationSession::new(
        family.clone(),
        BTreeSet::from([key(1), key(2)]),
        0,
        Default::default(),
    )
    .unwrap();
    s.step(&AtomicBool::new(false)).unwrap();
    let saved = s.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&saved, Default::default(), Default::default())
            .unwrap();
    assert_eq!(s.statistics(), resumed.statistics());
    let before = resumed.statistics();
    resumed.step(&AtomicBool::new(true)).unwrap();
    assert_eq!(before, resumed.statistics());
    finish(&mut s);
    finish(&mut resumed);
    assert_eq!(s.terminal_rules(), resumed.terminal_rules());
    let completed = resumed.statistics().completed_source_rows;
    resumed.extend(&BTreeSet::from([key(3)]), 0).unwrap();
    assert_eq!(resumed.statistics().completed_source_rows, completed);
    // Serialize in the middle of column-role promotion/rebuild as well.
    let saved = resumed.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&saved, Default::default(), Default::default())
            .unwrap();
    finish(&mut resumed);
    let mut fresh = TerminalRelationSession::new(
        family,
        BTreeSet::from([key(1), key(2), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    finish(&mut fresh);
    assert_eq!(resumed.remaining_terminals(), fresh.remaining_terminals());
    assert_eq!(
        resumed.apply_terminal(&key(3)).unwrap(),
        fresh.apply_terminal(&key(3)).unwrap()
    );
    assert!(resumed.extend(&BTreeSet::new(), 0).is_ok());
}

#[test]
fn empty_inventory_and_seed_limits_are_explicit() {
    let s =
        TerminalRelationSession::new(tadpole(), BTreeSet::new(), 0, Default::default()).unwrap();
    assert!(s.is_complete());
    assert_eq!(s.statistics().remaining_terminals, 0);
    let saved = s.to_native_bytes(Default::default()).unwrap();
    assert!(
        TerminalRelationSession::from_native_bytes(&saved, Default::default(), Default::default())
            .unwrap()
            .is_complete()
    );
    let mut limits = TerminalRelationLimits::default();
    limits.max_seeds = 1;
    assert!(TerminalRelationSession::new(tadpole(), BTreeSet::from([key(1)]), 1, limits).is_err());
    let seed = sources::seeds(&BTreeSet::from([key(1)]), 1, 10).unwrap();
    assert_eq!(seed, vec![key(1), key(0), key(2)]);
}

#[test]
fn refused_scope_extension_keeps_old_checkpoint_usable() {
    let limits = TerminalRelationLimits {
        max_columns: 2,
        ..Default::default()
    };
    let mut s =
        TerminalRelationSession::new(tadpole(), BTreeSet::from([key(1), key(2)]), 0, limits)
            .unwrap();
    let before = s.statistics();
    assert!(s.extend(&BTreeSet::from([key(3)]), 0).is_err());
    assert_eq!(s.statistics(), before);
    let bytes = s.to_native_bytes(Default::default()).unwrap();
    let reopened =
        TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default()).unwrap();
    assert_eq!(reopened.statistics(), before);
}

#[test]
fn four_loop_circuit_dots_obey_exact_homogeneity_after_column_equivalence() {
    let loops = 4;
    let c = CoefficientContext::new(["d"]);
    let mut momenta: Vec<Vec<i64>> = (0..loops)
        .map(|i| (0..loops).map(|j| i64::from(i == j)).collect())
        .collect();
    momenta.push(vec![1; loops]);
    for i in 0..loops {
        for j in i + 1..loops {
            if (i, j) != (0, 1) {
                let mut q = vec![0; loops];
                q[i] = 1;
                q[j] = 1;
                momenta.push(q);
            }
        }
    }
    let denominators = momenta
        .iter()
        .map(|q| {
            AffineDenominator::new(
                c.integer(-1),
                (0..loops)
                    .flat_map(|i| (i..loops).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                    .map(|n| c.integer(n))
                    .collect(),
            )
        })
        .collect();
    let family = Arc::new(
        IntegralFamily::new(
            "finite-circuit-control",
            (0..loops).map(|i| format!("k{i}")).collect(),
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![c.zero(); momenta.len()],
        )
        .unwrap(),
    );
    let mut powers = vec![0; momenta.len()];
    powers[..loops + 1].fill(1);
    let base = IntegralKey::try_new(powers.clone()).unwrap();
    let mut raw = BTreeSet::from([base.clone()]);
    let mut dots = Vec::new();
    for slot in 0..loops + 1 {
        powers[slot] = 2;
        let dot = IntegralKey::try_new(powers.clone()).unwrap();
        // Only one dot belongs to the requested terminal block. The other
        // four must remain auxiliary until exact generated-column equivalence
        // binds their classes back to this original terminal.
        if slot == 0 {
            raw.insert(dot.clone());
        }
        dots.push(dot);
        powers[slot] = 1;
    }
    let mut session = TerminalRelationSession::new(family, raw, 0, Default::default()).unwrap();
    finish(&mut session);
    assert_eq!(
        session.remaining_terminals(),
        BTreeSet::from([base.clone()])
    );
    let expected = c
        .try_div(
            &c.try_sub(
                &c.try_mul(
                    &c.integer(2),
                    &c.parameter("d").unwrap(),
                    Default::default(),
                )
                .unwrap(),
                &c.integer(5),
                Default::default(),
            )
            .unwrap(),
            &c.integer(5),
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        session.apply_terminal(&dots[0]).unwrap(),
        BTreeMap::from([(base, expected)])
    );
    assert!(
        dots[1..]
            .iter()
            .any(|dot| session.aliases.get(dot) == Some(&dots[0]))
    );
    assert!(session.apply_terminal(&dots[1]).is_err());
}

#[test]
fn fill_admission_never_makes_a_checkpoint_exceed_its_own_nonzero_limit() {
    let family = tadpole();
    let one = family.coefficient_context().one();
    let limits = TerminalRelationLimits {
        max_nonzeros: 4,
        ..Default::default()
    };
    let mut s =
        TerminalRelationSession::new(family, BTreeSet::from([key(1), key(2), key(3)]), 0, limits)
            .unwrap();
    s.add_row(BTreeMap::from([
        (key(3), one.clone()),
        (key(2), one.clone()),
        (key(1), one.clone()),
    ]))
    .unwrap();
    let before = s.statistics();
    // Eliminating this one-entry input against the previous row would append
    // a two-entry U row and exceed 4 nonzeros. Input sparsity is not the bound.
    assert!(s.add_row(BTreeMap::from([(key(3), one)])).is_err());
    assert_eq!(s.statistics(), before);
    let bytes = s.to_native_bytes(Default::default()).unwrap();
    let loaded =
        TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default()).unwrap();
    assert_eq!(loaded.statistics(), before);
}

fn assisted_tadpole() -> TerminalRelationSession {
    let mut session = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    session
        .enable_assistance("fixed-test-authority".into())
        .unwrap();
    session
}

fn saved_middle_equations(family: &IntegralFamily) -> Vec<TerminalEquation> {
    // The saved middle recurrence is absent from the depth-zero ordinary
    // worklist, but closes its gap between the two requested terminals.
    let (terms, mut conditions) = sources::Sources::new(family)
        .unwrap()
        .row(&key(2), 0, Default::default())
        .unwrap();
    conditions.push(family.coefficient_context().parameter("d").unwrap());
    let equation = TerminalEquation {
        terms,
        nonzero_conditions: conditions,
    };
    vec![equation.clone(), equation]
}

#[test]
fn rebuild_only_preserves_assisted_queues_and_never_advances_sources() {
    let mut session = assisted_tadpole();
    let equations = saved_middle_equations(session.family_owner());
    while !session.is_complete() {
        session
            .step_with_provider(&AtomicBool::new(false), |target| {
                Ok(if target == &key(2) {
                    equations.clone()
                } else {
                    vec![]
                })
            })
            .unwrap();
    }
    let application = session.apply_terminal(&key(3)).unwrap();
    let conditions = session.nonzero_conditions().to_vec();
    session.extend(&BTreeSet::from([key(5)]), 0).unwrap();
    let before = session.statistics();
    let assistance = session.assistance.as_ref().unwrap();
    let pending_before = assistance.pending.clone();
    let queried_before = assistance.queried.clone();
    let equations_before = assistance.equations.clone();
    let equation_cursor_before = assistance.equation_cursor;
    let rows_before = assistance.completed_rows;
    assert!(before.pending_rebuild_rows > 0);
    session.step_rebuild_only(&AtomicBool::new(true)).unwrap();
    assert_eq!(session.statistics(), before);
    while session.statistics().pending_rebuild_rows > 0 {
        session.step_rebuild_only(&AtomicBool::new(false)).unwrap();
        session = TerminalRelationSession::from_native_bytes(
            &session.to_native_bytes(Default::default()).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    }
    assert_eq!(
        session.statistics().completed_source_rows,
        before.completed_source_rows
    );
    let after = session.assistance.as_ref().unwrap();
    assert_eq!(after.pending, pending_before);
    assert_eq!(after.queried, queried_before);
    assert_eq!(after.equations, equations_before);
    assert_eq!(after.equation_cursor, equation_cursor_before);
    assert_eq!(after.completed_rows, rows_before);
    assert_eq!(session.nonzero_conditions(), conditions);
    assert_eq!(session.apply_terminal(&key(3)).unwrap(), application);
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    session.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), bytes);
}

#[test]
fn rebuild_only_preserves_a_partially_consumed_provider_batch() {
    let mut session = assisted_tadpole();
    let equations = saved_middle_equations(session.family_owner());
    for _ in 0..100 {
        session
            .step_with_provider(&AtomicBool::new(false), |target| {
                Ok(if target == &key(2) {
                    equations.clone()
                } else {
                    vec![]
                })
            })
            .unwrap();
        if session.statistics().pending_assistance_rows == 2 {
            break;
        }
    }
    assert_eq!(session.statistics().pending_assistance_rows, 2);
    session
        .step_with_provider(&AtomicBool::new(false), |_| {
            panic!("batch consumption must not query provider")
        })
        .unwrap();
    assert_eq!(session.statistics().pending_assistance_rows, 1);
    session.extend(&BTreeSet::from([key(5)]), 0).unwrap();
    let before = session.statistics();
    let assistance = session.assistance.as_ref().unwrap();
    assert_eq!(assistance.equation_cursor, 1);
    let equations_before = assistance.equations.clone();
    let queried_before = assistance.queried.clone();
    let pending_before = assistance.pending.clone();
    let guards = session.nonzero_conditions().to_vec();
    assert!(before.pending_rebuild_rows > 0);
    while session.statistics().pending_rebuild_rows > 0 {
        session.step_rebuild_only(&AtomicBool::new(false)).unwrap();
        session = TerminalRelationSession::from_native_bytes(
            &session.to_native_bytes(Default::default()).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    }
    let after = session.assistance.as_ref().unwrap();
    assert_eq!(after.equation_cursor, 1);
    assert_eq!(after.equations, equations_before);
    assert_eq!(after.pending, pending_before);
    assert_eq!(after.queried, queried_before);
    assert_eq!(
        session.statistics().completed_source_rows,
        before.completed_source_rows
    );
    assert_eq!(
        session.statistics().completed_assistance_rows,
        before.completed_assistance_rows
    );
    assert_eq!(session.nonzero_conditions(), guards);
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    session.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), bytes);
}

#[test]
fn assisted_exact_rows_match_larger_ordinary_reference_and_reuse_keys() {
    let mut assisted = assisted_tadpole();
    let equations = saved_middle_equations(assisted.family_owner());
    let mut queries = Vec::new();
    for _ in 0..1000 {
        if assisted.is_complete() {
            break;
        }
        assisted
            .step_with_provider(&AtomicBool::new(false), |target| {
                queries.push(target.clone());
                Ok(if target == &key(2) {
                    equations.clone()
                } else {
                    vec![]
                })
            })
            .unwrap();
    }
    assert!(assisted.is_complete());
    assert_eq!(queries.len(), queries.iter().collect::<BTreeSet<_>>().len());
    assert_eq!(
        queries.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([key(1), key(2), key(3), key(4)])
    );
    assert_eq!(assisted.statistics().completed_assistance_rows, 2);
    assert!(
        assisted.nonzero_conditions().contains(
            &assisted
                .family_owner()
                .coefficient_context()
                .parameter("d")
                .unwrap()
        )
    );
    let mut reference = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        1,
        Default::default(),
    )
    .unwrap();
    finish(&mut reference);
    assert_eq!(
        assisted.remaining_terminals(),
        reference.remaining_terminals()
    );
    assert_eq!(
        assisted.apply_terminal(&key(3)).unwrap(),
        reference.apply_terminal(&key(3)).unwrap()
    );

    let mut ordinary = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    finish(&mut ordinary);
    assert_eq!(ordinary.statistics().remaining_terminals, 2);
    assert_eq!(assisted.statistics().remaining_terminals, 1);
    assert_eq!(
        ordinary.statistics().completed_source_rows,
        assisted.statistics().completed_source_rows
    );
}

#[test]
fn assistance_children_stay_unresolved_and_do_not_expand_provider_work() {
    let family = tadpole();
    let c = family.coefficient_context();
    let equation = TerminalEquation {
        terms: BTreeMap::from([(key(1), c.one()), (key(99), c.integer(-1))]),
        nonzero_conditions: vec![],
    };
    let mut session =
        TerminalRelationSession::new(family, BTreeSet::from([key(1)]), 0, Default::default())
            .unwrap();
    session.enable_assistance("finite-support".into()).unwrap();
    let mut queries = Vec::new();
    for _ in 0..100 {
        if session.is_complete() {
            break;
        }
        session
            .step_with_provider(&AtomicBool::new(false), |target| {
                queries.push(target.clone());
                assert_ne!(target, &key(99));
                Ok(if target == &key(1) {
                    vec![equation.clone()]
                } else {
                    vec![]
                })
            })
            .unwrap();
    }
    assert!(session.is_complete());
    assert_eq!(queries, vec![key(1), key(2)]);
    assert!(session.columns.contains(&key(99)));
    assert_eq!(session.remaining_terminals(), BTreeSet::from([key(1)]));
    assert_eq!(
        session.apply_terminal(&key(1)).unwrap(),
        BTreeMap::from([(key(1), session.family_owner().coefficient_context().one())])
    );
}

#[test]
fn assistance_checkpoint_at_every_boundary_preserves_requests_rows_and_conditions() {
    let mut continuous = assisted_tadpole();
    let mut resumed = assisted_tadpole();
    let equations = saved_middle_equations(continuous.family_owner());
    let mut continuous_queries = Vec::new();
    let mut resumed_queries = Vec::new();
    for _ in 0..1000 {
        if continuous.is_complete() {
            break;
        }
        continuous
            .step_with_provider(&AtomicBool::new(false), |target| {
                continuous_queries.push(target.clone());
                Ok(if target == &key(2) {
                    equations.clone()
                } else {
                    vec![]
                })
            })
            .unwrap();
        let before = resumed.statistics();
        resumed
            .step_with_provider(&AtomicBool::new(true), |_| {
                panic!("cancelled provider called")
            })
            .unwrap();
        assert_eq!(before, resumed.statistics());
        resumed
            .step_with_provider(&AtomicBool::new(false), |target| {
                resumed_queries.push(target.clone());
                Ok(if target == &key(2) {
                    equations.clone()
                } else {
                    vec![]
                })
            })
            .unwrap();
        let bytes = resumed.to_native_bytes(Default::default()).unwrap();
        resumed = TerminalRelationSession::from_native_bytes(
            &bytes,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        resumed
            .enable_assistance("fixed-test-authority".into())
            .unwrap();
        assert!(
            resumed
                .enable_assistance("changed-authority".into())
                .is_err()
        );
        assert_eq!(continuous.statistics(), resumed.statistics());
        assert_eq!(continuous.basis_rows(), resumed.basis_rows());
        assert_eq!(
            continuous.nonzero_conditions(),
            resumed.nonzero_conditions()
        );
    }
    assert!(continuous.is_complete() && resumed.is_complete());
    assert_eq!(continuous_queries, resumed_queries);
    assert_eq!(continuous.terminal_rules(), resumed.terminal_rules());
    assert!(resumed.step(&AtomicBool::new(false)).is_err());
}

#[test]
fn assistance_validation_provider_and_admission_errors_preserve_retry_boundaries() {
    let mut session = assisted_tadpole();
    let before = session.statistics();
    assert!(
        session
            .step_with_provider(&AtomicBool::new(false), |_| Err(invalid(
                "provider failure"
            )))
            .is_err()
    );
    assert_eq!(before, session.statistics());
    let c = session.family_owner().coefficient_context();
    let invalid_equation = TerminalEquation {
        terms: BTreeMap::from([(key(1), c.one())]),
        nonzero_conditions: vec![c.zero()],
    };
    assert!(
        session
            .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![
                invalid_equation.clone()
            ]))
            .is_err()
    );
    assert_eq!(before, session.statistics());

    let family = tadpole();
    let equation = TerminalEquation {
        terms: BTreeMap::from([
            (key(1), family.coefficient_context().one()),
            (key(99), family.coefficient_context().integer(-1)),
        ]),
        nonzero_conditions: vec![],
    };
    let limits = TerminalRelationLimits {
        max_columns: 1,
        ..Default::default()
    };
    let mut session =
        TerminalRelationSession::new(family, BTreeSet::from([key(1)]), 0, limits).unwrap();
    session.enable_assistance("column-limit".into()).unwrap();
    session
        .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![equation.clone()]))
        .unwrap();
    let before = session.statistics();
    assert!(
        session
            .step_with_provider(&AtomicBool::new(false), |_| panic!(
                "cached provider called again"
            ))
            .is_err()
    );
    assert_eq!(before, session.statistics());
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default()).unwrap();
    assert_eq!(before, resumed.statistics());
    assert!(
        resumed
            .step_with_provider(&AtomicBool::new(false), |_| panic!(
                "cached provider called again"
            ))
            .is_err()
    );
    assert_eq!(before, resumed.statistics());
}

#[test]
fn old_unassisted_checkpoint_remains_v1_and_can_bind_before_sources() {
    use crate::persistence::{SectionTag, inspect_program};
    let mut session =
        TerminalRelationSession::new(tadpole(), BTreeSet::from([key(1)]), 0, Default::default())
            .unwrap();
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    assert_eq!(envelope.section(SectionTag::PROGRAM).unwrap()[0], 1);
    let mut loaded =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    loaded.enable_assistance("v1-upgrade".into()).unwrap();
    let bytes = loaded.to_native_bytes(Default::default()).unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    assert_eq!(envelope.section(SectionTag::PROGRAM).unwrap()[0], 2);
    session.step(&AtomicBool::new(false)).unwrap();
    assert!(session.enable_assistance("too-late".into()).is_err());
}

#[test]
fn empty_provider_preserves_ordinary_rows_and_assisted_extension_reuses_queries() {
    let mut ordinary = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    finish(&mut ordinary);
    let mut assisted = assisted_tadpole();
    let mut queries = Vec::new();
    for _ in 0..1000 {
        if assisted.is_complete() {
            break;
        }
        assisted
            .step_with_provider(&AtomicBool::new(false), |target| {
                queries.push(target.clone());
                Ok(vec![])
            })
            .unwrap();
    }
    assert!(assisted.is_complete());
    assert_eq!(assisted.basis_rows(), ordinary.basis_rows());
    assert_eq!(assisted.terminal_rules(), ordinary.terminal_rules());
    let completed = assisted.statistics().completed_source_rows;
    // I(2) was already queried as ordinary support; promoting it to a target
    // must not request its equations again after the persisted basis rebuild.
    assisted.extend(&BTreeSet::from([key(2)]), 1).unwrap();
    assert_eq!(assisted.statistics().completed_source_rows, completed);
    let bytes = assisted.to_native_bytes(Default::default()).unwrap();
    assisted =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    for _ in 0..1000 {
        if assisted.is_complete() {
            break;
        }
        assisted
            .step_with_provider(&AtomicBool::new(false), |target| {
                queries.push(target.clone());
                Ok(vec![])
            })
            .unwrap();
    }
    assert!(assisted.is_complete());
    assert_eq!(queries.len(), queries.iter().collect::<BTreeSet<_>>().len());
    let mut reference = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(2), key(3)]),
        1,
        Default::default(),
    )
    .unwrap();
    finish(&mut reference);
    for target in [key(1), key(2), key(3)] {
        assert_eq!(
            assisted.apply_terminal(&target).unwrap(),
            reference.apply_terminal(&target).unwrap()
        );
    }
}
