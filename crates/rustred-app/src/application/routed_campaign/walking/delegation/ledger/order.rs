//! Measurement-only Ready dispatch-order knob (fable_5_1 next push, W0.8).
//!
//! `RUSTRED_WALK_DISPATCH_ORDER` selects which Unreserved obligation a freed
//! Ready credit reserves next. Order is performance-only: it never changes
//! which containments are proved, only which pending ID is inspected first
//! and therefore which later admissions can still transfer it. `fifo` (the
//! default, and the only value allowed outside Ready) keeps the historical
//! monotone reservation scan byte for byte. The other orders are not
//! resumable: their candidate structures are not persisted, so a checkpoint
//! written under them fails the restore validator by design.
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

pub(in super::super::super) const DISPATCH_ORDER_ENV: &str = "RUSTRED_WALK_DISPATCH_ORDER";
/// Starvation bound of the priority orders: when the oldest Unreserved ID
/// trails the newest admission by more than this many IDs, it is reserved
/// first (FIFO fallback). Unset: no bound.
pub(in super::super::super) const AGE_BOUND_ENV: &str = "RUSTRED_WALK_ORDER_AGE_BOUND";

pub(in super::super::super) fn age_bound_from_env() -> Result<Option<usize>, String> {
    match std::env::var(AGE_BOUND_ENV) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => value
            .trim()
            .parse::<usize>()
            .map(Some)
            .map_err(|error| format!("{AGE_BOUND_ENV}={value:?}: {error}")),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(format!("{AGE_BOUND_ENV}: {error}")),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super::super) enum DispatchOrder {
    /// Historical ID-ordered reservation scan.
    #[default]
    Fifo,
    /// Owner support t descending, then (log) box volume descending, then ID.
    SupportVolume,
    /// Pending IDs in the unresolved cones of initial roots whose blocker
    /// set is small first (closure monitor), FIFO otherwise.
    ClosureBoost,
    /// Newest Unreserved ID first (LIFO).
    DepthFirst,
}

impl DispatchOrder {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "fifo" => Ok(Self::Fifo),
            "support-then-volume" | "support-volume" => Ok(Self::SupportVolume),
            "closure-boost" => Ok(Self::ClosureBoost),
            "depth-first" | "lifo" => Ok(Self::DepthFirst),
            other => Err(format!(
                "{DISPATCH_ORDER_ENV}={other:?}: expected fifo, support-then-volume, closure-boost or depth-first"
            )),
        }
    }

    pub fn from_env() -> Result<Self, String> {
        match std::env::var(DISPATCH_ORDER_ENV) {
            Ok(value) => Self::parse(&value),
            Err(std::env::VarError::NotPresent) => Ok(Self::Fifo),
            Err(error) => Err(format!("{DISPATCH_ORDER_ENV}: {error}")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::SupportVolume => "support-then-volume",
            Self::ClosureBoost => "closure-boost",
            Self::DepthFirst => "depth-first",
        }
    }

    pub fn needs_priority(self) -> bool {
        self == Self::SupportVolume
    }
}

/// Dispatch priority of one admitted domain for `SupportVolume`: owner
/// support t in the top bits, then the quantized log2 box volume. A plain
/// performance heuristic, never a containment or closure statement.
pub(in super::super::super) fn support_volume_priority(support: usize, log2_volume: f64) -> u64 {
    const VOLUME_BITS: u32 = 48;
    let volume = if log2_volume.is_finite() && log2_volume > 0.0 {
        ((log2_volume * 1024.0) as u64).min((1 << VOLUME_BITS) - 1)
    } else {
        0
    };
    ((support as u64).min(u64::from(u16::MAX)) << VOLUME_BITS) | volume
}

/// Candidate structures of the non-FIFO orders. Every admitted ID is offered
/// exactly once at admission; entries that stopped being Unreserved
/// (transferred, or reserved through another structure) are skipped lazily.
#[derive(Debug, Default)]
pub(super) struct Prioritized {
    pub heap: BinaryHeap<(u64, Reverse<usize>)>,
    pub stack: Vec<usize>,
    /// ClosureBoost base order and the age-bound fallback: ID-ordered scan
    /// over Unreserved entries.
    pub fifo_scan: usize,
    pub age_bound: Option<usize>,
    pub boost: VecDeque<usize>,
    boosted: Vec<u64>,
    /// Reserved IDs not yet handed to a worker, in reservation order.
    pub reserved: VecDeque<usize>,
    /// Transferred IDs whose alias is not yet published.
    pub transferred: Vec<usize>,
}

impl Prioritized {
    /// Marks `id` boosted; false when it already was.
    pub fn mark_boosted(&mut self, id: usize) -> bool {
        let word = id / 64;
        if self.boosted.len() <= word {
            self.boosted.resize(word + 1, 0);
        }
        let bit = 1u64 << (id % 64);
        let fresh = self.boosted[word] & bit == 0;
        self.boosted[word] |= bit;
        fresh
    }
}

/// Telemetry of the reservation order, identical in meaning for every order
/// (FIFO included). Not persisted: counts cover this process session.
#[derive(Clone, Debug, Default)]
pub(super) struct OrderStats {
    /// Reverse retirements (a newer admission contains the old candidate),
    /// by the old ID's responsibility when it was retired.
    pub retired_transferred: usize,
    pub retired_reserved: usize,
    pub retired_started: usize,
    pub retired_published: usize,
    pub retired_protected_initial: usize,
    pub retired_other: usize,
    /// Currently Unreserved local obligations, and the peaks of that count
    /// and of admitted-but-unpublished IDs (pending obligations).
    pub unreserved: usize,
    pub peak_unreserved: usize,
    pub peak_pending: usize,
    pub reservations: usize,
    pub boost_pushed: usize,
    pub boost_reserved: usize,
    pub boost_rounds: usize,
    /// Reservations taken by the age-bound FIFO fallback.
    pub aged_reservations: usize,
}

impl OrderStats {
    /// Retired candidates whose native inspection had already been committed
    /// to (Reserved, Started or Published): inspections later contained.
    pub fn mistakes(&self) -> usize {
        self.retired_reserved + self.retired_started + self.retired_published
    }

    pub fn json(&self, order: DispatchOrder, native_publications: usize) -> Value {
        let per_native = |count: usize| {
            (native_publications > 0).then(|| count as f64 / native_publications as f64)
        };
        json!({"order":order.name(),
            "retired_transferred":self.retired_transferred,
            "retired_reserved":self.retired_reserved,
            "retired_started":self.retired_started,
            "retired_published":self.retired_published,
            "retired_protected_initial":self.retired_protected_initial,
            "retired_other":self.retired_other,
            "mistakes":self.mistakes(),
            "mistakes_per_native":per_native(self.mistakes()),
            "transfers_per_native":per_native(self.retired_transferred),
            "unreserved":self.unreserved,"peak_unreserved":self.peak_unreserved,
            "peak_pending":self.peak_pending,"reservations":self.reservations,
            "boost_pushed":self.boost_pushed,"boost_reserved":self.boost_reserved,
            "boost_rounds":self.boost_rounds,"aged_reservations":self.aged_reservations,
            "scope":"this_process_session; mistakes = reverse retirements of IDs already Reserved, Started or Published; measurement knob, not persisted"})
    }
}
