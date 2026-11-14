//! Lane closeloop3 (interfaces/closeloop.md 16): coefficient facts of a record with ELIDED witness loci from the
//! EXACT evaluation of its own coefficient text (`coefficient-facts` jobs, emulated by `rr_fake_child` with the campaign
//! core's exact evaluation), under a REAL `rr_campaign` supervisor. Spawns processes and sends signals: run ONLY inside
//! a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh closeloop3 test -p rustred-campaign --test
//! closeloop3_fake`).
//!
//! The node is the TOY of closeloop_support (arity 3) with its table rule REPLACED by a record whose witness loci are
//! elided, bound to an exact per-key proof (regularity), and whose right-hand side has a POSITIVE-numerator term with a
//! coefficient identically zero in d at the key: Tier C needs that zero fact; rr_proof instantiate would refuse the
//! record; the exact evaluation supplies it (`producer: exact_eval`).

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::state::ItemPhase;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

/// The elided record at the start key [5,1,0] (top 4): a lower term, a hand-down and a positive-numerator term whose
/// coefficient `((n0-5)*(d-2))/(d-1)` vanishes identically at the key.
fn elided_rule() -> String {
    json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","(d-4)/(d-3)"],["I(5,0,0)","2*n0"],["I(5,1,1)","((n0-5)*(d-2))/(d-1)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":["elided"]})
    .to_string()
}

fn sha(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

/// The toy material with the elided table rule; `exact`: the node's `proof.exact_eval` (fake child), with an optional
/// oracle of lies.
fn setup(tag: &str, exact: bool, lies: &[Value]) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = Toy {
        table_evidence: true,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let dir = &files.dir;
    let rule = elided_rule();
    let a = write_table(dir, "A.table.jsonl", std::slice::from_ref(&rule));
    // the record's exact per-key proof (regularity at the key); NO fact line: the exact tier has to ask for it
    std::fs::write(
        dir.join("proofs.json"),
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": sha(&rule), "key": [5,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]})
        .to_string(),
    )
    .unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    spec["sources"][0]["sha256"] = json!(sha256_file(&a));
    spec["evidence"] = json!({"proofs":[{"path": dir.join("proofs.json").display().to_string()}]});
    std::fs::write(
        &files.import_spec,
        serde_json::to_vec_pretty(&spec).unwrap(),
    )
    .unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    if exact {
        let lp = dir.join("exact-lies.jsonl");
        std::fs::write(
            &lp,
            lies.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        c["nodes"][0]["proof"]["exact_eval"] = json!({"program": "rr_coef_facts", "max_keys_per_job": 8,
            "wall_s": 60, "cap_bytes": 268435456, "expected_peak_bytes": 67108864,
            "fake": {"oracle": lp.display().to_string(), "ms": 20, "program": "rr_fake_child"}});
    }
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let mut spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    // the exact-evaluation program key must exist in the spec (its digests are recorded); the fake runs instead
    spec["programs"]["rr_coef_facts"] = json!({"path": FAKE});
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn statement_row(cdir: &Path, key: Value) -> Value {
    std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/statement.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["key"] == key)
        .unwrap_or_else(|| panic!("no statement row for {key}"))
}

fn blockers(r: &Value) -> Vec<String> {
    r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn an_elided_record_gets_exact_eval_facts_and_the_node_closes_boxed_per_key() {
    let _g = serial();
    let (_t, sp, cdir) = setup("cl3ok", true, &[]);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    assert_eq!(r["check"]["tier"], "C");
    // the fact of the elided record came from the exact evaluation, nothing else needed a fact line
    assert_eq!(r["fact_basis"], json!({"exact_eval": 1}), "{r:#}");
    // box clause view (closeloop.md 16.9): every start key listed with its clause; one verdict per clause
    assert_eq!(r["box_clause"], "box");
    assert_eq!(r["clauses"], json!({"box": "closed"}), "{r:#}");
    let start_keys = 2; // the toy's start set: [5,1,0] and [3,2,0]
    assert_eq!(r["clause_keys"]["count"], start_keys, "{r:#}");
    assert_eq!(r["clause_keys"]["not_certified"], 0);
    let ck = std::fs::read_to_string(cdir.join("closeloop/toy/final/clause_keys.jsonl")).unwrap();
    assert_eq!(ck.lines().count(), start_keys);
    assert!(ck.lines().all(|l| l.contains("\"clause\":\"box\"")), "{ck}");
    assert_eq!(r["check"]["exact_tier"]["exact_eval_missing_keys"], 0);
    assert_eq!(r["check"]["exact_tier"]["facts_unavailable_elided_keys"], 0);
    // the key reads per-key (its bound proof) with the fact basis exact_eval; the zero positive-numerator term is no
    // edge: [5,1,1] is not in the closure
    let row = statement_row(&cdir, json!([5, 1, 0]));
    assert_eq!(row["evidence"], "per-key", "{row}");
    assert_eq!(row["basis"], "per_key_proof", "{row}");
    assert_eq!(row["facts"]["basis"], "exact_eval", "{row}");
    assert_eq!(row["facts"]["coefficient_unknown"], false, "{row}");
    let stmt = std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/statement.jsonl"))
        .unwrap();
    assert!(!stmt.contains("[5,1,1]"), "the zero term was followed");
    // one coefficient-facts item, accepted; its recorded fact line states its provenance
    let st = state(&cdir);
    let ex: Vec<_> = st
        .items
        .iter()
        .filter(|(_, it)| it.kind == "proof-exact-eval")
        .collect();
    assert_eq!(ex.len(), 1, "{ex:?}");
    assert_eq!(ex[0].1.label, "exact:import_fact_request_exact_eval");
    assert_eq!(ex[0].1.phase.tag(), "accepted");
    let job = &ex[0].1.jobs[0];
    assert_eq!(st.jobs[job].kind, "coefficientfacts"); // the state's kind word (Debug, lower case)
    let facts_dir = cdir.join("closeloop/toy/facts");
    let f = std::fs::read_dir(&facts_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("toy.x")
                && p.extension().is_some_and(|x| x == "jsonl")
        })
        .expect("exact-eval fact file");
    let line: Value =
        serde_json::from_str(std::fs::read_to_string(&f).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(line["producer"], "exact_eval", "{line}");
    assert_eq!(line["verdict"], "hidden", "{line}");
    assert_eq!(line["record_sha256"], sha(&elided_rule()), "{line}");
    assert_eq!(line["n_terms"], 3);
    assert_eq!(line["zero_terms"], 1);
    assert_eq!(line["nonzero"].as_array().unwrap().len(), 2);
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn without_exact_eval_the_elided_record_keeps_its_key_uncertified() {
    let _g = serial();
    let (_t, sp, cdir) = setup("cl3none", false, &[]);
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&10),
        "{}",
        log_tail(&cdir)
    );
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    assert_eq!(r["check"]["exact_tier"]["facts_unavailable_elided_keys"], 1);
    let row = statement_row(&cdir, json!([5, 1, 0]));
    assert_eq!(row["facts"]["coefficient_unknown"], true, "{row}");
    assert!(row["facts"].get("basis").is_none(), "{row}");
    assert!(
        !state(&cdir)
            .items
            .values()
            .any(|it| it.kind == "proof-exact-eval")
    );
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn an_exact_eval_answer_for_another_record_or_without_its_producer_is_refused() {
    let _g = serial();
    for (tag, lie, why) in [
        (
            "cl3liesha",
            json!({"key": [5, 1, 0], "record_sha256": "0".repeat(64)}),
            "no request asked for",
        ),
        (
            "cl3lieprod",
            json!({"key": [5, 1, 0], "line": {"producer": "rr_proof"}}),
            "producer",
        ),
    ] {
        let (_t, sp, cdir) = setup(tag, true, std::slice::from_ref(&lie));
        assert_eq!(
            run_to_end(&sp, &cdir, &["--plain"]).last(),
            Some(&10),
            "{}",
            log_tail(&cdir)
        );
        let r = result_json(&cdir);
        assert_eq!(r["verdict"], "open", "{tag}: {r:#}");
        let b = blockers(&r);
        assert!(
            b.iter().any(|x| x == "proof_items_in_attention:1"),
            "{tag}: {b:?}"
        );
        let st = state(&cdir);
        let (_, it) = st
            .items
            .iter()
            .find(|(_, it)| it.kind == "proof-exact-eval")
            .expect("exact item");
        assert!(matches!(it.phase, ItemPhase::Attention(_)), "{tag}");
        assert!(it.accepted.is_none());
        for j in &it.jobs {
            let run = st.jobs[j].run_dir.clone().unwrap();
            let w = std::fs::read_to_string(Path::new(&run).join("sup/wrapper-refusal.txt"))
                .unwrap_or_default();
            assert!(w.contains(why), "{tag} {j}: wrapper refusal {w:?}");
        }
        // nothing of the refused answers reached the evidence: the key stays without a fact
        let row = statement_row(&cdir, json!([5, 1, 0]));
        assert_eq!(row["facts"]["coefficient_unknown"], true, "{tag}: {row}");
        assert_eq!(verify(&cdir), 0);
    }
}

/// The answer lines (solved-relation v1) of the node's solve items.
fn answer_lines(cdir: &Path) -> Vec<Value> {
    let mut v = vec![];
    for e in std::fs::read_dir(cdir.join("closeloop/toy/answers")).unwrap() {
        let p = e.unwrap().path();
        if p.to_string_lossy().ends_with(".keys.json")
            || p.to_string_lossy().ends_with(".retry.jsonl")
        {
            continue;
        }
        for l in std::fs::read_to_string(&p).unwrap().lines() {
            if !l.trim().is_empty() {
                v.push(serde_json::from_str(l).unwrap());
            }
        }
    }
    v
}

#[test]
fn a_key_over_a_per_key_cap_goes_to_the_next_recipe_step() {
    let _g = serial();
    // the m1_first S3 slowdown of closeloop2's run (a), in small: recipe R1 needs too long at [3,1,0] -> the per-key
    // cap stops it (wall: time-decided; work: work-decided); the key is asked of the NEXT candidate recipe (the
    // chart-wide default recipe), never of R1 again and never under another relation rule
    for (tag, verdict, work_cap) in [
        ("cl3capwall", "BUDGET_WALL", None),
        ("cl3capwork", "BUDGET_WORK", Some(5000u64)),
    ] {
        let t = Tmp::new(tag);
        let cfg = Toy {
            overrides: vec![
                ("R1".into(), [3, 1, 0], verdict.into(), vec![]),
                (
                    "default-recipe".into(),
                    [3, 1, 0],
                    "SOLVED".into(),
                    vec![[2, 1, 0], [3, 0, 0]],
                ),
            ],
            ..Toy::default()
        };
        let files = toy(&t.path.join("material"), &cfg, FAKE);
        std::fs::write(&files.hand, "").unwrap();
        if let Some(w) = work_cap {
            let mut c: Value =
                serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
            c["nodes"][0]["solve"]["max_trace_rows"] = json!(w);
            std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
        }
        let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
        let sp = write_spec(&t, "spec.json", &spec);
        let cdir = t.cdir("cdir");
        let codes = run_to_end(&sp, &cdir, &["--plain"]);
        assert_eq!(codes.last(), Some(&0), "{tag}: {}", log_tail(&cdir));
        let r = result_json(&cdir);
        assert_eq!(r["verdict"], "closed", "{tag}: {r:#}");
        // the cap hit is recorded on the key's answer line, R1 was asked once only at the key
        let ans = answer_lines(&cdir);
        let r1: Vec<&Value> = ans
            .iter()
            .filter(|v| v["source"] == "R1" && v["key"] == json!([3, 1, 0]))
            .collect();
        assert_eq!(r1.len(), 1, "{tag}: {r1:?}");
        assert_eq!(r1[0]["verdict"], verdict);
        let cap = &r1[0]["cap"];
        assert_eq!(cap["next"], "next_recipe_step", "{tag}: {cap}");
        if work_cap.is_none() {
            assert_eq!(cap["kind"], "wall", "{tag}: {cap}");
            assert_eq!(cap["time_decided"], true);
            assert_eq!(cap["key_budget_s"], 1800);
            assert_eq!(
                r["economy"]["cap_hits"],
                json!({"wall_time_decided": 1, "work": 0})
            );
        } else {
            assert_eq!(cap["kind"], "work", "{tag}: {cap}");
            assert_eq!(cap["time_decided"], false);
            assert_eq!(cap["max_trace_rows"], 5000);
            assert_eq!(
                r["economy"]["cap_hits"],
                json!({"wall_time_decided": 0, "work": 1})
            );
        }
        // the next step served it
        let dflt: Vec<&Value> = ans
            .iter()
            .filter(|v| v["source"] == "default-recipe" && v["key"] == json!([3, 1, 0]))
            .collect();
        assert_eq!(dflt.len(), 1, "{tag}");
        assert_eq!(dflt[0]["verdict"], "SOLVED");
        let row = statement_row(&cdir, json!([3, 1, 0]));
        assert!(
            row["server"].to_string().contains("default-recipe"),
            "{tag}: {row}"
        );
        // every solve job carried the caps as data (default wall cap 1,800 s)
        let st = state(&cdir);
        let mut solve_jobs = 0;
        for j in st.jobs.values().filter(|j| j.kind == "solverecipe") {
            let run = j.run_dir.clone().unwrap();
            let spec: Value =
                serde_json::from_slice(&std::fs::read(Path::new(&run).join("job.json")).unwrap())
                    .unwrap();
            let args: Vec<String> = serde_json::from_value(spec["args"].clone()).unwrap();
            let at = args
                .iter()
                .position(|a| a == "--key-budget-s")
                .expect("wall cap");
            assert_eq!(args[at + 1], "1800");
            if let Some(w) = work_cap {
                let at = args
                    .iter()
                    .position(|a| a == "--max-trace-rows")
                    .expect("work cap");
                assert_eq!(args[at + 1], w.to_string());
            }
            solve_jobs += 1;
        }
        assert!(solve_jobs >= 3, "{tag}: {solve_jobs}");
        assert_eq!(verify(&cdir), 0);
    }
}

#[test]
fn a_key_typed_error_or_irregular_is_retried_then_in_attention_never_moved_to_the_next_recipe() {
    let _g = serial();
    // engfix4's engine types a key whose sub-solve FAILED (resource limit, irregular lift, internal error) ERROR /
    // IRREGULAR: a per-key failure, retried in a later round, then attention; never "no relation" (the default recipe,
    // the key's next candidate, must never be asked for it)
    for (tag, verdict) in [("cl3err", "ERROR"), ("cl3irr", "IRREGULAR")] {
        let t = Tmp::new(tag);
        let cfg = Toy {
            overrides: vec![("R1".into(), [3, 1, 0], verdict.into(), vec![])],
            ..Toy::default()
        };
        let files = toy(&t.path.join("material"), &cfg, FAKE);
        std::fs::write(&files.hand, "").unwrap();
        let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
        let sp = write_spec(&t, "spec.json", &spec);
        let cdir = t.cdir("cdir");
        let codes = run_to_end(&sp, &cdir, &["--plain"]);
        assert_eq!(codes.last(), Some(&10), "{tag}: {}", log_tail(&cdir));
        let r = result_json(&cdir);
        assert_eq!(r["verdict"], "open", "{tag}: {r:#}");
        let b = blockers(&r);
        assert!(
            b.iter()
                .any(|x| x == "solve_keys_engine_failure_attention:1"),
            "{tag}: {b:?}"
        );
        let att = &r["check"]["engine_failures"]["attention"];
        assert_eq!(
            att,
            &json!([{"source": "R1", "key": [3, 1, 0], "verdict": verdict, "attempts": 2}]),
            "{tag}"
        );
        // no answer line for the key at all (an answer would have moved it on), never asked of the default recipe
        let ans = answer_lines(&cdir);
        assert!(
            !ans.iter().any(|v| v["key"] == json!([3, 1, 0])),
            "{tag}: {ans:?}"
        );
        // R1 was asked twice for the key (the retry), each failure recorded with its R48-3b record
        let mut retries = vec![];
        for e in std::fs::read_dir(cdir.join("closeloop/toy/answers")).unwrap() {
            let p = e.unwrap().path();
            if p.to_string_lossy().ends_with(".retry.jsonl") {
                for l in std::fs::read_to_string(&p).unwrap().lines() {
                    retries.push(serde_json::from_str::<Value>(l).unwrap());
                }
            }
        }
        assert_eq!(retries.len(), 2, "{tag}: {retries:?}");
        for v in &retries {
            assert_eq!(v["verdict"], verdict);
            assert_eq!(v["source"], "R1");
            assert_eq!(v["retry"]["route"], "retry_then_attention");
            assert!(v["failure_record"].is_object());
        }
        // the open list names the engine failure
        let open: Vec<Value> =
            std::fs::read_to_string(cdir.join("closeloop/toy/final/open_keys.jsonl"))
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
        let o = open
            .iter()
            .find(|v| v["key"] == json!([3, 1, 0]))
            .expect("open");
        assert_eq!(o["engine_failure"][0]["verdict"], verdict, "{tag}: {o}");
        assert_eq!(verify(&cdir), 0);
    }
}

#[test]
fn the_run_exit_code_follows_the_closing_loops_verdicts_closed_open_error() {
    let _g = serial();
    // procfix5 (supervisor.md 19.4) + lane closeloop3: CloseFromMaterialSource::outcome from the nodes' final verdicts:
    // closed -> 0 (the other tests), open -> 10 (the other tests), ERROR -> 11: a hand statement that cannot be read
    // makes phase D fail (verdict error, gate4 m3 / E6: it exited 0 before)
    let t = Tmp::new("cl3exit");
    let files = toy(&t.path.join("material"), &Toy::default(), FAKE);
    let _ = std::fs::remove_file(&files.hand); // never written: the hand statement is missing
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&11), "{codes:?} {}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "error", "{r:#}");
    assert_eq!(verify(&cdir), 0);
    // the same material with the hand statement present closes: exit 0
    let t2 = Tmp::new("cl3exit0");
    let files2 = toy(&t2.path.join("material"), &Toy::default(), FAKE);
    std::fs::write(&files2.hand, "").unwrap();
    let spec2 = campaign_spec(&t2.campaign, &files2, FAKE, &cpus());
    let sp2 = write_spec(&t2, "spec.json", &spec2);
    let cdir2 = t2.cdir("cdir");
    assert_eq!(run_to_end(&sp2, &cdir2, &["--plain"]).last(), Some(&0));
    assert_eq!(result_json(&cdir2)["verdict"], "closed");
}
