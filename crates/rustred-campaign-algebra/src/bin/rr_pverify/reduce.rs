//! The verifier's own EXACT reduction to the terminals: the chain of first-match rule applications (my dispatcher),
//! each coefficient evaluated exactly (Symbolica Rational) at given values of d, accumulated over the reduction DAG in
//! reverse post-order. Semantics of the engine's `Table::reduce_mod` (rustred-core node/mastermap.rs at f9132a56),
//! re-implemented: residual records = masters (not reduced); a term with a positive numerator index must have a zero
//! coefficient and is dropped; a term in a lower sector is a terminal unless its support is a zero sector (dropped);
//! a cycle is an error. Used to compare `rr_node reduce` results on small keys and to locate sample-point poles.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};

use serde_json::{Value, json};
use symbolica::domains::rational::Rational;

use crate::dispatch;
use crate::expr::Expr;
use crate::table::{Table, at, for_each_line};
use crate::{Args, die};

fn parse_q(t: &str) -> Rational {
    let t = t.trim();
    match t.split_once('/') {
        Some((a, b)) => Rational::from((
            a.trim().parse::<i64>().unwrap(),
            b.trim().parse::<i64>().unwrap(),
        )),
        None => Rational::from(t.parse::<i64>().unwrap()),
    }
}

struct Coefs {
    file: std::fs::File,
    offsets: Vec<(u64, usize)>,
    texts: HashMap<usize, Vec<String>>,
    exprs: HashMap<(usize, usize), Expr>,
    n: usize,
}

impl Coefs {
    fn expr(&mut self, rule: usize, term: usize) -> Result<&Expr, String> {
        if !self.texts.contains_key(&rule) {
            let (off, len) = self.offsets[rule];
            let mut buf = vec![0u8; len];
            self.file
                .seek(SeekFrom::Start(off))
                .map_err(|e| e.to_string())?;
            self.file.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
            let t: Vec<String> = v["rhs"]
                .as_array()
                .ok_or("rule without rhs")?
                .iter()
                .map(|x| x[1].as_str().unwrap_or("").to_string())
                .collect();
            self.texts.insert(rule, t);
        }
        if !self.exprs.contains_key(&(rule, term)) {
            let e = Expr::parse(&self.texts[&rule][term], self.n)?;
            self.exprs.insert((rule, term), e);
        }
        Ok(&self.exprs[&(rule, term)])
    }
}

/// `myreduce --table T --index INDEX.jsonl --keys FILE [--dims 37/7,-11/5,9/13] [--max-nodes N] --out FILE`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let ip = a.need("--index");
    let kp = a.need("--keys");
    let out = a.need("--out");
    let dims_s: Vec<String> = a
        .take("--dims")
        .unwrap_or_else(|| "37/7,-11/5,9/13".into())
        .split(',')
        .map(|s| s.to_string())
        .collect();
    let max_nodes: usize = a
        .take("--max-nodes")
        .map(|x| x.parse().unwrap())
        .unwrap_or(200_000);
    a.done();
    let dims: Vec<Rational> = dims_s.iter().map(|s| parse_q(s)).collect();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    let n = t.n;
    let mut offsets = vec![];
    for_each_line(&ip, |_, _, v| {
        if v["kind"].as_str() == Some("rule") {
            offsets.push((
                v["offset"].as_u64().unwrap(),
                v["len"].as_u64().unwrap() as usize,
            ));
        }
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    if offsets.len() != t.rules.len() {
        die("index and table disagree on the rule count");
    }
    let mut co = Coefs {
        file: std::fs::File::open(&tp).unwrap(),
        offsets,
        texts: HashMap::new(),
        exprs: HashMap::new(),
        n,
    };
    let masters: std::collections::HashSet<Vec<i64>> = t.masters.iter().cloned().collect();
    let seeds = crate::keys::read_keys(&kp).unwrap_or_else(|e| die(&e));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    for seed in seeds {
        let res = reduce_one(&t, &mut co, &masters, &seed, &dims, &dims_s, max_nodes);
        use std::io::Write;
        writeln!(f, "{}", res).unwrap();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Inner,
    Master,
    Lower,
}

fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn reduce_one(
    t: &Table,
    co: &mut Coefs,
    masters: &std::collections::HashSet<Vec<i64>>,
    seed: &[i64],
    dims: &[Rational],
    dims_s: &[String],
    max_nodes: usize,
) -> Value {
    let n = t.n;
    let mut id: HashMap<Vec<i64>, usize> = HashMap::new();
    let mut keys: Vec<Vec<i64>> = vec![];
    let mut kind: Vec<Kind> = vec![];
    // inner: (rule, pt, children (node, term))
    let mut rule_of: Vec<Option<(usize, Vec<i64>)>> = vec![];
    let mut kids: Vec<Vec<(usize, usize)>> = vec![];
    let mut state: Vec<u8> = vec![];
    let mut post: Vec<usize> = vec![];
    let mut findings: Vec<Value> = vec![];
    let classify = |k: &[i64]| -> Option<Kind> {
        let in_sector = (0..n).all(|i| !t.mask[i] || k[i] >= 1);
        if in_sector {
            Some(if masters.contains(k) {
                Kind::Master
            } else {
                Kind::Inner
            })
        } else {
            let sup: Vec<bool> = k.iter().map(|&x| x > 0).collect();
            if t.zero.contains(&sup) {
                None
            } else {
                Some(Kind::Lower)
            }
        }
    };
    let Some(sk) = classify(seed) else {
        return json!({"key": seed, "status": "seed in a zero sector"});
    };
    id.insert(seed.to_vec(), 0);
    keys.push(seed.to_vec());
    kind.push(sk);
    rule_of.push(None);
    kids.push(vec![]);
    state.push(0);
    let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
    while let Some((u, ci)) = stack.pop() {
        if ci == 0 {
            if state[u] == 2 {
                continue;
            }
            if state[u] == 1 {
                return json!({"key": seed, "status": "cycle", "at": key_text(&keys[u])});
            }
            state[u] = 1;
            if kind[u] == Kind::Inner {
                let k = keys[u].clone();
                let Some(ri) = dispatch::first_match(t, &k) else {
                    return json!({"key": seed, "status": "uncovered key", "at": key_text(&k)});
                };
                let pt = dispatch::rule_point(&t.rules[ri], &k).unwrap();
                let mut ch = vec![];
                for (ti, p) in t.rules[ri].rhs_pats.iter().enumerate() {
                    let tk = at(p, &pt);
                    if (0..n).any(|i| !t.mask[i] && tk[i] > 0) {
                        // positive numerator index: the coefficient must vanish (checked exactly at every d)
                        let e = match co.expr(ri, ti) {
                            Ok(e) => e,
                            Err(err) => {
                                return json!({"key": seed, "status": "coefficient parse", "why": err});
                            }
                        };
                        for (d, ds) in dims.iter().zip(dims_s) {
                            match e.eval(&pt, d) {
                                Ok(v) if v.is_zero() => {}
                                Ok(_) => findings.push(json!({"at": key_text(&k), "rule": ri, "term": ti, "d": ds,
                                    "why": "ISP term with a NONZERO coefficient"})),
                                Err(_) => findings.push(json!({"at": key_text(&k), "rule": ri, "term": ti, "d": ds,
                                    "why": "ISP term: coefficient has a pole at this sample d (engine reads it as nonzero)"})),
                            }
                        }
                        continue;
                    }
                    let Some(kd) = classify(&tk) else { continue };
                    let c = match id.get(&tk) {
                        Some(&c) => c,
                        None => {
                            let c = keys.len();
                            id.insert(tk.clone(), c);
                            keys.push(tk);
                            kind.push(kd);
                            rule_of.push(None);
                            kids.push(vec![]);
                            state.push(0);
                            c
                        }
                    };
                    ch.push((c, ti));
                }
                rule_of[u] = Some((ri, pt));
                kids[u] = ch;
                if keys.len() > max_nodes {
                    return json!({"key": seed, "status": "too large", "nodes": keys.len()});
                }
            }
        }
        let mut next = ci;
        let mut pushed = false;
        while next < kids[u].len() {
            let c = kids[u][next].0;
            next += 1;
            if state[c] == 0 {
                stack.push((u, next));
                stack.push((c, 0));
                pushed = true;
                break;
            } else if state[c] == 1 {
                return json!({"key": seed, "status": "cycle", "below": key_text(&keys[u])});
            }
        }
        if !pushed {
            state[u] = 2;
            post.push(u);
        }
    }
    // values per d, reverse post-order
    let mut per_d = vec![];
    for (d, ds) in dims.iter().zip(dims_s) {
        let mut acc: Vec<Option<Rational>> = vec![None; keys.len()];
        acc[0] = Some(Rational::one());
        let mut pole: Option<Value> = None;
        for &u in post.iter().rev() {
            let Some(a) = acc[u].clone() else { continue };
            if kind[u] != Kind::Inner {
                continue;
            }
            let (ri, pt) = rule_of[u].clone().unwrap();
            for &(c, ti) in &kids[u].clone() {
                let e = co.expr(ri, ti).unwrap();
                match e.eval(&pt, d) {
                    Ok(v) => {
                        let add = a.clone() * v;
                        acc[c] = Some(match acc[c].take() {
                            Some(x) => x + add,
                            None => add,
                        });
                    }
                    Err(_) => {
                        if pole.is_none() {
                            pole = Some(json!({"at": key_text(&keys[u]), "rule": ri, "term": ti,
                                "why": "coefficient pole at this sample d"}));
                        }
                    }
                }
            }
        }
        let mut ms: BTreeMap<String, String> = BTreeMap::new();
        let mut lo: BTreeMap<String, String> = BTreeMap::new();
        for (i, k) in keys.iter().enumerate() {
            match kind[i] {
                Kind::Master => {
                    ms.insert(
                        key_text(k),
                        acc[i].clone().unwrap_or_else(Rational::zero).to_string(),
                    );
                }
                Kind::Lower => {
                    if let Some(v) = &acc[i]
                        && !v.is_zero()
                    {
                        lo.insert(key_text(k), v.to_string());
                    }
                }
                Kind::Inner => {}
            }
        }
        per_d.push(json!({"d": ds, "pole": pole, "masters": ms, "lower": lo}));
    }
    let inner = kind.iter().filter(|k| **k == Kind::Inner).count();
    json!({"key": seed, "status": "ok", "nodes": keys.len(), "inner": inner, "findings": findings, "dims": per_d})
}

/// `onelevel --table T --keys FILE --out FILE`: per key, the first-match rule and its in-sector children (the keys a
/// one-level `rr_node reduce --masters` needs so that the engine evaluates exactly that rule's coefficients).
pub fn cmd_onelevel(mut a: Args) {
    let tp = a.need("--table");
    let kp = a.need("--keys");
    let out = a.need("--out");
    a.done();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    let n = t.n;
    let seeds = crate::keys::read_keys(&kp).unwrap_or_else(|e| die(&e));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    for k in seeds {
        let Some(ri) = dispatch::first_match(&t, &k) else {
            use std::io::Write;
            writeln!(f, "{}", json!({"key": k, "rule": null})).unwrap();
            continue;
        };
        let pt = dispatch::rule_point(&t.rules[ri], &k).unwrap();
        let mut kids: Vec<String> = vec![];
        let mut isp = 0usize;
        let mut lower = 0usize;
        for p in &t.rules[ri].rhs_pats {
            let tk = at(p, &pt);
            if (0..n).any(|i| !t.mask[i] && tk[i] > 0) {
                isp += 1;
                continue;
            }
            if (0..n).all(|i| !t.mask[i] || tk[i] >= 1) {
                kids.push(key_text(&tk));
            } else {
                lower += 1;
            }
        }
        kids.sort();
        kids.dedup();
        use std::io::Write;
        writeln!(
            f,
            "{}",
            json!({"key": k, "rule": ri, "children": kids, "isp_terms": isp, "lower_terms": lower})
        )
        .unwrap();
    }
}

/// `termcheck --table T --index INDEX.jsonl --keys FILE [--dims 37/7,-11/5,9/13] --out FILE`: per key, every term of the
/// first-match rule whose written divisor vanishes at a sample d, with exact facts at the rule point: numerator
/// identically zero in d (the coefficient is exactly 0), divisor identically zero in d (a generic pole = a TABLE
/// defect), the term kind (positive numerator index / in sector / lower sector).
pub fn cmd_termcheck(mut a: Args) {
    let tp = a.need("--table");
    let ip = a.need("--index");
    let kp = a.need("--keys");
    let out = a.need("--out");
    let dims_s: Vec<String> = a
        .take("--dims")
        .unwrap_or_else(|| "37/7,-11/5,9/13".into())
        .split(',')
        .map(|s| s.to_string())
        .collect();
    a.done();
    let dims: Vec<Rational> = dims_s.iter().map(|s| parse_q(s)).collect();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    let n = t.n;
    let vars = crate::expr::poly_vars(n);
    let mut offsets = vec![];
    for_each_line(&ip, |_, _, v| {
        if v["kind"].as_str() == Some("rule") {
            offsets.push((
                v["offset"].as_u64().unwrap(),
                v["len"].as_u64().unwrap() as usize,
            ));
        }
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    let mut co = Coefs {
        file: std::fs::File::open(&tp).unwrap(),
        offsets,
        texts: HashMap::new(),
        exprs: HashMap::new(),
        n,
    };
    let seeds = crate::keys::read_keys(&kp).unwrap_or_else(|e| die(&e));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    for k in seeds {
        let Some(ri) = dispatch::first_match(&t, &k) else {
            continue;
        };
        let pt = dispatch::rule_point(&t.rules[ri], &k).unwrap();
        let pins: Vec<Option<i64>> = pt.iter().map(|&x| Some(x)).collect();
        let mut terms = vec![];
        for ti in 0..t.rules[ri].rhs_pats.len() {
            let e = co.expr(ri, ti).unwrap().clone();
            let Some((num, den)) = crate::expr::split_ratio(&e.root) else {
                continue;
            };
            let (Ok(nz), Ok(dz)) = (
                crate::expr::to_zpoly(&num, n, &vars),
                crate::expr::to_zpoly(&den, n, &vars),
            ) else {
                continue;
            };
            let dpin = crate::coef::pin_poly(&dz, &pins);
            let vanish: Vec<&String> = dims
                .iter()
                .zip(&dims_s)
                .filter(|(d, _)| {
                    let q = crate::expr::zpoly_to_q(&dpin).replace(n, d);
                    q.is_zero()
                })
                .map(|(_, s)| s)
                .collect();
            if vanish.is_empty() {
                continue;
            }
            let tk = at(&t.rules[ri].rhs_pats[ti], &pt);
            let kind = if (0..n).any(|i| !t.mask[i] && tk[i] > 0) {
                "positive numerator index (ISP)"
            } else if (0..n).all(|i| !t.mask[i] || tk[i] >= 1) {
                "in sector"
            } else {
                "lower sector"
            };
            terms.push(json!({"term": ti, "kind": kind, "target_key": key_text(&tk), "divisor_vanishes_at": vanish,
                "numerator_identically_zero_in_d": crate::coef::pin_poly(&nz, &pins).is_zero(),
                "divisor_identically_zero_in_d": dpin.is_zero()}));
        }
        use std::io::Write;
        writeln!(f, "{}", json!({"key": k, "rule": ri, "terms": terms})).unwrap();
    }
}
