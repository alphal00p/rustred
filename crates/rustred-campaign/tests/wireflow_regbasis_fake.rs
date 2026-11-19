//! Lane wireflow (g), interfaces/repairflow.md 23: the REGULAR TERMINAL SET R as a phase-D DIAGNOSTIC job at P5 under a
//! REAL `rr_campaign` supervisor with `rr_fake_child` emulating `rr_regbasis` (kind regular-basis, legacy-wrapped; TEST
//! RULE: R = the statement's terminals). The closing TOY node of closeloop_support (test data only) closes; then
//! the repair flow issues ONE regular-basis job on the final statement + effective import spec; the wrapper checks the
//! export object against the PINNED statement (its sha256) and seals it; row 37 writes R and the map NEXT TO THE
//! STATEMENT (final/regbasis/) and the record says so. A forged record (bound to another statement) is refused (row
//! 38): no R, the node is not held. The verdict and the statement are byte-identical with and without the step.
//! Spawns processes: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh wireflow test -p rustred-campaign --test wireflow_regbasis_fake`).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod sup_common;

use closeloop_support::{Toy, blake3_file, campaign_spec, sha256_file, toy};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

/// The closing TOY of closeloop_support (Tier C, recipes R1 / R2 + the default recipe, table evidence) with the repair
/// flow on top (symbolic and recipe escalation off); `regbasis` = the fake script of the diagnostic job (None = off).
fn setup(tag: &str, regbasis: Option<Value>) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let dir = t.path.join("material");
    let files = toy(&dir, &Toy::default(), FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let mut rp = json!({"symbolic": {"enabled": false}, "shadow_test": false, "recipes": {"enabled": false},
        "route0": {"symmetry_credit": false, "symmetry_relation": false},
        "proposals": {"declare_terminal": false}});
    let mut fake = json!({"program": "rr_fake_child", "ms": 5, "faces": {}});
    if let Some(ops) = regbasis {
        rp["regbasis"] = json!({"enabled": true, "wall_s": 120, "cap_bytes": 268435456u64,
            "expected_peak_bytes": 67108864u64});
        fake["regbasis_ops"] = ops;
    }
    let policy = dir.join("repairflow.json");
    std::fs::write(
        &policy,
        serde_json::to_vec_pretty(
            &json!({"schema":"rustred.campaign.repairflow.v1", "repair_policy": rp,
            "nodes": {}, "fake": fake}),
        )
        .unwrap(),
    )
    .unwrap();
    let mut spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    spec["inputs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "repairflow", "role": "repairflow",
        "path": policy.display().to_string(), "blake3": blake3_file(&policy)}));
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

fn run(tag: &str, regbasis: Option<Value>) -> (Tmp, PathBuf) {
    let (t, sp, cdir) = setup(tag, regbasis);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cdir)
    );
    (t, cdir)
}

#[test]
fn the_regular_basis_is_written_next_to_the_statement_and_changes_nothing() {
    let _g = serial();
    let (_t0, c0) = run("wfrb0", None);
    let (_t1, c1) = run("wfrb1", Some(json!([{"op": "prepare", "ms": 10}])));
    let fin0 = read_json(&c0.join("closeloop/toy/final/result.json"));
    let fin1 = read_json(&c1.join("closeloop/toy/final/result.json"));
    assert_eq!(fin1["verdict"], "closed", "{fin1:#}");
    assert_eq!(fin0["verdict"], fin1["verdict"]);
    let st0 = std::fs::read(c0.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    let st1 = std::fs::read(c1.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert!(st0 == st1, "statements differ");
    // without the step: nothing of it
    assert!(!state(&c0).inputs.contains_key("repairflow:toy:regbasis"));
    assert!(!c0.join("closeloop/toy/final/regbasis").exists());
    // with it: one regular-basis job, row 37, R next to the statement bound to its sha256
    let st = state(&c1);
    assert!(
        st.inputs.contains_key("repairflow:toy:regbasis"),
        "{}",
        log_tail(&c1)
    );
    let rec = read_json(&c1.join("closeloop/toy/repairflow/regbasis.json"));
    assert_eq!(rec["status"], "regular_basis", "{rec:#}");
    assert_eq!(rec["row"], 37);
    let rb = read_json(&c1.join("closeloop/toy/final/regbasis/regular_basis.json"));
    assert_eq!(rb["schema"], "rustred.campaign.regular-basis.v1");
    assert_eq!(
        rb["closure_statement_digest"].as_str().unwrap(),
        sha256_file(&c1.join("closeloop/toy/final/statement/statement.jsonl"))
    );
    assert!(c1.join("closeloop/toy/final/regbasis/tower.json").is_file());
    let rep = read_json(&c1.join("closeloop/toy/repairflow/report.json"));
    assert_eq!(rep["regular_basis"]["status"], "regular_basis", "{rep:#}");
    assert!(st.items.values().any(|i| i.kind == "regular-basis"));
    assert_eq!(verify(&c1), 0);
}

#[test]
fn a_record_bound_to_another_statement_is_refused_and_holds_nothing() {
    let _g = serial();
    let (_t, c) = run(
        "wfrbforge",
        Some(json!([{"op": "recipe_quirk", "quirk": "wrong_key"}])),
    );
    assert_eq!(
        read_json(&c.join("closeloop/toy/final/result.json"))["verdict"],
        "closed"
    );
    let rec = read_json(&c.join("closeloop/toy/repairflow/regbasis.json"));
    assert_eq!(rec["status"], "no_regular_basis", "{rec:#}");
    assert!(!c.join("closeloop/toy/final/regbasis").exists());
    assert_eq!(verify(&c), 0);
}
