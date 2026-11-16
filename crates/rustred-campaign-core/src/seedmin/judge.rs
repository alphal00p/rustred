//! Judges of the pole objective (interfaces/seedmin.md 3): stubs for lane poles' `pole_order_of` and lane tadic's
//! `keeps_regularity`, a TEMPORARY local valuation judge, and the conversion of a verified relation record into the
//! small [`RelationFacts`] the minimiser journals.

use serde_json::Value;

use super::{Offset, PoleOrder, RelationFacts, RelationPoles, SystemPoles, sub};

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

/// The in-process relation judge of the seed minimiser: THE core text judge ([`crate::poles::textjudge`], lane polefix,
/// audit r2-poles F5; one implementation shared with coreseed's `DenominatorJudge`). Exact pole order at an INTEGER d0
/// for coefficients in the ENGINE's printed form (`(N)/(D)`, `(N)/q`, `p/q`, a polynomial without `/`); anything else
/// (an unreadable or non-engine form, a non-integer d0, an unnormalised record) is UNDECIDED, never a number. Where
/// the Symbolica-backed algebra crate is linked, `rustred_campaign_algebra::valuation::relation::ExactJudge` replaces
/// it; in the supervisor the orders it leaves undecided come from the `pole-regular-at` child job.
pub struct LocalValuationJudge;

/// Re-exported from the shared text judge (one implementation).
pub use crate::poles::textjudge::{coefficient_pole_order, root_multiplicity};

impl RelationPoles for LocalValuationJudge {
    fn pole_order_of(&mut self, record: &Value, _key: &[i32], d0: &str) -> PoleOrder {
        match crate::poles::textjudge::relation_pole_order(record, d0) {
            Some(order) => PoleOrder::Order { order },
            None => PoleOrder::Undecided,
        }
    }
    fn name(&self) -> String {
        "local_valuation (core text judge: engine form only)".into()
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
        let rec = json!({"rhs": [["I(1)", "(8-3*d)/(-14+6*d)", "top"], ["I(2)", "1/(16-8*d+d^2)", "sub"], ["I(3)", "2", "sub"]]});
        assert_eq!(
            j.pole_order_of(&rec, &[], "4"),
            PoleOrder::Order { order: 2 }
        );
        // lane polefix (F5): `1/(d-4)^2` is not the engine's printed form: undecided, never a guess
        let hand = json!({"rhs": [["I(2)", "1/(d-4)^2", "sub"]]});
        assert_eq!(j.pole_order_of(&hand, &[], "4"), PoleOrder::Undecided);
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
