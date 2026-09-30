//! One compute reservation shared by both publication coordinators.
//!
//! Inspection streams may block awaiting publication; admission therefore uses
//! distinct reserved helpers, never the same pool. These are configured limits,
//! not measurements of worker activity. One-worker execution remains inline.
use super::{OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest};
use serde_json::{Value, json};

/// Largest automatic Ready helper allocation; explicit `--inspection-workers`
/// partitions are never capped.
pub(super) const READY_HELPER_CAP: usize = 32;

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

/// Resolve the Epoch helper request as a partition of the existing budget.
/// `None` retains the historical policy; zero is an explicit serial P2 control.
pub(super) fn epoch_inspection(
    workers: usize,
    inspection: Option<usize>,
    finite_comparison_cap: Option<usize>,
    preparation: Option<usize>,
) -> Result<Option<usize>, &'static str> {
    validate(workers, inspection, finite_comparison_cap)?;
    let Some(helpers) = preparation else {
        return Ok(inspection);
    };
    let available = if workers == 1 { 1 } else { workers - 1 };
    if helpers >= available {
        return Err(
            "Epoch preparation workers must leave at least one inspector within the total worker budget",
        );
    }
    let inspectors = available - helpers;
    if inspection.is_some_and(|requested| requested != inspectors) {
        return Err(
            "Epoch preparation and inspection workers must exactly partition the non-coordinator budget",
        );
    }
    validate(workers, Some(inspectors), finite_comparison_cap)?;
    Ok(Some(inspectors))
}

impl WorkerBudget {
    pub fn for_request(request: &OwnerDomainWalkRequest) -> Self {
        let inspection = if request.publication_policy == OwnerDomainWalkPublicationPolicy::Epoch {
            epoch_inspection(
                request.workers,
                request.inspection_workers,
                request.max_containment_checks,
                request.epoch_preparation_workers,
            )
            .expect("validated Epoch worker partition")
        } else {
            request.inspection_workers
        };
        Self::new(
            request.workers,
            inspection,
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
            OwnerDomainWalkPublicationPolicy::Ordered
            | OwnerDomainWalkPublicationPolicy::Ready
            | OwnerDomainWalkPublicationPolicy::Epoch => 5,
            OwnerDomainWalkPublicationPolicy::OwnerBatched => 4,
        };
        let helpers = match inspection {
            Some(inspection) => available - inspection,
            // Epoch defaults to serial P2; helpers require an explicit
            // preparation request or inspector partition within this budget.
            None if policy == OwnerDomainWalkPublicationPolicy::Epoch => 0,
            None if requested >= helper_threshold && finite_comparison_cap.is_none() => {
                let half = available / 2;
                if policy == OwnerDomainWalkPublicationPolicy::Ready {
                    // Lookup preparation saturates well before the inspection
                    // side does; large Ready budgets keep the rest inspecting.
                    // Every W <= 64 default is unchanged (half <= 31 there).
                    half.min(READY_HELPER_CAP)
                } else {
                    half
                }
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
