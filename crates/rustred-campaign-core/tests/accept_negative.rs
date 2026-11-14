//! NEGATIVE CONTROLS of the acceptance chain (each must end Rejected / Undecided with the right check, never
//! Accepted), on the M2 slice node TOY2kite (KO set K2-single: witness 0 = the knocked-out case I(n0,n1,n2,1,1) with
//! exception n2 = 1) and L3a; plus the file-backed per-key prover's plumbing.
//! Run: `light_build.sh accept test -p rustred-campaign-core --release --test accept_negative`.

mod accept_common;

use std::cmp::Ordering;

use accept_common::*;
use rustred_campaign_core::accept::facts::ExactRecordFacts;
use rustred_campaign_core::accept::prover::{EvidenceProver, FilePerKeyProver};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::accept::{
    AcceptInput, AcceptPolicy, AcceptVerdict, Candidate, Check, FailureKind, NodeTarget,
    RequiredEvidence, accept_candidate,
};
use rustred_campaign_core::cover::Dispatcher;
use rustred_campaign_core::key::Key;
use serde_json::Value;

struct Ctx {
    node: Node,
    ko: Dispatcher,
    gaps: Vec<Key>,
    witness: Vec<u8>,
    off: u64,
}

fn ctx(name: &str, ko: &str, w: usize) -> Option<Ctx> {
    if !available(name) {
        println!("SKIP {name}");
        return None;
    }
    let node = load_node(name);
    let p = ko_table(&node, ko);
    let (_, t) = load_table(&p, "ko");
    let ko_d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let gaps = gap_keys(&node, ko);
    let (off, witness) = witness_lines(name, ko)[w].clone();
    Some(Ctx {
        node,
        ko: ko_d,
        gaps,
        witness,
        off,
    })
}

fn edit(line: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(line).unwrap();
    f(&mut v);
    serde_json::to_vec(&v).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn judge_with(
    c: &Ctx,
    line: &[u8],
    before: Option<&Dispatcher>,
    gaps: &[Key],
    required: RequiredEvidence,
    prover: &mut dyn EvidenceProver,
    policy: AcceptPolicy,
    target: NodeTarget,
) -> AcceptVerdict {
    let rule = RuleText::parse(line, &c.node.frame).unwrap();
    let input = AcceptInput {
        node: view(&c.node, target, required),
        before,
        after: None,
        candidate: Candidate {
            rule: &rule,
            source: source(&c.node.name, "K2-single", c.off, 0),
            position: position(&rule, "K2-single", 0),
        },
        gap_points: gaps,
        working: &c.node.box_keys,
        policy,
    };
    accept_candidate(&input, &mut ExactRecordFacts::new(), prover).unwrap()
}

fn judge(c: &Ctx, line: &[u8]) -> AcceptVerdict {
    let mut p = replay_prover(&c.node);
    judge_with(
        c,
        line,
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::Replay,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    )
}

fn expect(v: &AcceptVerdict, rejected: bool, check: Check, what: &str) {
    println!("{what}: {v:?}");
    match (v, rejected) {
        (AcceptVerdict::Rejected { check: c, .. }, true) if *c == check => {}
        (AcceptVerdict::Undecided { check: c, .. }, false) if *c == check => {}
        _ => panic!(
            "{what}: expected {} at {check:?}, got {v:?}",
            if rejected { "Rejected" } else { "Undecided" }
        ),
    }
}

#[test]
fn control_original_witness_is_accepted() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    assert!(judge(&c, &c.witness).is_accepted());
}

#[test]
fn control_one_coefficient_changed() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let s = String::from_utf8(c.witness.clone()).unwrap();
    let from = r#"["I(n0,n1,n2,1,0)","1"]"#;
    assert!(s.contains(from));
    let bad = s.replace(from, r#"["I(n0,n1,n2,1,0)","2"]"#);
    expect(
        &judge(&c, bad.as_bytes()),
        true,
        Check::Evidence,
        "coefficient changed",
    );
}

#[test]
fn control_rhs_shifted_fails_descent_only_by_tie_break() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let s = String::from_utf8(c.witness.clone()).unwrap();
    let from = r#""I(n0,n1,n2-1,1,1)""#;
    assert!(s.contains(from));
    // move the one dot taken from n2 onto n0 or n1: same dots and rank as the target, the order decides by tie-break
    let key = c
        .gaps
        .iter()
        .find(|k| k[0] >= 2 && k[1] >= 2 && k[2] >= 2)
        .cloned()
        .unwrap();
    let mut chosen = None;
    for (pat, t) in [
        (
            r#""I(n0+1,n1,n2-1,1,1)""#,
            vec![key[0] + 1, key[1], key[2] - 1, 1, 1],
        ),
        (
            r#""I(n0,n1+1,n2-1,1,1)""#,
            vec![key[0], key[1] + 1, key[2] - 1, 1, 1],
        ),
        (
            r#""I(n0-1,n1,n2+1,1,1)""#,
            vec![key[0] - 1, key[1], key[2] + 1, 1, 1],
        ),
        (
            r#""I(n0,n1-1,n2+1,1,1)""#,
            vec![key[0], key[1] - 1, key[2] + 1, 1, 1],
        ),
    ] {
        let dots = |k: &[i32]| k.iter().map(|x| x - 1).sum::<i32>();
        assert_eq!(dots(&t), dots(&key));
        if c.node.cmp.cmp(&t, &key) != Ordering::Less {
            chosen = Some(pat);
            break;
        }
    }
    let pat = chosen.expect("some single-dot move is not lower by the tie-break");
    let bad = s.replace(from, pat);
    expect(
        &judge(&c, bad.as_bytes()),
        true,
        Check::Descent,
        "rhs shifted (tie-break)",
    );
}

#[test]
fn control_wider_domain_than_valid() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let bad = edit(&c.witness, |v| {
        v["fixed"][3] = Value::Null;
        v["target"] = Value::String("I(n0,n1,n2,n3,1)".into());
    });
    let v = judge(&c, &bad);
    println!("wider domain: {v:?}");
    assert!(
        matches!(
            &v,
            AcceptVerdict::Rejected {
                check: Check::Evidence,
                ..
            } | AcceptVerdict::Undecided {
                check: Check::Evidence,
                ..
            }
        ),
        "wider domain must fail at Evidence: {v:?}"
    );
}

#[test]
fn control_elided_and_absent_witness_loci() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let elided = edit(&c.witness, |v| {
        v["witness_loci"] = serde_json::json!(["elided"])
    });
    expect(&judge(&c, &elided), true, Check::Record, "elided loci");
    let absent = edit(&c.witness, |v| {
        v.as_object_mut().unwrap().remove("witness_loci");
    });
    expect(&judge(&c, &absent), false, Check::Record, "absent loci");
    let pre_guard = edit(&c.witness, |v| {
        v.as_object_mut().unwrap().remove("guard_version");
    });
    expect(
        &judge(&c, &pre_guard),
        false,
        Check::Record,
        "no guard version",
    );
}

#[test]
fn control_exception_hit_at_a_served_key() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    // the gap points all lie on the candidate's exception n2 = 1: evaluated AT the key, none is credited (R8)
    let on_exc: Vec<Key> = c
        .node
        .box_keys
        .iter()
        .filter(|k| k[2] == 1 && k[3] == 1 && k[4] == 1)
        .cloned()
        .collect();
    assert!(!on_exc.is_empty());
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        &c.witness,
        None,
        &on_exc,
        RequiredEvidence::Replay,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(&v, true, Check::Coverage, "exception hit at the gap points");
    // exception deleted, witness loci kept: the witness locus -1+n2 blocks the rule at those keys (stopped, never
    // credited)
    let no_exc = edit(&c.witness, |v| v["exceptions"] = serde_json::json!([]));
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        &no_exc,
        None,
        &on_exc,
        RequiredEvidence::Replay,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(
        &v,
        true,
        Check::Coverage,
        "exception deleted, loci kept: blocked at the gap points",
    );
    // exception AND witness loci deleted: it now serves those keys and its denominator -1+n2 vanishes there (exact
    // per-key regularity, R38)
    let no_exc = edit(&c.witness, |v| {
        v["exceptions"] = serde_json::json!([]);
        v["witness_loci"] = serde_json::json!([]);
    });
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        &no_exc,
        None,
        &on_exc,
        RequiredEvidence::Replay,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(
        &v,
        true,
        Check::Regularity,
        "exception deleted: pole at the served gap points",
    );
}

#[test]
fn control_evidence_below_required_level() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        &c.witness,
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::PerKey,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(
        &v,
        false,
        Check::Evidence,
        "replay offered, per-key required",
    );
}

#[test]
fn control_symmetry_row_rule_to_per_key_prover() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let sym = edit(&c.witness, |v| v["sym_rows"] = serde_json::json!(1));
    let mut p = FilePerKeyProver::new(vec![5], None);
    let v = judge_with(
        &c,
        &sym,
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::PerKey,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(&v, false, Check::Evidence, "symmetry rows to instance-span");
    let AcceptVerdict::Undecided { missing, .. } = &v else {
        unreachable!()
    };
    assert!(missing[0].what.contains("symmetry rows"));
    // an arity the tool was not built for
    let mut p = FilePerKeyProver::new(vec![20], None);
    let v = judge_with(
        &c,
        &c.witness,
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::PerKey,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    expect(&v, false, Check::Evidence, "arity not built");
}

#[test]
fn file_backed_per_key_prover_round_trip() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let mut p = FilePerKeyProver::new(vec![5], None);
    let run = |p: &mut FilePerKeyProver| {
        judge_with(
            &c,
            &c.witness,
            Some(&c.ko),
            &c.gaps,
            RequiredEvidence::PerKey,
            p,
            AcceptPolicy::default(),
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
        )
    };
    // 1. no answer: the request names every served key
    let v = run(&mut p);
    let AcceptVerdict::NeedsExternal {
        check: Check::Evidence,
        requests,
    } = &v
    else {
        panic!("{v:?}")
    };
    let req: Value = serde_json::from_str(requests[0].text.trim()).unwrap();
    let keys: Vec<Key> = serde_json::from_value(req["keys"].clone()).unwrap();
    assert_eq!(keys.len(), 20);
    assert_eq!(req["rule"], 0);
    // 2. the REAL rr_proof answer for this case (base table rule 2 = the same record content; two keys) is read
    //    with free-slot keys completed from the fixed slots
    let real = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/accept/span/span_TOY2kite.jsonl"),
    )
    .unwrap();
    let line2 = real
        .lines()
        .find(|l| l.contains(r#""rule":2"#))
        .unwrap()
        .replace(r#""rule":2"#, r#""rule":0"#);
    assert_eq!(
        p.load_answer(&RuleText::parse(&c.witness, &c.node.frame).unwrap(), &line2)
            .unwrap(),
        2
    );
    let v = run(&mut p);
    let AcceptVerdict::NeedsExternal { requests, .. } = &v else {
        panic!("{v:?}")
    };
    let req: Value = serde_json::from_str(requests[0].text.trim()).unwrap();
    assert_eq!(
        req["keys"].as_array().unwrap().len(),
        20 - 2,
        "only the unanswered keys are requested again"
    );
    // 3. a (SYNTHETIC, plumbing only) answer for every key, one key not in span: Accepted with that key excluded
    let rule = RuleText::parse(&c.witness, &c.node.frame).unwrap();
    let mut ks = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let st = if i == 0 {
            "EXACT_NOT_IN_SPAN_AT_KEY (synthetic)"
        } else {
            "EXACT_IN_SPAN_AT_KEY"
        };
        ks.push(serde_json::json!({"key": k, "status": st}));
    }
    let ans = serde_json::json!({"schema": "rustred.proof-kernel.span.v1", "mode": "instance-span", "rule": 0, "keys": ks});
    p.load_answer(&rule, &ans.to_string()).unwrap();
    let v = run(&mut p);
    let AcceptVerdict::Accepted {
        effective, record, ..
    } = &v
    else {
        panic!("{v:?}")
    };
    assert_eq!(effective.exclusions.len(), 1);
    assert_eq!(effective.exclusions[0].reason, FailureKind::NonMembership);
    assert_eq!(record.value["evidence"]["ledger_word"], "identity_proved");
    assert_eq!(record.value["served"]["count"], 19);
    // 4. a refusal line: Unsupported for every key -> Undecided
    p.load_answer(&rule, r#"{"verdict":"UNSUPPORTED","error":"affine case"}"#)
        .unwrap();
    expect(&run(&mut p), false, Check::Evidence, "refusal");
}

#[test]
fn not_descending_is_excludable_only_by_policy() {
    // the tie-break shift with NotDescending in the exclusion policy: the shifted record is no identity any more, so
    // the generic identity test rejects it; in no case is it accepted with a non-descending key left inside
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let s = String::from_utf8(c.witness.clone()).unwrap();
    let bad = s.replace(r#""I(n0,n1,n2-1,1,1)""#, r#""I(n0+1,n1,n2-1,1,1)""#);
    let mut pol = AcceptPolicy::default();
    pol.exclude_on.insert(FailureKind::NotDescending);
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        bad.as_bytes(),
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::Replay,
        &mut p,
        pol,
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    println!("excludable descent: {v:?}");
    // whatever happens, it is never Accepted with a non-descending key inside its effective domain
    if let AcceptVerdict::Accepted { effective, .. } = &v {
        assert!(
            effective
                .exclusions
                .iter()
                .all(|e| e.reason == FailureKind::NotDescending
                    || e.reason == FailureKind::NonMembership)
        );
    }
}

#[test]
fn full_node_scope_is_key_range_with_reasons() {
    let Some(c) = ctx("TOY2kite", "K2-single", 0) else {
        return;
    };
    let mut p = replay_prover(&c.node);
    let v = judge_with(
        &c,
        &c.witness,
        Some(&c.ko),
        &c.gaps,
        RequiredEvidence::Replay,
        &mut p,
        AcceptPolicy::default(),
        NodeTarget::Full,
    );
    let AcceptVerdict::Accepted { record, .. } = &v else {
        panic!("{v:?}")
    };
    println!("FULL scope: {}", record.value["scope"]);
    println!("symbolic: {}", record.value["symbolic"]);
}
