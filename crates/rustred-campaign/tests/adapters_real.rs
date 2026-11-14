//! REAL children through the adapters and the REAL launcher (lane adapters; interfaces/adapters.md 4). Every test is
//! `#[ignore]`: run by hand inside a private pid namespace, after `source WS/ops/env.sh` (licence by NAME only):
//! `LIGHT_PIDNS=1 light_build.sh adapters test -p rustred-campaign --test adapters_real --release -- --ignored
//!  --test-threads 1 --nocapture`
//! Environment:
//! - `RR_NODE_LINE2`: the engine-line binary with `rr_node solve-recipe` v1.1 (job mode `solve-recipe --job`);
//! - `RR_NODE_CONTRACT`: an rr_node with the contract's `job` subcommand (kind generate);
//! - `RR_PROOF_BIN`: an rr_proof with `instantiate` and the answer echo (`record_sha256`, proof-kernel e13ebcb1);
//! - `RR_T178_FAMILY`, `RR_T178_RECIPES` (dir with `recipes/<id>.json`, `keys/<id>.jsonl`), `RR_T178_REFERENCE` (dir
//!   with `<id>/res/results.jsonl` of the hand reference pass a1), `RR_T178_ORDER` (the order JSON);
//! - `RR_M2_SLICE`: the campaign fixture `small/m2-slice` (TOY2kite, L3a);
//! - `RR_ADAPTERS_OUT`: where run directories are kept (with `RR_KEEP=1`).

mod adapters_support;
use adapters_support::*;

use rustred_campaign::adapters::generate::{GenerateAdapter, GenerateRequest, SeedOptions};
use rustred_campaign::adapters::job::{InputFile, Invocation, JobAdapter, Program, RunOptions, run_attempt};
use rustred_campaign::adapters::proof::{ProofInstantiateAdapter, fact_request};
use rustred_campaign::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest,
};
use rustred_campaign_contract::classify::recipe::KeyOutcome;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn env(k: &str) -> PathBuf {
    PathBuf::from(std::env::var(k).unwrap_or_else(|_| panic!("{k} not set")))
}

fn licence() -> Option<String> {
    let n = "SYMBOLICA_LICENSE";
    std::env::var_os(n).map(|_| n.to_string())
}

fn opts(cap_s: u64) -> RunOptions {
    RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(2 << 30),
        licence_env: licence(),
        wall_cap: Some(Duration::from_secs(cap_s)),
        grace: Some(Duration::from_secs(20)),
    }
}

/// (recipe id, number of keys from the start of its key list): a handful of T178 (recipe, key) pairs of different
/// kinds (line + fixed numerator, point erratum, numerator at -1 with a second fixed numerator).
const PAIRS: [(&str, usize); 3] = [
    ("results_47_fix10m1_n10_M1_json", 3),
    ("results_12_pt_x7_C_json", 1),
    ("results_85_fix11m1_12m1_n11_C_json", 2),
];

fn t178_requests() -> (Vec<SolveRequest>, BTreeMap<Vec<i64>, String>) {
    let dir = env("RR_T178_RECIPES");
    let refd = env("RR_T178_REFERENCE");
    let mut reqs = Vec::new();
    let mut want = BTreeMap::new();
    for (id, n) in PAIRS {
        let r = RecipeInput {
            id: id.into(),
            file: InputFile::from_file(&dir.join("recipes").join(format!("{id}.json"))).unwrap(),
            digest: None,
            sha256: None,
        };
        let keys = std::fs::read_to_string(dir.join("keys").join(format!("{id}.jsonl"))).unwrap();
        for l in keys.lines().take(n) {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            reqs.push(SolveRequest {
                recipe: r.clone(),
                key: serde_json::from_value(v["key"].clone()).unwrap(),
                tag: v["tag"].as_str().map(str::to_string),
                context: None,
            });
        }
        // the hand reference (pass a1): relation digest per key
        let text =
            std::fs::read_to_string(refd.join(id).join("res").join("results.jsonl")).unwrap();
        for l in text.lines() {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["type"] == "relation" {
                let k: Vec<i64> = serde_json::from_value(v["key"].clone()).unwrap();
                want.insert(k, v["relation_digest"].as_str().unwrap_or("").to_string());
            }
        }
    }
    (reqs, want)
}

fn solve_config(program: Program) -> SolveRecipeConfig {
    SolveRecipeConfig {
        program,
        family: InputFile::from_file(&env("RR_T178_FAMILY")).unwrap(),
        order: std::fs::read_to_string(env("RR_T178_ORDER"))
            .unwrap()
            .trim()
            .to_string(),
        batch: BatchPolicy {
            max_keys_per_job: 8,
            max_threads: 2,
            min_keys_per_thread: 2,
        },
        max_trace_rows: None,
        key_budget_s: None,
        expect: None,
        determinism_id: None,
        heartbeat_s: 10,
        stop_grace_s: 20,
        package: None,
        live_repair: false,
        licence_env: licence(),
        fake: None,
    }
}

/// Conformance of the REAL `rr_node solve-recipe` job mode (engine line 2) against the contract: every job goes
/// through the launcher (guard path) and the contract's classification; the report lists every mismatch. The
/// relation digests of the sealed results file are compared with the hand reference as a DIAGNOSTIC (the stream is
/// verified with the contract's verifier; nothing is accepted from it unless the classification accepts).
#[test]
#[ignore = "real child: RR_NODE_LINE2 + T178 data, LIGHT_PIDNS=1"]
fn real_solve_recipe_engine_line2_conformance_and_digests() {
    require_namespace();
    let root = Root::new("real-sr");
    let prog = Program::from_file(
        "rr_node",
        &env("RR_NODE_LINE2"),
        Invocation::VerbJobFlag {
            verb: "solve-recipe".into(),
        },
    )
    .unwrap();
    let ad = SolveRecipeAdapter::new(solve_config(prog));
    let (reqs, want) = t178_requests();
    let plans = ad.plan(&reqs).unwrap();
    eprintln!("{} requests -> {} jobs", reqs.len(), plans.len());
    let mut l = launcher();
    let mut equal = 0usize;
    let mut compared = 0usize;
    for p in &plans {
        let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(900)).unwrap();
        let c = &out.classification;
        eprintln!(
            "job {}: wait {:?} -> row {}{} {:?} accept {:?} ({})",
            p.job_id, rep.ended.wait, c.row, c.sub, c.kind, c.accept, c.why
        );
        if let Some(r) = &out.refusal {
            eprintln!("  refusal: unsealed {:?}", r.unsealed_why);
            for m in &r.conformance {
                eprintln!("  MISMATCH {}", serde_json::to_string(m).unwrap());
            }
        }
        eprintln!(
            "  accepted {} failures {} not_decided {} refused_records {}",
            out.relations.len(),
            out.failures.len(),
            out.not_decided.len(),
            out.refused_records.len()
        );
        // diagnostic: the sealed results stream (contract verifier, through its handle) vs the reference digests
        let layout = RunLayout::new(&rep.prepared.spec.run_dir);
        match verify_stream_handle(&layout.root, &RunLayout::out_rel("results.jsonl"), &["end"]) {
            Ok((_, h)) => {
                let lines = h.canonical_lines().unwrap();
                let keys = rustred_campaign_contract::classify::recipe::parse_keys_input(
                    &std::fs::read(layout.in_dir().join("keys.jsonl")).unwrap(),
                )
                .unwrap();
                let (outs, t) =
                    rustred_campaign_contract::classify::recipe::key_outcomes(&lines, &keys)
                        .expect("results consistent with the keys input");
                eprintln!(
                    "  results: {} records, {} SOLVED+proved, recipe {:?}, det {:?}",
                    t.records, t.accepted, t.recipe_digest, t.determinism_id
                );
                for o in outs {
                    if let KeyOutcome::Accepted(a) = o {
                        compared += 1;
                        let w = want.get(&a.key).cloned().unwrap_or_default();
                        let same = w == a.relation_digest;
                        equal += same as usize;
                        eprintln!(
                            "  key {:?} digest {} reference {} {}",
                            a.key,
                            &a.relation_digest[..16],
                            &w[..w.len().min(16)],
                            if same { "EQUAL" } else { "DIFFERENT" }
                        );
                    }
                }
            }
            Err(e) => eprintln!("  results stream not verified: {e}"),
        }
        // the contract decides: nothing may be accepted from a deviating child
        if out.refusal.is_some() {
            assert!(out.relations.is_empty());
        }
    }
    eprintln!("DIGESTS vs hand reference: {equal}/{compared} equal ({} requests)", reqs.len());
    assert_eq!(compared, want_count(&reqs), "every key solved");
    assert_eq!(equal, compared, "relation digests equal to the hand reference");
}

fn want_count(reqs: &[SolveRequest]) -> usize {
    let mut k: Vec<&Vec<i64>> = reqs.iter().map(|r| &r.key).collect();
    k.sort();
    k.dedup();
    k.len()
}

/// The same request against the contract binary's `job` subcommand (kind solve-recipe unknown to its contract
/// crate): refused (usage, row 12) — the second half of the gap.
#[test]
#[ignore = "real child: RR_NODE_CONTRACT + T178 data, LIGHT_PIDNS=1"]
fn real_solve_recipe_through_the_contract_job_subcommand() {
    require_namespace();
    let root = Root::new("real-sr-job");
    let prog =
        Program::from_file("rr_node", &env("RR_NODE_CONTRACT"), Invocation::Contract).unwrap();
    let ad = SolveRecipeAdapter::new(solve_config(prog));
    let (reqs, _) = t178_requests();
    let reqs = vec![reqs[0].clone()];
    let p = &ad.plan(&reqs).unwrap()[0];
    let mut l = launcher();
    let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(300)).unwrap();
    let c = &out.classification;
    eprintln!(
        "wait {:?} -> row {}{} ({}); stderr: {}",
        rep.ended.wait,
        c.row,
        c.sub,
        c.why,
        std::fs::read_to_string(RunLayout::new(&rep.prepared.spec.run_dir).stderr_log())
            .unwrap_or_default()
            .trim()
    );
    assert!(out.relations.is_empty());
    assert!(matches!(c.row, 12 | 14), "refused by the binary's contract crate");
}

/// One rr_proof `instantiate` batch (legacy child, sealed by the wrapper, answers bound by record_sha256 + key) on the
/// L3a fixture table (arity 9).
#[test]
#[ignore = "real child: RR_PROOF_BIN + RR_M2_SLICE, LIGHT_PIDNS=1"]
fn real_rr_proof_instantiate_batch() {
    require_namespace();
    let root = Root::new("real-proof");
    let t = InputFile::from_file(&env("RR_M2_SLICE").join("L3a").join("table.jsonl")).unwrap();
    let off = rule_offsets(&t.path);
    let reqs = vec![
        fact_request(
            &t,
            off[&0],
            0,
            vec![vec![2, 1, 1, 1, 1, 1, 1, 2, -1], vec![1, 1, 1, 1, 1, 1, 1, 2, -1]],
        )
        .unwrap(),
        fact_request(&t, off[&2], 2, vec![vec![1, 1, 1, 1, 1, 1, 1, 2, 0]]).unwrap(),
    ];
    let ad = ProofInstantiateAdapter {
        program: Program::from_file("rr_proof", &env("RR_PROOF_BIN"), Invocation::Legacy).unwrap(),
        licence_env: licence(),
    };
    let p = &ad.plan(&reqs).unwrap()[0];
    let mut l = launcher();
    let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(300)).unwrap();
    let c = &out.classification;
    eprintln!(
        "legacy wait {:?} mapped {:?} -> row {}{} ({}) refusal {:?}",
        out.legacy_wait, out.mapped_wait, c.row, c.sub, c.why, out.refusal
    );
    for f in &out.facts {
        eprintln!(
            "  rule {} key {:?} in_case {} regular {} rhs {}",
            f.rule,
            f.key,
            f.line["in_case"],
            f.line["regular"],
            f.line["rhs"].as_array().map_or(0, Vec::len)
        );
    }
    let _ = rep;
    assert_eq!(c.row, 1);
    assert_eq!(out.facts.len(), 3);
}

fn rule_offsets(p: &Path) -> BTreeMap<u64, u64> {
    let b = std::fs::read(p).unwrap();
    let mut m = BTreeMap::new();
    let mut off = 0u64;
    for l in b.split_inclusive(|&c| c == b'\n') {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(l)
            && v["type"] == "rule"
        {
            m.insert(v["index"].as_u64().unwrap(), off);
        }
        off += l.len() as u64;
    }
    m
}

/// One tiny generate job (`rr_node job`, kind generate) on the TOY2kite fixture (arity 5): the whole chart, order id.
/// Then the EC-1 v2 seed option "symmetry at the key alone" on one PRESENTED case.
#[test]
#[ignore = "real child: RR_NODE_CONTRACT + RR_M2_SLICE, LIGHT_PIDNS=1"]
fn real_generate_tiny_job() {
    require_namespace();
    let root = Root::new("real-gen");
    let fam = InputFile::from_file(&env("RR_M2_SLICE").join("TOY2kite").join("family.sym")).unwrap();
    let mut prog =
        Program::from_file("rr_node", &env("RR_NODE_CONTRACT"), Invocation::Contract).unwrap();
    prog.features_required = vec!["job".into(), "no_elision".into()];
    let ad = GenerateAdapter {
        program: prog,
        licence_env: licence(),
        heartbeat_s: 2,
        stop_grace_s: 10,
        fake: None,
    };
    let reqs = vec![
        GenerateRequest {
            family: fam.clone(),
            cases: vec![],
            order: "id".into(),
            seed: SeedOptions::default(),
            max_cases: None,
            descend_open: false,
            wall_case_s: Some(300),
            wall_job_s: None,
        },
        GenerateRequest {
            family: fam,
            cases: vec![serde_json::json!({"frame": "PRESENTED", "fixed": [null, null, null, null, 1]})],
            order: "id".into(),
            seed: SeedOptions::symmetry_at_key_alone(),
            max_cases: Some(1),
            descend_open: true,
            wall_case_s: Some(300),
            wall_job_s: None,
        },
    ];
    let plans = ad.plan(&reqs).unwrap();
    let mut l = launcher();
    let mut rows = vec![];
    for p in &plans {
        let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(600)).unwrap();
        let c = &out.classification;
        eprintln!(
            "job {}: wait {:?} -> row {}{} {:?} accept {:?} ({}); accepted {} lines, subcases {}, refusal {:?}",
            p.job_id,
            rep.ended.wait,
            c.row,
            c.sub,
            c.kind,
            c.accept,
            c.why,
            out.accepted.len(),
            out.subcases.len(),
            out.refusal
        );
        rows.push(c.row);
    }
    assert_eq!(rows[0], 1, "whole chart complete");
    assert!(rows[1] != 22 && rows[1] != 23, "the seed-option job is a contract outcome");
}

/// Exit paths and strict-spec probes of the real `solve-recipe` job mode (conformance, engine line 2): exit 10 with a
/// key outside the recipe's domain, a per-key wall budget (time-decided), a STOP file (exit 30), and job.json files
/// a conformant child must refuse (unknown field 40, foreign program digest 42, missing feature 42, wrong input
/// digest 41). Prints what the child did; asserts only that nothing is accepted from a deviating child.
#[test]
#[ignore = "real child: RR_NODE_LINE2 + T178 data, LIGHT_PIDNS=1"]
fn real_solve_recipe_exit_paths_and_spec_probes() {
    use rustred_campaign::adapters::conformance::diagnose;
    use rustred_campaign::launch::ChildLauncher;
    require_namespace();
    let root = Root::new("real-sr-paths");
    let prog = Program::from_file(
        "rr_node",
        &env("RR_NODE_LINE2"),
        Invocation::VerbJobFlag {
            verb: "solve-recipe".into(),
        },
    )
    .unwrap();
    let (base, _) = t178_requests();
    let mut l = launcher();
    let report = |tag: &str, out: &rustred_campaign::adapters::solve_recipe::SolveBatch,
                  rep: &rustred_campaign::adapters::job::AttemptReport| {
        let c = &out.classification;
        let layout = RunLayout::new(&rep.prepared.spec.run_dir);
        let o: serde_json::Value = std::fs::read(layout.outcome())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        eprintln!(
            "{tag}: wait {:?} -> row {}{} ({}); child stop {} source {} time_decided {}",
            rep.ended.wait, c.row, c.sub, c.why, o["job"]["stop"]["code"], o["job"]["stop"]["source"],
            o["job"]["time_decided"]
        );
        for m in diagnose(&layout, &rep.prepared.spec, &rep.prepared.spec_blake3) {
            eprintln!("  MISMATCH {}", serde_json::to_string(&m).unwrap());
        }
        if let Ok((_, h)) =
            verify_stream_handle(&layout.root, &RunLayout::out_rel("results.jsonl"), &["end", "partial"])
        {
            let lines = h.canonical_lines().unwrap();
            for v in lines.iter().filter_map(|l| serde_json::from_slice::<serde_json::Value>(l).ok()) {
                if v["type"] == "relation" {
                    eprintln!("  record {:?} {}", v["key"], v["verdict"]);
                } else {
                    eprintln!("  end {} counts {}", v["type"], v["counts"]);
                }
            }
        }
        assert!(out.relations.is_empty() || out.refusal.is_none());
    };
    // 1. exit 10: a key outside the recipe's domain (its fixed numerator slot moved off the domain value)
    let mut off = base[0].clone();
    off.key[10] -= 1;
    let reqs = vec![base[0].clone(), off];
    let ad = SolveRecipeAdapter::new(solve_config(prog.clone()));
    let p = &ad.plan(&reqs).unwrap()[0];
    let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(600)).unwrap();
    report("keys_unsolved", &out, &rep);
    // 2. per-key wall budget (time-decided): 1 s on a key that takes longer
    let mut c = solve_config(prog.clone());
    c.key_budget_s = Some(1);
    let ad = SolveRecipeAdapter::new(c);
    let reqs = vec![base[base.len() - 1].clone()];
    let p = &ad.plan(&reqs).unwrap()[0];
    let (out, rep) = run_attempt(&mut l, &ad, &reqs, p, root.path(), 1, &opts(600)).unwrap();
    report("key_budget", &out, &rep);
    // 3. STOP file during the batch: graceful stop, exit 30, sealed partial
    let ad = SolveRecipeAdapter::new(solve_config(prog.clone()));
    let reqs: Vec<SolveRequest> = base[..3].to_vec();
    let p = ad.plan(&reqs).unwrap().remove(0);
    let run = rustred_campaign_contract::layout::attempt_dir(
        &std::path::absolute(root.path()).unwrap(),
        &p.job_id,
        1,
    );
    let t = std::thread::spawn(move || {
        for _ in 0..1200 {
            if let Ok(e) = std::fs::read_to_string(run.join("EVENTS.jsonl"))
                && e.contains("\"phase\"")
            {
                std::fs::write(run.join("STOP"), b"stop\n").unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let (out, rep) = run_attempt(&mut l, &ad, &reqs, &p, root.path(), 1, &opts(600)).unwrap();
    t.join().unwrap();
    report("stopfile", &out, &rep);
    // 4. strict-spec probes: the adapter never writes such specs; job.json is rewritten after `prepare`
    let ad = SolveRecipeAdapter::new(solve_config(prog.clone()));
    let reqs = vec![base[1].clone()];
    let plans = ad.plan(&reqs).unwrap();
    for (attempt, (probe, want)) in [
        ("unknown_field", "40"),
        ("foreign_program_digest", "42"),
        ("missing_feature", "42"),
        ("wrong_input_digest", "41"),
    ]
    .into_iter()
    .enumerate()
    {
        let prep = ad.prepare(&reqs, &plans[0], root.path(), attempt as u32 + 1).unwrap();
        let path = prep.layout.job_spec();
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match probe {
            "unknown_field" => v["bogus_budget"] = serde_json::json!(1),
            "foreign_program_digest" => v["program"]["blake3"] = serde_json::json!("0".repeat(64)),
            "missing_feature" => {
                v["program"]["features_required"] = serde_json::json!(["no_such_feature"])
            }
            _ => v["inputs"][1]["blake3"] = serde_json::json!("0".repeat(64)),
        }
        let mut perm = std::fs::metadata(&path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        std::fs::set_permissions(&path, perm).unwrap();
        std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let mut ls = prep.launch.clone();
        ls.memory_cap_bytes = Some(2 << 30);
        if let Some(n) = licence()
            && !ls.env_passthrough.contains(&n)
        {
            ls.env_passthrough.push(n);
        }
        let mut h = l.launch(&ls).unwrap();
        let r = rustred_campaign::adapters::job::drive(&mut h, &opts(300));
        let results = prep.layout.results().exists();
        eprintln!(
            "probe {probe}: exit {:?} (a conformant child: {want}), results written: {results}; stderr: {}",
            r.exit,
            std::fs::read_to_string(prep.layout.stderr_log())
                .unwrap_or_default()
                .lines()
                .last()
                .unwrap_or("")
        );
    }
}
