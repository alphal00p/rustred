//! O(1) cumulative rank telemetry, never an admission or coverage decision.
//! The rank is the sum of inactive scalar-product powers, not twice that
//! sum (the Lorentz numerator degree). Declared rank caps alone are not
//! sufficient: a domain without a cap can still have rank zero or a finite
//! rank imposed by its box/power constraints.

use super::queue::{CompactDomain, CompactSummary};
use rustred::solver::DomainPowerSummary;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RankExtent {
    Empty,
    Finite(u128),
    Unbounded,
    Unknown,
}

impl RankExtent {
    pub fn from_core<const N: usize>(summary: &DomainPowerSummary<N>) -> Self {
        match summary.extrema() {
            None => Self::Empty,
            Some(extrema) => match extrema.numerator_rank().1 {
                Some(maximum) => Self::Finite(maximum),
                None => Self::Unbounded,
            },
        }
    }
}

#[derive(Default)]
pub(super) struct RankCensus {
    maximum: Option<u128>,
    finite_domains: u64,
    unbounded_domains: u64,
    unknown_domains: u64,
    empty_domains: u64,
}

impl RankCensus {
    pub fn observe(&mut self, extent: RankExtent) {
        match extent {
            RankExtent::Empty => self.empty_domains += 1,
            RankExtent::Finite(maximum) => {
                self.finite_domains += 1;
                self.maximum = Some(self.maximum.map_or(maximum, |old| old.max(maximum)));
            }
            RankExtent::Unbounded => self.unbounded_domains += 1,
            RankExtent::Unknown => self.unknown_domains += 1,
        }
    }

    pub fn observe_domain<const N: usize>(
        &mut self,
        domain: &CompactDomain<N>,
        summary: Option<&CompactSummary<N>>,
    ) {
        // The common path borrows the summary already made for containment.
        // Only the rare wide/legacy capped path needs the existing exact
        // geometry implementation; this performs no symbolic algebra.
        let extent = summary
            .and_then(CompactSummary::numerator_rank_extent)
            .unwrap_or_else(|| {
                domain
                    .try_native_summary()
                    .map(|summary| RankExtent::from_core(&summary))
                    .unwrap_or(RankExtent::Unknown)
            });
        self.observe(extent);
    }

    pub fn json(&self) -> Value {
        let status = if self.unbounded_domains > 0 {
            "unbounded"
        } else if self.unknown_domains > 0 || self.maximum.is_none() {
            "unknown"
        } else {
            "finite"
        };
        // All campaign ranks normally fit u32. Preserve unusual larger exact
        // extrema without JSON number truncation (consumers can parse decimal).
        let maximum = self.maximum.map(|value| {
            u64::try_from(value)
                .map(Value::from)
                .unwrap_or_else(|_| Value::from(value.to_string()))
        });
        json!({"status":status,"maximum":maximum,
            "finite_domains":self.finite_domains,"unbounded_domains":self.unbounded_domains,
            "unknown_domains":self.unknown_domains,"empty_domains":self.empty_domains,
            "scope":"scheduled_domain_geometry_including_restored",
            "rank_convention":"sum_of_inactive_scalar_product_powers"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::routed_campaign::walking::queue::{Domain, Phase};

    fn observe(
        census: &mut RankCensus,
        owner: [bool; 2],
        upper: Vec<Option<u64>>,
        rank: Option<u32>,
    ) {
        let domain = Domain {
            phase: Phase::Route,
            owner,
            lower: vec![0, 0],
            upper,
            rank,
            powers: Default::default(),
        };
        let image = CompactDomain::try_from_domain(&domain).unwrap();
        let summary = image.native_summary();
        let compact = CompactSummary::from_core(&summary);
        let exact = RankExtent::from_core(&summary);
        assert_eq!(compact.numerator_rank_extent(), Some(exact));
        census.observe_domain(&image, Some(&compact));
    }

    #[test]
    fn tight_rank_distinguishes_no_cap_from_genuinely_unbounded() {
        let mut census = RankCensus::default();
        assert_eq!(census.json()["status"], "unknown");
        observe(&mut census, [true, true], vec![None, None], None);
        assert_eq!(census.json()["status"], "finite");
        assert_eq!(census.json()["maximum"], 0);
        observe(&mut census, [true, false], vec![None, Some(7)], None);
        assert_eq!(census.json()["maximum"], 7);
        observe(&mut census, [false, false], vec![None, None], Some(20));
        assert_eq!(census.json()["maximum"], 20);
        assert_eq!(census.json()["finite_domains"], 3);
        observe(&mut census, [true, false], vec![None, None], None);
        assert_eq!(census.json()["status"], "unbounded");
        assert_eq!(census.json()["unbounded_domains"], 1);
        assert_eq!(census.json()["maximum"], 20);
    }

    #[test]
    fn rank_census_preserves_large_values_and_unknown_evidence() {
        let mut census = RankCensus::default();
        census.observe(RankExtent::Finite(u128::from(u64::MAX) + 1));
        assert_eq!(census.json()["maximum"], "18446744073709551616");
        census.observe(RankExtent::Unknown);
        assert_eq!(census.json()["status"], "unknown");
        census.observe(RankExtent::Empty);
        assert_eq!(census.json()["empty_domains"], 1);
        census.observe(RankExtent::Unbounded);
        assert_eq!(census.json()["status"], "unbounded");
    }
}
