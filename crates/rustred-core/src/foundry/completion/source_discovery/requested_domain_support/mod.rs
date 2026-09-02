//! Authority-minimal parent-support ingress for requested-domain campaigns.
//!
//! This boundary deliberately carries only requested geometry, canonical
//! parent-lattice support, detached proposal provenance, and scalar resource
//! telemetry. It cannot carry a coefficient, Ore row, guard, circuit, owner,
//! closure assertion, or artifact material. The trusted campaign adapter must
//! still expand support through its sealed ordinary-source incidence index and
//! run the existing modular and exact-replay pipeline.

mod error;
mod limits;
mod model;
mod preflight;
mod union;

pub(crate) use error::RequestedDomainSupportError;
pub(crate) use limits::RequestedDomainSupportLimits;
pub(crate) use model::{
    RequestedDomainSemanticKey, RequestedDomainSupportCensus, RequestedDomainSupportProposal,
    RequestedSupportProposalOrigin, RequestedSupportProposalProvenance,
    RequestedSupportProposalProvenanceInput,
};
pub(crate) use preflight::{
    RequestedDomainSupportBatchPreflight, RequestedDomainSupportBatchShape,
    try_preflight_requested_domain_support_batch,
};
pub(crate) use union::{
    RequestedDomainSupportUnion, RequestedDomainSupportUnionCensus,
    try_union_requested_domain_support,
};

#[cfg(test)]
mod tests;
