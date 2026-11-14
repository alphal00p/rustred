//! TERMINAL MINIMISATION BY POINT SOLVES (interfaces/termmin.md; COORDINATOR_REQUIREMENTS addendum 2026-11-14T15:02:45Z
//! and its correction 15:08:50Z): after a node is closed with ANY finite terminal set, every terminal that is not in a
//! supplied preferred set is attempted, highest D+r first, by an escalation sequence of point solves at that single key
//! (DATA: the in-force dispatch, the exact symmetry relation at the key, point recipes of growing seed sets up to the
//! caps); a relation is kept iff the closure extended by it stays closed and the number of terminals goes down (then
//! the closure size). A failed attempt never changes the closure. The tool does not know which terminals are true
//! masters (L9): a true master simply yields no relation.
//!
//! Library only (R22): `spec` (the data), `steps` (point recipes and their explicit seed sets), `symrel` (the symmetry
//! relation at the key), `machine` (the deterministic state machine and the attempt records), `extend` (the extension
//! check on imported material). Processes, files and the ledger belong to the caller (closeloop phase E, the direct
//! driver).

pub mod extend;
pub mod machine;
pub mod spec;
pub mod steps;
pub mod symrel;

pub use extend::ImportExtension;
pub use machine::{
    Candidate, ClosingSolve, ClosureOracle, Cost, Decision, Economy, Extension, KeySetOracle, Report, SolveAnswer,
    SolvePurpose, SolveRequest, StepRecord, SymStep, TerminalRecord, Termmin, attempt_order, d_r, is_hand_down,
};
pub use spec::{Budget, Caps, StepKind, TermminSpec};
pub use steps::{PointRecipe, key_text, point_recipe, priorities};
pub use symrel::{SymRelation, symmetry_relation_at_key};
