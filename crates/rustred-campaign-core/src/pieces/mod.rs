//! Lane pieces (interfaces/pieces.md; TRICKS.md item 4 and the gate half of item 1): the PIECE PLANNER and the SHIP
//! GATE, pure deterministic functions over data (keys, candidate services, shipped relations), independent of how the
//! services were found and of any node (L8).
//!
//! - [`plan::plan`]: one chosen service per unserved key (all open faces together) such that every chosen edge
//!   strictly descends under the node's full order AND the plan's key graph is acyclic by an independent graph check;
//!   the three pieces per face ((i) rules, (ii) hand-downs, (iii) the IBP core), their solving order, the keys left
//!   to the core, and the face graph (possibly cyclic) next to the key- and piece-level verdicts.
//! - [`gate::ShipGate`]: streaming gate over everything shipped for a node: 0 non-descending edges under the full order
//!   AND acyclicity by Kahn over the edges (in memory by default, or spilled to disk); disagreement is a hard
//!   `Inconsistent`.
//! - [`import_edges`]: the shipped relations of an import check (rules at their keys, composites of credits, relation
//!   right-hand sides) for the gate.
//! - [`keychoices`]: the planner input from the repair flow's per-key record of all acceptable choices.
//! - [`graph`]: interner, in-memory / on-disk adjacency, Kahn, witness cycle, Tarjan.

pub mod gate;
pub mod graph;
pub mod import_edges;
pub mod keychoices;
pub mod order;
pub mod plan;

use serde::{Deserialize, Serialize};

pub use gate::{GateConfig, GateReport, GateStorage, GateVerdict, ShipGate};
pub use order::{EdgeOrder, FnOrder, KeyOrder};
pub use plan::{Candidate, Plan, PlanError, PlanInput, Ranking, ServiceKind, UnservedKey, plan};

/// Exact non-zero-ness of a right-hand-side term. `Zero` (a CONFIRMED zero) is not an edge; `Yes`, `Unknown` and
/// `ZeroUnconfirmed` are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonZero {
    Yes,
    Zero,
    Unknown,
    /// audit r2-planners G1: the FIRST route says zero, no independent second route confirmed it: an EDGE; non-descending
    /// it refuses the gate with the typed reason `all_zeros_unconfirmed:N` (the closing loop's rule "zeros need a second
    /// route").
    ZeroUnconfirmed,
}

/// One right-hand-side term.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Term {
    pub key: Vec<i32>,
    pub nonzero: NonZero,
}

impl Term {
    pub fn new(key: Vec<i32>, nonzero: NonZero) -> Term {
        Term { key, nonzero }
    }
    /// A term whose non-zero-ness is unknown (an edge).
    pub fn unknown(key: Vec<i32>) -> Term {
        Term {
            key,
            nonzero: NonZero::Unknown,
        }
    }
}
