//! Order predictor (order-study, 2026-11-11): choose ONE coordinate priority for a node from its own cases.
//!
//! **Depth-1 oracle.** For a case (fixed values, free axes, integer-linear equations) the IBP rows
//! `d/dk_i . v` (`v` = loop momenta and externals) are generated at the case point and its +-1 neighbours
//! (radius 1), evaluated mod p at a random point of the case (random residues on the free axes, equations solved,
//! random `d`). Columns are the keys of the case's sector ordered by the engine's legacy comparator
//! ([`crate::solver::IntegralOrder`]: total excess, numerator degree, then active axes in priority order with the larger
//! power simpler, then inactive axes with more numerator simpler). The case has a depth-1 rule iff an ELIGIBLE key
//! (offset only on free axes, `A delta = 0` for the equations) is a pivot column of the echelon form. Measured against
//! every native depth-1 probe recorded in the 5L campaign (repair order probes and rr_node probes): 499/499.
//!
//! **Rules used by the predictor** (notes/order-study.md):
//! * R2: an oversized rule is always the lift of a case WITHOUT a depth-1 rule under the generation order
//!   (8 shipped tables, 20,925 rules: 91/91 rules > 1M monomials, 0 of ~19,700 depth-1 cases);
//! * R1 (BFI): within the lines and within the ISPs, axes at their boundary value (line at 1, ISP at 0) before free axes
//!   before axes fixed at an interior value (dotted line, fixed numerator): the one-unit transfers of the natural
//!   relations (lower a free line / dot a boundary line; take numerator from a free ISP / put it on a zero ISP) are
//!   then simpler than the target.
//!
//! [`predict`] scores candidate priorities (the base order, the R1 vote of the hard cases weighted by their rule size,
//! the R1 order of the largest hard cases, lines/ISPs reversed) by the number of the node's hard cases (no depth-1 rule
//! under the base order) that still have none, plus the number of easy cases that lose their depth-1 rule, then improves
//! the best by moving single axes to the front/back of their group or swapping neighbours (greedy rounds).

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};

use serde::{Deserialize, Serialize};

const P: u64 = 2_147_483_647;

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Q(i128, i128);

impl Q {
    fn new(n: i128, d: i128) -> Q {
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Q(s * n / g, s * d / g)
    }
    fn i(n: i128) -> Q {
        Q(n, 1)
    }
    fn add(self, o: Q) -> Q {
        Q::new(self.0 * o.1 + o.0 * self.1, self.1 * o.1)
    }
    fn sub(self, o: Q) -> Q {
        Q::new(self.0 * o.1 - o.0 * self.1, self.1 * o.1)
    }
    fn mul(self, o: Q) -> Q {
        Q::new(self.0 * o.0, self.1 * o.1)
    }
    fn div(self, o: Q) -> Q {
        Q::new(self.0 * o.1, self.1 * o.0)
    }
    fn is0(self) -> bool {
        self.0 == 0
    }
    fn modp(self) -> u64 {
        let n = self.0.rem_euclid(P as i128) as u64;
        let d = self.1.rem_euclid(P as i128) as u64;
        n * powm(d, P - 2) % P
    }
}

fn powm(mut b: u64, mut e: u64) -> u64 {
    let mut r = 1u64;
    b %= P;
    while e > 0 {
        if e & 1 == 1 {
            r = r * b % P;
        }
        b = b * b % P;
        e >>= 1;
    }
    r
}

fn smod(x: i64) -> u64 {
    x.rem_euclid(P as i64) as u64
}

/// One IBP template term: offset, the axis whose power multiplies the coefficient (`None` = `d`), constant mod p.
type Term = (Vec<i32>, Option<usize>, u64);

/// The IBP templates of a massless family with one external momentum (`gram(q,q,1)`).
pub struct OracleFamily {
    pub n: usize,
    pub phys: Vec<bool>,
    templates: Vec<Vec<Term>>,
}

fn momentum(expr: &str, vecs: &[String]) -> Result<Vec<i128>, String> {
    let e: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let body = e
        .strip_prefix('(')
        .and_then(|x| x.strip_suffix(")^2"))
        .ok_or_else(|| format!("propagator {expr}: expected (..)^2"))?;
    let mut c = vec![0i128; vecs.len()];
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let mut sign = 1;
        if b[i] == b'+' {
            i += 1;
        } else if b[i] == b'-' {
            sign = -1;
            i += 1;
        }
        let mut coef = 0i128;
        let mut has = false;
        while i < b.len() && b[i].is_ascii_digit() {
            coef = coef * 10 + i128::from(b[i] - b'0');
            has = true;
            i += 1;
        }
        if i < b.len() && b[i] == b'*' {
            i += 1;
        }
        let st = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        let name = &body[st..i];
        let k = vecs
            .iter()
            .position(|v| v == name)
            .ok_or_else(|| format!("propagator {expr}: unknown vector {name}"))?;
        c[k] += sign * if has { coef } else { 1 };
        if i == st {
            return Err(format!("propagator {expr}: cannot parse"));
        }
    }
    Ok(c)
}

fn invert(m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, String> {
    let n = m.len();
    let mut a: Vec<Vec<Q>> = m
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut x = r.clone();
            x.extend((0..n).map(|j| Q::i(i128::from(i == j))));
            x
        })
        .collect();
    for c in 0..n {
        let p = (c..n)
            .find(|&r| !a[r][c].is0())
            .ok_or("propagators are not a complete scalar-product basis")?;
        a.swap(c, p);
        let piv = a[c][c];
        for x in a[c].iter_mut() {
            *x = x.div(piv);
        }
        for r in 0..n {
            if r != c && !a[r][c].is0() {
                let f = a[r][c];
                let rowc = a[c].clone();
                for (x, y) in a[r].iter_mut().zip(rowc) {
                    *x = x.sub(f.mul(y));
                }
            }
        }
    }
    Ok(a.into_iter().map(|r| r[n..].to_vec()).collect())
}

fn grab_list(text: &str, key: &str) -> Result<Vec<String>, String> {
    let s = text
        .find(&format!("{key}("))
        .ok_or(format!("family: no {key}(..)"))?
        + key.len()
        + 1;
    let e = s + text[s..].find(')').ok_or("family: unclosed list")?;
    Ok(text[s..e]
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect())
}

impl OracleFamily {
    /// Parse a compact family (`prop(LABEL,(momentum)^2,FLAG)` lines, `gram(q,q,1)`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let loops = grab_list(text, "loops")?;
        let ext = grab_list(text, "externals")?;
        let mut vecs = loops.clone();
        vecs.extend(ext.iter().cloned());
        let props = super::order::family_props(text);
        let n = props.len();
        let (l, e) = (loops.len(), ext.len());
        let mut sp = Vec::new();
        for i in 0..l + e {
            for j in i..l + e {
                if i < l {
                    sp.push((i, j));
                }
            }
        }
        if sp.len() != n {
            return Err(format!("{} scalar products vs {n} slots", sp.len()));
        }
        let spi: HashMap<(usize, usize), usize> =
            sp.iter().enumerate().map(|(k, &p)| (p, k)).collect();
        let mom: Vec<Vec<i128>> = props
            .iter()
            .map(|p| momentum(&p.expr, &vecs))
            .collect::<Result<_, _>>()?;
        let mut m = vec![vec![Q::i(0); n]; n];
        let mut cst = vec![Q::i(0); n];
        for (a, c) in mom.iter().enumerate() {
            for i in 0..l + e {
                for j in i..l + e {
                    let w = c[i] * c[j] * if i == j { 1 } else { 2 };
                    if w == 0 {
                        continue;
                    }
                    if i >= l {
                        cst[a] = cst[a].add(Q::i(w));
                    } else {
                        let k = spi[&(i, j)];
                        m[a][k] = m[a][k].add(Q::i(w));
                    }
                }
            }
        }
        let minv = invert(&m)?;
        let dot = |a: usize, v: usize| -> (Vec<Q>, Q) {
            let mut coeff = vec![Q::i(0); n];
            let mut beta = Q::i(0);
            for (j, &cj) in mom[a].iter().enumerate() {
                if cj == 0 {
                    continue;
                }
                let (i1, i2) = (j.min(v), j.max(v));
                if i1 >= l {
                    beta = beta.add(Q::i(cj));
                    continue;
                }
                let k = spi[&(i1, i2)];
                for b in 0..n {
                    if !minv[k][b].is0() {
                        coeff[b] = coeff[b].add(Q::i(cj).mul(minv[k][b]));
                        beta = beta.sub(Q::i(cj).mul(minv[k][b]).mul(cst[b]));
                    }
                }
            }
            (coeff, beta)
        };
        let mut templates = Vec::new();
        for i in 0..l {
            for v in 0..l + e {
                let mut terms: Vec<(Vec<i32>, Option<usize>, Q)> = Vec::new();
                if v == i {
                    terms.push((vec![0; n], None, Q::i(1)));
                }
                for a in 0..n {
                    let cai = mom[a][i];
                    if cai == 0 {
                        continue;
                    }
                    let (alpha, beta) = dot(a, v);
                    for (b, al) in alpha.iter().enumerate() {
                        if !al.is0() {
                            let mut s = vec![0; n];
                            s[a] += 1;
                            s[b] -= 1;
                            terms.push((s, Some(a), Q::i(-2 * cai).mul(*al)));
                        }
                    }
                    if !beta.is0() {
                        let mut s = vec![0; n];
                        s[a] += 1;
                        terms.push((s, Some(a), Q::i(-2 * cai).mul(beta)));
                    }
                }
                templates.push(
                    terms
                        .into_iter()
                        .map(|(s, w, q)| (s, w, q.modp()))
                        .collect(),
                );
            }
        }
        Ok(Self {
            n,
            phys: props.iter().map(|p| p.flag.trim() != "0").collect(),
            templates,
        })
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn val(&mut self) -> u64 {
        1000 + self.next() % (P - 2000)
    }
}

/// `c . n + c0 = 0` from an integer-linear equation in `n<i>`.
pub fn parse_equation(e: &str, n: usize) -> Result<(Vec<i64>, i64), String> {
    let s: String = e.chars().filter(|c| !c.is_whitespace()).collect();
    let s = if s.starts_with('+') || s.starts_with('-') {
        s
    } else {
        format!("+{s}")
    };
    let b = s.as_bytes();
    let mut c = vec![0i64; n];
    let mut c0 = 0i64;
    let mut i = 0;
    while i < b.len() {
        let sign = if b[i] == b'-' { -1 } else { 1 };
        i += 1;
        let mut num = 0i64;
        let mut has = false;
        while i < b.len() && b[i].is_ascii_digit() {
            num = num * 10 + i64::from(b[i] - b'0');
            has = true;
            i += 1;
        }
        if i < b.len() && b[i] == b'*' {
            i += 1;
        }
        if i < b.len() && b[i] == b'n' {
            i += 1;
            let mut k = 0usize;
            let st = i;
            while i < b.len() && b[i].is_ascii_digit() {
                k = k * 10 + usize::from(b[i] - b'0');
                i += 1;
            }
            if i == st || k >= n {
                return Err(format!("equation {e}: bad variable"));
            }
            c[k] += sign * if has { num } else { 1 };
        } else if has {
            c0 += sign * num;
        } else {
            return Err(format!("equation {e}: cannot parse"));
        }
    }
    Ok((c, c0))
}

/// A case prepared for the oracle (rows at radius 1 included).
pub struct OracleCase {
    fixed: Vec<Option<i64>>,
    eqs: Vec<(Vec<i64>, i64)>,
    sym: Vec<bool>,
    active: Vec<bool>,
    keys: Vec<Vec<i32>>,
    rows: Vec<Vec<(usize, u64)>>,
}

impl OracleCase {
    pub fn new(
        fam: &OracleFamily,
        fixed: &[Option<i64>],
        equations: &[String],
        seed: u64,
    ) -> Result<Self, String> {
        Self::new_centred(fam, fixed, equations, seed, None)
    }

    /// Like [`Self::new`], but the seeds are the radius-1 ball around the case point displaced by `centre`
    /// (`None` = no displacement). The displaced ball of an affine case is the depth-2 seed shell of
    /// [`super::affine`]: rows from one layer `l(n) = +-1` next to the face, where the face's own depth-1 relations
    /// are degenerate. Keys, sectors and eligible pivots stay relative to the case point.
    pub fn new_centred(
        fam: &OracleFamily,
        fixed: &[Option<i64>],
        equations: &[String],
        seed: u64,
        centre: Option<&[i32]>,
    ) -> Result<Self, String> {
        let n = fam.n;
        if fixed.len() != n {
            return Err(format!("case arity {} != {n}", fixed.len()));
        }
        let eqs: Vec<(Vec<i64>, i64)> = equations
            .iter()
            .map(|e| parse_equation(e, n))
            .collect::<Result<_, _>>()?;
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let sym: Vec<bool> = fixed.iter().map(|f| f.is_none()).collect();
        let mut dep = vec![false; n];
        for (c, _) in &eqs {
            if let Some(i) = (0..n).find(|&i| c[i] != 0 && sym[i] && !dep[i]) {
                dep[i] = true;
            }
        }
        let mut val: Vec<Option<u64>> = (0..n)
            .map(|i| match fixed[i] {
                Some(v) => Some(smod(v)),
                None if !dep[i] => Some(rng.val()),
                None => None,
            })
            .collect();
        for _ in 0..=eqs.len() {
            for (c, c0) in &eqs {
                let unk: Vec<usize> = (0..n).filter(|&i| c[i] != 0 && val[i].is_none()).collect();
                if unk.len() == 1 {
                    let i = unk[0];
                    let mut s = smod(*c0);
                    for j in 0..n {
                        if c[j] != 0 && j != i {
                            s = (s + smod(c[j]) * val[j].unwrap_or(0)) % P;
                        }
                    }
                    val[i] = Some((P - s) % P * powm(smod(c[i]), P - 2) % P);
                }
            }
        }
        let val: Vec<u64> = val
            .into_iter()
            .map(|v| v.ok_or("equations not solvable by substitution"))
            .collect::<Result<_, _>>()?;
        let active: Vec<bool> = (0..n)
            .map(|i| match fixed[i] {
                Some(v) => v > 0,
                None => fam.phys[i],
            })
            .collect();
        let d = rng.val();
        let mut case = Self {
            fixed: fixed.to_vec(),
            eqs,
            sym,
            active,
            keys: Vec::new(),
            rows: Vec::new(),
        };
        // seeds: the (displaced) case point and its admissible +-1 neighbours
        let c0: Vec<i32> = match centre {
            Some(c) if c.len() == n => c.to_vec(),
            Some(c) => return Err(format!("centre arity {} != {n}", c.len())),
            None => vec![0i32; n],
        };
        let mut seeds = Vec::new();
        if case.admissible_seed(fam, &c0) {
            seeds.push(c0.clone());
        }
        for i in 0..n {
            for dl in [1, -1] {
                let mut t = c0.clone();
                t[i] += dl;
                if !seeds.contains(&t) && case.admissible_seed(fam, &t) {
                    seeds.push(t);
                }
            }
        }
        let mut ids: HashMap<Vec<i32>, usize> = HashMap::new();
        for s in &seeds {
            let mval: Vec<u64> = (0..n)
                .map(|i| (val[i] + smod(i64::from(s[i]))) % P)
                .collect();
            for t in &fam.templates {
                let mut row: HashMap<usize, u64> = HashMap::new();
                for (shift, who, k) in t {
                    let w = match who {
                        None => d,
                        Some(a) => mval[*a],
                    };
                    let c = w * k % P;
                    if c == 0 {
                        continue;
                    }
                    let key: Vec<i32> = s.iter().zip(shift).map(|(a, b)| a + b).collect();
                    let id = match ids.get(&key) {
                        Some(&id) => id,
                        None => {
                            case.keys.push(key.clone());
                            ids.insert(key, case.keys.len() - 1);
                            case.keys.len() - 1
                        }
                    };
                    let e = row.entry(id).or_insert(0);
                    *e = (*e + c) % P;
                }
                let r: Vec<(usize, u64)> = row.into_iter().filter(|(_, v)| *v != 0).collect();
                if !r.is_empty() {
                    case.rows.push(r);
                }
            }
        }
        Ok(case)
    }

    /// Rows of the seed set (for diagnostics).
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    fn admissible_seed(&self, fam: &OracleFamily, s: &[i32]) -> bool {
        for (i, &o) in s.iter().enumerate() {
            if o == 0 || self.sym[i] {
                continue;
            }
            let v = self.fixed[i].unwrap_or(0) + i64::from(o);
            if (self.active[i] && v <= 0)
                || (!fam.phys[i] && v > 0)
                || (fam.phys[i] && !self.active[i] && v > 0)
            {
                return false;
            }
        }
        true
    }

    fn in_sector(&self, off: &[i32]) -> bool {
        off.iter().enumerate().all(|(i, &o)| {
            o == 0
                || self.sym[i]
                || (self.fixed[i].unwrap_or(0) + i64::from(o) > 0) == self.active[i]
        })
    }

    fn eligible(&self, off: &[i32]) -> bool {
        if off.iter().enumerate().any(|(i, &o)| o != 0 && !self.sym[i]) {
            return false;
        }
        self.eqs.iter().all(|(c, _)| {
            c.iter()
                .zip(off)
                .map(|(&a, &b)| a * i64::from(b))
                .sum::<i64>()
                == 0
        })
    }

    fn sort_key(&self, off: &[i32], prio: &[usize]) -> Vec<i64> {
        let (mut tot, mut num) = (0i64, 0i64);
        for (i, &o) in off.iter().enumerate() {
            let o = i64::from(o);
            if self.active[i] {
                tot += o;
            } else {
                tot -= o;
                num -= o;
            }
        }
        let mut k = vec![tot, num];
        k.extend(
            prio.iter()
                .filter(|&&i| self.active[i])
                .map(|&i| -i64::from(off[i])),
        );
        k.extend(
            prio.iter()
                .filter(|&&i| !self.active[i])
                .map(|&i| i64::from(off[i])),
        );
        k
    }

    /// Depth-1 rule of this case under the coordinate priority `prio` (original slot numbers).
    pub fn depth1(&self, prio: &[usize]) -> Depth1 {
        let sk: Vec<Option<Vec<i64>>> = self
            .keys
            .iter()
            .map(|k| self.in_sector(k).then(|| self.sort_key(k, prio)))
            .collect();
        let mut order: Vec<usize> = (0..self.keys.len()).filter(|&i| sk[i].is_some()).collect();
        order.sort_by(|&a, &b| sk[b].cmp(&sk[a]));
        let mut rank = vec![usize::MAX; self.keys.len()];
        for (r, &i) in order.iter().enumerate() {
            rank[i] = r;
        }
        let words = self.rows.len().div_ceil(64);
        let mut pivots: HashMap<usize, (Vec<(usize, u64)>, Vec<u64>)> = HashMap::new();
        for (idx, r) in self.rows.iter().enumerate() {
            let mut row: HashMap<usize, u64> = r.iter().copied().collect();
            let mut deps = vec![0u64; words];
            deps[idx / 64] |= 1 << (idx % 64);
            loop {
                let lead = row
                    .keys()
                    .filter(|&&k| rank[k] != usize::MAX)
                    .min_by_key(|&&k| rank[k])
                    .copied();
                let Some(lead) = lead else { break };
                if let Some((prow, pdeps)) = pivots.get(&lead) {
                    let lv = row[&lead];
                    let pv = prow.iter().find(|(k, _)| *k == lead).map_or(1, |x| x.1);
                    let f = lv * powm(pv, P - 2) % P;
                    for &(k, v) in prow {
                        let e = row.entry(k).or_insert(0);
                        *e = (*e + P - f * v % P) % P;
                        if *e == 0 {
                            row.remove(&k);
                        }
                    }
                    for (a, b) in deps.iter_mut().zip(pdeps) {
                        *a |= b;
                    }
                } else {
                    pivots.insert(lead, (row.into_iter().collect(), deps));
                    break;
                }
            }
        }
        let mut best: Option<(usize, usize, usize)> = None;
        for (&k, (row, deps)) in &pivots {
            if self.eligible(&self.keys[k]) {
                let tr: usize = deps.iter().map(|w| w.count_ones() as usize).sum();
                let c = (tr, row.len() - 1, rank[k]);
                if best.is_none_or(|b| c < b) {
                    best = Some(c);
                }
            }
        }
        match best {
            Some((trace, rhs, _)) => Depth1 {
                rule: true,
                trace,
                rhs,
            },
            None => Depth1 {
                rule: false,
                trace: 0,
                rhs: 0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Depth1 {
    pub rule: bool,
    pub trace: usize,
    pub rhs: usize,
}

/// A rule case of a node with its weight (record bytes of the shipped rule; 0 if unknown).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedCase {
    pub fixed: Vec<Option<i64>>,
    #[serde(default)]
    pub equations: Vec<String>,
    #[serde(default)]
    pub bytes: u64,
}

/// Rule cases of a table (JSONL, table_dump.v1) read with a bounded prefix per line (records can be GBs).
pub fn table_cases(path: &str) -> Result<(serde_json::Value, Vec<WeightedCase>), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut r = BufReader::with_capacity(1 << 24, f);
    let mut header = serde_json::Value::Null;
    let mut out = Vec::new();
    loop {
        // read up to 64 KB of the line, then skip to its end counting bytes
        let mut head = Vec::new();
        let mut size = 0u64;
        let mut ended = false;
        loop {
            let buf = r.fill_buf().map_err(|e| e.to_string())?;
            if buf.is_empty() {
                break;
            }
            let (chunk, nl) = match buf.iter().position(|&b| b == b'\n') {
                Some(p) => (p, true),
                None => (buf.len(), false),
            };
            if head.len() < 65536 {
                let take = chunk.min(65536 - head.len());
                head.extend_from_slice(&buf[..take]);
            }
            size += chunk as u64;
            r.consume(chunk + usize::from(nl));
            if nl {
                ended = true;
                break;
            }
        }
        if head.is_empty() && !ended {
            break;
        }
        let text = String::from_utf8_lossy(&head);
        if text.starts_with("{\"type\":\"header\"") {
            if size as usize == head.len() {
                header = serde_json::from_str(&text).map_err(|e| format!("{path} header: {e}"))?;
            }
            continue;
        }
        if !text.starts_with("{\"type\":\"rule\"") {
            continue;
        }
        let fixed = json_field(&text, "fixed");
        let equations = json_field(&text, "equations");
        if let Some(fixed) = fixed.and_then(|v| serde_json::from_value::<Vec<Option<i64>>>(v).ok())
        {
            let equations = equations
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or_default();
            out.push(WeightedCase {
                fixed,
                equations,
                bytes: size,
            });
        }
        if !ended {
            break;
        }
    }
    if header.is_null() {
        // a long header line (family source): read the first line whole
        let mut first = String::new();
        let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
        BufReader::new(f.take(64 << 20))
            .read_line(&mut first)
            .map_err(|e| e.to_string())?;
        header = serde_json::from_str(&first).unwrap_or(serde_json::Value::Null);
    }
    Ok((header, out))
}

/// `"key":[...]` of a JSON object prefix (array without nested brackets) parsed as JSON.
fn json_field(text: &str, key: &str) -> Option<serde_json::Value> {
    let pat = format!("\"{key}\":");
    let at = text.find(&pat)? + pat.len();
    let rest = text[at..].trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    let end = rest.find(']')?;
    serde_json::from_str(&rest[..=end]).ok()
}

/// Boundary / free / interior class of each axis in a case, and its activity (line in the sector or not).
fn classes(fixed: &[Option<i64>], phys: &[bool]) -> (Vec<bool>, Vec<u8>) {
    let act: Vec<bool> = fixed
        .iter()
        .zip(phys)
        .map(|(f, &p)| match f {
            Some(v) => *v > 0,
            None => p,
        })
        .collect();
    let cls = fixed
        .iter()
        .zip(&act)
        .map(|(f, &a)| match f {
            None => 1,
            Some(v) if (a && *v == 1) || (!a && *v == 0) => 0,
            Some(_) => 2,
        })
        .collect();
    (act, cls)
}

/// R1 vote: every case votes boundary < free < interior inside its activity groups with its weight; the axes of each
/// family group (lines, ISPs) are then sorted by net votes (base order breaks ties).
pub fn bfi_vote(cases: &[(&WeightedCase, f64)], phys: &[bool], base: &[usize]) -> Vec<usize> {
    let n = phys.len();
    let mut w = vec![vec![0f64; n]; n];
    for (c, wt) in cases {
        let (act, cls) = classes(&c.fixed, phys);
        for a in 0..n {
            for b in 0..n {
                if a != b && act[a] == act[b] && cls[a] < cls[b] {
                    w[a][b] += wt;
                }
            }
        }
    }
    let pos: HashMap<usize, usize> = base.iter().enumerate().map(|(k, &v)| (v, k)).collect();
    let mut out = Vec::new();
    for want in [true, false] {
        let g: Vec<usize> = base.iter().copied().filter(|&i| phys[i] == want).collect();
        let sc: HashMap<usize, f64> = g
            .iter()
            .map(|&x| (x, g.iter().map(|&y| w[x][y] - w[y][x]).sum()))
            .collect();
        let mut g2 = g.clone();
        g2.sort_by(|a, b| {
            sc[b]
                .partial_cmp(&sc[a])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(pos[a].cmp(&pos[b]))
        });
        out.extend(g2);
    }
    out
}

/// R1 order of one case.
pub fn bfi_case(c: &WeightedCase, phys: &[bool], base: &[usize]) -> Vec<usize> {
    let (_, cls) = classes(&c.fixed, phys);
    let mut lines: Vec<usize> = base.iter().copied().filter(|&i| phys[i]).collect();
    let mut isps: Vec<usize> = base.iter().copied().filter(|&i| !phys[i]).collect();
    let pos: HashMap<usize, usize> = base.iter().enumerate().map(|(k, &v)| (v, k)).collect();
    lines.sort_by_key(|&i| (cls[i], pos[&i]));
    isps.sort_by_key(|&i| (cls[i], pos[&i]));
    lines.extend(isps);
    lines
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictOptions {
    pub sample_easy: usize,
    pub rounds: usize,
    pub seed: u64,
    /// Score affine cases (cases with equations) without a depth-1 rule by their displaced-ball depth-2 relation
    /// ([`super::affine`]): a small one costs 5 % of the case weight instead of 100 %. Off = the depth-1 objective.
    #[serde(default)]
    pub affine_depth2: bool,
}

impl Default for PredictOptions {
    fn default() -> Self {
        Self {
            sample_easy: 300,
            rounds: 8,
            seed: 7,
            affine_depth2: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prediction {
    pub base: Vec<usize>,
    pub predicted: Vec<usize>,
    pub cases: usize,
    pub hard: usize,
    pub easy_sample: usize,
    /// (weighted misses, trace sum of the hard hits) per candidate
    pub candidates: Vec<(String, Vec<usize>, f64, usize)>,
    pub start: String,
    pub rounds: Vec<(usize, f64, f64)>,
    /// over ALL cases: [no depth-1 under base, under predicted]; hard misses; misses among cases with a record > 28 MB
    pub no_depth1: [usize; 2],
    pub no_depth1_hard: [usize; 2],
    pub no_depth1_big: [usize; 2],
    /// with `affine_depth2`: affine cases (of the scored set) WITHOUT a small depth-1 or displaced-ball relation,
    /// [base, predicted]
    #[serde(default)]
    pub affine_not_small: [usize; 2],
    pub order: super::order::NodeOrder,
}

/// Choose one coordinate priority for the family from its cases (see the module documentation).
pub fn predict(
    fam: &OracleFamily,
    cases: &[WeightedCase],
    base: &[usize],
    opts: &PredictOptions,
    log: &mut dyn FnMut(&str),
) -> Result<Prediction, String> {
    let prepared: Vec<OracleCase> = cases
        .iter()
        .enumerate()
        .map(|(i, c)| OracleCase::new(fam, &c.fixed, &c.equations, 1 + i as u64))
        .collect::<Result<_, _>>()?;
    let at_base: Vec<Depth1> = prepared.iter().map(|c| c.depth1(base)).collect();
    let hard: Vec<usize> = (0..cases.len()).filter(|&i| !at_base[i].rule).collect();
    let easy_all: Vec<usize> = (0..cases.len()).filter(|&i| at_base[i].rule).collect();
    let mut rng = Rng(opts.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut easy = easy_all.clone();
    for i in (1..easy.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        easy.swap(i, j);
    }
    easy.truncate(opts.sample_easy);
    easy.sort_unstable();
    let weight = |i: usize| 1.0 + cases[i].bytes as f64 / 1e6;
    let affine: HashMap<usize, super::affine::AffinePrepared> = if opts.affine_depth2 {
        hard.iter()
            .chain(easy.iter())
            .copied()
            .filter(|&i| !cases[i].equations.is_empty())
            .map(|i| {
                super::affine::AffinePrepared::new(
                    fam,
                    &cases[i].fixed,
                    &cases[i].equations,
                    1 + i as u64,
                )
                .map(|p| (i, p))
            })
            .collect::<Result<_, _>>()?
    } else {
        HashMap::new()
    };
    if opts.affine_depth2 {
        log(&format!(
            "affine depth-2 scoring on {} cases with equations",
            affine.len()
        ));
    }
    // cost of a case without a depth-1 rule: its weight, or 5 % of it for an affine case with a small displaced-ball
    // relation under `prio`
    let no_d1_cost = |i: usize, prio: &[usize], w: f64| -> f64 {
        match affine.get(&i).and_then(|p| p.best_shifted(prio)) {
            Some((_, d))
                if d.trace <= super::affine::SMALL_TRACE && d.rhs <= super::affine::SMALL_RHS =>
            {
                0.05 * w
            }
            _ => w,
        }
    };
    let score = |prio: &[usize]| -> (f64, usize) {
        let mut miss = 0.0;
        let mut tr = 0;
        for &i in &hard {
            let d = prepared[i].depth1(prio);
            if d.rule {
                tr += d.trace;
            } else {
                miss += no_d1_cost(i, prio, weight(i));
            }
        }
        for &i in &easy {
            if !prepared[i].depth1(prio).rule {
                miss += no_d1_cost(i, prio, 1.0);
            }
        }
        (miss, tr)
    };
    let better = |a: (f64, usize), b: (f64, usize)| {
        a.0 < b.0 - 1e-9 || ((a.0 - b.0).abs() <= 1e-9 && a.1 < b.1)
    };
    let phys = &fam.phys;
    let hw: Vec<(&WeightedCase, f64)> = hard.iter().map(|&i| (&cases[i], weight(i))).collect();
    let mut cands: Vec<(String, Vec<usize>)> = vec![
        ("base".into(), base.to_vec()),
        ("bfi_vote".into(), bfi_vote(&hw, phys, base)),
        (
            "lines_rev".into(),
            base.iter()
                .rev()
                .copied()
                .filter(|&i| phys[i])
                .chain(base.iter().copied().filter(|&i| !phys[i]))
                .collect(),
        ),
        (
            "isps_rev".into(),
            base.iter()
                .copied()
                .filter(|&i| phys[i])
                .chain(base.iter().rev().copied().filter(|&i| !phys[i]))
                .collect(),
        ),
    ];
    let mut by_size = hard.clone();
    by_size.sort_by(|&a, &b| cases[b].bytes.cmp(&cases[a].bytes));
    for (k, &i) in by_size.iter().take(3).enumerate() {
        cands.push((format!("bfi_case{k}"), bfi_case(&cases[i], phys, base)));
    }
    let mut scored = Vec::new();
    let mut best: Option<(String, Vec<usize>, (f64, usize))> = None;
    for (name, p) in &cands {
        let s = score(p);
        log(&format!(
            "candidate {name}: misses {:.2}, hard trace {}",
            s.0, s.1
        ));
        scored.push((name.clone(), p.clone(), s.0, s.1));
        if best.as_ref().is_none_or(|b| better(s, b.2)) {
            best = Some((name.clone(), p.clone(), s));
        }
    }
    let (start, mut cur, mut cur_s) = best.ok_or("no candidate")?;
    let mut rounds = Vec::new();
    for r in 0..opts.rounds {
        let mut seen: HashSet<Vec<usize>> = HashSet::new();
        let mut round_best: Option<(Vec<usize>, (f64, usize))> = None;
        let mut neighbours: Vec<Vec<usize>> = Vec::new();
        for want in [true, false] {
            let g: Vec<usize> = cur.iter().copied().filter(|&i| phys[i] == want).collect();
            if g.len() < 2 {
                continue;
            }
            // move one axis to the front / the back of its group
            for &x in &g {
                let p: Vec<usize> = cur.iter().copied().filter(|&v| v != x).collect();
                let first = if g[0] != x { g[0] } else { g[1] };
                let last = if g[g.len() - 1] != x {
                    g[g.len() - 1]
                } else {
                    g[g.len() - 2]
                };
                let lo = p.iter().position(|&v| v == first).unwrap_or(0);
                let hi = p.iter().position(|&v| v == last).unwrap_or(0) + 1;
                for at in [lo, hi] {
                    let mut q = p.clone();
                    q.insert(at, x);
                    neighbours.push(q);
                }
            }
            // swap neighbours inside the group
            for w in g.windows(2) {
                let mut q = cur.clone();
                let (a, b) = (
                    q.iter().position(|&v| v == w[0]).unwrap_or(0),
                    q.iter().position(|&v| v == w[1]).unwrap_or(0),
                );
                q.swap(a, b);
                neighbours.push(q);
            }
        }
        for q in neighbours {
            if q != cur && seen.insert(q.clone()) {
                let s = score(&q);
                if round_best.as_ref().is_none_or(|b| better(s, b.1)) {
                    round_best = Some((q, s));
                }
            }
        }
        let Some((q, s)) = round_best else { break };
        log(&format!(
            "round {r}: {} moves, best misses {:.2} (current {:.2})",
            seen.len(),
            s.0,
            cur_s.0
        ));
        rounds.push((seen.len(), s.0, cur_s.0));
        if better(s, cur_s) {
            cur = q;
            cur_s = s;
        } else {
            break;
        }
    }
    let at_pred: Vec<Depth1> = prepared.iter().map(|c| c.depth1(&cur)).collect();
    let count = |v: &[Depth1], filt: &dyn Fn(usize) -> bool| {
        (0..v.len()).filter(|&i| !v[i].rule && filt(i)).count()
    };
    let hard_set: HashSet<usize> = hard.iter().copied().collect();
    let big = |i: usize| cases[i].bytes > 28_000_000;
    let not_small = |prio: &[usize]| {
        affine
            .iter()
            .filter(|(i, p)| {
                !prepared[**i].depth1(prio).rule
                    && !p.best_shifted(prio).is_some_and(|(_, d)| {
                        d.trace <= super::affine::SMALL_TRACE && d.rhs <= super::affine::SMALL_RHS
                    })
            })
            .count()
    };
    let affine_not_small = [not_small(base), not_small(&cur)];
    Ok(Prediction {
        base: base.to_vec(),
        predicted: cur.clone(),
        cases: cases.len(),
        hard: hard.len(),
        easy_sample: easy.len(),
        candidates: scored,
        start,
        rounds,
        no_depth1: [count(&at_base, &|_| true), count(&at_pred, &|_| true)],
        no_depth1_hard: [
            count(&at_base, &|i| hard_set.contains(&i)),
            count(&at_pred, &|i| hard_set.contains(&i)),
        ],
        no_depth1_big: [count(&at_base, &big), count(&at_pred, &big)],
        affine_not_small,
        order: super::order::NodeOrder {
            name: "predicted".into(),
            slot_permutation: None,
            coordinate_priority: Some(cur),
            source_order: super::order::SourceOrder::Input,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// two-loop sunrise-like family with q (5 slots: 3 lines, 2 ISPs)
    const FAM: &str = "I(\n  name(T),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    #[test]
    fn family_parses_and_templates_have_the_d_term() {
        let f = OracleFamily::parse(FAM).unwrap();
        assert_eq!(f.n, 5);
        assert_eq!(f.phys, vec![true, true, true, false, false]);
        // 2 loops x 3 vectors
        assert_eq!(f.templates.len(), 6);
        assert!(f.templates[0].iter().any(|t| t.1.is_none()));
    }

    #[test]
    fn equation_parser() {
        assert_eq!(
            parse_equation("-1-n9-2*n8+n0", 10).unwrap(),
            (vec![1, 0, 0, 0, 0, 0, 0, 0, -2, -1], -1)
        );
        assert!(parse_equation("n12", 5).is_err());
    }

    #[test]
    fn top_case_has_a_depth1_rule_and_classes() {
        let f = OracleFamily::parse(FAM).unwrap();
        let c = OracleCase::new(&f, &[None, None, None, None, None], &[], 1).unwrap();
        assert!(c.depth1(&[0, 1, 2, 3, 4]).rule);
        let (act, cls) = classes(&[Some(1), None, Some(2), Some(0), Some(-1)], &f.phys);
        assert_eq!(act, vec![true, true, true, false, false]);
        assert_eq!(cls, vec![0, 1, 2, 0, 2]);
        let wc = WeightedCase {
            fixed: vec![None, Some(1), Some(2), None, Some(0)],
            equations: vec![],
            bytes: 0,
        };
        assert_eq!(
            bfi_case(&wc, &f.phys, &[0, 1, 2, 3, 4]),
            vec![1, 0, 2, 4, 3]
        );
    }
}
