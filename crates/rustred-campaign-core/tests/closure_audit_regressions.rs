//! Regression tests of the audit findings on the closure / symmetry credits (audit loop-A, WS/notes/
//! implementer.audit-loop-A.md; the first six port the auditor's confirming tests with the FIXED expectation). One
//! test per finding id. Default suite (synthetic node: arity 4, lines 0,1; numerators 2,3).

mod closure_common;

use closure_common::answer_facts;
use rustred_campaign_core::closure::{
    Closure, ClosureConfig, ClosureError, CoefFact, DiagnosticRelaxations, ExclusionReason,
    FactSource, HandDowns, KeyCodec, KeyExclusion, KeySet, NoFacts, RegFact, RuleBodies, RuleId,
    TerminalDispatch, TerminalSets, Tier, Verdict, closure, closure_incremental, closure_recheck,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement, Refusal};

const CHART: [bool; 4] = [true, true, false, false];

fn dom(fixed: [Option<i32>; 4]) -> CaseDomain {
    let mut d = CaseDomain::free(4, Some(CHART.to_vec()));
    d.fixed = fixed.to_vec();
    d
}
fn cmp() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}
fn start(keys: &[[i32; 4]]) -> KeySet {
    KeySet::from_keys(KeyCodec::new(4, 6).unwrap(), keys.iter().map(|k| k.as_slice())).unwrap()
}
fn base(tier: Tier) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 1,
        hand_downs: HandDowns::Keys,
        table_digests: [("A".to_string(), "sha256:test-A".to_string())].into(),
        ..Default::default()
    }
}
type Spec<'a> = ([Option<i32>; 4], &'a [&'a str]);
fn build(specs: &[(&str, &[Spec])], prio: Vec<usize>) -> (Dispatcher, RuleBodies) {
    let tabs = specs
        .iter()
        .map(|(n, r)| {
            let mut t = Table::new(*n, 4);
            for (i, (f, _)) in r.iter().enumerate() {
                t.push(i as i64, dom(*f)).unwrap();
            }
            t
        })
        .collect();
    let d = Dispatcher::new(tabs, prio).unwrap();
    let mut b = RuleBodies::new(&d);
    for (t, (_, r)) in specs.iter().enumerate() {
        for (i, (_, body)) in r.iter().enumerate() {
            b.set(&d, t, i, body).unwrap();
        }
    }
    (d, b)
}
fn swap() -> GroupElement {
    GroupElement {
        lines: vec![1, 0],
        numerators: None,
    }
}
fn swap_policy() -> CreditPolicy {
    CreditPolicy::new(vec![swap()])
}
/// Everything a result reports, as one value (report minus timing).
fn view(c: &Closure) -> serde_json::Value {
    serde_json::to_value(c.report(&Default::default())).unwrap()
}

struct Constant(CoefFact, RegFact);
impl FactSource for Constant {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        self.0
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        self.1
    }
    fn label(&self) -> String {
        format!("constant:{:?}:{:?}", self.0, self.1)
    }
}

// ------------------------------------------------------------------------------------------------ CRITICAL

/// C1: a declared terminal of the previous run, dropped in the incremental run, must be dispatched over EVERY record
/// (its key has a rule: covered by R0, never credited, R0 in the used set).
#[test]
fn c1_incremental_after_dropped_terminal_equals_scratch() {
    let a: &[Spec] = &[
        ([Some(1), Some(3), Some(0), Some(0)], &["I(0,3,0,0)"]),
        ([Some(3), Some(1), Some(0), Some(0)], &["I(3,0,0,0)"]),
    ];
    let (d, b) = build(&[("A", a)], vec![0]);
    let k = [1, 3, 0, 0];
    let st = start(&[k]);
    let prev_cfg = ClosureConfig {
        terminals: Some(TerminalSets {
            preferred: None,
            other: Some(start(&[k])),
        }),
        credits: Some(swap_policy()),
        ..base(Tier::K)
    };
    let prev = closure(&d, &b, &cmp(), &st, &prev_cfg, &NoFacts).unwrap();
    assert!(prev.is_terminal(0));
    // the terminal is in R0's domain without an exclusion (M1)
    assert_eq!(prev.summary().verdict, Verdict::TerminalInRuleDomain);
    let cfg = ClosureConfig {
        credits: Some(swap_policy()),
        ..base(Tier::K)
    };
    let inc =
        closure_incremental(&prev.dispatch_cache(), &d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    let scratch = closure(&d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    assert!(!scratch.is_credited(0));
    assert!(!inc.is_credited(0), "stale terminal cache credited a key that has a rule");
    assert_eq!(inc.winner(0).unwrap().record, 0);
    assert_eq!(view(&inc), view(&scratch));
    assert_eq!(inc.incremental_use().unwrap().reused, 1);
}

/// C1 (same class, found while fixing): keys left unevaluated by a round limit were cached as "uncovered".
#[test]
fn c1_incremental_after_truncated_run_equals_scratch() {
    let a: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let st = start(&[[4, 1, 0, 0]]);
    let prev_cfg = ClosureConfig {
        max_rounds: Some(1),
        ..base(Tier::K)
    };
    let prev = closure(&d, &b, &cmp(), &st, &prev_cfg, &NoFacts).unwrap();
    assert!(prev.summary().truncated);
    let cfg = base(Tier::K);
    let inc =
        closure_incremental(&prev.dispatch_cache(), &d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    let scratch = closure(&d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    assert_eq!(scratch.summary().open_total, 0);
    assert_eq!(view(&inc), view(&scratch));
}

/// C2: the credit takes the CLOSURE's tier: a Tier C closure whose credit image rule has a pole at g k is not closed.
#[test]
fn c2_tier_c_credit_needs_regularity_at_the_image() {
    let a: &[Spec] = &[([Some(3), Some(1), Some(0), Some(0)], &["I(3,0,0,0)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let st = start(&[[1, 3, 0, 0]]);
    let cfg = ClosureConfig {
        credits: Some(swap_policy()),
        ..base(Tier::C)
    };
    let s = closure(&d, &b, &cmp(), &st, &cfg, &Constant(CoefFact::NonZero, RegFact::Pole))
        .unwrap()
        .summary();
    assert_eq!((s.verdict, s.credited, s.open_total), (Verdict::Open, 0, 1));
    // regular by fact: credited and closed
    let s = closure(&d, &b, &cmp(), &st, &cfg, &Constant(CoefFact::NonZero, RegFact::Regular))
        .unwrap()
        .summary();
    assert_eq!((s.verdict, s.credited), (Verdict::Closed, 1));
    // unknown regularity at the image: a PENDING credit, undecided with a request, never closed
    let c = closure(&d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    assert_eq!((s.verdict, s.credited, s.undecided.regularity_unknown), (Verdict::Undecided, 1, 1));
    let r = &c.fact_requests()[0];
    assert_eq!(r.key, vec![3, 1, 0, 0], "the request is at the image");
}

/// C2: every dropped term of the image's rule needs its coefficient-zero fact; a positive-numerator term with an
/// unknown coefficient keeps the credit pending (undecided), with a non-zero one the element is refused.
#[test]
fn c2_tier_c_credit_dropped_terms_need_zero_facts() {
    let a: &[Spec] = &[(
        [Some(3), Some(1), Some(0), Some(0)],
        &["I(3,0,0,0)", "I(1,1,1,0)"],
    )];
    let (d, b) = build(&[("A", a)], vec![0]);
    let st = start(&[[1, 3, 0, 0]]);
    let cfg = ClosureConfig {
        credits: Some(swap_policy()),
        ..base(Tier::C)
    };
    let c = closure(&d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    assert_eq!(s.verdict, Verdict::Undecided);
    assert_eq!(s.undecided.coef_unknown_positive_numerator, 1);
    let reqs = c.fact_requests();
    let f = answer_facts(
        &reqs,
        &[(&[3, 1, 0, 0], None, "regular"), (&[3, 1, 0, 0], Some(1), "zero")],
        "exact",
        false,
    );
    let s = closure(&d, &b, &cmp(), &st, &cfg, &f).unwrap().summary();
    assert_eq!((s.verdict, s.credited, s.terms.dropped_zero), (Verdict::Closed, 1, 1));
    let f = answer_facts(
        &reqs,
        &[(&[3, 1, 0, 0], None, "regular"), (&[3, 1, 0, 0], Some(1), "nonzero")],
        "exact",
        false,
    );
    let s = closure(&d, &b, &cmp(), &st, &cfg, &f).unwrap().summary();
    assert_eq!((s.verdict, s.credited), (Verdict::Open, 0));
}

/// C3: relaxing the regularity check is a DIAGNOSTIC run: verdict `diagnostic` (never `closed`), relaxed check
/// listed in the summary and in the report's configuration, for direct and credited keys alike.
#[test]
fn c3_relaxed_regularity_is_diagnostic() {
    let a: &[Spec] = &[([None; 4], &["I(0,n1,n2,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let reg_unknown = Constant(CoefFact::NonZero, RegFact::Unknown);
    #[allow(deprecated)]
    let cfg = ClosureConfig {
        require_regularity: false,
        ..base(Tier::C)
    };
    let s = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &cfg, &reg_unknown)
        .unwrap()
        .summary();
    assert_eq!(s.verdict, Verdict::Diagnostic);
    assert_eq!(s.relaxed_checks, vec!["allow_unknown_regularity".to_string()]);
    let a2: &[Spec] = &[([Some(3), Some(1), Some(0), Some(0)], &["I(3,0,0,0)"])];
    let (d2, b2) = build(&[("A", a2)], vec![0]);
    let cfg2 = ClosureConfig {
        diagnostic: DiagnosticRelaxations {
            allow_unknown_regularity: true,
        },
        credits: Some(swap_policy()),
        ..base(Tier::C)
    };
    let r = closure(&d2, &b2, &cmp(), &start(&[[1, 3, 0, 0]]), &cfg2, &reg_unknown)
        .unwrap()
        .report(&Default::default());
    assert_eq!(r.summary.verdict, Verdict::Diagnostic);
    assert_eq!(r.summary.verdict_if_not_diagnostic, Some(Verdict::Closed));
    assert!(r.summary.diagnostic);
    assert_eq!(r.config["diagnostic"], "allow_unknown_regularity");
    // the same run without the relaxation: undecided (pending credit)
    let s = closure(
        &d2,
        &b2,
        &cmp(),
        &start(&[[1, 3, 0, 0]]),
        &ClosureConfig {
            diagnostic: Default::default(),
            ..cfg2
        },
        &reg_unknown,
    )
    .unwrap()
    .summary();
    assert_eq!(s.verdict, Verdict::Undecided);
}

// ------------------------------------------------------------------------------------------------ MAJOR

/// M1: a declared terminal inside a rule's domain without an exclusion: `terminal_in_rule_domain`; with a
/// `keeps_terminal` exclusion the dispatch skips the rule (the next record serves, here none): no violation.
#[test]
fn m1_terminal_in_rule_domain_and_exclusions() {
    let a: &[Spec] = &[
        ([Some(2), None, None, None], &["I(n0-1,n1,n2,n3)"]),
        ([None; 4], &["I(n0,n1-1,n2,n3)"]),
    ];
    let (d, b) = build(&[("A", a)], vec![0]);
    let k = [2, 1, 0, 0];
    let st = start(&[k]);
    let term = TerminalSets {
        preferred: Some(start(&[k])),
        other: None,
    };
    let cfg = ClosureConfig {
        terminals: Some(term.clone()),
        ..base(Tier::K)
    };
    let c = closure(&d, &b, &cmp(), &st, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    assert_eq!((s.verdict, s.terminal_in_rule_domain), (Verdict::TerminalInRuleDomain, 1));
    let tr = &c.terminal_records()[0];
    assert!(tr.in_rule_domain);
    assert_eq!((tr.dispatch, tr.would_dispatch.as_ref().unwrap().record), (TerminalDispatch::Covered, 0));
    // excluding record 0 at k: record 1 (free) serves next -> still inside a rule's domain
    let ex0 = KeyExclusion {
        key: k.to_vec(),
        table: "A".into(),
        record: 0,
        label: 0,
        reason: ExclusionReason::KeepsTerminal,
    };
    let cfg1 = ClosureConfig {
        exclusions: vec![ex0.clone()],
        ..cfg.clone()
    };
    let c1 = closure(&d, &b, &cmp(), &st, &cfg1, &NoFacts).unwrap();
    assert_eq!(c1.terminal_records()[0].would_dispatch.as_ref().unwrap().record, 1);
    assert_eq!(c1.summary().verdict, Verdict::TerminalInRuleDomain);
    // both excluded: uncovered there, the terminal holds
    let ex1 = KeyExclusion {
        record: 1,
        label: 1,
        ..ex0.clone()
    };
    let cfg2 = ClosureConfig {
        exclusions: vec![ex0.clone(), ex1.clone()],
        ..cfg.clone()
    };
    let c2 = closure(&d, &b, &cmp(), &st, &cfg2, &NoFacts).unwrap();
    assert_eq!(c2.terminal_records()[0].dispatch, TerminalDispatch::Uncovered);
    assert_eq!(c2.summary().verdict, Verdict::SupersetClean);
    // an exclusion at a non-terminal key: the next record serves (k reduced by record 1, not record 0)
    let cfg3 = ClosureConfig {
        exclusions: vec![ex0.clone()],
        ..base(Tier::K)
    };
    let c3 = closure(&d, &b, &cmp(), &st, &cfg3, &NoFacts).unwrap();
    let ik = c3.index_of(&k).unwrap();
    assert_eq!(c3.winner(ik).unwrap().record, 1);
    // an exclusion naming a record that does not exist is a typed error
    let bad = ClosureConfig {
        exclusions: vec![KeyExclusion { label: 7, ..ex0 }],
        ..base(Tier::K)
    };
    assert!(matches!(
        closure(&d, &b, &cmp(), &st, &bad, &NoFacts),
        Err(ClosureError::Config(_))
    ));
}

/// M2: an old table rebuilt in place (same name, length, labels; changed fixed slots) is detected by its content
/// digest: the incremental result equals the from-scratch one.
#[test]
fn m2_incremental_detects_changed_table_content() {
    let a_old: &[Spec] = &[
        ([Some(1), Some(4), Some(0), Some(0)], &["I(0,4,0,0)"]),
        ([None; 4], &["I(0,n1,n2,n3)"]),
    ];
    let a_new: &[Spec] = &[
        ([Some(1), Some(3), Some(0), Some(0)], &["I(1,2,0,0)"]),
        ([None; 4], &["I(0,n1,n2,n3)"]),
    ];
    let st = start(&[[1, 3, 0, 0]]);
    let (d0, b0) = build(&[("A", a_old)], vec![0]);
    let prev = closure(&d0, &b0, &cmp(), &st, &base(Tier::K), &NoFacts).unwrap();
    let (d1, b1) = build(&[("A", a_new)], vec![0]);
    let inc =
        closure_incremental(&prev.dispatch_cache(), &d1, &b1, &cmp(), &st, &base(Tier::K), &NoFacts)
            .unwrap();
    let scratch = closure(&d1, &b1, &cmp(), &st, &base(Tier::K), &NoFacts).unwrap();
    assert_eq!(view(&inc), view(&scratch));
    assert!(inc.incremental_use().unwrap().unchanged_tables.is_empty());
    // a different codec width is refused (m5)
    let wide = ClosureConfig {
        bits: 8,
        ..base(Tier::K)
    };
    assert!(matches!(
        closure_incremental(&prev.dispatch_cache(), &d1, &b1, &cmp(), &st, &wide, &NoFacts),
        Err(ClosureError::NotIncremental(_))
    ));
}

/// M3: a recheck under ANY changed component other than the start set / input labels is refused, naming the
/// components (hand-down mode, facts, terminals, credits, bodies, ...).
#[test]
fn m3_recheck_refuses_changed_configuration() {
    let a: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let c0 = ClosureConfig {
        hand_downs: HandDowns::Counts,
        ..base(Tier::K)
    };
    let prev = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &c0, &NoFacts).unwrap();
    assert!(!prev.exit_sets().complete, "counts only: exit key sets incomplete");
    let c1 = base(Tier::K);
    match closure_recheck(&prev, &d, &b, &cmp(), &start(&[[1, 2, 0, 0]]), &c1, &NoFacts) {
        Err(ClosureError::NotIncremental(m)) => assert!(m.contains("hand_downs"), "{m}"),
        r => panic!("recheck accepted a changed hand-down mode: {:?}", r.map(|c| c.summary())),
    }
    // changed facts
    let f = Constant(CoefFact::Zero, RegFact::Regular);
    match closure_recheck(&prev, &d, &b, &cmp(), &start(&[[1, 2, 0, 0]]), &c0, &f) {
        Err(ClosureError::NotIncremental(m)) => assert!(m.contains("facts"), "{m}"),
        r => panic!("{:?}", r.map(|c| c.summary())),
    }
    // changed body content (same table)
    let a2: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3-1)"])];
    let (d2, b2) = build(&[("A", a2)], vec![0]);
    match closure_recheck(&prev, &d2, &b2, &cmp(), &start(&[[1, 2, 0, 0]]), &c0, &NoFacts) {
        Err(ClosureError::NotIncremental(m)) => assert!(m.contains("bodies"), "{m}"),
        r => panic!("{:?}", r.map(|c| c.summary())),
    }
    // allowed: only the start set and input labels differ -> equals the closure of the union
    let p1 = closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0]]), &c1, &NoFacts).unwrap();
    assert!(p1.exit_sets().complete);
    let re = closure_recheck(&p1, &d, &b, &cmp(), &start(&[[1, 2, 0, 0]]), &c1, &NoFacts).unwrap();
    let scratch =
        closure(&d, &b, &cmp(), &start(&[[2, 1, 0, 0], [1, 2, 0, 0]]), &c1, &NoFacts).unwrap();
    assert_eq!(re.keys(), scratch.keys());
    assert_eq!(re.hand_down_sets(), scratch.hand_down_sets());
    assert_eq!(re.summary().config_digest, scratch.summary().config_digest);
}

/// M4: facts are keyed by rule CONTENT digest and the term's own key: an answer for a rule whose content changed
/// (same table path, record, label) does not apply; a producer term key that differs from the request is rejected.
#[test]
fn m4_facts_bound_to_rule_content_and_term_key() {
    let a: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3)", "I(n0-1,n1,n2+1,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let st = start(&[[2, 1, 0, 0]]);
    let reqs = closure(&d, &b, &cmp(), &st, &base(Tier::C), &NoFacts)
        .unwrap()
        .fact_requests();
    assert!(reqs.iter().all(|r| r.rule_digest.is_some() && r.config.is_some()));
    let r1 = reqs.iter().find(|r| r.term == Some(1) && r.key == vec![2, 1, 0, 0]).unwrap();
    assert_eq!(r1.rhs, Some(vec![1, 1, 1, 0]), "the request carries the term key");
    let spec: Vec<(&[i32], Option<usize>, &str)> = vec![
        (&[2, 1, 0, 0], None, "regular"),
        (&[1, 1, 0, 0], None, "regular"),
        (&[2, 1, 0, 0], Some(1), "zero"),
        (&[1, 1, 0, 0], Some(1), "zero"),
    ];
    let f = answer_facts(&reqs, &spec, "exact", false);
    assert_eq!(closure(&d, &b, &cmp(), &st, &base(Tier::C), &f).unwrap().summary().verdict, Verdict::Closed);
    // the same table name / record / label with another body: the old facts do not apply
    let a2: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3)", "I(n0-1,n1,n2+1,n3-1)"])];
    let (d2, b2) = build(&[("A", a2)], vec![0]);
    let s2 = closure(&d2, &b2, &cmp(), &st, &base(Tier::C), &f).unwrap().summary();
    assert_eq!(s2.verdict, Verdict::Undecided);
    assert_eq!(s2.terms.dropped_zero, 0);
    // a regenerated coefficient file (other digest): the old facts do not apply
    let cfg3 = ClosureConfig {
        table_digests: [("A".to_string(), "sha256:regenerated".to_string())].into(),
        ..base(Tier::C)
    };
    assert_eq!(closure(&d, &b, &cmp(), &st, &cfg3, &f).unwrap().summary().verdict, Verdict::Undecided);
    // the producer's term key differs from the request's: rejected, counted
    let mut bad = reqs.clone();
    for r in &mut bad {
        if r.term == Some(1) {
            r.rhs = Some(vec![9, 9, 9, 9]);
        }
    }
    let inst = serde_json::json!({"schema":"rustred.proof-kernel.instantiate.v1","rule":0,"key":[2,1,0,0],
        "rhs":[{"term":0,"key":[1,1,0,0]},{"term":1,"key":[1,1,1,0]}]});
    let ans = serde_json::json!({"schema":"rustred.campaign.fact.v1","table":r1.table,"record":0,"label":0,
        "key":[2,1,0,0],"kind":"coefficient","term":1,"value":"zero","method":"exact"});
    let txt = format!("{inst}\n{ans}\n");
    let ff = rustred_campaign_core::closure::FileFacts::read_bound(txt.as_bytes(), &bad, false).unwrap();
    assert_eq!((ff.n_facts, ff.n_rejected_term_key), (0, 1));
    // an unrequested answer is rejected
    let ff = rustred_campaign_core::closure::FileFacts::read_bound(txt.as_bytes(), &[], false).unwrap();
    assert_eq!((ff.n_facts, ff.n_rejected_unrequested), (0, 1));
}

/// M6: a zero-sector list needs its source label; it is part of the configuration identity.
#[test]
fn m6_zero_sectors_labelled_and_in_identity() {
    let a: &[Spec] = &[([None; 4], &["I(n0-1,n1,n2,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let st = start(&[[2, 1, 0, 0]]);
    let unl = ClosureConfig {
        zero_sectors: Some([1u64].into()),
        ..base(Tier::K)
    };
    assert!(matches!(
        closure(&d, &b, &cmp(), &st, &unl, &NoFacts),
        Err(ClosureError::MissingInput(_))
    ));
    let lab = ClosureConfig {
        zero_sector_source: Some("file:sha256:test".into()),
        ..unl
    };
    let c = closure(&d, &b, &cmp(), &st, &lab, &NoFacts).unwrap();
    let r = c.report(&Default::default());
    assert!(r.config["zero_sectors"].starts_with("file:sha256:test|"));
    assert_eq!(c.hand_down_sets().len(), 0, "the zero sector hides the exit");
    let none = closure(&d, &b, &cmp(), &st, &base(Tier::K), &NoFacts).unwrap();
    assert_eq!(
        none.report(&Default::default()).config["zero_sectors"],
        "none (no sector treated as zero)"
    );
    assert_ne!(none.summary().config_digest, c.summary().config_digest);
}

// ------------------------------------------------------------------------------------------------ MINOR

/// m1: a group element whose line block is not the node's line mask is refused (typed error).
#[test]
fn m1_group_line_block_must_match_the_line_mask() {
    let a: &[Spec] = &[([Some(3), Some(1), Some(0), Some(0)], &["I(3,0,0,0)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let pol = CreditPolicy::new(vec![GroupElement {
        lines: vec![2, 1, 0],
        numerators: None,
    }]);
    let cfg = ClosureConfig {
        credits: Some(pol),
        ..base(Tier::K)
    };
    assert!(matches!(
        closure(&d, &b, &cmp(), &start(&[[1, 3, 0, 0]]), &cfg, &NoFacts),
        Err(ClosureError::Config(_))
    ));
}

/// m2: a key with a non-zero numerator is credited only through an element whose numerator map is certified pure.
#[test]
fn m2_numerator_keys_need_a_certified_pure_map() {
    let a: &[Spec] = &[([Some(3), Some(1), None, None], &["I(3,0,n2,n3)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let g = GroupElement {
        lines: vec![1, 0],
        numerators: Some(vec![0, 1]),
    };
    let st = start(&[[1, 3, -1, 0]]);
    let mut pol = CreditPolicy::new(vec![g]);
    let cfg = |p: &CreditPolicy| ClosureConfig {
        credits: Some(p.clone()),
        ..base(Tier::K)
    };
    let s = closure(&d, &b, &cmp(), &st, &cfg(&pol), &NoFacts).unwrap().summary();
    assert_eq!((s.credited, s.open_total), (0, 1));
    pol.pure_numerator_maps.insert(0);
    let s = closure(&d, &b, &cmp(), &st, &cfg(&pol), &NoFacts).unwrap().summary();
    assert_eq!(s.credited, 1);
    let cm = cmp();
    let chk = rustred_campaign_core::symmetry::CreditChecker::new(
        &d,
        &b,
        &cm,
        rustred_campaign_core::closure::Sectors::new(CHART.to_vec()),
        &NoFacts,
        Tier::K,
    );
    let r = chk
        .serve(&[1, 3, -1, 0], &CreditPolicy::new(pol.group.clone()))
        .unwrap()
        .unwrap_err();
    assert_eq!(r, vec![(0, Refusal::NumeratorMapNotPure)]);
}

/// m3: an image that is a declared terminal is never reduced by its rule: no credit through it.
#[test]
fn m3_no_credit_through_a_terminal_image() {
    let a: &[Spec] = &[([Some(3), Some(1), Some(0), Some(0)], &["I(3,0,0,0)"])];
    let (d, b) = build(&[("A", a)], vec![0]);
    let cfg = ClosureConfig {
        credits: Some(swap_policy()),
        terminals: Some(TerminalSets {
            preferred: None,
            other: Some(start(&[[3, 1, 0, 0]])),
        }),
        ..base(Tier::K)
    };
    let s = closure(&d, &b, &cmp(), &start(&[[1, 3, 0, 0]]), &cfg, &NoFacts)
        .unwrap()
        .summary();
    assert_eq!((s.credited, s.open_total), (0, 1));
}

/// m4: two records with one label: loading bodies by label fails closed.
#[test]
fn m4_duplicate_labels_refused_by_label_loads() {
    let mut t = Table::new("A", 4);
    t.push(5, dom([Some(1), None, None, None])).unwrap();
    t.push(5, dom([Some(2), None, None, None])).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    let line = r#"{"i": 7, "rhs": ["I(n0-1,n1,n2,n3)"]}"#;
    assert!(b.load_keys_jsonl(&d, 0, line.as_bytes(), 2).is_err());
}
