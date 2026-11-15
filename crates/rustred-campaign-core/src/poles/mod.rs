//! # Spurious-pole diagnostics at fixed dimensions d0 and the preference "regular at d0" (lane poles;
//! interfaces/pole-diagnostics.md, all of it incl. addenda 1-7; TRICKS.md items 9, 12, 13)
//!
//! The Symbolica-free half (this crate takes no Symbolica, tests/schema_deps.rs): the data and the decisions.
//! The exact valuation itself (the order of a coefficient at d = d0, per key) lives in the algebra crate
//! (`rustred_campaign_algebra::valuation`), which fills the records defined here.
//!
//! - [`params`]: every spec parameter of section 10 + addendum 7 (`max_size_ratio`, default `"unbounded"`), parsed,
//!   defaulted and documented; no constant in code decides.
//! - [`record`]: the per-relation record `rustred.campaign.pole-relation.v1` (D1, section 2): deterministic,
//!   digest-bound to the relation's identity (`relation_digest` of the engine, `entry_digest` of
//!   `rustred.relation-entry.v1`).
//! - [`select`]: the SELECTION among admissible candidates (section 8 + addendum 7): regular at d0 first with the size
//!   guard `max_size_ratio`, then lower pole order, then fewer terms; a pure, order-independent function.
//! - [`split`]: a domain split into regular / singular parts by per-key verdicts, with an exact index condition when
//!   one exists and the generic cure candidates of TRICKS 13 (hints only).
//! - [`basis`]: the terminal-set check (D2, section 3 / 9) over node-basis diagnostic records (never evidence).
//! - [`census`]: the per-box-key census (D3, section 4) composition over a closure's edges, given per-edge orders.
//!
//! MUST NOT (section 13): no diagnostic and no preference decides acceptance, a closure verdict or the terminal bound;
//! the selection only CHOOSES among candidates the normal accept chain already admitted.
//! Generic (L8): no node, count, arity or family is known here; the five-loop data are test data.

pub mod basis;
pub mod census;
pub mod params;
pub mod record;
pub mod select;
pub mod split;

pub use params::{MaxSizeRatio, PoleParams, PolicyError};
pub use record::{PoleRelationRecord, RegularAt, TermPole};
pub use select::{
    Candidate, Choice, ChoiceReason, Measure, PoleObjective, PoleProfile, RegularAlternative, aggregate_orders,
    pole_objective, select,
};
pub use split::{DomainSplit, split_domain};
