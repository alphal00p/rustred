//! THE in-process TEXT judge of a relation's pole order at an integer d0 (lane polefix, audit r2-poles F5): ONE
//! implementation behind the two Symbolica-free call sites (seedmin's `LocalValuationJudge`, coreseed's
//! `DenominatorJudge`). It is the stated FALLBACK where the Symbolica-backed algebra crate is not linked (the
//! supervisor); where it is linked, `rustred_campaign_algebra::valuation::relation::ExactJudge` (Symbolica parse +
//! exact valuation) answers instead, and the supervisor asks the `pole-regular-at` child job for what this judge leaves
//! undecided.
//!
//! STRICT INPUT (anything else is UNDECIDED, never a number; the old top-level `/` split read `3/4*(-4+d)` as
//! `3/(4(d-4))` and evaluated `1/(d-4)+1` as `1/((d-4)+1)`): a coefficient text in `d` alone in one of the ENGINE's
//! printed forms
//! - `P` — a polynomial with no `/` at all (`3-d`, `-5`, `(16-8*d+d^2)`);
//! - `A/B` with exactly one `/` in the whole text, `A` = `(P)` (parentheses spanning all of `A`) or a signed integer,
//!   `B` = `(Q)` (parentheses spanning all of `B`) or a positive integer, `P`, `Q` polynomials without `/`
//!   (`(8-3*d)/(-8+2*d)`, `(1)/(64-48*d+12*d^2-d^3)`, `(-10+3*d)/2`, `-1/2`).
//!
//! The order is then exact: `v = mult_{d0}(P) - mult_{d0}(Q)` (Taylor coefficients at `d0` with the core's polynomial
//! reader and big integers; a common factor of `P` and `Q` cancels in the difference), the pole order `max(0, -v)`.
//! L11: this crate builds without Symbolica (design reason, recorded in WS/notes/implementer.polefix.md; not a
//! verification); the Symbolica cross-check is the algebra crate's test `audit_r2_poles_valuation`.

use serde_json::Value;

use crate::poly::{BigInt, Poly};

/// Multiplicity of the root `d0` of the polynomial text `t` in d (`None`: unreadable or the zero polynomial).
pub fn root_multiplicity(t: &str, d0: i64) -> Option<u32> {
    let p = Poly::parse_exact(t.trim(), 0, true).ok()?;
    let mut a: Vec<BigInt> = vec![];
    for (j, c) in p.d_coefficients() {
        let j = j as usize;
        if a.len() <= j {
            a.resize(j + 1, BigInt::zero());
        }
        a[j] = a[j].add(&c.eval_big(&[]).ok()?);
    }
    if a.iter().all(|x| x.is_zero()) {
        return None;
    }
    let n = a.len();
    let x = BigInt::from_i128(d0 as i128);
    // powers of d0
    let mut pw = vec![BigInt::from_i128(1)];
    for i in 1..n {
        pw.push(pw[i - 1].mul(&x));
    }
    // binomial rows: b_m = sum_{j >= m} a_j C(j, m) d0^(j - m) (the Taylor coefficients at d0)
    let mut row: Vec<BigInt> = vec![BigInt::from_i128(1)];
    let mut rows: Vec<Vec<BigInt>> = vec![row.clone()];
    for _ in 1..n {
        let mut next = vec![BigInt::from_i128(1)];
        for w in row.windows(2) {
            next.push(w[0].add(&w[1]));
        }
        next.push(BigInt::from_i128(1));
        rows.push(next.clone());
        row = next;
    }
    for m in 0..n {
        let mut b = BigInt::zero();
        for (j, aj) in a.iter().enumerate().skip(m) {
            if aj.is_zero() {
                continue;
            }
            b = b.add(&aj.mul(&rows[j][m]).mul(&pw[j - m]));
        }
        if !b.is_zero() {
            return Some(m as u32);
        }
    }
    None
}

/// The inside of `(X)` when the parentheses span the WHOLE text (`(a)+(b)` is not one group).
fn whole_group(t: &str) -> Option<&str> {
    let inner = t.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(inner)
}

fn is_int(t: &str, signed: bool) -> bool {
    let digits = if signed {
        t.strip_prefix('-').unwrap_or(t)
    } else {
        t
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The (numerator, denominator) texts of a coefficient in the ENGINE's printed form (module header), else `None`.
pub fn engine_form(c: &str) -> Option<(&str, &str)> {
    let c = c.trim();
    match c.matches('/').count() {
        0 => (!c.is_empty()).then_some((c, "1")),
        1 => {
            let (a, b) = c.split_once('/')?;
            let (a, b) = (a.trim(), b.trim());
            let num = match whole_group(a) {
                Some(p) => p,
                None if is_int(a, true) => a,
                None => return None,
            };
            let den = match whole_group(b) {
                Some(q) => q,
                None if is_int(b, false) => b,
                None => return None,
            };
            Some((num, den))
        }
        _ => None,
    }
}

/// Pole order at the integer `d0` of a coefficient text (`None`: undecided — not the engine's form, unreadable, zero).
pub fn coefficient_pole_order(c: &str, d0: i64) -> Option<u32> {
    let (n, d) = engine_form(c)?;
    let vn = root_multiplicity(n, d0)?;
    let vd = root_multiplicity(d, d0)?;
    Some(vd.saturating_sub(vn))
}

/// Pole order at `d0` of a NORMALISED relation record (engine solve-recipe `relation` record: `rhs` = [[key,
/// coefficient, part?], ..], coefficients in d alone): the maximum over the terms; `None` = undecided (a non-integer
/// d0, an unnormalised record with a `target_coefficient`, any coefficient outside the engine's form).
pub fn relation_pole_order(record: &Value, d0: &str) -> Option<u32> {
    let d0 = d0.trim().parse::<i64>().ok()?;
    if !record["target_coefficient"].is_null() {
        return None;
    }
    let rhs = record["rhs"].as_array()?;
    let mut max = 0u32;
    for t in rhs {
        let c = t.get(1).and_then(Value::as_str)?;
        max = max.max(coefficient_pole_order(c, d0)?);
    }
    Some(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_engine_form_gets_a_number() {
        assert_eq!(engine_form("(8-3*d)/(-8+2*d)"), Some(("8-3*d", "-8+2*d")));
        assert_eq!(engine_form("-1/2"), Some(("-1", "2")));
        assert_eq!(engine_form("(1)/(64-48*d)"), Some(("1", "64-48*d")));
        assert_eq!(engine_form("3-d"), Some(("3-d", "1")));
        assert_eq!(
            engine_form("(d-4)/((d-4)^3*(d-1))"),
            Some(("d-4", "(d-4)^3*(d-1)"))
        );
        // the forms the old split misread: undecided now
        assert_eq!(engine_form("3/4*(-4+d)"), None);
        assert_eq!(engine_form("1/(d-4)+1"), None);
        assert_eq!(engine_form("(1)+(2)/(d-4)"), None);
        assert_eq!(engine_form("1/(d-4)^2"), None);
        assert_eq!(engine_form("x/y/z"), None);
        assert_eq!(engine_form("3*d/(d-4)"), None);
        assert_eq!(engine_form("(d)/-2"), None);
        assert_eq!(coefficient_pole_order("(8-3*d)/(-8+2*d)", 4), Some(1));
        assert_eq!(coefficient_pole_order("(8-3*d)/(-14+6*d)", 4), Some(0));
        assert_eq!(coefficient_pole_order("(d-4)/((d-4)^3*(d-1))", 4), Some(2));
        assert_eq!(
            coefficient_pole_order("(16-8*d+d^2)/(-12+7*d-d^2)", 4),
            Some(0)
        );
        assert_eq!(coefficient_pole_order("1/(d-4)+1", 4), None);
        assert_eq!(coefficient_pole_order("3/4*(-4+d)", 4), None);
        assert_eq!(
            relation_pole_order(
                &json!({"rhs": [["I(1)", "(1)/(16-8*d+d^2)"], ["I(2)", "2"]]}),
                "4"
            ),
            Some(2)
        );
        assert_eq!(
            relation_pole_order(&json!({"rhs": [["I(1)", "1/(d-4)+1"]]}), "4"),
            None
        );
        assert_eq!(
            relation_pole_order(&json!({"rhs": [["I(1)", "2"]]}), "9/2"),
            None
        );
        assert_eq!(
            relation_pole_order(
                &json!({"target_coefficient": "d-4", "rhs": [["I(1)", "2"]]}),
                "4"
            ),
            None
        );
    }
}
