//! Re-audit A NEW-M4 (rows3): the MUTATION BASELINE through `replay_evidence` (the real certification path), with
//! the audit-B campaign classes: boundary-face terms, zero-sector terms, coefficient arithmetic (negate, double, +1,
//! swap), key / target perturbations, dropped and duplicated terms, and perturbations that VANISH at every served key
//! (detected only at the generic points). Soundness criterion (independent of the span machinery): a mutant is
//! UNDETECTED iff its record is `Established` while its rule vector differs from the original's at some served key
//! (coefficients evaluated at two primes and a random d, scaleless columns dropped). Classes reported: detected,
//! legit-invisible (Established, identical on the served set), safe side (not established, identical), undetected.
//! The default test is a seeded reduced campaign; `mutation_baseline_replay_full` (ignored, release) runs all rules.

mod rows_common;

use std::collections::BTreeMap;

use rows_common::face::{face, served};
use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    PointSpec, PointStatus, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, identity_test,
    replay_evidence,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Ctx};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::primes::{SplitMix64, verify_primes};
use rustred_campaign_core::replay_base::record::{Pat, key_at};
use serde_json::Value;

/// A coefficient text split into sign, numerator monomials (with signs) and denominator (text inside its parentheses,
/// or a single monomial), in the strict grammar of the streaming reader.
#[derive(Clone)]
struct Coef {
    neg: bool,
    num: Vec<String>,
    den: Option<String>,
}

fn terms(p: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, ch) in p.chars().filter(|c| !c.is_whitespace()).enumerate() {
        if (ch == '+' || ch == '-') && i > 0 && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|t| {
            if t.starts_with(['+', '-']) {
                t
            } else {
                format!("+{t}")
            }
        })
        .collect()
}

fn parse_coef(t: &str) -> Option<Coef> {
    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let (neg, rest) = match t.strip_prefix("-(") {
        Some(r) => (true, format!("({r}")),
        None => (false, t.clone()),
    };
    if let Some(r) = rest.strip_prefix('(') {
        let close = r.find(')')?;
        let num = terms(&r[..close]);
        let after = &r[close + 1..];
        let den = match after.strip_prefix('/') {
            None if after.is_empty() => None,
            None => return None,
            Some(d) => Some(d.trim_start_matches('(').trim_end_matches(')').to_string()),
        };
        return Some(Coef { neg, num, den });
    }
    match rest.split_once('/') {
        None => Some(Coef {
            neg: false,
            num: terms(&rest),
            den: None,
        }),
        Some((m, d)) => Some(Coef {
            neg: false,
            num: terms(m),
            den: Some(d.trim_start_matches('(').trim_end_matches(')').to_string()),
        }),
    }
}

fn show(c: &Coef) -> String {
    let num: String = c.num.concat();
    let num = num.strip_prefix('+').unwrap_or(&num).to_string();
    let s = match &c.den {
        Some(d) => format!("({num})/({d})"),
        None => format!("({num})"),
    };
    if c.neg { format!("-{s}") } else { s }
}

fn flip(t: &str) -> String {
    match t.strip_prefix('-') {
        Some(r) => format!("+{r}"),
        None => format!("-{}", t.trim_start_matches('+')),
    }
}

fn pat_text(p: &Pat) -> String {
    let parts: Vec<String> = p
        .iter()
        .enumerate()
        .map(|(i, &(ax, off))| match (ax, off) {
            (Some(_), 0) => format!("n{i}"),
            (Some(_), o) if o > 0 => format!("n{i}+{o}"),
            (Some(_), o) => format!("n{i}{o}"),
            (None, c) => c.to_string(),
        })
        .collect();
    format!("I({})", parts.join(","))
}

/// The rule point of a served key (target pattern inverted; fixed slots from the rule).
fn point_of(rule: &RowsRule, key: &[i64]) -> Vec<i64> {
    let mut pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
    for (j, &(ax, off)) in rule.target.iter().enumerate() {
        if let Some(a) = ax {
            pt[a] = key[j] - off;
        }
    }
    pt
}

/// The rule vector at a served key, at both primes and a fixed d: column -> value, scaleless columns dropped.
fn vector_at(
    fam: &Family,
    rule: &RowsRule,
    key: &[i64],
    primes: [u64; 2],
) -> Option<Vec<BTreeMap<Vec<i64>, u64>>> {
    let pt = point_of(rule, key);
    let a = Independent;
    let mut out = Vec::new();
    for p in primes {
        let mut vals: Vec<u64> = pt.iter().map(|&x| a.from_i128(p, x as i128)).collect();
        vals.push(1_234_567);
        let ctx = [Ctx { p, vals }];
        let mut v: BTreeMap<Vec<i64>, u64> = BTreeMap::new();
        let t = key_at(&rule.target, &pt);
        if !fam.zero_key(&t) {
            v.insert(t, 1);
        }
        for (k, c) in &rule.rhs {
            let (mut n, mut d) = ([0u64], [0u64]);
            a.eval_ratio(c, rule.arity, &ctx, &mut n, &mut d).ok()?;
            let val = a.mul(p, n[0], a.inv(p, d[0])?);
            let col = key_at(k, &pt);
            if fam.zero_key(&col) {
                continue;
            }
            let e = v.entry(col).or_insert(0);
            *e = a.sub(p, *e, val);
        }
        v.retain(|_, x| *x != 0);
        out.push(v);
    }
    Some(out)
}

fn mutants(
    fam: &Family,
    rule: &RowsRule,
    served: &[Vec<i64>],
    rng: &mut SplitMix64,
) -> Vec<(String, RowsRule)> {
    let mut out: Vec<(String, RowsRule)> = Vec::new();
    let n = rule.arity;
    let mut push = |name: String, r: RowsRule| out.push((name, r));
    // key / target / structure
    for k in 0..rule.rhs.len().min(4) {
        for s in [1i64, -1] {
            let mut r = rule.clone();
            let j = (k + 1) % n;
            r.rhs[k].0[j].1 += s;
            push(format!("rhs[{k}] slot {j} {s:+}"), r);
        }
        let mut r = rule.clone();
        r.rhs.remove(k);
        push(format!("drop rhs[{k}]"), r);
        let mut r = rule.clone();
        let t = r.rhs[k].clone();
        r.rhs.push(t);
        push(format!("duplicate rhs[{k}]"), r);
    }
    let mut r = rule.clone();
    if let Some(j) = r.target.iter().position(|x| x.0.is_some()) {
        r.target[j].1 += 1;
        push("target slot +1".into(), r);
    }
    // coefficient arithmetic
    for k in 0..rule.rhs.len().min(4) {
        let Some(c) = parse_coef(&rule.rhs[k].1) else {
            continue;
        };
        let mut neg = c.clone();
        neg.neg = !neg.neg;
        let mut dbl = c.clone();
        dbl.num = dbl
            .num
            .iter()
            .map(|t| format!("{}2*{}", &t[..1], &t[1..]))
            .collect();
        let mut plus1 = c.clone();
        match &c.den {
            Some(d) if parse_coef(&format!("({d})")).is_some() => {
                let dt = terms(d);
                plus1.num.extend(if c.neg {
                    dt.iter().map(|t| flip(t)).collect()
                } else {
                    dt
                });
            }
            Some(_) => {}
            None => plus1
                .num
                .push(if c.neg { "-1".into() } else { "+1".into() }),
        }
        for (name, m) in [("negate", neg), ("double", dbl), ("+1", plus1)] {
            let mut r = rule.clone();
            r.rhs[k].1 = show(&m);
            if r.rhs[k].1 != rule.rhs[k].1 {
                push(format!("rhs[{k}] coefficient {name}"), r);
            }
        }
    }
    if rule.rhs.len() > 1 {
        let mut r = rule.clone();
        let (a, b) = (r.rhs[0].1.clone(), r.rhs[1].1.clone());
        r.rhs[0].1 = b;
        r.rhs[1].1 = a;
        push("swap coefficients 0,1".into(), r);
    }
    // zero-sector term: every line slot 0 (no active slot): scaleless everywhere (legit-invisible)
    let zero: Pat = (0..n)
        .map(|i| {
            if rule.chart[i] {
                (None, 0)
            } else {
                rule.target[i]
            }
        })
        .collect();
    let mut r = rule.clone();
    r.rhs.push((zero, "7".into()));
    push("zero-sector term".into(), r);
    // boundary-face term: lines of a scaleless sector s kept, others 0, a non-line slot j at +2 with s+j live
    let lines: Vec<usize> = (0..n).filter(|&i| rule.chart[i]).collect();
    if lines.len() <= 12 {
        'search: for m in 1u32..(1 << lines.len()) {
            let mut mask = vec![false; n];
            for (b, &i) in lines.iter().enumerate() {
                mask[i] = m >> b & 1 == 1;
            }
            if !fam.zero_mask(&mask) {
                continue;
            }
            for j in (0..n).filter(|&j| !rule.chart[j] && rule.target[j].0.is_some()) {
                let mut m2 = mask.clone();
                m2[j] = true;
                if fam.zero_mask(&m2) {
                    continue;
                }
                let p: Pat = (0..n)
                    .map(|i| {
                        if rule.chart[i] {
                            if mask[i] { rule.target[i] } else { (None, 0) }
                        } else if i == j {
                            (rule.target[i].0, rule.target[i].1 + 2)
                        } else {
                            rule.target[i]
                        }
                    })
                    .collect();
                let mut r = rule.clone();
                r.rhs.push((p, "3".into()));
                push(format!("boundary-face term (slot {j})"), r);
                break 'search;
            }
        }
    }
    // a perturbation vanishing at every served key: (n_a - v) * den added to the numerator, a free slot `a` constant
    // on the served set (detected only at the generic points)
    let pts: Vec<Vec<i64>> = served.iter().map(|k| point_of(rule, k)).collect();
    let free = (0..n).find(|&a| {
        rule.fixed[a].is_none() && !pts.is_empty() && pts.iter().all(|p| p[a] == pts[0][a])
    });
    if let (Some(a), Some(c)) = (free, parse_coef(&rule.rhs[0].1)) {
        let v = pts[0][a];
        let dt = match &c.den {
            Some(d) => terms(d),
            None => vec!["+1".to_string()],
        };
        let mut m = c.clone();
        for t in &dt {
            let t = if c.neg { flip(t) } else { t.clone() };
            m.num.push(format!("{}n{a}*{}", &t[..1], &t[1..]));
            // - v * t
            let coef = -v;
            if coef != 0 {
                let sign = if (coef < 0) ^ t.starts_with('-') {
                    "-"
                } else {
                    "+"
                };
                m.num.push(format!("{sign}{}*{}", coef.abs(), &t[1..]));
            }
        }
        let mut r = rule.clone();
        r.rhs[0].1 = show(&m);
        push(format!("vanishing at served keys: (n{a}-{v})*den"), r);
    }
    let _ = rng.next_u64();
    out.retain(|(_, r)| {
        r.rhs
            .iter()
            .all(|(k, c)| !pat_text(k).is_empty() && !c.is_empty())
    });
    out
}

#[derive(Default, Debug)]
struct Tally {
    mutants: usize,
    detected: usize,
    legit_invisible: usize,
    safe_side: usize,
    undetected: Vec<String>,
}

fn campaign(
    name: &str,
    fam: &Family,
    rule: &RowsRule,
    served: &[Vec<i64>],
    tally: &mut Tally,
    seed: u64,
) {
    let opts = bound(fam);
    let pol = ReplayPolicy::default();
    let base = replay_evidence(fam, rule, served, &pol, &opts);
    assert_eq!(
        base.level,
        ReplayLevel::Established,
        "{name}: baseline {:?}",
        base.level
    );
    let primes = verify_primes(seed, &[]);
    let v0: Vec<_> = served
        .iter()
        .map(|k| vector_at(fam, rule, k, primes))
        .collect();
    let mut rng = SplitMix64::new(seed);
    for (label, m) in mutants(fam, rule, served, &mut rng) {
        tally.mutants += 1;
        let rec = replay_evidence(fam, &m, served, &pol, &opts);
        let differs = served
            .iter()
            .zip(&v0)
            .any(|(k, v)| vector_at(fam, &m, k, primes).as_ref() != v.as_ref());
        match (rec.level == ReplayLevel::Established, differs) {
            (true, true) => tally.undetected.push(format!("{name}: {label}")),
            (true, false) => tally.legit_invisible += 1,
            (false, true) => tally.detected += 1,
            (false, false) => tally.safe_side += 1,
        }
    }
}

/// Served keys of an F6 case: the census keys that are applicable (covered_regular) and pass.
fn f6_served(fam: &Family, rule: &RowsRule, case: &Value) -> Vec<Vec<i64>> {
    let mut out = Vec::new();
    for k in ["C2", "C5"] {
        for p in case["clean"][0][k].as_array().into_iter().flatten() {
            if p["verdict"] == "covered_regular" {
                out.push(
                    p["point"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_i64().unwrap())
                        .collect::<Vec<i64>>(),
                );
            }
        }
    }
    out.retain(|k| {
        let v = identity_test(
            fam,
            rule,
            &[PointSpec::Key(k.clone())],
            &RowsOptions::default(),
        );
        v.points[0].status == PointStatus::Pass
    });
    out
}

fn run(full: bool) -> Tally {
    let mut tally = Tally::default();
    // (1) the boundary-face rule of audit A (LA3, 9 slots) on keys incl. the face
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let all = served(&f);
    let keys: Vec<Vec<i64>> = if full {
        all
    } else {
        // seeded reduction: every face key plus a deterministic sample of the others
        let mut s = SplitMix64::new(11);
        all.into_iter()
            .filter(|k| k[f.j] == 0 || s.range(0, 10) == 0)
            .collect()
    };
    campaign("LA3 face rule", &f.fam, &good, &keys, &mut tally, 11);
    // (2) the frozen F6 clean rules (T162, arity 20) on their applicable census keys
    if let Some(o) = oracle() {
        let fam = t162_family();
        let ids: &[&str] = if full {
            &["f111", "f046", "f000", "fs023", "fs034", "fd006"]
        } else {
            &["f111"]
        };
        for case in o.jsonl("span/F6_span_cases.jsonl") {
            let id = case["id"].as_str().unwrap();
            if !ids.contains(&id) {
                continue;
            }
            let Some(line) = f6_line(&o, &case) else {
                continue;
            };
            let rule = rule0(&line);
            let keys = f6_served(&fam, &rule, &case);
            if keys.is_empty() {
                continue;
            }
            campaign(&format!("F6 {id}"), &fam, &rule, &keys, &mut tally, 12);
        }
    }
    println!(
        "mutation baseline through replay_evidence ({}): {tally:?}",
        if full { "full" } else { "seeded reduced" }
    );
    tally
}

#[test]
fn mutation_baseline_replay_reduced() {
    let t = run(false);
    assert!(t.mutants >= 30, "{t:?}");
    assert!(t.undetected.is_empty(), "UNDETECTED: {:?}", t.undetected);
    assert!(t.detected > 0);
}

#[test]
#[ignore]
fn mutation_baseline_replay_full() {
    let t = run(true);
    assert!(t.undetected.is_empty(), "UNDETECTED: {:?}", t.undetected);
}
