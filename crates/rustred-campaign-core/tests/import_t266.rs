//! Lane import2, BIG DATA (ignored): the T266 import on table jointest9 (85 GB, 33,660 rules, pre-guard records without
//! witness loci) as far as the existing material allows. The spec is DATA: tests/data/import/t266-jointest9.spec.json
//! (+ the box B document). Facts / proofs for the served keys do not exist yet; the run reports the load (two passes,
//! time, peak memory), the verified symmetry group (derived from the table header's presented family_source), and a
//! STREAMED, round-limited Tier K check (rows to the statement file): request counts and what is blocked.
//!
//! Env: `T266_KEEP_BIG_LITERALS=1` keeps the 4 records whose exception literals overflow the core's i128 polynomial
//! coefficients (the load then refuses them, as it must); by default they are REMOVED with that reason (a DIAGNOSTIC
//! variant: their keys fall through). `T266_MAX_ROUNDS` (default 1) bounds the check. Scratch output under
//! `RUSTRED_IMPORT_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/import2).

use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, TableSetSpec, load, write_statement};
use serde_json::{Value, json};

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

#[test]
#[ignore = "big data: the 85 GB T266 table; run with --ignored in release (10-20 min)"]
fn t266_jointest9_import_as_far_as_the_material_allows() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import");
    let mut sv: Value = serde_json::from_str(
        &std::fs::read_to_string(data.join("t266-jointest9.spec.json")).unwrap(),
    )
    .unwrap();
    let table = sv["sources"][0]["path"].as_str().unwrap().to_string();
    if !Path::new(&table).is_file() {
        println!("SKIP: {table} missing");
        return;
    }
    if std::env::var("T266_KEEP_BIG_LITERALS").is_err() {
        let reason = "exception literal with a coefficient beyond i128: refused by the core polynomial parser \
                      (Poly::parse, CoefficientOverflow); removed in this DIAGNOSTIC run";
        sv["sources"][0]["removed"] = json!([29474, 30764, 31751, 33573]
            .iter()
            .map(|l| json!({"label": l, "reason": reason}))
            .collect::<Vec<_>>());
    }
    let out = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import2",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
        .join("t266");
    std::fs::create_dir_all(&out).unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = match load(&spec, &data) {
        Ok(s) => s,
        Err(e) => {
            println!(
                "T266 LOAD REFUSED after {:.1}s (VmHWM {} MB): {e}",
                t0.elapsed().as_secs_f64(),
                vm_hwm_kb() / 1024
            );
            return;
        }
    };
    let t_load = t0.elapsed().as_secs_f64();
    let src = &set.report.sources;
    println!(
        "T266 load {:.1}s VmHWM {} MB: tables {} records {} start {} keys; sources {}",
        t_load,
        vm_hwm_kb() / 1024,
        set.report.tables,
        set.report.records,
        set.report.start_keys,
        serde_json::to_string(&src.iter().map(|s| json!({"id": s.id, "records_in_files": s.records_in_files,
            "dispatched": s.records_dispatched, "removed": s.removed.len(), "not_dispatched": s.not_dispatched.len(),
            "witness": s.witness, "group": s.group})).collect::<Vec<_>>()).unwrap()
    );
    let ev = Evidence::from_spec(&set, &data).unwrap();
    let max_rounds: u32 = std::env::var("T266_MAX_ROUNDS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1);
    let t1 = std::time::Instant::now();
    let res = import::check::check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads: 4,
            tier: None,
            max_rounds: Some(max_rounds),
        },
        &out.join("rows.jsonl"),
    )
    .unwrap();
    let t_check = t1.elapsed().as_secs_f64();
    let dir = out.join("statement");
    write_statement(
        &set,
        &ev,
        &res,
        &dir,
        Some(json!({"load_s": t_load, "check_s": t_check, "vm_hwm_kb": vm_hwm_kb(), "max_rounds": max_rounds})),
    )
    .unwrap();
    println!(
        "T266 check (Tier {:?}, max_rounds {max_rounds}) {:.1}s VmHWM {} MB: keys {} counts {:?}; closure verdict {:?} \
         rounds {} edges {} open {} requires_regularity_fact {} fact_requests {} credited {} truncated {}; \
         kept rows {}; requests solve {} facts {} proofs {}; blocking {:?}",
        res.tier,
        t_check,
        vm_hwm_kb() / 1024,
        res.summary.keys_total,
        res.counts(),
        res.summary.verdict,
        res.summary.rounds,
        res.summary.edges,
        res.summary.open_total,
        res.summary.requires_regularity_fact,
        res.summary.fact_requests,
        res.summary.credited,
        res.summary.truncated,
        res.rows.len(),
        res.solve_requests.len(),
        res.fact_requests.len(),
        res.proof_requests.len(),
        res.blocking()
    );
}
