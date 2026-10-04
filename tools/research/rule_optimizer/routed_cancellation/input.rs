use crate::applied_observer::input::{Queries, Request as ObservationRequest};
use rustred::sector::symmetry::integral_transport::ExpansionLimits;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub observation: ObservationRequest,
    pub expected_successors: usize,
    pub expected_strict_subsupport_successors: usize,
    pub limits: Limits,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_route_calls: usize,
    pub max_endpoint_occurrences: usize,
    pub max_distinct_keys: usize,
    pub max_coefficient_terms: usize,
    pub max_ledger_records: usize,
    pub max_report_bytes: usize,
    /// All numeric per-transport fields are explicit; exact algebra is the
    /// admitted owner's unchanged native exact-algebra policy.
    pub expansion: BTreeMap<String, usize>,
}

impl Limits {
    pub fn expansion(&self) -> Result<ExpansionLimits, String> {
        let map = &self.expansion;
        macro_rules! fields { ($($field:ident),+) => {{
            let names = [$(stringify!($field)),+];
            if map.len() != names.len() || names.iter().any(|k| !map.contains_key(*k)) {
                return Err("incomplete per-route expansion policy".into());
            }
            ExpansionLimits { $($field:map[stringify!($field)].try_into()
                .map_err(|_| "expansion policy conversion")?),+, ..Default::default() }
        }}; }
        Ok(fields!(
            max_factors,
            max_relation_coefficient_entries,
            max_total_power,
            max_native_polynomial_terms,
            max_native_polynomial_operations,
            max_native_exponent_entries,
            max_endpoints,
            max_endpoint_power_entries,
            max_retained_endpoint_key_bytes,
            max_retained_coefficient_terms,
            max_retained_coefficient_clone_owned_bytes
        ))
    }
}

impl Request {
    pub fn validate(&self, queries: &Queries) -> Result<usize, String> {
        self.observation.validate()?;
        self.limits.expansion()?;
        let n = queries.validate(1)?;
        if self.schema != "rustred.routed-cancellation.request.v1"
            || self.expected_successors == 0
            || self.expected_strict_subsupport_successors > self.expected_successors
            || [
                self.limits.max_route_calls,
                self.limits.max_endpoint_occurrences,
                self.limits.max_distinct_keys,
                self.limits.max_coefficient_terms,
                self.limits.max_ledger_records,
                self.limits.max_report_bytes,
            ]
            .contains(&0)
            || queries.queries[0]
                .lower
                .iter()
                .zip(&queries.queries[0].upper)
                .any(|(lo, hi)| *hi != Some(*lo))
        {
            return Err("requires one complete singleton and positive finite budgets".into());
        }
        Ok(n)
    }
}

/// Additional fields are preserved in the literal binding, not interpreted here.
/// The complete selection is first admitted by RoutedFeedbackSession.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Route {
    pub source_mask: String,
    pub owner_mask: String,
    pub requires_transport: bool,
    pub source_to_representative: Vec<Vec<String>>,
    pub owner_to_representative: Vec<Vec<String>>,
}

#[derive(Deserialize)]
pub struct Selection {
    #[serde(rename = "initial_frontier_routes")]
    pub routing: Vec<Route>,
}
