//! Lane import, BIG DATA (ignored): reproduction of the T162 hand statement statement-closure-20261114T0623Z (bank
//! qualified-domain bounds applied, every key certified) from the EXISTING material through the import: the bank set
//! bank-conv/20261114c (qualified, recenter bounds as SELECTION bounds, labels set-unique), then the lane's rules in the
//! statement's stage order (bank, sc, symr, fp .. e, sc-r, p2rows, u) from their run tables, the 2 terminals; evidence:
//! the per-key regularity facts facts-20261114c (whatever files exist: a partial set is reported as such) and the
//! rr_proof span records of cert-closure-0623Z / cert-bankall-0623Z (bound by record_sha256). Tier C, both elided
//! policies; the comparison is typed and complete. (The last column of keys.jsonl is the key's (dots, rank) layer, not a
//! BFS round: the hand rounds are statement.json's total.)
//!
//! Paths (test data only): `RUSTRED_IMPORT_WS`, `RUSTRED_IMPORT_LARGE` (default /common/build-cache/vhirschi/
//! p_integral_dev/bank-conv), scratch `RUSTRED_IMPORT_SCRATCH`.
//! Run: `light_build.sh import test -p rustred-campaign-core --release --test import_t162 -- --ignored --nocapture`.

// ---- shim until I-INT adds `pub mod import;` to lib.rs (interfaces/import.md s.10)
#[allow(unused_imports)]
use rustred_campaign_core::{closure, cover, domain, key, order, poly, record, schema, symmetry};
#[allow(dead_code, unused_imports)]
#[path = "../src/import/mod.rs"]
mod import;

use std::path::{Path, PathBuf};

use import::lines::parse_key_text;
use import::{
    CheckOptions, Evidence, HandFormat, TableSetSpec, check, compare, load, read_hand_statement,
    write_statement,
};
use serde_json::{Value, json};

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}
fn large() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_LARGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/bank-conv"))
}
fn scratch() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
}
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

fn hand_format() -> HandFormat {
    HandFormat {
        key: 0,
        in_box: 3,
        label: 1,
        rule: 2,
        round: None,
        round_index: None,
        terminal_label: "terminal".into(),
    }
}

/// SHA256SUMS of a directory: verify every listed file that exists; returns (listed, present, verified).
fn verify_sums(dir: &Path) -> (usize, usize) {
    let (mut listed, mut ok) = (0, 0);
    for l in std::fs::read_to_string(dir.join("SHA256SUMS"))
        .unwrap()
        .lines()
    {
        let (h, n) = l.split_once(char::is_whitespace).unwrap();
        let n = n.trim_start().trim_start_matches('*');
        listed += 1;
        let p = dir.join(n);
        if p.is_file() {
            assert_eq!(
                import::lines::file_sha256(&p).unwrap(),
                h,
                "{n} differs from its SHA256SUMS"
            );
            ok += 1;
        }
    }
    (listed, ok)
}

fn build_spec(ws: &Path, large: &Path, stmt_dir: &Path) -> Value {
    let cat: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt_dir.join("catalogue.json")).unwrap())
            .unwrap();
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt_dir.join("statement.json")).unwrap())
            .unwrap();
    let runs = ws.join("SCRATCHPAD/t162-box/runs");
    let mut sources = vec![json!({
        "id":"bank","kind":"table_set","class":"bank",
        "priority_file": ws.join("SCRATCHPAD/bank-conv/out/20261114c/DISPATCH-PRIORITY.json").display().to_string(),
        "variant":"qualified","tables_dir": large.join("20261114c").display().to_string(),
        "sha256sums":[{"path": large.join("20261114c/SHA256SUMS").display().to_string(),
                       "sha256":"fbaa6454dc87a4ac4b2ed13067d75ea8183a01018078d455dbf0ad3f8b5aef12"}],
        "label_scope":"t162-set","domain":"qualified_recenter","hand_name":{"template":"bank#{label}"}})];
    // the lane's rules: consecutive catalogue entries of one run = one source (records in catalogue order)
    let mut cur: Option<(String, Vec<i64>)> = None;
    let push = |cur: &mut Option<(String, Vec<i64>)>, sources: &mut Vec<Value>| {
        if let Some((run, recs)) = cur.take() {
            let p = runs.join(&run).join("node/F.table.jsonl");
            assert!(p.is_file(), "{}", p.display());
            sources.push(json!({"id": run, "kind":"engine_table", "class": run.split('-').next().unwrap(),
                "path": p.display().to_string(), "records": recs, "hand_name":{"template":"{source}#{index}"}}));
        }
    };
    let mut n_ours = 0;
    for e in cat["entries"].as_array().unwrap() {
        if e["kind"] == "bank" {
            continue;
        }
        n_ours += 1;
        let name = e["name"].as_str().unwrap();
        let (run, idx) = name.split_once('#').unwrap();
        let idx: i64 = idx.parse().unwrap();
        match &mut cur {
            Some((r, v)) if r == run => v.push(idx),
            _ => {
                push(&mut cur, &mut sources);
                cur = Some((run.to_string(), vec![idx]));
            }
        }
    }
    push(&mut cur, &mut sources);
    assert_eq!(n_ours, 950);
    let terms: Vec<Vec<i32>> = st["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| parse_key_text(t.as_str().unwrap(), 20).unwrap())
        .collect();
    sources.push(json!({"id":"terminals","kind":"terminals","keys": terms}));
    let facts = large.join("facts-20261114c/facts");
    json!({"schema":"rustred.campaign.import-spec.v1","node":"T162","frame_from":"bank","bits":6,
        "start":{"path": stmt_dir.join("keys.jsonl").display().to_string(),"format":"hand_statement",
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"auto"},
        "sources": sources,
        "evidence":{"facts":[{"path": facts.display().to_string()}],
                    "proofs":[{"path": ws.join("SCRATCHPAD/t162-box/runs/cert-closure-0623Z").display().to_string()},
                              {"path": ws.join("SCRATCHPAD/t162-box/runs/cert-bankall-0623Z").display().to_string()}]}})
}

#[test]
#[ignore = "big data: the T162 hand statement, the bank set, facts and proofs; run with --ignored in release"]
fn t162_reproduction_of_the_hand_statement() {
    let (ws, large) = (ws(), large());
    let stmt = ws.join("SCRATCHPAD/t162-box/runs/statement-closure-20261114T0623Z");
    if !stmt.join("keys.jsonl").is_file() || !large.join("20261114c/SHA256SUMS").is_file() {
        println!("SKIP: T162 material missing");
        return;
    }
    assert_eq!(verify_sums(&stmt), (3, 3));
    let fdir = large.join("facts-20261114c");
    let (fl, fp) = if fdir.join("SHA256SUMS").is_file() {
        verify_sums(&fdir)
    } else {
        (0, 0)
    };
    println!("T162 facts-20261114c: SHA256SUMS lists {fl} files, {fp} present and verified");
    let out = scratch().join("t162");
    std::fs::create_dir_all(&out).unwrap();
    let spec_v = build_spec(&ws, &large, &stmt);
    std::fs::write(
        out.join("spec.T162.json"),
        serde_json::to_string_pretty(&spec_v).unwrap(),
    )
    .unwrap();
    let hand = read_hand_statement(&stmt.join("keys.jsonl"), &hand_format(), 20).unwrap();
    assert_eq!(hand.rows.len(), 65_908);
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("statement.json")).unwrap())
            .unwrap();
    println!(
        "T162 hand statement.json: keys {} rounds {} edges {}",
        st["keys"], st["rounds"], st["edges"]
    );
    for policy in ["block", "lift_with_per_key_proof"] {
        let t0 = std::time::Instant::now();
        let mut sv = spec_v.clone();
        sv["policies"]["elided"] = json!(policy);
        let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
        let set = load(&spec, Path::new("/")).unwrap();
        let t_load = t0.elapsed().as_secs_f64();
        assert_eq!(set.start.len(), 55_704);
        let t1 = std::time::Instant::now();
        let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
        let t_ev = t1.elapsed().as_secs_f64();
        let res = check(
            &set,
            &ev,
            &CheckOptions {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let dir = out.join(format!("statement-{policy}"));
        let timing = json!({"load_s": t_load, "evidence_s": t_ev, "check_s": res.seconds, "vm_hwm_kb": vm_hwm_kb()});
        write_statement(&set, &ev, &res, &dir, Some(timing)).unwrap();
        let cmp = compare(&set, &res, &hand);
        cmp.write(&dir).unwrap();
        let not_disp: usize = set
            .report
            .sources
            .iter()
            .map(|s| s.not_dispatched.len())
            .sum();
        println!(
            "T162 [{policy}]: load {:.1}s evidence {:.1}s check {:.1}s VmHWM {} MB; tables {} records {} (not dispatched {}); \
             evidence {:?}; keys ours {} hand {}; counts {:?}; closure verdict {:?} tier {:?} rounds {} edges {} \
             violations {} undecided {:?}; blocking {:?}",
            t_load,
            t_ev,
            res.seconds,
            vm_hwm_kb() / 1024,
            set.report.tables,
            set.report.records,
            not_disp,
            ev.counts,
            res.rows.len(),
            hand.rows.len(),
            res.counts(),
            res.summary.verdict,
            res.tier,
            res.summary.rounds,
            res.summary.edges,
            res.summary.descent_violations,
            res.summary.undecided,
            res.blocking()
        );
        println!(
            "T162 [{policy}] comparison: key sets equal {} same server {} terminals equal {} ({:?} vs {:?}) rounds {:?} vs {}; \
             differences {:?}; unmapped hand rules {}; label matrix {:?}; requests solve {} facts {} proofs {}",
            cmp.key_sets_equal,
            cmp.same_server,
            cmp.terminals_equal,
            cmp.terminals_hand,
            cmp.terminals_ours,
            cmp.rounds_hand,
            cmp.rounds_ours,
            cmp.counts,
            cmp.unmapped_hand_rules.len(),
            cmp.label_matrix,
            res.solve_requests.len(),
            res.fact_requests.len(),
            res.proof_requests.len()
        );
        for d in cmp.differences.iter().take(12) {
            println!("  diff {:?}", d);
        }
    }
}
