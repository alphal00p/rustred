//! Work list, ladder and seed-search rung of the boxed / full fixer (plan 2.4.3, 2.4.4, 2.4.13, 2.4.14 (b)),
//! as a PURE, deterministic library (R22): no processes, files, ledger, clock or global state.
//!
//! Interface: `plan/auto-campaign/interfaces/repair-library.md` (and `seed-policy.md` for [`policy`]).
//!
//! - [`affine`]: integer affine equations, canonical case equations (RREF), fits through points.
//! - [`items`]: faces, cases, items, item ids, node parameters, item construction (partition report / closure keys).
//! - [`seed`]: the seed-search space (centres, floor, depth), budgets, the ONE mapping to engine argv.
//! - [`policy`]: the seed-policy file `rustred.campaign.seed-policy.v1` and its evidence records (R37).
//! - [`ladder`]: acceptance at the gap points, selection, the state machine [`ladder::next_steps`].
//! - [`children`]: sub-cases from the accepted rules' exception sets, standard planes, fits, point items.

pub mod affine;
pub mod children;
pub mod items;
pub mod ladder;
pub mod policy;
pub mod seed;

pub use affine::AffineEq;
pub use items::{CaseSpec, Face, Item, ItemId, ItemKind, NodeParams};
pub use ladder::{Disposition, ItemState, Next, next_steps};
pub use seed::{CentreKind, SeedSetting, engine_seed_args};
