use super::*;

fn multi(values: &[i64]) -> IntegralKey {
    IntegralKey::try_new(values.iter().copied()).unwrap()
}

fn sunset() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "finite-containing-sector-sources",
            vec!["k1".into(), "k2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|powers| {
                    AffineDenominator::new(
                        c.integer(-1),
                        powers.into_iter().map(|n| c.integer(n)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![c.zero(); 3],
        )
        .unwrap(),
    )
}

#[test]
fn containing_sectors_promote_support_directly_with_generic_combination_depth() {
    let raw = BTreeSet::from([multi(&[2, -3, 0])]);
    assert!(
        sources::containing_sector_seeds(&raw, 0, 10)
            .unwrap()
            .is_empty()
    );
    let single = sources::containing_sector_seeds(&raw, 1, 10).unwrap();
    assert_eq!(
        single,
        BTreeSet::from([multi(&[2, 1, 0]), multi(&[2, -3, 1])])
    );
    let double = sources::containing_sector_seeds(&raw, 2, 10).unwrap();
    assert_eq!(
        double,
        single
            .union(&BTreeSet::from([multi(&[2, 1, 1])]))
            .cloned()
            .collect()
    );
    assert!(
        !sources::seeds(&raw, 1, 100)
            .unwrap()
            .contains(&multi(&[2, 1, 0]))
    );

    let three_axes = BTreeSet::from([multi(&[0, -2, -3, 7])]);
    let promoted = sources::containing_sector_seeds(&three_axes, 3, 7).unwrap();
    assert_eq!(promoted.len(), 7);
    assert!(promoted.contains(&multi(&[1, 1, 1, 7])));
    assert!(sources::containing_sector_seeds(&three_axes, 3, 6).is_err());
}

#[test]
fn containing_sector_inventory_roundtrips_without_changing_terminal_roles() {
    let raw = BTreeSet::from([multi(&[2, -3, 1])]);
    let mut session =
        TerminalRelationSession::new(sunset(), raw.clone(), 0, Default::default()).unwrap();
    let original_terminals = session.terminals.clone();
    let original_seeds = session.seeds.clone();
    let promoted = multi(&[2, 1, 1]);
    assert!(!original_seeds.contains(&promoted));
    assert_eq!(session.extend_containing_sector_seeds(1).unwrap(), 1);
    assert_eq!(session.seeds.last(), Some(&promoted));
    assert_eq!(session.terminals, original_terminals);
    assert_eq!(session.raw_terminals(), &raw);
    assert_eq!(session.statistics().seed_depth, 0);
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    let mut resumed =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    assert_eq!(resumed.seeds, session.seeds);
    assert_eq!(resumed.statistics(), session.statistics());
    assert_eq!(resumed.extend_containing_sector_seeds(1).unwrap(), 0);
    assert_eq!(resumed.to_native_bytes(Default::default()).unwrap(), bytes);
}

#[test]
fn explicit_source_seed_finds_missing_relation_without_adding_a_terminal() {
    let raw = BTreeSet::from([key(1), key(3)]);
    let mut session =
        TerminalRelationSession::new(tadpole(), raw.clone(), 0, Default::default()).unwrap();
    finish(&mut session);
    assert_eq!(session.statistics().remaining_terminals, 2);
    let completed = session.statistics().completed_source_rows;
    assert_eq!(
        session
            .extend_source_seeds(&BTreeSet::from([key(2)]))
            .unwrap(),
        1
    );
    assert_eq!(session.statistics().completed_source_rows, completed);
    assert_eq!(session.statistics().seed_depth, 0);
    assert_eq!(session.raw_terminals(), &raw);
    assert!(!session.terminals.contains(&key(2)));
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    session =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    finish(&mut session);
    let done = session.statistics();
    assert_eq!(
        session
            .extend_source_seeds(&BTreeSet::from([key(2)]))
            .unwrap(),
        0
    );
    assert_eq!(session.statistics(), done);
    assert_eq!(session.statistics().completed_source_rows, completed + 1);

    let mut reference =
        TerminalRelationSession::new(tadpole(), raw, 1, Default::default()).unwrap();
    finish(&mut reference);
    assert_eq!(
        session.remaining_terminals(),
        reference.remaining_terminals()
    );
    assert_eq!(
        session.apply_terminal(&key(3)).unwrap(),
        reference.apply_terminal(&key(3)).unwrap()
    );
}

#[test]
fn source_seed_append_preserves_partial_source_and_rebuild_cursors() {
    let family = sunset();
    let raw = BTreeSet::from([multi(&[1, 1, 1])]);
    let extra = BTreeSet::from([multi(&[1, 2, 1])]);
    let mut interrupted =
        TerminalRelationSession::new(family.clone(), raw.clone(), 0, Default::default()).unwrap();
    interrupted.step(&AtomicBool::new(false)).unwrap();
    assert_eq!(interrupted.source_cursor, 1);
    interrupted.extend_source_seeds(&extra).unwrap();
    assert_eq!(interrupted.source_cursor, 1);
    let bytes = interrupted.to_native_bytes(Default::default()).unwrap();
    interrupted =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    let mut reference = TerminalRelationSession::new(family, raw, 0, Default::default()).unwrap();
    reference.extend_source_seeds(&extra).unwrap();
    finish(&mut interrupted);
    finish(&mut reference);
    assert_eq!(interrupted.basis_rows(), reference.basis_rows());

    let mut rebuilding = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1), key(3)]),
        0,
        Default::default(),
    )
    .unwrap();
    while rebuilding.seed_cursor < rebuilding.seeds.len() {
        rebuilding.step(&AtomicBool::new(false)).unwrap();
    }
    rebuilding.step(&AtomicBool::new(false)).unwrap();
    assert!(rebuilding.statistics().pending_rebuild_rows > 0);
    let pending = rebuilding.statistics().pending_rebuild_rows;
    assert_eq!(
        rebuilding
            .extend_source_seeds(&BTreeSet::from([key(2)]))
            .unwrap(),
        1
    );
    assert_eq!(rebuilding.statistics().pending_rebuild_rows, pending);
    let bytes = rebuilding.to_native_bytes(Default::default()).unwrap();
    rebuilding =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    finish(&mut rebuilding);
    assert_eq!(rebuilding.statistics().remaining_terminals, 1);
}

#[test]
fn source_seed_limits_and_invalid_arity_are_atomic_and_zero_promotions_are_noop() {
    let mut session = TerminalRelationSession::new(
        tadpole(),
        BTreeSet::from([key(1)]),
        0,
        TerminalRelationLimits {
            max_seeds: 2,
            ..Default::default()
        },
    )
    .unwrap();
    finish(&mut session);
    let before = session.statistics();
    let seeds = session.seeds.clone();
    assert_eq!(session.extend_containing_sector_seeds(0).unwrap(), 0);
    assert_eq!(session.extend_containing_sector_seeds(1).unwrap(), 0);
    assert!(session.extend_containing_sector_seeds(2).is_err());
    assert!(
        session
            .extend_source_seeds(&BTreeSet::from([multi(&[1, 2])]))
            .is_err()
    );
    assert!(
        session
            .extend_source_seeds(&BTreeSet::from([key(2), key(3)]))
            .is_err()
    );
    assert_eq!(session.statistics(), before);
    assert_eq!(session.seeds, seeds);
}

#[test]
fn additional_source_seeds_join_assistance_once_after_resume() {
    let mut session = assisted_tadpole();
    for _ in 0..1000 {
        if session.is_complete() {
            break;
        }
        session
            .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![]))
            .unwrap();
    }
    assert!(session.is_complete());
    let old_queries = session.statistics().completed_assistance_keys;
    assert_eq!(
        session
            .extend_source_seeds(&BTreeSet::from([key(5)]))
            .unwrap(),
        1
    );
    assert_eq!(
        session
            .extend_source_seeds(&BTreeSet::from([key(5)]))
            .unwrap(),
        0
    );
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    session =
        TerminalRelationSession::from_native_bytes(&bytes, Default::default(), Default::default())
            .unwrap();
    let mut queries = Vec::new();
    for _ in 0..1000 {
        if session.is_complete() {
            break;
        }
        session
            .step_with_provider(&AtomicBool::new(false), |target| {
                queries.push(target.clone());
                Ok(vec![])
            })
            .unwrap();
    }
    assert!(session.is_complete());
    assert_eq!(queries, vec![key(5), key(6)]);
    assert_eq!(
        session.statistics().completed_assistance_keys,
        old_queries + 2
    );
    assert_eq!(session.raw_terminals(), &BTreeSet::from([key(1), key(3)]));
}
