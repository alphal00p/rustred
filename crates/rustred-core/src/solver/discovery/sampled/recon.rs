//! Modular-sampled reconstruction of ONE target row of a fixed elimination trace (p5l-lift-sampled, hypothesis H2).
//!
//! The caller supplies an oracle: active-variable point mod p -> values of the (fixed, ordered) support columns of the
//! normalized target row, or None for an unusable point (pole of a source coefficient, non-generic pivot trace, extra
//! column). Steps:
//! 1. degree probe: Thiele interpolation along one line per variable and one generic line (3 fresh confirmations per
//!    column) gives per column total and per-variable (numerator, denominator) degrees, and the same for F = sum r_i c_i
//!    with random r_i, whose reduced denominator is the common denominator L of the row (generic r);
//! 2. per prime: a dense ansatz F*L - N_F = 0 with the exact degree boxes gives L (1-dim kernel, fixed normalization
//!    monomial); then every column P_i = c_i*L is a polynomial in one shared monomial box, solved with ONE elimination of
//!    the sample Vandermonde matrix against all columns at once (extra sample rows must be consistent);
//! 3. CRT over primes + maximal-quotient rational reconstruction, accepted when the next prime agrees.
//! Nothing here is authority: the caller verifies the result at fresh points of a fresh prime (and a mutation control).
//! Self-contained (u64/u128 modular arithmetic + Symbolica Integer/Rational for CRT), shared verbatim by the standalone
//! trace_probe tool and the engine backend.

#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_range_loop,
    clippy::explicit_counter_loop,
    clippy::collapsible_if,
    clippy::doc_lazy_continuation,
    reason = "self-contained numeric kernel (p5l-lift-sampled), kept close to its standalone trace_probe source"
)]

use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

#[derive(Clone, Copy, Debug)]
pub struct Fp {
    pub p: u64,
}

impl Fp {
    #[inline]
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        ((a as u128 * b as u128) % self.p as u128) as u64
    }
    #[inline]
    pub fn add(&self, a: u64, b: u64) -> u64 {
        let s = a as u128 + b as u128;
        if s >= self.p as u128 {
            (s - self.p as u128) as u64
        } else {
            s as u64
        }
    }
    #[inline]
    pub fn sub(&self, a: u64, b: u64) -> u64 {
        if a >= b {
            a - b
        } else {
            (a as u128 + self.p as u128 - b as u128) as u64
        }
    }
    pub fn pow(&self, mut a: u64, mut e: u64) -> u64 {
        let mut r = 1u64;
        while e > 0 {
            if e & 1 == 1 {
                r = self.mul(r, a);
            }
            a = self.mul(a, a);
            e >>= 1;
        }
        r
    }
    pub fn inv(&self, a: u64) -> u64 {
        debug_assert!(a != 0);
        self.pow(a, self.p - 2)
    }
    pub fn of_integer(&self, x: &Integer) -> u64 {
        let p = Integer::from(self.p);
        let mut r = x % &p;
        if r.is_negative() {
            r = &r + &p;
        }
        match r {
            Integer::Single(v) => v as u64,
            Integer::Double(v) => v.get() as u64,
            other => panic!("residue out of range: {other}"),
        }
    }
}

/// Deterministic xorshift generator (reproducible sample points per seed).
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn element(&mut self, p: u64) -> u64 {
        1 + self.next() % (p - 1)
    }
}

pub trait Oracle: Sync {
    /// Number of active variables.
    fn nvars(&self) -> usize;
    /// Number of support columns of the target row (fixed order).
    fn ncols(&self) -> usize;
    /// Support-column values at `point` mod `p`, or None for an unusable point.
    fn eval(&self, p: u64, point: &[u64]) -> Option<Vec<u64>>;
}

/// Evaluate the oracle at many points with `threads` worker threads.
pub fn eval_many<O: Oracle>(
    oracle: &O,
    p: u64,
    points: &[Vec<u64>],
    threads: usize,
) -> Vec<Option<Vec<u64>>> {
    let next = AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<Vec<u64>>>> =
        points.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, AtomicOrdering::Relaxed);
                    if i >= points.len() {
                        break;
                    }
                    let value = oracle.eval(p, &points[i]);
                    *out[i].lock().unwrap() = value;
                }
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().unwrap()).collect()
}

/// Incremental Thiele interpolation; `confirmed` counts consecutive fresh points already reproduced.
#[derive(Clone)]
struct Thiele {
    xs: Vec<u64>,
    a: Vec<u64>,
    confirmed: usize,
    failed: bool,
}

impl Thiele {
    fn new() -> Self {
        Thiele {
            xs: vec![],
            a: vec![],
            confirmed: 0,
            failed: false,
        }
    }
    fn matches(&self, f: &Fp, x: u64, y: u64) -> bool {
        if self.a.is_empty() {
            return false;
        }
        let (mut pm2, mut qm2, mut pm1, mut qm1) = (1u64, 0u64, self.a[0], 1u64);
        for j in 1..self.a.len() {
            let l = f.sub(x, self.xs[j - 1]);
            let p = f.add(f.mul(self.a[j], pm1), f.mul(l, pm2));
            let q = f.add(f.mul(self.a[j], qm1), f.mul(l, qm2));
            pm2 = pm1;
            qm2 = qm1;
            pm1 = p;
            qm1 = q;
        }
        qm1 != 0 && pm1 == f.mul(y, qm1)
    }
    fn push(&mut self, f: &Fp, x: u64, y: u64) {
        if self.failed || self.confirmed >= 3 {
            return;
        }
        if self.matches(f, x, y) {
            self.confirmed += 1;
            return;
        }
        self.confirmed = 0;
        let (mut vn, mut vd) = (y, 1u64);
        for j in 0..self.a.len() {
            let dd = f.sub(vn, f.mul(self.a[j], vd));
            if dd == 0 {
                self.failed = true;
                return;
            }
            vn = f.mul(f.sub(x, self.xs[j]), vd);
            vd = dd;
        }
        self.xs.push(x);
        self.a.push(f.mul(vn, f.inv(vd)));
    }
    /// Degrees (numerator, denominator) of the reduced interpolant.
    fn degrees(&self, f: &Fp) -> (usize, usize) {
        let k = self.a.len();
        let mut pm2 = vec![1u64];
        let mut qm2 = vec![0u64];
        let mut pm1 = vec![self.a[0]];
        let mut qm1 = vec![1u64];
        for j in 1..k {
            let lin = |p: &Vec<u64>| {
                let mut r = vec![0u64; p.len() + 1];
                for (i, &c) in p.iter().enumerate() {
                    r[i + 1] = f.add(r[i + 1], c);
                    r[i] = f.sub(r[i], f.mul(c, self.xs[j - 1]));
                }
                r
            };
            let comb = |p1: &Vec<u64>, p2: &Vec<u64>| {
                let l = lin(p2);
                let mut r = vec![0u64; p1.len().max(l.len())];
                for (i, &c) in p1.iter().enumerate() {
                    r[i] = f.add(r[i], f.mul(c, self.a[j]));
                }
                for (i, &c) in l.iter().enumerate() {
                    r[i] = f.add(r[i], c);
                }
                r
            };
            let p = comb(&pm1, &pm2);
            let q = comb(&qm1, &qm2);
            pm2 = pm1;
            qm2 = qm1;
            pm1 = p;
            qm1 = q;
        }
        let n = trim(pm1);
        let d = trim(qm1);
        let g = pgcd(f, n.clone(), d.clone());
        (deg(&n) - deg(&g), deg(&d) - deg(&g))
    }
}

fn trim(mut p: Vec<u64>) -> Vec<u64> {
    while p.len() > 1 && *p.last().unwrap() == 0 {
        p.pop();
    }
    p
}
fn is_zero_poly(p: &[u64]) -> bool {
    p.len() == 1 && p[0] == 0
}
fn deg(p: &[u64]) -> usize {
    if is_zero_poly(p) { 0 } else { p.len() - 1 }
}
fn prem(f: &Fp, a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut r = trim(a.to_vec());
    let lb = f.inv(*b.last().unwrap());
    while !is_zero_poly(&r) && r.len() >= b.len() {
        let c = f.mul(*r.last().unwrap(), lb);
        let sh = r.len() - b.len();
        for (i, &bc) in b.iter().enumerate() {
            r[sh + i] = f.sub(r[sh + i], f.mul(c, bc));
        }
        r.pop();
        if r.is_empty() {
            r.push(0);
        }
        r = trim(r);
    }
    r
}
fn pgcd(f: &Fp, mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    while !is_zero_poly(&b) {
        let r = prem(f, &a, &b);
        a = b;
        b = r;
    }
    a
}

#[derive(Clone, Debug, Default)]
pub struct DegreeInfo {
    /// per support column: total (num, den) degrees
    pub total: Vec<(usize, usize)>,
    /// per variable, per support column: (num, den) degrees in that variable
    pub per_var: Vec<Vec<(usize, usize)>>,
    /// F = sum r_i c_i: total and per-variable (num, den)
    pub f_total: (usize, usize),
    pub f_per_var: Vec<(usize, usize)>,
    pub weights: Vec<u64>,
    pub points: usize,
    pub bad_points: usize,
}

/// Thiele along one line base + t*dir; returns per-column degrees and F's degrees.
pub fn probe_line<O: Oracle>(
    oracle: &O,
    f: &Fp,
    base: &[u64],
    dir: &[u64],
    weights: &[u64],
    rng: &mut Rng,
    threads: usize,
    max_points: usize,
    used: &mut usize,
    bad: &mut usize,
) -> Result<(Vec<(usize, usize)>, (usize, usize)), String> {
    let s = oracle.ncols();
    let mut th = vec![Thiele::new(); s + 1];
    let mut batch = (2 * threads).max(8);
    let mut local = 0usize;
    while th.iter().any(|x| !x.failed && x.confirmed < 3) {
        if local >= max_points {
            return Err(format!(
                "degree probe: line not converged after {local} points"
            ));
        }
        let ts: Vec<u64> = (0..batch).map(|_| rng.element(f.p)).collect();
        let points: Vec<Vec<u64>> = ts
            .iter()
            .map(|&t| {
                base.iter()
                    .zip(dir)
                    .map(|(&b, &d)| f.add(b, f.mul(d, t)))
                    .collect()
            })
            .collect();
        let values = eval_many(oracle, f.p, &points, threads);
        for (t, v) in ts.iter().zip(values) {
            *used += 1;
            local += 1;
            let Some(v) = v else {
                *bad += 1;
                continue;
            };
            let mut fv = 0u64;
            for (k, x) in th.iter_mut().take(s).enumerate() {
                x.push(f, *t, v[k]);
                fv = f.add(fv, f.mul(weights[k], v[k]));
            }
            th[s].push(f, *t, fv);
        }
        batch = (batch * 2).min(256);
    }
    if th.iter().any(|x| x.failed) {
        return Err("degree probe: Thiele breakdown (unlucky point)".into());
    }
    let mut out: Vec<(usize, usize)> = th.iter().map(|x| x.degrees(f)).collect();
    let fd = out.pop().unwrap();
    Ok((out, fd))
}

pub fn probe_degrees<O: Oracle>(
    oracle: &O,
    f: &Fp,
    seed: u64,
    threads: usize,
    max_points_per_line: usize,
) -> Result<DegreeInfo, String> {
    let v = oracle.nvars();
    let mut rng = Rng(seed | 1);
    let weights: Vec<u64> = (0..oracle.ncols()).map(|_| rng.element(f.p)).collect();
    let mut info = DegreeInfo {
        weights: weights.clone(),
        ..Default::default()
    };
    let (mut used, mut bad) = (0, 0);
    for var in 0..v {
        let base: Vec<u64> = (0..v).map(|_| rng.element(f.p)).collect();
        let mut dir = vec![0u64; v];
        dir[var] = 1;
        let (cols, fd) = probe_line(
            oracle,
            f,
            &base,
            &dir,
            &weights,
            &mut rng,
            threads,
            max_points_per_line,
            &mut used,
            &mut bad,
        )?;
        info.per_var.push(cols);
        info.f_per_var.push(fd);
    }
    let base: Vec<u64> = (0..v).map(|_| rng.element(f.p)).collect();
    let dir: Vec<u64> = (0..v).map(|_| rng.element(f.p)).collect();
    let (cols, fd) = probe_line(
        oracle,
        f,
        &base,
        &dir,
        &weights,
        &mut rng,
        threads,
        max_points_per_line,
        &mut used,
        &mut bad,
    )?;
    info.total = cols;
    info.f_total = fd;
    info.points = used;
    info.bad_points = bad;
    Ok(info)
}

/// Exponent vectors with total degree <= total and e_v <= bound[v], graded then lexicographic.
pub fn monomials(total: usize, bound: &[usize]) -> Vec<Vec<u16>> {
    let v = bound.len();
    let mut out = Vec::new();
    for t in 0..=total {
        let mut e = vec![0u16; v];
        fn rec(i: usize, rest: usize, e: &mut Vec<u16>, bound: &[usize], out: &mut Vec<Vec<u16>>) {
            if i + 1 == e.len() {
                if rest <= bound[i] {
                    e[i] = rest as u16;
                    out.push(e.clone());
                }
                return;
            }
            for x in (0..=rest.min(bound[i])).rev() {
                e[i] = x as u16;
                rec(i + 1, rest - x, e, bound, out);
            }
        }
        if v == 0 {
            if t == 0 {
                out.push(vec![]);
            }
            continue;
        }
        rec(0, t, &mut e, bound, &mut out);
    }
    out
}

fn monomial_values(f: &Fp, mons: &[Vec<u16>], point: &[u64], maxdeg: usize) -> Vec<u64> {
    let pows: Vec<Vec<u64>> = point
        .iter()
        .map(|&x| {
            let mut p = vec![1u64; maxdeg + 1];
            for i in 1..=maxdeg {
                p[i] = f.mul(p[i - 1], x);
            }
            p
        })
        .collect();
    mons.iter()
        .map(|e| {
            e.iter().enumerate().fold(1u64, |acc, (v, &k)| {
                if k == 0 {
                    acc
                } else {
                    f.mul(acc, pows[v][k as usize])
                }
            })
        })
        .collect()
}

/// Kernel of a (rows x u) matrix mod p if it is exactly one-dimensional.
fn kernel_1(f: &Fp, mut a: Vec<Vec<u64>>, u: usize) -> Option<Vec<u64>> {
    let mut pivcol = Vec::new();
    let mut r = 0;
    for c in 0..u {
        let Some(k) = (r..a.len()).find(|&k| a[k][c] != 0) else {
            continue;
        };
        a.swap(r, k);
        let iv = f.inv(a[r][c]);
        for x in a[r].iter_mut() {
            *x = f.mul(*x, iv);
        }
        let prow = a[r].clone();
        for (k, row) in a.iter_mut().enumerate() {
            if k != r && row[c] != 0 {
                let m = row[c];
                for (x, &y) in row.iter_mut().zip(&prow).skip(c) {
                    *x = f.sub(*x, f.mul(m, y));
                }
            }
        }
        pivcol.push(c);
        r += 1;
        if r == a.len() {
            break;
        }
    }
    if pivcol.len() + 1 != u {
        return None;
    }
    let free = (0..u).find(|c| !pivcol.contains(c))?;
    let mut v = vec![0u64; u];
    v[free] = 1;
    for (row, &c) in pivcol.iter().enumerate() {
        v[c] = f.sub(0, a[row][free]);
    }
    Some(v)
}

/// Solve V X = Y (V: k x u with k >= u, full column rank) for all right-hand sides at once; the k-u extra rows must be
/// consistent. Returns X (u rows x m columns) or None.
fn solve_multi(
    f: &Fp,
    mut v: Vec<Vec<u64>>,
    mut y: Vec<Vec<u64>>,
    u: usize,
) -> Option<Vec<Vec<u64>>> {
    let k = v.len();
    let mut r = 0;
    for c in 0..u {
        let piv = (r..k).find(|&i| v[i][c] != 0)?;
        v.swap(r, piv);
        y.swap(r, piv);
        let iv = f.inv(v[r][c]);
        for x in v[r].iter_mut() {
            *x = f.mul(*x, iv);
        }
        for x in y[r].iter_mut() {
            *x = f.mul(*x, iv);
        }
        let (vr, yr) = (v[r].clone(), y[r].clone());
        for i in 0..k {
            if i != r && v[i][c] != 0 {
                let m = v[i][c];
                for (x, &z) in v[i].iter_mut().zip(&vr).skip(c) {
                    *x = f.sub(*x, f.mul(m, z));
                }
                for (x, &z) in y[i].iter_mut().zip(&yr) {
                    *x = f.sub(*x, f.mul(m, z));
                }
            }
        }
        r += 1;
    }
    // consistency of the extra rows
    for i in u..k {
        if y[i].iter().any(|&x| x != 0) {
            return None;
        }
    }
    y.truncate(u);
    Some(y)
}

/// The reconstructed row in one prime: common denominator L and numerators P_i (coefficients over the monomial boxes).
pub struct PrimeImage {
    pub l: Vec<u64>,
    pub p: Vec<Vec<u64>>,
    pub points: usize,
    pub bad_points: usize,
}

pub struct Boxes {
    pub l_mons: Vec<Vec<u16>>,
    pub nf_mons: Vec<Vec<u16>>,
    pub p_mons: Vec<Vec<u16>>,
    pub maxdeg: usize,
}

pub fn boxes(info: &DegreeInfo) -> Boxes {
    let v = info.f_per_var.len();
    let l_total = info.f_total.1;
    let l_bound: Vec<usize> = info.f_per_var.iter().map(|x| x.1).collect();
    let nf_mons = monomials(
        info.f_total.0,
        &info.f_per_var.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let l_mons = monomials(l_total, &l_bound);
    // P_i = N_i * (L / D_i): total <= n_i - m_i + deg L; per variable <= n_iv - m_iv + deg_v L
    let p_total = info
        .total
        .iter()
        .map(|&(n, m)| (n + l_total).saturating_sub(m))
        .max()
        .unwrap_or(0);
    let p_bound: Vec<usize> = (0..v)
        .map(|var| {
            info.per_var[var]
                .iter()
                .map(|&(n, m)| (n + l_bound[var]).saturating_sub(m))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let p_mons = monomials(p_total, &p_bound);
    let maxdeg = [info.f_total.0, l_total, p_total]
        .into_iter()
        .max()
        .unwrap();
    Boxes {
        l_mons,
        nf_mons,
        p_mons,
        maxdeg,
    }
}

/// Dense-ansatz limit (unknowns of one elimination): above it the cubic solve and the sample count are impractical.
pub const MAX_BOX: usize = 4000;

/// One prime image. `norm` = index of the L monomial whose coefficient is fixed to 1 (chosen at the first prime).
pub fn prime_image<O: Oracle>(
    oracle: &O,
    f: &Fp,
    b: &Boxes,
    weights: &[u64],
    norm: &mut Option<usize>,
    seed: u64,
    threads: usize,
) -> Result<PrimeImage, String> {
    let v = oracle.nvars();
    let s = oracle.ncols();
    let uf = b.nf_mons.len() + b.l_mons.len();
    if uf > MAX_BOX || b.p_mons.len() > MAX_BOX {
        return Err(format!(
            "dense box too large (L {} + NF {} unknowns, P {} monomials; limit {MAX_BOX}): the row needs a sparse/factored reconstruction",
            b.l_mons.len(),
            b.nf_mons.len(),
            b.p_mons.len()
        ));
    }
    let need = uf.max(b.p_mons.len()) + 8;
    let mut rng = Rng(seed | 1);
    let mut pts: Vec<Vec<u64>> = Vec::new();
    let mut vals: Vec<Vec<u64>> = Vec::new();
    let mut bad = 0usize;
    let mut total = 0usize;
    while pts.len() < need {
        let want = need - pts.len();
        let cand: Vec<Vec<u64>> = (0..want)
            .map(|_| (0..v).map(|_| rng.element(f.p)).collect())
            .collect();
        for (x, y) in cand.iter().zip(eval_many(oracle, f.p, &cand, threads)) {
            total += 1;
            match y {
                Some(y) => {
                    pts.push(x.clone());
                    vals.push(y);
                }
                None => bad += 1,
            }
        }
        if total > 4 * need + 64 {
            return Err(format!("too many unusable points ({bad} of {total})"));
        }
    }
    // (a) common denominator from F = sum w_i c_i: rows [N_F monomials | -F * L monomials]
    let a: Vec<Vec<u64>> = pts
        .iter()
        .zip(&vals)
        .map(|(x, y)| {
            let fv = y
                .iter()
                .zip(weights)
                .fold(0u64, |acc, (&c, &w)| f.add(acc, f.mul(c, w)));
            let mut row = monomial_values(f, &b.nf_mons, x, b.maxdeg);
            row.extend(
                monomial_values(f, &b.l_mons, x, b.maxdeg)
                    .into_iter()
                    .map(|m| f.sub(0, f.mul(fv, m))),
            );
            row
        })
        .collect();
    let kernel =
        kernel_1(f, a, uf).ok_or("common-denominator ansatz: kernel is not one-dimensional")?;
    let mut l: Vec<u64> = kernel[b.nf_mons.len()..].to_vec();
    let j = match *norm {
        Some(j) => j,
        None => {
            let j = l
                .iter()
                .position(|&c| c != 0)
                .ok_or("zero common denominator")?;
            *norm = Some(j);
            j
        }
    };
    if l[j] == 0 {
        return Err("normalization monomial vanishes in this prime".into());
    }
    let iv = f.inv(l[j]);
    for c in l.iter_mut() {
        *c = f.mul(*c, iv);
    }
    // (b) P_i = c_i * L in the shared box, all columns at once
    let vmat: Vec<Vec<u64>> = pts
        .iter()
        .map(|x| monomial_values(f, &b.p_mons, x, b.maxdeg))
        .collect();
    let y: Vec<Vec<u64>> = pts
        .iter()
        .zip(&vals)
        .map(|(x, row)| {
            let lv = monomial_values(f, &b.l_mons, x, b.maxdeg)
                .iter()
                .zip(&l)
                .fold(0u64, |acc, (&m, &c)| f.add(acc, f.mul(m, c)));
            row.iter().map(|&c| f.mul(c, lv)).collect()
        })
        .collect();
    let sol = solve_multi(f, vmat, y, b.p_mons.len())
        .ok_or("numerator box: singular or inconsistent sample system")?;
    // transpose: per column coefficient vector
    let p: Vec<Vec<u64>> = (0..s)
        .map(|i| sol.iter().map(|row| row[i]).collect())
        .collect();
    Ok(PrimeImage {
        l,
        p,
        points: total,
        bad_points: bad,
    })
}

/// CRT accumulator over primes with rational reconstruction.
pub struct Lift {
    pub residues: Vec<Integer>,
    pub modulus: Integer,
    pub primes: Vec<u64>,
    pub candidate: Option<Vec<Rational>>,
}

impl Lift {
    pub fn new() -> Self {
        Lift {
            residues: vec![],
            modulus: Integer::one(),
            primes: vec![],
            candidate: None,
        }
    }
    fn flatten(img: &PrimeImage) -> Vec<u64> {
        let mut v = img.l.clone();
        for c in &img.p {
            v.extend_from_slice(c);
        }
        v
    }
    /// Add one prime image; returns true when a candidate reconstructed from the earlier primes is confirmed by it.
    pub fn add(&mut self, p: u64, img: &PrimeImage) -> bool {
        let flat = Self::flatten(img);
        let f = Fp { p };
        if let Some(c) = &self.candidate {
            if c.len() == flat.len()
                && c.iter().zip(&flat).all(|(q, &r)| {
                    let n = f.of_integer(&q.numerator());
                    let d = f.of_integer(&q.denominator());
                    d != 0 && f.mul(n, f.inv(d)) == r
                })
            {
                self.primes.push(p);
                return true;
            }
        }
        let pi = Integer::from(p);
        if self.primes.is_empty() {
            self.residues = flat.iter().map(|&r| Integer::from(r)).collect();
        } else {
            self.residues = self
                .residues
                .iter()
                .zip(&flat)
                .map(|(a, &r)| {
                    Integer::chinese_remainder(
                        Integer::from(r),
                        a.clone(),
                        pi.clone(),
                        self.modulus.clone(),
                    )
                })
                .collect();
        }
        self.modulus = &self.modulus * &pi;
        self.primes.push(p);
        let rec: Result<Vec<Rational>, _> = self
            .residues
            .iter()
            .map(|r| Rational::maximal_quotient_reconstruction(r, &self.modulus, None))
            .collect();
        self.candidate = rec.ok();
        false
    }
}

/// Primes just below 2^62 (Miller-Rabin via Symbolica's Integer::is_prime), descending.
pub fn primes_below_2_62(count: usize, skip: usize) -> Vec<u64> {
    let mut out = Vec::new();
    let mut n: u64 = (1u64 << 62) - 57;
    let mut seen = 0;
    while out.len() < count {
        if Integer::from(n).is_prime(20) {
            if seen >= skip {
                out.push(n);
            }
            seen += 1;
        }
        n -= 2;
    }
    out
}

/// Evaluate a reconstructed coefficient vector (rationals) at a point mod p: returns the column values c_i = P_i / L.
pub fn eval_rational_row(
    f: &Fp,
    b: &Boxes,
    coeffs: &[Rational],
    point: &[u64],
) -> Option<Vec<u64>> {
    let to = |q: &Rational| -> Option<u64> {
        let d = f.of_integer(&q.denominator());
        if d == 0 {
            return None;
        }
        Some(f.mul(f.of_integer(&q.numerator()), f.inv(d)))
    };
    let lm = monomial_values(f, &b.l_mons, point, b.maxdeg);
    let pm = monomial_values(f, &b.p_mons, point, b.maxdeg);
    let nl = b.l_mons.len();
    let mut lv = 0u64;
    for (m, q) in lm.iter().zip(&coeffs[..nl]) {
        lv = f.add(lv, f.mul(*m, to(q)?));
    }
    if lv == 0 {
        return None;
    }
    let il = f.inv(lv);
    let np = b.p_mons.len();
    let ncols = (coeffs.len() - nl) / np;
    let mut out = Vec::with_capacity(ncols);
    for i in 0..ncols {
        let mut s = 0u64;
        for (m, q) in pm.iter().zip(&coeffs[nl + i * np..nl + (i + 1) * np]) {
            if !q.is_zero() {
                s = f.add(s, f.mul(*m, to(q)?));
            }
        }
        out.push(f.mul(s, il));
    }
    Some(out)
}
