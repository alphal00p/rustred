//! Lane termmin (interfaces/termmin.md): the pure core: the state machine on a toy closed key set (3 terminals: one
//! without any relation, one eliminable at depth 2, one only by a relation that brings a new key), preferred terminals,
//! lying candidates, determinism under any answer order, resume from recorded answers, the exact symmetry relation at
//! the key, and the extension check through `import::check` on a toy imported set. Nothing here spawns a process.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use rustred_campaign_core::termmin::machine::{Candidate, Cost, SolveAnswer, SolvePurpose, SolveRequest};
use rustred_campaign_core::termmin::{
    ClosureOracle, Economy, ImportExtension, KeySetOracle, Termmin, TermminSpec, symmetry_relation_at_key,
};
use serde_json::{Value, json};

type Key = Vec<i32>;
const LINES: [bool; 3] = [true, true, false];
const M: [i32; 3] = [1, 1, 0]; // no relation exists
const E: [i32; 3] = [2, 1, 0]; // eliminable at depth 2
const N: [i32; 3] = [1, 3, 0]; // only by a relation that brings a new key
const X: [i32; 3] = [1, 1, -1]; // the new key

fn dr(k: &[i32]) -> i32 {
    k.iter().zip(LINES).map(|(x, l)| if l { (*x).max(0) } else { (-*x).max(0) }).sum()
}
fn cmp(a: &[i32], b: &[i32]) -> Ordering {
    (dr(a), a).cmp(&(dr(b), b))
}

fn spec() -> TermminSpec {
    TermminSpec::parse(
        r#"{"sequence":["dispatch","symmetry_relation","d1_cnone_foff","d2_clast_fon_mM1C","d3_cfirst_foff_mM1C"],
            "caps":{"max_seeds":1000}}"#,
    )
    .unwrap()
}

fn solved_line(source: &str, key: &[i32], rhs: &[&[i32]]) -> Value {
    json!({"schema":"rustred.campaign.solved-relation.v1","source":source,"recipe_sha256":"aa","recipe_digest":"d",
           "determinism_id":"det","binary_sha256":null,"key":key,"verdict":"SOLVED","relation_digest":"00",
           "rhs": rhs.iter().map(|k| json!({"key":k,"coef_sha256":"c","mark":"top"})).collect::<Vec<_>>(),
           "proof":{"kind":"exact-weights","ok":true,"weights":1},"regular":true,"results":null,"failure":null})
}

fn relation(step: &str, key: &[i32], rhs: &[&[i32]], sha: &str) -> SolveAnswer {
    SolveAnswer::Relation {
        candidate: Candidate {
            kind: "recipe".into(),
            step: step.into(),
            rhs: rhs.iter().map(|k| k.to_vec()).collect(),
            relation_digest: Some(format!("rel-{step}-{key:?}")),
            recipe_id: Some(format!("rid-{step}")),
            recipe_sha256: Some(sha.into()),
            payload: {
                let mut l = solved_line("x", key, rhs);
                l["recipe_sha256"] = json!(sha);
                json!({ "solved": l })
            },
        },
        cost: Cost::default(),
    }
}

fn failed() -> SolveAnswer {
    SolveAnswer::Failed {
        class: "no_relation_in_seeds".into(),
        verdict: Some("NO_RELATION_IN_SEEDS".into()),
        failure: json!({"class":"no_relation_in_seeds"}),
        line: None,
        cost: Cost::default(),
    }
}

/// The toy "engine": E solves at the depth-2 step, N solves at depth 2 into the new key X, the closing try serves X
/// (when `closable`), everything else has no relation in its seeds. `lie` = E's depth-2 answer names a higher key.
fn solver(closable: bool, lie: bool) -> impl Fn(&SolveRequest) -> SolveAnswer {
    move |r: &SolveRequest| match &r.purpose {
        SolvePurpose::Step { recipe, .. } => {
            let st = recipe.step.as_str();
            if r.key == E && st.starts_with("d2") {
                if lie {
                    relation(st, &E, &[&[3, 1, 0]], "aa")
                } else {
                    relation(st, &E, &[&M, &[2, 0, 0]], "aa")
                }
            } else if r.key == E && lie && st.starts_with("d3") {
                relation(st, &E, &[&M, &[2, 0, 0]], "aa")
            } else if r.key == N && st.starts_with("d2") {
                relation(st, &N, &[&X, &[1, 0, 0]], "aa")
            } else {
                failed()
            }
        }
        SolvePurpose::Closing { solve, .. } => {
            if closable && solve.key == X {
                let l = solved_line(&solve.source, &X, &[&M, &[0, 1, -1]]);
                SolveAnswer::Relation {
                    candidate: Candidate {
                        kind: "recipe".into(),
                        step: "closing".into(),
                        rhs: vec![M.to_vec(), vec![0, 1, -1]],
                        relation_digest: None,
                        recipe_id: None,
                        recipe_sha256: None,
                        payload: json!({ "solved": l }),
                    },
                    cost: Cost::default(),
                }
            } else {
                failed()
            }
        }
    }
}

fn keyset_oracle(closing: bool) -> KeySetOracle {
    KeySetOracle {
        lines: LINES.to_vec(),
        served: [vec![4, 1, 0]].into_iter().collect(),
        terminals: [M.to_vec(), E.to_vec(), N.to_vec()].into_iter().collect(),
        used_rules: 1,
        cmp: Box::new(cmp),
        symmetry: BTreeMap::new(),
        closing_source: closing.then(|| "default".to_string()),
        commits: vec![],
    }
}

fn machine(preferred: &[&[i32]]) -> Termmin {
    let pref: BTreeSet<Key> = preferred.iter().map(|k| k.to_vec()).collect();
    Termmin::new(
        &spec(),
        &[M.to_vec(), E.to_vec(), N.to_vec()],
        &pref,
        &LINES,
        &[0, 1],
        &[2],
        Economy {
            terminals: 3,
            closure_keys: 4,
            used_rules: 1,
            recipes: 0,
        },
        &cmp,
    )
    .unwrap()
}

/// Drive to the end; `perm` permutes each batch of pending solves (answer order), `one` = one solve at a time.
fn drive(
    m: &mut Termmin,
    o: &mut dyn ClosureOracle,
    s: &dyn Fn(&SolveRequest) -> SolveAnswer,
    perm: usize,
    one: bool,
    log: &mut Vec<(String, SolveAnswer)>,
) {
    for _ in 0..1000 {
        m.advance(o).unwrap();
        if m.done() {
            return;
        }
        let mut p = m.pending_solves();
        assert!(!p.is_empty(), "not done and nothing pending");
        let k = p.len();
        p.rotate_left(perm % k);
        if perm % 2 == 1 {
            p.reverse();
        }
        if one {
            p.truncate(1);
        }
        for r in p {
            let a = s(&r);
            log.push((r.id.clone(), a.clone()));
            m.answer(&r.id, a).unwrap();
        }
    }
    panic!("no termination");
}

fn outcome(m: &Termmin) -> (Vec<Key>, Vec<(Key, String, String)>) {
    let rep = m.report("x");
    (
        rep.final_terminals.clone(),
        rep.decisions.iter().map(|d| (d.key.clone(), d.outcome.clone(), d.class.clone())).collect(),
    )
}

#[test]
fn toy_three_terminals_final_set_is_the_true_master_only() {
    let mut m = machine(&[]);
    assert_eq!(m.order(), vec![N.to_vec(), E.to_vec(), M.to_vec()], "D+r descending");
    let mut o = keyset_oracle(true);
    let mut log = vec![];
    drive(&mut m, &mut o, &solver(true, false), 0, false, &mut log);
    let (fin, dec) = outcome(&m);
    assert_eq!(fin, vec![M.to_vec()]);
    assert_eq!(
        dec,
        vec![
            (N.to_vec(), "eliminated".into(), "eliminated".into()),
            (E.to_vec(), "eliminated".into(), "eliminated".into()),
            (M.to_vec(), "kept".into(), "no_relation_in_seeds".into()),
        ]
    );
    let rep = m.report("x");
    assert_eq!(rep.economy_before.terminals, 3);
    assert_eq!(rep.economy_after.terminals, 1);
    // E: dispatch (no rule), symmetry (no group), d1 no relation, d2 eliminated; d3 never attempted
    let e = &rep.terminals[1];
    let outs: Vec<&str> = e.steps.iter().map(|s| s.outcome.as_str()).collect();
    assert_eq!(outs, vec!["no_relation", "skipped", "no_relation", "eliminated", "pending"]);
    // N went through a closing try: its eliminating extension grew the closure by X
    let n = &rep.terminals[0];
    assert!(n.eliminated_by.is_some());
    assert_eq!(n.steps[3].outcome, "eliminated");
    assert!(rep.economy_after.closure_keys > rep.economy_before.closure_keys - 2);
    // M: every step recorded, kept
    assert!(rep.terminals[2].steps.iter().all(|s| s.outcome != "pending"));
    assert_eq!(o.commits, vec![N.to_vec(), E.to_vec()]);
}

#[test]
fn a_relation_bringing_an_unservable_key_is_discarded() {
    let mut m = machine(&[]);
    let mut o = keyset_oracle(false);
    let mut log = vec![];
    drive(&mut m, &mut o, &solver(false, false), 0, false, &mut log);
    let (fin, dec) = outcome(&m);
    assert_eq!(fin, vec![M.to_vec(), N.to_vec()]);
    assert_eq!(dec[0], (N.to_vec(), "kept".into(), "brings_unserved_keys".into()));
    assert_eq!(dec[1], (E.to_vec(), "eliminated".into(), "eliminated".into()));
    let rep = m.report("x");
    let ext = rep.terminals[0].steps[3].extension.clone().unwrap();
    assert_eq!(ext.new_open, vec![X.to_vec()]);
}

#[test]
fn a_preferred_terminal_is_never_attempted() {
    let mut m = machine(&[&E]);
    let mut o = keyset_oracle(true);
    let mut log = vec![];
    drive(&mut m, &mut o, &solver(true, false), 0, false, &mut log);
    assert!(log.iter().all(|(id, _)| !id.starts_with("t1.")), "no request for the preferred terminal: {log:?}");
    let rep = m.report("x");
    assert_eq!(rep.terminals[1].outcome, "preferred");
    assert!(rep.final_terminals.contains(&E.to_vec()));
}

#[test]
fn a_lying_relation_is_refused_and_the_next_step_tried() {
    let mut m = machine(&[]);
    let mut o = keyset_oracle(true);
    let mut log = vec![];
    drive(&mut m, &mut o, &solver(true, true), 0, false, &mut log);
    let rep = m.report("x");
    let e = &rep.terminals[1];
    let d2 = &e.steps[3];
    assert_eq!(d2.extension.as_ref().unwrap().class, "relation_does_not_descend", "{d2:?}");
    assert_eq!(e.steps[4].outcome, "eliminated");
    assert_eq!(rep.final_terminals, vec![M.to_vec()]);
}

#[test]
fn decisions_do_not_depend_on_the_answer_order_and_resume_replays_them() {
    let mut reference = None;
    for perm in 0..6 {
        for one in [false, true] {
            let mut m = machine(&[]);
            let mut o = keyset_oracle(true);
            let mut log = vec![];
            drive(&mut m, &mut o, &solver(true, false), perm, one, &mut log);
            let rep = m.report("x");
            let mut terms = serde_json::to_value(&rep.terminals).unwrap();
            for t in terms.as_array_mut().unwrap() {
                for s in t["steps"].as_array_mut().unwrap() {
                    s["cost"] = Value::Null;
                }
            }
            let got = (outcome(&m), terms);
            match &reference {
                None => reference = Some(got),
                Some(r) => assert_eq!(&got, r, "perm {perm} one {one}"),
            }
            // resume at every prefix of the recorded answers
            for cut in 0..log.len() {
                let mut m2 = machine(&[]);
                let mut o2 = keyset_oracle(true);
                for (id, a) in &log[..cut] {
                    m2.answer(id, a.clone()).unwrap();
                }
                let mut log2 = vec![];
                drive(&mut m2, &mut o2, &solver(true, false), perm + 1, !one, &mut log2);
                assert_eq!(outcome(&m2), reference.as_ref().unwrap().0, "resume after {cut} answers");
            }
        }
    }
}

#[test]
fn a_second_different_answer_is_refused() {
    let mut m = machine(&[]);
    let p = m.pending_solves();
    let id = p[0].id.clone();
    m.answer(&id, failed()).unwrap();
    m.answer(&id, failed()).unwrap();
    assert!(m.answer(&id, relation("d2", &E, &[&M], "aa")).is_err());
    assert!(m.answer("t9.s9", failed()).is_err());
}

const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";

#[test]
fn symmetry_relation_at_the_key_is_exact_and_descends() {
    use rustred_campaign_core::import::symmetry::verify_relation_with;
    use rustred_campaign_core::independent_algebra::Independent;
    let f = Family::parse(SUNSET).unwrap();
    let chart = [true, true, true, false, false];
    let g = verify_derived(&f, &chart, &DeriveOptions::default()).unwrap();
    let lines = chart;
    let c = |a: &[i32], b: &[i32]| -> Ordering {
        let d = |k: &[i32]| -> i32 {
            k.iter().zip(lines).map(|(x, l)| if l { (*x).max(0) } else { (-*x).max(0) }).sum()
        };
        (d(a), a).cmp(&(d(b), b))
    };
    // the corner: every element fixes it up to a permutation of equal powers -> no relation
    let r = symmetry_relation_at_key(&Independent, &g, &[1, 1, 1, 0, 0], &c, 1 << 16);
    assert!(matches!(r, rustred_campaign_core::termmin::SymRelation::NoRelation { .. }), "{r:?}");
    // a dotted key: its image with the dot on a lower line is a one-term relation
    let mut found = 0;
    for key in [[1, 2, 1, 0, 0], [2, 1, 1, 0, 0], [1, 1, 2, 0, 0], [1, 1, 1, -1, 0], [1, 1, 1, 0, -1]] {
        match symmetry_relation_at_key(&Independent, &g, &key, &c, 1 << 16) {
            rustred_campaign_core::termmin::SymRelation::Relation { rhs, .. } => {
                found += 1;
                for (k, _) in &rhs {
                    assert_eq!(c(k, &key), Ordering::Less, "{key:?} -> {k:?}");
                }
                let txt: Vec<(Vec<i32>, String)> = rhs
                    .iter()
                    .map(|(k, q)| (k.clone(), rustred_campaign_core::termmin::symrel::q_text(q)))
                    .collect();
                let v = verify_relation_with(&Independent, &g, &key, &txt, &[key.to_vec()], 1 << 16);
                assert!(v.verified(), "{key:?}: {v:?}");
            }
            rustred_campaign_core::termmin::SymRelation::NoRelation { .. } => {}
            rustred_campaign_core::termmin::SymRelation::Unknown(w) => panic!("{key:?}: {w}"),
        }
    }
    assert!(found >= 2, "dotted keys of a symmetric family have lower images ({found})");
}

// ------------------------------------------------------------------ the import-based oracle ----------------------------

const HEADER: &str = r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha256(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn toy_import(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let rule = json!({"type":"rule","chart":"110","index":0,"fixed":[4,1,0],"equations":[],"target":"I(4,1,0)",
        "rhs":[["I(2,1,0)","1"],["I(1,3,0)","1"],["I(1,1,0)","1"],["I(4,0,0)","1"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let a = dir.join("A.table.jsonl");
    std::fs::write(
        &a,
        format!(
            "{HEADER}\n{}\n{rule}\n{}\n",
            r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#,
            r#"{"type":"end","charts":1}"#
        ),
    )
    .unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[4,1,0]\n").unwrap();
    let dflt = dir.join("default.recipe.json");
    std::fs::write(
        &dflt,
        json!({"schema":"rustred.recipe.v1","id":"default","arity":3,"chart":"110",
            "domain":{"fixed":[null,null,null],"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"K"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256(&std::fs::read(&a).unwrap())},
                   {"id":"default-recipe","kind":"recipe","class":"recipe","path": dflt.display().to_string()},
                   {"id":"terminals","kind":"terminals","keys":[M, E, N]}]});
    let p = dir.join("import-spec.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    p
}

fn import_oracle(dir: &Path) -> ImportExtension {
    let p = toy_import(dir);
    let spec = TableSetSpec::read(&p).unwrap();
    let set = load(&spec, dir).unwrap();
    let ev = Evidence::new(&set);
    ImportExtension::new(set, ev, CheckOptions::default(), None, 1 << 16).unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("termmin-core-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn import_extension_decides_through_the_import_check() {
    let d = tmp("ext");
    let mut o = import_oracle(&d);
    assert_eq!(o.base().economy.terminals, 3, "{:?}", o.base());
    assert!(o.base().open.is_empty(), "{:?}", o.base().open);
    // dispatch: no rule serves E -> not served
    let x = o.extend(&E, &Candidate::dispatch(), &[]);
    assert!(!x.served, "{x:?}");
    // E by a relation onto the remaining terminal M and a hand-down: closed, one terminal less
    let SolveAnswer::Relation { candidate: ce, .. } = relation("d2_clast_fon_mM1C", &E, &[&M, &[2, 0, 0]], "aa") else {
        unreachable!()
    };
    let x = o.extend(&E, &ce, &[]);
    assert!(x.served && x.new_open.is_empty(), "{x:?}");
    assert_eq!(x.economy.terminals, 2);
    // the base is untouched by a check
    assert_eq!(o.extend(&E, &Candidate::dispatch(), &[]).served, false);
    // N onto the new key X: X is new and open; the closing try asks the node's default recipe for it
    let SolveAnswer::Relation { candidate: cn, .. } = relation("d2_clast_fon_mM1C", &N, &[&X, &[1, 0, 0]], "aa") else {
        unreachable!()
    };
    let x = o.extend(&N, &cn, &[]);
    assert!(x.served, "{x:?}");
    assert_eq!(x.new_open, vec![X.to_vec()]);
    let reqs = o.closing_requests(&N, &cn, &x);
    assert_eq!(reqs.len(), 1);
    assert_eq!((reqs[0].source.as_str(), reqs[0].key.clone()), ("default-recipe", X.to_vec()));
    // with the closing answer (the default recipe's sha256 must be the file's) X is served
    let mut line = solved_line("default-recipe", &X, &[&M, &[0, 1, -1]]);
    line["recipe_sha256"] = json!(reqs[0].recipe_sha256);
    let x2 = o.extend(&N, &cn, &[line.clone()]);
    assert!(x2.served && x2.new_open.is_empty(), "{x2:?}");
    assert_eq!(x2.economy.terminals, 2);
    assert_eq!(x2.economy.closure_keys, x.economy.closure_keys);
    // commit E, then N with its closing answer: one terminal left
    o.commit(&E, &ce, &[]).unwrap();
    o.commit(&N, &cn, &[line]).unwrap();
    assert_eq!(o.current().terminals, vec![M.to_vec()]);
    // a recipe relation whose sha256 is not its recipe's is refused by the import (served = false)
    let SolveAnswer::Relation { candidate: mut bad, .. } = relation("d3", &M, &[&[1, 0, 0]], "aa") else {
        unreachable!()
    };
    bad.recipe_sha256 = Some("bb".into());
    let x = o.extend(&M, &bad, &[]);
    assert!(!x.served, "{x:?}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn machine_with_the_import_oracle_reaches_the_same_final_set() {
    let d = tmp("mach");
    let mut o = import_oracle(&d);
    let dflt_sha = sha256(&std::fs::read(d.join("default.recipe.json")).unwrap());
    let mut m = machine(&[]);
    let base = solver(true, false);
    let s = move |r: &SolveRequest| -> SolveAnswer {
        let mut a = base(r);
        if let (SolvePurpose::Closing { .. }, SolveAnswer::Relation { candidate, .. }) = (&r.purpose, &mut a) {
            candidate.payload["solved"]["recipe_sha256"] = json!(dflt_sha);
        }
        a
    };
    let mut log = vec![];
    drive(&mut m, &mut o, &s, 0, false, &mut log);
    let (fin, dec) = outcome(&m);
    assert_eq!(fin, vec![M.to_vec()], "{dec:?}");
    let rep = m.report("x");
    assert_eq!(rep.economy_after.terminals, 1);
    assert!(rep.economy_after.closure_keys > rep.economy_before.closure_keys, "X joined the closure");
    let _ = std::fs::remove_dir_all(&d);
}
