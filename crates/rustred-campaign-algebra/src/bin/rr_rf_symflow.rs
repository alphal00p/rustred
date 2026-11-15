//! `rr_rf_symflow`: the repair flow's SYMMETRY PIECES of a node on Symbolica (lane repairflow; interfaces/repairflow.md
//! 17-19): composite descent per open key (TRICKS 1 + 3 + 8), regions (TRICKS 2), the three pieces per face and the
//! per-key relation graph with its acyclicity gate (TRICKS 4).
//!
//! ```text
//! rr_rf_symflow --spec IMPORT_SPEC --out DIR [--base DIR] [--keys KEYS.jsonl] [--threads N] [--policy POLICY.json]
//!               [--node NAME] [--limit N]
//! ```
//! Without `--keys` the open (`uncovered`) keys of the spec's own import check are taken (the closing loop's P1).
//! OUT: `keys.jsonl` (per key: face, every element's outcome, the preferred one), `choices.jsonl` (the per-key record
//! of ALL acceptable choices, `composite::KeyChoices`), `composite.table.jsonl` (the
//! SHIPPED relations: a `supplied_relations` point table, exact coefficients), `composite.verification.jsonl`
//! (bindings), `report.json` (per face counts, regions, pieces, gate). The policy is the repair flow's policy file
//! (`rustred.campaign.repairflow.v1`; section `route0` / `composite` / `regions`); absent = defaults.
//! The Symbolica licence must be in the environment (`source WS/ops/env.sh`). Errors: exit 2 with a message.

#[path = "../compose_sym.rs"]
mod compose_sym;

use compose_sym::{SetCatalogue, SymQd};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::{Evidence, TableSetSpec};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    ComposeOptions, ElementOutcome, KeyChoices, KeyComposites, compose_all, import_lower,
    rows_by_element, shipped_record, verification_record,
};
use rustred_campaign_core::repairflow::policy::PolicyFile;
use rustred_campaign_core::repairflow::symflow;
use rustred_campaign_core::worklist::items::Face;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn die(msg: &str) -> ! {
    eprintln!("rr_rf_symflow: {msg}");
    std::process::exit(2)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let spec_path = arg(&args, "--spec").unwrap_or_else(|| die("--spec"));
    let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| die("--out")));
    let base = PathBuf::from(arg(&args, "--base").unwrap_or_else(|| "/".into()));
    let threads: usize = arg(&args, "--threads")
        .map(|s| s.parse().unwrap_or_else(|_| die("--threads")))
        .unwrap_or(1)
        .max(1);
    let limit: Option<usize> =
        arg(&args, "--limit").map(|s| s.parse().unwrap_or_else(|_| die("--limit")));
    let node = arg(&args, "--node");
    let policy = match arg(&args, "--policy") {
        Some(p) => PolicyFile::parse(
            &std::fs::read_to_string(&p).unwrap_or_else(|e| die(&format!("{p}: {e}"))),
        )
        .and_then(|f| f.effective(node.as_deref()))
        .unwrap_or_else(|e| die(&format!("policy: {e}"))),
        None => Default::default(),
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let t0 = Instant::now();
    let text =
        std::fs::read_to_string(&spec_path).unwrap_or_else(|e| die(&format!("{spec_path}: {e}")));
    let spec = TableSetSpec::from_json(&text).unwrap_or_else(|e| die(&format!("import spec: {e}")));
    let set = rustred_campaign_core::import::load(&spec, &base)
        .unwrap_or_else(|e| die(&format!("import load: {e}")));
    let t_load = t0.elapsed().as_secs_f64();
    let lines = set.frame.mask().to_vec();
    // the open keys: given, or the import check's `uncovered` keys
    let (keys, check_info) = match arg(&args, "--keys") {
        Some(p) => {
            let ks: Vec<Vec<i32>> = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| die(&format!("{p}: {e}")))
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&format!("{p}: {e}"))))
                .collect();
            (ks, json!({"keys_file": p}))
        }
        None => {
            let ev =
                Evidence::from_spec(&set, &base).unwrap_or_else(|e| die(&format!("evidence: {e}")));
            let rows = out.join("check.rows");
            let res = rustred_campaign_core::import::check::check_streamed(
                &set,
                &ev,
                &CheckOptions {
                    threads,
                    tier: None,
                    max_rounds: None,
                },
                &rows,
            )
            .unwrap_or_else(|e| die(&format!("import check: {e}")));
            let _ = std::fs::remove_file(&rows);
            let mut open = vec![];
            let mut other: BTreeMap<String, u64> = BTreeMap::new();
            for r in &res.rows {
                if r.status != RowStatus::Open {
                    continue;
                }
                let s = r.open_status.clone().unwrap_or_else(|| "open".into());
                if s == "uncovered" {
                    open.push(r.key.clone());
                } else {
                    *other.entry(s).or_default() += 1;
                }
            }
            (
                open,
                json!({"closure_keys": res.summary.keys_total, "closure_open": res.summary.open_total,
                "open_other": other, "spec_digest": res.spec_digest}),
            )
        }
    };
    let keys: Vec<Vec<i32>> = match limit {
        Some(n) => keys.into_iter().take(n).collect(),
        None => keys,
    };
    let t_check = t0.elapsed().as_secs_f64();
    // the verified group (derived from the frame header's family, or the policy's family file)
    let fsrc = spec
        .sources
        .iter()
        .find(|s| s.id == spec.frame_from)
        .unwrap_or_else(|| die("frame_from"));
    let fpath = fsrc.path.clone().unwrap_or_else(|| die("frame_from path"));
    let fpath = if Path::new(&fpath).is_absolute() {
        PathBuf::from(&fpath)
    } else {
        base.join(&fpath)
    };
    let header: Value = {
        use std::io::BufRead;
        let f = std::fs::File::open(&fpath)
            .unwrap_or_else(|e| die(&format!("{}: {e}", fpath.display())));
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .unwrap_or_else(|e| die(&e.to_string()));
        serde_json::from_str(l.trim_end()).unwrap_or_else(|e| die(&format!("frame header: {e}")))
    };
    let gs: rustred_campaign_core::import::symmetry::GroupSpec = match &policy.route0.family {
        Some(f) => serde_json::from_value(json!({"family": {"path": f}})),
        None => serde_json::from_value(json!({})),
    }
    .unwrap_or_else(|e| die(&e.to_string()));
    let (group, grep) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        &base,
        &lines,
        set.cmp.presentation().new_to_old(),
        header["family_source"].as_str(),
    )
    .unwrap_or_else(|e| die(&format!("verified group: {e}")));
    let qd = SymQd::new().unwrap_or_else(|e| die(&e));
    let cat = SetCatalogue::new(&set, alg, policy.composite.max_record_bytes);
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lower = |k: &[i32], t: &[i32]| import_lower(&lines, k, t, &cmp);
    let opts = ComposeOptions {
        max_depth: policy.composite.max_depth,
        max_terms: policy.composite.max_terms,
        max_replacements: policy.composite.max_replacements,
    };
    let max_row_terms = policy.route0.max_terms;
    // compose every key (threads over chunks; deterministic order of the results)
    let results: Vec<(Vec<i32>, Result<KeyComposites<compose_sym::Rd>, String>)> = {
        let chunk = keys.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let hs: Vec<_> =
                keys.chunks(chunk)
                    .map(|ks| {
                        let (qd, cat, group) = (&qd, &cat, &group);
                        let (cmp, lower) = (&cmp, &lower);
                        s.spawn(move || {
                            ks.iter()
                                .map(|k| {
                                    let r = rows_by_element(&Independent, group, k, max_row_terms)
                                        .map(|by| {
                                            compose_all(
                                                &Independent,
                                                qd,
                                                cat,
                                                &by,
                                                k,
                                                lower,
                                                cmp,
                                                &opts,
                                            )
                                        });
                                    (k.clone(), r)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
            hs.into_iter()
                .flat_map(|h| h.join().expect("worker"))
                .collect()
        })
    };
    let t_compose = t0.elapsed().as_secs_f64();
    // records: per key, shipped table, verification
    let chart = set.frame.mask_string();
    let mut fk =
        std::fs::File::create(out.join("keys.jsonl")).unwrap_or_else(|e| die(&e.to_string()));
    let mut fc =
        std::fs::File::create(out.join("choices.jsonl")).unwrap_or_else(|e| die(&e.to_string()));
    let mut table = String::new();
    table.push_str(
        &json!({"type": "header", "schema": "table_dump.v1", "family": header["family"].clone(),
            "producer": "rr_rf_symflow", "kind": "symmetry-composite"})
        .to_string(),
    );
    table.push('\n');
    let mut ver = String::new();
    let mut accepted: Vec<symflow::KeyService> = vec![];
    for (k, r) in &results {
        let face = Face::of_key(k, &lines)
            .map(|f| f.label())
            .unwrap_or_default();
        let line = match r {
            Err(e) => json!({"key": k, "face": face, "error": e}),
            Ok(kc) => {
                let ch = KeyChoices::of_composites(k, kc, &lines);
                writeln!(fc, "{}", serde_json::to_string(&ch).expect("json"))
                    .unwrap_or_else(|e| die(&e.to_string()));
                let outs: Vec<Value> = kc
                    .outcomes
                    .iter()
                    .map(|o| match o {
                        ElementOutcome::Accepted(c) => json!({"element": c.element, "accepted": true,
                            "terms": c.rhs.len(), "replacements": c.replacements.len(),
                            "key_coefficient": c.key_coefficient.is_some(), "dropped_zero": c.dropped_zero.len()}),
                        ElementOutcome::Refused { element, why } => json!({"element": element, "accepted": false,
                            "class": why.class(), "why": format!("{why:?}")}),
                    })
                    .collect();
                if let Some(b) = kc.best() {
                    let idx = accepted.len();
                    table.push_str(&shipped_record(&qd, k, b, &chart, idx).to_string());
                    table.push('\n');
                    ver.push_str(
                        &verification_record(&qd, k, b, &grep.digest, "rr_rf_symflow/symbolica")
                            .to_string(),
                    );
                    ver.push('\n');
                    accepted.push(symflow::KeyService::from_composite(k, b, &face));
                }
                json!({"key": k, "face": face, "best": kc.best().map(|b| json!({"element": b.element,
                    "terms": b.rhs.len(), "plain": b.is_plain(), "replacements": b.replacements.len()})),
                    "alternatives": kc.accepted().count(), "outcomes": outs})
            }
        };
        writeln!(fk, "{line}").unwrap_or_else(|e| die(&e.to_string()));
    }
    std::fs::write(out.join("composite.table.jsonl"), &table)
        .unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(out.join("composite.verification.jsonl"), &ver)
        .unwrap_or_else(|e| die(&e.to_string()));
    // per face: served plain / composite / none; the gate over the shipped relations
    let summary = symflow::summarise(
        &results
            .iter()
            .map(|(k, r)| (k.clone(), r.as_ref().ok()))
            .collect::<Vec<_>>(),
        &lines,
    );
    let gate = symflow::gate(&accepted, &|a: &[i32], b: &[i32]| lower(a, b));
    let rep = json!({"schema": "rustred.campaign.repairflow-symflow.v1", "node": node, "spec": spec_path,
        "policy_digest": policy.digest(), "group": {"elements": grep.elements, "digest": grep.digest},
        "check": check_info, "keys": keys.len(), "summary": summary, "gate": gate,
        "seconds": {"load": t_load, "check": t_check - t_load, "compose": t_compose - t_check}, "threads": threads});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&rep).expect("json") + "\n",
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    println!(
        "{}",
        json!({"keys": keys.len(), "summary_totals": rep["summary"]["totals"], "gate": rep["gate"]["verdict"]})
    );
}
