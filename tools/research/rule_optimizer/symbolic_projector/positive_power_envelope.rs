//! Optional cost restriction on actual, fixed-specialized source columns.
//! This is stronger than necessary cancellation: a coefficient could vanish on
//! a finite violating boundary. No source term or guard is removed here.
use super::{Result, array, checked, fixed, limit, require};
use rustred::{identity::IndexShift, sector::OrderingPolicy};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn cap(r: &Value) -> Result<Option<u64>> {
    r.get("max_positive_power_excess")
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| "max_positive_power_excess must be a nonnegative integer".into())
        })
        .transpose()
}

struct Ray {
    free: usize,
    parent: Vec<i64>,
    parent_a: i128,
}

fn positive_power(order: &OrderingPolicy, powers: &[i64]) -> Result<i128> {
    let key = checked(order.complexity_key(powers))?;
    let a = key
        .dots()
        .checked_add(key.propagators() as u128)
        .ok_or("positive-power sum overflow")?;
    checked(i128::try_from(a))
}

impl Ray {
    fn new(r: &Value, order: &OrderingPolicy) -> Result<Self> {
        for key in [
            "affine",
            "cuts",
            "max_numerator_rank",
            "max_positive_degree",
            "max_total_degree",
            "constraints",
        ] {
            require(
                r.get(key).is_none(),
                "positive-power envelope refuses additional domain restrictions",
            )?;
        }
        let mask = r["owner_mask"]
            .as_str()
            .ok_or("envelope owner mask required")?;
        require(
            (1..=16).contains(&mask.len()) && mask.bytes().all(|b| matches!(b, b'0' | b'1')),
            "envelope mask invalid",
        )?;
        let chart = r["chart"].as_object().ok_or("envelope chart required")?;
        require(
            chart
                .keys()
                .all(|k| matches!(k.as_str(), "lower" | "upper" | "fixed")),
            "positive-power envelope refuses affine or correlated charts",
        )?;
        let lower = array(&r["chart"], "lower")?;
        let upper = array(&r["chart"], "upper")?;
        require(
            lower.len() == mask.len() && upper.len() == mask.len(),
            "envelope chart arity differs",
        )?;
        let free = upper
            .iter()
            .enumerate()
            .filter_map(|(i, hi)| hi.is_null().then_some(i))
            .collect::<Vec<_>>();
        require(
            free.len() == 1 && mask.as_bytes()[free[0]] == b'1',
            "positive-power envelope needs one unbounded positive axis",
        )?;
        let free = free[0];
        let mut restrictions = BTreeMap::new();
        for (axis, value) in fixed(r)? {
            require(
                axis < mask.len() && axis != free && restrictions.insert(axis, value).is_none(),
                "envelope fixed axes invalid",
            )?;
        }
        require(
            restrictions.len() == mask.len() - 1,
            "positive-power envelope needs all other axes explicitly fixed",
        )?;
        let mut parent = Vec::with_capacity(mask.len());
        for i in 0..mask.len() {
            let lo = lower[i].as_u64().ok_or("envelope lower must be u64")?;
            let n = checked(i64::try_from(if mask.as_bytes()[i] == b'1' {
                i128::from(lo) + 1
            } else {
                -i128::from(lo)
            }))?;
            if i != free {
                require(
                    upper[i].as_u64() == Some(lo) && restrictions[&i] == n,
                    "envelope fixed coordinate differs from singleton chart",
                )?;
            }
            parent.push(n);
        }
        let parent_a = positive_power(order, &parent)?;
        Ok(Self {
            free,
            parent,
            parent_a,
        })
    }

    fn excess(&self, order: &OrderingPolicy, shift: &[i64]) -> Result<(i128, Vec<i64>)> {
        require(
            shift.len() == self.parent.len(),
            "envelope shift arity differs",
        )?;
        let child = self
            .parent
            .iter()
            .zip(shift)
            .map(|(&n, &s)| checked(n.checked_add(s).ok_or("envelope shifted power overflow")))
            .collect::<Result<Vec<_>>>()?;
        let delta = positive_power(order, &child)?
            .checked_sub(self.parent_a)
            .ok_or("envelope excess overflow")?;
        Ok((delta, child))
    }
}

pub struct Selection {
    pub columns: BTreeSet<IndexShift>,
    metadata: Value,
}

pub fn columns(
    r: &Value,
    order: &OrderingPolicy,
    universe: &BTreeSet<IndexShift>,
) -> Result<Option<Selection>> {
    let Some(cap) = cap(r)? else { return Ok(None) };
    require(
        universe.len() <= limit(r, "max_augmented_columns")?,
        "envelope universe exceeds column allowance",
    )?;
    let ray = Ray::new(r, order)?;
    let coordinates = universe
        .len()
        .checked_add(1)
        .and_then(|v| v.checked_mul(ray.parent.len()))
        .and_then(|v| v.checked_mul(3))
        .ok_or("envelope witness coordinate overflow")?;
    require(
        coordinates <= limit(r, "max_coordinate_cells")?,
        "envelope witness coordinates exceed allowance",
    )?;
    let mut columns = BTreeSet::new();
    let mut witnesses = Vec::new();
    for shift in universe {
        let (excess, child) = ray.excess(order, shift.values())?;
        if excess > i128::from(cap) {
            require(
                shift.values().iter().any(|&x| x != 0),
                "envelope cannot forbid target",
            )?;
            columns.insert(shift.clone());
            witnesses.push(json!({"shift":shift.values(),"child_at_lower_corner":child,"maximum_positive_power_excess":excess.to_string()}));
        }
    }
    Ok(Some(Selection {
        metadata: json!({
        "max_positive_power_excess":cap,"actual_universe_columns":universe.len(),
        "selected_columns":columns.len(),"free_axis":ray.free,"parent_at_lower_corner":ray.parent,
        "parent_positive_power":ray.parent_a.to_string(),"witnesses":witnesses,
        "scope":"One positive unbounded physical index; all other coordinates fixed; unshifted powers. Delta A = C + max(n+s,0) - n is nonincreasing, so its exact maximum is at the declared lower corner, including sign crossings.",
        "restriction_kind":"optional cost-envelope cancellation, not a necessary descent condition; whole-column banning is stronger than allowing finite-boundary coefficient zeros",
        "cost_gain_or_rule_authority":false}),
        columns,
    }))
}

pub fn annotate(mut report: Value, selected: Option<&Selection>, new_count: usize) -> Value {
    if let Some(selected) = selected {
        report["positive_power_envelope"] = selected.metadata.clone();
        report["positive_power_envelope"]["new_forbidden_columns"] = json!(new_count);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ray(mask: &str, lower: Value, upper: Value, fixed: Value) -> Value {
        json!({"owner_mask":mask,"chart":{"lower":lower,"upper":upper,"fixed":fixed},"max_positive_power_excess":0,
            "limits":{"max_augmented_columns":100,"max_coordinate_cells":1000}})
    }
    fn order() -> OrderingPolicy {
        OrderingPolicy::SpiredUncutV1
    }

    #[test]
    fn strict_optional_cap_and_default_annotation_identity() {
        assert_eq!(cap(&json!({})).unwrap(), None);
        assert_eq!(
            cap(&json!({"max_positive_power_excess":0})).unwrap(),
            Some(0)
        );
        for bad in [json!(null), json!(false), json!(-1), json!(0.5), json!("0")] {
            assert!(cap(&json!({"max_positive_power_excess":bad})).is_err());
        }
        let old = json!({"native":"unchanged"});
        assert_eq!(annotate(old.clone(), None, 0), old);
    }

    #[test]
    fn lower_corner_maximum_includes_all_crossings_and_fixed_activations() {
        let r = ray(
            "110",
            json!([2, 0, 0]),
            json!([null, 0, 0]),
            json!([[1, 1], [2, 0]]),
        );
        let exact = Ray::new(&r, &order()).unwrap();
        for shift in [[-4, 0, 5], [-3, 0, 4], [-2, 0, 3], [1, 0, 0], [0, -1, 1]] {
            let (maximum, _) = exact.excess(&order(), &shift).unwrap();
            for n in 3..=20 {
                let parent = [n, 1, 0];
                let child = [n + shift[0], 1 + shift[1], shift[2]];
                assert!(
                    positive_power(&order(), &child).unwrap()
                        - positive_power(&order(), &parent).unwrap()
                        <= maximum
                );
            }
        }
        assert_eq!(exact.excess(&order(), &[-4, 0, 5]).unwrap().0, 2);
        assert_eq!(exact.excess(&order(), &[0, 0, 1]).unwrap().0, 1);
        assert_eq!(exact.excess(&order(), &[0, 0, 0]).unwrap().0, 0);
    }

    #[test]
    fn coordinate_permutation_preserves_positive_power_decision() {
        let a = Ray::new(
            &ray(
                "110",
                json!([2, 0, 0]),
                json!([null, 0, 0]),
                json!([[1, 1], [2, 0]]),
            ),
            &order(),
        )
        .unwrap();
        let b = Ray::new(
            &ray(
                "011",
                json!([0, 0, 2]),
                json!([0, 0, null]),
                json!([[0, 0], [1, 1]]),
            ),
            &order(),
        )
        .unwrap();
        for s in [[-4, 0, 5], [0, -1, 1], [1, 2, -3]] {
            assert_eq!(
                a.excess(&order(), &s).unwrap().0,
                b.excess(&order(), &[s[2], s[1], s[0]]).unwrap().0
            );
        }
    }

    #[test]
    fn unsupported_or_inconsistent_geometry_refuses() {
        for r in [
            ray("1", json!([0]), json!([3]), json!([])),
            ray("11", json!([0, 0]), json!([null, null]), json!([])),
            ray("0", json!([0]), json!([null]), json!([])),
            ray("11", json!([0, 0]), json!([null, 0]), json!([[1, 2]])),
        ] {
            assert!(Ray::new(&r, &order()).is_err());
        }
        let r = ray("1", json!([2]), json!([null]), json!([]));
        for key in ["affine", "cuts", "max_total_degree", "constraints"] {
            let mut bad = r.clone();
            bad[key] = json!([]);
            assert!(Ray::new(&bad, &order()).is_err());
        }
        let mut bad = r;
        bad["chart"]["correlated"] = json!([]);
        assert!(Ray::new(&bad, &order()).is_err());
    }

    #[test]
    fn checked_overflow_and_witness_budget_refuse() {
        let r = ray("1", json!([2]), json!([null]), json!([]));
        let a = Ray::new(&r, &order()).unwrap();
        assert!(a.excess(&order(), &[i64::MAX]).is_err());
        assert!(a.excess(&order(), &[]).is_err());
        let mut bad = r.clone();
        bad["chart"]["lower"] = json!([u64::MAX]);
        assert!(Ray::new(&bad, &order()).is_err());
        let mut bad = r;
        bad["limits"]["max_coordinate_cells"] = json!(2);
        assert!(
            columns(&bad, &order(), &BTreeSet::new())
                .err()
                .unwrap()
                .contains("witness coordinates")
        );
    }

    #[test]
    fn actual_universe_only_no_invented_columns_and_complete_proof_unchanged() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        r["fresh_original_source_certificate"] = json!(true);
        let (before, before_bytes) = crate::run::<1>(&bytes, &r, true).unwrap();
        r["max_positive_power_excess"] = json!(0);
        let (after, after_bytes) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(after["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(before_bytes, after_bytes);
        assert_eq!(before["attempts"], after["attempts"]); // Includes every original/discovery guard and source.
        assert_eq!(after["positive_power_envelope"]["selected_columns"], 1);
        assert_eq!(after["positive_power_envelope"]["new_forbidden_columns"], 0);
        r["forbid_cofinally_higher_columns"] = json!(false);
        let (envelope_only, envelope_bytes) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(envelope_only["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(envelope_bytes, before_bytes);
        assert_eq!(
            envelope_only["positive_power_envelope"]["new_forbidden_columns"],
            1
        );
        assert_eq!(
            envelope_only["attempts"][0]["forbidden_shifts"],
            json!([[1]])
        );
        r["max_positive_power_excess"] = json!(1);
        let (allowed, _) = crate::run::<1>(&bytes, &r, false).unwrap();
        assert_eq!(allowed["positive_power_envelope"]["selected_columns"], 0);
        r["max_positive_power_excess"] = json!(0);
        r["sources"] = json!([r["sources"][1].clone()]);
        let (backward, _) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(backward["positive_power_envelope"]["selected_columns"], 0);
        assert_eq!(backward["positive_power_envelope"]["witnesses"], json!([]));
        // A generic forward +1 is forbidden by the policy, but is absent from
        // this actual backward-only universe and must never be manufactured.
    }
}
