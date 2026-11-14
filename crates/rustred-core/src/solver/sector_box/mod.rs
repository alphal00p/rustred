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
//! - the session [`BoxLaportaSession`]: top-down demand propagation through
//!   routes, Kira-box seeding ([`Seeding::KiraBox`], with Kira 3 numerator
//!   truncation), parallel row generation and batched insertion into the
//!   global modular system, the two-sample master selection with row
//!   selection, preferred masters (forced to the easy end of the column
//!   order) and per-sector reports with Lee-Pomeransky counts.
//!
//! Demand and staircase seeding with growth, the master check, numerator
//! tubes and the stability check, and the finite-field and exact lanes follow
//! in the next steps.

mod driver;
mod modular;
mod order;
mod router;
mod router_census;
#[cfg(test)]
mod router_tests;
mod rows;
mod seeds;
mod select;
#[cfg(test)]
mod select_tests;
mod system;

use std::fmt;
use std::time::Duration;

pub use driver::{BoxLaportaSession, census_targets, down_closure_representatives};
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

/// Options of one sector-box session.
#[derive(Clone, Debug)]
pub struct BoxOptions {
    pub seeding: Seeding,
    pub order: ColumnOrder,
    pub sample: Sample,
    /// Opt-in caps; the default is no cap.
    pub limits: BoxLimits,
    /// Row-generation threads. Results do not depend on this number.
    pub workers: usize,
    /// Per-sector Lee-Pomeransky count (`MasterCounter`), reported per
    /// sector and compared with the in-box residuals.
    pub lp_gate: bool,
    /// Row insertion schedule of the modular elimination.
    pub insertion: Insertion,
    /// Kira 3 `truncate_sp` parameter `l` (arXiv:2505.20197 Eq. 9): a
    /// sector with `t` lines inherits numerator rank at most
    /// `max(1, t - l + 1, s_set)` from its supersectors, where `s_set` is
    /// the rank set explicitly for that sector by its own demand (0 when it
    /// has none). Explicit (demand) boxes are never capped; dots are never
    /// capped. `None` disables the truncation (Kira <= 2.3 inheritance).
    pub truncate: Option<i16>,
    /// Lexicographic tie-break priority of the slots in the column order.
    pub slot_priority: SlotPriority,
}

/// Row insertion schedule. Pivots and masters do not depend on it (one row
/// space, one column order); the elimination cost does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Insertion {
    /// Plan every level's boxes top-down, then insert all new rows as one
    /// batch, easiest leading integral first (lowest sectors first), so
    /// subsector columns already have pivots when supersector rows arrive.
    #[default]
    BottomUp,
    /// Insert each sector level as it is planned, highest level first, and
    /// size subsector boxes from the U-closure of the demand. Supersector
    /// rows that are dependent on their own sector then become long pivot
    /// rows in subsectors ("spill"), which makes elimination slower.
    TopDownLevels,
}

impl Default for BoxOptions {
    fn default() -> Self {
        Self {
            // One dot and one numerator in every sector, any number of lines.
            seeding: Seeding::KiraBox { r: 63, s: 1, d: 1 },
            order: ColumnOrder::Numeric,
            sample: Sample::default(),
            limits: BoxLimits::default(),
            workers: 1,
            lp_gate: true,
            insertion: Insertion::BottomUp,
            truncate: None,
            slot_priority: SlotPriority::Natural,
        }
    }
}

/// Seed placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seeding {
    /// Kira's box: in every nonzero subsector of a demanded sector, all
    /// integrals with `r <= r`, `s <= s` and at most `d` dots (non-demand
    /// subsectors with `s` capped by [`BoxOptions::truncate`]).
    KiraBox { r: u8, s: u8, d: u8 },
}

/// The modular sample. Parameters (including `d`) are drawn from `seed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub prime: u64,
    pub seed: u64,
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            prime: DEFAULT_PRIME,
            seed: 0x5345_4354_4f52_424f,
        }
    }
}

/// Opt-in caps. Exceeding one is a typed [`crate::solver::SolverError::Limit`]
/// error. The default is no cap: memory is the caller's admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxLimits {
    pub max_rows: usize,
    pub max_columns: usize,
    pub max_u_nnz: usize,
    pub max_l_entries: usize,
    pub max_selected: usize,
    /// Cap on the structural estimate of the modular state (U, L, columns,
    /// provenance), not on allocator bytes.
    pub max_bytes: usize,
    pub wall: Option<Duration>,
}

impl Default for BoxLimits {
    fn default() -> Self {
        Self {
            max_rows: usize::MAX,
            max_columns: usize::MAX,
            max_u_nnz: usize::MAX,
            max_l_entries: usize::MAX,
            max_selected: usize::MAX,
            max_bytes: usize::MAX,
            wall: None,
        }
    }
}

/// Which cap a [`crate::solver::SolverError::Limit`] refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    Rows,
    Columns,
    UNonzeros,
    LEntries,
    Selected,
    Bytes,
    Wall,
}

impl fmt::Display for LimitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rows => "rows",
            Self::Columns => "columns",
            Self::UNonzeros => "U nonzeros",
            Self::LEntries => "L entries",
            Self::Selected => "selected rows",
            Self::Bytes => "structural bytes",
            Self::Wall => "wall seconds",
        })
    }
}

/// Work of one insertion batch or selection step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhaseStats {
    pub phase: String,
    pub seeds: usize,
    pub rows: usize,
    /// Terms of the generated rows.
    pub terms: usize,
    /// Router automorphism rows among `rows`.
    pub automorphism_rows: usize,
    pub columns: usize,
    pub rank: usize,
    pub u_nnz: usize,
    pub l_entries: usize,
    pub selected: usize,
    /// Wall seconds of row generation in this phase.
    pub generation_seconds: f64,
    /// Wall seconds of modular elimination (or selection) in this phase.
    pub seconds: f64,
    /// Structural estimate of resident modular state, not allocator bytes.
    pub bytes: usize,
}

/// Session totals plus one entry per phase.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxStats {
    pub seeds: usize,
    pub rows: usize,
    /// Terms of the rows of the current system.
    pub terms: usize,
    pub automorphism_rows: usize,
    pub empty_rows: usize,
    pub columns: usize,
    pub rank: usize,
    pub u_nnz: usize,
    pub l_entries: usize,
    pub column_batches: usize,
    pub generation_seconds: f64,
    pub elimination_seconds: f64,
    pub selection_seconds: f64,
    pub lp_seconds: f64,
    pub phases: Vec<PhaseStats>,
}

/// Per-sector outcome of one `extend`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorReport {
    /// Kira sector id `sum 2^i` over active slots.
    pub id: u32,
    pub lines: usize,
    /// The hull of the seeded region, if any.
    pub seed_box: Option<SeedBox>,
    /// The seeded region (maximal boxes); one box with Kira boxes.
    pub seed_region: Vec<SeedBox>,
    /// Seeds of the region and of its hull.
    pub seeds: usize,
    pub hull_seeds: usize,
    pub masters: usize,
    /// `Some(k)` for a Lee-Pomeransky count, `None` for no verdict or no gate.
    pub lp: Option<usize>,
    pub lp_no_verdict: bool,
    /// Non-pivot columns of this sector inside its seeded box.
    pub in_box_residuals: usize,
}

/// Result of the modular phase of `extend`.
#[derive(Clone, Debug, PartialEq)]
pub struct ModularReport<const N: usize> {
    /// Masters in column order (hardest first).
    pub masters: Vec<[i16; N]>,
    pub sectors: Vec<SectorReport>,
    /// Demanded integrals (targets and preferred) after zero-sector removal.
    pub demand: usize,
    /// Targets and preferred integrals in proved-zero sectors.
    pub zero_demand: usize,
    pub closure_columns: usize,
    pub closure_pivots: usize,
    pub candidates: usize,
    /// Selected original rows (L-dependency closure of the closure pivots).
    pub selected_rows: usize,
    /// blake3 of the sorted pivot integrals of the whole system.
    pub pivot_hash: String,
    /// blake3 of the selected rows' provenance `(seed, source)`, in order.
    pub selected_hash: String,
    /// blake3 of the masters.
    pub master_hash: String,
    /// Masters nonzero at the first sample / at the replay sample.
    pub masters_first_sample: usize,
    pub masters_second_sample: usize,
    pub rounds: usize,
    /// Sectors grown in each round, by id (empty with Kira boxes).
    pub growth: Vec<Vec<u32>>,
    /// Sectors whose in-box residuals fell below their LP count at some step
    /// (identity routing only: symmetry rows legitimately go below LP).
    pub monitor_violations: Vec<(u32, usize, usize)>,
    /// The truncation parameter `l` in force at the end (growth may lower
    /// it); `None` without truncation.
    pub truncate: Option<i16>,
    /// Seeds of the whole session, and the seeds the hull of every seeded
    /// region would need (rows versus the hull).
    pub seeds: usize,
    pub hull_seeds: usize,
    pub seconds: f64,
}
