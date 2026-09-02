//! Bounded attachment of an explicit leader sequence to exact cover geometry.
//!
//! This is an offline proposal seam for diagnosing scheduling hypotheses. A
//! caller supplies rectangular chart domains only: no source row, support,
//! coefficient, or rule is accepted. Each domain is intersected with every
//! exact uncovered box and each nonempty residual becomes executable only
//! through the ordinary source/replay/admission pipeline. A domain is reported
//! as covered only when no residual remains, and every ledger mutation
//! requires the caller to rebuild the plan.

mod model;
mod plan;

pub(crate) use model::{
    RequestedDomain, RequestedDomainPlan, RequestedDomainScopePartition, RequestedDomainTask,
    RequestedDomainTaskKey,
};
pub(crate) use plan::try_plan_requested_domains;

#[cfg(test)]
mod tests;
