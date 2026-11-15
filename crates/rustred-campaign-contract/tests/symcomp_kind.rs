//! Kind `symmetry-composite` (lane symjob; interfaces/symmetry-composite-job.md): spec rules (roles, empty allow-list,
//! legacy constraints), the wrapper over a synthetic child seal written in the tool's exact format (rr_rf_symflow
//! `run --request`), and the decision rows it reaches: 33 / 34 / 35 (success with candidates), 36 (bound to other
//! inputs), 22 (unsealed / truncated / tampered / re-sealed results / self-check), 13 / 15b (tool refusal), signals.
//! Forged children: a child result RE-SEALED consistently passes the CONTRACT (the seal cannot know coefficients: the
//! import's verification refuses it, supervisor test symcomp_fake.rs); a changed file after the seal and a tampered
//! sealed results stream are refused here. The effective digest: pinned vector, independent recomputation, determinism.
//! Test data only (L8): keys of arity 4, a made-up family name.

mod common;
#[path = "../../rustred-campaign-core/src/closure/sha256.rs"]
#[allow(dead_code)]
mod sha256;

use common::*;
use rustred_campaign_contract::classify::recipe::FailureClass;
use rustred_campaign_contract::classify::symcomp::{self, ChildRefusal, SymClass};
use rustred_campaign_contract::classify::{
    self, Accept, Action, KillReason, OutcomeKind, ROWS, WaitStatus,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::{CONTEXT_EFFECTIVE, LICENCE_PRESENT};
use rustred_campaign_contract::spec::*;
use serde_json::{Value, json};
use std::path::Path;

fn sha(b: &[u8]) -> String {
    let mut h = sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn b3(b: &[u8]) -> String {
    rustred_campaign_contract::blake3_hex(b)
}

const PROG_SHA: &str = "ab12000000000000000000000000000000000000000000000000000000000000";

/// A consistent forger's edit of (table lines, verification lines, result.json) before sealing.
type ChildEdit = dyn Fn(&mut Vec<String>, &mut Vec<String>, &mut Value);
/// An edit of (results records, end record).
type ResultsEdit = dyn Fn(&mut Vec<Value>, &mut Value);

/// One synthetic key of the child: (key, class).
type K = (Vec<i64>, SymClass);

fn kt(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// A job: inputs outside the run dir (shared by several runs), request, spec; the run dir prepared.
struct Job {
    _t: TempDir,
    spec: JobSpec,
    digest: String,
    layout: RunLayout,
}

fn inputs_dir(t: &TempDir, keys: &[K], with_policy: bool) -> (Vec<InputRef>, Value) {
    let d = t.path().join("inputs");
    std::fs::create_dir_all(&d).unwrap();
    let file = |name: &str, body: &[u8], role: &str| {
        let p = d.join(name);
        std::fs::write(&p, body).unwrap();
        (
            InputRef {
                role: role.into(),
                path: p.display().to_string(),
                blake3: b3(body),
                upto: None,
            },
            json!({"path": p.display().to_string(), "sha256": sha(body)}),
        )
    };
    let (spec_in, spec_pin) = file("import-spec.json", b"{\"schema\":\"x\"}\n", "import_spec");
    let kb: String = keys
        .iter()
        .map(|(k, _)| format!("{}\n", serde_json::to_string(k).unwrap()))
        .collect();
    let (keys_in, keys_pin) = file("keys.jsonl", kb.as_bytes(), "keys");
    let mut ins = vec![spec_in, keys_in];
    let mut req = json!({"schema": symcomp::REQUEST_SCHEMA, "node": "N", "import_spec": spec_pin,
        "import_base": "/", "keys": keys_pin, "policy": null, "threads": 2, "limit": null,
        "program_sha256": PROG_SHA});
    if with_policy {
        let (p_in, p_pin) = file("policy.json", b"{\"schema\":\"p\"}\n", "policy");
        ins.push(p_in);
        req["policy"] = p_pin;
    }
    (ins, req)
}

fn job(t: TempDir, mut ins: Vec<InputRef>, req: &Value, attempt: u32) -> Job {
    let run = t.path().join(format!("run{attempt}"));
    let layout = RunLayout::new(&run);
    for d in [
        layout.in_dir(),
        layout.out_dir(),
        layout.tmp_dir(),
        layout.sup_dir(),
    ] {
        std::fs::create_dir_all(d).unwrap();
    }
    let rb = serde_json::to_vec_pretty(req).unwrap();
    let rp = layout.in_dir().join("request.json");
    std::fs::write(&rp, &rb).unwrap();
    ins.insert(
        0,
        InputRef {
            role: "request".into(),
            path: rp.display().to_string(),
            blake3: b3(&rb),
            upto: None,
        },
    );
    let spec = spec_with(&run, ins);
    spec.validate().unwrap();
    let bytes = spec.to_json_line();
    let digest = spec_blake3(&bytes);
    std::fs::write(layout.job_spec(), &bytes).unwrap();
    Job {
        _t: t,
        spec,
        digest,
        layout,
    }
}

fn spec_with(run: &Path, inputs: Vec<InputRef>) -> JobSpec {
    let mut s = sample_spec(&run.display().to_string());
    s.kind = JobKind::SymmetryComposite;
    s.program = ProgramRef {
        name: "rr_rf_symflow".into(),
        blake3: D0.into(),
        features_required: vec![],
    };
    s.inputs = inputs;
    s.args = vec![];
    s.resume = None;
    s.resume_every_s = 0;
    s.budgets = Budgets::default();
    s
}

/// The tool's sealed layout for `keys` (exact format of rr_rf_symflow run --request), optionally altered by `edit`
/// BEFORE sealing (a consistent forger).
fn write_child(j: &Job, keys: &[K], edit: &ChildEdit) {
    let out = j.layout.out_dir().join(symcomp::CHILD_DIR);
    std::fs::create_dir_all(&out).unwrap();
    let mut table = vec![
        json!({"type": "header", "schema": "table_dump.v1", "family": "TOY", "producer": "rr_rf_symflow",
        "kind": "symmetry-composite"})
        .to_string(),
    ];
    let mut ver = vec![];
    let mut kl = String::new();
    let mut cl = String::new();
    for (k, c) in keys {
        let face = "{0}x{}";
        let (status, best) = match c {
            SymClass::Plain => (
                "ok".to_string(),
                json!({"element": 1, "terms": 1, "plain": true, "replacements": 0}),
            ),
            SymClass::Composite => (
                "ok".to_string(),
                json!({"element": 2, "terms": 2, "plain": false, "replacements": 1}),
            ),
            SymClass::None => ("ok".to_string(), Value::Null),
            SymClass::OverCap => (
                "over_cap:row_expansion: symmetry row expansion bound 8".to_string(),
                Value::Null,
            ),
            SymClass::Error => ("error: no rows".to_string(), Value::Null),
        };
        kl.push_str(
            &json!({"key": k, "face": face, "status": status, "best": best, "alternatives": 1, "outcomes": []})
                .to_string(),
        );
        kl.push('\n');
        cl.push_str(&json!({"key": k, "face": face, "status": status}).to_string());
        cl.push('\n');
        if c.has_relation() {
            let idx = table.len() - 1;
            let mut lower = k.clone();
            lower[0] -= 1;
            let rhs = if *c == SymClass::Plain {
                json!([[kt(&lower), "1"]])
            } else {
                json!([[kt(&lower), "(d-4)/(2*d-6)"], [kt(&[0, 0, 0, 0]), "-1"]])
            };
            table.push(
                json!({"type": "rule", "chart": "1100", "index": idx, "kind": "symmetry-composite", "case": "point",
                "fixed": k, "equations": [], "target": kt(k), "rhs": rhs, "exceptions": [], "dispatch": "Point",
                "witness_loci": [], "symmetry_elements": [1]})
                .to_string(),
            );
            let reps = if *c == SymClass::Plain {
                json!([])
            } else {
                json!([{"term": kt(k), "depth": 0, "source": "S", "label": 3, "record_sha256": "cd".repeat(32)}])
            };
            ver.push(
                json!({"schema": symcomp::VERIFICATION_SCHEMA, "target": kt(k), "group_digest": "g".repeat(8),
                "element": 1, "image": [[kt(k), "1"]], "replacements": reps, "key_coefficient": null,
                "dropped_zero": [], "rhs_terms": rhs.as_array().unwrap().len(), "plain": *c == SymClass::Plain,
                "producer": "rr_rf_symflow/symbolica"})
                .to_string(),
            );
        }
    }
    let shipped = ver.len();
    let req: Value =
        serde_json::from_slice(&std::fs::read(j.layout.in_dir().join("request.json")).unwrap())
            .unwrap();
    let rb = std::fs::read(j.layout.in_dir().join("request.json")).unwrap();
    let mut inputs = vec![];
    for role in symcomp::PINNED_ROLES {
        if !req[role].is_null() {
            inputs.push(
                json!({"role": role, "path": req[role]["path"], "sha256": req[role]["sha256"]}),
            );
        }
    }
    let mut result = json!({"schema": symcomp::RESULT_SCHEMA, "verdict": "DONE", "request_sha256": sha(&rb),
        "tool": {"name": "rr_rf_symflow", "sha256": PROG_SHA}, "inputs": inputs,
        "group": {"digest": "g".repeat(8), "elements": 4}, "policy_digest": "pd", "spec_digest": null,
        "keys": keys.len(), "outputs": [], "totals": {}, "gate": "pass", "shipped_relations": shipped,
        "independently_verified": shipped});
    edit(&mut table, &mut ver, &mut result);
    let join = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("choices.jsonl", cl.into_bytes()),
        ("keys.jsonl", kl.into_bytes()),
        ("composite.table.jsonl", join(&table).into_bytes()),
        ("composite.verification.jsonl", join(&ver).into_bytes()),
        ("report.json", b"{\"schema\":\"r\"}\n".to_vec()),
    ];
    let mut sums = String::new();
    let mut outs = vec![];
    for (n, b) in &files {
        std::fs::write(out.join(n), b).unwrap();
        sums.push_str(&format!("{}  {n}\n", sha(b)));
        outs.push(json!({"file": n, "sha256": sha(b), "lines": b.iter().filter(|&&c| c == b'\n').count()}));
    }
    std::fs::write(out.join("SHA256SUMS"), sums).unwrap();
    if result["outputs"].as_array().is_some_and(Vec::is_empty) {
        result["outputs"] = json!(outs);
    }
    std::fs::write(
        out.join("result.json"),
        serde_json::to_string_pretty(&result).unwrap() + "\n",
    )
    .unwrap();
}

fn no_edit(_: &mut Vec<String>, _: &mut Vec<String>, _: &mut Value) {}

/// wrap + assess + classify: (row, sub, the classification).
fn settle(j: &Job, wait: WaitStatus) -> classify::Classification {
    let w = symcomp::wrap(&j.layout, &j.spec, &j.digest, wait, &sha, true);
    let ev = classify::assess(&j.layout, &j.spec, &j.digest, w, KillReason::None, false);
    classify::classify(&ev)
}

fn keys3() -> Vec<K> {
    vec![
        (vec![2, 1, 0, 0], SymClass::Plain),
        (vec![1, 2, 0, -1], SymClass::Composite),
        (vec![3, 1, -1, 0], SymClass::Plain),
    ]
}

fn mixed() -> Vec<K> {
    vec![
        (vec![2, 1, 0, 0], SymClass::Plain),
        (vec![1, 2, 0, -1], SymClass::None),
        (vec![1, 3, 0, -1], SymClass::Composite),
        (vec![4, 1, 0, 0], SymClass::OverCap),
        (vec![5, 1, 0, 0], SymClass::Error),
    ]
}

fn fresh(tag: &str, keys: &[K]) -> Job {
    let t = TempDir::new(tag);
    let (ins, req) = inputs_dir(&t, keys, false);
    job(t, ins, &req, 1)
}

#[test]
fn spec_rules_of_the_kind() {
    let t = TempDir::new("spec");
    let (ins, req) = inputs_dir(&t, &keys3(), true);
    let j = job(t, ins, &req, 1);
    assert_eq!(JobKind::SymmetryComposite.program(), "rr_rf_symflow");
    assert_eq!(JobKind::SymmetryComposite.level(), Level::Legacy);
    assert!(JobKind::SymmetryComposite.legacy_wrapped());
    assert!(JobKind::SymmetryComposite.fake_emulated());
    assert!(JobKind::SymmetryComposite.allowed_args().is_empty());
    assert!(JobKind::ALL.contains(&JobKind::SymmetryComposite));
    assert_eq!(
        serde_json::to_value(JobKind::SymmetryComposite).unwrap(),
        "symmetry-composite"
    );
    // every arg refused (empty allow-list)
    let mut s = j.spec.clone();
    s.args = vec!["--threads".into(), "4".into()];
    assert!(s.validate().is_err());
    // roles: request + import_spec required; keys / policy optional; no other role; no duplicate
    let mut s = j.spec.clone();
    s.inputs.retain(|i| i.role != "import_spec");
    assert!(s.validate().is_err());
    let mut s = j.spec.clone();
    s.inputs.retain(|i| i.role != "keys" && i.role != "policy");
    s.validate().unwrap();
    let mut s = j.spec.clone();
    let mut extra = s.inputs[0].clone();
    extra.role = "family".into();
    s.inputs.push(extra);
    assert!(s.validate().is_err());
    let mut s = j.spec.clone();
    let dup = s.inputs[1].clone();
    s.inputs.push(dup);
    assert!(s.validate().is_err());
    // legacy constraints: no resume, no work budget, sample seed 0, no case budget
    let mut s = j.spec.clone();
    s.seed.sample_seed = 7;
    assert!(s.validate().is_err());
    let mut s = j.spec.clone();
    s.budgets.wall.case_s = Some(10);
    assert!(s.validate().is_err());
    // the legacy argv is derived from the spec only
    let a = symcomp::argv(&j.spec).unwrap();
    assert_eq!(a[0], "run");
    assert_eq!(
        a[2],
        j.layout.in_dir().join("request.json").display().to_string()
    );
    assert_eq!(
        a[4],
        j.layout.out_dir().join("symflow").display().to_string()
    );
}

#[test]
fn rows_33_34_35_from_a_sealed_child() {
    // 33: every key has a relation; accept supplied_relations (candidates), item done
    let j = fresh("r33", &keys3());
    write_child(&j, &keys3(), &no_edit);
    let c = settle(&j, WaitStatus::Exited(0));
    assert_eq!(
        (c.row, c.accept, c.action),
        (33, Accept::SuppliedRelations, Action::ItemDone),
        "{}",
        c.why
    );
    assert_eq!(c.kind, OutcomeKind::Complete);
    // the sealed results: 3 key records, 3 relations, 6 file records; tally accepted 3
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Exited(0),
        KillReason::None,
        false,
    );
    let v = ev.verified.unwrap();
    let t = v.relations.unwrap();
    assert_eq!((t.records, t.accepted, t.unsolved_total()), (3, 3, 0));
    assert!(
        t.determinism_id
            .unwrap()
            .starts_with(symcomp::DETERMINISM_PREFIX)
    );
    // 34: some keys none / over_cap / error -> exit 10 keys_unsolved, typed per-key classes with their routes
    let j = fresh("r34", &mixed());
    write_child(&j, &mixed(), &no_edit);
    let c = settle(&j, WaitStatus::Exited(0));
    assert_eq!(
        (c.row, c.accept, c.action),
        (34, Accept::SuppliedRelations, Action::QueueFailures),
        "{}",
        c.why
    );
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Exited(10),
        KillReason::None,
        false,
    );
    let o = match &ev.outcome {
        classify::OutcomeState::Valid(o) => o.clone(),
        x => panic!("{x:?}"),
    };
    let (res, t) =
        classify::symmetry_composite_tally(&j.layout, &j.spec, &o, ev.verified.as_ref().unwrap())
            .unwrap();
    assert_eq!((t.records, t.accepted), (5, 2));
    assert_eq!(t.unsolved.get(&FailureClass::NoSymmetryRelation), Some(&1));
    assert_eq!(t.unsolved.get(&FailureClass::OverCapRowExpansion), Some(&1));
    assert_eq!(t.unsolved.get(&FailureClass::Error), Some(&1));
    let classes: Vec<SymClass> = res.keys.iter().map(|k| k.class).collect();
    assert_eq!(classes, mixed().iter().map(|(_, c)| *c).collect::<Vec<_>>());
    use rustred_campaign_contract::classify::recipe::KeyRoute;
    assert_eq!(
        FailureClass::NoSymmetryRelation.route(),
        KeyRoute::NextRecipeRung
    );
    assert_eq!(
        FailureClass::OverCapRowExpansion.route(),
        KeyRoute::NextWorkClass
    );
    assert_eq!(o.counters.rules_committed, 2);
    assert!(o.effective.as_ref().unwrap().verify());
    // 35: no relation at all
    let none: Vec<K> = vec![
        (vec![1, 1, 0, 0], SymClass::None),
        (vec![2, 2, 0, 0], SymClass::OverCap),
    ];
    let j = fresh("r35", &none);
    write_child(&j, &none, &no_edit);
    let c = settle(&j, WaitStatus::Exited(0));
    assert_eq!(
        (c.row, c.accept, c.action),
        (35, Accept::Nothing, Action::QueueFailures),
        "{}",
        c.why
    );
}

#[test]
fn row_36_results_bound_to_other_inputs() {
    let cases: Vec<(&str, Box<ChildEdit>)> = vec![
        (
            "request digest",
            Box::new(|_, _, r| r["request_sha256"] = json!("00".repeat(32))),
        ),
        (
            "input path",
            Box::new(|_, _, r| r["inputs"][0]["path"] = json!("/elsewhere/import-spec.json")),
        ),
        (
            "input digest",
            Box::new(|_, _, r| r["inputs"][1]["sha256"] = json!("11".repeat(32))),
        ),
        (
            "input missing",
            Box::new(|_, _, r| {
                r["inputs"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "tool digest",
            Box::new(|_, _, r| r["tool"]["sha256"] = json!("ff".repeat(32))),
        ),
    ];
    for (what, edit) in cases {
        let j = fresh("r36", &keys3());
        write_child(&j, &keys3(), &*edit);
        let c = settle(&j, WaitStatus::Exited(0));
        assert_eq!(
            (c.row, c.accept, c.action),
            (36, Accept::Nothing, Action::ItemAttentionNoRetry),
            "{what}: {}",
            c.why
        );
        assert!(c.attention);
        assert!(!j.layout.results().exists(), "{what}: nothing sealed");
    }
    // the request itself pins a file that changed after the spec was written
    let j = fresh("r36b", &keys3());
    write_child(&j, &keys3(), &no_edit);
    let kp = j
        .spec
        .inputs
        .iter()
        .find(|i| i.role == "keys")
        .unwrap()
        .path
        .clone();
    std::fs::write(&kp, "[9,9,9,9]\n").unwrap();
    let c = settle(&j, WaitStatus::Exited(0));
    assert_eq!(c.row, 36, "{}", c.why);
    // the request names another file than the spec's input of that role
    let t = TempDir::new("r36c");
    let (ins, mut req) = inputs_dir(&t, &keys3(), false);
    let other = t.path().join("inputs/other.jsonl");
    std::fs::copy(&ins[1].path, &other).unwrap();
    req["keys"]["path"] = json!(other.display().to_string());
    let j = job(t, ins, &req, 1);
    write_child(&j, &keys3(), &no_edit);
    assert!(matches!(
        symcomp::check_child(&j.spec, &sha),
        Err(ChildRefusal::PinnedInputs(_))
    ));
    assert_eq!(settle(&j, WaitStatus::Exited(0)).row, 36);
}

#[test]
fn unsealed_truncated_tampered_and_self_check_failures_accept_nothing() {
    type Mut = Box<dyn Fn(&Path)>;
    let dir = |j: &Job| j.layout.out_dir().join(symcomp::CHILD_DIR);
    let cases: Vec<(&str, Mut)> = vec![
        (
            "no result.json",
            Box::new(|d: &Path| std::fs::remove_file(d.join("result.json")).unwrap()),
        ),
        (
            "result.json torn",
            Box::new(|d: &Path| std::fs::write(d.join("result.json"), "{\"schema\":").unwrap()),
        ),
        (
            "table truncated",
            Box::new(|d: &Path| {
                let p = d.join("composite.table.jsonl");
                let b = std::fs::read(&p).unwrap();
                std::fs::write(&p, &b[..b.len() - 7]).unwrap();
            }),
        ),
        (
            "coefficient changed after the seal",
            Box::new(|d: &Path| {
                let p = d.join("composite.table.jsonl");
                let s = std::fs::read_to_string(&p)
                    .unwrap()
                    .replacen("\"1\"]]", "\"2\"]]", 1);
                std::fs::write(&p, s).unwrap();
            }),
        ),
        (
            "SHA256SUMS changed",
            Box::new(|d: &Path| std::fs::write(d.join("SHA256SUMS"), "x\n").unwrap()),
        ),
        (
            "a file missing",
            Box::new(|d: &Path| std::fs::remove_file(d.join("report.json")).unwrap()),
        ),
    ];
    for (what, m) in cases {
        let j = fresh("r22", &keys3());
        write_child(&j, &keys3(), &no_edit);
        m(&dir(&j));
        let c = settle(&j, WaitStatus::Exited(0));
        assert_eq!(
            (c.row, c.accept),
            (22, Accept::Nothing),
            "{what}: {}",
            c.why
        );
        assert!(!j.layout.results().exists(), "{what}");
        assert!(
            j.layout.sup_dir().join(symcomp::REFUSAL_FILE).exists(),
            "{what}"
        );
    }
    // consistent (re-sealed) children whose content contradicts itself: refused by the seal check
    let edits: Vec<(&str, Box<ChildEdit>)> = vec![
        (
            "self-check below shipped",
            Box::new(|_, _, r| r["independently_verified"] = json!(2)),
        ),
        (
            "relation count",
            Box::new(|_, _, r| r["shipped_relations"] = json!(2)),
        ),
        (
            "a relation dropped",
            Box::new(|t, v, _| {
                t.pop();
                v.pop();
            }),
        ),
        (
            "target of another key",
            Box::new(|t, _, _| {
                t[1] = t[1].replace("\"fixed\":[2,1,0,0]", "\"fixed\":[7,1,0,0]");
            }),
        ),
        ("keys count", Box::new(|_, _, r| r["keys"] = json!(4))),
        (
            "verdict",
            Box::new(|_, _, r| r["verdict"] = json!("PARTIAL")),
        ),
    ];
    for (what, e) in edits {
        let j = fresh("r22b", &keys3());
        write_child(&j, &keys3(), &*e);
        let c = settle(&j, WaitStatus::Exited(0));
        assert_eq!(
            (c.row, c.accept),
            (22, Accept::Nothing),
            "{what}: {}",
            c.why
        );
    }
    // keys of the child are not the keys input
    let j = fresh("r22c", &keys3());
    let mut other = keys3();
    other.swap(0, 2);
    write_child(&j, &other, &no_edit);
    assert_eq!(settle(&j, WaitStatus::Exited(0)).row, 22);
}

#[test]
fn a_consistently_resealed_forgery_passes_the_contract_but_a_tampered_results_stream_does_not() {
    // a forger who changes a coefficient BEFORE sealing produces a valid seal: the contract accepts the CANDIDATES
    // (row 33); only the import's verification can refuse the relation (supervisor fake test)
    let j = fresh("forge", &keys3());
    write_child(&j, &keys3(), &|t, _, _| {
        t[1] = t[1].replace("\"1\"]]", "\"5/7\"]]")
    });
    let c = settle(&j, WaitStatus::Exited(0));
    assert_eq!(
        (c.row, c.accept),
        (33, Accept::SuppliedRelations),
        "{}",
        c.why
    );
    // the wrapper's sealed stream changed after sealing (in place or re-sealed by hand): S false -> 22
    let p = j.layout.results();
    let s = std::fs::read_to_string(&p)
        .unwrap()
        .replacen("\"plain\"", "\"composite\"", 1);
    std::fs::write(&p, s).unwrap();
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Exited(0),
        KillReason::None,
        false,
    );
    assert_eq!(classify::classify(&ev).row, 22);
}

#[test]
fn tally_refuses_inconsistent_results_streams() {
    let j = fresh("tally", &mixed());
    write_child(&j, &mixed(), &no_edit);
    let seal = symcomp::check_child(&j.spec, &sha).unwrap();
    let lines = |recs: Vec<Value>, end: Value| -> Vec<Vec<u8>> {
        recs.iter()
            .chain([&end])
            .map(|r| r.to_string().into_bytes())
            .collect()
    };
    let recs = symcomp::results_records(&seal);
    let end = symcomp::end_record(&seal);
    symcomp::tally(&lines(recs.clone(), end.clone()), &j.spec).unwrap();
    let bad: Vec<(&str, Box<ResultsEdit>)> = vec![
        ("key out of order", Box::new(|r, _| r.swap(0, 1))),
        (
            "unknown outcome",
            Box::new(|r, _| r[0]["outcome"] = json!("maybe")),
        ),
        (
            "relation without key",
            Box::new(|r, _| r[1]["relation"] = json!(0)),
        ),
        (
            "relation target",
            Box::new(|r, _| r[5]["target"] = json!([9, 9, 9, 9])),
        ),
        (
            "file record missing",
            Box::new(|r, _| {
                r.pop();
            }),
        ),
        ("end records", Box::new(|_, e| e["records"] = json!(1))),
        (
            "end self-check",
            Box::new(|_, e| e["independently_verified"] = json!(0)),
        ),
        (
            "request of another job",
            Box::new(|_, e| e["request_blake3"] = json!(D1)),
        ),
        (
            "unknown record type",
            Box::new(|r, _| r.push(json!({"type": "rule", "index": 0}))),
        ),
    ];
    for (what, f) in bad {
        let (mut r, mut e) = (recs.clone(), end.clone());
        f(&mut r, &mut e);
        assert!(symcomp::tally(&lines(r, e), &j.spec).is_err(), "{what}");
    }
}

/// audit r2-sym F7 (lane symfix): the two self-check guards are each tested ALONE. `check_child` (the wrapper's check of
/// the child's own seal, symcomp.rs "the child's own re-check verified ...") refuses a result whose
/// `independently_verified` is below the shipped relations WITHOUT the tally; the tally's own guard (end record
/// `independently_verified`) is tested alone by `tally_refuses_inconsistent_results_streams` ("end self-check").
#[test]
fn check_child_alone_refuses_a_self_check_below_the_shipped_relations() {
    let j = fresh("selfcheck", &keys3());
    write_child(&j, &keys3(), &|_, _, r| r["independently_verified"] = json!(2));
    let e = symcomp::check_child(&j.spec, &sha)
        .err()
        .expect("check_child must refuse a child whose own re-check verified fewer relations than it shipped");
    assert!(format!("{e:?}").contains("re-check"), "{e:?}");
    // control: the honest child passes check_child
    let j = fresh("selfcheck-ok", &keys3());
    write_child(&j, &keys3(), &no_edit);
    symcomp::check_child(&j.spec, &sha).unwrap();
}

#[test]
fn tool_refusal_resource_failure_and_signals() {
    // exit 2 -> 41 (row 13, attention, no retry: deterministic)
    let j = fresh("r13", &keys3());
    std::fs::write(
        j.layout.stdout_log(),
        "{\"verdict\":\"ERROR\",\"error\":\"import load: frame\"}\n",
    )
    .unwrap();
    let c = settle(&j, WaitStatus::Exited(2));
    assert_eq!(
        (c.row, c.action),
        (13, Action::ItemAttentionNoRetry),
        "{}",
        c.why
    );
    // exit 2 naming a host resource errno -> 51 (row 15b: retry after back-off, then attention)
    let j = fresh("r15b", &keys3());
    std::fs::write(
        j.layout.stdout_log(),
        "{\"verdict\":\"ERROR\",\"error\":\"keys: Too many open files (os error 24)\"}\n",
    )
    .unwrap();
    let c = settle(&j, WaitStatus::Exited(2));
    assert_eq!(
        (c.row, c.sub.as_str(), c.action),
        (15, "b", Action::RetryAfterResourceBackoff),
        "{}",
        c.why
    );
    // killed (OOM / signal from outside): no OUTCOME, nothing accepted (rows 18 / 21)
    let j = fresh("r21", &keys3());
    write_child(&j, &keys3(), &no_edit);
    let w = symcomp::wrap(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Signaled(9),
        &sha,
        true,
    );
    assert_eq!(w, WaitStatus::Signaled(9));
    let ev = classify::assess(&j.layout, &j.spec, &j.digest, w, KillReason::None, false);
    assert_eq!(classify::classify(&ev).row, 21);
    let ev = classify::assess(&j.layout, &j.spec, &j.digest, w, KillReason::Memory, true);
    assert_eq!(classify::classify(&ev).row, 18);
    assert!(!j.layout.outcome().exists());
    // exit 10 / 20 of the child itself are not codes of the tool: nothing written, row 23
    let j = fresh("r23", &keys3());
    assert_eq!(settle(&j, WaitStatus::Exited(3)).row, 23);
}

#[test]
fn effective_digest_is_pinned_and_deterministic() {
    let opts = symcomp::SymmetryCompositeNodeOptions {
        request_sha256: "aa".repeat(32),
        node: Some("N".into()),
        import_base: "/".into(),
        limit: None,
        threads: 2,
        program_sha256: PROG_SHA.into(),
    };
    let e = symcomp::effective(&opts, true).unwrap();
    assert!(e.verify());
    let canonical = format!(
        r#"{{"args":[],"env":{{"SYMBOLICA_LICENSE":"{LICENCE_PRESENT}"}},"node_options":{{"import_base":"/","limit":null,"node":"N","program_sha256":"{PROG_SHA}","request_sha256":"{}","threads":2}},"order":null,"subcommand":"run"}}"#,
        "aa".repeat(32)
    );
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_EFFECTIVE);
    h.update(canonical.as_bytes());
    assert_eq!(e.blake3, h.finalize().to_hex().to_string());
    assert_eq!(
        e.blake3, "719fb2e6f0d3273a52656d871c054749308936509cc7668c51c1d6961fc78c2f",
        "pinned vector (recompute only for a deliberate change of the definition)"
    );
    assert_eq!(symcomp::effective(&opts, true).unwrap().blake3, e.blake3);
    let mut o2 = opts.clone();
    o2.limit = Some(3);
    assert_ne!(symcomp::effective(&o2, true).unwrap().blake3, e.blake3);
    assert_ne!(symcomp::effective(&opts, false).unwrap().blake3, e.blake3);
    // two attempts of the same request (inputs shared): equal effective digests and equal sealed results digests
    let t = TempDir::new("det");
    let (ins, req) = inputs_dir(&t, &mixed(), false);
    let base = t.path().to_path_buf();
    let j1 = job(t, ins.clone(), &req, 1);
    let j2 = {
        let t2 = TempDir(base.join("second"));
        std::fs::create_dir_all(&t2.0).unwrap();
        job(t2, ins, &req, 2)
    };
    let mut digests = vec![];
    for j in [&j1, &j2] {
        write_child(j, &mixed(), &no_edit);
        assert_eq!(settle(j, WaitStatus::Exited(0)).row, 34);
        let o = rustred_campaign_contract::outcome::Outcome::from_slice(
            &std::fs::read(j.layout.outcome()).unwrap(),
        )
        .unwrap();
        let x = o
            .job
            .outputs
            .extra
            .iter()
            .find(|x| x.role == "results")
            .unwrap()
            .clone();
        digests.push((o.job.effective.unwrap().blake3, x.canonical_blake3));
    }
    assert_eq!(digests[0], digests[1]);
}

#[test]
fn rows_33_to_36_are_in_the_table_with_their_columns() {
    let get = |r: u8| {
        ROWS.iter()
            .find(|x| x.row == r && x.sub.is_empty())
            .unwrap()
    };
    assert_eq!(
        (get(33).kind, get(33).accept, get(33).action),
        (
            OutcomeKind::Complete,
            Accept::SuppliedRelations,
            Action::ItemDone
        )
    );
    assert_eq!(
        (get(34).kind, get(34).accept, get(34).action),
        (
            OutcomeKind::RuleFound,
            Accept::SuppliedRelations,
            Action::QueueFailures
        )
    );
    assert_eq!(
        (get(35).kind, get(35).accept, get(35).action),
        (OutcomeKind::NoRule, Accept::Nothing, Action::QueueFailures)
    );
    assert_eq!(
        (get(36).kind, get(36).accept, get(36).action),
        (
            OutcomeKind::Refused,
            Accept::Nothing,
            Action::ItemAttentionNoRetry
        )
    );
    // audit r2-sym F6 (lane symfix): the rows BY NAME, not by count (other kinds add rows: the table grows): every
    // (row, variant) name is unique, and the rows whose condition names `kind symmetry-composite` are exactly 33-36
    let names: Vec<String> = ROWS.iter().map(|r| format!("{}{}", r.row, r.sub)).collect();
    let unique: std::collections::BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "a row name repeated: {names:?}");
    let ours: std::collections::BTreeSet<String> = ROWS
        .iter()
        .filter(|r| r.condition.contains("kind symmetry-composite"))
        .map(|r| format!("{}{}", r.row, r.sub))
        .collect();
    assert_eq!(
        ours,
        ["33", "34", "35", "36"]
            .iter()
            .map(|s| s.to_string())
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert!(classify::decision_table_markdown().contains("| supplied_relations |"));
    assert!(symcomp::names_pinned_mismatch("PINNED_INPUTS_MISMATCH: x"));
    assert!(!symcomp::names_pinned_mismatch(
        "NOT_PINNED_INPUTS_MISMATCHED"
    ));
}
