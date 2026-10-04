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
    /// Omission preserves the original one-Apply, weighted-routing observer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plateau_cut: Option<PlateauCut>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlateauCut {
    pub max_depth: usize,
    pub max_parents: usize,
    pub max_apply_calls: usize,
    pub max_pending_terms: usize,
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
        let n = queries.validate(self.plateau_cut.as_ref().map_or(1, |p| p.max_parents))?;
        if self.schema != "rustred.routed-cancellation.request.v1"
            || (self.plateau_cut.is_none() && self.expected_successors == 0)
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
            || queries.queries.iter().any(|q| {
                q.lower
                    .iter()
                    .zip(&q.upper)
                    .any(|(lo, hi)| *hi != Some(*lo))
            })
        {
            return Err("requires one complete singleton and positive finite budgets".into());
        }
        if let Some(p) = &self.plateau_cut {
            if p.max_depth == 0
                || p.max_parents == 0
                || p.max_apply_calls < queries.queries.len()
                || p.max_pending_terms == 0
                || self.observation.recorder_limits.max_queries < p.max_apply_calls
                || self.expected_successors != 0
                || self.expected_strict_subsupport_successors != 0
            {
                return Err(
                    "invalid plateau cut/recording budgets or legacy count expectations".into(),
                );
            }
            let mut keys = std::collections::BTreeSet::new();
            for q in &queries.queries {
                let sector: Vec<_> = q.owner.bytes().map(|b| b == b'1').collect();
                let key = super::singleton(&sector, &q.lower, &q.upper)?;
                super::plateau::check_query_point(q, &key)?;
                if !keys.insert(key) {
                    return Err("duplicate plateau parent integral".into());
                }
            }
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
