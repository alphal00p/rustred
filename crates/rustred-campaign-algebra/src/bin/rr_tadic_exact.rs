//! `rr_tadic_exact` (lane tadicjob; interfaces/tadic-job.md 3, the second route at node level): our own EXACT composition
//! of one candidate rr_node table (Q(d), Symbolica) for the keys / weighted requests of a node-level t-adic result, and
//! the comparison of EVERY replayed residue (each term, prime and order below its precision).
//!
//! ```text
//! rr_tadic_exact --table T.jsonl --result R.json [--masters 'I(..);I(..)'] --out OUT.json
//! ```
//! Exit 0 = every residue equal; 1 = a mismatch (listed in OUT.json: a FINDING); 2 = usage / unreadable / refused.

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_algebra::polerank::nodeexact::{NodeTable, compare, parse_key};
use serde_json::json;

fn die(m: &str) -> ! {
    eprintln!("rr_tadic_exact: {m}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(tp), Some(rp), Some(out)) = (get("--table"), get("--result"), get("--out")) else {
        die(
            "usage: rr_tadic_exact --table T.jsonl --result R.json [--masters 'I(..);I(..)'] --out OUT.json",
        )
    };
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&rp).unwrap_or_else(|e| die(&e.to_string())))
            .unwrap_or_else(|e| die(&e.to_string()));
    let masters = get("--masters").map(|m| {
        m.split(';')
            .filter(|x| !x.trim().is_empty())
            .map(|x| parse_key(x).unwrap_or_else(|e| die(&e)))
            .collect()
    });
    let t0 = std::time::Instant::now();
    let table = NodeTable::load(std::path::Path::new(&tp), masters).unwrap_or_else(|e| die(&e));
    let d0 = result["expansion"]["d0"].as_i64().unwrap_or(4).to_string();
    let (n, bad) = compare(&alg, &table, &result, &d0).unwrap_or_else(|e| die(&e));
    let rec = json!({"table": tp, "result": rp, "masters": table.masters.len(), "residues_compared": n,
        "mismatches": bad.len(), "first_mismatches": bad.iter().take(50).collect::<Vec<_>>(),
        "seconds": t0.elapsed().as_secs_f64()});
    std::fs::write(&out, serde_json::to_vec_pretty(&rec).expect("json"))
        .unwrap_or_else(|e| die(&e.to_string()));
    println!("compared {n} residues, {} mismatches", bad.len());
    std::process::exit(if bad.is_empty() { 0 } else { 1 })
}
