//! Independent regressions for finite row-space feedback and its trust boundary.
use super::*;
use crate::family::AffineDenominator;
use crate::reduction::terminal_relations::TerminalRelationSession;
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

fn session(family: Arc<IntegralFamily>, powers: &[i64]) -> TerminalRelationSession {
    TerminalRelationSession::new(family, BTreeSet::from([key(powers)]), 0, Default::default())
        .unwrap()
}

fn finish(source: &mut TerminalRelationSession) {
    for _ in 0..1000 {
        if source.is_complete() {
            return;
        }
        source.step(&AtomicBool::new(false)).unwrap();
    }
    panic!("small finite audit fixture did not complete");
}

fn shared_auxiliary_sources() -> (TerminalRelationSession, TerminalRelationSession) {
    let mut first = session(tadpole("feedback-audit-one-dot"), &[1]);
    let mut second = session(tadpole("feedback-audit-three-dots"), &[3]);
    // All sources are generated before feedback. I2 is auxiliary in both
    // inventories, but matching it joins the two retained finite row spaces.
    second
        .extend_source_seeds(&BTreeSet::from([key(&[2])]))
        .unwrap();
    finish(&mut first);
    finish(&mut second);
    (first, second)
}

#[test]
fn repeated_feedback_is_byte_stable_and_cancellation_leaves_sources_untouched() {
    let (first, second) = shared_auxiliary_sources();
    let sessions = [&first, &second];
    let before = sessions.map(|session| session.statistics());
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(&sessions, Default::default())
        .unwrap();
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let repeated = plan
        .refine_with_finite_feedback(&sessions, Default::default())
        .unwrap();
    assert_eq!(bytes, repeated.to_native_bytes(Default::default()).unwrap());
    let result = plan.refine_with_finite_feedback_cancellable(
        &sessions,
        Default::default(),
        &AtomicBool::new(true),
    );
    assert!(matches!(result, Err(VacuumCollectionError::Cancelled)));
    assert_eq!(before, sessions.map(|session| session.statistics()));
}

#[test]
fn feedback_limits_are_separate_and_do_not_advance_retained_sources() {
    let (first, second) = shared_auxiliary_sources();
    let sessions = [&first, &second];
    let before = sessions.map(|session| session.statistics());
    let mut limits = TerminalCollectionLimits::default();
    limits.finite_feedback.max_source_rows = 0;
    assert!(TerminalCollectionPlan::prepare(&sessions, limits).is_ok());
    assert!(matches!(
        TerminalCollectionPlan::prepare_with_finite_feedback(&sessions, limits),
        Err(VacuumCollectionError::Limit { .. })
    ));
    assert_eq!(before, sessions.map(|session| session.statistics()));
}

#[test]
fn cross_family_auxiliary_feedback_finds_real_relation_without_new_sources() {
    let (first, second) = shared_auxiliary_sources();
    let sessions = [&first, &second];
    let before = sessions.map(|s| s.statistics().completed_source_rows);
    let baseline = TerminalCollectionPlan::prepare(&sessions, Default::default()).unwrap();
    assert_eq!(baseline.remaining_terminals().len(), 2);
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(&sessions, Default::default())
        .unwrap();
    assert_eq!(
        sessions.map(|s| s.statistics().completed_source_rows),
        before
    );
    assert_eq!(plan.remaining_terminals().len(), 1);
    let c = first.family_owner().coefficient_context();
    let d = c.parameter("d").unwrap();
    let coefficient = c
        .try_div(
            &c.try_mul(
                &c.try_sub(&d, &c.integer(2), Default::default()).unwrap(),
                &c.try_sub(&d, &c.integer(4), Default::default()).unwrap(),
                Default::default(),
            )
            .unwrap(),
            &c.integer(8),
            Default::default(),
        )
        .unwrap();
    let first_map = plan
        .apply(first.family_owner(), &key(&[1]))
        .unwrap()
        .terms();
    let second_map = plan
        .apply(second.family_owner(), &key(&[3]))
        .unwrap()
        .terms();
    assert_eq!(first_map.len(), 1);
    assert_eq!(second_map.len(), 1);
    let (output, first_factor) = first_map.first_key_value().unwrap();
    assert_eq!(
        second_map.get(output).unwrap(),
        &c.try_mul(first_factor, &coefficient, Default::default())
            .unwrap()
    );
    assert_eq!(
        plan.common_mass_squared_power(second.family_owner(), &key(&[3]), &output)
            .unwrap(),
        output.integral().powers()[0] - 3
    );
    let cold = TerminalCollectionPlan::from_native_bytes(
        &plan.to_native_bytes(Default::default()).unwrap(),
        &sessions,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    for source in sessions {
        for raw in source.raw_terminals() {
            assert_eq!(
                cold.apply(source.family_owner(), raw).unwrap().terms(),
                plan.apply(source.family_owner(), raw).unwrap().terms()
            );
        }
    }
}

#[test]
fn hidden_row_space_is_bound_even_if_old_terminal_maps_are_unchanged() {
    let (first, mut second) = shared_auxiliary_sources();
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(
        &[&first, &second],
        Default::default(),
    )
    .unwrap();
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let old_maps = second.apply_all_terminals().unwrap();
    // A different finite session can have the same raw/final terminal maps
    // while no longer proving the auxiliary identity needed by feedback.
    second.reducer = symbolica::tensors::sparse::SparseRowReducer::new(
        second.columns.len() as u32,
        symbolica::domains::rational_polynomial::RationalPolynomialField::new(
            symbolica::prelude::Z,
        ),
        symbolica::tensors::sparse::LuLMode::None,
    );
    assert_eq!(second.apply_all_terminals().unwrap(), old_maps);
    assert!(
        TerminalCollectionPlan::from_native_bytes(
            &bytes,
            &[&first, &second],
            Default::default(),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn finite_feedback_survives_scope_reindexing_without_advancing_sources() {
    let (mut first, second) = shared_auxiliary_sources();
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(
        &[&first, &second],
        Default::default(),
    )
    .unwrap();
    let before = first.statistics().completed_source_rows;
    let old = plan
        .apply(second.family_owner(), &key(&[3]))
        .unwrap()
        .terms()
        .clone();
    first.extend(&BTreeSet::from([key(&[2])]), 0).unwrap();
    while first.statistics().pending_rebuild_rows != 0 {
        first.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    }
    assert_eq!(first.statistics().completed_source_rows, before);
    let sessions = [&first, &second];
    let rebound = plan.rebind(&sessions, Default::default()).unwrap();
    assert_eq!(
        rebound
            .apply(second.family_owner(), &key(&[3]))
            .unwrap()
            .terms(),
        &old
    );
    let cold = TerminalCollectionPlan::from_native_bytes(
        &rebound.to_native_bytes(Default::default()).unwrap(),
        &sessions,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        cold.apply(second.family_owner(), &key(&[3]))
            .unwrap()
            .terms(),
        &old
    );
}

#[test]
fn cross_family_feedback_retains_explicit_predecessor_domain() {
    let (mut first, second) = shared_auxiliary_sources();
    let c = first.family_owner().coefficient_context();
    let guard = c
        .try_sub(
            &c.parameter("d").unwrap(),
            &c.integer(17),
            Default::default(),
        )
        .unwrap();
    first.conditions.push(guard.clone());
    let sessions = [&first, &second];
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(&sessions, Default::default())
        .unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &plan.to_native_bytes(Default::default()).unwrap(),
        &sessions,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    // A feedback equation that uses another family's retained row cannot
    // silently broaden the inherited equation's permitted domain.
    let first_row = cold.apply(first.family_owner(), &key(&[1])).unwrap();
    let second_row = cold.apply(second.family_owner(), &key(&[3])).unwrap();
    assert!(first_row.nonzero_conditions().contains(&guard));
    let second_identity = BTreeMap::from([(
        VacuumIntegralKey::from_family(second.family_owner(), key(&[3])),
        second.family_owner().coefficient_context().one(),
    )]);
    if second_row.terms() != &second_identity {
        assert!(second_row.nonzero_conditions().contains(&guard));
    }
}

#[test]
fn generated_numerator_auxiliaries_are_not_dropped_to_claim_a_zero_master() {
    let family = sunset("feedback-audit-numerator-auxiliary");
    let mut source = session(family.clone(), &[1, 1, 1]);
    source
        .extend_source_seeds(&BTreeSet::from([key(&[1, 1, 0])]))
        .unwrap();
    finish(&mut source);
    assert!(
        source
            .basis_rows()
            .iter()
            .flat_map(|r| r.keys())
            .any(|k| k.powers().iter().any(|&p| p < 0))
    );
    let original_rows = source.statistics().completed_source_rows;
    let plan = TerminalCollectionPlan::prepare_with_finite_feedback(&[&source], Default::default())
        .unwrap();
    assert_eq!(source.statistics().completed_source_rows, original_rows);
    assert_eq!(plan.remaining_terminals().len(), 1);
    assert_eq!(
        plan.apply(&family, &key(&[1, 1, 1])).unwrap().terms(),
        &BTreeMap::from([(
            VacuumIntegralKey::from_family(&family, key(&[1, 1, 1])),
            family.coefficient_context().one(),
        )])
    );
}

#[test]
fn forged_feedback_cannot_erase_current_source_guards_from_published_maps() {
    let (mut first, second) = shared_auxiliary_sources();
    let c = first.family_owner().coefficient_context();
    let guard = c
        .try_sub(
            &c.parameter("d").unwrap(),
            &c.integer(19),
            Default::default(),
        )
        .unwrap();
    first.conditions.push(guard.clone());
    let sessions = [&first, &second];
    let mut plan =
        TerminalCollectionPlan::prepare_with_finite_feedback(&sessions, Default::default())
            .unwrap();
    for feedback in &mut plan.feedbacks {
        let feedback = Arc::make_mut(feedback);
        for source in feedback.local.values_mut() {
            source.conditions.retain(|condition| condition != &guard);
        }
    }
    for rows in plan.reductions.values_mut() {
        for row in rows.values_mut() {
            row.conditions = Arc::new(
                row.conditions
                    .iter()
                    .filter(|condition| *condition != &guard)
                    .cloned()
                    .collect(),
            );
        }
    }
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    assert!(
        TerminalCollectionPlan::from_native_bytes(
            &bytes,
            &sessions,
            Default::default(),
            Default::default(),
        )
        .is_err()
    );
}
