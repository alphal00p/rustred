//! Offline derivation of proof-bearing integral rules.
//!
//! [`anchored`] derives exact rules at one concrete integer anchor, while
//! [`parametric`] derives exactly replayed recurrences on a fixed-sector
//! interior and replays them directly in the exact base field. Both
//! foundries offer their original forward-selected rule and a target-directed
//! serial-RREF path. The parametric target path can also prove a recurrence on
//! a sector-monotone parent box, retaining every pinch cylinder as an explicit
//! unresolved proper-subsector dependency.
//! [`search`] plans complete, bounded same-sector L1 translation diamonds and
//! exact finite RuleCell reachability censuses; it owns no topology or
//! elimination logic, and neither search certifies an infinite domain.
//! [`artifact`] generates and seals the canonical unit-mass `K = 1` tadpole
//! and `K = 3` sunset partitions, and [`crate::reduction`] applies either
//! sealed owner through a topology-independent runtime. Three-loop `K = 6`
//! closure remains an explicit Stage 1 frontier.

pub mod anchored;
pub mod artifact;
pub mod cell;
// The completion engine remains crate-private while Stage 1 turns its exact
// discovery evidence into published artifacts.  Compiling it in production
// prevents the offline lowering path from drifting away from the code that
// will eventually own K = 6 closure.
#[allow(dead_code)]
pub(crate) mod completion;
pub mod dependency;
pub mod parametric;
pub mod search;

mod target_rref;
