//! Lane closeloop (interfaces/closeloop.md): the `WorkSource` that closes nodes from EXISTING material under the
//! supervisor: import -> strict closure check -> recipe solves (child jobs) -> per-key acceptance -> the grown closure
//! (R49) -> statement + comparison. Generic over nodes (L8): node names, paths, orders and budgets are configuration.

pub mod accept;
pub mod clauses;
pub mod config;
// lane t266run (closeloop.md 17): own escalation of uncovered keys, terminal proposals, derived spec part
pub mod derived;
pub mod ids;
// lane layers (closeloop.md 23): close the box layer by layer, earlier layers as leaves
pub mod layers;
pub mod pass;
pub mod proofs;
// lane t266run (closeloop.md 17): big-node helpers of the pass
pub mod scale;
pub mod source;
// lane termmin (interfaces/termmin.md): phase E, terminal minimisation (FOR CLOSELOOP2: one line)
pub mod termmin;
// lane closeloop4 (closeloop.md 18 / 19): second route of the zero test, terminals behind a decision
pub mod terminals;
pub mod zeros;
// lane repairflow (interfaces/repairflow.md): cheap symbolic first, else recipe export (FOR THE INTEGRATOR: one line)
pub mod repair;
// lane coreseed (interfaces/coreseed.md): the IBP core of a face, solved through solve-recipe children
pub mod coreseed;
// lane seedmin (interfaces/seedmin.md): solve-recipe children for the seed minimiser (REQUEST TO I-INT, one line)
pub mod seedmin;

pub use config::{CONFIG_ROLE, CONFIG_SCHEMA, CloseLoopConfig, NodeCfg, SolveCfg, Target};
pub use source::CloseFromMaterialSource;
