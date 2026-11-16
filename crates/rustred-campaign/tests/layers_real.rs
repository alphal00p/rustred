//! lane layers (interfaces/closeloop.md 23.4): the equality of COMPOSED statements with the statement of ONE walk on REAL
//! material (ignored; material named by data, nothing node-specific here). `LAYERS_REAL_COMPARE` = a JSON file
//! `[{"name": .., "single": <statement.jsonl of one walk>, "parts": [<statement files composing it>, ..]}, ..]`;
//! `LAYERS_REAL_OUT` (optional) = where the reports are written. Every comparison must be equal: identical key sets,
//! rows key for key without `in_start`, terminals, route-free digests.

use serde_json::{Value, json};
use std::path::PathBuf;

#[test]
#[ignore]
fn composed_statements_equal_the_single_walk_on_real_material() {
    let Ok(spec) = std::env::var("LAYERS_REAL_COMPARE") else {
        eprintln!("LAYERS_REAL_COMPARE not set: nothing to compare");
        return;
    };
    let list: Vec<Value> = serde_json::from_slice(&std::fs::read(&spec).unwrap()).unwrap();
    let mut out = vec![];
    let mut all = true;
    for c in &list {
        let single = PathBuf::from(c["single"].as_str().unwrap());
        let parts: Vec<PathBuf> = c["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PathBuf::from(p.as_str().unwrap()))
            .collect();
        let t0 = std::time::Instant::now();
        let r = rustred_campaign::closeloop::layers::compare_composed(&single, &parts).unwrap();
        let eq = r["equal"] == true;
        all &= eq;
        eprintln!(
            "{}: equal {eq}, keys {} / {}, route-free digests {} / {} ({:.1} s)",
            c["name"],
            r["single"]["keys"],
            r["composed"]["keys"],
            r["single"]["route_free_digest"],
            r["composed"]["route_free_digest"],
            t0.elapsed().as_secs_f64()
        );
        out.push(json!({"name": c["name"], "report": r}));
    }
    if let Ok(o) = std::env::var("LAYERS_REAL_OUT") {
        std::fs::write(o, serde_json::to_vec_pretty(&out).unwrap()).unwrap();
    }
    assert!(all, "{}", serde_json::to_string_pretty(&out).unwrap());
}
