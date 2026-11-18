//! Lane gencheck (emission contract, round 3 G5): the IN-FLOW GATE of the closing loop's relation tables, under a REAL
//! `rr_campaign` supervisor with `rr_fake_child` as producer, as rr_proof (proof jobs) AND as the independent checker
//! of the emission (`--mode instance-span`, bound to the record line like rr_proof). The TOY node of closeloop_support
//! with `emission` in its node config: every relation table the flow writes gets its EMITTED COPY beside it (every
//! point record with its derivation member), table-check accepts each copy, and the node closes as without emission.
//! Spawns processes: run ONLY inside a private pid namespace (`LIGHT_PIDNS=1`).

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

fn emitted_tables(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            emitted_tables(&p, out);
        } else if p.to_string_lossy().ends_with(".emitted.table.jsonl") {
            out.push(p);
        }
    }
}

#[test]
fn closeloop_relation_tables_are_emitted_inside_the_flow_and_table_check_accepts_them() {
    let _g = serial();
    let t = Tmp::new("emitcl");
    let toy_cfg = Toy {
        proof: true,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &toy_cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    // emission ON for the node (the fake checker)
    let mut cfg: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    cfg["nodes"][0]["emission"] = json!({"checker": FAKE, "recover_rows": false});
    std::fs::write(&files.config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r: Value = serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/toy/result.json"))
            .or_else(|_| std::fs::read(cdir.join("result.json")))
            .unwrap_or_default(),
    )
    .unwrap_or(Value::Null);
    let mut em = vec![];
    emitted_tables(&cdir, &mut em);
    assert!(!em.is_empty(), "no emitted relation table under {} (result {r})", cdir.display());
    for p in &em {
        let (tc, ok) =
            rustred_campaign::emission::tablecheck::table_check(&rustred_campaign::emission::tablecheck::Args {
                table: p.clone(),
                every: 1,
                ..Default::default()
            })
            .unwrap();
        assert!(ok, "{}: {tc:#}", p.display());
        assert!(tc["rules"].as_u64().unwrap() > 0);
        // the copy's bodies are the flow's own table, line for line
        let plain = PathBuf::from(p.to_string_lossy().replace(".emitted.table.jsonl", ".table.jsonl"));
        let a = std::fs::read_to_string(&plain).unwrap();
        let b = std::fs::read_to_string(p).unwrap();
        assert_eq!(a.lines().count(), b.lines().count(), "{}", p.display());
        for (x, y) in a.lines().zip(b.lines()) {
            let (body, _) = rustred_campaign_core::emission::body::strip_derivation(y).unwrap();
            assert_eq!(x, body);
        }
    }
    assert_eq!(state(&cdir).nodes["toy"].state.phase, rustred_campaign::state::NodePhase::Closed);
}
