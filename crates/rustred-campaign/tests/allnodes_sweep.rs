//! Lane allnodes: the per-node step of the integer-index sweep ("demonstrate that it could close the full campaign").
//!
//! `allnodes_node` (ignored; env-driven, run by tests/data/allnodes/sweep.sh): ONE node, one mode, through the integrated
//! import + closure library and nothing else (no hand tuning: the spec is generated from the ledgers by
//! tests/data/allnodes/gen_specs.py; this step only adds the mode's policies and the evidence files):
//! - `k`: strict LOAD of the spec (completeness markers, record kinds, terminal ledgers) + Tier K keys-only closure from the
//!   box (`witness_check: off_diagnostic`: a keys-only SUPERSET; the verdict is reported as the one without the stamp);
//! - `c`: strict Tier C with the facts gathered so far (`AN_FACTS`, pinned by sha256 + family); writes the check's own fact
//!   requests (`requests.jsonl`) and proof requests (`proof_requests.jsonl`); when no fact request is left, the decisive
//!   zeros of the decided check (campaign `closeloop::zeros::scan`) and their second-route requests (`zreq.jsonl`, form A,
//!   for `rr_coef_facts facts`) with the pair list (`decisive.jsonl`);
//! - `z2`: the zero second route: first-route evidence (`AN_FACTS`) vs second-route evidence (`AN_FACTS2`) on every
//!   decisive pair (`closeloop::zeros::compare_pair`).
//!
//! One JSON line per run is appended to `AN_OUT`. Nothing here knows a node: names, paths and numbers are env / spec data.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::closeloop::zeros;
use rustred_campaign_core::import::check::check_streamed;
use rustred_campaign_core::import::load::{LoadOptions, load_with};
use rustred_campaign_core::import::{CheckOptions, Evidence, RowStatus, TableSetSpec};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|x| x.parse().ok())
        })
        .unwrap_or(0)
}

fn sha256_file(p: &Path) -> Result<String, String> {
    let out = std::process::Command::new("sha256sum")
        .arg("--")
        .arg(p)
        .output()
        .map_err(|e| format!("sha256sum: {e}"))?;
    if !out.status.success() {
        return Err(format!("sha256sum {} failed", p.display()));
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or_else(|| "sha256sum: no output".into())
}

/// The spec of this mode: the generated spec + policies + (pinned) evidence files.
fn spec_for(base: &Value, mode: &str, facts: &[PathBuf], family: &str) -> Result<Value, String> {
    let mut spec = base.clone();
    if mode == "k" {
        spec["policies"] = json!({"witness_check": "off_diagnostic", "tier": "K"});
        return Ok(spec);
    }
    spec["policies"] = json!({"tier": "C"});
    let mut ev = vec![];
    for f in facts {
        if std::fs::metadata(f).map(|m| m.len() > 0).unwrap_or(false) {
            ev.push(json!({"path": f.display().to_string(), "sha256": sha256_file(f)?, "family": family}));
        }
    }
    if !ev.is_empty() {
        spec["evidence"] = json!({"facts": ev});
    }
    Ok(spec)
}

fn write_jsonl<T: serde::Serialize>(p: &Path, v: &[T]) -> Result<(), String> {
    let text: String = v
        .iter()
        .map(|q| format!("{}\n", serde_json::to_string(q).unwrap()))
        .collect();
    std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))
}

#[test]
#[ignore = "lane allnodes sweep step; env-driven (tests/data/allnodes/sweep.sh)"]
fn allnodes_node() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k}"));
    let (node, spec_path, dir, mode, family) = (
        env("AN_NODE"),
        env("AN_SPEC"),
        PathBuf::from(env("AN_DIR")),
        env("AN_MODE"),
        env("AN_FAMILY"),
    );
    let threads: usize = std::env::var("AN_THREADS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(4);
    let index_dir = std::env::var("AN_INDEX").ok().map(PathBuf::from);
    let facts: Vec<PathBuf> = std::env::var("AN_FACTS")
        .ok()
        .map(|s| {
            s.split(':')
                .filter(|x| !x.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    std::fs::create_dir_all(&dir).unwrap();
    let t0 = std::time::Instant::now();
    let mut line = json!({"node": node, "mode": mode});
    let res = (|| -> Result<(), String> {
        let base: Value = serde_json::from_str(
            &std::fs::read_to_string(&spec_path).map_err(|e| format!("{spec_path}: {e}"))?,
        )
        .map_err(|e| format!("spec json: {e}"))?;
        let lopts = LoadOptions {
            index_dir: index_dir.clone(),
        };
        if mode == "z2" {
            return zero_second_route(&base, &facts, &family, &dir, &lopts, &mut line);
        }
        let spec = spec_for(&base, &mode, &facts, &family)?;
        let spec = TableSetSpec::from_json(&spec.to_string()).map_err(|e| format!("spec: {e}"))?;
        let tl = std::time::Instant::now();
        let set = load_with(&spec, Path::new("/"), &lopts).map_err(|e| format!("load: {e}"))?;
        line["load_s"] = json!(tl.elapsed().as_secs_f64());
        line["loaded"] = json!(true);
        line["records"] = json!(set.rules.iter().map(Vec::len).sum::<usize>());
        line["table_terminals_declared"] = json!(set.terminal_provenance.len());
        line["family"] = json!(set.family);
        if mode == "k" {
            std::fs::write(
                dir.join("load_report.json"),
                serde_json::to_string(&set.report).unwrap_or_default(),
            )
            .ok();
        }
        let ev = Evidence::from_spec(&set, Path::new("/")).map_err(|e| format!("evidence: {e}"))?;
        let rows = dir.join(format!("rows_{mode}.jsonl"));
        let r = check_streamed(
            &set,
            &ev,
            &CheckOptions {
                threads,
                tier: None,
                max_rounds: None,
            },
            &rows,
        )
        .map_err(|e| format!("check: {e}"))?;
        let s = &r.summary;
        line["tier"] = json!(format!("{:?}", r.tier));
        line["verdict"] = serde_json::to_value(s.verdict).unwrap();
        line["verdict_if_not_diagnostic"] =
            serde_json::to_value(s.verdict_if_not_diagnostic).unwrap();
        line["blockers"] = serde_json::to_value(&s.blocking).unwrap_or(Value::Null);
        line["keys"] = json!(s.keys_total);
        line["keys_in_start"] = json!(s.keys_in_start);
        line["keys_outside"] = json!(s.keys_outside);
        line["open"] = json!(s.open_total);
        line["open_in_start"] = json!(s.open_in_start);
        line["open_by_status"] = json!(s.open_by_status);
        line["terminals"] = json!(s.terminals_other + s.terminals_preferred);
        line["descent_violations"] = json!(s.descent_violations);
        line["cycles"] = json!(s.cycles);
        line["used_rules"] = json!(s.used_rules);
        line["fact_source"] = json!(s.fact_source);
        line["facts_exact"] = json!(s.facts_exact);
        line["hand_down_children"] = json!(s.hand_downs.len());
        // terminal keys (small: the exceptional rows only)
        let term: Vec<&Vec<i32>> = r
            .rows
            .iter()
            .filter(|row| row.status == RowStatus::Terminal)
            .map(|row| &row.key)
            .collect();
        line["terminal_keys"] = json!(term);
        // open keys (first 50; the exceptional rows are kept by check_streamed): key, core status, reason, blocking rule
        let open: Vec<Value> = r
            .rows
            .iter()
            .filter(|row| row.status == RowStatus::Open)
            .take(50)
            .map(|row| {
                json!({"key": row.key, "status": row.open_status, "reason": row.open_reason,
                    "rule": row.open_rule.map(|(t, rr)| set.rules[t][rr].index.unwrap_or(set.rules[t][rr].label))})
            })
            .collect();
        line["open_keys"] = json!(open);
        // load-report facts of the strict load: completeness markers, bounds records, witness kinds, line kinds
        let rep = serde_json::to_value(&set.report).unwrap_or(Value::Null);
        if let Some(src) = rep["sources"].as_array() {
            line["load"] = json!(src.iter().map(|x| json!({"files_without_end": x["files_without_end"],
                "bounds_records": x["bounds_records"], "witness": x["witness"], "line_kinds": x["line_kinds"],
                "records_in_files": x["records_in_files"], "records_dispatched": x["records_dispatched"],
                "not_dispatched": x["not_dispatched"].as_array().map(Vec::len)})).collect::<Vec<_>>());
        }
        let req_keys: usize = r.fact_requests.iter().map(|q| q.keys.len()).sum();
        let proof_keys: usize = r.proof_requests.iter().map(|q| q.keys.len()).sum();
        line["request_records"] = json!(r.fact_requests.len());
        line["request_keys"] = json!(req_keys);
        line["proof_request_records"] = json!(r.proof_requests.len());
        line["proof_request_keys"] = json!(proof_keys);
        line["solve_requests"] = json!(r.solve_requests.len());
        if mode == "k" {
            // PREFETCH (driver option, lane allnodes DEVIATION): the Tier K superset's rule-served keys OUTSIDE the box with
            // the record that wins there, as rr_proof requests (form A): the strict closure reaches outside keys one frontier
            // per round; answering the superset's outside winners up front saves those rounds. The check still decides from
            // its own requests; an answer for a record that is not the strict winner is never used.
            let mut by: BTreeMap<(usize, usize), Vec<Vec<i32>>> = BTreeMap::new();
            let f = std::fs::File::open(&rows).map_err(|e| e.to_string())?;
            for l in std::io::BufRead::lines(std::io::BufReader::new(f)) {
                let l = l.map_err(|e| e.to_string())?;
                let v: Value = serde_json::from_str(&l).map_err(|e| e.to_string())?;
                if v["in_start"] == json!(true) || v["server"]["kind"] != json!("rule") {
                    continue;
                }
                // the winning record by its sha256 (the set's own index; never by position)
                let Some((t, o)) = v["server"]["record_sha256"]
                    .as_str()
                    .and_then(|x| set.find_sha(x))
                else {
                    continue;
                };
                let key: Vec<i32> =
                    serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
                by.entry((t, o)).or_default().push(key);
            }
            let mut pre = vec![];
            let mut n = 0usize;
            for ((t, o), keys) in by {
                let info = &set.rules[t][o];
                n += keys.len();
                pre.push(json!({"table": info.path, "offset": info.offset,
                    "rule": info.index.unwrap_or(info.label), "record_sha256": info.record_sha256, "keys": keys}));
            }
            write_jsonl(&dir.join("k_requests.jsonl"), &pre)?;
            line["prefetch_outside_keys"] = json!(n);
        }
        if mode == "c" {
            write_jsonl(&dir.join("requests.jsonl"), &r.fact_requests)?;
            write_jsonl(&dir.join("proof_requests.jsonl"), &r.proof_requests)?;
            if r.fact_requests.is_empty() {
                // decided: the decisive zeros and their second-route requests
                let z = zeros::scan(&set, &ev, &r, threads)?;
                line["zero_terms"] = json!({"top": z.zero_top, "positive_numerator": z.zero_positive_numerator,
                    "other": z.zero_other});
                line["decisive_zeros"] = json!(z.decisive.len());
                line["non_descending"] = json!(z.non_descending.len());
                line["followed_top_edges"] = json!(z.followed_top);
                let mut by: BTreeMap<(usize, usize), Vec<Vec<i32>>> = BTreeMap::new();
                let mut pairs = vec![];
                for d in &z.decisive {
                    by.entry((d.table, d.record))
                        .or_default()
                        .push(d.applied.clone());
                    let info = &set.rules[d.table][d.record];
                    pairs.push(
                        json!({"record_sha256": info.record_sha256, "applied": d.applied,
                        "key": d.key, "kind": d.kind, "term_key": d.term_key}),
                    );
                }
                let mut zreq = vec![];
                for ((t, rr), mut keys) in by {
                    keys.sort();
                    keys.dedup();
                    let info = &set.rules[t][rr];
                    zreq.push(json!({"table": info.path, "offset": info.offset,
                        "rule": info.index.unwrap_or(info.label), "keys": keys}));
                }
                write_jsonl(&dir.join("zreq.jsonl"), &zreq)?;
                write_jsonl(&dir.join("decisive.jsonl"), &pairs)?;
            }
        }
        Ok(())
    })();
    if let Err(e) = res {
        line["loaded"] = line.get("loaded").cloned().unwrap_or(json!(false));
        line["error"] = json!(e);
    }
    line["seconds"] = json!(t0.elapsed().as_secs_f64());
    line["peak_rss_kb"] = json!(vm_hwm_kb());
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(env("AN_OUT"))
        .unwrap();
    writeln!(f, "{line}").unwrap();
}

/// `z2`: compare the first route (the facts the check used) with the second route on every decisive pair.
fn zero_second_route(
    base: &Value,
    facts: &[PathBuf],
    family: &str,
    dir: &Path,
    lopts: &LoadOptions,
    line: &mut Value,
) -> Result<(), String> {
    let f2 = PathBuf::from(std::env::var("AN_FACTS2").map_err(|_| "AN_FACTS2")?);
    let s1 = TableSetSpec::from_json(&spec_for(base, "c", facts, family)?.to_string())
        .map_err(|e| format!("spec: {e}"))?;
    let mut set = load_with(&s1, Path::new("/"), lopts).map_err(|e| format!("load: {e}"))?;
    let ev1 = Evidence::from_spec(&set, Path::new("/")).map_err(|e| format!("evidence: {e}"))?;
    // the second route's own evidence object, over the same set (never merged with the first): the set's evidence
    // list is swapped for the second route's file while that object is built, then restored
    let s2 = TableSetSpec::from_json(&spec_for(base, "c", &[f2], family)?.to_string())
        .map_err(|e| format!("spec2: {e}"))?;
    let s1 = std::mem::replace(&mut set.spec, s2);
    let ev2 = Evidence::from_spec(&set, Path::new("/")).map_err(|e| format!("evidence2: {e}"));
    set.spec = s1;
    let ev2 = ev2?;
    let text = std::fs::read_to_string(dir.join("decisive.jsonl")).map_err(|e| e.to_string())?;
    let (mut confirmed, mut disagree, mut unanswered, mut dis) = (0u64, 0u64, 0u64, vec![]);
    let mut seen = std::collections::BTreeSet::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
        let sha = v["record_sha256"].as_str().unwrap_or_default().to_string();
        let key: Vec<i32> =
            serde_json::from_value(v["applied"].clone()).map_err(|e| e.to_string())?;
        if !seen.insert((sha.clone(), key.clone())) {
            continue;
        }
        match zeros::compare_pair(&set, &ev1, &ev2, &sha, &key) {
            None => unanswered += 1,
            Some(pc) if pc.second == "none" => unanswered += 1,
            Some(pc) if pc.agree() => confirmed += 1,
            Some(pc) => {
                disagree += 1;
                if dis.len() < 20 {
                    dis.push(pc.json());
                }
            }
        }
    }
    line["second_route"] = json!({"routes": "rr_proof + exact_eval", "pairs": seen.len(), "confirmed": confirmed,
        "disagreements": disagree, "unanswered": unanswered, "disagreement_list": dis});
    Ok(())
}
