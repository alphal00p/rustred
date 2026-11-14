//! `rr_coef_facts`: EXACT coefficient facts of rule records from their own coefficient text, on Symbolica (lane
//! closeloop3; interfaces/closeloop.md 16). A LEGACY child of the campaign (contract kind `coefficient-facts`): no job
//! mode; the supervisor-side wrapper checks every answer against the request, seals and writes OUTCOME.
//!
//! ```text
//! rr_coef_facts facts --requests R --out OUT [--threads N] [--detail]
//! rr_coef_facts crosscheck --requests R --answers A [--answers A2 ..] --out REPORT [--threads N] [--max-pairs N]
//! rr_coef_facts version
//! ```
//! `facts`: R = rr_proof instantiate form (A) lines `{"table", "offset", "rule", "keys"}`; OUT = one
//! `rustred.campaign.fact-verdict.v1` line per (record, key) with `producer: exact_eval` (written as OUT.part, then
//! renamed). `crosscheck`: the same evaluation (with per-term detail) compared with rr_proof instantiate answers A
//! (`rustred.proof-kernel.instantiate.v1` lines) on every (record, key) both answer; REPORT = the counts and the first
//! disagreements; exit 3 if any disagreement. Errors: one `{"verdict":"ERROR","error":..}` line on stdout, exit 2
//! (rr_proof's convention: the wrapper maps it to a refusal, or to an io end when it names a host-resource errno).
//! The Symbolica licence must be in the environment (`source WS/ops/env.sh`); without it the program refuses (exit 2).

#[path = "../coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;

use rustred_campaign_algebra::SymbolicaAlgebra;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

struct Args {
    requests: Option<PathBuf>,
    answers: Vec<PathBuf>,
    out: Option<PathBuf>,
    threads: usize,
    detail: bool,
    max_pairs: Option<usize>,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args {
        requests: None,
        answers: vec![],
        out: None,
        threads: 1,
        detail: false,
        max_pairs: None,
    };
    let mut i = 0;
    let val = |i: &mut usize| -> String {
        *i += 1;
        args.get(*i)
            .cloned()
            .unwrap_or_else(|| die(&format!("{} needs a value", args[*i - 1])))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--requests" => a.requests = Some(PathBuf::from(val(&mut i))),
            "--answers" => a.answers.push(PathBuf::from(val(&mut i))),
            "--out" => a.out = Some(PathBuf::from(val(&mut i))),
            "--threads" => {
                a.threads = val(&mut i)
                    .parse()
                    .unwrap_or_else(|_| die("--threads: not a number"));
                if a.threads == 0 || a.threads > 4 {
                    die("--threads: 1..4");
                }
            }
            "--max-pairs" => {
                a.max_pairs = Some(
                    val(&mut i)
                        .parse()
                        .unwrap_or_else(|_| die("--max-pairs: not a number")),
                )
            }
            "--detail" => a.detail = true,
            x => die(&format!("unknown argument {x:?}")),
        }
        i += 1;
    }
    a
}

fn write_atomic(path: &Path, lines: &[Value]) {
    let part = path.with_extension("part");
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.to_string());
        s.push('\n');
    }
    if let Err(e) = std::fs::write(&part, s) {
        die(&format!("{}: {e}", part.display()));
    }
    if let Err(e) = std::fs::rename(&part, path) {
        die(&format!("{}: {e}", path.display()));
    }
}

fn backend(threads: usize) -> coef_facts::SymbolicaRecordFacts {
    match SymbolicaAlgebra::new() {
        Ok(a) => coef_facts::SymbolicaRecordFacts::new(a).with_threads(threads),
        Err(e) => die(&format!("UNSUPPORTED: {e}")),
    }
}

fn requests(a: &Args) -> Vec<coef_facts::Request> {
    let p = a
        .requests
        .as_ref()
        .unwrap_or_else(|| die("--requests missing"));
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    let r = coef_facts::parse_requests(&t).unwrap_or_else(|e| die(&e));
    if r.is_empty() {
        die("no request");
    }
    r
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let Some(cmd) = argv.get(1) else {
        die("usage: rr_coef_facts facts|crosscheck|version ...")
    };
    let a = parse(&argv[2..]);
    match cmd.as_str() {
        "version" => {
            println!(
                "{}",
                json!({"program": "rr_coef_facts", "version": env!("CARGO_PKG_VERSION"),
                       "producer": coef_facts::PRODUCER, "backend": coef_facts::BACKEND, "method": coef_facts::METHOD})
            );
        }
        "facts" => {
            let reqs = requests(&a);
            let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
            let mut f = backend(a.threads);
            let lines = coef_facts::answer(&reqs, &mut f, a.detail).unwrap_or_else(|e| die(&e));
            write_atomic(&out, &lines);
        }
        "crosscheck" => {
            let mut reqs = requests(&a);
            if a.answers.is_empty() {
                die("--answers missing");
            }
            let out = a.out.clone().unwrap_or_else(|| die("--out missing"));
            if let Some(m) = a.max_pairs {
                let mut left = m;
                for r in reqs.iter_mut() {
                    r.keys.truncate(left);
                    left -= r.keys.len();
                }
                reqs.retain(|r| !r.keys.is_empty());
            }
            // only the answers of requested keys are kept (answer files of a whole node do not fit in memory)
            let wanted: HashSet<Vec<i32>> =
                reqs.iter().flat_map(|r| r.keys.iter().cloned()).collect();
            let mut theirs: HashMap<(String, Vec<i32>), coef_facts::TheirAnswer> = HashMap::new();
            for p in &a.answers {
                let f = std::fs::File::open(p)
                    .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                for l in BufReader::new(f).lines() {
                    let l = l.unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                    if l.trim().is_empty() {
                        continue;
                    }
                    let v: Value = serde_json::from_str(&l)
                        .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
                    if v["schema"] != "rustred.proof-kernel.instantiate.v1" {
                        continue;
                    }
                    let (Some(sha), Ok(key)) = (
                        v["record_sha256"].as_str(),
                        serde_json::from_value::<Vec<i32>>(v["key"].clone()),
                    ) else {
                        continue;
                    };
                    if wanted.contains(&key) {
                        theirs.insert((sha.to_string(), key), coef_facts::their_answer(&v));
                    }
                }
            }
            let t0 = std::time::Instant::now();
            let mut f = backend(a.threads);
            let c = coef_facts::cross_check(&reqs, &mut f, &theirs).unwrap_or_else(|e| die(&e));
            let rep = json!({"schema": "rustred.campaign.coefficient-facts-crosscheck.v1",
                "producer": coef_facts::PRODUCER, "backend": coef_facts::BACKEND,
                "requests": a.requests, "answers": a.answers, "seconds": t0.elapsed().as_secs_f64(),
                "result": c});
            write_atomic(&out, &[rep]);
            println!(
                "{}",
                json!({"pairs": c.pairs, "triples": c.triples, "agree": c.agree, "disagree": c.disagree,
                       "pairs_without_answer": c.pairs_without_answer, "pairs_unusable": c.pairs_unusable})
            );
            if c.disagree > 0 {
                std::process::exit(3);
            }
        }
        x => die(&format!("unknown command {x:?}")),
    }
}
