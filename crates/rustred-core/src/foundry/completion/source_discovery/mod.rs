//! Bounded inverse-incidence discovery of exact translated-source requests.
//!
//! This layer only nominates rows from a sealed ordinary-source module.  A
//! nomination is neither a modular hit nor an exact relation, and no result
//! here can authorize a rule, owner, terminal, artifact, or closure claim.

mod error;
mod incidence;
mod limits;
mod model;
mod nominate;

pub(crate) use error::SourceDiscoveryError;
pub(crate) use incidence::OrdinarySourceIncidenceIndex;
pub(crate) use limits::SourceDiscoveryLimits;
pub(crate) use model::IncidentTranslationNominations;

#[cfg(test)]
mod tests;
