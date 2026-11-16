//! Lane wireflow (e), interfaces/repairflow.md 24: coreseed's BUDGETED VARIANT PHASE after close-first, at P5, under a
//! REAL `rr_campaign` supervisor with `rr_fake_child` (TOY node of tests/wireflow_seedmin_fake.rs, test data only): the
//! closing loop closes face {1}x{} with the FIRST step `mixed_shell` (close-first); the variant phase then solves the
//! face's other step `mixed_shell@up` at exactly the certified keys through the child solver (fake oracle: the up
//! centre gives a 1-term relation, the key alone 2 terms), ranks per key (pole objective, then terms; the closing step
//! wins ties) and SHIPS the variant preferred at every key. Without the step nothing changes.
//! Spawns processes: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh wireflow test -p rustred-campaign --test wireflow_variants_fake`).

mod closeloop_support;
mod sup_common;

use closeloop_support::{HEADER, blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

const SRC: &str = "rf:d1_n:mixed_shell";

fn material(dir: &Path, seedmin: Option<Vec<i32>>, ms: u64) -> (PathBuf, PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
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
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[1,3,0]\n[1,4,0]\n[1,5,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"K"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a)}]});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    // the closing loop's solve oracle: the face recipe serves every face key (rhs = the next lower key + a corner)
    let mut oracle = String::new();
    for k in 2..=5 {
        oracle.push_str(&format!(
            "{}\n",
            json!({"source": SRC, "key": [1, k, 0], "verdict": "SOLVED", "rhs": [[1, k - 1, 0], [0, k, 0]]})
        ));
    }
    oracle.push_str(&format!(
        "{}\n",
        json!({"source": SRC, "key": [1, 1, 0], "verdict": "SOLVED", "rhs": [[0, 1, 0], [1, 0, 0]]})
    ));
    let op = dir.join("oracle.jsonl");
    std::fs::write(&op, oracle).unwrap();
    let family = dir.join("TOY.family");
    std::fs::write(&family, "toy family (the fake child never reads it)\n").unwrap();
    let config = dir.join("closeloop.json");
    let cfg = json!({"schema":"rustred.campaign.closeloop.v1","nodes":[{
        "node":"toy", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": null, "exact_tier": false,
        "budgets": {"max_rounds": 20, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(),
                  "order": {"name":"o","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"},
                  "relation_rule": "first_admissible", "max_keys_per_job": 4, "threads": 1,
                  "cap_bytes": 268435456, "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": op.display().to_string(), "ms": 5, "program": "rr_fake_child"}}}]});
    std::fs::write(&config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    // the child solver's oracle: the up centre [0,1,0] gives a 1-term relation, the key alone a 2-term one
    let up = seedmin.clone().unwrap_or_default();
    let alts: Vec<Value> = (2..=5)
        .map(|k| {
            json!([[1, k, 0], [{"needs": [up.clone()], "rhs": [[1, k - 1, 0]]},
                                    {"needs": [[0, 0, 0]], "rhs": [[1, k - 1, 0], [0, k, 0]]}]])
        })
        .collect();
    let mut rp = json!({"symbolic": {"enabled": false}, "shadow_test": false,
        "route0": {"symmetry_credit": false, "symmetry_relation": false},
        "recipes": {"escalation": ["mixed_shell", "mixed_shell@up"]},
        "proposals": {"declare_terminal": false}});
    if seedmin.is_some() {
        rp["variants"] = json!({"enabled": true, "jobs": 2, "wall_cap_s": 120});
    }
    let policy = dir.join("repairflow.json");
    std::fs::write(
        &policy,
        serde_json::to_vec_pretty(&json!({"schema":"rustred.campaign.repairflow.v1", "repair_policy": rp,
            "nodes": {}, "fake": {"program": "rr_fake_child", "ms": ms, "faces": {}, "seedmin_oracle": alts}}))
        .unwrap(),
    )
    .unwrap();
    (config, policy)
}

fn setup(tag: &str, seedmin: Option<Vec<i32>>, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let (config, policy) = material(&t.path.join("material"), seedmin, ms);
    let spec = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": config.display().to_string(),
                    "blake3": blake3_file(&config)},
                   {"id": "repairflow", "role": "repairflow", "path": policy.display().to_string(),
                    "blake3": blake3_file(&policy)}]
    });
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

#[test]
fn the_variant_phase_tries_the_other_steps_at_the_certified_keys_and_ships_the_preferred_one() {
    let _g = serial();
    let (_t0, sp0, c0) = setup("wfvar0", None, 5);
    let codes = run_to_end(&sp0, &c0, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&c0)
    );
    // a centre only the variant step has (from the composed spec of the run without the phase): the fake oracle gives
    // the 1-term relation only to a list holding it
    let spec = read_json(&c0.join("closeloop/toy/repairflow/import-spec.composed.json"));
    let centres = |id: &str| -> Vec<Vec<i32>> {
        let p = spec["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .unwrap()["path"]
            .as_str()
            .unwrap()
            .to_string();
        serde_json::from_value(read_json(Path::new(&p))["seeds"]["centres"].clone()).unwrap()
    };
    let (plain, up) = (centres(SRC), centres("rf:d1_n:mixed_shell@up"));
    let only_up = up
        .iter()
        .find(|c| !plain.contains(c))
        .cloned()
        .expect("an up-only centre");
    let (_t1, sp1, c1) = setup("wfvar1", Some(only_up), 5);
    let codes = run_to_end(&sp1, &c1, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&c1)
    );
    // the closing itself is unchanged
    // (the two campaigns have their own material directories: rows compared by key, status and serving source)
    let rows = |c: &Path| -> Vec<(Value, Value, Value)> {
        std::fs::read_to_string(c.join("closeloop/toy/final/statement/statement.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .map(|v| {
                (
                    v["key"].clone(),
                    v["status"].clone(),
                    v["server"]["source"].clone(),
                )
            })
            .collect()
    };
    assert_eq!(rows(&c0), rows(&c1));
    assert!(!state(&c0).inputs.contains_key("repairflow:toy:variants"));
    assert!(
        state(&c1).inputs.contains_key("repairflow:toy:variants"),
        "{}",
        log_tail(&c1)
    );
    let rec = read_json(&c1.join("closeloop/toy/repairflow/variants/variants.json"));
    eprintln!("variants record: {rec:#}");
    let f = rec["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["closing"] == SRC)
        .unwrap_or_else(|| panic!("{rec:#}"))
        .clone();
    assert_eq!(
        f["tried"],
        json!(["mixed_shell", "mixed_shell@up"]),
        "{f:#}"
    );
    assert!(f["keys"].as_u64().unwrap() >= 3, "{f:#}");
    for k in f["per_key"].as_array().unwrap() {
        assert_eq!(k["alternatives"].as_array().unwrap().len(), 2, "{k:#}");
    }
    assert_eq!(f["improved_keys"], f["keys"], "{f:#}");
    assert_eq!(f["shipped"]["step"], "mixed_shell@up", "{f:#}");
    let man = read_json(&c1.join("closeloop/toy/repairflow/export/manifest.json"));
    let line = man["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["closing"] == SRC)
        .unwrap()
        .clone();
    let r = read_json(
        &c1.join("closeloop/toy/repairflow/export")
            .join(line["file"].as_str().unwrap()),
    );
    assert!(
        r["id"].as_str().unwrap().ends_with("mixed_shell@up"),
        "{r:#}"
    );
    assert_eq!(rec["requests_used"], 2);
    assert_eq!(verify(&c1), 0);
}
