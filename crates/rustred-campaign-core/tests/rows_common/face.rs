//! The audit-A boundary-face construction (experiment `e3_boundary_face`, LA3_handwritten.sym, 3 loops, 9 slots): a
//! TRUE rule (one IBP row solved for its top term) and a FALSE copy with an extra term `1*I(Z)` whose column is
//! scaleless in the generic convention but not on the face n_j = 0; and its served key set (incl. face keys). Shared by
//! rows_audit_regressions.rs and rows_mutation_replay.rs (moved here by rows3).
#![allow(dead_code)]

use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Q};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::{Value, json};

use super::data;

pub fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

pub fn form_text(form: &[Q], m: i128, n: usize) -> String {
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
        terms.push(if var.is_empty() {
            format!("{c}")
        } else {
            format!("{c}*{var}")
        });
    }
    let s = terms.join("+").replace("+-", "-");
    if s.is_empty() { "0".into() } else { s }
}

pub fn pat(shift: &[i64]) -> String {
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

pub struct Face {
    pub fam: Family,
    pub good: Value,
    pub bad: Value,
    pub j: usize,
    pub tshift: Vec<i64>,
}

pub fn face() -> Face {
    let fam = Family::parse(&data("LA3_handwritten.sym")).unwrap();
    let n = fam.n();
    let mut found = None;
    'o: for m in 1u32..(1 << 7) {
        let s: Vec<bool> = (0..n).map(|i| i < 7 && m >> i & 1 == 1).collect();
        if !fam.zero_mask(&s) {
            continue;
        }
        for j in [7usize, 8] {
            let mut t = s.clone();
            t[j] = true;
            if !fam.zero_mask(&t) {
                found = Some((s, j));
                break 'o;
            }
        }
    }
    let (s, j) = found.unwrap();
    let row = fam.exact_row(0).unwrap();
    let (tshift, tform) = row
        .iter()
        .filter(|(_, f)| f[1..=n].iter().any(|q| !q.is_zero()))
        .max_by_key(|(s, _)| s.iter().sum::<i64>())
        .map(|(s, f)| (s.clone(), f.clone()))
        .unwrap();
    let mut mm: i128 = 1;
    for f in row.values() {
        for q in f {
            mm = mm / gcd(mm, q.d) * q.d;
        }
    }
    let den = form_text(&tform, mm, n);
    let mut rhs: Vec<Value> = row
        .iter()
        .filter(|(sh, _)| **sh != tshift)
        .map(|(sh, f)| {
            let neg: Vec<Q> = f.iter().map(|q| Independent.q_neg(*q).unwrap()).collect();
            json!([pat(sh), format!("({})/({den})", form_text(&neg, mm, n))])
        })
        .collect();
    let chart: String = (0..n).map(|i| if i < 7 { '1' } else { '0' }).collect();
    let good = json!({
        "type": "rule", "index": 0, "chart": chart, "fixed": vec![Value::Null; n],
        "equations": [], "exceptions": [], "target": pat(&tshift), "rhs": rhs.clone(),
        "seeds": [pat(&vec![0; n])], "recenter": vec![0; n], "witness_loci": [], "guard_version": 2,
    });
    let zparts: Vec<String> = (0..n)
        .map(|i| {
            if i < 7 && !s[i] {
                "0".into()
            } else {
                let o = tshift[i] + i64::from(i == j);
                match o {
                    0 => format!("n{i}"),
                    o if o > 0 => format!("n{i}+{o}"),
                    o => format!("n{i}{o}"),
                }
            }
        })
        .collect();
    rhs.push(json!([format!("I({})", zparts.join(",")), "1"]));
    let mut bad = good.clone();
    bad["rhs"] = json!(rhs);
    Face {
        fam,
        good,
        bad,
        j,
        tshift,
    }
}

pub fn served(f: &Face) -> Vec<Vec<i64>> {
    let n = f.fam.n();
    let mut out: Vec<Vec<i64>> = Vec::new();
    for t in 0..400i64 {
        let mut pt: Vec<i64> = (0..n)
            .map(|i| {
                let h = (t * 2654435761 + (i as i64) * 40503).rem_euclid(1000003);
                if i < 7 { 1 + h % 4 } else { -1 - (h / 7) % 4 }
            })
            .collect();
        if pt[f.j] == 0 {
            pt[f.j] = -1;
        }
        out.push((0..n).map(|i| pt[i] + f.tshift[i]).collect());
    }
    for a in 0..3i64 {
        let mut pt: Vec<i64> = (0..n)
            .map(|i| {
                if i < 7 {
                    2 + (a + i as i64) % 3
                } else {
                    -1 - (i as i64) % 3
                }
            })
            .collect();
        pt[f.j] = -f.tshift[f.j];
        out.push((0..n).map(|i| pt[i] + f.tshift[i]).collect());
    }
    out.sort();
    out.dedup();
    out
}
