//! The first match in EXACT arithmetic, written from the engine's `Table::find_rule` (rustred-core node/mastermap.rs at
//! f9132a56): fixed slots, target pattern (rule point), case equations at d = 0, exception branches (a branch fires
//! when every member vanishes at both generic d = 37/7 and d = -11/5), table order. The engine decides "vanishes" by
//! the two-prime test; here it is decided exactly (Symbolica Integer / Rational, i128 fast path for c + a n_v).

use symbolica::domains::rational::Rational;

use crate::table::{Member, Rule, Table};

pub fn exception_ds() -> [Rational; 2] {
    [Rational::from((37, 7)), Rational::from((-11, 5))]
}

/// The rule point of key `k`, or None if a fixed slot or a constant target slot disagrees.
pub fn rule_point(r: &Rule, k: &[i64]) -> Option<Vec<i64>> {
    let n = k.len();
    for (i, f) in r.fixed.iter().enumerate() {
        if let Some(x) = f
            && k[i] != *x
        {
            return None;
        }
    }
    let mut pt = vec![i64::MIN; n];
    for (j, (a, o)) in r.target.iter().enumerate() {
        match a {
            Some(a) => pt[*a] = k[j] - o,
            None => {
                if k[j] != *o {
                    return None;
                }
            }
        }
    }
    for (x, f) in pt.iter_mut().zip(r.fixed.iter()) {
        if *x == i64::MIN {
            *x = f.unwrap_or(0);
        }
    }
    Some(pt)
}

/// Exact "vanishes at both exception values of d" (engine: evaluation error = not zero).
pub fn exception_member_zero(m: &Member, pt: &[i64]) -> bool {
    if let Some((c, v, a)) = m.fast {
        let x = a
            .checked_mul(pt[v] as i128)
            .and_then(|y| y.checked_add(c))
            .expect("i128 overflow in a one-variable literal");
        return x == 0;
    }
    if !m.uses_d && !m.expr.has_division() {
        return m.expr.eval_int(pt, 0).map(|v| v.is_zero()).unwrap_or(false);
    }
    exception_ds()
        .iter()
        .all(|d| m.expr.eval(pt, d).map(|v| v.is_zero()).unwrap_or(false))
}

/// Exact "case equation holds" (engine: evaluated at d = 0, error = does not hold).
pub fn equation_holds(m: &Member, pt: &[i64]) -> bool {
    if let Some((c, v, a)) = m.fast {
        return a * pt[v] as i128 + c == 0;
    }
    if !m.uses_d && !m.expr.has_division() {
        return m.expr.eval_int(pt, 0).map(|v| v.is_zero()).unwrap_or(false);
    }
    m.expr
        .eval(pt, &Rational::zero())
        .map(|v| v.is_zero())
        .unwrap_or(false)
}

pub fn applies(r: &Rule, k: &[i64]) -> bool {
    if r.skip {
        return false;
    }
    let Some(pt) = rule_point(r, k) else {
        return false;
    };
    if !r.equations.iter().all(|e| equation_holds(e, &pt)) {
        return false;
    }
    !r.exceptions
        .iter()
        .any(|br| br.iter().all(|m| exception_member_zero(m, &pt)))
}

/// Position of the first applicable rule.
pub fn first_match(t: &Table, k: &[i64]) -> Option<usize> {
    t.rules.iter().position(|r| applies(r, k))
}

/// Top-sector key test of the chart: lines >= 1, others <= 0.
pub fn in_chart(mask: &[bool], k: &[i64]) -> bool {
    k.iter()
        .zip(mask)
        .all(|(&x, &m)| if m { x >= 1 } else { x <= 0 })
}

/// D + r of a chart key.
pub fn dr(mask: &[bool], k: &[i64]) -> i128 {
    k.iter()
        .zip(mask)
        .map(|(&x, &m)| if m { x as i128 - 1 } else { -(x as i128) })
        .sum()
}
