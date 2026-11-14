//! Import of EXISTING material (hand-made rule tables, bank sets, recipe solves, supplied relations, terminals) into
//! the in-force dispatch, the strict closure check on it, OUR statement and the comparison with a hand statement
//! (interfaces/import.md; requirements R41 P-1, R40 U-9..U-11, R44, R48, R49).
//!
//! A LIBRARY (R22): no process, no ledger, no supervisor, no clock in any identifying field. Everything node-specific
//! (names, arities, orders, label schemes, file layouts, hand-statement columns) is DATA of the [`TableSetSpec`] (L8);
//! knock-out status and master bases are not inputs (L9).
//!
//! Pipeline: [`TableSetSpec`] -> [`load`] -> [`ImportedSet`] (+ [`Evidence::from_spec`]) -> [`check`] ->
//! [`CheckResult`] -> [`write_statement`] / [`compare`] with a [`HandStatement`].

pub mod check;
pub mod error;
pub mod evidence;
pub mod lines;
pub mod load;
pub mod recipe_steps;
pub mod spec;
pub mod statement;
pub mod symmetry;

pub use check::{CheckOptions, CheckResult, KeyRow, RowStatus, Server, SolveRequest, check};
pub use error::ImportError;
pub use evidence::{Evidence, EvidenceCounts};
pub use load::{ImportedSet, LoadReport, RuleInfo, WitnessKind, load};
pub use spec::{HandFormat, Level, TableSetSpec};
pub use statement::{
    Comparison, DiffKind, HandStatement, compare, read_hand_statement, write_statement,
};

/// Schema of OUR statement (summary.json).
pub const STATEMENT_SCHEMA: &str = "rustred.campaign.import-statement.v1";
/// Schema of a solve request line (interfaces/import.md 6.1).
pub const SOLVE_REQUEST_SCHEMA: &str = "rustred.campaign.solve-request.v1";
/// Schema of a solved relation line (interfaces/import.md 6.2).
pub const SOLVED_SCHEMA: &str = "rustred.campaign.solved-relation.v1";
/// Schema of a per-key proof request line (interfaces/import.md 6.4).
pub const PROOF_REQUEST_SCHEMA: &str = "rustred.campaign.instance-span-request.v1";
