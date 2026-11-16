//! Lane wireflow (f), interfaces/repairflow.md 21: SEED MINIMISATION of the closing face recipes at P5 under a REAL
//! `rr_campaign` supervisor with `rr_fake_child` (TOY node of tests/repairflow_fake.rs, test data only): face {1}x{} is
//! closed by its `mixed_shell` face recipe through the closing loop's own solve jobs (fake oracle by recipe source and
//! key); then the policy's `seedmin` step minimises that recipe's explicit seed list JOINTLY over the keys it certified
//! (fake solve oracle: every key needs the key itself and line 1 one step down) and the minimised recipe is SHIPPED in the
//! export (record compared with the full list). Spawns processes: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh wireflow test -p rustred-campaign --test wireflow_seedmin_fake`).

mod closeloop_support;
mod sup_common;

use closeloop_support::{HEADER, blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

const SRC: &str = "rf:d1_n:mixed_shell";

fn material(dir: &Path, seedmin: bool, ms: u64) -> (PathBuf, PathBuf) {
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
    // the seed-minimisation oracle: every face key needs the key and line 1 one step down (offsets from the key)
    let alts: Vec<Value> = (2..=5)
        .map(|k| json!([[1, k, 0], [{"needs": [[0, 0, 0], [0, -1, 0]], "rhs": [[1, k - 1, 0], [0, k, 0]]}]]))
        .collect();
    let mut rp = json!({"symbolic": {"enabled": false}, "shadow_test": false,
        "route0": {"symmetry_credit": false, "symmetry_relation": false},
        "recipes": {"escalation": ["mixed_shell"]},
        "proposals": {"declare_terminal": false}});
    if seedmin {
        rp["seedmin"] = json!({"enabled": true, "jobs": 2, "wall_cap_s": 120,
            "policy": {"parallel_trials": 2}});
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

fn setup(tag: &str, seedmin: bool, ms: u64) -> (Tmp, PathBuf, PathBuf) {
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
fn the_closing_face_recipe_is_minimised_and_the_minimised_list_is_shipped() {
    let _g = serial();
    let (_t, sp, cdir) = setup("wfsm", true, 5);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cdir)
    );
    let st = state(&cdir);
    assert!(
        st.inputs.contains_key("repairflow:toy:seedmin"),
        "{}",
        log_tail(&cdir)
    );
    let sm = read_json(&cdir.join("closeloop/toy/repairflow/seedmin/seedmin.json"));
    let e = sm["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["source"] == SRC)
        .unwrap_or_else(|| panic!("{sm:#}"))
        .clone();
    assert!(e["keys"].as_u64().unwrap() >= 3, "{e:#}");
    assert_eq!(e["shipped"], "minimised", "{e:#}");
    assert_eq!(e["min_seeds"], 2, "{e:#}");
    assert!(e["full_seeds"].as_u64().unwrap() > 2, "{e:#}");
    // the export ships the minimised recipe (same domain, the two needed offsets) and lists it in the manifest
    let man = read_json(&cdir.join("closeloop/toy/repairflow/export/manifest.json"));
    let line = man["seedmin"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["source"] == SRC)
        .unwrap()
        .clone();
    let file = line["file"].as_str().unwrap();
    let r = read_json(&cdir.join("closeloop/toy/repairflow/export").join(file));
    let mut centres: Vec<Vec<i32>> = serde_json::from_value(r["seeds"]["centres"].clone()).unwrap();
    centres.sort();
    assert_eq!(centres, vec![vec![0, -1, 0], vec![0, 0, 0]]);
    assert!(r["id"].as_str().unwrap().ends_with("/min"));
    let rep = read_json(&cdir.join("closeloop/toy/repairflow/report.json"));
    assert!(rep["seedmin"].is_array(), "{rep:#}");
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn without_the_step_the_export_is_unchanged() {
    let _g = serial();
    let (_t, sp, cdir) = setup("wfsm0", false, 5);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert!(matches!(codes.last(), Some(&0) | Some(&10)), "{codes:?}");
    assert!(!state(&cdir).inputs.contains_key("repairflow:toy:seedmin"));
    assert!(!cdir.join("closeloop/toy/repairflow/seedmin").exists());
    let man = read_json(&cdir.join("closeloop/toy/repairflow/export/manifest.json"));
    assert!(man.get("seedmin").is_none());
    let rep = read_json(&cdir.join("closeloop/toy/repairflow/report.json"));
    assert!(rep.get("seedmin").is_none());
}

#[test]
fn an_interrupted_minimisation_resumes_to_the_same_record() {
    let _g = serial();
    let (_ta, spa, ca) = setup("wfsmref", true, 200);
    let codes = run_to_end(&spa, &ca, &["--plain"]);
    assert!(matches!(codes.last(), Some(&0) | Some(&10)), "{codes:?}");
    let reference =
        std::fs::read(ca.join("closeloop/toy/repairflow/seedmin/seedmin.json")).unwrap();
    let (_tb, spb, cb) = setup("wfsmint", true, 200);
    let mut x = 11u64;
    let mut codes = vec![];
    for round in 0..16 {
        let mut c = start(&spb, &cb, &["--plain"], &[]);
        if round < 6 {
            x = mix(x);
            std::thread::sleep(std::time::Duration::from_millis(1500 + x % 3000));
            signal(&c, libc::SIGTERM);
        }
        let code = wait(&mut c, std::time::Duration::from_secs(900));
        codes.push(code);
        if code == 0 || code == 10 {
            break;
        }
    }
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cb)
    );
    assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
    let got = std::fs::read(cb.join("closeloop/toy/repairflow/seedmin/seedmin.json")).unwrap();
    let norm = |b: &[u8], d: &Path| {
        String::from_utf8_lossy(b).replace(&d.parent().unwrap().display().to_string(), "<ROOT>")
    };
    assert_eq!(norm(&got, &cb), norm(&reference, &ca));
    assert_eq!(verify(&cb), 0);
}
