//! Transactional exact-cover deltas for canonical discovery proposals.
//!
//! This cold seam owns no discovery policy. It stages one topology-neutral
//! sector through the existing closure coordinator, delegates all semantic
//! and geometric authority to the exact owner-cover compiler, and reports
//! whether one canonical owner proposal strictly reduced the exact uncovered
//! box union. It never seals a cover or publishes a predecessor successor.

mod error;
mod geometry;
mod ledger;
mod limits;
mod model;

pub(crate) use error::ExactOwnerCoverDeltaError;
pub(crate) use ledger::CanonicalExactOwnerLedger;
pub(crate) use limits::ExactOwnerCoverDeltaLimits;
pub(crate) use model::{
    ExactOwnerCoverDelta, ExactOwnerCoverDeltaKind, ExactOwnerCoverSnapshot,
    ExactOwnerLedgerCoverStatus,
};

#[cfg(test)]
mod tests;
