//! `rr_rf_symflow`: the repair flow's SYMMETRY step of a node on Symbolica (lane repairflow; interfaces/repairflow.md
//! 17; proposed contract kind `symmetry-composite`, notes/implementer.repairflow.md "CONTRACT KIND"): composite descent
//! per open key (TRICKS 1 + 3 + 8), every acceptable choice per key, the shipped relations and the interim gate.
//!
//! ```text
//! rr_rf_symflow run --request REQUEST.json --out DIR          (the contract form)
//! rr_rf_symflow --spec IMPORT_SPEC --out DIR [--base DIR] [--keys KEYS.jsonl] [--threads N] [--policy POLICY.json]
//!               [--node NAME] [--limit N]                     (the hand form: builds the same request, unpinned)
//! rr_rf_symflow version
//! ```
//! REQUEST (`rustred.campaign.symmetry-composite.request.v1`): node, import spec / keys / policy as `{path, sha256}`
//! (every given sha256 is verified before use: a mismatch is a refusal), import base, threads, limit. Without `keys` the
//! open (`uncovered`) keys of the spec's own import check are taken.
//! OUT (sealed: each file written as `.part`, then renamed; `SHA256SUMS` over them; `result.json` LAST): `choices.jsonl`
//! (the per-key record of ALL acceptable choices, `composite::KeyChoices`), `keys.jsonl` (per key: every element's
//! outcome), `composite.table.jsonl` (the SHIPPED relations: a `supplied_relations` point table, exact coefficients),
//! `composite.verification.jsonl` (bindings: group digest, element, image, record_sha256 of every rule used),
//! `report.json` (per face, gate). Deterministic: the same request gives byte-identical sealed files for any thread
//! count (results in key order; no clock in them; timings go to the unsealed `timing.json`): resume = re-run.
//! Exit: 0 done (sealed); 2 refusal (usage, input, digest, licence: one `{"verdict":"ERROR","error":..}` line on stdout,
//! nothing sealed). A signal / OOM leaves no `result.json` (the wrapper treats it as a resource failure).
//! The Symbolica licence must be in the environment (`source WS/ops/env.sh`).

#[path = "../compose_sym.rs"]
mod compose_sym;

use compose_sym::{SetCatalogue, SymQd};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::{Evidence, TableSetSpec};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    ComposeAlgebra, ComposeOptions, ElementOutcome, KeyChoices, KeyComposites, compose_all,
    import_lower, rows_by_element, shipped_record, verification_record,
};
use rustred_campaign_core::repairflow::policy::PolicyFile;
use rustred_campaign_core::repairflow::symflow;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Schema of a request.
const REQUEST_SCHEMA: &str = "rustred.campaign.symmetry-composite.request.v1";
/// Schema of `result.json`.
const RESULT_SCHEMA: &str = "rustred.campaign.symmetry-composite.result.v1";

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Read a pinned input: the bytes, refused when a stated sha256 differs.
fn pinned(v: &Value, role: &str) -> Option<(PathBuf, Vec<u8>, String)> {
    if v.is_null() {
        return None;
    }
    let p = PathBuf::from(
        v["path"]
            .as_str()
            .unwrap_or_else(|| die(&format!("{role}: path"))),
    );
    let b = std::fs::read(&p).unwrap_or_else(|e| die(&format!("{role} {}: {e}", p.display())));
    let got = sha256_hex(&b);
    if let Some(want) = v["sha256"].as_str()
        && !want
            .trim_start_matches("sha256:")
            .eq_ignore_ascii_case(&got)
    {
        die(&format!(
            "{role} {}: sha256 {got} differs from the request's {want}",
            p.display()
        ));
    }
    Some((p, b, got))
}

struct Sealer {
    out: PathBuf,
    sums: Vec<(String, String, usize)>,
}

impl Sealer {
    fn write(&mut self, name: &str, bytes: &[u8]) {
        let part = self.out.join(format!("{name}.part"));
        std::fs::write(&part, bytes).unwrap_or_else(|e| die(&format!("{}: {e}", part.display())));
        std::fs::rename(&part, self.out.join(name)).unwrap_or_else(|e| die(&e.to_string()));
        let lines = bytes.iter().filter(|&&c| c == b'\n').count();
        self.sums.push((name.to_string(), sha256_hex(bytes), lines));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("version") {
        println!(
            "{}",
            json!({"tool": "rr_rf_symflow", "request_schema": REQUEST_SCHEMA, "result_schema": RESULT_SCHEMA})
        );
        return;
    }
    if args.get(1).map(String::as_str) == Some("regions") {
        // post-pass on an earlier run: rr_rf_symflow regions --choices FILE --out FILE [--policy P] [--node N]
        let cf = arg(&args, "--choices").unwrap_or_else(|| die("--choices"));
        let recs: Vec<KeyChoices> = std::fs::read_to_string(&cf)
            .unwrap_or_else(|e| die(&format!("{cf}: {e}")))
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&format!("{cf}: {e}"))))
            .collect();
        let pol = match arg(&args, "--policy") {
            Some(p) => PolicyFile::parse(
                &std::fs::read_to_string(&p).unwrap_or_else(|e| die(&e.to_string())),
            )
            .and_then(|f| f.effective(arg(&args, "--node").as_deref()))
            .unwrap_or_else(|e| die(&e)),
            None => Default::default(),
        };
        let v = symflow::fit_face_regions(&recs, &pol.regions);
        let o = arg(&args, "--out").unwrap_or_else(|| die("--out"));
        std::fs::write(&o, serde_json::to_string_pretty(&v).expect("json") + "\n")
            .unwrap_or_else(|e| die(&e.to_string()));
        return;
    }
    let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| die("--out")));
    // the request: given (contract form) or built from the flags (hand form, inputs unpinned)
    let (request, request_bytes): (Value, Vec<u8>) = if args.get(1).map(String::as_str)
        == Some("run")
    {
        let p = arg(&args, "--request").unwrap_or_else(|| die("--request"));
        let b = std::fs::read(&p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        let v: Value = serde_json::from_slice(&b).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        (v, b)
    } else {
        let file = |n: &str| arg(&args, n).map(|p| json!({"path": p}));
        let v = json!({"schema": REQUEST_SCHEMA, "node": arg(&args, "--node"),
                "import_spec": file("--spec").unwrap_or_else(|| die("--spec")),
                "import_base": arg(&args, "--base").unwrap_or_else(|| "/".into()),
                "keys": file("--keys"), "policy": file("--policy"),
                "threads": arg(&args, "--threads").map(|s| s.parse::<u64>().unwrap_or_else(|_| die("--threads"))),
                "limit": arg(&args, "--limit").map(|s| s.parse::<u64>().unwrap_or_else(|_| die("--limit")))});
        let b = serde_json::to_vec_pretty(&v).expect("json");
        (v, b)
    };
    if request["schema"] != REQUEST_SCHEMA {
        die(&format!("request schema must be {REQUEST_SCHEMA}"));
    }
    let node = request["node"].as_str().map(str::to_string);
    let threads = request["threads"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = request["limit"].as_u64().map(|n| n as usize);
    let base = PathBuf::from(request["import_base"].as_str().unwrap_or("/"));
    let (spec_path, spec_bytes, spec_sha) =
        pinned(&request["import_spec"], "import_spec").unwrap_or_else(|| die("import_spec"));
    let keys_in = pinned(&request["keys"], "keys");
    let policy_in = pinned(&request["policy"], "policy");
    let policy = match &policy_in {
        Some((p, b, _)) => PolicyFile::parse(&String::from_utf8_lossy(b))
            .and_then(|f| f.effective(node.as_deref()))
            .unwrap_or_else(|e| die(&format!("policy {}: {e}", p.display()))),
        None => Default::default(),
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let _ = std::fs::remove_file(out.join("result.json"));
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let t0 = Instant::now();
    let spec = TableSetSpec::from_json(&String::from_utf8_lossy(&spec_bytes))
        .unwrap_or_else(|e| die(&format!("import spec: {e}")));
    let set = rustred_campaign_core::import::load(&spec, &base)
        .unwrap_or_else(|e| die(&format!("import load: {e}")));
    let t_load = t0.elapsed().as_secs_f64();
    let lines = set.frame.mask().to_vec();
    // the open keys: given, or the import check's `uncovered` keys (check order)
    let (keys, check_info) = match &keys_in {
        Some((p, b, _)) => {
            let ks: Vec<Vec<i32>> = String::from_utf8_lossy(b)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    serde_json::from_str(l)
                        .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())))
                })
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
    let fpath = symflow::frame_table_path(&set).unwrap_or_else(|| die("frame_from: no table file"));
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
    // compose every key (threads over chunks; results in key order: deterministic)
    type R = Result<KeyComposites<compose_sym::Rd>, String>;
    let results: Vec<(Vec<i32>, R)> = {
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
    // the sealed records
    let chart = set.frame.mask_string();
    let mut fk = String::new();
    let mut fc = String::new();
    let mut table =
        json!({"type": "header", "schema": "table_dump.v1", "family": header["family"].clone(),
        "producer": "rr_rf_symflow", "kind": "symmetry-composite"})
        .to_string();
    table.push('\n');
    let mut ver = String::new();
    let mut accepted: Vec<symflow::KeyService> = vec![];
    let mut all_choices: Vec<KeyChoices> = vec![];
    for (k, r) in &results {
        let line = match r {
            Err(e) => {
                let ch = KeyChoices::failed(k, &lines, e);
                fc.push_str(&serde_json::to_string(&ch).expect("json"));
                fc.push('\n');
                all_choices.push(ch.clone());
                json!({"key": k, "face": ch.face, "status": ch.status})
            }
            Ok(kc) => {
                let ch = KeyChoices::of_composites(k, kc, &lines);
                fc.push_str(&serde_json::to_string(&ch).expect("json"));
                fc.push('\n');
                all_choices.push(ch.clone());
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
                    accepted.push(symflow::KeyService::from_composite(k, b, &ch.face));
                }
                json!({"key": k, "face": ch.face, "status": "ok", "best": kc.best().map(|b| json!({"element": b.element,
                    "terms": b.rhs.len(), "plain": b.is_plain(), "replacements": b.replacements.len()})),
                    "alternatives": kc.accepted().count(), "outcomes": outs})
            }
        };
        fk.push_str(&line.to_string());
        fk.push('\n');
    }
    // independent check of every shipped relation: the image re-derived, the cited records, the composition repeated
    // mod p (engine primes x three d values) with the core's certificate backend (no Symbolica)
    let mf = rustred_campaign_core::repairflow::composite_verify::ModField::engine(
        &Independent,
        &[1_000_003, 7_777_771, 123_456_791],
    );
    let line_of = |t: usize, r: usize| cat.record_line(t, r);
    let mcat = rustred_campaign_core::repairflow::composite_verify::ModCatalogue {
        set: &set,
        line_of: &line_of,
        field: &mf,
    };
    let mut verified = 0usize;
    let mut not_verified: Vec<Value> = vec![];
    for (k, r) in &results {
        let Ok(kc) = r else { continue };
        let Some(b) = kc.best() else { continue };
        let records: Vec<(Vec<i32>, String)> = b
            .replacements
            .iter()
            .map(|x| (x.term.clone(), x.rule.record_sha256.clone()))
            .collect();
        let rhs: Vec<(Vec<i32>, String)> =
            b.rhs.iter().map(|(q, v)| (q.clone(), qd.text(v))).collect();
        let depth = b
            .replacements
            .iter()
            .map(|x| x.depth + 1)
            .max()
            .unwrap_or(0)
            .max(opts.max_depth);
        match rustred_campaign_core::repairflow::composite_verify::verify_shipped(
            &Independent,
            &mf,
            &mcat,
            &group,
            k,
            b.element,
            &b.image,
            &records,
            &rhs,
            &lower,
            &cmp,
            max_row_terms,
            depth,
        ) {
            Ok(()) => verified += 1,
            Err(e) => {
                if not_verified.len() < 50 {
                    not_verified.push(json!({"key": k, "why": e}));
                }
            }
        }
    }
    let t_verify = t0.elapsed().as_secs_f64();
    let summary = symflow::summarise(
        &results
            .iter()
            .map(|(k, r)| (k.clone(), r.as_ref().map_err(|e| e.clone())))
            .collect::<Vec<_>>(),
        &lines,
    );
    let gate = symflow::gate(&accepted, &|a: &[i32], b: &[i32]| lower(a, b));
    let rep = json!({"schema": "rustred.campaign.repairflow-symflow.v1", "node": node,
        "policy_digest": policy.digest(), "group": {"elements": grep.elements, "digest": grep.digest},
        "check": check_info, "keys": keys.len(), "summary": summary, "gate": gate,
        "row_expansion_cap": max_row_terms, "composite_max_depth": opts.max_depth,
        "independent_check": {"method": "image re-derived from the verified group; cited records = first match; composition repeated mod p (2 engine primes x 3 values of d) with the core's certificate backend; shipped coefficients compared",
            "verified": verified, "shipped": accepted.len(), "not_verified": not_verified}});
    let mut seal = Sealer {
        out: out.clone(),
        sums: vec![],
    };
    seal.write("choices.jsonl", fc.as_bytes());
    seal.write("keys.jsonl", fk.as_bytes());
    seal.write("composite.table.jsonl", table.as_bytes());
    seal.write("composite.verification.jsonl", ver.as_bytes());
    let regions = symflow::fit_face_regions(&all_choices, &policy.regions);
    seal.write(
        "regions.json",
        (serde_json::to_string_pretty(&regions).expect("json") + "\n").as_bytes(),
    );
    seal.write(
        "report.json",
        (serde_json::to_string_pretty(&rep).expect("json") + "\n").as_bytes(),
    );
    let sums: String = seal
        .sums
        .iter()
        .map(|(n, h, _)| format!("{h}  {n}\n"))
        .collect();
    std::fs::write(out.join("SHA256SUMS"), &sums).unwrap_or_else(|e| die(&e.to_string()));
    let exe = std::fs::read("/proc/self/exe")
        .map(|b| sha256_hex(&b))
        .unwrap_or_default();
    let mut inputs = vec![json!({"role": "import_spec", "path": spec_path, "sha256": spec_sha})];
    for (role, i) in [("keys", &keys_in), ("policy", &policy_in)] {
        if let Some((p, _, h)) = i {
            inputs.push(json!({"role": role, "path": p, "sha256": h}));
        }
    }
    let result = json!({"schema": RESULT_SCHEMA, "verdict": "DONE", "request_sha256": sha256_hex(&request_bytes),
        "tool": {"name": "rr_rf_symflow", "sha256": exe}, "inputs": inputs,
        "group": {"digest": grep.digest, "elements": grep.elements}, "policy_digest": policy.digest(),
        "spec_digest": rep["check"]["spec_digest"].clone(), "keys": keys.len(),
        "outputs": seal.sums.iter().map(|(n, h, l)| json!({"file": n, "sha256": h, "lines": l})).collect::<Vec<_>>(),
        "totals": rep["summary"]["totals"].clone(), "gate": rep["gate"]["verdict"].clone(),
        "shipped_relations": accepted.len(), "independently_verified": verified});
    let _ = std::fs::write(
        out.join("timing.json"),
        json!({"load_s": t_load, "check_s": t_check - t_load, "compose_s": t_compose - t_check,
            "verify_s": t_verify - t_compose, "threads": threads})
        .to_string(),
    );
    let part = out.join("result.json.part");
    std::fs::write(
        &part,
        serde_json::to_string_pretty(&result).expect("json") + "\n",
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    std::fs::rename(&part, out.join("result.json")).unwrap_or_else(|e| die(&e.to_string()));
    println!(
        "{}",
        json!({"keys": keys.len(), "totals": rep["summary"]["totals"], "gate": rep["gate"]["verdict"]})
    );
}
