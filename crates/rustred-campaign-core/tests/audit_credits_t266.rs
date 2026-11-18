//! Independent audit (lane audit-credits, 2026-11-14), item 5: what the T266 comparison of lane import3 shows about the
//! symmetry credits. HEAVY (85 GB table through a private copy of the table index; ~1 GB): `#[ignore]`, run with
//! `--ignored` under an own memory scope. Inputs (read only): the jointest9 table, the independent checker's run
//! (`svc*/service.jsonl`: per served key the chosen element, image and image record), import3's walk statement
//! (our credited keys and the label of the image's record).
//!
//! Samples: >= 200 of the checker's credited keys (deterministic stride), EVERY checker-credited key with a numerator
//! (the class that needed per-slot purity: T266 has no fully pure numerator map), and 100 of the keys only we credit.
//! Per key: our credit is recomputed with our verified group and our dispatch (the standalone checker, Tier K, the
//! walk's diagnostic witness policy) and must name the image record the walk statement names; our image and the
//! checker's image are each verified EXACT by the independent geometric route (element momentum matrix, family
//! momenta) and by the expanded symmetry row; our image's top terms are strictly below the key; the checker's image
//! dispatches to the record the checker names.

mod inputs;
use rustred_campaign_core::closure::{NoFacts, Sectors, Tier};
use rustred_campaign_core::cover::Dispatch;
use rustred_campaign_core::import;
use rustred_campaign_core::import::symmetry::{GroupFormat, GroupSpec, load_verified_group};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{SymRowMode, VerifiedGroup, rows_at};
use rustred_campaign_core::symmetry::CreditChecker;

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

fn env_or(k: &str, d: &str) -> PathBuf {
    std::env::var_os(k)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(d))
}

fn parse_key(t: &str) -> Vec<i32> {
    t.trim()
        .trim_start_matches("I(")
        .trim_end_matches(')')
        .split(',')
        .map(|x| x.trim().parse().unwrap())
        .collect()
}

fn canon(v: &[i64]) -> Vec<i64> {
    match v.iter().find(|&&x| x != 0) {
        Some(&x) if x < 0 => v.iter().map(|y| -y).collect(),
        _ => v.to_vec(),
    }
}

/// One-term image of `key` under the momentum matrix `t` (independent of every SlotImage and of symmetry.rs).
fn geometric_image(fam: &Family, t: &[i64], key: &[i32]) -> Option<Vec<i32>> {
    let w = fam.loops() + 1;
    let c = fam.momenta();
    let ks = fam.kshift();
    let n = fam.n();
    let cans: Vec<Vec<i64>> = c.iter().map(|m| canon(m)).collect();
    let mut b = vec![0i32; n];
    let mut hit = vec![false; n];
    for i in 0..n {
        if key[i] == 0 && ks[i] == 0 {
            continue;
        }
        let img: Vec<i64> = (0..w)
            .map(|j| (0..w).map(|v| c[i][v] * t[v * w + j]).sum())
            .collect();
        let ci = canon(&img);
        let tg = (0..n).find(|&s| cans[s] == ci)?;
        if ks[tg] != ks[i] {
            return None;
        }
        if key[i] != 0 {
            if std::mem::replace(&mut hit[tg], true) {
                return None;
            }
            b[tg] = key[i];
        }
    }
    Some(b)
}

fn row_one_term(vg: &VerifiedGroup, ei: usize, key: &[i32], image: &[i32]) -> Option<bool> {
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let rows = rows_at(vg, &seed, SymRowMode::Expanded, 1 << 14).ok()?;
    let r = rows.iter().find(|r| r.element == ei)?;
    let got: BTreeSet<(Vec<i64>, i128, i128)> = r
        .terms
        .iter()
        .map(|((_, k), q)| (k.clone(), q.n, q.d))
        .collect();
    let want: BTreeSet<(Vec<i64>, i128, i128)> = [
        (seed.1.clone(), 1, 1),
        (image.iter().map(|&x| x as i64).collect(), -1, 1),
    ]
    .into_iter()
    .collect();
    Some(got == want)
}

struct CheckerCredit {
    g: i64,
    image: Vec<i32>,
    case: i64,
}

#[test]
#[ignore = "heavy: T266 jointest9 (85 GB table via the index copy); run with --ignored in an own scope"]
fn audit_credits_t266_samples_vs_checker() {
    let ws = env_or(
        "RUSTRED_IMPORT_WS",
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev",
    );
    let table = ws.join("SCRATCHPAD/t266/jointest9/F.table.jsonl");
    let run = env_or(
        "AUDIT_T266_CHECKER_RUN",
        "/common/ben/sym-investigation/k7/runs/jointest9-full",
    );
    let statement = env_or(
        "AUDIT_T266_STATEMENT",
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/import3/t266-walk/statement/statement.jsonl",
    );
    let index_dir = env_or(
        "AUDIT_T266_INDEX",
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/audit-credits/index",
    );
    if !table.is_file() || !run.is_dir() || !statement.is_file() {
        inputs::missing(format_args!("SKIP: T266 inputs missing"));
        return;
    }
    let t0 = std::time::Instant::now();
    // checker credits
    let mut chk: BTreeMap<Vec<i32>, CheckerCredit> = BTreeMap::new();
    for s in 0..8 {
        let p = run.join(format!("svc{s}/service.jsonl"));
        let Ok(f) = std::fs::File::open(&p) else {
            continue;
        };
        for l in std::io::BufReader::new(f).lines() {
            let l = l.unwrap();
            if !l.contains("\"served\": true") {
                continue;
            }
            let v: Value = serde_json::from_str(&l).unwrap();
            let c = &v["chosen"];
            chk.insert(
                parse_key(v["key"].as_str().unwrap()),
                CheckerCredit {
                    g: c["g"].as_i64().unwrap(),
                    image: parse_key(c["image"].as_str().unwrap()),
                    case: c["case"].as_i64().unwrap(),
                },
            );
        }
    }
    // our credited keys (image record label) from the walk statement
    let mut ours: BTreeMap<Vec<i32>, (String, Option<i64>)> = BTreeMap::new();
    {
        let f = std::fs::File::open(&statement).unwrap();
        for l in std::io::BufReader::with_capacity(1 << 20, f).lines() {
            let l = l.unwrap();
            if !l.contains("\"basis\":\"symmetry_credit") {
                continue;
            }
            let v: Value = serde_json::from_str(&l).unwrap();
            let k: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
            ours.insert(
                k,
                (
                    v["basis"].as_str().unwrap().to_string(),
                    v["server"]["label"].as_i64(),
                ),
            );
        }
    }
    println!(
        "checker credits {}, our credited keys {} ({} via table rules); read in {:.1}s",
        chk.len(),
        ours.len(),
        ours.values()
            .filter(|v| v.0 == "symmetry_credit_verified")
            .count(),
        t0.elapsed().as_secs_f64()
    );
    // the import (diagnostic witness policy as the walk) through the private index copy
    let h = import::lines::read_header("jointest9", &table).unwrap();
    let frame = rustred_campaign_core::key::NodeFrame::from_header(&h).unwrap();
    let mask = frame.mask().to_vec();
    let numerator = |k: &[i32]| k.iter().zip(&mask).any(|(&v, &l)| !l && v != 0);
    // samples
    let chk_keys: Vec<&Vec<i32>> = chk.keys().collect();
    let stride = (chk_keys.len() / 220).max(1);
    let s1: Vec<Vec<i32>> = chk_keys
        .iter()
        .step_by(stride)
        .map(|k| (*k).clone())
        .collect();
    let s2: Vec<Vec<i32>> = chk_keys
        .iter()
        .filter(|k| numerator(k))
        .map(|k| (*k).clone())
        .collect();
    let only_ours: Vec<&Vec<i32>> = ours.keys().filter(|k| !chk.contains_key(*k)).collect();
    let stride3 = (only_ours.len() / 100).max(1);
    let s3: Vec<Vec<i32>> = only_ours
        .iter()
        .step_by(stride3)
        .take(100)
        .map(|k| (*k).clone())
        .collect();
    println!(
        "samples: checker stride {} -> {}, checker numerator keys {}, ours-only {} of {}",
        stride,
        s1.len(),
        s2.len(),
        s3.len(),
        only_ours.len()
    );
    let dir = std::env::temp_dir().join(format!("audit-credits-t266-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(
        &st,
        s1.iter()
            .take(1)
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"T266","frame_from":"jointest9","bits":6,
        "policies":{"elided":"lift_with_per_key_proof","witness_absent":"lift_with_regular_fact","witness_check":"off_diagnostic"},
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"jointest9","kind":"engine_table","class":"table","path":table.display().to_string(),
                    "hand_name":{"template":"rule#{index}"}},
                   {"id":"credits","kind":"symmetry_credit","group_format":"derive"}]});
    let ts = import::TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = import::load_with(
        &ts,
        &dir,
        &import::LoadOptions {
            index_dir: Some(index_dir.clone()),
        },
    )
    .unwrap();
    println!(
        "loaded in {:.1}s; index {:?}",
        t0.elapsed().as_secs_f64(),
        set.report.sources[0]
            .indexes
            .iter()
            .map(|i| i.how)
            .collect::<Vec<_>>()
    );
    let pol = set.credits.clone().unwrap();
    assert!(pol.is_verified());
    let cmp = NodeComparator::from_header(&h.raw).unwrap();
    let (vg, _rep) = load_verified_group(
        &GroupSpec {
            family: None,
            group: None,
            group_format: GroupFormat::Derive,
            max_candidates: None,
            max_terms: None,
        },
        Path::new("/"),
        &mask,
        cmp.presentation().new_to_old(),
        h.raw.get("family_source").and_then(Value::as_str),
    )
    .unwrap();
    assert_eq!(vg.group().elements.len(), pol.group().len());
    let fam = Family::parse(h.raw["family_source"].as_str().unwrap()).unwrap();
    let checker = CreditChecker::new(
        &set.disp,
        &set.bodies,
        &set.cmp,
        Sectors::new(mask.clone()),
        &NoFacts,
        Tier::K,
    );
    let label_of =
        |pos: rustred_campaign_core::cover::RuleRef| set.rules[pos.table][pos.record].label;
    #[derive(Default, Debug)]
    struct Cnt {
        keys: usize,
        ours_credited: usize,
        ours_label_equals_statement: usize,
        ours_exact_geometry: usize,
        ours_exact_row: usize,
        ours_terms_below: usize,
        checker_exact_geometry: usize,
        checker_exact_row: usize,
        checker_case_dispatch_equal: usize,
        same_image_as_checker: usize,
        refuted: Vec<String>,
        notes: Vec<String>,
    }
    let mut res: BTreeMap<&str, Cnt> = BTreeMap::new();
    for (tag, sample) in [
        ("checker-stride", &s1),
        ("checker-numerator", &s2),
        ("ours-only", &s3),
    ] {
        let c = res.entry(tag).or_default();
        for k in sample.iter() {
            c.keys += 1;
            // ours
            match checker.serve(k, &pol).unwrap() {
                Ok(cr) => {
                    c.ours_credited += 1;
                    let lab = label_of(cr.rule);
                    match ours.get(k) {
                        Some((b, Some(l))) if b == "symmetry_credit_verified" && *l == lab => {
                            c.ours_label_equals_statement += 1
                        }
                        other => c.notes.push(format!(
                            "{k:?}: recomputed credit via record {lab}, statement {other:?}"
                        )),
                    }
                    let mat = &vg.proofs()[cr.element].matrix;
                    if geometric_image(&fam, mat, k).as_deref() == Some(&cr.image[..]) {
                        c.ours_exact_geometry += 1;
                    } else {
                        c.refuted.push(format!(
                            "OURS {k:?} -> {:?} (element {}): not exact by geometry",
                            cr.image, cr.element
                        ));
                    }
                    match row_one_term(&vg, cr.element, k, &cr.image) {
                        Some(true) => c.ours_exact_row += 1,
                        Some(false) => c.refuted.push(format!(
                            "OURS {k:?} -> {:?}: expanded row not one-term",
                            cr.image
                        )),
                        None => {}
                    }
                    if cr.top_terms.iter().all(|t| set.cmp.lower(t, k)) {
                        c.ours_terms_below += 1;
                    } else {
                        c.refuted
                            .push(format!("OURS {k:?}: a top term not below the key"));
                    }
                    if let Some(cc) = chk.get(k)
                        && cc.image == cr.image
                    {
                        c.same_image_as_checker += 1;
                    }
                }
                Err(refusals) => c.notes.push(format!(
                    "{k:?}: not credited by the standalone checker (statement {:?}); refusals {:?}",
                    ours.get(k),
                    refusals
                        .iter()
                        .map(|(e, r)| format!("{e}:{r:?}"))
                        .collect::<Vec<_>>()
                )),
            }
            // the checker's credit, verified independently
            if let Some(cc) = chk.get(k) {
                let exact_el: Vec<usize> = (0..pol.group().len())
                    .filter(|&e| {
                        geometric_image(&fam, &vg.proofs()[e].matrix, k).as_deref()
                            == Some(&cc.image[..])
                    })
                    .collect();
                if exact_el.is_empty() {
                    c.refuted.push(format!(
                        "CHECKER {k:?} -> {:?} (g {}): no exact element",
                        cc.image, cc.g
                    ));
                } else {
                    c.checker_exact_geometry += 1;
                    if exact_el
                        .iter()
                        .any(|&e| row_one_term(&vg, e, k, &cc.image) == Some(true))
                    {
                        c.checker_exact_row += 1;
                    }
                }
                match checker.dispatch(&cc.image) {
                    Dispatch::Covered(r) if label_of(r) == cc.case => {
                        c.checker_case_dispatch_equal += 1
                    }
                    d => c.notes.push(format!(
                        "{k:?}: checker image {:?} case {} vs our dispatch {d:?}",
                        cc.image, cc.case
                    )),
                }
            }
        }
    }
    for (tag, c) in &res {
        println!(
            "{tag}: keys {} | ours credited {} label=statement {} exact geometry {} exact row {} terms below {} | checker exact geometry {} exact row {} case=our dispatch {} | same image {} | refuted {} | notes {}",
            c.keys,
            c.ours_credited,
            c.ours_label_equals_statement,
            c.ours_exact_geometry,
            c.ours_exact_row,
            c.ours_terms_below,
            c.checker_exact_geometry,
            c.checker_exact_row,
            c.checker_case_dispatch_equal,
            c.same_image_as_checker,
            c.refuted.len(),
            c.notes.len()
        );
        for n in c.notes.iter().take(5) {
            println!("   note: {n}");
        }
        for r in c.refuted.iter().take(10) {
            println!("   REFUTED: {r}");
        }
    }
    println!("total {:.1}s", t0.elapsed().as_secs_f64());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(res.values().all(|c| c.refuted.is_empty()));
}
