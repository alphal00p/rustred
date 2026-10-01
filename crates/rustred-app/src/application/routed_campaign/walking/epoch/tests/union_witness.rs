//! P1 error/counter order remains unchanged by local union-result reuse.
use super::*;

#[test]
fn union_witness_p1_structural_and_false_cover_fail_before_verify_counters() {
    for (scenario, violation) in [
        ("none", "NoAnchors"),
        ("duplicate", "DuplicateAnchor"),
        ("range", "AnchorOutOfRange"),
        ("stamp", "StampAfterDispatch"),
        ("lent", "LentScopeMismatch"),
        ("arity", "ScopeArity"),
        ("gap", "UnionNotCovered"),
    ] {
        let (mut state, _, _, job) = d_band_fixture(G2);
        let mut result = g2_result(&job, 2, &[(0, 1, 0)], vec![Piece::band(2, None, Some(5))]);
        let part = result.g2.as_mut().unwrap();
        match scenario {
            "none" => part.anchors.clear(),
            "duplicate" => part.anchors.push(part.anchors[0]),
            "range" => part.anchors[0].0 = 999,
            "stamp" => part.anchors[0].1 = 2,
            "lent" => part.anchors[0].2 = 1,
            "arity" => part.pieces[0].lower.pop().map(|_| ()).unwrap(),
            "gap" => part.pieces[0].d_hi = Some(4),
            _ => unreachable!(),
        }
        let before = state.verify;
        let ledger = state.ledger.words().to_vec();
        let error = merge::p1_anchors(&mut state, &result, G2).err().unwrap();
        assert_eq!(error.0, format!("P1: 3 anchors: {violation}"), "{scenario}");
        assert_eq!(state.verify, before, "{scenario}");
        assert_eq!(state.ledger.words(), ledger, "{scenario}");
        assert_eq!(state.anchors.len(), 0, "{scenario}");
    }
}

#[test]
fn union_witness_p1_success_preserves_tokens_digest_and_counter_delta() {
    let (mut state, _, _, job) = d_band_fixture(G2);
    let result = g2_result(&job, 2, &[(0, 1, 0)], vec![Piece::band(2, None, Some(5))]);
    let mut expected = state.verify;
    expected.calls += 1;
    expected.union_covers += 1;
    expected.accepted += 1;
    let checked = merge::p1_anchors(&mut state, &result, G2).unwrap().unwrap();
    assert_eq!(state.verify, expected);
    assert_eq!(checked.q_digest, job.image.digest().0);
    assert_eq!(checked.record.node, job.parent);
    assert_eq!(checked.tokens.len(), 1);
    let token = checked.tokens[0].into_id(0).unwrap();
    assert_eq!(token.id(), 0);
    assert_eq!(token.q_digest(), checked.q_digest);
}

#[test]
fn union_witness_does_not_replace_initial_d_band_high_slice_verification() {
    let (mut state, _, _, job) = d_band_fixture(CONFIG);
    let before = state.verify;
    let checked = merge::p1_anchors(&mut state, &partial(&job, 0, 6), CONFIG)
        .unwrap()
        .unwrap();
    let mut high = job.image.expand();
    high.powers.min_power_difference = Some(6);
    assert_eq!(checked.q_digest, image(&high).digest().0);
    assert_eq!(state.verify.calls, before.calls + 1);
    assert_eq!(state.verify.accepted, before.accepted + 1);
    assert_eq!(state.verify.union_covers, before.union_covers);
}
