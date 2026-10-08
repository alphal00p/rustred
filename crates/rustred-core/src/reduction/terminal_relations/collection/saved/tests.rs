use super::*;
use crate::family::AffineDenominator;
use std::sync::atomic::AtomicBool;

fn family(name: &str) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            name,
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
fn session(family: Arc<IntegralFamily>, powers: &[i64]) -> TerminalRelationSession {
    TerminalRelationSession::new(
        family,
        powers.iter().map(|&n| key(n)).collect(),
        0,
        Default::default(),
    )
    .unwrap()
}
fn finish(session: &mut TerminalRelationSession) {
    let cancel = AtomicBool::new(false);
    for _ in 0..1000 {
        if session.is_complete() {
            return;
        }
        session.step(&cancel).unwrap();
    }
    panic!("finite fixture did not finish");
}

#[test]
fn bulk_maps_match_every_individual_map_before_and_after_search() {
    let mut s = session(family("bulk"), &[1, 2, 3]);
    for completed in [false, true] {
        if completed {
            finish(&mut s);
        }
        let all = s.apply_all_terminals().unwrap();
        for raw in s.raw_terminals() {
            assert_eq!(all[raw], s.apply_terminal(raw).unwrap());
        }
    }
}

#[test]
fn composition_cold_load_preserves_source_provenance_and_all_maps() {
    let a = session(family("collection-a"), &[1, 2]);
    let b = session(family("collection-b"), &[1, 2]);
    let plan = TerminalCollectionPlan::prepare(&[&a, &b], Default::default()).unwrap();
    assert_eq!(plan.statistics().raw_terminals, 4);
    assert_eq!(plan.statistics().precollection_remaining, 4);
    assert_eq!(plan.statistics().remaining_terminals, 1);
    assert_eq!(plan.proofs()[0].sources().len(), 2);
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let a = TerminalRelationSession::from_native_bytes(
        &a.to_native_bytes(Default::default()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let b = TerminalRelationSession::from_native_bytes(
        &b.to_native_bytes(Default::default()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &bytes,
        &[&b, &a],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(cold.statistics(), plan.statistics());
    for s in [&a, &b] {
        for raw in s.raw_terminals() {
            assert_eq!(
                cold.apply(s.family_owner(), raw).unwrap().terms(),
                plan.apply(s.family_owner(), raw).unwrap().terms()
            );
            assert_eq!(
                cold.apply(s.family_owner(), raw)
                    .unwrap()
                    .nonzero_conditions(),
                plan.apply(s.family_owner(), raw)
                    .unwrap()
                    .nonzero_conditions()
            );
        }
    }
    assert_eq!(plan.to_native_bytes(Default::default()).unwrap(), bytes);
}

#[test]
fn rebind_retains_prior_collection_without_new_source_rows() {
    let mut s = session(family("extended-collection"), &[1, 2]);
    let plan = TerminalCollectionPlan::prepare(&[&s], Default::default()).unwrap();
    let source_count = plan.proofs()[0].sources().len();
    s.extend(&BTreeSet::from([key(3)]), 0).unwrap();
    while s.statistics().pending_rebuild_rows > 0 {
        s.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    }
    assert!(!s.is_complete());
    assert!(
        TerminalCollectionPlan::from_native_bytes(
            &plan.to_native_bytes(Default::default()).unwrap(),
            &[&s],
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    let extended = plan.rebind(&[&s], Default::default()).unwrap();
    assert_eq!(extended.statistics().remaining_terminals, 2);
    assert_eq!(extended.proofs()[0].sources().len(), source_count);
    assert_eq!(
        extended.apply(s.family_owner(), &key(2)).unwrap().terms(),
        plan.apply(s.family_owner(), &key(2)).unwrap().terms()
    );
    let preserved = TerminalCollectionPlan::from_native_bytes(
        &extended.to_native_bytes(Default::default()).unwrap(),
        &[&s],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(preserved.statistics(), extended.statistics());
    finish(&mut s);
    let refined = TerminalCollectionPlan::prepare(&[&s], Default::default()).unwrap();
    assert_eq!(refined.statistics().remaining_terminals, 1);
}

#[test]
fn no_discovery_publication_and_duplicate_family_fail_closed() {
    let s = session(family("unrefined"), &[1, 2]);
    let plan = TerminalCollectionPlan::from_sessions(&[&s], Default::default()).unwrap();
    assert_eq!(plan.statistics().remaining_terminals, 2);
    assert_eq!(plan.statistics().collection_groups, 0);
    assert!(TerminalCollectionPlan::prepare(&[&s, &s], Default::default()).is_err());
    assert!(plan.apply(s.family_owner(), &key(3)).is_err());
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &bytes,
        &[&s],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics(), cold.statistics());
}

#[test]
fn repeated_refine_does_not_grow_proof_history_and_extension_keeps_old_sources() {
    let mut s = session(family("incremental-refine"), &[1, 2]);
    let first = TerminalCollectionPlan::prepare(&[&s], Default::default()).unwrap();
    let again = first.refine(&[&s], Default::default()).unwrap();
    assert_eq!(first.statistics(), again.statistics());
    assert_eq!(
        first.to_native_bytes(Default::default()).unwrap(),
        again.to_native_bytes(Default::default()).unwrap()
    );
    s.extend(&BTreeSet::from([key(3)]), 0).unwrap();
    while s.statistics().pending_rebuild_rows > 0 {
        s.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    }
    let next = first.refine(&[&s], Default::default()).unwrap();
    // Corner diagonal discovery alone does not invent the missing dot-three
    // equation; the old dot-two rule remains, while dot-three stays free.
    assert_eq!(
        next.apply(s.family_owner(), &key(2)).unwrap().terms(),
        first.apply(s.family_owner(), &key(2)).unwrap().terms()
    );
    assert!(
        next.remaining_terminals()
            .iter()
            .any(|k| k.integral() == &key(3))
    );
    let cold = TerminalCollectionPlan::from_native_bytes(
        &next.to_native_bytes(Default::default()).unwrap(),
        &[&s],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(next.statistics(), cold.statistics());
    finish(&mut s);
    let finished = next.refine(&[&s], Default::default()).unwrap();
    assert_eq!(finished.statistics().remaining_terminals, 1);
    assert!(finished.proofs().len() >= first.proofs().len());
}
