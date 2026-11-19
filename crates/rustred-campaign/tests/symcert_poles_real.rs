//! Lane symcert, SECOND ROUTE (b) (ignored; comparison only, never evidence): re-solve a sample of recipe relations with
//! the FROZEN engine (whose relation records carry `pole_order_d4`) through the solve-recipe adapter and the process
//! layer (one job per (recipe, key); run inside a private pid namespace, L10), and write every SOLVED record for the
//! valuation comparison of the algebra crate (`tests/symcert_valuation.rs` there). Driven by `SYMCERT_POLES` = a JSON
//! config {"program": path, "program_sha256": hex, "family": path, "order": value, "sample": path of JSONL lines
//! {"recipe": path, "key": [..], "relation_digest": hex | null}, "work_root": dir, "parallel": n, "key_budget_s": n,
//! "mem_cap_gb": n, "wall_s": n}. Output: <work_root>/records.jsonl (one line per pair: the pair, the job's row, the
//! verified record when solved, and whether its relation digest equals the earlier engine's).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::adapters::job::{
    InputFile, Invocation, JobAdapter, Program, RunOptions, run_attempt,
};
use rustred_campaign::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest,
};
use rustred_campaign::launch::RealLauncher;
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[test]
#[ignore = "real engine run: SYMCERT_POLES=<config.json>, LIGHT_PIDNS=1"]
fn symcert_poles_real() {
    let c: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("SYMCERT_POLES").unwrap()).unwrap(),
    )
    .unwrap();
    let st = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    // `rr_node job RUN/job.json` (the frozen engines' contract mode, as lane coreseed's real runs)
    let prog =
        Program::from_file("rr_node", Path::new(&st("program")), Invocation::Contract).unwrap();
    assert_eq!(prog.sha256.as_deref(), Some(st("program_sha256").as_str()));
    let family = InputFile::from_file(Path::new(&st("family"))).unwrap();
    let order = match &c["order"] {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let work = PathBuf::from(st("work_root"));
    std::fs::create_dir_all(&work).unwrap();
    let sample: Vec<Value> = std::fs::read_to_string(st("sample"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
            foreign_pin_max_cpus: usize::MAX,
        })
        .unwrap(),
    );
    let queue = Arc::new(Mutex::new(
        sample.iter().cloned().enumerate().collect::<VecDeque<_>>(),
    ));
    let (tx, rx) = mpsc::channel::<(usize, Value)>();
    let mut hs = vec![];
    for _ in 0..c["parallel"].as_u64().unwrap_or(4) as usize {
        let (q, tx, sampler) = (queue.clone(), tx.clone(), sampler.clone());
        let (prog, family, order, work, c) = (
            prog.clone(),
            family.clone(),
            order.clone(),
            work.clone(),
            c.clone(),
        );
        hs.push(std::thread::spawn(move || {
            loop {
                let next = q.lock().unwrap().pop_front();
                let Some((i, p)) = next else { break };
                let t0 = Instant::now();
                let recipe = PathBuf::from(p["recipe"].as_str().unwrap());
                let key: Vec<i64> = serde_json::from_value(p["key"].clone()).unwrap();
                let cfg = SolveRecipeConfig {
                    program: prog.clone(),
                    family: family.clone(),
                    order: order.clone(),
                    batch: BatchPolicy {
                        max_keys_per_job: 1,
                        max_threads: 1,
                        min_keys_per_thread: 1,
                    },
                    max_trace_rows: None,
                    key_budget_s: c["key_budget_s"].as_u64(),
                    expect: None,
                    determinism_id: None,
                    heartbeat_s: 10,
                    stop_grace_s: 30,
                    package: None,
                    live_repair: false,
                    licence_env: Some("SYMBOLICA_LICENSE".into()),
                    fake: None,
                };
                let ad = SolveRecipeAdapter::new(cfg);
                let reqs = vec![SolveRequest {
                    recipe: RecipeInput {
                        id: format!("p{i}"),
                        file: InputFile::from_file(&recipe).unwrap(),
                        digest: None,
                        sha256: None,
                    },
                    key: key.clone(),
                    tag: None,
                    context: None,
                }];
                let plan = ad.plan(&reqs).unwrap().remove(0);
                let opts = RunOptions {
                    cpus: vec![],
                    memory_cap_bytes: Some(c["mem_cap_gb"].as_u64().unwrap_or(8) << 30),
                    licence_env: Some("SYMBOLICA_LICENSE".into()),
                    wall_cap: Some(Duration::from_secs(c["wall_s"].as_u64().unwrap_or(7200))),
                    grace: Some(Duration::from_secs(30)),
                };
                let mut l = RealLauncher::new(sampler.clone()).expect("launcher (procfix7 round 19: fail-closed)");
                let wdir = work.join(format!("p{i}"));
                let line = match run_attempt(&mut l, &ad, &reqs, &plan, &wdir, 1, &opts) {
                    Err(e) => json!({"pair": p, "error": format!("{e:?}")}),
                    Ok((out, rep)) => {
                        let c = &out.classification;
                        let layout = RunLayout::new(&rep.prepared.spec.run_dir);
                        let rec = verify_stream_handle(
                            &layout.root,
                            &RunLayout::out_rel("results.jsonl"),
                            &["end"],
                        )
                        .ok()
                        .and_then(|(_, h)| h.canonical_lines().ok())
                        .and_then(|ls| {
                            ls.into_iter()
                                .filter_map(|b| serde_json::from_slice::<Value>(&b).ok())
                                .find(|v| v["key"] == json!(key) && v["verdict"] == "SOLVED")
                        });
                        let same = rec.as_ref().map(|r| r["relation_digest"] == p["relation_digest"]);
                        json!({"pair": p, "row": format!("{}{}", c.row, c.sub), "why": c.why,
                            "record": rec, "same_relation_digest_as_before": same,
                            "seconds": t0.elapsed().as_secs_f64(), "run_dir": rep.prepared.spec.run_dir})
                    }
                };
                if tx.send((i, line)).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);
    let mut f = std::fs::File::create(work.join("records.jsonl")).unwrap();
    for (i, l) in rx.iter() {
        println!(
            "pair {i}: row {} solved {}",
            l["row"],
            !l["record"].is_null()
        );
        writeln!(f, "{l}").unwrap();
    }
    for h in hs {
        let _ = h.join();
    }
}
