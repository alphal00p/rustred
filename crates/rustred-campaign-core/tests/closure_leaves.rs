//! lane layers (interfaces/closure.md 18): leaf sets. A later walk stops at the keys of an earlier, closed walk; the
//! composed result equals one walk of the union; the binding refuses another table set / order / a relation set that
//! serves a leaf; a changed byte of a leaf-set file is refused; an uncertified leaf (or a leaf set that is not closed)
//! blocks the later walk with the keys named; an edge into a leaf is still checked strictly lower. Toy node of arity 4
//! (lines 0, 1; numerators 2, 3). Default suite.

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred_campaign_core::closure::{
    ClosureConfig, ClosureError, CoefFact, DispatchSlot, FactSource, HandDowns, KeyCodec, KeySet,
    LeafBinding, LeafProducer, LeafSet, LeafSetBuilder, NoFacts, RegFact, RelationSet,
    RelationStoreBuilder, RuleBodies, RuleId, Tier, VERDICT_CERTIFIED, Verdict, closure,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;

const CHART: [bool; 4] = [true, true, false, false];

/// Exact facts for the toy: every winner regular, every coefficient non-zero (no term dropped).
struct AllRegular;
impl FactSource for AllRegular {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        CoefFact::NonZero
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Regular
    }
    fn label(&self) -> String {
        "toy:all-regular".into()
    }
}

fn codec() -> KeyCodec {
    KeyCodec::new(4, 6).unwrap()
}
fn ks(keys: &[[i32; 4]]) -> KeySet {
    KeySet::from_keys(codec(), keys.iter().map(|k| k.as_slice())).unwrap()
}
fn cmp() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}
/// One table `A`: a free rule lowering line 1 (`I(n0, n1-1)`; at n1 = 1 a hand-down), optionally a second record before
/// it on the face n1 <= `up_below` that RAISES line 1 (a non-descending rule).
fn node(body: &str, up_below: Option<i32>) -> (Dispatcher, RuleBodies) {
    let mut t = Table::new("A", 4);
    let mut n = 0usize;
    if let Some(u) = up_below {
        let mut d = CaseDomain::free(4, Some(CHART.to_vec()));
        d.bounds =
            vec![rustred_campaign_core::poly::Poly::parse_d_free(&format!("{u}-n1"), 4).unwrap()];
        t.push(0, d).unwrap();
        n += 1;
    }
    t.push(n as i64, CaseDomain::free(4, Some(CHART.to_vec())))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    if up_below.is_some() {
        b.set(&d, 0, 0, &["I(n0,n1+1,n2,n3)"]).unwrap();
    }
    b.set(&d, 0, n, &[body]).unwrap();
    (d, b)
}
fn cfg(tier: Tier, digest: &str) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 2,
        hand_downs: HandDowns::Keys,
        table_digests: [("A".to_string(), digest.to_string())].into(),
        ..Default::default()
    }
}
/// The leaf set of a finished walk: every own key, certified iff `certified`, with the run's walk binding.
fn leafset(c: &rustred_campaign_core::closure::Closure, verdict: &str, certified: bool) -> LeafSet {
    let mut b = LeafSetBuilder::new(codec(), vec![], vec![]);
    let own = c.own_keys();
    for k in own.iter() {
        b.push(
            &k,
            "table",
            if certified { "per-key" } else { "open" },
            certified,
        )
        .unwrap();
    }
    for im in c.credit_images().iter() {
        b.aux(&im).unwrap();
    }
    b.finish(LeafBinding {
        statement_digest: "toy-statement".into(),
        source_verdict: verdict.into(),
        node: "toy".into(),
        clause: "base".into(),
        start_digest: c.summary().start_set_digest,
        walk: c.leaf_walk().clone(),
        relative_to: vec![],
        provenance: BTreeMap::new(),
        // format 2 (audit r2-planners L1): certified at per-key, exact tier, strict basis
        producer: Some(LeafProducer::new("per-key", Tier::C, true)),
    })
    .unwrap()
}
/// key -> winner (table, record) of every own key.
fn winners(c: &rustred_campaign_core::closure::Closure) -> BTreeMap<Vec<i32>, String> {
    (0..c.len())
        .filter(|&i| !c.is_leaf(i))
        .map(|i| {
            (
                c.key(i),
                c.winner(i)
                    .map(|w| format!("{}#{}", w.table, w.record))
                    .unwrap_or_else(|| "open".into()),
            )
        })
        .collect()
}

#[test]
fn a_later_walk_stops_at_the_leaves_and_the_composition_equals_one_walk() {
    let (d, b) = node("I(n0,n1-1,n2,n3)", None);
    let cf = cfg(Tier::C, "sha256:A");
    // clause 1 (base): start (3,3) -> (3,3),(3,2),(3,1)
    let base = closure(&d, &b, &cmp(), &ks(&[[3, 3, 0, 0]]), &cf, &AllRegular).unwrap();
    assert_eq!(base.summary().verdict, Verdict::Closed);
    let l1 = Arc::new(leafset(&base, "closed", true));
    assert_eq!(l1.len(), 3);
    // clause 2: start (3,6) and the leaf (3,2) itself as a start key
    let cf2 = ClosureConfig {
        leaves: vec![l1.clone()],
        ..cf.clone()
    };
    let st2 = ks(&[[3, 6, 0, 0], [3, 2, 0, 0]]);
    let later = closure(&d, &b, &cmp(), &st2, &cf2, &AllRegular).unwrap();
    let s = later.summary();
    assert_eq!(s.verdict, Verdict::Closed, "{:?}", s.blocking);
    let lv = s.leaves.clone().unwrap();
    // own keys (3,6),(3,5),(3,4); leaves met: (3,3) (by the walk) and (3,2) (start)
    assert_eq!(later.own_keys().len(), 3);
    assert_eq!((lv.reached, lv.in_start, lv.open_reached), (2, 1, 0));
    assert_eq!(lv.relative_to, vec![l1.digest().to_string()]);
    assert_eq!(s.cycles, Some(0));
    // edges into the leaf are generated and counted (3,4)->(3,3)
    assert_eq!(s.descent_violations, 0);
    // composed (base own + later own) == one walk of the union of the starts, key for key and winner for winner
    let union = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 3, 0, 0], [3, 6, 0, 0], [3, 2, 0, 0]]),
        &cf,
        &AllRegular,
    )
    .unwrap();
    let mut comp = winners(&base);
    comp.extend(winners(&later));
    assert_eq!(comp, winners(&union));
    assert_eq!(
        base.own_keys().union(&later.own_keys()).digest(),
        union.keys().digest()
    );
    // hand-down term counts add up (each key expanded once overall)
    let hd = |c: &rustred_campaign_core::closure::Closure| -> u64 {
        c.summary().hand_downs.values().map(|v| v[0]).sum()
    };
    assert_eq!(hd(&base) + hd(&later), hd(&union));
    // identity names the leaves; without leaves it is unchanged
    assert!(later.config_identity().components.contains_key("leaves"));
    assert!(!base.config_identity().components.contains_key("leaves"));
    assert!(base.summary().leaves.is_none());
}

#[test]
fn the_binding_refuses_another_table_set_another_order_and_a_relation_serving_a_leaf() {
    let (d, b) = node("I(n0,n1-1,n2,n3)", None);
    let cf = cfg(Tier::C, "sha256:A");
    let base = closure(&d, &b, &cmp(), &ks(&[[3, 3, 0, 0]]), &cf, &AllRegular).unwrap();
    let l1 = Arc::new(leafset(&base, "closed", true));
    let st = ks(&[[3, 5, 0, 0]]);
    // another coefficient file of table A (table set): refused, naming the component
    let other = ClosureConfig {
        leaves: vec![l1.clone()],
        ..cfg(Tier::C, "sha256:A-other")
    };
    match closure(&d, &b, &cmp(), &st, &other, &AllRegular) {
        Err(ClosureError::Leaves(m)) => assert!(m.contains("table_sources"), "{m}"),
        r => panic!("not refused: {:?}", r.map(|c| c.summary().verdict)),
    }
    // another body (rule content)
    let (d2, b2) = node("I(n0,n1-1,n2,n3-1)", None);
    let with = ClosureConfig {
        leaves: vec![l1.clone()],
        ..cf.clone()
    };
    match closure(&d2, &b2, &cmp(), &st, &with, &AllRegular) {
        Err(ClosureError::Leaves(m)) => assert!(m.contains("bodies"), "{m}"),
        r => panic!("not refused: {:?}", r.map(|c| c.summary().verdict)),
    }
    // another order (comparator)
    let cmp2 = NodeComparator::legacy(4, Some(vec![1, 0, 2, 3])).unwrap();
    match closure(&d, &b, &cmp2, &st, &with, &AllRegular) {
        Err(ClosureError::Leaves(m)) => assert!(m.contains("comparator"), "{m}"),
        r => panic!("not refused: {:?}", r.map(|c| c.summary().verdict)),
    }
    // a relation set certified at a key ELSEWHERE does not touch the binding; one certified AT a leaf refuses
    let rel = |at: [i32; 4]| -> Arc<RelationSet> {
        let mut rb = RelationStoreBuilder::new(codec());
        rb.push(
            &at,
            &[vec![at[0], at[1] - 1, 0, 0]],
            [7u8; 32],
            VERDICT_CERTIFIED,
        )
        .unwrap();
        Arc::new(RelationSet {
            name: "rec".into(),
            recipe_digest: "sha256:recipe".into(),
            domain: CaseDomain::free(4, Some(CHART.to_vec())),
            relations: rb.finish().unwrap(),
            slot: DispatchSlot::BeforeTable("A".into()),
        })
    };
    let elsewhere = ClosureConfig {
        relation_sets: vec![rel([7, 7, 0, 0])],
        ..with.clone()
    };
    let c = closure(&d, &b, &cmp(), &st, &elsewhere, &AllRegular).unwrap();
    assert_eq!(c.summary().verdict, Verdict::Closed);
    let at_leaf = ClosureConfig {
        relation_sets: vec![rel([3, 2, 0, 0])],
        ..with.clone()
    };
    match closure(&d, &b, &cmp(), &st, &at_leaf, &AllRegular) {
        Err(ClosureError::Leaves(m)) => assert!(m.contains("relations@keys"), "{m}"),
        r => panic!("not refused: {:?}", r.map(|c| c.summary().verdict)),
    }
}

#[test]
fn a_changed_byte_of_a_leaf_set_file_is_refused_and_the_digest_is_codec_independent() {
    let (d, b) = node("I(n0,n1-1,n2,n3)", None);
    let base = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 3, 0, 0]]),
        &cfg(Tier::C, "sha256:A"),
        &AllRegular,
    )
    .unwrap();
    let l1 = leafset(&base, "closed", true);
    let mut bytes = vec![];
    let dg = l1.write(&mut bytes).unwrap();
    assert_eq!(dg, l1.digest());
    let back = LeafSet::read(std::io::Cursor::new(&bytes)).unwrap();
    assert_eq!(back, l1);
    // flip the certification flag of the last key (last section before the aux codes; no aux here)
    let mut bad = bytes.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    let e = LeafSet::read(std::io::Cursor::new(&bad)).unwrap_err();
    assert!(e.to_string().contains("digest"), "{e}");
    // a changed binding in the header (another statement digest) with the old digest: refused
    let text = String::from_utf8_lossy(&bytes[..bytes.iter().position(|&c| c == b'\n').unwrap()])
        .to_string();
    let swapped = text.replace("toy-statement", "toy-statemenX");
    let mut bad2 = swapped.into_bytes();
    bad2.extend_from_slice(&bytes[bytes.iter().position(|&c| c == b'\n').unwrap()..]);
    assert!(LeafSet::read(std::io::Cursor::new(&bad2)).is_err());
    // the digest does not depend on the codec width
    let wide = l1.recode(KeyCodec::new(4, 9).unwrap()).unwrap();
    assert_eq!(wide.digest(), l1.digest());
}

#[test]
fn an_uncertified_leaf_or_a_leaf_set_that_is_not_closed_blocks_with_the_keys_named() {
    let (d, b) = node("I(n0,n1-1,n2,n3)", None);
    let cf = cfg(Tier::C, "sha256:A");
    let base = closure(&d, &b, &cmp(), &ks(&[[3, 3, 0, 0]]), &cf, &AllRegular).unwrap();
    let open_set = Arc::new(leafset(&base, "open", false));
    let later = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 5, 0, 0]]),
        &ClosureConfig {
            leaves: vec![open_set.clone()],
            ..cf.clone()
        },
        &AllRegular,
    )
    .unwrap();
    let s = later.summary();
    assert_eq!(s.verdict, Verdict::Open);
    assert!(
        s.blocking.iter().any(|b| b == "leaf_open:1"),
        "{:?}",
        s.blocking
    );
    assert!(
        s.blocking
            .iter()
            .any(|b| b.starts_with(&format!("leaf_set_not_closed:{}", open_set.digest()))),
        "{:?}",
        s.blocking
    );
    assert_eq!(later.leaf_open_keys(), vec![vec![3, 3, 0, 0]]);
    // a walk that never reaches the open set is not affected by it
    let apart = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[5, 2, 0, 0]]),
        &ClosureConfig {
            leaves: vec![open_set],
            ..cf.clone()
        },
        &AllRegular,
    )
    .unwrap();
    assert_eq!(apart.summary().verdict, Verdict::Closed);
    // Tier K: leaves work the same (no facts)
    let k = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 5, 0, 0]]),
        &ClosureConfig {
            leaves: vec![Arc::new(leafset(
                &closure(
                    &d,
                    &b,
                    &cmp(),
                    &ks(&[[3, 3, 0, 0]]),
                    &cfg(Tier::K, "sha256:A"),
                    &NoFacts,
                )
                .unwrap(),
                "closed",
                true,
            ))],
            ..cfg(Tier::K, "sha256:A")
        },
        &NoFacts,
    )
    .unwrap();
    assert_eq!(k.own_keys().len(), 2);
    assert_eq!(k.summary().verdict, Verdict::SupersetClean);
    // a leaf set of the EXACT tier serves the keys-only walk (the witness-absent policy is bound as configured, not as
    // the tier-effective identity word)
    let exact = Arc::new(leafset(&base, "closed", true));
    let k2 = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 5, 0, 0]]),
        &ClosureConfig {
            leaves: vec![exact],
            ..cfg(Tier::K, "sha256:A")
        },
        &NoFacts,
    )
    .unwrap();
    assert_eq!(k2.own_keys().len(), 2);
}

#[test]
fn an_edge_into_a_leaf_is_still_checked_strictly_lower() {
    // record 0 on n1 <= 2 RAISES line 1: (3,2) -> (3,3) is not lower; (3,3) is a leaf of a closed set
    let (d, b) = node("I(n0,n1-1,n2,n3)", Some(2));
    let cf = cfg(Tier::C, "sha256:A");
    // the leaf set {(3,3)} (built by hand: no relation set / terminal / exclusion here, so the walk binding of any
    // key set of this configuration equals the run's)
    let probe = closure(&d, &b, &cmp(), &ks(&[[9, 9, 0, 0]]), &cf, &AllRegular).unwrap();
    let mut lb = LeafSetBuilder::new(codec(), vec![], vec![]);
    lb.push(&[3, 3, 0, 0], "table", "per-key", true).unwrap();
    let leaf = Arc::new(
        lb.finish(LeafBinding {
            statement_digest: "x".into(),
            source_verdict: "closed".into(),
            walk: probe.leaf_walk().clone(),
            producer: Some(LeafProducer::new("per-key", Tier::C, true)),
            ..Default::default()
        })
        .unwrap(),
    );
    let c = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 2, 0, 0]]),
        &ClosureConfig {
            leaves: vec![leaf],
            ..cf
        },
        &AllRegular,
    )
    .unwrap();
    let s = c.summary();
    assert_eq!(s.descent_violations, 1, "{:?}", s.blocking);
    assert_eq!(s.verdict, Verdict::Failed);
    assert_eq!(s.leaves.unwrap().reached, 1);
}
