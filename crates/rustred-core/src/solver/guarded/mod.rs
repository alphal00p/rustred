//! Guarded, caller-supplied identities for distribution-weighted integrals.
//!
//! Ordinary coordinates, required on-shell cuts and occupation coordinates
//! retain separate meanings. Search uses the native sector discovery engine;
//! replay proves consequences of the supplied identities, not their physical
//! validity. No vacuum zero census or master-count assumption is imported.

mod domain;
mod lifecycle;
mod model;
mod persistence;
mod replay;
mod search;

pub use domain::{IndexBounds, IndexDomain, IndexRole};
pub use lifecycle::*;
pub use model::{
    GuardedRule, GuardedSolution, GuardedSource, GuardedSourceInfo, GuardedSourceSystem,
    GuardedUnresolved, GuardedUnresolvedReason,
};

pub(super) use search::GuardedSearchScope;

#[cfg(test)]
mod boundary_tests;
#[cfg(test)]
mod reference_conversion;
#[cfg(test)]
mod reference_cut_conversion;
#[cfg(test)]
mod reference_cut_plan;
#[cfg(test)]
mod reference_routing;
#[cfg(test)]
mod reference_spatial;
#[cfg(test)]
mod reference_validation;
#[cfg(test)]
mod reference_weighted_spatial;
#[cfg(test)]
mod tests;
