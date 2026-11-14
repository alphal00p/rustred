//! Lane repairflow (interfaces/repairflow.md): the repair flow phase under a REAL `rr_campaign` supervisor with
//! `rr_fake_child` emulating the generate jobs (step S) and the solve-recipe jobs (step R). Spawns processes and sends
//! signals: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh repairflow test -p rustred-campaign --test repairflow_fake`).
//!
//! TOY node (arity 3, chart 110: lines 0, 1, numerator 2; test data only), start set on four uncovered faces + one key
//! of a base table:
//! - face {0}x{}  : the symbolic attempt gives a small rule serving both open keys -> KEPT;
//! - face {1}x{}  : the symbolic rule is within budget but over the monomial cap -> REJECTED (numbers recorded) ->
//!   recipes: step d2 gives no relation, step d3 serves (escalation);
//! - face {}x{2}  : the symbolic search hits its wall budget -> recipes (step d2 serves);
//! - face {0}x{2} : refused symbolically, no recipe step and no default serves -> TERMINAL PROPOSAL (node stays open);
//! - the corner [1,1,0] is first met while closing (exception of the kept rule, outside every face recipe's domain):
//!   only the DEFAULT recipe serves it (R49).

mod closeloop_support;
mod sup_common;

use closeloop_support::{HEADER, blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

const F_KEPT: &str = "{0}x{}";
const F_BIG: &str = "{1}x{}";
const F_BUDGET: &str = "{}x{2}";
const F_NONE: &str = "{0}x{2}";

fn rule_rec(fixed: [Option<i64>; 3], target: &str, rhs: &[&str], exceptions: Value, monomials: u64) -> Value {
    json!({"type":"rule","chart":"110","index":0,"case":"toy","fixed":fixed,"equations":[],"target":target,
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":exceptions,"dispatch":"Partition","witness_loci":[],"monomials":monomials,"trace_rows":12,
        "recenter":[0,0,0]})
}

struct Files {
    config: PathBuf,
    policy: PathBuf,
}

fn material(dir: &Path, ms: u64) -> Files {
    std::fs::create_dir_all(dir).unwrap();
    // base table A: a point rule at [1,2,-1] (hand-downs only)
    let a_rule = json!({"type":"rule","chart":"110","index":0,"fixed":[1,2,-1],"equations":[],"target":"I(1,2,-1)",
        "rhs":[["I(0,2,-1)","1"],["I(1,0,-1)","1"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let a = dir.join("A.table.jsonl");
    std::fs::write(
        &a,
        format!(
            "{HEADER}\n{}\n{a_rule}\n{}\n",
            r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#,
            r#"{"type":"end","charts":1}"#
        ),
    )
    .unwrap();
    let start = [
        [3, 1, 0],
        [4, 1, 0],
        [1, 3, 0],
        [1, 4, 0],
        [1, 1, -1],
        [1, 1, -2],
        [2, 1, -1],
        [1, 2, -1],
    ];
    let st = dir.join("start.jsonl");
    std::fs::write(&st, start.iter().map(|k| format!("{}\n", json!(k))).collect::<String>()).unwrap();
    let dflt = dir.join("default.recipe.json");
    std::fs::write(
        &dflt,
        json!({"schema":"rustred.recipe.v1","id":"default","arity":3,"chart":"110",
            "domain":{"fixed":[null,null,null],"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1","S2"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"K"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a)}]});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    // solve oracle (step R): face {1}x{} only at step d3; face {}x{2} at d2; the corner by the default recipe
    let mut oracle: Vec<Value> = vec![];
    for k in 2..=4 {
        oracle.push(json!({"source":"rf:d1_n:d3_clast_fon_mM1C","key":[1,k,0],"verdict":"SOLVED",
            "rhs":[[1,k-1,0],[0,k,0]]}));
    }
    oracle.push(json!({"source":"rf:d_n2:d2_clast_fon_mM1C","key":[1,1,-2],"verdict":"SOLVED","rhs":[[1,1,-1],[0,1,-2]]}));
    oracle.push(json!({"source":"rf:d_n2:d2_clast_fon_mM1C","key":[1,1,-1],"verdict":"SOLVED","rhs":[[1,1,0],[0,1,-1]]}));
    oracle.push(json!({"source":"default-recipe","key":[1,1,0],"verdict":"SOLVED","rhs":[[0,1,0],[1,0,0]]}));
    let op = dir.join("oracle.jsonl");
    std::fs::write(&op, oracle.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let family = dir.join("TOY.family");
    std::fs::write(&family, "toy family (the fake child never reads it)\n").unwrap();
    let config = dir.join("closeloop.json");
    let cfg = json!({"schema":"rustred.campaign.closeloop.v1","nodes":[{
        "node":"toy", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": dflt.display().to_string(), "exact_tier": false,
        "budgets": {"max_rounds": 20, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "toy-order",
                  "relation_rule": "first_admissible", "max_keys_per_job": 2, "threads": 1,
                  "cap_bytes": 268435456, "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": op.display().to_string(), "ms": ms, "program": "rr_fake_child"}}}]});
    std::fs::write(&config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    // the repair policy (+ the test-only fake generate answers per face)
    let kept = rule_rec([None, Some(1), Some(0)], "I(n0,1,0)", &["I(n0-1,1,0)", "I(n0,0,0)"], json!([["-1+n0"]]), 40);
    let big = rule_rec([Some(1), None, Some(0)], "I(1,n1,0)", &["I(1,n1-1,0)", "I(0,n1,0)"], json!([["-1+n1"]]), 5_000_000);
    let policy = dir.join("repairflow.json");
    std::fs::write(
        &policy,
        serde_json::to_vec_pretty(&json!({"schema":"rustred.campaign.repairflow.v1",
            "repair_policy":{"symbolic":{"heartbeat_s":1,"stop_grace_s":3,"cap_bytes":268435456,
                                         "expected_peak_bytes":67108864},
                             "recipes":{"escalation":["d2_clast_fon_mM1C","d3_clast_fon_mM1C"]}},
            "nodes":{},
            "fake":{"program":"rr_fake_child","ms":ms,"faces":{
                F_KEPT: {"rule": kept}, F_BIG: {"rule": big},
                F_BUDGET: {"stop":"budget_wall"}, F_NONE: {"stop":"open_obligations"}}}}))
        .unwrap(),
    )
    .unwrap();
    Files { config, policy }
}

fn setup(tag: &str, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let f = material(&t.path.join("material"), ms);
    let spec = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": f.config.display().to_string(),
                    "blake3": blake3_file(&f.config)},
                   {"id": "repairflow", "role": "repairflow", "path": f.policy.display().to_string(),
                    "blake3": blake3_file(&f.policy)}]
    });
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn face<'a>(d: &'a Value, label: &str) -> &'a Value {
    d["faces"].as_array().unwrap().iter().find(|f| f["face"] == label).unwrap_or_else(|| panic!("face {label}: {d:#}"))
}

#[test]
fn the_flow_decides_per_face_and_exports_recipes() {
    let _g = serial();
    let (_t, sp, cdir) = setup("rfflow", 20);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let d = read_json(&cdir.join("closeloop/toy/repairflow/decisions.json"));
    // the cheap small rule is kept
    let k = face(&d, F_KEPT);
    assert_eq!(k["symbolic"]["class"]["class"], "keep", "{k:#}");
    assert!(k["kept"].is_string());
    assert_eq!(k["served_keys"].as_array().unwrap().len(), 2);
    // within budget but over the size cap: rejected with the numbers that decided it
    let b = face(&d, F_BIG);
    assert_eq!(b["symbolic"]["class"], json!({"class":"over_cap","field":"monomials","value":5000000,"cap":200000}), "{b:#}");
    assert!(b["kept"].is_null());
    // the budget stop and the refusal: no candidate, recorded with the job's outcome
    let w = face(&d, F_BUDGET);
    assert_eq!(w["symbolic"]["class"]["class"], "no_candidate", "{w:#}");
    let n = face(&d, F_NONE);
    assert_eq!(n["symbolic"]["class"]["class"], "no_candidate", "{n:#}");
    assert_eq!(d["kept_rules"], 1);
    // the composed spec carries the re-homed table and the face recipes before the default recipe
    let comp = read_json(&cdir.join("closeloop/toy/repairflow/import-spec.composed.json"));
    let ids: Vec<String> = comp["sources"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect();
    let pos = |id: &str| ids.iter().position(|x| x == id).unwrap_or_else(|| panic!("{id} not in {ids:?}"));
    assert!(pos("A") < pos("rf:symbolic") && pos("rf:symbolic") < pos("rf:d1_n:d2_clast_fon_mM1C"));
    assert!(pos("rf:d1_n:d2_clast_fon_mM1C") < pos("rf:d1_n:d3_clast_fon_mM1C"));
    assert!(pos("rf:d0_n2:d3_clast_fon_mM1C") < pos("default-recipe"));
    // the closing loop solved with the escalation, the default recipe served the corner
    let rep = read_json(&cdir.join("closeloop/toy/repairflow/report.json"));
    let keys = &rep["keys"];
    assert!(keys["symbolic"].as_u64().unwrap() >= 3, "{rep:#}");
    assert!(keys["face_recipe"].as_u64().unwrap() >= 5, "{rep:#}");
    assert_eq!(keys["default_recipe"], 1, "{rep:#}");
    // the key nothing serves is a TYPED terminal proposal, never accepted
    let props = rep["terminal_proposals"].as_array().unwrap();
    assert_eq!(props.len(), 1, "{rep:#}");
    assert_eq!(props[0]["key"], json!([2, 1, -1]));
    assert_eq!(props[0]["face"], F_NONE);
    assert_eq!(rep["closeloop_final"]["verdict"], "open");
    // the close-out by declared proposals is refused here (keys-only tier, unproved recipe relations: other blockers),
    // the withheld open declaration is released: the node ends OPEN, never stalls (pure close-out rule: test below)
    assert_eq!(rep["closeout"]["closed_by_declared_proposals"], false, "{rep:#}");
    assert!(rep["closeout"]["why"].as_str().unwrap().contains("tier_k_superset"), "{rep:#}");
    assert_eq!(state(&cdir).nodes["toy"].state.phase, rustred_campaign::state::NodePhase::Open);
    // the self-contained export manifest: escalation per face in order, default sequence, runtime budgets
    let man = read_json(&cdir.join("closeloop/toy/repairflow/export/manifest.json"));
    let fb = man["faces"].as_array().unwrap().iter().find(|f| f["face"] == F_BIG).unwrap();
    let steps: Vec<&str> = fb["escalation"].as_array().unwrap().iter().map(|e| e["step"].as_str().unwrap()).collect();
    assert_eq!(steps, vec!["d2_clast_fon_mM1C", "d3_clast_fon_mM1C"]);
    assert_eq!(fb["escalation"][1]["certified_keys"], 3);
    assert_eq!(man["default_sequence"][0]["certified_keys"], 1);
    assert_eq!(man["runtime"]["max_solves_per_input"], 10000);
    assert!(man["descent"].as_str().unwrap().contains("STRICTLY LOWER"));
    // the export lists the recipe files used (face {1}x{} by step d3 only, face {}x{2} by d2) with digests
    let used: Vec<String> = rep["recipes_used"].as_array().unwrap().iter().map(|r| r["source"].as_str().unwrap().to_string()).collect();
    assert!(used.contains(&"rf:d1_n:d3_clast_fon_mM1C".to_string()), "{used:?}");
    assert!(used.contains(&"rf:d_n2:d2_clast_fon_mM1C".to_string()), "{used:?}");
    assert!(!used.contains(&"rf:d1_n:d2_clast_fon_mM1C".to_string()), "{used:?}");
    let sums = std::fs::read_to_string(cdir.join("closeloop/toy/repairflow/export/SHA256SUMS")).unwrap();
    assert!(sums.contains("recipes/d1_n.d3_clast_fon_mM1C.json") && sums.contains("symbolic.table.jsonl"), "{sums}");
    // the re-homed table: frame header first, the kept record with `type` first and index 0
    let tab = std::fs::read_to_string(cdir.join("closeloop/toy/repairflow/export/symbolic.table.jsonl")).unwrap();
    let mut it = tab.lines();
    assert_eq!(it.next().unwrap(), HEADER);
    assert!(it.next().unwrap().starts_with(r#"{"type":"rule","index":0,"#));
    // the effective policy is recorded (ledger input + report)
    assert_eq!(rep["policy"]["recipes"]["escalation"], json!(["d2_clast_fon_mM1C", "d3_clast_fon_mM1C"]));
    assert!(state(&cdir).inputs.contains_key("repairflow:toy:policy"));
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn closeout_rule_declares_proposals_only_when_nothing_else_blocks() {
    use rustred_campaign::closeloop::repair::closeout_check;
    let p = rustred_campaign_core::repairflow::policy::ProposalPolicy::default();
    let fin = |blockers: Value, closure: u64, cert: u64| {
        json!({"verdict": "open", "blockers": blockers, "keys": {"closure": closure, "certified": cert}})
    };
    assert!(closeout_check(&p, &fin(json!(["open_keys:2", "import_open_keys:2"]), 10, 8), 2).0);
    assert!(!closeout_check(&p, &fin(json!(["import_open_keys:2", "relation_proofs_unbound:1"]), 10, 8), 2).0);
    assert!(!closeout_check(&p, &fin(json!(["import_open_keys:2"]), 10, 7), 2).0, "a key below evidence");
    let small = rustred_campaign_core::repairflow::policy::ProposalPolicy { max_terminals: 1, ..p.clone() };
    assert!(!closeout_check(&small, &fin(json!(["import_open_keys:2"]), 10, 8), 2).0);
    let off = rustred_campaign_core::repairflow::policy::ProposalPolicy { declare_terminal: false, ..p.clone() };
    assert!(!closeout_check(&off, &fin(json!(["import_open_keys:2"]), 10, 8), 2).0);
    assert!(!closeout_check(&p, &json!({"verdict": "budget", "blockers": [], "keys": {}}), 2).0);
}

/// Decisions, report decisions and the statement of a finished run (what resume must reproduce).
fn outcome(cdir: &Path) -> (Vec<u8>, Value, Vec<u8>, Vec<(String, String)>) {
    let d = std::fs::read(cdir.join("closeloop/toy/repairflow/decisions.json")).unwrap();
    let rep = read_json(&cdir.join("closeloop/toy/repairflow/report.json"));
    let stmt = std::fs::read(cdir.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    let st = state(cdir);
    let items = st
        .items
        .iter()
        .filter(|(_, it)| it.kind == "repair-symbolic")
        .map(|(k, it)| (k.clone(), it.phase.tag().to_string()))
        .collect();
    (d, json!([rep["keys"], rep["terminal_proposals"], rep["recipes_used"]]), stmt, items)
}

#[test]
fn interrupt_and_resume_gives_the_same_decisions() {
    let _g = serial();
    let (_ta, spa, ca) = setup("rfref", 150);
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0), "{}", log_tail(&ca));
    let reference = outcome(&ca);
    for seed in [5u64, 23] {
        let (_tb, spb, cb) = setup(&format!("rfint{seed}"), 150);
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..12 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 4 {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(300 + x % 1500));
                signal(&c, libc::SIGTERM);
            }
            let code = wait(&mut c, Duration::from_secs(300));
            codes.push(code);
            if code == 0 {
                break;
            }
        }
        assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
        assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
        let o = outcome(&cb);
        assert!(o.0 == reference.0, "seed {seed}: decisions differ");
        assert_eq!(o.1, reference.1, "seed {seed}");
        assert!(o.2 == reference.2, "seed {seed}: statements differ");
        assert_eq!(o.3, reference.3, "seed {seed}");
        assert_eq!(verify(&cb), 0);
    }
}
