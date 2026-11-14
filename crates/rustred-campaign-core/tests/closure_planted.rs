//! Planted negatives and tier semantics on a synthetic node (arity 4: lines 0,1; numerators 2,3), default test run.

use rustred_campaign_core::closure::{
    ClosureConfig, CoefFact, FactSource, FileFacts, HandDowns, KeyCodec, KeySet, NoFacts, OpenStatus, RegFact,
    RequestReason, RuleBodies, RuleId, Sectors, Tier, Verdict, closure, write_fact_requests,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::symmetry::{CreditFrame, CreditPolicy, GroupElement, Refusal, serve_by_image};

const CHART: [bool; 4] = [true, true, false, false];

fn dom(fixed: [Option<i32>; 4]) -> CaseDomain {
    let mut d = CaseDomain::free(4, Some(CHART.to_vec()));
    d.fixed = fixed.to_vec();
    d
}

/// One table "T" with the given (fixed, body) rules; returns dispatcher and bodies.
fn node(rules: &[([Option<i32>; 4], Vec<&str>, bool)]) -> (Dispatcher, RuleBodies) {
    let mut t = Table::new("T", 4);
    for (i, (f, _, elided)) in rules.iter().enumerate() {
        let mut d = dom(*f);
        d.witness_elided = *elided;
        t.push(i as i64, d).unwrap();
    }
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&disp);
    for (i, (_, body, _)) in rules.iter().enumerate() {
        b.set(&disp, 0, i, body).unwrap();
    }
    (disp, b)
}

fn cmp() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}

fn start(keys: &[[i32; 4]]) -> KeySet {
    KeySet::from_keys(KeyCodec::new(4, 6).unwrap(), keys.iter().map(|k| k.as_slice())).unwrap()
}

fn cfg(tier: Tier) -> ClosureConfig {
    ClosureConfig { tier, threads: 2, hand_downs: HandDowns::Keys, ..Default::default() }
}

fn fact_line(record: usize, key: &[i32], kind: &str, term: Option<usize>, value: &str, method: &str) -> String {
    let mut v = serde_json::json!({"schema":"rustred.campaign.fact.v1","table":"T","record":record,"label":record as i64,
        "key":key,"kind":kind,"value":value,"method":method});
    if let Some(t) = term {
        v["term"] = t.into();
    }
    v.to_string()
}

/// A descending chain to the subsector: Tier K is a SUPERSET verdict, never closed; hand-downs recorded per child.
#[test]
fn descending_chain_superset_clean_and_hand_downs() {
    let (d, b) = node(&[([None; 4], vec!["I(n0-1,n1,n2,n3)"], false)]);
    let cl = closure(&d, &b, &cmp(), &start(&[[3, 1, 0, 0]]), &cfg(Tier::K), &NoFacts).unwrap();
    let s = cl.summary();
    assert_eq!((s.keys_total, s.edges, s.rounds, s.open_total), (3, 2, 3, 0));
    assert_eq!(s.verdict, Verdict::SupersetClean);
    assert_eq!(s.longest_chain.unwrap().edges, 2);
    let hd = cl.hand_down_sets();
    assert_eq!(hd.len(), 1);
    assert_eq!(hd[&1].iter().collect::<Vec<_>>(), vec![vec![0, 1, 0, 0]]);
}

/// Positive-numerator term: Tier K flags it (requires coefficient zero), Tier C needs a verified zero: NonZero ->
/// failed (witness), Unknown -> undecided (requests), Zero -> dropped -> closed; provisional facts ->
/// closed_provisional.
#[test]
fn positive_numerator_tiers() {
    let (d, b) = node(&[([None; 4], vec!["I(n0-1,n1,n2,n3)", "I(n0-1,n1,n2+1,n3)"], false)]);
    let st = start(&[[2, 1, 0, 0]]);
    let k = cl_tier(&d, &b, &st, Tier::K, &NoFacts);
    assert_eq!(k.verdict, Verdict::SupersetClean);
    assert_eq!(k.requires_coefficient_zero, 2);
    assert_eq!(k.terms.positive_numerator_top, 1);
    assert_eq!(k.terms.positive_numerator_sub, 1);
    assert_eq!(k.terms.positive_numerator_symbolic, 2);
    // Tier C without facts: undecided, with requests
    let c0 = closure(&d, &b, &cmp(), &st, &cfg(Tier::C), &NoFacts).unwrap();
    assert_eq!(c0.summary().verdict, Verdict::Undecided);
    let reqs = c0.fact_requests();
    assert_eq!(reqs.iter().filter(|r| r.reason == RequestReason::PositiveNumerator).count(), 2);
    assert_eq!(reqs.iter().filter(|r| r.reason == RequestReason::Regularity).count(), 2);
    let mut buf = vec![];
    write_fact_requests(&reqs, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap().lines().count(), 4);
    // answers: both keys regular; the positive term zero at [2,1,0,0] and at [1,1,0,0]
    let base = [
        fact_line(0, &[2, 1, 0, 0], "regularity", None, "regular", "exact"),
        fact_line(0, &[1, 1, 0, 0], "regularity", None, "regular", "exact"),
    ];
    let zero = |m: &str| {
        let mut v = base.to_vec();
        v.push(fact_line(0, &[2, 1, 0, 0], "coefficient", Some(1), "zero", m));
        v.push(fact_line(0, &[1, 1, 0, 0], "coefficient", Some(1), "zero", m));
        v.join("\n")
    };
    let f = FileFacts::read(zero("exact").as_bytes(), false).unwrap();
    let s = cl_tier(&d, &b, &st, Tier::C, &f);
    assert_eq!(s.verdict, Verdict::Closed);
    assert_eq!(s.terms.dropped_zero, 2);
    let f = FileFacts::read(zero("modular-2prime").as_bytes(), true).unwrap();
    assert_eq!(cl_tier(&d, &b, &st, Tier::C, &f).verdict, Verdict::ClosedProvisional);
    let f = FileFacts::read(zero("modular-2prime").as_bytes(), false).unwrap();
    assert_eq!(cl_tier(&d, &b, &st, Tier::C, &f).verdict, Verdict::Undecided);
    // NonZero at the start key: the closure FAILS at that edge, never drops it
    let mut v = base.to_vec();
    v.push(fact_line(0, &[2, 1, 0, 0], "coefficient", Some(1), "nonzero", "exact"));
    v.push(fact_line(0, &[1, 1, 0, 0], "coefficient", Some(1), "zero", "exact"));
    let f = FileFacts::read(v.join("\n").as_bytes(), false).unwrap();
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::C), &f).unwrap();
    let s = c.summary();
    assert_eq!(s.verdict, Verdict::Failed);
    assert_eq!(s.positive_numerator_nonzero, 1);
    let w = &c.failure_witnesses()[0];
    assert_eq!((w.key.clone(), w.term, w.rhs.clone(), w.reason.as_str()), (vec![2, 1, 0, 0], 1, vec![1, 1, 1, 0], "positive_numerator_nonzero"));
    // a pole at a key: the key is open (irregular), naming the rule
    let mut v = base.to_vec();
    v[1] = fact_line(0, &[1, 1, 0, 0], "regularity", None, "pole", "exact");
    v.push(fact_line(0, &[2, 1, 0, 0], "coefficient", Some(1), "zero", "exact"));
    let f = FileFacts::read(v.join("\n").as_bytes(), false).unwrap();
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::C), &f).unwrap();
    let o = c.open_keys();
    assert_eq!(o.len(), 1);
    assert_eq!((o[0].key.clone(), o[0].status), (vec![1, 1, 0, 0], OpenStatus::Irregular));
    assert_eq!(o[0].rule.as_ref().unwrap().table, "T");
}

fn cl_tier(
    d: &Dispatcher,
    b: &RuleBodies,
    st: &KeySet,
    tier: Tier,
    f: &dyn FactSource,
) -> rustred_campaign_core::closure::ClosureSummary {
    closure(d, b, &cmp(), st, &cfg(tier), f).unwrap().summary()
}

/// An elided (blocked) winner leaves the key open and names the rule; a later rule that would apply is not used.
#[test]
fn blocked_winner_is_open() {
    let (d, b) = node(&[
        ([Some(2), None, None, None], vec!["I(n0-1,n1,n2,n3)"], true),
        ([None; 4], vec!["I(n0-1,n1,n2,n3)"], false),
    ]);
    let cl = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0], [3, 1, 0, 0]]), &cfg(Tier::K), &NoFacts).unwrap();
    let o = cl.open_keys();
    assert_eq!(o.len(), 1);
    assert_eq!(o[0].key, vec![2, 1, 0, 0]);
    assert_eq!(o[0].status, OpenStatus::Blocked);
    assert_eq!(o[0].rule.as_ref().unwrap().label, 0);
    assert!(o[0].detail.contains("Elided"));
    assert_eq!(cl.summary().verdict, Verdict::Open);
}

/// A cycle planted through a non-descending rule: descent violation with its witness, cycle found, failed.
#[test]
fn planted_cycle_is_a_descent_violation() {
    let (d, b) = node(&[
        ([Some(2), Some(1), Some(0), Some(0)], vec!["I(3,1,0,0)"], false),
        ([Some(3), Some(1), Some(0), Some(0)], vec!["I(2,1,0,0)", "I(1,1,0,0)"], false),
        ([None; 4], vec!["I(n0-1,n1,n2,n3)"], false),
    ]);
    let cl = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &cfg(Tier::K), &NoFacts).unwrap();
    let s = cl.summary();
    assert_eq!(s.verdict, Verdict::Failed);
    assert_eq!(s.descent_violations, 1);
    assert_eq!(s.cycles, Some(1));
    let w = &cl.descent_witnesses()[0];
    assert_eq!((w.key.clone(), w.rule.label, w.term, w.rhs.clone()), (vec![2, 1, 0, 0], 0, 0, vec![3, 1, 0, 0]));
    assert_eq!(w.reason, "descent_violation");
    // Tier C: the non-lower term with a verified zero coefficient is dropped (no edge, no cycle)
    let facts = [
        fact_line(0, &[2, 1, 0, 0], "coefficient", Some(0), "zero", "exact"),
        fact_line(0, &[2, 1, 0, 0], "regularity", None, "regular", "exact"),
    ]
    .join("\n");
    let f = FileFacts::read(facts.as_bytes(), false).unwrap();
    let c = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &cfg(Tier::C), &f).unwrap();
    let s = c.summary();
    assert_eq!((s.descent_violations, s.cycles, s.keys_total, s.verdict), (0, Some(0), 1, Verdict::Closed));
}

struct ZeroAll;
impl FactSource for ZeroAll {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize) -> CoefFact {
        CoefFact::NonZero
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Regular
    }
    fn label(&self) -> String {
        "nonzero-everywhere".into()
    }
}

/// Symmetry credit (adopted condition): accepted when every top term at g k is below k; refused when a term lands
/// above the ORIGINAL key (even though it is below g k); never via an uncovered image; closure reports credits.
#[test]
fn symmetry_credit_adopted_condition() {
    let swap = GroupElement { lines: vec![1, 0], numerators: None };
    let id = GroupElement { lines: vec![0, 1], numerators: None };
    let pol = CreditPolicy { group: vec![id.clone(), swap.clone()], tier: Tier::K, frame: CreditFrame::Direct };
    let sec = Sectors::new(CHART.to_vec());
    // rule at (3,1): RHS (1,1) and (2,1)
    let (d, b) = node(&[([Some(3), Some(1), Some(0), Some(0)], vec!["I(1,1,0,0)", "I(2,1,0,0)"], false)]);
    let k = [1, 3, 0, 0];
    let c = serve_by_image(&k, &pol, &d, &b, &cmp(), &sec, &NoFacts, true).unwrap().unwrap();
    assert_eq!((c.element, c.image.clone(), c.rule.label), (1, vec![3, 1, 0, 0], 0));
    assert_eq!(c.top_terms, vec![vec![1, 1, 0, 0], vec![2, 1, 0, 0]]);
    // planted: a rule term above the ORIGINAL key: (1,4,0,0) has D = 3 > D(1,3,0,0) = 2
    let (d2, b2) = node(&[([Some(3), Some(1), Some(0), Some(0)], vec!["I(1,1,0,0)", "I(1,4,0,0)"], false)]);
    let r = serve_by_image(&k, &pol, &d2, &b2, &cmp(), &sec, &NoFacts, true).unwrap().unwrap_err();
    assert!(r.iter().any(|(e, x)| *e == 1 && matches!(x, Refusal::TermNotBelow { term: 1, .. })), "{r:?}");
    assert!(r.iter().any(|(e, x)| *e == 0 && *x == Refusal::ImageEqualsKey));
    // planted: the image is HIGHER than k and its rule's term is below the image but lands ON k (not strictly
    // below): k = (3,2,0,0), g k = (2,3,0,0) (legacy order: larger excess on slot 0 = lower, so k < g k), rule at
    // g k with the term (3,2,0,0) = k: refused (an "image lower" or "term below image" test would mis-handle it)
    let cm = cmp();
    let k3 = [3, 2, 0, 0];
    assert!(cm.lower(&k3, &[2, 3, 0, 0]));
    let (d3, b3) = node(&[([Some(2), Some(3), Some(0), Some(0)], vec!["I(1,1,0,0)", "I(3,2,0,0)"], false)]);
    let r = serve_by_image(&k3, &pol, &d3, &b3, &cm, &sec, &NoFacts, true).unwrap().unwrap_err();
    assert!(
        r.iter().any(|(e, x)| *e == 1 && *x == Refusal::TermNotBelow { term: 1, rhs: vec![3, 2, 0, 0] }),
        "{r:?}"
    );
    // an uncovered image never credits (no image of an image)
    let (d4, b4) = node(&[([Some(5), Some(5), Some(0), Some(0)], vec!["I(1,1,0,0)"], false)]);
    let r = serve_by_image(&k, &pol, &d4, &b4, &cmp(), &sec, &NoFacts, true).unwrap().unwrap_err();
    assert!(r.iter().any(|(_, x)| matches!(x, Refusal::ImageNotCovered(_))));
    // Tier C: a positive-numerator term at g k with a non-zero coefficient refuses the credit
    let (d5, b5) = node(&[([Some(3), Some(1), None, Some(0)], vec!["I(1,1,n2,0)", "I(1,1,n2+1,0)"], false)]);
    let polc = CreditPolicy { tier: Tier::C, ..pol.clone() };
    let r = serve_by_image(&k, &polc, &d5, &b5, &cmp(), &sec, &ZeroAll, true).unwrap().unwrap_err();
    assert!(r.iter().any(|(_, x)| matches!(x, Refusal::CoefficientNonZero { term: 1 })), "{r:?}");
    // the closure with credits: k credited, its composite step followed
    let cfgc = ClosureConfig { credits: Some(pol), ..cfg(Tier::K) };
    let cl = closure(&d, &b, &cmp(), &start(&[k]), &cfgc, &NoFacts).unwrap();
    let s = cl.summary();
    assert_eq!((s.credited, s.credited_in_start), (1, 1));
    let cr = &cl.credits()[0];
    assert_eq!((cr.key.clone(), cr.element, cr.image.clone(), cr.descent_checked), (k.to_vec(), 1, vec![3, 1, 0, 0], true));
    // (1,1) and (2,1) reached; (2,1) is uncovered here (only (3,1) has a rule) -> credit tried via swap: (1,2) uncovered
    assert_eq!(s.descent_violations, 0);
}
