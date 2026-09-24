//! Explicit finite starting domains, separate from saved source-search scope.
use rustred::family::IntegralKey;
use rustred::solver::{CandidateEntryAdmission, FiniteRootAdmission, RootRegionInput};
use serde_json::{Value, json};

use super::matching::input;
use crate::AppError;

/// This is an input-size allowance, not an integral-rank or loop-count limit.
const MAX_REGIONS: usize = 10_000;

#[derive(Debug)]
pub(super) struct RequestedEntryDomain<const N: usize> {
    policy: FiniteRootAdmission<N>,
    description: Value,
    description_json_bytes: usize,
    /// Original typed union members, not their projected rectangular hulls.
    regions: Vec<RootRegionInput<N>>,
}

impl<const N: usize> RequestedEntryDomain<N> {
    pub fn parse(text: &str) -> Result<Self, AppError> {
        // Reuse the same strict, bounded coordinate/A/R/D input vocabulary as
        // local matching. FiniteRootAdmission additionally requires finiteness.
        let queries = input::parse(text, N, MAX_REGIONS, 1024 * 1024)?;
        let description = json!({
            "mode":"explicit_finite", "region_count":queries.len(),
            "exhaustive_coverage_claim":false,
            "regions":queries.iter().map(|q| json!({
                "id":q.id,
                "owner":q.owner.iter().map(|&b| if b {'1'} else {'0'}).collect::<String>(),
                "lower":q.lower, "upper":q.upper, "max_numerator_rank":q.rank,
                "power_bounds":input::power_bounds_json(q.powers)
            })).collect::<Vec<_>>()
        });
        // Input size is not an output bound: the normalized description adds
        // explicit optional fields. Measure it once, outside reduction loops.
        let description_json_bytes = serde_json::to_vec(&description)
            .map_err(|e| AppError::input(format!("finite starting domain description: {e}")))?
            .len();
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(queries.len())
            .map_err(|_| AppError::limit("finite starting domain region allocation failed"))?;
        regions.extend(queries.into_iter().map(|q| RootRegionInput {
            support: std::array::from_fn(|i| q.owner[i]),
            lower: q.lower,
            upper: q.upper,
            rank: q.rank,
            powers: q.powers,
        }));
        let policy = FiniteRootAdmission::try_new(regions.iter().cloned(), MAX_REGIONS)
            .map_err(|e| AppError::input(format!("finite starting domain: {e}")))?;
        Ok(Self {
            policy,
            description,
            description_json_bytes,
            regions,
        })
    }

    pub fn description_json_bytes(&self) -> usize {
        self.description_json_bytes
    }

    pub fn regions(&self) -> &[RootRegionInput<N>] {
        &self.regions
    }

    pub fn validate(&self, target: &IntegralKey) -> Result<(), AppError> {
        self.policy
            .validate_entry(target)
            .map_err(|e| AppError::input(format!("finite starting domain: {e}")))
    }
}

pub(super) fn admission<const N: usize>(
    domain: Option<&RequestedEntryDomain<N>>,
) -> CandidateEntryAdmission<'_, N> {
    match domain {
        Some(domain) => CandidateEntryAdmission::ExplicitFinite(&domain.policy),
        None => CandidateEntryAdmission::SavedGenerationScope,
    }
}

pub(super) fn description<const N: usize>(domain: Option<&RequestedEntryDomain<N>>) -> Value {
    domain.map_or_else(
        || json!({"mode":"saved_generation_scope", "exhaustive_coverage_claim":false}),
        |domain| domain.description.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(powers: [i64; 2]) -> IntegralKey {
        IntegralKey::try_new(powers).unwrap()
    }

    #[test]
    fn union_keeps_holes_and_correlated_bounds() {
        let text = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"first","owner":"10","lower":[0,0],"upper":[2,1],
             "max_numerator_rank":1,"power_bounds":{"min_power_difference":2}},
            {"id":"second","owner":"10","lower":[5,3],"upper":[5,3],
             "max_numerator_rank":3}]})
        .to_string();
        let domain = RequestedEntryDomain::<2>::parse(&text).unwrap();
        for powers in [[2, 0], [3, -1], [6, -3]] {
            domain.validate(&key(powers)).unwrap();
        }
        for powers in [[1, 0], [2, -1], [4, -2], [0, 0], [2, 1]] {
            assert!(domain.validate(&key(powers)).is_err(), "{powers:?}");
        }
        assert_eq!(description(Some(&domain))["region_count"], 2);
        assert_eq!(
            description(Some(&domain))["exhaustive_coverage_claim"],
            false
        );
    }

    #[test]
    fn aggregate_bounds_can_make_individually_unbounded_axes_finite() {
        let mut query = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"finite","owner":"10","lower":[0,0],"upper":[null,null],
             "max_numerator_rank":15,"power_bounds":{"max_positive_power":24,
             "min_power_difference":9}}]});
        let domain = RequestedEntryDomain::<2>::parse(&query.to_string()).unwrap();
        // Witness selection retains the actual input predicates, not the
        // finite coordinate projection used internally by fast membership.
        assert_eq!(domain.regions().len(), 1);
        assert_eq!(domain.regions()[0].upper, [None, None]);
        assert_eq!(domain.regions()[0].rank, Some(15));
        assert_eq!(domain.regions()[0].powers.max_positive_power, Some(24));
        assert_eq!(domain.regions()[0].powers.min_power_difference, Some(9));
        assert!(
            domain.policy.regions()[0]
                .extrema()
                .unwrap()
                .upper()
                .iter()
                .all(Option::is_some)
        );
        domain.validate(&key([24, -15])).unwrap();
        for powers in [[24, -16], [25, -15], [23, -15]] {
            assert!(domain.validate(&key(powers)).is_err());
        }
        query["queries"][0]["power_bounds"] = json!({});
        assert!(RequestedEntryDomain::<2>::parse(&query.to_string()).is_err());
    }

    #[test]
    fn invalid_empty_or_wrong_schema_domains_fail_closed() {
        for text in [
            "{}",
            r#"{"schema":"rustred.owner-domain-queries.json.v2","queries":[]}"#,
        ] {
            assert!(RequestedEntryDomain::<2>::parse(text).is_err());
        }
        let text = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"empty","owner":"10","lower":[0,0],"upper":[0,0],
             "max_numerator_rank":0,"power_bounds":{"min_power_difference":2}}]})
        .to_string();
        assert!(RequestedEntryDomain::<2>::parse(&text).is_err());
        assert_eq!(description::<2>(None)["mode"], "saved_generation_scope");
    }
}
