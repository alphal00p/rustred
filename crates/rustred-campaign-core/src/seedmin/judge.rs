//! Judges of the pole objective (interfaces/seedmin.md 3): stubs for lane poles' `pole_order_of` and lane tadic's
//! `keeps_regularity`, a TEMPORARY local valuation judge, and the conversion of a verified relation record into the
//! small [`RelationFacts`] the minimiser journals.

use serde_json::Value;

use super::{Offset, PoleOrder, RelationFacts, RelationPoles, SystemPoles, sub};
use crate::poly::{BigInt, Poly};

/// Until lane poles' function lands: every relation is Undecided.
pub struct StubRelationPoles;

impl RelationPoles for StubRelationPoles {
    fn pole_order_of(&mut self, _record: &Value, _key: &[i32], _d0: &str) -> PoleOrder {
        PoleOrder::Undecided
    }
    fn name(&self) -> String {
        "stub (undecided)".into()
    }
}

/// Until lane tadic's function lands: not available (only the relation's pole order decides).
pub struct StubSystemPoles;

impl SystemPoles for StubSystemPoles {
    fn available(&self) -> bool {
        false
    }
    fn keeps_regularity(&mut self, _: &[Offset], _: &[Offset], _: &[i32], _: &str) -> Option<bool> {
        None
    }
    fn name(&self) -> String {
        "stub (not available)".into()
    }
}

/// TEMPORARY relation judge (to be replaced by lane poles' `pole_order_of`, Symbolica-backed on ac/poles): at a
/// numeric key every coefficient is a rational function of d alone, printed by the engine NORMALISED (`N/D`, coprime);
/// its pole order at an INTEGER d0 is `max(0, v(D) - v(N))` with `v` the multiplicity of the root d0 (exact: Taylor
/// coefficients at d0 with the core's own polynomial reader and big integers); the relation's order is the maximum over
/// its terms. An unreadable coefficient, a non-integer d0 or a zero numerator gives Undecided.
/// L11: the campaign-core crate builds without Symbolica (design reason, recorded in the notes, not a verification).
pub struct LocalValuationJudge;

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
    // binomial rows on the fly: b_m = sum_{j >= m} a_j C(j, m) d0^(j - m)
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

/// Split a coefficient text at its single top-level `/` (none: denominator 1).
pub fn split_fraction(c: &str) -> Option<(String, String)> {
    let mut depth = 0i32;
    let mut at: Vec<usize> = vec![];
    for (i, ch) in c.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => at.push(i),
            _ => {}
        }
    }
    match at.as_slice() {
        [] => Some((c.to_string(), "1".to_string())),
        [i] => Some((c[..*i].to_string(), c[i + 1..].to_string())),
        _ => None,
    }
}

/// Pole order at `d0` of a coefficient text (None: undecided).
pub fn coefficient_pole_order(c: &str, d0: i64) -> Option<u32> {
    let (n, d) = split_fraction(c)?;
    let vn = root_multiplicity(&n, d0)?;
    let vd = root_multiplicity(&d, d0)?;
    Some(vd.saturating_sub(vn))
}

impl RelationPoles for LocalValuationJudge {
    fn pole_order_of(&mut self, record: &Value, _key: &[i32], d0: &str) -> PoleOrder {
        let Ok(d0) = d0.trim().parse::<i64>() else {
            return PoleOrder::Undecided;
        };
        let Some(rhs) = record["rhs"].as_array() else {
            return PoleOrder::Undecided;
        };
        let mut max = 0u32;
        for t in rhs {
            let Some(c) = t.get(1).and_then(Value::as_str) else {
                return PoleOrder::Undecided;
            };
            match coefficient_pole_order(c, d0) {
                Some(o) => max = max.max(o),
                None => return PoleOrder::Undecided,
            }
        }
        PoleOrder::Order { order: max }
    }
    fn name(&self) -> String {
        "local_valuation (temporary)".into()
    }
}

/// The facts of a VERIFIED SOLVED relation record (coord-recipe-solve.md 7) at `key` for a trial with `centre`:
/// `seeds_used` (engine: offsets from the key) become centre-relative; `bytes` = the record's compact JSON length;
/// `pole` from the given judge. `Err` when the record has no relation digest.
pub fn facts_from_record(
    record: &Value,
    key: &[i32],
    centre: &[i32],
    d0: &str,
    judge: &mut dyn RelationPoles,
) -> Result<RelationFacts, String> {
    let digest = record["relation_digest"]
        .as_str()
        .ok_or("record without relation_digest")?
        .to_string();
    let rhs_count = record["rhs_count"]
        .as_u64()
        .or_else(|| record["rhs"].as_array().map(|a| a.len() as u64))
        .ok_or("record without rhs_count / rhs")?;
    let seeds_used = record["seeds_used"].as_array().map(|a| {
        a.iter()
            .filter_map(|o| serde_json::from_value::<Offset>(o.clone()).ok())
            .filter(|o| o.len() == centre.len())
            .map(|o| sub(&o, centre))
            .collect::<Vec<_>>()
    });
    Ok(RelationFacts {
        relation_digest: digest,
        rhs_count,
        rhs_sub_count: record["rhs_sub_count"].as_u64(),
        bytes: serde_json::to_string(record).ok().map(|s| s.len() as u64),
        trace_rows: record["trace_rows"].as_u64(),
        seeds_used,
        pole: judge.pole_order_of(record, key, d0),
        minimise: record.get("minimise").filter(|m| !m.is_null()).cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn local_valuation_reads_pole_orders() {
        assert_eq!(root_multiplicity("d-4", 4), Some(1));
        assert_eq!(root_multiplicity("(d-4)^2*(3*d-14)", 4), Some(2));
        assert_eq!(root_multiplicity("-8+2*d", 4), Some(1));
        assert_eq!(root_multiplicity("7", 4), Some(0));
        assert_eq!(root_multiplicity("0", 4), None);
        assert_eq!(coefficient_pole_order("(8-3*d)/(-14+6*d)", 4), Some(0));
        assert_eq!(coefficient_pole_order("(8-3*d)/(-8+2*d)", 4), Some(1));
        assert_eq!(coefficient_pole_order("(d-4)/((d-4)^3*(d-1))", 4), Some(2));
        assert_eq!(coefficient_pole_order("x/y/z", 4), None);
        let mut j = LocalValuationJudge;
        let rec = json!({"rhs": [["I(1)", "(8-3*d)/(-14+6*d)", "top"], ["I(2)", "1/(d-4)^2", "sub"], ["I(3)", "2", "sub"]]});
        assert_eq!(
            j.pole_order_of(&rec, &[], "4"),
            PoleOrder::Order { order: 2 }
        );
        assert_eq!(j.pole_order_of(&rec, &[], "9/2"), PoleOrder::Undecided);
        let big = json!({"rhs": [["I(1)", "(123456789012345678901234567890*d)/(-4+d)", "top"]]});
        assert_eq!(
            j.pole_order_of(&big, &[], "4"),
            PoleOrder::Order { order: 1 }
        );
        let f = facts_from_record(
            &json!({"relation_digest": "ab", "rhs": [["I(1)", "1", "top"]], "seeds_used": [[0, -1], [1, -1]]}),
            &[3, 2],
            &[0, -1],
            "4",
            &mut j,
        )
        .unwrap();
        assert_eq!(f.seeds_used, Some(vec![vec![0, 0], vec![1, 0]]));
        assert_eq!(f.rhs_count, 1);
        assert_eq!(f.pole, PoleOrder::regular());
    }
}
