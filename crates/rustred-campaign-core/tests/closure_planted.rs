//! Planted negatives and tier semantics on a synthetic node (arity 4: lines 0,1; numerators 2,3), default test run.

mod closure_common;

use closure_common::answer_facts;
use rustred_campaign_core::closure::{
    ClosureConfig, CoefFact, FactSource, HandDowns, KeyCodec, KeySet, NoFacts, OpenStatus, RegFact,
    RequestReason, RuleBodies, RuleId, Sectors, Tier, Verdict, closure, write_fact_requests,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement, Refusal, serve_by_image};

/// A rule of the synthetic tables: fixed slots, body patterns.
type Spec<'a> = ([Option<i32>; 4], &'a [&'a str]);
/// (fixed slots, body, elided witness loci).
type RuleSpec<'a> = ([Option<i32>; 4], Vec<&'a str>, bool);

const CHART: [bool; 4] = [true, true, false, false];

fn dom(fixed: [Option<i32>; 4]) -> CaseDomain {
    let mut d = CaseDomain::free(4, Some(CHART.to_vec()));
    d.fixed = fixed.to_vec();
    d
}

/// One table "T" with the given (fixed, body) rules; returns dispatcher and bodies.
fn node(rules: &[RuleSpec]) -> (Dispatcher, RuleBodies) {
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
    KeySet::from_keys(
        KeyCodec::new(4, 6).unwrap(),
        keys.iter().map(|k| k.as_slice()),
    )
    .unwrap()
}

fn cfg(tier: Tier) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 2,
        hand_downs: HandDowns::Keys,
        table_digests: [("T".to_string(), "sha256:test-table-T".to_string())].into(),
        ..Default::default()
    }
}

/// A descending chain to the subsector: Tier K is a SUPERSET verdict, never closed; hand-downs recorded per child.
#[test]
fn descending_chain_superset_clean_and_hand_downs() {
    let (d, b) = node(&[([None; 4], vec!["I(n0-1,n1,n2,n3)"], false)]);
    let cl = closure(
        &d,
        &b,
        &cmp(),
        &start(&[[3, 1, 0, 0]]),
        &cfg(Tier::K),
        &NoFacts,
    )
    .unwrap();
    let s = cl.summary();
    assert_eq!(
        (s.keys_total, s.edges, s.rounds, s.open_total),
        (3, 2, 3, 0)
    );
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
    let (d, b) = node(&[(
        [None; 4],
        vec!["I(n0-1,n1,n2,n3)", "I(n0-1,n1,n2+1,n3)"],
        false,
    )]);
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
    assert_eq!(
        reqs.iter()
            .filter(|r| r.reason == RequestReason::PositiveNumerator)
            .count(),
        2
    );
    assert_eq!(
        reqs.iter()
            .filter(|r| r.reason == RequestReason::Regularity)
            .count(),
        2
    );
    let mut buf = vec![];
    write_fact_requests(&reqs, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap().lines().count(), 4);
    // answers: both keys regular; the positive term zero at [2,1,0,0] and at [1,1,0,0]
    let base: Vec<(&[i32], Option<usize>, &str)> = vec![
        (&[2, 1, 0, 0], None, "regular"),
        (&[1, 1, 0, 0], None, "regular"),
    ];
    let zero = [
        base.clone(),
        vec![
            (&[2, 1, 0, 0][..], Some(1), "zero"),
            (&[1, 1, 0, 0], Some(1), "zero"),
        ],
    ]
    .concat();
    let f = answer_facts(&reqs, &zero, "exact", false);
    let s = cl_tier(&d, &b, &st, Tier::C, &f);
    assert_eq!(s.verdict, Verdict::Closed);
    assert!(s.blocking.is_empty());
    assert_eq!(s.terms.dropped_zero, 2);
    let f = answer_facts(&reqs, &zero, "modular-2prime", true);
    assert_eq!(
        cl_tier(&d, &b, &st, Tier::C, &f).verdict,
        Verdict::ClosedProvisional
    );
    let f = answer_facts(&reqs, &zero, "modular-2prime", false);
    assert_eq!(
        cl_tier(&d, &b, &st, Tier::C, &f).verdict,
        Verdict::Undecided
    );
    // without the table's coefficient-file digest: at best closed_provisional (M4)
    let f = answer_facts(&reqs, &zero, "exact", false);
    let nodig = ClosureConfig {
        table_digests: Default::default(),
        ..cfg(Tier::C)
    };
    let s = closure(&d, &b, &cmp(), &st, &nodig, &f).unwrap().summary();
    // (the requests were bound to rule digests WITH the table digest: the facts do not apply)
    assert_eq!(s.verdict, Verdict::Undecided);
    let reqs2 = closure(&d, &b, &cmp(), &st, &nodig, &NoFacts)
        .unwrap()
        .fact_requests();
    let f = answer_facts(&reqs2, &zero, "exact", false);
    let s = closure(&d, &b, &cmp(), &st, &nodig, &f).unwrap().summary();
    assert_eq!(s.verdict, Verdict::ClosedProvisional);
    assert!(
        s.blocking
            .contains(&"table_coefficient_digest_missing:T".to_string()),
        "{:?}",
        s.blocking
    );
    // NonZero at the start key: the closure FAILS at that edge, never drops it
    let v = [
        base.clone(),
        vec![
            (&[2, 1, 0, 0][..], Some(1), "nonzero"),
            (&[1, 1, 0, 0], Some(1), "zero"),
        ],
    ]
    .concat();
    let f = answer_facts(&reqs, &v, "exact", false);
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::C), &f).unwrap();
    let s = c.summary();
    assert_eq!(s.verdict, Verdict::Failed);
    assert_eq!(s.positive_numerator_nonzero, 1);
    let w = &c.failure_witnesses()[0];
    assert_eq!(
        (w.key.clone(), w.term, w.rhs.clone(), w.reason.as_str()),
        (
            vec![2, 1, 0, 0],
            1,
            vec![1, 1, 1, 0],
            "positive_numerator_nonzero"
        )
    );
    // a pole at a key: the key is open (irregular), naming the rule
    let v: Vec<(&[i32], Option<usize>, &str)> = vec![
        (&[2, 1, 0, 0], None, "regular"),
        (&[1, 1, 0, 0], None, "pole"),
        (&[2, 1, 0, 0], Some(1), "zero"),
    ];
    let f = answer_facts(&reqs, &v, "exact", false);
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::C), &f).unwrap();
    let o = c.open_keys();
    assert_eq!(o.len(), 1);
    assert_eq!(
        (o[0].key.clone(), o[0].status),
        (vec![1, 1, 0, 0], OpenStatus::Irregular)
    );
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
    let cl = closure(
        &d,
        &b,
        &cmp(),
        &start(&[[2, 1, 0, 0], [3, 1, 0, 0]]),
        &cfg(Tier::K),
        &NoFacts,
    )
    .unwrap();
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
        (
            [Some(2), Some(1), Some(0), Some(0)],
            vec!["I(3,1,0,0)"],
            false,
        ),
        (
            [Some(3), Some(1), Some(0), Some(0)],
            vec!["I(2,1,0,0)", "I(1,1,0,0)"],
            false,
        ),
        ([None; 4], vec!["I(n0-1,n1,n2,n3)"], false),
    ]);
    let cl = closure(
        &d,
        &b,
        &cmp(),
        &start(&[[2, 1, 0, 0]]),
        &cfg(Tier::K),
        &NoFacts,
    )
    .unwrap();
    let s = cl.summary();
    assert_eq!(s.verdict, Verdict::Failed);
    assert_eq!(s.descent_violations, 1);
    assert_eq!(s.cycles, Some(1));
    let w = &cl.descent_witnesses()[0];
    assert_eq!(
        (w.key.clone(), w.rule.label, w.term, w.rhs.clone()),
        (vec![2, 1, 0, 0], 0, 0, vec![3, 1, 0, 0])
    );
    assert_eq!(w.reason, "descent_violation");
    // Tier C: the non-lower term with a verified zero coefficient is dropped (no edge, no cycle)
    let reqs = closure(
        &d,
        &b,
        &cmp(),
        &start(&[[2, 1, 0, 0]]),
        &cfg(Tier::C),
        &NoFacts,
    )
    .unwrap()
    .fact_requests();
    let spec: Vec<(&[i32], Option<usize>, &str)> = vec![
        (&[2, 1, 0, 0], Some(0), "zero"),
        (&[2, 1, 0, 0], None, "regular"),
    ];
    let f = answer_facts(&reqs, &spec, "exact", false);
    let c = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &cfg(Tier::C), &f).unwrap();
    let s = c.summary();
    assert_eq!(
        (s.descent_violations, s.cycles, s.keys_total, s.verdict),
        (0, Some(0), 1, Verdict::Closed)
    );
}

struct ZeroAll;
impl FactSource for ZeroAll {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
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
    let swap = GroupElement {
        lines: vec![1, 0],
        numerators: None,
    };
    let id = GroupElement {
        lines: vec![0, 1],
        numerators: None,
    };
    let pol = CreditPolicy::new(vec![id.clone(), swap.clone()]);
    let sec = Sectors::new(CHART.to_vec());
    // rule at (3,1): RHS (1,1) and (2,1)
    let (d, b) = node(&[(
        [Some(3), Some(1), Some(0), Some(0)],
        vec!["I(1,1,0,0)", "I(2,1,0,0)"],
        false,
    )]);
    let k = [1, 3, 0, 0];
    let c = serve_by_image(&k, &pol, &d, &b, &cmp(), &sec, &NoFacts, Tier::K)
        .unwrap()
        .unwrap();
    assert_eq!(
        (c.element, c.image.clone(), c.rule.label),
        (1, vec![3, 1, 0, 0], 0)
    );
    assert_eq!(c.top_terms, vec![vec![1, 1, 0, 0], vec![2, 1, 0, 0]]);
    // planted: a rule term above the ORIGINAL key: (1,4,0,0) has D = 3 > D(1,3,0,0) = 2
    let (d2, b2) = node(&[(
        [Some(3), Some(1), Some(0), Some(0)],
        vec!["I(1,1,0,0)", "I(1,4,0,0)"],
        false,
    )]);
    let r = serve_by_image(&k, &pol, &d2, &b2, &cmp(), &sec, &NoFacts, Tier::K)
        .unwrap()
        .unwrap_err();
    assert!(
        r.iter()
            .any(|(e, x)| *e == 1 && matches!(x, Refusal::TermNotBelow { term: 1, .. })),
        "{r:?}"
    );
    assert!(
        r.iter()
            .any(|(e, x)| *e == 0 && *x == Refusal::ImageEqualsKey)
    );
    // planted: the image is HIGHER than k and its rule's term is below the image but lands ON k (not strictly
    // below): k = (3,2,0,0), g k = (2,3,0,0) (legacy order: larger excess on slot 0 = lower, so k < g k), rule at
    // g k with the term (3,2,0,0) = k: refused (an "image lower" or "term below image" test would mis-handle it)
    let cm = cmp();
    let k3 = [3, 2, 0, 0];
    assert!(cm.lower(&k3, &[2, 3, 0, 0]));
    let (d3, b3) = node(&[(
        [Some(2), Some(3), Some(0), Some(0)],
        vec!["I(1,1,0,0)", "I(3,2,0,0)"],
        false,
    )]);
    let r = serve_by_image(&k3, &pol, &d3, &b3, &cm, &sec, &NoFacts, Tier::K)
        .unwrap()
        .unwrap_err();
    assert!(
        r.iter().any(|(e, x)| *e == 1
            && *x
                == Refusal::TermNotBelow {
                    term: 1,
                    rhs: vec![3, 2, 0, 0]
                }),
        "{r:?}"
    );
    // an uncovered image never credits (no image of an image)
    let (d4, b4) = node(&[(
        [Some(5), Some(5), Some(0), Some(0)],
        vec!["I(1,1,0,0)"],
        false,
    )]);
    let r = serve_by_image(&k, &pol, &d4, &b4, &cmp(), &sec, &NoFacts, Tier::K)
        .unwrap()
        .unwrap_err();
    assert!(
        r.iter()
            .any(|(_, x)| matches!(x, Refusal::ImageNotCovered(_)))
    );
    // Tier C: a positive-numerator term at g k with a non-zero coefficient refuses the credit
    let (d5, b5) = node(&[(
        [Some(3), Some(1), None, Some(0)],
        vec!["I(1,1,n2,0)", "I(1,1,n2+1,0)"],
        false,
    )]);
    let r = serve_by_image(&k, &pol, &d5, &b5, &cmp(), &sec, &ZeroAll, Tier::C)
        .unwrap()
        .unwrap_err();
    assert!(
        r.iter()
            .any(|(_, x)| matches!(x, Refusal::CoefficientNonZero { term: 1 })),
        "{r:?}"
    );
    // the closure with credits: k credited, its composite step followed
    let cfgc = ClosureConfig {
        credits: Some(pol),
        ..cfg(Tier::K)
    };
    let cl = closure(&d, &b, &cmp(), &start(&[k]), &cfgc, &NoFacts).unwrap();
    let s = cl.summary();
    assert_eq!((s.credited, s.credited_in_start), (1, 1));
    let cr = &cl.credits()[0];
    assert_eq!(
        (
            cr.key.clone(),
            cr.element,
            cr.image.clone(),
            cr.descent_checked
        ),
        (k.to_vec(), 1, vec![3, 1, 0, 0], true)
    );
    // (1,1) and (2,1) reached; (2,1) is uncovered here (only (3,1) has a rule) -> credit tried via swap: (1,2) uncovered
    assert_eq!(s.descent_violations, 0);
}

// ------------------------------------------------------------------------------------------------ genericity (L8)

/// A toy node of another arity (7 slots: lines 0,1,2; numerators 3..6) at 8 bits per slot: the closure, the
/// recheck with an extra (envelope) key set, declared terminals and the measured/proved labels.
#[test]
fn toy_arity7_recheck_terminals_provisional() {
    use rustred_campaign_core::closure::{
        InputLabel, InputProvenance, TerminalKind, TerminalSets, closure_recheck,
    };
    let n = 7;
    let chart: Vec<bool> = (0..n).map(|s| s < 3).collect();
    let mut t = Table::new("toy", n);
    // r0: line 0 >= 2 -> lower line 0;  r1: line 1 >= 2 (line 0 = 1) -> lower line 1 and raise numerator 3 by -1
    let mut d0 = CaseDomain::free(n, Some(chart.clone()));
    d0.equations = vec![];
    let mut d1 = d0.clone();
    d1.fixed[0] = Some(1);
    t.push(1, d1).unwrap();
    t.push(0, d0).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&disp);
    b.set(
        &disp,
        0,
        0,
        &["I(n0,n1-1,n2,n3,n4,n5,n6)", "I(n0,n1-1,n2,n3+1,n4,n5,n6)"],
    )
    .unwrap();
    b.set(&disp, 0, 1, &["I(n0-1,n1,n2,n3,n4,n5,n6)"]).unwrap();
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let codec = KeyCodec::new(n, 8).unwrap();
    let ks = |v: &[[i32; 7]]| KeySet::from_keys(codec, v.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 3,
        bits: 8,
        hand_downs: HandDowns::Keys,
        inputs: vec![InputLabel {
            name: "box".into(),
            digest: "x".into(),
            provenance: InputProvenance::Proved,
        }],
        ..Default::default()
    };
    let start = ks(&[[3, 2, 1, 0, 0, 0, 0]]);
    let c = closure(&disp, &b, &cmp, &start, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    // (3,2,1) -> (2,2,1) -> (1,2,1) -> (1,1,1) -> hand-down (1,0,1); numerator-raising term flagged
    assert_eq!((s.keys_total, s.edges, s.open_total), (4, 3, 0));
    assert_eq!(s.requires_coefficient_zero, 2); // r1 fires at (1,2,1) and (1,1,1)
    assert!(!s.provisional);
    assert_eq!(s.verdict, Verdict::SupersetClean);
    // recheck with a MEASURED envelope key set: only new keys are processed; equals the from-scratch closure
    let env = ks(&[[4, 3, 1, 0, 0, 0, 0], [2, 2, 1, 0, 0, 0, 0]]);
    let cfg2 = ClosureConfig {
        inputs: vec![InputLabel {
            name: "envelope".into(),
            digest: env.digest(),
            provenance: InputProvenance::Measured,
        }],
        ..cfg.clone()
    };
    let r = closure_recheck(&c, &disp, &b, &cmp, &env, &cfg2, &NoFacts).unwrap();
    let both = start.union(&env);
    let cfg_both = ClosureConfig {
        inputs: [cfg.inputs.clone(), cfg2.inputs.clone()].concat(),
        ..cfg.clone()
    };
    let f = closure(&disp, &b, &cmp, &both, &cfg_both, &NoFacts).unwrap();
    let (rs, fs) = (r.summary(), f.summary());
    assert_eq!(r.keys(), f.keys());
    assert_eq!(
        (rs.keys_total, rs.keys_in_start, rs.edges, rs.open_total),
        (fs.keys_total, fs.keys_in_start, fs.edges, fs.open_total)
    );
    assert_eq!(rs.used_rules, fs.used_rules);
    assert!(rs.provisional);
    assert_eq!(rs.start_set_digest, fs.start_set_digest);
    // declared terminals take precedence over dispatch: (1,2,1) preferred, (2,2,1) other
    let cfg3 = ClosureConfig {
        terminals: Some(TerminalSets {
            preferred: Some(ks(&[[1, 2, 1, 0, 0, 0, 0]])),
            other: Some(ks(&[[2, 2, 1, 0, 0, 0, 0]])),
        }),
        ..cfg.clone()
    };
    let c3 = closure(&disp, &b, &cmp, &start, &cfg3, &NoFacts).unwrap();
    let s3 = c3.summary();
    assert_eq!(
        (
            s3.keys_total,
            s3.terminals_preferred,
            s3.terminals_other,
            s3.open_total
        ),
        (2, 0, 1, 0)
    );
    let tr = c3.terminal_records();
    assert_eq!(tr[0].kind, TerminalKind::Other);
    assert_eq!(
        tr[0].would_dispatch.as_ref().unwrap().label,
        0,
        "inside rule r0's domain, not applied"
    );
    // a codec too narrow is a typed error, never a wrap
    assert!(KeyCodec::new(40, 16).is_err());
    let narrow = KeyCodec::new(n, 4).unwrap();
    assert!(KeySet::from_keys(narrow, [[9, 1, 1, 0, 0, 0, 0].as_slice()]).is_err());
}

/// Arity 27 (a six-loop propagator family size) packs at the default 6 bits; a closure runs unchanged.
#[test]
fn toy_arity27_runs() {
    let n = 27;
    let chart: Vec<bool> = (0..n).map(|s| s < 15).collect();
    let mut t = Table::new("six", n);
    t.push(0, CaseDomain::free(n, Some(chart))).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&disp);
    let pat = format!(
        "I({})",
        (0..n)
            .map(|s| if s == 14 {
                "n14-1".to_string()
            } else {
                format!("n{s}")
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    b.set(&disp, 0, 0, &[pat]).unwrap();
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let mut k: Vec<i32> = (0..n).map(|s| if s < 15 { 1 } else { 0 }).collect();
    k[14] = 6;
    let st = KeySet::from_keys(KeyCodec::new(n, 6).unwrap(), [k.as_slice()]).unwrap();
    let c = closure(&disp, &b, &cmp, &st, &ClosureConfig::default(), &NoFacts).unwrap();
    let s = c.summary();
    assert_eq!(
        (s.keys_total, s.edges, s.open_total, s.cycles),
        (6, 5, 0, Some(0))
    );
    assert_eq!(s.longest_chain.unwrap().edges, 5);
}

/// R44: image dispatch ranges over the rules in force and is re-evaluated after every change: a key becomes
/// creditable only after a second table is added (incremental closure), and its credit is invalidated when a new
/// higher-priority rule replaces the winner at the image with one whose term lands above the key.
#[test]
fn credits_follow_table_changes() {
    use rustred_campaign_core::closure::{closure_incremental, credit_diff};
    let tab = |name: &str, rules: &[Spec]| {
        let mut t = Table::new(name, 4);
        for (i, (f, _)) in rules.iter().enumerate() {
            t.push(i as i64, dom(*f)).unwrap();
        }
        t
    };
    let ra: &[Spec] = &[([Some(5), Some(5), Some(0), Some(0)], &["I(1,1,0,0)"])];
    let rb: &[Spec] = &[(
        [Some(3), Some(1), Some(0), Some(0)],
        &["I(1,1,0,0)", "I(2,1,0,0)"],
    )];
    let rc: &[Spec] = &[([Some(3), Some(1), Some(0), Some(0)], &["I(1,4,0,0)"])];
    let build = |specs: &[(&str, &[Spec])], prio: Vec<usize>| {
        let d = Dispatcher::new(specs.iter().map(|(n, r)| tab(n, r)).collect(), prio).unwrap();
        let mut b = RuleBodies::new(&d);
        for (t, (_, r)) in specs.iter().enumerate() {
            for (i, (_, body)) in r.iter().enumerate() {
                b.set(&d, t, i, body).unwrap();
            }
        }
        (d, b)
    };
    let pol = CreditPolicy::new(vec![GroupElement {
        lines: vec![1, 0],
        numerators: None,
    }]);
    let cfgc = ClosureConfig {
        credits: Some(pol),
        ..cfg(Tier::K)
    };
    let st = start(&[[1, 3, 0, 0]]);
    let (d1, b1) = build(&[("A", ra)], vec![0]);
    let c1 = closure(&d1, &b1, &cmp(), &st, &cfgc, &NoFacts).unwrap();
    assert_eq!((c1.summary().credited, c1.summary().open_total), (0, 1));
    let (d2, b2) = build(&[("A", ra), ("B", rb)], vec![0, 1]);
    let c2 =
        closure_incremental(&c1.dispatch_cache(), &d2, &b2, &cmp(), &st, &cfgc, &NoFacts).unwrap();
    assert_eq!(
        c2.summary(),
        closure(&d2, &b2, &cmp(), &st, &cfgc, &NoFacts)
            .unwrap()
            .summary()
    );
    assert_eq!(c2.summary().credited, 1);
    let (inval, created) = credit_diff(&c1, &c2);
    assert!(inval.is_empty());
    assert_eq!(created.len(), 1);
    assert_eq!(
        (created[0].image.clone(), created[0].rule.table.clone()),
        (vec![3, 1, 0, 0], "B".to_string())
    );
    let (d3, b3) = build(&[("A", ra), ("B", rb), ("C", rc)], vec![2, 0, 1]);
    let c3 =
        closure_incremental(&c2.dispatch_cache(), &d3, &b3, &cmp(), &st, &cfgc, &NoFacts).unwrap();
    assert_eq!(
        c3.summary(),
        closure(&d3, &b3, &cmp(), &st, &cfgc, &NoFacts)
            .unwrap()
            .summary()
    );
    let (inval, created) = credit_diff(&c2, &c3);
    assert_eq!(inval.len(), 1);
    assert_eq!(inval[0].reason, "refused");
    assert!(created.is_empty());
}
