//! Masters modulo symmetry of one node's top sector (sym-masters, 2026-11-12; design:
//! handoff/sym-investigation-for-ben/results/DESIGN-masters-mod-symmetry.md of the p-integral workspace).
//!
//! * [`automorphisms`]: the symmetry group G of the top sector from the family's momenta: every integer substitution
//!   of the loop momenta (with `q -> +-q`, `|det| = 1`) that maps each line momentum to `+-` a line momentum. Each
//!   element is stored by its action on ALL slots: a line goes to a line, an ISP square goes to a rational
//!   combination of the slots and `q^2` (= 1). Elements are deduplicated by that full action (`T` and `-T` coincide).
//! * [`run`]: per sample `(d, p)`, RustRed's IBP rows ([`Family::rows_at`]) at the seeds of a box (lines >= 1, at most
//!   `D` dots, ISP rank <= `R`) modulo subsectors, eliminated in two column orders:
//!   - pass T ("rank first": `(tot, rank, dots, key)`, dotted keys stay masters): `V` = free inner columns after the
//!     IBP rows, traces `tr(g|V)` from the images of `V`'s basis, `dim V^G` = average trace; then the exact symmetry
//!     rows `I(a) - I(g a)` at ISP-free seeds; the free inner columns are a basis of `V_G`;
//!   - pass M (preferred keys and the given masters lowest): the same rows; the free inner columns after the
//!     symmetry rows are the reported `V_G` basis and every master is reduced onto it.
//! * exact coefficients per `d` by CRT + rational reconstruction ([`super::mastermap::reconstruct`], leave one
//!   prime out).
//!
//! Why exact rows at ISP-free seeds suffice (REPORT 3.1, claim C4): every element acts on an ISP-free key as a
//! permutation of the lines; if ISP-free keys span `V`, `span{v - g v} = span{u - g u : u ISP-free}`. The certificate
//! checks the consequence `dim V^G` (trace) `= |V_G basis|` at every sample. Counts are upper bounds unless IBP is
//! saturated in the box.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde_json::{Value, json};

use super::mastermap::{Key, key_text, reconstruct};
use super::replay::arith::{self, Q};
use super::replay::family::Family;

type R<T> = Result<T, String>;

// ---------------------------------------------------------------------------------------------------------------
// exact linear algebra over Q (small matrices)

fn qv(x: i64) -> Q {
    Q::int(x as i128)
}

/// Inverse of a square matrix over Q.
fn inverse(m: &[Vec<Q>]) -> R<Vec<Vec<Q>>> {
    let n = m.len();
    let mut a: Vec<Vec<Q>> = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .copied()
                .chain((0..n).map(|j| if i == j { Q::ONE } else { Q::ZERO }))
                .collect()
        })
        .collect();
    for c in 0..n {
        let p = (c..n).find(|&r| !a[r][c].is_zero()).ok_or("singular matrix")?;
        a.swap(c, p);
        let f = a[c][c];
        a[c] = a[c].iter().map(|x| x.div(f)).collect::<R<_>>()?;
        for r in 0..n {
            if r != c && !a[r][c].is_zero() {
                let g = a[r][c];
                let pivot = a[c].clone();
                a[r] = a[r]
                    .iter()
                    .zip(&pivot)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<R<_>>()?;
            }
        }
    }
    Ok(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

fn rank(rows: &[Vec<Q>]) -> R<usize> {
    let mut a: Vec<Vec<Q>> = rows.to_vec();
    let m = a.first().map_or(0, Vec::len);
    let mut r = 0;
    for c in 0..m {
        let Some(p) = (r..a.len()).find(|&i| !a[i][c].is_zero()) else {
            continue;
        };
        a.swap(r, p);
        let f = a[r][c];
        for i in r + 1..a.len() {
            if !a[i][c].is_zero() {
                let g = a[i][c].div(f)?;
                let pr = a[r].clone();
                a[i] = a[i]
                    .iter()
                    .zip(&pr)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<R<_>>()?;
            }
        }
        r += 1;
    }
    Ok(r)
}

fn det(m: &[Vec<Q>]) -> R<Q> {
    let n = m.len();
    let mut a = m.to_vec();
    let mut d = Q::ONE;
    for c in 0..n {
        let Some(p) = (c..n).find(|&r| !a[r][c].is_zero()) else {
            return Ok(Q::ZERO);
        };
        if p != c {
            a.swap(c, p);
            d = d.neg();
        }
        let f = a[c][c];
        d = d.mul(f)?;
        for r in c + 1..n {
            if !a[r][c].is_zero() {
                let g = a[r][c].div(f)?;
                let pr = a[c].clone();
                a[r] = a[r]
                    .iter()
                    .zip(&pr)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<R<_>>()?;
            }
        }
    }
    Ok(d)
}

/// `v M` (row vector times matrix).
fn vecmat(v: &[Q], m: &[Vec<Q>]) -> R<Vec<Q>> {
    let cols = m.first().map_or(0, Vec::len);
    let mut out = vec![Q::ZERO; cols];
    for (i, x) in v.iter().enumerate() {
        if x.is_zero() {
            continue;
        }
        for (j, o) in out.iter_mut().enumerate() {
            if !m[i][j].is_zero() {
                *o = o.add(x.mul(m[i][j])?)?;
            }
        }
    }
    Ok(out)
}

/// Coefficients of the quadratic form `(w.x)^2` over the monomials `x_a x_b`, `a <= b` (row-major).
fn quad(w: &[Q]) -> R<Vec<Q>> {
    let n = w.len();
    let mut out = Vec::with_capacity(n * (n + 1) / 2);
    for a in 0..n {
        for b in a..n {
            let x = w[a].mul(w[b])?;
            out.push(if a == b { x } else { x.mul(Q::int(2))? });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// group

/// One element of the top-sector symmetry group, by its action on the slots.
#[derive(Clone, Debug)]
pub struct Element {
    /// Image of `D_s` for every slot `s`: coefficients over `[D_0, .., D_{n-1}, q^2]` (`q^2 = 1`).
    pub images: Vec<Vec<Q>>,
    /// `Some(t)` when `D_s -> D_t` exactly (every line; ISPs where the image is one slot).
    pub exact: Vec<Option<usize>>,
}

impl Element {
    /// Line permutation (slot -> slot on lines, `None` on ISPs).
    pub fn line_perm(&self, mask: &[bool]) -> Vec<Option<usize>> {
        (0..mask.len())
            .map(|s| if mask[s] { self.exact[s] } else { None })
            .collect()
    }

    /// Exact image `g a` of a key when every slot with a nonzero exponent maps exactly, else `None`.
    pub fn exact_image(&self, a: &[i64]) -> Option<Key> {
        let mut b = vec![0i64; a.len()];
        for (s, &x) in a.iter().enumerate() {
            if x != 0 {
                b[self.exact[s]?] = x;
            }
        }
        Some(b)
    }

    /// Full image of a key `a` (lines >= 1, ISPs <= 0) as a combination of keys (exact rationals); terms with a
    /// line exponent <= 0 (subsector) are kept, the caller drops them.
    pub fn full_image(&self, a: &[i64], mask: &[bool]) -> R<BTreeMap<Key, Q>> {
        let n = a.len();
        let mut base = vec![0i64; n];
        let mut factors: Vec<(usize, i64)> = vec![];
        for s in 0..n {
            if a[s] == 0 {
                continue;
            }
            match self.exact[s] {
                Some(t) => base[t] += a[s],
                None => {
                    if mask[s] || a[s] > 0 {
                        return Err("non-exact image of a denominator".into());
                    }
                    factors.push((s, -a[s]));
                }
            }
        }
        let mut cur: BTreeMap<Key, Q> = BTreeMap::new();
        cur.insert(base, Q::ONE);
        for (s, pw) in factors {
            for _ in 0..pw {
                let mut nx: BTreeMap<Key, Q> = BTreeMap::new();
                for (k, c) in &cur {
                    for (t, y) in self.images[s].iter().enumerate() {
                        if y.is_zero() {
                            continue;
                        }
                        let mut k2 = k.clone();
                        if t < n {
                            k2[t] -= 1; // one more power of D_t in the numerator
                        }
                        let e = nx.entry(k2).or_insert(Q::ZERO);
                        *e = e.add(c.mul(*y)?)?;
                    }
                }
                nx.retain(|_, v| !v.is_zero());
                cur = nx;
            }
        }
        Ok(cur)
    }
}

/// The top-sector symmetry group.
#[derive(Clone, Debug)]
pub struct Group {
    pub n: usize,
    pub mask: Vec<bool>,
    /// Distinct full actions, identity first.
    pub elements: Vec<Element>,
    /// Indices into `elements` of one representative per distinct line permutation (identity first).
    pub line_reps: Vec<usize>,
    /// Closure under composition checked.
    pub closed: bool,
}

impl Group {
    pub fn order_full(&self) -> usize {
        self.elements.len()
    }
    pub fn order_lines(&self) -> usize {
        self.line_reps.len()
    }
    pub fn to_json(&self) -> Value {
        let el: Vec<Value> = self
            .elements
            .iter()
            .map(|e| {
                let lp: Vec<Value> = e
                    .line_perm(&self.mask)
                    .iter()
                    .map(|x| x.map_or(Value::Null, |t| json!(t)))
                    .collect();
                let isp_exact: Vec<Value> = (0..self.n)
                    .filter(|&s| !self.mask[s])
                    .map(|s| json!([s, e.exact[s]]))
                    .collect();
                json!({"lines": lp, "isp_exact": isp_exact})
            })
            .collect();
        json!({"order_full": self.order_full(), "order_lines": self.order_lines(), "closed": self.closed, "elements": el})
    }
}

/// All automorphisms of the top sector `mask` of `fam` (see the module header).
pub fn automorphisms(fam: &Family, mask: &[bool]) -> R<Group> {
    let n = fam.n;
    let lp = fam.loops;
    let nv = lp + 1;
    let mom = fam.momenta();
    let lines: Vec<usize> = (0..n).filter(|&s| mask[s]).collect();
    let vq: Vec<Vec<Q>> = mom.iter().map(|v| v.iter().map(|&x| qv(x)).collect()).collect();
    let eq: Vec<Q> = (0..nv).map(|i| if i == lp { Q::ONE } else { Q::ZERO }).collect();
    // basis: L lines independent together with q
    let mut basis: Vec<usize> = vec![];
    for &s in &lines {
        let mut rows: Vec<Vec<Q>> = basis.iter().map(|&b| vq[b].clone()).collect();
        rows.push(vq[s].clone());
        rows.push(eq.clone());
        if rank(&rows)? == basis.len() + 2 {
            basis.push(s);
        }
        if basis.len() == lp {
            break;
        }
    }
    if basis.len() < lp {
        return Err("lines and q do not span the momenta: not a proper top sector".into());
    }
    let mut bm: Vec<Vec<Q>> = basis.iter().map(|&b| vq[b].clone()).collect();
    bm.push(eq.clone());
    let bmi = inverse(&bm)?;
    let det_b = det(&bm)?;
    // coefficients of every slot momentum over the basis (L lines, q)
    let coeffs: Vec<Vec<Q>> = vq.iter().map(|v| vecmat(v, &bmi)).collect::<R<_>>()?;
    let last: Vec<Option<usize>> = (0..n)
        .map(|s| (0..lp).rev().find(|&j| !coeffs[s][j].is_zero()))
        .collect();
    // line lookup by momentum up to sign
    let mut lookup: HashMap<Vec<Q>, usize> = HashMap::new();
    for &s in &lines {
        lookup.insert(vq[s].clone(), s);
        lookup.insert(vq[s].iter().map(|x| x.neg()).collect(), s);
    }
    // invariant: number of 3-term relations v_i = +-v_x +-v_y among lines, and v_i = +-v_x +- q
    let inv: HashMap<usize, (usize, usize)> = lines
        .iter()
        .map(|&i| {
            let (mut a, mut b) = (0, 0);
            let others: Vec<usize> = lines.iter().copied().filter(|&j| j != i).collect();
            for (xi, &x) in others.iter().enumerate() {
                for &y in &others[xi + 1..] {
                    for sx in [1i64, -1] {
                        for sy in [1i64, -1] {
                            if (0..nv).all(|t| mom[i][t] == sx * mom[x][t] + sy * mom[y][t]) {
                                a += 1;
                            }
                        }
                    }
                }
                for sx in [1i64, -1] {
                    for sq in [1i64, -1] {
                        if (0..nv).all(|t| {
                            mom[i][t] == sx * mom[x][t] + if t == lp { sq } else { 0 }
                        }) {
                            b += 1;
                        }
                    }
                }
            }
            (i, (a, b))
        })
        .collect();
    // slot quadratic forms + q^2: basis of the quadratic forms
    let mut qm: Vec<Vec<Q>> = vq.iter().map(|v| quad(v)).collect::<R<_>>()?;
    qm.push(quad(&eq)?);
    if qm.len() != qm[0].len() {
        return Err("slots + q^2 do not match the number of quadratic monomials".into());
    }
    let qmi = inverse(&qm)?;

    struct Search<'a> {
        lp: usize,
        nv: usize,
        lines: &'a [usize],
        basis: &'a [usize],
        coeffs: &'a [Vec<Q>],
        last: &'a [Option<usize>],
        lookup: &'a HashMap<Vec<Q>, usize>,
        vq: &'a [Vec<Q>],
        inv: &'a HashMap<usize, (usize, usize)>,
        found: Vec<Vec<Vec<Q>>>,
    }
    fn image_of(s: &Search<'_>, c: &[Q], img: &[Vec<Q>], sq: i64) -> R<Vec<Q>> {
        let mut w = vec![Q::ZERO; s.nv];
        for j in 0..s.lp {
            if c[j].is_zero() {
                continue;
            }
            for t in 0..s.nv {
                if !img[j][t].is_zero() {
                    w[t] = w[t].add(c[j].mul(img[j][t])?)?;
                }
            }
        }
        if !c[s.lp].is_zero() {
            w[s.lp] = w[s.lp].add(c[s.lp].mul(qv(sq))?)?;
        }
        Ok(w)
    }
    fn rec(
        s: &mut Search<'_>,
        j: usize,
        img: &mut Vec<Vec<Q>>,
        perm: &mut HashMap<usize, usize>,
        sq: i64,
    ) -> R<()> {
        if j == s.lp {
            if perm.len() != s.lines.len() {
                return Ok(());
            }
            let mut b = img.clone();
            let mut e = vec![Q::ZERO; s.nv];
            e[s.lp] = qv(sq);
            b.push(e);
            s.found.push(b);
            return Ok(());
        }
        let src = s.basis[j];
        let cands: Vec<usize> = s.lines.to_vec();
        for t in cands {
            if s.inv[&t] != s.inv[&src] || perm.values().any(|&x| x == t) {
                continue;
            }
            for sg in [1i64, -1] {
                img.push(s.vq[t].iter().map(|x| x.mul(qv(sg))).collect::<R<_>>()?);
                // determined lines: last == j
                let mut ok = true;
                let mut added: Vec<usize> = vec![];
                for &i in s.lines {
                    if s.last[i] != Some(j) {
                        continue;
                    }
                    let w = image_of(s, &s.coeffs[i], img, sq)?;
                    match s.lookup.get(&w) {
                        Some(&tt)
                            if s.inv[&tt] == s.inv[&i]
                                && !perm.values().any(|&x| x == tt)
                                && !perm.contains_key(&i) =>
                        {
                            perm.insert(i, tt);
                            added.push(i);
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    rec(s, j + 1, img, perm, sq)?;
                }
                for i in added {
                    perm.remove(&i);
                }
                img.pop();
            }
        }
        Ok(())
    }
    let mut srch = Search {
        lp,
        nv,
        lines: &lines,
        basis: &basis,
        coeffs: &coeffs,
        last: &last,
        lookup: &lookup,
        vq: &vq,
        inv: &inv,
        found: vec![],
    };
    for sq in [1i64, -1] {
        let mut img = vec![];
        let mut perm = HashMap::new();
        rec(&mut srch, 0, &mut img, &mut perm, sq)?;
    }
    // full actions, |det| = 1, dedup
    let mut seen: HashSet<Vec<Vec<Q>>> = HashSet::new();
    let mut elements: Vec<Element> = vec![];
    for bp in &srch.found {
        let d = det(bp)?.div(det_b)?;
        if !(d == Q::ONE || d == Q::ONE.neg()) {
            continue;
        }
        let mut images = Vec::with_capacity(n);
        for s in 0..n {
            let w = vecmat(&coeffs[s], bp)?;
            images.push(vecmat(&quad(&w)?, &qmi)?);
        }
        if !seen.insert(images.clone()) {
            continue;
        }
        let exact = images
            .iter()
            .map(|im| {
                let nz: Vec<usize> = (0..=n).filter(|&t| !im[t].is_zero()).collect();
                (nz.len() == 1 && nz[0] < n && im[nz[0]] == Q::ONE).then(|| nz[0])
            })
            .collect();
        elements.push(Element { images, exact });
    }
    // identity first
    let id = elements
        .iter()
        .position(|e| (0..n).all(|s| e.exact[s] == Some(s)))
        .ok_or("identity not found")?;
    elements.swap(0, id);
    // lines must map exactly
    for e in &elements {
        if lines.iter().any(|&s| e.exact[s].is_none()) {
            return Err("a line maps to a non-line".into());
        }
    }
    let mut lseen: HashSet<Vec<Option<usize>>> = HashSet::new();
    let mut line_reps = vec![];
    for (k, e) in elements.iter().enumerate() {
        if lseen.insert(e.line_perm(mask)) {
            line_reps.push(k);
        }
    }
    // closure: matrices R_g (rows = images, last row q^2 -> q^2)
    let mat = |e: &Element| -> Vec<Vec<Q>> {
        let mut m = e.images.clone();
        m.push((0..=n).map(|t| if t == n { Q::ONE } else { Q::ZERO }).collect());
        m
    };
    let keys: HashSet<Vec<Vec<Q>>> = elements.iter().map(|e| e.images.clone()).collect();
    let mut closed = true;
    'c: for a in &elements {
        let ma = mat(a);
        for b in &elements {
            let mb = mat(b);
            let mut prod = Vec::with_capacity(n);
            for row in ma.iter().take(n) {
                prod.push(vecmat(row, &mb)?);
            }
            if !keys.contains(&prod) {
                closed = false;
                break 'c;
            }
        }
    }
    Ok(Group {
        n,
        mask: mask.to_vec(),
        elements,
        line_reps,
        closed,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// sparse elimination mod p

struct Elim {
    p: u64,
    piv: HashMap<usize, Vec<(usize, u64)>>,
}

impl Elim {
    fn new(p: u64) -> Self {
        Elim {
            p,
            piv: HashMap::new(),
        }
    }

    /// Insert a row (column, value), any order, duplicates summed; true if it gave a new pivot.
    fn insert(&mut self, row: &[(usize, u64)]) -> R<bool> {
        let p = self.p;
        let mut r: Vec<(usize, u64)> = row.to_vec();
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
            if let Some(pr) = self.piv.get(&c0) {
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
                self.piv
                    .insert(c0, r.iter().map(|&(c, v)| (c, arith::mul(p, v, iv))).collect());
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Fully reduce a combination onto the free (non-pivot) columns.
    fn reduce(&self, v: &[(usize, u64)]) -> BTreeMap<usize, u64> {
        let p = self.p;
        let mut work: BTreeMap<usize, u64> = BTreeMap::new();
        for &(c, x) in v {
            let e = work.entry(c).or_insert(0);
            *e = arith::add(p, *e, x);
        }
        let mut out: BTreeMap<usize, u64> = BTreeMap::new();
        while let Some((&c, &x)) = work.iter().next() {
            work.remove(&c);
            if x == 0 {
                continue;
            }
            if let Some(pc) = self.piv.get(&c) {
                for &(c2, v2) in &pc[1..] {
                    let e = work.entry(c2).or_insert(0);
                    *e = arith::sub(p, *e, arith::mul(p, x, v2));
                }
            } else {
                let e = out.entry(c).or_insert(0);
                *e = arith::add(p, *e, x);
            }
        }
        out.retain(|_, x| *x != 0);
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the computation

/// Options of [`run`].
#[derive(Clone, Debug)]
pub struct Options {
    pub dots: i64,
    pub rank: i64,
    /// Inner keys: dots + rank <= inner (plus the masters and preferred keys).
    pub inner: i64,
    pub dims: Vec<Q>,
    pub primes: Vec<u64>,
    /// Masters to map (e.g. a table's residuals), in sector.
    pub masters: Vec<Key>,
    /// Keys to keep in the V_G basis if possible.
    pub prefer: Vec<Key>,
    /// Also add exact symmetry rows at seeds with ISPs where the element is exact on every nonzero ISP.
    pub isp_exact_seeds: bool,
    pub threads: usize,
}

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

/// (dots, rank) of an in-sector key.
fn dr(mask: &[bool], k: &[i64]) -> (i64, i64) {
    let mut d = 0;
    let mut r = 0;
    for (s, &x) in k.iter().enumerate() {
        if mask[s] {
            d += x - 1;
        } else {
            r += -x;
        }
    }
    (d, r)
}

fn in_sector(mask: &[bool], k: &[i64]) -> bool {
    k.iter()
        .enumerate()
        .all(|(s, &x)| if mask[s] { x >= 1 } else { x <= 0 })
}

/// Seeds of the box: lines >= 1 with at most `dots` dots, ISP rank <= `rank`.
pub fn seeds(mask: &[bool], dots: i64, rank: i64) -> Vec<Key> {
    let n = mask.len();
    let lines: Vec<usize> = (0..n).filter(|&i| mask[i]).collect();
    let isps: Vec<usize> = (0..n).filter(|&i| !mask[i]).collect();
    let mut out = vec![];
    for dv in comps(lines.len(), dots) {
        for rv in comps(isps.len(), rank) {
            let mut k = vec![0i64; n];
            for (j, &l) in lines.iter().enumerate() {
                k[l] = 1 + dv[j];
            }
            for (j, &i) in isps.iter().enumerate() {
                k[i] = -rv[j];
            }
            out.push(k);
        }
    }
    out
}

/// Result of one sample.
#[derive(Clone, Debug)]
pub struct Sample {
    pub p: u64,
    pub json: Value,
    /// V basis of pass T (keys), traces per element (mod p), dim V^G (None if not an integer <= |V|).
    pub v_t: Vec<Key>,
    pub traces: Vec<u64>,
    pub dim_inv: Option<u64>,
    pub vg_t: Vec<Key>,
    pub v_m: Vec<Key>,
    pub vg_m: Vec<Key>,
    /// master -> (V_G basis key -> coefficient mod p)
    pub map: Vec<BTreeMap<Key, u64>>,
    pub leaks: usize,
    pub trace_incomplete: bool,
}

fn small_int(x: u64, bound: u64) -> Option<u64> {
    (x <= bound).then_some(x)
}

#[allow(clippy::too_many_arguments)]
fn one_sample(
    fam: &Family,
    g: &Group,
    opt: &Options,
    seeds: &[Key],
    sym_pairs: &[(Key, Key)],
    inner_keys: &[Key],
    d: Q,
    p: u64,
) -> R<Sample> {
    let mask = &g.mask;
    let t0 = std::time::Instant::now();
    let dmod = d.modp(p).ok_or("d pole")?;
    let mut rows: Vec<Vec<(Key, u64)>> = vec![];
    let mut cols: HashSet<Key> = inner_keys.iter().cloned().collect();
    for sk in seeds {
        for r in fam.rows_at(sk, dmod, p)? {
            let mut row = vec![];
            for (off, c) in r {
                let k: Key = sk.iter().zip(&off).map(|(a, b)| a + b).collect();
                if (0..k.len()).any(|s| mask[s] && k[s] <= 0) {
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
    for (a, b) in sym_pairs {
        cols.insert(a.clone());
        cols.insert(b.clone());
    }
    let inner: HashSet<Key> = inner_keys.iter().cloned().collect();
    // inner keys that occur in no IBP row are unconstrained, not masters: the box is too small for them
    let in_rows: HashSet<&Key> = rows.iter().flat_map(|r| r.iter().map(|x| &x.0)).collect();
    let unconstrained = inner_keys.iter().filter(|k| !in_rows.contains(k)).count();
    drop(in_rows);
    let colv_all: Vec<Key> = cols.into_iter().collect();
    let n_ibp = rows.len();

    // ---- pass T
    let wt = |k: &Key| -> (i64, i64, i64, i64, Key) {
        let (dd, rr) = dr(mask, k);
        (1, dd + rr, rr, dd, k.clone())
    };
    let mut colt = colv_all.clone();
    colt.sort_by_key(|k| std::cmp::Reverse(wt(k)));
    let cid_t: HashMap<Key, usize> = colt.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
    let mut et = Elim::new(p);
    for row in &rows {
        let r: Vec<(usize, u64)> = row.iter().map(|(k, c)| (cid_t[k], *c)).collect();
        et.insert(&r)?;
    }
    let free_inner = |e: &Elim, cid: &HashMap<Key, usize>, colv: &[Key]| -> Vec<Key> {
        let mut v: Vec<usize> = inner
            .iter()
            .map(|k| cid[k])
            .filter(|c| !e.piv.contains_key(c))
            .collect();
        v.sort_unstable();
        v.into_iter().map(|c| colv[c].clone()).collect()
    };
    let v_t = free_inner(&et, &cid_t, &colt);
    let mut leaks = unconstrained;
    // leak check: every inner key reduces onto inner free columns
    for k in inner_keys {
        let red = et.reduce(&[(cid_t[k], 1)]);
        if red.keys().any(|c| !inner.contains(&colt[*c])) {
            leaks += 1;
        }
    }
    // traces
    let mut traces = vec![];
    let mut trace_incomplete = false;
    let vset: HashMap<usize, usize> = v_t
        .iter()
        .enumerate()
        .map(|(i, k)| (cid_t[k], i))
        .collect();
    for e in &g.elements {
        let mut tr = 0u64;
        for b in &v_t {
            let img: Vec<(usize, u64)> = match e.exact_image(b) {
                Some(k) => match cid_t.get(&k) {
                    Some(&c) => vec![(c, 1)],
                    None => {
                        trace_incomplete = true;
                        continue;
                    }
                },
                None => {
                    let mut v = vec![];
                    for (k, c) in e.full_image(b, mask)? {
                        if (0..k.len()).any(|s| mask[s] && k[s] <= 0) {
                            continue;
                        }
                        match cid_t.get(&k) {
                            Some(&cc) => v.push((cc, c.modp(p).ok_or("image pole")?)),
                            None => trace_incomplete = true,
                        }
                    }
                    v
                }
            };
            let red = et.reduce(&img);
            if red.keys().any(|c| !vset.contains_key(c)) {
                leaks += 1;
            }
            if let Some(x) = red.get(&cid_t[b]) {
                tr = arith::add(p, tr, *x);
            }
        }
        traces.push(tr);
    }
    let sum = traces.iter().fold(0u64, |a, &x| arith::add(p, a, x));
    let ginv = arith::inv(p, g.order_full() as u64).ok_or("|G| = 0 mod p")?;
    let dim_inv = small_int(arith::mul(p, sum, ginv), v_t.len() as u64);
    let symrow = |cid: &HashMap<Key, usize>, a: &Key, b: &Key| vec![(cid[a], 1u64), (cid[b], p - 1)];
    for (a, b) in sym_pairs {
        et.insert(&symrow(&cid_t, a, b))?;
    }
    let vg_t = free_inner(&et, &cid_t, &colt);
    let t_pass_t = t0.elapsed().as_secs_f64();

    // ---- pass M
    let low: Vec<Key> = opt
        .prefer
        .iter()
        .chain(opt.masters.iter())
        .cloned()
        .fold(vec![], |mut acc, k| {
            if !acc.contains(&k) {
                acc.push(k);
            }
            acc
        });
    let wm = |k: &Key| -> (i64, i64, i64, i64, Key) {
        if let Some(pos) = low.iter().position(|m| m == k) {
            return (0, -(pos as i64), 0, 0, k.clone());
        }
        let (dd, rr) = dr(mask, k);
        (1, dd + rr, rr, dd, k.clone())
    };
    let mut colm = colv_all;
    colm.sort_by_key(|k| std::cmp::Reverse(wm(k)));
    let cid_m: HashMap<Key, usize> = colm.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
    let mut em = Elim::new(p);
    for row in &rows {
        let r: Vec<(usize, u64)> = row.iter().map(|(k, c)| (cid_m[k], *c)).collect();
        em.insert(&r)?;
    }
    let v_m = free_inner(&em, &cid_m, &colm);
    for (a, b) in sym_pairs {
        em.insert(&symrow(&cid_m, a, b))?;
    }
    let vg_m = free_inner(&em, &cid_m, &colm);
    let vgs: HashSet<usize> = vg_m.iter().map(|k| cid_m[k]).collect();
    let mut map = vec![];
    for m in &opt.masters {
        let red = em.reduce(&[(cid_m[m], 1)]);
        if red.keys().any(|c| !vgs.contains(c)) {
            leaks += 1;
        }
        map.push(red.into_iter().map(|(c, x)| (colm[c].clone(), x)).collect::<BTreeMap<_, _>>());
    }
    let kt = |v: &[Key]| v.iter().map(|k| key_text(k)).collect::<Vec<_>>();
    let json = json!({
        "d": format!("{}/{}", d.n, d.d), "p": p.to_string(), "seeds": seeds.len(), "rows_ibp": n_ibp,
        "rows_sym": sym_pairs.len(), "cols": colt.len(), "pivots_ibp_T": et.piv.len(),
        "V_T": kt(&v_t), "traces": traces.iter().map(|x| if *x > p / 2 { format!("-{}", p - x) } else { x.to_string() }).collect::<Vec<_>>(),
        "dim_VG_trace": dim_inv, "VG_T": kt(&vg_t), "V_M": kt(&v_m), "VG_M": kt(&vg_m),
        "leaks": leaks, "unconstrained_inner": unconstrained, "trace_incomplete": trace_incomplete,
        "seconds_pass_T": t_pass_t, "seconds": t0.elapsed().as_secs_f64(),
    });
    Ok(Sample {
        p,
        json,
        v_t,
        traces,
        dim_inv,
        vg_t,
        v_m,
        vg_m,
        map,
        leaks,
        trace_incomplete,
    })
}

/// Masters modulo symmetry of the top sector `mask` of the family text `src` (see the module header). Returns the report and
/// whether the certificate holds.
pub fn run(src: &str, mask: &[bool], opt: &Options) -> R<(Value, bool)> {
    let t0 = std::time::Instant::now();
    let fam = &Family::parse(src)?;
    let n = fam.n;
    if mask.len() != n {
        return Err("mask length != number of slots".into());
    }
    for k in opt.masters.iter().chain(&opt.prefer) {
        if k.len() != n || !in_sector(mask, k) {
            return Err(format!("key {} not in the top sector", key_text(k)));
        }
    }
    let g = automorphisms(fam, mask)?;
    let sd = seeds(mask, opt.dots, opt.rank);
    // inner keys: all in-sector keys with dots + rank <= inner, plus masters and preferred keys
    let mut inner: BTreeSet<Key> = BTreeSet::new();
    for k in seeds(mask, opt.inner, opt.inner) {
        let (dd, rr) = dr(mask, &k);
        if dd + rr <= opt.inner {
            inner.insert(k);
        }
    }
    inner.extend(opt.masters.iter().cloned());
    inner.extend(opt.prefer.iter().cloned());
    let inner_keys: Vec<Key> = inner.into_iter().collect();
    // symmetry rows: ISP-free seeds (and optionally seeds with exactly mapped ISPs), one per distinct image
    let mut pairs: BTreeSet<(Key, Key)> = BTreeSet::new();
    for a in &sd {
        let isp_free = (0..n).all(|s| mask[s] || a[s] == 0);
        if !isp_free && !opt.isp_exact_seeds {
            continue;
        }
        for &ei in &g.line_reps[1..] {
            let reps: Vec<&Element> = if isp_free {
                vec![&g.elements[ei]]
            } else {
                // every full element with this line permutation that is exact on a
                g.elements
                    .iter()
                    .filter(|e| e.line_perm(mask) == g.elements[ei].line_perm(mask))
                    .collect()
            };
            for e in reps {
                if let Some(b) = e.exact_image(a) {
                    if &b != a {
                        let pr = if *a < b { (a.clone(), b) } else { (b, a.clone()) };
                        pairs.insert(pr);
                    }
                }
            }
        }
    }
    let sym_pairs: Vec<(Key, Key)> = pairs.into_iter().collect();
    let jobs: Vec<(Q, u64)> = opt
        .dims
        .iter()
        .flat_map(|d| opt.primes.iter().map(move |p| (*d, *p)))
        .collect();
    let threads = opt.threads.max(1).min(jobs.len().max(1));
    let mut results: Vec<Option<R<Sample>>> = (0..jobs.len()).map(|_| None).collect();
    std::thread::scope(|sc| {
        let chunks: Vec<Vec<usize>> = (0..threads)
            .map(|t| (0..jobs.len()).filter(|i| i % threads == t).collect())
            .collect();
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|idx| {
                let (g, sd, sp, ik, jobs) = (&g, &sd, &sym_pairs, &inner_keys, &jobs);
                sc.spawn(move || {
                    // Family keeps a RefCell cache (not Sync): one parse per thread
                    let fam = match Family::parse(src) {
                        Ok(f) => f,
                        Err(e) => return idx.into_iter().map(|i| (i, Err(e.clone()))).collect::<Vec<_>>(),
                    };
                    let fam = &fam;
                    idx.into_iter()
                        .map(|i| (i, one_sample(fam, g, opt, sd, sp, ik, jobs[i].0, jobs[i].1)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in hs {
            for (i, r) in h.join().expect("sample thread") {
                results[i] = Some(r);
            }
        }
    });
    let samples: Vec<Sample> = results
        .into_iter()
        .map(|r| r.expect("sample"))
        .collect::<R<_>>()?;
    // certificate
    let s0 = &samples[0];
    let same = samples.iter().all(|s| {
        s.v_t == s0.v_t && s.vg_t == s0.vg_t && s.v_m == s0.v_m && s.vg_m == s0.vg_m
    });
    let no_leak = samples.iter().all(|s| s.leaks == 0);
    let trace_ok = samples
        .iter()
        .all(|s| !s.trace_incomplete && s.dim_inv == Some(s.vg_t.len() as u64) && s.vg_t.len() == s.vg_m.len());
    let dims_ok = samples.iter().all(|s| s.v_t.len() == s.v_m.len());
    let masters_basis = !opt.masters.is_empty()
        && samples.iter().all(|s| {
            let a: BTreeSet<&Key> = s.v_m.iter().collect();
            let b: BTreeSet<&Key> = opt.masters.iter().collect();
            a == b
        });
    // exact maps per d
    let np = opt.primes.len();
    let mut exact = vec![];
    let mut recon_ok = true;
    for (di, d) in opt.dims.iter().enumerate() {
        let ss: Vec<&Sample> = (0..np).map(|pi| &samples[di * np + pi]).collect();
        let mut per = serde_json::Map::new();
        for (mi, m) in opt.masters.iter().enumerate() {
            let mut terms = serde_json::Map::new();
            let ks: BTreeSet<&Key> = ss.iter().flat_map(|s| s.map[mi].keys()).collect();
            for k in ks {
                let rs: Vec<u64> = ss.iter().map(|s| s.map[mi].get(k).copied().unwrap_or(0)).collect();
                match reconstruct(&rs, &opt.primes) {
                    Some(r) => {
                        terms.insert(key_text(k), json!(r.to_string()));
                    }
                    None => {
                        recon_ok = false;
                        terms.insert(key_text(k), Value::Null);
                    }
                }
            }
            per.insert(key_text(m), Value::Object(terms));
        }
        exact.push(json!({"d": format!("{}/{}", d.n, d.d), "masters": per}));
    }
    let traces_int: Vec<Value> = s0
        .traces
        .iter()
        .map(|&x| {
            if x > s0.p / 2 {
                json!(-((s0.p - x) as i64))
            } else {
                json!(x as i64)
            }
        })
        .collect();
    let ok = g.closed && same && no_leak && trace_ok && dims_ok && recon_ok;
    let kt = |v: &[Key]| v.iter().map(|k| key_text(k)).collect::<Vec<_>>();
    let doc = json!({
        "tool": "rustred rr_node symmasters",
        "n": n, "mask": mask.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>(),
        "group": g.to_json(),
        "box": [opt.dots, opt.rank], "inner": opt.inner, "inner_keys": inner_keys.len(),
        "isp_exact_seeds": opt.isp_exact_seeds, "sym_rows": sym_pairs.len(),
        "dim_V": s0.v_t.len(), "V_rank_first": kt(&s0.v_t), "V_masters_order": kt(&s0.v_m),
        "traces": traces_int, "dim_VG_trace": s0.dim_inv, "dim_VG": s0.vg_m.len(),
        "VG_basis": kt(&s0.vg_m), "VG_basis_rank_first": kt(&s0.vg_t),
        "masters": kt(&opt.masters), "masters_are_V_basis": masters_basis,
        "exact": exact,
        "certificate": {"group_closed": g.closed, "same_at_every_sample": same, "no_leak": no_leak,
            "trace_equals_count": trace_ok, "dim_V_both_orders": dims_ok, "reconstruction": recon_ok, "ok": ok},
        "samples": samples.iter().map(|s| s.json.clone()).collect::<Vec<_>>(),
        "seconds": t0.elapsed().as_secs_f64(),
    });
    Ok((doc, ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUBBLE: &str = "I(\n  name(B),\n  loops(k1),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k1-q)^2,1),\n  gram(q,q,1)\n)\n";
    const SUNSET: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2-q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";
    const KITE: &str = "I(\n  name(K),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-q)^2,1),\n  prop(D4,(k2-q)^2,1),\n  prop(D5,(k1-k2)^2,1),\n  gram(q,q,1)\n)\n";

    fn opts(masters: Vec<Key>, dots: i64, rank: i64) -> Options {
        Options {
            dots,
            rank,
            inner: 1,
            dims: super::super::mastermap::check_dims(),
            primes: super::super::mastermap::PRIMES[..3].to_vec(),
            masters,
            prefer: vec![],
            isp_exact_seeds: false,
            threads: 2,
        }
    }

    #[test]
    fn groups_of_small_graphs() {
        let b = Family::parse(BUBBLE).unwrap();
        let g = automorphisms(&b, &[true, true]).unwrap();
        assert_eq!((g.order_lines(), g.order_full()), (2, 2));
        assert!(g.closed);
        let s = Family::parse(SUNSET).unwrap();
        let g = automorphisms(&s, &[true, true, true, false, false]).unwrap();
        assert_eq!(g.order_lines(), 6, "sunset: S3 on the three lines");
        assert!(g.closed);
        assert!(g.order_full() >= 6);
        let k = Family::parse(KITE).unwrap();
        let g = automorphisms(&k, &[true; 5]).unwrap();
        assert_eq!(g.order_lines(), 4, "kite: k1<->k2 and k -> q - k");
        assert!(g.closed);
        // the identity first, every line image exact
        assert!((0..5).all(|s| g.elements[0].exact[s] == Some(s)));
    }

    #[test]
    fn full_image_of_isp_is_consistent() {
        // the image of an ISP square is a combination of slots and q^2 whose own image under the inverse-free check:
        // applying the element twice to an exact key gives a key of the group's orbit
        let s = Family::parse(SUNSET).unwrap();
        let mask = [true, true, true, false, false];
        let g = automorphisms(&s, &mask).unwrap();
        for e in &g.elements {
            let im = e.full_image(&[1, 1, 1, -1, 0], &mask).unwrap();
            assert!(!im.is_empty());
            // each image key keeps the line exponents' sum or lowers one line by one (a D_t of the numerator)
            for k in im.keys() {
                let lines: i64 = (0..3).map(|i| k[i]).sum();
                assert!(lines == 3 || lines == 2, "{k:?}");
            }
        }
    }

    #[test]
    fn bubble_and_sunset_masters() {
        let (doc, ok) = run(BUBBLE, &[true, true], &opts(vec![vec![1, 1]], 1, 0)).unwrap();
        assert!(ok, "{doc:#}");
        assert_eq!(doc["dim_V"], 1);
        assert_eq!(doc["dim_VG"], 1);
        assert_eq!(doc["traces"], json!([1, 1]));
        let mask = [true, true, true, false, false];
        let (doc, ok) = run(SUNSET, &mask, &opts(vec![vec![1, 1, 1, 0, 0]], 2, 3)).unwrap();
        assert!(ok, "{doc:#}");
        assert_eq!(doc["dim_V"], 1, "{doc:#}");
        assert_eq!(doc["dim_VG"], 1);
        assert!(doc["traces"].as_array().unwrap().iter().all(|t| t == 1));
        assert_eq!(doc["masters_are_V_basis"], true);
        // the master maps to itself with coefficient 1 at every d
        for e in doc["exact"].as_array().unwrap() {
            assert_eq!(e["masters"]["I(1,1,1,0,0)"]["I(1,1,1,0,0)"], "1");
        }
    }

    #[test]
    fn symmetry_rows_are_implied_by_ibp_on_the_sunset() {
        // V = V_G = 1: every exact row I(a) - I(g a) must already lie in the IBP span (signs / slot order check)
        let s = Family::parse(SUNSET).unwrap();
        let mask = [true, true, true, false, false];
        let g = automorphisms(&s, &mask).unwrap();
        let p = super::super::mastermap::PRIMES[0];
        let d = Q { n: 37, d: 7 };
        let sd = seeds(&mask, 2, 2);
        let mut rows: Vec<Vec<(Key, u64)>> = vec![];
        let mut cols: HashSet<Key> = HashSet::new();
        for sk in &sd {
            for r in s.rows_at(sk, d.modp(p).unwrap(), p).unwrap() {
                let row: Vec<(Key, u64)> = r
                    .into_iter()
                    .map(|(off, c)| (sk.iter().zip(&off).map(|(a, b)| a + b).collect::<Key>(), c))
                    .filter(|(k, _)| (0..3).all(|i| k[i] >= 1))
                    .collect();
                cols.extend(row.iter().map(|x| x.0.clone()));
                rows.push(row);
            }
        }
        let mut colv: Vec<Key> = cols.into_iter().collect();
        colv.sort_by_key(|k| std::cmp::Reverse({
            let (dd, rr) = dr(&mask, k);
            (dd + rr, rr, dd, k.clone())
        }));
        let cid: HashMap<Key, usize> = colv.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
        let mut e = Elim::new(p);
        for r in &rows {
            e.insert(&r.iter().map(|(k, c)| (cid[k], *c)).collect::<Vec<_>>()).unwrap();
        }
        let mut checked = 0;
        for a in seeds(&mask, 1, 0) {
            for &gi in &g.line_reps[1..] {
                let b = g.elements[gi].exact_image(&a).unwrap();
                if let (Some(&ca), Some(&cb)) = (cid.get(&a), cid.get(&b)) {
                    assert!(e.reduce(&[(ca, 1), (cb, p - 1)]).is_empty(), "{a:?} vs {b:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }

    fn node_family(name: &str) -> Option<(String, Vec<bool>)> {
        let p = format!("{}/src/node/testdata/{name}.sym", env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(&p).ok()?;
        let mask: Vec<bool> = src
            .lines()
            .filter(|l| l.trim_start().starts_with("prop("))
            .map(|l| l.trim_end().trim_end_matches(',').trim_end_matches(')').ends_with(",1"))
            .collect();
        Some((src, mask))
    }

    /// Five-loop nodes against mcount_sym (REPORT A5, box (1,2), interior dots + rank <= 1): IBP / exact rows.
    #[test]
    #[ignore = "five-loop Laporta (seconds in release, minutes in debug)"]
    fn five_loop_nodes_against_mcount_sym() {
        for (name, gl, ibp, exact) in [("T266", 8, 7, 3), ("T265", 4, 3, 2)] {
            let (fam, mask) = node_family(name).expect("testdata family");
            let mut o = opts(vec![], 1, 2);
            o.threads = 3;
            let (doc, ok) = run(&fam, &mask, &o).unwrap();
            eprintln!("{name}: {}", serde_json::to_string(&doc["certificate"]).unwrap());
            assert_eq!(doc["group"]["order_lines"], gl, "{name}");
            assert_eq!(doc["dim_V"], ibp, "{name}");
            assert_eq!(doc["dim_VG"], exact, "{name}");
            assert!(ok, "{name}: certificate");
        }
    }
}
