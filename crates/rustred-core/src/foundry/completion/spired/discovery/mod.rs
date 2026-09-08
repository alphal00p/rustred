//! Streaming modular discovery for one immutable SpIRed case and probe.

mod error;
mod limits;
mod registry;
mod run;

pub(crate) use error::SpiredStreamingError;
pub(crate) use limits::SpiredStreamingLimits;
pub(crate) use run::SpiredStreamingDiscovery;

#[cfg(test)]
mod tests;
