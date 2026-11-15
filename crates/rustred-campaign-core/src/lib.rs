//! Dependency-light core of the automatic node campaign.
//!
//! Modules:
//! - [`key`]: arity-generic integral keys and node frames (sector, chart orthant).
//! - [`poly`]: sparse integer polynomials in the indices `n0..n{N-1}`, exact evaluation.
//! - [`record`]: streaming readers of rule records (rr_node `table_dump.v1`, case-domain export, T162 bank).
//! - [`domain`]: case domains (fixed slots, equations, chart, exceptions) and their verdict at a key.
//! - [`cover`]: prioritized tables, first-match dispatch, batch and union coverage.
//! - [`order`]: the node comparator on `&[i32]` slices (owned by the sibling package core-order).
//! - [`import`]: existing material (rule tables, bank sets, recipes, supplied relations) -> in-force dispatch ->
//!   closure check -> statement (interfaces/import.md).

pub mod cover;
pub mod domain;
pub mod key;
pub mod order;
pub mod poly;
pub mod record;
pub mod boxdef;
pub mod winner;
pub mod contain;
pub mod ibp_rows;
pub mod partition;
pub mod loci;
pub mod closure;
pub mod symmetry;
pub mod accept;
pub mod assemble;
pub mod worklist;
pub mod terminals;
pub mod explain;
pub mod repair;
pub mod economy;
pub mod glue;
pub mod replay_base;
pub mod independent_algebra;
pub mod schema;
// lane import (interfaces/import.md; listed by lane closeloop as its one line in this file)
pub mod import;
// lane termmin (interfaces/termmin.md): REQUEST TO I-INT, one line
pub mod termmin;
// lane repairflow (interfaces/repairflow.md): REQUEST TO I-INT, one line
pub mod repairflow;
// lane pieces (interfaces/pieces.md): REQUEST TO I-INT, one line
pub mod pieces;
