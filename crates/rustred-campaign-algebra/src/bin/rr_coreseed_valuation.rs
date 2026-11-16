//! `rr_coreseed_valuation` (lane coreseed, TEMPORARY until lane poles' `rr_poles` CLI lands): the regularity verdict of
//! certified relations at their keys for the coreseed flow's `RegularityJudge` (interfaces/coreseed.md 1), computed by
//! lane poles' exact valuation `valuation::relation::is_regular_at` (Symbolica; this crate is the only one that links
//! it, so the supervisor calls this program as a child).
//!
//! ```text
//! rr_coreseed_valuation --d0 D < IN.jsonl > OUT.jsonl
//! ```
//! IN: one `{"key": [ints], "record": <engine relation record>}` per line; OUT: one line per input line, in order:
//! `{"verdict": "regular" | "singular" | "undecided", "order": n | null, "reason": text | null}`. Errors: one
//! `{"verdict":"ERROR","error":..}` line, exit 2. Needs the Symbolica licence in the environment.

use rustred_campaign_algebra::valuation::D0;
use rustred_campaign_algebra::valuation::relation::is_regular_at;
use rustred_campaign_core::poles::RegularAt;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let d0 = match args
        .iter()
        .position(|a| a == "--d0")
        .and_then(|i| args.get(i + 1))
    {
        Some(t) => D0::parse(t).unwrap_or_else(|e| die(&format!("--d0: {e:?}"))),
        None => die("usage: rr_coreseed_valuation --d0 D < IN.jsonl > OUT.jsonl"),
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (i, line) in stdin.lock().lines().enumerate() {
        let line = line.unwrap_or_else(|e| die(&format!("stdin: {e}")));
        if line.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| die(&format!("line {}: {e}", i + 1)));
        let key: Vec<i64> = serde_json::from_value(v["key"].clone())
            .unwrap_or_else(|e| die(&format!("line {}: key: {e}", i + 1)));
        let ans = match is_regular_at(&v["record"], &key, &d0) {
            RegularAt::Regular => json!({"verdict": "regular", "order": 0, "reason": null}),
            RegularAt::Singular { order } => {
                json!({"verdict": "singular", "order": order, "reason": null})
            }
            RegularAt::Undecided { reason } => {
                json!({"verdict": "undecided", "order": null, "reason": reason})
            }
        };
        writeln!(out, "{ans}").unwrap_or_else(|e| die(&format!("stdout: {e}")));
    }
}
