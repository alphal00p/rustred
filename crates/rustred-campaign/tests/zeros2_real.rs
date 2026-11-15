//! Lane zeros2 (interfaces/zero-proofs.md): REAL measurement, IGNORED by default. The structural zero proofs of a table
//! set (written by `rr_coef_facts prove`) loaded as a FACT SOURCE (`import::zero_proofs::ZeroProofs`, verified on load)
//! and looked up for every POSITIVE-NUMERATOR term of every rule-served row (table rules at the key, credits at the
//! image) of a checked import spec: how many such edges a proof covers, by source class, and the lookup time.
//! Spawns nothing. Environment: ZEROS2_SPEC (import spec), ZEROS2_PROOFS (comma-separated proof files), ZEROS2_OUT
//! (summary JSON), ZEROS2_TIER (`K` default | `C`), ZEROS2_THREADS (default 4), ZEROS2_ROWS (spill file of the check).

use rustred_campaign::closeloop::zeros;
use rustred_campaign_core::closure::{CoefFact, FactSource, Tier};
use rustred_campaign_core::import::check::check_streamed;
use rustred_campaign_core::import::zero_proofs::{LoadOptions, ZeroProofs, ZeroStatus};
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[test]
#[ignore]
fn zero_proofs_cover_the_positive_numerator_edges_of_a_spec() {
    let Ok(spec_path) = std::env::var("ZEROS2_SPEC") else {
        eprintln!("SKIP: ZEROS2_SPEC not set");
        return;
    };
    let proofs: Vec<PathBuf> = std::env::var("ZEROS2_PROOFS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    let out = PathBuf::from(std::env::var("ZEROS2_OUT").expect("ZEROS2_OUT"));
    let rows = PathBuf::from(std::env::var("ZEROS2_ROWS").expect("ZEROS2_ROWS"));
    let threads: usize = std::env::var("ZEROS2_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let tier = match std::env::var("ZEROS2_TIER").as_deref() {
        Ok("C") => Tier::C,
        _ => Tier::K,
    };
    let t0 = Instant::now();
    let spec: TableSetSpec =
        serde_json::from_slice(&std::fs::read(&spec_path).expect("spec")).expect("spec json");
    let set = load(&spec, Path::new("/")).expect("load");
    let ev = Evidence::from_spec(&set, Path::new("/")).expect("evidence");
    let res = check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads,
            tier: Some(tier),
            max_rounds: None,
        },
        &rows,
    )
    .expect("check");
    let load_check_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let zp = ZeroProofs::load(&set, &proofs, &LoadOptions::default()).expect("proofs");
    let proofs_load_s = t1.elapsed().as_secs_f64();
    let served = zeros::served_rule_rows(&set, &res).expect("rows");
    let lines = set.frame.mask().to_vec();
    let t2 = Instant::now();
    // class -> [pos-num terms, of them first-route unknown, proved, proved where regular, not proved]
    let mut by: BTreeMap<String, [u64; 5]> = BTreeMap::new();
    let mut not_proved: BTreeMap<(String, usize), u64> = BTreeMap::new();
    let mut lookups = 0u64;
    for (key, applied, t, r) in &served {
        let Some(terms) = zeros::term_keys(&set, *t, *r, applied) else {
            continue;
        };
        let rid = zeros::rule_id(&set, *t, *r);
        let kind = if key == applied { "rule" } else { "credit" };
        let class = format!("{}|{kind}", set.rules[*t][*r].class);
        for (j, tk) in terms.iter().enumerate() {
            if !tk.iter().zip(&lines).any(|(&x, &l)| !l && x > 0) {
                continue;
            }
            let e = by.entry(class.clone()).or_default();
            e[0] += 1;
            if ev.coefficient(&rid, applied, j, tk) != CoefFact::Unknown {
                continue;
            }
            e[1] += 1;
            lookups += 1;
            match zp.is_zero_term(&set, *t, *r, j, applied) {
                ZeroStatus::Proved => e[2] += 1,
                ZeroStatus::ProvedWhereRegular => e[3] += 1,
                ZeroStatus::NotProved => {
                    e[4] += 1;
                    *not_proved
                        .entry((set.rules[*t][*r].record_sha256.clone(), j))
                        .or_default() += 1;
                }
            }
        }
    }
    let scan_s = t2.elapsed().as_secs_f64();
    let mut top: Vec<((String, usize), u64)> = not_proved.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    let total = by.values().fold([0u64; 5], |mut a, v| {
        for i in 0..5 {
            a[i] += v[i];
        }
        a
    });
    let s = json!({"schema": "rustred.campaign.zeros2-real.v1", "spec": spec_path, "proofs": proofs,
        "tier": format!("{tier:?}"), "closure_keys": res.streamed.as_ref().map(|s| s.rows).unwrap_or(res.rows.len() as u64),
        "served_rule_rows": served.len(), "proof_stats": zp.stats, "proofs_held": zp.len(),
        "positive_numerator_terms": total[0], "first_route_unknown": total[1], "proved": total[2],
        "proved_where_regular": total[3], "not_proved": total[4],
        "by_class": by.iter().map(|(k, v)| (k.clone(), json!({"terms": v[0], "unknown": v[1], "proved": v[2],
            "proved_where_regular": v[3], "not_proved": v[4]}))).collect::<BTreeMap<_, _>>(),
        "not_proved_distinct_record_terms": top.len(),
        "not_proved_top": top.iter().take(30).map(|((s, j), n)| json!({"record_sha256": s, "term": j, "edges": n}))
            .collect::<Vec<_>>(),
        "seconds": {"load_check": load_check_s, "proofs_load": proofs_load_s, "lookup_scan": scan_s},
        "lookups": lookups});
    std::fs::write(&out, serde_json::to_vec_pretty(&s).unwrap()).expect("write");
    println!("{s}");
}
