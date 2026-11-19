//! THE d = 4 REGULARITY COMPARISON of a replacement (emission contract section 13). A REPORTED NUMBER, tie-break only,
//! since the user ruling of 2026-11-19 (LANE_RULES rule 21; [`ROLE`]); the rule of the same morning ("a relation regular
//! at d = 4 wins whatever its size; a replacement may never trade d = 4 regularity for speed") is withdrawn. PURE: no
//! process, no file.
//!
//! At an integer key the relation `target -> sum_t c_t(n, d) * daughter_t` becomes, after evaluating the indices, a sum
//! over DISTINCT daughter keys of univariate rational functions in d (terms with the same daughter key are MERGED, as
//! the engine adds them). The REGULARITY of the relation at the key is the largest POLE ORDER at d = 4 among the merged
//! nonzero daughter coefficients (0 = regular). EXACT: every coefficient is evaluated in exact integers, expanded around
//! d = 4 (Taylor shift, e = d - 4) and the order of the merged sum is the order of its numerator over the common
//! denominator, so a 0/0 at d = 4 is resolved (never counted as regular, never as singular, unless it is one);
//! a merged coefficient that is identically zero at the key is no daughter.
//!
//! The comparison old vs new runs over the keys the OLD record serves as first match: a key is WORSE when the new
//! relation's pole order there exceeds the old one's (in particular regular -> singular); NOT WORSE on every key is the
//! acceptance condition of a live replacement (no policy switch turns it off).
use super::body::sha256_hex;
use crate::poly::{BigInt, Poly};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "rustred.emission.regularity-d4.v2";
/// verdicts written under the OLD rule (2026-11-19, withdrawn the same day): NOT WORSE was an acceptance condition then
pub const SCHEMA_V1: &str = "rustred.emission.regularity-d4.v1";
/// the role of the measure since the user ruling of 2026-11-19 (WS/ops/LANE_RULES.md rule 21)
pub const ROLE: &str = "REPORTED NUMBER, tie-break only (user ruling 2026-11-19, LANE_RULES rule 21): never a reason to refuse, drop or deprioritise";
/// The tool id stated in every regularity member (bump on any change of the measure).
pub const TOOL: &str = "rustred-campaign-core emission::regularity v1 (exact, merged daughters, Taylor shift at d = 4)";

/// One slot of a key pattern: `n<j> + o` or a constant.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Slot {
    Var(usize, i64),
    Const(i64),
}

fn parse_pattern(text: &str) -> Result<Vec<Slot>, String> {
    let t = text.trim();
    let inner = t
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or_else(|| format!("not a key pattern: {t}"))?;
    inner
        .split(',')
        .map(|s| {
            let s = s.trim();
            if let Some(r) = s.strip_prefix('n') {
                let n = r.bytes().take_while(u8::is_ascii_digit).count();
                let j: usize = r[..n].parse().map_err(|_| format!("bad slot {s}"))?;
                let rest = r[n..].trim();
                let o: i64 = if rest.is_empty() {
                    0
                } else {
                    rest.replace(' ', "")
                        .parse()
                        .map_err(|_| format!("bad offset in {s}"))?
                };
                Ok(Slot::Var(j, o))
            } else {
                s.parse::<i64>()
                    .map(Slot::Const)
                    .map_err(|_| format!("bad slot {s}"))
            }
        })
        .collect()
}

fn eval_pattern(p: &[Slot], key: &[i64]) -> Vec<i64> {
    p.iter()
        .map(|s| match s {
            Slot::Var(j, o) => key[*j] + o,
            Slot::Const(c) => *c,
        })
        .collect()
}

/// One rhs term: the daughter pattern and the d-coefficients (power, polynomial in the indices) of numerator and
/// denominator of its coefficient.
#[derive(Clone, Debug)]
struct Term {
    daughter: Vec<Slot>,
    num: Vec<(u32, Poly)>,
    den: Vec<(u32, Poly)>,
}

/// A relation parsed for the regularity measure.
#[derive(Clone, Debug)]
pub struct Relation {
    pub arity: usize,
    target: Vec<Slot>,
    terms: Vec<Term>,
}

/// Split a coefficient text at its top-level `/` (at most one).
fn split_fraction(text: &str) -> Result<(&str, Option<&str>), String> {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut at = None;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'/' if depth == 0 => {
                if at.is_some() {
                    return Err("two top-level '/' in a coefficient".into());
                }
                at = Some(i);
            }
            _ => {}
        }
    }
    Ok(match at {
        Some(i) => (&text[..i], Some(&text[i + 1..])),
        None => (text, None),
    })
}

impl Relation {
    /// Parse a rule line (its `target` and `rhs`; every other member is ignored).
    pub fn parse(line: &str) -> Result<Relation, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("record: {e}"))?;
        let target = parse_pattern(v["target"].as_str().ok_or("no target")?)?;
        let arity = target.len();
        let mut terms = vec![];
        for t in v["rhs"].as_array().ok_or("no rhs")? {
            let (k, c) = (
                t[0].as_str().ok_or("rhs key")?,
                t[1].as_str().ok_or("rhs coefficient")?,
            );
            let daughter = parse_pattern(k)?;
            if daughter.len() != arity {
                return Err(format!("rhs key {k}: arity {} != {arity}", daughter.len()));
            }
            let (n, d) = split_fraction(c)?;
            let num =
                Poly::parse_exact(n, arity, true).map_err(|e| format!("numerator {n:.60}: {e}"))?;
            let den = match d {
                Some(d) => Poly::parse_exact(d, arity, true)
                    .map_err(|e| format!("denominator {d:.60}: {e}"))?,
                None => Poly::constant(1),
            };
            terms.push(Term {
                daughter,
                num: num.d_coefficients(),
                den: den.d_coefficients(),
            });
        }
        Ok(Relation {
            arity,
            target,
            terms,
        })
    }
}

fn big_of(p: &Poly, key: &[i32]) -> Result<BigInt, String> {
    // checked i128 first, exact BigInt on overflow (Poly::eval)
    match p.eval(key).map_err(|e| format!("evaluation: {e}"))? {
        crate::poly::Value::Small(x) => Ok(BigInt::from_i128(x)),
        crate::poly::Value::Big(b) => Ok(b),
    }
}

/// The univariate polynomial in d at the key, as coefficients c_k (k = power of d).
fn univariate(dc: &[(u32, Poly)], key: &[i32]) -> Result<Vec<BigInt>, String> {
    let deg = dc.iter().map(|x| x.0).max().unwrap_or(0) as usize;
    let mut c = vec![BigInt::zero(); deg + 1];
    for (k, p) in dc {
        c[*k as usize] = c[*k as usize].add(&big_of(p, key)?);
    }
    Ok(c)
}

/// Taylor shift: the coefficients of p(4 + e) in powers of e (exact; repeated synthetic division by d - 4).
fn shift4(mut c: Vec<BigInt>) -> Vec<BigInt> {
    let four = BigInt::from_i128(4);
    let n = c.len();
    for j in 0..n {
        for i in (j..n.saturating_sub(1)).rev() {
            let t = c[i + 1].mul(&four);
            c[i] = c[i].add(&t);
        }
    }
    c
}

fn order(e: &[BigInt]) -> Option<usize> {
    e.iter().position(|x| !x.is_zero())
}

/// Truncated product of two series in e (degree < m).
fn mul_trunc(a: &[BigInt], b: &[BigInt], m: usize) -> Vec<BigInt> {
    let mut r = vec![BigInt::zero(); m];
    for (i, x) in a.iter().enumerate().take(m) {
        if x.is_zero() {
            continue;
        }
        for (j, y) in b.iter().enumerate().take(m - i) {
            if !y.is_zero() {
                r[i + j] = r[i + j].add(&x.mul(y));
            }
        }
    }
    r
}

/// The regularity of a relation at one key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRegularity {
    /// largest pole order at d = 4 among the merged nonzero daughter coefficients (0 = regular)
    pub max_pole: u32,
    /// merged nonzero daughters, and those with a pole at d = 4
    pub daughters: u32,
    pub singular_daughters: u32,
    /// a daughter of largest pole order (for the record of a regression)
    pub worst_daughter: Option<Vec<i64>>,
    /// a term whose denominator is identically zero at the key (the relation is not defined there)
    pub undefined: bool,
}

/// The pole order at d = 4 of every merged daughter of `rel` at `key` (daughters whose terms have no denominator
/// vanishing at d = 4 are regular and listed with 0 without evaluating their numerators; a merged coefficient that is
/// identically zero is left out), and whether some term is undefined at the key.
pub fn daughter_poles(
    rel: &Relation,
    key: &[i64],
) -> Result<(BTreeMap<Vec<i64>, u32>, bool), String> {
    let r = regularity_inner(rel, key, true)?;
    Ok((r.1, r.0.undefined))
}

/// The regularity of `rel` at `key` (the key must match the target pattern).
pub fn regularity_at(rel: &Relation, key: &[i64]) -> Result<KeyRegularity, String> {
    Ok(regularity_inner(rel, key, false)?.0)
}

fn regularity_inner(
    rel: &Relation,
    key: &[i64],
    list: bool,
) -> Result<(KeyRegularity, BTreeMap<Vec<i64>, u32>), String> {
    let mut poles: BTreeMap<Vec<i64>, u32> = BTreeMap::new();
    if key.len() != rel.arity {
        return Err(format!("key arity {} != {}", key.len(), rel.arity));
    }
    if eval_pattern(&rel.target, key) != key {
        return Err(format!("key {key:?} is not an instance of the target"));
    }
    let k32: Vec<i32> = key
        .iter()
        .map(|&x| i32::try_from(x).map_err(|_| "key value out of range".to_string()))
        .collect::<Result<_, _>>()?;
    // per daughter: (numerator series, its order) and (denominator series, its order) of every term
    let mut by_daughter: BTreeMap<Vec<i64>, Vec<(usize, Vec<BigInt>, usize)>> = BTreeMap::new();
    let mut r = KeyRegularity::default();
    for (ti, t) in rel.terms.iter().enumerate() {
        let den = shift4(univariate(&t.den, &k32)?);
        let Some(od) = order(&den) else {
            r.undefined = true;
            continue;
        };
        by_daughter
            .entry(eval_pattern(&t.daughter, key))
            .or_default()
            .push((ti, den, od));
    }
    // numerators only where a pole is possible (some denominator vanishes at d = 4 on that daughter)
    let mut by_daughter: BTreeMap<Vec<i64>, Vec<(Vec<BigInt>, Vec<BigInt>, usize)>> = {
        let mut m = BTreeMap::new();
        for (dk, ts) in by_daughter {
            let v: usize = ts.iter().map(|x| x.2).sum();
            let mut out = vec![];
            for (ti, den, od) in ts {
                let num = if v == 0 {
                    vec![BigInt::from_i128(1)] // not evaluated: the daughter is regular whatever it is
                } else {
                    shift4(univariate(&rel.terms[ti].num, &k32)?)
                };
                if order(&num).is_none() {
                    continue; // a zero coefficient at this key
                }
                out.push((num, den, od));
            }
            if !out.is_empty() {
                m.insert(dk, out);
            }
        }
        m
    };
    for (dk, ts) in std::mem::take(&mut by_daughter) {
        let v: usize = ts.iter().map(|x| x.2).sum();
        let pole = if v == 0 {
            0
        } else if ts.len() == 1 {
            let on = order(&ts[0].0).expect("nonzero");
            v.saturating_sub(on)
        } else {
            // numerator of the merged sum over the common denominator, modulo e^v
            let mut total = vec![BigInt::zero(); v];
            for (i, (a, _, _)) in ts.iter().enumerate() {
                let mut p: Vec<BigInt> = a.iter().take(v).cloned().collect();
                p.resize(v, BigInt::zero());
                for (j, (_, b, _)) in ts.iter().enumerate() {
                    if i != j {
                        p = mul_trunc(&p, b, v);
                    }
                }
                for (s, x) in total.iter_mut().zip(p) {
                    *s = s.add(&x);
                }
            }
            match order(&total) {
                Some(on) => v - on,
                None => 0, // the merged numerator vanishes to order >= v: regular (or zero)
            }
        };
        // a merged coefficient that is identically zero is no daughter: test exactly when the terms could cancel
        if ts.len() > 1 && merged_is_zero(&ts) {
            continue;
        }
        r.daughters += 1;
        if list {
            poles.insert(dk.clone(), pole as u32);
        }
        if pole > 0 {
            r.singular_daughters += 1;
        }
        if pole as u32 > r.max_pole || (pole > 0 && r.worst_daughter.is_none()) {
            r.max_pole = r.max_pole.max(pole as u32);
            r.worst_daughter = Some(dk.clone());
        }
    }
    let _ = &mut by_daughter;
    Ok((r, poles))
}

/// Whether the merged numerator sum_i a_i prod_{j != i} b_j is the zero polynomial (full, not truncated).
fn merged_is_zero(ts: &[(Vec<BigInt>, Vec<BigInt>, usize)]) -> bool {
    let full = |a: &[BigInt], b: &[BigInt]| -> Vec<BigInt> {
        let mut r = vec![BigInt::zero(); a.len() + b.len()];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                r[i + j] = r[i + j].add(&x.mul(y));
            }
        }
        r
    };
    let mut total: Vec<BigInt> = vec![];
    for (i, (a, _, _)) in ts.iter().enumerate() {
        let mut p = a.clone();
        for (j, (_, b, _)) in ts.iter().enumerate() {
            if i != j {
                p = full(&p, b);
            }
        }
        if total.len() < p.len() {
            total.resize(p.len(), BigInt::zero());
        }
        for (s, x) in total.iter_mut().zip(p) {
            *s = s.add(&x);
        }
    }
    total.iter().all(BigInt::is_zero)
}

/// The comparison at one key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyComparison {
    pub key: Vec<i64>,
    /// the case branch (new piece) that serves the key
    pub branch: String,
    pub old: KeyRegularity,
    pub new: KeyRegularity,
}

impl KeyComparison {
    pub fn worse(&self) -> bool {
        self.new.undefined && !self.old.undefined || self.new.max_pole > self.old.max_pole
    }
    pub fn better(&self) -> bool {
        self.new.max_pole < self.old.max_pole
    }
}

/// Counts of a comparison (overall or per branch).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub keys: u64,
    pub regular_to_singular: u64,
    pub singular_to_regular: u64,
    pub unchanged: u64,
    /// pole order raised (including regular -> singular)
    pub worse: u64,
    /// pole order lowered (including singular -> regular)
    pub better: u64,
    /// the new relation undefined at a served key
    pub new_undefined: u64,
}

impl Counts {
    pub fn add(&mut self, c: &KeyComparison) {
        self.keys += 1;
        let (o, n) = (c.old.max_pole, c.new.max_pole);
        if o == 0 && n > 0 {
            self.regular_to_singular += 1;
        }
        if o > 0 && n == 0 {
            self.singular_to_regular += 1;
        }
        if o == n && !(c.new.undefined && !c.old.undefined) {
            self.unchanged += 1;
        }
        if c.worse() {
            self.worse += 1;
        }
        if c.better() {
            self.better += 1;
        }
        if c.new.undefined {
            self.new_undefined += 1;
        }
    }
}

/// How the compared keys were chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySet {
    /// "first-match set" (every key of the cone whose first match is the old record) or "sample"
    pub kind: String,
    pub cone: String,
    /// first-match keys of the old record in the cone (all of them when kind = first-match set)
    pub served: u64,
    pub compared: u64,
    /// sampling rule when kind = sample (every branch and shell represented)
    #[serde(default)]
    pub sampling: Option<String>,
    pub keys_sha256: String,
}

/// The REGULARITY MEMBER of a replacement (side file `regularity/<sha>.json`; the record names its sha256).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub schema: String,
    pub tool: String,
    pub old_body_sha256: String,
    /// the replacement records compared (body sha256 in piece order)
    pub new_body_sha256: Vec<String>,
    pub keys: KeySet,
    pub counts: Counts,
    pub by_branch: BTreeMap<String, Counts>,
    /// the first regressions (key, branch, old / new pole order, worst new daughter)
    pub first_regressions: Vec<Value>,
    /// a structural finding (a denominator factor of the new relation vanishing at d = 4 where the old has none)
    #[serde(default)]
    pub structural: Vec<String>,
    /// NOT WORSE on every compared key (a REPORTED number since rule 21; under schema v1 it was an acceptance condition)
    pub not_worse: bool,
    /// the measure's role (absent in v1 verdicts)
    #[serde(default)]
    pub role: String,
    /// information (not a verdict): the records' stated `witness_d_poles` factors that vanish at d = 4 for every index
    /// value (a pure d factor, e.g. "-4+d"): the producer's own statement of a d = 4 pole on the whole domain
    #[serde(default)]
    pub d4_factors_info: Vec<String>,
}

/// The stated `witness_d_poles` factors of a record that vanish at d = 4 whatever the indices (pure d factors).
pub fn d4_factors(line: &str) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(line).ok()?;
    let a = v.get("witness_d_poles")?.as_array()?;
    Some(
        a.iter()
            .filter_map(Value::as_str)
            .filter(|f| {
                Poly::parse_exact(f, 64, true).is_ok_and(|p| {
                    p.support().is_empty()
                        && shift4(p.d_coefficients().iter().fold(vec![], |mut c, (k, q)| {
                            let k = *k as usize;
                            if c.len() <= k {
                                c.resize(k + 1, BigInt::zero());
                            }
                            c[k] = q.as_constant().map_or(BigInt::zero(), BigInt::from_i128);
                            c
                        }))
                        .first()
                        .is_some_and(BigInt::is_zero)
                })
            })
            .map(str::to_string)
            .collect(),
    )
}

/// Compare per key and summarise. `cmp` = the per-key comparisons in key order.
pub fn verdict(
    old_body: &str,
    new_bodies: &[String],
    keys: KeySet,
    cmp: &[KeyComparison],
    structural: Vec<String>,
) -> Verdict {
    let mut counts = Counts::default();
    let mut by_branch: BTreeMap<String, Counts> = BTreeMap::new();
    let mut first = vec![];
    for c in cmp {
        counts.add(c);
        by_branch.entry(c.branch.clone()).or_default().add(c);
        if c.worse() && first.len() < 5 {
            first.push(json!({"key": c.key, "branch": c.branch, "old_pole": c.old.max_pole, "new_pole": c.new.max_pole,
                              "new_worst_daughter": c.new.worst_daughter, "new_undefined": c.new.undefined}));
        }
    }
    Verdict {
        schema: SCHEMA.into(),
        tool: TOOL.into(),
        old_body_sha256: old_body.into(),
        new_body_sha256: new_bodies.to_vec(),
        keys,
        not_worse: counts.worse == 0 && structural.is_empty() && cmp.len() as u64 == counts.keys,
        counts,
        by_branch,
        first_regressions: first,
        structural,
        d4_factors_info: vec![],
        role: ROLE.into(),
    }
}

/// sha256 of a key list (one compact JSON array per line, in order).
pub fn keys_sha256(keys: &[Vec<i64>]) -> String {
    let mut s = String::new();
    for k in keys {
        s.push_str(&serde_json::to_string(k).unwrap_or_default());
        s.push('\n');
    }
    sha256_hex(s.as_bytes())
}

/// D + r of a key on a chart mask ('1' = line).
pub fn dr(chart: &str, key: &[i64]) -> i64 {
    chart
        .bytes()
        .zip(key)
        .map(|(m, &x)| if m == b'1' { x - 1 } else { -x })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(target: &str, rhs: &[(&str, &str)]) -> String {
        json!({"target": target, "rhs": rhs.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>()})
            .to_string()
    }

    #[test]
    fn pole_orders_are_exact_and_merge_daughters() {
        // regular: 1/(d-3) at d = 4
        let r = Relation::parse(&rec("I(n0,n1)", &[("I(n0-1,n1)", "(1)/(d-3)")])).unwrap();
        assert_eq!(regularity_at(&r, &[2, 1]).unwrap().max_pole, 0);
        // simple pole 1/(d-4); a double pole 1/(d-4)^2
        let r = Relation::parse(&rec(
            "I(n0,n1)",
            &[("I(n0-1,n1)", "(1)/(d-4)"), ("I(n0,n1-1)", "(1)/((d-4)^2)")],
        ))
        .unwrap();
        let k = regularity_at(&r, &[2, 1]).unwrap();
        assert_eq!((k.max_pole, k.singular_daughters, k.daughters), (2, 2, 2));
        // 0/0 resolved: (d-4)/(d-4) is regular; (d-4)/(d-4)^2 a simple pole
        let r = Relation::parse(&rec(
            "I(n0)",
            &[("I(n0-1)", "(d-4)/(d-4)"), ("I(n0-2)", "(d-4)/((d-4)^2)")],
        ))
        .unwrap();
        let k = regularity_at(&r, &[3]).unwrap();
        assert_eq!((k.max_pole, k.singular_daughters), (1, 1));
        // the pole depends on the key: (n0-4+d-d)... 1/(d - n0) is singular only at n0 = 4
        let r = Relation::parse(&rec("I(n0)", &[("I(n0-1)", "(1)/(d-n0)")])).unwrap();
        assert_eq!(regularity_at(&r, &[4]).unwrap().max_pole, 1);
        assert_eq!(regularity_at(&r, &[5]).unwrap().max_pole, 0);
        // MERGED daughters cancel: 1/(d-4) - 1/(d-4) on the same daughter = 0 (no daughter); 1/(d-4) + (3-d)/(d-4) = -1
        let r = Relation::parse(&rec(
            "I(n0)",
            &[("I(n0-1)", "(1)/(d-4)"), ("I(n0-1)", "(-1)/(d-4)")],
        ))
        .unwrap();
        let k = regularity_at(&r, &[2]).unwrap();
        assert_eq!((k.max_pole, k.daughters), (0, 0));
        let r = Relation::parse(&rec(
            "I(n0)",
            &[("I(n0-1)", "(1)/(d-4)"), ("I(n0-1)", "(3-d)/(d-4)")],
        ))
        .unwrap();
        let k = regularity_at(&r, &[2]).unwrap();
        assert_eq!((k.max_pole, k.daughters), (0, 1));
        // two daughters that coincide only at some keys: I(n0-1) and I(1) merge at n0 = 2
        let r = Relation::parse(&rec(
            "I(n0)",
            &[("I(n0-1)", "(1)/(d-4)"), ("I(1)", "(-1)/(d-4)")],
        ))
        .unwrap();
        assert_eq!(regularity_at(&r, &[2]).unwrap().max_pole, 0);
        assert_eq!(regularity_at(&r, &[3]).unwrap().max_pole, 1);
        // undefined: denominator identically zero at the key
        let r = Relation::parse(&rec("I(n0)", &[("I(n0-1)", "(1)/(n0-2)")])).unwrap();
        assert!(regularity_at(&r, &[2]).unwrap().undefined);
    }

    #[test]
    fn worse_means_a_raised_pole_order_at_a_served_key() {
        let old = Relation::parse(&rec("I(n0)", &[("I(n0-1)", "(1)/(d-3)")])).unwrap();
        let new = Relation::parse(&rec("I(n0)", &[("I(n0-1)", "(1)/(d-n0)")])).unwrap();
        let keys: Vec<Vec<i64>> = (2..7).map(|x| vec![x]).collect();
        let cmp: Vec<KeyComparison> = keys
            .iter()
            .map(|k| KeyComparison {
                key: k.clone(),
                branch: "p0".into(),
                old: regularity_at(&old, k).unwrap(),
                new: regularity_at(&new, k).unwrap(),
            })
            .collect();
        let ks = KeySet {
            kind: "first-match set".into(),
            cone: "test".into(),
            served: 5,
            compared: 5,
            sampling: None,
            keys_sha256: keys_sha256(&keys),
        };
        let v = verdict("old", &["new".into()], ks, &cmp, vec![]);
        assert!(!v.not_worse);
        assert_eq!(
            (
                v.counts.regular_to_singular,
                v.counts.worse,
                v.counts.unchanged
            ),
            (1, 1, 4)
        );
        assert_eq!(v.first_regressions[0]["key"], json!([4]));
    }
}
