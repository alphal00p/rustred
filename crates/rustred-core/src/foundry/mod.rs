//! Offline derivation of proof-bearing integral rules.
//!
//! The current capability remains deliberately below closure. [`anchored`]
//! derives exact rules at one concrete integer anchor, while [`parametric`]
//! derives exactly replayed recurrences on a fixed-sector interior and proves
//! agreement with anchored specializations. Both foundries offer their
//! original forward-selected rule and a target-directed serial-RREF path. The
//! parametric target path can also prove a recurrence on a sector-monotone
//! parent box, retaining every pinch cylinder as an explicit unresolved
//! proper-subsector dependency.
//! Neither is a resumable workspace, published closure artifact, or integral
//! reducer.

pub mod anchored;
pub mod parametric;

mod target_rref;
