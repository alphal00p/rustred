use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn tadpole() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "selective-refinement-feedback",
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

fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}

fn finish(session: &mut TerminalRelationSession, provider: &TerminalRelationFiniteProvider) {
    while !session.is_complete() {
        session
            .step_with_finite_provider(&AtomicBool::new(false), provider)
            .unwrap();
    }
}

fn assisted() -> (TerminalRelationSession, TerminalRelationFiniteProvider) {
    let mut session = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    session
        .enable_assistance("exact-selective-round-fixture".into())
        .unwrap();
    let provider = TerminalRelationFiniteProvider::new(Arc::new(BTreeMap::new()));
    finish(&mut session, &provider);
    (session, provider)
}

#[test]
fn reverse_and_transitive_native_incidence_excludes_unrelated_auxiliaries() {
    let mut session =
        TerminalRelationSession::new(tadpole(), BTreeSet::from([key(1)]), 0, Default::default())
            .unwrap();
    let one = session.family.coefficient_context().one();
    // Synthetic exact row-space fixture: the incidence diagnostic is not an
    // authority that would install these test equations as physical IBPs.
    for (left, right) in [(2, 1), (3, 2), (10, 11)] {
        session
            .add_row(BTreeMap::from([
                (key(left), one.clone()),
                (key(right), one.clone()),
            ]))
            .unwrap();
    }
    assert_eq!(
        session.terminal_relevant_auxiliary_keys().unwrap(),
        BTreeSet::from([key(2), key(3)])
    );
    assert!(
        session
            .terminal_relevant_auxiliary_keys_with_control(&AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(session.raw_terminals(), &BTreeSet::from([key(1)]));
}

#[test]
fn relevant_corner_feedback_discovers_a_real_new_relation_without_master_promotion() {
    let (mut session, provider) = assisted();
    assert_eq!(session.remaining_terminals().len(), 2);
    let initial_raw = session.raw_terminals().clone();
    let initial_seeds = session.source_seeds().to_vec();
    let frontier = session.terminal_relevant_auxiliary_keys().unwrap();
    assert!(frontier.contains(&key(2)));
    // Ordinary corner 2 bridges the two disconnected terminal recurrences.
    session
        .extend_source_seeds(&BTreeSet::from([key(2)]))
        .unwrap();
    let mut expected_seeds = initial_seeds;
    expected_seeds.push(key(2));
    assert_eq!(session.source_seeds(), expected_seeds);
    finish(&mut session, &provider);
    assert_eq!(session.remaining_terminals(), BTreeSet::from([key(1)]));
    assert_eq!(session.raw_terminals(), &initial_raw);
    assert!(!session.terminals.contains(&key(2)));
    assert_eq!(
        session
            .extend_source_seeds(&BTreeSet::from([key(2)]))
            .unwrap(),
        0
    );
}

#[test]
fn exact_residual_support_is_read_only_and_does_not_drop_unknown_columns() {
    let (session, _) = assisted();
    let c = session.family.coefficient_context();
    let existing = TerminalEquation {
        terms: session.basis_rows()[0].clone(),
        nonzero_conditions: Vec::new(),
    };
    let before = session.to_native_bytes(Default::default()).unwrap();
    assert!(
        session
            .authorized_equation_residual_support(&existing, &AtomicBool::new(false))
            .unwrap()
            .is_empty()
    );
    let unseen = TerminalEquation {
        terms: BTreeMap::from([(key(50), c.one()), (key(1), c.one())]),
        nonzero_conditions: Vec::new(),
    };
    let residual = session
        .authorized_equation_residual_support(&unseen, &AtomicBool::new(false))
        .unwrap();
    assert!(residual.contains(&key(50)));
    assert!(
        session
            .authorized_equation_residual_support(&unseen, &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);
}

#[test]
fn support_selection_is_actual_deterministic_and_attempted_only_per_rank_epoch() {
    let (session, _) = assisted();
    let none = BTreeSet::new();
    let selected = session
        .selective_auxiliary_source_seeds(&none, &AtomicBool::new(false))
        .unwrap();
    let support = Mask::try_from_indices(&[1]).unwrap();
    assert_eq!(selected, BTreeMap::from([(support.clone(), key(2))]));
    let attempted = BTreeSet::from([support]);
    assert!(
        session
            .selective_auxiliary_source_seeds(&attempted, &AtomicBool::new(false))
            .unwrap()
            .is_empty()
    );
    // Reopening the caller's epoch ledger, not U growth, permits a retry.
    assert_eq!(
        session
            .selective_auxiliary_source_seeds(&none, &AtomicBool::new(false))
            .unwrap(),
        selected
    );
    assert!(
        session
            .selective_auxiliary_source_seeds(
                &BTreeSet::from([Mask::try_from_indices(&[1, 0]).unwrap()]),
                &AtomicBool::new(false)
            )
            .is_err()
    );
}

#[test]
fn structural_cost_accounts_for_complete_signed_tuple_without_i64_min_overflow() {
    let make = |powers: &[i64]| IntegralKey::try_new(powers.iter().copied()).unwrap();
    let scalar = make(&[2, 1, 0]);
    let numerator = make(&[1, 1, -1]);
    assert!(structural_cost(&scalar) < structural_cost(&numerator));
    assert_eq!(structural_cost(&scalar).0, 3);
    assert_eq!(structural_cost(&numerator).1, 1);
    assert_eq!(structural_cost(&make(&[1, i64::MIN])).1, 1u128 << 63);
}

#[test]
fn batched_native_span_validation_preserves_session_and_rejects_new_relations() {
    let (session, _) = assisted();
    let existing: Vec<_> = session
        .basis_rows()
        .into_iter()
        .map(|terms| TerminalEquation {
            terms,
            nonzero_conditions: Vec::new(),
        })
        .collect();
    let before = session.to_native_bytes(Default::default()).unwrap();
    assert!(
        session
            .authorized_equations_are_satisfied(&existing, &AtomicBool::new(false))
            .unwrap()
    );
    assert!(
        session
            .authorized_equations_are_satisfied(&[], &AtomicBool::new(false))
            .unwrap()
    );
    let context = session.family.coefficient_context();
    let additional = TerminalEquation {
        terms: BTreeMap::from([(key(50), context.one()), (key(1), context.one())]),
        nonzero_conditions: Vec::new(),
    };
    let mut mixed = existing;
    // The duplicate reduces against the first new row inside the scratch
    // cohort, but that must not make the whole batch appear satisfied.
    mixed.extend([additional.clone(), additional]);
    assert!(
        !session
            .authorized_equations_are_satisfied(&mixed, &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);
}

#[test]
fn batched_native_span_validation_cancellation_and_invalid_guards_fail_closed() {
    let (session, _) = assisted();
    let before = session.to_native_bytes(Default::default()).unwrap();
    assert!(
        session
            .authorized_equations_are_satisfied(&[], &AtomicBool::new(true))
            .is_err()
    );
    let invalid = TerminalEquation {
        terms: BTreeMap::new(),
        nonzero_conditions: vec![session.family.coefficient_context().zero()],
    };
    assert!(
        session
            .authorized_equations_are_satisfied(&[invalid], &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);
}

#[test]
fn batched_span_validation_cold_paused_rebuild_keeps_trusted_pending_rows() {
    let (mut session, provider) = assisted();
    let previous_maps = session.apply_all_terminals().unwrap();
    let queries: Vec<_> = session
        .basis_rows()
        .into_iter()
        .map(|terms| TerminalEquation {
            terms,
            nonzero_conditions: Vec::new(),
        })
        .collect();
    assert!(queries.len() >= 2);
    session.normalize_generated().unwrap();
    // A valid interruption may occur after one trusted old basis row has
    // entered the new U while the remaining old rows still await rebuild.
    session.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    assert!(session.rebuild_cursor < session.rebuild.len());
    let checkpoint = session.to_native_bytes(Default::default()).unwrap();
    let mut resumed = TerminalRelationSession::from_native_bytes(
        &checkpoint,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(
        resumed
            .authorized_equations_are_satisfied(&queries, &AtomicBool::new(false))
            .unwrap()
    );
    let unseen = TerminalEquation {
        terms: BTreeMap::from([(key(50), resumed.family.coefficient_context().one())]),
        nonzero_conditions: Vec::new(),
    };
    assert!(
        !resumed
            .authorized_equations_are_satisfied(&[unseen], &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(
        resumed.to_native_bytes(Default::default()).unwrap(),
        checkpoint
    );
    finish(&mut resumed, &provider);
    assert_eq!(resumed.apply_all_terminals().unwrap(), previous_maps);
}

#[test]
fn appended_authorized_corner_rows_resume_with_guards_and_unchanged_provider() {
    let (mut session, provider) = assisted();
    let raw = session.raw_terminals().clone();
    let completed_sources = session.statistics().completed_source_rows;
    let (row, mut conditions) = session
        .sources
        .row(&key(2), 0, session.limits.algebra)
        .unwrap();
    let guard = session.family.coefficient_context().parameter("d").unwrap();
    conditions.push(guard.clone());
    let equation = TerminalEquation {
        terms: row,
        nonzero_conditions: conditions,
    };
    assert!(session.externally_queued_equations().is_none());
    assert_eq!(
        session
            .enqueue_authorized_equations(&[equation.clone(), equation.clone()])
            .unwrap(),
        2
    );
    assert!(!session.is_complete());
    assert_eq!(
        session.externally_queued_equations(),
        Some(([equation.clone(), equation.clone()].as_slice(), 0))
    );
    assert!(session.enqueue_authorized_equations(&[equation]).is_err());
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    let paused = resumed.to_native_bytes(Default::default()).unwrap();
    resumed
        .step_with_finite_provider(&AtomicBool::new(true), &provider)
        .unwrap();
    assert_eq!(resumed.to_native_bytes(Default::default()).unwrap(), paused);
    resumed
        .step_with_finite_provider(&AtomicBool::new(false), &provider)
        .unwrap();
    assert_eq!(resumed.assistance.as_ref().unwrap().equation_cursor, 1);
    assert_eq!(resumed.externally_queued_equations().unwrap().1, 1);
    assert_eq!(resumed.externally_queued_equations().unwrap().0.len(), 2);
    assert!(resumed.nonzero_conditions().contains(&guard));
    // Preserve an actually committed equation prefix, not just cursor zero.
    let prefix = resumed.to_native_bytes(Default::default()).unwrap();
    resumed =
        TerminalRelationSession::from_native_bytes(&prefix, Default::default(), Default::default())
            .unwrap();
    assert_eq!(resumed.assistance.as_ref().unwrap().equation_cursor, 1);
    finish(&mut session, &provider);
    finish(&mut resumed, &provider);
    assert!(resumed.externally_queued_equations().is_none());
    assert_eq!(resumed.remaining_terminals(), BTreeSet::from([key(1)]));
    assert_eq!(
        session.apply_all_terminals().unwrap(),
        resumed.apply_all_terminals().unwrap()
    );
    assert_eq!(resumed.raw_terminals(), &raw);
    assert_eq!(
        resumed.statistics().completed_source_rows,
        completed_sources
    );
    assert!(resumed.nonzero_conditions().contains(&guard));
    assert_eq!(
        resumed.assistance_binding(),
        Some("exact-selective-round-fixture")
    );
    assert!(
        resumed
            .assistance
            .as_ref()
            .unwrap()
            .finite_provider
            .as_ref()
            .unwrap()
            .rows()
            .is_empty()
    );
}

#[test]
fn invalid_or_unbound_append_leaves_native_state_unchanged() {
    let (mut session, _) = assisted();
    let before = session.to_native_bytes(Default::default()).unwrap();
    let c = session.family.coefficient_context().clone();
    let bad = TerminalEquation {
        terms: BTreeMap::from([(IntegralKey::try_new([1, 2]).unwrap(), c.one())]),
        nonzero_conditions: Vec::new(),
    };
    assert!(session.enqueue_authorized_equations(&[bad]).is_err());
    let bad_guard = TerminalEquation {
        terms: BTreeMap::new(),
        nonzero_conditions: vec![c.zero()],
    };
    assert!(session.enqueue_authorized_equations(&[bad_guard]).is_err());
    assert_eq!(session.to_native_bytes(Default::default()).unwrap(), before);
    let mut unbound =
        TerminalRelationSession::new(tadpole(), BTreeSet::new(), 0, Default::default()).unwrap();
    assert!(unbound.enqueue_authorized_equations(&[]).is_err());
}

#[test]
fn malformed_external_queue_markers_and_unowned_legacy_queues_are_rejected() {
    for retained in [false, true] {
        let (mut session, _) = assisted();
        if retained {
            session.retained_relations = Some(RetainedRelationsAuthority {
                binding: "fixture-inherited-source-authority".into(),
                protected_outputs: session.remaining_terminals(),
            });
        }
        // Schema 5/6 must actually carry a pending externally owned queue.
        session.assistance.as_mut().unwrap().externally_queued = true;
        let invalid = session.to_native_bytes(Default::default()).unwrap();
        assert!(
            TerminalRelationSession::from_native_bytes(
                &invalid,
                Default::default(),
                Default::default()
            )
            .is_err()
        );
        // Schema 2/4 cannot impersonate an externally owned queue when there
        // has never been a queried callback owner for the nonempty rows.
        let queued = session.assistance.as_mut().unwrap();
        queued.externally_queued = false;
        queued.equations.push(TerminalEquation {
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        });
        assert!(queued.queried.is_empty());
        let invalid = session.to_native_bytes(Default::default()).unwrap();
        assert!(
            TerminalRelationSession::from_native_bytes(
                &invalid,
                Default::default(),
                Default::default()
            )
            .is_err()
        );
    }
}
