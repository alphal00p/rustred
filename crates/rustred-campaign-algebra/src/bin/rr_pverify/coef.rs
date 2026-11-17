//! Coefficient algebra shared by the descent and validity checks: a coefficient text read as N / D (both sides
//! division-free, as the table writes them; anything else is reported), sides as exact integer polynomials
//! (Symbolica), restriction of a rule's polynomials to its fixed slots plus a locus of pinned slots, and the
//! "locus excluded by an exception branch" test.

use std::sync::Arc;

use symbolica::domains::integer::Integer;
use symbolica::poly::PolyVariable;

use crate::expr::{Expr, ZPoly, node_has_division, split_ratio, to_zpoly, zpoly_to_q};
use crate::face::{Face, Region};
use crate::table::Rule;

pub struct Ratio {
    pub num: ZPoly,
    /// the written divisor (kept for diagnostics; validity reads divisors itself)
    #[allow(dead_code)]
    pub den: ZPoly,
}

pub fn parse_ratio(text: &str, n: usize, vars: &Arc<Vec<PolyVariable>>) -> Result<Ratio, String> {
    let e = Expr::parse(text, n)?;
    let (a, b) = split_ratio(&e.root).ok_or("not a ratio")?;
    if node_has_division(&a) || node_has_division(&b) {
        return Err(format!("nested division in coefficient {}", short(text)));
    }
    Ok(Ratio {
        num: to_zpoly(&a, n, vars)?,
        den: to_zpoly(&b, n, vars)?,
    })
}

pub fn short(t: &str) -> String {
    if t.len() > 120 {
        format!("{}...", &t[..120])
    } else {
        t.to_string()
    }
}

/// Substitute constants: `pins[v] = Some(x)` replaces n_v by x.
pub fn pin_poly(p: &ZPoly, pins: &[Option<i64>]) -> ZPoly {
    let mut q = p.clone();
    for (v, x) in pins.iter().enumerate() {
        if let Some(x) = x
            && q.degree(v) > 0
        {
            q = q.replace(v, &Integer::from(*x));
        }
    }
    q
}

/// The rule's case (fixed slots, case equations) cut by the locus pins, as an exact face; None = empty.
pub fn locus_face(r: &Rule, pins: &[Option<i64>], n: usize) -> Option<Face> {
    let mut f = Face::top();
    for (i, x) in r.fixed.iter().enumerate().chain(pins.iter().enumerate()) {
        if let Some(x) = x {
            f = f.pin(n, i, *x)?;
        }
    }
    for e in &r.equations {
        let l = e.lin.as_ref()?;
        f = f.add_eq(l)?;
    }
    Some(f)
}

/// Some exception branch whose every member vanishes identically on the face (the face holds no selected key).
pub fn branch_vanishes_on(
    r: &Rule,
    f: &Face,
    n: usize,
    vars: &Arc<Vec<PolyVariable>>,
) -> Option<usize> {
    'b: for (bi, br) in r.exceptions.iter().enumerate() {
        for m in br {
            if let Some(l) = &m.lin {
                let s = f.subst(l);
                if !(s.is_const() && s.c.is_zero()) {
                    continue 'b;
                }
            } else {
                let Ok(z) = to_zpoly(&m.expr.root, n, vars) else {
                    continue 'b;
                };
                if !crate::poles::restrict(&zpoly_to_q(&z), f, n).is_zero() {
                    continue 'b;
                }
            }
        }
        return Some(bi);
    }
    None
}

/// Is the numerator identically zero on (rule's case + locus pins)? Exact: the pins substituted; else the case
/// equations through the exact face; else the locus is empty in the chart, or inside an exception branch.
/// Ok(how) = proved, Err(reason) = not proved.
pub fn numerator_zero_on(
    num: &ZPoly,
    r: &Rule,
    pins: &[Option<i64>],
    n: usize,
    vars: &Arc<Vec<PolyVariable>>,
) -> Result<&'static str, String> {
    let mut all = r.fixed.clone();
    for (i, x) in pins.iter().enumerate() {
        if x.is_some() {
            all[i] = *x;
        }
    }
    let q = pin_poly(num, &all);
    if q.is_zero() {
        return Ok("numerator identically zero on the locus");
    }
    let Some(f) = locus_face(r, pins, n) else {
        return Ok("locus empty (case equations and pins inconsistent)");
    };
    let chart = Region {
        mask: r_mask(r),
        r: None,
    };
    if !f.feasible(&chart) {
        return Ok("locus empty in the chart");
    }
    let qq = crate::poles::restrict(&zpoly_to_q(&q), &f, n);
    if qq.is_zero() {
        return Ok("numerator zero on the locus after the case equations");
    }
    if branch_vanishes_on(r, &f, n, vars).is_some() {
        return Ok("locus excluded by an exception branch");
    }
    Err(format!(
        "numerator not zero on the locus ({} terms left)",
        qq.nterms()
    ))
}

/// The rule's chart mask (from its record's chart string, kept in the rule origin-independent field).
fn r_mask(r: &Rule) -> Vec<bool> {
    r.chart.clone()
}
