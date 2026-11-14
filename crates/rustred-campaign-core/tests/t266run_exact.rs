//! Lane t266run, BIG DATA (ignored; release): the EXACT tier (Tier C, strict, per-key required) of T266 on jointest9
//! with the coordinator lane's exported per-key proofs and exact facts as evidence, held in the disk-backed evidence
//! store (interfaces/evidence-store.md). Node names and numbers here are TEST DATA (L8). No terminals are given: the
//! keys nothing serves stay open (the closing loop's escalation and terminal proposals come later).
//!
//! Env: `T266RUN_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/t266run),
//! `T266RUN_EXPORT` (default the coordinator lane's export-implementer directory), `T266RUN_THREADS` (default 4).
//! Inputs: `T266RUN_INPUTS` (converted native files) or the files listed in the export's SHA256SUMS (each verified by
//! its sha256).

use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::evidence::EvidenceOptions;
use import::store::StoreOptions;
use import::{CheckOptions, Evidence, LoadOptions, TableSetSpec, load_with};
use serde_json::{Value, json};

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn env_path(k: &str, d: &str) -> PathBuf {
    std::env::var_os(k)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(d))
}

fn scratch() -> PathBuf {
    env_path(
        "T266RUN_SCRATCH",
        &format!(
            "/common/build-cache/{}/p_integral_dev/implementer-scratch/t266run",
            std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
        ),
    )
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import")
}

/// (facts, proofs) FileRefs: `T266RUN_INPUTS` (a directory of converted `facts-*.jsonl` / `proofs-*.jsonl`, see
/// tests/t266run_convert.rs) when set, else the export's SHA256SUMS (`<hex>  <relative path>`).
fn export_inputs(export: &Path) -> (Vec<Value>, Vec<Value>) {
    if let Some(d) = std::env::var_os("T266RUN_INPUTS") {
        let mut facts = vec![];
        let mut proofs = vec![];
        let mut v: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        v.sort();
        for p in v {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            let r = json!({"path": p.display().to_string()});
            if n.starts_with("facts-") && n.ends_with(".jsonl") {
                facts.push(r);
            } else if n.starts_with("proofs-") && n.ends_with(".jsonl") {
                proofs.push(r);
            }
        }
        return (facts, proofs);
    }
    let sums = import::lines::read_sha256sums(&export.join("SHA256SUMS")).unwrap();
    let mut facts = vec![];
    let mut proofs = vec![];
    for (name, hex) in sums {
        let p = export.join(&name);
        let r = json!({"path": p.display().to_string(), "sha256": hex});
        if name.starts_with("facts/") {
            facts.push(r);
        } else if name.starts_with("proofs/") {
            proofs.push(r);
        }
    }
    (facts, proofs)
}

#[test]
#[ignore = "big data: T266 exact tier with the exported evidence in the store (index + store reused when valid)"]
fn t266_exact_tier_with_exported_evidence() {
    let s = scratch();
    let export = env_path(
        "T266RUN_EXPORT",
        "/common/build-cache/vhirschi/p_integral_dev/t266-close/export-implementer",
    );
    let threads: usize = std::env::var("T266RUN_THREADS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(4);
    // `T266RUN_SPEC`: a complete import spec (absolute paths; its evidence and terminals are used as they are)
    let given = std::env::var_os("T266RUN_SPEC").map(PathBuf::from);
    let mut sv: Value = serde_json::from_str(
        &std::fs::read_to_string(
            given
                .clone()
                .unwrap_or_else(|| data_dir().join("t266-jointest9.spec.json")),
        )
        .unwrap(),
    )
    .unwrap();
    // no terminals as input; per-key required; the exact tier
    sv["sources"] = json!(
        sv["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["kind"] != "terminals")
            .cloned()
            .collect::<Vec<_>>()
    );
    sv["required_evidence"] = json!("per-key");
    sv["policies"]["tier"] = json!("C");
    if given.is_none() {
        let (facts, proofs) = export_inputs(&export);
        println!(
            "T266RUN inputs: {} fact files, {} proof files",
            facts.len(),
            proofs.len()
        );
        sv["evidence"] = json!({"facts": facts, "proofs": proofs, "solved": []});
    }
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let t0 = std::time::Instant::now();
    let set = load_with(
        &spec,
        &data_dir(),
        &LoadOptions {
            index_dir: Some(s.join("index")),
        },
    )
    .unwrap();
    let load_s = t0.elapsed().as_secs_f64();
    let rss_load = status_kb("VmRSS:");
    let t1 = std::time::Instant::now();
    let ev = Evidence::from_spec_with(
        &set,
        &data_dir(),
        &EvidenceOptions {
            store: Some(StoreOptions {
                dir: std::env::var_os("T266RUN_STORE")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| s.join("store")),
                budget_bytes: 192 << 20,
                threads,
            }),
        },
    )
    .unwrap();
    let store_s = t1.elapsed().as_secs_f64();
    let hwm_store = status_kb("VmHWM:");
    println!(
        "T266RUN store: {} ({store_s:.1} s, VmHWM {hwm_store} kB)",
        serde_json::to_string(&ev.counts.stores).unwrap()
    );
    let out = s.join("exact");
    std::fs::create_dir_all(&out).unwrap();
    let t2 = std::time::Instant::now();
    let res = import::check::check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads,
            tier: Some(rustred_campaign_core::closure::Tier::C),
            max_rounds: None,
        },
        &out.join("rows.jsonl"),
    )
    .unwrap();
    let check_s = t2.elapsed().as_secs_f64();
    let mut open_status: std::collections::BTreeMap<String, u64> = Default::default();
    for r in &res.rows {
        if r.status == import::RowStatus::Open {
            *open_status
                .entry(r.open_status.clone().unwrap_or_default())
                .or_default() += 1;
        }
    }
    let fact_keys: usize = res.fact_requests.iter().map(|r| r.keys.len()).sum();
    let proof_keys: usize = res.proof_requests.iter().map(|r| r.keys.len()).sum();
    let rep = json!({
        "load_s": load_s, "store_s": store_s, "check_s": check_s,
        "vm_rss_after_load_kb": rss_load, "vm_hwm_kb": status_kb("VmHWM:"),
        "keys": res.streamed.as_ref().map(|x| x.rows), "kept_rows": res.rows.len(),
        "counts": res.counts(), "evidence_by_class": res.evidence_by_class(&set),
        "blocking": res.blocking(), "open_by_status": open_status,
        "closure": {"verdict": format!("{:?}", res.summary.verdict), "rounds": res.summary.rounds,
            "edges": res.summary.edges, "used_rules": res.summary.used_rules,
            "descent_violations": res.summary.descent_violations, "blocking": res.summary.blocking,
            "credited": res.summary.credited, "per_key_lifted": res.summary.per_key_lifted},
        "requests": {"fact_records": res.fact_requests.len(), "fact_keys": fact_keys,
            "proof_records": res.proof_requests.len(), "proof_keys": proof_keys,
            "solve": res.solve_requests.len()},
        "evidence_counts": serde_json::to_value(&ev.counts).unwrap(),
    });
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&rep).unwrap(),
    )
    .unwrap();
    // the open keys and the requests (inputs of the residual jobs)
    let mut w = String::new();
    for r in &res.rows {
        if r.status == import::RowStatus::Open {
            w.push_str(&format!(
                "{}\n",
                json!({"key": r.key, "status": r.open_status, "reason": r.open_reason})
            ));
        }
    }
    std::fs::write(out.join("open.jsonl"), w).unwrap();
    let mut w = String::new();
    for r in &res.proof_requests {
        w.push_str(&format!(
            "{}\n",
            json!({"record_sha256": r.record_sha256, "rule": r.rule, "offset": r.offset, "keys": r.keys})
        ));
    }
    std::fs::write(out.join("proof-requests.jsonl"), w).unwrap();
    let mut w = String::new();
    for r in &res.fact_requests {
        w.push_str(&format!(
            "{}\n",
            json!({"record_sha256": r.record_sha256, "keys": r.keys})
        ));
    }
    std::fs::write(out.join("fact-requests.jsonl"), w).unwrap();
    println!("T266RUN EXACT {}", serde_json::to_string(&rep).unwrap());
}
