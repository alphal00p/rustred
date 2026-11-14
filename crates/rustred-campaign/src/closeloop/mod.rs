//! Lane closeloop (interfaces/closeloop.md): the `WorkSource` that closes nodes from EXISTING material under the
//! supervisor: import -> strict closure check -> recipe solves (child jobs) -> per-key acceptance -> the grown closure
//! (R49) -> statement + comparison. Generic over nodes (L8): node names, paths, orders and budgets are configuration.

pub mod accept;
pub mod config;
pub mod ids;
pub mod pass;
pub mod proofs;
pub mod source;
// lane termmin (interfaces/termmin.md): phase E, terminal minimisation (FOR CLOSELOOP2: one line)
pub mod termmin;
// lane repairflow (interfaces/repairflow.md): cheap symbolic first, else recipe export (FOR THE INTEGRATOR: one line)
pub mod repair;

pub use config::{CONFIG_ROLE, CONFIG_SCHEMA, CloseLoopConfig, NodeCfg, SolveCfg, Target};
pub use source::CloseFromMaterialSource;
