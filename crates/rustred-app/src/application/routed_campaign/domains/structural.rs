//! Conservative shift census, not guard validation or a closure certificate.
use rustred::solver::{OwnerSuccessorRegion, OwnerSuccessorTransition};
use serde_json::{Value, json};

const MAX_EXAMPLES_PER_KIND: usize = 4;

#[derive(Default)]
pub(super) struct Census {
    regions: usize,
    same_support: usize,
    strict_pinch: usize,
    unsupported: usize,
    positive_delta_p: usize,
    max_shift_l1: Option<u128>,
    max_pinch_shift_l1: Option<u128>,
    max_same_support_delta_p: Option<i128>,
    unsupported_examples: Vec<Value>,
    positive_delta_p_examples: Vec<Value>,
}

impl Census {
    pub(super) fn observe<const N: usize>(
        &mut self,
        region: &OwnerSuccessorRegion<'_, N>,
    ) -> Result<(), &'static str> {
        self.record(
            region.shift(),
            region.source_sector(),
            region.transition(),
            || {
                json!({
                    "owner":super::mask(region.source_sector()),
                    "target_sector":super::mask(region.target_sector()),
                    "batch":region.batch_ordinal(), "rule":region.rule_ordinal(),
                    "term":region.term_ordinal(), "shift":region.shift().as_slice(),
                    "source_local_lower":region.source_local_lower(),
                    "source_local_upper":region.source_local_upper(),
                    "equalities":region.equalities().len(),
                    "excluded_zero_conjunctions":region.excluded_zero_conjunctions().len(),
                    "original_denominators":region.original_denominators().count(),
                    "source_conditions":region.source_conditions().len(),
                    "exact_zero_sector":region.is_exact_zero_sector(),
                    "diagnostic_only":true,
                })
            },
        )
    }

    fn record(
        &mut self,
        shift: &[i64],
        active: &[bool],
        transition: OwnerSuccessorTransition,
        witness: impl FnOnce() -> Value,
    ) -> Result<(), &'static str> {
        if shift.len() != active.len() {
            return Err("structural census arity mismatch");
        }
        let l1 = shift.iter().try_fold(0u128, |sum, value| {
            sum.checked_add(u128::from(value.unsigned_abs()))
                .ok_or("structural census shift norm overflow")
        })?;
        let delta_p = if transition == OwnerSuccessorTransition::SameSupport {
            Some(shift.iter().zip(active).try_fold(0i128, |sum, (&s, &a)| {
                sum.checked_add(if a { i128::from(s) } else { -i128::from(s) })
                    .ok_or("structural census potential delta overflow")
            })?)
        } else {
            None
        };
        let next_regions = increment(self.regions)?;
        let count = match transition {
            OwnerSuccessorTransition::SameSupport => &mut self.same_support,
            OwnerSuccessorTransition::StrictPinch => &mut self.strict_pinch,
            OwnerSuccessorTransition::UnsupportedSupportChange => &mut self.unsupported,
        };
        let next_count = increment(*count)?;
        let positive = delta_p.is_some_and(|delta| delta > 0);
        let next_positive = if positive {
            increment(self.positive_delta_p)?
        } else {
            self.positive_delta_p
        };
        // No mutation until all fallible arithmetic for this observation passed.
        *count = next_count;
        self.regions = next_regions;
        self.positive_delta_p = next_positive;
        self.max_shift_l1 = Some(self.max_shift_l1.map_or(l1, |old| old.max(l1)));
        if transition == OwnerSuccessorTransition::StrictPinch {
            self.max_pinch_shift_l1 = Some(self.max_pinch_shift_l1.map_or(l1, |old| old.max(l1)));
        }
        if let Some(delta) = delta_p {
            self.max_same_support_delta_p = Some(
                self.max_same_support_delta_p
                    .map_or(delta, |old| old.max(delta)),
            );
        }
        let examples = if positive {
            Some(&mut self.positive_delta_p_examples)
        } else if transition == OwnerSuccessorTransition::UnsupportedSupportChange {
            Some(&mut self.unsupported_examples)
        } else {
            None
        };
        if let Some(examples) = examples
            && examples.len() < MAX_EXAMPLES_PER_KIND
        {
            examples.push(witness());
        }
        Ok(())
    }

    pub(super) fn document(&self, complete: bool, rank: Option<u32>) -> Value {
        let authoritative = complete && rank.is_none();
        json!({
            "scan_complete":complete,
            "scope":if rank.is_none() { "unbounded_saved_rule_geometry" } else { "rank_bounded_saved_rule_geometry" },
            "requested_max_numerator_rank":rank,
            "observed_callback_regions":self.regions,
            "observed_max_shift_l1":self.max_shift_l1.map(|n|n.to_string()),
            "observed_max_strict_pinch_shift_l1":self.max_pinch_shift_l1.map(|n|n.to_string()),
            "complete_unbounded_shift_l1_bound":authoritative.then(||self.max_shift_l1.unwrap_or(0).to_string()),
            "potential_same_support_regions":self.same_support,
            "potential_strict_pinch_regions":self.strict_pinch,
            "potential_unsupported_support_change_regions":self.unsupported,
            "same_support_max_delta_p":self.max_same_support_delta_p.map(|n|n.to_string()),
            "potential_same_support_positive_delta_p_regions":self.positive_delta_p,
            "unsupported_support_change_examples":self.unsupported_examples,
            "same_support_positive_delta_p_examples":self.positive_delta_p_examples,
            "max_examples_per_kind":MAX_EXAMPLES_PER_KIND,
            "counter_semantics":"potential sign regions, not unique terms or witnessed transitions",
            "bound_semantics":"all scanned non-identically-zero RHS shifts only when complete and unbounded; otherwise observations are scoped or prefix-only",
            "guard_satisfiability_decided":false,
            "first_applicable_priority_resolved":false,
            "source_validity_proved":false,
            "family_closure_claim":false,
        })
    }
}

fn increment(value: usize) -> Result<usize, &'static str> {
    value
        .checked_add(1)
        .ok_or("structural census count overflow")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_observes_all_shifts_not_only_a_group_example() {
        let mut census = Census::default();
        for shift in [[-1, 0], [-7, 0]] {
            census
                .record(
                    &shift,
                    &[true, false],
                    OwnerSuccessorTransition::SameSupport,
                    || json!({}),
                )
                .unwrap();
        }
        let document = census.document(true, None);
        assert_eq!(document["observed_max_shift_l1"], "7");
        assert_eq!(document["complete_unbounded_shift_l1_bound"], "7");
        assert_eq!(document["same_support_max_delta_p"], "-1");
        assert_eq!(document["potential_same_support_regions"], 2);
    }

    #[test]
    fn census_potential_uses_both_positive_and_numerator_powers() {
        let mut census = Census::default();
        census
            .record(
                &[-2, 0, -1],
                &[true, true, false],
                OwnerSuccessorTransition::SameSupport,
                || json!({}),
            )
            .unwrap();
        let document = census.document(true, None);
        assert_eq!(document["same_support_max_delta_p"], "-1"); // Rank grows by one.
        assert_eq!(
            document["potential_same_support_positive_delta_p_regions"],
            0
        );
        assert!(
            document["same_support_positive_delta_p_examples"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn census_large_signed_shifts_and_bounded_obstruction_examples() {
        let mut census = Census::default();
        for _ in 0..10 {
            census
                .record(
                    &[i64::MIN],
                    &[false],
                    OwnerSuccessorTransition::SameSupport,
                    || json!({"diagnostic_only":true}),
                )
                .unwrap();
            census
                .record(
                    &[1],
                    &[false],
                    OwnerSuccessorTransition::UnsupportedSupportChange,
                    || json!({"diagnostic_only":true}),
                )
                .unwrap();
        }
        let document = census.document(true, None);
        assert_eq!(document["observed_max_shift_l1"], "9223372036854775808");
        assert_eq!(document["same_support_max_delta_p"], "9223372036854775808");
        assert_eq!(
            document["potential_same_support_positive_delta_p_regions"],
            10
        );
        assert_eq!(document["potential_unsupported_support_change_regions"], 10);
        for key in [
            "same_support_positive_delta_p_examples",
            "unsupported_support_change_examples",
        ] {
            assert_eq!(
                document[key].as_array().unwrap().len(),
                MAX_EXAMPLES_PER_KIND
            );
        }
    }

    #[test]
    fn census_only_complete_unbounded_scans_authorize_a_bound() {
        let mut census = Census::default();
        census
            .record(
                &[-2],
                &[true],
                OwnerSuccessorTransition::StrictPinch,
                || json!({}),
            )
            .unwrap();
        for (complete, rank) in [(false, None), (false, Some(2)), (true, Some(2))] {
            let document = census.document(complete, rank);
            assert!(document["complete_unbounded_shift_l1_bound"].is_null());
            assert_eq!(document["observed_max_shift_l1"], "2");
            assert!(document["same_support_max_delta_p"].is_null());
        }
        let empty = Census::default().document(true, None);
        assert!(empty["observed_max_shift_l1"].is_null());
        assert_eq!(empty["complete_unbounded_shift_l1_bound"], "0");
    }

    #[test]
    fn census_count_overflow_does_not_commit_a_partial_observation() {
        let mut census = Census {
            regions: usize::MAX,
            ..Default::default()
        };
        assert!(
            census
                .record(
                    &[1],
                    &[true],
                    OwnerSuccessorTransition::SameSupport,
                    || json!({})
                )
                .is_err()
        );
        assert_eq!(census.same_support, 0);
        assert!(census.max_shift_l1.is_none());
    }
}
