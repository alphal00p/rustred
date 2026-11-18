//! Lane import2 (SYMBOLICA FIRST): the import's verification of SUPPLIED SYMMETRY RELATIONS
//! (`rustred_campaign_core::import::symmetry::verify_relation_with`, generic over `CheckAlgebra`) run with the
//! Symbolica backend and with the core's certificate backend `Independent`: the verdicts must be IDENTICAL. Data: the
//! handoff family files (read-only); T178's 4 supplied relations (lane t178-close), T162's I(1^9; n9 = -1) =
//! I(1^9) + I(D4 = 0); planted wrong coefficients. SKIPs without the Symbolica licence (source WS/ops/env.sh).

mod common;
mod inputs;

use common::*;
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::import::lines::parse_key_text;
use rustred_campaign_core::import::symmetry::verify_relation_with;
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use serde_json::Value;

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

type Rel = (Vec<i32>, Vec<(Vec<i32>, String)>);

fn relations_of(path: &str, n: usize) -> Vec<Rel> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "rule")
        .map(|v| {
            let t = parse_key_text(v["target"].as_str().unwrap(), n).unwrap();
            let rhs = v["rhs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    (
                        parse_key_text(e[0].as_str().unwrap(), n).unwrap(),
                        e[1].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            (t, rhs)
        })
        .collect()
}

#[test]
fn import_symmetry_relations_symbolica_equals_independent() {
    if !licensed_or_skip("import_symmetry_relations_symbolica_equals_independent") {
        return;
    }
    let s = SymbolicaAlgebra::new().expect("licensed");
    let ho = format!("{WS}/handoff/sym-investigation-for-ben");
    let cases: Vec<(String, Vec<bool>, Vec<Rel>)> = {
        let mut v = vec![];
        let t178 = format!("{WS}/SCRATCHPAD/t178-close/data/symrules.table.jsonl");
        if std::path::Path::new(&t178).is_file() {
            v.push((
                format!("{ho}/families/T178.sym"),
                (0..20).map(|i| i < 10).collect(),
                relations_of(&t178, 20),
            ));
        }
        let k = |t: &str| parse_key_text(t, 20).unwrap();
        v.push((
            format!("{ho}/t162-bank/family/T162-presented-P3.sym"),
            (0..20).map(|i| i < 9).collect(),
            vec![(
                k("I(1,1,1,1,1,1,1,1,1,-1,0,0,0,0,0,0,0,0,0,0)"),
                vec![
                    (k("I(1,1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0)"), "1".into()),
                    (k("I(1,1,1,0,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0)"), "1".into()),
                ],
            )],
        ));
        v
    };
    let mut n_ok = 0;
    for (fam_path, chart, rels) in cases {
        let Ok(src) = std::fs::read_to_string(&fam_path) else {
            inputs::missing(format_args!("SKIP {fam_path}"));
            continue;
        };
        let fam = Family::parse(&src).unwrap();
        let vg = verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap();
        for (t, rhs) in rels {
            let a = verify_relation_with(&Independent, &vg, &t, &rhs, &[], 1 << 16);
            let b = verify_relation_with(&s, &vg, &t, &rhs, &[], 1 << 16);
            println!("{fam_path} {t:?}: independent {a:?} symbolica {b:?}");
            assert_eq!(a, b);
            assert!(b.verified());
            // planted: the first coefficient doubled is never in the span (both backends)
            let bad: Vec<(Vec<i32>, String)> = rhs
                .clone()
                .into_iter()
                .enumerate()
                .map(|(i, (k, c))| {
                    if i == 0 {
                        let q = rhs[0].1.clone();
                        let doubled = match q.split_once('/') {
                            Some((n, d)) => format!("{}/{d}", 2 * n.trim().parse::<i64>().unwrap()),
                            None => format!("{}", 2 * q.trim().parse::<i64>().unwrap()),
                        };
                        (k, doubled)
                    } else {
                        (k, c)
                    }
                })
                .collect();
            let a = verify_relation_with(&Independent, &vg, &t, &bad, &[], 1 << 16);
            let b = verify_relation_with(&s, &vg, &t, &bad, &[], 1 << 16);
            assert_eq!(a, b);
            assert!(!b.verified());
            n_ok += 1;
        }
    }
    println!(
        "{n_ok} relations verified identically by both backends (and their planted variants refused)"
    );
    assert!(n_ok >= 1);
}
