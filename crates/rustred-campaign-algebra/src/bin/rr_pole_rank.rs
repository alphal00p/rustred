//! `rr_pole_rank` (lane tadicjob; interfaces/tadic-job.md 4): the rows of rho and their rank over F_p for ONE t-adic
//! replay result (the driver's `helper-vh2.tadic-result.v1`, whose certificate the caller has checked).
//!
//! ```text
//! rr_pole_rank --result R.json --depths D.json [--dependences X.json] [--library lib/library.json --catalogue C.json]
//!              [--rank-prefix c] [--tseed N] --out OUT.json
//! ```
//! Every input file is bound by its SHA-256 in OUT.json. Exit 0 = a record written (its verdict may be "not decided");
//! 2 = usage / unreadable input. Needs the Symbolica licence (source WS/ops/env.sh).

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_algebra::epsdepth::SymbolicaLaurent;
use rustred_campaign_algebra::polerank::{
    DepthTable, Dependences, RankData, TerminalMap, cell_rank,
};
use serde_json::{Value, json};

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn die(m: &str) -> ! {
    eprintln!("rr_pole_rank: {m}");
    std::process::exit(2)
}

fn read(path: &str, bound: &mut Vec<Value>, role: &str) -> Value {
    let b = std::fs::read(path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    bound.push(json!({"role": role, "path": path, "sha256": sha256_hex(&b), "bytes": b.len()}));
    serde_json::from_slice(&b).unwrap_or_else(|e| die(&format!("{path}: {e}")))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| -> Option<String> {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let results: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--result")
        .filter_map(|(i, _)| args.get(i + 1).cloned())
        .collect();
    let (Some(rp), Some(dp), Some(out)) = (results.first().cloned(), get("--depths"), get("--out")) else {
        die("usage: rr_pole_rank --result R.json --depths D.json [--dependences X.json] [--library L --catalogue C] [--rank-prefix c] [--tseed N] --out OUT.json")
    };
    let mut bound = vec![];
    // several results (rounds of combinations of one cell): their requests are merged (labels must differ)
    let mut result = read(&rp, &mut bound, "result");
    let mut labels = std::collections::BTreeSet::new();
    for r in result["requests"].as_array().into_iter().flatten() {
        labels.insert(r["label"].as_str().unwrap_or("").to_string());
    }
    for p in results.iter().skip(1) {
        let more = read(p, &mut bound, "result");
        if more["status"] == "ERROR" {
            die(&format!("{p} is an ERROR result"));
        }
        for r in more["requests"].as_array().cloned().unwrap_or_default() {
            if !labels.insert(r["label"].as_str().unwrap_or("").to_string()) {
                die(&format!("label {} in two results", r["label"]));
            }
            result["requests"].as_array_mut().expect("requests").push(r);
        }
    }
    if result["status"] == "ERROR" {
        die("the result is an ERROR result (nothing to rank)");
    }
    let _ = rp;
    let depths = DepthTable::from_json(&read(&dp, &mut bound, "depths")).unwrap_or_else(|e| die(&e));
    let deps = get("--dependences").map(|p| {
        Dependences::from_json(&read(&p, &mut bound, "dependences")).unwrap_or_else(|e| die(&e))
    });
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let d0 = result["expansion"]["d0"].as_i64().unwrap_or(4).to_string();
    let lau = SymbolicaLaurent::new(&d0).unwrap_or_else(|e| die(&e));
    let map = match (get("--library"), get("--catalogue")) {
        (Some(l), Some(c)) => {
            let lib = read(&l, &mut bound, "library");
            let cat = read(&c, &mut bound, "catalogue");
            TerminalMap::from_package(&alg, &lau, &lib, &cat).unwrap_or_else(|e| die(&e))
        }
        (None, None) => TerminalMap::default(),
        _ => die("--library and --catalogue go together"),
    };
    let data = RankData {
        map: &map,
        depths: &depths,
        dependences: deps.as_ref(),
        lau: &lau,
    };
    let prefix = get("--rank-prefix").unwrap_or_else(|| "c".into());
    let tseed: u64 = get("--tseed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let t0 = std::time::Instant::now();
    let mut rec = cell_rank(&data, &result, &prefix, tseed);
    rec["inputs"] = json!(bound);
    rec["job_id"] = result["job_id"].clone();
    rec["seconds"] = json!(t0.elapsed().as_secs_f64());
    let tmp = format!("{out}.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&rec).expect("json")).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::rename(&tmp, &out).unwrap_or_else(|e| die(&e.to_string()));
    println!("{}", rec["verdict"]);
}
