//! Lane seedmin: REAL joint seed minimisation on one face through solve-recipe children (interfaces/seedmin.md 5, 8).
//! `#[ignore]`: run by hand INSIDE A PRIVATE PID NAMESPACE (rule L10) after `source WS/ops/env.sh`, with
//! `SEEDMIN_REAL_CONFIG` = a JSON file:
//! `{"program", "family", "order": name | {order JSON}, "lines": "<chart mask 1/0>",
//!   "keys": [[..], ..] | "keys_file": "<path: one I(..) per line, extra columns ignored>",
//!   "face_dots": [slots] (optional filter: exactly these lines at power >= 2), "face_nums": [slots] (optional: exactly
//!   these numerators < 0), "max_keys": n (optional), "centre": [..] (default zero), "shells": [..] (harder unit, all
//!   slots, no floor, around the centre) | "list": [[offset], ..] (explicit start list, centre-relative, row order kept),
//!   "domain": {recipe domain} (optional), "verify_keys_file": "<I(..) per line>" + "verify_chunk": n (optional: every
//!   piece list is then also solved at those keys, "valid at N of N"), "policy": {seedmin policy}, "relation_rule", "cpus": [..] | null (HELD_CPUS),
//!   "pin_cpus": bool (default true; false = the CPUs are only a pool size, rule L21),
//!   "threads_per_job", "cap_bytes", "key_budget_s", "wall_cap_s", "work_root", "out_dir", "records_cap_bytes"}`.
//! Writes `out_dir/report.json`: the start list, the joint record (per-key minimisations at the spread keys, pieces,
//! valid at N of N keys, trials), cost (CPU, wall, jobs, journal hits). Resumable: the journal lives in `work_root`.

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::seedmin::{
    ChildConfig, ChildTrialSolver, engine_caps_from_manifest,
};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::coreseed::seeds::{shell_offsets, valid_at};
use rustred_campaign_core::seedmin::{
    LocalValuationJudge, SeedMinPolicy, StubSystemPoles, Trial, TrialMode, TrialSolver, add,
    minimise_joint, rule_candidate,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn parse_key(l: &str) -> Option<Vec<i32>> {
    let s = l.trim();
    let s = s.strip_prefix("I(")?;
    let e = s.find(')')?;
    s[..e].split(',').map(|x| x.trim().parse().ok()).collect()
}

#[test]
#[ignore = "real children: SEEDMIN_REAL_CONFIG, private pid namespace"]
fn real_joint_seed_minimisation_on_one_face() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10)"
    );
    let p = std::env::var("SEEDMIN_REAL_CONFIG").expect("SEEDMIN_REAL_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let s = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    let lines: Vec<bool> = s("lines").chars().map(|x| x == '1').collect();
    let n = lines.len();
    // the face keys
    let mut keys: Vec<Vec<i32>> = match c["keys"].as_array() {
        Some(a) => a
            .iter()
            .map(|k| serde_json::from_value(k.clone()).unwrap())
            .collect(),
        None => std::fs::read_to_string(s("keys_file"))
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(parse_key)
            .collect(),
    };
    let set_of = |k: &str| -> Option<BTreeSet<usize>> {
        c[k].as_array()
            .map(|a| a.iter().map(|x| x.as_u64().unwrap() as usize).collect())
    };
    if let Some(d) = set_of("face_dots") {
        keys.retain(|k| {
            (0..n)
                .filter(|&i| lines[i] && k[i] >= 2)
                .collect::<BTreeSet<_>>()
                == d
        });
    }
    if let Some(m) = set_of("face_nums") {
        keys.retain(|k| {
            (0..n)
                .filter(|&i| !lines[i] && k[i] < 0)
                .collect::<BTreeSet<_>>()
                == m
        });
    }
    let mut seen = BTreeSet::new();
    keys.retain(|k| seen.insert(k.clone()));
    if let Some(m) = c["max_keys"].as_u64() {
        keys.truncate(m as usize);
    }
    assert!(!keys.is_empty(), "no keys");
    let centre: Vec<i32> = c["centre"]
        .as_array()
        .map(|a| a.iter().map(|x| x.as_i64().unwrap() as i32).collect())
        .unwrap_or(vec![0; n]);
    let shells: Vec<String> = serde_json::from_value(c["shells"].clone()).unwrap_or_default();
    // the start list: an explicit "list" (centre-relative, in its ROW order: e.g. the list that closed the face), else the
    // shells around the centre, kept when the seed is in the top sector at SOME key
    let list: Vec<Vec<i32>> = match c["list"].as_array() {
        Some(a) => a
            .iter()
            .map(|o| serde_json::from_value(o.clone()).unwrap())
            .collect(),
        None => shell_offsets(&vec![0; n], &lines, &shells)
            .unwrap()
            .into_iter()
            .filter(|o| keys.iter().any(|k| valid_at(k, &add(&centre, o), &lines)))
            .collect(),
    };
    let policy: SeedMinPolicy = serde_json::from_value(c["policy"].clone()).unwrap();
    policy.validate().unwrap();
    let cpus: Vec<usize> = match c["cpus"].as_array() {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap() as usize).collect(),
        None => std::env::var("HELD_CPUS")
            .expect("HELD_CPUS")
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect(),
    };
    let order = match &c["order"] {
        Value::String(x) => x.clone(),
        v => v.to_string(),
    };
    let out = PathBuf::from(s("out_dir"));
    std::fs::create_dir_all(&out).unwrap();
    let work = PathBuf::from(s("work_root"));
    let program = PathBuf::from(s("program"));
    // capabilities from the engine's manifest (config "engine_manifest", else version.json next to the program)
    let manifest = c["engine_manifest"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| program.with_file_name("version.json"));
    let caps = engine_caps_from_manifest(&manifest);
    eprintln!(
        "[seedmin] {} keys, start list {} seeds, engine features {:?}",
        keys.len(),
        list.len(),
        caps.features
    );
    let cfg = ChildConfig {
        program: Program::from_file("rr_node", &program, Invocation::Contract).unwrap(),
        family: s("family").into(),
        order,
        lines: lines.clone(),
        domain: (!c["domain"].is_null()).then(|| c["domain"].clone()),
        relation_rule: c["relation_rule"]
            .as_str()
            .unwrap_or("first_admissible")
            .into(),
        cpus: cpus.clone(),
        // L21: on a saturated host the run is UNPINNED (its unit's CPUQuota bounds it); `cpus` is then a pool size
        pin_cpus: c["pin_cpus"].as_bool().unwrap_or(true),
        threads_per_job: c["threads_per_job"].as_u64().unwrap_or(4) as usize,
        cap_bytes: c["cap_bytes"].as_u64().unwrap_or(1_950_000_000),
        key_budget_s: c["key_budget_s"].as_u64(),
        max_trace_rows: c["max_trace_rows"].as_u64(),
        wall_cap_s: c["wall_cap_s"].as_u64().unwrap_or(3600),
        work_root: work.clone(),
        licence_env: std::env::var_os("SYMBOLICA_LICENSE").map(|_| "SYMBOLICA_LICENSE".to_string()),
        keep_run_dirs: false,
        d0: policy.d0.clone(),
        records_out: Some(out.join("relations.jsonl")),
        records_cap_bytes: c["records_cap_bytes"].as_u64().unwrap_or(400_000_000),
        fake: None,
        verbose: true,
    };
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(1000),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    );
    let mut solver =
        ChildTrialSolver::new(cfg, sampler, Box::new(LocalValuationJudge), caps).unwrap();
    let resumed = solver.journal_len();
    let t0 = Instant::now();
    let jm = minimise_joint(
        &keys,
        &centre,
        &list,
        &policy,
        &mut solver,
        &mut StubSystemPoles,
    )
    .unwrap();
    // VERIFICATION at further keys (e.g. the whole core of the face): every piece list solved at the verify keys
    // outside its own keys, in chunks (one trial each, scheduled over the pool); "valid at N of N" per piece
    let mut verify = vec![];
    if let Some(vf) = c["verify_keys_file"].as_str() {
        let vkeys: Vec<Vec<i32>> = std::fs::read_to_string(vf)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(parse_key)
            .collect();
        let chunk = c["verify_chunk"].as_u64().unwrap_or(40).max(1) as usize;
        for p in &jm.pieces {
            let own: BTreeSet<&Vec<i32>> = p.keys.iter().collect();
            let extra: Vec<Vec<i32>> = vkeys.iter().filter(|k| !own.contains(k)).cloned().collect();
            let trials: Vec<Trial> = extra
                .chunks(chunk)
                .enumerate()
                .map(|(i, ks)| Trial {
                    label: format!("seedmin/verify/{}/{i}", p.label),
                    centre: centre.clone(),
                    list: p.list.clone(),
                    keys: ks.to_vec(),
                    mode: TrialMode::Solve,
                    engine_minimise: None,
                })
                .collect();
            let ans = solver.solve(&trials).unwrap();
            let all: Vec<_> = ans.into_iter().flatten().collect();
            let ok: Vec<_> = all.iter().filter_map(|a| a.relation()).collect();
            let terms: Vec<u64> = ok.iter().map(|f| f.rhs_count).collect();
            let mut poles: std::collections::BTreeMap<String, usize> = Default::default();
            for f in &ok {
                *poles
                    .entry(serde_json::to_string(&f.pole).unwrap())
                    .or_default() += 1;
            }
            let failed: Vec<Value> = extra
                .iter()
                .zip(&all)
                .filter(|(_, a)| a.relation().is_none())
                .map(|(k, a)| json!([k, a.word()]))
                .collect();
            eprintln!(
                "[seedmin] VERIFY piece {}: valid at {} of {} keys",
                p.label,
                p.keys.len() + ok.len(),
                p.keys.len() + extra.len()
            );
            verify.push(json!({"piece": p.label, "seeds": p.list.len(),
                "valid": format!("{} of {} keys", p.keys.len() + ok.len(), p.keys.len() + extra.len()),
                "extra_keys": extra.len(), "extra_solved": ok.len(),
                "extra_terms": [terms.iter().min(), terms.iter().max()], "extra_poles": poles,
                "failed": failed.iter().take(50).collect::<Vec<_>>(), "failed_count": failed.len()}));
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    let covered: usize = jm.pieces.iter().map(|p| p.keys.len()).sum();
    let candidates: Vec<Value> = jm
        .pieces
        .iter()
        .map(|p| serde_json::to_value(rule_candidate(p, &keys)).unwrap())
        .collect();
    let rep = json!({
        "config": c, "keys": keys.len(), "start_seeds": list.len(), "row_order": policy.row_order,
        "valid": format!("{covered} of {} keys", keys.len()), "unserved": jm.unserved, "undecided": jm.undecided,
        "pieces": jm.pieces.iter().map(|p| json!({"label": p.label, "keys": p.keys.len(), "seeds": p.list.len(),
            "how": p.how, "list": p.list,
            "terms": p.per_key.iter().map(|f| f.rhs_count).collect::<Vec<_>>(),
            "poles": p.per_key.iter().map(|f| f.pole).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "rule_candidates": candidates, "choice_sets": jm.choice_sets, "verify": verify,
        "solves": jm.solves, "trials_run": jm.trials_run, "journal_resumed": resumed,
        "cost": solver.stats, "wall_s": wall, "pole_judge": "local_valuation (temporary)", "record": jm.record});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&rep).unwrap(),
    )
    .unwrap();
    eprintln!(
        "[seedmin] DONE valid {covered} of {} keys; pieces {:?}; solves {}; cpu {:.0} s; wall {:.0} s",
        keys.len(),
        jm.pieces
            .iter()
            .map(|p| (p.keys.len(), p.list.len()))
            .collect::<Vec<_>>(),
        jm.solves,
        solver.stats.cpu_s,
        wall
    );
}
