//! `rr_fake_child` emulating kind `pole-regular-at` (lane coreseed; interfaces/coreseed.md 10) like lane poles'
//! `rr_poles regular-at --requests`: one `rustred.campaign.pole-answer.v1` line per request (in order) to
//! `out/pole_answers.jsonl`, then exit 0 WITHOUT OUTCOME (a legacy child: the supervisor-side wrapper checks, seals and
//! writes OUTCOME). TEST RULE (a stand-in for the exact valuation): the pole order of a relation = the number of its
//! right-hand-side terms whose key text starts with `I(9,`; 0 -> `regular`; a record without `rhs` -> `undecided`.
//! Script ops: `fail` = rr_poles' refusal (an ERROR line on stdout, exit 2); `recipe_quirk wrong_key` = the first
//! answer names another id (the wrapper refuses the answers); `prepare {ms}` = a delay.

use rustred_campaign_contract::fake::{FakeOp, FakeScript, RecipeQuirk};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

pub fn run(spec_path: &Path) -> ! {
    let spec: Value = std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| die("unreadable job.json"));
    let run_dir = spec["run_dir"].as_str().unwrap_or(".").to_string();
    let script: Option<FakeScript> = serde_json::from_value(spec["fake"].clone()).ok();
    let mut wrong_key = false;
    for op in script.iter().flat_map(|s| s.script.iter()) {
        match op {
            FakeOp::Fail { message, .. } => die(message),
            FakeOp::Prepare { ms } => std::thread::sleep(Duration::from_millis(*ms)),
            FakeOp::RecipeQuirk {
                quirk: RecipeQuirk::WrongKey,
            } => wrong_key = true,
            _ => {}
        }
    }
    let rq = spec["inputs"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["role"] == "requests"))
        .and_then(|i| i["path"].as_str())
        .unwrap_or_else(|| die("no requests input"));
    let rp = if Path::new(rq).is_absolute() {
        Path::new(rq).to_path_buf()
    } else {
        Path::new(&run_dir).join(rq)
    };
    let text = std::fs::read_to_string(&rp).unwrap_or_else(|e| die(&format!("requests: {e}")));
    let d0 = spec["args"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .position(|x| x == "--d0")
                .and_then(|i| a.get(i + 1))
        })
        .and_then(Value::as_str)
        .unwrap_or("4")
        .to_string();
    let mut out = String::new();
    for (n, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let q: Value =
            serde_json::from_str(l).unwrap_or_else(|e| die(&format!("line {}: {e}", n + 1)));
        let rec = &q["record"];
        let (verdict, order) = match rec["rhs"].as_array() {
            None => ("undecided", Value::Null),
            Some(rhs) => {
                let p = rhs
                    .iter()
                    .filter(|t| t[0].as_str().is_some_and(|k| k.starts_with("I(9,")))
                    .count();
                if p == 0 {
                    ("regular", json!(0))
                } else {
                    ("pole", json!(p))
                }
            }
        };
        let id = if wrong_key && n == 0 {
            json!("not-the-request")
        } else {
            q["id"].clone()
        };
        let a = json!({"schema": "rustred.campaign.pole-answer.v1", "id": id, "key": q["key"], "d0": d0,
            "verdict": verdict, "pole_order": order,
            "relation": {"relation_digest": rec["relation_digest"], "recipe_digest": rec["recipe_digest"]}});
        out.push_str(&a.to_string());
        out.push('\n');
    }
    let dest = Path::new(&run_dir).join("out").join("pole_answers.jsonl");
    if let Err(e) = std::fs::write(&dest, out) {
        die(&format!("{}: {e}", dest.display()));
    }
    std::process::exit(0)
}
