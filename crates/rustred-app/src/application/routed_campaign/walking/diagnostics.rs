//! Bounded diagnostics for optional coefficient support classification.
//! These are not missing-rule frontiers or retained exact coefficient predicates.
use rustred::algebra::IndexedAlgebraError;
use rustred::solver::{OwnerAppliedStats, OwnerDomainMatchDisposition};
use serde_json::{Value, json};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct OptionalCounts {
    pub total: usize,
    pub original: usize,
    pub coalesced: usize,
}

impl OptionalCounts {
    pub fn add(&mut self, stats: OwnerAppliedStats) -> Result<(), &'static str> {
        let overflow = "optional coefficient refusal counter overflow";
        let total = self
            .total
            .checked_add(stats.optional_coefficient_refusals)
            .ok_or(overflow)?;
        let original = self
            .original
            .checked_add(stats.optional_original_refusals)
            .ok_or(overflow)?;
        let coalesced = self
            .coalesced
            .checked_add(stats.optional_coalesced_refusals)
            .ok_or(overflow)?;
        *self = Self {
            total,
            original,
            coalesced,
        };
        Ok(())
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct OptionalRefusals {
    pub records: Vec<Value>,
    seen_original: bool,
    seen_coalesced: bool,
}

impl OptionalRefusals {
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        disposition: OwnerDomainMatchDisposition,
        rank: Option<u32>,
        powers: rustred::solver::DomainPowerBounds,
        lower: &[u64],
        upper: &[Option<u64>],
        shift: &[i64],
        original_term: Option<usize>,
        failure: &IndexedAlgebraError,
    ) -> Result<(), &'static str> {
        let seen = if original_term.is_some() {
            &mut self.seen_original
        } else {
            &mut self.seen_coalesced
        };
        if *seen || self.records.len() >= 2 {
            return Err("duplicate optional coefficient refusal provenance phase");
        }
        let IndexedAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } = failure
        else {
            return Err("unexpected optional coefficient refusal diagnostic");
        };
        self.records
            .try_reserve(1)
            .map_err(|_| "optional coefficient refusal diagnostic allocation")?;
        self.records.push(json!({
            "phase":if original_term.is_some() { "original" } else { "coalesced" },
            "selected_rule":format!("{disposition:?}"),
            "source_lower":lower, "source_upper":upper, "rank":rank, "shift":shift,
            "power_bounds":super::power_bounds_json(powers),
            "original_term":original_term,
            "resource":resource, "requested":requested, "limit":limit,
            "conditional_domain_overcover":true, "exact_coefficient_predicate_retained":false,
            "reached_missing_rule_claim":false,
        }));
        *seen = true;
        Ok(())
    }

    pub fn truncated(&self, stats: OwnerAppliedStats) -> bool {
        stats.optional_coefficient_refusals > self.records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal() -> IndexedAlgebraError {
        IndexedAlgebraError::ResourceLimit {
            resource: "guard factor variables",
            requested: 2,
            limit: 1,
        }
    }

    fn record(
        out: &mut OptionalRefusals,
        original_term: Option<usize>,
    ) -> Result<(), &'static str> {
        out.record(
            OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 3 },
            Some(11),
            Default::default(),
            &[3, 0],
            &[None, None],
            &[-1, 1],
            original_term,
            &refusal(),
        )
    }

    #[test]
    fn optional_provenance_is_bounded_and_distinct_from_frontiers() {
        let mut out = OptionalRefusals::default();
        record(&mut out, Some(12)).unwrap();
        record(&mut out, None).unwrap();
        assert!(record(&mut out, Some(13)).is_err());
        assert_eq!(out.records.len(), 2);
        for entry in &out.records {
            assert_eq!(entry["conditional_domain_overcover"], true);
            assert_eq!(entry["exact_coefficient_predicate_retained"], false);
            assert_eq!(entry["reached_missing_rule_claim"], false);
            assert_eq!(entry["rank"], 11);
            assert_eq!(entry["source_upper"], json!([null, null]));
            assert_eq!(entry["resource"], "guard factor variables");
            assert!(entry.get("coefficient").is_none());
        }
        assert_eq!(out.records[0]["original_term"], 12);
        assert_eq!(out.records[1]["original_term"], Value::Null);
        let mut stats = OwnerAppliedStats {
            optional_coefficient_refusals: 2,
            ..Default::default()
        };
        assert!(!out.truncated(stats));
        stats.optional_coefficient_refusals = 3;
        assert!(out.truncated(stats));
    }

    #[test]
    fn counted_refusal_without_delivered_event_is_partial_provenance() {
        let out = OptionalRefusals::default();
        assert!(!out.truncated(OwnerAppliedStats::default()));
        assert!(out.truncated(OwnerAppliedStats {
            optional_coefficient_refusals: 1,
            optional_original_refusals: 1,
            ..Default::default()
        }));
    }

    #[test]
    fn aggregate_refusal_count_overflow_is_atomic() {
        let mut out = OptionalCounts {
            total: 4,
            original: 2,
            coalesced: usize::MAX,
        };
        assert!(
            out.add(OwnerAppliedStats {
                optional_coefficient_refusals: 1,
                optional_coalesced_refusals: 1,
                ..Default::default()
            })
            .is_err()
        );
        assert_eq!(out.total, 4);
        assert_eq!(out.original, 2);
        assert_eq!(out.coalesced, usize::MAX);
    }
}
