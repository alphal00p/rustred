//! # tadic (Symbolica-backed): the system pole order p* by elimination over the local ring at d = d0 + t
//!
//! Implements `rustred_campaign_core::tadic::PoleOrderOracle`. Mathematics, proofs and the limits of screens:
//! WS/notes/implementer.tadic.md ("THE MATHEMATICAL OBJECT"); interface WS/plan/auto-campaign/interfaces/tadic.md.
//!
//! Two phases, both forward eliminations (no back-substitution):
//! 1. F-elimination of the UPPER columns (Q(d)-linear; fraction-free over Z[t] with exact cofactors and row
//!    contents): rows left without an upper entry generate W_0 = {row space, upper part 0}; the upper pivot rows are
//!    discarded.
//! 2. Valuation-aware echelon over R = Q[t]_(t) on the columns of interest S (K and the other lower columns are
//!    CARRIED, true value = poly / t^shift): each new row is reduced by the earlier pivots (creation order), its
//!    content divided out, then divided by t^e (e = its minimal valuation on S: the SATURATION step), and a column
//!    where it is a unit becomes its pivot. The pivot rows h_k (unit at c_k, zero at every earlier pivot) are an
//!    R-basis of the saturated lattice of pi_S(V) (notes, CLAIM 2, triangular form); p* = max(0, min_k v((h_k)_K)),
//!    0 if some row has S-part 0 and K-part != 0 (I(K) needs no column of interest), "no relation" if K is 0 on all.
//!
//! Arithmetic: every operation is a Symbolica polynomial operation (`MultivariatePolynomial` in one variable t over
//! `IntegerRing` (exact) or `Zp64` (screen, truncated with `mod_var`); `gcd`, `try_div`, `shift_var`). OWN code (L11,
//! verified missing in Symbolica: no Hermite / Smith form, no saturation, no valuation-aware elimination) = the pivot
//! logic and the precision bookkeeping of the truncated screen. Check mode: [`check`] (independent route through
//! numerica `Matrix` over Q(d): row-reduced W_0, relation membership, dual certificate).

pub mod check;
pub mod lattice;

use std::sync::Arc;
use std::time::Instant;

use rustred_campaign_core::tadic::{
    Arith, ExactSystem, Interest, PStar, PoleOrderOracle, PoleOrderResult, PoleQuery, Profile,
    RankData, Rat, RelTerm, Relation, TADIC_VERSION, Verdict,
};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::Ring;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;

/// Z[t] (exact mode).
pub type ZP = MultivariatePolynomial<IntegerRing, u16>;
/// Z/p[t] (screen mode).
pub type FP = MultivariatePolynomial<Zp64, u16>;

/// The variable map `[t]` (namespace `rustred_tadic`).
pub fn t_vars() -> Arc<Vec<PolyVariable>> {
    let ns = NamespacedSymbol::try_parse("rustred_tadic::t").expect("symbol name");
    let s = SymbolBuilder::new(ns).build().expect("symbol");
    Arc::new(vec![PolyVariable::Symbol(s)])
}

/// The arithmetic of one mode (all operations are Symbolica's; `prec` = truncation of a row, `None` = exact).
trait Ar {
    type P: Clone;
    fn is_zero(&self, p: &Self::P) -> bool;
    /// lowest exponent (the valuation at t = 0); `None` for 0
    fn val(&self, p: &Self::P) -> Option<u32>;
    fn nterms(&self, p: &Self::P) -> usize;
    fn degree(&self, p: &Self::P) -> u32;
    /// the integer affine form `c0 + c1 t`
    fn affine(&self, c0: &Integer, c1: &Integer) -> Self::P;
    fn mul(&self, a: &Self::P, b: &Self::P) -> Self::P;
    fn sub(&self, a: &Self::P, b: &Self::P) -> Self::P;
    fn neg(&self, a: &Self::P) -> Self::P;
    /// exact division by t^e (caller guarantees v >= e)
    fn div_t(&self, p: &Self::P, e: u32) -> Self::P;
    fn mul_t(&self, p: &Self::P, e: u32) -> Self::P;
    /// (a/g, b/g) for a common factor g (exact: the gcd; screen: g = 1)
    fn cofactors(&self, a: &Self::P, b: &Self::P) -> (Self::P, Self::P);
    /// divide all entries by a common factor (exact: their gcd; screen: nothing)
    fn normalize(&self, entries: &mut [&mut Self::P]);
    fn truncate(&self, p: Self::P, prec: Option<u32>) -> Self::P;
}

struct ExactZ {
    vars: Arc<Vec<PolyVariable>>,
}

impl Ar for ExactZ {
    type P = ZP;
    fn is_zero(&self, p: &ZP) -> bool {
        p.is_zero()
    }
    fn val(&self, p: &ZP) -> Option<u32> {
        if p.is_zero() {
            None
        } else {
            Some(p.exponents(0)[0] as u32)
        }
    }
    fn nterms(&self, p: &ZP) -> usize {
        p.nterms()
    }
    fn degree(&self, p: &ZP) -> u32 {
        if p.is_zero() {
            0
        } else {
            p.last_exponents()[0] as u32
        }
    }
    fn affine(&self, c0: &Integer, c1: &Integer) -> ZP {
        let mut p: ZP = MultivariatePolynomial::new(&Z, Some(2), self.vars.clone());
        if !c0.is_zero() {
            p.append_monomial(c0.clone(), &[0]);
        }
        if !c1.is_zero() {
            p.append_monomial(c1.clone(), &[1]);
        }
        p
    }
    fn mul(&self, a: &ZP, b: &ZP) -> ZP {
        a * b
    }
    fn sub(&self, a: &ZP, b: &ZP) -> ZP {
        a - b
    }
    fn neg(&self, a: &ZP) -> ZP {
        -a.clone()
    }
    fn div_t(&self, p: &ZP, e: u32) -> ZP {
        if e == 0 {
            return p.clone();
        }
        let mut q = p.clone();
        for x in q.exponents_iter_mut() {
            x[0] -= e as u16;
        }
        q
    }
    fn mul_t(&self, p: &ZP, e: u32) -> ZP {
        if e == 0 {
            p.clone()
        } else {
            p.clone().mul_exp(&[e as u16])
        }
    }
    fn cofactors(&self, a: &ZP, b: &ZP) -> (ZP, ZP) {
        let g = a.gcd(b);
        if g.is_one() {
            return (a.clone(), b.clone());
        }
        (
            a.try_div(&g).expect("gcd divides"),
            b.try_div(&g).expect("gcd divides"),
        )
    }
    fn normalize(&self, entries: &mut [&mut ZP]) {
        if entries.is_empty() {
            return;
        }
        let mut g = entries[0].clone();
        for e in entries.iter().skip(1) {
            if g.is_one() {
                return;
            }
            g = g.gcd(e);
        }
        if g.is_one() || g.is_zero() {
            return;
        }
        for e in entries.iter_mut() {
            **e = e.try_div(&g).expect("gcd divides");
        }
    }
    fn truncate(&self, p: ZP, _prec: Option<u32>) -> ZP {
        p
    }
}

struct TruncP {
    vars: Arc<Vec<PolyVariable>>,
    field: Zp64,
}

impl Ar for TruncP {
    type P = FP;
    fn is_zero(&self, p: &FP) -> bool {
        p.is_zero()
    }
    fn val(&self, p: &FP) -> Option<u32> {
        if p.is_zero() {
            None
        } else {
            Some(p.exponents(0)[0] as u32)
        }
    }
    fn nterms(&self, p: &FP) -> usize {
        p.nterms()
    }
    fn degree(&self, p: &FP) -> u32 {
        if p.is_zero() {
            0
        } else {
            p.last_exponents()[0] as u32
        }
    }
    fn affine(&self, c0: &Integer, c1: &Integer) -> FP {
        let mut p: FP = MultivariatePolynomial::new(&self.field, Some(2), self.vars.clone());
        let m = |i: &Integer| {
            let pr = Integer::from(self.field.get_prime());
            let mut r = i % &pr;
            if r < 0 {
                r += &pr;
            }
            self.field.to_element(u64::try_from(r).expect("residue"))
        };
        let a = m(c0);
        let b = m(c1);
        if !self.field.is_zero(&a) {
            p.append_monomial(a, &[0]);
        }
        if !self.field.is_zero(&b) {
            p.append_monomial(b, &[1]);
        }
        p
    }
    fn mul(&self, a: &FP, b: &FP) -> FP {
        a * b
    }
    fn sub(&self, a: &FP, b: &FP) -> FP {
        a - b
    }
    fn neg(&self, a: &FP) -> FP {
        -a.clone()
    }
    fn div_t(&self, p: &FP, e: u32) -> FP {
        if e == 0 {
            return p.clone();
        }
        let mut q = p.clone();
        for x in q.exponents_iter_mut() {
            x[0] -= e as u16;
        }
        q
    }
    fn mul_t(&self, p: &FP, e: u32) -> FP {
        if e == 0 {
            p.clone()
        } else {
            p.clone().mul_exp(&[e as u16])
        }
    }
    fn cofactors(&self, a: &FP, b: &FP) -> (FP, FP) {
        (a.clone(), b.clone())
    }
    fn normalize(&self, _entries: &mut [&mut FP]) {}
    fn truncate(&self, p: FP, prec: Option<u32>) -> FP {
        match prec {
            Some(n) => p.mod_var(0, n as u16),
            None => p,
        }
    }
}

type Ent<P> = Vec<(u32, P)>;
/// A phase-1 row with its truncation (None = exact).
type PRow<P> = (Ent<P>, Option<u32>, Tr);
/// Seed trace of a row: bitset over the system's seed labels (which seeds' rows it combines).
type Tr = Vec<u64>;

fn tr_or(a: &mut Tr, b: &Tr) {
    if a.len() < b.len() {
        a.resize(b.len(), 0);
    }
    for (x, y) in a.iter_mut().zip(b) {
        *x |= *y;
    }
}

fn tr_bit(i: u32) -> Tr {
    let mut t = vec![0u64; i as usize / 64 + 1];
    t[i as usize / 64] |= 1u64 << (i % 64);
    t
}

fn tr_list(t: &Tr) -> Vec<u32> {
    (0..t.len() * 64)
        .filter(|&i| t[i / 64] >> (i % 64) & 1 == 1)
        .map(|i| i as u32)
        .collect()
}
/// The check mode's view of one phase-2 basis row: (pivot, interest entries, carried entries, shift).
pub(crate) type BasisRowZ = (u32, Vec<(u32, ZP)>, Vec<(u32, ZP)>, u32);

/// out = a*x - b*y on sorted sparse vectors (truncated to `prec`, zeros dropped).
fn comb<A: Ar>(
    ar: &A,
    a: &A::P,
    x: &Ent<A::P>,
    b: &A::P,
    y: &Ent<A::P>,
    prec: Option<u32>,
) -> Ent<A::P> {
    let mut out = Vec::with_capacity(x.len() + y.len());
    let (mut i, mut j) = (0, 0);
    while i < x.len() || j < y.len() {
        let v = if j >= y.len() || (i < x.len() && x[i].0 < y[j].0) {
            let v = (x[i].0, ar.mul(a, &x[i].1));
            i += 1;
            v
        } else if i >= x.len() || y[j].0 < x[i].0 {
            let v = (y[j].0, ar.neg(&ar.mul(b, &y[j].1)));
            j += 1;
            v
        } else {
            let v = (x[i].0, ar.sub(&ar.mul(a, &x[i].1), &ar.mul(b, &y[j].1)));
            i += 1;
            j += 1;
            v
        };
        let p = ar.truncate(v.1, prec);
        if !ar.is_zero(&p) {
            out.push((v.0, p));
        }
    }
    out
}

fn min_prec(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) => Some(x.min(y)),
    }
}

/// A phase-2 row: `s` = entries on the columns of interest (integral), `c` = carried columns (K and the other lower
/// columns) with true value `poly / t^shift`.
#[derive(Clone)]
struct Row2<P> {
    s: Ent<P>,
    c: Ent<P>,
    shift: u32,
    prec: Option<u32>,
    tr: Tr,
}

struct Stop(String);

struct Budgeter<'a> {
    q: &'a PoleQuery<'a>,
    t0: Instant,
    ops: u64,
}

impl Budgeter<'_> {
    fn tick(&mut self, terms: usize, degree: u32) -> Result<(), Stop> {
        self.ops += 1;
        if let Some(m) = self.q.budget.max_terms
            && terms > m
        {
            return Err(Stop(format!("budget max_terms {m} exceeded ({terms})")));
        }
        if let Some(m) = self.q.budget.max_degree
            && degree > m
        {
            return Err(Stop(format!("budget max_degree {m} exceeded ({degree})")));
        }
        if self.ops.is_multiple_of(64)
            && let Some(s) = self.q.budget.max_seconds
            && self.t0.elapsed().as_secs_f64() > s
        {
            return Err(Stop(format!(
                "budget max_seconds {s} exceeded (time decision)"
            )));
        }
        Ok(())
    }
}

fn ent_terms<A: Ar>(ar: &A, e: &Ent<A::P>) -> (usize, u32) {
    let mut t = 0;
    let mut d = 0;
    for (_, p) in e {
        t += ar.nterms(p);
        d = d.max(ar.degree(p));
    }
    (t, d)
}

/// Column roles at query time.
struct Layout {
    /// position of every system column: upper columns first (0..n_upper), then interest, then carried
    pos: Vec<u32>,
    n_upper: u32,
    n_interest: u32,
    target_pos: Option<u32>,
    /// column index (into the system) of every position
    col_of_pos: Vec<u32>,
}

fn layout(sys: &ExactSystem, q: &PoleQuery<'_>) -> Layout {
    let interest_keys: Option<std::collections::HashSet<&Vec<i64>>> = match &q.interest {
        Interest::AllLower => None,
        Interest::Keys(k) => Some(k.iter().collect()),
    };
    let mut upper = Vec::new();
    let mut interest = Vec::new();
    let mut carried = Vec::new();
    let mut target_col = None;
    for (i, k) in sys.columns.iter().enumerate() {
        if k.as_slice() == q.target {
            target_col = Some(i as u32);
            carried.push(i as u32);
        } else if (q.lower)(k) {
            if interest_keys.as_ref().is_none_or(|s| s.contains(k)) {
                interest.push(i as u32);
            } else {
                carried.push(i as u32);
            }
        } else {
            upper.push(i as u32);
        }
    }
    // upper columns MOST COMPLEX FIRST (Laporta-like: a row's leading column is its most complex unknown, so a
    // reduction only brings in simpler ones): generic key weight = (number of positive indices, sum of |index|, key),
    // descending. Only the cost depends on this order, never p* (an invariant of the row space).
    let weight = |c: &u32| {
        let k = &sys.columns[*c as usize];
        (
            k.iter().filter(|&&x| x > 0).count(),
            k.iter().map(|x| x.unsigned_abs()).sum::<u64>(),
            k.clone(),
        )
    };
    upper.sort_by_key(|c| std::cmp::Reverse(weight(c)));
    // interest columns too: phase 2 takes the most complex unit column as pivot (lowest position)
    interest.sort_by_key(|c| std::cmp::Reverse(weight(c)));
    let mut pos = vec![0u32; sys.columns.len()];
    let mut col_of_pos = Vec::with_capacity(sys.columns.len());
    for &c in upper.iter().chain(&interest).chain(&carried) {
        pos[c as usize] = col_of_pos.len() as u32;
        col_of_pos.push(c);
    }
    Layout {
        target_pos: target_col.map(|c| pos[c as usize]),
        n_upper: upper.len() as u32,
        n_interest: interest.len() as u32,
        pos,
        col_of_pos,
    }
}

/// Output of one elimination run.
struct Run<P> {
    /// seed labels combined into the relation row (best or case A)
    trace: Option<Vec<u32>>,
    p_star: PStar,
    best: Option<Row2<P>>,
    case_a: Option<Row2<P>>,
    rank: RankData,
    indeterminate: usize,
    /// the phase-2 basis (for the check mode's dual certificate)
    basis: Vec<(u32, Row2<P>)>,
}

/// The two phases on rows in the given order.
fn eliminate<A: Ar>(
    ar: &A,
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
    lay: &Layout,
    order: &[usize],
    prec0: Option<u32>,
    bud: &mut Budgeter<'_>,
) -> Result<Run<A::P>, Stop> {
    let mut rank = RankData {
        rows: sys.rows.len(),
        columns: sys.columns.len(),
        upper_columns: lay.n_upper as usize,
        lower_columns: sys.columns.len()
            - lay.n_upper as usize
            - usize::from(lay.target_pos.is_some()),
        interest_columns: lay.n_interest as usize,
        target_absent: lay.target_pos.is_none(),
        ..Default::default()
    };
    let mut indeterminate = 0usize;
    // ---- phase 1
    let mut piv: Vec<Option<PRow<A::P>>> = vec![None; lay.n_upper as usize];
    let mut held_terms = 0usize;
    let mut w0: Vec<PRow<A::P>> = Vec::new();
    let dn = Integer::from(q.d0.n);
    let dd = Integer::from(q.d0.d);
    let progress = std::env::var_os("RUSTRED_TADIC_PROGRESS").is_some();
    for (done, &ri) in order.iter().enumerate() {
        if progress && done % 500 == 0 {
            eprintln!(
                "tadic phase 1: {done}/{} rows, {} upper pivots, {} W0 rows, {held_terms} terms held, {:.1} s",
                order.len(),
                rank.upper_pivots,
                w0.len(),
                bud.t0.elapsed().as_secs_f64()
            );
        }
        // a + b d = a + b (t + n/m)  ->  times m:  (a m + b n) + (b m) t
        let mut r: Ent<A::P> = sys.rows[ri]
            .iter()
            .map(|&(c, a, b)| {
                let a = Integer::from(a);
                let b = Integer::from(b);
                let c0 = &(&a * &dd) + &(&b * &dn);
                let c1 = &b * &dd;
                (lay.pos[c as usize], ar.affine(&c0, &c1))
            })
            .filter(|(_, p)| !ar.is_zero(p))
            .collect();
        r.sort_by_key(|e| e.0);
        let mut r: Ent<A::P> = r
            .into_iter()
            .map(|(c, p)| (c, ar.truncate(p, prec0)))
            .filter(|e| !ar.is_zero(&e.1))
            .collect();
        let mut prec = prec0;
        let mut tr = tr_bit(sys.labels[ri].seed);
        loop {
            if prec == Some(0) {
                indeterminate += 1;
                break;
            }
            let Some(lead) = r.first() else {
                if prec.is_some() {
                    indeterminate += 1;
                }
                break;
            };
            let c = lead.0;
            if c >= lay.n_upper {
                w0.push((r, prec, tr));
                break;
            }
            match &piv[c as usize] {
                None => {
                    let (t, d) = ent_terms(ar, &r);
                    held_terms += t;
                    bud.tick(held_terms, d)?;
                    piv[c as usize] = Some((r, prec, tr));
                    rank.upper_pivots += 1;
                    break;
                }
                Some((h, hp, htr)) => {
                    tr_or(&mut tr, htr);
                    let (a, b) = ar.cofactors(&h[0].1, &r[0].1);
                    let np = min_prec(prec, *hp);
                    let mut nr = comb(ar, &a, &r, &b, h, np);
                    // F-scaling: divide by the content and by t^(min valuation) (exact: in the gcd)
                    {
                        let mut refs: Vec<&mut A::P> = nr.iter_mut().map(|e| &mut e.1).collect();
                        ar.normalize(&mut refs);
                    }
                    prec = np;
                    if prec.is_some() {
                        let e = nr.iter().filter_map(|x| ar.val(&x.1)).min().unwrap_or(0);
                        if e > 0 {
                            for x in nr.iter_mut() {
                                x.1 = ar.div_t(&x.1, e);
                            }
                            prec = prec.map(|p| p.saturating_sub(e));
                        }
                    }
                    let (t, d) = ent_terms(ar, &nr);
                    bud.tick(held_terms + t, d)?;
                    r = nr;
                }
            }
        }
    }
    drop(piv);
    rank.w0_generators = w0.len();
    // ---- phase 2
    let ni = lay.n_upper + lay.n_interest;
    let tp = lay.target_pos;
    // early exit only in exact mode and when the full basis is not wanted (rank data then counts rows seen)
    let p2 = phase2(ar, w0, ni, tp, prec0.is_none(), bud)?;
    indeterminate += p2.indeterminate;
    let (basis, case_a, sat_exp) = (p2.basis, p2.case_a, p2.saturation_exponent);
    rank.saturation_exponent = sat_exp;
    rank.partial = p2.stopped_early;
    rank.dim_v = basis.len() + usize::from(case_a.is_some());
    rank.rank_rows = match q.interest {
        Interest::AllLower => Some(rank.upper_pivots + basis.len() + usize::from(case_a.is_some())),
        Interest::Keys(_) => None,
    };
    // p*
    let (p_star, best) = if case_a.is_some() {
        (PStar::Order(0), None)
    } else if let Some(k) = tp {
        let mut best: Option<(i64, usize)> = None;
        for (i, (_, h)) in basis.iter().enumerate() {
            if let Ok(ix) = h.c.binary_search_by_key(&k, |x| x.0) {
                let v = ar.val(&h.c[ix].1).expect("nonzero") as i64 - h.shift as i64;
                let better = match best {
                    None => true,
                    Some((bv, bi)) => {
                        v < bv || (v == bv && row_terms(ar, h) < row_terms(ar, &basis[bi].1))
                    }
                };
                if better {
                    best = Some((v, i));
                }
            }
        }
        match best {
            None => (PStar::NoRelation, None),
            Some((v, i)) => (PStar::Order(v.max(0) as u32), Some(basis[i].1.clone())),
        }
    } else {
        (PStar::NoRelation, None)
    };
    let trace = case_a.as_ref().or(best.as_ref()).map(|r| tr_list(&r.tr));
    Ok(Run {
        trace,
        p_star,
        best,
        case_a,
        rank,
        indeterminate,
        basis,
    })
}

/// Output of phase 2.
struct Phase2<P> {
    /// phase 2 stopped at the first row proving p* = 0 (rank data then count the rows seen)
    stopped_early: bool,
    basis: Vec<(u32, Row2<P>)>,
    case_a: Option<Row2<P>>,
    saturation_exponent: u64,
    indeterminate: usize,
}

/// Phase 2: valuation-aware triangular echelon over R on the positions `< ni` ("interest"; integral entries), the
/// positions `>= ni` carried (true value poly / t^shift); `tp` = the carried target position, if any. Each row is
/// reduced by the earlier pivots (creation order), its content divided out, saturated (divided by t^e, e its minimal
/// valuation on the interest part) and given a pivot where it is a unit. Rows whose interest part vanishes are dropped,
/// except the first one with a target entry (`case_a`).
fn phase2<A: Ar>(
    ar: &A,
    rows: Vec<PRow<A::P>>,
    ni: u32,
    tp: Option<u32>,
    early_exit: bool,
    bud: &mut Budgeter<'_>,
) -> Result<Phase2<A::P>, Stop> {
    let mut basis: Vec<(u32, Row2<A::P>)> = Vec::new();
    let mut case_a: Option<Row2<A::P>> = None;
    let mut sat_exp = 0u64;
    let mut indeterminate = 0usize;
    let mut held = 0usize;
    let mut stopped_early = false;
    let progress = std::env::var_os("RUSTRED_TADIC_PROGRESS").is_some();
    let n_rows = rows.len();
    let t0 = std::time::Instant::now();
    for (done, (e, prec, tr)) in rows.into_iter().enumerate() {
        if progress && done % 500 == 0 {
            eprintln!(
                "tadic phase 2: {done}/{n_rows} rows, {} pivots, {held} terms held, {:.1} s",
                basis.len(),
                t0.elapsed().as_secs_f64()
            );
        }
        let (s, c): (Ent<A::P>, Ent<A::P>) = e.into_iter().partition(|x| x.0 < ni);
        let mut r = Row2 {
            s,
            c,
            shift: 0,
            prec,
            tr,
        };
        // the row may enter with a common t-factor on its interest part: saturate first
        normalize2(ar, &mut r, &mut sat_exp);
        for (pc, h) in basis.iter() {
            let Ok(ix) = r.s.binary_search_by_key(pc, |x| x.0) else {
                continue;
            };
            let u = &h.s[h.s.binary_search_by_key(pc, |x| x.0).expect("pivot entry")].1;
            let rc = r.s[ix].1.clone();
            let (a, b) = ar.cofactors(u, &rc);
            r = comb2(ar, &a, &r, &b, h);
            let (t, d) = ent_terms(ar, &r.s);
            bud.tick(held + t, d)?;
        }
        normalize2(ar, &mut r, &mut sat_exp);
        if r.prec == Some(0) {
            indeterminate += 1;
            continue;
        }
        if r.s.is_empty() {
            let has_k = tp.is_some_and(|k| r.c.iter().any(|x| x.0 == k));
            if has_k {
                if case_a.is_none() {
                    case_a = Some(r);
                }
                if early_exit {
                    // I(K) needs no column of interest: p* = 0, final
                    stopped_early = true;
                    break;
                }
            } else if r.prec.is_some() {
                indeterminate += 1;
            }
            continue;
        }
        // pivot: the most complex unit column (val 0) = the lowest position (Laporta-like, keeps fill-in low)
        let pc =
            r.s.iter()
                .find(|x| ar.val(&x.1) == Some(0))
                .map(|x| x.0)
                .expect("saturated row has a unit entry");
        held += ent_terms(ar, &r.s).0 + ent_terms(ar, &r.c).0;
        // a basis row with v(true K entry) <= 0 is an element of the saturated lattice with a unit-or-polar K
        // coordinate: a REGULAR relation exists, p* = 0 is final (no order is lower): stop (exact early exit)
        let regular = tp
            .and_then(|k| r.c.binary_search_by_key(&k, |x| x.0).ok().map(|ix| (k, ix)))
            .is_some_and(|(_, ix)| {
                (ar.val(&r.c[ix].1).unwrap_or(0) as i64) - (r.shift as i64) <= 0
            });
        basis.push((pc, r));
        if regular && early_exit {
            stopped_early = true;
            break;
        }
    }
    Ok(Phase2 {
        stopped_early,
        basis,
        case_a,
        saturation_exponent: sat_exp,
        indeterminate,
    })
}

fn row_terms<A: Ar>(_ar: &A, r: &Row2<A::P>) -> usize {
    r.s.len() + r.c.len()
}

/// a*x - b*y on phase-2 rows (carried parts aligned to a common shift).
fn comb2<A: Ar>(ar: &A, a: &A::P, x: &Row2<A::P>, b: &A::P, y: &Row2<A::P>) -> Row2<A::P> {
    let prec = min_prec(x.prec, y.prec);
    let s = comb(ar, a, &x.s, b, &y.s, prec);
    let sh = x.shift.max(y.shift);
    let xa = ar.mul_t(a, sh - x.shift);
    let yb = ar.mul_t(b, sh - y.shift);
    let c = comb(ar, &xa, &x.c, &yb, &y.c, prec.map(|p| p + sh));
    let mut tr = x.tr.clone();
    tr_or(&mut tr, &y.tr);
    Row2 {
        tr,
        s,
        c,
        shift: sh,
        prec,
    }
}

/// content division (exact mode), saturation step on S, shift reduction on the carried part.
fn normalize2<A: Ar>(ar: &A, r: &mut Row2<A::P>, sat_exp: &mut u64) {
    let before = r.s.iter().filter_map(|x| ar.val(&x.1)).min();
    {
        let mut refs: Vec<&mut A::P> =
            r.s.iter_mut()
                .chain(r.c.iter_mut())
                .map(|e| &mut e.1)
                .collect();
        ar.normalize(&mut refs);
    }
    let e = r.s.iter().filter_map(|x| ar.val(&x.1)).min().unwrap_or(0);
    // a t-power inside the content counts as saturation too (the true row was divided by it)
    if let Some(b) = before {
        *sat_exp += b.saturating_sub(e) as u64;
    }
    if e > 0 {
        for x in r.s.iter_mut() {
            x.1 = ar.div_t(&x.1, e);
        }
        r.shift += e;
        r.prec = r.prec.map(|p| p.saturating_sub(e));
        *sat_exp += e as u64;
    }
    if r.shift > 0 && !r.c.is_empty() {
        let vc = r.c.iter().filter_map(|x| ar.val(&x.1)).min().unwrap_or(0);
        let k = vc.min(r.shift);
        if k > 0 {
            for x in r.c.iter_mut() {
                x.1 = ar.div_t(&x.1, k);
            }
            r.shift -= k;
        }
    }
}

/// Rational function `num(t) / den(t)` as text in d (t = d - d0), reduced, integer coefficients.
pub fn ratio_text_in_d(num: &ZP, den: &ZP, d0: &Rat) -> String {
    let g = num.gcd(den);
    let (n, d) = if g.is_one() {
        (num.clone(), den.clone())
    } else {
        (num.try_div(&g).expect("gcd"), den.try_div(&g).expect("gcd"))
    };
    let shift = Rational::new(Integer::from(-d0.n), Integer::from(d0.d));
    let nq = n
        .map_coeff(|c| Rational::from(c.clone()), Q)
        .shift_var(0, &shift);
    let dq = d
        .map_coeff(|c| Rational::from(c.clone()), Q)
        .shift_var(0, &shift);
    // clear denominators: common factor lcm of all denominators, applied to both
    let mut l = Integer::one();
    for c in nq.coefficients.iter().chain(&dq.coefficients) {
        l = l.lcm(&c.denominator());
    }
    let to_z = |p: &MultivariatePolynomial<symbolica::domains::rational::RationalField, u16>| -> Vec<(u16, Integer)> {
        (0..p.nterms())
            .map(|i| {
                let c = &p.coefficients[i] * &Rational::from(l.clone());
                (p.exponents(i)[0], c.numerator())
            })
            .collect()
    };
    let mut nz = to_z(&nq);
    let mut dz = to_z(&dq);
    // integer content
    let mut cg = Integer::zero();
    for (_, c) in nz.iter().chain(&dz) {
        cg = cg.gcd(c);
    }
    // sign: leading coefficient of the denominator positive
    let sign_neg = dz.last().is_some_and(|(_, c)| *c < 0);
    if !cg.is_zero() {
        for (_, c) in nz.iter_mut().chain(dz.iter_mut()) {
            *c = &*c / &cg;
            if sign_neg {
                *c = -c.clone();
            }
        }
    }
    let fmt = |v: &[(u16, Integer)]| -> String {
        if v.is_empty() {
            return "0".into();
        }
        let mut s = String::new();
        for (k, (e, c)) in v.iter().rev().enumerate() {
            let neg = *c < 0;
            let a = if neg { -c.clone() } else { c.clone() };
            if k == 0 {
                if neg {
                    s.push('-');
                }
            } else {
                s.push_str(if neg { "-" } else { "+" });
            }
            match (*e, a == Integer::one()) {
                (0, _) => s.push_str(&a.to_string()),
                (1, true) => s.push('d'),
                (1, false) => s.push_str(&format!("{a}*d")),
                (e, true) => s.push_str(&format!("d^{e}")),
                (e, false) => s.push_str(&format!("{a}*d^{e}")),
            }
        }
        s
    };
    let ns = fmt(&nz);
    if dz.len() == 1 && dz[0].0 == 0 && dz[0].1 == Integer::one() {
        ns
    } else {
        format!("({ns})/({})", fmt(&dz))
    }
}

/// Exact relation from the best phase-2 row (or the case-A row).
fn relation_from(
    ar: &ExactZ,
    sys: &ExactSystem,
    lay: &Layout,
    row: &Row2<ZP>,
    d0: &Rat,
) -> Relation {
    let tp = lay.target_pos.expect("target present");
    let ni = lay.n_upper + lay.n_interest;
    let kx = row.c.binary_search_by_key(&tp, |x| x.0).expect("K entry");
    let hk = &row.c[kx].1;
    let mut terms = Vec::new();
    let mut prof = Profile::default();
    // interest entries: c_j = - h_j t^shift / h_K ; carried: c_j = - h_j / h_K
    for (pos, p) in row.s.iter().chain(row.c.iter()) {
        if *pos == tp {
            continue;
        }
        let of_interest = *pos < ni;
        let num = if of_interest {
            ar.neg(&ar.mul_t(p, row.shift))
        } else {
            ar.neg(p)
        };
        let vn = ar.val(&num).unwrap_or(0) as i64;
        let vd = ar.val(hk).unwrap_or(0) as i64;
        let order = (vd - vn).max(0) as u32;
        if of_interest {
            prof.max_order = prof.max_order.max(order);
            prof.sum_orders += order as u64;
            prof.singular_terms += usize::from(order > 0);
        }
        terms.push(RelTerm {
            key: sys.columns[lay.col_of_pos[*pos as usize] as usize].clone(),
            coefficient: ratio_text_in_d(&num, hk, d0),
            pole_order: order,
            of_interest,
        });
    }
    prof.terms = terms.len();
    Relation {
        terms,
        profile: prof,
    }
}

/// The oracle (Symbolica licence required: construct with [`SymbolicaTadic::new`]).
pub struct SymbolicaTadic {
    /// additional deterministic row orders tried in exact mode to return the smallest p*-optimal relation
    /// (p* itself never depends on the order: checked, a difference is a hard error recorded in `notes`)
    pub relation_orders: u32,
    /// also run the order-1 screen (rows at d0) and report `screen_at_d0`
    pub with_screen_at_d0: bool,
}

impl SymbolicaTadic {
    pub fn new() -> Result<SymbolicaTadic, String> {
        if !crate::licensed() {
            return Err(
                "Symbolica licence absent (source WS/ops/env.sh): the tadic oracle refuses to run"
                    .into(),
            );
        }
        Ok(SymbolicaTadic {
            relation_orders: 1,
            with_screen_at_d0: true,
        })
    }
}

/// Deterministic row orders: 0 = as given, 1 = reversed, k >= 2 = a SplitMix-seeded shuffle.
pub fn row_order(n: usize, k: u32) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    match k {
        0 => {}
        1 => v.reverse(),
        _ => {
            let mut s: u64 = 0x9E37_79B9_7F4A_7C15 ^ k as u64;
            for i in (1..n).rev() {
                s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = s;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                v.swap(i, (z % (i as u64 + 1)) as usize);
            }
        }
    }
    v
}

fn base_result(sys: &ExactSystem, q: &PoleQuery<'_>) -> PoleOrderResult {
    PoleOrderResult {
        schema: TADIC_VERSION,
        system: sys.id.clone(),
        target: q.target.to_vec(),
        d0: q.d0.text(),
        arith: q.arith,
        verdict: Verdict::Undecided("not run".into()),
        p_star: None,
        relation: None,
        rank: RankData::default(),
        screen_at_d0: None,
        relation_seeds: None,
        seconds: 0.0,
        notes: Vec::new(),
    }
}

fn pre_budget(sys: &ExactSystem, q: &PoleQuery<'_>) -> Option<String> {
    if let Some(m) = q.budget.max_rows
        && sys.rows.len() > m
    {
        return Some(format!("budget max_rows {m} exceeded ({})", sys.rows.len()));
    }
    if let Some(m) = q.budget.max_columns
        && sys.columns.len() > m
    {
        return Some(format!(
            "budget max_columns {m} exceeded ({})",
            sys.columns.len()
        ));
    }
    None
}

/// Order-1 screen (rows evaluated at d0, mod a prime): does an admissible relation exist AT d0?
pub fn screen_at_d0(sys: &ExactSystem, q: &PoleQuery<'_>, prime: u64) -> Result<bool, String> {
    let ar = TruncP {
        vars: t_vars(),
        field: Zp64::new(prime),
    };
    let lay = layout(sys, q);
    let mut bud = Budgeter {
        q,
        t0: Instant::now(),
        ops: 0,
    };
    let order: Vec<usize> = (0..sys.rows.len()).collect();
    eliminate(&ar, sys, q, &lay, &order, Some(1), &mut bud)
        .map(|r| r.p_star == PStar::Order(0))
        .map_err(|s| s.0)
}

/// The default screen prime pair (the engine's replay pair).
pub const SCREEN_PRIMES: [u64; 2] = [2_305_843_009_213_693_951, 2_305_843_009_213_693_921];

impl PoleOrderOracle for SymbolicaTadic {
    fn best_pole_order(&self, sys: &ExactSystem, q: &PoleQuery<'_>) -> PoleOrderResult {
        let t0 = Instant::now();
        let mut res = base_result(sys, q);
        if let Some(why) = pre_budget(sys, q) {
            res.verdict = Verdict::Undecided(why);
            return res;
        }
        let lay = layout(sys, q);
        let mut bud = Budgeter { q, t0, ops: 0 };
        match q.arith {
            Arith::Screen { order, prime } => {
                let ar = TruncP {
                    vars: t_vars(),
                    field: Zp64::new(prime),
                };
                let ord: Vec<usize> = (0..sys.rows.len()).collect();
                match eliminate(&ar, sys, q, &lay, &ord, Some(order.max(1)), &mut bud) {
                    Ok(run) => {
                        res.verdict = Verdict::Screen;
                        res.p_star = Some(run.p_star);
                        res.rank = run.rank;
                        res.notes.push(format!(
                            "screen mod {prime} to t^{order}: {} rows/entries indeterminate (zero to the truncation)",
                            run.indeterminate
                        ));
                    }
                    Err(Stop(why)) => res.verdict = Verdict::Undecided(why),
                }
            }
            Arith::Exact => {
                let ar = ExactZ { vars: t_vars() };
                let mut chosen: Option<(Run<ZP>, u32)> = None;
                for k in 0..self.relation_orders.max(1) {
                    let ord = row_order(sys.rows.len(), k);
                    match eliminate(&ar, sys, q, &lay, &ord, None, &mut bud) {
                        Ok(run) => {
                            if let Some((c, _)) = &chosen
                                && c.p_star != run.p_star
                            {
                                res.verdict = Verdict::Undecided(format!(
                                    "INTERNAL: p* differs between row orders 0 and {k} ({:?} vs {:?})",
                                    c.p_star, run.p_star
                                ));
                                res.seconds = t0.elapsed().as_secs_f64();
                                return res;
                            }
                            let size = |r: &Run<ZP>| {
                                r.best
                                    .as_ref()
                                    .or(r.case_a.as_ref())
                                    .map_or(usize::MAX, |b| b.s.len() + b.c.len())
                            };
                            if chosen.as_ref().is_none_or(|(c, _)| size(&run) < size(c)) {
                                chosen = Some((run, k));
                            }
                        }
                        Err(Stop(why)) => {
                            if chosen.is_none() {
                                res.verdict = Verdict::Undecided(why);
                                res.seconds = t0.elapsed().as_secs_f64();
                                return res;
                            }
                            res.notes
                                .push(format!("row order {k}: {why} (kept the earlier orders)"));
                            break;
                        }
                    }
                }
                let (run, k) = chosen.expect("at least one order ran");
                res.verdict = Verdict::Exact;
                res.p_star = Some(run.p_star);
                res.relation_seeds = run.trace.clone();
                res.rank = run.rank;
                res.notes.push(format!(
                    "relation from row order {k} (0 = as given, 1 = reversed, >= 2 seeded shuffle)"
                ));
                if q.want_relation
                    && let Some(r) = run.case_a.as_ref().or(run.best.as_ref())
                {
                    res.relation = Some(relation_from(&ar, sys, &lay, r, &q.d0));
                }
            }
        }
        if self.with_screen_at_d0 && matches!(res.verdict, Verdict::Exact) {
            match screen_at_d0(sys, q, SCREEN_PRIMES[0]) {
                Ok(b) => res.screen_at_d0 = Some(b),
                Err(e) => res.notes.push(format!("screen at d0 not computed: {e}")),
            }
        }
        res.seconds = t0.elapsed().as_secs_f64();
        res
    }
}

/// Internal access for the check mode: the exact phase-2 basis as (pivot column, interest entries, carried entries,
/// shift), columns as SYSTEM column indices, polynomials in t.
pub(crate) struct ExactBasis {
    pub rows: Vec<BasisRowZ>,
    pub target_col: Option<u32>,
    pub interest_cols: Vec<u32>,
    pub p_star: PStar,
}

pub(crate) fn exact_basis(sys: &ExactSystem, q: &PoleQuery<'_>) -> Result<ExactBasis, String> {
    let ar = ExactZ { vars: t_vars() };
    let lay = layout(sys, q);
    let mut bud = Budgeter {
        q,
        t0: Instant::now(),
        ops: 0,
    };
    let ord: Vec<usize> = (0..sys.rows.len()).collect();
    let run = eliminate(&ar, sys, q, &lay, &ord, None, &mut bud).map_err(|s| s.0)?;
    let m = |e: &Ent<ZP>| -> Vec<(u32, ZP)> {
        e.iter()
            .map(|(p, x)| (lay.col_of_pos[*p as usize], x.clone()))
            .collect()
    };
    let ni = lay.n_upper + lay.n_interest;
    Ok(ExactBasis {
        rows: run
            .basis
            .iter()
            .map(|(pc, r)| (lay.col_of_pos[*pc as usize], m(&r.s), m(&r.c), r.shift))
            .collect(),
        target_col: lay.target_pos.map(|p| lay.col_of_pos[p as usize]),
        interest_cols: (lay.n_upper..ni)
            .map(|p| lay.col_of_pos[p as usize])
            .collect(),
        p_star: run.p_star,
    })
}
