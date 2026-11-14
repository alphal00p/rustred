//! Port of audit B's mutation campaign (WS/notes/implementer.audit-rows-B.md, harness audit-rows-B/harness) as a
//! regression baseline: true rules pass over 5 seeds; every visible single mutation of a true rule is detected (Fail,
//! or refused: Pole / NotApplicable / Unsupported), NONE passes. Grammar-preserving classes: RHS key +1 / -1 on one slot,
//! drop a term, duplicate a term (= coefficient x2), swap the coefficients of two terms, target +1 (raw pivot removed).
//! Rules: the 7 clean F6 rules (k3 fixtures) and rules built from exact IBP rows of T001c (arity 5, eps lines),
//! LA3_handwritten (9) and bubu (14). `#[ignore]` (release, minutes): `-- --ignored`.

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{PointSpec, RowsOptions, RowsRule, Verdict, identity_test};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Q};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::{Value, json};

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
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
        terms.push(if var.is_empty() {
            format!("{c}")
        } else {
            format!("{c}*{var}")
        });
    }
    let s = terms.join("+").replace("+-", "-");
    if s.is_empty() { "0".into() } else { s }
}

/// A true rule from IBP operator `op` at the seed I(n): its top term solved for.
fn built(text: &str, op: usize) -> (Family, Value) {
    let fam = Family::parse(text).unwrap();
    let n = fam.n();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let chart: String = compact
        .split("prop(")
        .skip(1)
        .map(|p| {
            if p.contains(",1)") && p.find(",1)") < p.find("prop").or(Some(usize::MAX)) {
                '1'
            } else {
                '0'
            }
        })
        .collect();
    let row = fam.exact_row(op).unwrap();
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
    let rhs: Vec<Value> = row
        .iter()
        .filter(|(sh, _)| **sh != tshift)
        .map(|(sh, f)| {
            let neg: Vec<Q> = f.iter().map(|q| Independent.q_neg(*q).unwrap()).collect();
            json!([pat(sh), format!("({})/({den})", form_text(&neg, mm, n))])
        })
        .collect();
    let r = json!({"type": "rule", "index": op, "chart": chart, "fixed": vec![Value::Null; n], "equations": [],
        "exceptions": [], "target": pat(&tshift), "rhs": rhs, "seeds": [pat(&vec![0; n])], "recenter": vec![0; n],
        "witness_loci": [], "guard_version": 2});
    (fam, r)
}

fn shift_key(k: &str, slot: usize, by: i64) -> String {
    let inner = &k[2..k.len() - 1];
    let parts: Vec<String> = inner
        .split(',')
        .enumerate()
        .map(|(i, x)| {
            if i != slot {
                return x.to_string();
            }
            if let Some(rest) = x.strip_prefix('n') {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let off: i64 = rest[digits.len()..]
                    .trim_start_matches('+')
                    .parse()
                    .unwrap_or(0)
                    + by;
                match off {
                    0 => format!("n{digits}"),
                    o if o > 0 => format!("n{digits}+{o}"),
                    o => format!("n{digits}{o}"),
                }
            } else {
                (x.parse::<i64>().unwrap() + by).to_string()
            }
        })
        .collect();
    format!("I({})", parts.join(","))
}

fn mutants(r: &Value, max_terms: usize) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let rhs = r["rhs"].as_array().unwrap().clone();
    let n = r["fixed"].as_array().unwrap().len();
    for t in 0..rhs.len().min(max_terms) {
        let k = rhs[t][0].as_str().unwrap();
        for by in [1i64, -1] {
            let slot = (t * 7 + 3) % n;
            let mut m = r.clone();
            m["rhs"][t][0] = json!(shift_key(k, slot, by));
            out.push((format!("key{by:+}@{t}"), m));
        }
        let mut m = r.clone();
        m["rhs"].as_array_mut().unwrap().remove(t);
        out.push((format!("drop@{t}"), m));
        let mut m = r.clone();
        m["rhs"].as_array_mut().unwrap().push(rhs[t].clone());
        out.push((format!("dup@{t}"), m));
        let u = (t + 1) % rhs.len();
        if rhs[u][1] != rhs[t][1] {
            let mut m = r.clone();
            m["rhs"][t][1] = rhs[u][1].clone();
            m["rhs"][u][1] = rhs[t][1].clone();
            out.push((format!("swap@{t}"), m));
        }
    }
    let mut m = r.clone();
    let slot = (0..n).find(|&i| r["fixed"][i].is_null()).unwrap_or(0);
    m["target"] = json!(shift_key(r["target"].as_str().unwrap(), slot, 1));
    m.as_object_mut().unwrap().remove("raw_pivot_encoded");
    out.push(("target+1".into(), m));
    out
}

#[test]
#[ignore = "release, minutes: audit B mutation baseline"]
fn rows_mutation_baseline() {
    let mut rules: Vec<(String, Family, Value, Vec<Vec<i64>>)> = Vec::new();
    if let Some(o) = oracle() {
        for case in o.jsonl("span/F6_span_cases.jsonl") {
            let id = case["id"].as_str().unwrap().to_string();
            let Some(line) = f6_line(&o, &case) else {
                continue;
            };
            let v: Value = serde_json::from_str(line.trim_end()).unwrap();
            let r = if v.get("rules").is_some() {
                v["rules"][0].clone()
            } else {
                v
            };
            if r.get("sym_rows").is_some_and(|x| !x.is_null()) {
                continue;
            }
            let keys: Vec<Vec<i64>> = ["C2", "C5"]
                .iter()
                .flat_map(|k| case["clean"][0][*k].as_array().cloned().unwrap_or_default())
                .filter(|p| p["verdict"] == "covered_regular")
                .map(|p| {
                    p["point"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_i64().unwrap())
                        .collect()
                })
                .take(1)
                .collect();
            rules.push((id, t162_family(), r, keys));
        }
    }
    for (name, ops) in [
        ("T001c.sym", vec![0, 3]),
        ("LA3_handwritten.sym", vec![0, 5]),
        ("bubu.sym", vec![0]),
    ] {
        for op in ops {
            let (fam, r) = built(&data(name), op);
            rules.push((format!("{name}#op{op}"), fam, r, Vec::new()));
        }
    }
    let (mut applied, mut fail, mut refused, mut undetected) = (0, 0, 0, Vec::new());
    let mut true_pass = 0;
    for (id, fam, r, keys) in &rules {
        let n = fam.n();
        let rule = RowsRule::from_value(r.clone(), n).unwrap();
        let mut pts: Vec<PointSpec> = Vec::new();
        if rule.fixed.iter().any(Option::is_none) {
            pts.extend([PointSpec::Generic, PointSpec::Generic]);
        }
        pts.extend(keys.iter().cloned().map(PointSpec::Key));
        if pts.is_empty() {
            pts.push(PointSpec::Key(
                rule.fixed.iter().map(|f| f.unwrap()).collect(),
            ));
        }
        for seed in 1..=5u64 {
            let v = identity_test(
                fam,
                &rule,
                &pts,
                &RowsOptions {
                    seed,
                    ..RowsOptions::default()
                },
            );
            assert_eq!(v.verdict, Verdict::Pass, "{id} seed {seed}: true rule");
            true_pass += 1;
        }
        let max_terms = if r.to_string().len() > 200_000 { 4 } else { 12 };
        for (cls, m) in mutants(r, max_terms) {
            let Ok(mr) = RowsRule::from_value(m, n) else {
                refused += 1;
                continue;
            };
            applied += 1;
            let v = identity_test(fam, &mr, &pts, &RowsOptions::default());
            match v.verdict {
                Verdict::Fail { .. } => fail += 1,
                Verdict::Pass => undetected.push(format!("{id} {cls}")),
                _ => refused += 1,
            }
        }
        println!("{id}: done ({applied} mutants so far)");
    }
    println!(
        "MUTATION BASELINE: {} rules, {true_pass} true-rule verdicts all Pass; {applied} mutants: {fail} Fail, {refused} refused (Pole/NotApplicable/Unsupported/unreadable), {} UNDETECTED {:?}",
        rules.len(),
        undetected.len(),
        undetected
    );
    // legitimately invisible: a key moved within a scaleless sector stays scaleless (both zero) -> reported, not failed
    assert!(undetected.is_empty(), "{undetected:?}");
}
