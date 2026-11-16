//! Lane polefix (audit r2-poles F4): a call site whose pole code only ANNOTATES (termmin's `eliminated_by.choice`) writes
//! NO annotation with `pole_objective: "none"`: the report is byte-identical to the default run's report with every
//! `choice` member removed (= a run without the pole code); an objective object is read from the spec; bad settings
//! are refused. Termmin harness copied from tests/termmin_core.rs (toy, no process). Pure.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::poles::{Measure, PoleObjective};
use rustred_campaign_core::termmin::machine::{
    Candidate, Cost, SolveAnswer, SolvePurpose, SolveRequest,
};
use rustred_campaign_core::termmin::{ClosureOracle, Economy, KeySetOracle, Termmin, TermminSpec};
use serde_json::{Value, json};

type Key = Vec<i32>;
const LINES: [bool; 3] = [true, true, false];
const M: [i32; 3] = [1, 1, 0]; // no relation exists
const E: [i32; 3] = [2, 1, 0]; // eliminable at depth 2
const N: [i32; 3] = [1, 3, 0]; // only by a relation that brings a new key
const X: [i32; 3] = [1, 1, -1]; // the new key

fn dr(k: &[i32]) -> i32 {
    k.iter()
        .zip(LINES)
        .map(|(x, l)| if l { (*x).max(0) } else { (-*x).max(0) })
        .sum()
}
fn cmp(a: &[i32], b: &[i32]) -> Ordering {
    (dr(a), a).cmp(&(dr(b), b))
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

fn machine_with(spec: &TermminSpec, preferred: &[&[i32]]) -> Termmin {
    let pref: BTreeSet<Key> = preferred.iter().map(|k| k.to_vec()).collect();
    Termmin::new(
        spec,
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

const SEQ: &str = r#""sequence":["dispatch","symmetry_relation","d1_cnone_foff","d2_clast_fon_mM1C","d3_cfirst_foff_mM1C"],
            "caps":{"max_seeds":1000}"#;

fn spec_with(extra: &str) -> TermminSpec {
    TermminSpec::parse(&format!("{{{SEQ}{extra}}}")).unwrap()
}

fn run(spec: &TermminSpec) -> Value {
    let mut m = machine_with(spec, &[]);
    let mut o = keyset_oracle(true);
    let mut log = vec![];
    drive(&mut m, &mut o, &solver(true, false), 0, false, &mut log);
    serde_json::to_value(m.report("x")).unwrap()
}

fn strip_choice(v: &mut Value) -> usize {
    let mut n = 0;
    for t in v["terminals"].as_array_mut().unwrap() {
        if let Some(e) = t["eliminated_by"].as_object_mut()
            && e.remove("choice").is_some()
        {
            n += 1;
        }
    }
    n
}

#[test]
fn objective_none_writes_no_choice_and_the_report_is_byte_identical() {
    let dflt = run(&spec_with(""));
    let none = run(&spec_with(r#","pole_objective":"none""#));
    // the default run annotates both eliminations
    let mut stripped = dflt.clone();
    assert_eq!(strip_choice(&mut stripped), 2, "{dflt:#}");
    // "none": no choice member anywhere, and byte-identical to the default report without its annotations
    assert!(!none.to_string().contains("\"choice\""), "{none:#}");
    assert_eq!(none.to_string(), stripped.to_string());
    // the decisions themselves never depend on the setting
    assert_eq!(none["decisions"], dflt["decisions"]);
    assert_eq!(none["final_terminals"], dflt["final_terminals"]);
    // an objective object from the spec is used (its measures appear in the annotation)
    let obj = run(&spec_with(
        r#","pole_objective":{"measures":["size","max_pole_order"],"max_size_ratio":"8"}"#,
    ));
    let c = &obj["terminals"][0]["eliminated_by"]["choice"];
    assert_eq!(c["measures"], json!(["size", "max_pole_order"]), "{c}");
    assert_eq!(c["max_size_ratio"], "8");
    // an absent setting keeps the spec digest of before (the member is not serialised)
    assert!(
        !serde_json::to_string(&spec_with(""))
            .unwrap()
            .contains("pole_objective")
    );
}

#[test]
fn the_setting_reads_none_default_and_objects_and_refuses_the_rest() {
    assert_eq!(
        PoleObjective::setting(None).unwrap(),
        Some(PoleObjective::default())
    );
    assert_eq!(
        PoleObjective::setting(Some(&Value::Null)).unwrap(),
        Some(PoleObjective::default())
    );
    assert_eq!(PoleObjective::setting(Some(&json!("none"))).unwrap(), None);
    let o = PoleObjective::setting(Some(&json!({"measures": ["regular", "size"]})))
        .unwrap()
        .unwrap();
    assert_eq!(o.measures, vec![Measure::Regular, Measure::Size]);
    assert!(PoleObjective::setting(Some(&json!("off"))).is_err());
    assert!(PoleObjective::setting(Some(&json!(3))).is_err());
    assert!(PoleObjective::setting(Some(&json!({"measures": []}))).is_err());
    assert!(TermminSpec::parse(&format!("{{{SEQ},\"pole_objective\":\"off\"}}")).is_err());
}
