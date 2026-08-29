//! Offline derivation of proof-bearing integral rules.
//!
//! [`anchored`] derives exact rules at one concrete integer anchor, while
//! [`parametric`] derives exactly replayed recurrences on a fixed-sector
//! interior and proves agreement with anchored specializations. Both
//! foundries offer their original forward-selected rule and a target-directed
//! serial-RREF path. The parametric target path can also prove a recurrence on
//! a sector-monotone parent box, retaining every pinch cylinder as an explicit
//! unresolved proper-subsector dependency.
//! [`artifact`] can now generate and seal the canonical one-loop unit-mass
//! vacuum partition, and [`crate::reduction`] applies any such sealed owner
//! through a topology-independent runtime. Durable encoding/loading and
//! closure of any two- or higher-loop family remain explicit frontiers.

pub mod anchored;
pub mod artifact;
pub mod dependency;
pub mod parametric;

mod target_rref;
