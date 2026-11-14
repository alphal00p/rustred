//! Genericity (R41 P-5 / lane rule L8): the family parser, row generator and A2 test on families of other loop
//! orders and arities than the five-loop fixtures, from the engine's own test data (`rustred-core/tests/support/
//! eps_families`: T001c 2 loops / 5 slots with two eps lines, bubu = T148 4 loops / 14 slots, T070c 4 loops / 14
//! slots with one eps line) and one hand-written 3-loop family (LA3_handwritten.sym, 9 slots). No rule tables exist
//! for these families, so the rules are BUILT from the exact IBP operators: target = the shift of one operator row at
//! a symbolic seed with the most entries, RHS = its other shifts, coefficients -form_k / form_T as "(P)/(Q)" text in
//! n and d (an identity by construction). Each must PASS at two generic points and at an integer key, at both primes,
//! with the mutation control rejected; its coefficient x2 copy and a shifted-RHS copy must FAIL.

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{PointSpec, PointStatus, RowsOptions, RowsRule, Verdict, identity_test};
use rustred_campaign_core::replay_base::arith::Q;
use rustred_campaign_core::replay_base::family::Family;
use serde_json::json;

/// Line flags from the family text (`prop(D<k>, (...)^2, 1)` = line).
fn line_flags(src: &str) -> Vec<bool> {
    let compact: String = src.chars().filter(|c| !c.is_whitespace()).collect();
    compact
        .split("prop(D")
        .skip(1)
        .map(|part| {
            let close = part.find(")^2,").expect("prop form");
            part[close + 4..].starts_with('1')
        })
        .collect()
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// A linear form [const, y_0.., y_{n-1}, d] scaled by `m` (integral) as polynomial text in n_i (y = n + shift) and d.
fn form_text(form: &[Q], m: i128, n: usize) -> String {
    let mut terms = Vec::new();
    for (j, q) in form.iter().enumerate() {
        if q.is_zero() {
            continue;
        }
        let c = q.n * (m / q.d);
        let var = match j {
            0 => String::new(),
            j if j == n + 1 => "d".into(),
            j => format!("n{}", j - 1),
        };
        terms.push(if var.is_empty() { format!("{c}") } else { format!("{c}*{var}") });
    }
    let mut s = terms.join("+").replace("+-", "-");
    if s.is_empty() {
        s = "0".into();
    }
    s
}

fn pat(shift: &[i64]) -> String {
    let parts: Vec<String> = shift
        .iter()
        .enumerate()
        .map(|(i, &o)| match o {
            0 => format!("n{i}"),
            o if o > 0 => format!("n{i}+{o}"),
            o => format!("n{i}{o}"),
        })
        .collect();
    format!("I({})", parts.join(","))
}

/// A symbolic rule built from operator `op` of the family at the seed I(n0, ..., n{N-1}).
fn built_rule(fam: &Family, lines: &[bool], op: usize) -> serde_json::Value {
    let n = fam.n;
    let row = fam.exact_row(op).unwrap();
    // target: the shift whose form has an index variable (generic nonzero), preferring a raising shift
    let (tshift, tform) = row
        .iter()
        .filter(|(_, f)| f[1..=n].iter().any(|q| !q.is_zero()))
        .max_by_key(|(s, _)| s.iter().sum::<i64>())
        .map(|(s, f)| (s.clone(), f.clone()))
        .expect("operator with an index-dependent entry");
    let mut m: i128 = 1;
    for f in row.values() {
        for q in f {
            m = m / gcd(m, q.d) * q.d;
        }
    }
    let den = form_text(&tform, m, n);
    let rhs: Vec<serde_json::Value> = row
        .iter()
        .filter(|(s, _)| **s != tshift)
        .map(|(s, f)| json!([pat(s), format!("(-({}))/({den})", form_text(f, m, n)).replace("(-(", "(-1*(")]))
        .collect();
    let chart: String = lines.iter().map(|&l| if l { '1' } else { '0' }).collect();
    json!({
        "type": "rule", "index": op, "chart": chart, "fixed": vec![serde_json::Value::Null; n],
        "equations": [], "exceptions": [], "target": pat(&tshift), "rhs": rhs,
        "seeds": [pat(&vec![0; n])], "recenter": vec![0; n], "witness_loci": [],
    })
}

/// The built coefficient text uses "(-1*(P))/(Q)", which the strict grammar refuses (nested parentheses): expand
/// the sign into the polynomial instead.
fn negate_poly_text(p: &str) -> String {
    let mut out = String::new();
    for (i, ch) in p.char_indices() {
        match ch {
            '+' if i > 0 => out.push('-'),
            '-' if i > 0 => out.push('+'),
            '-' => {}
            c => {
                if i == 0 {
                    out.push('-');
                }
                out.push(c);
            }
        }
    }
    out
}

fn strict_rule(mut v: serde_json::Value) -> serde_json::Value {
    for t in v["rhs"].as_array_mut().unwrap() {
        let c = t[1].as_str().unwrap().to_string();
        let inner = c.strip_prefix("(-1*(").unwrap();
        let (p, q) = inner.split_once("))/(").unwrap();
        t[1] = json!(format!("({})/({}", negate_poly_text(p), q));
    }
    v
}

fn check_family(file: &str, loops: usize, arity: usize, eps_lines: usize) {
    let src = data(file);
    let fam = Family::parse(&src).unwrap();
    let lines = line_flags(&src);
    assert_eq!((fam.loops, fam.n, lines.len()), (loops, arity, arity), "{file}");
    assert_eq!(fam.n_ops(), loops * (loops + 1), "{file}: L(L+1) operators");
    assert_eq!(fam.eps_axes.len(), eps_lines, "{file}: eps lines");
    assert!(!fam.zero_mask(&lines), "{file}: the top sector is not scaleless");
    assert!(fam.zero_mask(&vec![false; arity]), "{file}: the empty sector is scaleless");
    let opts = RowsOptions::default();
    // an integer key: lines at 2, numerators at -1, shifted by the target offsets of the built rule
    let mut tested = 0;
    for op in [0, fam.n_ops() - 1] {
        let rule_v = strict_rule(built_rule(&fam, &lines, op));
        let rule = RowsRule::from_value(rule_v.clone(), arity).unwrap();
        let point: Vec<i64> = lines.iter().map(|&l| if l { 3 } else { -2 }).collect();
        let key: Vec<i64> = rule
            .target
            .iter()
            .enumerate()
            .map(|(i, (_, off))| point[i] + off)
            .collect();
        let pts = vec![PointSpec::Generic, PointSpec::Generic, PointSpec::Key(key)];
        let v = identity_test(&fam, &rule, &pts, &opts);
        println!("{}", summary(&format!("{file} op {op}"), &v));
        assert_eq!(v.verdict, Verdict::Pass, "{file} op {op}: {:?}", v.verdict);
        assert!(v.meets_a2_policy);
        for p in &v.points {
            assert_eq!(p.status, PointStatus::Pass);
            assert!(p.cells.iter().all(|c| c.mutation_rejected == Some(true) && c.rows == loops * (loops + 1)));
        }
        // coefficient x2 of rhs[0]
        let mut bad = rule_v.clone();
        let c = bad["rhs"][0][1].as_str().unwrap().to_string();
        let (p, q) = c.split_once(")/(").unwrap();
        bad["rhs"][0][1] = json!(format!("(2*{})/({q}", p.trim_start_matches('(').replace('+', "+2*").replace('-', "-2*")).replace("(2*-2*", "(-2*"));
        let vb = identity_test(&fam, &RowsRule::from_value(bad, arity).unwrap(), &pts, &opts);
        assert!(matches!(vb.verdict, Verdict::Fail { .. }), "{file} op {op} x2: {:?}", vb.verdict);
        // rhs[0] key shifted by +1 on slot 0
        let mut bad = rule_v.clone();
        let k = bad["rhs"][0][0].as_str().unwrap().to_string();
        let first = k[2..].split(',').next().unwrap().to_string();
        let shifted = match first.split_once('+').map(|(_, o)| o.parse::<i64>().unwrap()).or_else(|| {
            first.split_once('-').map(|(_, o)| -o.parse::<i64>().unwrap())
        }) {
            Some(o) if o + 1 == 0 => "n0".to_string(),
            Some(o) if o + 1 > 0 => format!("n0+{}", o + 1),
            Some(o) => format!("n0{}", o + 1),
            None => "n0+1".to_string(),
        };
        bad["rhs"][0][0] = json!(k.replacen(&format!("I({first},"), &format!("I({shifted},"), 1));
        let vb = identity_test(&fam, &RowsRule::from_value(bad, arity).unwrap(), &pts, &opts);
        assert!(matches!(vb.verdict, Verdict::Fail { .. }), "{file} op {op} shifted key: {:?}", vb.verdict);
        tested += 1;
    }
    assert_eq!(tested, 2);
}

#[test]
fn rows_arity_5_two_loops_two_eps_lines() {
    check_family("T001c.sym", 2, 5, 2);
}

#[test]
fn rows_arity_9_three_loops_handwritten() {
    check_family("LA3_handwritten.sym", 3, 9, 0);
}

#[test]
fn rows_arity_14_four_loops() {
    check_family("bubu.sym", 4, 14, 0);
}

#[test]
fn rows_arity_14_four_loops_one_eps_line() {
    check_family("T070c.sym", 4, 14, 1);
}
