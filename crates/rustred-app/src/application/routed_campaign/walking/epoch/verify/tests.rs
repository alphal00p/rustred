//! Local union-result handoff: no native algebra, persisted witness, or cache.
use super::*;
use crate::application::routed_campaign::walking::{
    epoch::{
        anchors::{AnchorKind, AnchorRef, AnchorScope, AnchorView, AnchorViolation, Lent, Piece},
        ledger6::{Entry6, Ledger6},
    },
    queue::{Domain, Phase},
};
use rustred::solver::DomainPowerBounds;
use std::cell::Cell;

fn image(upper: [u64; 2]) -> CompactDomain<2> {
    CompactDomain::try_from_domain(&Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![0, 0],
        upper: upper.into_iter().map(Some).collect(),
        rank: Some(100),
        powers: DomainPowerBounds::default(),
    })
    .unwrap()
}

fn record() -> AnchorRecord {
    AnchorRecord {
        node: 1,
        kind: AnchorKind::G2Residual,
        dispatch_version: 1,
        scope: AnchorScope::Residual(vec![Piece::band(2, Some(3), None)]),
        anchors: vec![AnchorRef {
            anchor: 0,
            stamp: Some(1),
            lent: Lent::LowSlice,
        }],
    }
}

// The former token-producing function, kept only as an independent scalar
// reference. P1 used union_cover once in validate before calling this again.
fn reference<const N: usize>(
    record: &AnchorRecord,
    q: &QueryImage<N>,
    domains: &[CompactDomain<N>],
    published_len: usize,
    cut_of: &dyn Fn(u32) -> Option<i64>,
    counters: &mut VerifyCounters,
) -> Option<Vec<Verified>> {
    counters.calls += 1;
    for a in &record.anchors {
        if (a.anchor as usize) >= published_len.min(domains.len()) {
            counters.refused_range += 1;
            return None;
        }
        let image = &domains[a.anchor as usize];
        if image.phase() != q.image.phase() || image.owner() != q.image.owner() {
            counters.refused_bucket += 1;
            return None;
        }
    }
    counters.union_covers += 1;
    if union_cover(&q.image, domains, record, cut_of) != Some(true) {
        return None;
    }
    counters.accepted += 1;
    Some(
        record
            .anchors
            .iter()
            .map(|a| Verified {
                container: ContainerRef::Stored(a.anchor),
                q_digest: q.digest,
            })
            .collect(),
    )
}

#[test]
fn union_witness_evaluates_once_and_preserves_tokens_and_counters() {
    let node = image([5, 3]);
    let domains = [image([8, 4]), node];
    let mut record = record();
    // Deliberately retain anchor order, not numeric ID order, in the tokens.
    record.anchors.insert(
        0,
        AnchorRef {
            anchor: 1,
            stamp: Some(1),
            lent: Lent::Full,
        },
    );
    let visits = Cell::new(0);
    let cut_of = |_| {
        visits.set(visits.get() + 1);
        Some(3)
    };
    let q = QueryImage::new(node).unwrap();
    let mut expected_counters = VerifyCounters::default();
    assert_eq!(union_cover(&node, &domains, &record, &cut_of), Some(true));
    let expected = reference(&record, &q, &domains, 2, &cut_of, &mut expected_counters);
    assert_eq!(visits.get(), 2, "the former validate + verify path");
    visits.set(0);
    let witness = UnionCover::evaluate(&record, &node, &domains, 2, &cut_of);
    assert_eq!(witness.verdict(), Some(true));
    let mut actual_counters = VerifyCounters::default();
    let actual = verify_cover(&record, &q, witness, &mut actual_counters);
    assert_eq!(visits.get(), 1, "consumption must not rerun the oracle");
    assert_eq!(actual, expected);
    assert_eq!(actual_counters, expected_counters);
    let tokens = actual.unwrap();
    assert_eq!(tokens[0].into_id(0).unwrap().id(), 1);
    assert_eq!(tokens[1].into_id(0).unwrap().id(), 0);
    assert!(tokens.iter().all(|token| token.q_digest == q.digest));
}

#[test]
fn union_witness_true_false_and_undecided_match_reference() {
    let node = image([5, 3]);
    let domains = [image([8, 4]), node];
    let record = record();
    let q = QueryImage::new(node).unwrap();
    for (cut, verdict) in [(Some(3), Some(true)), (Some(2), Some(false)), (None, None)] {
        let cut_of = |_| cut;
        let witness = UnionCover::evaluate(&record, &node, &domains, 2, &cut_of);
        assert_eq!(witness.verdict(), verdict);
        let mut actual_counters = VerifyCounters::default();
        let actual = verify_cover(&record, &q, witness, &mut actual_counters);
        let mut expected_counters = VerifyCounters::default();
        let expected = reference(&record, &q, &domains, 2, &cut_of, &mut expected_counters);
        assert_eq!(actual, expected);
        assert_eq!(actual_counters, expected_counters);
        assert_eq!(actual_counters.calls, 1);
        assert_eq!(actual_counters.union_covers, 1);
        assert_eq!(actual_counters.accepted, u64::from(verdict == Some(true)));
    }
}

#[test]
fn union_witness_rejects_substituted_query_and_record_context() {
    let node = image([5, 3]);
    let domains = [image([8, 4]), node];
    let record = record();
    let other_record = record.clone();
    let cut_of = |_| Some(3);
    for (used_record, query_image) in [(&other_record, node), (&record, image([6, 3]))] {
        let witness = UnionCover::evaluate(&record, &node, &domains, 2, &cut_of);
        assert_eq!(witness.verdict(), Some(true));
        let q = QueryImage::new(query_image).unwrap();
        let mut counters = VerifyCounters::default();
        assert!(verify_cover(used_record, &q, witness, &mut counters).is_none());
        assert_eq!(
            counters,
            VerifyCounters {
                calls: 1,
                ..Default::default()
            },
            "a witness cannot authorize another record even with equal bytes"
        );
    }
}

#[test]
fn union_witness_keeps_range_bucket_and_malformed_union_refusals() {
    let node = image([5, 3]);
    let q = QueryImage::new(node).unwrap();
    let cut_of = |_| Some(3);
    for scenario in ["prefix", "range", "phase", "owner", "arity"] {
        let mut domains = [image([8, 4]), node];
        let mut record = record();
        let published_len = if scenario == "prefix" { 0 } else { 2 };
        match scenario {
            "range" => record.anchors[0].anchor = 9,
            "phase" | "owner" => {
                let mut domain = domains[0].expand();
                if scenario == "phase" {
                    domain.phase = Phase::Route;
                } else {
                    domain.owner = [false, true];
                }
                domains[0] = CompactDomain::try_from_domain(&domain).unwrap();
            }
            "arity" => record.scope = AnchorScope::Residual(vec![Piece::band(1, None, None)]),
            _ => {}
        }
        let witness = UnionCover::evaluate(&record, &node, &domains, published_len, &cut_of);
        let mut actual_counters = VerifyCounters::default();
        let actual = verify_cover(&record, &q, witness, &mut actual_counters);
        let mut expected_counters = VerifyCounters::default();
        let expected = reference(
            &record,
            &q,
            &domains,
            published_len,
            &cut_of,
            &mut expected_counters,
        );
        assert_eq!(actual, expected, "{scenario}");
        assert_eq!(actual_counters, expected_counters, "{scenario}");
        assert!(actual.is_none(), "{scenario}");
    }
}

#[test]
fn union_witness_small_exact_panel_matches_former_verifier() {
    for upper in 1..8 {
        let node = image([upper, 3]);
        let domains = [image([8, 4]), node];
        let q = QueryImage::new(node).unwrap();
        for low in -2..6 {
            for cut in -2..6 {
                let mut record = record();
                record.scope = AnchorScope::Residual(vec![Piece::band(2, Some(low), None)]);
                let cut_of = |_| Some(cut);
                let witness = UnionCover::evaluate(&record, &node, &domains, 2, &cut_of);
                let mut actual_counters = VerifyCounters::default();
                let actual = verify_cover(&record, &q, witness, &mut actual_counters);
                let mut expected_counters = VerifyCounters::default();
                let expected = reference(&record, &q, &domains, 2, &cut_of, &mut expected_counters);
                assert_eq!(actual, expected, "upper={upper}, low={low}, cut={cut}");
                assert_eq!(actual_counters, expected_counters);
            }
        }
    }
}

#[test]
fn union_witness_validator_preserves_false_none_and_structural_error_order() {
    let node = image([5, 3]);
    let domains = [image([8, 4]), node];
    let mut ledger = Ledger6::default();
    ledger
        .restore_word(
            Entry6::Native {
                epoch: 1,
                residual: false,
                dband: true,
            }
            .encode(),
        )
        .unwrap();
    for (scenario, cut, expected) in [
        ("true", Some(3), Ok(())),
        ("false", Some(2), Err(AnchorViolation::UnionNotCovered)),
        ("missing cut", None, Err(AnchorViolation::UnionUndecided)),
        (
            "arity before missing cut",
            None,
            Err(AnchorViolation::ScopeArity),
        ),
        (
            "duplicate before missing cut",
            None,
            Err(AnchorViolation::DuplicateAnchor),
        ),
    ] {
        let mut record = record();
        if scenario.starts_with("arity") {
            record.scope = AnchorScope::Residual(vec![Piece::band(1, None, None)]);
        } else if scenario.starts_with("duplicate") {
            record.anchors.push(record.anchors[0]);
        }
        let cut_calls = Cell::new(0);
        let cut_of = |_| {
            cut_calls.set(cut_calls.get() + 1);
            cut
        };
        let evaluated = std::cell::RefCell::new(None);
        let cover_calls = Cell::new(0);
        let cover = |r: &AnchorRecord| {
            assert!(std::ptr::eq(r, &record));
            cover_calls.set(cover_calls.get() + 1);
            let witness = UnionCover::evaluate(&record, &node, &domains, 2, &cut_of);
            let verdict = witness.verdict();
            evaluated.replace(Some(witness));
            verdict
        };
        let view = AnchorView {
            p0: 1,
            published_len: 2,
            arity: 2,
            ledger: &ledger,
            same_bucket: &|_, _| true,
            record_of: &|_| Some((AnchorKind::InitialDBand, Some(3))),
            edges_of: None,
            merged_view: Some(&|_, _| true),
            cover: &cover,
        };
        let mut counters = VerifyCounters::default();
        let actual = record.validate(&view, 2);
        assert_eq!(actual, expected, "{scenario}");
        if actual.is_ok() {
            let q = QueryImage::new(node).unwrap();
            let witness = evaluated.into_inner().unwrap();
            assert!(verify_cover(&record, &q, witness, &mut counters).is_some());
            assert_eq!(counters.calls, 1);
            assert_eq!(counters.union_covers, 1);
            assert_eq!(counters.accepted, 1);
        } else {
            assert_eq!(counters, VerifyCounters::default(), "{scenario}");
        }
        let expected_evaluations = usize::from(!scenario.contains("before"));
        assert_eq!(cover_calls.get(), expected_evaluations, "{scenario}");
        assert_eq!(cut_calls.get(), expected_evaluations, "{scenario}");
    }
}
