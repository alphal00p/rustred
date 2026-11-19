//! The independent checker's records with symmetry rows (h3/REPORT.md: "the 4 numerical fs rules fail without, pass
//! with our rows"; h3e: 59 of 135 fa rules declare sym_rows, pass with / fail without; h3g: 8 of 19 boxB rules): the
//! same verdicts here with the T162 group derived from the family (cross-checked against the checker's 12 maps).
//! `#[ignore]` (release): `-- --ignored`.

mod inputs;
mod rows_common;

use std::io::{BufRead, BufReader};

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    PointSpec, RowsOptions, RowsRule, SymUse, Verdict, identity_test,
};

fn run(file: &str, max_lines: usize) -> (usize, usize, usize, usize) {
    let path = format!("{HO}/t162-faces/{file}");
    let Ok(f) = std::fs::File::open(&path) else {
        inputs::missing(format_args!("SKIP {path}"));
        return (0, 0, 0, 0);
    };
    let fam = t162_family();
    let g = rows_common::sym_t162::t162_group(&fam);
    let with = RowsOptions {
        symmetry: Some(g.clone()),
        ..RowsOptions::default()
    };
    let without = RowsOptions {
        symmetry: Some(g),
        sym_use: SymUse::Without,
        ..RowsOptions::default()
    };
    let (mut sym, mut pass, mut fail_without, mut plain_pass) = (0, 0, 0, 0);
    let t0 = std::time::Instant::now();
    for (ln, line) in BufReader::with_capacity(1 << 20, f).lines().enumerate() {
        if ln >= max_lines {
            break;
        }
        let line = line.unwrap();
        for r in RowsRule::from_json_line(&line, 20).unwrap() {
            let mut pts = Vec::new();
            if r.fixed.iter().any(Option::is_none) {
                pts.extend([PointSpec::Generic, PointSpec::Generic]);
            } else {
                pts.push(PointSpec::Key(r.fixed.iter().map(|x| x.unwrap()).collect()));
            }
            let v = identity_test(&fam, &r, &pts, &with);
            if r.sym_rows.is_none() {
                assert_eq!(v.verdict, Verdict::Pass, "{file} line {ln}");
                plain_pass += 1;
                continue;
            }
            sym += 1;
            assert_eq!(
                v.verdict,
                Verdict::Pass,
                "{file} line {ln} rule {:?}: a FINDING if this fails",
                r.index
            );
            pass += 1;
            let w = identity_test(&fam, &r, &pts, &without);
            if matches!(w.verdict, Verdict::Fail { .. }) {
                fail_without += 1;
            } else {
                println!("{file} line {ln}: without symmetry rows {:?}", w.verdict);
            }
        }
    }
    println!(
        "{file}: {sym} sym_rows rules: {pass} pass with, {fail_without} fail without; {plain_pass} other rules pass ({:.1}s)",
        t0.elapsed().as_secs_f64()
    );
    (sym, pass, fail_without, plain_pass)
}

#[test]
#[ignore = "release: the 4 numerical fs rules (first 5 lines of the 756 MB fs export)"]
fn rows_sym_checker_fs_four() {
    let (s, p, f, _) = run("batch-01/fs.faces.jsonl", 5);
    if s > 0 {
        assert_eq!((s, p, f), (4, 4, 4));
    }
}

#[test]
#[ignore = "release, minutes: fa export (135 rules, 59 with sym_rows)"]
fn rows_sym_checker_fa() {
    let (s, p, f, o) = run("batch-02/fa.faces.jsonl", usize::MAX);
    if s > 0 {
        assert_eq!((s, p, f, s + o), (59, 59, 59, 135));
    }
}

#[test]
#[ignore = "release, minutes: boxB point rules (19, 8 with sym_rows)"]
fn rows_sym_checker_boxb() {
    let (s, p, f, o) = run("batch-03/boxB_point_rules.jsonl", usize::MAX);
    if s > 0 {
        assert_eq!((s, p, f, s + o), (8, 8, 8, 19));
    }
}
