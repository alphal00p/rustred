//! Optional rule-record field `domain_bounds` (lane engine-line, 2026-11-14; for IMPORTED rules such as Astra's T162
//! bank, whose valid domain is the chart orthant tightened by the record's `recenter`).
//!
//! `"domain_bounds": {"min": [int|null; n], "max": [int|null; n]}` bounds the rule's case variables `n_i` (the slots of
//! the target pattern; for a canonical target `I(n0,..)` = the key's slots), inclusive. Absent or null = no bound =
//! today's behaviour (every existing table is unchanged). The engine generator never writes it.
//!
//! Every consumer applies the bounds as EXTRA exception branches derived here, so that the existing exception code path
//! (first match, import into a package, replay sampling, proof `in_case`) needs no new semantics: on a chart LINE slot
//! (`n >= 1`) only a lower bound `min = m` is accepted and excludes `n = 1, .., m - 1`; on a non-line slot (`n <= 0`)
//! only an upper bound `max = M` excludes `n = M + 1, .., 0`; both are finite sets. A bound that would cut an infinite
//! set (a max on a line, a min on a numerator) is refused. On a FIXED slot a violated bound excludes the whole case.
//!
//! ONE refusal rule with the campaign core's importer (lane engfix4, re-audit mn-2; crates/rustred-campaign-core
//! import/load.rs): `domain_bounds` must be an object with only `min` / `max` (each null or an array of the arity of
//! integers / nulls); a `min` only on a line slot and a `max` only on a non-line slot, FIXED OR NOT (before engfix4 a
//! fixed slot took either kind here and was refused by the core); on a FREE slot at most [`MAX_EXCLUDED_PER_SLOT`]
//! excluded values (the core refuses the same); a non-object value is refused (it was silently ignored here).

use serde_json::Value;

/// Upper limit of excluded values per slot (a bound further than this from the chart is refused).
pub const MAX_EXCLUDED_PER_SLOT: i64 = 64;

/// `n<i> - v` in the engine's printing style (`n3`, `-2+n3`, `1+n3`).
fn eq_text(i: usize, v: i64) -> String {
    match v {
        0 => format!("n{i}"),
        v if v > 0 => format!("-{v}+n{i}"),
        v => format!("{}+n{i}", -v),
    }
}

/// The exception branches (each a one-polynomial AND-branch) that a record's `domain_bounds` adds; empty when absent.
pub fn bound_exclusions(record: &Value) -> Result<Vec<Vec<String>>, String> {
    let Some(b) = record.get("domain_bounds").filter(|b| !b.is_null()) else {
        return Ok(Vec::new());
    };
    let Some(o) = b.as_object() else {
        return Err("domain_bounds: not an object".into());
    };
    if let Some(k) = o.keys().find(|k| *k != "min" && *k != "max") {
        return Err(format!("domain_bounds: unknown field {k}"));
    }
    let chart: Vec<bool> = record["chart"]
        .as_str()
        .ok_or("domain_bounds: rule without chart")?
        .chars()
        .map(|c| c == '1')
        .collect();
    let n = chart.len();
    let list = |k: &str| -> Result<Vec<Option<i64>>, String> {
        match b.get(k) {
            None | Some(Value::Null) => Ok(vec![None; n]),
            Some(Value::Array(a)) if a.len() == n => a
                .iter()
                .map(|x| match x {
                    Value::Null => Ok(None),
                    x => x
                        .as_i64()
                        .map(Some)
                        .ok_or(format!("domain_bounds.{k}: integers or null")),
                })
                .collect(),
            _ => Err(format!(
                "domain_bounds.{k}: an array of the chart's arity {n}"
            )),
        }
    };
    let (min, max) = (list("min")?, list("max")?);
    let fixed: Vec<Option<i64>> = record["fixed"]
        .as_array()
        .map(|a| a.iter().map(Value::as_i64).collect())
        .unwrap_or_else(|| vec![None; n]);
    let mut out = Vec::new();
    for i in 0..n {
        // the kind of a bound depends on the slot kind only (fixed or not): the core's rule (mn-2)
        if chart[i] && max[i].is_some() {
            return Err(format!(
                "domain_bounds: a max on line slot {i} would cut an infinite set"
            ));
        }
        if !chart[i] && min[i].is_some() {
            return Err(format!(
                "domain_bounds: a min on non-line slot {i} would cut an infinite set"
            ));
        }
        if let Some(f) = fixed.get(i).copied().flatten() {
            if min[i].is_some_and(|m| f < m) || max[i].is_some_and(|m| f > m) {
                // the fixed value violates the bound: n_i == f on the whole case, so this branch excludes it all
                out.push(vec![eq_text(i, f)]);
            }
            continue;
        }
        if chart[i] {
            if let Some(m) = min[i] {
                if m - 1 > MAX_EXCLUDED_PER_SLOT {
                    return Err(format!(
                        "domain_bounds: min {m} on slot {i} is too far from the chart"
                    ));
                }
                for v in 1..m {
                    out.push(vec![eq_text(i, v)]);
                }
            }
        } else {
            if let Some(m) = max[i] {
                if -m > MAX_EXCLUDED_PER_SLOT {
                    return Err(format!(
                        "domain_bounds: max {m} on slot {i} is too far from the chart"
                    ));
                }
                for v in (m + 1)..=0 {
                    out.push(vec![eq_text(i, v)]);
                }
            }
        }
    }
    Ok(out)
}

/// The bounds that the T162 bank's qualified domain implies (lines `n_i >= max(1, 1 - recenter_i)`, numerators
/// `n_j <= min(0, -recenter_j)`), as a `domain_bounds` value; None when the recenter tightens nothing.
pub fn bounds_from_recenter(record: &Value) -> Option<Value> {
    let chart: Vec<bool> = record["chart"]
        .as_str()?
        .chars()
        .map(|c| c == '1')
        .collect();
    let rc: Vec<i64> = record["recenter"]
        .as_array()?
        .iter()
        .map(|x| x.as_i64().unwrap_or(0))
        .collect();
    if rc.len() != chart.len() {
        return None;
    }
    let mut min = vec![Value::Null; chart.len()];
    let mut max = vec![Value::Null; chart.len()];
    let mut any = false;
    for i in 0..chart.len() {
        if chart[i] && 1 - rc[i] > 1 {
            min[i] = (1 - rc[i]).into();
            any = true;
        } else if !chart[i] && -rc[i] < 0 {
            max[i] = (-rc[i]).into();
            any = true;
        }
    }
    any.then(|| serde_json::json!({"min": min, "max": max}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_bounds_add_nothing_and_finite_cuts_only() {
        let r = json!({"chart":"110","fixed":[null,1,null]});
        assert!(bound_exclusions(&r).unwrap().is_empty());
        let r = json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"min":[3,null,null],"max":[null,null,-2]}});
        assert_eq!(
            bound_exclusions(&r).unwrap(),
            vec![
                vec!["-1+n0".to_string()],
                vec!["-2+n0".into()],
                vec!["1+n2".into()],
                vec!["n2".into()]
            ]
        );
        let bad =
            json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"max":[4,null,null]}});
        assert!(bound_exclusions(&bad).is_err());
        let fixed_violated =
            json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"min":[null,2,null]}});
        assert_eq!(
            bound_exclusions(&fixed_violated).unwrap(),
            vec![vec!["-1+n1".to_string()]]
        );
        // engfix4 mn-2 (one rule with the campaign core): the bound kind follows the slot kind on FIXED slots too,
        // a non-object value is refused, a far bound on a fixed slot is a comparison (no enumeration, no limit)
        let min_on_fixed_numerator =
            json!({"chart":"110","fixed":[null,1,0],"domain_bounds":{"min":[null,null,-1]}});
        assert!(bound_exclusions(&min_on_fixed_numerator).is_err());
        let max_on_fixed_line =
            json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"max":[null,3,null]}});
        assert!(bound_exclusions(&max_on_fixed_line).is_err());
        let not_object = json!({"chart":"110","fixed":[null,1,null],"domain_bounds":[1,2,3]});
        assert!(bound_exclusions(&not_object).is_err());
        let far_free =
            json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"min":[100,null,null]}});
        assert!(bound_exclusions(&far_free).is_err());
        let far_fixed =
            json!({"chart":"110","fixed":[null,1,null],"domain_bounds":{"min":[null,100,null]}});
        assert_eq!(
            bound_exclusions(&far_fixed).unwrap(),
            vec![vec!["-1+n1".to_string()]]
        );
        let rc = json!({"chart":"110","recenter":[-1,0,1]});
        assert_eq!(
            bounds_from_recenter(&rc).unwrap(),
            json!({"min":[2,null,null],"max":[null,null,-1]})
        );
    }
}
