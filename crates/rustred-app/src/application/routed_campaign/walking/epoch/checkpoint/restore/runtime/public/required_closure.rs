//! Request-sized observation of cumulative required-query closure.
//!
//! The final `rescue::query_certification` remains the authority. This monitor
//! uses the same closed-containing-input-root criterion, but prepares inclusion
//! candidates only when input rows are appended and reads CLOSED bits only
//! after a completed closure scan. It never scans the domain ledger or edges.

use super::{Query, epoch};
use crate::application::routed_campaign::walking::{
    queue::{Domain, Phase},
    rescue,
};
use serde_json::{Value, json};
use std::{collections::HashSet, io};

struct Requested<const N: usize> {
    id: String,
    auxiliary: bool,
    domain: Domain<N>,
    containing_roots: Vec<usize>,
}

pub(super) struct Observation<const N: usize> {
    queries: Vec<Requested<N>>,
    indexed_roots: HashSet<usize>,
    admitted: usize,
    requested: usize,
    required_total: usize,
    unavailable: Option<&'static str>,
    cached: Option<((u64, u64, usize), usize)>,
    #[cfg(test)]
    evaluations: usize,
}

impl<const N: usize> Observation<N> {
    pub(super) fn admitted(&self) -> usize {
        self.admitted
    }

    pub(super) fn disabled(requested: usize, reason: &'static str) -> Self {
        Self {
            queries: Vec::new(),
            indexed_roots: HashSet::new(),
            admitted: 0,
            requested,
            required_total: 0,
            unavailable: Some(reason),
            cached: None,
            #[cfg(test)]
            evaluations: 0,
        }
    }

    pub(super) fn new<'a>(queries: impl Iterator<Item = &'a Query>) -> io::Result<Self> {
        let mut requested = Vec::new();
        for query in queries {
            requested
                .try_reserve(1)
                .map_err(|_| io::Error::other("required-query observation allocation"))?;
            requested.push(Requested {
                id: query.id.clone(),
                auxiliary: query.auxiliary,
                domain: Domain {
                    phase: Phase::Apply,
                    owner: rustred::storage_array(&query.owner, false)
                        .ok_or_else(|| io::Error::other("required-query observation arity"))?,
                    lower: rustred::storage_array::<_, N>(&query.lower, 0)
                        .ok_or_else(|| io::Error::other("required-query lower arity"))?
                        .to_vec(),
                    upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                        .ok_or_else(|| io::Error::other("required-query upper arity"))?
                        .to_vec(),
                    rank: query.rank,
                    powers: query.powers,
                },
                containing_roots: Vec::new(),
            });
        }
        let count = requested.len();
        let required_total = requested.iter().filter(|q| !q.auxiliary).count();
        Ok(Self {
            queries: requested,
            indexed_roots: HashSet::new(),
            admitted: 0,
            requested: count,
            required_total,
            unavailable: None,
            cached: None,
            #[cfg(test)]
            evaluations: 0,
        })
    }

    /// Called after restore and at admission/save boundaries. All old input
    /// rows and domains are immutable; only the newly appended rows need work.
    pub(super) fn inputs(
        &mut self,
        inputs: &[Value],
        domain_of: impl Fn(usize) -> Option<Domain<N>>,
    ) {
        if self.unavailable.is_some() {
            self.admitted = inputs.len().min(self.requested);
            return;
        }
        if inputs.len() == self.admitted {
            return;
        }
        if inputs.len() < self.admitted || inputs.len() > self.queries.len() {
            self.unavailable = Some("required-query input inventory mismatch");
            return;
        }
        let first = self.admitted;
        self.admitted = inputs.len();
        self.cached = None;
        for (index, input) in inputs.iter().enumerate().skip(first) {
            if input["id"].as_str() != Some(self.queries[index].id.as_str()) {
                self.unavailable = Some("required-query input identity mismatch");
                return;
            }
            let Some(root) = input["domain"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
            else {
                // A source-validity frontier has no root. It can only be
                // covered by some other actually closed containing root.
                continue;
            };
            if self.indexed_roots.contains(&root) {
                continue;
            }
            let Some(domain) = domain_of(root) else {
                self.unavailable = Some("required-query input root unavailable");
                return;
            };
            if self.indexed_roots.try_reserve(1).is_err() {
                self.unavailable = Some("required-query root-index allocation unavailable");
                return;
            }
            self.indexed_roots.insert(root);
            for query in &mut self.queries {
                if query.auxiliary || query.domain.owner != domain.owner {
                    continue;
                }
                // Match the final certifier: an uninstalled owner's admitted
                // Route root is compared to the query in the root's phase.
                let mut probe = query.domain.clone();
                probe.phase = domain.phase;
                if rescue::contains(&domain, &probe) {
                    if query.containing_roots.try_reserve(1).is_err() {
                        self.unavailable =
                            Some("required-query inclusion-index allocation unavailable");
                        return;
                    }
                    query.containing_roots.push(root);
                }
            }
        }
    }

    /// Attach bounded scalars to the existing declared-role census. Repeated
    /// heartbeats perform no containment checks, root scan, or closure refresh.
    pub(super) fn attach(&mut self, census: &mut Value, state: &epoch::state::EpochState<N>) {
        let counters = state.tracker.counters();
        let available = self.unavailable.is_none()
            && counters.unavailable.is_none()
            && state.tracker.node_count() == state.watermark() as usize
            && counters.initial == state.p0 as usize
            && counters.refresh_count > 0;
        let key = (
            counters.snapshot_revision,
            counters.refresh_count,
            self.admitted,
        );
        let closed = if available {
            if self
                .cached
                .as_ref()
                .is_none_or(|(cached, _)| *cached != key)
            {
                let count = self.queries[..self.admitted]
                    .iter()
                    .filter(|query| !query.auxiliary)
                    .filter(|query| {
                        query
                            .containing_roots
                            .iter()
                            .any(|&root| state.tracker.closed(root) == Some(true))
                    })
                    .count();
                self.cached = Some((key, count));
                #[cfg(test)]
                {
                    self.evaluations += 1;
                }
            }
            self.cached.as_ref().map(|(_, count)| *count)
        } else {
            None
        };
        census["required_closed"] = json!(closed);
        census["required_closed_evaluated"] = json!(available);
        census["required_closure"] = json!({
            "available":available,
            "method":"closed_containing_input_root",
            "snapshot_revision":counters.snapshot_revision,
            "refresh_count":counters.refresh_count,
            "snapshot_stale":counters.snapshot_revision != counters.revision,
            "snapshot_age_seconds":state.tracker.snapshot_age_seconds(),
            "observed_query_rows":self.admitted,
            "requested_query_rows":self.requested,
            "required_queries_total":self.required_total,
            "closed_counts_are_conservative_lower_bounds":true,
            "reason":if available { None } else {Some(self.unavailable
                .or(counters.unavailable.as_deref())
                .unwrap_or(if counters.refresh_count == 0 {"awaiting first completed closure scan"}
                    else {"dependency domain inventory mismatch"}))},
            "scope":"cumulative admitted required queries covered by a closed containing input root; auxiliary roots may supply coverage but are not required; snapshot progress is not independent certification",
            "family_closure_claim":false
        });
    }
}

#[cfg(test)]
mod tests;
