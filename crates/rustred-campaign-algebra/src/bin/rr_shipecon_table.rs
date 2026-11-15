//! `rr_shipecon_table`: the MEASURED TABLE TIME of rules for the ship economy (lane shipecon; interfaces/shipecon.md):
//! per rule, the load share (read + parse of the record's coefficient texts with the engine's Symbolica path, once) and
//! the application at each served key (the rule point substituted exactly, `compose_sym::SetCatalogue::serve`: the
//! same parse + `replace_multiple` the composite step uses), plus the size of the instantiated relation (the point
//! relation in Q(d) as text: what a stored relation from the table rule would cost).
//!
//! ```text
//! rr_shipecon_table --spec IMPORT_SPEC [--base DIR] --request REQUEST.jsonl --out OUT.jsonl [--max-record-bytes N]
//! ```
//! REQUEST: one line per rule `{"record_sha256", "keys": [[..], ..]}` (the ship-economy driver writes it:
//! `table_time_request.jsonl`). OUT: one line per rule `{"record_sha256", "load_s", "apply_s": {"n0,n1,..": s},
//! "instantiated_bytes": {"n0,..": bytes}, "tool", "error"?}`. Timings are wall clock on a shared host (stated as
//! such); a fresh catalogue per rule (memory: one parsed record at a time). The Symbolica licence must be in the
//! environment (`source WS/ops/env.sh`).

#[path = "../compose_sym.rs"]
mod compose_sym;

use compose_sym::{SetCatalogue, SymQd};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::import::TableSetSpec;
use rustred_campaign_core::repairflow::composite::{Catalogue, Service};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Instant;

fn die(m: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": m}));
    std::process::exit(2)
}

fn key_text(k: &[i32]) -> String {
    k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let get = |n: &str| -> Option<String> {
        a.iter()
            .position(|x| x == n)
            .and_then(|i| a.get(i + 1).cloned())
    };
    let spec_p = PathBuf::from(get("--spec").unwrap_or_else(|| die("--spec")));
    let base = get("--base").map(PathBuf::from).unwrap_or_else(|| {
        spec_p
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let req_p = PathBuf::from(get("--request").unwrap_or_else(|| die("--request")));
    let out_p = PathBuf::from(get("--out").unwrap_or_else(|| die("--out")));
    let max_bytes: u64 = get("--max-record-bytes")
        .map(|x| x.parse().unwrap_or_else(|_| die("--max-record-bytes")))
        .unwrap_or(u64::MAX);
    SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let spec = TableSetSpec::read(&spec_p).unwrap_or_else(|e| die(&format!("import spec: {e}")));
    let t0 = Instant::now();
    let set = rustred_campaign_core::import::load(&spec, &base)
        .unwrap_or_else(|e| die(&format!("import load: {e}")));
    eprintln!("loaded in {:.1} s", t0.elapsed().as_secs_f64());
    let qd = SymQd::new().unwrap_or_else(|e| die(&e));
    let req = std::fs::File::open(&req_p).unwrap_or_else(|e| die(&format!("{}: {e}", req_p.display())));
    let mut out = std::fs::File::create(&out_p).unwrap_or_else(|e| die(&format!("{}: {e}", out_p.display())));
    for l in std::io::BufReader::new(req).lines() {
        let l = l.unwrap_or_else(|e| die(&e.to_string()));
        if l.trim().is_empty() {
            continue;
        }
        let r: Value = serde_json::from_str(&l).unwrap_or_else(|e| die(&format!("request: {e}")));
        let sha = r["record_sha256"].as_str().unwrap_or_else(|| die("record_sha256")).to_string();
        let keys: Vec<Vec<i32>> =
            serde_json::from_value(r["keys"].clone()).unwrap_or_else(|e| die(&format!("keys: {e}")));
        let line = measure(&set, &qd, max_bytes, &sha, &keys);
        writeln!(out, "{line}").unwrap_or_else(|e| die(&e.to_string()));
        out.flush().ok();
        eprintln!("{} {}", &sha[..16.min(sha.len())], line["load_s"]);
    }
}

fn measure(
    set: &rustred_campaign_core::import::ImportedSet,
    qd: &SymQd,
    max_bytes: u64,
    sha: &str,
    keys: &[Vec<i32>],
) -> Value {
    let tool = "rr_shipecon_table: Symbolica parse_ratio of every coefficient text (load, once) + exact replace_multiple \
                at the rule point per key (application); wall clock, shared host";
    let Some((t, r)) = set.find_sha(sha) else {
        return json!({"record_sha256": sha, "error": "record not in the set", "tool": tool});
    };
    let alg = match SymbolicaAlgebra::new() {
        Ok(a) => a,
        Err(e) => return json!({"record_sha256": sha, "error": e, "tool": tool}),
    };
    // a fresh catalogue per rule: one parsed record in memory at a time
    let cat = SetCatalogue::new(set, alg, max_bytes);
    let mut apply = BTreeMap::new();
    let mut inst = BTreeMap::new();
    let mut load_s = None;
    for (i, k) in keys.iter().enumerate() {
        let t1 = Instant::now();
        let s1 = cat.serve(qd, k);
        let first = t1.elapsed().as_secs_f64();
        let svc = match s1 {
            Ok(Service::Rule { rule, rhs }) => {
                if rule.table != t || rule.record != r {
                    return json!({"record_sha256": sha, "error": format!("key {k:?} is served by another record"),
                                  "tool": tool});
                }
                rhs
            }
            Ok(Service::Unserved(w)) => {
                return json!({"record_sha256": sha, "error": format!("key {k:?}: {w}"), "tool": tool});
            }
            Ok(_) => {
                return json!({"record_sha256": sha, "error": format!("key {k:?}: not a rule service"), "tool": tool});
            }
            Err(e) => return json!({"record_sha256": sha, "error": format!("key {k:?}: {e}"), "tool": tool}),
        };
        // the application alone: the record is parsed now (cached in the catalogue)
        let t2 = Instant::now();
        let _ = cat.serve(qd, k);
        let second = t2.elapsed().as_secs_f64();
        if i == 0 {
            load_s = Some((first - second).max(0.0));
        }
        apply.insert(key_text(k), second);
        let bytes: usize = svc
            .iter()
            .map(|(rk, c)| key_text(rk).len() + format!("{c}").len() + 12)
            .sum();
        inst.insert(key_text(k), bytes as u64);
    }
    json!({"record_sha256": sha, "load_s": load_s.unwrap_or(0.0), "apply_s": apply, "instantiated_bytes": inst,
           "keys": keys.len(), "record_bytes": set.rules[t][r].bytes, "tool": tool})
}
