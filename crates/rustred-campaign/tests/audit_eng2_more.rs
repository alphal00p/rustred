//! Lane audit-eng2, further probes of the real child: planted outputs (M6 F.subcases, results temp file), the
//! relation rule of solve-recipe against the frozen engine-line4 binary, options-off identity of legacy `generate`
//! against the frozen binary, and `domain_bounds` (engine first match / domaincheck vs the campaign core's domain code).
//! All `#[ignore]`; env as the other audit_eng2 tests, plus `RR_FROZEN_BIN` (frozen engine-line4 rr_node).

mod audit_eng2_common;
use audit_eng2_common::*;

// the campaign core's import module (exported since; lane engfix4 replaced the audit's path-include shim, which no
// longer compiles against the current core)
use rustred_campaign_core::{cover, import};

use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn frozen() -> PathBuf {
    PathBuf::from(std::env::var("RR_FROZEN_BIN").expect("RR_FROZEN_BIN"))
}

fn note(v: Value) {
    eprintln!("PROBE {v}");
    if let Ok(p) = std::env::var("RR_AUDIT_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let _ = writeln!(f, "{v}");
        }
    }
}

fn l4a(args: &[&str]) -> Fx {
    Fx {
        tag: "l4a",
        node: "L4a",
        args: args.iter().map(|s| s.to_string()).collect(),
        cases: vec![],
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: false,
    }
}

/// M6: a failure to create `out/F.subcases.jsonl` (planted while the job runs: a file, then a symlink) must be an
/// output failure (51 / 50), never swallowed, never a success.
#[test]
#[ignore]
fn audit_eng2_m6_subcases_create_failure() {
    let r = root("m6");
    let mut problems = Vec::new();
    for (k, how) in ["file", "symlink"].iter().enumerate() {
        let s = job_spec(&r, &format!("m6-{k}"), 1, &l4a(&["--order", "id"]), 0);
        let mut c = spawn(&s);
        let out = RunLayout::new(&s.run_dir).out_dir();
        let t = Instant::now();
        while units(&events_of(&s), false) < 5 && t.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(1));
        }
        let p = out.join("F.subcases.jsonl");
        if *how == "file" {
            std::fs::write(&p, b"{\"type\":\"end\",\"subcases\":0}\n").unwrap();
        } else {
            std::os::unix::fs::symlink(r.join("elsewhere.jsonl"), &p).unwrap();
        }
        let w = wait(&mut c);
        let cl = classified(&s, w, KillReason::None);
        let o = outcome(&s);
        note(
            json!({"probe": "m6_subcases", "how": how, "wait": format!("{w:?}"), "row": cl.row,
                    "accept": format!("{:?}", cl.accept), "why": cl.why,
                    "error": o.as_ref().and_then(|o| o.job.error.as_ref().map(|e| format!("{:?}: {}", e.class, e.message))),
                    "elsewhere_written": r.join("elsewhere.jsonl").exists()}),
        );
        if cl.accept != Accept::Nothing
            || matches!(w, WaitStatus::Exited(0) | WaitStatus::Exited(10))
        {
            problems.push(format!(
                "{how}: planted F.subcases -> {w:?} row {} accept {:?}",
                cl.row, cl.accept
            ));
        }
        if r.join("elsewhere.jsonl").exists() {
            problems.push(format!("{how}: the child wrote through a planted symlink"));
        }
    }
    assert!(problems.is_empty(), "{problems:?}");
}

// ---------------------------------------------------------------------------------------------------------------

fn recipe_fixtures() -> PathBuf {
    std::env::var("RR_RECIPE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve",
            )
        })
}

fn recipe_spec(root: &Path, job: &str, rid: &str, keys: &[Value], rule: &str) -> JobSpec {
    use rustred_campaign_contract::spec::*;
    let run = rustred_campaign_contract::layout::attempt_dir(root, job, 1);
    std::fs::create_dir_all(run.join("in")).unwrap();
    let fx = recipe_fixtures();
    let fam = fx.join("T178.sym");
    let mut r: Value = serde_json::from_str(
        &std::fs::read_to_string(fx.join("recipes").join(format!("{rid}.json"))).unwrap(),
    )
    .unwrap();
    r["relation_rule"] = json!(rule);
    let rec = run.join("in/recipe.json");
    std::fs::write(&rec, serde_json::to_string(&r).unwrap()).unwrap();
    let kp = run.join("in/keys.jsonl");
    let text: String = keys.iter().map(|k| format!("{k}\n")).collect();
    std::fs::write(&kp, text).unwrap();
    let order = std::fs::read_to_string(fx.join("order.json")).unwrap();
    let input = |role: &str, path: String, file: &Path| InputRef {
        role: role.into(),
        path,
        blake3: b3(file),
        upto: None,
    };
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job.into(),
        attempt: 1,
        kind: JobKind::SolveRecipe,
        run_dir: run.to_str().unwrap().into(),
        program: ProgramRef {
            name: "rr_node".into(),
            blake3: b3(&bin()),
            features_required: vec!["job".into(), "solve_recipe".into()],
        },
        inputs: vec![
            input("family", fam.to_str().unwrap().into(), &fam),
            input("recipe", rec.to_str().unwrap().into(), &rec),
            input("keys", "in/keys.jsonl".into(), &kp),
        ],
        args: vec!["--order".into(), order, "--threads".into(), "1".into()],
        seed: SeedSpec { sample_seed: 0 },
        budgets: Budgets::default(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 1,
        stop_grace_s: 5,
        resume_every_s: 0,
        resume: None,
        env_allow: vec![],
        fake: None,
    }
}

/// (key, verdict, relation_digest, relation_rule, relation_rule_taken, m1_steps, rhs) per relation record.
fn relation_rows(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "relation")
        .map(|v| {
            json!({"key": v["key"], "verdict": v["verdict"], "relation_digest": v["relation_digest"],
                   "relation_rule": v["relation_rule"], "relation_rule_taken": v["relation_rule_taken"],
                   "m1_steps": v["m1_steps"], "rhs": v["rhs"], "all_lower": v["all_lower"],
                   "proof_ok": v["proof"]["ok"]})
        })
        .collect()
}

/// Item 4: `relation_rule` recorded truthfully: each rule (m1_first, first_admissible) through `rr_node job` against the
/// frozen engine-line4 binary's legacy form on the same recipe file and keys: verdict, relation digest, rule taken,
/// m1 steps and rhs per key must agree, and OUTCOME must name the rule.
#[test]
#[ignore]
fn audit_eng2_relation_rule_vs_frozen() {
    let r = root("relrule");
    let mut problems = Vec::new();
    let sets: Vec<(String, usize, Vec<&str>)> = vec![
        (
            "g026_d2_ccomb_fon_sfirst".into(),
            2,
            vec!["m1_first", "first_admissible"],
        ),
        (
            "results_13_x8_C_json".into(),
            2,
            vec!["first_admissible", "m1_first"],
        ),
        (
            "g038_d2_cL1_fon_sfirst".into(),
            env_u64("RR_AUDIT_G038_KEYS", 1) as usize,
            vec!["m1_first"],
        ),
    ];
    let mut compared = 0;
    for (rid, nkeys, rules) in sets {
        let keys: Vec<Value> =
            std::fs::read_to_string(recipe_fixtures().join("keys").join(format!("{rid}.jsonl")))
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).unwrap())
                .take(nkeys)
                .collect();
        for rule in rules {
            let job = format!("{}-{rule}", &rid[..8]);
            let s = recipe_spec(&r, &job, &rid, &keys, rule);
            let t = Instant::now();
            let mut c = spawn(&s);
            let w = wait(&mut c);
            let ours_s = t.elapsed().as_secs_f64();
            let cl = classified(&s, w, KillReason::None);
            let o = outcome(&s);
            let ours = relation_rows(&RunLayout::new(&s.run_dir).out_dir().join("results.jsonl"));
            // the frozen binary, legacy form, same recipe + keys files
            let fout = r.join(format!("{job}.frozen"));
            let order = std::fs::read_to_string(recipe_fixtures().join("order.json")).unwrap();
            let t = Instant::now();
            let st = std::process::Command::new(frozen())
                .arg("solve-recipe")
                .arg("--family")
                .arg(recipe_fixtures().join("T178.sym"))
                .arg("--order")
                .arg(order.trim())
                .arg("--recipe")
                .arg(RunLayout::new(&s.run_dir).root.join("in/recipe.json"))
                .arg("--keys")
                .arg(RunLayout::new(&s.run_dir).root.join("in/keys.jsonl"))
                .arg("--out")
                .arg(&fout)
                .arg("--threads")
                .arg("1")
                .env("RAYON_NUM_THREADS", "1")
                .env("SYMBOLICA_HIDE_BANNER", "1")
                .stdout(std::process::Stdio::null())
                .stderr(std::fs::File::create(r.join(format!("{job}.frozen.stderr"))).unwrap())
                .status()
                .unwrap();
            let frozen_s = t.elapsed().as_secs_f64();
            let theirs = relation_rows(&fout.join("results.jsonl"));
            let outcome_rule = o.as_ref().map(|o| o.legacy.get("relation_rule").cloned());
            let eff_rule = o.as_ref().and_then(|o| {
                o.job
                    .effective
                    .as_ref()
                    .map(|e| e.node_options["relation_rule"].clone())
            });
            let equal = ours == theirs && !ours.is_empty();
            compared += ours.len();
            note(
                json!({"probe": "relation_rule", "recipe": rid, "rule": rule, "keys": keys.len(),
                        "ours": {"wait": format!("{w:?}"), "row": cl.row, "accept": format!("{:?}", cl.accept), "seconds": ours_s,
                                 "outcome_relation_rule": outcome_rule, "effective_relation_rule": eff_rule},
                        "frozen": {"exit": st.code(), "seconds": frozen_s},
                        "records_equal": equal,
                        "summary": ours.iter().zip(theirs.iter()).map(|(a, b)| json!([a["verdict"], a["relation_rule_taken"], a["m1_steps"], a["relation_digest"] == b["relation_digest"], b["verdict"]])).collect::<Vec<_>>()}),
            );
            if !equal {
                problems.push(format!(
                    "{rid} {rule}: records differ from the frozen binary ({} vs {} records)",
                    ours.len(),
                    theirs.len()
                ));
            }
            if ours.iter().any(|v| v["relation_rule"] != json!(rule)) {
                problems.push(format!(
                    "{rid} {rule}: a record names another relation_rule"
                ));
            }
            if eff_rule != Some(json!(rule)) {
                problems.push(format!(
                    "{rid} {rule}: OUTCOME effective.node_options.relation_rule = {eff_rule:?}"
                ));
            }
        }
    }
    eprintln!(
        "relation records compared: {compared}; problems {}",
        problems.len()
    );
    assert!(problems.is_empty(), "{problems:?}");
}

// ---------------------------------------------------------------------------------------------------------------

/// The queue log without its clock members (any key ending in `seconds`, `t`, and the GuardReport timing text).
fn queue_without_clock(p: &Path) -> Vec<Value> {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.retain(|k, _| !(k.ends_with("seconds") || k == "t"));
                for (_, x) in m.iter_mut() {
                    strip(x);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            Value::String(s) if s.contains("GuardReport") => {
                *s = s
                    .split(", ")
                    .filter(|p| !p.contains("seconds"))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            _ => {}
        }
    }
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            strip(&mut v);
            v
        })
        .collect()
}

/// Item 7: Options-off identity of the LEGACY invocation (`generate --family F --out DIR --order id --case-budget 300`)
/// against the frozen engine-line4 binary on the two smallest fixtures: F.table and F.cert byte-identical, F.queue
/// identical without its clock members, same exit code.
#[test]
#[ignore]
fn audit_eng2_legacy_identity_vs_frozen() {
    let r = root("legacy");
    let mut problems = Vec::new();
    for node in ["TOY2kite", "L3a"] {
        let mut res = Vec::new();
        for (who, b) in [("ours", bin()), ("frozen", frozen())] {
            let out = r.join(format!("{node}.{who}"));
            let st = std::process::Command::new(&b)
                .arg("generate")
                .arg("--family")
                .arg(family(node))
                .arg("--out")
                .arg(&out)
                .arg("--order")
                .arg("id")
                .arg("--case-budget")
                .arg("300")
                .env("RAYON_NUM_THREADS", "1")
                .env("SYMBOLICA_HIDE_BANNER", "1")
                .stdout(std::fs::File::create(r.join(format!("{node}.{who}.stdout"))).unwrap())
                .stderr(std::fs::File::create(r.join(format!("{node}.{who}.stderr"))).unwrap())
                .status()
                .unwrap();
            res.push((st.code(), out));
        }
        let (a, b) = (&res[0], &res[1]);
        // byte-identical except the volatile timing members (canonical form of the contract: seconds_search,
        // lift_seconds, seconds, vmhwm_kb); the header's family_file is the same path in both runs
        let same = |f: &str| {
            let c = |p: PathBuf| std::fs::read(p).ok().map(|b| canonical_lines(&b));
            c(a.1.join(f)) == c(b.1.join(f))
        };
        let lines = |f: &str| {
            std::fs::read_to_string(a.1.join(f))
                .map(|s| s.lines().count())
                .unwrap_or(0)
        };
        let q = queue_without_clock(&a.1.join("F.queue.jsonl"))
            == queue_without_clock(&b.1.join("F.queue.jsonl"));
        // the table header names the family file path; both runs use the same path
        let v = json!({"probe": "legacy_identity", "node": node, "exit": [a.0, b.0], "table_identical": same("F.table.jsonl"),
                       "table_lines": lines("F.table.jsonl"), "cert_identical": same("F.cert.jsonl"), "cert_lines": lines("F.cert.jsonl"),
                       "queue_identical_without_clock": q, "queue_lines": lines("F.queue.jsonl")});
        note(v.clone());
        if a.0 != b.0 || !same("F.table.jsonl") || !same("F.cert.jsonl") || !q {
            problems.push(v);
        }
    }
    assert!(problems.is_empty(), "{problems:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// 5. domain_bounds

fn bounds_from_recenter(chart: &str, rc: &[i64]) -> Option<Value> {
    let line: Vec<bool> = chart.chars().map(|c| c == '1').collect();
    if rc.len() != line.len() {
        return None;
    }
    let mut min = vec![Value::Null; line.len()];
    let mut max = vec![Value::Null; line.len()];
    let mut any = false;
    for i in 0..line.len() {
        if line[i] && 1 - rc[i] > 1 {
            min[i] = json!(1 - rc[i]);
            any = true;
        } else if !line[i] && -rc[i] < 0 {
            max[i] = json!(-rc[i]);
            any = true;
        }
    }
    any.then(|| json!({"min": min, "max": max}))
}

fn engine_first_match(b: &Path, table: &Path, keys: &Path) -> Vec<Value> {
    let out = std::process::Command::new(b)
        .arg("first-match")
        .arg("--table")
        .arg(table)
        .arg("--keys")
        .arg(keys)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "first-match: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["label"].clone())
        .collect()
}

fn consumer_first_match(
    table: &Path,
    qualified: bool,
    keys: &[Vec<i64>],
    scratch: &Path,
) -> Vec<Value> {
    let st = scratch.join("start.jsonl");
    std::fs::write(&st, format!("{}\n", json!(keys[0]))).unwrap();
    let mut src = json!({"id": "T", "kind": "engine_table", "path": table.display().to_string()});
    if qualified {
        src["domain"] = json!("qualified_recenter");
    }
    let spec = json!({"schema": "rustred.campaign.import-spec.v1", "frame_from": "T",
                      "policies": {"witness_absent": "block"},
                      "start": {"path": st.display().to_string(), "format": "keys_jsonl"}, "sources": [src]});
    let sp = import::TableSetSpec::from_json(&spec.to_string())
        .unwrap_or_else(|e| panic!("spec: {e:?}"));
    let set =
        import::load(&sp, scratch).unwrap_or_else(|e| panic!("load {}: {e:?}", table.display()));
    keys.iter()
        .map(|k| {
            let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
            match set.disp.first_match(&k32) {
                Ok(cover::Dispatch::Covered(r)) => json!(r.label),
                Ok(cover::Dispatch::Uncovered) => Value::Null,
                Ok(cover::Dispatch::Blocked { at, reason }) => {
                    json!({"blocked_at": at.label, "reason": format!("{reason:?}")})
                }
                Ok(other) => json!({"other": format!("{other:?}")}),
                Err(e) => json!({"error": format!("{e:?}")}),
            }
        })
        .collect()
}

/// Keys around the domain boundary of records with a non-trivial recenter: free slots at the chart minimum (lines 1,
/// numerators 0), fixed slots at their values, each bounded slot at its bound and one step outside.
fn boundary_keys(table: &Path, limit: usize) -> Vec<Vec<i64>> {
    let mut keys = std::collections::BTreeSet::new();
    for l in std::fs::read_to_string(table).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        if v["type"] != "rule" {
            continue;
        }
        let chart = v["chart"].as_str().unwrap_or("");
        let rc: Vec<i64> = v["recenter"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect())
            .unwrap_or_default();
        let Some(b) = bounds_from_recenter(chart, &rc) else {
            continue;
        };
        let fixed: Vec<Option<i64>> = v["fixed"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_i64)
            .collect();
        if !v["equations"].as_array().is_none_or(|e| e.is_empty()) {
            continue;
        }
        let base: Vec<i64> = chart
            .chars()
            .enumerate()
            .map(|(i, c)| fixed[i].unwrap_or(if c == '1' { 1 } else { 0 }))
            .collect();
        for i in 0..base.len() {
            if fixed[i].is_some() {
                continue;
            }
            for bound in [b["min"][i].as_i64(), b["max"][i].as_i64()]
                .into_iter()
                .flatten()
            {
                let outside = if chart.as_bytes()[i] == b'1' {
                    bound - 1
                } else {
                    bound + 1
                };
                for val in [bound, outside] {
                    let mut k = base.clone();
                    // every bounded slot at its bound, slot i at `val`
                    for j in 0..k.len() {
                        if fixed[j].is_none() && j != i {
                            if let Some(m) = b["min"][j].as_i64() {
                                k[j] = m;
                            }
                            if let Some(m) = b["max"][j].as_i64() {
                                k[j] = m;
                            }
                        }
                    }
                    k[i] = val;
                    keys.insert(k);
                }
            }
        }
        if keys.len() >= limit {
            break;
        }
    }
    keys.into_iter().collect()
}

/// Random keys near the chart minimum (lines 1..3, numerators -2..0), drawn from the table's chart.
fn random_keys(table: &Path, n: usize, seed: u64) -> Vec<Vec<i64>> {
    let chart = std::fs::read_to_string(table)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["type"] == "rule")
        .and_then(|v| v["chart"].as_str().map(str::to_string))
        .unwrap_or_default();
    let mut rng = Rng(seed ^ 0xB0D5);
    (0..n)
        .map(|_| {
            chart
                .chars()
                .map(|c| {
                    if c == '1' {
                        1 + rng.below(3) as i64
                    } else {
                        -(rng.below(3) as i64)
                    }
                })
                .collect()
        })
        .collect()
}

/// Item 5: The engine's first match (and domaincheck) against the campaign core's domain code on the T162 bank set
/// (consumer side: `domain: qualified_recenter`), with and without `domain_bounds` written into the records.
#[test]
#[ignore]
fn audit_eng2_domain_bounds_first_match() {
    let r = root("bounds");
    let bank = PathBuf::from(std::env::var("RR_AUDIT_BANK").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/bank-conv/20261114c/qualified".into()
    }));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&bank)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    let nfiles = env_u64("RR_AUDIT_BANK_FILES", 3) as usize;
    let mut problems = Vec::new();
    let mut totals = json!({"keys": 0, "bounded_records": 0, "eng_b_vs_cons_q_diff": 0, "eng_b_vs_cons_b_diff": 0,
                            "eng_plain_vs_cons_plain_diff": 0, "eng_plain_vs_cons_q_diff": 0, "domaincheck_vs_first_match_diff": 0});
    for f in files.iter().take(nfiles) {
        // a copy with domain_bounds written from recenter (the engine helper's formula)
        let name = f.file_name().unwrap().to_str().unwrap().to_string();
        let tb = r.join(format!("bounded.{name}"));
        let mut text = String::new();
        let mut nb = 0;
        for l in std::fs::read_to_string(f).unwrap().lines() {
            let mut v: Value = serde_json::from_str(l).unwrap();
            if v["type"] == "rule" {
                let rc: Vec<i64> = v["recenter"]
                    .as_array()
                    .map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect())
                    .unwrap_or_default();
                if let Some(b) = bounds_from_recenter(v["chart"].as_str().unwrap_or(""), &rc) {
                    v["domain_bounds"] = b;
                    nb += 1;
                }
            }
            text.push_str(&serde_json::to_string(&v).unwrap());
            text.push('\n');
        }
        std::fs::write(&tb, text).unwrap();
        let mut keys = boundary_keys(f, env_u64("RR_AUDIT_BOUND_KEYS", 400) as usize);
        keys.extend(random_keys(
            f,
            env_u64("RR_AUDIT_RANDOM_KEYS", 1500) as usize,
            keys.len() as u64 + 7,
        ));
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            continue;
        }
        let kp = r.join(format!("keys.{name}"));
        std::fs::write(
            &kp,
            keys.iter()
                .map(|k| format!("{}\n", json!(k)))
                .collect::<String>(),
        )
        .unwrap();
        let sd = r.join(format!("scratch.{name}"));
        std::fs::create_dir_all(&sd).unwrap();
        // derived keys: from every key whose plain engine winner carries bounds, each bounded slot moved one step
        // outside the bound (the keys where the plain dispatch applies the record outside its qualified domain)
        let bounds_of: std::collections::BTreeMap<String, Value> = std::fs::read_to_string(&tb)
            .unwrap()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v["type"] == "rule" && v.get("domain_bounds").is_some())
            .map(|v| (v["index"].to_string(), v["domain_bounds"].clone()))
            .collect();
        let first = engine_first_match(&bin(), f, &kp);
        let mut derived = Vec::new();
        for (k, lab) in keys.iter().zip(&first) {
            let Some(b) = bounds_of.get(&lab.to_string()) else {
                continue;
            };
            for i in 0..k.len() {
                if let Some(m) = b["min"][i].as_i64().filter(|&m| k[i] >= m && m > 1) {
                    let mut x = k.clone();
                    x[i] = m - 1;
                    derived.push(x);
                }
                if let Some(m) = b["max"][i].as_i64().filter(|&m| k[i] <= m && m < 0) {
                    let mut x = k.clone();
                    x[i] = m + 1;
                    derived.push(x);
                }
            }
        }
        let n_derived = derived.len();
        keys.extend(derived);
        keys.sort();
        keys.dedup();
        std::fs::write(
            &kp,
            keys.iter()
                .map(|k| format!("{}\n", json!(k)))
                .collect::<String>(),
        )
        .unwrap();
        let eng_b = engine_first_match(&bin(), &tb, &kp);
        let eng_p = engine_first_match(&bin(), f, &kp);
        let cons_q = consumer_first_match(f, true, &keys, &sd);
        let cons_b = consumer_first_match(&tb, false, &keys, &sd);
        let cons_p = consumer_first_match(f, false, &keys, &sd);
        // domaincheck on the bounded table
        let dout = r.join(format!("dc.{name}"));
        let st = std::process::Command::new(bin())
            .args(["domaincheck", "--table"])
            .arg(&tb)
            .arg("--keys")
            .arg(&kp)
            .arg("--out")
            .arg(&dout)
            .env("SYMBOLICA_HIDE_BANNER", "1")
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        let dc: Vec<Value> = std::fs::read_to_string(&dout)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v["type"] == "key")
            .map(|v| v["selected"]["index"].clone())
            .collect();
        let sel = |x: &Value| -> Value {
            if let Some(b) = x.get("blocked_at") {
                b.clone()
            } else {
                x.clone()
            }
        };
        let count =
            |a: &[Value], b: &[Value]| a.iter().zip(b).filter(|(x, y)| sel(x) != sel(y)).count();
        let d1 = count(&eng_b, &cons_q);
        let d2 = count(&eng_b, &cons_b);
        let d3 = count(&eng_p, &cons_p);
        let d4 = count(&eng_p, &cons_q);
        let d5 = if dc.len() == eng_b.len() {
            count(&dc, &eng_b)
        } else {
            usize::MAX
        };
        let blocked = cons_q
            .iter()
            .filter(|x| x.get("blocked_at").is_some())
            .count();
        let matter = eng_b.iter().zip(&eng_p).filter(|(x, y)| x != y).count();
        let reasons: std::collections::BTreeMap<String, usize> = cons_q
            .iter()
            .filter_map(|x| x.get("reason").and_then(Value::as_str))
            .fold(Default::default(), |mut m, r| {
                *m.entry(r.chars().take(40).collect()).or_default() += 1;
                m
            });
        let mut examples = Vec::new();
        for (i, k) in keys.iter().enumerate() {
            if sel(&eng_b[i]) != sel(&cons_q[i]) || sel(&eng_p[i]) != sel(&cons_p[i]) {
                examples.push(json!({"key": k, "eng_bounded": eng_b[i], "cons_qualified": cons_q[i], "eng_plain": eng_p[i], "cons_plain": cons_p[i]}));
                if examples.len() >= 4 {
                    break;
                }
            }
        }
        let first_plain_vs_q: Vec<Value> = keys
            .iter()
            .enumerate()
            .filter(|(i, _)| sel(&eng_p[*i]) != sel(&cons_q[*i]))
            .take(2)
            .map(|(i, k)| json!({"key": k, "eng_plain": eng_p[i], "cons_qualified": cons_q[i]}))
            .collect();
        note(
            json!({"probe": "domain_bounds", "file": name, "bounded_records": nb, "keys": keys.len(),
                    "eng_bounded_vs_cons_qualified_diff": d1, "eng_bounded_vs_cons_bounded_diff": d2,
                    "eng_plain_vs_cons_plain_diff": d3, "eng_plain_vs_cons_qualified_diff": d4,
                    "domaincheck_vs_first_match_diff": d5, "domaincheck_exit": st.code(), "cons_blocked": blocked,
                    "keys_where_bounds_change_the_engine_match": matter, "blocked_reasons": reasons, "derived_keys": n_derived,
                    "examples": examples, "plain_vs_qualified_examples": first_plain_vs_q}),
        );
        for (k, v) in [
            ("keys", keys.len()),
            ("bounded_records", nb),
            ("eng_b_vs_cons_q_diff", d1),
            ("eng_b_vs_cons_b_diff", d2),
            ("eng_plain_vs_cons_plain_diff", d3),
            ("eng_plain_vs_cons_q_diff", d4),
            ("domaincheck_vs_first_match_diff", d5),
        ] {
            totals[k] = json!(totals[k].as_u64().unwrap_or(0).saturating_add(v as u64));
        }
        if d1 > 0 || d2 > 0 || d3 > 0 || d5 > 0 {
            problems.push(format!("{name}: disagreements eng_b/cons_q {d1}, eng_b/cons_b {d2}, plain {d3}, domaincheck {d5}"));
        }
    }
    // front-loaded bounded records: each bounded record R moved to the FRONT of its table (so that R is the plain
    // first match on its whole case domain) and queried inside its bounds and one step outside each bound
    let front = env_u64("RR_AUDIT_BOUNDS_FRONT", 30) as usize;
    let mut fstats = json!({"records": 0, "keys": 0, "inside_R_engine_bounded": 0, "outside_R_engine_plain": 0,
                            "outside_R_engine_bounded": 0, "eng_b_vs_cons_b_diff": 0, "eng_b_vs_cons_q_diff": 0, "eng_p_vs_cons_p_diff": 0});
    let f0 = files.first().unwrap();
    let text = std::fs::read_to_string(f0).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let bounded: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v["type"] == "rule" && v["equations"].as_array().is_none_or(|e| e.is_empty())
        })
        .filter(|(_, v)| {
            let rc: Vec<i64> = v["recenter"]
                .as_array()
                .map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect())
                .unwrap_or_default();
            bounds_from_recenter(v["chart"].as_str().unwrap_or(""), &rc).is_some()
        })
        .map(|(i, _)| i)
        .collect();
    let step = (bounded.len() / front.max(1)).max(1);
    for &ri in bounded.iter().step_by(step).take(front) {
        let rv = &lines[ri];
        let rc: Vec<i64> = rv["recenter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap_or(0))
            .collect();
        let chart = rv["chart"].as_str().unwrap().to_string();
        let b = bounds_from_recenter(&chart, &rc).unwrap();
        let write = |name: &str, with_bounds: bool| -> PathBuf {
            let mut out = String::new();
            let mut rule_lines = Vec::new();
            for (i, v) in lines.iter().enumerate() {
                let mut v = v.clone();
                if v["type"] == "rule" {
                    if with_bounds {
                        let rc2: Vec<i64> = v["recenter"]
                            .as_array()
                            .map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0)).collect())
                            .unwrap_or_default();
                        if let Some(b2) =
                            bounds_from_recenter(v["chart"].as_str().unwrap_or(""), &rc2)
                        {
                            v["domain_bounds"] = b2;
                        }
                    }
                    if i == ri {
                        rule_lines.insert(0, v);
                    } else {
                        rule_lines.push(v);
                    }
                } else if rule_lines.is_empty() {
                    out.push_str(&serde_json::to_string(&v).unwrap());
                    out.push('\n');
                } else {
                    rule_lines.push(v);
                }
            }
            for v in rule_lines {
                out.push_str(&serde_json::to_string(&v).unwrap());
                out.push('\n');
            }
            let p = r.join(name);
            std::fs::write(&p, out).unwrap();
            p
        };
        let tb = write(&format!("front{ri}.bounded.jsonl"), true);
        let tp = write(&format!("front{ri}.plain.jsonl"), false);
        let fixed: Vec<Option<i64>> = rv["fixed"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_i64)
            .collect();
        let inside: Vec<i64> = chart
            .chars()
            .enumerate()
            .map(|(i, c)| {
                fixed[i].unwrap_or_else(|| {
                    b["min"][i]
                        .as_i64()
                        .or(b["max"][i].as_i64())
                        .unwrap_or(if c == '1' { 1 } else { 0 })
                })
            })
            .collect();
        let mut keys = vec![inside.clone()];
        for i in 0..inside.len() {
            if fixed[i].is_some() {
                continue;
            }
            if let Some(m) = b["min"][i].as_i64() {
                let mut x = inside.clone();
                x[i] = m - 1;
                keys.push(x);
            }
            if let Some(m) = b["max"][i].as_i64() {
                let mut x = inside.clone();
                x[i] = m + 1;
                keys.push(x);
            }
        }
        let kp = r.join(format!("front{ri}.keys.jsonl"));
        std::fs::write(
            &kp,
            keys.iter()
                .map(|k| format!("{}\n", json!(k)))
                .collect::<String>(),
        )
        .unwrap();
        let sd = r.join(format!("front{ri}.scratch"));
        std::fs::create_dir_all(&sd).unwrap();
        let eb = engine_first_match(&bin(), &tb, &kp);
        let ep = engine_first_match(&bin(), &tp, &kp);
        let cb = consumer_first_match(&tb, false, &keys, &sd);
        let cq = consumer_first_match(&tp, true, &keys, &sd);
        let cp = consumer_first_match(&tp, false, &keys, &sd);
        let sel = |x: &Value| -> Value {
            if let Some(b) = x.get("blocked_at") {
                b.clone()
            } else {
                x.clone()
            }
        };
        let rl = &rv["index"];
        let inc = |st: &mut Value, k: &str, by: usize| {
            st[k] = json!(st[k].as_u64().unwrap_or(0) + by as u64);
        };
        inc(&mut fstats, "records", 1);
        inc(&mut fstats, "keys", keys.len());
        inc(
            &mut fstats,
            "inside_R_engine_bounded",
            usize::from(eb[0] == *rl),
        );
        inc(
            &mut fstats,
            "outside_R_engine_plain",
            ep[1..].iter().filter(|x| *x == rl).count(),
        );
        inc(
            &mut fstats,
            "outside_R_engine_bounded",
            eb[1..].iter().filter(|x| *x == rl).count(),
        );
        let d =
            |a: &[Value], c: &[Value]| a.iter().zip(c).filter(|(x, y)| sel(x) != sel(y)).count();
        let (d1, d2, d3) = (d(&eb, &cb), d(&eb, &cq), d(&ep, &cp));
        inc(&mut fstats, "eng_b_vs_cons_b_diff", d1);
        inc(&mut fstats, "eng_b_vs_cons_q_diff", d2);
        inc(&mut fstats, "eng_p_vs_cons_p_diff", d3);
        if d1 + d2 + d3 > 0 || eb[1..].iter().any(|x| x == rl) {
            note(
                json!({"probe": "domain_bounds_front", "record": rl, "bounds": b, "keys": keys, "eng_bounded": eb, "eng_plain": ep,
                        "cons_bounded": cb, "cons_qualified": cq, "cons_plain": cp}),
            );
            problems.push(format!(
                "front-loaded record {rl}: disagreement or bound ignored"
            ));
        }
    }
    note(json!({"probe": "domain_bounds_front_totals", "stats": fstats}));
    note(json!({"probe": "domain_bounds_totals", "totals": totals}));
    assert!(problems.is_empty(), "{problems:?}");
}

/// 5 (semantics where the bounds matter): the L3a fixture table with `domain_bounds` added to rule 0 (a min on line
/// slot 0, a max on numerator slot 8) and rule 1 (a min on its FIXED slot 7 that the fixed value violates): the engine's
/// first match and the campaign core's dispatcher (explicit bounds) must agree, and both must differ from the plain
/// table exactly where a bound is violated. Then two edge tables where the two readers' REFUSAL rules differ.
#[test]
#[ignore]
fn audit_eng2_domain_bounds_synthetic_l3a() {
    let r = root("bounds-l3a");
    let src = fixtures().join("small/m2-slice/L3a/table.jsonl");
    let lines: Vec<Value> = std::fs::read_to_string(&src)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let write = |name: &str, f: &dyn Fn(&mut Value)| -> PathBuf {
        let p = r.join(name);
        let mut out = String::new();
        for v in &lines {
            let mut v = v.clone();
            if v["type"] == "rule" {
                f(&mut v);
            }
            out.push_str(&serde_json::to_string(&v).unwrap());
            out.push('\n');
        }
        std::fs::write(&p, out).unwrap();
        p
    };
    let n = |x: Option<i64>| x.map_or(Value::Null, |v| json!(v));
    let bounded = write("bounded.jsonl", &|v| {
        if v["index"] == 0 {
            let mut min = vec![Value::Null; 9];
            let mut max = vec![Value::Null; 9];
            min[0] = n(Some(3));
            max[8] = n(Some(-2));
            v["domain_bounds"] = json!({"min": min, "max": max});
        }
        if v["index"] == 1 {
            let mut min = vec![Value::Null; 9];
            min[7] = n(Some(2));
            v["domain_bounds"] = json!({"min": min, "max": Value::Null});
        }
    });
    let plain = src.clone();
    let keys: Vec<Vec<i64>> = vec![
        vec![3, 1, 1, 1, 1, 1, 1, 2, -2],
        vec![2, 1, 1, 1, 1, 1, 1, 2, -2],
        vec![1, 1, 1, 1, 1, 1, 1, 2, -2],
        vec![3, 1, 1, 1, 1, 1, 1, 2, -1],
        vec![4, 2, 1, 1, 1, 1, 1, 3, -3],
        vec![3, 1, 1, 1, 1, 1, 2, 1, -1],
        vec![1, 1, 1, 1, 1, 1, 2, 1, 0],
        vec![2, 2, 2, 1, 1, 1, 2, 1, -2],
    ];
    let kp = r.join("keys.jsonl");
    std::fs::write(
        &kp,
        keys.iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let sd = r.join("scratch");
    std::fs::create_dir_all(&sd).unwrap();
    let eb = engine_first_match(&bin(), &bounded, &kp);
    let ep = engine_first_match(&bin(), &plain, &kp);
    let cb = consumer_first_match(&bounded, false, &keys, &sd);
    let cp = consumer_first_match(&plain, false, &keys, &sd);
    let sel = |x: &Value| -> Value {
        if let Some(b) = x.get("blocked_at") {
            b.clone()
        } else {
            x.clone()
        }
    };
    let rows: Vec<Value> = keys
        .iter()
        .enumerate()
        .map(|i_k| {
            let (i, k) = i_k;
            json!({"key": k, "eng_bounded": eb[i], "cons_bounded": cb[i], "eng_plain": ep[i], "cons_plain": cp[i]})
        })
        .collect();
    note(json!({"probe": "domain_bounds_synthetic", "rows": rows}));
    let mut problems = Vec::new();
    for i in 0..keys.len() {
        if sel(&eb[i]) != sel(&cb[i]) {
            problems.push(format!(
                "key {:?}: engine bounded {} vs core bounded {}",
                keys[i], eb[i], cb[i]
            ));
        }
        if sel(&ep[i]) != sel(&cp[i]) {
            problems.push(format!(
                "key {:?}: engine plain {} vs core plain {}",
                keys[i], ep[i], cp[i]
            ));
        }
    }
    let changed = (0..keys.len()).filter(|&i| eb[i] != ep[i]).count();
    note(
        json!({"probe": "domain_bounds_synthetic_changed", "keys_changed_by_bounds": changed, "of": keys.len()}),
    );
    if changed == 0 {
        problems.push("the bounds changed no first match (not honoured?)".into());
    }
    // edge tables: (a) a min on a FIXED numerator slot (rule 2: n8 fixed 0, min 0, satisfied); (b) a min of 100 on a line
    let edge_a = write("edge_min_on_fixed_numerator.jsonl", &|v| {
        if v["index"] == 2 {
            let mut min = vec![Value::Null; 9];
            min[8] = n(Some(0));
            v["domain_bounds"] = json!({"min": min, "max": Value::Null});
        }
    });
    let edge_b = write("edge_min_100_on_line.jsonl", &|v| {
        if v["index"] == 0 {
            let mut min = vec![Value::Null; 9];
            min[0] = n(Some(100));
            v["domain_bounds"] = json!({"min": min, "max": Value::Null});
        }
    });
    for (name, t) in [
        ("min_on_fixed_numerator", edge_a),
        ("min_100_on_line", edge_b),
    ] {
        let out = std::process::Command::new(bin())
            .arg("first-match")
            .arg("--table")
            .arg(&t)
            .arg("--keys")
            .arg(&kp)
            .env("SYMBOLICA_HIDE_BANNER", "1")
            .output()
            .unwrap();
        let st = json!({"probe": "domain_bounds_edge", "table": name, "engine_exit": out.status.code(),
            "engine_stderr": String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()});
        let core = std::panic::catch_unwind(|| consumer_first_match(&t, false, &keys[..1], &sd))
            .map(|x| x[0].clone());
        note(
            json!({"probe": "domain_bounds_edge", "table": name, "engine": st, "core": format!("{:?}", core.as_ref().map(|v| v.to_string()).map_err(|_| "load refused (panic in test helper = ImportError)"))}),
        );
    }
    assert!(problems.is_empty(), "{problems:?}");
}
