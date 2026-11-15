//! Reusable LOCAL-RING LATTICE tools at d = d0 + t (API FOR OTHER LANES, e.g. regbasis: a regular terminal set from
//! composed vectors). Input: a matrix whose rows are sparse vectors of exact rational functions of d (coefficient
//! TEXTS, the house format, parsed with Symbolica). Over the discrete valuation ring R = Q[t]_(t):
//!
//! - [`saturate_rows`]: an R-basis of the SATURATED lattice V ∩ R^n of the row space V (triangular: pivot entry a
//!   unit, zero at every earlier pivot), its pivots and rank; the basis depends only on V up to R-row operations
//!   (proof: notes implementer.tadic.md, CLAIM 2).
//! - [`smith_exponents`]: the exponents e_1 <= .. <= e_r of the Smith form over R of the matrix as given (invariant
//!   factors t^{e_i}; negative when rows carry poles): sum e_i = length of (saturation / module generated) after a
//!   uniform shift; e_i > 0 = directions that degenerate at d0 (rank drop of the matrix at d0 = number of e_i > 0
//!   for an integral matrix).
//!
//! Arithmetic: Symbolica polynomials over Z in t; the pivot logic is the oracle's phase 2 (shared code).

use rustred_campaign_core::tadic::Rat;
use serde::Serialize;
use symbolica::domains::integer::Integer;

use super::check::{Ctx, parse_in_t_with};
use super::{Ar, Budgeter, ExactZ, ZP, phase2, ratio_text_in_d, t_vars};

/// One row of a saturated basis: its pivot column and entries `(column, coefficient text in d, valuation at d0)`.
#[derive(Clone, Debug, Serialize)]
pub struct BasisRow {
    pub pivot: u32,
    pub entries: Vec<(u32, String, i64)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SaturatedBasis {
    pub rank: usize,
    pub basis: Vec<BasisRow>,
    /// total t-exponent divided out to saturate (0 = the rows already generated a saturated module, up to scaling)
    pub saturation_exponent: u64,
}

/// A row of (column, numerator, denominator) in t.
type RowT = Vec<(u32, ZP, ZP)>;

fn rows_in_t(rows: &[Vec<(u32, String)>], d0: &Rat) -> Result<Vec<RowT>, String> {
    let ctx = Ctx::new();
    rows.iter()
        .map(|r| {
            let mut v: Vec<(u32, ZP, ZP)> = Vec::with_capacity(r.len());
            for (c, text) in r {
                let f = parse_in_t_with(&ctx, text, d0)?;
                if !f.numerator.is_zero() {
                    v.push((*c, f.numerator, f.denominator));
                }
            }
            v.sort_by_key(|x| x.0);
            Ok(v)
        })
        .collect()
}

fn lcm(a: &ZP, b: &ZP) -> ZP {
    let g = a.gcd(b);
    (a * b).try_div(&g).expect("gcd divides")
}

/// Saturated basis of the row space (rows: `(column, coefficient text in d)`).
pub fn saturate_rows(rows: &[Vec<(u32, String)>], d0: Rat) -> Result<SaturatedBasis, String> {
    let ar = ExactZ { vars: t_vars() };
    let rt = rows_in_t(rows, &d0)?;
    // scale every row by the lcm of its denominators (an F-scaling: the saturation depends only on the row space)
    let mut input = Vec::with_capacity(rt.len());
    for r in rt {
        if r.is_empty() {
            continue;
        }
        let mut l = r[0].2.clone();
        for x in &r[1..] {
            l = lcm(&l, &x.2);
        }
        let e: Vec<(u32, ZP)> = r
            .into_iter()
            .map(|(c, n, d)| (c, &n * &l.try_div(&d).expect("lcm")))
            .collect();
        input.push((e, None, Vec::new()));
    }
    let q = dummy_query();
    let mut bud = Budgeter {
        q: &q,
        t0: std::time::Instant::now(),
        ops: 0,
    };
    let p2 = phase2(&ar, input, u32::MAX, None, &mut bud).map_err(|s| s.0)?;
    let one = ar.affine(&Integer::one(), &Integer::zero());
    let basis = p2
        .basis
        .iter()
        .map(|(pc, r)| BasisRow {
            pivot: *pc,
            entries: r
                .s
                .iter()
                .map(|(c, p)| {
                    (
                        *c,
                        ratio_text_in_d(p, &one, &d0),
                        ar.val(p).unwrap_or(0) as i64,
                    )
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    Ok(SaturatedBasis {
        rank: basis.len(),
        basis,
        saturation_exponent: p2.saturation_exponent,
    })
}

/// Smith exponents over R of the matrix as given (full pivoting on the entry of least valuation).
pub fn smith_exponents(rows: &[Vec<(u32, String)>], d0: Rat) -> Result<Vec<i64>, String> {
    let ar = ExactZ { vars: t_vars() };
    let rt = rows_in_t(rows, &d0)?;
    // make the matrix integral by a UNIFORM t^k (shifts every exponent by k) and row-wise UNITS (change nothing)
    let mut k = 0u32;
    let mut m: Vec<Vec<(u32, ZP)>> = Vec::new();
    let mut units: Vec<(ZP, u32)> = Vec::new();
    for r in &rt {
        if r.is_empty() {
            continue;
        }
        let mut l = r[0].2.clone();
        for x in &r[1..] {
            l = lcm(&l, &x.2);
        }
        let a = ar.val(&l).unwrap_or(0);
        k = k.max(a);
        units.push((ar.div_t(&l, a), a));
    }
    for (r, (u, _)) in rt.iter().filter(|r| !r.is_empty()).zip(&units) {
        // row * u * t^k: entries n * (u t^k / d), d = t^{a_d} * unit_d
        let e = r
            .iter()
            .map(|(c, n, d)| {
                let ad = ar.val(d).unwrap_or(0);
                let du = ar.div_t(d, ad);
                (
                    *c,
                    ar.mul_t(&(n * &u.try_div(&du).expect("unit lcm")), k - ad),
                )
            })
            .collect();
        m.push(e);
    }
    let mut out = Vec::new();
    loop {
        // entry of least valuation
        let mut best: Option<(u32, usize, u32)> = None;
        for (i, r) in m.iter().enumerate() {
            for (c, p) in r {
                let v = ar.val(p).expect("nonzero");
                if best.is_none_or(|(bv, _, _)| v < bv) {
                    best = Some((v, i, *c));
                }
            }
        }
        let Some((e, i, c)) = best else { break };
        out.push(e as i64 - k as i64);
        let piv = m.swap_remove(i);
        let pe = piv.iter().find(|x| x.0 == c).expect("pivot").1.clone();
        let u = ar.div_t(&pe, e);
        let ph: Vec<(u32, ZP)> = piv.iter().map(|(cc, p)| (*cc, ar.div_t(p, e))).collect();
        for r in m.iter_mut() {
            let Some(rc) = r.iter().find(|x| x.0 == c).map(|x| x.1.clone()) else {
                continue;
            };
            // r <- u r - rc ph  (column c cancels: u rc - rc u)
            let nr = super::comb(&ar, &u, r, &rc, &ph, None);
            *r = nr;
        }
        m.retain(|r| !r.is_empty());
    }
    out.sort_unstable();
    Ok(out)
}

fn dummy_query() -> rustred_campaign_core::tadic::PoleQuery<'static> {
    fn never(_: &[i64]) -> bool {
        false
    }
    rustred_campaign_core::tadic::PoleQuery {
        target: &[],
        lower: &never,
        d0: Rat::int(0),
        interest: rustred_campaign_core::tadic::Interest::AllLower,
        arith: rustred_campaign_core::tadic::Arith::Exact,
        budget: rustred_campaign_core::tadic::TadicBudget::default(),
        want_relation: false,
    }
}
