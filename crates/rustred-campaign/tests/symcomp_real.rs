//! Kind `symmetry-composite` with the REAL tool `rr_rf_symflow` through the supervisor adapter and the REAL launcher
//! (lane symjob; interfaces/symmetry-composite-job.md 7). IGNORED by default: it needs the Symbolica licence
//! (`source WS/ops/env.sh`), the tool binary (RUSTRED_SYMFLOW_BIN) and the T178 arrival case of lane repairflow2
//! (test data: RUSTRED_SYMCOMP_SPEC = its import spec, RUSTRED_SYMCOMP_KEYS = its 371 unserved arrival keys; expected
//! counts from its notes: plain 237 / composite 44 / none 85 / over_cap 5). Run in a private pid namespace under the
//! runner's own unit:
//! `LIGHT_PIDNS=1 light_build.sh symjob test -p rustred-campaign --test symcomp_real -- --ignored --nocapture`
//!
//! int8 (round 3): the table set is the PINNED box statement v1.0.3 of T178: the arrival case's import spec keeps its
//! order and fields but only the tables that service 1 of the coordinator's services file pins
//! (`plan/auto-campaign/reports/T178-MARGIN-SERVICES.json`, `RUSTRED_T178_SERVICES` overrides), each with its sha256
//! (verified at load). Left out: 10 order-B pieces the statement's table list does not pin (no statement key served by
//! them; 8 without an end record, refused since partfix4: before int8 the spec could not load). An explicit
//! RUSTRED_SYMCOMP_SPEC is used as given.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod adapters_support;
use adapters_support::*;

use rustred_campaign::adapters::job::{Invocation, JobAdapter, Program, RunOptions, run_attempt};
use rustred_campaign::adapters::symcomp::{
    SymCompRequest, SymmetryCompositeAdapter, VerifyOptions,
};
use rustred_campaign_contract::classify::Accept;
use rustred_campaign_core::worklist::items::Face;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

fn env_path(k: &str, default: &str) -> PathBuf {
    std::env::var_os(k)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

const RF: &str = "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/repairflow";
const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

/// int8: `base` (an import spec) restricted to the tables of T178's pinned statement (service 1 of the services file),
/// each source with its sha256 pin; written under `dir`. Prints what was left out.
fn pinned_spec(base: &str, dir: &std::path::Path) -> PathBuf {
    let services = env_path(
        "RUSTRED_T178_SERVICES",
        &format!("{WS}/plan/auto-campaign/reports/T178-MARGIN-SERVICES.json"),
    );
    let sv: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&services).unwrap()).unwrap();
    let s1 = sv["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["order"] == 1)
        .expect("service 1 (the pinned statement)");
    let pins: BTreeMap<&str, &str> = s1["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["path"].as_str().unwrap(), t["sha256"].as_str().unwrap()))
        .collect();
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base).unwrap()).unwrap();
    let mut left_out = vec![];
    let sources: Vec<serde_json::Value> = spec["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| {
            let Some(sha) = s["path"].as_str().and_then(|p| pins.get(p)) else {
                left_out.push(s["id"].as_str().unwrap_or("?").to_string());
                return None;
            };
            let mut s = s.clone();
            s["sha256"] = serde_json::json!(sha);
            Some(s)
        })
        .collect();
    eprintln!(
        "pinned statement tables: {} of {} sources kept; left out {left_out:?}",
        sources.len(),
        sources.len() + left_out.len()
    );
    spec["sources"] = serde_json::Value::Array(sources);
    let p = dir.join("import-spec.pinned.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    p
}

#[test]
#[ignore = "real tool + Symbolica licence + T178 data: run by hand (module documentation)"]
fn t178_arrival_keys_through_the_adapter_with_the_real_tool() {
    require_namespace();
    let bin = env_path("RUSTRED_SYMFLOW_BIN", "rr_rf_symflow");
    let root = Root::new("real");
    let spec = match std::env::var_os("RUSTRED_SYMCOMP_SPEC") {
        Some(s) => PathBuf::from(s),
        None => pinned_spec(&format!("{RF}/t178faces/import-spec.json"), root.path()),
    };
    let keys_file = env_path(
        "RUSTRED_SYMCOMP_KEYS",
        &format!("{RF}/t178faces/I1.unserved.keys.jsonl"),
    );
    assert!(
        std::env::var_os("SYMBOLICA_LICENSE").is_some(),
        "source WS/ops/env.sh first"
    );
    let threads: u32 = std::env::var("RUSTRED_SYMCOMP_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    // the per-face key lists of the repair flow (faces by support signature, keys in input order)
    let mask: Vec<bool> = "11111111110000000000".chars().map(|c| c == '1').collect();
    let mut faces: BTreeMap<String, Vec<Vec<i64>>> = BTreeMap::new();
    let text = std::fs::read_to_string(&keys_file).unwrap();
    let mut n = 0;
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let k: Vec<i64> = serde_json::from_str(l).unwrap();
        let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        let f = Face::of_key(&k32, &mask)
            .map(|f| f.label())
            .unwrap_or_default();
        faces.entry(f).or_default().push(k);
        n += 1;
    }
    eprintln!("{n} keys on {} faces", faces.len());
    let req = SymCompRequest {
        node: Some("T178arr".into()),
        import_spec: spec,
        import_base: "/".into(),
        faces: faces.into_iter().collect(),
        policy: None,
        threads,
        limit: None,
    };
    let ad = SymmetryCompositeAdapter {
        program: Program::from_file("rr_rf_symflow", &bin, Invocation::Legacy).unwrap(),
        licence_env: Some("SYMBOLICA_LICENSE".into()),
        verify: VerifyOptions::default(),
        fake: None,
    };
    let reqs = vec![req];
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let mut l = launcher();
    let t0 = Instant::now();
    let (out, rep) = run_attempt(
        &mut l,
        &ad,
        &reqs,
        &plan,
        root.path(),
        1,
        &RunOptions::default(),
    )
    .unwrap();
    let so = std::fs::read_to_string(rep.prepared.layout.stdout_log()).unwrap_or_default();
    eprintln!("child stdout: {}", so.trim());
    eprintln!(
        "wall {:.1} s; child {:?} -> {:?}: row {}{} ({}) refusal {:?}; peak rss {:?}",
        t0.elapsed().as_secs_f64(),
        rep.ended.wait,
        out.mapped_wait,
        out.classification.row,
        out.classification.sub,
        out.classification.why,
        out.refusal,
        rep.report.as_ref().map(|r| r.peak_rss_bytes)
    );
    assert_eq!(out.classification.row, 34);
    assert_eq!(out.classification.accept, Accept::SuppliedRelations);
    let r = out.routed.expect("routed");
    let c = r.counts();
    eprintln!("counts {c:?}");
    eprintln!(
        "refused {:?}",
        r.verified.refused.iter().take(5).collect::<Vec<_>>()
    );
    let get = |k: &str| c.get(k).copied().unwrap_or(0);
    assert_eq!(
        (
            get("plain"),
            get("composite"),
            get("none"),
            get("over_cap"),
            get("error")
        ),
        (237, 44, 85, 5, 0)
    );
    assert_eq!(
        r.verified.accepted.len(),
        281,
        "every shipped relation accepted by the import"
    );
    assert!(r.verified.refused.is_empty());
    assert_eq!(get("verdict:accepted"), 281);
    if let Some(d) = std::env::var_os("RUSTRED_SYMCOMP_KEEP") {
        let d = PathBuf::from(d);
        std::fs::create_dir_all(&d).unwrap();
        for a in r.records("accepted") {
            let p = d.join(a.path.trim_start_matches("accepted/"));
            std::fs::write(&p, &a.bytes).unwrap();
            eprintln!("kept {}", p.display());
        }
    }
}
