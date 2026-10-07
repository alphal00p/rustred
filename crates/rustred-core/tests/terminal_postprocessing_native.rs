//! Public-API controls: build only this small integration-test executable.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::reduction::terminal_relations::{TerminalRelationLimits, TerminalRelationSession};

fn circuit(loops: usize) -> Arc<IntegralFamily> {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let mut momenta: Vec<Vec<i64>> = (0..loops)
        .map(|i| (0..loops).map(|j| i64::from(i == j)).collect())
        .collect();
    if loops > 1 {
        momenta.push(vec![1; loops]);
    }
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
    Arc::new(
        IntegralFamily::new(
            "native-terminal-control",
            (0..loops).map(|i| format!("k{i}")).collect(),
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![c.zero(); momenta.len()],
        )
        .unwrap(),
    )
}
fn key(values: &[i64]) -> IntegralKey {
    IntegralKey::try_new(values.iter().copied()).unwrap()
}
fn finish(session: &mut TerminalRelationSession) {
    for _ in 0..2000 {
        if session.is_complete() {
            return;
        }
        session.step(&AtomicBool::new(false)).unwrap();
    }
    panic!("small finite source pool did not drain");
}
fn reopen(
    session: &TerminalRelationSession,
    limits: TerminalRelationLimits,
) -> TerminalRelationSession {
    let bytes = session.to_native_bytes(Default::default()).unwrap();
    TerminalRelationSession::from_native_bytes(&bytes, limits, Default::default()).unwrap()
}

#[test]
fn four_loop_generated_column_equivalence_and_native_resume() {
    let family = circuit(4);
    let base = key(&[1, 1, 1, 1, 1, 0, 0, 0, 0, 0]);
    let dot = key(&[2, 1, 1, 1, 1, 0, 0, 0, 0, 0]);
    let mut session = TerminalRelationSession::new(
        family.clone(),
        BTreeSet::from([base.clone(), dot.clone()]),
        0,
        Default::default(),
    )
    .unwrap();
    // Stop after actual source work; restore rather than regenerate those rows.
    session.step(&AtomicBool::new(false)).unwrap();
    let before = session.statistics();
    let mut session = reopen(&session, Default::default());
    assert_eq!(session.statistics(), before);
    session.step(&AtomicBool::new(true)).unwrap();
    assert_eq!(session.statistics(), before);
    finish(&mut session);
    assert_eq!(
        session.remaining_terminals(),
        BTreeSet::from([base.clone()])
    );
    let c = family.coefficient_context();
    let twice_d = c
        .try_mul(
            &c.integer(2),
            &c.parameter("d").unwrap(),
            Default::default(),
        )
        .unwrap();
    let numerator = c
        .try_sub(&twice_d, &c.integer(5), Default::default())
        .unwrap();
    let expected = c
        .try_div(&numerator, &c.integer(5), Default::default())
        .unwrap();
    assert_eq!(
        session.apply_terminal(&dot).unwrap(),
        BTreeMap::from([(base, expected)])
    );
    let final_session = reopen(&session, Default::default());
    assert!(final_session.is_complete());
    assert_eq!(
        final_session.apply_terminal(&dot).unwrap(),
        session.apply_terminal(&dot).unwrap()
    );
}

#[test]
fn refused_row_keeps_native_checkpoint_loadable_under_same_limits() {
    let limits = TerminalRelationLimits {
        max_nonzeros: 2,
        ..Default::default()
    };
    let mut session = TerminalRelationSession::new(
        circuit(1),
        BTreeSet::from([key(&[1]), key(&[2])]),
        0,
        limits,
    )
    .unwrap();
    session.step(&AtomicBool::new(false)).unwrap();
    let before = session.statistics();
    assert_eq!(before.completed_source_rows, 1);
    assert!(session.step(&AtomicBool::new(false)).is_err());
    assert_eq!(session.statistics(), before);
    let reopened = reopen(&session, limits);
    assert_eq!(reopened.statistics(), before);
    let mut extended = reopen(&session, Default::default());
    finish(&mut extended);
    assert_eq!(extended.remaining_terminals(), BTreeSet::from([key(&[1])]));
}
