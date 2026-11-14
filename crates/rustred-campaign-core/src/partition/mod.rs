//! EXACT gap partition of the top sector of a node (plan 2.4.2, WP-I3): from the case domains alone, the whole
//! top-sector chart orthant (every line >= 1, every numerator <= 0, all powers, all ranks, no window) is split into
//! finitely many classes of per-slot states, labelled Covered (not listed), Uncovered (U) or Fragile (F, covered
//! except on the zero sets of live exception branches); U classes give the GENERIC GAP FACES with per-slot
//! thresholds, F classes are resolved into coupled LOCI by [`crate::loci`].
//!
//! Coverage here is UNION coverage (some domain contains the key), never first-match dispatch; the dispatch order is
//! recorded as a digest only. Interface: `plan/auto-campaign/interfaces/partition.md`.
//!
//! No floating point anywhere (test `partition_nofloat`). Every exact step is checked integer arithmetic; an overflow
//! or a work cap makes the affected literal test conservative (counted in `stats.unknown_literal_tests`), never a
//! decided verdict.

pub mod classes;
pub mod exact;
pub mod nf;

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

use crate::cover::{Dispatcher, RuleRef};
use crate::domain::CaseDomain;
use crate::key::{Key, NodeFrame};
pub use classes::{Class, EnumStats, Label, Node};
pub use exact::{ExactError, Range};
pub use nf::{NfRules, NfStats};
pub use crate::loci::contain::{Containment, Emptiness, contained_in, is_empty, shadowed};

/// Errors of the partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartitionError {
    Arity { expected: usize, got: usize },
    /// A domain's chart differs from the frame mask.
    ChartNotFrame { source: usize },
    Exact(ExactError),
    Eval(String),
    Unsupported(String),
    /// A key outside the top-sector chart orthant was given to the oracle or the census.
    NotChartKey(Key),
    Io(String),
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
}

/// A plain list of domains with their provenance.
#[derive(Clone, Debug, Default)]
pub struct DomainList {
    pub arity: usize,
    pub items: Vec<(RuleRef, CaseDomain)>,
}

impl DomainList {
    pub fn new(arity: usize) -> Self {
        DomainList { arity, items: vec![] }
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
        RuleRef {
            table: t,
            record: r,
            label: self.disp.tables()[t].labels[r],
        }
    }
}

/// How a domain with elided witness loci is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ElidedPolicy {
    /// As an ordinary domain (the checker's h11b reading; regression reference).
    Ordinary,
    /// FAIL CLOSED as the core's first-match dispatcher (ruling F1, R24 (6); the campaign's verdict): the record
    /// credits no key AND blocks every later record on its domain (the engine's search stops at a selected but
    /// unusable record). In union terms: the record is removed and its domain is subtracted from every domain
    /// after it in dispatch order. T178 order B: 409 + 21 open box keys.
    FailClosed,
    /// Removed (skipped as if absent; later records still cover its keys). T178 order B: identical to `Ordinary`,
    /// because the elided record's domain also occurs in another piece.
    Absent,
}

/// Which status tests multi-slot linear literals get.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LiteralTests {
    /// Exact integer feasibility for every linear literal (default).
    Exact,
    /// The checker's uenum_g statuses (regression of its class counts).
    ReferenceCompatible,
}

/// Work limits.
#[derive(Clone, Debug, Default)]
pub struct Budget {
    pub max_nodes: Option<u64>,
    pub wall: Option<Duration>,
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
}

impl Proof {
    pub fn tag(&self) -> &'static str {
        match self {
            Proof::ExactPartition => "exact_partition",
            Proof::Tested { .. } => "tested",
            Proof::ClosureLowerBound => "closure_lower_bound",
        }
    }
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
}

impl GapFace {
    pub fn key(&self) -> (Vec<usize>, Vec<usize>) {
        (self.sd.clone(), self.sn.clone())
    }
}

/// Outcome of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Outcome {
    Complete,
    BudgetExceeded,
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
    pub elided: ElidedPolicy,
    pub literal_tests: LiteralTests,
    pub millis: u64,
    pub state_unknown: u64,
    /// FailClosed: per elided source index, the partition of its BLOCKED region (domain minus the domains before
    /// it); its U / F classes are gaps of the node in addition to the main classes.
    pub blocked: Vec<(usize, PartitionReport)>,
}

/// Where a key falls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassHit {
    Covered,
    Uncovered,
    Fragile,
}

/// Exact census of a finite key set.
#[derive(Clone, Debug, Default)]
pub struct KeyCensus {
    pub n_keys: usize,
    /// (key, face, label of its class: Uncovered = generic gap, Fragile = locus gap).
    pub uncovered: Vec<(Key, (Vec<usize>, Vec<usize>), Label)>,
    /// Covered keys in U classes (on coupled case-equation loci).
    pub covered_in_u: Vec<Key>,
    /// Uncovered keys in C classes: must be empty (a soundness check).
    pub uncovered_in_c: Vec<Key>,
}

impl KeyCensus {
    pub fn n_generic(&self) -> usize {
        self.uncovered.iter().filter(|x| x.2 == Label::Uncovered).count()
    }
    pub fn n_locus(&self) -> usize {
        self.uncovered.iter().filter(|x| x.2 == Label::Fragile).count()
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
    let nf = nf::build(
        src,
        frame,
        &nf::NfOptions {
            elided: opts.elided,
            dedup: opts.dedup,
            subset: opts.rule_subset.clone(),
            factor_exceptions: opts.factor_exceptions,
            complement_of: None,
        },
    )?;
    let elided = nf.elided.clone();
    let mut rep = partition_nf(nf, opts, progress, t0)?;
    // FailClosed: each elided record blocks its domain minus the domains before it (first match stops there).
    // Its BLOCKED region = the uncovered set of {domains before it} u {complement of its domain}: a separate exact
    // partition whose classes are gaps of the node.
    let order: Vec<usize> = opts.rule_subset.clone().unwrap_or_else(|| (0..src.len()).collect());
    for e in elided {
        let pos = order.iter().position(|&x| x == e).expect("elided source in the order");
        let mut subset: Vec<usize> = order[..pos].to_vec();
        subset.push(e);
        let sub = nf::build(
            src,
            frame,
            &nf::NfOptions {
                elided: ElidedPolicy::FailClosed,
                dedup: opts.dedup,
                subset: Some(subset),
                factor_exceptions: opts.factor_exceptions,
                complement_of: Some(e),
            },
        )?;
        let b = partition_nf(sub, opts, None, Instant::now())?;
        if b.outcome == Outcome::BudgetExceeded {
            rep.outcome = Outcome::BudgetExceeded;
        }
        rep.blocked.push((e, b));
    }
    if !rep.blocked.is_empty() {
        rep.merge_blocked_faces();
    }
    rep.millis = t0.elapsed().as_millis() as u64;
    Ok(rep)
}

/// The partition of an already built normal form.
pub fn partition_nf(
    nf: NfRules,
    opts: &PartitionOptions,
    progress: Option<&dyn Fn(&Progress)>,
    t0: Instant,
) -> Result<PartitionReport, PartitionError> {
    let (states, state_unknown) = classes::slot_states(&nf, opts.extra_critical.as_deref())?;
    let (root, stats) = {
        let mut eng = classes::Engine::new(&nf, states.clone(), opts.literal_tests)?;
        eng.use_memo = opts.memo;
        eng.cap = opts.max_off_floor;
        eng.max_nodes = opts.budget.max_nodes;
        eng.deadline = opts.budget.wall.map(|w| t0 + w);
        let root = eng.run(progress);
        (root, eng.stats.clone())
    };
    let outcome = if stats.aborted {
        Outcome::BudgetExceeded
    } else {
        Outcome::Complete
    };
    let classes = classes::collect_classes(&root, nf.arity);
    let mut rep = PartitionReport {
        nf,
        states,
        root,
        classes,
        stats,
        faces: vec![],
        resolution: None,
        outcome,
        elided: opts.elided,
        literal_tests: opts.literal_tests,
        millis: 0,
        state_unknown,
        blocked: vec![],
    };
    if outcome == Outcome::Complete && opts.resolve_loci {
        rep.resolution = Some(crate::loci::resolve_fragile_classes(&rep, &opts.loci)?);
    }
    rep.faces = rep.build_faces();
    rep.millis = t0.elapsed().as_millis() as u64;
    Ok(rep)
}

/// Fallback face list from uncovered keys (R30): a LOWER BOUND, every face labelled `closure_lower_bound`.
pub fn faces_from_uncovered_keys(frame: &NodeFrame, keys: &[Key]) -> Vec<GapFace> {
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
                kind: FaceKind::Generic,
                sd,
                sn,
            }
        })
        .collect();
    out.sort_by_key(|f| (f.sd.len() + f.sn.len(), f.sd.clone(), f.sn.clone()));
    out
}

impl PartitionReport {
    pub fn arity(&self) -> usize {
        self.nf.arity
    }

    /// State index of a value in a slot.
    pub fn state_index(&self, slot: usize, v: i64) -> Option<u16> {
        self.states[slot].iter().position(|r| r.contains(v)).map(|i| i as u16)
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
        for (_, b) in &self.blocked {
            if !b.nf.covered(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The class of a chart key.
    pub fn class_of(&self, key: &[i32]) -> Result<ClassHit, PartitionError> {
        self.check_chart(key)?;
        let idx: Vec<u16> = key
            .iter()
            .enumerate()
            .map(|(i, &x)| self.state_index(i, x as i64).expect("states cover the chart"))
            .collect();
        let main = match classes::lookup(&self.root, &idx) {
            None => ClassHit::Covered,
            Some(Node::Leaf(Label::Uncovered)) => ClassHit::Uncovered,
            Some(_) => ClassHit::Fragile,
        };
        if main == ClassHit::Uncovered {
            return Ok(main);
        }
        let mut best = main;
        for (_, b) in &self.blocked {
            match b.class_of(key)? {
                ClassHit::Uncovered => return Ok(ClassHit::Uncovered),
                ClassHit::Fragile => best = ClassHit::Fragile,
                ClassHit::Covered => {}
            }
        }
        Ok(best)
    }

    /// Add the faces of the blocked regions (FailClosed) to the face list.
    fn merge_blocked_faces(&mut self) {
        let mut faces = std::mem::take(&mut self.faces);
        for (_, b) in &self.blocked {
            for f in &b.faces {
                match faces.iter_mut().find(|g| g.key() == f.key()) {
                    Some(g) => {
                        if f.kind == FaceKind::Generic && g.kind == FaceKind::LocusOnly {
                            g.kind = FaceKind::Generic;
                            g.proof = Proof::ExactPartition;
                        }
                        g.n_u_classes += f.n_u_classes;
                        g.generic_cells.extend(f.generic_cells.iter().cloned());
                        g.generic_cells.sort();
                        g.generic_cells.dedup();
                        g.unbounded_in_dots |= f.unbounded_in_dots;
                        g.unbounded_in_rank |= f.unbounded_in_rank;
                        if g.witness.is_none() {
                            g.witness = f.witness.clone();
                        }
                    }
                    None => faces.push(f.clone()),
                }
            }
        }
        faces.sort_by_key(|f| (f.sd.len() + f.sn.len(), f.sd.clone(), f.sn.clone()));
        self.faces = faces;
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
                (true, ClassHit::Uncovered) => c.uncovered.push((k.clone(), face_of(&self.nf.lines, k), Label::Uncovered)),
                (true, ClassHit::Fragile) => c.uncovered.push((k.clone(), face_of(&self.nf.lines, k), Label::Fragile)),
                (true, ClassHit::Covered) => c.uncovered_in_c.push(k.clone()),
                (false, ClassHit::Uncovered) => c.covered_in_u.push(k.clone()),
                _ => {}
            }
        }
        Ok(c)
    }

    pub fn n_u(&self) -> usize {
        self.classes.iter().filter(|c| c.label == Label::Uncovered).count()
    }
    pub fn n_f(&self) -> usize {
        self.classes.iter().filter(|c| c.label == Label::Fragile).count()
    }

    /// Expand the `any` slots of a class into (floor, any off-floor): (face, cell states over all slots).
    pub fn expand(&self, c: &Class) -> Vec<((Vec<usize>, Vec<usize>), Vec<CellState>)> {
        let n = self.nf.arity;
        let stars: Vec<usize> = (0..n).filter(|&i| c.states[i].is_none()).collect();
        let mut out = vec![];
        let combos = 1usize << stars.len().min(24);
        for bits in 0..combos {
            let mut cell: Vec<CellState> = c.states.iter().map(|s| s.unwrap_or(0)).collect();
            for (j, &s) in stars.iter().enumerate() {
                if (bits >> j) & 1 == 1 {
                    cell[s] = ANY_OFF;
                }
            }
            let sd: Vec<usize> = (0..n).filter(|&i| self.nf.lines[i] && cell[i] != 0).collect();
            let sn: Vec<usize> = (0..n).filter(|&i| !self.nf.lines[i] && cell[i] != 0).collect();
            out.push(((sd, sn), cell));
        }
        out
    }

    fn build_faces(&self) -> Vec<GapFace> {
        type FKey = (Vec<usize>, Vec<usize>);
        #[derive(Default)]
        struct Acc {
            u: BTreeSet<Vec<CellState>>,
            nu: usize,
            nf: usize,
            nfg: usize,
            loci: BTreeSet<String>,
            tested: bool,
        }
        let mut by: BTreeMap<FKey, Acc> = BTreeMap::new();
        for c in &self.classes {
            for (f, cell) in self.expand(c) {
                let a = by.entry(f.clone()).or_default();
                let fc: Vec<CellState> = f.0.iter().chain(&f.1).map(|&s| cell[s]).collect();
                match c.label {
                    Label::Uncovered => {
                        a.u.insert(fc);
                        a.nu += 1;
                    }
                    Label::Fragile => a.nf += 1,
                }
            }
        }
        if let Some(res) = &self.resolution {
            for l in &res.loci {
                let a = by.entry(l.face.clone()).or_default();
                a.nfg += l.n_classes;
                a.loci.insert(l.id.clone());
                if l.proof != Proof::ExactPartition {
                    a.tested = true;
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
                    range.clone().any(|j| c[j] == ANY_OFF || c[j] as usize + 1 == self.states[slots[j]].len())
                })
            };
            let mut unbounded_in_dots = unb(0..sd.len());
            let unbounded_in_rank = unb(sd.len()..slots.len());
            if let Some(res) = &self.resolution {
                for l in res.loci.iter().filter(|l| l.face == (sd.clone(), sn.clone())) {
                    if l.unbounded_in_dots {
                        unbounded_in_dots = true;
                    }
                }
            }
            let kind = if a.u.is_empty() { FaceKind::LocusOnly } else { FaceKind::Generic };
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
                proof: if kind == FaceKind::Generic || !a.tested {
                    Proof::ExactPartition
                } else {
                    Proof::Tested {
                        bound: "locus window".into(),
                    }
                },
            };
            face.witness = self.face_witness(&face);
            out.push(face);
        }
        out.sort_by_key(|f| (f.sd.len() + f.sn.len(), f.sd.clone(), f.sn.clone()));
        out
    }

    /// Smallest state from which the generic verdict is constant, per face slot.
    fn thresholds(&self, slots: &[usize], cells: &BTreeSet<Vec<CellState>>) -> Vec<(usize, i64)> {
        // expand ANY_OFF to every off-floor state
        let mut full: BTreeSet<Vec<u16>> = BTreeSet::new();
        for c in cells {
            let mut acc: Vec<Vec<u16>> = vec![vec![]];
            for (j, &s) in c.iter().enumerate() {
                let opts: Vec<u16> = if s == ANY_OFF {
                    (1..self.states[slots[j]].len() as u16).collect()
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
            let m = self.states[s].len() as u16;
            let mut t_found = m - 1;
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
            let r = self.states[s][t_found as usize];
            out.push((s, if self.nf.lines[s] { r.lo.expect("line state") } else { r.hi.expect("numerator state") }));
        }
        out
    }

    /// Representative values of a state, nearest to the floor first.
    fn reps(&self, slot: usize, st: CellState) -> Vec<i64> {
        let line = self.nf.lines[slot];
        let ranges: Vec<Range> = if st == ANY_OFF {
            self.states[slot][1..].to_vec()
        } else {
            vec![self.states[slot][st as usize]]
        };
        let mut v = vec![];
        for r in ranges {
            let base = if line { r.lo.expect("line") } else { r.hi.expect("num") };
            for d in 0..3 {
                let x = if line { base + d } else { base - d };
                if r.contains(x) {
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
            let per: Vec<Vec<i64>> = slots.iter().zip(cell).map(|(&s, &st)| self.reps(s, st)).collect();
            // walk a few combinations
            let mut idx = vec![0usize; per.len()];
            for _ in 0..64 {
                let mut key: Key = (0..self.nf.arity).map(|i| self.nf.floor(i) as i32).collect();
                for (j, &s) in slots.iter().enumerate() {
                    key[s] = per[j][idx[j]] as i32;
                }
                if self.uncovered(&key).ok() == Some(true) {
                    return Some(key);
                }
                // next combination (odometer)
                let mut j = 0;
                loop {
                    if j == idx.len() {
                        break;
                    }
                    idx[j] += 1;
                    if idx[j] < per[j].len() {
                        break;
                    }
                    idx[j] = 0;
                    j += 1;
                }
                if j == idx.len() {
                    break;
                }
            }
        }
        if let Some(res) = &self.resolution {
            for l in res.loci.iter().filter(|l| l.face == (f.sd.clone(), f.sn.clone())) {
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
            return if self.nf.lines[slot] { ">=2".into() } else { "<=-1".into() };
        }
        let r = self.states[slot][st as usize];
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
                    "n_U_classes": f.n_u_classes, "n_F_classes": f.n_f_classes, "n_F_with_gaps": f.n_f_with_gaps,
                    "witness": f.witness, "loci": f.loci, "proof": f.proof.tag(),
                })
            })
            .collect();
        let loci: Vec<Json> = self.resolution.as_ref().map(|r| r.loci_json()).unwrap_or_default();
        let unresolved: Vec<String> = self
            .resolution
            .as_ref()
            .map(|r| r.loci.iter().filter(|l| l.unresolved).map(|l| l.id.clone()).collect())
            .unwrap_or_default();
        json!({
            "schema": "rustred.campaign.partition.v1",
            "node": ids.node, "seq": ids.seq,
            "coverage": "union",
            "elided_policy": match self.elided { ElidedPolicy::Ordinary => "ordinary", ElidedPolicy::FailClosed => "fail_closed", ElidedPolicy::Absent => "absent" },
            "table_set_digest": ids.table_set_digest,
            "dispatch_order_digest": ids.dispatch_order_digest,
            "tool_versions": {"partition": concat!(env!("CARGO_PKG_VERSION"), "+part.1"),
                              "literal_tests": match self.literal_tests { LiteralTests::Exact => "exact", LiteralTests::ReferenceCompatible => "reference_compatible" }},
            "frame": {"arity": frame.arity(), "mask": frame.mask_string()},
            "inputs": self.nf.stats,
            "states": states,
            "stats": {"nodes": self.stats.nodes, "memo_hits": self.stats.memo_hits, "memo_size": self.stats.memo_size,
                      "U": self.n_u(), "F": self.n_f(), "pruned_off_floor": self.stats.pruned_off_floor,
                      "unknown_literal_tests": self.stats.unknown_literal_tests + self.state_unknown, "millis": self.millis},
            "outcome": match self.outcome { Outcome::Complete => if unresolved.is_empty() { "complete" } else { "unresolved" }, Outcome::BudgetExceeded => "budget_exceeded" },
            "faces": faces,
            "loci": loci,
            "unresolved": unresolved,
            "blocked": self.blocked.iter().map(|(e, b)| json!({"source": e, "U": b.n_u(), "F": b.n_f(),
                "nodes": b.stats.nodes, "faces": b.faces.len()})).collect::<Vec<_>>(),
        })
    }
}
