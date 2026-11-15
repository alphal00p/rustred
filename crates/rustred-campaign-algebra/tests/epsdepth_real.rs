//! Lane epsdepth: the composed pole profiles of a REAL closed node (ignored by default; run with `--ignored`).
//!
//! Environment (data, nothing node-specific in code):
//!   EPSDEPTH_IMPORT_SPEC   import TableSetSpec JSON of the statement's material (base: EPSDEPTH_IMPORT_BASE, default "/")
//!   EPSDEPTH_STATEMENT_DIR the closing loop's statement directory (statement.jsonl, terminals.json, SHA256SUMS)
//!   EPSDEPTH_OUT           output directory (report.json, rows.jsonl)
//!   EPSDEPTH_SPEC          optional epsdepth spec JSON (text or a path); EPSDEPTH_NODE optional node name
//!   EPSDEPTH_INDEX_DIR     optional persistent table-index directory (import::load_with)
//!   EPSDEPTH_CACHE_MB      parsed-record cache budget (default 4096)
//! Needs the Symbolica licence (`source WS/ops/env.sh`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred_campaign_algebra::epsdepth::SymbolicaLaurent;
use rustred_campaign_algebra::epsdepth::record_source::RecordSource;
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use rustred_campaign_core::epsdepth::{
    ComposeParams, EpsDepthSpec, compose, dots_rank, node_report,
};
use rustred_campaign_core::import::{self, LoadOptions, TableSetSpec};
use serde_json::{Value, json};

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|s| !s.is_empty())
}

fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .map(|l| l.to_string())
        })
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}

#[test]
#[ignore]
fn real_node_composed_profiles() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (Some(spec_path), Some(stmt), Some(out)) = (
        env("EPSDEPTH_IMPORT_SPEC"),
        env("EPSDEPTH_STATEMENT_DIR"),
        env("EPSDEPTH_OUT"),
    ) else {
        eprintln!("SKIP: EPSDEPTH_IMPORT_SPEC / EPSDEPTH_STATEMENT_DIR / EPSDEPTH_OUT not set");
        return;
    };
    let t0 = Instant::now();
    let spec = match env("EPSDEPTH_SPEC") {
        None => EpsDepthSpec::default(),
        Some(s) => {
            let text = if Path::new(&s).is_file() {
                std::fs::read_to_string(&s).unwrap()
            } else {
                s
            };
            EpsDepthSpec::from_value(&serde_json::from_str::<Value>(&text).unwrap()).unwrap()
        }
    };
    let node = env("EPSDEPTH_NODE").unwrap_or_else(|| "node".into());
    let base = PathBuf::from(env("EPSDEPTH_IMPORT_BASE").unwrap_or_else(|| "/".into()));
    let ts = TableSetSpec::read(Path::new(&spec_path)).unwrap();
    let opts = LoadOptions {
        index_dir: env("EPSDEPTH_INDEX_DIR").map(PathBuf::from),
    };
    let set = import::load_with(&ts, &base, &opts).unwrap();
    let t_load = t0.elapsed().as_secs_f64();
    eprintln!("loaded set in {t_load:.1} s, rss peak {} MB", rss_mb());
    let alg = SymbolicaAlgebra::new().unwrap();
    let lau = SymbolicaLaurent::new(&spec.d0).unwrap();
    let cache = env("EPSDEPTH_CACHE_MB")
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(4096)
        << 20;
    let mut src =
        RecordSource::from_statement(&set, Path::new(&stmt), alg, lau.clone(), cache).unwrap();
    eprintln!(
        "statement: {:?} kinds {:?}",
        src.stats.statement_rows,
        src.kinds()
    );
    let lines = src.lines().to_vec();
    let box_idx = src.box_keys();
    let box_keys: Vec<Vec<i32>> = box_idx
        .iter()
        .map(|&i| rustred_campaign_core::epsdepth::ReductionSource::key(&src, i))
        .collect();
    let chosen = spec.select_keys(&box_keys, |k| dots_rank(k, &lines));
    let sel: Vec<usize> = chosen.iter().map(|k| src.index_of(k).unwrap()).collect();
    eprintln!("box keys {}, selected {}", box_idx.len(), sel.len());
    let params = ComposeParams {
        target: spec.target_exponent,
        max_depth: spec.budgets.max_depth,
        max_keys: spec.budgets.max_keys,
        max_seconds: spec.budgets.max_seconds,
    };
    let nt = src.n_terminals();
    let terminals = src.terminals().to_vec();
    let mut lau2 = lau.clone();
    let t1 = Instant::now();
    let comp = compose(&mut src, &mut lau2, &sel, nt, &params).unwrap();
    let t_comp = t1.elapsed().as_secs_f64();
    let keys_of = |i: usize| rustred_campaign_core::epsdepth::ReductionSource::key(&src, i);
    let (mut header, rows) =
        node_report(&comp, &spec, &terminals, &keys_of, &lines, &node).unwrap();
    header["source"] = serde_json::to_value(&src.stats).unwrap();
    header["inputs"] = json!({"import_spec": spec_path, "statement_dir": stmt});
    header["timing"] = json!({"load_s": t_load, "compose_s": t_comp, "keys_per_s": sel.len() as f64 / t_comp.max(1e-9),
        "cone_keys_per_s": comp.stats.cone_keys as f64 / t_comp.max(1e-9), "rss_peak_mb": rss_mb(),
        "cache": src.cache_state()});
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        Path::new(&out).join("report.json"),
        serde_json::to_string_pretty(&header).unwrap(),
    )
    .unwrap();
    let mut f =
        std::io::BufWriter::new(std::fs::File::create(Path::new(&out).join("rows.jsonl")).unwrap());
    for r in &rows {
        writeln!(f, "{r}").unwrap();
    }
    f.flush().unwrap();
    // optional FULL local census over every served statement key (POLES_OBJECTIVE 2A: route poles, incl. subsector terms)
    if env("EPSDEPTH_LOCAL_FULL").is_some() {
        let t2 = Instant::now();
        let n = rustred_campaign_core::epsdepth::ReductionSource::n_keys(&src);
        let mut by_rec: Vec<(usize, usize, usize)> = (0..n)
            .filter_map(|k| src.record_of(k).map(|(t, r)| (t, r, k)))
            .collect();
        by_rec.sort();
        let mut keys_singular = 0u64;
        let mut keys_singular_top = 0u64;
        let mut recs: std::collections::BTreeMap<(usize, usize), u32> = Default::default();
        let mut hist: std::collections::BTreeMap<u32, u64> = Default::default();
        let mut errors = 0u64;
        for &(t, r, k) in &by_rec {
            let e = recs.entry((t, r)).or_insert(0);
            match src.local_profile_full(k, 3) {
                Ok(p) => {
                    *hist.entry(p.pole_order).or_default() += 1;
                    if p.pole_order > 0 {
                        keys_singular += 1;
                        *e = (*e).max(p.pole_order);
                    }
                    if p.pole_order_top > 0 {
                        keys_singular_top += 1;
                    }
                }
                Err(_) => errors += 1,
            }
        }
        let rules_singular = recs.values().filter(|&&o| o > 0).count();
        let local = json!({"served_keys": by_rec.len(), "records_used": recs.len(), "records_singular": rules_singular,
            "keys_singular": keys_singular, "keys_singular_top": keys_singular_top, "histogram": hist, "errors": errors,
            "seconds": t2.elapsed().as_secs_f64()});
        eprintln!("LOCAL FULL census: {local}");
        std::fs::write(
            Path::new(&out).join("local_census.json"),
            serde_json::to_string_pretty(&local).unwrap(),
        )
        .unwrap();
    }
    // the local census (POLES_OBJECTIVE 2A comparison): keys whose own relation has a top-term pole
    eprintln!(
        "DONE node {node}: selected {} cone {} edges {} singular edges {} singular relations {} max edge pole {} in {t_comp:.1} s; rss {} MB",
        sel.len(),
        comp.stats.cone_keys,
        comp.stats.edges,
        comp.stats.singular_edges,
        comp.stats.singular_relations,
        comp.stats.max_edge_pole,
        rss_mb()
    );
    eprintln!("verdicts {}", header["verdicts"]);
    for t in header["per_terminal"].as_array().unwrap() {
        eprintln!(
            "terminal {} max {} hist {} blame {}",
            t["terminal"], t["max_order"], t["histogram"], t["blamed_relations"]
        );
    }
}
