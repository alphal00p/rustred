use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

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
