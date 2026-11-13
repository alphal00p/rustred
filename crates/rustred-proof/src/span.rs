//! Mode `instance-span`: weight-free, per integer key, exact.
//!
//! Statement proved at a key k (all 20 indices integer, d symbolic): with A(d) the IBP rows of the rule's recorded
//! seed set (seed pattern at k plus the rule's `recenter`, the 30 ordinary rows of each seed) and
//! v(d) = e_target - sum_c r_c(k,d) e_c the written rule at k, there are integer polynomials W_i(d) (one per
//! independent row) and L(d) != 0 with
//!     sum_i W_i(d) A_i(d) = L(d) T(d) v(d)          exactly in Z[d], column by column,
//! where T(d) != 0 is the product of the distinct rhs denominators at k. Hence v = sum_i W_i/(L T) A_i over Q(d):
//! the rule at k is an exact consequence of the IBP rows at its seeds (verdict `EXACT_IN_SPAN_AT_KEY`). Every
//! column of the instantiated rows is retained unless `drop_zero_sectors` is set (then the statement is modulo the
//! table's proved zero sectors, and the record says so).
//!
//! How W, L are found (soundness does NOT depend on it, only the final exact product does): one modular elimination
//! at a hash-derived (prime, d) fixes the independent rows S and pivot columns P (most complex integral first);
//! at further points d_j mod p the square system w A[S,P] = T v[P] is solved, each w_i is rationally reconstructed
//! in d (Newton interpolation + extended Euclid, early termination checked on 2 held-out points, hard cap from the
//! degree bound of handoff/helper-h10-kernel/INSTANCE_SPAN_NOTE.md: deg <= D_v + S_d, S_d = seed columns carrying
//! d), normalised to a common monic denominator, CRT over primes + rational number reconstruction, then the exact
//! check above. Primes are drawn in the window of `stream::PRIME_BASE` (disjoint from the engine's streams).
//!
//! Non-membership: if the residual of T v after reduction by the rows is nonzero at two independent points, the
//! status is `NOT_IN_SPAN_MODULAR` (meaning: not derivable from THESE seeds at k, with high probability); an exact
//! certificate u(d) (A u = 0, v u != 0) is not implemented here (see notes/proof-kernel.md).
#![allow(clippy::all)]
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use crate::core::*;
use crate::stream::{load_rows, prime_candidate, EngineRows, Rows, PRIME_RANGE_NOTE};

#[derive(Clone, Debug)]
pub struct SpanArgs {
    pub table: String,
    /// byte offset of the rule line in `table` (for huge tables; else the table is scanned)
    pub offset: Option<u64>,
    pub cert: Option<String>,
    pub rule: i64,
    pub chart: Option<String>,
    pub keys: Vec<Vec<(usize, i64)>>,
    pub drop_zero_sectors: bool,
    pub ignore_exc: bool,
    pub max_primes: usize,
    /// override of the per-prime point cap (default: degree bound + 4)
    pub max_points: Option<usize>,
    /// write one certificate file per proved key (weights W_i/L on the independent rows) into this directory
    pub emit_dir: Option<String>,
}
impl Default for SpanArgs {
    fn default() -> Self {
        SpanArgs { table: String::new(), offset: None, cert: None, rule: -1, chart: None, keys: vec![], drop_zero_sectors: false, ignore_exc: false, max_primes: 40, max_points: None, emit_dir: None }
    }
}

// ===================================================================== modular helpers (plain representation)
#[inline]
fn mm(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 * b as u128) % p as u128) as u64
}
#[inline]
fn ma(a: u64, b: u64, p: u64) -> u64 {
    let s = a as u128 + b as u128;
    (if s >= p as u128 { s - p as u128 } else { s }) as u64
}
#[inline]
fn ms(a: u64, b: u64, p: u64) -> u64 {
    if a >= b { a - b } else { a + (p - b) }
}
fn minv(a: u64, p: u64) -> u64 {
    powmod(a, p - 2, p)
}
fn mi64(x: i64, p: u64) -> u64 {
    x.rem_euclid(p as i64) as u64
}
fn mi128(x: i128, p: u64) -> u64 {
    x.rem_euclid(p as i128) as u64
}
type MP = Vec<u64>; // polynomial mod p, ascending, trimmed
fn mp_trim(a: &mut MP) {
    while a.last() == Some(&0) {
        a.pop();
    }
}
fn mp_eval(a: &MP, x: u64, p: u64) -> u64 {
    let mut r = 0;
    for &c in a.iter().rev() {
        r = ma(mm(r, x, p), c, p);
    }
    r
}
fn mp_mul(a: &MP, b: &MP, p: u64) -> MP {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut o = vec![0u64; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        if x == 0 {
            continue;
        }
        for (j, &y) in b.iter().enumerate() {
            o[i + j] = ma(o[i + j], mm(x, y, p), p);
        }
    }
    mp_trim(&mut o);
    o
}
fn mp_sub(a: &MP, b: &MP, p: u64) -> MP {
    let mut o = vec![0u64; a.len().max(b.len())];
    for i in 0..o.len() {
        o[i] = ms(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), p);
    }
    mp_trim(&mut o);
    o
}
fn mp_divrem(a: &MP, b: &MP, p: u64) -> (MP, MP) {
    let mut r = a.clone();
    if r.len() < b.len() {
        return (vec![], r);
    }
    let lc = minv(*b.last().unwrap(), p);
    let mut q = vec![0u64; r.len() - b.len() + 1];
    for k in (0..q.len()).rev() {
        let c = mm(r[k + b.len() - 1], lc, p);
        q[k] = c;
        if c != 0 {
            for (j, &y) in b.iter().enumerate() {
                r[k + j] = ms(r[k + j], mm(c, y, p), p);
            }
        }
    }
    mp_trim(&mut r);
    mp_trim(&mut q);
    (q, r)
}
fn mp_monic(a: &MP, p: u64) -> (MP, u64) {
    let l = *a.last().unwrap();
    let li = minv(l, p);
    (a.iter().map(|&c| mm(c, li, p)).collect(), l)
}
fn mp_gcd(a: &MP, b: &MP, p: u64) -> MP {
    let (mut x, mut y) = (a.clone(), b.clone());
    while !y.is_empty() {
        let (_, r) = mp_divrem(&x, &y, p);
        x = y;
        y = r;
    }
    if x.is_empty() { x } else { mp_monic(&x, p).0 }
}
/// Newton interpolation through (xs[i], ys[i]) -> coefficients
thread_local! {
    // inverses of the divided-difference denominators of the last point set (shared by all weights of a prime)
    static INV_CACHE: std::cell::RefCell<(u64, Vec<u64>, Vec<Vec<u64>>)> = std::cell::RefCell::new((0, vec![], vec![]));
}
fn mp_interp(xs: &[u64], ys: &[u64], p: u64) -> MP {
    let n = xs.len();
    let mut c = ys.to_vec();
    INV_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.0 != p || cache.1.as_slice() != xs {
            let inv: Vec<Vec<u64>> = (0..n).map(|j| (0..n).map(|i| if j >= 1 && i >= j { minv(ms(xs[i], xs[i - j], p), p) } else { 0 }).collect()).collect();
            *cache = (p, xs.to_vec(), inv);
        }
        let inv = &cache.2;
        for j in 1..n {
            for i in (j..n).rev() {
                let num = ms(c[i], c[i - 1], p);
                c[i] = mm(num, inv[j][i], p);
            }
        }
    });
    // expand Newton form
    let mut poly: MP = vec![];
    for i in (0..n).rev() {
        // poly = poly * (x - xs[i]) + c[i]
        let mut np = vec![0u64; poly.len() + 1];
        for (k, &a) in poly.iter().enumerate() {
            np[k + 1] = ma(np[k + 1], a, p);
            np[k] = ms(np[k], mm(a, xs[i], p), p);
        }
        np[0] = ma(np[0], c[i], p);
        poly = np;
    }
    mp_trim(&mut poly);
    poly
}
/// Rational reconstruction of a univariate function from values: returns (num, monic den) reduced, or None.
fn mp_ratrec(xs: &[u64], ys: &[u64], chk_x: &[u64], chk_y: &[u64], p: u64) -> Option<(MP, MP)> {
    let pp = mp_interp(xs, ys, p);
    let mut m: MP = vec![1];
    for &x in xs {
        m = mp_mul(&m, &vec![(p - x) % p, 1], p);
    }
    let ok = |r: &MP, t: &MP| -> bool {
        chk_x.iter().zip(chk_y).all(|(&x, &y)| {
            let tv = mp_eval(t, x, p);
            tv != 0 && mm(mp_eval(r, x, p), minv(tv, p), p) == y
        })
    };
    let (mut r0, mut r1) = (m, pp);
    let (mut t0, mut t1): (MP, MP) = (vec![], vec![1]);
    loop {
        if !t1.is_empty() && ok(&r1, &t1) {
            let g = mp_gcd(&r1, &t1, p);
            let (r, t) = if g.len() > 1 { (mp_divrem(&r1, &g, p).0, mp_divrem(&t1, &g, p).0) } else { (r1.clone(), t1.clone()) };
            let (tm, l) = mp_monic(&t, p);
            let li = minv(l, p);
            let rm: MP = r.iter().map(|&c| mm(c, li, p)).collect();
            return Some((rm, tm));
        }
        if r1.is_empty() {
            return None;
        }
        let (q, r) = mp_divrem(&r0, &r1, p);
        let t = mp_sub(&t0, &mp_mul(&q, &t1, p), p);
        r0 = std::mem::replace(&mut r1, r);
        t0 = std::mem::replace(&mut t1, t);
    }
}

// ===================================================================== sparse elimination mod p (Montgomery)
struct Elim {
    m: Mont,
    nunk: usize,
    piv_unk: Vec<u32>,
    piv_rows: Vec<Vec<(u32, u64)>>, // normalised: pivot coefficient 1 (Montgomery one)
    piv_rhs: Vec<u64>,
    is_piv: Vec<bool>,
    acc: Vec<u64>,
    touched: Vec<u32>,
    mark: Vec<bool>,
}
impl Elim {
    fn new(m: Mont, nunk: usize) -> Self {
        Elim { m, nunk, piv_unk: vec![], piv_rows: vec![], piv_rhs: vec![], is_piv: vec![false; nunk], acc: vec![0; nunk], touched: vec![], mark: vec![false; nunk] }
    }
    fn load(&mut self, eq: &[(u32, u64)]) {
        for &(u, x) in eq {
            let u = u as usize;
            if !self.mark[u] {
                self.mark[u] = true;
                self.touched.push(u as u32);
            }
            self.acc[u] = self.m.add(self.acc[u], x);
        }
    }
    /// reduce the loaded accumulator (and its rhs) by all pivots; returns the rhs
    fn reduce(&mut self, mut rhs: u64) -> u64 {
        let m = self.m;
        for k in 0..self.piv_unk.len() {
            let u = self.piv_unk[k] as usize;
            let f = self.acc[u];
            if f == 0 {
                continue;
            }
            self.acc[u] = 0;
            for &(w, x) in &self.piv_rows[k] {
                let w = w as usize;
                if !self.mark[w] {
                    self.mark[w] = true;
                    self.touched.push(w as u32);
                }
                self.acc[w] = m.sub(self.acc[w], m.mul(f, x));
            }
            rhs = m.sub(rhs, m.mul(f, self.piv_rhs[k]));
        }
        rhs
    }
    fn take(&mut self) -> Vec<(u32, u64)> {
        let mut out = vec![];
        for &u in &self.touched {
            let u = u as usize;
            if self.acc[u] != 0 {
                out.push((u as u32, self.acc[u]));
            }
            self.acc[u] = 0;
            self.mark[u] = false;
        }
        self.touched.clear();
        out.sort_unstable_by_key(|x| x.0);
        out
    }
    /// add one equation; returns Some(residual rhs) if it reduced to 0 = rhs (dependent), None if it became a pivot
    fn add(&mut self, eq: &[(u32, u64)], rhs: u64, choose: &dyn Fn(&[(u32, u64)]) -> u32) -> Option<u64> {
        self.load(eq);
        let rhs = self.reduce(rhs);
        let row = self.take();
        if row.is_empty() {
            return Some(rhs);
        }
        let u = choose(&row);
        let pv = row.iter().find(|x| x.0 == u).unwrap().1;
        let inv = self.m.inv(pv);
        let row: Vec<(u32, u64)> = row.into_iter().filter(|x| x.0 != u).map(|(w, x)| (w, self.m.mul(x, inv))).collect();
        self.piv_unk.push(u);
        self.is_piv[u as usize] = true;
        self.piv_rows.push(row);
        self.piv_rhs.push(self.m.mul(rhs, inv));
        None
    }
    /// back substitution, free unknowns = 0 (Montgomery values)
    fn solve(&self) -> Vec<u64> {
        let m = self.m;
        let mut x = vec![0u64; self.nunk];
        for k in (0..self.piv_unk.len()).rev() {
            let mut s = self.piv_rhs[k];
            for &(w, c) in &self.piv_rows[k] {
                s = m.sub(s, m.mul(c, x[w as usize]));
            }
            x[self.piv_unk[k] as usize] = s;
        }
        x
    }
}

// ===================================================================== instantiated system at a key
struct Inst {
    cols: Vec<Vec<i64>>,
    rows: Vec<Vec<(u32, i64, i64)>>, // (column, b, c): entry b + c d
    row_label: Vec<(usize, i64)>,    // (seed index in the record, source row m)
    v: Vec<(u32, BigPoly, BigPoly)>, // v_c = num_c(d)/den_c(d), integer polys, den != 0
    t_deg: usize,                    // max deg den_c
    dropped_cols: usize,
    seeds: usize,
    seed_cols_with_d: usize,
}

/// value of a sum text at an integer key as a polynomial in d (exact). keypow[v][e] = key_v^e.
pub(crate) fn eval_text_d(s: &[u8], keypow: &[Vec<Big>]) -> BigPoly {
    let mut acc: Vec<(i128, Big)> = vec![];
    let mut it = Terms::new(s);
    while let Some(t) = it.next() {
        let mut de = 0usize;
        let mut small: Option<i128> = match coef_i64(t.coef) { Some(c) => Some(c as i128), None => None };
        let mut big: Option<Big> = if small.is_none() { Some(Big::from_dec(t.coef)) } else { None };
        for k in 0..t.nf {
            let v = t.fv[k] as usize;
            let e = t.fe[k] as usize;
            if v == DV {
                de += e;
                continue;
            }
            let f = &keypow[v][e];
            if let Some(sv) = small {
                match f.to_i128().and_then(|fv| sv.checked_mul(fv)) {
                    Some(x) => small = Some(x),
                    None => { big = Some(Big::from_i128(sv).mul(f)); small = None; }
                }
            } else {
                big = Some(big.unwrap().mul(f));
            }
        }
        if acc.len() <= de {
            acc.resize(de + 1, (0, Big::zero()));
        }
        let slot = &mut acc[de];
        match small {
            Some(x) => {
                let x = if t.neg { -x } else { x };
                match slot.0.checked_add(x) {
                    Some(y) => slot.0 = y,
                    None => { slot.1.add(&Big::from_i128(slot.0)); slot.0 = x; }
                }
            }
            None => { let mut b = big.unwrap(); if t.neg { b = b.negated(); } slot.1.add(&b); }
        }
    }
    let mut out: BigPoly = acc.into_iter().map(|(s, mut b)| { b.add(&Big::from_i128(s)); b }).collect();
    bp_trim(&mut out);
    out
}

pub(crate) fn pat_at(p: &Pat, key: &[i64; 20]) -> Vec<i64> {
    p.iter().enumerate().map(|(i, &(s, o))| if s { key[i] + o } else { o }).collect()
}
fn complexity(c: &[i64]) -> (i64, i64, i64) {
    let lines = c.iter().filter(|&&x| x > 0).count() as i64;
    let pos: i64 = c.iter().filter(|&&x| x > 0).sum();
    let neg: i64 = c.iter().filter(|&&x| x < 0).map(|x| -x).sum();
    (lines, pos, neg)
}

pub fn run_span(args: &SpanArgs, engine: Option<&EngineRows>) -> String {
    let t_all = Instant::now();
    let mut o: Vec<(String, String)> = vec![];
    macro_rules! put { ($k:expr, $v:expr) => { o.push(($k.to_string(), $v)) } }
    put!("schema", jstr("rustred.proof-kernel.span.v1"));
    put!("mode", jstr("instance-span"));
    // ---------------------------------------------------------------- records: header, zero sectors, rule
    let t0 = Instant::now();
    let mut hline: Vec<u8> = vec![];
    let mut zero_masks: std::collections::HashSet<Vec<u8>> = Default::default();
    let mut zero_scope = String::new();
    let mut nl = 0;
    read_lines_matching(&args.table, |l| {
        nl += 1;
        if l.starts_with(b"{\"type\":\"header\"") { hline = l.to_vec(); }
        if l.starts_with(b"{\"type\":\"zero_sectors\"") {
            let j = jparse(l);
            zero_scope = String::from_utf8_lossy(j.get(l, "scope").map(|x| x.s(l)).unwrap_or(b"?")).to_string();
            for m in j.get(l, "masks").unwrap().arr() { zero_masks.insert(m.s(l).to_vec()); }
            return false;
        }
        nl < 8 && !l.starts_with(b"{\"type\":\"rule\"")
    });
    let mut tline: Vec<u8> = vec![];
    if let Some(off) = args.offset {
        use std::io::{BufRead, BufReader, Seek, SeekFrom};
        let mut f = std::fs::File::open(&args.table).unwrap_or_else(|e| die(&format!("open: {}", e)));
        f.seek(SeekFrom::Start(off)).unwrap();
        let mut r = BufReader::with_capacity(1 << 22, f);
        r.read_until(b'\n', &mut tline).unwrap();
        while tline.last() == Some(&b'\n') { tline.pop(); }
    } else {
        let pat_idx = format!("\"index\":{},", args.rule);
        read_lines_matching(&args.table, |l| {
            if l.starts_with(b"{\"type\":\"rule\"") && find(&l[..l.len().min(400)], pat_idx.as_bytes()) {
                let j = jparse(&l[..l.len()]);
                let ok = j.get(l, "index").map(|x| x.int(l)) == Some(args.rule) && args.chart.as_ref().map_or(true, |c| j.get(l, "chart").unwrap().s(l) == c.as_bytes());
                if ok { tline = l.to_vec(); return false; }
            }
            true
        });
    }
    if tline.is_empty() || !tline.starts_with(b"{\"type\":\"rule\"") { die("rule record not found in table"); }
    let tj = jparse(&tline);
    if tj.get(&tline, "index").map(|x| x.int(&tline)) != Some(args.rule) && args.rule >= 0 { die("record at offset is not the requested rule index"); }
    let chart = String::from_utf8(tj.get(&tline, "chart").unwrap().s(&tline).to_vec()).unwrap();
    let t_load = t0.elapsed().as_secs_f64();
    put!("rule", format!("{}", tj.get(&tline, "index").unwrap().int(&tline)));
    put!("chart", jstr(&chart));
    put!("record_bytes", format!("{}", tline.len()));

    // ---------------------------------------------------------------- rows (engine or cert)
    let mut nonaffine = 0; let mut row_terms = 0;
    let mut cert_lines: Vec<Vec<u8>> = vec![];
    if let Some(c) = &args.cert { read_lines_matching(c, |l| { if l.starts_with(b"{\"type\":\"source_row\"") { cert_lines.push(l.to_vec()); } true }); }
    let cert_rows = load_rows(&cert_lines, &mut nonaffine, &mut row_terms);
    let rows: Rows = if let Some(er) = engine {
        let (mut na, mut rt) = (0, 0);
        let exr = load_rows(&er.lines, &mut na, &mut rt);
        nonaffine += na;
        let same = if cert_rows.is_empty() { "null".to_string() } else { crate::stream::rows_equal(&cert_rows, &exr).to_string() };
        put!("rows", format!("{{\"source\":\"engine\",\"engine\":{},\"engine_rows\":{},\"cert_rows\":{},\"equal_to_cert_source_rows\":{}}}", jstr(&er.note), exr.len(), cert_rows.len(), same));
        exr
    } else {
        if cert_rows.is_empty() { die("no rows: give --cert with source_row records or build with the engine"); }
        put!("rows", format!("{{\"source\":\"cert source_row records\",\"cert_rows\":{}}}", cert_rows.len()));
        cert_rows
    };
    if nonaffine > 0 { die("a row coefficient is not affine"); }
    // item 1 of the helper note: d appears only at the zero shift (the seed's own column), affinely
    let mut d_off_seed = 0usize;
    for v in rows.values() { for (off, a) in v { if a.lin.iter().any(|x| x.0 as usize == DV) && off.iter().any(|&x| x != 0) { d_off_seed += 1; } } }
    put!("row_shape", format!("{{\"rows\":{},\"d_terms_off_the_seed_column\":{},\"all_coefficients_affine\":true}}", rows.len(), d_off_seed));
    let mut row_ids: Vec<i64> = rows.keys().copied().collect();
    row_ids.sort();

    // ---------------------------------------------------------------- rule fields
    let mut fixed: [Option<i64>; 20] = [None; 20];
    for (i, x) in tj.get(&tline, "fixed").unwrap().arr().iter().enumerate() { if let J::Num(..) = x { fixed[i] = Some(x.int(&tline)); } }
    let eq_aff: Vec<Aff> = tj.get(&tline, "equations").map(|x| x.arr().iter().map(|e| parse_affine(e.s(&tline)).unwrap_or_else(|er| die(&format!("case equation: {}", er)))).collect()).unwrap_or_default();
    let exc: Vec<Vec<&[u8]>> = tj.get(&tline, "exceptions").map(|e| e.arr().iter().map(|b| b.arr().iter().map(|x| x.s(&tline)).collect()).collect()).unwrap_or_default();
    let seeds: Vec<Pat> = tj.get(&tline, "seeds").unwrap_or_else(|| die("MISSING_SEEDS: the rule record has no seeds; instance-span needs the seed set")).arr().iter().map(|x| parse_pat(x.s(&tline))).collect();
    // recenter: the rule record's own (new tables), else the certificate's cert_rule (older rr_node dumps keep it
    // there only), else none
    let mut recenter_from = "rule record";
    let recenter: Vec<i64> = match tj.get(&tline, "recenter") {
        Some(J::Arr(a)) => a.iter().map(|x| x.int(&tline)).collect(),
        _ => {
            let mut rc: Option<Vec<i64>> = None;
            if let Some(c) = &args.cert {
                let rule_idx = tj.get(&tline, "index").unwrap().int(&tline);
                let pat_rule = format!("\"rule\":{},", rule_idx);
                read_lines_matching(c, |l| {
                    if l.starts_with(b"{\"type\":\"cert_rule\"") && find(&l[..l.len().min(400)], pat_rule.as_bytes()) {
                        let j = jparse(l);
                        if j.get(l, "chart").map(|x| x.s(l)) == Some(chart.as_bytes()) && j.get(l, "rule").map(|x| x.int(l)) == Some(rule_idx) {
                            if let Some(J::Arr(a)) = j.get(l, "recenter") { rc = Some(a.iter().map(|x| x.int(l)).collect()); }
                            return false;
                        }
                    }
                    true
                });
            }
            match rc { Some(v) => { recenter_from = "cert_rule record"; v } None => { recenter_from = "none (zero)"; vec![0; 20] } }
        }
    };
    put!("recenter_from", jstr(recenter_from));
    if recenter.len() != 20 { die("recenter arity"); }
    let target = parse_pat(tj.get(&tline, "target").unwrap().s(&tline));
    let rhs_arr = tj.get(&tline, "rhs").unwrap().arr();
    let rhs: Vec<(Pat, &[u8], Option<&[u8]>)> = rhs_arr.iter().map(|x| { let a = x.arr(); let (n, d) = split_coef(a[1].s(&tline)); (parse_pat(a[0].s(&tline)), n, d) }).collect();
    put!("rule_sizes", format!("{{\"rhs_terms\":{},\"seeds\":{},\"exception_branches\":{},\"case_equations\":{},\"recenter\":{:?}}}", rhs.len(), seeds.len(), exc.len(), eq_aff.len(), recenter));
    put!("zero_sectors", format!("{{\"scope\":{},\"masks\":{},\"dropped_from_identity\":{}}}", jstr(&zero_scope), zero_masks.len(), args.drop_zero_sectors));

    // ---------------------------------------------------------------- classify keys, evaluate rhs at all regular keys (one pass)
    struct KeyState { kv: [i64; 20], keystr: String, status: Option<String> }
    let mut ks: Vec<KeyState> = vec![];
    for key in &args.keys {
        let mut kv: [Option<i64>; 20] = fixed;
        let mut bad = None;
        for &(i, x) in key { if fixed[i].is_some() && fixed[i] != Some(x) { bad = Some("NOT_IN_CASE (key contradicts a fixed slot)".to_string()); } kv[i] = Some(x); }
        if (0..20).any(|i| kv[i].is_none()) { die("key does not assign every free index"); }
        let kv: [i64; 20] = std::array::from_fn(|i| kv[i].unwrap());
        let keystr = key.iter().map(|(i, x)| format!("\"n{}\":{}", i, x)).collect::<Vec<_>>().join(",");
        if bad.is_none() && !eq_aff.iter().all(|e| e.lin.iter().fold(e.c0, |s, &(v, c)| s + c * kv[v as usize]) == 0) { bad = Some("NOT_IN_CASE (case equations fail at the key)".into()); }
        if bad.is_none() {
            let tv = pat_at(&target, &kv);
            let cb = chart.as_bytes();
            if !(0..20).all(|i| if cb[i] == b'1' { tv[i] >= 1 } else { tv[i] <= 0 }) { bad = Some("NOT_IN_CHART (target at the key violates the chart guards)".into()); }
        }
        ks.push(KeyState { kv, keystr, status: bad });
    }
    let keypows = |kv: &[i64; 20], maxe: usize| -> Vec<Vec<Big>> { (0..20).map(|v| { let mut t = vec![Big::one()]; for e in 1..=maxe { let y = t[e - 1].mul_i64(kv[v]); t.push(y); } t }).collect() };
    // exceptions
    let mut exc_maxe = 1u32;
    for b in &exc { for s in b { let mut it = Terms::new(s); while let Some(t) = it.next() { for q in 0..t.nf { exc_maxe = exc_maxe.max(t.fe[q]); } } } }
    for k in ks.iter_mut() {
        if k.status.is_some() { continue; }
        let kp = keypows(&k.kv, exc_maxe as usize);
        let hit = exc.iter().any(|b| !b.is_empty() && b.iter().all(|s| eval_text_d(s, &kp).is_empty()));
        if hit && !args.ignore_exc { k.status = Some("ON_EXCEPTION_BRANCH (not covered by this rule)".into()); }
    }
    // rhs numerators / denominators at every regular key: one pass over the texts
    let te = Instant::now();
    let mut maxe = 0u32;
    for (_, n, d) in &rhs { for s in [Some(*n), *d].into_iter().flatten() { let mut it = Terms::new(s); while let Some(t) = it.next() { for k in 0..t.nf { if (t.fv[k] as usize) != DV { maxe = maxe.max(t.fe[k]); } } } } }
    let live: Vec<usize> = (0..ks.len()).filter(|&i| ks[i].status.is_none()).collect();
    let kpw: Vec<Vec<Vec<Big>>> = live.iter().map(|&i| keypows(&ks[i].kv, maxe as usize)).collect();
    let mut rnum: Vec<Vec<BigPoly>> = vec![vec![]; live.len()]; // [key][rhs]
    let mut rden: Vec<Vec<BigPoly>> = vec![vec![]; live.len()];
    let mut nmono = 0u64;
    for (_, n, d) in &rhs {
        let mut it = Terms::new(n); while it.next().is_some() { nmono += 1; }
        for (li, kp) in kpw.iter().enumerate() {
            rnum[li].push(eval_text_d(n, kp));
            rden[li].push(match d { Some(s) => eval_text_d(s, kp), None => vec![Big::one()] });
        }
    }
    let t_eval = te.elapsed().as_secs_f64();
    put!("rhs_evaluation", format!("{{\"numerator_monomials\":{},\"regular_keys\":{},\"seconds\":{:.3}}}", nmono, live.len(), t_eval));

    // ---------------------------------------------------------------- per key
    let mut base = Sha256::new(); base.update(&tline); base.update(b"|instance-span|");
    let mut key_json = vec![];
    let (mut n_proved, mut n_fail, mut n_other) = (0, 0, 0);
    let mut live_pos = 0;
    for ki in 0..ks.len() {
        let k = &ks[ki];
        if let Some(st) = &k.status {
            n_other += 1;
            key_json.push(format!("{{\"key\":{{{}}},\"status\":{}}}", k.keystr, jstr(st)));
            continue;
        }
        let li = live_pos; live_pos += 1;
        let tk = Instant::now();
        // irregular: an rhs denominator identically zero in d at the key
        if let Some(i) = rden[li].iter().position(|d| d.is_empty()) {
            n_other += 1;
            key_json.push(format!("{{\"key\":{{{}}},\"status\":{}}}", k.keystr, jstr(&format!("IRREGULAR_KEY (rhs denominator of term {} vanishes identically in d)", i))));
            continue;
        }
        let r = span_at_key(&k.kv, &target, &rhs, &rnum[li], &rden[li], &seeds, &recenter, &rows, &row_ids, &zero_masks, args, &base, &k.keystr);
        match r.0.as_str() {
            "EXACT_IN_SPAN_AT_KEY" => n_proved += 1,
            s if s.contains("NOT_IN_SPAN") || s.starts_with("FAIL") => n_fail += 1,
            _ => n_other += 1,
        }
        key_json.push(format!("{{\"key\":{{{}}},\"status\":{},{},\"seconds\":{:.3}}}", k.keystr, jstr(&r.0), r.1, tk.elapsed().as_secs_f64()));
    }
    put!("prime_range", jstr(PRIME_RANGE_NOTE));
    put!("statement", jstr("EXACT_IN_SPAN_AT_KEY: at that integer key, sum_i W_i(d) A_ic(d) den_c(d) = L(d) num_c(d) holds exactly in Z[d] on every column c of the instantiated seed rows and of the rule (A_i: ordinary IBP rows at seed+recenter, v_c = num_c/den_c the rule vector e_target - sum r_c e_c at the key, L != 0), hence the rule at the key is a Q(d)-combination of IBP rows at its seeds"));
    put!("keys", format!("[{}]", key_json.join(",")));
    let verdict = if args.keys.is_empty() { "NO_KEYS".to_string() } else if n_fail > 0 { "NOT_ALL_KEYS_IN_SPAN".to_string() } else if n_other == 0 { "EXACT_ALL_KEYS_IN_SPAN".to_string() } else { "EXACT_KEYS_IN_SPAN_PARTIAL".to_string() };
    put!("verdict", jstr(&verdict));
    put!("counts", format!("{{\"in_span\":{},\"not_in_span\":{},\"other\":{}}}", n_proved, n_fail, n_other));
    put!("timings_s", format!("{{\"load\":{:.3},\"rhs_eval\":{:.3},\"total\":{:.3}}}", t_load, t_eval, t_all.elapsed().as_secs_f64()));
    put!("peak_rss_mb", format!("{:.1}", peak_mb()));
    crate::stream::emit(&o)
}

#[allow(clippy::too_many_arguments)]
fn instantiate(kv: &[i64; 20], target: &Pat, rhs: &[(Pat, &[u8], Option<&[u8]>)], rnum: &[BigPoly], rden: &[BigPoly], seeds: &[Pat], recenter: &[i64], rows: &Rows, row_ids: &[i64], zero: &std::collections::HashSet<Vec<u8>>, drop_zero: bool) -> Inst {
    let mut colid: HashMap<Vec<i64>, u32> = HashMap::new();
    let mut cols: Vec<Vec<i64>> = vec![];
    let mut dropped: std::collections::HashSet<Vec<i64>> = Default::default();
    let is_zero_col = |c: &[i64]| -> bool { let mask: Vec<u8> = c.iter().map(|&x| if x > 0 { b'1' } else { b'0' }).collect(); zero.contains(&mask) };
    let mut cid = |c: Vec<i64>, colid: &mut HashMap<Vec<i64>, u32>, cols: &mut Vec<Vec<i64>>| -> Option<u32> {
        if drop_zero && is_zero_col(&c) { dropped.insert(c); return None; }
        Some(*colid.entry(c.clone()).or_insert_with(|| { cols.push(c); (cols.len() - 1) as u32 }))
    };
    // v_c = sum of the terms landing in column c, as one fraction num/den (den = product of the DISTINCT
    // denominators of that column's terms; usually one term per column)
    let mut vmap: BTreeMap<u32, Vec<(BigPoly, BigPoly)>> = BTreeMap::new();
    if let Some(c) = cid(pat_at(target, kv), &mut colid, &mut cols) { vmap.entry(c).or_default().push((vec![Big::one()], vec![Big::one()])); }
    for (i, (p, _, _)) in rhs.iter().enumerate() {
        let n: BigPoly = rnum[i].iter().map(|c| c.negated()).collect();
        if let Some(c) = cid(pat_at(p, kv), &mut colid, &mut cols) { vmap.entry(c).or_default().push((n, rden[i].clone())); }
    }
    let mut v: Vec<(u32, BigPoly, BigPoly)> = vec![];
    for (c, terms) in vmap {
        let mut dl: Vec<BigPoly> = vec![];
        for (_, d) in &terms { if !dl.contains(d) { dl.push(d.clone()); } }
        let mut den: BigPoly = vec![Big::one()];
        for d in &dl { den = bp_mul(&den, d); }
        let mut num: BigPoly = vec![];
        for (n, d) in &terms {
            let mut cof: BigPoly = vec![Big::one()];
            for e in &dl { if e != d { cof = bp_mul(&cof, e); } }
            bp_add_into(&mut num, &bp_mul(n, &cof));
        }
        if !num.is_empty() { v.push((c, num, den)); }
    }
    let t_deg = v.iter().map(|x| x.2.len().saturating_sub(1)).max().unwrap_or(0);
    // rows at the seeds
    let mut seen = std::collections::HashSet::new();
    let mut out_rows = vec![];
    let mut row_label = vec![];
    let mut seed_d_cols = std::collections::HashSet::new();
    for (si, s) in seeds.iter().enumerate() {
        let sg: Vec<i64> = pat_at(s, kv).iter().zip(recenter).map(|(a, b)| a + b).collect();
        if !seen.insert(sg.clone()) { continue; }
        for m in row_ids {
            let mut acc: BTreeMap<u32, (i64, i64)> = BTreeMap::new();
            for (off, a) in &rows[m] {
                let c: Vec<i64> = sg.iter().zip(off).map(|(x, t)| x + t).collect();
                let mut b = a.c0; let mut dc = 0;
                for &(var, k) in &a.lin { if var as usize == DV { dc += k; } else { b += k * sg[var as usize]; } }
                if b == 0 && dc == 0 { continue; }
                if dc != 0 { seed_d_cols.insert(c.clone()); }
                if let Some(id) = cid(c, &mut colid, &mut cols) { let e = acc.entry(id).or_insert((0, 0)); e.0 += b; e.1 += dc; }
            }
            let r: Vec<(u32, i64, i64)> = acc.into_iter().filter(|x| x.1 .0 != 0 || x.1 .1 != 0).map(|(c, (b, d))| (c, b, d)).collect();
            if !r.is_empty() { out_rows.push(r); row_label.push((si, *m)); }
        }
    }
    Inst { cols, rows: out_rows, row_label, v, t_deg, dropped_cols: dropped.len(), seeds: seen.len(), seed_cols_with_d: seed_d_cols.len() }
}

#[allow(clippy::too_many_arguments)]
fn span_at_key(kv: &[i64; 20], target: &Pat, rhs: &[(Pat, &[u8], Option<&[u8]>)], rnum: &[BigPoly], rden: &[BigPoly], seeds: &[Pat], recenter: &[i64], rows: &Rows, row_ids: &[i64], zero: &std::collections::HashSet<Vec<u8>>, args: &SpanArgs, base: &Sha256, keystr: &str) -> (String, String) {
    let ti = Instant::now();
    let inst = instantiate(kv, target, rhs, rnum, rden, seeds, recenter, rows, row_ids, zero, args.drop_zero_sectors);
    let t_inst = ti.elapsed().as_secs_f64();
    let ncol = inst.cols.len();
    let nrow = inst.rows.len();
    let dv = inst.v.iter().map(|x| x.1.len().saturating_sub(1)).max().unwrap_or(0);
    // helper note bound: weights have numerator degree <= D_N + D_T + S_d, denominator <= D_T + S_d (column
    // denominators combined); values needed <= sum + 1, plus 3 held-out checks
    let sd = inst.seed_cols_with_d;
    let ndistinct_den = { let mut dl: Vec<&BigPoly> = vec![]; for x in &inst.v { if !dl.contains(&&x.2) { dl.push(&x.2); } } dl.iter().map(|d| d.len().saturating_sub(1)).sum::<usize>() };
    let cap = args.max_points.unwrap_or(dv + 2 * ndistinct_den + 2 * sd + 2 + 3);
    let mut info = vec![
        format!("\"instance\":{{\"rows\":{},\"columns\":{},\"seeds\":{},\"seed_columns_with_d\":{},\"zero_sector_columns_dropped\":{},\"v_columns\":{},\"max_deg_den\":{},\"max_deg_num\":{},\"sum_deg_distinct_den\":{},\"point_cap_per_prime\":{},\"seconds\":{:.3}}}", nrow, ncol, inst.seeds, sd, inst.dropped_cols, inst.v.len(), inst.t_deg, dv, ndistinct_den, cap, t_inst),
    ];
    if inst.v.is_empty() { return ("EXACT_IN_SPAN_AT_KEY".into(), { info.push("\"note\":\"rule vector is zero at the key\"".into()); info.join(",") }); }
    // hash stream for this key
    let mut b = base.clone(); b.update(keystr.as_bytes());
    let mut stream = Stream::new(b);
    let mut used_primes: Vec<u64> = vec![];
    let mut next_prime = |stream: &mut Stream, used: &mut Vec<u64>| -> u64 { loop { let c = prime_candidate(stream.next()); if is_prime(c) && !used.contains(&c) { used.push(c); return c; } } };
    let draw_d = |stream: &mut Stream, p: u64| -> u64 { loop { let w = stream.next() >> 1; if w < p { return w; } } };
    // complexity order of columns: pivot on the most complex integral
    let ckey: Vec<(i64, i64, i64)> = inst.cols.iter().map(|c| complexity(c)).collect();
    let choose_col = |row: &[(u32, u64)]| -> u32 { row.iter().max_by(|a, b| ckey[a.0 as usize].cmp(&ckey[b.0 as usize]).then(inst.cols[a.0 as usize].cmp(&inst.cols[b.0 as usize]))).unwrap().0 };
    // ---------------- structure at (p0, d0): independent rows S, pivot columns P, membership test
    let ts = Instant::now();
    let p0 = next_prime(&mut stream, &mut used_primes);
    let m0 = Mont::new(p0);
    // v at a point mod p (None if a column denominator vanishes there)
    let v_at = |p: u64, d: u64| -> Option<Vec<(u32, u64)>> {
        let mut out = vec![];
        for (c, n, de) in &inst.v {
            let dn = mp_eval(&de.iter().map(|x| x.mod_u64(p)).collect(), d, p);
            if dn == 0 { return None; }
            out.push((*c, mm(mp_eval(&n.iter().map(|x| x.mod_u64(p)).collect(), d, p), minv(dn, p), p)));
        }
        Some(out)
    };
    let structure = |m: Mont, d: u64| -> (Vec<usize>, Vec<u32>, usize, Vec<u32>) {
        let p = m.p;
        let mut el = Elim::new(m, ncol);
        let mut s_rows = vec![];
        for (i, r) in inst.rows.iter().enumerate() {
            let eq: Vec<(u32, u64)> = r.iter().map(|&(c, bb, dc)| (c, m.to(ma(mi64(bb, p), mm(mi64(dc, p), d, p), p)))).filter(|x| x.1 != 0).collect();
            if el.add(&eq, 0, &choose_col).is_none() { s_rows.push(i); }
        }
        // residual of T v at d
        let vm = match v_at(p, d) { Some(x) => x, None => return (s_rows, el.piv_unk.clone(), usize::MAX, vec![]) };
        let veq: Vec<(u32, u64)> = vm.iter().map(|&(c, x)| (c, m.to(x))).filter(|x| x.1 != 0).collect();
        el.load(&veq);
        el.reduce(0);
        let res = el.take();
        (s_rows, el.piv_unk.clone(), res.len(), res.iter().map(|x| x.0).collect())
    };
    let (mut s_rows, mut piv_cols, mut res0, mut res_cols) = (vec![], vec![], usize::MAX, vec![]);
    for _ in 0..16 { let d0 = draw_d(&mut stream, p0); let r = structure(m0, d0); s_rows = r.0; piv_cols = r.1; res0 = r.2; res_cols = r.3; if res0 != usize::MAX { break; } }
    let rank = s_rows.len();
    let t_struct = ts.elapsed().as_secs_f64();
    info.push(format!("\"structure\":{{\"prime\":\"{}\",\"rank\":{},\"residual_nonzero_columns\":{},\"seconds\":{:.3}}}", p0, rank, res0, t_struct));
    let col_in_p: Vec<u32> = { let mut v = vec![u32::MAX; ncol]; for (k, &c) in piv_cols.iter().enumerate() { v[c as usize] = k as u32; } v };
    if res0 > 0 {
        // NON-MEMBERSHIP certificate (helper note item 4): residual nonzero in a free column c*; u = e_c* - (A_SP^-1 A_Sc*)
        // on the pivot columns, reconstructed over Q(d), checked EXACTLY: A u = 0 on ALL rows in Z[d], and
        // v(d0).u(d0) != 0 at an exact integer d0. Then v is not in the row space over Q(d) of these seed rows.
        let cstar = *res_cols.iter().filter(|&&c| col_in_p[c as usize] == u32::MAX).min_by_key(|&&c| (std::cmp::Reverse(ckey[c as usize]), inst.cols[c as usize].clone())).unwrap_or(&res_cols[0]);
        let solve_u = |m: Mont, d: u64| -> Option<Vec<u64>> {
            let p = m.p;
            // equations = independent rows: sum_k A[s,P_k] x_k = A[s,c*]
            let mut el = Elim::new(m, rank);
            let first = |row: &[(u32, u64)]| -> u32 { row[0].0 };
            let mut eqs: Vec<(Vec<(u32, u64)>, u64)> = vec![];
            for &ri in &s_rows {
                let mut eq = vec![]; let mut rhs = 0u64;
                for &(c, bb, dc) in &inst.rows[ri] {
                    let x = ma(mi64(bb, p), mm(mi64(dc, p), d, p), p);
                    if x == 0 { continue; }
                    if c == cstar { rhs = m.to(x); } else { let k = col_in_p[c as usize]; if k != u32::MAX { eq.push((k, m.to(x))); } }
                }
                eqs.push((eq, rhs));
            }
            eqs.sort_by_key(|e| e.0.len());
            for (eq, rhs) in &eqs { if el.add(eq, *rhs, &first).is_some() { return None; } }
            Some(el.solve().into_iter().map(|x| m.from(x)).collect())
        };
        let verify_u = |l: &BigPoly, w: &[BigPoly]| -> usize {
            // u_{P_k} = -W_k, u_{c*} = L; every row r: sum_k A[r,P_k] (-W_k) + A[r,c*] L == 0
            let mut bad = 0;
            for r in &inst.rows {
                let mut acc: BigPoly = vec![];
                for &(c, bb, dc) in r {
                    let (poly, sign) = if c == cstar { (l, 1i64) } else { let k = col_in_p[c as usize]; if k == u32::MAX { continue; } (&w[k as usize], -1i64) };
                    if poly.is_empty() { continue; }
                    let need = poly.len() + 1;
                    if acc.len() < need { acc.resize(need, Big::zero()); }
                    for (e, x) in poly.iter().enumerate() { if bb != 0 { acc[e].add(&x.mul_i64(bb * sign)); } if dc != 0 { acc[e + 1].add(&x.mul_i64(dc * sign)); } }
                }
                if acc.iter().any(|x| !x.is_zero()) { bad += 1; }
            }
            bad
        };
        let p1 = next_prime(&mut stream, &mut used_primes);
        match reconstruct(rank, cap, args.max_primes, p1, &mut stream, &mut used_primes, &solve_u, &verify_u) {
            Ok((lz, wz, frag)) => {
                // exact v(d0).u(d0) != 0 at the first integer d0 = 1, 2, ... with every column denominator nonzero
                let mut found = None;
                for d0 in 1i128..200 {
                    if inst.v.iter().any(|(_, _, de)| bp_eval_i128(de, d0).is_zero()) { continue; }
                    // sum_c num_c(d0)/den_c(d0) * u_c(d0) as one fraction N/D
                    let (mut nn, mut dd) = (Big::zero(), Big::one());
                    for (c, num, de) in &inst.v {
                        let uc = if *c == cstar { bp_eval_i128(&lz, d0) } else { let k = col_in_p[*c as usize]; if k == u32::MAX { continue; } bp_eval_i128(&wz[k as usize], d0).negated() };
                        if uc.is_zero() { continue; }
                        let a = bp_eval_i128(num, d0).mul(&uc); let b = bp_eval_i128(de, d0);
                        let mut t = nn.mul(&b); t.add(&a.mul(&dd)); nn = t; dd = dd.mul(&b);
                        let g = Big::gcd(&nn, &dd); if !g.is_zero() && !g.is_one() { nn = nn.div_exact(&g); dd = dd.div_exact(&g); }
                    }
                    if !nn.is_zero() { if dd.neg { nn = nn.negated(); dd = dd.negated(); } found = Some((d0, nn, dd)); break; }
                }
                info.push(format!("\"nonmembership\":{{\"free_column\":{},\"reconstruction\":{},\"u_support\":{}}}", jstr(&format!("{:?}", inst.cols[cstar as usize])), frag, 1 + wz.iter().filter(|x| !x.is_empty()).count()));
                match found {
                    Some((d0, nn, dd)) => {
                        info.push(format!("\"witness\":{{\"d0\":{},\"v_dot_u\":\"{}/{}\"}}", d0, nn.to_dec(), dd.to_dec()));
                        ("EXACT_NOT_IN_SPAN_AT_KEY (exact certificate u(d): A u = 0 on all rows in Z[d], v(d0).u(d0) != 0; meaning: NOT derivable from THESE seeds at this key)".into(), info.join(","))
                    }
                    None => ("UNDECIDED (kernel vector verified but v.u vanished at d0 = 1..199)".into(), info.join(",")),
                }
            }
            Err((st, frag)) => { info.push(format!("\"nonmembership\":{{\"reconstruction\":{}}}", frag)); (format!("NOT_IN_SPAN_MODULAR (residual nonzero; exact certificate not obtained: {})", st), info.join(",")) }
        }
    } else {
    // ---------------- square solves at points; reconstruction per prime; CRT; exact check
    // transposed square system: equations = pivot columns, unknowns = independent rows
    let solve_at = |m: Mont, d: u64| -> Option<Vec<u64>> {
        let p = m.p;
        let mut eqs: Vec<Vec<(u32, u64)>> = vec![vec![]; rank];
        for (si, &ri) in s_rows.iter().enumerate() {
            for &(c, bb, dc) in &inst.rows[ri] {
                let k = col_in_p[c as usize];
                if k == u32::MAX { continue; }
                let x = ma(mi64(bb, p), mm(mi64(dc, p), d, p), p);
                if x != 0 { eqs[k as usize].push((si as u32, m.to(x))); }
            }
        }
        let mut rhsv = vec![0u64; rank];
        for (c, x) in v_at(p, d)? { let k = col_in_p[c as usize]; if k != u32::MAX { rhsv[k as usize] = m.to(x); } }
        let mut order: Vec<usize> = (0..rank).collect();
        order.sort_by_key(|&i| eqs[i].len());
        let mut el = Elim::new(m, rank);
        let first = |row: &[(u32, u64)]| -> u32 { row[0].0 };
        for i in order {
            if let Some(_) = el.add(&eqs[i], rhsv[i], &first) { return None; } // singular at this point
        }
        Some(el.solve().into_iter().map(|x| m.from(x)).collect())
    };
    let verify = |l: &BigPoly, w: &[BigPoly]| -> usize { verify_exact(&inst, &s_rows, l, w) };
    match reconstruct(rank, cap, args.max_primes, p0, &mut stream, &mut used_primes, &solve_at, &verify) {
        Ok((lz, wz, frag)) => {
            info.push(format!("\"reconstruction\":{},\"nonzero_weights\":{},\"deg_L\":{}", frag, wz.iter().filter(|x| !x.is_empty()).count(), lz.len() - 1));
            if let Some(dir) = &args.emit_dir {
                let ps = |p: &BigPoly| format!("[{}]", p.iter().map(|c| format!("\"{}\"", c.to_dec())).collect::<Vec<_>>().join(","));
                let ws: Vec<String> = s_rows.iter().zip(&wz).filter(|(_, w)| !w.is_empty()).map(|(&ri, w)| format!("[{},{},{}]", inst.row_label[ri].0, inst.row_label[ri].1, ps(w))).collect();
                let kv_s = kv.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("_");
                let body = format!("{{\"schema\":\"rustred.proof-kernel.span-cert.v1\",\"key\":{:?},\"statement\":\"sum over rows of (W/L) * IBP row(seed+recenter, m) = e_target - sum r_c e_c at the key, exactly in Q(d)\",\"L\":{},\"weights\":[{}],\"weights_format\":\"[seed index in the record, source row m, W coefficients ascending in d]\"}}", kv, ps(&lz), ws.join(","));
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::write(format!("{}/cert_{}.json", dir, kv_s), body);
            }
            ("EXACT_IN_SPAN_AT_KEY".into(), info.join(","))
        }
        Err((st, frag)) => { info.push(format!("\"reconstruction\":{}", frag)); (st, info.join(",")) }
    }
    }
}

/// Reconstruct a vector of rational functions in d from modular solves: per prime, values at hash-derived d (early
/// termination, 3 held-out values, cap `cap`), common monic denominator L and polynomials W_i = L w_i mod p; the
/// reduced degree pattern must agree across primes (an odd prime is dropped); CRT + rational number reconstruction on
/// all but the newest prime, checked against the newest; then `verify` (EXACT, in Z[d]) decides. Returns integer
/// polynomials (L, W) with verify(L, W) == 0, or (status, log).
#[allow(clippy::too_many_arguments)]
fn reconstruct(dim: usize, cap: usize, max_primes: usize, first: u64, stream: &mut Stream, used: &mut Vec<u64>, solve: &dyn Fn(Mont, u64) -> Option<Vec<u64>>, verify: &dyn Fn(&BigPoly, &[BigPoly]) -> usize) -> Result<(BigPoly, Vec<BigPoly>, String), (String, String)> {
    let next_prime = |stream: &mut Stream, used: &mut Vec<u64>| -> u64 { loop { let c = prime_candidate(stream.next()); if is_prime(c) && !used.contains(&c) { used.push(c); return c; } } };
    let draw_d = |stream: &mut Stream, p: u64| -> u64 { loop { let w = stream.next() >> 1; if w < p { return w; } } };
    type PrimeImage = (u64, MP, Vec<MP>);
    let mut images: Vec<PrimeImage> = vec![];
    let mut prime_log = vec![];
    let (mut solve_secs, mut recon_secs) = (0.0, 0.0);
    let (mut npoints_total, mut skipped_points) = (0usize, 0usize);
    let mut verify_log: Vec<String> = vec![];
    let mut first_prime = Some(first);
    let mut start_points = 8usize;
    let log = |prime_log: &Vec<String>, verify_log: &Vec<String>, n: usize, sk: usize, ss: f64, rs: f64| format!("{{\"primes\":[{}],\"points_solved\":{},\"singular_points_skipped\":{},\"solve_seconds\":{:.3},\"reconstruct_seconds\":{:.3},\"exact_checks\":[{}]}}", prime_log.join(","), n, sk, ss, rs, verify_log.join(","));
    while images.len() < max_primes {
        let p = match first_prime.take() { Some(p) => p, None => next_prime(stream, used) };
        let m = Mont::new(p);
        let mut xs: Vec<u64> = vec![];
        let mut ys: Vec<Vec<u64>> = vec![];
        let mut want = start_points.min(cap);
        let mut got: Option<(MP, Vec<MP>)> = None;
        loop {
            while xs.len() < want {
                let d = draw_d(stream, p);
                if xs.contains(&d) { continue; }
                let t = Instant::now();
                let s = solve(m, d);
                solve_secs += t.elapsed().as_secs_f64();
                match s { Some(w) => { xs.push(d); ys.push(w); npoints_total += 1; } None => { skipped_points += 1; if skipped_points > 64 + 4 * cap { return Err(("UNDECIDED (too many singular points)".into(), log(&prime_log, &verify_log, npoints_total, skipped_points, solve_secs, recon_secs))); } } }
            }
            let tr = Instant::now();
            let n = xs.len() - 3;
            // (a) common denominator from a pseudo-random combination (helper h15), (b) q*w_i as polynomials checked
            // on the held-out values, (c) fallback per-component rational reconstruction + lcm
            let mut lam_state = p ^ 0x9e37_79b9_7f4a_7c15;
            let lam: Vec<u64> = (0..dim).map(|_| { lam_state = lam_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (lam_state >> 1) % p }).collect();
            let z: Vec<u64> = ys.iter().map(|w| w.iter().zip(&lam).fold(0u64, |a, (&x, &l)| ma(a, mm(x, l, p), p))).collect();
            let mut res: Option<(MP, Vec<MP>)> = None;
            if let Some((_, q)) = mp_ratrec(&xs[..n], &z[..n], &xs[n..], &z[n..], p) {
                let qv: Vec<u64> = xs.iter().map(|&x| mp_eval(&q, x, p)).collect();
                if qv.iter().all(|&x| x != 0) {
                    let mut w = vec![];
                    let mut ok = true;
                    for i in 0..dim {
                        let col: Vec<u64> = ys.iter().zip(&qv).map(|(wv, &qq)| mm(wv[i], qq, p)).collect();
                        let poly = mp_interp(&xs[..n], &col[..n], p);
                        if (n..xs.len()).any(|j| mp_eval(&poly, xs[j], p) != col[j]) { ok = false; break; }
                        w.push(poly);
                    }
                    if ok { res = Some((q, w)); }
                }
            }
            if res.is_none() {
                let mut ok = true;
                let mut nums = vec![]; let mut dens = vec![];
                for i in 0..dim {
                    let col: Vec<u64> = ys.iter().map(|w| w[i]).collect();
                    match mp_ratrec(&xs[..n], &col[..n], &xs[n..], &col[n..], p) { Some((a, b)) => { nums.push(a); dens.push(b); } None => { ok = false; break; } }
                }
                if ok {
                    let mut l: MP = vec![1];
                    for d in &dens { if d.len() > 1 && d != &l { let g = mp_gcd(&l, d, p); l = mp_divrem(&mp_mul(&l, d, p), &g, p).0; } }
                    let w: Vec<MP> = nums.iter().zip(&dens).map(|(a, b)| mp_mul(a, &mp_divrem(&l, b, p).0, p)).collect();
                    res = Some((l, w));
                }
            }
            recon_secs += tr.elapsed().as_secs_f64();
            if let Some(r) = res { start_points = xs.len(); got = Some(r); break; }
            if want >= cap { break; }
            want = (want * 2).min(cap);
        }
        let (l, w) = match got { Some(x) => x, None => return Err(("UNDECIDED (reconstruction did not terminate within the point cap)".into(), log(&prime_log, &verify_log, npoints_total, skipped_points, solve_secs, recon_secs))) };
        prime_log.push(format!("{{\"prime\":\"{}\",\"points\":{},\"deg_L\":{},\"max_deg_W\":{}}}", p, xs.len(), l.len() - 1, w.iter().map(|x| x.len()).max().unwrap_or(0).saturating_sub(1)));
        images.push((p, l, w));
        let shape = |im: &PrimeImage| -> (usize, Vec<usize>) { (im.1.len(), im.2.iter().map(|x| x.len()).collect()) };
        if shape(images.last().unwrap()) != shape(&images[0]) { images.pop(); continue; }
        if images.len() < 2 { continue; }
        let tr = Instant::now();
        let use_n = images.len() - 1;
        let rec = crt_ratrec(&images[..use_n]);
        let checked = rec.as_ref().map(|(l, w, _)| agrees(&images[use_n], l, w)).unwrap_or(false);
        recon_secs += tr.elapsed().as_secs_f64();
        if !checked { continue; }
        let (lq, wq, bits) = rec.unwrap();
        let tv = Instant::now();
        let (lz, wz) = integerise(&lq, &wq);
        let bad = verify(&lz, &wz);
        verify_log.push(format!("{{\"primes_in_crt\":{},\"failed_columns_or_rows\":{},\"max_coefficient_bits\":{},\"seconds\":{:.3}}}", use_n, bad, bits, tv.elapsed().as_secs_f64()));
        if bad == 0 && !lz.is_empty() {
            let frag = format!("{{\"deg_L\":{},\"max_deg_W\":{},\"log\":{}}}", lz.len() - 1, wz.iter().map(|x| x.len()).max().unwrap_or(0).saturating_sub(1), log(&prime_log, &verify_log, npoints_total, skipped_points, solve_secs, recon_secs));
            return Ok((lz, wz, frag));
        }
    }
    Err(("UNDECIDED (no verified reconstruction within max_primes)".into(), log(&prime_log, &verify_log, npoints_total, skipped_points, solve_secs, recon_secs)))
}

/// rational number reconstruction (Wang): a mod m -> n/d with |n|, d <= sqrt(m/2)
fn ratrec_big(a: &Big, m: &Big) -> Option<(Big, Big)> {
    // bound^2 * 2 <= m
    let bound_bits = (m.bits().saturating_sub(1)) / 2;
    let small = |x: &Big| x.bits() <= bound_bits;
    let (mut r0, mut r1) = (m.clone(), a.clone());
    let (mut t0, mut t1) = (Big::zero(), Big::one());
    while !small(&r1) {
        if r1.is_zero() { return None; }
        let (q, r) = r0.divrem(&r1);
        let mut t = t0.clone(); t.sub(&q.mul(&t1));
        r0 = std::mem::replace(&mut r1, r);
        t0 = std::mem::replace(&mut t1, t);
    }
    if t1.is_zero() || !small(&t1) { return None; }
    if !Big::gcd(&r1, &t1).is_one() { return None; }
    if t1.neg { Some((r1.negated(), t1.negated())) } else { Some((r1, t1)) }
}
type QPoly = Vec<(Big, Big)>; // rational coefficients (num, den>0)
fn crt_ratrec(ims: &[(u64, MP, Vec<MP>)]) -> Option<(QPoly, Vec<QPoly>, u64)> {
    // modulus
    let mut m = Big::one();
    for im in ims { m = m.mul(&Big::from_u64(im.0)); }
    let mut maxbits = 0u64;
    let mut rec = |get: &dyn Fn(&(u64, MP, Vec<MP>)) -> u64| -> Option<(Big, Big)> {
        // Garner-style incremental CRT
        let mut x = Big::zero(); let mut mm_ = Big::one();
        for im in ims {
            let p = im.0;
            let r = get(im);
            let xm = x.mod_u64(p);
            let mi = mm_.mod_u64(p);
            let t = mm(ms(r, xm, p), minv(mi, p), p);
            x.add(&mm_.mul(&Big::from_u64(t)));
            mm_ = mm_.mul(&Big::from_u64(p));
        }
        let (n, d) = ratrec_big(&x, &m)?;
        maxbits = maxbits.max(n.bits()).max(d.bits());
        Some((n, d))
    };
    let deg_l = ims[0].1.len();
    let mut l = vec![];
    for k in 0..deg_l { l.push(rec(&|im| *im.1.get(k).unwrap_or(&0))?); }
    let mut w = vec![];
    for i in 0..ims[0].2.len() {
        let mut wi = vec![];
        for k in 0..ims[0].2[i].len() { wi.push(rec(&|im| *im.2[i].get(k).unwrap_or(&0))?); }
        w.push(wi);
    }
    Some((l, w, maxbits))
}
fn agrees(im: &(u64, MP, Vec<MP>), l: &QPoly, w: &[QPoly]) -> bool {
    let p = im.0;
    let red = |q: &QPoly| -> Option<MP> { let mut o: MP = vec![]; for (n, d) in q { let dm = d.mod_u64(p); if dm == 0 { return None; } o.push(mm(n.mod_u64(p), minv(dm, p), p)); } mp_trim(&mut o); Some(o) };
    match red(l) { Some(x) if x == im.1 => {}, _ => return false }
    w.iter().zip(&im.2).all(|(q, x)| red(q).as_ref() == Some(x))
}
fn integerise(l: &QPoly, w: &[QPoly]) -> (BigPoly, Vec<BigPoly>) {
    let mut s = Big::one();
    for (_, d) in l.iter().chain(w.iter().flatten()) { if !d.is_one() { s = Big::lcm(&s, d); } }
    let conv = |q: &QPoly| -> BigPoly { let mut o: BigPoly = q.iter().map(|(n, d)| n.mul(&s.div_exact(d))).collect(); bp_trim(&mut o); o };
    (conv(l), w.iter().map(conv).collect())
}
/// exact check: for every column c, sum_i W_i(d) (b_ic + c_ic d) den_c(d) - L(d) num_c(d) == 0 in Z[d]
/// (den_c = 1, num_c = 0 off the support of v); returns the number of failing columns
fn verify_exact(inst: &Inst, s_rows: &[usize], l: &BigPoly, w: &[BigPoly]) -> usize {
    let ncol = inst.cols.len();
    let mut acc: Vec<BigPoly> = vec![vec![]; ncol];
    for (si, &ri) in s_rows.iter().enumerate() {
        let wi = &w[si];
        if wi.is_empty() { continue; }
        for &(c, b, dc) in &inst.rows[ri] {
            // wi * (b + dc d)
            let a = &mut acc[c as usize];
            let need = wi.len() + if dc != 0 { 1 } else { 0 };
            if a.len() < need { a.resize(need, Big::zero()); }
            for (k, x) in wi.iter().enumerate() {
                if b != 0 { a[k].add(&x.mul_i64(b)); }
                if dc != 0 { a[k + 1].add(&x.mul_i64(dc)); }
            }
        }
    }
    // columns of v: sum_i W_i a_ic * den_c - L num_c == 0; other columns: sum_i W_i a_ic == 0
    let mut bad = 0;
    let mut in_v = vec![false; ncol];
    for (c, num, den) in &inst.v {
        in_v[*c as usize] = true;
        let mut t = bp_mul(&acc[*c as usize], den);
        let u = bp_mul(l, num);
        if t.len() < u.len() { t.resize(u.len(), Big::zero()); }
        for (k, x) in u.iter().enumerate() { t[k].sub(x); }
        if t.iter().any(|x| !x.is_zero()) { bad += 1; }
    }
    bad + acc.iter().enumerate().filter(|(c, a)| !in_v[*c] && a.iter().any(|x| !x.is_zero())).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratrec_univariate_mod_p() {
        let p = 2305843009213693951u64; // test only
        // f(x) = (3x^2 + 1)/(x - 5)
        let f = |x: u64| mm(ma(mm(3, mm(x, x, p), p), 1, p), minv(ms(x, 5, p), p), p);
        let xs: Vec<u64> = (10..20).collect();
        let ys: Vec<u64> = xs.iter().map(|&x| f(x)).collect();
        let (n, d) = mp_ratrec(&xs[..8], &ys[..8], &xs[8..], &ys[8..], p).unwrap();
        assert_eq!(d, vec![p - 5, 1]);
        assert_eq!(n, vec![1, 0, 3]);
    }
    #[test]
    fn rational_number_reconstruction() {
        let m = Big::from_u64(2305843009213693951).mul(&Big::from_u64(2305843009213693921));
        // -7/12 mod m
        let inv12 = { // brute: m mod 12 known; use extended euclid via ratrec of 1/12
            let (_, _) = (0, 0); 0 };
        let _ = inv12;
        let a = { let mut x = Big::zero(); let p1 = 2305843009213693951u64; let p2 = 2305843009213693921u64;
            let r1 = mm(p1 - 7, minv(12, p1), p1); let r2 = mm(p2 - 7, minv(12, p2), p2);
            let t = mm(ms(r2, r1 % p2, p2), minv(p1 % p2, p2), p2); x.add(&Big::from_u64(r1)); x.add(&Big::from_u64(p1).mul(&Big::from_u64(t))); x };
        let (n, d) = ratrec_big(&a, &m).unwrap();
        assert_eq!((n.to_dec(), d.to_dec()), ("-7".to_string(), "12".to_string()));
    }
}
