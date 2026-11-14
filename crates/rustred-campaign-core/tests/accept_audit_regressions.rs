//! REGRESSIONS of the two independent audits of `accept` + `assemble` (notes/implementer.audit-accept-A.md, -B.md):
//! one test per finding, each the audit's reproduction (or the closest one the new API still lets a caller write),
//! failing on campaign b5ba6b33 and passing after lane acceptfix. Fixture names and numbers are TEST DATA (L8).
//! Run: `light_build.sh acceptfix test -p rustred-campaign-core --release --test accept_audit_regressions`.

mod accept_common;

use std::path::{Path, PathBuf};

use accept_common::*;
use rustred_campaign_core::accept::descent::{
    Instantiator, symbolic_not_proved, symbolic_regularity_not_proved,
};
use rustred_campaign_core::accept::facts::{
    AnswerBinding, CoefficientFacts, ExactRecordFacts, InstantiateFacts, Regularity,
};
use rustred_campaign_core::accept::prover::{
    EvidenceProver, FilePerKeyProver, ImportedEvidence, ProverAnswer, ReplayProver,
};
use rustred_campaign_core::accept::recipe::{
    Determinism, OpenReason, Recipe, RecipeInput, RecipeVerdict, SolveAnswers, accept_recipe,
};
use rustred_campaign_core::accept::rule::{RuleText, sha256_tag};
use rustred_campaign_core::accept::{
    AcceptInput, AcceptPolicy, AcceptVerdict, Candidate, Check, InForce, InForceBuilder,
    NodeTarget, Position, PriorityClass, RequiredEvidence, RuleSource, Scope, accept_candidate,
};
use rustred_campaign_core::assemble::domaincheck::{DispatchEqual, compare, core_view};
use rustred_campaign_core::assemble::recipes::{RecipeToShip, write as write_recipes};
use rustred_campaign_core::assemble::{
    AssembleError, AssemblyInput, FullRetirements, InputTable, Splice, assemble, splice_index_head,
    tables_in_force, verify_assembled,
};
use rustred_campaign_core::closure::facts::CoefFact;
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::economy::{
    CoverageState, ProofWork, Retirement, RuleMeasure, SupersedeReason, prove_coverage_preserved,
    table_set_digest,
};
use rustred_campaign_core::ibp_rows::TableBinding;
use rustred_campaign_core::key::Key;
use rustred_campaign_core::partition::{
    DispatchOrderStatement, DispatcherSource, DomainSource,
};
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};
use serde_json::{Value, json};

const TOY: &str = "TOY2kite";
const KO: &str = "K2-single";

fn boxed() -> NodeTarget {
    NodeTarget::Boxed {
        box_id: "fixture-box".into(),
    }
}

fn edit(line: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(line).unwrap();
    f(&mut v);
    serde_json::to_vec(&v).unwrap()
}

struct Toy {
    node: Node,
    ko_path: PathBuf,
    ko: InForce,
    gaps: Vec<Key>,
    w0: Vec<u8>,
    off: u64,
}

fn toy() -> Option<Toy> {
    if !available(TOY) {
        println!("SKIP {TOY}");
        return None;
    }
    let node = load_node(TOY);
    let ko_path = ko_table(&node, KO);
    let ko = in_force(&node, Some(&ko_path), &[]);
    let gaps = gap_keys(&node, KO);
    let (off, w0) = witness_lines(TOY, KO)[0].clone();
    Some(Toy {
        node,
        ko_path,
        ko,
        gaps,
        w0,
        off,
    })
}

#[allow(clippy::too_many_arguments)]
fn judge(
    t: &Toy,
    line: &[u8],
    before: Option<&InForce>,
    target: NodeTarget,
    required: RequiredEvidence,
    facts: &mut dyn rustred_campaign_core::accept::KeyFacts,
    prover: &mut dyn EvidenceProver,
    src: RuleSource,
    pos: Position,
    gaps: &[Key],
) -> AcceptVerdict {
    let rule = RuleText::parse(line, &t.node.frame).unwrap();
    let input = AcceptInput {
        node: view(&t.node, target, required),
        before,
        after: None,
        candidate: Candidate {
            rule: &rule,
            source: src,
            position: pos,
        },
        gap_points: gaps,
        working: &t.node.box_keys,
        policy: AcceptPolicy::default(),
    };
    accept_candidate(&input, facts, prover).unwrap()
}

fn judge_replay(t: &Toy, line: &[u8], target: NodeTarget) -> AcceptVerdict {
    let rule = RuleText::parse(line, &t.node.frame).unwrap();
    let mut p = replay_prover(&t.node);
    judge(
        t,
        line,
        Some(&t.ko),
        target,
        RequiredEvidence::Replay,
        &mut ExactRecordFacts::new(),
        &mut p,
        source(TOY, KO, t.off, 0),
        position(&rule, KO, 0),
        &t.gaps.clone(),
    )
}

/// Per-key judgement with SYNTHETIC statuses (plumbing; every answer echoes the rule digest).
fn judge_per_key(
    t: &Toy,
    line: &[u8],
    before: &InForce,
    src: RuleSource,
    pos: Position,
    gaps: &[Key],
    fail: &[(Key, &str)],
) -> AcceptVerdict {
    let rule = RuleText::parse(line, &t.node.frame).unwrap();
    let mut prover = FilePerKeyProver::new(vec![t.node.frame.arity()], None);
    let mut facts = ExactRecordFacts::new();
    let first = judge(
        t,
        line,
        Some(before),
        boxed(),
        RequiredEvidence::PerKey,
        &mut facts,
        &mut prover,
        src.clone(),
        pos.clone(),
        gaps,
    );
    let AcceptVerdict::NeedsExternal { requests, .. } = first else {
        return first;
    };
    let req: Value = serde_json::from_str(requests[0].text.trim()).unwrap();
    let keys: Vec<Key> = serde_json::from_value(req["keys"].clone()).unwrap();
    let ks: Vec<Value> = keys
        .iter()
        .map(|k| {
            let st = fail
                .iter()
                .find(|(f, _)| f == k)
                .map(|(_, s)| s.to_string())
                .unwrap_or_else(|| "EXACT_IN_SPAN_AT_KEY".into());
            json!({"key": k, "status": st})
        })
        .collect();
    let ans = json!({"schema": "rustred.proof-kernel.span.v1", "rule": rule.index(),
        "rule_sha256": rule.line_sha256, "keys": ks});
    prover.load_answer(&rule, &ans.to_string()).unwrap();
    judge(
        t,
        line,
        Some(before),
        boxed(),
        RequiredEvidence::PerKey,
        &mut facts,
        &mut prover,
        src,
        pos,
        gaps,
    )
}

fn assembly(
    t: &Toy,
    inputs: Vec<InputTable>,
    accepted: Vec<rustred_campaign_core::accept::Accepted>,
    target: NodeTarget,
) -> AssemblyInput {
    AssemblyInput {
        node: t.node.name.clone(),
        target,
        inputs,
        accepted,
        terminals: t.node.terminals.clone(),
        credits: vec![],
        retirements: vec![],
        removals: vec![],
        full_retirements: FullRetirements::Refuse,
        keep_skipped: false,
        used: None,
        renumber_index: true,
        recipes: 0,
    }
}

fn ko_input(t: &Toy) -> InputTable {
    InputTable {
        id: "ko".into(),
        path: t.ko_path.clone(),
        priority: 0,
        sha256: None,
    }
}

/// The removed records of the KO set as a table of their own (header, zero_sectors, chart, those rules, end), with
/// an optional edit of the header / rule lines; written to scratch.
fn removed_part(t: &Toy, name: &str, f: impl Fn(&str, &mut Value) -> bool) -> PathBuf {
    let removed: Vec<i64> = ko_entry(&t.node, KO)["removed_record_indices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    let text = std::fs::read_to_string(t.node.dir.join("table.jsonl")).unwrap();
    let mut out = String::new();
    for l in text.lines() {
        let mut v: Value = serde_json::from_str(l).unwrap();
        let kind = v["type"].as_str().unwrap().to_string();
        if kind == "rule" && !removed.contains(&v["index"].as_i64().unwrap()) {
            continue;
        }
        if kind == "residual" {
            continue;
        }
        let edited = f(&kind, &mut v);
        if v.is_null() {
            continue; // the edit deleted the line
        }
        if edited {
            out.push_str(&serde_json::to_string(&v).unwrap());
        } else {
            out.push_str(l);
        }
        out.push('\n');
    }
    let p = scratch(&format!("{TOY}-regress")).join(format!("{name}.table.jsonl"));
    std::fs::write(&p, out).unwrap();
    p
}

// =================================================================================================================
// 1. BINDING: evidence, facts and answers are bound to the rule they certify
// =================================================================================================================

/// audit-accept-B C1 / A M5: per-key answers of the true witness loaded for a coefficient mutant with the same index.
#[test]
fn b_c1_per_key_answers_bound_to_the_rule() {
    let Some(t) = toy() else { return };
    let truth = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let s = String::from_utf8(t.w0.clone()).unwrap();
    let from = r#"["I(n0,n1,n2,1,0)","1"]"#;
    assert!(s.contains(from));
    let keys: Vec<Key> = t
        .gaps
        .iter()
        .filter(|g| truth.domain.as_ref().unwrap().applies(g).applies())
        .cloned()
        .collect();
    let ks: Vec<Value> = keys
        .iter()
        .map(|k| json!({"key": k, "status": "EXACT_IN_SPAN_AT_KEY"}))
        .collect();
    let with_digest = json!({"schema": "rustred.proof-kernel.span.v1", "rule": 0,
        "rule_sha256": truth.line_sha256, "record_bytes": truth.line.len(), "keys": ks})
    .to_string();
    let without_digest = json!({"schema": "rustred.proof-kernel.span.v1", "rule": 0,
        "record_bytes": truth.line.len(), "keys": ks})
    .to_string();
    for bad in ["2", "2*(1)", "-(1)", "(1)+1"] {
        let m = RuleText::parse(
            s.replace(from, &format!(r#"["I(n0,n1,n2,1,0)","{bad}"]"#))
                .as_bytes(),
            &t.node.frame,
        )
        .unwrap();
        let mut p = FilePerKeyProver::new(vec![5], None);
        assert!(
            p.load_answer(&m, &with_digest).is_err(),
            "{bad}: an answer naming another rule digest"
        );
        assert!(
            p.load_answer(&m, &without_digest).is_err(),
            "{bad}: no digest echo under the default binding"
        );
        p.binding = AnswerBinding::Structural;
        if bad != "2" {
            // the structural echo (record bytes) still tells these mutants apart
            assert!(
                p.load_answer(&m, &without_digest).is_err(),
                "{bad}: record bytes"
            );
        }
    }
    // the true rule: loaded, and the chain accepts with the artefact naming it
    let mut p = FilePerKeyProver::new(vec![5], None);
    assert_eq!(p.load_answer(&truth, &with_digest).unwrap(), keys.len());
    let v = judge(
        &t,
        &t.w0,
        Some(&t.ko),
        boxed(),
        RequiredEvidence::PerKey,
        &mut ExactRecordFacts::new(),
        &mut p,
        source(TOY, KO, t.off, 0),
        position(&truth, KO, 0),
        &t.gaps,
    );
    let a = v.accepted().unwrap_or_else(|| panic!("{v:?}"));
    assert_eq!(a.artefact().rule_sha256, truth.line_sha256);
    assert!(a.artefact().tool.contains("binding=digest"));
}

fn instantiate_lines(rule: &RuleText, keys: &[Key], digest: bool, coef: &str) -> String {
    let inst = Instantiator::new(rule).unwrap();
    let mut out = String::new();
    for k in keys {
        let rhs: Vec<Value> = inst
            .terms(k)
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, (tk, _))| json!({"term": i, "key": tk, "coef": coef}))
            .collect();
        let mut v = json!({"rule": rule.index(), "key": k, "in_case": true, "chart_ok": true, "branch": null,
            "regular": "regular", "irregular_terms": [], "hidden_loci": [], "rhs": rhs, "zero_test": "exact",
            "error": null});
        if digest {
            v["rule_sha256"] = json!(rule.line_sha256);
        }
        out.push_str(&format!("{v}\n"));
    }
    out
}

/// audit-accept-B C2 / A M4: regularity / coefficient facts keyed by (record index, key) only.
#[test]
fn b_c2_instantiate_facts_bound_to_the_rule() {
    let Some(t) = toy() else { return };
    let a = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let keys: Vec<Key> = t
        .gaps
        .iter()
        .filter(|g| a.domain.as_ref().unwrap().applies(g).applies())
        .cloned()
        .collect();
    // B: same index, another body (its first rhs term inverted onto a higher key), loci absent (fact-credited)
    let b_line = edit(&t.w0, |v| {
        v["rhs"][0][0] = json!("I(n0,n1,n2+1,1,1)");
        v.as_object_mut().unwrap().remove("witness_loci");
    });
    let b = RuleText::parse(&b_line, &t.node.frame).unwrap();
    let a_text = instantiate_lines(&a, &keys, true, "zero");
    let a_text_nodigest = instantiate_lines(&a, &keys, false, "zero");
    let mut f = InstantiateFacts::new(AnswerBinding::DigestEcho);
    assert!(f.load(&b, &a_text).is_err(), "digest of another rule");
    assert!(f.load(&b, &a_text_nodigest).is_err(), "no digest echo");
    let mut fs = InstantiateFacts::new(AnswerBinding::Structural);
    assert!(
        fs.load(&b, &a_text_nodigest).is_err(),
        "the instantiated rhs keys are another rule body's"
    );
    // facts loaded for A answer for A only
    assert_eq!(f.load(&a, &a_text).unwrap(), keys.len());
    assert!(f.fact_digest_for(&a).is_some());
    assert!(f.fact_digest_for(&b).is_none());
    assert_eq!(
        f.regular(&a, &keys[0]),
        rustred_campaign_core::accept::facts::RegVerdict::Regular
    );
    assert!(matches!(
        f.regular(&b, &keys[0]),
        rustred_campaign_core::accept::facts::RegVerdict::Unknown(_)
    ));
    assert_eq!(f.coefficient(&b, &keys[0], 0), CoefFact::Unknown);
    // the chain on B with A's facts: B is witness-absent, the facts are not B's -> Undecided at Record (was: credited
    // on "regularity_fact" and its non-descending term dropped as "zero")
    let mut p = replay_prover(&t.node);
    let v = judge(
        &t,
        &b_line,
        Some(&t.ko),
        boxed(),
        RequiredEvidence::Replay,
        &mut f,
        &mut p,
        source(TOY, KO, t.off, 0),
        position(&b, KO, 0),
        &t.gaps,
    );
    assert!(
        matches!(
            v,
            AcceptVerdict::Undecided {
                check: Check::Record,
                ..
            }
        ),
        "{v:?}"
    );
    // a line not in case / on a branch gives no usable fact
    let mut g = InstantiateFacts::new(AnswerBinding::DigestEcho);
    let on_branch = a_text
        .lines()
        .next()
        .unwrap()
        .replace(r#""branch":null"#, r#""branch":0"#);
    g.load(&a, &on_branch).unwrap();
    assert!(matches!(
        g.regular(&a, &keys[0]),
        rustred_campaign_core::accept::facts::RegVerdict::Unknown(_)
    ));
    // no silent drop: an unclassifiable line is an error
    assert!(g.load(&a, r#"{"something":"else"}"#).is_err());
}

/// A prover wired to the TRUE rule whatever candidate it is asked about (audit-accept-B G10 / M1).
struct MisWired {
    inner: ReplayProver,
    truth: RuleText,
}
impl EvidenceProver for MisWired {
    fn level(&self) -> rustred_campaign_core::accept::prover::Evidence {
        self.inner.level()
    }
    fn prove(&mut self, _rule: &RuleText, source: &RuleSource, served: &[Key]) -> ProverAnswer {
        let t = self.truth.clone();
        self.inner.prove(&t, source, served)
    }
}

/// audit-accept-B M1: the evidence artefact carries no rule identity.
#[test]
fn b_m1_artefact_of_another_rule_is_not_evidence() {
    let Some(t) = toy() else { return };
    let truth = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let s = String::from_utf8(t.w0.clone()).unwrap();
    let mutant = s.replace(r#"["I(n0,n1,n2,1,0)","1"]"#, r#"["I(n0,n1,n2,1,0)","2"]"#);
    let m = RuleText::parse(mutant.as_bytes(), &t.node.frame).unwrap();
    let mut p = MisWired {
        inner: replay_prover(&t.node),
        truth,
    };
    let v = judge(
        &t,
        mutant.as_bytes(),
        Some(&t.ko),
        boxed(),
        RequiredEvidence::Replay,
        &mut ExactRecordFacts::new(),
        &mut p,
        source(TOY, KO, t.off, 0),
        position(&m, KO, 0),
        &t.gaps,
    );
    assert!(
        matches!(v, AcceptVerdict::Undecided { check: Check::Evidence, ref missing } if missing[0].what.contains("not bound")),
        "{v:?}"
    );
}

/// rows3 NEW-M1: the replay prover certifies only with a family binding; a binding to another family is not evidence.
#[test]
fn rows3_replay_needs_the_right_family_binding() {
    let Some(t) = toy() else { return };
    let mut p = ReplayProver::new(
        &t.node.family_text,
        TableBinding::from_family_digest(&"0".repeat(64)),
        rustred_campaign_core::ibp_rows::Beacon::from_hex("0123456789abcdef0123456789abcdef")
            .unwrap(),
        Default::default(),
    )
    .unwrap();
    let rule = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let v = judge(
        &t,
        &t.w0,
        Some(&t.ko),
        boxed(),
        RequiredEvidence::Replay,
        &mut ExactRecordFacts::new(),
        &mut p,
        source(TOY, KO, t.off, 0),
        position(&rule, KO, 0),
        &t.gaps,
    );
    assert!(!v.is_accepted(), "{v:?}");
    let ok = judge_replay(&t, &t.w0, boxed());
    let a = ok
        .accepted()
        .expect("accepted with the table header binding");
    assert_eq!(
        a.artefact().body.as_ref().unwrap()["family_bound_to_table"],
        json!(true)
    );
}

// =================================================================================================================
// 2. DESCENT AND REGULARITY OVER THE WHOLE DOMAIN (FULL scope)
// =================================================================================================================

/// audit-accept-A C1: a term that becomes a POSITIVE NUMERATOR on the domain was "symbolically proved".
#[test]
fn a_c1_positive_numerator_term_is_not_symbolically_proved() {
    let name = "L4a";
    if !available(name) {
        return;
    }
    let node = load_node(name);
    let text = std::fs::read_to_string(node.dir.join("table.jsonl")).unwrap();
    let lines_mask = node.frame.mask().to_vec();
    let mut tested = 0;
    for l in text.lines().filter(|l| l.starts_with(r#"{"type":"rule""#)) {
        let r = RuleText::parse(l.as_bytes(), &node.frame).unwrap();
        let target = r.record.target.clone().unwrap();
        let args: Vec<String> = target[2..target.len() - 1]
            .split(',')
            .map(str::to_string)
            .collect();
        // a free numerator slot j whose target slot is n<j>, and a free line slot i
        let Some(j) = (0..args.len())
            .find(|&j| !lines_mask[j] && r.record.fixed[j].is_none() && args[j] == format!("n{j}"))
        else {
            continue;
        };
        let Some(i) = (0..args.len())
            .find(|&i| lines_mask[i] && r.record.fixed[i].is_none() && args[i] == format!("n{i}"))
        else {
            continue;
        };
        let mut pat = args.clone();
        pat[i] = format!("n{i}-1");
        pat[j] = format!("n{j}+1");
        let p = format!("I({})", pat.join(","));
        let m = edit(l.as_bytes(), |v| v["rhs"][0] = json!([p, "1"]));
        let mr = RuleText::parse(&m, &node.frame).unwrap();
        let bad = symbolic_not_proved(&mr, &node.cmp, &lines_mask);
        assert!(
            bad.contains(&0),
            "rule {:?}: term n{j}+1 on a numerator slot",
            r.index()
        );
        tested += 1;
        break;
    }
    assert!(tested > 0, "no rule with a free numerator slot found");
}

/// audit-accept-B C3: FULL scope granted after a witness locus was dropped together with its exception branch.
#[test]
fn b_c3_dropped_locus_and_exception_never_full_domain() {
    let Some(t) = toy() else { return };
    let orig = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    assert!(symbolic_regularity_not_proved(&orig).is_empty());
    let m = edit(&t.w0, |v| {
        v["exceptions"] = json!([]);
        v["witness_loci"] = json!([]);
    });
    let mr = RuleText::parse(&m, &t.node.frame).unwrap();
    let why = symbolic_regularity_not_proved(&mr);
    assert!(
        !why.is_empty(),
        "the denominators (-1+n2) vanish on the face n2 = 1"
    );
    let v = judge_replay(&t, &m, NodeTarget::Full);
    if let Some(a) = v.accepted() {
        assert!(
            matches!(a.scope(), Scope::KeyRange { missing, .. } if missing.iter().any(|w| w.contains("symbolic regularity"))),
            "{:?}",
            a.scope()
        );
    }
    // the original is FULL-domain certified (reachable, audit-accept-A m6)
    let o = judge_replay(&t, &t.w0, NodeTarget::Full);
    assert_eq!(o.accepted().unwrap().scope(), &Scope::FullDomain);
}

// =================================================================================================================
// 3. HEADER IDENTITY and 4. NO SILENT DROPS (assembly inputs)
// =================================================================================================================

/// An edit of one table line (kind, value): true = edited.
type LineEdit = dyn Fn(&str, &mut Value) -> bool;

/// audit-accept-A C2 / B C5: tables with another frame / order / guard version / chart line / zero sectors, or none.
#[test]
fn a_c2_b_c5_header_identity_enforced() {
    let Some(t) = toy() else { return };
    let part = |name: &str, f: &dyn Fn(&str, &mut Value) -> bool| InputTable {
        id: name.into(),
        path: removed_part(&t, name, f),
        priority: 1,
        sha256: None,
    };
    // positive control: the removed records with the same header assemble with the KO table
    let ok = part("same", &|_, _| false);
    let out = assemble(
        &assembly(&t, vec![ko_input(&t), ok], vec![], boxed()),
        &scratch("regress-hdr-ok"),
    )
    .unwrap();
    assert!(out.report.input_records > 0);
    let cases: Vec<(&str, Box<LineEdit>)> = vec![
        (
            "order_name",
            Box::new(|k: &str, v: &mut Value| {
                k == "header" && {
                    v["order_name"] = json!("other");
                    true
                }
            }),
        ),
        (
            "node_order",
            Box::new(|k: &str, v: &mut Value| {
                k == "header" && {
                    v["node_order"] = json!({"name": "x", "slot_permutation": [4, 3, 2, 1, 0]});
                    true
                }
            }),
        ),
        (
            "guard_version",
            Box::new(|k: &str, v: &mut Value| {
                k == "header" && {
                    v["guard_version"] = json!(1);
                    true
                }
            }),
        ),
        (
            "charts",
            Box::new(|k: &str, v: &mut Value| {
                k == "header" && {
                    v["charts"] = json!(["11110"]);
                    true
                }
            }),
        ),
        (
            "chart line",
            Box::new(|k: &str, v: &mut Value| {
                k == "chart" && {
                    v["zero"] = json!(true);
                    true
                }
            }),
        ),
        (
            "zero_sectors absent",
            Box::new(|k: &str, v: &mut Value| {
                k == "zero_sectors" && {
                    *v = Value::Null;
                    true
                }
            }),
        ),
        (
            "no header",
            Box::new(|k: &str, v: &mut Value| {
                k == "header" && {
                    *v = Value::Null;
                    true
                }
            }),
        ),
    ];
    for (what, f) in cases {
        let other = InputTable {
            id: what.replace(' ', "_"),
            path: removed_part(&t, &what.replace(' ', "_"), &*f),
            priority: 1,
            sha256: None,
        };
        let r = assemble(
            &assembly(&t, vec![ko_input(&t), other], vec![], boxed()),
            &scratch("regress-hdr"),
        );
        assert!(
            matches!(
                r,
                Err(AssembleError::HeaderMismatch { .. }) | Err(AssembleError::Record { .. })
            ),
            "{what}: {:?}",
            r.as_ref().map(|o| o.report.records)
        );
    }
}

/// audit-accept-B C4 / A M8: rule lines whose "type" key is not first (sorted keys), a record index beyond the first
/// MiB (A m5 / B m3), an unknown line kind, a truncated table, a wrong expected digest.
#[test]
fn b_c4_every_line_classified_never_dropped() {
    let Some(t) = toy() else { return };
    let pad = "x".repeat(3 << 20);
    let sorted = removed_part(&t, "sorted", |k, v| {
        if k == "rule" {
            v["aaa_pad"] = json!(pad);
        }
        true // every line re-serialised with sorted keys: "type" is no longer first
    });
    let first = std::fs::read_to_string(&sorted).unwrap();
    assert!(
        first
            .lines()
            .nth(1)
            .is_some_and(|l| !l.starts_with(r#"{"type""#))
    );
    let n_removed = first
        .lines()
        .filter(|l| l.contains(r#""type":"rule""#))
        .count();
    assert!(n_removed > 0);
    let inp = InputTable {
        id: "sorted".into(),
        path: sorted.clone(),
        priority: 1,
        sha256: None,
    };
    let dir = scratch("regress-sorted");
    let out = assemble(
        &assembly(&t, vec![ko_input(&t), inp], vec![], boxed()),
        &dir,
    )
    .unwrap();
    let ko_rules = std::fs::read_to_string(&t.ko_path)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with(r#"{"type":"rule""#))
        .count();
    assert_eq!(out.report.input_records as usize, ko_rules + n_removed);
    assert_eq!(
        verify_assembled(&out.table, &out.manifest).unwrap() as u64,
        out.report.records
    );
    // the assembled file is itself a valid input (its header is recognised)
    let again = InputTable {
        id: "assembled".into(),
        path: out.table.clone(),
        priority: 0,
        sha256: Some(out.table_sha256.clone()),
    };
    let out2 = assemble(
        &assembly(&t, vec![again], vec![], boxed()),
        &scratch("regress-again"),
    )
    .unwrap();
    assert_eq!(out2.report.input_records, out.report.input_records);
    // an unknown line kind / no end record / a wrong digest are errors
    let unknown = removed_part(&t, "unknown", |k, v| {
        k == "chart" && {
            v["type"] = json!("mystery");
            true
        }
    });
    let truncated = removed_part(&t, "truncated", |k, v| {
        k == "end" && {
            *v = Value::Null;
            true
        }
    });
    for (p, sha) in [
        (unknown, None),
        (truncated, None),
        (sorted, Some("sha256:00".to_string())),
    ] {
        let inp = InputTable {
            id: "bad".into(),
            path: p,
            priority: 1,
            sha256: sha,
        };
        assert!(
            assemble(
                &assembly(&t, vec![ko_input(&t), inp], vec![], boxed()),
                &scratch("regress-bad")
            )
            .is_err()
        );
    }
}

/// No silent drops in the answer readers either.
#[test]
fn readers_refuse_unclassified_lines() {
    let Some(t) = toy() else { return };
    let rule = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let mut p = FilePerKeyProver::new(vec![5], None);
    assert!(p.load_answer(&rule, r#"{"type":"summary"}"#).is_err());
    assert!(
        ImportedEvidence::read(r#"{"schema":"rustred.campaign.accept.v1","rule":{"id":"r"}}"#)
            .is_err(),
        "an accept line naming no rule line"
    );
    let mut s = SolveAnswers::new("sha256:r");
    assert!(s.load(r#"{"schema":"other"}"#).is_err());
    assert!(
        s.load(r#"{"schema":"rustred.campaign.recipe-solve.v1","recipe_digest":"sha256:other","key":[1]}"#)
            .is_err()
    );
}

// =================================================================================================================
// 5. EXCLUSIONS HONOURED
// =================================================================================================================

/// audit-accept-A M1 / B C6: exclusions come from the chain; tables in force honour them (so a point rule can be
/// judged at an excluded key), and a point rule that excludes its only key is never accepted (the circular case).
#[test]
fn exclusions_honoured_in_force_and_no_circular_hoist() {
    let Some(t) = toy() else { return };
    let r0 = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let served: Vec<Key> = t
        .gaps
        .iter()
        .filter(|g| r0.domain.as_ref().unwrap().applies(g).applies())
        .cloned()
        .collect();
    let k0 = served[0].clone();
    let face = judge_per_key(
        &t,
        &t.w0,
        &t.ko,
        source(TOY, KO, t.off, 0),
        position(&r0, KO, 0),
        &t.gaps,
        &[(k0.clone(), "EXACT_NOT_IN_SPAN_AT_KEY (synthetic)")],
    );
    let fa = face.accepted().unwrap_or_else(|| panic!("{face:?}"));
    assert_eq!(fa.exclusions().len(), 1);
    // honoured: the face rule is not selected at k0 in the tables in force (it is everywhere else it served)
    let with = in_force(&t.node, Some(&t.ko_path), &[(&face, &r0)]);
    let d = with.dispatcher().unwrap();
    assert_eq!(d.first_match(&k0).unwrap(), Dispatch::Uncovered);
    assert!(d.first_match(&served[1]).unwrap().is_covered());
    // a point rule at k0 failing at its own key: no gap point left -> never accepted (B C6 needs exactly this token)
    let mut v: Value = serde_json::from_slice(&t.w0).unwrap();
    v["fixed"] = json!(k0);
    v["target"] = json!(format!(
        "I({})",
        k0.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
    v["exceptions"] = json!([]);
    v["witness_loci"] = json!([]);
    let pl = serde_json::to_vec(&v).unwrap();
    let pf = scratch("regress-point").join("point.jsonl");
    std::fs::write(&pf, [pl.as_slice(), b"\n"].concat()).unwrap();
    let src = RuleSource {
        table_id: "point".into(),
        table_path: pf.display().to_string(),
        table_digest: sha256_tag(&std::fs::read(&pf).unwrap()),
        ordinal: 0,
        byte_offset: Some(0),
        producer: None,
        table_guard_version: None,
    };
    let pos = Position {
        class: PriorityClass::Point,
        input_priority: 0,
        round: 1,
        item: "pt".into(),
        index: 0,
    };
    let circ = judge_per_key(
        &t,
        &pl,
        &with,
        src.clone(),
        pos.clone(),
        std::slice::from_ref(&k0),
        &[(k0.clone(), "EXACT_NOT_IN_SPAN_AT_KEY (synthetic)")],
    );
    assert!(!circ.is_accepted(), "{circ:?}");
    // the face rule's exclusion with no hoistable point: blocking, not export-ready
    let out = assemble(
        &assembly(&t, vec![ko_input(&t)], vec![fa.clone()], boxed()),
        &scratch("regress-excl"),
    )
    .unwrap();
    assert_eq!(out.report.blocking.len(), 1);
    assert!(!out.report.export_ready);
}

// =================================================================================================================
// 6. ORDER and 7. TOKENS
// =================================================================================================================

/// audit-accept-B C7: a rule judged against tables in force other than the records placed before it is refused.
#[test]
fn b_c7_judged_order_must_be_the_assembled_order() {
    let name = "L3a";
    if !available(name) {
        return;
    }
    let node = load_node(name);
    let mut found = 0;
    for ko in node.expected["ko"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["id"].as_str().unwrap().to_string())
        .filter(|k| witness_path(name, k).is_file())
    {
        let mut prover = replay_prover(&node);
        let run = run_ko(&node, &ko, &mut prover, RequiredEvidence::Replay);
        if run.accepted.len() < 2 {
            continue;
        }
        // naive: the second accepted rule judged against the KO table ONLY (the first one is placed before it)
        let (i1, off1, r1, _) = &run.accepted[1];
        let before = in_force(&node, Some(&run.ko_path), &[]);
        let mut p = replay_prover(&node);
        let input = AcceptInput {
            node: view(&node, boxed(), RequiredEvidence::Replay),
            before: Some(&before),
            after: None,
            candidate: Candidate {
                rule: r1,
                source: source(name, &ko, *off1, *i1),
                position: position(r1, &ko, *i1),
            },
            gap_points: &run.gaps,
            working: &run.working,
            policy: AcceptPolicy::default(),
        };
        let v = accept_candidate(&input, &mut ExactRecordFacts::new(), &mut p).unwrap();
        let Some(naive) = v.accepted() else { continue };
        let first = run.accepted[0].3.accepted().unwrap().clone();
        let inp = AssemblyInput {
            node: node.name.clone(),
            target: boxed(),
            inputs: vec![InputTable {
                id: "ko".into(),
                path: run.ko_path.clone(),
                priority: 0,
                sha256: None,
            }],
            accepted: vec![first, naive.clone()],
            terminals: node.terminals.clone(),
            credits: vec![],
            retirements: vec![],
            removals: vec![],
            full_retirements: FullRetirements::Refuse,
            keep_skipped: false,
            used: None,
            renumber_index: true,
            recipes: 0,
        };
        let r = assemble(&inp, &scratch("regress-order"));
        assert!(
            matches!(r, Err(AssembleError::Order { .. })),
            "{ko}: {:?}",
            r.err()
        );
        found += 1;
    }
    assert!(
        found > 0,
        "no KO set with two accepted rules judged naively"
    );
}

/// audit-accept-A M1 / B M2: tokens bind the node, the target and the frame; the same record twice (A m3) refused.
#[test]
fn tokens_bind_node_target_and_are_unique() {
    let Some(t) = toy() else { return };
    let v = judge_replay(&t, &t.w0, boxed());
    let a = v.accepted().unwrap().clone();
    let full = assembly(&t, vec![ko_input(&t)], vec![a.clone()], NodeTarget::Full);
    assert!(matches!(
        assemble(&full, &scratch("regress-bind")),
        Err(AssembleError::Binding { .. })
    ));
    let mut other = assembly(&t, vec![ko_input(&t)], vec![a.clone()], boxed());
    other.node = "another-node".into();
    assert!(matches!(
        assemble(&other, &scratch("regress-bind")),
        Err(AssembleError::Binding { .. })
    ));
    let twice = assembly(&t, vec![ko_input(&t)], vec![a.clone(), a], boxed());
    assert!(assemble(&twice, &scratch("regress-bind")).is_err());
}

// =================================================================================================================
// 8. dispatch_equal and the shipped file
// =================================================================================================================

/// audit-accept-A m4 / B M4: dispatch_equal is never vacuous; the shipped file is checked against the manifest.
#[test]
fn dispatch_equal_not_vacuous_and_file_verified() {
    let Some(t) = toy() else { return };
    let v = judge_replay(&t, &t.w0, boxed());
    let out = assemble(
        &assembly(
            &t,
            vec![ko_input(&t)],
            vec![v.accepted().unwrap().clone()],
            boxed(),
        ),
        &scratch("regress-de"),
    )
    .unwrap();
    let cv = core_view(&out.table).unwrap();
    assert!(compare(&cv, &[], "").is_err(), "empty key list");
    let empty = DispatchEqual::default();
    assert!(!empty.equal(true));
    let disjoint = DispatchEqual {
        keys: 3,
        agree: 3,
        none: 3,
        ..Default::default()
    };
    assert!(
        !disjoint.equal(true),
        "no key decided by a rule or a terminal"
    );
    let k = t.node.box_keys[0].clone();
    let other_table = format!(
        "{}\n",
        json!({"schema":"rustred.domaincheck.v1","type":"header","table":"/elsewhere.jsonl"})
    );
    assert!(compare(&cv, std::slice::from_ref(&k), &other_table).is_err());
    // a reordered or modified file no longer matches its manifest
    let text = std::fs::read_to_string(&out.table).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    let rules: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].contains(r#""type":"rule""#))
        .collect();
    assert!(rules.len() >= 2);
    lines.swap(rules[0], rules[1]);
    let swapped = out.table.with_extension("swapped.jsonl");
    std::fs::write(&swapped, lines.join("\n") + "\n").unwrap();
    assert!(verify_assembled(&swapped, &out.manifest).is_err());
    let tampered = out.table.with_extension("tampered.jsonl");
    std::fs::write(&tampered, text.replacen(r#""1"]"#, r#""2"]"#, 1)).unwrap();
    assert!(verify_assembled(&tampered, &out.manifest).is_err());
}

// =================================================================================================================
// 9. FULL-node retirements and removals
// =================================================================================================================

/// audit-accept-A M2 / B M3 + integration int4: a FULL retirement is refused by default; with `BoundProof` it is
/// accepted only when its containment proof was made by `economy::prove_coverage_preserved` (the ONLY constructor:
/// it runs the dispatch-mode partition and computes both digests itself) on THIS assembly's tables in force before
/// and after the retirement (`assemble::tables_in_force`, the one digest `economy::table_set_digest`), with the
/// same-order statement. Refused: a proof for other tables, for another order statement, a proof of another
/// retirement; and a retirement whose new table set has a gap gets no proof at all (Retirement::new needs one).
#[test]
fn full_retirement_refused_unless_bound() {
    let Some(t) = toy() else { return };
    let rule_lines = |p: &Path| -> Vec<(i64, String)> {
        std::fs::read(p)
            .unwrap()
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
            .filter_map(|l| {
                // an edited line is re-serialised with sorted keys: classify by the parsed kind, not a prefix
                let v: Value = serde_json::from_slice(l).unwrap();
                (v["type"] == "rule").then(|| (v["index"].as_i64().unwrap(), sha256_tag(l)))
            })
            .collect()
    };
    // inputs: the node's whole table (priority 0) and a table holding a COPY of one of its records (the K2-single
    // record) under another index (priority 1): the copy is selected exactly where its original is, AFTER it, so it
    // never serves and retiring it preserves dispatch coverage (TEST DATA: TOY2kite, record 2).
    let whole = InputTable {
        id: "whole".into(),
        path: t.node.dir.join("table.jsonl"),
        priority: 0,
        sha256: None,
    };
    let copy_path = removed_part(&t, "copy-of-k2", |kind, v| {
        if kind == "rule" {
            v["index"] = json!(1000);
        }
        kind == "rule"
    });
    let copied: i64 = ko_entry(&t.node, KO)["removed_record_indices"][0]
        .as_i64()
        .unwrap();
    let originals = rule_lines(&whole.path);
    let victim = rule_lines(&copy_path)[0].1.clone();
    let by = originals
        .iter()
        .find(|(i, _)| *i == copied)
        .unwrap()
        .1
        .clone();
    let copy = InputTable {
        id: "copy".into(),
        path: copy_path,
        priority: 1,
        sha256: None,
    };
    let mut inp = assembly(&t, vec![whole.clone(), copy], vec![], NodeTarget::Full);
    // partfix3 (audit-part3 C1): the minting function takes a work budget only
    let o = ProofWork::default();
    let same = DispatchOrderStatement::SameOrder;
    let before = tables_in_force(&inp, &[]).unwrap();
    let after = tables_in_force(&inp, std::slice::from_ref(&victim)).unwrap();
    let (bs, asrc) = (
        DispatcherSource::new(&before),
        DispatcherSource::new(&after),
    );
    assert_eq!(
        (bs.len(), asrc.len()),
        (originals.len() + 1, originals.len())
    );
    let proof = prove_coverage_preserved(&bs, &asrc, &t.node.frame, &o, &same)
        .expect("the copy never serves: dispatch coverage preserved");
    assert_eq!(
        proof.old_tables_digest(),
        table_set_digest(&bs, &t.node.frame)
    );
    assert_eq!(
        proof.new_tables_digest(),
        table_set_digest(&asrc, &t.node.frame)
    );
    let m = |terms: u64| RuleMeasure {
        cases: 1,
        rhs_terms: terms,
        monomials: terms,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 0,
    };
    let retire = |old: &str, new: &str, p| {
        Retirement::new(
            &t.node.name,
            CoverageState::Full,
            &[(old.to_string(), m(10))],
            (new, &m(2)),
            SupersedeReason::CheaperSameOrLargerDomain,
            p,
        )
    };
    inp.retirements = vec![retire(&victim, &by, Some(proof.clone())).unwrap()];
    assert!(
        assemble(&inp, &scratch("regress-ret")).is_err(),
        "refused by default"
    );
    inp.full_retirements = FullRetirements::BoundProof;
    let ok = assemble(&inp, &scratch("regress-ret")).unwrap();
    assert_eq!(ok.report.retired, vec![victim.clone()]);
    // the shipped table's dispatch set is the one the proof is about (labels / renumbering enter no digest)
    let tf = read_table_file(&ok.table, ReadOptions { rhs: RhsMode::Skip }).unwrap();
    let shipped = Dispatcher::new(
        vec![
            Table::from_table_file("shipped", "assembled", &tf, DomainOptions::default()).unwrap(),
        ],
        vec![0],
    )
    .unwrap();
    assert_eq!(
        table_set_digest(&DispatcherSource::new(&shipped), &t.node.frame),
        proof.new_tables_digest()
    );
    // (a) a proof for OTHER tables: the whole table alone (a valid proof, but not of this assembly's sets)
    let alone = assembly(&t, vec![whole.clone()], vec![], NodeTarget::Full);
    let w = tables_in_force(&alone, &[]).unwrap();
    let other = prove_coverage_preserved(
        &DispatcherSource::new(&w),
        &DispatcherSource::new(&w),
        &t.node.frame,
        &o,
        &same,
    )
    .expect("the whole table has no gap");
    inp.retirements = vec![retire(&victim, &by, Some(other)).unwrap()];
    let e = assemble(&inp, &scratch("regress-ret")).unwrap_err();
    assert!(
        e.to_string().contains("not about this assembly"),
        "proof about other tables: {e}"
    );
    // (b) the right tables but ANOTHER order statement (a retirement removes records in place)
    let new_order = prove_coverage_preserved(
        &bs,
        &asrc,
        &t.node.frame,
        &o,
        &DispatchOrderStatement::NewOrder("sha256:00ff".into()),
    )
    .unwrap();
    inp.retirements = vec![retire(&victim, &by, Some(new_order)).unwrap()];
    let e = assemble(&inp, &scratch("regress-ret")).unwrap_err();
    assert!(e.to_string().contains("order=same"), "order statement: {e}");
    // (c) a retirement whose new table set has a GAP: the partition verdict is not Proved, so no proof exists, and a
    // FULL retirement cannot be built without one; the valid proof of the copy's retirement does not fit it either
    let load_bearing = originals.last().unwrap().1.clone();
    let gap = tables_in_force(&inp, std::slice::from_ref(&load_bearing)).unwrap();
    assert!(
        prove_coverage_preserved(&bs, &DispatcherSource::new(&gap), &t.node.frame, &o, &same)
            .is_err(),
        "a gap is never proved"
    );
    assert!(retire(&load_bearing, &by, None).is_err());
    inp.retirements = vec![retire(&load_bearing, &by, Some(proof.clone())).unwrap()];
    let e = assemble(&inp, &scratch("regress-ret")).unwrap_err();
    assert!(
        e.to_string().contains("not about this assembly"),
        "proof of another retirement: {e}"
    );
}

// =================================================================================================================
// 10. int3 F2 (guard version) and other majors / minors
// =================================================================================================================

/// int3 F2: a pre-guard record (loci present, no guard version anywhere) is NOT CARRIED in the tables in force, as for
/// the candidate; a record without its own guard version takes the table header's.
#[test]
fn f2_guard_version_one_rule_for_chain_and_tables_in_force() {
    let Some(t) = toy() else { return };
    // the KO table without any guard_version
    let text = std::fs::read_to_string(&t.ko_path).unwrap();
    let stripped: String = text
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.remove("guard_version");
            }
            serde_json::to_string(&v).unwrap() + "\n"
        })
        .collect();
    let p = scratch("regress-f2").join("pre-guard.table.jsonl");
    std::fs::write(&p, stripped).unwrap();
    let tf = read_table_file(&p, ReadOptions { rhs: RhsMode::Skip }).unwrap();
    let core = Dispatcher::new(
        vec![
            Table::from_table_file("ko", p.display().to_string(), &tf, DomainOptions::default())
                .unwrap(),
        ],
        vec![0],
    )
    .unwrap();
    let mut b = InForceBuilder::new(&t.node.frame);
    b.table_file("ko", &p).unwrap();
    let inf = b.build().unwrap();
    let covered = t
        .node
        .box_keys
        .iter()
        .find(|k| core.first_match(k).unwrap().is_covered())
        .expect("the core loader alone credits pre-guard records")
        .clone();
    assert!(matches!(
        inf.dispatcher().unwrap().first_match(&covered).unwrap(),
        Dispatch::Blocked { .. }
    ));
    // the candidate without its own guard_version: credited with the table header's
    let ng = edit(&t.w0, |v| {
        v.as_object_mut().unwrap().remove("guard_version");
    });
    let rule = RuleText::parse(&ng, &t.node.frame).unwrap();
    let mut src = source(TOY, KO, t.off, 0);
    src.table_guard_version = Some(2);
    let mut prv = replay_prover(&t.node);
    let v = judge(
        &t,
        &ng,
        Some(&t.ko),
        boxed(),
        RequiredEvidence::Replay,
        &mut ExactRecordFacts::new(),
        &mut prv,
        src,
        position(&rule, KO, 0),
        &t.gaps,
    );
    // (the replay record itself still reads the record / header; acceptance's credit basis is the loci)
    if let Some(a) = v.accepted() {
        assert_eq!(a.record().value()["credit_basis"], "witness_loci");
    } else {
        assert!(
            !matches!(
                v,
                AcceptVerdict::Undecided {
                    check: Check::Record,
                    ..
                }
            ),
            "{v:?}"
        );
    }
}

/// audit-accept-A M7: `P + M/(Q)` is not a ratio of its two sides; the coefficient is Unknown, never Zero.
#[test]
fn a_m7_sum_after_ratio_is_unknown() {
    let Some(t) = toy() else { return };
    let m = edit(&t.w0, |v| v["rhs"][1][1] = json!("n2-4+1/(n1)"));
    let r = RuleText::parse(&m, &t.node.frame).unwrap();
    let k = t
        .gaps
        .iter()
        .find(|g| r.domain.as_ref().unwrap().applies(g).applies())
        .unwrap()
        .clone();
    assert_eq!(
        ExactRecordFacts::new().coefficient(&r, &k, 1),
        CoefFact::Unknown
    );
}

/// audit-accept-B m3 / A m5: a string VALUE "index" before the key, and an index beyond the first MiB, are spliced.
#[test]
fn splice_index_value_string_and_deep_index() {
    let rec = br#"{"type":"rule","note":"index","index":3,"rhs":[]}"#;
    match splice_index_head(rec, Some(3), 7) {
        Splice::Done(v) => assert_eq!(
            v,
            br#"{"type":"rule","note":"index","index":7,"rhs":[]}"#.to_vec()
        ),
        other => panic!("{other:?}"),
    }
    let dup = br#"{"type":"rule","index":3,"index":3,"rhs":[]}"#;
    assert!(matches!(
        splice_index_head(dup, Some(3), 7),
        Splice::Refused(_)
    ));
    let cut = br#"{"type":"rule","pad":"xx","ind"#;
    assert_eq!(splice_index_head(cut, Some(3), 7), Splice::NotInHead);
}

// =================================================================================================================
// Recipes (audit-accept-A M6, B M5 M6 m2)
// =================================================================================================================

fn recipe_ctx(t: &Toy) -> (Recipe, Dispatcher, Vec<Key>, Vec<Vec<Key>>) {
    let (_, tab) = load_table(&t.ko_path, "ko");
    let ko = Dispatcher::new(vec![tab], vec![0]).unwrap();
    let w = RuleText::parse(&t.w0, &t.node.frame).unwrap();
    let file = recipe_file(&w.value, &t.node.frame, 2);
    let recipe = Recipe::from_file(&file, "sha256:order", "c1", &t.node.frame).unwrap();
    let inst = Instantiator::new(&w).unwrap();
    let rhs: Vec<Vec<Key>> = t
        .gaps
        .iter()
        .map(|k| {
            inst.terms(k)
                .map(|ts| ts.into_iter().map(|(x, _)| x).collect())
                .unwrap_or_default()
        })
        .collect();
    (recipe, ko, t.gaps.clone(), rhs)
}

fn ans(d: &str, k: &Key, run: u32, rhs: &[Key], rel: &str) -> String {
    format!(
        "{}\n",
        json!({"schema": "rustred.campaign.recipe-solve.v1", "recipe_digest": d, "key": k, "run": run,
        "status": "ok", "failure": null, "rhs_keys": rhs, "relation_digest": rel,
        "regular": "regular", "exact_check": "pass", "cost_ms": 1, "binary_sha256": "sha256:bin",
        "determinism_contract": "c1"})
    )
}

#[test]
fn recipes_bound_sealed_and_deterministic() {
    let Some(t) = toy() else { return };
    let (recipe, ko, keys, rhs) = recipe_ctx(&t);
    let input = |bin: Option<&str>| RecipeInput {
        node: view(&t.node, boxed(), RequiredEvidence::PerKey),
        before: Some(&ko),
        after: None,
        recipe: &recipe,
        keys: &keys,
        determinism: Determinism::Count(1),
        expected_binary: bin.map(str::to_string),
    };
    let mut text = String::new();
    for (k, r) in keys.iter().zip(&rhs) {
        text.push_str(&ans(recipe.digest(), k, 1, r, &format!("sha256:rel-{k:?}")));
    }
    let mut a = SolveAnswers::new(recipe.digest());
    a.load(&text).unwrap();
    // the determinism sample (Count(1)) is the first candidate; give it run 2
    let first = keys.iter().min().unwrap().clone();
    let fr = &rhs[keys.iter().position(|k| *k == first).unwrap()];
    a.load(&ans(
        recipe.digest(),
        &first,
        2,
        fr,
        &format!("sha256:rel-{first:?}"),
    ))
    .unwrap();
    // A M6: no stated binary -> never certified
    assert!(matches!(
        accept_recipe(&input(None), &a).unwrap(),
        RecipeVerdict::Undecided { .. }
    ));
    let ok = accept_recipe(&input(Some("sha256:bin")), &a).unwrap();
    assert!(matches!(ok, RecipeVerdict::Certified { .. }), "{ok:?}");
    // B M6: a differing run-2 answer OUTSIDE the sample is a determinism witness too
    let other = keys.iter().max().unwrap().clone();
    let orr = &rhs[keys.iter().position(|k| *k == other).unwrap()];
    let mut b = a.clone();
    b.load(&ans(recipe.digest(), &other, 2, orr, "sha256:different"))
        .unwrap();
    assert!(matches!(
        accept_recipe(&input(Some("sha256:bin")), &b).unwrap(),
        RecipeVerdict::Rejected { .. }
    ));
    // B m2: an empty right-hand side is open (malformed), never certified on the exact check alone
    let mut c = SolveAnswers::new(recipe.digest());
    let mut ctext = text.replacen(&serde_json::to_string(&rhs[0]).unwrap(), "[]", 1);
    ctext.push_str(&ans(
        recipe.digest(),
        &first,
        2,
        fr,
        &format!("sha256:rel-{first:?}"),
    ));
    c.load(&ctext).unwrap();
    if let RecipeVerdict::Certified { open, .. } =
        accept_recipe(&input(Some("sha256:bin")), &c).unwrap()
    {
        assert!(
            open.iter()
                .any(|(_, r)| matches!(r, OpenReason::Malformed(_)))
        );
    }
    // A M6: shipping a verdict with another recipe is an error, not a silent mix
    let other_recipe = Recipe::from_file(
        &recipe_file(
            &RuleText::parse(&t.w0, &t.node.frame).unwrap().value,
            &t.node.frame,
            3,
        ),
        "sha256:order",
        "c1",
        &t.node.frame,
    )
    .unwrap();
    let ship = [RecipeToShip {
        recipe: &other_recipe,
        position: position(&RuleText::parse(&t.w0, &t.node.frame).unwrap(), KO, 0),
        verdict: &ok,
    }];
    assert!(write_recipes(TOY, &scratch("regress-recipes"), None, &ship).is_err());
    // A M6: engine results must be sealed; conflicts are errors
    let rel = |k: &Key, d: &str| {
        json!({"type":"relation","ordinal":0,"key":k,"verdict":"SOLVED","determinism_id":"c1","recipe_digest":"e",
            "rhs":[],"relation_digest":d,"proof":{"ok":true},"seconds":0.1})
        .to_string()
    };
    let k = keys[0].clone();
    let mut e = SolveAnswers::new(recipe.digest());
    assert!(
        e.load_engine_results(&format!("{}\n", rel(&k, "aa")), 1, "e", "b")
            .is_err(),
        "no end record"
    );
    assert!(
        e.load_engine_results(
            &format!(
                "{}\n{}\n",
                rel(&k, "aa"),
                json!({"type":"partial","records":1})
            ),
            1,
            "e",
            "b"
        )
        .is_err(),
        "partial"
    );
    assert!(
        e.load_engine_results(
            &format!(
                "{}\n{}\n{}\n",
                rel(&k, "aa"),
                rel(&k, "bb"),
                json!({"type":"end","records":2})
            ),
            1,
            "e",
            "b"
        )
        .is_err(),
        "conflicting answers"
    );
    assert_eq!(
        e.load_engine_results(
            &format!("{}\n{}\n", rel(&k, "aa"), json!({"type":"end","records":1})),
            1,
            "e",
            "b"
        )
        .unwrap(),
        1
    );
}

/// B M5: a certified recipe in force is its CERTIFIED keys only: an uncertified key of its domain falls through.
#[test]
fn b_m5_recipe_in_force_only_on_certified_keys() {
    let Some(t) = toy() else { return };
    let (recipe, _ko, keys, rhs) = recipe_ctx(&t);
    let ko_disp = {
        let (_, tab) = load_table(&t.ko_path, "ko");
        Dispatcher::new(vec![tab], vec![0]).unwrap()
    };
    let input = RecipeInput {
        node: view(&t.node, boxed(), RequiredEvidence::PerKey),
        before: Some(&ko_disp),
        after: None,
        recipe: &recipe,
        keys: &keys,
        determinism: Determinism::None,
        expected_binary: Some("sha256:bin".into()),
    };
    let mut text = String::new();
    for (i, (k, r)) in keys.iter().zip(&rhs).enumerate() {
        if i == 0 {
            // key 0 fails to solve: open, not certified
            text.push_str(
                &ans(recipe.digest(), k, 1, r, "x")
                    .replace(r#""status":"ok""#, r#""status":"failed""#),
            );
        } else {
            text.push_str(&ans(recipe.digest(), k, 1, r, &format!("sha256:rel-{k:?}")));
        }
    }
    let mut a = SolveAnswers::new(recipe.digest());
    a.load(&text).unwrap();
    let v = accept_recipe(&input, &a).unwrap();
    let RecipeVerdict::Certified { certified, .. } = &v else {
        panic!("{v:?}")
    };
    assert!(!certified.contains(&keys[0]));
    let mut b = InForceBuilder::new(&t.node.frame);
    b.table_file("ko", &t.ko_path).unwrap();
    b.recipe(&recipe, &v).unwrap();
    let inf = b.build().unwrap();
    let d = inf.dispatcher().unwrap();
    assert_eq!(
        d.first_match(&keys[0]).unwrap(),
        Dispatch::Uncovered,
        "falls through"
    );
    assert!(d.first_match(&certified[0]).unwrap().is_covered());
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
