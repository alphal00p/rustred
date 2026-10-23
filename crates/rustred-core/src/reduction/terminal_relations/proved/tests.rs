use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::persistence::BinaryIoLimits;

fn tadpole() -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "fresh-proved-terminal-relations",
            vec!["q".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap(),
    )
}

fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}

fn reductions(
    maps: &BTreeMap<IntegralKey, TerminalRelationRow>,
    conditions: &[Coefficient],
) -> Vec<ProvedTerminalReduction> {
    maps.iter()
        .map(|(target, terms)| {
            ProvedTerminalReduction::new(target.clone(), terms.clone(), conditions.to_vec())
        })
        .collect()
}

#[test]
fn fresh_import_retains_only_exact_maps_guards_and_requested_source_geometry() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    let mut predecessor =
        TerminalRelationSession::new(Arc::clone(&family), raw.clone(), 2, Default::default())
            .unwrap();

    // One proved terminal relation plus unrelated historical auxiliary state.
    predecessor
        .add_row(BTreeMap::from([
            (key(2), context.one()),
            (key(1), context.integer(-2)),
        ]))
        .unwrap();
    predecessor
        .add_row(BTreeMap::from([
            (key(9), context.one()),
            (key(8), context.one()),
        ]))
        .unwrap();
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    predecessor.conditions.push(guard.clone());
    assert!(predecessor.statistics().auxiliary_columns > 0);
    let maps = predecessor.apply_all_terminals().unwrap();

    let fresh = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(&family),
        raw,
        3,
        "authenticated-predecessor-v1:abc".into(),
        reductions(&maps, std::slice::from_ref(&guard)),
        Default::default(),
    )
    .unwrap();
    let statistics = fresh.statistics();
    assert_eq!(statistics.seed_depth, 3);
    assert_eq!(statistics.completed_seeds, 0);
    assert_eq!(statistics.completed_source_rows, 0);
    assert_eq!(statistics.pending_rebuild_rows, 0);
    assert_eq!(statistics.auxiliary_columns, 0);
    assert_eq!(fresh.apply_all_terminals().unwrap(), maps);
    assert_eq!(fresh.nonzero_conditions(), &[guard]);
    assert_eq!(
        fresh.retained_relations_binding(),
        Some("authenticated-predecessor-v1:abc")
    );

    // A relations-only restart may also deliberately request a shallower
    // ordinary-source shell. It must match a genuinely blank depth-one source
    // inventory, not retain the predecessor's depth-two seeds.
    let shallower = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(&family),
        fresh.raw_terminals().clone(),
        1,
        "authenticated-predecessor-v1:shallower".into(),
        reductions(&maps, fresh.nonzero_conditions()),
        Default::default(),
    )
    .unwrap();
    let blank = TerminalRelationSession::new(
        family,
        shallower.raw_terminals().clone(),
        1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(shallower.seeds, blank.seeds);
    assert_ne!(shallower.seeds, predecessor.seeds);
}

#[test]
fn each_fresh_round_uses_its_own_requested_containing_source_set() {
    let family = tadpole();
    let raw = BTreeSet::from([key(-4)]);
    let mut baseline =
        TerminalRelationSession::new(Arc::clone(&family), raw.clone(), 2, Default::default())
            .unwrap();
    // This one-loop fixture has one promotable coordinate. The genuine
    // two-loop CLI acceptance separately covers the old (2,2) shape.
    baseline.extend_containing_sector_seeds(1).unwrap();
    let maps = baseline.apply_all_terminals().unwrap();

    let mut depth_three_with_containing = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(&family),
        raw.clone(),
        3,
        "round-3-1".into(),
        reductions(&maps, &[]),
        Default::default(),
    )
    .unwrap();
    assert!(!depth_three_with_containing.seeds.contains(&key(1)));
    assert_eq!(
        depth_three_with_containing
            .extend_containing_sector_seeds(1)
            .unwrap(),
        1
    );
    assert!(depth_three_with_containing.seeds.contains(&key(1)));

    let depth_three_without_containing = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        raw,
        3,
        "round-3-0".into(),
        reductions(&maps, &[]),
        Default::default(),
    )
    .unwrap();
    assert!(!depth_three_without_containing.seeds.contains(&key(1)));
    assert_eq!(
        depth_three_without_containing
            .statistics()
            .completed_source_rows,
        0
    );
    assert_eq!(
        depth_three_without_containing
            .apply_all_terminals()
            .unwrap(),
        maps
    );
}

#[test]
fn retained_relation_binding_maps_and_guards_survive_cold_roundtrip() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    let maps = BTreeMap::from([
        (key(1), BTreeMap::from([(key(1), context.one())])),
        (key(2), BTreeMap::from([(key(1), context.integer(3))])),
    ]);
    let guard = context
        .try_add(
            &context.parameter("d").unwrap(),
            &context.integer(101),
            Default::default(),
        )
        .unwrap();
    let fresh = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        raw,
        3,
        "cold-binding".into(),
        reductions(&maps, std::slice::from_ref(&guard)),
        Default::default(),
    )
    .unwrap();
    let bytes = fresh.to_native_bytes(Default::default()).unwrap();
    let restored =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    assert_eq!(restored.retained_relations_binding(), Some("cold-binding"));
    assert_eq!(restored.apply_all_terminals().unwrap(), maps);
    assert_eq!(restored.nonzero_conditions(), &[guard]);
    assert_eq!(restored.statistics(), fresh.statistics());
    restored.validate_retained_relations(&fresh).unwrap();

    let tight = BinaryIoLimits {
        max_collection_entries: "cold-binding".len() - 1,
        ..Default::default()
    };
    assert!(fresh.to_native_bytes(tight).is_err());
}

#[test]
fn imported_nonminimal_output_basis_overrides_default_terminal_pivot_preference() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    // The ordinary harder-first order prefers eliminating key(2). This exact
    // predecessor instead retained key(2), so key(1) must pivot first.
    let maps = BTreeMap::from([
        (key(1), BTreeMap::from([(key(2), context.integer(3))])),
        (key(2), BTreeMap::from([(key(2), context.one())])),
    ]);
    let fresh = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        raw,
        0,
        "nonminimal-output-basis".into(),
        reductions(&maps, &[]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(fresh.remaining_terminals(), BTreeSet::from([key(2)]));
    assert_eq!(fresh.apply_all_terminals().unwrap(), maps);
    let bytes = fresh.to_native_bytes(Default::default()).unwrap();
    let restored =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    assert_eq!(restored.apply_all_terminals().unwrap(), maps);
}

#[test]
fn raw_inventory_extension_rebuilds_without_losing_retained_relations() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    let maps = BTreeMap::from([
        (key(1), BTreeMap::from([(key(2), context.integer(3))])),
        (key(2), BTreeMap::from([(key(2), context.one())])),
    ]);
    let capsule = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        raw,
        0,
        "extend-retained-basis".into(),
        reductions(&maps, &[]),
        Default::default(),
    )
    .unwrap();
    let bytes = capsule.to_native_bytes(Default::default()).unwrap();
    let authority =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    let mut extended =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();

    extended.extend(&BTreeSet::from([key(3)]), 0).unwrap();
    assert!(extended.raw_terminals().contains(&key(3)));
    assert!(extended.seeds.contains(&key(3)));
    assert!(extended.statistics().pending_rebuild_rows > 0);
    extended
        .validate_retained_baseline_identity(&authority)
        .unwrap();
    assert!(extended.validate_retained_baseline(&authority).is_err());

    while extended.statistics().pending_rebuild_rows != 0 {
        extended.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    }
    let extended_bytes = extended.to_native_bytes(Default::default()).unwrap();
    let extended = TerminalRelationSession::from_native_bytes(
        &extended_bytes,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    extended.validate_retained_baseline(&authority).unwrap();
    for target in [key(1), key(2)] {
        assert_eq!(
            extended.apply_terminal(&target).unwrap(),
            authority.apply_terminal(&target).unwrap()
        );
    }
    assert_eq!(extended.statistics().completed_source_rows, 0);
}

#[test]
fn cold_validation_detects_dropped_relations_or_guards_despite_matching_binding() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    let guard = context
        .try_add(
            &context.parameter("d").unwrap(),
            &context.integer(137),
            Default::default(),
        )
        .unwrap();
    let retained_maps = BTreeMap::from([
        (key(1), BTreeMap::from([(key(1), context.one())])),
        (key(2), BTreeMap::from([(key(1), context.integer(3))])),
    ]);
    let authority = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(&family),
        raw.clone(),
        0,
        "same-binding".into(),
        reductions(&retained_maps, std::slice::from_ref(&guard)),
        Default::default(),
    )
    .unwrap();

    let identity_maps = BTreeMap::from([
        (key(1), BTreeMap::from([(key(1), context.one())])),
        (key(2), BTreeMap::from([(key(2), context.one())])),
    ]);
    let dropped_relation = TerminalRelationSession::fresh_with_proved_reductions(
        Arc::clone(&family),
        raw.clone(),
        0,
        "same-binding".into(),
        reductions(&identity_maps, std::slice::from_ref(&guard)),
        Default::default(),
    )
    .unwrap();
    assert!(
        dropped_relation
            .validate_retained_relations(&authority)
            .is_err()
    );

    let dropped_guard = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        raw,
        0,
        "same-binding".into(),
        reductions(&retained_maps, &[]),
        Default::default(),
    )
    .unwrap();
    assert!(
        dropped_guard
            .validate_retained_relations(&authority)
            .is_err()
    );
}

#[test]
fn shared_guard_owner_is_authenticated_once_and_deduplicated_in_stable_order() {
    let family = tadpole();
    let context = family.coefficient_context();
    let first = context
        .try_add(
            &context.parameter("d").unwrap(),
            &context.integer(211),
            Default::default(),
        )
        .unwrap();
    let second = context
        .try_add(
            &context.parameter("d").unwrap(),
            &context.integer(223),
            Default::default(),
        )
        .unwrap();
    let guards = Arc::new(vec![first.clone(), second.clone(), first.clone()]);
    let reductions = [key(1), key(2)]
        .into_iter()
        .map(|target| {
            ProvedTerminalReduction::new_with_shared_conditions(
                target.clone(),
                BTreeMap::from([(target, context.one())]),
                Arc::clone(&guards),
            )
        })
        .collect();
    let session = TerminalRelationSession::fresh_with_proved_reductions(
        family,
        BTreeSet::from([key(1), key(2)]),
        0,
        "shared-guards".into(),
        reductions,
        Default::default(),
    )
    .unwrap();
    assert_eq!(session.nonzero_conditions(), &[first, second]);
}

#[test]
fn malformed_or_nonlocal_proved_map_sets_fail_closed() {
    let family = tadpole();
    let context = family.coefficient_context();
    let raw = BTreeSet::from([key(1), key(2)]);
    let identity = |target: i64| {
        ProvedTerminalReduction::new(
            key(target),
            BTreeMap::from([(key(target), context.one())]),
            Vec::new(),
        )
    };

    assert!(
        TerminalRelationSession::fresh_with_proved_reductions(
            Arc::clone(&family),
            raw.clone(),
            0,
            String::new(),
            vec![identity(1), identity(2)],
            Default::default(),
        )
        .is_err()
    );
    assert!(
        TerminalRelationSession::fresh_with_proved_reductions(
            Arc::clone(&family),
            raw.clone(),
            0,
            "missing".into(),
            vec![identity(1)],
            Default::default(),
        )
        .is_err()
    );
    assert!(
        TerminalRelationSession::fresh_with_proved_reductions(
            Arc::clone(&family),
            raw.clone(),
            0,
            "duplicate".into(),
            vec![identity(1), identity(1)],
            Default::default(),
        )
        .is_err()
    );

    let outside = ProvedTerminalReduction::new(
        key(2),
        BTreeMap::from([(key(99), context.one())]),
        Vec::new(),
    );
    assert!(
        TerminalRelationSession::fresh_with_proved_reductions(
            Arc::clone(&family),
            raw.clone(),
            0,
            "outside".into(),
            vec![identity(1), outside],
            Default::default(),
        )
        .is_err()
    );

    let bad_guard = ProvedTerminalReduction::new(
        key(2),
        BTreeMap::from([(key(1), context.one())]),
        vec![context.zero()],
    );
    assert!(
        TerminalRelationSession::fresh_with_proved_reductions(
            Arc::clone(&family),
            raw,
            0,
            "zero-guard".into(),
            vec![identity(1), bad_guard],
            Default::default(),
        )
        .is_err()
    );
}
