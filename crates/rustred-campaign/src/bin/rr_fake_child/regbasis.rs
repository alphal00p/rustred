//! `rr_fake_child` emulating kind `regular-basis` (lane wireflow (g); interfaces/regbasis.md) like lane regbasis'
//! `rr_regbasis --import-spec --statement-dir --out`: writes `out/regbasis/regular_basis.json`
//! (`rustred.campaign.regular-basis.v1`, bound to the statement by its sha256; TEST RULE: R = the statement's terminal
//! keys, M = identity) and `out/regbasis/tower.json`, then exits 0 WITHOUT OUTCOME (a legacy child: the supervisor-side
//! wrapper checks, seals and writes OUTCOME). Script ops: `fail` = the tool's refusal (exit 2); `recipe_quirk
//! wrong_key` = the record names another statement digest (the wrapper refuses it: row 38); `prepare {ms}` = a delay.

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
    let mut wrong = false;
    for op in script.iter().flat_map(|s| s.script.iter()) {
        match op {
            FakeOp::Fail { message, .. } => die(message),
            FakeOp::Prepare { ms } => std::thread::sleep(Duration::from_millis(*ms)),
            FakeOp::RecipeQuirk {
                quirk: RecipeQuirk::WrongKey,
            } => wrong = true,
            _ => {}
        }
    }
    let path_of = |role: &str| -> std::path::PathBuf {
        let p = spec["inputs"]
            .as_array()
            .and_then(|a| a.iter().find(|i| i["role"] == role))
            .and_then(|i| i["path"].as_str())
            .unwrap_or_else(|| die(&format!("no {role} input")));
        if Path::new(p).is_absolute() {
            Path::new(p).to_path_buf()
        } else {
            Path::new(&run_dir).join(p)
        }
    };
    let stmt =
        std::fs::read(path_of("statement")).unwrap_or_else(|e| die(&format!("statement: {e}")));
    let terms: Value = std::fs::read(path_of("terminals"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let sha = rustred_campaign_contract::symcert::sha256_hex(&stmt);
    let rec = json!({"schema": rustred_campaign_contract::regbasis::RECORD_SCHEMA, "producer": "rr_fake_child",
        "closure_statement_digest": if wrong { "0".repeat(64) } else { sha },
        "regular_set": terms, "convention": "band_plus_terminals", "matrix": "identity (fake)"});
    let tower = json!({"schema": rustred_campaign_contract::regbasis::TOWER_SCHEMA, "producer": "rr_fake_child",
        "rows": []});
    let out = Path::new(&run_dir)
        .join("out")
        .join(rustred_campaign_contract::regbasis::CHILD_DIR);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(out.join("regular_basis.json"), rec.to_string())
        .unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(out.join("tower.json"), tower.to_string())
        .unwrap_or_else(|e| die(&e.to_string()));
    println!("{}", json!({"fake": true, "written": "regular_basis.json"}));
    std::process::exit(0)
}
