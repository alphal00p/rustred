//! Lane shipecon: the ship-economy driver end to end with FAKE children (`rr_fake_child` emulating kind solve-recipe)
//! through the REAL launcher, on a toy node (arity 3, mask 110; test data only): census, candidacy (examined / kept
//! aside), the guard (one solve job, the closeloop accept chain), the decision, the closing check of the shipped spec
//! (the import emulating the engine order), apply mode, and the iteration rule (a second run with the same settings
//! launches nothing and decides the same). Spawns processes: run ONLY under `LIGHT_PIDNS=1` (private pid namespace,
//! rule L10): `LIGHT_PIDNS=1 light_build.sh shipecon test -p rustred-campaign --test shipecon_fake`
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "pf19_support/mod.rs"]
mod pf19;
use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::shipecon::{ShipFake, ShipRunConfig, run_ship};
use rustred_campaign_core::economy::ship::ShipPolicy;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
const STEP: &str = "d2_clast_fon";

/// A NEW directory under the (private, per-invocation) TMPDIR.
fn root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("rr-shipecon-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}

fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[&str], extra: Value) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    let mut v = json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v.to_string()
}

/// The toy material: rule 0 = a heavy point rule at I(2,1,0); rule 1 = the n0 descent on n1 = 1, n2 = 0 except at
/// n0 = 2 (so no LATER rule takes I(2,1,0)); rule 2 = a heavy point rule no certified key reaches.
fn material(dir: &Path) -> PathBuf {
    let pad = "x".repeat(4000);
    let lines = [
        r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#.to_string(),
        r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#.to_string(),
        rule(
            0,
            [Some(2), Some(1), Some(0)],
            &["I(1,1,0)", "I(2,0,0)"],
            json!({"origin": pad}),
        ),
        rule(
            1,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            json!({"exceptions": [["n0-2"]]}),
        ),
        rule(2, [Some(9), Some(9), Some(0)], &["I(8,9,0)"], json!({"origin": pad})),
        r#"{"type":"end","charts":1}"#.to_string(),
    ];
    let t = dir.join("A.table.jsonl");
    std::fs::write(&t, lines.join("\n") + "\n").unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[3,1,0]\n[2,1,0]\n").unwrap();
    let sha = rustred_campaign_core::import::lines::file_sha256(&t).unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":t.display().to_string(),"sha256": sha}]});
    let p = dir.join("spec.json");
    std::fs::write(&p, spec.to_string()).unwrap();
    p
}

fn config(dir: &Path, out: &str, previous: Option<PathBuf>, fake: ShipFake) -> ShipRunConfig {
    let family = dir.join("family.sym");
    std::fs::write(&family, "fake family\n").unwrap();
    let mut program =
        Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract).unwrap();
    program.features_required = vec!["fake".into()];
    let policy = ShipPolicy::parse(
        &json!({"schema": "rustred.campaign.shipecon.v1", "mode": "apply",
            "candidates": {"min_rule_bytes": 1000},
            "bars": {"min_bytes_saved": 100},
            "recipe": {"steps": [STEP], "relation_rule": "first_admissible"}})
        .to_string(),
    )
    .unwrap();
    ShipRunConfig {
        node: "toy".into(),
        import_spec: material(dir),
        import_base: dir.to_path_buf(),
        statement_rows: None,
        policy,
        program,
        family,
        order: r#"{"name":"o","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}"#
            .into(),
        parallel: 2,
        threads_per_job: 1,
        cap_bytes: 256 << 20,
        wall_cap_s: 60,
        work_root: dir.join(out).join("work"),
        out_dir: dir.join(out).join("out"),
        check_threads: 1,
        licence_env: None,
        previous,
        table_times: None,
        engine_probe_sample: 10,
        engine_first_match: None,
        statement_format: "tool".into(),
        rule_index: None,
        check_mode: "full".into(),
        later_rule_probe: None,
        final_probe: None,
        resume_trials: None,
        previous_solved: None,
        previous_work: None,
        fake: Some(fake),
        verbose: false,
    }
}

#[test]
fn the_ship_economy_migrates_a_heavy_point_rule_and_a_rerun_changes_nothing() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10): LIGHT_PIDNS=1"
    );
    let dir = root("toy");
    let mut answers = BTreeMap::new();
    answers.insert(
        STEP.to_string(),
        BTreeMap::from([(vec![2, 1, 0], vec![vec![1, 1, 0]])]),
    );
    let fake = ShipFake { answers, ms: 1 };
    let cfg = config(&dir, "r1", None, fake.clone());
    let rep = run_ship(&cfg).unwrap();
    // census: 3 rules; rule 2 serves no certified key, rule 0 is examined, rule 1 is below min_rule_bytes
    assert_eq!(rep["census"]["rules"], 3, "{}", rep["census"]);
    assert_eq!(rep["examined"], 1);
    let dec: Vec<&Value> = rep["decisions"].as_array().unwrap().iter().collect();
    let by = |l: i64| {
        dec.iter()
            .find(|d| d["rule"]["label"] == l)
            .unwrap_or_else(|| panic!("rule {l}: {}", rep["decisions"]))
    };
    assert_eq!(by(2)["outcome"], "kept_aside");
    let o0 = by(0)["outcome"].as_str().unwrap().to_string();
    assert!(
        o0 == "stored_relations" || o0 == "recipe",
        "rule 0: {}",
        by(0)
    );
    assert_eq!(by(0)["guard_holds"], true);
    // the closing check of the shipped spec: same start, same terminals, no new open key
    let cc = &rep["closing_check"];
    assert_eq!(cc["holds"], true, "{cc}");
    assert_eq!(cc["same_start"], true);
    assert_eq!(cc["new_open_keys"], 0);
    assert!(
        rep["totals"]["bytes_saved"].as_i64().unwrap() > 4000,
        "{}",
        rep["totals"]
    );
    // coverage never reduced, key by key: every key the original serves is served by the shipped artifact
    let last = rep["closing_iterations"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last["served_set"]["holds"], true, "{last}");
    assert_eq!(last["served_set"]["lost"], 0);
    assert_eq!(last["served_set"]["original"], 3, "{last}");
    assert_eq!(last["served_set"]["dropped_in_start"], 0);
    // apply: the artifact names the removed rules
    assert_eq!(rep["applied"]["removed"], 2, "{}", rep["applied"]);
    let out = dir.join("r1").join("out");
    assert!(out.join("ship/SHA256SUMS").is_file());
    // the per-node service manifest (plan/ARTIFACT_MANIFEST.md)
    let rel = std::fs::read_dir(out.join("ship/relations"))
        .unwrap()
        .flatten()
        .next()
        .unwrap()
        .path();
    let m: Value =
        serde_json::from_slice(&std::fs::read(rel.join("MANIFEST.json")).unwrap()).unwrap();
    assert_eq!(m["schema"], "rustred.node-service-manifest.v1");
    assert_eq!(m["table"]["removed"].as_array().unwrap().len(), 2, "{m}");
    assert_eq!(m["entries"]["precedence"], "after_table");
    assert!(rel.join("index.jsonl").is_file());
    let records = out.join("records.jsonl");
    assert!(out.join("trials.jsonl").is_file());
    // the iteration rule: same settings, the previous records -> nothing to solve, the same decisions
    let cfg2 = config(&dir, "r2", Some(records), fake);
    let rep2 = run_ship(&cfg2).unwrap();
    assert_eq!(rep2["unchanged_since_previous"], 1);
    let jobs = std::fs::read_to_string(dir.join("r2").join("out").join("jobs.jsonl")).unwrap();
    assert!(
        jobs.trim().is_empty(),
        "jobs launched on an unchanged rerun: {jobs}"
    );
    assert_eq!(rep2["totals"], rep["totals"]);
    assert_eq!(rep2["closing_check"]["holds"], true);
}

/// procfix7 round 19 (the coordinator's landing condition): the ship economy when its job launcher cannot hold its
/// (ad-hoc) id: every trial job is refused (named), no rule is migrated by a job, nothing is saved.
#[test]
fn pf19_a_refused_launcher_migrates_nothing() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: builds launchers: only inside a private pid namespace (rule L10)");
        return;
    }
    if let Some(v) = pf19::in_fresh_process("pf19_a_refused_launcher_migrates_nothing") {
        assert!(v.starts_with("PASS"), "{v}");
        return;
    }
    let _held = pf19::hold_own_adhoc_lock();
    let dir = root("pf19");
    let mut answers = BTreeMap::new();
    answers.insert(
        STEP.to_string(),
        BTreeMap::from([(vec![2, 1, 0], vec![vec![1, 1, 0]])]),
    );
    let cfg = config(&dir, "r1", None, ShipFake { answers, ms: 1 });
    let rep = run_ship(&cfg).unwrap();
    let j = rep.to_string();
    eprintln!("PF19E {j}");
    // rule 0 (the one the trial job would migrate): its guard fails with the refusal named; no job-decided outcome
    let r0 = rep["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["rule"]["label"] == 0)
        .unwrap_or_else(|| panic!("rule 0: {}", rep["decisions"]))
        .clone();
    assert_eq!(r0["guard_holds"], false, "{r0}");
    let o0 = r0["outcome"].as_str().unwrap_or("");
    assert!(o0 != "stored_relations" && o0 != "recipe", "{r0}");
    let why = r0["why"].as_str().unwrap_or("").to_string();
    assert!(why.contains("launcher refused"), "{r0}");
    let detail = why;
    pf19::passed(&detail);
}
