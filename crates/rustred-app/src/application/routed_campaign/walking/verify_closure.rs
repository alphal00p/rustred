//! Offline closure oracle over a saved CP5 generation (plan W0.2, v3 note
//! F8/F10). Everything a closure claim rests on is re-derived here from the
//! raw sections and a reference inspection, never from the walker's restore
//! validators, containment index or dependency tracker:
//!
//! - the checkpoint is bound to the request (queries included) by its digest;
//!   roots are the query records, each re-checked to contain its query; an
//!   optional `result.json` is bound to the generation record by record;
//! - record images equal the saved domain images; records seal only as
//!   (native with 0 frontiers and no error) or alias (F8), and the saved
//!   seal flags must agree;
//! - every alias is an exact integer-set subset of its representative in the
//!   same (phase, owner), with its dependency edge; every partial record is
//!   an Apply record anchored (with its edge) on an EARLIER INITIAL record
//!   that is itself a Native inspection (so the anchor relation is
//!   well-founded: no self-anchor, partial-on-partial chain or anchor cycle),
//!   its D >= cut slice lies in that anchor, its recorded residual is the
//!   D < cut slice, and Q lies in anchor u residual (exact union cover);
//! - every G2' residual-anchor record (`g2_residual_anchor_inspection`) is
//!   an Apply record at or after the initial prefix whose recorded merge
//!   stamp is its own position in the publication stream (re-derived here
//!   from the record order), whose snapshot stamp is at most that position,
//!   and each of whose anchors carries its edge, its true position as stamp,
//!   a position strictly below the snapshot (merge order is well-founded: no
//!   self-anchor or anchor cycle), the same (phase, owner) and an admissible
//!   kind: a Native inspection, an initial-D-band partial (lending only its
//!   D < cut slice) or a G2' record with a residual (lending its domain),
//!   sealed (0 frontiers, no error); and Q lies in residual u anchor scopes
//!   (exact multi-target union cover, `covered_by_union`);
//! - closure is re-derived from the edge set (reverse reachability from
//!   unsealed nodes, cross-checked by forward cones per root); an engine
//!   closed flag that the oracle does not re-derive is a false claim;
//! - F10: natives are re-inspected with the walker's own native visitor
//!   under the run's request, with the walk-level reuse levers removed;
//!   frontier, error, event and (Apply) successor counts must match the
//!   record, and every admitted domain must be contained, in the same phase
//!   and owner, in a recorded target of its parent or along that target's
//!   alias chain. Successor generation is thus reproduced, not independently
//!   derived; F10 independently checks the graph bookkeeping.
//!
//! Verdicts: FAIL on any violation; INCOMPLETE when no violation was found
//! but not every native was re-inspected (a partial check is never a PASS);
//! PASS otherwise. Inclusion uses `lattice::Cell`, cross-checked by
//! brute-force lattice enumeration on small cells; `--union-sample` checks
//! the exact multi-target cover predicate against enumeration on sampled
//! real cells (`union_sample`). Mutations (`--mutate`)
//! inject one defect in memory after loading; each must turn the verdict
//! into FAIL (the alias-chain detour is a positive control that must PASS).
#[cfg(all(test, feature = "cli"))]
mod e2e_tests;
mod epoch_checkpoint;
mod epoch_export;
mod epoch_g2;
mod finite_replay;
#[cfg(all(test, feature = "cli"))]
mod g2_e2e_tests;
mod graph;
mod inventory;
pub use inventory::{
    OwnerDomainWalkInventory, OwnerDomainWalkInventoryOptions, owner_domain_walk_inventory,
};
pub(super) mod lattice;
mod result_binding;
#[cfg(test)]
mod test_scratch;
mod union_sample;

use super::super::{RoutedCampaignRequest, input, matching, prepare};
use super::{
    OwnerDomainWalkRequest, checkpoint,
    inspection::{self, Effect, Event},
    mask,
    queue::{CompactDomain, Domain, Phase},
};
use crate::AppError;
use graph::Graph;
use lattice::Cell;
use result_binding::Digest;
use rustred::solver::{DomainPowerBounds, RoutedCandidateReducer};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const FLAG_SEALED: u8 = 1;
const FLAG_CLOSED: u8 = 4;
pub const OWNER_DOMAIN_WALK_VERIFY_SCHEMA: &str = "rustred.walk-verify-closure.v2";
/// Forward cones are computed per root while roots x (nodes + edges) stays
/// below this many visits.
const CONE_VISIT_BUDGET: u128 = 20_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyReinspect {
    All,
    None,
    Sample { count: usize, seed: u64 },
}

/// Native levers of the F10 reference inspection.
///
/// The reference always removes the walk-level levers (`inspect_reference`:
/// job-local reuse cache, pre-admitted orthant shortcuts, initial-overlap
/// planning; physical Apply subdivision is refused). `Off` (the default)
/// also forces every native lever in `REFERENCE_NATIVE_LEVERS` to its exact
/// setting; `AsRun` keeps the run's native policies. Request fields that
/// define the obligations themselves (match limits and refinement axes, Apply
/// cell refinement, Route overcover, Route mask and native resource
/// allowances) are always kept: they are bound in the request digest and
/// change which pieces exist, not how a piece is decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyReferenceLevers {
    Off,
    AsRun,
}

/// Native shortcuts the `Off` reference disables. In this tree the only one
/// is the opt-in Route joint source-support pruning. The planned native
/// levers N1 (modular zero certificates) and N4 (cover-first) do not exist
/// yet; each must be added here, and forced off in `reference_request`,
/// when it lands (plan W1.2/W4.3, audit directive "F10 runs with N1/N4 off").
const REFERENCE_NATIVE_LEVERS: [&str; 1] = ["route_joint_source_support_pruning"];

/// The request the reference inspects under, and the native levers that
/// were on in the run but are off in the reference.
fn reference_request(
    request: &OwnerDomainWalkRequest,
    levers: OwnerDomainWalkVerifyReferenceLevers,
) -> (OwnerDomainWalkRequest, Vec<&'static str>) {
    let mut reference = request.clone();
    let mut differing = Vec::new();
    if levers == OwnerDomainWalkVerifyReferenceLevers::Off {
        if reference.route_joint_source_support_pruning {
            differing.push(REFERENCE_NATIVE_LEVERS[0]);
        }
        reference.route_joint_source_support_pruning = false;
    }
    (reference, differing)
}

/// One injected defect; the verdict must become FAIL, except for the
/// positive control `AliasChainDetour`, which must still PASS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyMutation {
    /// Drop the only recorded edge covering some successor.
    DroppedEdge,
    /// Point an alias (record and edge) at a same-bucket non-container.
    RetargetedAlias,
    /// Forget a native's frontiers and seal it (saved counter unchanged).
    DroppedFrontierRecord,
    /// Seal a node whose record carries a frontier.
    SealWithFrontier,
    /// Seal a deep (non-root) native whose record carries an error.
    SealWithError,
    /// Replace the only covering edge by one to a sealed non-container.
    InjectedFalseHit,
    /// Forget a native's frontiers consistently: record, saved frontier
    /// counter, seal flag and engine closed flags all agree; only the
    /// per-node re-inspection can see it.
    HiddenFrontier,
    /// A native whose true inspection fails, recorded (and counted) as a
    /// success: the reference inspection of one sealed deep Apply native is
    /// forced to fail with a resource limit; only per-node error parity can
    /// see it.
    HiddenError,
    /// One native's event count off by one, saved counter adjusted.
    MiscountedEvents,
    /// One Apply native's successor count off by one, saved counter adjusted.
    MiscountedSuccessors,
    /// Point a partial record (link and edge) at an initial native that does
    /// not contain its D >= cut slice.
    RetargetedAnchor,
    /// Resolve a query to an initial record that does not contain it.
    RemappedQuery,
    /// The checkpoint's request digest belongs to another request.
    ForeignRequest,
    /// One saved owner payload digest differs.
    ForeignOwners,
    /// One committed record differs between the checkpoint and result.json.
    MismatchedResult,
    /// Positive control: a load-bearing edge P -> T is replaced by P -> A,
    /// where A is an alias of T that does not itself contain the successor;
    /// coverage must then go through the alias chain and the verdict PASS.
    AliasChainDetour,
    /// A partial record anchored on itself (link and anchor edge): its
    /// D >= cut slice is discharged by nobody, yet the self-edge seals.
    SelfAnchoredPartial,
    /// A partial record anchored on another (earlier) partial record, whose
    /// own D >= cut slice is not natively inspected either.
    PartialAsAnchor,
    /// Two partial records anchored on each other (a two-cycle).
    PartialAnchorCycle,
    /// A partial record anchored on a non-initial Native, a later one when
    /// it exists (the ID analogue of a G2' anchor stamped at or after the
    /// dispatch epoch), else an earlier non-initial one.
    NonInitialAnchor,
    /// A partial record whose saved domain is a Route domain (same owner
    /// and bounds): residual inspection exists only in Apply.
    RoutePartial,
    /// A partial record's recorded residual shrunk by one D layer (the
    /// D = cut - 1 layer is then inspected by nobody): the exact union cover
    /// `Q <= anchor u residual` fails.
    ShrunkResidual,
    /// Drop the only recorded edge covering an Apply domain routed by a
    /// Route native.
    DroppedRoutedEdge,
    /// Replace the only edge covering a Route native's routed Apply domain
    /// by one to a same-owner native non-container.
    RoutedFalseHit,
    /// One Route native's event count off by one, saved counter adjusted.
    MiscountedRouteEvents,
    /// A G2' record's recorded residual D band shrunk by one D layer (its
    /// highest level, which holds a point no anchor covers).
    G2ShrunkResidual,
    /// A G2' anchor replaced (record and edge) by a record merged at or after
    /// the snapshot, with that record's true stamp.
    G2LateAnchor,
    /// A G2' anchor replaced (record and edge) by an earlier record that is
    /// neither a Native inspection nor a validated G2' record (an alias, a
    /// G2' full cover or a Route record).
    G2InadmissibleAnchor,
    /// The edge to one G2' anchor dropped.
    G2DroppedAnchorEdge,
    /// Two G2' records made anchors of each other (records and edges).
    G2AnchorCycle,
}
impl OwnerDomainWalkVerifyMutation {
    pub const ALL: [Self; 30] = [
        Self::DroppedEdge,
        Self::RetargetedAlias,
        Self::DroppedFrontierRecord,
        Self::SealWithFrontier,
        Self::SealWithError,
        Self::InjectedFalseHit,
        Self::HiddenFrontier,
        Self::HiddenError,
        Self::MiscountedEvents,
        Self::MiscountedSuccessors,
        Self::RetargetedAnchor,
        Self::RemappedQuery,
        Self::ForeignRequest,
        Self::ForeignOwners,
        Self::MismatchedResult,
        Self::AliasChainDetour,
        Self::SelfAnchoredPartial,
        Self::PartialAsAnchor,
        Self::PartialAnchorCycle,
        Self::NonInitialAnchor,
        Self::RoutePartial,
        Self::ShrunkResidual,
        Self::DroppedRoutedEdge,
        Self::RoutedFalseHit,
        Self::MiscountedRouteEvents,
        Self::G2ShrunkResidual,
        Self::G2LateAnchor,
        Self::G2InadmissibleAnchor,
        Self::G2DroppedAnchorEdge,
        Self::G2AnchorCycle,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::DroppedEdge => "dropped-edge",
            Self::RetargetedAlias => "retargeted-alias",
            Self::DroppedFrontierRecord => "dropped-frontier-record",
            Self::SealWithFrontier => "seal-with-frontier",
            Self::SealWithError => "seal-with-error",
            Self::InjectedFalseHit => "injected-false-hit",
            Self::HiddenFrontier => "hidden-frontier",
            Self::HiddenError => "hidden-error",
            Self::MiscountedEvents => "miscounted-events",
            Self::MiscountedSuccessors => "miscounted-successors",
            Self::RetargetedAnchor => "retargeted-anchor",
            Self::RemappedQuery => "remapped-query",
            Self::ForeignRequest => "foreign-request",
            Self::ForeignOwners => "foreign-owners",
            Self::MismatchedResult => "mismatched-result",
            Self::AliasChainDetour => "alias-chain-detour",
            Self::SelfAnchoredPartial => "self-anchored-partial",
            Self::PartialAsAnchor => "partial-as-anchor",
            Self::PartialAnchorCycle => "partial-anchor-cycle",
            Self::NonInitialAnchor => "non-initial-anchor",
            Self::RoutePartial => "route-partial",
            Self::ShrunkResidual => "shrunk-residual",
            Self::DroppedRoutedEdge => "dropped-routed-edge",
            Self::RoutedFalseHit => "routed-false-hit",
            Self::MiscountedRouteEvents => "miscounted-route-events",
            Self::G2ShrunkResidual => "g2-shrunk-residual",
            Self::G2LateAnchor => "g2-late-anchor",
            Self::G2InadmissibleAnchor => "g2-inadmissible-anchor",
            Self::G2DroppedAnchorEdge => "g2-dropped-anchor-edge",
            Self::G2AnchorCycle => "g2-anchor-cycle",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }
    /// The verdict an applied mutation must produce.
    pub fn expected_verdict(self) -> &'static str {
        if self == Self::AliasChainDetour {
            "PASS"
        } else {
            "FAIL"
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkVerifyOptions {
    pub checkpoint: PathBuf,
    pub threads: usize,
    pub reinspect: OwnerDomainWalkVerifyReinspect,
    /// Cells with at most this many lattice points are also checked by
    /// enumeration (0 disables).
    pub brute_force_max_points: u64,
    pub brute_force_point_budget: u64,
    pub require_closure: bool,
    pub mutation: Option<OwnerDomainWalkVerifyMutation>,
    pub max_violations: usize,
    /// A published result.json to bind to the checkpoint generation.
    pub result: Option<PathBuf>,
    pub reference_levers: OwnerDomainWalkVerifyReferenceLevers,
    /// Records sampled for the real-data multi-target union-cover
    /// validation (`union_sample`; 0 disables), and its seed.
    pub union_sample: usize,
    pub union_sample_seed: u64,
    /// Which roots a PASS requires (see `OwnerDomainWalkVerifyScope`).
    pub certification_scope: OwnerDomainWalkVerifyScope,
}

/// Roots a `--require-closure` PASS certifies. `Auto` is `AllRoots` for an
/// unamended walk and `PhysicsQueries` for a walk with rescue amendments:
/// every required query (immutable exact-ID declaration) through its first
/// closed containing input root; helper roots are reported, not required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyScope {
    Auto,
    AllRoots,
    PhysicsQueries,
}
impl OwnerDomainWalkVerifyOptions {
    pub fn new(checkpoint: impl Into<PathBuf>) -> Self {
        Self {
            checkpoint: checkpoint.into(),
            threads: 1,
            reinspect: OwnerDomainWalkVerifyReinspect::All,
            brute_force_max_points: 4096,
            brute_force_point_budget: 1 << 32,
            require_closure: false,
            mutation: None,
            max_violations: 200,
            result: None,
            reference_levers: OwnerDomainWalkVerifyReferenceLevers::Off,
            union_sample: 0,
            union_sample_seed: 1,
            certification_scope: OwnerDomainWalkVerifyScope::Auto,
        }
    }
}

#[derive(Default)]
struct Violations {
    list: Vec<String>,
    suppressed: u64,
    by_class: BTreeMap<&'static str, u64>,
    limit: usize,
}
impl Violations {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }
    fn add(&mut self, class: &'static str, message: impl FnOnce() -> String) {
        *self.by_class.entry(class).or_default() += 1;
        if self.list.len() < self.limit {
            self.list.push(format!("{class}: {}", message()));
        } else {
            self.suppressed += 1;
        }
    }
    fn merge(&mut self, other: Violations) {
        for (class, count) in other.by_class {
            *self.by_class.entry(class).or_default() += count;
        }
        for message in other.list {
            if self.list.len() < self.limit {
                self.list.push(message);
            } else {
                self.suppressed += 1;
            }
        }
        self.suppressed += other.suppressed;
    }
    fn is_empty(&self) -> bool {
        self.by_class.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Missing,
    Native,
    Partial,
    Alias,
    /// G2' residual-anchor record (its residual, if any, inspected natively).
    G2,
}

#[derive(Clone, Copy)]
struct Node {
    kind: Kind,
    error: bool,
    frontiers: u32,
    finished: bool,
    events: Option<u64>,
    successors: Option<u64>,
    accepted: Option<u64>,
    /// Alias representative or partial anchor.
    link: usize,
    cut: i64,
    /// Rescue-abandoned (published without inspection; never re-inspected).
    abandoned: bool,
}
impl Node {
    const MISSING: Self = Self {
        kind: Kind::Missing,
        error: false,
        frontiers: 0,
        finished: false,
        events: None,
        successors: None,
        accepted: None,
        link: usize::MAX,
        cut: 0,
        abandoned: false,
    };
    fn native(&self) -> bool {
        matches!(self.kind, Kind::Native | Kind::Partial | Kind::G2)
    }
}

#[derive(Deserialize)]
struct PowersRow {
    max_positive_power: Option<u64>,
    min_power_difference: Option<i64>,
    max_power_difference: Option<i64>,
}
impl PowersRow {
    fn bounds(&self) -> DomainPowerBounds {
        DomainPowerBounds {
            max_positive_power: self.max_positive_power,
            min_power_difference: self.min_power_difference,
            max_power_difference: self.max_power_difference,
        }
    }
}
#[derive(Deserialize)]
struct OverlapRow {
    anchor_id: usize,
    cut: i64,
    residual_power_bounds: PowersRow,
}
#[derive(Clone, Debug, Deserialize)]
struct G2AnchorRow {
    id: usize,
    stamp: u64,
    kind: String,
}
#[derive(Deserialize)]
struct G2RecordRow {
    merge_stamp: Option<u64>,
    snapshot_stamp: u64,
    residual_power_bounds: Option<PowersRow>,
    anchors: Vec<G2AnchorRow>,
}
/// A G2' record as published (checked in `check_g2`, after any mutation).
#[derive(Clone, Debug)]
struct G2Info {
    merge_stamp: Option<u64>,
    snapshot: u64,
    residual: Option<DomainPowerBounds>,
    anchors: Vec<G2AnchorRow>,
}
#[derive(Deserialize)]
struct StatsRow {
    events: Option<u64>,
    successors: Option<u64>,
}
#[derive(Deserialize)]
struct RecordRow {
    id: usize,
    /// Absent only on a plain native record of a walk without a delegation
    /// ledger (`execution.rs` writes it with the ledger; partial and
    /// subdivided records always carry it).
    #[serde(default = "native_inspection_kind")]
    record_kind: String,
    #[serde(default)]
    finite_replay_recipe: Option<super::finite_replay::Recipe>,
    phase: String,
    owner: String,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    power_bounds: PowersRow,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    frontiers: Option<Vec<Value>>,
    #[serde(default)]
    local_inspection_finished: Option<bool>,
    #[serde(default)]
    residual_inspection_finished: Option<bool>,
    #[serde(default)]
    stats: Option<StatsRow>,
    #[serde(default)]
    accepted_events: Option<u64>,
    #[serde(default)]
    representative_id: Option<usize>,
    #[serde(default)]
    initial_overlap: Option<OverlapRow>,
    #[serde(default)]
    g2_residual_anchors: Option<G2RecordRow>,
    #[serde(default)]
    g2: Option<Value>,
    #[serde(default)]
    epoch: Option<Value>,
    /// Rescue: an obligation published without inspection (`rescue.rs`).
    #[serde(default)]
    rescue_abandoned: Option<bool>,
}

fn native_inspection_kind() -> String {
    "native_inspection".into()
}

fn cell<const N: usize>(domain: &Domain<N>) -> Cell {
    Cell {
        owner: domain.owner.to_vec(),
        lower: domain.lower.clone(),
        upper: domain.upper.clone(),
        rank: domain.rank,
        powers: domain.powers,
    }
}

/// The cell of a saved (compact, exact) domain image.
fn ccell<const N: usize>(domain: &CompactDomain<N>) -> Cell {
    Cell {
        owner: domain.owner().to_vec(),
        lower: (0..N).map(|axis| domain.lower(axis)).collect(),
        upper: (0..N).map(|axis| domain.upper(axis)).collect(),
        rank: domain.rank(),
        powers: domain.powers(),
    }
}

/// Exact inclusion plus a budgeted brute-force enumeration cross-check.
/// Positive and negative outcomes are counted separately: most calls in the
/// coverage scan are non-inclusions settled after one lattice point.
struct Containment {
    max_points: u64,
    budget: u64,
    points: AtomicU64,
    exact: AtomicU64,
    exact_positive: AtomicU64,
    brute: AtomicU64,
    brute_positive: AtomicU64,
    disagreements: AtomicU64,
    /// Partial records with a well-founded anchor: exact union cover of the
    /// whole domain by {anchor, recorded residual} (`Cell::covered_by_union`).
    /// While the recorded residual equals the D < cut slice the two targets
    /// partition Q along D, so this is logically the single-target D >= cut
    /// slice inclusion (compared, a disagreement is a violation): a check of
    /// two implementations on real cells, not a multi-target validation
    /// (that is `union_sample`).
    union_checks: AtomicU64,
    union_covered: AtomicU64,
    union_not_covered: AtomicU64,
    union_undecided: AtomicU64,
    union_compared: AtomicU64,
    union_disagreements: AtomicU64,
    /// G2' records: exact multi-target cover Q <= residual u anchor scopes,
    /// cross-checked by enumeration on small Q.
    g2_checks: AtomicU64,
    g2_covered: AtomicU64,
    g2_not_covered: AtomicU64,
    g2_undecided: AtomicU64,
    g2_brute: AtomicU64,
    g2_disagreements: AtomicU64,
}
impl Containment {
    fn new(max_points: u64, budget: u64) -> Self {
        Self {
            max_points,
            budget,
            points: AtomicU64::new(0),
            exact: AtomicU64::new(0),
            exact_positive: AtomicU64::new(0),
            brute: AtomicU64::new(0),
            brute_positive: AtomicU64::new(0),
            disagreements: AtomicU64::new(0),
            union_checks: AtomicU64::new(0),
            union_covered: AtomicU64::new(0),
            union_not_covered: AtomicU64::new(0),
            union_undecided: AtomicU64::new(0),
            union_compared: AtomicU64::new(0),
            union_disagreements: AtomicU64::new(0),
            g2_checks: AtomicU64::new(0),
            g2_covered: AtomicU64::new(0),
            g2_not_covered: AtomicU64::new(0),
            g2_undecided: AtomicU64::new(0),
            g2_brute: AtomicU64::new(0),
            g2_disagreements: AtomicU64::new(0),
        }
    }
    /// G2': exact `whole <= t_1 u ... u t_k` (None: undecided), and whether
    /// lattice enumeration (small Q only) disagrees with it.
    fn g2_union(&self, whole: &Cell, targets: &[&Cell]) -> (Option<bool>, bool) {
        self.g2_checks.fetch_add(1, Ordering::Relaxed);
        let covered = whole.covered_by_union(targets, 1 << 18);
        let counter = match covered {
            None => &self.g2_undecided,
            Some(true) => &self.g2_covered,
            Some(false) => &self.g2_not_covered,
        };
        counter.fetch_add(1, Ordering::Relaxed);
        let mut disagrees = false;
        if self.max_points > 0
            && let Some(brute) = whole.brute_force_covered_by_union(targets, self.max_points)
        {
            self.g2_brute.fetch_add(1, Ordering::Relaxed);
            disagrees = covered.is_some_and(|c| c != brute);
            self.g2_disagreements
                .fetch_add(u64::from(disagrees), Ordering::Relaxed);
        }
        (covered, disagrees)
    }
    /// `whole <= t_1 u ... u t_k`, exact (None: region budget exhausted),
    /// and whether it disagrees with `slice_included` (the anchor contains
    /// the D >= cut slice), compared only when given (recorded residual equal
    /// to the D < cut slice).
    fn partial_union(
        &self,
        whole: &Cell,
        targets: &[&Cell],
        slice_included: Option<bool>,
    ) -> (Option<bool>, bool) {
        self.union_checks.fetch_add(1, Ordering::Relaxed);
        let covered = whole.covered_by_union(targets, 1 << 16);
        let counter = match covered {
            None => &self.union_undecided,
            Some(true) => &self.union_covered,
            Some(false) => &self.union_not_covered,
        };
        counter.fetch_add(1, Ordering::Relaxed);
        let disagrees = match (covered, slice_included) {
            (Some(covered), Some(included)) => {
                self.union_compared.fetch_add(1, Ordering::Relaxed);
                covered != included
            }
            _ => false,
        };
        self.union_disagreements
            .fetch_add(u64::from(disagrees), Ordering::Relaxed);
        (covered, disagrees)
    }
    fn contains(&self, outer: &Cell, inner: &Cell) -> bool {
        self.exact.fetch_add(1, Ordering::Relaxed);
        let exact = outer.contains(inner);
        self.exact_positive
            .fetch_add(u64::from(exact), Ordering::Relaxed);
        if self.max_points > 0
            && self.points.load(Ordering::Relaxed) < self.budget
            && let Some((brute, visited)) = outer.brute_force_contains(inner, self.max_points)
        {
            self.brute.fetch_add(1, Ordering::Relaxed);
            self.brute_positive
                .fetch_add(u64::from(brute), Ordering::Relaxed);
            self.points.fetch_add(visited, Ordering::Relaxed);
            if brute != exact {
                self.disagreements.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        exact
    }
    fn disagreements(&self) -> u64 {
        self.disagreements.load(Ordering::Relaxed)
    }
    fn json(&self) -> Value {
        json!({"exact_checks":self.exact.load(Ordering::Relaxed),
            "exact_inclusions":self.exact_positive.load(Ordering::Relaxed),
            "brute_force_checks":self.brute.load(Ordering::Relaxed),
            "brute_force_confirmed_inclusions":self.brute_positive.load(Ordering::Relaxed),
            "brute_force_confirmed_non_inclusions":
                self.brute.load(Ordering::Relaxed) - self.brute_positive.load(Ordering::Relaxed),
            "brute_force_points":self.points.load(Ordering::Relaxed),
            "brute_force_max_points":self.max_points,"brute_force_point_budget":self.budget,
            "exact_vs_brute_force_disagreements":self.disagreements(),
            "partial_union_cover":{
                "checks":self.union_checks.load(Ordering::Relaxed),
                "covered":self.union_covered.load(Ordering::Relaxed),
                "not_covered":self.union_not_covered.load(Ordering::Relaxed),
                "undecided":self.union_undecided.load(Ordering::Relaxed),
                "compared_with_slice_inclusion":self.union_compared.load(Ordering::Relaxed),
                "disagreements_with_slice_inclusion":self.union_disagreements.load(Ordering::Relaxed),
                "predicate":"exact Q <= anchor u recorded residual (lattice.rs covered_by_union), on partial records with a well-founded anchor",
                "scope":"D-cut identity check, not a multi-target validation: with the recorded residual equal to the D < cut slice (partial_residual), {anchor, residual} partitions Q along D and the union cover is logically the single-target D >= cut slice inclusion; genuine multi-target covers are validated by union_sample"},
            "g2_union_cover":{
                "checks":self.g2_checks.load(Ordering::Relaxed),
                "covered":self.g2_covered.load(Ordering::Relaxed),
                "not_covered":self.g2_not_covered.load(Ordering::Relaxed),
                "undecided":self.g2_undecided.load(Ordering::Relaxed),
                "brute_force_cross_checks":self.g2_brute.load(Ordering::Relaxed),
                "brute_force_disagreements":self.g2_disagreements.load(Ordering::Relaxed),
                "predicate":"exact Q <= recorded residual u anchor scopes (lattice.rs covered_by_union, region budget 2^18), on G2' records with well-founded admissible anchors; cross-checked by lattice enumeration when Q has at most brute_force_max_points points"}})
    }
}

struct Loaded<const N: usize> {
    raw: checkpoint::RawCheckpoint<N>,
    /// The saved domain images in their exact 96-byte (N = 15) checkpoint
    /// encoding; expanded on use (a `Domain<15>` costs about 500 bytes).
    domains: Vec<CompactDomain<N>>,
    nodes: Vec<Node>,
    /// Per-record digests of the committed record images (result binding).
    digests: Vec<Digest>,
    /// Partial records: the residual power bounds the record says were
    /// inspected natively (checked against the D < cut slice and used in the
    /// exact union cover `Q <= anchor u residual`).
    residuals: BTreeMap<usize, DomainPowerBounds>,
    /// G2' records as published.
    g2: BTreeMap<usize, G2Info>,
    /// Position of each record in the publication stream (its merge stamp,
    /// re-derived from the record order; u64::MAX: unpublished).
    positions: Vec<u64>,
    /// An epoch (walk semantics 3) S2 export: its raw ledger6, edge runs and
    /// anchors for the epoch-specific re-derivations.
    epoch: Option<epoch_export::EpochSections>,
    finite_replay: Option<super::finite_replay::Recipe>,
}

/// Verify one saved walk generation; returns the report (verdict inside).
pub fn owner_domain_walk_verify_closure(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    verify_closure_with_progress(request, options, cancellation, &observer, None)
}

// One callback type reaches the sixteen native arities, independently of the
// public caller's captures. No callback is moved to a worker or required to be
// thread-safe; the existing coordinator invocation sites remain unchanged.
fn verify_closure_with_progress(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    cancellation: &AtomicBool,
    observer: &dyn Fn(Value),
    inventory: Option<&inventory::Collector>,
) -> Result<Value, AppError> {
    request
        .validate_epoch_inspector_lookup()
        .map_err(AppError::input)?;
    if request.apply_subdivision.is_some() {
        // Physical parts inspect under part-local limits; the reference
        // inspection here would not reproduce them. Refuse, never guess.
        return Err(AppError::input(
            "walk-verify-closure does not support walks with physical Apply subdivision",
        ));
    }
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity ){
        $($n => verify::<$n>(request, options, &selection, limits, cancellation, &observer, inventory),)*
        _ => Err(AppError::input("unsupported owner arity")),
    }} }
    {
        crate::ensure_runtime_arity(arity)?;
        rustred::with_app_runtime_arities!(dispatch)
    }
}

fn load<const N: usize>(
    options: &OwnerDomainWalkVerifyOptions,
    digests: bool,
    violations: &mut Violations,
) -> Result<Loaded<N>, String> {
    if options.checkpoint.join(epoch_export::MANIFEST).exists()
        && options.checkpoint.join("latest.json").exists()
    {
        return Err("ambiguous epoch export and resumable checkpoint authorities".into());
    }
    let (raw, epoch, cp6_records) = if epoch_checkpoint::present(&options.checkpoint) {
        let (raw, sections, records) = epoch_checkpoint::read_raw::<N>(&options.checkpoint)?;
        (raw, Some(sections), Some(records))
    } else if options.checkpoint.join(epoch_export::MANIFEST).is_file() {
        let (raw, sections) = epoch_export::read_raw::<N>(&options.checkpoint)?;
        (raw, Some(sections), None)
    } else {
        (checkpoint::read_raw::<N>(&options.checkpoint)?, None, None)
    };
    load_records(raw, epoch, cp6_records, digests, violations)
}

/// Read-only rescue-planner view of one selected immutable generation. Record
/// streams keep the captured CP6 references, so a later latest.json cannot
/// redirect or silently replace the records classified by the planner.
pub(super) struct RescueCheckpoint<const N: usize> {
    pub raw: checkpoint::RawCheckpoint<N>,
    records: Option<Vec<epoch_checkpoint::RecordRef>>,
}

impl<const N: usize> RescueCheckpoint<N> {
    pub fn record_reader(&self, index: usize) -> std::io::Result<Box<dyn std::io::Read>> {
        let (path, count) = &self.raw.records[index];
        if let Some(records) = &self.records {
            Ok(Box::new(super::epoch::record_store::JsonLines::new(
                records[index].open(path, *count)?,
                *count,
            )))
        } else {
            Ok(Box::new(std::fs::File::open(path)?))
        }
    }
}

pub(super) fn rescue_checkpoint<const N: usize>(
    directory: &std::path::Path,
) -> Result<RescueCheckpoint<N>, String> {
    if epoch_checkpoint::present(directory) {
        let (raw, _, records) = epoch_checkpoint::read_raw(directory)?;
        Ok(RescueCheckpoint {
            raw,
            records: Some(records),
        })
    } else {
        Ok(RescueCheckpoint {
            raw: checkpoint::read_raw(directory)?,
            records: None,
        })
    }
}

fn load_records<const N: usize>(
    mut raw: checkpoint::RawCheckpoint<N>,
    epoch: Option<epoch_export::EpochSections>,
    cp6_records: Option<Vec<epoch_checkpoint::RecordRef>>,
    digests: bool,
    violations: &mut Violations,
) -> Result<Loaded<N>, String> {
    if cp6_records
        .as_ref()
        .is_some_and(|r| r.len() != raw.records.len())
    {
        return Err("CP6 captured record inventory differs".into());
    }
    let domains = std::mem::take(&mut raw.domains);
    let total = domains.len();
    if raw.flags.len() != total {
        violations.add("structure", || {
            format!("{} node flags for {total} domains", raw.flags.len())
        });
    }
    let mut nodes = vec![Node::MISSING; total];
    let mut record_digests = if digests {
        vec![[0u8; 16]; total]
    } else {
        Vec::new()
    };
    let mut residuals = BTreeMap::new();
    let mut g2 = BTreeMap::new();
    let mut positions = vec![u64::MAX; total];
    let mut records = 0usize;
    let mut finite_replay = None;
    let epoch_records = epoch.as_ref().map(epoch_g2::View::new).transpose()?;
    for (segment, (path, count)) in raw.records.iter().enumerate() {
        let file: Box<dyn std::io::Read> = if let Some(references) = &cp6_records {
            Box::new(
                references
                    .get(segment)
                    .ok_or("CP6 missing captured record reference")?
                    .open(path, *count)
                    .map_err(|e| format!("{}: {e}", path.display()))?,
            )
        } else {
            Box::new(std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?)
        };
        let file: Box<dyn std::io::Read> = if epoch.is_some() {
            Box::new(super::epoch::record_store::JsonLines::new(file, *count))
        } else {
            file
        };
        let mut lines = 0usize;
        for line in BufReader::with_capacity(1 << 20, file).lines() {
            let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
            if line.is_empty() {
                continue;
            }
            lines += 1;
            let parse_error =
                |e: serde_json::Error| format!("{}: record line {lines}: {e}", path.display());
            let mut row: RecordRow = if digests {
                let mut value: Value = serde_json::from_str(&line).map_err(parse_error)?;
                let digest = result_binding::record_digest(&mut value);
                let row: RecordRow = serde_json::from_value(value).map_err(parse_error)?;
                if let Some(slot) = record_digests.get_mut(row.id) {
                    *slot = digest;
                }
                row
            } else {
                serde_json::from_str(&line).map_err(parse_error)?
            };
            if let Some(slot) = positions.get_mut(row.id)
                && *slot == u64::MAX
            {
                *slot = epoch_records
                    .as_ref()
                    .map_or((records + lines - 1) as u64, |view| view.position(row.id));
            }
            if let Some(view) = &epoch_records {
                view.normalize(&mut row, &domains)?;
            }
            if let Some(recipe) = row.finite_replay_recipe {
                if row.id != 0
                    || row.record_kind != "finite_replay_summary"
                    || epoch.is_none()
                    || finite_replay.replace(recipe).is_some()
                {
                    violations.add("finite_replay", || {
                        "summary recipe outside unique CP6 initial ID0".into()
                    });
                }
            }
            record_node(
                &row,
                &domains,
                &mut nodes,
                &mut residuals,
                &mut g2,
                violations,
            );
        }
        if lines != *count {
            violations.add("structure", || {
                format!("{}: {lines} records, manifest says {count}", path.display())
            });
        }
        records += lines;
    }
    let published = nodes.iter().filter(|n| n.kind != Kind::Missing).count();
    if published != records {
        violations.add("structure", || {
            format!("{records} record lines but {published} distinct record ids")
        });
    }
    Ok(Loaded {
        raw,
        domains,
        nodes,
        digests: record_digests,
        residuals,
        g2,
        positions,
        epoch,
        finite_replay,
    })
}

fn record_node<const N: usize>(
    row: &RecordRow,
    domains: &[CompactDomain<N>],
    nodes: &mut [Node],
    residuals: &mut BTreeMap<usize, DomainPowerBounds>,
    g2: &mut BTreeMap<usize, G2Info>,
    violations: &mut Violations,
) {
    let id = row.id;
    let Some(domain) = domains.get(id).map(CompactDomain::expand) else {
        violations.add("structure", || {
            format!("record {id} beyond the saved domains")
        });
        return;
    };
    if nodes[id].kind != Kind::Missing {
        violations.add("structure", || format!("record {id} published twice"));
        return;
    }
    let phase = match row.phase.as_str() {
        "Apply" => Some(Phase::Apply),
        "Route" => Some(Phase::Route),
        _ => None,
    };
    let owner = row
        .owner
        .chars()
        .map(|c| match c {
            '0' => Some(false),
            '1' => Some(true),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let owner = owner.and_then(|owner| super::super::storage::restore_array::<_, N>(&owner, false));
    let lower = super::super::storage::restore_array::<_, N>(&row.lower, 0);
    let upper = super::super::storage::restore_array::<_, N>(&row.upper, Some(0));
    if phase != Some(domain.phase)
        || owner != Some(domain.owner)
        || lower.as_ref().map(|v| v.as_slice()) != Some(domain.lower.as_slice())
        || upper.as_ref().map(|v| v.as_slice()) != Some(domain.upper.as_slice())
        || row.rank != domain.rank
        || row.power_bounds.bounds() != domain.powers
    {
        violations.add("domain_parity", || {
            format!("record {id} image differs from the saved domain")
        });
    }
    let stats = row.stats.as_ref();
    let mut node = Node {
        kind: Kind::Native,
        error: row.error.as_ref().is_some_and(|e| !e.is_null()),
        frontiers: row.frontiers.as_ref().map_or(0, |f| f.len() as u32),
        finished: row.local_inspection_finished == Some(true),
        events: stats.and_then(|s| s.events),
        successors: stats.and_then(|s| s.successors),
        accepted: row.accepted_events,
        link: usize::MAX,
        cut: 0,
        abandoned: row.rescue_abandoned == Some(true),
    };
    if node.abandoned {
        // Exactly one bookkeeping frontier, one accepted event, nothing else.
        let marker = row.frontiers.as_deref().is_some_and(|f| {
            f.len() == 1 && f[0]["kind"] == super::inspection::RESCUE_ABANDONED_KIND
        });
        if row.record_kind != "native_inspection"
            || !marker
            || node.error
            || node.events != Some(1)
            || node.successors.unwrap_or(0) != 0
            || node.finished
        {
            violations.add("rescue_abandoned", || {
                format!("record {id} is not a well-formed rescue-abandoned record")
            });
        }
    }
    match row.record_kind.as_str() {
        "native_inspection" => {}
        "finite_replay_summary" => {
            if row.id != 0
                || row.finite_replay_recipe.is_none()
                || row.initial_overlap.is_some()
                || row.g2.is_some()
                || row.g2_residual_anchors.is_some()
                || node.error
                || node.frontiers != 0
                || !node.finished
                || node.abandoned
                || node.events != Some(1)
                || node.accepted != Some(1)
                || node.successors.unwrap_or(0) != 0
            {
                violations.add("finite_replay", || {
                    format!("malformed finite replay summary {id}")
                });
            }
        }
        "partial_initial_overlap_inspection" => {
            node.kind = Kind::Partial;
            node.finished = row.residual_inspection_finished == Some(true);
            match &row.initial_overlap {
                Some(overlap) => {
                    node.link = overlap.anchor_id;
                    node.cut = overlap.cut;
                    // Checked in `check_partial` (after any mutation).
                    residuals.insert(id, overlap.residual_power_bounds.bounds());
                }
                None => violations.add("partial_anchor", || {
                    format!("record {id} has no overlap link")
                }),
            }
        }
        "g2_residual_anchor_inspection" => {
            node.kind = Kind::G2;
            node.finished = row.residual_inspection_finished == Some(true);
            match &row.g2_residual_anchors {
                Some(block) => {
                    g2.insert(
                        id,
                        G2Info {
                            merge_stamp: block.merge_stamp,
                            snapshot: block.snapshot_stamp,
                            residual: block.residual_power_bounds.as_ref().map(PowersRow::bounds),
                            anchors: block.anchors.clone(),
                        },
                    );
                }
                None => violations.add("g2_anchor_kind", || {
                    format!("G2' record {id} has no anchor block")
                }),
            }
        }
        "delegated_not_inspected" => {
            node.kind = Kind::Alias;
            node.finished = false;
            match row.representative_id {
                Some(to) if to > id => node.link = to,
                _ => violations.add("alias_containment", || {
                    format!("alias {id} has no later representative")
                }),
            }
        }
        other => {
            violations.add("structure", || {
                format!("record {id} has unknown kind {other:?}")
            });
            return;
        }
    }
    if node.native()
        && let (Some(accepted), Some(events)) = (node.accepted, node.events)
        && accepted != events
    {
        violations.add("accepted_events", || {
            format!("record {id} accepted {accepted} events, stream emitted {events}")
        });
    }
    nodes[id] = node;
}

/// The D < cut residual power bounds of a partial record.
fn residual(mut powers: DomainPowerBounds, cut: i64) -> DomainPowerBounds {
    powers.max_power_difference = Some(
        powers
            .max_power_difference
            .map_or(cut - 1, |d| d.min(cut - 1)),
    );
    powers
}

/// The D < cut slice of a cell (a partial record's residual obligation).
fn low_slice(mut cell: Cell, cut: i64) -> Cell {
    let high = cut - 1;
    cell.powers.max_power_difference = Some(
        cell.powers
            .max_power_difference
            .map_or(high, |d| d.min(high)),
    );
    cell
}

/// The D >= cut slice of a partial record, discharged by its anchor.
fn high_slice(mut cell: Cell, cut: i64) -> Cell {
    cell.powers.min_power_difference =
        Some(cell.powers.min_power_difference.map_or(cut, |d| d.max(cut)));
    cell
}

/// F8 on records alone: natives seal with 0 frontiers, no error and a
/// finished (residual) inspection plus their anchor edge; aliases seal with
/// their representative edge.
#[cfg(test)]
fn sealed(nodes: &[Node], graph: &Graph) -> Vec<bool> {
    sealed_with(nodes, &BTreeMap::new(), graph)
}

/// `sealed` with G2' records: sealed like a partial, with every anchor edge.
fn sealed_with(nodes: &[Node], g2: &BTreeMap<usize, G2Info>, graph: &Graph) -> Vec<bool> {
    nodes
        .iter()
        .enumerate()
        .map(|(id, node)| match node.kind {
            Kind::G2 => {
                !node.error
                    && node.frontiers == 0
                    && node.finished
                    && g2.get(&id).is_some_and(|info| {
                        info.anchors
                            .iter()
                            .all(|a| a.id < nodes.len() && graph.has_edge(id, a.id))
                    })
            }
            Kind::Native => !node.error && node.frontiers == 0 && node.finished,
            Kind::Partial => {
                !node.error
                    && node.frontiers == 0
                    && node.finished
                    && node.link < nodes.len()
                    && graph.has_edge(id, node.link)
            }
            Kind::Alias => node.link < nodes.len() && graph.has_edge(id, node.link),
            Kind::Missing => false,
        })
        .collect()
}

/// A partial record's obligations (the D >= cut slice is discharged by its
/// anchor, the D < cut residual by its own native inspection). Classes:
/// - `partial_phase`: residual inspection exists only in Apply (a Route
///   record's cell would be matched by the phaseless `Cell::contains`);
/// - `missing_edge`: no dependency edge to the anchor;
/// - `partial_anchor_order` / `partial_anchor_kind`: the anchor must be an
///   EARLIER INITIAL record (`link < id`, `link < initial_count`) that is
///   itself a Native inspection. Together they make the anchor relation
///   well-founded: no self-anchor, no partial-on-partial chain, no anchor
///   cycle. Closure is coinductive (sealed cycles count as closed) and F10
///   re-inspects only the residual, so without them a D >= cut slice could
///   be discharged by nobody;
/// - only against a well-founded anchor (otherwise meaningless):
///   `partial_anchor` (the anchor is an Apply record containing the
///   D >= cut slice) and `partial_union_cover` (exact `Q <= anchor u recorded
///   residual`, compared with the slice inclusion while the residual is the
///   D < cut slice);
/// - `partial_residual`: the recorded residual is the D < cut slice.
#[allow(clippy::too_many_arguments)]
fn check_partial<const N: usize>(
    id: usize,
    nodes: &[Node],
    domains: &[CompactDomain<N>],
    residuals: &BTreeMap<usize, DomainPowerBounds>,
    graph: &Graph,
    containment: &Containment,
    initial_count: usize,
    violations: &mut Violations,
) {
    let node = nodes[id];
    let (link, cut) = (node.link, node.cut);
    let phase = domains[id].phase();
    if phase != Phase::Apply {
        violations.add("partial_phase", || {
            format!("partial {id} is a {phase:?} record: residual inspection exists only in Apply")
        });
    }
    let whole = ccell(&domains[id]);
    let derived = residual(whole.powers, cut);
    let recorded = residuals.get(&id).copied();
    let identity = recorded == Some(derived);
    if !identity {
        violations.add("partial_residual", || {
            format!(
                "partial {id}: recorded residual bounds {recorded:?} are not the D < {cut} slice {derived:?}"
            )
        });
    }
    if link >= nodes.len() || !graph.has_edge(id, link) {
        violations.add("missing_edge", || {
            format!("partial {id} has no edge to anchor {link}")
        });
        return;
    }
    let ordered = link < id && link < initial_count;
    if !ordered {
        violations.add("partial_anchor_order", || {
            format!(
                "partial {id}: anchor {link} is not an earlier initial record ({initial_count} initial records)"
            )
        });
    }
    let anchor_kind = nodes[link].kind;
    if anchor_kind != Kind::Native {
        violations.add("partial_anchor_kind", || {
            format!(
                "partial {id}: anchor {link} is a {anchor_kind:?} record, not a Native inspection"
            )
        });
    }
    if !(ordered && anchor_kind == Kind::Native) {
        return;
    }
    let anchor = &domains[link];
    let anchor_apply = anchor.phase() == Phase::Apply;
    let anchor_cell = ccell(anchor);
    let slice_included =
        anchor_apply && containment.contains(&anchor_cell, &high_slice(whole.clone(), cut));
    if !slice_included {
        violations.add("partial_anchor", || {
            format!("partial {id}: D >= {cut} slice not covered by initial native anchor {link}")
        });
    }
    let residual_cell = Cell {
        powers: recorded.unwrap_or(derived),
        ..whole.clone()
    };
    let targets: Vec<&Cell> = if anchor_apply {
        vec![&anchor_cell, &residual_cell]
    } else {
        vec![&residual_cell]
    };
    let (covered, disagrees) =
        containment.partial_union(&whole, &targets, identity.then_some(slice_included));
    if covered == Some(false) || disagrees {
        violations.add("partial_union_cover", || {
            format!(
                "partial {id}: exact union cover by anchor {link} and the recorded residual is {covered:?}{}",
                if disagrees {
                    " and disagrees with the D >= cut slice inclusion"
                } else {
                    ""
                }
            )
        });
    }
}

/// A G2' record's obligations (module note). Classes: `g2_phase`,
/// `g2_initial` (R1: never an initial ID), `g2_merge_stamp` (the recorded
/// stamp is the record's own publication position), `g2_snapshot` (at most
/// that position), `g2_residual` (a D restriction of Q), `missing_edge`,
/// `g2_anchor_stamp` (the anchor's true position), `g2_anchor_order` (merged
/// strictly before the snapshot: positions strictly decrease along anchor
/// links, so they are well-founded), `g2_anchor_kind` (Native, initial-D-band
/// partial or a G2' record with a residual, sealed; recorded kind agrees),
/// `g2_anchor_bucket` (same phase and owner) and, against well-founded
/// admissible anchors only, `g2_union_cover` (exact Q <= residual u scopes).
fn check_g2<const N: usize>(
    id: usize,
    loaded: &Loaded<N>,
    graph: &Graph,
    containment: &Containment,
    initial_count: usize,
    violations: &mut Violations,
) {
    let (nodes, domains) = (&loaded.nodes, &loaded.domains);
    let Some(info) = loaded.g2.get(&id) else {
        return;
    };
    let phase = domains[id].phase();
    if phase != Phase::Apply {
        violations.add("g2_phase", || {
            format!(
                "G2' record {id} is a {phase:?} record: residual inspection exists only in Apply"
            )
        });
    }
    if id < initial_count {
        violations.add("g2_initial", || {
            format!("G2' record {id} is an initial record ({initial_count} initial records)")
        });
    }
    let own = loaded.positions[id];
    if info.merge_stamp != Some(own) {
        violations.add("g2_merge_stamp", || {
            format!(
                "G2' record {id}: recorded merge stamp {:?}, publication position {own}",
                info.merge_stamp
            )
        });
    }
    if info.snapshot > own {
        violations.add("g2_snapshot", || {
            format!(
                "G2' record {id}: snapshot stamp {} after its own publication position {own}",
                info.snapshot
            )
        });
    }
    let whole = ccell(&domains[id]);
    let mut targets: Vec<Cell> = Vec::new();
    if let Some(residual) = info.residual {
        if residual.max_positive_power != whole.powers.max_positive_power
            || residual.min_power_difference.is_none()
            || residual.max_power_difference.is_none()
        {
            violations.add("g2_residual", || {
                format!("G2' record {id}: residual {residual:?} is not a D band of the domain")
            });
        }
        targets.push(Cell {
            powers: residual,
            ..whole.clone()
        });
    }
    if info.anchors.is_empty() {
        violations.add("g2_anchor_kind", || {
            format!("G2' record {id} has no anchor")
        });
    }
    let mut admissible = true;
    for anchor in &info.anchors {
        let a = anchor.id;
        if a >= nodes.len() || nodes[a].kind == Kind::Missing {
            violations.add("g2_anchor_kind", || {
                format!("G2' record {id}: anchor {a} has no record")
            });
            admissible = false;
            continue;
        }
        if !graph.has_edge(id, a) {
            violations.add("missing_edge", || {
                format!("G2' record {id} has no edge to anchor {a}")
            });
        }
        let position = loaded.positions[a];
        if position != anchor.stamp {
            violations.add("g2_anchor_stamp", || {
                format!(
                    "G2' record {id}: anchor {a} stamp {} but publication position {position}",
                    anchor.stamp
                )
            });
        }
        if position >= info.snapshot {
            violations.add("g2_anchor_order", || {
                format!(
                    "G2' record {id}: anchor {a} published at {position}, not before the snapshot {}",
                    info.snapshot
                )
            });
            admissible = false;
        }
        let node = &nodes[a];
        let (kind_ok, scope) = match (anchor.kind.as_str(), node.kind) {
            ("native", Kind::Native) => (true, ccell(&domains[a])),
            ("initial_d_band", Kind::Partial) => (true, low_slice(ccell(&domains[a]), node.cut)),
            ("g2_residual", Kind::G2) => (
                loaded.g2.get(&a).is_some_and(|i| i.residual.is_some()),
                ccell(&domains[a]),
            ),
            _ => (false, ccell(&domains[a])),
        };
        if !kind_ok || node.error || node.frontiers != 0 || !node.finished {
            violations.add("g2_anchor_kind", || {
                format!(
                    "G2' record {id}: anchor {a} ({:?}, recorded {:?}, error {}, {} frontiers) is not a sealed Native, initial-D-band or G2' residual record",
                    node.kind, anchor.kind, node.error, node.frontiers
                )
            });
            admissible = false;
            continue;
        }
        if domains[a].phase() != phase || domains[a].owner() != domains[id].owner() {
            violations.add("g2_anchor_bucket", || {
                format!("G2' record {id}: anchor {a} is in another (phase, owner) bucket")
            });
            admissible = false;
            continue;
        }
        targets.push(scope);
    }
    if !admissible {
        return;
    }
    let refs: Vec<&Cell> = targets.iter().collect();
    let (covered, disagrees) = containment.g2_union(&whole, &refs);
    if covered != Some(true) || disagrees {
        violations.add("g2_union_cover", || {
            format!(
                "G2' record {id}: exact cover by its residual and {} anchors is {covered:?}{}",
                info.anchors.len(),
                if disagrees {
                    " and disagrees with lattice enumeration"
                } else {
                    ""
                }
            )
        });
    }
}

struct Ctx<'a, const N: usize> {
    request: &'a OwnerDomainWalkRequest,
    reducer: &'a RoutedCandidateReducer<N>,
    loaded: &'a Loaded<N>,
    graph: &'a Graph,
    containment: &'a Containment,
    cancellation: &'a AtomicBool,
    /// HiddenError: this node's reference inspection runs under a request
    /// whose native-operation allowance it cannot meet.
    failing: Option<(usize, &'a OwnerDomainWalkRequest)>,
    /// False when a native lever that was on in the run is off in the
    /// reference: event and successor counts may then legitimately differ
    /// (tallied, not violations); errors, frontiers and coverage stay strict.
    count_parity: bool,
}

#[derive(Default)]
struct Tally {
    inspected: u64,
    events: u64,
    successor_events: u64,
    admits: u64,
    admits_successor: u64,
    admits_routed: u64,
    covered_direct: u64,
    covered_alias_chain: u64,
    uncovered: u64,
    uncovered_after_reference_error: u64,
    frontiers: u64,
    errors: u64,
    count_mismatches_under_disabled_levers: u64,
    native_seconds: f64,
    finite_replay_work: Option<Value>,
}
impl Tally {
    fn add(&mut self, other: &Tally) {
        self.inspected += other.inspected;
        self.events += other.events;
        self.successor_events += other.successor_events;
        self.admits += other.admits;
        self.admits_successor += other.admits_successor;
        self.admits_routed += other.admits_routed;
        self.covered_direct += other.covered_direct;
        self.covered_alias_chain += other.covered_alias_chain;
        self.uncovered += other.uncovered;
        self.uncovered_after_reference_error += other.uncovered_after_reference_error;
        self.frontiers += other.frontiers;
        self.errors += other.errors;
        self.count_mismatches_under_disabled_levers += other.count_mismatches_under_disabled_levers;
        self.native_seconds += other.native_seconds;
        if other.finite_replay_work.is_some() {
            self.finite_replay_work = other.finite_replay_work.clone();
        }
    }
    fn json(&self) -> Value {
        let mut value = json!({"inspected":self.inspected,"events":self.events,
            "successor_events":self.successor_events,
            "admitted_domains":self.admits,
            "admitted_successor_domains":self.admits_successor,
            "admitted_routed_domains":self.admits_routed,
            "covered_by_recorded_target":self.covered_direct,
            "covered_along_alias_chain":self.covered_alias_chain,"uncovered":self.uncovered,
            "uncovered_after_reference_error":self.uncovered_after_reference_error,
            "frontiers":self.frontiers,"errors":self.errors,"native_seconds":self.native_seconds,
            "count_mismatches_under_disabled_levers":self.count_mismatches_under_disabled_levers,
            "definitions":"admitted_domains = every Admit effect (successor domains plus Apply domains routed by Route natives); successor_events = events with successor=true (Admit, Frontier or reuse)"});
        if let Some(work) = &self.finite_replay_work {
            value["finite_replay"] = work.clone();
        }
        value
    }
}

/// The domain the walker inspected natively for `id`: the whole domain, or
/// a partial record's D < cut residual.
fn inspected_domain<const N: usize>(loaded: &Loaded<N>, id: usize) -> Domain<N> {
    let node = loaded.nodes[id];
    let mut domain = loaded.domains[id].expand();
    if node.kind == Kind::Partial {
        domain.powers = residual(domain.powers, node.cut);
    }
    if node.kind == Kind::G2
        && let Some(residual) = loaded.g2.get(&id).and_then(|info| info.residual)
    {
        domain.powers = residual;
    }
    domain
}

fn targets_of<const N: usize>(ctx: &Ctx<'_, N>, id: usize) -> Vec<(usize, Phase, Cell)> {
    ctx.graph
        .out(id)
        .iter()
        .map(|&t| {
            let domain = &ctx.loaded.domains[t as usize];
            (t as usize, domain.phase(), ccell(domain))
        })
        .collect()
}

/// Include medium Route fanouts: repeated full scans of 16–255 recorded
/// targets can dominate cold reinspection. This only orders candidates;
/// exact inclusion, brute cross-checks and the full-scan fallback stay intact.
const WIDE_NODE_TARGETS: usize = 16;

/// Candidate order for the coverage scan of a wide node (a rescue's dead
/// prefix holder admits ~1M successors): targets with the admitted image
/// first, then targets of the same phase and owner. Fingerprint collisions
/// only reorder candidates; every candidate is still judged by the caller's
/// exact inclusion, and the caller falls back to the full scan, so the
/// verdict and the coverage tallies never depend on the index.
struct TargetIndex {
    exact: std::collections::HashMap<u64, usize>,
    owners: std::collections::HashMap<u64, Vec<usize>>,
}

impl TargetIndex {
    fn fingerprint(phase: Phase, cell: &Cell, image: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        phase.hash(&mut hasher);
        cell.owner.hash(&mut hasher);
        if image {
            cell.lower.hash(&mut hasher);
            cell.upper.hash(&mut hasher);
            cell.rank.hash(&mut hasher);
            cell.powers.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn new(targets: &[(usize, Phase, Cell)]) -> Self {
        let mut exact = std::collections::HashMap::with_capacity(targets.len());
        let mut owners = std::collections::HashMap::<u64, Vec<usize>>::new();
        for (index, (_, phase, cell)) in targets.iter().enumerate() {
            exact
                .entry(Self::fingerprint(*phase, cell, true))
                .or_insert(index);
            owners
                .entry(Self::fingerprint(*phase, cell, false))
                .or_default()
                .push(index);
        }
        Self { exact, owners }
    }

    fn find(
        &self,
        phase: Phase,
        inner: &Cell,
        targets: &[(usize, Phase, Cell)],
        fits: impl Fn(&(usize, Phase, Cell)) -> bool,
    ) -> Option<usize> {
        if let Some(&index) = self.exact.get(&Self::fingerprint(phase, inner, true))
            && fits(&targets[index])
        {
            return Some(index);
        }
        self.owners
            .get(&Self::fingerprint(phase, inner, false))?
            .iter()
            .copied()
            .find(|&index| fits(&targets[index]))
    }
}

/// Outcome of one reference inspection used to plan mutations: the admitted
/// domains covered by exactly one recorded target of the parent, directly,
/// and by no other target's alias chain (that edge is load-bearing).
struct Reference<const N: usize> {
    admits: Vec<(Domain<N>, usize)>,
    error: bool,
}

fn reference<const N: usize>(ctx: &Ctx<'_, N>, id: usize) -> Reference<N> {
    let targets = targets_of(ctx, id);
    let mut admits = Vec::new();
    let domain = inspected_domain(ctx.loaded, id);
    // Planning must not perturb the report's inclusion counters.
    let scratch = Containment::new(0, 0);
    let mut emit = |event: Event<N>| {
        if let Effect::Admit { domain, .. } = event.effect {
            let inner = cell(&domain);
            let covering: Vec<usize> = targets
                .iter()
                .filter(|(_, phase, outer)| *phase == domain.phase && outer.contains(&inner))
                .map(|(t, _, _)| *t)
                .collect();
            if let [only] = covering[..] {
                let others: Vec<(usize, Phase, Cell)> = targets
                    .iter()
                    .filter(|(t, _, _)| *t != only)
                    .cloned()
                    .collect();
                if !alias_chain_covers(
                    &ctx.loaded.nodes,
                    &ctx.loaded.domains,
                    &scratch,
                    &others,
                    domain.phase,
                    &inner,
                ) {
                    admits.push((domain, only));
                }
            }
        }
        ControlFlow::Continue(())
    };
    let finished = inspection::inspect_reference(
        ctx.reducer,
        &domain,
        ctx.request,
        ctx.cancellation,
        &mut emit,
    );
    Reference {
        admits,
        error: finished.error.is_some(),
    }
}

fn reinspect<const N: usize>(
    ctx: &Ctx<'_, N>,
    id: usize,
    tally: &mut Tally,
    violations: &mut Violations,
    inventory: Option<&inventory::Collector>,
) {
    if id == 0
        && let Some(recipe) = ctx.loaded.finite_replay
    {
        finite_replay::reinspect(ctx, recipe, tally, violations);
        return;
    }
    let node = ctx.loaded.nodes[id];
    if node.kind == Kind::G2
        && ctx
            .loaded
            .g2
            .get(&id)
            .is_none_or(|info| info.residual.is_none())
    {
        // A G2' full cover inspects nothing: it must record nothing.
        let local = Tally {
            inspected: 1,
            ..Tally::default()
        };
        tally.add(&local);
        if node.error || node.frontiers != 0 || node.events.unwrap_or(0) != 0 {
            violations.add("event_parity", || {
                format!("G2' full cover {id} records events, frontiers or an error")
            });
        }
        return;
    }
    let domain = inspected_domain(ctx.loaded, id);
    let targets = targets_of(ctx, id);
    let index = (targets.len() >= WIDE_NODE_TARGETS).then(|| TargetIndex::new(&targets));
    let mut events = 0u64;
    let mut successors = 0u64;
    let mut frontiers = 0u64;
    let mut shortcut = false;
    let mut recent = 0usize;
    let mut local = Tally::default();
    // Coverage misses are judged after the inspection: an inspection that
    // fails carries no successor claim (its node never seals).
    let mut misses: Vec<String> = Vec::new();
    let mut missed = 0u64;
    let mut emit = |event: Event<N>| {
        events += event.count as u64;
        match event.effect {
            Effect::Admit {
                domain: admitted,
                successor,
                ..
            } => {
                successors += u64::from(successor);
                local.admits += 1;
                if successor {
                    local.admits_successor += 1;
                } else {
                    local.admits_routed += 1;
                }
                let inner = cell(&admitted);
                let fits = |(_, phase, outer): &(usize, Phase, Cell)| {
                    *phase == admitted.phase && ctx.containment.contains(outer, &inner)
                };
                // Any fitting target covers; the candidate order (last match,
                // the wide-node index, then the full scan resuming after the
                // last match) only keeps a node with ~1M successors from
                // going quadratic.
                let found = if recent < targets.len() && fits(&targets[recent]) {
                    Some(recent)
                } else {
                    index
                        .as_ref()
                        .and_then(|index| index.find(admitted.phase, &inner, &targets, &fits))
                        .or_else(|| {
                            (recent + 1..targets.len())
                                .chain(0..recent.min(targets.len()))
                                .find(|&index| fits(&targets[index]))
                        })
                };
                if let Some(index) = found {
                    recent = index;
                    local.covered_direct += 1;
                } else if alias_chain_covers(
                    &ctx.loaded.nodes,
                    &ctx.loaded.domains,
                    ctx.containment,
                    &targets,
                    admitted.phase,
                    &inner,
                ) {
                    local.covered_alias_chain += 1;
                } else {
                    missed += 1;
                    if misses.len() < 4 {
                        misses.push(format!(
                            "node {id}: {:?} {} {} {:?}..{:?} r{:?} {:?} is contained in no recorded target of {} edges",
                            admitted.phase,
                            if successor { "successor" } else { "routed domain" },
                            mask(&admitted.owner),
                            admitted.lower,
                            admitted.upper,
                            admitted.rank,
                            admitted.powers,
                            targets.len()
                        ));
                    }
                }
            }
            Effect::Frontier { successor, .. } => {
                successors += u64::from(successor);
                frontiers += 1;
            }
            Effect::KnownReuse { successor, .. }
            | Effect::PreAdmittedOrthantReuse { successor, .. } => {
                successors += u64::from(successor);
                shortcut = true;
            }
            Effect::Count | Effect::Optional(_) => {}
        }
        ControlFlow::Continue(())
    };
    let request = match ctx.failing {
        Some((node_id, failing)) if node_id == id => failing,
        _ => ctx.request,
    };
    let finished = if let Some(inventory) = inventory {
        inspection::inspect_reference_with_classifications(
            ctx.reducer,
            &domain,
            request,
            ctx.cancellation,
            &mut |piece| inventory.classified(piece),
            &mut emit,
        )
    } else {
        inspection::inspect_reference(ctx.reducer, &domain, request, ctx.cancellation, &mut emit)
    };
    let failed = finished.error.is_some();
    local.inspected = 1;
    local.events = events;
    local.successor_events = successors;
    local.frontiers = frontiers;
    local.errors = u64::from(failed);
    local.native_seconds = finished.seconds;
    if failed {
        local.uncovered_after_reference_error = missed;
    } else {
        local.uncovered = missed;
        let mut messages = misses.into_iter();
        for _ in 0..missed {
            let message = messages.next();
            violations.add("successor_uncovered", || {
                message.unwrap_or_else(|| format!("node {id}: further uncovered admitted domain"))
            });
        }
    }
    tally.add(&local);
    if shortcut {
        violations.add("reinspection", || {
            format!("node {id}: reference emitted a reuse shortcut")
        });
    }
    if failed != node.error {
        violations.add("error_parity", || {
            format!(
                "node {id}: record error {} but reference error {:?}",
                node.error, finished.error
            )
        });
    }
    if frontiers != u64::from(node.frontiers) {
        violations.add("frontier_parity", || {
            format!(
                "node {id}: record has {} frontiers, reference inspection {frontiers}",
                node.frontiers
            )
        });
    }
    let counts_differ = !failed
        && (node.events != Some(events)
            || (domain.phase == Phase::Apply && node.successors != Some(successors)));
    if !ctx.count_parity {
        tally.count_mismatches_under_disabled_levers += u64::from(counts_differ);
        return;
    }
    if !failed && node.events != Some(events) {
        violations.add("event_parity", || {
            format!(
                "node {id}: record events {:?}, reference {events}",
                node.events
            )
        });
    }
    // Route records carry no successor statistic; their admitted domains
    // are still covered above.
    if !failed && domain.phase == Phase::Apply && node.successors != Some(successors) {
        violations.add("successor_parity", || {
            format!(
                "node {id}: record successors {:?}, reference {successors}",
                node.successors
            )
        });
    }
}

/// A domain contained in a node reached from a recorded target through
/// alias representatives (the target's responsibility moved there). Sound
/// for closure: the parent depends on the target and, through the alias
/// edges, on every representative along the chain.
fn alias_chain_covers<const N: usize>(
    nodes: &[Node],
    domains: &[CompactDomain<N>],
    containment: &Containment,
    targets: &[(usize, Phase, Cell)],
    phase: Phase,
    inner: &Cell,
) -> bool {
    targets.iter().any(|&(target, _, _)| {
        let mut current = target;
        let mut hops = 0;
        while nodes[current].kind == Kind::Alias && nodes[current].link < nodes.len() && hops < 64 {
            current = nodes[current].link;
            hops += 1;
            let domain = &domains[current];
            if domain.phase() == phase && containment.contains(&ccell(domain), inner) {
                return true;
            }
        }
        false
    })
}

/// Result of the reference re-inspection phase.
struct Reinspection {
    selected: Vec<usize>,
    candidates: usize,
    tally: Tally,
    reinspected: Vec<bool>,
    seconds: f64,
}

fn run_reinspection<const N: usize>(
    ctx: &Ctx<'_, N>,
    options: &OwnerDomainWalkVerifyOptions,
    violations: &mut Violations,
    observer: &impl Fn(Value),
    inventory: Option<&inventory::Collector>,
) -> Reinspection {
    let started = Instant::now();
    let nodes = &ctx.loaded.nodes;
    let total = nodes.len();
    // Rescue-abandoned records were never inspected: nothing to re-derive
    // (they never seal, so no certified cone contains one).
    let candidates: Vec<usize> = (0..total)
        .filter(|&id| nodes[id].native() && !nodes[id].abandoned)
        .collect();
    let selected: Vec<usize> = match options.reinspect {
        OwnerDomainWalkVerifyReinspect::All => candidates.clone(),
        OwnerDomainWalkVerifyReinspect::None => Vec::new(),
        OwnerDomainWalkVerifyReinspect::Sample { count, seed } => sample(&candidates, count, seed),
    };
    let mut reinspected = vec![false; total];
    for &id in &selected {
        reinspected[id] = true;
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let shared = Mutex::new((Tally::default(), Violations::new(options.max_violations)));
    std::thread::scope(|scope| {
        for _ in 0..options.threads.max(1) {
            scope.spawn(|| {
                let mut tally = Tally::default();
                let mut local = Violations::new(options.max_violations);
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= selected.len() || ctx.cancellation.load(Ordering::Relaxed) {
                        break;
                    }
                    reinspect(ctx, selected[index], &mut tally, &mut local, inventory);
                    done.fetch_add(1, Ordering::Relaxed);
                }
                let mut guard = shared.lock().expect("verifier tally");
                guard.0.add(&tally);
                guard.1.merge(local);
            });
        }
        let mut last = Instant::now();
        while done.load(Ordering::Relaxed) < selected.len()
            && !ctx.cancellation.load(Ordering::Relaxed)
        {
            std::thread::sleep(Duration::from_millis(200));
            if last.elapsed() >= Duration::from_secs(10) {
                last = Instant::now();
                observer(
                    json!({"event":"verify_progress","reinspected":done.load(Ordering::Relaxed),
                    "selected":selected.len(),"seconds":started.elapsed().as_secs_f64()}),
                );
            }
        }
    });
    let (tally, worker_violations) = shared.into_inner().expect("verifier tally");
    violations.merge(worker_violations);
    if tally.inspected != selected.len() as u64 {
        violations.add("reinspection", || {
            format!(
                "{} of {} selected natives re-inspected (cancelled)",
                tally.inspected,
                selected.len()
            )
        });
    }
    Reinspection {
        candidates: candidates.len(),
        selected,
        tally,
        reinspected,
        seconds: started.elapsed().as_secs_f64(),
    }
}

/// Binds `path` to the loaded generation; see `result_binding`.
fn bind_result<const N: usize>(
    path: &std::path::Path,
    loaded: &Loaded<N>,
    closed: &[bool],
    violations: &mut Violations,
) -> Result<Value, AppError> {
    let started = Instant::now();
    // Size and mtime before and after the read: a result rewritten while it
    // is being bound is refused, and the pair lets the audit confirm that it
    // read the same file (see the audit's --verify-report).
    let stamp = |path: &std::path::Path| {
        std::fs::metadata(path).ok().map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64);
            (m.len(), mtime)
        })
    };
    let before = stamp(path);
    let total = loaded.nodes.len();
    let mut seen = vec![false; total];
    let (mut compared, mut mismatched, mut duplicates) = (0u64, 0u64, 0u64);
    let (mut result_only, mut result_only_failures, mut result_only_bad) = (0u64, 0u64, 0u64);
    let (mut claimed, mut claimed_not_closed) = (0u64, 0u64);
    let mut first = Vec::new();
    let top = result_binding::stream(path, |row| match row.id {
        Some(id) if id < total && loaded.nodes[id].kind != Kind::Missing => {
            if std::mem::replace(&mut seen[id], true) {
                duplicates += 1;
                return;
            }
            compared += 1;
            if loaded.digests[id] != row.digest {
                mismatched += 1;
                if first.len() < 4 {
                    first.push(id);
                }
            }
            if row.claimed_closed {
                claimed += 1;
                claimed_not_closed += u64::from(!closed[id]);
            }
        }
        _ => {
            // Only an uncommitted failure may be published without being
            // committed: it carries an error or a frontier and no closure.
            result_only += 1;
            let failure = row.has_error || row.frontiers > 0;
            result_only_failures += u64::from(failure);
            result_only_bad += u64::from(row.claimed_closed || !failure);
        }
    })
    .map_err(AppError::input)?;
    let after = stamp(path);
    let records = loaded
        .nodes
        .iter()
        .filter(|n| n.kind != Kind::Missing)
        .count() as u64;
    let missing = records - compared;
    let generation = top.checkpoint["generation"].as_u64();
    let generation_matches = generation == Some(loaded.raw.generation);
    let committed_matches = top.checkpoint["committed_domains"].as_u64() == Some(records);
    let mut problem = |ok: bool, message: String| {
        if !ok {
            violations.add("result_binding", || message);
        }
    };
    problem(
        before.is_some() && before == after && before.map(|b| b.0) == Some(top.bytes),
        format!(
            "result file changed while being read (size/mtime {before:?} -> {after:?}, {} bytes parsed)",
            top.bytes
        ),
    );
    if top.full_result == Some(false) {
        problem(
            loaded
                .epoch
                .as_ref()
                .is_some_and(|s| s.manifest["format"] == "RUSTRED-WALK-CP6")
                && top.checkpoint["format"] == "RUSTRED-WALK-CP6"
                && top.checkpoint["state"] == "saved"
                && generation_matches
                && !top.has_domains
                && top.rows == 0,
            "checkpoint-only summary identity, generation or shape differs".into(),
        );
        return Ok(json!({"kind":"checkpoint_only_summary", "complete":false,
            "generation":generation, "generation_matches":generation_matches,
            "result":path,"bytes":top.bytes,"blake3":top.blake3,
            "note":"summary metadata is not full record binding; verify raw checkpoint with --no-result (omitting --result still auto-selects a nearby result.json)",
            "seconds":started.elapsed().as_secs_f64()}));
    }
    problem(top.has_domains, "result has no domains array".into());
    problem(
        generation_matches,
        format!(
            "result names checkpoint generation {generation:?}, verifier read {}",
            loaded.raw.generation
        ),
    );
    problem(
        committed_matches,
        format!(
            "result committed_domains {} != {records} checkpoint records",
            top.checkpoint["committed_domains"]
        ),
    );
    problem(
        mismatched == 0,
        format!(
            "{mismatched} committed records differ from their published rows (first ids {first:?})"
        ),
    );
    problem(
        missing == 0,
        format!("{missing} committed records are not published in the result"),
    );
    problem(
        duplicates == 0,
        format!("{duplicates} rows published twice"),
    );
    problem(
        result_only_bad == 0,
        format!("{result_only_bad} uncommitted rows are neither failures nor unclaimed"),
    );
    if claimed_not_closed > 0 {
        violations.add("false_closure", || {
            format!("{claimed_not_closed} published descendant_closed claims are not re-derived")
        });
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(json!({"path": path, "canonical_path": canonical,
        "file_bytes": top.bytes, "file_blake3": top.blake3,
        "file_mtime_unix_ns": before.and_then(|b| b.1),
        "generation": generation, "generation_matches": generation_matches,
        "committed_domains_match": committed_matches, "rows": top.rows,
        "records_compared": compared, "records_mismatched": mismatched,
        "records_not_published": missing, "duplicate_rows": duplicates,
        "result_only_rows": result_only, "result_only_failure_rows": result_only_failures,
        "published_closed_claims": claimed,
        "published_closed_claims_not_rederived": claimed_not_closed,
        "result_frontiers": top.frontiers, "result_failed_nodes": top.failed_nodes,
        "ignored_publication_fields": result_binding::PUBLICATION_FIELDS,
        "seconds": started.elapsed().as_secs_f64()}))
}

fn verify<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    inventory: Option<&inventory::Collector>,
) -> Result<Value, AppError> {
    let started = Instant::now();
    let mut violations = Violations::new(options.max_violations);
    let queries = matching::input::parse(
        &request.matching.queries_json,
        selection.physical_arity(),
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    let mut loaded =
        load::<N>(options, options.result.is_some(), &mut violations).map_err(AppError::input)?;
    if inventory.is_some() && loaded.finite_replay.is_some() {
        return Err(AppError::input(
            "walk inventory does not yet support finite-replay summary records",
        ));
    }
    if loaded.finite_replay.is_some()
        && (options.reinspect != OwnerDomainWalkVerifyReinspect::All
            || options.reference_levers != OwnerDomainWalkVerifyReferenceLevers::Off)
    {
        return Err(AppError::input(
            "finite replay summaries require cold reinspect=All and reference-levers=Off",
        ));
    }
    if loaded.raw.publication_policy == "epoch" {
        super::epoch::admit_extensions(request)?;
    }
    let loaded_seconds = started.elapsed().as_secs_f64();
    let memory_loaded = memory_status();
    observer(
        json!({"event":"verify_loaded","domains":loaded.domains.len(),
        "edges":loaded.raw.edges.len(),"seconds":loaded_seconds,"memory":memory_loaded}),
    );
    let prepare_started = Instant::now();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.workers = options.threads.max(1);
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    super::finite_replay::configure_load(request, &mut load);
    if let Some(budget) = request.finite_replay_budget_summary() {
        observer(
            json!({"event":"finite_replay_budget", "phase":"cold_preparation", "budget":budget}),
        );
    }
    let mut prepared_owners = None;
    let mut bind = |owners: Vec<String>| {
        prepared_owners = Some(owners);
        Ok::<(), String>(())
    };
    let reducer = prepare::prepare_with_fingerprints::<N>(
        &load,
        selection,
        load_limits,
        cancellation,
        observer,
        Some(&mut bind),
    )?
    .ok_or_else(|| AppError::input("cancelled during owner preparation"))?;
    if let Some(inventory) = inventory {
        inventory.prepared(reducer.programs())?;
    }
    let prepare_seconds = prepare_started.elapsed().as_secs_f64();
    let memory_prepared = memory_status();
    observer(json!({"event":"verify_prepared","seconds":prepare_seconds,"memory":memory_prepared}));
    let containment = Containment::new(
        options.brute_force_max_points,
        options.brute_force_point_budget,
    );
    // The reference request (native levers per `options.reference_levers`).
    let (reference, levers_differing) = reference_request(request, options.reference_levers);
    let count_parity = levers_differing.is_empty();
    // A request the HiddenError node's reference inspection cannot satisfy.
    let mut failing_request = reference.clone();
    failing_request.applied_limits.max_native_operations = 1;
    let mut failing = None;
    let mutation = match options.mutation {
        Some(kind) => {
            let graph = Graph::from_edges(loaded.domains.len(), &loaded.raw.edges)
                .map_err(AppError::input)?;
            let ctx = Ctx {
                request: &reference,
                reducer: &reducer,
                loaded: &loaded,
                graph: &graph,
                containment: &containment,
                cancellation,
                failing: None,
                count_parity,
            };
            let plan = plan_mutation(&ctx, kind, &queries);
            drop(ctx);
            let (report, fail) = apply_mutation(&mut loaded, kind, plan, &graph);
            failing = fail;
            Some(report)
        }
        None => None,
    };
    // Walk semantics 3 binds its own request digest. CP6 additionally compares
    // authenticated scheduling scalars; worker width and aggregate allowances
    // remain changeable, and an omitted window inherits the saved bound.
    let bound = if loaded.raw.publication_policy == "epoch" {
        checkpoint::epoch_request_binding(request)
    } else {
        checkpoint::request_binding(request)
    } == loaded.raw.request
        && loaded.epoch.as_ref().is_none_or(|sections| {
            epoch_checkpoint::schedule_matches_request(&sections.manifest, request)
        });
    if let Some(sections) = &loaded.epoch {
        let nodes = &loaded.nodes;
        let record_of = |id: usize| {
            nodes.get(id).and_then(|node| match node.kind {
                Kind::Native | Kind::Partial | Kind::G2 => {
                    Some((true, node.frontiers, node.error, None, node.abandoned))
                }
                Kind::Alias => Some((false, 0, false, Some(node.link), false)),
                Kind::Missing => None,
            })
        };
        epoch_export::check(
            sections,
            &loaded.domains,
            &loaded.raw.flags,
            &record_of,
            &mut |class, message| violations.add(class, || message),
        );
    }
    if !bound {
        violations.add("binding", || {
            "checkpoint request digest or persisted schedule differs from the command's binding"
                .into()
        });
    }
    let owners_match = prepared_owners.as_ref() == Some(&loaded.raw.owners);
    if !owners_match {
        violations.add("binding", || {
            "owner payload digests differ from the checkpoint".into()
        });
    }
    let checks_started = Instant::now();
    let total = loaded.domains.len();
    let edge_records = loaded.raw.edges.len();
    let graph = Graph::from_edges(total, &loaded.raw.edges).map_err(AppError::input)?;
    // The CSR holds every edge; the raw pair list is not used again.
    loaded.raw.edges = Vec::new();
    let sealed = sealed_with(&loaded.nodes, &loaded.g2, &graph);
    let loaded = &loaded;
    let nodes = &loaded.nodes;
    let flags = &loaded.raw.flags;
    let mut counts = BTreeMap::<&'static str, u64>::new();
    let initial_count = loaded.raw.counters[10];
    for (id, node) in nodes.iter().enumerate() {
        let flag = flags.get(id).copied().unwrap_or(0);
        *counts
            .entry(match node.kind {
                Kind::Missing => "unpublished",
                Kind::Native => "natives",
                Kind::Partial => "partials",
                Kind::Alias => "aliases",
                Kind::G2 => "g2_records",
            })
            .or_default() += 1;
        if node.abandoned && (loaded.raw.amendments.is_empty() || !graph.out(id).is_empty()) {
            violations.add("rescue_abandoned", || {
                format!(
                    "record {id}: rescue-abandoned outside an amended walk or with dependency edges"
                )
            });
        }
        if (flag & FLAG_SEALED != 0) != sealed[id] {
            violations.add("seal_parity", || {
                format!(
                    "node {id} ({:?}, error {}, {} frontiers): saved seal flag {} but records imply {}",
                    node.kind,
                    node.error,
                    node.frontiers,
                    flag & FLAG_SEALED != 0,
                    sealed[id]
                )
            });
        }
        match node.kind {
            Kind::Alias if node.link < total => {
                if !graph.has_edge(id, node.link) {
                    violations.add("missing_edge", || {
                        format!("alias {id} has no edge to {}", node.link)
                    });
                }
                let (inner, outer) = (&loaded.domains[id], &loaded.domains[node.link]);
                if inner.phase() != outer.phase()
                    || !containment.contains(&ccell(outer), &ccell(inner))
                {
                    violations.add("alias_containment", || {
                        format!(
                            "alias {id} is not contained in its representative {}",
                            node.link
                        )
                    });
                }
            }
            Kind::Partial => check_partial(
                id,
                &loaded.nodes,
                &loaded.domains,
                &loaded.residuals,
                &graph,
                &containment,
                initial_count,
                &mut violations,
            ),
            Kind::G2 => check_g2(
                id,
                loaded,
                &graph,
                &containment,
                initial_count,
                &mut violations,
            ),
            _ => {}
        }
    }
    // Global parity against the saved counters: every retained frontier and
    // failure is explicit in a record, an input or a carried receipt.
    let [
        events_counter,
        successor_counter,
        _,
        _,
        _,
        frontier_counter,
        completed_counter,
        native_counter,
        ..,
    ] = loaded.raw.counters;
    let epoch_abandonment = loaded.raw.publication_policy == "epoch";
    let record_frontiers: u64 = nodes
        .iter()
        .filter(|n| !epoch_abandonment || !n.abandoned)
        .map(|n| u64::from(n.frontiers))
        .sum();
    let carried_frontiers: u64 = loaded
        .raw
        .uncommitted
        .iter()
        .map(|u| u["frontiers"].as_array().map_or(0, |a| a.len() as u64))
        .sum();
    let explicit_frontiers =
        record_frontiers + loaded.raw.input_frontiers.len() as u64 + carried_frontiers;
    if frontier_counter as u64 != explicit_frontiers {
        violations.add("frontier_counter", || {
            format!("saved frontier counter {frontier_counter} != {explicit_frontiers} explicit frontiers")
        });
    }
    let natives = nodes
        .iter()
        .filter(|n| n.native() && (!epoch_abandonment || !n.abandoned))
        .count();
    let completed = nodes
        .iter()
        .filter(|n| n.native() && !n.error && (!epoch_abandonment || !n.abandoned))
        .count();
    if native_counter != natives || completed_counter != completed {
        violations.add("native_counter", || {
            format!(
                "saved native/completed counters {native_counter}/{completed_counter} != records {natives}/{completed}"
            )
        });
    }
    let native_sum = |field: fn(&Node) -> Option<u64>| -> u64 {
        nodes
            .iter()
            .filter(|n| n.native())
            .map(|n| field(n).unwrap_or(0))
            .sum()
    };
    let record_events = native_sum(|n| n.events);
    let record_successors = native_sum(|n| n.successors);
    if loaded.raw.uncommitted.is_empty() {
        if events_counter as u64 != record_events {
            violations.add("event_counter", || {
                format!("saved event counter {events_counter} != {record_events} record events")
            });
        }
        if successor_counter as u64 != record_successors {
            violations.add("event_counter", || {
                format!(
                    "saved successor counter {successor_counter} != {record_successors} record successors"
                )
            });
        }
    }
    let closed = graph.closed(&sealed);
    let mut engine_closed = 0u64;
    let mut engine_open_oracle_closed = 0u64;
    for id in 0..total {
        let flag_closed = flags.get(id).is_some_and(|f| f & FLAG_CLOSED != 0);
        engine_closed += u64::from(flag_closed);
        if flag_closed && !closed[id] {
            violations.add("false_closure", || {
                format!("node {id} is flagged closed but reaches an unsealed node")
            });
        }
        engine_open_oracle_closed += u64::from(!flag_closed && closed[id]);
    }
    let cyclic = graph.cyclic();
    // Rescue amendments (`rescue.rs`): the command must name exactly the
    // checkpoint's recorded chain, each file bound by its digest and chained
    // from the request binding. Amended queries follow the original ones.
    let amended = match request
        .amendments
        .iter()
        .map(|a| super::rescue::parse(a, selection.physical_arity()))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(parsed) => parsed,
        Err(error) => {
            violations.add("amendment_chain", || error);
            Vec::new()
        }
    };
    {
        let chain_check = checkpoint::rescue_chain_base(
            request,
            &loaded.raw.request,
            loaded.raw.g2_activation.as_ref(),
            &loaded.raw.amendments,
        )
        .and_then(|base| {
            super::rescue::check_chain(&loaded.raw.amendments, &amended, &base, &queries)
        });
        if let Err(error) = chain_check {
            violations.add("amendment_chain", || error);
        } else if amended.len() != loaded.raw.amendments.len() {
            violations.add("amendment_chain", || {
                format!(
                    "the command supplies {} amendments, the checkpoint records {}",
                    amended.len(),
                    loaded.raw.amendments.len()
                )
            });
        }
    }
    let amended_start = queries.len();
    let queries: Vec<&matching::input::Query> = queries
        .iter()
        .chain(amended.iter().flat_map(|a| a.queries.iter()))
        .collect();
    let amendment_of: Vec<Option<u64>> = (0..queries.len())
        .map(|index| {
            let mut offset = amended_start;
            for amendment in &amended {
                if index >= offset && index < offset + amendment.queries.len() {
                    return Some(amendment.sequence);
                }
                offset += amendment.queries.len();
            }
            None
        })
        .collect();
    // Roots: every query's record, authenticated by the request binding.
    let mut root_of_query = Vec::new();
    let mut admitting = BTreeMap::<usize, usize>::new();
    let mut unresolved_input_queries = 0usize;
    let cp6 = loaded
        .epoch
        .as_ref()
        .is_some_and(|e| e.manifest["format"] == "RUSTRED-WALK-CP6");
    let unadmitted = if cp6 {
        let metadata = &loaded.epoch.as_ref().expect("CP6 sections").manifest;
        if metadata["total_queries"].as_u64() != Some(amended_start as u64)
            || metadata["processed_queries"].as_u64()
                != Some(loaded.raw.inputs.len().min(amended_start) as u64)
        {
            violations.add("root_mapping", || {
                "CP6 query prefix inventory differs".into()
            });
        }
        queries.len().saturating_sub(loaded.raw.inputs.len())
    } else {
        0
    };
    if loaded.raw.inputs.len() != queries.len() && !(cp6 && unadmitted > 0) {
        violations.add("root_mapping", || {
            format!(
                "{} saved inputs for {} queries",
                loaded.raw.inputs.len(),
                queries.len()
            )
        });
    }
    for (index, query) in queries.iter().enumerate() {
        let entry = loaded.raw.inputs.get(index);
        if cp6 && entry.is_none() && index >= loaded.raw.inputs.len() {
            // An authenticated unfinished initial-admission suffix is not an
            // invalid root, nor an implicitly discharged request.
            root_of_query.push(None);
            continue;
        }
        let record = entry.and_then(|e| e["domain"].as_u64()).map(|r| r as usize);
        // Derive phase from the immutable request and independently prepared
        // owner registry, never from the record being authenticated. Initial
        // queries and amendments have different missing-owner admission rules.
        let installed = reducer.programs().owner_sectors().any(|owner| {
            owner.as_slice()
                == rustred::storage_array::<_, N>(&query.owner, false)
                    .as_ref()
                    .map(|owner| owner.as_slice())
                    .unwrap_or(&[])
        });
        let phase = query_root_phase(
            installed,
            request.route_domain_overcover,
            reducer.domain_routing_requires_source_conditions(),
            amendment_of[index].is_some(),
        );
        if entry.and_then(|e| e["id"].as_str()) != Some(query.id.as_str()) {
            violations.add("root_mapping", || {
                format!("saved input {index} is not query {}", query.id)
            });
        }
        if cp6 {
            let role = if query.auxiliary {
                "auxiliary"
            } else {
                "required"
            };
            if entry.and_then(|e| e["role"].as_str()) != Some(role)
                || entry.and_then(|e| e["role_declared"].as_bool()) != Some(query.role_declared)
            {
                violations.add("root_mapping", || {
                    format!("CP6 query {} role differs", query.id)
                });
            }
            if phase.is_none()
                && entry.is_some_and(|e| {
                    e["domain"].is_null() && e["source_validity_unresolved"] == true
                })
                && loaded
                    .raw
                    .input_frontiers
                    .get(unresolved_input_queries)
                    .is_some_and(|f| epoch_checkpoint::source_frontier_matches::<N>(query, f))
            {
                unresolved_input_queries += 1;
                root_of_query.push(None);
                continue;
            }
        }
        if let Some(sequence) = amendment_of[index] {
            // An amended query resolves to any admitted record (a new one, or
            // an existing record outside the quarantine that contains it).
            if entry.and_then(|e| e["amendment"].as_u64()) != Some(sequence) {
                violations.add("root_mapping", || {
                    format!(
                        "saved input {index} is not amended query {} of amendment {sequence}",
                        query.id
                    )
                });
            }
            let Some(record) = record.filter(|&r| r < total) else {
                violations.add("root_mapping", || {
                    format!("amended query {} has no record", query.id)
                });
                root_of_query.push(None);
                continue;
            };
            admitting.entry(record).or_insert(index);
            if !query_root_matches(
                &loaded.domains[record],
                phase,
                &query_cell::<N>(query),
                false,
                &containment,
            ) {
                violations.add("root_mapping", || {
                    format!(
                        "amended query {} is not contained in record {record}",
                        query.id
                    )
                });
            }
            root_of_query.push(Some(record));
            continue;
        }
        let Some(record) = record.filter(|&r| r < initial_count && r < total) else {
            violations.add("root_mapping", || {
                format!("query {} has no initial record", query.id)
            });
            root_of_query.push(None);
            continue;
        };
        let query_cell = query_cell::<N>(query);
        let first = *admitting.entry(record).or_insert(index) == index;
        let ok = query_root_matches(
            &loaded.domains[record],
            phase,
            &query_cell,
            first,
            &containment,
        );
        if !ok {
            violations.add("root_mapping", || {
                format!(
                    "query {} is not {} record {record}",
                    query.id,
                    if first { "equal to" } else { "contained in" }
                )
            });
        }
        root_of_query.push(Some(record));
    }
    // Rescue amendments may admit new roots beyond the protected initial
    // prefix. Keep those mappings for certification below, but require only
    // the original prefix to have a complete set of admitting queries here.
    if cp6
        && admitting
            .range(..initial_count)
            .map(|(&id, _)| id)
            .ne(0..initial_count)
    {
        violations.add("root_mapping", || {
            "CP6 protected prefix contains an ID without an admitting query".into()
        });
    }
    if cp6 && unresolved_input_queries != loaded.raw.input_frontiers.len() {
        violations.add("root_mapping", || {
            "CP6 input frontiers differ from independently required source obligations".into()
        });
    }
    let result_binding = match &options.result {
        Some(path) => Some(bind_result(path, loaded, &closed, &mut violations)?),
        None => None,
    };
    let union_report = (options.union_sample > 0).then(|| {
        let (report, failures) = union_sample::validate(
            &loaded.domains,
            &loaded.nodes,
            options.union_sample,
            options.union_sample_seed,
            options.threads,
            cancellation,
        );
        for _ in 0..failures {
            violations.add("union_cross_check", || {
                "exact union cover disagrees with point enumeration (see union_sample)".into()
            });
        }
        report
    });
    let checks_seconds = checks_started.elapsed().as_secs_f64();
    let memory_checked = memory_status();
    observer(json!({"event":"verify_checked","seconds":checks_seconds,"memory":memory_checked}));
    // F10 reference re-inspection.
    let ctx = Ctx {
        request: &reference,
        reducer: &reducer,
        loaded,
        graph: &graph,
        containment: &containment,
        cancellation,
        failing: failing.map(|id| (id, &failing_request)),
        count_parity,
    };
    let reinspection = run_reinspection(&ctx, options, &mut violations, observer, inventory);
    let complete = reinspection.selected.len() == reinspection.candidates
        && reinspection.tally.inspected == reinspection.candidates as u64;
    // Roots, cones and certification.
    let mut roots = BTreeMap::<usize, Vec<&str>>::new();
    for (query, root) in queries.iter().zip(&root_of_query) {
        if let Some(root) = root {
            roots.entry(*root).or_default().push(query.id.as_str());
        }
    }
    let cone_budget =
        (roots.len() as u128) * (total as u128 + graph.edges() as u128) <= CONE_VISIT_BUDGET;
    let mut mark = vec![0u32; if cone_budget { total } else { 0 }];
    let mut root_rows = Vec::new();
    let mut root_state = BTreeMap::new();
    for (stamp, (&root, ids)) in roots.iter().enumerate() {
        let mut row = json!({"record":root,"queries":ids,"oracle_closed":closed[root],
            "engine_closed":flags.get(root).is_some_and(|f| f & FLAG_CLOSED != 0)});
        let mut fully_reinspected = complete;
        if cone_budget {
            let (mut unsealed, mut missing, mut on_cycles, mut natives_in_cone) = (0, 0, 0, 0);
            let size = graph.cone(root, &mut mark, stamp as u32 + 1, |id| {
                unsealed += usize::from(!sealed[id]);
                natives_in_cone += usize::from(nodes[id].native());
                missing += usize::from(nodes[id].native() && !reinspection.reinspected[id]);
                on_cycles += usize::from(cyclic[id]);
            });
            if (unsealed == 0) != closed[root] {
                violations.add("closure_derivation", || {
                    format!("root {root}: forward cone and reverse reachability disagree")
                });
            }
            fully_reinspected = missing == 0;
            row["cone_nodes"] = json!(size);
            row["cone_natives"] = json!(natives_in_cone);
            row["cone_unsealed"] = json!(unsealed);
            row["cone_natives_not_reinspected"] = json!(missing);
            row["cone_nodes_on_cycles"] = json!(on_cycles);
        }
        row["fully_reinspected"] = json!(fully_reinspected);
        root_state.insert(root, (closed[root], fully_reinspected));
        root_rows.push(row);
    }
    // Certification scope. An amended walk (frontier rescue, `rescue.rs`)
    // certifies PER REQUIRED QUERY (immutable exact-ID declaration): through
    // the first input root, in input order, that is oracle-closed and
    // contains the query (same phase as the query's own root; exact lattice
    // inclusion). Only those certifying roots are required (roots_total);
    // helper roots are reported separately and may stay open. A physics
    // query without such a root requires its own root, which then fails.
    let all_root_state = root_state.clone();
    let physics_scope = match options.certification_scope {
        OwnerDomainWalkVerifyScope::AllRoots => false,
        OwnerDomainWalkVerifyScope::PhysicsQueries => true,
        OwnerDomainWalkVerifyScope::Auto => !amended.is_empty(),
    };
    let mut physics_report = Value::Null;
    let mut helper_report = Value::Null;
    let root_state = if physics_scope {
        let mut required = BTreeMap::new();
        let (mut total_physics, mut certified) = (0usize, 0usize);
        let mut uncertified = Vec::new();
        let mut via_amendment = 0usize;
        for (index, query) in queries.iter().enumerate() {
            if query.auxiliary {
                continue;
            }
            total_physics += 1;
            if cp6 && root_of_query[index].is_none() {
                if uncertified.len() < 1_000 {
                    uncertified.push(query.id.clone());
                }
                continue;
            }
            let cell = query_cell::<N>(query);
            let own = root_of_query[index];
            let phase = own.map_or(Phase::Apply, |r| loaded.domains[r].phase());
            let via = root_of_query.iter().enumerate().find_map(|(input, root)| {
                let root = (*root)?;
                (closed[root]
                    && loaded.domains[root].phase() == phase
                    && containment.contains(&ccell(&loaded.domains[root]), &cell))
                .then_some((input, root))
            });
            match via {
                Some((input, root)) => {
                    certified += 1;
                    via_amendment += usize::from(amendment_of[input].is_some());
                    required.insert(root, all_root_state[&root]);
                }
                None => {
                    if uncertified.len() < 1_000 {
                        uncertified.push(query.id.clone());
                    }
                    if let Some(root) = own {
                        required.insert(root, all_root_state[&root]);
                    }
                    if options.require_closure {
                        violations.add("closure_required", || {
                            format!("physics query {} has no closed containing root", query.id)
                        });
                    }
                }
            }
        }
        let helper_roots: std::collections::BTreeSet<usize> = queries
            .iter()
            .zip(&root_of_query)
            .filter(|(query, _)| query.auxiliary)
            .filter_map(|(_, root)| *root)
            .collect();
        let open: Vec<usize> = helper_roots
            .iter()
            .copied()
            .filter(|&r| !closed[r])
            .collect();
        physics_report = json!({"total":total_physics,"certified":certified,
            "certified_through_amended_roots":via_amendment,"uncertified":uncertified,
            "certifying_roots":required.len()});
        helper_report = json!({"total":helper_roots.len(),"closed":helper_roots.len() - open.len(),
            "not_closed":open.iter().take(1_000).collect::<Vec<_>>(),"required":false,
            "note":"helper roots are auxiliary; a frontier-bearing helper may stay uncertified once every physics query it held is certified through a closed containing root"});
        required
    } else {
        root_state
    };
    if options.require_closure {
        for (&root, &(is_closed, _)) in &root_state {
            if !is_closed {
                violations.add("closure_required", || format!("root {root} is not closed"));
            }
        }
    }
    let initial_closed_oracle = (0..initial_count.min(total))
        .filter(|&id| closed[id])
        .count();
    let engine_initial_closed = loaded.raw.closure["initial_closed"].as_u64();
    if engine_initial_closed.is_some_and(|c| c as usize > initial_closed_oracle) {
        violations.add("false_closure", || {
            "engine initial_closed exceeds the re-derived count".into()
        });
    }
    if containment.disagreements() > 0 {
        violations.add("containment_cross_check", || {
            format!(
                "{} exact inclusions disagree with lattice enumeration",
                containment.disagreements()
            )
        });
    }
    let consistent = violations.is_empty();
    // Under --require-closure a PASS certifies every root: each must be
    // closed (a violation otherwise) AND independently verified, i.e. every
    // native of its cone re-inspected. Partial or no re-inspection is never
    // a PASS: it is INCOMPLETE, with its own exit status.
    let roots_not_independently_verified = root_state.values().filter(|s| !s.1).count();
    let roots_independently_verified = if consistent {
        root_state.values().filter(|s| s.0 && s.1).count()
    } else {
        0
    };
    let summary_only = result_binding
        .as_ref()
        .is_some_and(|b| b["kind"] == "checkpoint_only_summary");
    let verdict = if !consistent {
        "FAIL"
    } else if unadmitted > 0
        || unresolved_input_queries > 0
        || summary_only
        || !complete
        || (options.require_closure && roots_not_independently_verified > 0)
    {
        "INCOMPLETE"
    } else {
        "PASS"
    };
    let verdict_reason = match verdict {
        "FAIL" => "violations found".to_string(),
        "INCOMPLETE" if summary_only => "checkpoint-only result has no full record proof; verify the checkpoint with --no-result (omitting --result still auto-selects a nearby result.json)".into(),
        "INCOMPLETE" if unadmitted > 0 => format!("{unadmitted} immutable input queries remain UNADMITTED; no closure certificate"),
        "INCOMPLETE" if unresolved_input_queries > 0 => format!("{unresolved_input_queries} input queries retain unresolved source validity; no closure certificate"),
        "INCOMPLETE" => format!(
            "no violation found, but only {} of {} natives were re-inspected ({} of {} roots \
             have a fully re-inspected cone); not a certificate",
            reinspection.tally.inspected,
            reinspection.candidates,
            root_state.len() - roots_not_independently_verified,
            root_state.len()
        ),
        _ if options.require_closure && physics_scope => {
            "no violation; every native re-inspected; every physics query certified through a closed, independently verified containing root (helper roots reported separately)"
                .to_string()
        }
        _ if options.require_closure => {
            "no violation; every native re-inspected; every root closed and independently verified"
                .to_string()
        }
        _ => "no violation; every native re-inspected".to_string(),
    };
    let mut classes = BTreeMap::<&str, BTreeMap<&str, u64>>::new();
    for (index, (query, root)) in queries.iter().zip(&root_of_query).enumerate() {
        let class = if query.auxiliary { "helper" } else { "physics" };
        let entry = classes.entry(class).or_default();
        *entry.entry("total").or_default() += 1;
        let state = root
            .and_then(|r| all_root_state.get(&r))
            .copied()
            .unwrap_or((false, false));
        let admitted = root.is_some_and(|r| admitting.get(&r) == Some(&index));
        *entry
            .entry(if cp6 && index >= loaded.raw.inputs.len() {
                "unadmitted"
            } else if admitted {
                "admitting"
            } else {
                "absorbed"
            })
            .or_default() += 1;
        *entry.entry("oracle_closed").or_default() += u64::from(state.0);
        *entry.entry("consistent_closed").or_default() += u64::from(consistent && state.0);
        *entry.entry("independently_verified").or_default() +=
            u64::from(consistent && state.0 && state.1);
    }
    let sample_fraction = if reinspection.candidates == 0 {
        1.0
    } else {
        reinspection.selected.len() as f64 / reinspection.candidates as f64
    };
    let a = request.applied_limits;
    Ok(json!({
        "schema": OWNER_DOMAIN_WALK_VERIFY_SCHEMA,
        "verdict": verdict,
        "verdict_reason": verdict_reason,
        // Gate fields: a gate asserts verdict == PASS and
        // roots_independently_verified == roots_total (assert_oracle_pass.py).
        "roots_total": root_state.len(),
        "roots_independently_verified": roots_independently_verified,
        "closure_required": options.require_closure,
        "certification_scope": if physics_scope { "physics_queries_through_closed_containing_roots" } else { "all_roots" },
        "physics_queries": physics_report,
        "helper_roots": helper_report,
        "amendments": {"count": amended.len(), "digests": amended.iter().map(|a| a.digest.as_str()).collect::<Vec<_>>(),
            "amended_queries": queries.len() - amended_start},
        "family_closure_claim": false,
        "scope": "re-derived dependency closure (coinductive: sealed cycles count as closed) and reference re-inspection coverage of saved natives; not IBP replay, descent or termination",
        "reference": {
            "inspector": "the walker's native visitor (inspect_native) under the run's request, with the job-local reuse cache, pre-admitted orthants and initial-overlap planning removed (a partial record's D < cut residual is inspected); physical subdivision runs are refused",
            "walk_levers_disabled": ["job-local reuse cache", "pre-admitted orthant shortcuts", "initial-overlap planning (the partial record's D < cut residual is inspected; its D >= cut slice is checked against the anchor)", "physical Apply subdivision (refused)"],
            "native_levers": format!("{:?}", options.reference_levers),
            "native_levers_disabled": if options.reference_levers == OwnerDomainWalkVerifyReferenceLevers::Off { json!(REFERENCE_NATIVE_LEVERS) } else { json!([]) },
            "native_levers_not_in_tree": ["N1 modular zero certificates", "N4 cover-first"],
            "native_levers_on_in_run_off_in_reference": levers_differing,
            "count_parity": if count_parity { "enforced" } else { "informational: a native lever that was on in the run is off in the reference; error, frontier and coverage checks stay enforced" },
            "kept_as_run": "match limits and refinement axes, Apply cell refinement, Route domain overcover, Route mask and native resource allowances (they define which pieces and successors exist and are bound in the request digest)",
            "independent_of_engine": "graph bookkeeping: recorded edges, semantic hits, aliases, partial anchors, orthant hits, seal flags, counters",
            "reproduced_not_independent": "successor, frontier and problem generation by the native reducer (the same visitor under the run's native policies minus native_levers_disabled: with no lever differing, event and successor counts agree by construction for a deterministic visitor; a successor lost inside the reducer, or an unsound policy kept as run, is reproduced, not caught)",
            "per_node_parity": {
                "apply_natives": ["error", "frontier count", "event count", "successor count (successor=true events)"],
                "route_natives": ["error", "frontier count", "event count"],
                "route_note": "Route records carry no successor statistic; every domain a Route native admits (routed Apply domains, successor=false) is still checked for coverage",
                "coverage": "every Admit effect (successor or routed domain) must be contained, same phase and owner, in a recorded out-edge target of its parent or along that target's alias chain"
            },
            "native_policies_in_force": {
                "match_limits": format!("{:?}", request.matching.match_limits),
                "cell_refinement": format!("{:?}", a.cell_refinement),
                "route_domain_overcover": request.route_domain_overcover,
                "route_joint_source_support_pruning": reference.route_joint_source_support_pruning,
                "route_joint_source_support_pruning_in_run": request.route_joint_source_support_pruning,
                "max_route_masks": request.max_route_masks,
            },
        },
        "checkpoint": {"directory": options.checkpoint, "generation": loaded.raw.generation,
            "publication_policy": loaded.raw.publication_policy,
            "walk_semantics_version": loaded.raw.walk_semantics_version,
            "executable": loaded.raw.executable, "request_digest": loaded.raw.request,
            "request_binding_matches": bound, "owner_digests_match": owners_match,
            // Actual immutable bytes imported by this cold preparation, not a
            // later re-read of mutable paths. Consumers must still require PASS
            // and owner_digests_match before binding a portable package to them.
            "prepared_owner_payload_blake3": prepared_owners,
            "prepared_owner_payload_order": "selection owners, then domain_rule_overlays, then preferred_owner_programs; selection order within each group",
            "file_digest_verify_seconds": loaded.raw.verify_seconds,
            "engine_closure": loaded.raw.closure},
        "result_binding": result_binding,
        "queries": {"count": queries.len(), "admitted_prefix":loaded.raw.inputs.len(),
            "unadmitted":unadmitted, "unadmitted_ids":queries.iter().skip(queries.len() - unadmitted).map(|q| &q.id).collect::<Vec<_>>(),
            "unresolved_input_frontiers":unresolved_input_queries,
            "blake3": blake3::hash(request.matching.queries_json.as_bytes()).to_hex().to_string()},
        "counts": {"domains": total, "edges": edge_records,
            "g2_residual_records": loaded.g2.values().filter(|info| info.residual.is_some()).count(),
            "duplicate_edges": graph.duplicate_edges(), "records": counts,
            "sealed": sealed.iter().filter(|&&s| s).count(),
            "oracle_closed": closed.iter().filter(|&&c| c).count(),
            "engine_closed": engine_closed,
            "engine_open_oracle_closed": engine_open_oracle_closed,
            "nodes_on_cycles": cyclic.iter().filter(|&&c| c).count(),
            "initial_records": initial_count, "initial_closed_oracle": initial_closed_oracle,
            "roots": roots.len(),
            "roots_oracle_closed": root_state.values().filter(|s| s.0).count(),
            "roots_independently_verified": roots_independently_verified},
        "roots": root_rows,
        "cones_computed": cone_budget,
        "certification": {"query_roles": "immutable_exact_id_declaration; undeclared_queries_required", "classes": classes,
            "claim_levels": "oracle_closed = re-derived closed from the saved edges and seal rule; consistent_closed = oracle_closed and no violation; independently_verified = consistent_closed and every native in the root's cone re-inspected (only these may be cited as verified)"},
        "reinspection": {"mode": format!("{:?}", options.reinspect),
            "selected": reinspection.selected.len(), "candidates": reinspection.candidates,
            "complete": complete, "sampled_fraction": sample_fraction,
            "single_defective_native_detection_probability": sample_fraction,
            "threads": options.threads, "tally": reinspection.tally.json()},
        "containment": containment.json(),
        "union_sample": union_report,
        "mutation": mutation,
        "violations": violations.list,
        "violations_by_class": violations.by_class,
        "violations_suppressed": violations.suppressed,
        "timing": {"load_seconds": loaded_seconds, "prepare_seconds": prepare_seconds,
            "checks_seconds": checks_seconds, "reinspect_seconds": reinspection.seconds,
            "total_seconds": started.elapsed().as_secs_f64()},
        "memory": {"after_load": memory_loaded, "after_prepare": memory_prepared,
            "after_checks": memory_checked, "at_end": memory_status(),
            "scope": "whole verifier process (domains, records, edge CSR, prepared owners); peak_rss_bytes is VmHWM"},
    }))
}

/// `VmRSS` and `VmHWM` of this process in bytes (null off Linux).
fn memory_status() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().strip_suffix("kB"))
            .and_then(|kb| kb.trim().parse::<u64>().ok())
            .map(|kb| kb * 1024)
    };
    json!({"rss_bytes": field("VmRSS:"), "peak_rss_bytes": field("VmHWM:")})
}

fn query_cell<const N: usize>(query: &matching::input::Query) -> Cell {
    Cell {
        owner: rustred::storage_array::<_, N>(&query.owner, false)
            .expect("admitted arity")
            .to_vec(),
        lower: rustred::storage_array::<_, N>(&query.lower, 0)
            .expect("admitted arity")
            .to_vec(),
        upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
            .expect("admitted arity")
            .to_vec(),
        rank: query.rank,
        powers: query.powers,
    }
}

/// Independent statement of initial admission and rescue amendment policy.
/// Missing-owner originals may be inspected as Apply without overcover;
/// amendments must name an installed owner or an unconditional Route scope.
fn query_root_phase(
    installed: bool,
    overcover: bool,
    source_conditions: bool,
    amended: bool,
) -> Option<Phase> {
    match (installed, overcover, source_conditions, amended) {
        (true, _, _, _) => Some(Phase::Apply),
        (false, true, false, _) => Some(Phase::Route),
        (false, false, _, false) => Some(Phase::Apply),
        _ => None,
    }
}

fn query_root_matches<const N: usize>(
    record: &CompactDomain<N>,
    expected_phase: Option<Phase>,
    query: &Cell,
    first_original: bool,
    containment: &Containment,
) -> bool {
    if expected_phase != Some(record.phase()) {
        return false;
    }
    let recorded = ccell(record);
    if first_original {
        recorded == *query
    } else {
        containment.contains(&recorded, query)
    }
}

fn sample(candidates: &[usize], count: usize, seed: u64) -> Vec<usize> {
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut pool = candidates.to_vec();
    let count = count.min(pool.len());
    for index in 0..count {
        let pick = index + (next() % (pool.len() - index) as u64) as usize;
        pool.swap(index, pick);
    }
    pool.truncate(count);
    pool.sort_unstable();
    pool
}

/// What a mutation will change, found before any state is modified.
enum Plan {
    NotApplicable(String),
    DropEdge {
        source: usize,
        target: usize,
        successor: String,
    },
    Retarget {
        source: usize,
        from: usize,
        to: usize,
        successor: String,
    },
    Node {
        id: usize,
        depth: Option<usize>,
    },
    Query {
        index: usize,
        to: usize,
    },
    /// Partial records re-anchored: (partial, old anchor, new anchor); the
    /// anchor edge moves with the link.
    Relink(Vec<(usize, usize, usize)>),
    Whole,
    G2(G2Edit),
}

/// G2' mutations (records and edges).
enum G2Edit {
    /// Residual D band of `id` loses its highest layer (None: now empty).
    Shrink {
        id: usize,
        to: Option<(i64, i64)>,
    },
    /// Anchor slot `slot` of `id` replaced by record `to` (stamp = its true
    /// position, recorded kind `kind`); the edge moves with it.
    Replace {
        id: usize,
        slot: usize,
        to: usize,
        kind: &'static str,
    },
    DropEdge {
        id: usize,
        anchor: usize,
    },
    Cycle {
        a: usize,
        b: usize,
    },
}

/// BFS depth of every node from the query records (None: unreachable).
fn depths(graph: &Graph, roots: impl IntoIterator<Item = usize>) -> Vec<Option<usize>> {
    let mut depth = vec![None; graph.nodes()];
    let mut queue = std::collections::VecDeque::new();
    for root in roots {
        if root < depth.len() && depth[root].is_none() {
            depth[root] = Some(0);
            queue.push_back(root);
        }
    }
    while let Some(node) = queue.pop_front() {
        let next = depth[node].map(|d| d + 1);
        for &target in graph.out(node) {
            if depth[target as usize].is_none() {
                depth[target as usize] = next;
                queue.push_back(target as usize);
            }
        }
    }
    depth
}

fn describe<const N: usize>(domain: &Domain<N>) -> String {
    format!(
        "{:?} {} {:?}..{:?} r{:?} {:?}",
        domain.phase,
        mask(&domain.owner),
        domain.lower,
        domain.upper,
        domain.rank,
        domain.powers
    )
}

fn plan_mutation<const N: usize>(
    ctx: &Ctx<'_, N>,
    kind: OwnerDomainWalkVerifyMutation,
    queries: &[matching::input::Query],
) -> Plan {
    use OwnerDomainWalkVerifyMutation as M;
    let nodes = &ctx.loaded.nodes;
    let domains = &ctx.loaded.domains;
    let flags = &ctx.loaded.raw.flags;
    let flagged_sealed = |id: usize| flags.get(id).is_some_and(|f| f & FLAG_SEALED != 0);
    let initial_count = ctx.loaded.raw.counters[10].min(nodes.len());
    let roots = || {
        ctx.loaded
            .raw
            .inputs
            .iter()
            .filter_map(|e| e["domain"].as_u64().map(|r| r as usize))
    };
    // A sealed Apply native at depth >= 2 from every root, so that a root
    // can only change closure through transitive propagation.
    let deep_native = |extra: &dyn Fn(usize) -> bool| {
        let depth = depths(ctx.graph, roots());
        (initial_count..nodes.len())
            .filter(|&id| {
                nodes[id].kind == Kind::Native
                    && flagged_sealed(id)
                    && domains[id].phase() == Phase::Apply
                    && depth[id].is_some_and(|d| d >= 2)
                    && extra(id)
            })
            .min_by_key(|&id| (std::cmp::Reverse(depth[id]), id))
            .map_or(
                Plan::NotApplicable("no sealed Apply native at depth >= 2".into()),
                |id| Plan::Node {
                    id,
                    depth: depth[id],
                },
            )
    };
    match kind {
        M::DroppedEdge
        | M::InjectedFalseHit
        | M::AliasChainDetour
        | M::DroppedRoutedEdge
        | M::RoutedFalseHit => {
            // Routed variants: a Route native's admitted Apply domain.
            let routed = matches!(kind, M::DroppedRoutedEdge | M::RoutedFalseHit);
            // Representatives of aliases, for the detour.
            let mut aliases_of = BTreeMap::<usize, Vec<usize>>::new();
            if kind == M::AliasChainDetour {
                for (id, node) in nodes.iter().enumerate() {
                    if node.kind == Kind::Alias && node.link < nodes.len() {
                        aliases_of.entry(node.link).or_default().push(id);
                    }
                }
            }
            // The first native (by ID) with an admitted domain covered by
            // exactly one recorded target: that edge is load-bearing.
            for id in (0..nodes.len())
                .filter(|&id| {
                    nodes[id].native()
                        && !ctx.graph.out(id).is_empty()
                        && (!routed || domains[id].phase() == Phase::Route)
                })
                .take(if kind == M::AliasChainDetour {
                    200_000
                } else {
                    20_000
                })
            {
                if kind == M::AliasChainDetour
                    && !ctx
                        .graph
                        .out(id)
                        .iter()
                        .any(|t| aliases_of.contains_key(&(*t as usize)))
                {
                    continue;
                }
                let found = reference(ctx, id);
                if found.error {
                    continue;
                }
                for (successor, target) in found.admits {
                    if routed && successor.phase != Phase::Apply {
                        continue;
                    }
                    let label = describe(&successor);
                    let inner = cell(&successor);
                    match kind {
                        M::DroppedEdge | M::DroppedRoutedEdge => {
                            return Plan::DropEdge {
                                source: id,
                                target,
                                successor: label,
                            };
                        }
                        M::InjectedFalseHit | M::RoutedFalseHit => {
                            // A native non-container: no alias chain can
                            // re-cover the successor through it.
                            let replacement = (0..nodes.len())
                                .filter(|&t| {
                                    t != target && nodes[t].native() && !ctx.graph.has_edge(id, t)
                                })
                                .filter(|&t| {
                                    domains[t].phase() == successor.phase
                                        && domains[t].owner() == successor.owner
                                })
                                .filter(|&t| !ccell(&domains[t]).contains(&inner))
                                .max_by_key(|&t| {
                                    (
                                        flags.get(t).is_some_and(|f| f & FLAG_CLOSED != 0),
                                        flagged_sealed(t),
                                        std::cmp::Reverse(t),
                                    )
                                });
                            if let Some(to) = replacement {
                                return Plan::Retarget {
                                    source: id,
                                    from: target,
                                    to,
                                    successor: label,
                                };
                            }
                        }
                        _ => {
                            // Detour through an alias of the target that does
                            // not itself contain the successor.
                            let detour = aliases_of.get(&target).and_then(|aliases| {
                                aliases.iter().copied().find(|&a| {
                                    a != id
                                        && !ctx.graph.has_edge(id, a)
                                        && !ccell(&domains[a]).contains(&inner)
                                })
                            });
                            if let Some(to) = detour {
                                return Plan::Retarget {
                                    source: id,
                                    from: target,
                                    to,
                                    successor: label,
                                };
                            }
                        }
                    }
                }
            }
            Plan::NotApplicable(format!("no load-bearing edge found for {}", kind.name()))
        }
        M::SelfAnchoredPartial
        | M::PartialAsAnchor
        | M::PartialAnchorCycle
        | M::NonInitialAnchor => {
            // Partials whose anchor edge carries only the D >= cut slice (no
            // residual successor leans on it alone), so that moving the edge
            // changes nothing F10 sees: only the anchor rules can fire.
            let wanted = if kind == M::PartialAnchorCycle { 2 } else { 1 };
            let mut chosen = Vec::new();
            for id in (0..nodes.len()).filter(|&id| nodes[id].kind == Kind::Partial) {
                let from = nodes[id].link;
                if from >= nodes.len() || !ctx.graph.has_edge(id, from) {
                    continue;
                }
                let found = reference(ctx, id);
                if found.error || found.admits.iter().any(|(_, only)| *only == from) {
                    continue;
                }
                chosen.push(id);
                if chosen.len() == wanted {
                    break;
                }
            }
            if chosen.len() < wanted {
                return Plan::NotApplicable(format!(
                    "fewer than {wanted} partial records whose anchor edge carries only the D >= cut slice"
                ));
            }
            let id = chosen[0];
            let from = nodes[id].link;
            match kind {
                M::SelfAnchoredPartial => Plan::Relink(vec![(id, from, id)]),
                M::PartialAnchorCycle => {
                    let other = chosen[1];
                    Plan::Relink(vec![(id, from, other), (other, nodes[other].link, id)])
                }
                M::PartialAsAnchor => {
                    // Prefer an earlier partial (link < id holds; only the
                    // initial-record and Native rules can fire).
                    (0..nodes.len())
                        .filter(|&q| {
                            q != id && nodes[q].kind == Kind::Partial && !ctx.graph.has_edge(id, q)
                        })
                        .max_by_key(|&q| (q < id, std::cmp::Reverse(q.abs_diff(id))))
                        .map_or(
                            Plan::NotApplicable("no second partial record".into()),
                            |q| Plan::Relink(vec![(id, from, q)]),
                        )
                }
                _ => {
                    // A non-initial same-owner Apply native, preferably later
                    // and containing the D >= cut slice (only the order rule
                    // can fire; containment is not judged against it).
                    let high = high_slice(ccell(&domains[id]), nodes[id].cut);
                    (initial_count..nodes.len())
                        .filter(|&t| {
                            t != id
                                && nodes[t].kind == Kind::Native
                                && domains[t].phase() == Phase::Apply
                                && domains[t].owner() == domains[id].owner()
                                && !ctx.graph.has_edge(id, t)
                        })
                        .max_by_key(|&t| {
                            (
                                t > id,
                                ccell(&domains[t]).contains(&high),
                                std::cmp::Reverse(t),
                            )
                        })
                        .map_or(
                            Plan::NotApplicable("no non-initial same-owner Apply native".into()),
                            |t| Plan::Relink(vec![(id, from, t)]),
                        )
                }
            }
        }
        M::RoutePartial => (0..nodes.len())
            .find(|&id| {
                // No alias rests on it (an alias containment check would
                // also see the phase change).
                nodes[id].kind == Kind::Partial
                    && !nodes.iter().any(|n| n.kind == Kind::Alias && n.link == id)
            })
            .map_or(
                Plan::NotApplicable("no partial record without aliases".into()),
                |id| Plan::Node { id, depth: None },
            ),
        M::ShrunkResidual => (0..nodes.len())
            .find(|&id| {
                // The D = cut - 1 layer must hold points of Q.
                let node = nodes[id];
                node.kind == Kind::Partial
                    && ctx.loaded.residuals.contains_key(&id)
                    && !high_slice(low_slice(ccell(&domains[id]), node.cut), node.cut - 1)
                        .is_empty()
            })
            .map_or(
                Plan::NotApplicable("no partial record with a nonempty D = cut - 1 layer".into()),
                |id| Plan::Node { id, depth: None },
            ),
        M::G2ShrunkResidual
        | M::G2LateAnchor
        | M::G2InadmissibleAnchor
        | M::G2DroppedAnchorEdge
        | M::G2AnchorCycle => plan_g2_mutation(ctx, kind),
        M::MiscountedRouteEvents => (0..nodes.len())
            .find(|&id| {
                nodes[id].kind == Kind::Native
                    && flagged_sealed(id)
                    && domains[id].phase() == Phase::Route
                    && nodes[id].events.is_some()
            })
            .map_or(Plan::NotApplicable("no sealed Route native".into()), |id| {
                Plan::Node { id, depth: None }
            }),
        M::RetargetedAlias => {
            for id in (0..nodes.len()).filter(|&id| nodes[id].kind == Kind::Alias) {
                let inner = ccell(&domains[id]);
                let from = nodes[id].link;
                if let Some(to) = (id + 1..nodes.len()).find(|&t| {
                    t != from
                        && domains[t].phase() == domains[id].phase()
                        && domains[t].owner() == domains[id].owner()
                        && !ccell(&domains[t]).contains(&inner)
                }) {
                    return Plan::Retarget {
                        source: id,
                        from,
                        to,
                        successor: "alias domain".into(),
                    };
                }
            }
            Plan::NotApplicable("no alias with a same-bucket non-container".into())
        }
        M::RetargetedAnchor => {
            for id in (0..nodes.len()).filter(|&id| nodes[id].kind == Kind::Partial) {
                let from = nodes[id].link;
                // The anchor edge must carry only the D >= cut slice: skip a
                // partial whose residual successors lean on it alone.
                let found = reference(ctx, id);
                if found.error || found.admits.iter().any(|(_, only)| *only == from) {
                    continue;
                }
                let high = high_slice(ccell(&domains[id]), nodes[id].cut);
                if let Some(to) = (0..initial_count).find(|&t| {
                    t != from
                        && nodes[t].native()
                        && domains[t].phase() == Phase::Apply
                        && !ctx.graph.has_edge(id, t)
                        && !ccell(&domains[t]).contains(&high)
                }) {
                    return Plan::Retarget {
                        source: id,
                        from,
                        to,
                        successor: "partial D >= cut slice".into(),
                    };
                }
            }
            Plan::NotApplicable("no partial record with a non-containing initial native".into())
        }
        M::DroppedFrontierRecord | M::HiddenFrontier => (0..nodes.len())
            .find(|&id| {
                nodes[id].kind == Kind::Native && nodes[id].frontiers > 0 && !nodes[id].error
            })
            .map_or(
                Plan::NotApplicable("no native record carries a frontier".into()),
                |id| Plan::Node { id, depth: None },
            ),
        M::SealWithFrontier => (0..nodes.len())
            .find(|&id| nodes[id].native() && nodes[id].frontiers > 0)
            .or_else(|| (0..nodes.len()).find(|&id| nodes[id].native() && flagged_sealed(id)))
            .map_or(Plan::NotApplicable("no native record".into()), |id| {
                Plan::Node { id, depth: None }
            }),
        M::SealWithError => deep_native(&|_| true),
        M::HiddenError => deep_native(&|id| {
            // Its reference inspection must reach the native-operation
            // allowance: an Apply native that emits at least one event.
            nodes[id].events.is_some_and(|e| e > 1)
        }),
        M::MiscountedEvents => (0..nodes.len())
            .find(|&id| nodes[id].native() && flagged_sealed(id) && nodes[id].events.is_some())
            .map_or(Plan::NotApplicable("no sealed native".into()), |id| {
                Plan::Node { id, depth: None }
            }),
        M::MiscountedSuccessors => (0..nodes.len())
            .find(|&id| {
                nodes[id].native()
                    && flagged_sealed(id)
                    && domains[id].phase() == Phase::Apply
                    && nodes[id].successors.is_some()
            })
            .map_or(Plan::NotApplicable("no sealed Apply native".into()), |id| {
                Plan::Node { id, depth: None }
            }),
        M::RemappedQuery => {
            for (index, query) in queries.iter().enumerate() {
                let inner = query_cell::<N>(query);
                let current = ctx
                    .loaded
                    .raw
                    .inputs
                    .get(index)
                    .and_then(|e| e["domain"].as_u64());
                if let Some(to) = (0..initial_count).find(|&t| {
                    Some(t as u64) != current
                        && nodes[t].kind != Kind::Missing
                        && !ccell(&domains[t]).contains(&inner)
                }) {
                    return Plan::Query { index, to };
                }
            }
            Plan::NotApplicable("no query with a non-containing initial record".into())
        }
        M::ForeignRequest | M::ForeignOwners => Plan::Whole,
        M::MismatchedResult => (0..nodes.len())
            .find(|&id| nodes[id].kind != Kind::Missing && !ctx.loaded.digests.is_empty())
            .map_or(
                Plan::NotApplicable("no committed record or no --result to bind".into()),
                |id| Plan::Node { id, depth: None },
            ),
    }
}

/// Applies `plan`; returns the report and, for HiddenError, the node whose
/// reference inspection must fail.
fn plan_g2_mutation<const N: usize>(ctx: &Ctx<'_, N>, kind: OwnerDomainWalkVerifyMutation) -> Plan {
    use OwnerDomainWalkVerifyMutation as M;
    let loaded = ctx.loaded;
    let (nodes, domains, g2) = (&loaded.nodes, &loaded.domains, &loaded.g2);
    let with_anchor = || g2.iter().find(|(_, info)| !info.anchors.is_empty());
    let same_bucket = |x: usize, y: usize| {
        domains[x].phase() == domains[y].phase() && domains[x].owner() == domains[y].owner()
    };
    match kind {
        M::G2ShrunkResidual => {
            let pick = g2
                .iter()
                .filter_map(|(&id, info)| {
                    let r = info.residual?;
                    Some((id, r.min_power_difference?, r.max_power_difference?))
                })
                .min_by_key(|&(id, lo, hi)| (lo == hi, id));
            match pick {
                Some((id, lo, hi)) => Plan::G2(G2Edit::Shrink {
                    id,
                    to: (lo < hi).then_some((lo, hi - 1)),
                }),
                None => Plan::NotApplicable("no G2' record with a residual".into()),
            }
        }
        M::G2LateAnchor => {
            for (&id, info) in g2.iter().filter(|(_, info)| !info.anchors.is_empty()) {
                let late = (0..nodes.len()).find(|&b| {
                    b != id
                        && nodes[b].kind == Kind::Native
                        && same_bucket(b, id)
                        && loaded.positions[b] != u64::MAX
                        && loaded.positions[b] >= info.snapshot
                });
                if let Some(to) = late {
                    return Plan::G2(G2Edit::Replace {
                        id,
                        slot: 0,
                        to,
                        kind: "native",
                    });
                }
            }
            Plan::NotApplicable("no same-bucket Native record merged after a G2' snapshot".into())
        }
        M::G2InadmissibleAnchor => {
            for (&id, info) in g2.iter().filter(|(_, info)| !info.anchors.is_empty()) {
                let early = |b: usize| {
                    b != id
                        && loaded.positions[b] != u64::MAX
                        && loaded.positions[b] < info.snapshot
                };
                let inadmissible = (0..nodes.len())
                    .filter(|&b| {
                        early(b)
                            && match nodes[b].kind {
                                Kind::Alias => true,
                                Kind::G2 => g2.get(&b).is_some_and(|i| i.residual.is_none()),
                                Kind::Native => domains[b].phase() == Phase::Route,
                                _ => false,
                            }
                    })
                    .min_by_key(|&b| (nodes[b].kind != Kind::Alias, !same_bucket(b, id), b));
                if let Some(to) = inadmissible {
                    return Plan::G2(G2Edit::Replace {
                        id,
                        slot: 0,
                        to,
                        kind: "native",
                    });
                }
            }
            Plan::NotApplicable("no alias, G2' full cover or Route record before a snapshot".into())
        }
        M::G2DroppedAnchorEdge => {
            // Prefer a G2' full cover: it makes no native call, so its anchor
            // edges carry no successor coverage and only the anchor rules fire.
            let full_cover = g2
                .iter()
                .find(|(_, info)| info.residual.is_none() && !info.anchors.is_empty())
                .map(|(&id, info)| (id, info.anchors[0].id));
            match full_cover.or_else(|| with_anchor().map(|(&id, info)| (id, info.anchors[0].id))) {
                Some((id, anchor)) => Plan::G2(G2Edit::DropEdge { id, anchor }),
                None => Plan::NotApplicable("no G2' record with an anchor".into()),
            }
        }
        M::G2AnchorCycle => {
            let residual: Vec<usize> = g2
                .iter()
                .filter(|(_, info)| info.residual.is_some())
                .map(|(&id, _)| id)
                .collect();
            let pair = residual.iter().enumerate().find_map(|(i, &a)| {
                residual[i + 1..]
                    .iter()
                    .find(|&&b| same_bucket(a, b))
                    .map(|&b| (a, b))
            });
            let pair = pair.or_else(|| match residual[..] {
                [a, b, ..] => Some((a, b)),
                _ => None,
            });
            match pair {
                Some((a, b)) => Plan::G2(G2Edit::Cycle { a, b }),
                None => Plan::NotApplicable("fewer than two G2' residual records".into()),
            }
        }
        _ => unreachable!("G2' mutation kind"),
    }
}

fn apply_g2_mutation<const N: usize>(loaded: &mut Loaded<N>, edit: G2Edit, report: &mut Value) {
    let positions = &loaded.positions;
    let kind_of = |nodes: &[Node], g2: &BTreeMap<usize, G2Info>, b: usize| -> &'static str {
        match nodes[b].kind {
            Kind::Partial => "initial_d_band",
            Kind::G2 if g2.get(&b).is_some_and(|i| i.residual.is_some()) => "g2_residual",
            _ => "native",
        }
    };
    match edit {
        G2Edit::Shrink { id, to } => {
            let info = loaded.g2.get_mut(&id).expect("planned G2' record");
            let from = info
                .residual
                .map(|r| (r.min_power_difference, r.max_power_difference));
            match to {
                Some((lo, hi)) => {
                    let residual = info.residual.as_mut().expect("residual");
                    residual.min_power_difference = Some(lo);
                    residual.max_power_difference = Some(hi);
                }
                None => info.residual = None,
            }
            report["node"] = json!(id);
            report["residual_d_band"] = json!({"from": from, "to": to});
        }
        G2Edit::Replace { id, slot, to, kind } => {
            let info = loaded.g2.get_mut(&id).expect("planned G2' record");
            let from = info.anchors[slot].id;
            info.anchors[slot] = G2AnchorRow {
                id: to,
                stamp: positions[to],
                kind: kind.to_owned(),
            };
            // The new anchor gets its edge; the old edge stays (it may also
            // carry successor coverage), so only the anchor rules can fire.
            loaded.raw.edges.push((id as u32, to as u32));
            report["node"] = json!(id);
            report["anchor_from"] = json!(from);
            report["anchor_to"] = json!(to);
            report["anchor_to_position"] = json!(positions[to]);
            report["snapshot"] = json!(info.snapshot);
        }
        G2Edit::DropEdge { id, anchor } => {
            loaded
                .raw
                .edges
                .retain(|&(s, t)| (s as usize, t as usize) != (id, anchor));
            report["edge"] = json!([id, anchor]);
        }
        G2Edit::Cycle { a, b } => {
            for (x, y) in [(a, b), (b, a)] {
                let kind = kind_of(&loaded.nodes, &loaded.g2, y);
                let stamp = positions[y];
                loaded
                    .g2
                    .get_mut(&x)
                    .expect("planned G2' record")
                    .anchors
                    .push(G2AnchorRow {
                        id: y,
                        stamp,
                        kind: kind.to_owned(),
                    });
                loaded.raw.edges.push((x as u32, y as u32));
            }
            report["cycle"] = json!([a, b]);
        }
    }
}

fn apply_mutation<const N: usize>(
    loaded: &mut Loaded<N>,
    kind: OwnerDomainWalkVerifyMutation,
    plan: Plan,
    graph: &Graph,
) -> (Value, Option<usize>) {
    use OwnerDomainWalkVerifyMutation as M;
    let mut report = json!({"kind": kind.name(), "expected_verdict": kind.expected_verdict()});
    let mut failing = None;
    match plan {
        Plan::NotApplicable(reason) => {
            report["applied"] = json!(false);
            report["reason"] = json!(reason);
            return (report, None);
        }
        Plan::DropEdge {
            source,
            target,
            successor,
        } => {
            loaded
                .raw
                .edges
                .retain(|&(s, t)| (s as usize, t as usize) != (source, target));
            report["edge"] = json!([source, target]);
            report["successor"] = json!(successor);
        }
        Plan::Retarget {
            source,
            from,
            to,
            successor,
        } => {
            for edge in &mut loaded.raw.edges {
                if (edge.0 as usize, edge.1 as usize) == (source, from) {
                    edge.1 = to as u32;
                }
            }
            if matches!(kind, M::RetargetedAlias | M::RetargetedAnchor) {
                loaded.nodes[source].link = to;
            }
            report["edge"] = json!([source, from]);
            report["retargeted_to"] = json!(to);
            report["successor"] = json!(successor);
        }
        Plan::Relink(moves) => {
            for &(source, from, to) in &moves {
                for edge in &mut loaded.raw.edges {
                    if (edge.0 as usize, edge.1 as usize) == (source, from) {
                        edge.1 = to as u32;
                    }
                }
                loaded.nodes[source].link = to;
            }
            report["relinked"] = json!(
                moves
                    .iter()
                    .map(|&(partial, from, to)| json!({"partial": partial, "from": from, "to": to}))
                    .collect::<Vec<_>>()
            );
        }
        Plan::G2(edit) => apply_g2_mutation(loaded, edit, &mut report),
        Plan::Query { index, to } => {
            report["query_index"] = json!(index);
            report["from"] = loaded.raw.inputs[index]["domain"].clone();
            report["to"] = json!(to);
            loaded.raw.inputs[index]["domain"] = json!(to);
        }
        Plan::Whole => match kind {
            M::ForeignRequest => {
                loaded.raw.request = blake3::hash(b"foreign request").to_hex().to_string();
            }
            M::ForeignOwners => {
                if let Some(first) = loaded.raw.owners.first_mut() {
                    *first = blake3::hash(b"foreign owner").to_hex().to_string();
                }
            }
            _ => unreachable!("whole-checkpoint mutation kind"),
        },
        Plan::Node { id, depth } => {
            report["node"] = json!(id);
            report["depth_from_roots"] = json!(depth);
            if kind == M::RoutePartial {
                let mut domain = loaded.domains[id].expand();
                domain.phase = Phase::Route;
                loaded.domains[id] =
                    CompactDomain::try_from_domain(&domain).expect("same compact range");
                report["phase"] = json!("Route");
            }
            if kind == M::ShrunkResidual
                && let Some(bounds) = loaded.residuals.get_mut(&id)
            {
                let high = bounds
                    .max_power_difference
                    .unwrap_or(loaded.nodes[id].cut - 1);
                bounds.max_power_difference = Some(high - 1);
                report["residual_max_power_difference"] = json!([high, high - 1]);
            }
            let node = &mut loaded.nodes[id];
            let flag = &mut loaded.raw.flags[id];
            let counters = &mut loaded.raw.counters;
            match kind {
                M::DroppedFrontierRecord => {
                    report["dropped_frontiers"] = json!(node.frontiers);
                    node.frontiers = 0;
                    *flag |= FLAG_SEALED;
                }
                M::HiddenFrontier => {
                    report["dropped_frontiers"] = json!(node.frontiers);
                    counters[5] -= node.frontiers as usize;
                    node.frontiers = 0;
                    *flag |= FLAG_SEALED;
                    // The engine's closed flags and closure counters agree
                    // with the forged seal.
                    let sealed = sealed_with(&loaded.nodes, &loaded.g2, graph);
                    let closed = graph.closed(&sealed);
                    for (flag, &is_closed) in loaded.raw.flags.iter_mut().zip(&closed) {
                        if is_closed {
                            *flag |= FLAG_CLOSED;
                        } else {
                            *flag &= !FLAG_CLOSED;
                        }
                    }
                    let initial = loaded.raw.counters[10].min(closed.len());
                    let initial_closed = closed[..initial].iter().filter(|&&c| c).count();
                    loaded.raw.closure["initial_closed"] = json!(initial_closed);
                    report["engine_initial_closed_after"] = json!(initial_closed);
                }
                M::SealWithFrontier => {
                    if node.frontiers == 0 {
                        node.frontiers = 1;
                    }
                    *flag |= FLAG_SEALED;
                }
                M::SealWithError => {
                    node.error = true;
                    *flag |= FLAG_SEALED;
                }
                M::HiddenError => failing = Some(id),
                M::RoutePartial | M::ShrunkResidual => {}
                M::MiscountedEvents | M::MiscountedRouteEvents => {
                    node.events = node.events.map(|e| e + 1);
                    node.accepted = node.accepted.map(|e| e + 1);
                    counters[0] += 1;
                }
                M::MiscountedSuccessors => {
                    node.successors = node.successors.map(|s| s + 1);
                    counters[1] += 1;
                }
                M::MismatchedResult => loaded.digests[id][0] ^= 0xff,
                _ => unreachable!("node mutation kind"),
            }
        }
    }
    report["applied"] = json!(true);
    (report, failing)
}

#[cfg(test)]
mod target_index_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(lower: u64, upper: Option<u64>) -> Domain<2> {
        Domain {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![lower, 0],
            upper: vec![upper, Some(2)],
            rank: Some(2),
            powers: DomainPowerBounds::default(),
        }
    }

    fn compact(domains: &[Domain<2>]) -> Vec<CompactDomain<2>> {
        domains
            .iter()
            .map(|d| CompactDomain::try_from_domain(d).unwrap())
            .collect()
    }

    #[test]
    fn query_root_phase_distinguishes_originals_amendments_and_source_refusals() {
        for overcover in [false, true] {
            for source_conditions in [false, true] {
                for amended in [false, true] {
                    assert_eq!(
                        query_root_phase(true, overcover, source_conditions, amended),
                        Some(Phase::Apply)
                    );
                }
            }
        }
        for (overcover, source_conditions, amended, expected) in [
            (false, false, false, Some(Phase::Apply)),
            (false, true, false, Some(Phase::Apply)),
            (true, false, false, Some(Phase::Route)),
            (true, true, false, None),
            (false, false, true, None),
            (false, true, true, None),
            (true, false, true, Some(Phase::Route)),
            (true, true, true, None),
        ] {
            assert_eq!(
                query_root_phase(false, overcover, source_conditions, amended),
                expected
            );
        }
    }

    #[test]
    fn query_root_mapping_rejects_wrong_phase_even_with_identical_geometry() {
        let containment = Containment::new(256, 4096);
        let apply = domain(0, Some(3));
        let mut route = apply.clone();
        route.phase = Phase::Route;
        let records = compact(&[apply, route]);
        let query = ccell(&records[0]);
        for first_original in [false, true] {
            for (id, phase) in [(0, Phase::Apply), (1, Phase::Route)] {
                assert!(query_root_matches(
                    &records[id],
                    Some(phase),
                    &query,
                    first_original,
                    &containment
                ));
                assert!(!query_root_matches(
                    &records[1 - id],
                    Some(phase),
                    &query,
                    first_original,
                    &containment
                ));
                assert!(!query_root_matches(
                    &records[id],
                    None,
                    &query,
                    first_original,
                    &containment
                ));
            }
        }
        let mut smaller = query.clone();
        smaller.lower[0] = 1;
        assert!(!query_root_matches(
            &records[0],
            Some(Phase::Apply),
            &smaller,
            true,
            &containment
        ));
        assert!(query_root_matches(
            &records[0],
            Some(Phase::Apply),
            &smaller,
            false,
            &containment
        ));
    }

    fn row(id: usize, kind: &str, domain: &Domain<2>, extra: Value) -> RecordRow {
        let mut value = json!({"id":id,"record_kind":kind,"phase":"Apply","owner":mask(&domain.owner),
            "lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
            "power_bounds":{"max_positive_power":null,"min_power_difference":null,"max_power_difference":null},
            "error":null,"frontiers":[],"local_inspection_finished":true,"stats":{"events":3,"successors":2}});
        for (key, field) in extra.as_object().unwrap() {
            value[key] = field.clone();
        }
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn records_seal_only_under_the_f8_rule_and_with_their_links() {
        let domains = vec![
            domain(0, Some(3)),
            domain(1, Some(2)),
            domain(0, None),
            domain(2, Some(2)),
            domain(1, Some(1)),
        ];
        let saved = compact(&domains);
        let mut nodes = vec![Node::MISSING; 5];
        let mut residuals = BTreeMap::new();
        let mut g2 = BTreeMap::new();
        let mut violations = Violations::new(50);
        record_node(
            &row(0, "native_inspection", &domains[0], json!({})),
            &saved,
            &mut nodes,
            &mut residuals,
            &mut g2,
            &mut violations,
        );
        record_node(
            &row(
                1,
                "delegated_not_inspected",
                &domains[1],
                json!({"representative_id":2}),
            ),
            &saved,
            &mut nodes,
            &mut residuals,
            &mut g2,
            &mut violations,
        );
        record_node(
            &row(
                2,
                "native_inspection",
                &domains[2],
                json!({"frontiers":[{"kind":"x"}]}),
            ),
            &saved,
            &mut nodes,
            &mut residuals,
            &mut g2,
            &mut violations,
        );
        record_node(
            &row(
                3,
                "native_inspection",
                &domains[3],
                json!({"error":"boom","accepted_events":4}),
            ),
            &saved,
            &mut nodes,
            &mut residuals,
            &mut g2,
            &mut violations,
        );
        assert!(violations.by_class.contains_key("accepted_events"));
        let mut wrong = row(4, "native_inspection", &domains[4], json!({}));
        wrong.upper = vec![Some(5), Some(2)];
        record_node(
            &wrong,
            &saved,
            &mut nodes,
            &mut residuals,
            &mut g2,
            &mut violations,
        );
        assert!(violations.by_class.contains_key("domain_parity"));
        let graph = Graph::from_edges(5, &[(0, 1), (1, 2)]).unwrap();
        assert_eq!(sealed(&nodes, &graph), [true, true, false, false, true]);
        // Without its representative edge an alias does not seal.
        let bare = Graph::from_edges(5, &[(0, 1)]).unwrap();
        assert!(!sealed(&nodes, &bare)[1]);
        // Root 0 reaches the frontier-bearing node 2 through the alias.
        assert_eq!(
            graph.closed(&sealed(&nodes, &graph)),
            [false, false, false, false, true]
        );
    }

    #[test]
    fn alias_chains_cover_only_through_representatives_that_contain_the_domain() {
        // 0: alias of 1, 1: alias of 2, 2: native [0..5]; 3: native [0..1].
        let domains = vec![
            domain(0, Some(1)),
            domain(0, Some(2)),
            domain(0, Some(5)),
            domain(0, Some(1)),
        ];
        let mut nodes = vec![Node::MISSING; 4];
        nodes[0] = Node {
            kind: Kind::Alias,
            link: 1,
            ..Node::MISSING
        };
        nodes[1] = Node {
            kind: Kind::Alias,
            link: 2,
            ..Node::MISSING
        };
        nodes[2].kind = Kind::Native;
        nodes[3].kind = Kind::Native;
        let saved = compact(&domains);
        let containment = Containment::new(4096, 1 << 20);
        let target = |id: usize| (id, Phase::Apply, cell(&domains[id]));
        let wide = cell(&domain(3, Some(4)));
        // Reached two hops down the chain from target 0.
        assert!(alias_chain_covers(
            &nodes,
            &saved,
            &containment,
            &[target(0)],
            Phase::Apply,
            &wide
        ));
        // A native target has no chain; the other phase never covers.
        assert!(!alias_chain_covers(
            &nodes,
            &saved,
            &containment,
            &[target(3)],
            Phase::Apply,
            &wide
        ));
        assert!(!alias_chain_covers(
            &nodes,
            &saved,
            &containment,
            &[target(0)],
            Phase::Route,
            &wide
        ));
        // Nothing along the chain contains a domain beyond the representative.
        let beyond = cell(&domain(4, Some(6)));
        assert!(!alias_chain_covers(
            &nodes,
            &saved,
            &containment,
            &[target(0)],
            Phase::Apply,
            &beyond
        ));
        // A broken chain (dangling link) stops without covering.
        nodes[1].link = usize::MAX;
        assert!(!alias_chain_covers(
            &nodes,
            &saved,
            &containment,
            &[target(0)],
            Phase::Apply,
            &wide
        ));
        assert_eq!(containment.disagreements(), 0);
    }

    /// Each partial rule in isolation on synthetic records (the engine never
    /// writes most of these states, so only a forged table exercises them):
    /// owner axis x0 (A = x0 + 1), other axis x1 (R = x1), D = x0 + 1 - x1.
    #[test]
    fn partial_records_need_an_apply_phase_a_well_founded_anchor_and_an_exact_cover() {
        let cell_domain = |phase: Phase, upper0: u64, min_d: Option<i64>| Domain {
            phase,
            owner: [true, false],
            lower: vec![0, 0],
            upper: vec![Some(upper0), Some(2)],
            rank: Some(2),
            powers: DomainPowerBounds {
                max_positive_power: None,
                min_power_difference: min_d,
                max_power_difference: None,
            },
        };
        // 0, 1: initial natives (0 contains the D >= 2 slice of 3; 1 does
        // not); 2: a non-initial native (contains it); 3: the partial
        // under test, cut 2; 4: another partial.
        let domains = vec![
            cell_domain(Phase::Apply, 5, Some(2)),
            cell_domain(Phase::Apply, 1, Some(2)),
            cell_domain(Phase::Apply, 6, Some(2)),
            cell_domain(Phase::Apply, 3, None),
            cell_domain(Phase::Apply, 3, None),
        ];
        let initial_count = 2;
        let base = |kind: Kind, link: usize| Node {
            kind,
            link,
            cut: 2,
            finished: true,
            ..Node::MISSING
        };
        let nodes = vec![
            base(Kind::Native, usize::MAX),
            base(Kind::Native, usize::MAX),
            base(Kind::Native, usize::MAX),
            base(Kind::Partial, 0),
            base(Kind::Partial, 0),
        ];
        let exact = residual(domains[3].powers, 2);
        let residuals = BTreeMap::from([(3, exact), (4, exact)]);
        let classes = |nodes: &[Node],
                       domains: &[Domain<2>],
                       residuals: &BTreeMap<usize, DomainPowerBounds>,
                       edges: &[(u32, u32)]| {
            let graph = Graph::from_edges(nodes.len(), edges).unwrap();
            let containment = Containment::new(4096, 1 << 20);
            let mut violations = Violations::new(50);
            check_partial(
                3,
                nodes,
                &compact(domains),
                residuals,
                &graph,
                &containment,
                initial_count,
                &mut violations,
            );
            assert_eq!(containment.disagreements(), 0);
            violations.by_class
        };
        let set = |names: &[(&'static str, u64)]| names.iter().copied().collect::<BTreeMap<_, _>>();
        // Valid: Apply, earlier initial Native anchor containing the slice,
        // exact residual, anchor edge.
        assert_eq!(classes(&nodes, &domains, &residuals, &[(3, 0)]), set(&[]));
        // No anchor edge.
        assert_eq!(
            classes(&nodes, &domains, &residuals, &[]),
            set(&[("missing_edge", 1)])
        );
        // Route partial (the phaseless cells still nest): phase rule only.
        let mut route = domains.clone();
        route[3].phase = Phase::Route;
        assert_eq!(
            classes(&nodes, &route, &residuals, &[(3, 0)]),
            set(&[("partial_phase", 1)])
        );
        // Self-anchor: order and kind (containment is not judged).
        let mut relinked = nodes.clone();
        relinked[3].link = 3;
        assert_eq!(
            classes(&relinked, &domains, &residuals, &[(3, 3)]),
            set(&[("partial_anchor_kind", 1), ("partial_anchor_order", 1)])
        );
        // Anchored on another (later, non-initial) partial.
        relinked[3].link = 4;
        assert_eq!(
            classes(&relinked, &domains, &residuals, &[(3, 4)]),
            set(&[("partial_anchor_kind", 1), ("partial_anchor_order", 1)])
        );
        // Kind alone: an initial, earlier anchor that is itself a Partial.
        let mut partial_anchor = nodes.clone();
        partial_anchor[0].kind = Kind::Partial;
        assert_eq!(
            classes(&partial_anchor, &domains, &residuals, &[(3, 0)]),
            set(&[("partial_anchor_kind", 1)])
        );
        // Order alone: an earlier but non-initial Native containing the slice.
        relinked[3].link = 2;
        assert_eq!(
            classes(&relinked, &domains, &residuals, &[(3, 2)]),
            set(&[("partial_anchor_order", 1)])
        );
        // Containment: an initial Native that misses the slice; the union
        // cover fails with it.
        relinked[3].link = 1;
        assert_eq!(
            classes(&relinked, &domains, &residuals, &[(3, 1)]),
            set(&[("partial_anchor", 1), ("partial_union_cover", 1)])
        );
        // Residual shrunk by one D layer: identity and union cover fail.
        let mut short = residuals.clone();
        short.get_mut(&3).unwrap().max_power_difference = Some(0);
        assert_eq!(
            classes(&nodes, &domains, &short, &[(3, 0)]),
            set(&[("partial_residual", 1), ("partial_union_cover", 1)])
        );
        // A wider recorded residual (overlapping the anchor) still covers:
        // only the identity rule fires.
        let mut wide = residuals.clone();
        wide.get_mut(&3).unwrap().max_power_difference = Some(3);
        assert_eq!(
            classes(&nodes, &domains, &wide, &[(3, 0)]),
            set(&[("partial_residual", 1)])
        );
    }

    #[test]
    fn partial_slices_and_residuals_split_the_power_difference() {
        let mut powers = DomainPowerBounds::default();
        powers.min_power_difference = Some(3);
        assert_eq!(residual(powers, 7).max_power_difference, Some(6));
        powers.max_power_difference = Some(5);
        assert_eq!(residual(powers, 7).max_power_difference, Some(5));
        let high = high_slice(cell(&domain(0, Some(3))), 7);
        assert_eq!(high.powers.min_power_difference, Some(7));
    }

    #[test]
    fn mutation_names_round_trip_and_samples_are_deterministic() {
        for kind in OwnerDomainWalkVerifyMutation::ALL {
            assert_eq!(
                OwnerDomainWalkVerifyMutation::parse(kind.name()),
                Some(kind)
            );
        }
        assert_eq!(
            OwnerDomainWalkVerifyMutation::AliasChainDetour.expected_verdict(),
            "PASS"
        );
        let pool: Vec<usize> = (0..100).collect();
        assert_eq!(sample(&pool, 10, 3), sample(&pool, 10, 3));
        assert_eq!(sample(&pool, 10, 3).len(), 10);
        assert_eq!(sample(&pool, 1000, 3), pool);
        let graph = Graph::from_edges(4, &[(0, 1), (1, 2), (3, 2)]).unwrap();
        assert_eq!(depths(&graph, [0]), [Some(0), Some(1), Some(2), None]);
    }

    #[test]
    fn reference_request_turns_native_levers_off_unless_as_run() {
        use crate::OwnerDomainMatchRequest;
        let mut run =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        let (reference, differing) =
            reference_request(&run, OwnerDomainWalkVerifyReferenceLevers::Off);
        assert!(!reference.route_joint_source_support_pruning && differing.is_empty());
        run.route_joint_source_support_pruning = true;
        let (reference, differing) =
            reference_request(&run, OwnerDomainWalkVerifyReferenceLevers::Off);
        assert!(!reference.route_joint_source_support_pruning);
        assert_eq!(differing, REFERENCE_NATIVE_LEVERS);
        let (reference, differing) =
            reference_request(&run, OwnerDomainWalkVerifyReferenceLevers::AsRun);
        assert!(reference.route_joint_source_support_pruning && differing.is_empty());
        // Everything that defines the obligations is kept as run.
        assert_eq!(reference.route_domain_overcover, run.route_domain_overcover);
        assert_eq!(reference.max_route_masks, run.max_route_masks);
        assert_eq!(
            OwnerDomainWalkVerifyOptions::new("").reference_levers,
            OwnerDomainWalkVerifyReferenceLevers::Off
        );
    }
}
