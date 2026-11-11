//! Node-level generation tools consolidated from the 5L campaign (rr-consolidate, 2026-11-11).
//!
//! * [`order`]: a node order (presentation permutation, tie priority, source visit order) and its probe variants;
//! * [`screen`]: the lift pre-screen policy (modular estimate of an exact lift before it runs);
//! * [`driver`]: the case-queue generation of one chart under one node order (the former face-queue harness),
//!   with the order recorded in the table and enforced on the numeric endpoint cases;
//! * [`auto`]: bounded depth-1 probes of order variants at a stuck case and the automatic choice of ONE order for a
//!   whole node;
//! * [`domain`]: integer domain of a queued case (empty / finite / infinite) before any lift;
//! * [`descent`]: strict-descent check of an assembled rule set under its one declared order;
//! * [`replay`]: the independent numeric replay of a table (P61, three values of d, all physical columns, independent
//!   IBP-span witness, mutation control), semantics of Astra's `p5l-replay.v1`.
//!
//! The command-line entry point is the `rr_node` binary of this crate.

// nested `if let` blocks are kept in the ladder / replay code where a let chain would hide the order of the checks
#![allow(clippy::collapsible_if)]

pub mod auto;
pub mod descent;
pub mod domain;
pub mod driver;
pub mod order;
pub mod replay;
pub mod screen;
