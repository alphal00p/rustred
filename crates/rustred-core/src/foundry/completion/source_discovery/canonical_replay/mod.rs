//! Canonical common-plan replay of independent probe-local nominations.
//!
//! Probe-local exact circuits are never merged directly: each scheduler probe
//! owns a freshly tokened physical plan.  This boundary unions only complete
//! translated-source request identities, rebuilds one fresh common epoch,
//! resamples the retained raw probes, and exact-lifts again before any
//! candidate can enter promotion or semantic-owner compilation.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use build::try_canonicalize_replayed_probes;
pub(crate) use error::CanonicalReplayError;
pub(crate) use limits::CanonicalReplayLimits;
pub(crate) use model::{
    CanonicalRebaseAttempt, CanonicalRebaseAttemptOutcome, CanonicalRebasedCandidate,
    CanonicalReplayBatch, CanonicalReplayDisposition, CanonicalReplayTelemetry,
};

#[cfg(test)]
mod tests;
