//! Lane import2, BIG DATA (ignored): the T266 import on table jointest9 (85 GB, 33,660 rules, pre-guard records without
//! witness loci) as far as the existing material allows. The spec is DATA: tests/data/import/t266-jointest9.spec.json
//! (+ the box B document). Facts / proofs for the served keys do not exist yet; the run reports the load (two passes,
//! time, peak memory), the verified symmetry group (derived from the table header's presented family_source), and a
//! STREAMED, round-limited Tier K check (rows to the statement file): request counts and what is blocked.
//!
//! Env: `T266_KEEP_BIG_LITERALS=1` keeps the 4 records whose exception literals overflow the core's i128 polynomial
//! coefficients (the load then refuses them, as it must); by default they are REMOVED with that reason (a DIAGNOSTIC
//! variant: their keys fall through). `T266_MAX_ROUNDS` (default 1) bounds the check. Scratch output under
//! `RUSTRED_IMPORT_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/import2).

use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, TableSetSpec, load, write_statement};
use serde_json::{Value, json};

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

#[test]
#[ignore = "big data: the 85 GB T266 table; run with --ignored in release (10-20 min)"]
fn t266_jointest9_import_as_far_as_the_material_allows() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import");
    let mut sv: Value = serde_json::from_str(
        &std::fs::read_to_string(data.join("t266-jointest9.spec.json")).unwrap(),
    )
    .unwrap();
    let table = sv["sources"][0]["path"].as_str().unwrap().to_string();
    if !Path::new(&table).is_file() {
        println!("SKIP: {table} missing");
        return;
    }
    if std::env::var("T266_KEEP_BIG_LITERALS").is_err() {
        let reason = "exception literal with a coefficient beyond i128: refused by the core polynomial parser \
                      (Poly::parse, CoefficientOverflow); removed in this DIAGNOSTIC run";
        sv["sources"][0]["removed"] = json!(
            [29474, 30764, 31751, 33573]
                .iter()
                .map(|l| json!({"label": l, "reason": reason}))
                .collect::<Vec<_>>()
        );
    }
    let out = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import2",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
        .join("t266");
    std::fs::create_dir_all(&out).unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = match load(&spec, &data) {
        Ok(s) => s,
        Err(e) => {
            println!(
                "T266 LOAD REFUSED after {:.1}s (VmHWM {} MB): {e}",
                t0.elapsed().as_secs_f64(),
                vm_hwm_kb() / 1024
            );
            return;
        }
    };
    let t_load = t0.elapsed().as_secs_f64();
    let src = &set.report.sources;
    println!(
        "T266 load {:.1}s VmHWM {} MB: tables {} records {} start {} keys; sources {}",
        t_load,
        vm_hwm_kb() / 1024,
        set.report.tables,
        set.report.records,
        set.report.start_keys,
        serde_json::to_string(&src.iter().map(|s| json!({"id": s.id, "records_in_files": s.records_in_files,
            "dispatched": s.records_dispatched, "removed": s.removed.len(), "not_dispatched": s.not_dispatched.len(),
            "witness": s.witness, "group": s.group})).collect::<Vec<_>>()).unwrap()
    );
    let ev = Evidence::from_spec(&set, &data).unwrap();
    let max_rounds: u32 = std::env::var("T266_MAX_ROUNDS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1);
    let t1 = std::time::Instant::now();
    let res = import::check::check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads: 4,
            tier: None,
            max_rounds: Some(max_rounds),
        },
        &out.join("rows.jsonl"),
    )
    .unwrap();
    let t_check = t1.elapsed().as_secs_f64();
    let dir = out.join("statement");
    write_statement(
        &set,
        &ev,
        &res,
        &dir,
        Some(json!({"load_s": t_load, "check_s": t_check, "vm_hwm_kb": vm_hwm_kb(), "max_rounds": max_rounds})),
    )
    .unwrap();
    println!(
        "T266 check (Tier {:?}, max_rounds {max_rounds}) {:.1}s VmHWM {} MB: keys {} counts {:?}; closure verdict {:?} \
         rounds {} edges {} open {} requires_regularity_fact {} fact_requests {} credited {} truncated {}; \
         kept rows {}; requests solve {} facts {} proofs {}; blocking {:?}",
        res.tier,
        t_check,
        vm_hwm_kb() / 1024,
        res.summary.keys_total,
        res.counts(),
        res.summary.verdict,
        res.summary.rounds,
        res.summary.edges,
        res.summary.open_total,
        res.summary.requires_regularity_fact,
        res.summary.fact_requests,
        res.summary.credited,
        res.summary.truncated,
        res.rows.len(),
        res.solve_requests.len(),
        res.fact_requests.len(),
        res.proof_requests.len(),
        res.blocking()
    );
}

/// Item 6: the T266 delivery's per-key recipe steps (solves.jsonl: the serving step of each relation-served key) mapped
/// onto explicit recipes: v1.2 (one recipe per (step, explicit seed set), `domain.keys`, `relation_rule`) and v1.1 (point
/// recipes; the M1C steps are refused: v1.1 cannot state the relation rule). Writes the recipes and the per-key
/// assignment side file under the scratch directory. Line / numerator priorities: the table header's coordinate
/// priority (test data; [inferred] equal to the checker's).
#[test]
fn t266_recipe_steps_map_onto_explicit_recipes() {
    use import::recipe_steps::{RecipeForm, assignment_lines, parse_step, recipes_for_steps};
    let p = Path::new("/common/ben/sym-investigation/k7/runs/jointest9-full/deliver/solves.jsonl");
    if !p.is_file() {
        println!("SKIP: {} missing", p.display());
        return;
    }
    assert_eq!(
        import::lines::file_sha256(p).unwrap(),
        "7dc5ffa9c368d7939c24b2a36a5444b7c61423b90bb7b7d9f49850bf8748809e"
    );
    let mut asg: Vec<(String, Vec<i32>)> = vec![];
    let mut none = 0;
    for l in std::fs::read_to_string(p).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        let k = import::lines::parse_key_text(v["key"].as_str().unwrap(), 20).unwrap();
        match (v["result"].as_str(), v["recipe"].as_str()) {
            (Some("served"), Some(r)) => asg.push((r.to_string(), k)),
            _ => none += 1,
        }
    }
    assert_eq!((asg.len(), none), (2117, 3));
    // the DEFAULT sequence's step names all parse
    for s in [
        "d0",
        "d1_cnone_foff_sfirst_mM1C",
        "d2_cunion_fon_sfirst_mM1C",
        "d3_cunion_fon_sfirst_mM1C",
    ] {
        parse_step(s).unwrap();
    }
    let lines: Vec<bool> = (0..20).map(|i| i < 14).collect();
    let lp: Vec<usize> = (0..14).collect();
    let np: Vec<usize> = (14..20).collect();
    let v12 =
        recipes_for_steps(&asg, &lines, &lp, &np, RecipeForm::V1_2, "T266/jointest9/").unwrap();
    let mut by_step: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for g in &v12 {
        let e = by_step.entry(g.step.clone()).or_default();
        e.0 += 1;
        e.1 += g.keys.len();
    }
    println!(
        "T266 steps -> v1.2 recipes: {} recipes for {} keys; per step (recipes, keys) {:?}",
        v12.len(),
        asg.len(),
        by_step
    );
    assert_eq!(v12.iter().map(|g| g.keys.len()).sum::<usize>(), 2117);
    let v11 = recipes_for_steps(&asg, &lines, &lp, &np, RecipeForm::V1_1, "T266/jointest9/");
    println!(
        "T266 steps -> v1.1: {:?}",
        v11.as_ref().err().map(|e| e.to_string())
    );
    assert!(
        v11.is_err(),
        "every T266 step carries M1C: v1.1 cannot state it"
    );
    let out = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import2",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
        .join("t266-recipes");
    std::fs::create_dir_all(out.join("recipes")).unwrap();
    for (i, g) in v12.iter().enumerate() {
        std::fs::write(
            out.join("recipes").join(format!("r{i:05}.json")),
            serde_json::to_string_pretty(&g.recipe).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(out.join("assignment.jsonl"), assignment_lines(&v12)).unwrap();
}
