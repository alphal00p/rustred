//! EXACT gap partition of the top sector of a node (plan 2.4.2, WP-I3): from the case domains alone, the whole
//! top-sector chart orthant (every line >= 1, every numerator <= 0, all powers, all ranks, no window) is split into
//! finitely many classes of per-slot states, labelled Covered (not listed), Uncovered (U) or Fragile (F, covered
//! except on the zero sets of live exception branches); U classes give the GENERIC GAP FACES with per-slot
//! thresholds, F classes are resolved into coupled LOCI by [`crate::loci`].
//!
//! Two coverage notions (audit-part-A M3, [`CoverageNotion`]): `UnionCoverage` (some record APPLIES at the key; no
//! order) is what the classic partition computes; `DispatchCoverage` (the FIRST record SELECTED at the key in
//! dispatch order applies there) is what a reduction gets. Bridge (exact): a key is dispatch-uncovered iff it is
//! union-uncovered or, for some record `r`, it lies in `selected(r) \ applies(r)` (r is blocked there: pole locus,
//! recenter bound, rule-point range, elided witness loci) and in no `applies(s)` of an earlier record `s`. Dispatch
//! mode computes exactly that union of blocked sub-partitions. Interface: `plan/auto-campaign/interfaces/partition.md`.
//!
//! THE GOVERNING PRINCIPLE (audits part-A / part-B): every output is an exact statement or an explicit
//! `Unknown` / `Unexplored` / `Unresolved` / `Incomplete(reason)`; nothing sampled, capped, pruned, budget-aborted,
//! overflowed or excluded from the normal form is ever reported as covered, empty, contained, shadowed or complete.
//!
//! No floating point anywhere (test `partition_nofloat`). Every exact step is checked integer arithmetic; an overflow
//! or a work cap makes the affected literal test conservative (counted in `stats.unknown_literal_tests`), never a
//! decided verdict. Clippy refuses unchecked operators and truncating casts in every file of `partition` and `loci`
//! (`#![deny(clippy::arithmetic_side_effects, ...)]`, re-audit RA2-C1).
//!
//! ONE work budget per call (re-audit RA2-M5): [`Budget::max_nodes`] / [`Budget::wall`] bound the main recursion,
//! every blocked sub-partition and every locus resolution TOGETHER ([`WorkMeter`]); once exhausted nothing restarts
//! with a fresh budget (the remaining blocked sub-partitions are skipped and reported, the outcome is `Partial`).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

pub mod classes;
pub mod exact;
pub mod nf;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

use crate::cover::{Dispatcher, RuleRef};
use crate::domain::CaseDomain;
use crate::key::{Key, NodeFrame};
pub use crate::loci::contain::{
    Containment, Emptiness, ProofBudget, contained_in, contained_in_with, is_empty, is_empty_with,
    removal_preserves_dispatch, shadowed, shadowed_with,
};
pub use classes::{Class, EnumStats, Label, Node};
pub use exact::{ExactError, Range};
pub use nf::{NfRules, NfStats};

/// Errors of the partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartitionError {
    Arity {
        expected: usize,
        got: usize,
    },
    /// A domain's chart differs from the frame mask.
    ChartNotFrame {
        source: usize,
    },
    Exact(ExactError),
    Eval(String),
    Unsupported(String),
    /// A key outside the top-sector chart orthant was given to the oracle or the census.
    NotChartKey(Key),
    Io(String),
    /// A resumable checkpoint was written for OTHER inputs (tables, frame, options, order) or by an older format:
    /// refused (re-audit RA2-M4); delete it to start over.
    StaleCheckpoint(String),
}

impl std::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PartitionError {}

/// The case domains of a partition run, in dispatch order.
pub trait DomainSource {
    fn arity(&self) -> usize;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn domain(&self, i: usize) -> &CaseDomain;
    fn rule_ref(&self, i: usize) -> RuleRef;
    /// The verdict that blocks record `i` wherever it is selected because of its WITNESS STATUS (Elided /
    /// NotCarried without attestation), as the core dispatcher enforces it (`RecordMeta::witness_block`); `None` =
    /// Complete (or the check is off). A plain [`DomainList`] has no witness metadata: only `witness_elided` counts.
    fn witness_block(&self, _i: usize) -> Option<crate::domain::Verdict> {
        None
    }
}

/// A plain list of domains with their provenance.
#[derive(Clone, Debug, Default)]
pub struct DomainList {
    pub arity: usize,
    pub items: Vec<(RuleRef, CaseDomain)>,
}

impl DomainList {
    pub fn new(arity: usize) -> Self {
        DomainList {
            arity,
            items: vec![],
        }
    }
    pub fn push(&mut self, r: RuleRef, d: CaseDomain) {
        self.items.push((r, d));
    }
}

impl DomainSource for DomainList {
    fn arity(&self) -> usize {
        self.arity
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn domain(&self, i: usize) -> &CaseDomain {
        &self.items[i].1
    }
    fn rule_ref(&self, i: usize) -> RuleRef {
        self.items[i].0
    }
}

/// The dispatcher's tables flattened in its priority order (records in record order).
pub struct DispatcherSource<'a> {
    disp: &'a Dispatcher,
    index: Vec<(usize, usize)>,
}

impl<'a> DispatcherSource<'a> {
    pub fn new(disp: &'a Dispatcher) -> Self {
        let mut index = vec![];
        for &t in disp.priority() {
            for r in 0..disp.tables()[t].domains.len() {
                index.push((t, r));
            }
        }
        DispatcherSource { disp, index }
    }
}

impl DomainSource for DispatcherSource<'_> {
    fn arity(&self) -> usize {
        self.disp.arity()
    }
    fn len(&self) -> usize {
        self.index.len()
    }
    fn domain(&self, i: usize) -> &CaseDomain {
        let (t, r) = self.index[i];
        &self.disp.tables()[t].domains[r]
    }
    fn rule_ref(&self, i: usize) -> RuleRef {
        let (t, r) = self.index[i];
        self.disp.rule_ref(t, r)
    }
    fn witness_block(&self, i: usize) -> Option<crate::domain::Verdict> {
        let (t, r) = self.index[i];
        self.disp.tables()[t]
            .meta
            .get(r)
            .and_then(|m| m.witness_block())
    }
}

/// How a domain with elided witness loci is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ElidedPolicy {
    /// As an ordinary domain (the checker's h11b reading; regression reference ONLY: the report then carries a
    /// caveat and is never a coverage proof).
    Ordinary,
    /// FAIL CLOSED as the core's first-match dispatcher (ruling F1, R24 (6); the campaign's verdict): the record
    /// credits no key AND blocks every later record on its SELECTED set (the engine's search stops at a selected
    /// but unusable record): its blocked region `selected minus the earlier applies sets` is a separate exact
    /// sub-partition. Only elided records are modelled as blockers here; `CoverageNotion::DispatchCoverage` models
    /// every blocked set. T178 order B: 409 + 21 open box keys.
    FailClosed,
    /// Removed (skipped as if absent; later records still cover its keys). T178 order B: identical to `Ordinary`,
    /// because the elided record's domain also occurs in another piece.
    Absent,
}

/// The two coverage notions (audit-part-A M3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum CoverageNotion {
    /// Some record APPLIES at the key (its own verdict, no order). What the classic partition proves; NOT what a
    /// reduction gets when an earlier record is selected but blocked at the key.
    UnionCoverage,
    /// Under first-match dispatch a key is served iff its FIRST selected record (dispatch order) applies there. A
    /// record selected but blocked (pole locus, recenter bound, rule-point range of a target pattern, elided witness
    /// loci) SHADOWS every later record without serving. Dispatch-mode partitions model every such blocked set
    /// exactly (one blocked sub-partition per blocking record).
    DispatchCoverage,
}

impl CoverageNotion {
    pub fn tag(&self) -> &'static str {
        match self {
            CoverageNotion::UnionCoverage => "union",
            CoverageNotion::DispatchCoverage => "dispatch",
        }
    }
}

/// The `method` string of a coverage-preservation proof made by a DISPATCH-mode partition (audit-part-A M3 (c)):
/// [`dispatch_proof_method`] writes it, [`parse_dispatch_proof_method`] reads it. The economy refuses any other
/// method (a union-coverage partition cannot prove coverage preservation under first-match dispatch).
pub const DISPATCH_PROOF_METHOD: &str = "dispatch_partition.v1";

/// The dispatch order the proof was made with, relative to the old table set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchOrderStatement {
    /// The new table set is dispatched in the same order as the old one (records removed / replaced in place).
    SameOrder,
    /// The new table set is dispatched in the stated order (digest of the priority list, non-empty).
    NewOrder(String),
}

/// The `method` string of a dispatch-mode coverage proof: `dispatch_partition.v1;order=same` or
/// `dispatch_partition.v1;order=new:<digest>`. Write it only from a report for which
/// `proves_no_gap(CoverageNotion::DispatchCoverage)` is `Ok`.
pub fn dispatch_proof_method(order: &DispatchOrderStatement) -> String {
    match order {
        DispatchOrderStatement::SameOrder => format!("{DISPATCH_PROOF_METHOD};order=same"),
        DispatchOrderStatement::NewOrder(d) => format!("{DISPATCH_PROOF_METHOD};order=new:{d}"),
    }
}

/// Read a proof `method` string: `Some(order)` only for a dispatch-mode proof with an order statement.
pub fn parse_dispatch_proof_method(method: &str) -> Option<DispatchOrderStatement> {
    let rest = method
        .strip_prefix(DISPATCH_PROOF_METHOD)?
        .strip_prefix(";order=")?;
    if rest == "same" {
        return Some(DispatchOrderStatement::SameOrder);
    }
    let d = rest.strip_prefix("new:")?;
    (!d.is_empty() && !d.contains(char::is_whitespace))
        .then(|| DispatchOrderStatement::NewOrder(d.to_string()))
}

/// Which status tests multi-slot linear literals get.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LiteralTests {
    /// Exact integer feasibility for every linear literal (default).
    Exact,
    /// The checker's uenum_g statuses (regression of its class counts).
    ReferenceCompatible,
}

/// Work limits of ONE partition call (re-audit RA2-M5): they bound the main recursion, every blocked sub-partition
/// and every locus resolution together (one [`WorkMeter`]).
#[derive(Clone, Debug, Default)]
pub struct Budget {
    /// Work units of the whole call: one per recursion node (main and blocked sub-partitions) and one per locus
    /// resolution node / emptiness split node. Checked at every unit; deterministic.
    pub max_nodes: Option<u64>,
    /// Wall clock of the whole call (checked every [`WorkMeter::WALL_EVERY`] units; not deterministic).
    pub wall: Option<Duration>,
    /// Memo entries kept at most (memory bound; exactness unaffected).
    pub max_memo: Option<usize>,
    /// Face cells expanded at most (beyond: the face list stops, outcome `Incomplete`; audit-part-B M3).
    pub max_faces: Option<usize>,
}

/// The ONE deterministic work budget of a partition call (re-audit RA2-M5), shared by the main recursion, the
/// blocked sub-partitions and the locus resolution. Exhaustion is sticky: after it every consumer stops and reports
/// what it did not do (Unexplored classes, skipped sub-partitions, open regions); nothing restarts with a fresh
/// budget. The node budget is checked at every unit (exact, deterministic); the wall clock every
/// [`WorkMeter::WALL_EVERY`] units.
#[derive(Debug)]
pub struct WorkMeter {
    used: Cell<u64>,
    max: Option<u64>,
    deadline: Option<Instant>,
    exhausted: Cell<bool>,
    why: RefCell<Option<String>>,
}

impl WorkMeter {
    /// Granularity of the wall-clock check, in work units.
    pub const WALL_EVERY: u64 = 256;

    pub fn new(max: Option<u64>, deadline: Option<Instant>) -> Self {
        WorkMeter {
            used: Cell::new(0),
            max,
            deadline,
            exhausted: Cell::new(false),
            why: RefCell::new(None),
        }
    }
    pub fn unlimited() -> Self {
        WorkMeter::new(None, None)
    }
    /// The meter of a [`Budget`] started at `t0`.
    pub fn of_budget(b: &Budget, t0: Instant) -> Self {
        WorkMeter::new(b.max_nodes, b.wall.and_then(|w| t0.checked_add(w)))
    }
    fn exhaust(&self, why: String) {
        self.exhausted.set(true);
        let mut w = self.why.borrow_mut();
        if w.is_none() {
            *w = Some(why);
        }
    }
    /// Count `n` units; `false` = the budget is exhausted (now or before).
    pub fn tick(&self, n: u64) -> bool {
        if self.exhausted.get() {
            return false;
        }
        let before = self.used.get();
        let u = before.saturating_add(n);
        self.used.set(u);
        if let Some(m) = self.max
            && u > m
        {
            self.exhaust(format!(
                "work budget of {m} units exhausted (recursion nodes of the partition and its blocked sub-partitions, locus resolution nodes)"
            ));
            return false;
        }
        if let Some(d) = self.deadline
            && before.checked_div(Self::WALL_EVERY) != u.checked_div(Self::WALL_EVERY)
            && Instant::now() > d
        {
            self.exhaust("wall-clock budget exhausted".into());
            return false;
        }
        true
    }
    pub fn exhausted(&self) -> bool {
        self.exhausted.get()
    }
    pub fn used(&self) -> u64 {
        self.used.get()
    }
    pub fn reason(&self) -> Option<String> {
        self.why.borrow().clone()
    }
    /// The wall deadline (passed to the locus resolution for its own early stop).
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
}

/// Progress report (no floating point: milliseconds).
#[derive(Clone, Debug)]
pub struct Progress {
    pub nodes: u64,
    pub memo_size: u64,
    pub depth: usize,
    pub seconds_x1000: u64,
    pub finished_subtrees: u64,
    pub total_subtrees: u64,
}

/// Options of [`gap_partition`].
#[derive(Clone, Debug)]
pub struct PartitionOptions {
    pub elided: ElidedPolicy,
    pub literal_tests: LiteralTests,
    /// false = SLOW REFERENCE MODE (no memo).
    pub memo: bool,
    pub max_off_floor: Option<usize>,
    pub rule_subset: Option<Vec<usize>>,
    pub dedup: bool,
    pub factor_exceptions: bool,
    /// Resolve every F class into loci ([`crate::loci`]).
    pub resolve_loci: bool,
    pub loci: crate::loci::LociOptions,
    pub budget: Budget,
    /// Additional critical values per slot (DATA, e.g. to refine the states to a coarser reference granularity);
    /// `None` = only the critical values derived from the literals.
    pub extra_critical: Option<Vec<Vec<i64>>>,
    /// Which coverage the run computes (default `UnionCoverage`; `DispatchCoverage` models every blocked set and
    /// ignores `elided`: elided records always block in dispatch mode).
    pub coverage: CoverageNotion,
    /// A class with more `any` slots than this is not expanded into its 2^k faces: it is listed as a
    /// [`FaceFamily`] and the outcome is `Incomplete` (audit-part-A M4; no fixed limit, sized by the arity).
    pub max_any_slots: usize,
}

impl Default for PartitionOptions {
    fn default() -> Self {
        PartitionOptions {
            elided: ElidedPolicy::FailClosed,
            literal_tests: LiteralTests::Exact,
            memo: true,
            max_off_floor: None,
            rule_subset: None,
            dedup: true,
            factor_exceptions: false,
            resolve_loci: true,
            loci: crate::loci::LociOptions::default(),
            budget: Budget::default(),
            extra_critical: None,
            coverage: CoverageNotion::UnionCoverage,
            max_any_slots: 24,
        }
    }
}

/// How a face or locus is established.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Proof {
    /// By the exact partition (generic gaps; loci resolved by linear cuts only).
    ExactPartition,
    /// By an exact window enumeration (stated bound) only.
    Tested { bound: String },
    /// Lower bound from uncovered closure keys (R30 fallback).
    ClosureLowerBound,
    /// An OPEN gap region the exact resolution could not decide (cap, overflow, nonlinear cut, unexpanded class):
    /// reported as a gap, never as covered.
    Unresolved { reason: String },
}

impl Proof {
    pub fn tag(&self) -> &'static str {
        match self {
            Proof::ExactPartition => "exact_partition",
            Proof::Tested { .. } => "tested",
            Proof::ClosureLowerBound => "closure_lower_bound",
            Proof::Unresolved { .. } => "unresolved",
        }
    }
}

/// Whether a finite key range a statement was computed against is a measured envelope or a proved bound
/// (carried through every coverage statement that involves a range).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum KeyRange {
    Measured,
    Proved,
}

/// Generic (some U class) or locus-only (gaps only on coupled loci).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum FaceKind {
    Generic,
    LocusOnly,
}

/// A cell state: a state index of the slot, or [`ANY_OFF`] (every off-floor state).
pub type CellState = u16;
pub const ANY_OFF: CellState = u16::MAX;

/// A gap face (Sd dotted lines, Sn nonzero numerators, every other slot at its floor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GapFace {
    pub sd: Vec<usize>,
    pub sn: Vec<usize>,
    pub kind: FaceKind,
    /// Per face slot: the generic verdict is constant for line values >= t / numerator values <= t.
    pub thresholds: Vec<(usize, i64)>,
    pub unbounded_in_dots: bool,
    pub unbounded_in_rank: bool,
    /// U cells: one cell state per slot of `sd` then `sn`.
    pub generic_cells: Vec<Vec<CellState>>,
    pub n_u_classes: usize,
    pub n_f_classes: usize,
    pub n_f_with_gaps: usize,
    /// An uncovered key on the face (exact oracle), if one was found.
    pub witness: Option<Key>,
    pub loci: Vec<String>,
    pub proof: Proof,
    /// For faces derived from a finite key set (fallback): the label of that key range; `None` for partition faces
    /// (valid for all powers).
    pub key_range: Option<KeyRange>,
    /// U cells of BLOCKED sub-partitions (dispatch / FailClosed): (blocking source, cell) with the cell in the state
    /// indices of THAT sub-report (`PartitionReport::blocked`), never mixed into `generic_cells`, which use this
    /// report's states (re-audit RA2-m1).
    pub blocked_cells: Vec<(usize, Vec<CellState>)>,
}

impl GapFace {
    pub fn key(&self) -> (Vec<usize>, Vec<usize>) {
        (self.sd.clone(), self.sn.clone())
    }
}

/// What part of the chart a [`Outcome::Partial`] run explored.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Explored {
    /// Every class with at most this many off-floor slots is classified (`max_off_floor`): the face list is
    /// complete for faces with at most this many off-floor slots; classes beyond are `Unexplored`.
    OffFloorUpTo(usize),
    /// A work budget stopped the run: `Unexplored` classes anywhere, NOTHING is proved complete.
    Aborted(String),
}

/// Outcome of a run (audit-part-A M1 / M2 / M4).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Outcome {
    /// Every class classified, every fragile region resolved exactly, every face expanded: the face list and the
    /// loci are a complete description of the gaps under the run's coverage notion.
    Complete,
    /// Every class classified, but some gap region is only listed as UNRESOLVED (cap, overflow, nonlinear cut), as
    /// an unexpanded [`FaceFamily`], or fragile classes were not resolved: everything open is REPORTED as open, but
    /// the description is not exact.
    Incomplete { reasons: Vec<String> },
    /// Part of the chart is `Unexplored` (off-floor cap or budget abort): nothing there is covered, uncovered or
    /// complete; `reasons` as for `Incomplete`.
    Partial {
        explored: Explored,
        reasons: Vec<String>,
    },
}

impl Outcome {
    pub fn is_complete(&self) -> bool {
        matches!(self, Outcome::Complete)
    }
    pub fn is_aborted(&self) -> bool {
        matches!(
            self,
            Outcome::Partial {
                explored: Explored::Aborted(_),
                ..
            }
        )
    }
    pub fn tag(&self) -> &'static str {
        match self {
            Outcome::Complete => "complete",
            Outcome::Incomplete { .. } => "unresolved",
            Outcome::Partial {
                explored: Explored::Aborted(_),
                ..
            } => "budget_exceeded",
            Outcome::Partial { .. } => "partial",
        }
    }
    pub fn reasons(&self) -> Vec<String> {
        match self {
            Outcome::Complete => vec![],
            Outcome::Incomplete { reasons } => reasons.clone(),
            Outcome::Partial { explored, reasons } => {
                let mut r = vec![match explored {
                    Explored::OffFloorUpTo(l) => {
                        format!("explored only classes with <= {l} off-floor slots")
                    }
                    Explored::Aborted(w) => format!("aborted: {w}"),
                }];
                r.extend(reasons.iter().cloned());
                r
            }
        }
    }
}

/// A class with more `any` slots than `max_any_slots`, not expanded: EVERY face `base + T` (T a subset of `free`,
/// slots of T off floor) carries this class's label on the cells of the class (audit-part-A M4).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FaceFamily {
    pub base_sd: Vec<usize>,
    pub base_sn: Vec<usize>,
    pub free: Vec<usize>,
    pub label: Label,
}

/// Identifying fields of the artefact (plan 3.3; digests computed by the caller).
#[derive(Clone, Debug, Default)]
pub struct ArtefactIds {
    pub node: String,
    pub seq: u64,
    pub table_set_digest: String,
    pub dispatch_order_digest: String,
}

/// The result of [`gap_partition`].
pub struct PartitionReport {
    pub nf: NfRules,
    pub states: Vec<Vec<Range>>,
    pub root: Option<Rc<Node>>,
    pub classes: Vec<Class>,
    pub stats: EnumStats,
    pub faces: Vec<GapFace>,
    pub resolution: Option<crate::loci::LociReport>,
    pub outcome: Outcome,
    pub coverage: CoverageNotion,
    /// Reasons why this report's COVERAGE claims are not a proof even when the outcome is complete (an elided
    /// record credited under `ElidedPolicy::Ordinary`; in dispatch mode a record excluded or weakened in the normal
    /// form, whose blocked set is then unknown).
    pub caveats: Vec<String>,
    /// Classes with more `any` slots than `max_any_slots` (not expanded into faces / loci).
    pub families: Vec<FaceFamily>,
    pub max_any_slots: usize,
    /// The off-floor cap of the run (faces beyond it are not listed).
    pub max_off_floor: Option<usize>,
    /// The face budget (`Budget::max_faces`) stopped the face list: outcome `Incomplete`.
    pub faces_truncated: bool,
    pub elided: ElidedPolicy,
    pub literal_tests: LiteralTests,
    pub millis: u64,
    pub state_unknown: u64,
    /// FailClosed (elided records) / dispatch mode (every blocking record): per blocking source index, the
    /// partition of its BLOCKED region (selected set minus applies set minus the earlier applies sets); its U / F
    /// classes are gaps of the node in addition to the main classes. Sub-reports with no class and a complete
    /// outcome are not kept (counted in `blocked_empty`).
    pub blocked: Vec<(usize, PartitionReport)>,
    pub blocked_empty: usize,
    /// Blocked sub-partitions NOT computed because the call's work budget was exhausted (re-audit RA2-M5): (source,
    /// dispatch position). Their regions are open: the point oracle stays exact (`uncovered`), `class_of` answers
    /// `Unexplored` there, the outcome is `Partial` (aborted).
    pub blocked_skipped: Vec<(usize, usize)>,
    /// Why the call's work budget was exhausted (main recursion, a sub-partition or the loci), if it was.
    pub budget_exhausted: Option<String>,
    /// Main report: per rule the smallest dispatch position of its sources (for the blocked regions' oracle).
    pub rule_min_pos: Vec<usize>,
    /// Blocked sub-report: dispatch position of its blocking record; its `nf.rules` keep only the complement of
    /// the record's selected set (`compacted`), the earlier records being answered by the main report.
    pub blocked_pos: usize,
    pub compacted: bool,
}

/// Where a key falls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassHit {
    Covered,
    Uncovered,
    Fragile,
    /// In a pruned / unexplored part of a `Partial` run: NOT known to be covered (audit-part-A M2).
    Unexplored,
}

/// (Sd, Sn) of a face.
pub type FaceKey = (Vec<usize>, Vec<usize>);

/// Exact census of a finite key set.
#[derive(Clone, Debug, Default)]
pub struct KeyCensus {
    pub n_keys: usize,
    /// (key, face, label of its class: Uncovered = generic gap, Fragile = locus gap).
    pub uncovered: Vec<(Key, FaceKey, Label)>,
    /// Covered keys in U classes (on coupled case-equation loci).
    pub covered_in_u: Vec<Key>,
    /// Uncovered keys in C classes: must be empty (a soundness check).
    pub uncovered_in_c: Vec<Key>,
    /// Uncovered keys in Unexplored classes of a `Partial` run (exact oracle; their face may be unlisted).
    pub uncovered_unexplored: Vec<Key>,
    /// Label of the key range (set by [`PartitionReport::census_in`]).
    pub range: Option<KeyRange>,
}

impl KeyCensus {
    pub fn n_generic(&self) -> usize {
        self.uncovered
            .iter()
            .filter(|x| x.2 == Label::Uncovered)
            .count()
    }
    pub fn n_locus(&self) -> usize {
        self.uncovered
            .iter()
            .filter(|x| x.2 == Label::Fragile)
            .count()
    }
    pub fn faces(&self) -> BTreeSet<(Vec<usize>, Vec<usize>)> {
        self.uncovered.iter().map(|x| x.1.clone()).collect()
    }
}

/// The face of a chart key.
pub fn face_of(lines: &[bool], key: &[i32]) -> (Vec<usize>, Vec<usize>) {
    let mut sd = vec![];
    let mut sn = vec![];
    for (i, &x) in key.iter().enumerate() {
        if lines[i] {
            if x != 1 {
                sd.push(i);
            }
        } else if x != 0 {
            sn.push(i);
        }
    }
    (sd, sn)
}

/// Compute the exact partition.
pub fn gap_partition(
    src: &dyn DomainSource,
    frame: &NodeFrame,
    opts: &PartitionOptions,
    progress: Option<&dyn Fn(&Progress)>,
) -> Result<PartitionReport, PartitionError> {
    let t0 = Instant::now();
    let dispatch = opts.coverage == CoverageNotion::DispatchCoverage;
    let elided_policy = if dispatch {
        ElidedPolicy::FailClosed
    } else {
        opts.elided
    };
    let nf = nf::build(
        src,
        frame,
        &nf::NfOptions {
            elided: elided_policy,
            dedup: opts.dedup,
            subset: opts.rule_subset.clone(),
            factor_exceptions: opts.factor_exceptions,
        },
    )?;
    // FailClosed: each elided record blocks; dispatch mode: every record with a blocked set blocks.
    let blockers: Vec<usize> = if dispatch {
        nf.blockers.clone()
    } else if elided_policy == ElidedPolicy::FailClosed {
        nf.elided.clone()
    } else {
        vec![]
    };
    let order: Vec<usize> = opts
        .rule_subset
        .clone()
        .unwrap_or_else(|| (0..src.len()).collect());
    let pos: std::collections::HashMap<usize, usize> =
        order.iter().enumerate().map(|(p, &s)| (s, p)).collect();
    let rule_min_pos: Vec<usize> = nf
        .sources
        .iter()
        .map(|ss| {
            ss.iter()
                .filter_map(|s| pos.get(s).copied())
                .min()
                .unwrap_or(usize::MAX)
        })
        .collect();
    let meter = Rc::new(WorkMeter::of_budget(&opts.budget, t0));
    let mut rep = partition_nf(nf, opts, progress, &meter)?;
    rep.elided = elided_policy;
    rep.coverage = opts.coverage;
    rep.rule_min_pos = rule_min_pos;
    // one sub-problem at a time (memory): built, partitioned, then COMPACTED to its complement variants (the rules
    // of the earlier records are answered by the main normal form, see `uncovered`). The SAME meter: after an
    // exhaustion no sub-partition starts (re-audit RA2-M5).
    for &e in &blockers {
        let pe = pos.get(&e).copied().ok_or_else(|| {
            PartitionError::Unsupported(format!("blocker {e} has no dispatch position"))
        })?;
        if meter.exhausted() {
            rep.blocked_skipped.push((e, pe));
            continue;
        }
        let sub = rep.nf.blocked_subproblem(e, &pos)?;
        let nvar = sub.stats.blocked_variants;
        let inexact: Vec<String> = sub
            .notes
            .iter()
            .filter_map(|n| match &n.note {
                nf::NfNote::Weakened(w) => Some(format!("blocked region of source {e}: {w}")),
                _ => None,
            })
            .collect();
        let mut b = partition_nf(sub, opts, None, &meter)?;
        b.coverage = opts.coverage;
        if !inexact.is_empty() {
            // the sub-partition describes a SUPERSET of the blocked region: never a complete description
            b.outcome = match std::mem::replace(&mut b.outcome, Outcome::Complete) {
                Outcome::Complete => Outcome::Incomplete { reasons: inexact },
                Outcome::Incomplete { mut reasons } => {
                    reasons.extend(inexact);
                    Outcome::Incomplete { reasons }
                }
                Outcome::Partial {
                    explored,
                    mut reasons,
                } => {
                    reasons.extend(inexact);
                    Outcome::Partial { explored, reasons }
                }
            };
        }
        if b.classes.is_empty() && b.outcome.is_complete() {
            rep.blocked_empty = rep.blocked_empty.saturating_add(1);
            continue;
        }
        b.blocked_pos = pe;
        let keep = b.nf.rules.len().saturating_sub(nvar);
        b.nf.rules.drain(..keep);
        b.nf.sources.drain(..keep);
        b.nf.quick.drain(..keep);
        b.compacted = true;
        rep.blocked.push((e, b));
    }
    rep.budget_exhausted = meter.reason();
    if !rep.blocked.is_empty() {
        rep.merge_blocked_faces();
    }
    if dispatch {
        let weak: Vec<usize> = rep
            .nf
            .notes
            .iter()
            .filter(|n| n.note.forbids_proof_about_domain())
            .map(|n| n.source)
            .collect();
        if !weak.is_empty() {
            rep.caveats.push(format!(
                "dispatch coverage: {} record(s) excluded or weakened in the normal form (sources {:?}): their selected / blocked sets are not exact",
                weak.len(),
                &weak[..weak.len().min(8)]
            ));
        }
    }
    rep.finish_outcome(opts);
    rep.millis = millis_since(t0);
    Ok(rep)
}

fn millis_since(t0: Instant) -> u64 {
    u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The partition of an already built normal form, charged to `meter` (the call's ONE work budget; re-audit RA2-M5).
pub fn partition_nf(
    nf: NfRules,
    opts: &PartitionOptions,
    progress: Option<&dyn Fn(&Progress)>,
    meter: &Rc<WorkMeter>,
) -> Result<PartitionReport, PartitionError> {
    let t0 = Instant::now();
    let (states, state_unknown) = classes::slot_states(&nf, opts.extra_critical.as_deref())?;
    let (root, stats) = {
        let mut eng = classes::Engine::new(&nf, states.clone(), opts.literal_tests)?;
        eng.use_memo = opts.memo;
        eng.cap = opts.max_off_floor;
        eng.meter = meter.clone();
        eng.max_memo = opts.budget.max_memo;
        let root = eng.run(progress);
        (root, eng.stats.clone())
    };
    let classes = classes::collect_classes(&root, nf.arity);
    let credited = nf
        .notes
        .iter()
        .filter(|n| n.note == nf::NfNote::ElidedCredited)
        .count();
    let mut caveats = vec![];
    if credited > 0 {
        caveats.push(format!(
            "{credited} elided record(s) credited (ElidedPolicy::Ordinary): a regression reference, never a proof"
        ));
    }
    let mut rep = PartitionReport {
        nf,
        states,
        root,
        classes,
        stats,
        faces: vec![],
        resolution: None,
        outcome: Outcome::Complete,
        coverage: opts.coverage,
        caveats,
        families: vec![],
        max_any_slots: opts.max_any_slots,
        max_off_floor: opts.max_off_floor,
        faces_truncated: false,
        elided: opts.elided,
        literal_tests: opts.literal_tests,
        millis: 0,
        state_unknown,
        blocked: vec![],
        blocked_empty: 0,
        blocked_skipped: vec![],
        budget_exhausted: None,
        rule_min_pos: vec![],
        blocked_pos: 0,
        compacted: false,
    };
    if !rep.stats.aborted && opts.resolve_loci {
        // the call's budget reaches the locus resolution (audit-part-B M3, re-audit RA2-M5): regions left when it is
        // exhausted are OPEN (a bounded summary per class, never one region per pending call)
        let mut lo = opts.loci.clone();
        if lo.deadline.is_none() {
            lo.deadline = meter.deadline();
        }
        rep.resolution = Some(crate::loci::resolve_fragile_classes_with(&rep, &lo, meter)?);
    }
    rep.budget_exhausted = meter.reason();
    let (faces, truncated) = rep.build_faces(opts.budget.max_faces);
    rep.faces = faces;
    rep.faces_truncated = truncated;
    rep.families = rep
        .classes
        .iter()
        .filter(|c| c.label != Label::Unexplored)
        .filter_map(|c| rep.family_of(c))
        .collect();
    rep.finish_outcome(opts);
    rep.millis = millis_since(t0);
    Ok(rep)
}

/// Fallback face list from uncovered keys (R30): a LOWER BOUND, every face labelled `closure_lower_bound`.
pub fn faces_from_uncovered_keys(frame: &NodeFrame, keys: &[Key], range: KeyRange) -> Vec<GapFace> {
    let lines: Vec<bool> = (0..frame.arity()).map(|i| frame.is_line(i)).collect();
    let mut by: BTreeMap<(Vec<usize>, Vec<usize>), Vec<&Key>> = BTreeMap::new();
    for k in keys {
        by.entry(face_of(&lines, k)).or_default().push(k);
    }
    let mut out: Vec<GapFace> = by
        .into_iter()
        .map(|((sd, sn), ks)| {
            let unb_d = false;
            GapFace {
                thresholds: vec![],
                unbounded_in_dots: unb_d,
                unbounded_in_rank: false,
                generic_cells: vec![],
                n_u_classes: 0,
                n_f_classes: 0,
                n_f_with_gaps: 0,
                witness: ks.iter().min().map(|k| (*k).clone()),
                loci: vec![],
                proof: Proof::ClosureLowerBound,
                key_range: Some(range),
                blocked_cells: vec![],
                kind: FaceKind::Generic,
                sd,
                sn,
            }
        })
        .collect();
    out.sort_by_key(|f| {
        (
            f.sd.len().saturating_add(f.sn.len()),
            f.sd.clone(),
            f.sn.clone(),
        )
    });
    out
}

impl PartitionReport {
    pub fn arity(&self) -> usize {
        self.nf.arity
    }

    /// State index of a value in a slot.
    pub fn state_index(&self, slot: usize, v: i64) -> Option<u16> {
        self.states[slot]
            .iter()
            .position(|r| r.contains(v))
            .and_then(|i| u16::try_from(i).ok())
    }

    fn check_chart(&self, key: &[i32]) -> Result<(), PartitionError> {
        if key.len() != self.nf.arity {
            return Err(PartitionError::Arity {
                expected: self.nf.arity,
                got: key.len(),
            });
        }
        for (i, &x) in key.iter().enumerate() {
            if (self.nf.lines[i] && x < 1) || (!self.nf.lines[i] && x > 0) {
                return Err(PartitionError::NotChartKey(key.to_vec()));
            }
        }
        Ok(())
    }

    /// POINT ORACLE: is the chart key uncovered (union coverage of the normal-form rules, exact)?
    pub fn uncovered(&self, key: &[i32]) -> Result<bool, PartitionError> {
        self.check_chart(key)?;
        if !self.nf.covered(key)? {
            return Ok(true);
        }
        if self.blocked.is_empty() && self.blocked_skipped.is_empty() {
            return Ok(false);
        }
        let first = self.first_applying_pos(key)?;
        for (e, b) in &self.blocked {
            // blocked region of b: selected(e) minus applies(s <= e), read from the selected set itself (exact, even
            // when the sub-partition's complement had to be over-approximated)
            if first > b.blocked_pos && self.selected_at(*e, key)? {
                return Ok(true);
            }
        }
        // skipped sub-partitions (budget): the same membership
        for &(e, pe) in &self.blocked_skipped {
            if first > pe && self.selected_at(e, key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The smallest dispatch position of a record that applies at the key (`usize::MAX` if none).
    fn first_applying_pos(&self, key: &[i32]) -> Result<usize, PartitionError> {
        let mut first = usize::MAX;
        for r in 0..self.nf.rules.len() {
            let p = self.rule_min_pos.get(r).copied().unwrap_or(usize::MAX);
            if p < first && self.nf.rule_covers(r, key)? {
                first = p;
            }
        }
        Ok(first)
    }

    /// Is blocker `e` SELECTED at the key (its selected set, exact)?
    fn selected_at(&self, e: usize, key: &[i32]) -> Result<bool, PartitionError> {
        match self.nf.selected.get(&e) {
            Some(sel) => self.nf.nf_rule_covers(sel, key),
            None => Ok(true), // unknown selected set: conservatively selected (the key reads open)
        }
    }

    /// The class of a chart key.
    pub fn class_of(&self, key: &[i32]) -> Result<ClassHit, PartitionError> {
        self.check_chart(key)?;
        let idx: Vec<u16> = key
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                self.state_index(i, i64::from(x)).ok_or_else(|| {
                    PartitionError::Unsupported(format!("no state of slot {i} contains {x}"))
                })
            })
            .collect::<Result<_, _>>()?;
        let main = match classes::lookup(&self.root, &idx) {
            None => ClassHit::Covered,
            Some(Node::Leaf(Label::Uncovered)) => ClassHit::Uncovered,
            Some(Node::Leaf(Label::Unexplored)) => ClassHit::Unexplored,
            Some(_) => ClassHit::Fragile,
        };
        if main == ClassHit::Uncovered {
            return Ok(main);
        }
        // precedence: Uncovered (definite) > Unexplored (unknown) > Fragile > Covered
        let rank = |h: &ClassHit| match h {
            ClassHit::Covered => 0,
            ClassHit::Fragile => 1,
            ClassHit::Unexplored => 2,
            ClassHit::Uncovered => 3,
        };
        let mut best = main;
        for (_, b) in &self.blocked {
            let h = b.class_of(key)?;
            if h == ClassHit::Uncovered {
                return Ok(h);
            }
            if rank(&h) > rank(&best) {
                best = h;
            }
        }
        if !self.blocked_skipped.is_empty() && rank(&best) < rank(&ClassHit::Unexplored) {
            let first = self.first_applying_pos(key)?;
            for &(e, pe) in &self.blocked_skipped {
                if first > pe && self.selected_at(e, key)? {
                    best = ClassHit::Unexplored; // in a blocked region whose sub-partition was not computed
                    break;
                }
            }
        }
        Ok(best)
    }

    /// Add the faces of the blocked regions (FailClosed) to the face list.
    fn merge_blocked_faces(&mut self) {
        let mut faces = std::mem::take(&mut self.faces);
        for (e, b) in &self.blocked {
            self.families.extend(b.families.iter().cloned());
            for f in &b.faces {
                // the sub-report's cells are in ITS state indices: kept apart, tagged with the blocker (RA2-m1)
                let cells: Vec<(usize, Vec<CellState>)> =
                    f.generic_cells.iter().map(|c| (*e, c.clone())).collect();
                match faces.iter_mut().find(|g| g.key() == f.key()) {
                    Some(g) => {
                        if f.kind == FaceKind::Generic && g.kind == FaceKind::LocusOnly {
                            g.kind = FaceKind::Generic;
                            g.proof = Proof::ExactPartition;
                        }
                        g.n_u_classes = g.n_u_classes.saturating_add(f.n_u_classes);
                        g.blocked_cells.extend(cells);
                        g.blocked_cells.sort();
                        g.blocked_cells.dedup();
                        g.unbounded_in_dots |= f.unbounded_in_dots;
                        g.unbounded_in_rank |= f.unbounded_in_rank;
                        if g.witness.is_none() {
                            g.witness = f.witness.clone();
                        }
                    }
                    None => {
                        let mut g = f.clone();
                        g.generic_cells.clear();
                        g.blocked_cells = cells;
                        faces.push(g);
                    }
                }
            }
        }
        faces.sort_by_key(|f| {
            (
                f.sd.len().saturating_add(f.sn.len()),
                f.sd.clone(),
                f.sn.clone(),
            )
        });
        self.faces = faces;
    }

    /// [`PartitionReport::census`] with the key range's label carried.
    pub fn census_in(&self, keys: &[Key], range: KeyRange) -> Result<KeyCensus, PartitionError> {
        let mut c = self.census(keys)?;
        c.range = Some(range);
        Ok(c)
    }

    /// Exact census of a finite key set (box, envelope, closure keys): every key evaluated by the oracle and placed.
    pub fn census(&self, keys: &[Key]) -> Result<KeyCensus, PartitionError> {
        let mut c = KeyCensus {
            n_keys: keys.len(),
            ..Default::default()
        };
        for k in keys {
            let unc = self.uncovered(k)?;
            let hit = self.class_of(k)?;
            match (unc, hit) {
                (true, ClassHit::Uncovered) => {
                    c.uncovered
                        .push((k.clone(), face_of(&self.nf.lines, k), Label::Uncovered))
                }
                (true, ClassHit::Fragile) => {
                    c.uncovered
                        .push((k.clone(), face_of(&self.nf.lines, k), Label::Fragile))
                }
                (true, ClassHit::Covered) => c.uncovered_in_c.push(k.clone()),
                (true, ClassHit::Unexplored) => c.uncovered_unexplored.push(k.clone()),
                (false, ClassHit::Uncovered) => c.covered_in_u.push(k.clone()),
                _ => {}
            }
        }
        Ok(c)
    }

    pub fn n_u(&self) -> usize {
        self.classes
            .iter()
            .filter(|c| c.label == Label::Uncovered)
            .count()
    }
    pub fn n_f(&self) -> usize {
        self.classes
            .iter()
            .filter(|c| c.label == Label::Fragile)
            .count()
    }

    pub fn n_unexplored(&self) -> usize {
        self.classes
            .iter()
            .filter(|c| c.label == Label::Unexplored)
            .count()
    }

    /// The class as an unexpanded family, if it has more `any` slots than `max_any_slots`.
    pub fn family_of(&self, c: &Class) -> Option<FaceFamily> {
        let n = self.nf.arity;
        let free: Vec<usize> = (0..n).filter(|&i| c.states[i].is_none()).collect();
        let fixed_off0 = c
            .states
            .iter()
            .filter(|s| s.is_some_and(|v| v != 0))
            .count();
        let capped = self
            .max_off_floor
            .is_some_and(|l| l.saturating_sub(fixed_off0) < free.len());
        if capped || free.len() <= self.max_any_slots {
            return None; // expanded by `expand`
        }
        let fixed_off = |i: usize| c.states[i].is_some_and(|v| v != 0);
        Some(FaceFamily {
            base_sd: (0..n)
                .filter(|&i| self.nf.lines[i] && fixed_off(i))
                .collect(),
            base_sn: (0..n)
                .filter(|&i| !self.nf.lines[i] && fixed_off(i))
                .collect(),
            free,
            label: c.label,
        })
    }

    /// Expand the `any` slots of a class into (floor, any off-floor): (face, cell states over all slots). A class
    /// with more `any` slots than `max_any_slots` gives `None` (see [`PartitionReport::family_of`]): never a silent
    /// truncation (audit-part-A M4).
    pub fn expand(&self, c: &Class) -> Option<Vec<(FaceKey, Vec<CellState>)>> {
        let n = self.nf.arity;
        let stars: Vec<usize> = (0..n).filter(|&i| c.states[i].is_none()).collect();
        // under an off-floor cap L only the faces with <= L off-floor slots are listed (the run claims nothing
        // beyond; audit-part-B M3: the cap bounds the expansion)
        let fixed_off = c
            .states
            .iter()
            .filter(|s| s.is_some_and(|v| v != 0))
            .count();
        let r = match self.max_off_floor {
            Some(l) => l.saturating_sub(fixed_off).min(stars.len()),
            None => stars.len(),
        };
        if r == stars.len() && stars.len() > self.max_any_slots {
            return None;
        }
        if self.max_off_floor.is_some_and(|l| fixed_off > l) {
            return Some(vec![]);
        }
        let mut out = vec![];
        // every subset of `stars` with at most r elements (no fixed word width)
        fn rec(
            rep: &PartitionReport,
            c: &Class,
            stars: &[usize],
            from: usize,
            left: usize,
            pick: &mut Vec<usize>,
            out: &mut Vec<(FaceKey, Vec<CellState>)>,
        ) {
            let n = rep.nf.arity;
            let mut cell: Vec<CellState> = c.states.iter().map(|s| s.unwrap_or(0)).collect();
            for &s in pick.iter() {
                cell[s] = ANY_OFF;
            }
            let sd: Vec<usize> = (0..n)
                .filter(|&i| rep.nf.lines[i] && cell[i] != 0)
                .collect();
            let sn: Vec<usize> = (0..n)
                .filter(|&i| !rep.nf.lines[i] && cell[i] != 0)
                .collect();
            out.push(((sd, sn), cell));
            if left == 0 {
                return;
            }
            let left1 = left.saturating_sub(1); // left > 0 here
            for j in from..stars.len() {
                pick.push(stars[j]);
                rec(rep, c, stars, j.saturating_add(1), left1, pick, out);
                pick.pop();
            }
        }
        rec(self, c, &stars, 0, r, &mut vec![], &mut out);
        Some(out)
    }

    /// Settle the outcome (audit-part-A M1 / M2 / M4): Partial if any class is unexplored (cap or abort, here or in
    /// a blocked sub-partition), Incomplete if any gap region is only listed as unresolved / unexpanded.
    fn finish_outcome(&mut self, opts: &PartitionOptions) {
        let mut reasons: Vec<String> = vec![];
        let mut explored: Option<Explored> = None;
        if self.stats.aborted {
            explored = Some(Explored::Aborted(format!(
                "work budget (units {:?}, wall {:?}) exceeded after {} recursion nodes{}",
                opts.budget.max_nodes,
                opts.budget.wall,
                self.stats.nodes,
                self.budget_exhausted
                    .as_ref()
                    .map(|w| format!(": {w}"))
                    .unwrap_or_default()
            )));
        } else if let Some(w) = &self.budget_exhausted {
            // exhausted after the recursion (locus resolution or a blocked sub-partition): nothing is proved
            explored = Some(Explored::Aborted(w.clone()));
        } else if self.stats.pruned_off_floor > 0
            && let Some(c) = opts.max_off_floor
        {
            explored = Some(Explored::OffFloorUpTo(c));
        }
        if self.faces_truncated {
            reasons.push(format!(
                "face budget max_faces = {:?} reached: the face list is truncated",
                opts.budget.max_faces
            ));
        }
        if !self.families.is_empty() {
            reasons.push(format!(
                "{} class(es) with more than max_any_slots = {} any slots listed as face families only",
                self.families.len(),
                self.max_any_slots
            ));
        }
        let n_f = self.n_f();
        match &self.resolution {
            None if n_f > 0 && !self.stats.aborted => reasons.push(format!(
                "{n_f} fragile class(es) not resolved into loci (resolve_loci = false): their gap loci are not listed"
            )),
            Some(r) if r.n_unresolved_parts() > 0 => reasons.push(format!(
                "{} locus part(s) UNRESOLVED (listed as open gap regions): {}",
                r.n_unresolved_parts(),
                r.unresolved_reasons().join("; ")
            )),
            _ => {}
        }
        if let Some(r) = &self.resolution {
            // a flagged piece is a SUPERSET of its uncovered set (re-audit RA2-m4 / M3): not an exact description
            let flagged = r.n_flagged_pieces();
            if flagged > 0 {
                reasons.push(format!(
                    "{flagged} locus piece(s) flagged covered_part_unresolved (a superset of the uncovered set: a covered sub-locus not subtracted, a rule left undecided, or integer emptiness undecided)"
                ));
            }
        }
        if !self.blocked_skipped.is_empty() {
            let w = format!(
                "{} blocked sub-partition(s) not computed (work budget exhausted)",
                self.blocked_skipped.len()
            );
            reasons.push(w.clone());
            explored = Some(Explored::Aborted(w));
        }
        for (e, b) in &self.blocked {
            match &b.outcome {
                Outcome::Complete => {}
                Outcome::Incomplete { reasons: rs } => reasons.extend(
                    rs.iter()
                        .map(|r| format!("blocked region of source {e}: {r}")),
                ),
                Outcome::Partial {
                    explored: ex,
                    reasons: rs,
                } => {
                    reasons.extend(
                        rs.iter()
                            .map(|r| format!("blocked region of source {e}: {r}")),
                    );
                    match ex {
                        Explored::Aborted(w) => {
                            explored = Some(Explored::Aborted(format!(
                                "blocked region of source {e}: {w}"
                            )))
                        }
                        Explored::OffFloorUpTo(c) => {
                            if !matches!(explored, Some(Explored::Aborted(_))) {
                                explored = Some(Explored::OffFloorUpTo(*c));
                            }
                        }
                    }
                }
            }
        }
        self.outcome = match explored {
            Some(explored) => Outcome::Partial { explored, reasons },
            None if reasons.is_empty() => Outcome::Complete,
            None => Outcome::Incomplete { reasons },
        };
    }

    /// Is this report a PROOF that the node has NO gap under `notion`? `Ok(certificate)` only when the run used that
    /// notion, the outcome is complete, there is no caveat, no U class, no fragile class with a gap piece or an
    /// unresolved part, and no blocked sub-partition with a class. `Err(reason)` otherwise (refuse; never a guess).
    pub fn proves_no_gap(&self, notion: CoverageNotion) -> Result<String, String> {
        if self.coverage != notion {
            return Err(format!(
                "the report computes {:?}, not {notion:?}",
                self.coverage
            ));
        }
        if !self.outcome.is_complete() {
            return Err(format!(
                "outcome {}: {}",
                self.outcome.tag(),
                self.outcome.reasons().join("; ")
            ));
        }
        if !self.caveats.is_empty() {
            return Err(self.caveats.join("; "));
        }
        if self.n_u() > 0 {
            return Err(format!("{} uncovered class(es)", self.n_u()));
        }
        if self.n_f() > 0 {
            match &self.resolution {
                None => return Err("fragile classes not resolved".into()),
                Some(r) if !r.loci.is_empty() => {
                    return Err(format!("{} gap loci", r.loci.len()));
                }
                Some(_) => {}
            }
        }
        if !self.blocked.is_empty() {
            return Err(format!(
                "{} blocked region(s) with classes",
                self.blocked.len()
            ));
        }
        if !self.blocked_skipped.is_empty() || self.budget_exhausted.is_some() {
            return Err("the work budget was exhausted: blocked regions not partitioned".into());
        }
        Ok(format!(
            "exact partition ({}): {} nodes, 0 U, {} F all resolved without gap, {} blocked sub-partitions empty, outcome complete",
            notion.tag(),
            self.stats.nodes,
            self.n_f(),
            self.blocked_empty
        ))
    }

    fn build_faces(&self, max_faces: Option<usize>) -> (Vec<GapFace>, bool) {
        let mut n_cells = 0usize;
        let mut truncated = false;
        type FKey = (Vec<usize>, Vec<usize>);
        #[derive(Default)]
        struct Acc {
            u: BTreeSet<Vec<CellState>>,
            nu: usize,
            nf: usize,
            nfg: usize,
            loci: BTreeSet<String>,
            tested: bool,
            unresolved: bool,
        }
        let mut by: BTreeMap<FKey, Acc> = BTreeMap::new();
        for c in &self.classes {
            if c.label == Label::Unexplored {
                continue;
            }
            let Some(cells) = self.expand(c) else {
                continue; // listed in `families` (see `partition_nf`)
            };
            n_cells = n_cells.saturating_add(cells.len());
            if max_faces.is_some_and(|m| n_cells > m) {
                truncated = true;
                break;
            }
            for (f, cell) in cells {
                let a = by.entry(f.clone()).or_default();
                let fc: Vec<CellState> = f.0.iter().chain(&f.1).map(|&s| cell[s]).collect();
                match c.label {
                    Label::Uncovered => {
                        a.u.insert(fc);
                        a.nu = a.nu.saturating_add(1);
                    }
                    Label::Fragile => a.nf = a.nf.saturating_add(1),
                    Label::Unexplored => {}
                }
            }
        }
        if let Some(res) = &self.resolution {
            for l in &res.loci {
                let a = by.entry(l.face.clone()).or_default();
                a.nfg = a.nfg.saturating_add(l.n_classes);
                a.loci.insert(l.id.clone());
                match &l.proof {
                    Proof::ExactPartition => {}
                    Proof::Unresolved { .. } => a.unresolved = true,
                    _ => a.tested = true,
                }
            }
        }
        let mut out = vec![];
        for ((sd, sn), a) in by {
            if a.u.is_empty() && a.loci.is_empty() {
                continue;
            }
            let slots: Vec<usize> = sd.iter().chain(&sn).copied().collect();
            let thresholds = self.thresholds(&slots, &a.u);
            let unb = |range: std::ops::Range<usize>| {
                a.u.iter().any(|c| {
                    range.clone().any(|j| {
                        c[j] == ANY_OFF
                            || usize::from(c[j]).saturating_add(1) == self.states[slots[j]].len()
                    })
                })
            };
            let mut unbounded_in_dots = unb(0..sd.len());
            let unbounded_in_rank = unb(sd.len()..slots.len());
            if let Some(res) = &self.resolution {
                for l in res
                    .loci
                    .iter()
                    .filter(|l| l.face == (sd.clone(), sn.clone()))
                {
                    if l.unbounded_in_dots {
                        unbounded_in_dots = true;
                    }
                }
            }
            let kind = if a.u.is_empty() {
                FaceKind::LocusOnly
            } else {
                FaceKind::Generic
            };
            let mut face = GapFace {
                sd,
                sn,
                kind,
                thresholds,
                unbounded_in_dots,
                unbounded_in_rank,
                generic_cells: a.u.iter().cloned().collect(),
                n_u_classes: a.nu,
                n_f_classes: a.nf,
                n_f_with_gaps: a.nfg,
                witness: None,
                loci: a.loci.iter().cloned().collect(),
                key_range: None,
                blocked_cells: vec![],
                proof: if kind == FaceKind::Generic {
                    Proof::ExactPartition
                } else if a.unresolved {
                    Proof::Unresolved {
                        reason: "a locus part of this face is unresolved (open gap region)".into(),
                    }
                } else if a.tested {
                    Proof::Tested {
                        bound: "locus window".into(),
                    }
                } else {
                    Proof::ExactPartition
                },
            };
            face.witness = self.face_witness(&face);
            out.push(face);
        }
        out.sort_by_key(|f| {
            (
                f.sd.len().saturating_add(f.sn.len()),
                f.sd.clone(),
                f.sn.clone(),
            )
        });
        (out, truncated)
    }

    /// Smallest state from which the generic verdict is constant, per face slot.
    fn thresholds(&self, slots: &[usize], cells: &BTreeSet<Vec<CellState>>) -> Vec<(usize, i64)> {
        // expand ANY_OFF to every off-floor state
        let mut full: BTreeSet<Vec<u16>> = BTreeSet::new();
        for c in cells {
            let mut acc: Vec<Vec<u16>> = vec![vec![]];
            for (j, &s) in c.iter().enumerate() {
                let opts: Vec<u16> = if s == ANY_OFF {
                    (1..u16::try_from(self.states[slots[j]].len()).unwrap_or(u16::MAX)).collect()
                } else {
                    vec![s]
                };
                acc = acc
                    .into_iter()
                    .flat_map(|p| {
                        opts.iter().map(move |&o| {
                            let mut q = p.clone();
                            q.push(o);
                            q
                        })
                    })
                    .collect();
            }
            full.extend(acc);
        }
        let mut out = vec![];
        for (j, &s) in slots.iter().enumerate() {
            // state counts are < u16::MAX (checked by `slot_states`) and >= 1
            let m = u16::try_from(self.states[s].len()).unwrap_or(u16::MAX);
            let mut t_found = m.saturating_sub(1);
            'm: for t in 1..m {
                // all other cells (from the cell set's projections) must have one verdict for states t..m-1 of slot j
                let others: BTreeSet<Vec<u16>> = full
                    .iter()
                    .map(|c| {
                        let mut o = c.clone();
                        o.remove(j);
                        o
                    })
                    .collect();
                for o in &others {
                    let mut vs = BTreeSet::new();
                    for v in t..m {
                        let mut c = o.clone();
                        c.insert(j, v);
                        vs.insert(full.contains(&c));
                    }
                    if vs.len() > 1 {
                        continue 'm;
                    }
                }
                t_found = t;
                break;
            }
            let r = self.states[s][usize::from(t_found)];
            // a line state has a lower end, a numerator state an upper end (states are built that way)
            out.push((
                s,
                if self.nf.lines[s] {
                    r.lo.unwrap_or(1)
                } else {
                    r.hi.unwrap_or(0)
                },
            ));
        }
        out
    }

    /// Representative values of a state, nearest to the floor first.
    fn reps(&self, slot: usize, st: CellState) -> Vec<i64> {
        let line = self.nf.lines[slot];
        let ranges: Vec<Range> = if st == ANY_OFF {
            self.states[slot].get(1..).unwrap_or(&[]).to_vec()
        } else {
            vec![self.states[slot][usize::from(st)]]
        };
        let mut v = vec![];
        for r in ranges {
            let Some(base) = (if line { r.lo } else { r.hi }) else {
                continue;
            };
            for d in 0..3i64 {
                let x = if line {
                    base.checked_add(d)
                } else {
                    base.checked_sub(d)
                };
                if let Some(x) = x
                    && r.contains(x)
                {
                    v.push(x);
                }
            }
        }
        v
    }

    fn face_witness(&self, f: &GapFace) -> Option<Key> {
        let slots: Vec<usize> = f.sd.iter().chain(&f.sn).copied().collect();
        let cells: Vec<Vec<CellState>> = if !f.generic_cells.is_empty() {
            f.generic_cells.clone()
        } else {
            vec![]
        };
        for cell in cells.iter().take(16) {
            let per: Vec<Vec<i64>> = slots
                .iter()
                .zip(cell)
                .map(|(&s, &st)| self.reps(s, st))
                .collect();
            // walk a few combinations
            let mut idx = vec![0usize; per.len()];
            if per.iter().any(|p| p.is_empty()) {
                continue;
            }
            for _ in 0..64 {
                let mut key: Key = (0..self.nf.arity)
                    .map(|i| if self.nf.lines[i] { 1 } else { 0 })
                    .collect();
                let mut fits = true;
                for (j, &s) in slots.iter().enumerate() {
                    match i32::try_from(per[j][idx[j]]) {
                        Ok(x) => key[s] = x,
                        Err(_) => fits = false,
                    }
                }
                if fits && self.uncovered(&key).ok() == Some(true) {
                    return Some(key);
                }
                // next combination (odometer)
                let mut j = 0;
                loop {
                    if j == idx.len() {
                        break;
                    }
                    idx[j] = idx[j].saturating_add(1);
                    if idx[j] < per[j].len() {
                        break;
                    }
                    idx[j] = 0;
                    j = j.saturating_add(1);
                }
                if j == idx.len() {
                    break;
                }
            }
        }
        if let Some(res) = &self.resolution {
            for l in res
                .loci
                .iter()
                .filter(|l| l.face == (f.sd.clone(), f.sn.clone()))
            {
                if let Some(w) = l.witness.first() {
                    return Some(w.clone());
                }
            }
        }
        None
    }

    /// Render a cell state.
    pub fn cell_label(&self, slot: usize, st: CellState) -> String {
        if st == ANY_OFF {
            return if self.nf.lines[slot] {
                ">=2".into()
            } else {
                "<=-1".into()
            };
        }
        let r = self.states[slot][usize::from(st)];
        match (r.lo, r.hi) {
            (Some(a), Some(b)) if a == b => a.to_string(),
            (Some(a), Some(b)) => format!("{a}..{b}"),
            (Some(a), None) => format!(">={a}"),
            (None, Some(b)) => format!("<={b}"),
            (None, None) => "*".into(),
        }
    }

    /// The artefact `reports/<node>/partition-<seq>.json` (schema `rustred.campaign.partition.v1`).
    pub fn to_json(&self, ids: &ArtefactIds, frame: &NodeFrame) -> Json {
        let states: Vec<Json> = self
            .states
            .iter()
            .map(|s| Json::Array(s.iter().map(|r| json!({"lo": r.lo, "hi": r.hi})).collect()))
            .collect();
        let faces: Vec<Json> = self
            .faces
            .iter()
            .map(|f| {
                let slots: Vec<usize> = f.sd.iter().chain(&f.sn).copied().collect();
                json!({
                    "Sd": f.sd, "Sn": f.sn,
                    "kind": match f.kind { FaceKind::Generic => "generic", FaceKind::LocusOnly => "locus_only" },
                    "thresholds": f.thresholds.iter().map(|(s, t)| (format!("n{s}"), json!(t))).collect::<serde_json::Map<_, _>>(),
                    "unbounded_in_dots": f.unbounded_in_dots, "unbounded_in_rank": f.unbounded_in_rank,
                    "generic_cells": f.generic_cells.iter().map(|c| c.iter().zip(&slots).map(|(&st, &s)| self.cell_label(s, st)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "blocked_cells": f.blocked_cells.iter().map(|(e, c)| {
                        let sub = self.blocked.iter().find(|(x, _)| x == e).map(|(_, b)| b);
                        json!({"source": e, "cell": c.iter().zip(&slots).map(|(&st, &s)| match sub {
                            Some(b) => b.cell_label(s, st),
                            None => format!("state {st}"),
                        }).collect::<Vec<_>>()})
                    }).collect::<Vec<_>>(),
                    "n_U_classes": f.n_u_classes, "n_F_classes": f.n_f_classes, "n_F_with_gaps": f.n_f_with_gaps,
                    "witness": f.witness, "loci": f.loci, "proof": f.proof.tag(),
                })
            })
            .collect();
        let loci: Vec<Json> = self
            .resolution
            .as_ref()
            .map(|r| r.loci_json())
            .unwrap_or_default();
        let unresolved: Vec<String> = self
            .resolution
            .as_ref()
            .map(|r| {
                r.loci
                    .iter()
                    .filter(|l| l.unresolved)
                    .map(|l| l.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        json!({
            "schema": "rustred.campaign.partition.v1",
            "node": ids.node, "seq": ids.seq,
            "coverage": self.coverage.tag(),
            "caveats": self.caveats,
            "input_notes": self.nf.notes,
            "elided_policy": match self.elided { ElidedPolicy::Ordinary => "ordinary", ElidedPolicy::FailClosed => "fail_closed", ElidedPolicy::Absent => "absent" },
            "table_set_digest": ids.table_set_digest,
            "dispatch_order_digest": ids.dispatch_order_digest,
            "tool_versions": {"partition": concat!(env!("CARGO_PKG_VERSION"), "+part.1"),
                              "literal_tests": match self.literal_tests { LiteralTests::Exact => "exact", LiteralTests::ReferenceCompatible => "reference_compatible" }},
            "frame": {"arity": frame.arity(), "mask": frame.mask_string()},
            "inputs": self.nf.stats,
            "states": states,
            "stats": {"nodes": self.stats.nodes, "memo_hits": self.stats.memo_hits, "memo_size": self.stats.memo_size,
                      "U": self.n_u(), "F": self.n_f(), "unexplored": self.n_unexplored(), "pruned_off_floor": self.stats.pruned_off_floor,
                      "unknown_literal_tests": self.stats.unknown_literal_tests.saturating_add(self.state_unknown), "millis": self.millis,
                      "soft_fragile": self.stats.soft_fragile},
            "outcome": self.outcome.tag(),
            "outcome_reasons": self.outcome.reasons(),
            "face_families": self.families,
            "faces": faces,
            "loci": loci,
            "unresolved": unresolved,
            "blocked": self.blocked.iter().map(|(e, b)| json!({"source": e, "U": b.n_u(), "F": b.n_f(),
                "nodes": b.stats.nodes, "faces": b.faces.len(), "outcome": b.outcome.tag()})).collect::<Vec<_>>(),
            "blocked_empty": self.blocked_empty,
            "blocked_skipped": self.blocked_skipped.iter().map(|(e, _)| e).collect::<Vec<_>>(),
            "budget_exhausted": self.budget_exhausted,
        })
    }
}

/// One completed level of [`gap_partition_levels`].
#[derive(Clone, Debug)]
pub struct LevelResult {
    /// Every class with at most `level` off-floor slots is classified: the face list is COMPLETE for faces with at
    /// most `level` off-floor slots (generic faces exact; loci as resolved).
    pub level: usize,
    /// PROVED complete at this level: no budget abort, and no unresolved / unexpanded gap region among the faces
    /// of at most `level` off-floor slots (`incomplete` lists the reasons otherwise; the faces are then still a
    /// complete list of OPEN faces, but some open region is not described exactly). Aborted levels are never
    /// recorded.
    pub proved: bool,
    pub incomplete: Vec<String>,
    pub faces: Vec<FaceKey>,
    pub generic: Vec<FaceKey>,
    pub nodes: u64,
    pub millis: u64,
}

/// Format tag of a levels-checkpoint line (a version constant; re-audit RA2-M4).
pub const LEVELS_CHECKPOINT_FORMAT: &str = "rustred.campaign.partition-levels.v2";

/// Digest of every input a partition result depends on (re-audit RA2-M4): the domains IN DISPATCH ORDER (canonical
/// text of each domain and its witness-status block, restricted to `rule_subset` in its order), the frame (arity,
/// line mask) and every option that changes the result (coverage, elided policy, literal tests, dedup, factored
/// exceptions, loci resolution and its caps, extra critical values, face limits). Work limits (`max_nodes`, `wall`,
/// `max_memo`) and the per-level off-floor cap are not part of it (an aborted level is never recorded).
pub fn partition_inputs_digest(
    src: &dyn DomainSource,
    frame: &NodeFrame,
    opts: &PartitionOptions,
) -> String {
    let mut h = crate::replay_base::sha256::Sha256::new();
    h.update(
        format!(
            "{LEVELS_CHECKPOINT_FORMAT};arity={};mask={};n={}\n",
            frame.arity(),
            frame.mask_string(),
            src.len()
        )
        .as_bytes(),
    );
    let l = &opts.loci;
    h.update(
        format!(
            "coverage={:?};elided={:?};literal_tests={:?};dedup={};factor_exceptions={};resolve_loci={};max_any_slots={};max_faces={:?};extra_critical={:?};rule_subset={:?};loci=({},{},{},{:?},{},{},{},{})\n",
            opts.coverage,
            opts.elided,
            opts.literal_tests,
            opts.dedup,
            opts.factor_exceptions,
            opts.resolve_loci,
            opts.max_any_slots,
            opts.budget.max_faces,
            opts.extra_critical,
            opts.rule_subset,
            l.max_nodes,
            l.max_depth,
            l.max_split_nodes,
            l.window_radius,
            l.tries,
            l.witnesses,
            l.max_open_regions,
            l.max_enumeration,
        )
        .as_bytes(),
    );
    let order: Vec<usize> = opts
        .rule_subset
        .clone()
        .unwrap_or_else(|| (0..src.len()).collect());
    for i in order {
        if i < src.len() {
            h.update(format!("{i}:{:?};wb={:?}\n", src.domain(i), src.witness_block(i)).as_bytes());
        }
    }
    format!("sha256:{}", h.hex())
}

/// SCALING (R30 (2), (4)): the partition per face-lattice level. Level L runs the exact recursion capped at L
/// off-floor slots, so its result is complete for every face with at most L off-floor slots. Levels are run in
/// ascending order until `max_level`, the arity or the wall budget; each finished level is appended to the
/// `checkpoint` file (JSON lines) and a resumed call skips the levels already there. Faces of the last finished level
/// are proved complete up to that level; beyond it the caller falls back to [`faces_from_uncovered_keys`].
/// Every checkpoint line carries [`LEVELS_CHECKPOINT_FORMAT`] and the [`partition_inputs_digest`] of its run; a
/// resume with other inputs (or an older line format) is REFUSED with `PartitionError::StaleCheckpoint` (re-audit
/// RA2-M4).
pub fn gap_partition_levels(
    src: &dyn DomainSource,
    frame: &NodeFrame,
    opts: &PartitionOptions,
    max_level: usize,
    checkpoint: Option<&std::path::Path>,
    progress: Option<&dyn Fn(&Progress)>,
) -> Result<Vec<LevelResult>, PartitionError> {
    let t0 = Instant::now();
    let inputs = partition_inputs_digest(src, frame, opts);
    let mut done: Vec<LevelResult> = vec![];
    if let Some(cp) = checkpoint
        && let Ok(text) = std::fs::read_to_string(cp)
    {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        for (li, line) in lines.iter().enumerate() {
            let lineno = li.saturating_add(1);
            let v: Json = match serde_json::from_str(line) {
                Ok(v) => v,
                // a torn LAST line (a kill during an old non-atomic append): that level is not done (audit-part-B
                // M2); a bad line before the last is an error
                Err(_) if lineno == lines.len() => break,
                Err(e) => {
                    return Err(PartitionError::Io(format!("checkpoint line {lineno}: {e}")));
                }
            };
            if v["format"].as_str() != Some(LEVELS_CHECKPOINT_FORMAT) {
                return Err(PartitionError::StaleCheckpoint(format!(
                    "checkpoint line {lineno}: format {:?}, expected {LEVELS_CHECKPOINT_FORMAT}",
                    v["format"]
                )));
            }
            if v["inputs"].as_str() != Some(inputs.as_str()) {
                return Err(PartitionError::StaleCheckpoint(format!(
                    "checkpoint line {lineno} was written for other inputs ({:?}; this run: {inputs})",
                    v["inputs"]
                )));
            }
            let fk = |x: &Json| -> Vec<FaceKey> {
                x.as_array()
                    .map(|a| {
                        a.iter()
                            .map(|f| {
                                let g = |k: usize| {
                                    f[k].as_array()
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|y| y.as_u64())
                                                .filter_map(|y| usize::try_from(y).ok())
                                                .collect()
                                        })
                                        .unwrap_or_default()
                                };
                                (g(0), g(1))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
            done.push(LevelResult {
                level: v["level"]
                    .as_u64()
                    .and_then(|x| usize::try_from(x).ok())
                    .unwrap_or(0),
                faces: fk(&v["faces"]),
                generic: fk(&v["generic"]),
                nodes: v["nodes"].as_u64().unwrap_or(0),
                millis: v["millis"].as_u64().unwrap_or(0),
                // a checkpoint line without the flag is not trusted as a proof
                proved: v["proved"].as_bool().unwrap_or(false),
                incomplete: v["incomplete"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["checkpoint line without a proof flag".into()]),
            });
        }
    }
    let start = done.last().map_or(0, |l| l.level.saturating_add(1));
    for level in start..=max_level.min(frame.arity()) {
        let mut o = opts.clone();
        o.max_off_floor = Some(level);
        if let Some(w) = opts.budget.wall {
            let left = w.checked_sub(t0.elapsed()).unwrap_or_default();
            if left.is_zero() {
                break;
            }
            o.budget.wall = Some(left);
        }
        let rep = gap_partition(src, frame, &o, progress)?;
        if rep.outcome.is_aborted() {
            break;
        }
        let incomplete = match &rep.outcome {
            Outcome::Partial { reasons, .. } | Outcome::Incomplete { reasons } => reasons.clone(),
            Outcome::Complete => vec![],
        };
        let off = |f: &GapFace| f.sd.len().saturating_add(f.sn.len());
        let lr = LevelResult {
            level,
            faces: rep
                .faces
                .iter()
                .filter(|f| off(f) <= level)
                .map(|f| f.key())
                .collect(),
            generic: rep
                .faces
                .iter()
                .filter(|f| off(f) <= level && f.kind == FaceKind::Generic)
                .map(|f| f.key())
                .collect(),
            nodes: rep.stats.nodes,
            millis: rep.millis,
            proved: incomplete.is_empty(),
            incomplete,
        };
        done.push(lr);
        if let Some(cp) = checkpoint {
            // ATOMIC rewrite (audit-part-B M2): every finished level to a temporary file, fsync, rename
            use std::io::Write;
            let io = |e: std::io::Error| PartitionError::Io(format!("{e}"));
            let mut text = String::new();
            for d in &done {
                text.push_str(
                    &json!({"format": LEVELS_CHECKPOINT_FORMAT, "inputs": inputs, "level": d.level,
                    "faces": d.faces, "generic": d.generic, "nodes": d.nodes,
                    "millis": d.millis, "proved": d.proved, "incomplete": d.incomplete})
                    .to_string(),
                );
                text.push('\n');
            }
            let tmp = cp.with_extension("tmp");
            let mut f = std::fs::File::create(&tmp).map_err(io)?;
            f.write_all(text.as_bytes()).map_err(io)?;
            f.sync_all().map_err(io)?;
            std::fs::rename(&tmp, cp).map_err(io)?;
        }
    }
    Ok(done)
}
