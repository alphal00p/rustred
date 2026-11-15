//! Lane termmin2: REAL reproduction of a reference relation at one key from EXPLICIT seed lists and from the generic
//! MIXED-SHELL family (interfaces/termmin.md 2, step `mixed_shell`), each one solve-recipe child through the adapter and
//! the process layer. `#[ignore]`: run by hand INSIDE A PRIVATE PID NAMESPACE (rule L10) after `source WS/ops/env.sh`,
//! with `TERMMIN_KSTAR_CONFIG` = a JSON file:
//! `{"key": [..], "lines": "<chart mask 1/0>", "seed_lists": {"<id>": [[offset..], ..], ..}, "program", "family",
//!   "order": "<name>" | {order JSON}, "cpus": [..] | null (HELD_CPUS), "work_root", "out_dir", "wall_cap_s",
//!   "key_budget_s"}`.
//! Writes `out_dir/report.json`: per recipe the adapter row, the sealed record's verdict / sizes / digests / proof,
//! and the containment of each explicit list in the generic family at the key.

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::termmin::direct::{DirectConfig, solve_recipe_once};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::termmin::TermminSpec;
use rustred_campaign_core::termmin::spec::MixedShell;
use rustred_campaign_core::termmin::steps::{mixed_shell_offsets, point_recipe_mixed};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[test]
#[ignore = "real children: TERMMIN_KSTAR_CONFIG, private pid namespace"]
fn real_explicit_and_mixed_shell_solves_at_one_key() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10)"
    );
    let p = std::env::var("TERMMIN_KSTAR_CONFIG").expect("TERMMIN_KSTAR_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let s = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    let key: Vec<i32> = serde_json::from_value(c["key"].clone()).unwrap();
    let lines: Vec<bool> = s("lines").chars().map(|x| x == '1').collect();
    let cpus: Vec<usize> = match c["cpus"].as_array() {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap() as usize).collect(),
        None => std::env::var("HELD_CPUS")
            .expect("HELD_CPUS")
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect(),
    };
    let out = PathBuf::from(s("out_dir"));
    std::fs::create_dir_all(&out).unwrap();
    let ms = MixedShell::default();
    let generic = mixed_shell_offsets(&key, &lines, &ms.shells).unwrap();
    let gset: BTreeSet<Vec<i32>> = generic.iter().cloned().collect();
    // recipes: the generic mixed shell + one explicit recipe per list (offsets as centres, shell S0, point domain)
    let mut recipes: Vec<(String, PathBuf)> = vec![];
    let mr = point_recipe_mixed(&key, &lines, &ms).unwrap();
    let mp = out.join("recipe_mixed_shell.json");
    std::fs::write(&mp, &mr.text).unwrap();
    recipes.push(("mixed_shell".into(), mp));
    let mut containment = serde_json::Map::new();
    let chart: String = lines.iter().map(|&l| if l { '1' } else { '0' }).collect();
    for (id, list) in c["seed_lists"].as_object().unwrap() {
        let offs: Vec<Vec<i32>> = serde_json::from_value(list.clone()).unwrap();
        let missing: Vec<&Vec<i32>> = offs.iter().filter(|o| !gset.contains(*o)).collect();
        containment.insert(
            id.clone(),
            json!({"seeds": offs.len(), "in_generic_family": offs.len() - missing.len(),
                   "missing": missing.iter().take(10).collect::<Vec<_>>()}),
        );
        let doc = json!({"schema": "rustred.recipe.v1", "id": format!("termmin2/explicit/{id}"),
            "arity": key.len(), "chart": chart, "domain": {"fixed": key, "equations": [], "exceptions": []},
            "seeds": {"unit": "harder", "centres": offs, "generator": {"shells": ["S0"]}, "sector_filter": true},
            "symmetry_rows": false, "elimination": {"convention": "rustred.recipe-elim.v1"},
            "provenance": {"seeds": format!("explicit list {id}: each offset a centre with shell S0"),
                           "relation_rule": "m1_first (engine default)"}});
        let rp = out.join(format!("recipe_{id}.json"));
        let mut text = rustred_campaign_core::schema::canon::canonical_json(&doc, false).unwrap();
        text.push('\n');
        std::fs::write(&rp, text).unwrap();
        recipes.push((id.clone(), rp));
    }
    eprintln!(
        "generic mixed shell at the key: {} seeds; containment {:?}",
        generic.len(),
        containment
    );
    let order = match &c["order"] {
        Value::String(x) => x.clone(),
        v => v.to_string(),
    };
    let mut spec = TermminSpec::default();
    spec.caps.key_budget_s = c["key_budget_s"].as_u64().or(Some(1800));
    spec.caps.cap_bytes = 1_950_000_000;
    let cfg = Arc::new(DirectConfig {
        node: "reproduction".into(),
        import_spec: PathBuf::new(),
        import_base: PathBuf::new(),
        extra_solved: vec![],
        terminals: None,
        spec,
        program: Program::from_file(
            "rr_node",
            &PathBuf::from(s("program")),
            Invocation::Contract,
        )
        .unwrap(),
        family: s("family").into(),
        order,
        cpus: cpus.clone(),
        work_root: s("work_root").into(),
        out_dir: out.clone(),
        check_threads: 1,
        licence_env: std::env::var_os("SYMBOLICA_LICENSE").map(|_| "SYMBOLICA_LICENSE".to_string()),
        wall_cap_s: c["wall_cap_s"].as_u64().unwrap_or(2000),
        verbose: true,
    });
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    );
    assert!(cpus.len() >= recipes.len(), "one CPU per recipe");
    let hs: Vec<_> = recipes
        .iter()
        .zip(&cpus)
        .map(|((id, rp), &cpu)| {
            let (cfg, sampler, key, id, rp) = (
                cfg.clone(),
                sampler.clone(),
                key.clone(),
                id.clone(),
                rp.clone(),
            );
            std::thread::spawn(move || {
                solve_recipe_once(&cfg, &rp, &key, &id, cpu, sampler)
                    .unwrap_or_else(|e| json!({"id": id, "error": e}))
            })
        })
        .collect();
    let results: Vec<Value> = hs.into_iter().map(|h| h.join().unwrap()).collect();
    for r in &results {
        eprintln!("RESULT {}", serde_json::to_string(r).unwrap());
    }
    let rep = json!({"key": key, "generic_mixed_shell_seeds": generic.len(), "containment": containment,
                     "results": results});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&rep).unwrap(),
    )
    .unwrap();
}
