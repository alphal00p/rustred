//! Lane tworow, item 4: the SECOND ROUTE (Symbolica polynomial expansion, `tworow_check::second_route`) compared term
//! by term with the core's combinations; the same search run with the Symbolica backend gives identical relations;
//! a planted coefficient is detected. Real nodes: `tworow_second_route_on_results` (ignored; reads the JSONL of the
//! core's `tworow_real` test). SKIPs without the Symbolica licence (source WS/ops/env.sh).

mod common;

use std::io::BufRead;
use std::path::Path;

use common::licensed_or_skip;
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_algebra::tworow_check::second_route;
use rustred_campaign_core::import::symmetry::{GroupSpec, load_verified_group};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::order::{NodeComparator, NodeOrderRecord};
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Q};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use rustred_campaign_core::termmin::{SymRelation, symmetry_relation_at_key};
use rustred_campaign_core::tworow::{Combination, PairShift, RowRef, RowSpec, TwoRowPolicy, search_key};
use serde_json::{Value, json};

const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";
const BANANA3: &str = "I(name(B),loops(k1,k2,k3),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k3)^2,1),prop(D4,(k1+k2+k3+q)^2,1),prop(D5,(k1+q)^2,0),prop(D6,(k2+q)^2,0),prop(D7,(k3+q)^2,0),prop(D8,(k1-k2)^2,0),prop(D9,(k1-k3)^2,0),gram(q,q,1))";

fn box_keys(chart: &[bool], max_line: i32, max_rank: i32) -> Vec<Vec<i32>> {
    let mut out = vec![vec![]];
    for &l in chart {
        let vals: Vec<i32> = if l {
            (1..=max_line).collect()
        } else {
            (-max_rank..=0).rev().collect()
        };
        out = out
            .iter()
            .flat_map(|k: &Vec<i32>| {
                vals.iter().map(move |&v| {
                    let mut k2 = k.clone();
                    k2.push(v);
                    k2
                })
            })
            .collect();
    }
    out.retain(|k| {
        k.iter()
            .zip(chart)
            .filter(|(_, l)| !**l)
            .map(|(v, _)| -v)
            .sum::<i32>()
            <= max_rank
    });
    out
}

#[test]
fn second_route_agrees_and_symbolica_search_is_identical() {
    if !licensed_or_skip("second_route_agrees_and_symbolica_search_is_identical") {
        return;
    }
    let sym = SymbolicaAlgebra::new().unwrap();
    let policy = TwoRowPolicy::default();
    let (mut compared, mut relations, mut planted) = (0usize, 0usize, 0usize);
    for (text, lines, max_line, max_rank) in [(SUNSET, 3usize, 2, 3), (BANANA3, 4, 2, 2)] {
        let f = Family::parse(text).unwrap();
        let chart: Vec<bool> = (0..f.n()).map(|i| i < lines).collect();
        let g = verify_derived(&f, &chart, &DeriveOptions::default()).unwrap();
        let c = NodeComparator::legacy(f.n(), None).unwrap();
        let cmp = |a: &[i32], b: &[i32]| c.cmp(a, b);
        for k in box_keys(&chart, max_line, max_rank) {
            if matches!(
                symmetry_relation_at_key(&Independent, &g, &k, &cmp, 65_536),
                SymRelation::Relation { .. }
            ) {
                continue;
            }
            let ri = search_key(&Independent, &k, &g, &cmp, &policy, None);
            let rs = search_key(&sym, &k, &g, &cmp, &policy, None);
            // the same search with the Symbolica arithmetic: identical outcome and relations
            assert_eq!(ri.outcome, rs.outcome, "backends differ at {k:?}");
            if let rustred_campaign_core::tworow::KeyOutcome::Served { relations: rels } = &ri.outcome {
                for comb in rels {
                    relations += 1;
                    let sr = second_route(&g, comb, &cmp, policy.max_terms_per_row, None).unwrap();
                    assert!(sr.equal, "{:?} for {}", sr.difference, comb.to_json());
                    assert_eq!(sr.not_lower, None);
                    compared += sr.terms_compared;
                    // planted: one coefficient changed -> detected
                    if let Some((_, q)) = comb.rhs.first() {
                        let mut bad = comb.clone();
                        bad.rhs[0].1 = Independent.q_add(*q, Q::ONE).unwrap();
                        let sb = second_route(&g, &bad, &cmp, policy.max_terms_per_row, None).unwrap();
                        assert!(!sb.equal);
                        planted += 1;
                    }
                }
            }
        }
    }
    println!(
        "{}",
        json!({"relations": relations, "terms_compared": compared, "planted_detected": planted})
    );
    assert!(relations > 0 && planted > 0);
}

fn comb_from_json(v: &Value) -> Combination {
    let q = |t: &Value| Independent.q_parse(t.as_str().unwrap()).unwrap();
    let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
    Combination {
        key,
        rows: v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| RowRef {
                spec: RowSpec {
                    element: r["element"].as_u64().unwrap() as usize,
                    shift: r["shift"].as_str().map(|s| PairShift::parse(s).unwrap()),
                },
                seed: serde_json::from_value(r["seed"].clone()).unwrap(),
                terms: r["terms"].as_u64().unwrap() as usize,
            })
            .collect(),
        weights: v["weights"].as_array().unwrap().iter().map(q).collect(),
        key_coefficients: v["key_coefficients"].as_array().unwrap().iter().map(q).collect(),
        rhs: v["rhs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (serde_json::from_value(e[0].clone()).unwrap(), q(&e[1])))
            .collect(),
    }
}

/// REAL: every relation in `TWOROW_RESULTS` (the core `tworow_real` JSONL) recomputed by the second route; the frame as
/// in the core test (`TWOROW_HEADER_TABLE`, or `TWOROW_FAMILY` + `TWOROW_ORDER` + `TWOROW_CHART`).
#[test]
#[ignore]
fn tworow_second_route_on_results() {
    if !licensed_or_skip("tworow_second_route_on_results") {
        return;
    }
    let base = Path::new("/");
    let (g, cmpr) = if let Ok(tp) = std::env::var("TWOROW_HEADER_TABLE") {
        let mut l = String::new();
        std::io::BufReader::new(std::fs::File::open(&tp).unwrap())
            .read_line(&mut l)
            .unwrap();
        let h: Value = serde_json::from_str(&l).unwrap();
        let cmp = NodeComparator::from_header(&h).unwrap();
        let chart: Vec<bool> = h["mask"].as_str().unwrap().bytes().map(|b| b == b'1').collect();
        let gs: GroupSpec = serde_json::from_value(json!({})).unwrap();
        let (g, _) = load_verified_group(
            &gs,
            base,
            &chart,
            cmp.presentation().new_to_old(),
            h["family_source"].as_str(),
        )
        .unwrap();
        (g, cmp)
    } else {
        let fam = std::env::var("TWOROW_FAMILY").unwrap();
        let ord: NodeOrderRecord =
            serde_json::from_str(&std::fs::read_to_string(std::env::var("TWOROW_ORDER").unwrap()).unwrap())
                .unwrap();
        let chart: Vec<bool> = std::env::var("TWOROW_CHART")
            .unwrap()
            .bytes()
            .map(|b| b == b'1')
            .collect();
        let n = chart.len();
        let cmp = NodeComparator::from_node_order(&ord, n).unwrap();
        let gs: GroupSpec = serde_json::from_value(json!({"family": {"path": fam}})).unwrap();
        let id: Vec<usize> = (0..n).collect();
        let (g, _) = load_verified_group(&gs, base, &chart, &id, None).unwrap();
        (g, cmp)
    };
    let cmp = |a: &[i32], b: &[i32]| cmpr.cmp(a, b);
    let (mut n, mut equal, mut terms) = (0usize, 0usize, 0usize);
    let mut bad = vec![];
    for l in std::fs::read_to_string(std::env::var("TWOROW_RESULTS").unwrap())
        .unwrap()
        .lines()
    {
        let v: Value = serde_json::from_str(l).unwrap();
        for r in v["relations"].as_array().into_iter().flatten() {
            let c = comb_from_json(r);
            n += 1;
            let sr = second_route(&g, &c, &cmp, 1 << 20, None).unwrap();
            terms += sr.terms_compared;
            if sr.equal && sr.not_lower.is_none() {
                equal += 1;
            } else {
                bad.push(json!({"key": c.key, "difference": format!("{:?}", sr.difference),
                    "not_lower": sr.not_lower}));
            }
        }
    }
    println!(
        "{}",
        json!({"relations": n, "equal_and_descending": equal, "terms_compared": terms, "bad": bad})
    );
    assert_eq!(n, equal);
}
