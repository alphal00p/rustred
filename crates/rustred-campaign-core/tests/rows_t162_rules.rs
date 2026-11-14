//! Agreement with the independent checker (h3 lineage) on the exported T162 face rules, through the BATCH API
//! (`identity_test_batch`, streaming the export files read-only from the handoff copy; the fs export is 756 MB with a
//! 123 MB line). For every rule with a `done` checker record (W/h3/results.jsonl, h3b/results_fsA.jsonl,
//! h3c/results_fsB_merged.jsonl, h3d/results_fd.jsonl, h3d/results_fp.jsonl: 377 records), the A2 test at two generic
//! points must give the checker's C1 verdict (all `pass`), and the rank and column count of each generic cell must
//! equal the checker's C1 points. Rules that declare `sym_rows` are UNKNOWN (needs symmetry rows) and are counted as
//! excluded. `#[ignore]` (release, minutes): run with `-- --ignored`.
//!
//! Also (default run): the k3 near-corner fixture F4: of the 16 covers the checker verified, the rules without
//! symmetry rows must PASS at their key; the ones with symmetry rows are UNKNOWN (needs symmetry rows).

mod rows_common;

use std::collections::HashMap;
use std::io::BufReader;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    LineContext, PointSpec, PointStatus, RowsOptions, RowsRule, Unsupported, Verdict, identity_test,
    identity_test_batch,
};
use serde_json::Value;

const W: &str = "/common/ben/sym-investigation";

/// (line0, rule index) -> (checker C1 verdict, sym_rows, C1 points [(rank, ncols)])
fn checker(files: &[&str]) -> HashMap<(u64, i64), (String, bool, Vec<(u64, u64)>)> {
    let mut m = HashMap::new();
    for f in files {
        let Ok(t) = std::fs::read_to_string(format!("{W}/{f}")) else {
            println!("SKIP: {W}/{f} not readable");
            return HashMap::new();
        };
        for l in t.lines() {
            let r: Value = serde_json::from_str(l).unwrap();
            if r["status"] != "done" {
                continue;
            }
            let pts = r["C1"]["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p["rank"].as_u64().unwrap(), p["ncols"].as_u64().unwrap()))
                .collect();
            let sym = r["size"]["sym_rows"].as_i64().unwrap_or(0) != 0 || r["size"]["sym_rows"].as_bool() == Some(true);
            m.insert(
                (r["line"].as_u64().unwrap(), r["rule_index"].as_i64().unwrap()),
                (r["C1"]["verdict"].as_str().unwrap().to_string(), sym, pts),
            );
        }
    }
    m
}

fn agree(export: &str, checker_files: &[&str], expect_records: usize) {
    let ck = checker(checker_files);
    if ck.is_empty() {
        return;
    }
    assert_eq!(ck.len(), expect_records, "{export}: checker records");
    let path = format!("{HO}/t162-faces/{export}");
    let Ok(f) = std::fs::File::open(&path) else {
        println!("SKIP: {path}");
        return;
    };
    let fam = t162_family();
    let opts = RowsOptions::default();
    let plan = |_: &RowsRule, _: &LineContext| vec![PointSpec::Generic, PointSpec::Generic];
    let mut out: Vec<u8> = Vec::new();
    let sum = identity_test_batch(&fam, BufReader::with_capacity(1 << 20, f), &plan, &opts, &mut out).unwrap();
    println!("{export}: batch summary {}", serde_json::to_string(&sum).unwrap());
    let (mut compared, mut excluded, mut equal_numbers, mut seen) = (0, 0, 0, 0);
    let mut slow: Vec<(f64, String)> = Vec::new();
    for l in std::str::from_utf8(&out).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        let line0 = v["line_no"].as_u64().unwrap() - 1;
        let res = &v["result"];
        let idx = res["rule_index"].as_i64().unwrap();
        let Some((cv, sym, pts)) = ck.get(&(line0, idx)) else { continue };
        seen += 1;
        let kind = res["verdict"].as_str().map(str::to_string).unwrap_or_else(|| {
            res["verdict"].as_object().unwrap().keys().next().unwrap().clone()
        });
        slow.push((res["cost"]["seconds"].as_f64().unwrap(), format!("{} line {line0}", v["run"])));
        if *sym {
            assert!(
                res["verdict"].get("Unsupported").is_some(),
                "{export} line {line0}: sym_rows rule must be UNKNOWN, got {kind}"
            );
            excluded += 1;
            continue;
        }
        assert_eq!(cv, "pass");
        assert_eq!(kind, "Pass", "{export} line {line0} rule {idx}: {}", res["verdict"]);
        compared += 1;
        // generic-point numbers vs the checker's C1 points (same for every point of a rule)
        let (rank, ncols) = pts[0];
        let cells: Vec<(u64, u64)> = res["points"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["cells"].as_array().unwrap().iter())
            .map(|c| (c["rank"].as_u64().unwrap(), c["row_columns"].as_u64().unwrap()))
            .collect();
        if cells.iter().all(|&c| c == (rank, ncols)) {
            equal_numbers += 1;
        } else {
            println!("{export} line {line0}: ours {cells:?} checker {pts:?}");
        }
    }
    slow.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!(
        "{export}: {seen} checker records met, {compared} compared (all Pass = checker pass), {excluded} excluded (sym_rows), \
         {equal_numbers} with equal rank/ncols at both generic points; slowest {:?}",
        &slow[..slow.len().min(3)]
    );
    assert_eq!(seen, expect_records);
    assert_eq!(equal_numbers, compared);
}

#[test]
#[ignore = "release, ~1 min: 155 rules of batch-01/f.faces.jsonl"]
fn rows_agree_with_checker_f() {
    agree("batch-01/f.faces.jsonl", &["h3/results.jsonl"], 155);
}

#[test]
#[ignore = "release, minutes: 141 rules of batch-01/fs.faces.jsonl (756 MB, one 123 MB line)"]
fn rows_agree_with_checker_fs() {
    agree("batch-01/fs.faces.jsonl", &["h3b/results_fsA.jsonl", "h3c/results_fsB_merged.jsonl"], 141);
}

#[test]
#[ignore = "release, minutes: 74 point rules of batch-02/fp.faces.jsonl"]
fn rows_agree_with_checker_fp() {
    agree("batch-02/fp.faces.jsonl", &["h3d/results_fp.jsonl"], 74);
}

#[test]
#[ignore = "release, seconds: 7 equation-case rules of batch-02/fd.faces.jsonl"]
fn rows_agree_with_checker_fd() {
    agree("batch-02/fd.faces.jsonl", &["h3d/results_fd.jsonl"], 7);
}

/// k3 F4: the 16 near-corner covers verified by the checker (h16): rule `#k` of run named in `claimed`, at
/// `rule_file` line `rule_line0`.
#[test]
fn rows_f4_nearcorner_verified_covers() {
    let Some(o) = oracle() else { return };
    let fam = t162_family();
    let opts = RowsOptions::default();
    let (mut pass, mut sym) = (0, 0);
    // entries grouped by file; each file streamed once (f1.faces.jsonl is 1.6 GB), lines read one at a time
    let mut by_file: std::collections::BTreeMap<String, Vec<Value>> = std::collections::BTreeMap::new();
    for e in o.jsonl("T162/F4_nearcorner.jsonl") {
        if e["verified_by_us"] == true {
            by_file.entry(e["rule_file"].as_str().unwrap().to_string()).or_default().push(e);
        }
    }
    for (file, mut es) in by_file {
        es.sort_by_key(|e| e["rule_line0"].as_u64().unwrap());
        let path = format!("{HO}/t162-faces/{file}");
        let Ok(f) = std::fs::File::open(&path) else {
            println!("SKIP: {path}");
            return;
        };
        let mut rd = BufReader::with_capacity(1 << 20, f);
        let mut buf: Vec<u8> = Vec::new();
        let mut at = 0usize;
        for e in es {
            let line0 = e["rule_line0"].as_u64().unwrap() as usize;
            loop {
                buf.clear();
                assert!(std::io::BufRead::read_until(&mut rd, b'\n', &mut buf).unwrap() > 0, "{file}: short");
                at += 1;
                if at - 1 == line0 {
                    break;
                }
            }
            let claimed = e["claimed"].as_str().unwrap();
            let k: i64 = claimed.split('#').nth(1).unwrap().parse().unwrap();
            let rules = RowsRule::from_json_line(std::str::from_utf8(&buf).unwrap().trim_end(), 20).unwrap();
            // export wrapper: rule `#k` by index; a bare point-rule line (batch-03) holds exactly one rule
            let rule = if rules.len() == 1 { &rules[0] } else { rules.iter().find(|r| r.index == Some(k)).unwrap() };
            let key: Vec<i64> = e["key"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
            let v = identity_test(&fam, rule, &[PointSpec::Key(key), PointSpec::Generic], &opts);
            println!("{}", summary(claimed, &v));
            if e["sym_rows"] == true {
                assert!(matches!(v.verdict, Verdict::Unsupported(Unsupported::NeedsSymmetryRows { .. })));
                sym += 1;
            } else {
                assert_eq!(v.verdict, Verdict::Pass, "{claimed}");
                assert_eq!(v.points[0].status, PointStatus::Pass);
                pass += 1;
            }
        }
    }
    println!("F4 near-corner: {pass} pass, {sym} UNKNOWN (needs symmetry rows)");
    if pass + sym > 0 {
        assert_eq!((pass, sym), (4, 12));
    }
}
