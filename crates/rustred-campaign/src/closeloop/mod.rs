//! Lane closeloop (interfaces/closeloop.md): the `WorkSource` that closes nodes from EXISTING material under the
//! supervisor: import -> strict closure check -> recipe solves (child jobs) -> per-key acceptance -> the grown closure
//! (R49) -> statement + comparison. Generic over nodes (L8): node names, paths, orders and budgets are configuration.

pub mod accept;
pub mod clauses;
pub mod config;
// lane t266run (closeloop.md 17): own escalation of uncovered keys, terminal proposals, derived spec part
pub mod derived;
pub mod ids;
pub mod pass;
pub mod proofs;
// lane t266run (closeloop.md 17): big-node helpers of the pass
pub mod scale;
pub mod source;
// lane termmin (interfaces/termmin.md): phase E, terminal minimisation (FOR CLOSELOOP2: one line)
pub mod termmin;

pub use config::{CONFIG_ROLE, CONFIG_SCHEMA, CloseLoopConfig, NodeCfg, SolveCfg, Target};
pub use source::CloseFromMaterialSource;
