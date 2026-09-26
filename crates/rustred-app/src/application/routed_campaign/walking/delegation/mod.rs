//! Optional coordinator-owned transfer of unstarted inspection obligations.
//!
//! This protocol does not inspect domains, prove containment or replace native
//! guard/source/descent checks. The queue's exact retirement pass supplies
//! containment authority under one immutable snapshot and the same phase/owner.
//! Native event streams and diagnostic payloads remain owned by the scheduler.

mod ledger;
mod resolution;
mod types;

pub(super) use ledger::Ledger;
pub(super) use ledger::{LedgerRef, StoredLedger};
pub use types::SchedulingPolicy;
#[cfg(test)]
pub(super) use types::Summary;
pub(super) use types::{NativeOutcome, ResolutionStatus};

#[cfg(test)]
mod initial_overlap_tests;
#[cfg(test)]
mod queue_tests;
#[cfg(test)]
mod tests;
