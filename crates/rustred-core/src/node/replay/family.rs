//! Independent IBP generator of the replay (port of p5l-replay.v1 `checker/gibp.py`): reads only the family text,
//! builds the IBP operators exactly (small rationals), evaluates rows mod p, and decides scaleless sectors by Lee's
//! criterion on U + F computed from the propagators. No RustRed solver or Symbolica code is used.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use super::arith::{self, Q};
use super::expr::Unknown;

/// One IBP operator: `diag` (j == v) and per propagator `(t0, [(m, x)])`.
type OpTable = (bool, Vec<(Q, Vec<(usize, Q)>)>);

pub struct Family {
    pub n: usize,
    pub loops: usize,
    pub kshift: Vec<i64>,
    pub eps_axes: Vec<usize>,
    /// Momentum coefficients `c[i][v]`, `v` over loops then `q`.
    c: Vec<Vec<i64>>,
    ops: Vec<OpTable>,
    zero_cache: RefCell<HashMap<Vec<bool>, bool>>,
}

fn lin(mom: &str) -> Result<BTreeMap<String, i64>, Unknown> {
    // ([+-]?)(\d*)\*?([kq]\d*)
    let s: Vec<char> = mom.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out: BTreeMap<String, i64> = BTreeMap::new();
    let mut i = 0;
    while i < s.len() {
        let mut sign = 1;
        if s[i] == '+' || s[i] == '-' {
            if s[i] == '-' {
                sign = -1;
            }
            i += 1;
        }
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let coef: i64 = if i > start { s[start..i].iter().collect::<String>().parse().map_err(|_| "momentum coefficient")? } else { 1 };
        if i < s.len() && s[i] == '*' {
            i += 1;
        }
        if i >= s.len() || !(s[i] == 'k' || s[i] == 'q') {
            return Err(format!("unsupported momentum {mom}"));
        }
        let vstart = i;
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let v: String = s[vstart..i].iter().collect();
        *out.entry(v).or_insert(0) += sign * coef;
    }
    Ok(out)
}

fn inverse(m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
    let n = m.len();
    let mut a: Vec<Vec<Q>> = m.iter().enumerate().map(|(i, row)| row.iter().copied().chain((0..n).map(|j| if i == j { Q::ONE } else { Q::ZERO })).collect()).collect();
    for c in 0..n {
        let p = (c..n).find(|&r| !a[r][c].is_zero()).ok_or("singular scalar-product matrix")?;
        a.swap(c, p);
        let f = a[c][c];
        a[c] = a[c].iter().map(|x| x.div(f)).collect::<Result<_, _>>()?;
        for r in 0..n {
            if r != c && !a[r][c].is_zero() {
                let g = a[r][c];
                let pivot = a[c].clone();
                a[r] = a[r].iter().zip(&pivot).map(|(x, y)| x.sub(g.mul(*y)?)).collect::<Result<_, _>>()?;
            }
        }
    }
    Ok(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// Text inside the first balanced `name(` ... `)` occurrence.
fn call_args(src: &str, name: &str) -> Option<String> {
    let start = src.find(&format!("{name}("))? + name.len() + 1;
    let mut depth = 1;
    for (k, ch) in src[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(src[start..start + k].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

impl Family {
    pub fn parse(src: &str) -> Result<Self, Unknown> {
        let compact: String = src.chars().filter(|c| !c.is_whitespace()).collect();
        let loops: Vec<String> = call_args(&compact, "loops").ok_or("family without loops(...)")?.split(',').map(str::to_string).collect();
        let mut props: Vec<(String, String)> = Vec::new();
        let mut rest = compact.as_str();
        // prop\(D\d+,\(([^)]*)\)\^2,(\d)\)
        while let Some(at) = rest.find("prop(D") {
            let s = &rest[at + 6..];
            let digits = s.chars().take_while(char::is_ascii_digit).count();
            let s2 = &s[digits..];
            let parsed = (|| {
                let s3 = s2.strip_prefix(",(")?;
                let close = s3.find(')')?;
                let mom = &s3[..close];
                let s4 = s3[close..].strip_prefix(")^2,")?;
                let flag = s4.chars().next().filter(char::is_ascii_digit)?;
                s4[1..].starts_with(')').then(|| (mom.to_string(), flag.to_string()))
            })();
            if let (true, Some(p)) = (digits > 0, parsed) {
                props.push(p);
            }
            rest = &rest[at + 6..];
        }
        let n = props.len();
        let mut kshift = vec![0i64; n];
        let mut r = compact.as_str();
        while let Some(at) = r.find("power_shift(D") {
            let s = &r[at + 13..];
            let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
            let j: usize = digits.parse().map_err(|_| "power_shift label")?;
            let expr = call_args(&r[at..], "power_shift").unwrap_or_default();
            let value = expr.split_once(',').map(|x| x.1.to_string()).unwrap_or_default();
            if value != "0" && !value.is_empty() {
                // (4-d)/2 multiples: only the plain one-sided families are supported by this port
                return Err("power shifts (eps axes) are not supported by the replay port".into());
            }
            if j >= 1 && j <= n {
                kshift[j - 1] = 0;
            }
            r = &r[at + 13..];
        }
        let lcount = loops.len();
        let vs: Vec<String> = loops.iter().cloned().chain(["q".to_string()]).collect();
        let c: Vec<Vec<i64>> = props.iter().map(|(m, _)| lin(m).map(|l| vs.iter().map(|v| l.get(v).copied().unwrap_or(0)).collect())).collect::<Result<_, _>>()?;
        let pairs: Vec<(usize, usize)> = (0..=lcount).flat_map(|a| (a..=lcount).map(move |b| (a, b))).filter(|&(a, b)| !(a == lcount && b == lcount)).collect();
        if pairs.len() != n {
            return Err(format!("{} scalar products for {n} propagators: incomplete basis", pairs.len()));
        }
        let idx = |a: usize, b: usize| pairs.iter().position(|&p| p == (a.min(b), a.max(b)));
        let mut m = vec![vec![Q::ZERO; n]; n];
        let mut c0 = vec![Q::ZERO; n];
        for i in 0..n {
            for a in 0..=lcount {
                for b in a..=lcount {
                    let co = c[i][a] * c[i][b] * if a == b { 1 } else { 2 };
                    if co == 0 {
                        continue;
                    }
                    if a == lcount && b == lcount {
                        c0[i] = c0[i].add(Q::int(co as i128))?;
                    } else {
                        let k = idx(a, b).unwrap();
                        m[i][k] = m[i][k].add(Q::int(co as i128))?;
                    }
                }
            }
        }
        let mi = inverse(&m)?;
        let sp = |a: usize, b: usize| if a == lcount && b == lcount { None } else { idx(a, b) };
        let mut ops = Vec::new();
        for j in 0..lcount {
            for v in 0..=lcount {
                let mut tab = Vec::with_capacity(n);
                for i in 0..n {
                    let mut t0 = Q::ZERO;
                    let mut tm = vec![Q::ZERO; n];
                    let cij = c[i][j];
                    if cij != 0 {
                        for a in 0..=lcount {
                            let cia = c[i][a];
                            if cia == 0 {
                                continue;
                            }
                            let f = Q::int(2 * cij as i128 * cia as i128);
                            match sp(a, v) {
                                None => t0 = t0.add(f)?,
                                Some(s) => {
                                    for mm in 0..n {
                                        let x = mi[s][mm];
                                        if !x.is_zero() {
                                            tm[mm] = tm[mm].add(f.mul(x)?)?;
                                            t0 = t0.sub(f.mul(x)?.mul(c0[mm])?)?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tab.push((t0, tm.into_iter().enumerate().filter(|(_, x)| !x.is_zero()).collect()));
                }
                ops.push((j == v, tab));
            }
        }
        let eps_axes = (0..n).filter(|&i| kshift[i] != 0).collect();
        Ok(Self { n, loops: lcount, kshift, eps_axes, c, ops, zero_cache: RefCell::new(HashMap::new()) })
    }

    /// The L(L+1) IBP rows at base exponents `a` (integers), shift 0, dimension `d` (field element), mod p.
    /// Each row maps an offset (relative to `a`) to its coefficient.
    pub fn rows_at(&self, a: &[i64], d: u64, p: u64) -> Result<Vec<BTreeMap<Vec<i64>, u64>>, Unknown> {
        let n = self.n;
        let ai: Vec<u64> = (0..n).map(|i| arith::from_i128(p, a[i] as i128)).collect();
        let mut out = Vec::with_capacity(self.ops.len());
        for (diag, tab) in &self.ops {
            let mut r: BTreeMap<Vec<i64>, u64> = BTreeMap::new();
            if *diag {
                r.insert(vec![0; n], d % p);
            }
            for i in 0..n {
                let (t0, tm) = &tab[i];
                if t0.is_zero() && tm.is_empty() {
                    continue;
                }
                let x = ai[i];
                if x == 0 {
                    continue;
                }
                let mut up = vec![0i64; n];
                up[i] += 1;
                if !t0.is_zero() {
                    let v = arith::mul(p, x, t0.modp(p).ok_or("operator table pole")?);
                    let e = r.entry(up.clone()).or_insert(0);
                    *e = arith::sub(p, *e, v);
                }
                for (m, y) in tm {
                    up[*m] -= 1;
                    let v = arith::mul(p, x, y.modp(p).ok_or("operator table pole")?);
                    let e = r.entry(up.clone()).or_insert(0);
                    *e = arith::sub(p, *e, v);
                    up[*m] += 1;
                }
            }
            r.retain(|_, v| *v != 0);
            out.push(r);
        }
        Ok(out)
    }

    /// Exact operator row `op`: offset -> linear form `[const, y_0.., y_{n-1}, d]` (a_i = y_i).
    pub fn exact_row(&self, op: usize) -> Result<BTreeMap<Vec<i64>, Vec<Q>>, Unknown> {
        let n = self.n;
        let (diag, tab) = &self.ops[op];
        let mut r: BTreeMap<Vec<i64>, Vec<Q>> = BTreeMap::new();
        let mut bump = |k: Vec<i64>, var: usize, s: Q| -> Result<(), Unknown> {
            let e = r.entry(k.clone()).or_insert_with(|| vec![Q::ZERO; n + 2]);
            e[var] = e[var].add(s)?;
            if e.iter().all(Q::is_zero) {
                r.remove(&k);
            }
            Ok(())
        };
        if *diag {
            bump(vec![0; n], n + 1, Q::ONE)?;
        }
        for i in 0..n {
            let (t0, tm) = &tab[i];
            if t0.is_zero() && tm.is_empty() {
                continue;
            }
            let mut up = vec![0i64; n];
            up[i] = 1;
            if !t0.is_zero() {
                bump(up.clone(), i + 1, t0.neg())?;
            }
            for (m, x) in tm {
                up[*m] -= 1;
                bump(up.clone(), i + 1, x.neg())?;
                up[*m] += 1;
            }
        }
        Ok(r)
    }

    /// Scaleless sector (Lee's criterion; eps axes always on).
    pub fn zero_mask(&self, mask: &[bool]) -> bool {
        let mut m = mask.to_vec();
        for &i in &self.eps_axes {
            m[i] = true;
        }
        if let Some(z) = self.zero_cache.borrow().get(&m) {
            return *z;
        }
        let z = self.zero(&m);
        self.zero_cache.borrow_mut().insert(m, z);
        z
    }

    pub fn zero_key(&self, key: &[i64]) -> bool {
        self.zero_mask(&key.iter().map(|&x| x > 0).collect::<Vec<_>>())
    }

    fn zero(&self, m: &[bool]) -> bool {
        let axes: Vec<usize> = (0..self.n).filter(|&i| m[i]).collect();
        let t = axes.len();
        if t == 0 {
            return true;
        }
        let (u, f) = self.graph_polys(&axes);
        let mut vecs: Vec<Vec<u8>> = u.keys().chain(f.keys()).cloned().collect();
        vecs.sort();
        vecs.dedup();
        if vecs.is_empty() {
            return true;
        }
        let mut piv: Vec<(usize, Vec<Q>)> = Vec::new();
        for k in vecs {
            let mut r: Vec<Q> = k.iter().map(|&e| Q::int(e as i128)).chain([Q::ONE]).collect();
            for (c, pr) in &piv {
                if !r[*c].is_zero() {
                    let f = r[*c];
                    r = r.iter().zip(pr).map(|(a, b)| a.sub(f.mul(*b).unwrap()).unwrap()).collect();
                }
            }
            let Some(c) = (0..=t).find(|&j| !r[j].is_zero()) else { continue };
            if c == t {
                return false;
            }
            let f = r[c];
            r = r.iter().map(|a| a.div(f).unwrap()).collect();
            piv.push((c, r));
        }
        true
    }

    /// U and F (bordered determinant, q^2 = 1) of the lines `axes`, as exponent vectors -> coefficients.
    fn graph_polys(&self, axes: &[usize]) -> (HashMap<Vec<u8>, i128>, HashMap<Vec<u8>, i128>) {
        let l = self.loops;
        let t = axes.len();
        type Poly = HashMap<Vec<u8>, i128>;
        let mut mm: Vec<Vec<Poly>> = vec![vec![Poly::new(); l + 1]; l + 1];
        for (pos, &ax) in axes.iter().enumerate() {
            let c = &self.c[ax];
            let mut key = vec![0u8; t];
            key[pos] = 1;
            for a in 0..=l {
                for b in 0..=l {
                    let co = c[a] * c[b];
                    if co != 0 {
                        let e = mm[a][b].entry(key.clone()).or_insert(0);
                        *e += co as i128;
                        if *e == 0 {
                            mm[a][b].remove(&key);
                        }
                    }
                }
            }
        }
        fn det(mm: &[Vec<Poly>], rows: &[usize], cols: &[usize], memo: &mut HashMap<(Vec<usize>, Vec<usize>), Poly>, t: usize) -> Poly {
            if rows.is_empty() {
                let mut one = Poly::new();
                one.insert(vec![0u8; t], 1);
                return one;
            }
            let key = (rows.to_vec(), cols.to_vec());
            if let Some(r) = memo.get(&key) {
                return r.clone();
            }
            let r0 = rows[0];
            let mut out = Poly::new();
            for (j, &c) in cols.iter().enumerate() {
                let e = &mm[r0][c];
                if e.is_empty() {
                    continue;
                }
                let sub_cols: Vec<usize> = cols.iter().enumerate().filter(|(k, _)| *k != j).map(|(_, &x)| x).collect();
                let sub = det(mm, &rows[1..], &sub_cols, memo, t);
                if sub.is_empty() {
                    continue;
                }
                let sign: i128 = if j % 2 == 0 { 1 } else { -1 };
                for (k1, v1) in e {
                    for (k2, v2) in &sub {
                        let k: Vec<u8> = k1.iter().zip(k2).map(|(a, b)| a + b).collect();
                        let x = out.entry(k.clone()).or_insert(0);
                        *x += sign * v1 * v2;
                        if *x == 0 {
                            out.remove(&k);
                        }
                    }
                }
            }
            memo.insert(key, out.clone());
            out
        }
        let idx: Vec<usize> = (0..l).collect();
        let u = det(&mm, &idx, &idx, &mut HashMap::new(), t);
        let idx2: Vec<usize> = (0..=l).collect();
        let f = det(&mm, &idx2, &idx2, &mut HashMap::new(), t);
        (u, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUNSET: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2-q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    #[test]
    fn sunset_family_ibp_and_zero_sectors() {
        let f = Family::parse(SUNSET).unwrap();
        assert_eq!((f.n, f.loops), (5, 2));
        // vacuum-like supports are scaleless; the sunset (D1 D2 D3) is not; D1 D3 leaves a k2 tadpole
        assert!(f.zero_mask(&[true, true, false, false, false]));
        assert!(!f.zero_mask(&[true, true, true, false, false]));
        assert!(f.zero_mask(&[true, false, true, false, false]));
        assert!(f.zero_mask(&[false, false, false, false, false]));
        // L(L+1) operators; the diagonal ones contain d at offset 0
        let rows = f.rows_at(&[1, 1, 1, 0, 0], 7, arith::P).unwrap();
        assert_eq!(rows.len(), 6);
        // exact rows agree with the modular rows at y = a, d = 7
        for op in 0..6 {
            let ex = f.exact_row(op).unwrap();
            let md = &rows[op];
            for (k, form) in &ex {
                let a = [1i128, 1, 1, 0, 0];
                let mut v = form[0].modp(arith::P).unwrap();
                for i in 0..5 {
                    v = arith::add(arith::P, v, arith::mul(arith::P, form[i + 1].modp(arith::P).unwrap(), arith::from_i128(arith::P, a[i])));
                }
                v = arith::add(arith::P, v, arith::mul(arith::P, form[6].modp(arith::P).unwrap(), 7));
                assert_eq!(md.get(k).copied().unwrap_or(0), v, "op {op} key {k:?}");
            }
        }
    }
}
