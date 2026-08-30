//! Offline derivation of proof-bearing integral rules.
//!
//! [`anchored`] derives exact rules at one concrete integer anchor, while
//! [`parametric`] derives exactly replayed recurrences on a fixed-sector
//! interior and proves agreement with anchored specializations. Both
//! foundries offer their original forward-selected rule and a target-directed
//! serial-RREF path. The parametric target path can also prove a recurrence on
//! a sector-monotone parent box, retaining every pinch cylinder as an explicit
//! unresolved proper-subsector dependency.
//! [`artifact`] generates and seals the canonical unit-mass `K = 1` tadpole
//! and `K = 3` sunset partitions, and [`crate::reduction`] applies either
//! sealed owner through a topology-independent runtime. Three-loop `K = 6`
//! closure remains an explicit Stage 1 frontier.

pub mod anchored;
pub mod artifact;
pub mod cell;
pub mod dependency;
pub mod parametric;

mod target_rref;
