//! Sector-box Laporta: one global row space over seeded sector boxes.
//!
//! The engine reduces the integrals of one family with a single row space and a
//! single column order:
//!
//! - rows are the family's ordinary IBP identities instantiated at seed
//!   integrals, plus the router's automorphism rows ([`CensusRouter`]);
//! - columns are integrals, ordered hardest first ([`ColumnOrder`]), with
//!   preferred masters forced to the easy end;
//! - seeds are chosen top-down by sector, sized from the demand;
//! - a modular image of the whole system (Zp, `p = 2^61 - 1` by default) is
//!   eliminated by one Symbolica `SparseRowReducer` with L patterns
//!   (the solver's `Discovery` with a [`ColumnOrder`]);
//! - the masters are the non-pivot columns carrying a nonzero reduced-row
//!   coefficient for some demanded integral, decided at two independent
//!   samples, and the rows needed to reproduce the result are selected from
//!   the L dependencies (Kira 3 style).
//!
//! For a fixed row space and column order every echelon form has the same
//! pivot set, so masters do not depend on row insertion order, batching or
//! worker count. Because every parameter is sampled, the modular structure
//! does not depend on `d` or on any other parameter value.
//!
//! Seeding, truncation and growth follow the published Kira algorithms
//! (arXiv:1705.05610, arXiv:2505.20197); the implementation is clean room
//! (papers and black-box behaviour only).
//!
//! This module is added in steps. This step contains:
//! - [`ColumnOrder`] presets and [`SlotPriority`] (the tie-break of the
//!   lexicographic part, Kira's propagator permutation scheme 1);
//! - [`SectorRouter`]: the [`IdentityRouter`] (every sector is its own
//!   representative; zero sectors from [`crate::sector::zero`]) and the
//!   [`CensusRouter`] (routes integrals onto representative sectors of the
//!   vacuum sector-symmetry census and supplies automorphism rows);
//! - the Zp row factory: IBP templates compiled once, instantiated at seeds
//!   and parameter samples, with routing of every term onto representative
//!   sectors and exact zero-sector decisions.
//! - the global modular system (one `Discovery` over every seeded row, rows
//!   inserted as batches easiest first, so the system is a function of the
//!   row set) and the selection kernel: the closure of a demand over U,
//!   restricted back-substitution to reduced rows and masters, row selection
//!   from the L dependencies, and the second-sample replay of the selected
//!   rows;
//! - seed boxes and regions (Kira's `(r, s, d)` boxes and their unions, with
//!   shell enumeration between an old and a new region).
//!
//! The session (demand propagation, seeding, elimination, master selection)
//! and the finite-field and exact lanes follow in the next steps.

// `modular`, `order`, `rows`, `seeds` and `select` are used by the session (a later
// step of the series); their unit and property tests exercise them here.
#[allow(dead_code)]
mod modular;
#[allow(dead_code)]
mod order;
mod router;
mod router_census;
#[cfg(test)]
mod router_tests;
#[allow(dead_code)]
mod rows;
#[allow(dead_code)]
mod seeds;
#[allow(dead_code)]
mod select;
#[cfg(test)]
mod select_tests;

use std::fmt;

pub use order::{ColumnOrder, SlotPriority};
pub use router::{IdentityRouter, Route, SectorRouter};
pub use router_census::CensusRouter;
pub use seeds::{SeedBox, SeedRegion};

/// Default structure prime of the modular samples, `2^61 - 1`.
pub const DEFAULT_PRIME: u64 = (1 << 61) - 1;

/// The phase in which a modular sample proved unlucky.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxPhase {
    /// A family condition vanishes at the sample point.
    Conditions,
    /// The second-sample replay of the selected rows changed a pivot.
    Replay,
    /// The second sample found a master outside the first sample's closure.
    MasterTest,
}

impl fmt::Display for BoxPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Conditions => "sample conditions",
            Self::Replay => "selected-row replay",
            Self::MasterTest => "two-sample master test",
        })
    }
}
