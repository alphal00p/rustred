//! Table reduction and old <-> new master maps of a node (ported from the p5l-close-5 `mm` tool, 2026-11-12).
//!
//! A node table (`F.table.jsonl`, rustred node format) is a list of rules `target = sum coef(n, d) * rhs(n)`, each
//! valid on its case (fixed slots, equations) minus its exception conjunctions. [`Table::reduce_mod`] applies the
//! rules recursively to seed integrals (table order: the first rule whose case holds and no exception conjunction
//! vanishes) until the table's masters (residual records) and keys outside the node's sector (lower sectors;
//! scaleless `zero_sectors` dropped). Every rule is an IBP identity on its domain, so any applicable choice gives a
//! correct identity; table order guarantees descent. Coefficients are evaluated modulo several 61-bit primes at
//! several values of `d` (vectorised), then
//!
//! * exactly at each `d` by CRT + rational reconstruction (checked by leaving one prime out), or
//! * symbolically in `d` (integer sample points, Thiele interpolation per prime, coefficient-wise CRT + rational
//!   reconstruction, verified on every sample) as `N(d)/D(d)`.
//!
//! [`build_map`] relates two tables of the same family whose master sets differ: every new-only master is reduced
//! with the OLD table and every old-only master with the NEW table (top-sector parts modulo lower sectors), the two
//! matrices must be mutually inverse exactly at three values of `d` and symbolically; an independent Laporta
//! elimination of RustRed's own IBP rows ([`laporta`], [`super::replay::family::Family`]) in the top sector modulo
//! subsectors must reproduce each new master's row; and a few integrals reduced with the old table must equal their
//! reduction with the new table mapped back through the map ([`few_check`]). Valuations at `d = 4` flag a basis change
//! that carries powers of `eps`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader};

use serde_json::{Value, json};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::replay::arith::{self, Q};
use super::replay::expr::{Expr, Node};
use super::replay::family::Family;

pub type Key = Vec<i64>;
/// Symbolic fits: per seed, terminal key -> N(d)/D(d); plus the keys whose fit failed (key, reason).
pub type FitPart = (Vec<BTreeMap<Key, RatFun>>, Vec<(Key, String)>);

/// Ten 61-bit primes (the first two are the replay's `P` and `P2`).
pub const PRIMES: [u64; 10] = [
    2_305_843_009_213_693_951,
    2_305_843_009_213_693_921,
    2_305_843_009_213_693_907,
    2_305_843_009_213_693_723,
    2_305_843_009_213_693_693,
    2_305_843_009_213_693_669,
    2_305_843_009_213_693_613,
    2_305_843_009_213_693_561,
    2_305_843_009_213_693_549,
    2_305_843_009_213_693_487,
];

/// The three exact check values of `d` of the campaign maps.
pub fn check_dims() -> Vec<Q> {
    vec![Q { n: 37, d: 7 }, Q { n: -11, d: 5 }, Q { n: 9, d: 13 }]
}

/// `I(1,2,0)` or `1,2,0`.
pub fn parse_key(s: &str) -> Result<Key, String> {
    let t = s.trim();
    let t = t
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(t);
    t.split(',')
        .map(|x| x.trim().parse::<i64>().map_err(|_| format!("bad key {s}")))
        .collect()
}

/// `K1;K2;...` (each as [`parse_key`]).
pub fn parse_keys(s: &str) -> Result<Vec<Key>, String> {
    s.split(';')
        .filter(|x| !x.trim().is_empty())
        .map(parse_key)
        .collect()
}

pub fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// Pattern argument of a rule integral: `Some(axis) + offset`, or a constant.
type Pat = Vec<(Option<usize>, i64)>;

fn parse_pat(text: &str, n: usize) -> Result<Pat, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| format!("bad integral {text}"))?;
    let v: Pat = inner
        .split(',')
        .map(|a| {
            let a = a.trim();
            if let Some(rest) = a.strip_prefix('n') {
                let end = rest.find(['+', '-']).unwrap_or(rest.len());
                let ax: usize = rest[..end].parse().map_err(|_| format!("bad arg {a}"))?;
                let off: i64 = if end == rest.len() {
                    0
                } else {
                    rest[end..]
                        .trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("bad arg {a}"))?
                };
                Ok((Some(ax), off))
            } else {
                Ok((None, a.parse().map_err(|_| format!("bad arg {a}"))?))
            }
        })
        .collect::<Result<_, String>>()?;
    if v.len() != n {
        return Err(format!("arity {text}"));
    }
    Ok(v)
}

fn at(p: &Pat, point: &[i64]) -> Key {
    p.iter()
        .map(|(a, o)| a.map_or(*o, |a| point[a] + o))
        .collect()
}

/// Vectorised evaluation: one value per sample `(p_s, point mod p_s, d_s)`. A vanishing denominator is an error.
fn veval(
    node: &Node,
    n: usize,
    ps: &[u64],
    pt: &[Vec<u64>],
    ds: &[u64],
) -> Result<Vec<u64>, String> {
    let s = ps.len();
    Ok(match node {
        Node::Num(t) => (0..s)
            .map(|k| {
                t.bytes().fold(0u64, |acc, c| {
                    arith::add(ps[k], arith::mul(ps[k], acc, 10), (c - b'0') as u64)
                })
            })
            .collect(),
        Node::Var(v) => (0..s)
            .map(|k| if *v == n { ds[k] } else { pt[k][*v] })
            .collect(),
        Node::Neg(a) => {
            let x = veval(a, n, ps, pt, ds)?;
            (0..s).map(|k| arith::neg(ps[k], x[k])).collect()
        }
        Node::Sum(ts) => {
            let mut acc = vec![0u64; s];
            for (neg, t) in ts {
                let x = veval(t, n, ps, pt, ds)?;
                for k in 0..s {
                    acc[k] = if *neg {
                        arith::sub(ps[k], acc[k], x[k])
                    } else {
                        arith::add(ps[k], acc[k], x[k])
                    };
                }
            }
            acc
        }
        Node::Prod(fs) => {
            let mut acc = vec![1u64; s];
            for (op, f) in fs {
                let x = veval(f, n, ps, pt, ds)?;
                for k in 0..s {
                    let y = if *op == '/' {
                        arith::inv(ps[k], x[k]).ok_or("coefficient pole")?
                    } else {
                        x[k]
                    };
                    acc[k] = arith::mul(ps[k], acc[k], y);
                }
            }
            acc
        }
        Node::Bin(op, a, b) => {
            let x = veval(a, n, ps, pt, ds)?;
            if *op == '^' {
                let e: i64 = match &**b {
                    Node::Num(t) => t.parse().map_err(|_| "exponent")?,
                    Node::Neg(bb) => match &**bb {
                        Node::Num(t) => -t.parse::<i64>().map_err(|_| "exponent")?,
                        _ => return Err("exponent".into()),
                    },
                    _ => return Err("exponent".into()),
                };
                (0..s)
                    .map(|k| {
                        if e >= 0 {
                            Ok(arith::pow(ps[k], x[k], e as u64))
                        } else {
                            Ok(arith::pow(
                                ps[k],
                                arith::inv(ps[k], x[k]).ok_or("coefficient pole")?,
                                (-e) as u64,
                            ))
                        }
                    })
                    .collect::<Result<_, String>>()?
            } else {
                let y = veval(b, n, ps, pt, ds)?;
                (0..s)
                    .map(|k| {
                        Ok(match op {
                            '+' => arith::add(ps[k], x[k], y[k]),
                            '-' => arith::sub(ps[k], x[k], y[k]),
                            '*' => arith::mul(ps[k], x[k], y[k]),
                            '/' => arith::mul(
                                ps[k],
                                x[k],
                                arith::inv(ps[k], y[k]).ok_or("coefficient pole")?,
                            ),
                            _ => return Err(format!("operator {op}")),
                        })
                    })
                    .collect::<Result<_, String>>()?
            }
        }
    })
}

struct Rule {
    fixed: Vec<Option<i64>>,
    equations: Vec<Expr>,
    exceptions: Vec<Vec<Expr>>,
    target: Pat,
    rhs_pats: Vec<Pat>,
    rhs_text: Vec<String>,
    rhs_expr: Option<Vec<Expr>>,
    skip: bool,
}

/// A node table loaded for reduction.
pub struct Table {
    pub path: String,
    pub n: usize,
    pub mask: Vec<bool>,
    zero: HashSet<Vec<bool>>,
    rules: Vec<Rule>,
    /// Masters in table order: residual records, else an Astra composite's terminal ledger, else set by the caller.
    pub masters: Vec<Key>,
    pub family_source: Option<String>,
    pub family_file: Option<String>,
}

/// Result of [`Table::reduce_mod`]: per terminal key, per seed, one value per sample (prime-major: `pi * nd + di`).
pub struct ModReduction {
    pub seeds: Vec<Key>,
    pub primes: Vec<u64>,
    pub dims: Vec<Q>,
    pub masters: Vec<(Key, Vec<Vec<u64>>)>,
    pub lower: Vec<(Key, Vec<Vec<u64>>)>,
    pub stats: Value,
}

impl Table {
    pub fn load(path: &str) -> Result<Table, String> {
        let f = BufReader::with_capacity(
            1 << 24,
            std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?,
        );
        let mut t = Table {
            path: path.to_string(),
            n: 0,
            mask: vec![],
            zero: HashSet::new(),
            rules: vec![],
            masters: vec![],
            family_source: None,
            family_file: None,
        };
        let mut terminal: Vec<Key> = vec![];
        for line in f.lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&line).map_err(|e| format!("{path}: {e}"))?;
            match v["type"].as_str().unwrap_or("") {
                "header" => {
                    t.n = v["denominators"]
                        .as_u64()
                        .ok_or("header without denominators")? as usize;
                    t.mask = v["mask"]
                        .as_str()
                        .ok_or("header without mask")?
                        .chars()
                        .map(|c| c == '1')
                        .collect();
                    if v["power_shifts"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|x| x.as_str() != Some("0")))
                    {
                        return Err("eps family (power shifts) not supported".into());
                    }
                    t.family_source = v["family_source"].as_str().map(str::to_string);
                    t.family_file = v["family_file"].as_str().map(str::to_string);
                }
                "zero_sectors" => {
                    for m in v["masks"].as_array().ok_or("zero_sectors without masks")? {
                        t.zero
                            .insert(m.as_str().unwrap_or("").chars().map(|c| c == '1').collect());
                    }
                }
                "chart" => {
                    let c: Vec<bool> = v["chart"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .map(|c| c == '1')
                        .collect();
                    if c != t.mask {
                        return Err("chart other than the node mask".into());
                    }
                    let guards: Vec<&str> = v["guards"]
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    if guards
                        .iter()
                        .enumerate()
                        .any(|(i, g)| (t.mask[i] && *g != "n>=1") || (!t.mask[i] && *g != "n<=0"))
                    {
                        return Err("unsupported chart guards".into());
                    }
                }
                "rule" => {
                    let n = t.n;
                    let fixed = v["fixed"]
                        .as_array()
                        .ok_or("rule without fixed")?
                        .iter()
                        .map(Value::as_i64)
                        .collect();
                    let equations = v
                        .get("equations")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .map(|x| Expr::parse(x.as_str().unwrap_or(""), n))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    let exceptions = v["exceptions"]
                        .as_array()
                        .ok_or("rule without exceptions")?
                        .iter()
                        .map(|b| {
                            b.as_array()
                                .ok_or("exception conjunction")?
                                .iter()
                                .map(|x| Expr::parse(x.as_str().unwrap_or(""), n))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let mut rhs_pats = vec![];
                    let mut rhs_text = vec![];
                    for term in v["rhs"].as_array().ok_or("rule without rhs")? {
                        rhs_pats.push(parse_pat(term[0].as_str().unwrap_or(""), n)?);
                        rhs_text.push(term[1].as_str().unwrap_or("").to_string());
                    }
                    let skip = !matches!(
                        v.get("dispatch").and_then(Value::as_str),
                        None | Some("Partition")
                    );
                    t.rules.push(Rule {
                        fixed,
                        equations,
                        exceptions,
                        target: parse_pat(v["target"].as_str().unwrap_or(""), n)?,
                        rhs_pats,
                        rhs_text,
                        rhs_expr: None,
                        skip,
                    });
                }
                "residual" => t
                    .masters
                    .push(at(&parse_pat(v["key"].as_str().unwrap_or(""), t.n)?, &[])),
                "composite_terminal_ledger" => {
                    for k in v["keys"].as_array().ok_or("terminal ledger without keys")? {
                        terminal.push(at(&parse_pat(k.as_str().unwrap_or(""), t.n)?, &[]));
                    }
                }
                _ => {}
            }
        }
        if t.n == 0 {
            return Err(format!("{path}: no header"));
        }
        if t.masters.is_empty() {
            t.masters = terminal;
        }
        Ok(t)
    }

    /// The family text of the table (header `family_source`, else the file `family_file`).
    pub fn family_text(&self) -> Result<String, String> {
        if let Some(s) = &self.family_source {
            return Ok(s.clone());
        }
        let p = self
            .family_file
            .as_ref()
            .ok_or("table header has neither family_source nor family_file")?;
        std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
    }

    fn in_sector(&self, k: &[i64]) -> Result<bool, String> {
        if (0..self.n).any(|i| !self.mask[i] && k[i] > 0) {
            return Err(format!("ISP in a denominator: {}", key_text(k)));
        }
        Ok((0..self.n).all(|i| !self.mask[i] || k[i] >= 1))
    }

    /// First applicable rule (table order) and the rule point of key `k`.
    fn find_rule(&self, k: &[i64]) -> Option<(usize, Vec<i64>)> {
        let gen_d = [Q { n: 37, d: 7 }, Q { n: -11, d: 5 }];
        let n = self.n;
        'r: for (ri, r) in self.rules.iter().enumerate() {
            if r.skip {
                continue;
            }
            for (i, fx) in r.fixed.iter().enumerate() {
                if let Some(x) = fx {
                    if k[i] != *x {
                        continue 'r;
                    }
                }
            }
            let mut pt = vec![i64::MIN; n];
            for (j, (a, o)) in r.target.iter().enumerate() {
                match a {
                    Some(a) => pt[*a] = k[j] - o,
                    None => {
                        if k[j] != *o {
                            continue 'r;
                        }
                    }
                }
            }
            for (x, f) in pt.iter_mut().zip(r.fixed.iter()).take(n) {
                if *x == i64::MIN {
                    *x = f.unwrap_or(0);
                }
            }
            for e in &r.equations {
                if !e.is_zero_at(&pt, Q::ZERO).unwrap_or(false) {
                    continue 'r;
                }
            }
            for br in &r.exceptions {
                if br
                    .iter()
                    .all(|e| gen_d.iter().all(|d| e.is_zero_at(&pt, *d).unwrap_or(false)))
                {
                    continue 'r;
                }
            }
            return Some((ri, pt));
        }
        None
    }

    /// Reduce `seeds` modulo each prime at each `d`. Errors: a top-sector key no rule covers (listed), a cycle, an
    /// ISP in a denominator with a nonzero coefficient, a coefficient pole at a used rule point.
    pub fn reduce_mod(
        &mut self,
        seeds: &[Key],
        primes: &[u64],
        dims: &[Q],
        keep_lower: bool,
    ) -> Result<ModReduction, String> {
        let t0 = std::time::Instant::now();
        let n = self.n;
        let masters: HashSet<Key> = self.masters.iter().cloned().collect();
        // graph: kind 0 inner, 1 master, 2 lower
        let mut id: HashMap<Key, u32> = HashMap::new();
        let mut nodes: Vec<Key> = vec![];
        let mut kind: Vec<u8> = vec![];
        let mut rule_of: Vec<(u32, Vec<i64>)> = vec![];
        let mut kids: Vec<Vec<u32>> = vec![];
        let mut state: Vec<u8> = vec![];
        let mut post: Vec<u32> = vec![];
        let mut intern = |t: &Table,
                          k: Key,
                          nodes: &mut Vec<Key>,
                          kind: &mut Vec<u8>,
                          rule_of: &mut Vec<(u32, Vec<i64>)>,
                          kids: &mut Vec<Vec<u32>>,
                          state: &mut Vec<u8>|
         -> Result<Option<u32>, String> {
            if let Some(&i) = id.get(&k) {
                return Ok(Some(i));
            }
            let kd = if t.in_sector(&k)? {
                u8::from(masters.contains(&k))
            } else {
                if t.zero
                    .contains(&k.iter().map(|&x| x > 0).collect::<Vec<bool>>())
                {
                    return Ok(None);
                }
                2
            };
            let i = nodes.len() as u32;
            id.insert(k.clone(), i);
            nodes.push(k);
            kind.push(kd);
            rule_of.push((u32::MAX, vec![]));
            kids.push(vec![]);
            state.push(0);
            Ok(Some(i))
        };
        let mut roots = vec![];
        for k in seeds {
            if k.len() != n {
                return Err(format!(
                    "seed {} has {} indices, the table {}",
                    key_text(k),
                    k.len(),
                    n
                ));
            }
            roots.push(
                intern(
                    self,
                    k.clone(),
                    &mut nodes,
                    &mut kind,
                    &mut rule_of,
                    &mut kids,
                    &mut state,
                )?
                .ok_or("seed key in a zero sector")?,
            );
        }
        let mut uncovered: Vec<Key> = vec![];
        let mut vanishing = 0usize;
        let gen_d = [Q { n: 37, d: 7 }, Q { n: -11, d: 5 }];
        for &r in &roots {
            let mut stack: Vec<(u32, usize)> = vec![(r, 0)];
            while let Some((u, ci)) = stack.pop() {
                let ui = u as usize;
                if ci == 0 {
                    if state[ui] == 2 {
                        continue;
                    }
                    if state[ui] == 1 {
                        return Err(format!("cycle at {}", key_text(&nodes[ui])));
                    }
                    state[ui] = 1;
                    if kind[ui] == 0 {
                        match self.find_rule(&nodes[ui]) {
                            Some((ri, pt)) => {
                                if self.rules[ri].rhs_expr.is_none() {
                                    let ex = self.rules[ri]
                                        .rhs_text
                                        .iter()
                                        .map(|t| Expr::parse(t, n))
                                        .collect::<Result<Vec<_>, _>>()?;
                                    self.rules[ri].rhs_expr = Some(ex);
                                    self.rules[ri].rhs_text = vec![];
                                }
                                let mut ch: Vec<u32> = vec![];
                                let npat = self.rules[ri].rhs_pats.len();
                                for ti in 0..npat {
                                    let k = at(&self.rules[ri].rhs_pats[ti], &pt);
                                    if (0..n).any(|i| !self.mask[i] && k[i] > 0) {
                                        // outside the family support: allowed only with a coefficient vanishing here
                                        let e =
                                            &self.rules[ri].rhs_expr.as_ref().expect("parsed")[ti];
                                        if !gen_d
                                            .iter()
                                            .all(|d| e.is_zero_at(&pt, *d).unwrap_or(false))
                                        {
                                            return Err(format!(
                                                "ISP in a denominator with a nonzero coefficient: {} from {} (rule {ri})",
                                                key_text(&k),
                                                key_text(&nodes[ui])
                                            ));
                                        }
                                        vanishing += 1;
                                        ch.push(u32::MAX);
                                        continue;
                                    }
                                    ch.push(
                                        intern(
                                            self,
                                            k,
                                            &mut nodes,
                                            &mut kind,
                                            &mut rule_of,
                                            &mut kids,
                                            &mut state,
                                        )?
                                        .unwrap_or(u32::MAX),
                                    );
                                }
                                rule_of[ui] = (ri as u32, pt);
                                kids[ui] = ch;
                            }
                            None => {
                                uncovered.push(nodes[ui].clone());
                                kind[ui] = 3;
                            }
                        }
                    }
                }
                let ch = &kids[ui];
                let mut next = ci;
                let mut pushed = false;
                while next < ch.len() {
                    let c = ch[next];
                    next += 1;
                    if c != u32::MAX && state[c as usize] == 0 {
                        stack.push((u, next));
                        stack.push((c, 0));
                        pushed = true;
                        break;
                    } else if c != u32::MAX && state[c as usize] == 1 {
                        return Err(format!("cycle below {}", key_text(&nodes[ui])));
                    }
                }
                if !pushed {
                    state[ui] = 2;
                    post.push(u);
                }
            }
        }
        if !uncovered.is_empty() {
            return Err(format!(
                "uncovered top-sector keys (no rule, not a master): {}",
                uncovered
                    .iter()
                    .take(20)
                    .map(|k| key_text(k))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        let inner = kind.iter().filter(|&&k| k == 0).count();
        let edges: usize = kids.iter().map(Vec::len).sum();
        // samples, prime-major
        let mut ps = vec![];
        let mut dm = vec![];
        for &p in primes {
            for d in dims {
                ps.push(p);
                dm.push(
                    d.modp(p)
                        .ok_or_else(|| format!("d = {}/{} is a pole mod {p}", d.n, d.d))?,
                );
            }
        }
        let s = ps.len();
        let nseed = roots.len();
        let w = s * nseed;
        let mut acc: Vec<Option<Vec<u64>>> = vec![None; nodes.len()];
        for (j, &r) in roots.iter().enumerate() {
            let a = acc[r as usize].get_or_insert_with(|| vec![0; w]);
            for k in 0..s {
                a[j * s + k] = arith::add(ps[k], a[j * s + k], 1);
            }
        }
        let mut terminals: Vec<(u32, Vec<u64>)> = vec![];
        for &u in post.iter().rev() {
            let ui = u as usize;
            let Some(a) = acc[ui].take() else { continue };
            if kind[ui] != 0 {
                terminals.push((u, a));
                continue;
            }
            let (ri, pt) = &rule_of[ui];
            let ex = self.rules[*ri as usize].rhs_expr.as_ref().expect("parsed");
            let ptm: Vec<Vec<u64>> = ps
                .iter()
                .map(|&p| pt.iter().map(|&x| arith::from_i128(p, x as i128)).collect())
                .collect();
            for (ti, &c) in kids[ui].iter().enumerate() {
                if c == u32::MAX {
                    continue;
                }
                let cv = veval(&ex[ti].root, n, &ps, &ptm, &dm)
                    .map_err(|e| format!("{e} at {} (rule {ri})", key_text(&nodes[ui])))?;
                let dst = acc[c as usize].get_or_insert_with(|| vec![0; w]);
                for j in 0..nseed {
                    for k in 0..s {
                        let p = ps[k];
                        dst[j * s + k] =
                            arith::add(p, dst[j * s + k], arith::mul(p, a[j * s + k], cv[k]));
                    }
                }
            }
        }
        let split = |a: &[u64]| -> Vec<Vec<u64>> {
            (0..nseed).map(|j| a[j * s..(j + 1) * s].to_vec()).collect()
        };
        let mut out_m = vec![];
        let mut out_l = vec![];
        let mut lower_nonzero = 0usize;
        for (u, a) in &terminals {
            let k = nodes[*u as usize].clone();
            if kind[*u as usize] == 1 {
                out_m.push((k, split(a)));
            } else if a.iter().any(|&x| x != 0) {
                lower_nonzero += 1;
                if keep_lower {
                    out_l.push((k, split(a)));
                }
            }
        }
        out_m.sort();
        out_l.sort();
        Ok(ModReduction {
            seeds: seeds.to_vec(),
            primes: primes.to_vec(),
            dims: dims.to_vec(),
            masters: out_m,
            lower: out_l,
            stats: json!({"keys": nodes.len(), "inner": inner, "edges": edges, "lower_nonzero": lower_nonzero,
                "vanishing_support_terms": vanishing, "rules_used": self.rules.iter().filter(|r| r.rhs_expr.is_some()).count(),
                "seconds": t0.elapsed().as_secs_f64()}),
        })
    }

    /// Top-sector keys reached from `seeds` that no rule covers and that are not declared masters (for a table
    /// without residual records: its masters).
    pub fn uncovered(&mut self, seeds: &[Key]) -> Result<Vec<Key>, String> {
        let saved = self.masters.clone();
        let mut found: Vec<Key> = vec![];
        let res = loop {
            self.masters = saved.iter().chain(&found).cloned().collect();
            match self.reduce_mod(seeds, &PRIMES[..1], &[Q { n: 37, d: 7 }], false) {
                Ok(_) => break Ok(found.clone()),
                Err(e) if e.starts_with("uncovered top-sector keys") => {
                    let more =
                        parse_keys(&e.split(": ").nth(1).unwrap_or("").replace(") I(", ");I("))?;
                    if more.is_empty() || more.iter().all(|k| found.contains(k)) {
                        break Err(e);
                    }
                    found.extend(more);
                }
                Err(e) => break Err(e),
            }
        };
        self.masters = saved;
        res
    }
}

/// CRT of residues `rs` modulo `ps` -> (value in [0, M), M).
fn crt(rs: &[u64], ps: &[u64]) -> (Integer, Integer) {
    let mut x = Integer::zero();
    let mut m = Integer::one();
    for (&r, &p) in rs.iter().zip(ps) {
        let pi = Integer::from(p);
        let xm = x.quot_rem(&pi).1.to_i64().expect("residue < p") as u64;
        let mm = m.quot_rem(&pi).1.to_i64().expect("residue < p") as u64;
        let t = arith::mul(
            p,
            arith::sub(p, r % p, xm),
            arith::inv(p, mm).expect("distinct primes"),
        );
        x = &x + &(&m * &Integer::from(t));
        m = &m * &pi;
    }
    (x, m)
}

/// Wang's rational reconstruction with |num|, den <= 2^half_bits (2 * bound^2 <= M).
fn ratrec(a: &Integer, m: &Integer, half_bits: u32) -> Option<Rational> {
    if a.is_zero() {
        return Some(Rational::zero());
    }
    let bound = Integer::new(2).pow(half_bits as u64);
    let (mut r0, mut r1) = (m.clone(), a.clone());
    let (mut s0, mut s1) = (Integer::zero(), Integer::one());
    while r1 > bound {
        let (q, r2) = r0.quot_rem(&r1);
        r0 = std::mem::replace(&mut r1, r2);
        let s2 = &s0 - &(&q * &s1);
        s0 = std::mem::replace(&mut s1, s2);
    }
    if s1.is_zero() || s1.abs() > bound {
        return None;
    }
    let g = r1.gcd(&s1);
    if !g.is_one() {
        return None;
    }
    Some(Rational::from((r1, s1)))
}

fn half_bits(ps: &[u64]) -> u32 {
    let bits: u32 = ps.iter().map(|p| 63 - p.leading_zeros()).sum();
    bits.saturating_sub(2) / 2
}

/// Exact rational from residues modulo `ps` (at least 2 primes), confirmed by leaving the last prime out.
pub fn reconstruct(rs: &[u64], ps: &[u64]) -> Option<Rational> {
    if ps.len() < 2 {
        return None;
    }
    let (a, m) = crt(rs, ps);
    let f = ratrec(&a, &m, half_bits(ps))?;
    let (a2, m2) = crt(&rs[..rs.len() - 1], &ps[..ps.len() - 1]);
    let f2 = ratrec(&a2, &m2, half_bits(&ps[..ps.len() - 1]))?;
    (f == f2).then_some(f)
}

pub fn rat_modp(x: &Rational, p: u64) -> Option<u64> {
    let pi = Integer::from(p);
    let n = x.numerator().quot_rem(&pi).1.to_i64()? as u64;
    let d = x.denominator().quot_rem(&pi).1.to_i64()? as u64;
    Some(arith::mul(p, n, arith::inv(p, d)?))
}

pub fn rat_of_q(q: &Q) -> Rational {
    Rational::from((Integer::from(q.n), Integer::from(q.d)))
}

impl ModReduction {
    fn values_at(&self, v: &[u64], di: usize) -> Vec<u64> {
        let nd = self.dims.len();
        (0..self.primes.len()).map(|pi| v[pi * nd + di]).collect()
    }

    /// Exact master coefficients per seed: master -> one rational per `d` (Err when a reconstruction fails).
    pub fn exact_masters(&self) -> Result<Vec<BTreeMap<Key, Vec<Rational>>>, String> {
        self.exact_of(&self.masters)
    }

    pub fn exact_lower(&self) -> Result<Vec<BTreeMap<Key, Vec<Rational>>>, String> {
        self.exact_of(&self.lower)
    }

    fn exact_of(
        &self,
        part: &[(Key, Vec<Vec<u64>>)],
    ) -> Result<Vec<BTreeMap<Key, Vec<Rational>>>, String> {
        let mut out = vec![BTreeMap::new(); self.seeds.len()];
        for (k, per_seed) in part {
            for (j, v) in per_seed.iter().enumerate() {
                let mut row = vec![];
                for di in 0..self.dims.len() {
                    row.push(
                        reconstruct(&self.values_at(v, di), &self.primes).ok_or_else(|| {
                            format!(
                                "rational reconstruction failed: {} in {} at d = {}/{} ({} primes)",
                                key_text(k),
                                key_text(&self.seeds[j]),
                                self.dims[di].n,
                                self.dims[di].d,
                                self.primes.len()
                            )
                        })?,
                    );
                }
                if row.iter().any(|x| !x.is_zero()) {
                    out[j].insert(k.clone(), row);
                }
            }
        }
        Ok(out)
    }

    /// Symbolic-in-`d` fits (integer sample `d` required): per seed, terminal key -> N(d)/D(d).
    pub fn fit_part(&self, lower: bool) -> Result<FitPart, String> {
        let xs: Vec<i64> = self
            .dims
            .iter()
            .map(|d| {
                if d.d == 1 {
                    Ok(d.n as i64)
                } else {
                    Err("symbolic fits need integer sample d".to_string())
                }
            })
            .collect::<Result<_, _>>()?;
        let part = if lower { &self.lower } else { &self.masters };
        let nd = self.dims.len();
        let mut out = vec![BTreeMap::new(); self.seeds.len()];
        let mut failed = vec![];
        for (k, per_seed) in part {
            for (j, v) in per_seed.iter().enumerate() {
                let ys: Vec<Vec<u64>> = (0..self.primes.len())
                    .map(|pi| v[pi * nd..(pi + 1) * nd].to_vec())
                    .collect();
                match fit(&xs, &ys, &self.primes) {
                    Ok(f) => {
                        if !f.is_zero() {
                            out[j].insert(k.clone(), f);
                        }
                    }
                    Err(e) => failed.push((k.clone(), e)),
                }
            }
        }
        Ok((out, failed))
    }
}

/// A univariate rational function in `d`, coefficients low -> high, denominator monic.
#[derive(Clone, Debug, PartialEq)]
pub struct RatFun {
    pub num: Vec<Rational>,
    pub den: Vec<Rational>,
}

fn poly_eval(p: &[Rational], x: &Rational) -> Rational {
    let mut v = Rational::zero();
    for c in p.iter().rev() {
        v = &(&v * x) + c;
    }
    v
}

fn poly_text(p: &[Rational]) -> String {
    let t: Vec<String> = p
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_zero())
        .map(|(i, c)| match i {
            0 => format!("({c})"),
            1 => format!("({c})*d"),
            _ => format!("({c})*d^{i}"),
        })
        .collect();
    if t.is_empty() {
        "0".into()
    } else {
        t.join(" + ")
    }
}

/// Multiplicity of the root `x` of `p` (None for the zero polynomial).
fn root_order(p: &[Rational], x: &Rational) -> Option<i64> {
    if p.iter().all(Rational::is_zero) {
        return None;
    }
    let mut c: Vec<Rational> = p.to_vec();
    let mut v = 0;
    loop {
        // synthetic division by (d - x)
        let mut q = vec![Rational::zero(); c.len().saturating_sub(1)];
        let mut r = Rational::zero();
        for i in (0..c.len()).rev() {
            r = &(&r * x) + &c[i];
            if i > 0 {
                q[i - 1] = r.clone();
            }
        }
        if !r.is_zero() || q.is_empty() {
            return Some(v);
        }
        c = q;
        v += 1;
    }
}

impl RatFun {
    pub fn is_zero(&self) -> bool {
        self.num.iter().all(Rational::is_zero)
    }
    pub fn eval(&self, x: &Rational) -> Option<Rational> {
        let d = poly_eval(&self.den, x);
        if d.is_zero() {
            None
        } else {
            Some(&poly_eval(&self.num, x) / &d)
        }
    }
    pub fn text(&self) -> String {
        format!("({})/({})", poly_text(&self.num), poly_text(&self.den))
    }
    /// Order at `d = x` (negative: pole of that order; positive: zero); None for the zero function.
    pub fn order_at(&self, x: &Rational) -> Option<i64> {
        Some(root_order(&self.num, x)? - root_order(&self.den, x).unwrap_or(0))
    }
    /// Small rational roots (|num| <= 60, den <= 6) of numerator and denominator: exceptional values of `d`.
    pub fn small_roots(&self) -> (Vec<String>, Vec<String>) {
        let roots = |p: &[Rational]| -> Vec<String> {
            let mut out: Vec<Rational> = vec![];
            for den in 1..=6i64 {
                for num in -60..=60i64 {
                    let x = Rational::from((num, den));
                    if !out.contains(&x) && root_order(p, &x).unwrap_or(0) > 0 {
                        out.push(x);
                    }
                }
            }
            out.iter().map(Rational::to_string).collect()
        };
        (roots(&self.num), roots(&self.den))
    }
    pub fn to_json(&self) -> Value {
        let four = Rational::from(4);
        json!({"text": self.text(), "num": self.num.iter().map(Rational::to_string).collect::<Vec<_>>(),
            "den": self.den.iter().map(Rational::to_string).collect::<Vec<_>>(),
            "deg": [self.num.len().saturating_sub(1), self.den.len().saturating_sub(1)],
            "order_at_d4": self.order_at(&four)})
    }
}

fn ev_p(coef: &[u64], xs: &[u64], x: u64, p: u64) -> Option<u64> {
    let mut v = *coef.last()?;
    for k in (0..coef.len() - 1).rev() {
        let iv = arith::inv(p, v)?;
        v = arith::add(p, coef[k], arith::mul(p, arith::sub(p, x, xs[k]), iv));
    }
    Some(v)
}

fn padd(p: u64, a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut r = vec![0; a.len().max(b.len())];
    for (i, c) in a.iter().enumerate() {
        r[i] = arith::add(p, r[i], *c);
    }
    for (i, c) in b.iter().enumerate() {
        r[i] = arith::add(p, r[i], *c);
    }
    while r.len() > 1 && *r.last().expect("nonempty") == 0 {
        r.pop();
    }
    r
}

fn pmul(p: u64, a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut r = vec![0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            r[i + j] = arith::add(p, r[i + j], arith::mul(p, *x, *y));
        }
    }
    r
}

/// Thiele interpolation over Z_p (stops at the first stage whose remaining inverse differences are constant; the
/// continued fraction is then checked on every point) -> (N, D) with D monic.
fn fit_p(xs: &[u64], ys: &[u64], p: u64) -> Option<(Vec<u64>, Vec<u64>)> {
    if ys.iter().all(|&y| y % p == 0) {
        return Some((vec![0], vec![1]));
    }
    let n = xs.len();
    let mut cur: Vec<u64> = ys.iter().map(|&y| y % p).collect();
    let mut coef = vec![cur[0]];
    for k in 1..n.saturating_sub(4) {
        if (k..n).all(|i| cur[i] == cur[k - 1]) {
            if !xs
                .iter()
                .zip(ys)
                .all(|(&x, &y)| ev_p(&coef, &xs[..k], x, p) == Some(y % p))
            {
                return None;
            }
            // continued fraction -> N/D
            let mut num = vec![*coef.last().expect("nonempty")];
            let mut den = vec![1u64];
            for j in (0..coef.len() - 1).rev() {
                let lin = vec![arith::neg(p, xs[j]), 1];
                let nn = padd(p, &pmul(p, &[coef[j]], &num), &pmul(p, &lin, &den));
                den = num;
                num = nn;
            }
            let lead = arith::inv(p, *den.last()?)?;
            return Some((
                num.iter().map(|c| arith::mul(p, *c, lead)).collect(),
                den.iter().map(|c| arith::mul(p, *c, lead)).collect(),
            ));
        }
        let mut nxt = vec![0u64; n];
        for i in k..n {
            let dn = arith::sub(p, cur[i], cur[k - 1]);
            nxt[i] = arith::mul(p, arith::sub(p, xs[i], xs[k - 1]), arith::inv(p, dn)?);
        }
        coef.push(nxt[k]);
        cur = nxt;
    }
    None
}

/// Symbolic fit of samples `ys[prime][point]` at integer points `xs`: per-prime Thiele, coefficient-wise CRT +
/// rational reconstruction (leave-one-out), then every sample reproduced mod every prime.
pub fn fit(xs: &[i64], ys: &[Vec<u64>], primes: &[u64]) -> Result<RatFun, String> {
    let per: Vec<(Vec<u64>, Vec<u64>)> = primes
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let xp: Vec<u64> = xs.iter().map(|&x| arith::from_i128(p, x as i128)).collect();
            fit_p(&xp, &ys[i], p)
                .ok_or_else(|| "Thiele fit (too few points or unlucky)".to_string())
        })
        .collect::<Result<_, _>>()?;
    let shape = (per[0].0.len(), per[0].1.len());
    if per.iter().any(|(a, b)| (a.len(), b.len()) != shape) {
        return Err("degrees differ between primes".into());
    }
    let mut polys = vec![];
    for (which, len) in [(0usize, shape.0), (1, shape.1)] {
        let mut cs = vec![];
        for j in 0..len {
            let rs: Vec<u64> = per
                .iter()
                .map(|(a, b)| if which == 0 { a[j] } else { b[j] })
                .collect();
            cs.push(reconstruct(&rs, primes).ok_or("coefficient reconstruction")?);
        }
        polys.push(cs);
    }
    let f = RatFun {
        den: polys.pop().expect("two"),
        num: polys.pop().expect("two"),
    };
    for (i, &p) in primes.iter().enumerate() {
        for (x, &y) in xs.iter().zip(&ys[i]) {
            let xr = Rational::from(*x);
            let v = f.eval(&xr).ok_or("fit has a pole at a sample")?;
            if rat_modp(&v, p) != Some(y % p) {
                return Err("fit does not reproduce a sample".into());
            }
        }
    }
    Ok(f)
}

/// Independent Laporta of `target` in the top sector of `mask` modulo subsectors: RustRed's IBP rows of the family
/// ([`Family::rows_at`]) at all seeds with lines >= 1, at most `dots` dots and ISP rank <= `rank`; sparse elimination
/// mod `p` with `target` and `basis` ordered lowest. Returns (status, basis coefficients, other free columns).
#[allow(clippy::too_many_arguments)]
pub fn laporta(
    fam: &Family,
    mask: &[bool],
    target: &Key,
    basis: &[Key],
    dots: i64,
    rank: i64,
    d: &Q,
    p: u64,
) -> Result<Value, String> {
    let n = fam.n;
    let lines: Vec<usize> = (0..n).filter(|&i| mask[i]).collect();
    let isps: Vec<usize> = (0..n).filter(|&i| !mask[i]).collect();
    fn comps(k: usize, tot: i64) -> Vec<Vec<i64>> {
        let mut out = vec![vec![]];
        for _ in 0..k {
            let mut nx = vec![];
            for v in &out {
                let used: i64 = v.iter().sum();
                for x in 0..=(tot - used) {
                    let mut w = v.clone();
                    w.push(x);
                    nx.push(w);
                }
            }
            out = nx;
        }
        out
    }
    let mut seeds = vec![];
    for dv in comps(lines.len(), dots) {
        for rv in comps(isps.len(), rank) {
            let mut k = vec![0i64; n];
            for (j, &l) in lines.iter().enumerate() {
                k[l] = 1 + dv[j];
            }
            for (j, &i) in isps.iter().enumerate() {
                k[i] = -rv[j];
            }
            seeds.push(k);
        }
    }
    let mut masters: Vec<Key> = vec![target.clone()];
    masters.extend(basis.iter().filter(|b| *b != target).cloned());
    let weight = |k: &Key| -> (i64, i64, i64, i64, Key) {
        if let Some(pos) = masters.iter().position(|m| m == k) {
            return (0, -(pos as i64), 0, 0, k.clone());
        }
        let dts: i64 = lines.iter().map(|&l| k[l] - 1).sum();
        let rk: i64 = isps.iter().map(|&i| -k[i]).sum();
        (1, dts + rk, dts, rk, k.clone())
    };
    let dmod = d.modp(p).ok_or("d pole")?;
    let mut rows: Vec<Vec<(Key, u64)>> = vec![];
    let mut cols: HashSet<Key> = HashSet::new();
    for sk in &seeds {
        for r in fam.rows_at(sk, dmod, p)? {
            let mut row = vec![];
            for (off, c) in r {
                let k: Key = sk.iter().zip(&off).map(|(a, b)| a + b).collect();
                if lines.iter().any(|&l| k[l] <= 0) {
                    continue;
                }
                cols.insert(k.clone());
                row.push((k, c));
            }
            if !row.is_empty() {
                rows.push(row);
            }
        }
    }
    let mut colv: Vec<Key> = cols.into_iter().collect();
    colv.sort_by_key(|k| std::cmp::Reverse(weight(k)));
    let cid: HashMap<Key, usize> = colv
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();
    let mut piv: HashMap<usize, Vec<(usize, u64)>> = HashMap::new();
    let nrows = rows.len();
    for row in rows {
        let mut r: Vec<(usize, u64)> = row.iter().map(|(k, c)| (cid[k], *c)).collect();
        r.sort_unstable();
        let mut m: Vec<(usize, u64)> = vec![];
        for (c, v) in r {
            if let Some(l) = m.last_mut() {
                if l.0 == c {
                    l.1 = arith::add(p, l.1, v);
                    continue;
                }
            }
            m.push((c, v));
        }
        m.retain(|x| x.1 != 0);
        let mut r = m;
        while let Some(&(c0, v0)) = r.first() {
            if let Some(pr) = piv.get(&c0) {
                let mut outr = Vec::with_capacity(r.len() + pr.len());
                let (mut i, mut j) = (0, 0);
                while i < r.len() || j < pr.len() {
                    if j >= pr.len() || (i < r.len() && r[i].0 < pr[j].0) {
                        outr.push(r[i]);
                        i += 1;
                    } else if i >= r.len() || pr[j].0 < r[i].0 {
                        outr.push((pr[j].0, arith::neg(p, arith::mul(p, v0, pr[j].1))));
                        j += 1;
                    } else {
                        let v = arith::sub(p, r[i].1, arith::mul(p, v0, pr[j].1));
                        if v != 0 {
                            outr.push((r[i].0, v));
                        }
                        i += 1;
                        j += 1;
                    }
                }
                r = outr;
            } else {
                let iv = arith::inv(p, v0).ok_or("zero pivot")?;
                piv.insert(
                    c0,
                    r.iter().map(|&(c, v)| (c, arith::mul(p, v, iv))).collect(),
                );
                break;
            }
        }
    }
    let mut expr: HashMap<usize, u64> = HashMap::new();
    let mut status = "target not a pivot";
    if let Some(&t) = cid.get(target) {
        if let Some(pr) = piv.get(&t) {
            let mut work: BTreeMap<usize, u64> = BTreeMap::new();
            for &(c, v) in &pr[1..] {
                work.insert(c, arith::neg(p, v));
            }
            while let Some((&c, &v)) = work.iter().next() {
                work.remove(&c);
                if v == 0 {
                    continue;
                }
                if let Some(pc) = piv.get(&c) {
                    for &(c2, v2) in &pc[1..] {
                        let e = work.entry(c2).or_insert(0);
                        *e = arith::sub(p, *e, arith::mul(p, v, v2));
                    }
                } else {
                    let e = expr.entry(c).or_insert(0);
                    *e = arith::add(p, *e, v);
                }
            }
            status = "ok";
        }
    }
    let mut terms = BTreeMap::new();
    let mut extra = 0usize;
    for (c, v) in &expr {
        if *v == 0 {
            continue;
        }
        if basis.contains(&colv[*c]) {
            terms.insert(key_text(&colv[*c]), v.to_string());
        } else {
            extra += 1;
        }
    }
    Ok(
        json!({"d": format!("{}/{}", d.n, d.d), "p": p.to_string(), "seeds": seeds.len(), "rows": nrows, "cols": colv.len(),
        "pivots": piv.len(), "status": status, "terms": terms, "extra_terms": extra}),
    )
}

/// Options of [`build_map`].
pub struct MapOptions {
    pub dims: Vec<Q>,
    pub primes: Vec<u64>,
    pub symbolic: bool,
    pub sym_points: usize,
    pub sym_primes: usize,
    pub sym_start: i64,
    /// Also fit every lower-sector coefficient symbolically (large; written into the map).
    pub symbolic_lower: bool,
    /// Laporta seeds (dots, ISP rank); None = no Laporta.
    pub laporta: Option<(i64, i64)>,
    /// Few-integral check keys; None = the automatic four per changed master.
    pub check_keys: Option<Vec<Key>>,
    pub no_check: bool,
}

impl Default for MapOptions {
    fn default() -> Self {
        MapOptions {
            dims: check_dims(),
            primes: PRIMES[..6].to_vec(),
            symbolic: true,
            sym_points: 80,
            sym_primes: 8,
            sym_start: 1001,
            symbolic_lower: false,
            laporta: Some((1, 1)),
            check_keys: None,
            no_check: false,
        }
    }
}

fn rat_json(v: &[Rational]) -> Vec<String> {
    v.iter().map(Rational::to_string).collect()
}

/// Rows of a basis change: `row[a][b]` = coefficient of basis-`b` master in master `a`, per check `d`.
type Matrix = Vec<Vec<Vec<Rational>>>;

/// The automatic few-integral check keys: per changed master pair, the corner (first shared master) with a dot, the
/// corner with an ISP numerator, each old-only master with a numerator, each new-only master with a dot and a numerator.
pub fn auto_check_keys(
    mask: &[bool],
    shared: &[Key],
    old_only: &[Key],
    new_only: &[Key],
) -> Vec<Key> {
    let n = mask.len();
    let lines: Vec<usize> = (0..n).filter(|&i| mask[i]).collect();
    let isps: Vec<usize> = (0..n).filter(|&i| !mask[i]).collect();
    let mut out: Vec<Key> = vec![];
    let all: Vec<&Key> = shared.iter().chain(old_only).chain(new_only).collect();
    if let Some(c) = shared.first() {
        if let Some(&l) = lines.iter().find(|&&l| all.iter().all(|m| m[l] == 1)) {
            let mut a = c.clone();
            a[l] += 1;
            out.push(a);
        }
        if let Some(&i) = isps.first() {
            let mut a = c.clone();
            a[i] -= 1;
            out.push(a);
        }
    }
    for o in old_only {
        if let Some(&i) = isps.get(1).or(isps.first()) {
            let mut a = o.clone();
            a[i] -= 1;
            out.push(a);
        }
    }
    for m in new_only {
        let mut a = m.clone();
        if let Some(&l) = lines.iter().rev().find(|&&l| m[l] == 1) {
            a[l] += 1;
        }
        if let Some(&i) = isps.last() {
            a[i] -= 1;
        }
        out.push(a);
    }
    out.sort();
    out.dedup();
    out.retain(|k| !all.contains(&k));
    out
}

/// Few-integral cross-check: `keys` reduced with the OLD table must equal their NEW-table reduction mapped back
/// through `new_in_old` (top-sector part, exact at each `d`).
pub fn few_check(
    old: &mut Table,
    new: &mut Table,
    keys: &[Key],
    new_in_old: &Matrix,
    opt: &MapOptions,
) -> Result<Value, String> {
    let ro = old
        .reduce_mod(keys, &opt.primes, &opt.dims, false)?
        .exact_masters()?;
    let rn = new
        .reduce_mod(keys, &opt.primes, &opt.dims, false)?
        .exact_masters()?;
    let mut all = true;
    let mut detail = vec![];
    for (j, k) in keys.iter().enumerate() {
        for di in 0..opt.dims.len() {
            let mut eq = true;
            for (b, om) in old.masters.iter().enumerate() {
                let lhs = ro[j].get(om).map_or(Rational::zero(), |v| v[di].clone());
                let mut rhs = Rational::zero();
                for (a, nm) in new.masters.iter().enumerate() {
                    if let Some(v) = rn[j].get(nm) {
                        rhs = &rhs + &(&v[di] * &new_in_old[a][b][di]);
                    }
                }
                eq &= lhs == rhs;
            }
            all &= eq;
            detail.push(json!({"key": key_text(k), "d": format!("{}/{}", opt.dims[di].n, opt.dims[di].d), "equal": eq}));
        }
    }
    Ok(
        json!({"keys": keys.iter().map(|k| key_text(k)).collect::<Vec<_>>(), "all_equal": all, "detail": detail}),
    )
}

/// Exact old <-> new master map of two tables of one family (see the module doc). Returns the map document; its
/// `accepted` field is true when every check that ran passed.
pub fn build_map(old: &mut Table, new: &mut Table, opt: &MapOptions) -> Result<Value, String> {
    let t0 = std::time::Instant::now();
    if old.n != new.n || old.mask != new.mask {
        return Err("tables of different denominators/masks: a slot map is needed first".into());
    }
    if let (Some(a), Some(b)) = (&old.family_source, &new.family_source) {
        if a != b {
            return Err("tables with different family_source: a slot map is needed first".into());
        }
    }
    if old.masters.is_empty() || new.masters.is_empty() {
        return Err("a table without masters (pass them explicitly)".into());
    }
    let shared: Vec<Key> = new
        .masters
        .iter()
        .filter(|k| old.masters.contains(k))
        .cloned()
        .collect();
    let new_only: Vec<Key> = new
        .masters
        .iter()
        .filter(|k| !old.masters.contains(k))
        .cloned()
        .collect();
    let old_only: Vec<Key> = old
        .masters
        .iter()
        .filter(|k| !new.masters.contains(k))
        .cloned()
        .collect();
    let nd = opt.dims.len();
    let mut doc = json!({
        "tool": "rustred rr_node mastermap (node::mastermap)", "old_table": old.path, "new_table": new.path,
        "masters_old": old.masters.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "masters_new": new.masters.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "shared": shared.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "new_only": new_only.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "old_only": old_only.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "dims": opt.dims.iter().map(|d| format!("{}/{}", d.n, d.d)).collect::<Vec<_>>(),
        "primes": opt.primes.len(),
    });
    // exact rows at the check d
    let a_red = if new_only.is_empty() {
        None
    } else {
        Some(old.reduce_mod(&new_only, &opt.primes, &opt.dims, false)?)
    };
    let b_red = if old_only.is_empty() {
        None
    } else {
        Some(new.reduce_mod(&old_only, &opt.primes, &opt.dims, false)?)
    };
    let a_ex = a_red
        .as_ref()
        .map(ModReduction::exact_masters)
        .transpose()?
        .unwrap_or_default();
    let b_ex = b_red
        .as_ref()
        .map(ModReduction::exact_masters)
        .transpose()?
        .unwrap_or_default();
    let unit = |a: &Key, b: &Key| -> Vec<Rational> {
        vec![
            if a == b {
                Rational::one()
            } else {
                Rational::zero()
            };
            nd
        ]
    };
    let row_of = |m: &Key,
                  only: &[Key],
                  ex: &[BTreeMap<Key, Vec<Rational>>],
                  basis: &[Key]|
     -> Vec<Vec<Rational>> {
        match only.iter().position(|x| x == m) {
            Some(j) => basis
                .iter()
                .map(|b| {
                    ex[j]
                        .get(b)
                        .cloned()
                        .unwrap_or_else(|| vec![Rational::zero(); nd])
                })
                .collect(),
            None => basis.iter().map(|b| unit(m, b)).collect(),
        }
    };
    let new_in_old: Matrix = new
        .masters
        .iter()
        .map(|m| row_of(m, &new_only, &a_ex, &old.masters))
        .collect();
    let old_in_new: Matrix = old
        .masters
        .iter()
        .map(|m| row_of(m, &old_only, &b_ex, &new.masters))
        .collect();
    // a reduction that ends outside the other basis (non-master top-sector terminal) cannot happen: masters are the
    // only top-sector terminals; check that the composition is the identity both ways
    let compose = |x: &Matrix, y: &Matrix, di: usize| -> Vec<Vec<Rational>> {
        x.iter()
            .map(|row| {
                (0..y[0].len())
                    .map(|c| {
                        row.iter().zip(y).fold(Rational::zero(), |s, (xv, yrow)| {
                            &s + &(&xv[di] * &yrow[c][di])
                        })
                    })
                    .collect()
            })
            .collect()
    };
    let is_id = |m: &[Vec<Rational>]| {
        m.iter().enumerate().all(|(i, r)| {
            r.iter()
                .enumerate()
                .all(|(j, v)| if i == j { v.is_one() } else { v.is_zero() })
        })
    };
    let square = new.masters.len() == old.masters.len();
    let mut inv_ok = square;
    if square {
        for di in 0..nd {
            inv_ok &= is_id(&compose(&new_in_old, &old_in_new, di))
                && is_id(&compose(&old_in_new, &new_in_old, di));
        }
    }
    let mat_json = |m: &Matrix, rows: &[Key], basis: &[Key]| -> Value {
        let mut o = serde_json::Map::new();
        for (a, r) in rows.iter().zip(m) {
            let mut e = serde_json::Map::new();
            for (b, v) in basis.iter().zip(r) {
                if v.iter().any(|x| !x.is_zero()) {
                    e.insert(key_text(b), json!(rat_json(v)));
                }
            }
            o.insert(key_text(a), Value::Object(e));
        }
        Value::Object(o)
    };
    doc["new_in_old_exact"] = mat_json(&new_in_old, &new.masters, &old.masters);
    doc["old_in_new_exact"] = mat_json(&old_in_new, &old.masters, &new.masters);
    doc["mutually_inverse_exact"] = json!(inv_ok);
    doc["lower_nonzero"] = json!({"new_in_old": a_red.as_ref().map(|r| r.stats["lower_nonzero"].clone()), "old_in_new": b_red.as_ref().map(|r| r.stats["lower_nonzero"].clone())});
    let mut accepted = inv_ok;
    // Laporta: each new-only master in the old basis, independently
    if let (Some((dots, rank)), false) = (opt.laporta, new_only.is_empty()) {
        let fam = Family::parse(&new.family_text()?)?;
        let p = opt.primes[0];
        let mut lap = vec![];
        let mut all = true;
        for (j, t) in new_only.iter().enumerate() {
            for (di, d) in opt.dims.iter().enumerate() {
                let r = laporta(&fam, &new.mask, t, &old.masters, dots, rank, d, p)?;
                let mine: BTreeMap<String, String> = old
                    .masters
                    .iter()
                    .filter_map(|b| {
                        a_ex[j].get(b).map(|v| {
                            (
                                key_text(b),
                                rat_modp(&v[di], p).map_or("pole".into(), |x| x.to_string()),
                            )
                        })
                    })
                    .filter(|(_, v)| v != "0")
                    .collect();
                let eq = r["status"] == "ok" && r["extra_terms"] == 0 && r["terms"] == json!(mine);
                all &= eq;
                lap.push(json!({"master": key_text(t), "equal": eq, "laporta": r}));
            }
        }
        doc["laporta"] = json!(lap);
        doc["laporta_equal_all"] = json!(all);
        accepted &= all;
    }
    // symbolic in d
    if opt.symbolic {
        let xs: Vec<Q> = (0..opt.sym_points as i64)
            .map(|k| Q::int((opt.sym_start + k) as i128))
            .collect();
        let ps = &PRIMES[..opt.sym_primes.min(PRIMES.len())];
        let mut sym_rows: BTreeMap<String, BTreeMap<String, RatFun>> = BTreeMap::new();
        let mut fails: Vec<String> = vec![];
        let mut lower_doc = serde_json::Map::new();
        for (only, tab, name) in [
            (&new_only, &mut *old, "new_in_old"),
            (&old_only, &mut *new, "old_in_new"),
        ] {
            if only.is_empty() {
                continue;
            }
            let red = tab.reduce_mod(only, ps, &xs, opt.symbolic_lower)?;
            let (fm, f1) = red.fit_part(false)?;
            for (k, e) in f1 {
                fails.push(format!("{name} master {}: {e}", key_text(&k)));
            }
            for (j, m) in only.iter().enumerate() {
                let row: BTreeMap<String, RatFun> = fm[j]
                    .iter()
                    .map(|(b, f)| (key_text(b), f.clone()))
                    .collect();
                sym_rows.insert(format!("{name}:{}", key_text(m)), row);
            }
            if opt.symbolic_lower {
                let (fl, f2) = red.fit_part(true)?;
                let mut hist: BTreeMap<String, usize> = BTreeMap::new();
                let mut per = serde_json::Map::new();
                for (j, m) in only.iter().enumerate() {
                    let mut e = serde_json::Map::new();
                    for (k, f) in &fl[j] {
                        let o = f.order_at(&Rational::from(4));
                        *hist
                            .entry(o.map_or("zero".into(), |v| v.to_string()))
                            .or_insert(0) += 1;
                        e.insert(key_text(k), f.to_json());
                    }
                    per.insert(key_text(m), Value::Object(e));
                }
                lower_doc.insert(name.into(), json!({"terms": per, "failed": f2.iter().map(|(k, e)| format!("{}: {e}", key_text(k))).collect::<Vec<_>>(), "order_at_d4_histogram": hist}));
            }
        }
        // symbolic consistency: equal to the exact rows at the check d, mutually inverse at fresh d
        let get = |name: &str, m: &Key, b: &Key, x: &Rational| -> Option<Rational> {
            match sym_rows.get(&format!("{name}:{}", key_text(m))) {
                Some(row) => row
                    .get(&key_text(b))
                    .map_or(Some(Rational::zero()), |f| f.eval(x)),
                None => Some(if m == b {
                    Rational::one()
                } else {
                    Rational::zero()
                }),
            }
        };
        let mut sym_ok = fails.is_empty() && square;
        if sym_ok {
            for (di, d) in opt.dims.iter().enumerate() {
                let x = rat_of_q(d);
                for (a, m) in new.masters.iter().enumerate() {
                    for (b, o) in old.masters.iter().enumerate() {
                        sym_ok &=
                            get("new_in_old", m, o, &x).as_ref() == Some(&new_in_old[a][b][di]);
                    }
                }
            }
            for x in [
                Rational::from((53, 11)),
                Rational::from((-7, 3)),
                Rational::from((101, 29)),
            ] {
                let ab: Option<Vec<Vec<Rational>>> = new
                    .masters
                    .iter()
                    .map(|m| {
                        old.masters
                            .iter()
                            .map(|o| get("new_in_old", m, o, &x))
                            .collect()
                    })
                    .collect();
                let ba: Option<Vec<Vec<Rational>>> = old
                    .masters
                    .iter()
                    .map(|o| {
                        new.masters
                            .iter()
                            .map(|m| get("old_in_new", o, m, &x))
                            .collect()
                    })
                    .collect();
                match (ab, ba) {
                    (Some(ab), Some(ba)) => {
                        let prod: Vec<Vec<Rational>> = ab
                            .iter()
                            .map(|r| {
                                (0..ba[0].len())
                                    .map(|c| {
                                        r.iter()
                                            .zip(&ba)
                                            .fold(Rational::zero(), |s, (u, v)| &s + &(u * &v[c]))
                                    })
                                    .collect()
                            })
                            .collect();
                        sym_ok &= is_id(&prod);
                    }
                    _ => sym_ok = false,
                }
            }
        }
        let four = Rational::from(4);
        let mut sym_doc = serde_json::Map::new();
        let mut eps_flags = vec![];
        let mut exceptional: Vec<String> = vec![];
        for (row, terms) in &sym_rows {
            let mut e = serde_json::Map::new();
            for (b, f) in terms {
                let o = f.order_at(&four);
                if o.is_some_and(|v| v != 0) {
                    eps_flags.push(format!("{row} -> {b}: order {} at d = 4", o.unwrap_or(0)));
                }
                let (zr, pr) = f.small_roots();
                exceptional.extend(zr.iter().chain(&pr).cloned());
                let mut j = f.to_json();
                j["num_small_roots"] = json!(zr);
                j["den_small_roots"] = json!(pr);
                e.insert(b.clone(), j);
            }
            sym_doc.insert(row.clone(), Value::Object(e));
        }
        exceptional.sort();
        exceptional.dedup();
        doc["symbolic"] = json!({"points": opt.sym_points, "first_d": opt.sym_start, "primes": ps.len(), "rows": sym_doc,
            "fit_failures": fails, "mutually_inverse_symbolic": sym_ok, "eps_flags": eps_flags,
            "regular_at_d4": eps_flags.is_empty(), "exceptional_d_small_roots": exceptional});
        if opt.symbolic_lower {
            doc["symbolic_lower"] = Value::Object(lower_doc);
        }
        accepted &= sym_ok;
    }
    // few-integral cross-check
    if !opt.no_check {
        let keys = opt
            .check_keys
            .clone()
            .unwrap_or_else(|| auto_check_keys(&new.mask, &shared, &old_only, &new_only));
        if !keys.is_empty() {
            let c = few_check(old, new, &keys, &new_in_old, opt)?;
            accepted &= c["all_equal"] == true;
            doc["few_check"] = c;
        }
    }
    doc["accepted"] = json!(accepted);
    doc["seconds"] = json!(t0.elapsed().as_secs_f64());
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAM2: &str = "I(\n  name(B),\n  loops(k1),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k1-q)^2,1),\n  gram(q,q,1)\n)\n";

    fn header(fam: &str) -> String {
        json!({"type": "header", "schema": "table_dump.v1", "family": "B", "family_source": fam, "denominators": 2, "mask": "11", "power_shifts": ["0", "0"]}).to_string()
    }

    fn rule(fixed: Value, target: &str, rhs: Value, exceptions: Value) -> String {
        json!({"type": "rule", "chart": "11", "fixed": fixed, "target": target, "rhs": rhs, "exceptions": exceptions, "dispatch": "Partition"}).to_string()
    }

    fn write(name: &str, lines: &[String]) -> String {
        let dir = std::env::temp_dir().join(format!("rr-mastermap-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        p.to_string_lossy().into_owned()
    }

    /// A synthetic pair of tables over one 2-propagator family with the hidden relation
    /// I(1,2) = f I(2,1) + g I(1,1), f = (d-4)/(d-5), g = (3d-1)/(d-3) (not IBP, the reducer does not care):
    /// OLD masters {I(1,1), I(2,1)}, NEW masters {I(1,1), I(1,2)}; I(3,1) -> c I(2,1) in both.
    fn tables() -> (String, String) {
        let fam = FAM2;
        let three = rule(
            json!([null, 1]),
            "I(n0,1)",
            json!([["I(n0-1,1)", "(d-n0)/(n0+1)"]]),
            json!([["n0-1"], ["n0-2"]]),
        );
        let old = vec![
            header(fam),
            rule(
                json!([1, 2]),
                "I(1,2)",
                json!([["I(2,1)", "(d-4)/(d-5)"], ["I(1,1)", "(3*d-1)/(d-3)"]]),
                json!([]),
            ),
            three.clone(),
            json!({"type": "residual", "chart": "11", "key": "I(1,1)"}).to_string(),
            json!({"type": "residual", "chart": "11", "key": "I(2,1)"}).to_string(),
        ];
        let new = vec![
            header(fam),
            rule(
                json!([2, 1]),
                "I(2,1)",
                json!([
                    ["I(1,2)", "(d-5)/(d-4)"],
                    ["I(1,1)", "-(3*d-1)*(d-5)/((d-3)*(d-4))"]
                ]),
                json!([]),
            ),
            three,
            json!({"type": "residual", "chart": "11", "key": "I(1,1)"}).to_string(),
            json!({"type": "residual", "chart": "11", "key": "I(1,2)"}).to_string(),
        ];
        (write("old.jsonl", &old), write("new.jsonl", &new))
    }

    #[test]
    fn crt_and_reconstruction_roundtrip() {
        let x = Rational::from((
            Integer::from(-123_456_789_012_345_i64),
            Integer::from(98_765_432_101_i64),
        ));
        let rs: Vec<u64> = PRIMES[..4]
            .iter()
            .map(|&p| rat_modp(&x, p).unwrap())
            .collect();
        assert_eq!(reconstruct(&rs, &PRIMES[..4]), Some(x));
        // too few primes for a large value: no false reconstruction
        let big = Rational::from((Integer::new(10).pow(40) + Integer::one(), Integer::new(7)));
        let rs: Vec<u64> = PRIMES[..2]
            .iter()
            .map(|&p| rat_modp(&big, p).unwrap())
            .collect();
        assert_eq!(reconstruct(&rs, &PRIMES[..2]), None);
    }

    #[test]
    fn thiele_fit_recovers_rational_function() {
        let f = RatFun {
            num: vec![
                Rational::from(-6),
                Rational::from((5, 2)),
                Rational::from(1),
            ],
            den: vec![Rational::from(-4), Rational::from(1)],
        };
        let xs: Vec<i64> = (0..20).map(|k| 1001 + k).collect();
        let ps = &PRIMES[..4];
        let ys: Vec<Vec<u64>> = ps
            .iter()
            .map(|&p| {
                xs.iter()
                    .map(|&x| rat_modp(&f.eval(&Rational::from(x)).unwrap(), p).unwrap())
                    .collect()
            })
            .collect();
        let g = fit(&xs, &ys, ps).unwrap();
        assert_eq!(g, f);
        assert_eq!(g.order_at(&Rational::from(4)), Some(-1));
        assert_eq!(g.small_roots().1, vec!["4".to_string()]);
    }

    #[test]
    fn reduce_applies_rules_recursively_and_exactly() {
        let (old, _) = tables();
        let mut t = Table::load(&old).unwrap();
        // I(3,1) = (d-3)/4 I(2,1) (rule at n0 = 3); I(1,2) = f I(2,1) + g I(1,1)
        let r = t
            .reduce_mod(&[vec![3, 1], vec![1, 2]], &PRIMES[..4], &check_dims(), true)
            .unwrap();
        let ex = r.exact_masters().unwrap();
        let d = rat_of_q(&check_dims()[0]);
        let want = &(&d - &Rational::from(3)) / &Rational::from(4);
        assert_eq!(ex[0][&vec![2, 1]][0], want);
        assert!(!ex[0].contains_key(&vec![1, 1]));
        let f = &(&d - &Rational::from(4)) / &(&d - &Rational::from(5));
        assert_eq!(ex[1][&vec![2, 1]][0], f);
        // an uncovered key is reported, not silently treated as a master
        let mut t2 = Table::load(&old).unwrap();
        t2.masters = vec![vec![1, 1]];
        let e = t2
            .reduce_mod(&[vec![1, 2]], &PRIMES[..2], &check_dims(), false)
            .err()
            .unwrap();
        assert!(e.contains("I(2,1)"), "{e}");
        assert_eq!(t2.uncovered(&[vec![1, 2]]).unwrap(), vec![vec![2, 1]]);
    }

    #[test]
    fn master_map_of_synthetic_tables() {
        let (old, new) = tables();
        let mut o = Table::load(&old).unwrap();
        let mut n = Table::load(&new).unwrap();
        let opt = MapOptions {
            laporta: None,
            sym_points: 24,
            sym_primes: 4,
            check_keys: Some(vec![vec![3, 1]]),
            ..MapOptions::default()
        };
        let m = build_map(&mut o, &mut n, &opt).unwrap();
        assert_eq!(m["mutually_inverse_exact"], true, "{m}");
        assert_eq!(m["symbolic"]["mutually_inverse_symbolic"], true, "{m}");
        let a = &m["symbolic"]["rows"]["new_in_old:I(1,2)"]["I(2,1)"];
        assert_eq!(a["order_at_d4"], 1, "{a}");
        assert_eq!(m["symbolic"]["regular_at_d4"], false);
        assert_eq!(m["few_check"]["all_equal"], true, "{m}");
        assert_eq!(m["accepted"], true);
        // a wrong inverse is caught: the new table's I(2,1) rule off by a factor
        let bad = std::fs::read_to_string(&new)
            .unwrap()
            .replace("\"(d-5)/(d-4)\"", "\"2*(d-5)/(d-4)\"");
        let badp = write("bad.jsonl", &[bad.trim_end().to_string()]);
        let mut b = Table::load(&badp).unwrap();
        let mut o2 = Table::load(&old).unwrap();
        let m2 = build_map(&mut o2, &mut b, &opt).unwrap();
        assert_eq!(m2["mutually_inverse_exact"], false);
        assert_eq!(m2["accepted"], false);
    }

    #[test]
    fn laporta_one_loop_bubble() {
        // massless bubble, q.q = 1: I(1,2) = I(2,1) = -(d-3) I(1,1) (Euclidean G-functions)
        let fam = Family::parse(FAM2).unwrap();
        let d = Q { n: 37, d: 7 };
        let p = PRIMES[0];
        let want = rat_modp(&(&Rational::from(3) - &rat_of_q(&d)), p)
            .unwrap()
            .to_string();
        for t in [vec![1, 2], vec![2, 1]] {
            let r = laporta(&fam, &[true, true], &t, &[vec![1, 1]], 1, 0, &d, p).unwrap();
            assert_eq!(r["status"], "ok", "{r}");
            assert_eq!(r["terms"]["I(1,1)"], want.as_str(), "{r}");
        }
    }
}
