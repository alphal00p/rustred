//! THE reading of a rule record's `domain_bounds` (lane engine-line, 2026-11-15): one dependency-free function used by
//! BOTH rr_proof (rustred-proof core.rs) and rr_node (rustred-core node/bounds.rs, included with `#[path]`), so the two
//! cannot read the field differently.
//!
//! `"domain_bounds": {"min": [int|null; n], "max": [int|null; n]}` bounds the CASE VARIABLES `n_i` inclusively. The
//! target pattern's slot `i` is `n_i + s_i` (shift `s_i`, 0 for a canonical target) or a constant. Inside the chart the
//! variable ranges over `n_i >= 1 - s_i` on a line slot and `n_i <= -s_i` on a non-line slot, so a bound cuts a FINITE
//! set: on a line slot only `min = m` (excludes `1 - s_i .. m - 1`), on a non-line slot only `max = M` (excludes
//! `M + 1 ..= -s_i`). A bound that would cut an infinite set is refused; at most `max_excluded` values per free slot.
//! A FIXED slot (constant in `fixed`) whose value violates its bound excludes the whole case.
//! (Before 2026-11-15 rr_node started the excluded range at 1 / 0 whatever the shift, rr_proof compared the variable to
//! the bound directly: they disagreed on shifted pattern targets. No table carried the field then.)

/// Upper limit of excluded values per free slot (a bound further than this from the chart is refused).
pub const MAX_EXCLUDED_PER_SLOT: i64 = 64;

/// What a record's bounds exclude, inside the chart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoundExclusions {
    /// a fixed slot violates its bound: the rule's case is empty
    pub whole_case: bool,
    /// per free slot, the excluded values of its case variable (finite)
    pub per_slot: Vec<(usize, Vec<i64>)>,
}

impl BoundExclusions {
    /// Whether the case variables `kv` (slot -> value) are excluded by the bounds.
    pub fn excludes(&self, kv: impl Fn(usize) -> i64) -> bool {
        self.whole_case || self.per_slot.iter().any(|(i, vs)| vs.contains(&kv(*i)))
    }
}

/// `line[i]`: slot i is a chart line; `shift[i]`: Some(s) when the target slot is `n_i + s`, None when it is a constant;
/// `fixed[i]`: the record's fixed value; `min` / `max`: the bounds (None = no bound).
pub fn bound_exclusions(
    line: &[bool],
    shift: &[Option<i64>],
    fixed: &[Option<i64>],
    min: &[Option<i64>],
    max: &[Option<i64>],
    max_excluded: i64,
) -> Result<BoundExclusions, String> {
    let n = line.len();
    if [shift.len(), fixed.len(), min.len(), max.len()]
        .iter()
        .any(|&l| l != n)
    {
        return Err(format!("domain_bounds: arrays of the chart's arity {n}"));
    }
    let mut out = BoundExclusions::default();
    for i in 0..n {
        if line[i] && max[i].is_some() {
            return Err(format!(
                "domain_bounds: a max on line slot {i} would cut an infinite set"
            ));
        }
        if !line[i] && min[i].is_some() {
            return Err(format!(
                "domain_bounds: a min on non-line slot {i} would cut an infinite set"
            ));
        }
        if let Some(f) = fixed[i] {
            if min[i].is_some_and(|m| f < m) || max[i].is_some_and(|m| f > m) {
                out.whole_case = true;
            }
            continue;
        }
        if min[i].is_none() && max[i].is_none() {
            continue;
        }
        let s = shift[i].ok_or_else(|| {
            format!("domain_bounds: slot {i} is free but the target slot is not n{i}+s")
        })?;
        let vals: Vec<i64> = if line[i] {
            let (lo, m) = (1 - s, min[i].unwrap());
            if m - lo > max_excluded {
                return Err(format!(
                    "domain_bounds: min {m} on slot {i} is too far from the chart"
                ));
            }
            (lo..m).collect()
        } else {
            let (hi, m) = (-s, max[i].unwrap());
            if hi - m > max_excluded {
                return Err(format!(
                    "domain_bounds: max {m} on slot {i} is too far from the chart"
                ));
            }
            (m + 1..=hi).collect()
        };
        if !vals.is_empty() {
            out.per_slot.push((i, vals));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod bounds_rule_tests {
    use super::*;

    #[test]
    fn shifted_targets_and_refusals() {
        let line = [true, true, false];
        // canonical target I(n0,n1,n2): min 3 on n0 excludes n0 = 1, 2; max -2 on n2 excludes n2 = -1, 0
        let e = bound_exclusions(
            &line,
            &[Some(0); 3],
            &[None; 3],
            &[Some(3), None, None],
            &[None, None, Some(-2)],
            64,
        )
        .unwrap();
        assert_eq!(e.per_slot, vec![(0, vec![1, 2]), (2, vec![-1, 0])]);
        // shifted target I(n0+1, n1, n2-1): n0 >= 0 in the chart, so min 2 excludes n0 = 0, 1; n2 <= 1, max 0 excludes 1
        let e = bound_exclusions(
            &line,
            &[Some(1), Some(0), Some(-1)],
            &[None; 3],
            &[Some(2), None, None],
            &[None, None, Some(0)],
            64,
        )
        .unwrap();
        assert_eq!(e.per_slot, vec![(0, vec![0, 1]), (2, vec![1])]);
        assert!(
            e.excludes(|i| [0, 5, 0][i])
                && e.excludes(|i| [3, 5, 1][i])
                && !e.excludes(|i| [2, 5, 0][i])
        );
        // a fixed slot violating its bound empties the case
        let e = bound_exclusions(
            &line,
            &[Some(0); 3],
            &[Some(1), None, None],
            &[Some(2), None, None],
            &[None; 3],
            64,
        )
        .unwrap();
        assert!(e.whole_case && e.excludes(|_| 7));
        // refusals: infinite cuts, too far, a constant target slot that is free
        assert!(
            bound_exclusions(
                &line,
                &[Some(0); 3],
                &[None; 3],
                &[None; 3],
                &[Some(5), None, None],
                64
            )
            .is_err()
        );
        assert!(
            bound_exclusions(
                &line,
                &[Some(0); 3],
                &[None; 3],
                &[None, None, Some(-1)],
                &[None; 3],
                64
            )
            .is_err()
        );
        assert!(
            bound_exclusions(
                &line,
                &[Some(0); 3],
                &[None; 3],
                &[Some(70), None, None],
                &[None; 3],
                64
            )
            .is_err()
        );
        assert!(
            bound_exclusions(
                &line,
                &[None, Some(0), Some(0)],
                &[None; 3],
                &[Some(2), None, None],
                &[None; 3],
                64
            )
            .is_err()
        );
    }
}
