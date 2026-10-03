//! Opt-in necessary-column nomination on a single unbounded coordinate ray.
//! This is not a rule proof or a zero-sector quotient. Full sources stay intact.
use super::{Result, checked, integers, require};
use rustred::{
    identity::IndexShift,
    sector::{Mask, OrderingPolicy, SectorInteriorDomain},
};
use serde_json::{Value, json};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

pub fn enabled(r: &Value) -> Result<bool> {
    match r.get("forbid_cofinally_higher_columns") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("forbid_cofinally_higher_columns must be boolean".into()),
    }
}

pub struct Ray {
    sector: Mask,
    order: OrderingPolicy,
    free: usize,
    lower: u64,
    fixed: BTreeMap<usize, i64>,
}
impl Ray {
    pub fn new(r: &Value, order: &OrderingPolicy) -> Result<Self> {
        require(
            *order == OrderingPolicy::SpiredUncutV1,
            "cofinal nomination only supports the plain persisted Spired uncut order",
        )?;
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
                "cofinal nomination refuses additional domain restrictions",
            )?;
        }
        let text = r["owner_mask"]
            .as_str()
            .ok_or("cofinal owner mask required")?;
        require(
            !text.is_empty() && text.bytes().all(|b| matches!(b, b'0' | b'1')),
            "cofinal mask invalid",
        )?;
        let n = text.len();
        let sector = checked(Mask::try_new(text.bytes().map(|b| b == b'1')))?;
        let chart = r["chart"].as_object().ok_or("cofinal chart required")?;
        require(
            chart
                .keys()
                .all(|k| matches!(k.as_str(), "lower" | "upper" | "fixed")),
            "cofinal nomination refuses affine or correlated chart descriptors",
        )?;
        let lower = chart
            .get("lower")
            .and_then(Value::as_array)
            .ok_or("cofinal lower required")?;
        let upper = chart
            .get("upper")
            .and_then(Value::as_array)
            .ok_or("cofinal upper required")?;
        require(
            lower.len() == n && upper.len() == n,
            "cofinal chart arity differs",
        )?;
        let free: Vec<_> = (0..n).filter(|&i| upper[i].is_null()).collect();
        require(
            free.len() == 1,
            "cofinal nomination needs exactly one genuinely unbounded axis",
        )?;
        let free = free[0];
        let mut fixed = BTreeMap::new();
        for pair in chart
            .get("fixed")
            .and_then(Value::as_array)
            .ok_or("cofinal fixed required")?
        {
            let pair = integers(pair, 2)?;
            let axis = checked(usize::try_from(pair[0]))?;
            require(
                axis < n && axis != free && fixed.insert(axis, pair[1]).is_none(),
                "cofinal fixed axes invalid",
            )?;
        }
        require(
            fixed.len() == n - 1,
            "cofinal nomination requires every finite coordinate explicitly fixed",
        )?;
        for i in 0..n {
            let x = lower[i].as_u64().ok_or("cofinal local lower must be u64")?;
            if i == free {
                continue;
            }
            require(
                upper[i].as_u64() == Some(x),
                "cofinal finite coordinate is not singleton",
            )?;
            let physical = if sector.active_bits()[i] {
                1 + i128::from(x)
            } else {
                -i128::from(x)
            };
            require(
                i128::from(fixed[&i]) == physical,
                "cofinal fixed value differs from chart",
            )?;
        }
        Ok(Self {
            sector,
            order: order.clone(),
            free,
            lower: lower[free].as_u64().ok_or("cofinal lower required")?,
            fixed,
        })
    }

    /// Exact sign stabilization plus the native n-independent structural key.
    /// The finite i64 interior is only a checked carrier. The infinite-ray
    /// conclusion uses cancellation of the common n-part of the plain Spired
    /// key AFTER exact sign stabilization, not sampling or machine clipping.
    pub fn classify(&self, shift: &[i64]) -> Result<Option<Value>> {
        let n = self.sector.arity();
        require(shift.len() == n, "cofinal shift arity differs")?;
        if shift.iter().all(|&s| s == 0) {
            return Ok(None);
        }
        let active = self.sector.active_bits()[self.free];
        let crossing = if active {
            -i128::from(shift[self.free])
        } else {
            i128::from(shift[self.free])
        };
        let start = i128::from(self.lower).max(0).max(crossing);
        let mut parent = (0..n)
            .map(|i| self.fixed.get(&i).copied().unwrap_or(0))
            .collect::<Vec<_>>();
        parent[self.free] = checked(i64::try_from(if active { 1 + start } else { -start }))?;
        let child = parent
            .iter()
            .zip(shift)
            .map(|(&a, &b)| checked(i64::try_from(i128::from(a) + i128::from(b))))
            .collect::<Result<Vec<_>>>()?;
        let child_mask = checked(Mask::try_from_indices(&child))?;
        require(
            child_mask.active_bits()[self.free] == active,
            "cofinal sign-stability invariant failed",
        )?;
        let (comparison, kind) = if child_mask == self.sector {
            let zero = vec![0; n];
            let domain = checked(SectorInteriorDomain::try_maximal_for_shifts(
                self.sector.clone(),
                &[zero.as_slice(), shift],
            ))?;
            require(
                checked(domain.contains(&parent))?,
                "cofinal threshold outside checked native carrier",
            )?;
            let native = checked(self.order.compare_shifts_on_domain(&domain, shift, &zero))?;
            let structural = checked(self.order.shift_complexity_key(&self.sector, shift))?.cmp(
                &checked(self.order.shift_complexity_key(&self.sector, &zero))?,
            );
            require(
                native == structural,
                "cofinal native carrier/structural key mismatch",
            )?;
            (
                native,
                "exact native n-independent shift key after uniform sign stabilization",
            )
        } else {
            (
                checked(
                    self.order
                        .compare_support(child_mask.active_bits(), self.sector.active_bits()),
                )?,
                "exact native support-primary comparison after uniform sign stabilization",
            )
        };
        require(
            comparison != Ordering::Equal,
            "distinct actual shift unexpectedly has equal native key",
        )?;
        if comparison == Ordering::Less {
            return Ok(None);
        }
        Ok(Some(
            json!({"shift":shift,"free_axis":self.free,"cofinal_local_lower":checked(u64::try_from(start))?,
            "child_support":child_mask.active_bits(),"native_carrier_parent":parent,"native_carrier_child":child,
            "reason":kind,"infinite_ray_reason":"All fixed coordinates stay fixed; the translated free coordinate has its parent sign forever after the exact threshold. The native structural key difference is then independent of the free index. A nonzero rational coefficient cannot vanish on that infinite integer tail.",
            "zero_sector_quotient":false,"rule_authority":false}),
        ))
    }
}

pub fn columns(
    r: &Value,
    order: &OrderingPolicy,
    universe: &BTreeSet<IndexShift>,
) -> Result<(BTreeSet<IndexShift>, Vec<Value>)> {
    require(
        universe.len() <= super::limit(r, "max_augmented_columns")?,
        "cofinal universe exceeds column allowance",
    )?;
    let ray = Ray::new(r, order)?;
    // A worst-case witness retains shift, child support, parent and child
    // coordinate vectors. Charge all possible witnesses before constructing
    // per-column JSON; the report-byte cap and outer RSS guard still apply.
    let coordinate_bound = universe
        .len()
        .checked_mul(ray.sector.arity())
        .and_then(|n| n.checked_mul(4))
        .ok_or("cofinal witness coordinate overflow")?;
    require(
        coordinate_bound <= super::limit(r, "max_coordinate_cells")?,
        "cofinal witness coordinates exceed allowance",
    )?;
    let mut result = BTreeSet::new();
    let mut witnesses = Vec::new();
    for shift in universe {
        if let Some(witness) = ray.classify(shift.values())? {
            result.insert(shift.clone());
            witnesses.push(witness);
        }
    }
    Ok((result, witnesses))
}

pub fn annotate(mut report: Value, enabled: bool, witnesses: &[Value], new_count: usize) -> Value {
    if enabled {
        report["cofinal_higher_column_witnesses"] = json!(witnesses);
        report["cofinal_higher_new_count"] = json!(new_count);
        report["cofinal_scope"] = json!(
            "One exact unbounded local coordinate; plain Spired, unprojected strict descent. Finite native carrier supports the n-independent structural argument and is not an infinite-domain certificate by itself."
        );
    }
    report
}

#[cfg(test)]
#[path = "cofinal_tests.rs"]
mod tests;
