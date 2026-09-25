//! One compute reservation shared by both publication coordinators.
//!
//! Inspection streams may block awaiting publication; admission therefore uses
//! distinct reserved helpers, never the same pool. These are configured limits,
//! not measurements of worker activity. One-worker execution remains inline.
use super::{OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WorkerBudget {
    pub requested: usize,
    pub inspection: usize,
    pub helpers: usize,
    pub coordinator: usize,
}

pub(super) fn validate(
    workers: usize,
    inspection: Option<usize>,
    finite_comparison_cap: Option<usize>,
) -> Result<(), &'static str> {
    if workers == 0 {
        return Err("worker budget must be positive");
    }
    let Some(inspection) = inspection else {
        return Ok(());
    };
    let available = if workers == 1 { 1 } else { workers - 1 };
    if inspection == 0 || inspection > available {
        return Err(
            "inspection workers must be positive and leave one coordinator when workers > 1",
        );
    }
    if finite_comparison_cap.is_some() && inspection != available {
        return Err("finite containment cap requires all non-coordinator workers for inspection");
    }
    Ok(())
}

impl WorkerBudget {
    pub fn for_request(request: &OwnerDomainWalkRequest) -> Self {
        Self::new(
            request.workers,
            request.inspection_workers,
            request.max_containment_checks,
            request.publication_policy,
        )
    }

    pub fn new(
        requested: usize,
        inspection: Option<usize>,
        finite_comparison_cap: Option<usize>,
        policy: OwnerDomainWalkPublicationPolicy,
    ) -> Self {
        validate(requested, inspection, finite_comparison_cap).expect("validated worker partition");
        if requested == 1 {
            return Self {
                requested,
                inspection: 1,
                helpers: 0,
                coordinator: 0,
            };
        }
        let available = requested - 1;
        // Preserve the distinct historical defaults, including the W=4 case.
        let helper_threshold = match policy {
            OwnerDomainWalkPublicationPolicy::Ordered | OwnerDomainWalkPublicationPolicy::Ready => {
                5
            }
            OwnerDomainWalkPublicationPolicy::OwnerBatched => 4,
        };
        let helpers = match inspection {
            Some(inspection) => available - inspection,
            None if requested >= helper_threshold && finite_comparison_cap.is_none() => {
                available / 2
            }
            None => 0,
        };
        Self {
            requested,
            inspection: available - helpers,
            helpers,
            coordinator: 1,
        }
    }

    pub fn json(self, explicit_inspection: Option<usize>) -> Value {
        json!({
            "requested_worker_budget": self.requested,
            "requested_inspection_workers": explicit_inspection,
            "inspection_worker_limit": self.inspection,
            "admission_worker_limit": self.helpers,
            "coordinator_worker_limit": self.coordinator,
            "total_compute_worker_limit": self.inspection + self.helpers + self.coordinator,
            "scope": "configured compute reservation, not activity or successfully spawned workers"
        })
    }
}

#[cfg(test)]
mod tests;
