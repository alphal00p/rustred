//! Operational quotas for trusted saved-rule execution. These fields never
//! change source admission, exact exponents, rules or descendant geometry.
use super::policy::{nullable_u64, nullable_usize};
use crate::RoutedCampaignRequest;
use rustred::{
    algebra::{ExactAlgebraLimits, IndexedAlgebraLimits},
    sector::symmetry::integral_transport::ExpansionLimits,
};
use serde::{Deserialize, Serialize};

pub(crate) fn default_override() -> Option<OperationLimits> {
    Some(OperationLimits::default())
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct OperationLimits {
    pub(crate) planning: PlanningLimits,
    pub(crate) transport: TransportLimits,
    pub(crate) reduction: ReductionOperationLimits,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PlanningLimits {
    #[serde(with = "nullable_usize")]
    pub(crate) max_positive_layers: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_partition_checks: usize,
}
impl Default for PlanningLimits {
    fn default() -> Self {
        Self {
            max_positive_layers: usize::MAX,
            max_partition_checks: usize::MAX,
        }
    }
}
impl From<PlanningLimits> for super::super::windows::WindowPlanningLimits {
    fn from(limits: PlanningLimits) -> Self {
        Self {
            max_positive_layers: limits.max_positive_layers,
            max_partition_checks: limits.max_partition_checks,
        }
    }
}
impl OperationLimits {
    pub(crate) fn apply(&self, request: &mut RoutedCampaignRequest) {
        self.transport.apply(&mut request.trace_limits.expansion);
        self.reduction.apply(&mut request.reduction_limits);
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ExactResourceLimits {
    #[serde(with = "nullable_usize")]
    pub(crate) max_polynomial_terms: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_term_operations: usize,
}
impl Default for ExactResourceLimits {
    fn default() -> Self {
        let limits = ExactAlgebraLimits::unbounded_resources();
        Self {
            max_polynomial_terms: limits.max_polynomial_terms,
            max_term_operations: limits.max_term_operations,
        }
    }
}
impl ExactResourceLimits {
    fn apply(&self, limits: &mut ExactAlgebraLimits) {
        // In particular preserve an explicitly smaller source max_exponent.
        limits.max_polynomial_terms = self.max_polynomial_terms;
        limits.max_term_operations = self.max_term_operations;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct IndexedResourceLimits {
    pub(crate) exact_algebra: ExactResourceLimits,
    #[serde(with = "nullable_usize")]
    pub(crate) max_specialization_power_operations: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_specialization_integer_bits: usize,
}
impl Default for IndexedResourceLimits {
    fn default() -> Self {
        let limits = IndexedAlgebraLimits::unbounded_resources();
        Self {
            exact_algebra: Default::default(),
            max_specialization_power_operations: limits.max_specialization_power_operations,
            max_specialization_integer_bits: limits.max_specialization_integer_bits,
        }
    }
}
impl IndexedResourceLimits {
    fn apply(&self, limits: &mut IndexedAlgebraLimits) {
        self.exact_algebra.apply(&mut limits.exact_algebra);
        limits.max_specialization_power_operations = self.max_specialization_power_operations;
        limits.max_specialization_integer_bits = self.max_specialization_integer_bits;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TransportLimits {
    pub(crate) exact_algebra: ExactResourceLimits,
    #[serde(with = "nullable_usize")]
    pub(crate) max_factors: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_relation_coefficient_entries: usize,
    #[serde(with = "nullable_u64")]
    pub(crate) max_total_power: u64,
    #[serde(with = "nullable_usize")]
    pub(crate) max_native_polynomial_terms: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_native_polynomial_operations: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_native_exponent_entries: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_endpoints: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_endpoint_power_entries: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_retained_endpoint_key_bytes: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_retained_coefficient_terms: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_retained_coefficient_clone_owned_bytes: usize,
}
impl Default for TransportLimits {
    fn default() -> Self {
        let limits = ExpansionLimits::unbounded_resources();
        Self {
            exact_algebra: Default::default(),
            max_factors: limits.max_factors,
            max_relation_coefficient_entries: limits.max_relation_coefficient_entries,
            max_total_power: limits.max_total_power,
            max_native_polynomial_terms: limits.max_native_polynomial_terms,
            max_native_polynomial_operations: limits.max_native_polynomial_operations,
            max_native_exponent_entries: limits.max_native_exponent_entries,
            max_endpoints: limits.max_endpoints,
            max_endpoint_power_entries: limits.max_endpoint_power_entries,
            max_retained_endpoint_key_bytes: limits.max_retained_endpoint_key_bytes,
            max_retained_coefficient_terms: limits.max_retained_coefficient_terms,
            max_retained_coefficient_clone_owned_bytes: limits
                .max_retained_coefficient_clone_owned_bytes,
        }
    }
}
impl TransportLimits {
    fn apply(&self, limits: &mut ExpansionLimits) {
        self.exact_algebra.apply(&mut limits.exact_algebra);
        limits.max_factors = self.max_factors;
        limits.max_relation_coefficient_entries = self.max_relation_coefficient_entries;
        limits.max_total_power = self.max_total_power;
        limits.max_native_polynomial_terms = self.max_native_polynomial_terms;
        limits.max_native_polynomial_operations = self.max_native_polynomial_operations;
        limits.max_native_exponent_entries = self.max_native_exponent_entries;
        limits.max_endpoints = self.max_endpoints;
        limits.max_endpoint_power_entries = self.max_endpoint_power_entries;
        limits.max_retained_endpoint_key_bytes = self.max_retained_endpoint_key_bytes;
        limits.max_retained_coefficient_terms = self.max_retained_coefficient_terms;
        limits.max_retained_coefficient_clone_owned_bytes =
            self.max_retained_coefficient_clone_owned_bytes;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ReductionOperationLimits {
    pub(crate) exact_algebra: ExactResourceLimits,
    pub(crate) indexed_algebra: IndexedResourceLimits,
    #[serde(with = "nullable_usize")]
    pub(crate) max_cached_integrals: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_cached_coefficient_terms: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_cached_coefficient_bytes: usize,
    #[serde(with = "nullable_usize")]
    pub(crate) max_factorization_terms: usize,
}
impl Default for ReductionOperationLimits {
    fn default() -> Self {
        Self {
            exact_algebra: Default::default(),
            indexed_algebra: Default::default(),
            max_cached_integrals: usize::MAX,
            max_cached_coefficient_terms: usize::MAX,
            max_cached_coefficient_bytes: usize::MAX,
            max_factorization_terms: usize::MAX,
        }
    }
}
impl ReductionOperationLimits {
    fn apply(&self, limits: &mut rustred::reduction::ReductionLimits) {
        self.exact_algebra.apply(&mut limits.exact_algebra);
        self.indexed_algebra.apply(&mut limits.indexed_algebra);
        limits.max_cached_integrals = self.max_cached_integrals;
        limits.max_cached_coefficient_terms = self.max_cached_coefficient_terms;
        limits.max_cached_coefficient_bytes = self.max_cached_coefficient_bytes;
        limits.max_factorization_terms = self.max_factorization_terms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn operation_policy_is_strict_nullable_and_preserves_source_exponents_and_aggregates() {
        let policy: OperationLimits = serde_json::from_value(json!({
            "planning":{"max_positive_layers":null,"max_partition_checks":7},
            "transport":{"max_native_exponent_entries":null,"max_endpoints":17,
                "exact_algebra":{"max_polynomial_terms":31}},
            "reduction":{"max_factorization_terms":9,
                "indexed_algebra":{"max_specialization_integer_bits":27}}
        }))
        .unwrap();
        let mut request = RoutedCampaignRequest::new(String::new(), String::new());
        request.trace_limits.expansion.exact_algebra.max_exponent = 3;
        request.reduction_limits.exact_algebra.max_exponent = 4;
        request
            .reduction_limits
            .indexed_algebra
            .exact_algebra
            .max_exponent = 5;
        let aggregate = request.reduction_limits;
        policy.apply(&mut request);
        assert_eq!(policy.planning.max_positive_layers, usize::MAX);
        assert_eq!(policy.planning.max_partition_checks, 7);
        assert_eq!(
            request.trace_limits.expansion.max_native_exponent_entries,
            usize::MAX
        );
        assert_eq!(request.trace_limits.expansion.max_endpoints, 17);
        assert_eq!(
            request
                .trace_limits
                .expansion
                .exact_algebra
                .max_polynomial_terms,
            31
        );
        assert_eq!(request.trace_limits.expansion.exact_algebra.max_exponent, 3);
        assert_eq!(request.reduction_limits.exact_algebra.max_exponent, 4);
        assert_eq!(
            request
                .reduction_limits
                .indexed_algebra
                .exact_algebra
                .max_exponent,
            5
        );
        assert_eq!(request.reduction_limits.max_factorization_terms, 9);
        assert_eq!(
            request
                .reduction_limits
                .indexed_algebra
                .max_specialization_integer_bits,
            27
        );
        assert_eq!(
            request.reduction_limits.max_rule_applications,
            aggregate.max_rule_applications
        );
        assert_eq!(
            request.reduction_limits.max_pending_frames,
            aggregate.max_pending_frames
        );
        assert_eq!(
            request.reduction_limits.max_coalescing_additions,
            aggregate.max_coalescing_additions
        );
        let roundtrip: OperationLimits =
            serde_json::from_slice(&serde_json::to_vec(&policy).unwrap()).unwrap();
        assert_eq!(roundtrip, policy);
        for value in [
            json!({"typo":1}),
            json!({"transport":{"typo":1}}),
            json!({"transport":{"exact_algebra":{"max_exponent":2}}}),
            json!({"reduction":{"indexed_algebra":{"typo":1}}}),
        ] {
            assert!(serde_json::from_value::<OperationLimits>(value).is_err());
        }
        let null: Option<OperationLimits> =
            serde_json::from_value(serde_json::Value::Null).unwrap();
        assert!(null.is_none());
    }

    #[test]
    fn default_operation_policy_opens_all_count_quotas_but_explicit_zero_is_zero() {
        let policy: OperationLimits = serde_json::from_value(json!({})).unwrap();
        assert_eq!(policy, OperationLimits::default());
        let json = serde_json::to_value(policy).unwrap();
        for section in ["planning", "transport", "reduction"] {
            for (key, value) in json[section].as_object().unwrap() {
                if !value.is_object() {
                    assert!(value.is_null(), "{section}.{key}");
                }
            }
        }
        let policy: OperationLimits =
            serde_json::from_value(json!({"transport":{"max_endpoints":0}})).unwrap();
        let mut request = RoutedCampaignRequest::new(String::new(), String::new());
        policy.apply(&mut request);
        assert_eq!(request.trace_limits.expansion.max_endpoints, 0);
    }
}
