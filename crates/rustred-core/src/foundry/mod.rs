//! Offline derivation of proof-bearing integral rules.
//!
//! The current capability remains deliberately below closure. [`anchored`]
//! derives one exact rule at one concrete integer anchor, while
//! [`parametric`] derives one exactly replayed recurrence on a fixed-sector
//! interior and proves agreement with an anchored specialization. Neither is
//! a resumable workspace, published closure artifact, or integral reducer.

pub mod anchored;
pub mod parametric;
