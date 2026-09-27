//! Offline closure oracle over a saved CP5 generation (plan W0.2, v3 note
//! F8/F10). Everything a closure claim rests on is re-derived here from the
//! raw sections and the exact reference reducer, never from the walker's
//! restore validators, containment index or dependency tracker:
//!
//! - the checkpoint is bound to the request (queries included) by its digest;
//!   roots are the query records, each re-checked to contain its query;
//! - record images equal the saved domain images; records seal only as
//!   (native with 0 frontiers and no error) or alias (F8), and the saved
//!   seal flags must agree;
//! - every alias is an exact integer-set subset of its representative in the
//!   same (phase, owner), and every partial record's D >= cut slice lies in
//!   its initial anchor, each with its dependency edge;
//! - closure is re-derived from the edge set (reverse reachability from
//!   unsealed nodes, cross-checked by forward cones per root); an engine
//!   closed flag that the oracle does not re-derive is a false claim;
//! - F10: natives are re-inspected with every walk lever off; frontier,
//!   error and event counts must match the record, and every successor must
//!   be contained, in the same phase and owner, in a recorded target of its
//!   parent or along that target's alias chain.
//!
//! Inclusion uses `lattice::Cell`, cross-checked by brute-force lattice
//! enumeration on small cells. Mutations (`--mutate`) inject one engine
//! defect in memory after loading; each must turn the verdict into FAIL.
mod graph;
mod lattice;

use super::super::{RoutedCampaignRequest, input, matching, prepare};
use super::{
    OwnerDomainWalkRequest, checkpoint,
    inspection::{self, Effect, Event},
    mask,
    queue::{Domain, Phase},
};
use crate::AppError;
use graph::Graph;
use lattice::Cell;
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
pub const OWNER_DOMAIN_WALK_VERIFY_SCHEMA: &str = "rustred.walk-verify-closure.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyReinspect {
    All,
    None,
    Sample { count: usize, seed: u64 },
}

/// One injected engine defect; the verdict must become FAIL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDomainWalkVerifyMutation {
    /// Drop the only recorded edge covering some successor.
    DroppedEdge,
    /// Point an alias (record and edge) at a same-bucket non-container.
    RetargetedAlias,
    /// Forget a native's frontiers and seal it.
    DroppedFrontierRecord,
    /// Seal a node whose record carries a frontier.
    SealWithFrontier,
    /// Seal a node whose record carries an error.
    SealWithError,
    /// Replace the only covering edge by one to a sealed non-container.
    InjectedFalseHit,
}
impl OwnerDomainWalkVerifyMutation {
    pub const ALL: [Self; 6] = [
        Self::DroppedEdge,
        Self::RetargetedAlias,
        Self::DroppedFrontierRecord,
        Self::SealWithFrontier,
        Self::SealWithError,
        Self::InjectedFalseHit,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::DroppedEdge => "dropped-edge",
            Self::RetargetedAlias => "retargeted-alias",
            Self::DroppedFrontierRecord => "dropped-frontier-record",
            Self::SealWithFrontier => "seal-with-frontier",
            Self::SealWithError => "seal-with-error",
            Self::InjectedFalseHit => "injected-false-hit",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
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
    /// Query ids containing this substring are reported as helpers.
    pub helper_pattern: String,
    pub max_violations: usize,
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
            helper_pattern: "anchor".into(),
            max_violations: 200,
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
    };
    fn native(&self) -> bool {
        matches!(self.kind, Kind::Native | Kind::Partial)
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
#[derive(Deserialize)]
struct StatsRow {
    events: Option<u64>,
    successors: Option<u64>,
}
#[derive(Deserialize)]
struct RecordRow {
    id: usize,
    record_kind: String,
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

/// Exact inclusion plus a budgeted brute-force enumeration cross-check.
struct Containment {
    max_points: u64,
    budget: u64,
    points: AtomicU64,
    exact: AtomicU64,
    brute: AtomicU64,
    disagreements: AtomicU64,
}
impl Containment {
    fn contains(&self, outer: &Cell, inner: &Cell) -> bool {
        self.exact.fetch_add(1, Ordering::Relaxed);
        let exact = outer.contains(inner);
        if self.max_points > 0
            && self.points.load(Ordering::Relaxed) < self.budget
            && let Some((brute, visited)) = outer.brute_force_contains(inner, self.max_points)
        {
            self.brute.fetch_add(1, Ordering::Relaxed);
            self.points.fetch_add(visited, Ordering::Relaxed);
            if brute != exact {
                self.disagreements.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        exact
    }
    fn json(&self) -> Value {
        json!({"exact_checks":self.exact.load(Ordering::Relaxed),
            "brute_force_checks":self.brute.load(Ordering::Relaxed),
            "brute_force_points":self.points.load(Ordering::Relaxed),
            "brute_force_max_points":self.max_points,"brute_force_point_budget":self.budget,
            "exact_vs_brute_force_disagreements":self.disagreements.load(Ordering::Relaxed)})
    }
}

struct Loaded<const N: usize> {
    raw: checkpoint::RawCheckpoint<N>,
    domains: Vec<Domain<N>>,
    nodes: Vec<Node>,
}

/// Verify one saved walk generation; returns the report (verdict inside).
pub fn owner_domain_walk_verify_closure(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => verify::<$n>(request, options, &selection, limits, cancellation, &observer),)*
        _ => Err(AppError::input("unsupported owner arity")),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn load<const N: usize>(
    options: &OwnerDomainWalkVerifyOptions,
    violations: &mut Violations,
) -> Result<Loaded<N>, String> {
    let raw = checkpoint::read_raw::<N>(&options.checkpoint)?;
    let total = raw.domains.len();
    let domains: Vec<Domain<N>> = raw.domains.iter().map(|d| d.expand()).collect();
    if raw.flags.len() != total {
        violations.add("structure", || {
            format!("{} node flags for {total} domains", raw.flags.len())
        });
    }
    let mut nodes = vec![Node::MISSING; total];
    let mut records = 0usize;
    for (path, count) in &raw.records {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut lines = 0usize;
        for line in BufReader::with_capacity(1 << 20, file).lines() {
            let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
            if line.is_empty() {
                continue;
            }
            lines += 1;
            let row: RecordRow = serde_json::from_str(&line)
                .map_err(|e| format!("{}: record line {lines}: {e}", path.display()))?;
            record_node(&row, &domains, &mut nodes, violations);
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
    })
}

fn record_node<const N: usize>(
    row: &RecordRow,
    domains: &[Domain<N>],
    nodes: &mut [Node],
    violations: &mut Violations,
) {
    let id = row.id;
    let Some(domain) = domains.get(id) else {
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
    if phase != Some(domain.phase)
        || row.owner != mask(&domain.owner)
        || row.lower != domain.lower
        || row.upper != domain.upper
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
    };
    match row.record_kind.as_str() {
        "native_inspection" => {}
        "partial_initial_overlap_inspection" => {
            node.kind = Kind::Partial;
            node.finished = row.residual_inspection_finished == Some(true);
            match &row.initial_overlap {
                Some(overlap) => {
                    node.link = overlap.anchor_id;
                    node.cut = overlap.cut;
                    let mut residual = domain.powers;
                    residual.max_power_difference = Some(
                        residual
                            .max_power_difference
                            .map_or(overlap.cut - 1, |d| d.min(overlap.cut - 1)),
                    );
                    if overlap.residual_power_bounds.bounds() != residual {
                        violations.add("partial_anchor", || {
                            format!("record {id} residual bounds are not the D < cut slice")
                        });
                    }
                }
                None => violations.add("partial_anchor", || {
                    format!("record {id} has no overlap link")
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

/// F8 on records alone: natives seal with 0 frontiers, no error and a
/// finished (residual) inspection plus their anchor edge; aliases seal with
/// their representative edge.
fn sealed(nodes: &[Node], graph: &Graph) -> Vec<bool> {
    nodes
        .iter()
        .enumerate()
        .map(|(id, node)| match node.kind {
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

struct Ctx<'a, const N: usize> {
    request: &'a OwnerDomainWalkRequest,
    reducer: &'a RoutedCandidateReducer<N>,
    loaded: &'a Loaded<N>,
    graph: &'a Graph,
    containment: &'a Containment,
    cancellation: &'a AtomicBool,
}

#[derive(Default)]
struct Tally {
    inspected: u64,
    events: u64,
    successors: u64,
    admits: u64,
    covered_direct: u64,
    covered_alias_chain: u64,
    uncovered: u64,
    frontiers: u64,
    errors: u64,
    native_seconds: f64,
}
impl Tally {
    fn add(&mut self, other: &Tally) {
        self.inspected += other.inspected;
        self.events += other.events;
        self.successors += other.successors;
        self.admits += other.admits;
        self.covered_direct += other.covered_direct;
        self.covered_alias_chain += other.covered_alias_chain;
        self.uncovered += other.uncovered;
        self.frontiers += other.frontiers;
        self.errors += other.errors;
        self.native_seconds += other.native_seconds;
    }
    fn json(&self) -> Value {
        json!({"inspected":self.inspected,"events":self.events,"successor_events":self.successors,
            "admitted_successors":self.admits,"covered_by_recorded_target":self.covered_direct,
            "covered_along_alias_chain":self.covered_alias_chain,"uncovered":self.uncovered,
            "frontiers":self.frontiers,"errors":self.errors,"native_seconds":self.native_seconds})
    }
}

/// The domain the walker inspected natively for `id`: the whole domain, or
/// a partial record's D < cut residual.
fn inspected_domain<const N: usize>(loaded: &Loaded<N>, id: usize) -> Domain<N> {
    let node = loaded.nodes[id];
    let mut domain = loaded.domains[id].clone();
    if node.kind == Kind::Partial {
        domain.powers.max_power_difference = Some(
            domain
                .powers
                .max_power_difference
                .map_or(node.cut - 1, |d| d.min(node.cut - 1)),
        );
    }
    domain
}

/// Outcome of one reference inspection: per successor, the out-edge
/// targets of the parent that contain it.
struct Reference<const N: usize> {
    events: u64,
    successors: u64,
    frontiers: u64,
    error: Option<String>,
    admits: Vec<(Domain<N>, Vec<usize>)>,
    seconds: f64,
}

fn reference<const N: usize>(ctx: &Ctx<'_, N>, id: usize, keep: bool) -> Reference<N> {
    let targets: Vec<(usize, Phase, Cell)> = ctx
        .graph
        .out(id)
        .iter()
        .map(|&t| {
            (
                t as usize,
                ctx.loaded.domains[t as usize].phase,
                cell(&ctx.loaded.domains[t as usize]),
            )
        })
        .collect();
    let mut out = Reference {
        events: 0,
        successors: 0,
        frontiers: 0,
        error: None,
        admits: Vec::new(),
        seconds: 0.0,
    };
    let domain = inspected_domain(ctx.loaded, id);
    let mut emit = |event: Event<N>| {
        out.events += event.count as u64;
        match event.effect {
            Effect::Admit {
                domain, successor, ..
            } => {
                out.successors += u64::from(successor);
                let inner = cell(&domain);
                let covering: Vec<usize> = targets
                    .iter()
                    .filter(|(_, phase, outer)| *phase == domain.phase && outer.contains(&inner))
                    .map(|(t, _, _)| *t)
                    .collect();
                if keep || covering.len() <= 1 {
                    out.admits.push((domain, covering));
                }
            }
            Effect::Frontier { successor, .. } => {
                out.successors += u64::from(successor);
                out.frontiers += 1;
            }
            Effect::KnownReuse { successor, .. }
            | Effect::PreAdmittedOrthantReuse { successor, .. } => {
                // Impossible with every lever off; counted so parity fails.
                out.successors += u64::from(successor);
                out.error
                    .get_or_insert_with(|| "reference emitted a reuse shortcut".into());
            }
            Effect::Count | Effect::Optional(_) => {}
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
    out.seconds = finished.seconds;
    if out.error.is_none() {
        out.error = finished.error;
    }
    out
}

fn reinspect<const N: usize>(
    ctx: &Ctx<'_, N>,
    id: usize,
    tally: &mut Tally,
    violations: &mut Violations,
) {
    let node = ctx.loaded.nodes[id];
    let domain = inspected_domain(ctx.loaded, id);
    let targets: Vec<(usize, Phase, Cell)> = ctx
        .graph
        .out(id)
        .iter()
        .map(|&t| {
            (
                t as usize,
                ctx.loaded.domains[t as usize].phase,
                cell(&ctx.loaded.domains[t as usize]),
            )
        })
        .collect();
    let mut events = 0u64;
    let mut successors = 0u64;
    let mut frontiers = 0u64;
    let mut shortcut = false;
    let mut recent = 0usize;
    let mut local = Tally::default();
    let mut emit = |event: Event<N>| {
        events += event.count as u64;
        match event.effect {
            Effect::Admit {
                domain: successor_domain,
                successor,
                ..
            } => {
                successors += u64::from(successor);
                local.admits += 1;
                let inner = cell(&successor_domain);
                let fits = |(_, phase, outer): &(usize, Phase, Cell)| {
                    *phase == successor_domain.phase && ctx.containment.contains(outer, &inner)
                };
                if recent < targets.len() && fits(&targets[recent]) {
                    local.covered_direct += 1;
                } else if let Some(index) = targets.iter().position(fits) {
                    recent = index;
                    local.covered_direct += 1;
                } else if alias_chain_covers(ctx, &targets, &successor_domain, &inner) {
                    local.covered_alias_chain += 1;
                } else {
                    local.uncovered += 1;
                    violations.add("successor_uncovered", || {
                        format!(
                            "node {id}: {:?} successor {} {:?}..{:?} r{:?} {:?} is contained in no recorded target of {} edges",
                            successor_domain.phase,
                            mask(&successor_domain.owner),
                            successor_domain.lower,
                            successor_domain.upper,
                            successor_domain.rank,
                            successor_domain.powers,
                            targets.len()
                        )
                    });
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
    let finished = inspection::inspect_reference(
        ctx.reducer,
        &domain,
        ctx.request,
        ctx.cancellation,
        &mut emit,
    );
    local.inspected = 1;
    local.events = events;
    local.successors = successors;
    local.frontiers = frontiers;
    local.errors = u64::from(finished.error.is_some());
    local.native_seconds = finished.seconds;
    tally.add(&local);
    if shortcut {
        violations.add("reinspection", || {
            format!("node {id}: reference emitted a reuse shortcut")
        });
    }
    if finished.error.is_some() != node.error {
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
    if finished.error.is_none() && node.events != Some(events) {
        violations.add("event_parity", || {
            format!(
                "node {id}: record events {:?}, reference {events}",
                node.events
            )
        });
    }
    if finished.error.is_none()
        && domain.phase == Phase::Apply
        && node.successors != Some(successors)
    {
        violations.add("event_parity", || {
            format!(
                "node {id}: record successors {:?}, reference {successors}",
                node.successors
            )
        });
    }
}

/// A successor contained in a node reached from a recorded target through
/// alias representatives (the target's responsibility moved there).
fn alias_chain_covers<const N: usize>(
    ctx: &Ctx<'_, N>,
    targets: &[(usize, Phase, Cell)],
    successor: &Domain<N>,
    inner: &Cell,
) -> bool {
    let nodes = &ctx.loaded.nodes;
    targets.iter().any(|&(target, _, _)| {
        let mut current = target;
        let mut hops = 0;
        while nodes[current].kind == Kind::Alias && nodes[current].link < nodes.len() && hops < 64 {
            current = nodes[current].link;
            hops += 1;
            let domain = &ctx.loaded.domains[current];
            if domain.phase == successor.phase && ctx.containment.contains(&cell(domain), inner) {
                return true;
            }
        }
        false
    })
}

fn verify<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Value, AppError> {
    let started = Instant::now();
    let mut violations = Violations {
        limit: options.max_violations,
        ..Violations::default()
    };
    let queries = matching::input::parse(
        &request.matching.queries_json,
        N,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    let mut loaded = load::<N>(options, &mut violations).map_err(AppError::input)?;
    let loaded_seconds = started.elapsed().as_secs_f64();
    observer(
        json!({"event":"verify_loaded","domains":loaded.domains.len(),
        "edges":loaded.raw.edges.len(),"seconds":loaded_seconds}),
    );
    let binding = checkpoint::request_binding(request);
    let bound = binding == loaded.raw.request;
    if !bound {
        violations.add("binding", || {
            "checkpoint request digest differs from the command's request/queries binding".into()
        });
    }
    let prepare_started = Instant::now();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let saved_owners = loaded.raw.owners.clone();
    let mut owners_match = None;
    let mut bind = |owners: Vec<String>| {
        owners_match = Some(owners == saved_owners);
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
    if owners_match != Some(true) {
        violations.add("binding", || {
            "owner payload digests differ from the checkpoint".into()
        });
    }
    let prepare_seconds = prepare_started.elapsed().as_secs_f64();
    let containment = Containment {
        max_points: options.brute_force_max_points,
        budget: options.brute_force_point_budget,
        points: AtomicU64::new(0),
        exact: AtomicU64::new(0),
        brute: AtomicU64::new(0),
        disagreements: AtomicU64::new(0),
    };
    let mutation = match options.mutation {
        Some(kind) => {
            let graph = Graph::from_edges(loaded.domains.len(), &loaded.raw.edges)
                .map_err(AppError::input)?;
            let ctx = Ctx {
                request,
                reducer: &reducer,
                loaded: &loaded,
                graph: &graph,
                containment: &containment,
                cancellation,
            };
            let plan = plan_mutation(&ctx, kind);
            drop(ctx);
            Some(apply_mutation(&mut loaded, kind, plan))
        }
        None => None,
    };
    let checks_started = Instant::now();
    let total = loaded.domains.len();
    let graph = Graph::from_edges(total, &loaded.raw.edges).map_err(AppError::input)?;
    let sealed = sealed(&loaded.nodes, &graph);
    let nodes = &loaded.nodes;
    let flags = &loaded.raw.flags;
    let mut counts = BTreeMap::<&'static str, u64>::new();
    for (id, node) in nodes.iter().enumerate() {
        let flag = flags.get(id).copied().unwrap_or(0);
        *counts
            .entry(match node.kind {
                Kind::Missing => "unpublished",
                Kind::Native => "natives",
                Kind::Partial => "partials",
                Kind::Alias => "aliases",
            })
            .or_default() += 1;
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
                if inner.phase != outer.phase || !containment.contains(&cell(outer), &cell(inner)) {
                    violations.add("alias_containment", || {
                        format!(
                            "alias {id} is not contained in its representative {}",
                            node.link
                        )
                    });
                }
            }
            Kind::Partial => {
                if node.link >= total || !graph.has_edge(id, node.link) {
                    violations.add("missing_edge", || {
                        format!("partial {id} has no edge to anchor {}", node.link)
                    });
                    continue;
                }
                let anchor = &loaded.domains[node.link];
                let mut high = cell(&loaded.domains[id]);
                high.powers.min_power_difference = Some(
                    high.powers
                        .min_power_difference
                        .map_or(node.cut, |d| d.max(node.cut)),
                );
                if node.link >= loaded.raw.counters[10]
                    || !nodes[node.link].native()
                    || anchor.phase != Phase::Apply
                    || !containment.contains(&cell(anchor), &high)
                {
                    violations.add("partial_anchor", || {
                        format!(
                            "partial {id}: D >= {} slice not covered by initial native anchor {}",
                            node.cut, node.link
                        )
                    });
                }
            }
            _ => {}
        }
    }
    // Global parity against the saved counters: every retained frontier and
    // failure is explicit in a record, an input or a carried receipt.
    let [
        events_counter,
        _,
        _,
        _,
        _,
        frontier_counter,
        completed_counter,
        native_counter,
        ..,
    ] = loaded.raw.counters;
    let record_frontiers: u64 = nodes.iter().map(|n| u64::from(n.frontiers)).sum();
    let carried_frontiers: u64 = loaded
        .raw
        .uncommitted
        .iter()
        .map(|u| u["frontiers"].as_array().map_or(0, |a| a.len() as u64))
        .sum();
    let explicit_frontiers =
        record_frontiers + loaded.raw.input_frontiers.len() as u64 + carried_frontiers;
    if frontier_counter as u64 != explicit_frontiers {
        violations.add("frontier_parity", || {
            format!("saved frontier counter {frontier_counter} != {explicit_frontiers} explicit frontiers")
        });
    }
    let natives = nodes.iter().filter(|n| n.native()).count();
    let completed = nodes.iter().filter(|n| n.native() && !n.error).count();
    if native_counter != natives || completed_counter != completed {
        violations.add("error_parity", || {
            format!(
                "saved native/completed counters {native_counter}/{completed_counter} != records {natives}/{completed}"
            )
        });
    }
    let record_events: u64 = nodes
        .iter()
        .filter(|n| n.native())
        .map(|n| n.events.unwrap_or(0))
        .sum();
    if loaded.raw.uncommitted.is_empty() && events_counter as u64 != record_events {
        violations.add("event_parity", || {
            format!("saved event counter {events_counter} != {record_events} record events")
        });
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
    // Roots: every query's record, authenticated by the request binding.
    let initial_count = loaded.raw.counters[10];
    let mut root_of_query = Vec::new();
    let mut admitting = BTreeMap::<usize, usize>::new();
    if loaded.raw.inputs.len() != queries.len() {
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
        let record = entry.and_then(|e| e["domain"].as_u64()).map(|r| r as usize);
        if entry.and_then(|e| e["id"].as_str()) != Some(query.id.as_str()) {
            violations.add("root_mapping", || {
                format!("saved input {index} is not query {}", query.id)
            });
        }
        let Some(record) = record.filter(|&r| r < initial_count && r < total) else {
            violations.add("root_mapping", || {
                format!("query {} has no initial record", query.id)
            });
            root_of_query.push(None);
            continue;
        };
        let query_cell = Cell {
            owner: query.owner.clone(),
            lower: query.lower.clone(),
            upper: query.upper.clone(),
            rank: query.rank,
            powers: query.powers,
        };
        let record_cell = cell(&loaded.domains[record]);
        let first = *admitting.entry(record).or_insert(index) == index;
        let ok = if first {
            record_cell == query_cell
        } else {
            containment.contains(&record_cell, &query_cell)
        };
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
    let checks_seconds = checks_started.elapsed().as_secs_f64();
    // F10 reference re-inspection.
    let reinspect_started = Instant::now();
    let candidates: Vec<usize> = (0..total).filter(|&id| nodes[id].native()).collect();
    let selected: Vec<usize> = match options.reinspect {
        OwnerDomainWalkVerifyReinspect::All => candidates.clone(),
        OwnerDomainWalkVerifyReinspect::None => Vec::new(),
        OwnerDomainWalkVerifyReinspect::Sample { count, seed } => sample(&candidates, count, seed),
    };
    let mut reinspected = vec![false; total];
    for &id in &selected {
        reinspected[id] = true;
    }
    let ctx = Ctx {
        request,
        reducer: &reducer,
        loaded: &loaded,
        graph: &graph,
        containment: &containment,
        cancellation,
    };
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let shared = Mutex::new((
        Tally::default(),
        Violations {
            limit: options.max_violations,
            ..Violations::default()
        },
    ));
    std::thread::scope(|scope| {
        for _ in 0..options.threads.max(1) {
            scope.spawn(|| {
                let mut tally = Tally::default();
                let mut local = Violations {
                    limit: options.max_violations,
                    ..Violations::default()
                };
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= selected.len() || cancellation.load(Ordering::Relaxed) {
                        break;
                    }
                    reinspect(&ctx, selected[index], &mut tally, &mut local);
                    done.fetch_add(1, Ordering::Relaxed);
                }
                let mut guard = shared.lock().expect("verifier tally");
                guard.0.add(&tally);
                guard.1.merge(local);
            });
        }
        let mut last = Instant::now();
        while done.load(Ordering::Relaxed) < selected.len() && !cancellation.load(Ordering::Relaxed)
        {
            std::thread::sleep(Duration::from_millis(200));
            if last.elapsed() >= Duration::from_secs(10) {
                last = Instant::now();
                observer(
                    json!({"event":"verify_progress","reinspected":done.load(Ordering::Relaxed),
                    "selected":selected.len(),"seconds":reinspect_started.elapsed().as_secs_f64()}),
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
    let reinspect_seconds = reinspect_started.elapsed().as_secs_f64();
    // Roots, cones and certification.
    let mut roots = BTreeMap::<usize, Vec<&str>>::new();
    for (query, root) in queries.iter().zip(&root_of_query) {
        if let Some(root) = root {
            roots.entry(*root).or_default().push(query.id.as_str());
        }
    }
    let cone_budget = (roots.len() as u128) * (total as u128) <= 20_000_000_000;
    let mut mark = vec![0u32; if cone_budget { total } else { 0 }];
    let mut root_rows = Vec::new();
    let mut root_state = BTreeMap::new();
    for (stamp, (&root, ids)) in roots.iter().enumerate() {
        let mut row = json!({"record":root,"queries":ids,"oracle_closed":closed[root],
            "engine_closed":flags.get(root).is_some_and(|f| f & FLAG_CLOSED != 0)});
        let mut fully_reinspected = false;
        if cone_budget {
            let (size, unsealed, missing) = cone_with_reinspection(
                &graph,
                root,
                &sealed,
                &reinspected,
                nodes,
                &mut mark,
                stamp as u32 + 1,
            );
            if (unsealed == 0) != closed[root] {
                violations.add("closure_derivation", || {
                    format!("root {root}: forward cone and reverse reachability disagree")
                });
            }
            fully_reinspected = missing == 0;
            row["cone_nodes"] = json!(size);
            row["cone_unsealed"] = json!(unsealed);
            row["cone_natives_not_reinspected"] = json!(missing);
        }
        row["independently_verified"] = json!(closed[root] && fully_reinspected);
        root_state.insert(root, (closed[root], closed[root] && fully_reinspected));
        root_rows.push(row);
    }
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
    let consistent = violations.is_empty();
    let mut classes = BTreeMap::<&str, BTreeMap<&str, u64>>::new();
    for (index, (query, root)) in queries.iter().zip(&root_of_query).enumerate() {
        let class = if query.id.contains(options.helper_pattern.as_str()) {
            "helper"
        } else {
            "physics"
        };
        let entry = classes.entry(class).or_default();
        *entry.entry("total").or_default() += 1;
        let state = root
            .and_then(|r| root_state.get(&r))
            .copied()
            .unwrap_or((false, false));
        let admitted = root.is_some_and(|r| admitting.get(&r) == Some(&index));
        *entry
            .entry(if admitted { "admitting" } else { "absorbed" })
            .or_default() += 1;
        *entry.entry("oracle_closed").or_default() += u64::from(state.0);
        *entry.entry("certified").or_default() += u64::from(consistent && state.0);
        *entry.entry("independently_verified").or_default() += u64::from(consistent && state.1);
    }
    let verdict = if consistent { "PASS" } else { "FAIL" };
    Ok(json!({
        "schema": OWNER_DOMAIN_WALK_VERIFY_SCHEMA,
        "verdict": verdict,
        "closure_required": options.require_closure,
        "family_closure_claim": false,
        "scope": "re-derived dependency closure and reference re-inspection coverage of saved natives; not IBP replay, descent or termination",
        "checkpoint": {"directory": options.checkpoint, "generation": loaded.raw.generation,
            "publication_policy": loaded.raw.publication_policy,
            "walk_semantics_version": loaded.raw.walk_semantics_version,
            "executable": loaded.raw.executable, "request_digest": loaded.raw.request,
            "request_binding_matches": bound, "owner_digests_match": owners_match,
            "file_digest_verify_seconds": loaded.raw.verify_seconds,
            "engine_closure": loaded.raw.closure},
        "queries": {"count": queries.len(),
            "blake3": blake3::hash(request.matching.queries_json.as_bytes()).to_hex().to_string()},
        "counts": {"domains": total, "edges": loaded.raw.edges.len(),
            "duplicate_edges": graph.duplicate_edges(), "records": counts,
            "sealed": sealed.iter().filter(|&&s| s).count(),
            "oracle_closed": closed.iter().filter(|&&c| c).count(),
            "engine_closed": engine_closed,
            "engine_open_oracle_closed": engine_open_oracle_closed,
            "initial_records": initial_count, "initial_closed_oracle": initial_closed_oracle},
        "roots": root_rows,
        "certification": {"helper_pattern": options.helper_pattern, "classes": classes,
            "claim_levels": "certified = consistent and re-derived closed; independently_verified = certified and every native in the cone re-inspected"},
        "reinspection": {"mode": format!("{:?}", options.reinspect), "selected": selected.len(),
            "candidates": candidates.len(), "threads": options.threads, "tally": tally.json()},
        "containment": containment.json(),
        "mutation": mutation,
        "violations": violations.list,
        "violations_by_class": violations.by_class,
        "violations_suppressed": violations.suppressed,
        "timing": {"load_seconds": loaded_seconds, "prepare_seconds": prepare_seconds,
            "checks_seconds": checks_seconds, "reinspect_seconds": reinspect_seconds,
            "total_seconds": started.elapsed().as_secs_f64()},
    }))
}

fn cone_with_reinspection(
    graph: &Graph,
    root: usize,
    sealed: &[bool],
    reinspected: &[bool],
    nodes: &[Node],
    mark: &mut [u32],
    stamp: u32,
) -> (usize, usize, usize) {
    let (size, unsealed) = graph.cone(root, sealed, mark, stamp);
    let missing = (0..graph.nodes())
        .filter(|&id| mark[id] == stamp && nodes[id].native() && !reinspected[id])
        .count();
    (size, unsealed, missing)
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
    },
}

fn plan_mutation<const N: usize>(ctx: &Ctx<'_, N>, kind: OwnerDomainWalkVerifyMutation) -> Plan {
    use OwnerDomainWalkVerifyMutation as M;
    let nodes = &ctx.loaded.nodes;
    let flags = &ctx.loaded.raw.flags;
    let flagged_sealed = |id: usize| flags.get(id).is_some_and(|f| f & FLAG_SEALED != 0);
    match kind {
        M::DroppedEdge | M::InjectedFalseHit => {
            // The first native (by ID) with a successor covered by exactly
            // one recorded target: that edge is load-bearing.
            for id in (0..nodes.len())
                .filter(|&id| nodes[id].native() && !ctx.graph.out(id).is_empty())
                .take(20_000)
            {
                let found = reference(ctx, id, false);
                let Some((successor, covering)) =
                    found.admits.into_iter().find(|(_, c)| c.len() == 1)
                else {
                    continue;
                };
                let target = covering[0];
                let label = format!(
                    "{:?} {} {:?}..{:?} r{:?} {:?}",
                    successor.phase,
                    mask(&successor.owner),
                    successor.lower,
                    successor.upper,
                    successor.rank,
                    successor.powers
                );
                if kind == M::DroppedEdge {
                    return Plan::DropEdge {
                        source: id,
                        target,
                        successor: label,
                    };
                }
                let inner = cell(&successor);
                let replacement = (0..nodes.len())
                    .filter(|&t| t != target && !ctx.graph.has_edge(id, t))
                    .filter(|&t| {
                        ctx.loaded.domains[t].phase == successor.phase
                            && ctx.loaded.domains[t].owner == successor.owner
                    })
                    .filter(|&t| !cell(&ctx.loaded.domains[t]).contains(&inner))
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
            Plan::NotApplicable("no load-bearing edge found".into())
        }
        M::RetargetedAlias => {
            for id in (0..nodes.len()).filter(|&id| nodes[id].kind == Kind::Alias) {
                let inner = cell(&ctx.loaded.domains[id]);
                let from = nodes[id].link;
                let domain = &ctx.loaded.domains[id];
                if let Some(to) = (id + 1..nodes.len()).find(|&t| {
                    t != from
                        && ctx.loaded.domains[t].phase == domain.phase
                        && ctx.loaded.domains[t].owner == domain.owner
                        && !cell(&ctx.loaded.domains[t]).contains(&inner)
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
        M::DroppedFrontierRecord => (0..nodes.len())
            .find(|&id| nodes[id].native() && nodes[id].frontiers > 0 && !nodes[id].error)
            .map_or(
                Plan::NotApplicable("no native record carries a frontier".into()),
                |id| Plan::Node { id },
            ),
        M::SealWithFrontier => (0..nodes.len())
            .find(|&id| nodes[id].native() && nodes[id].frontiers > 0)
            .or_else(|| (0..nodes.len()).find(|&id| nodes[id].native() && flagged_sealed(id)))
            .map_or(Plan::NotApplicable("no native record".into()), |id| {
                Plan::Node { id }
            }),
        M::SealWithError => (0..nodes.len())
            .find(|&id| nodes[id].native() && flagged_sealed(id))
            .map_or(
                Plan::NotApplicable("no sealed native record".into()),
                |id| Plan::Node { id },
            ),
    }
}

fn apply_mutation<const N: usize>(
    loaded: &mut Loaded<N>,
    kind: OwnerDomainWalkVerifyMutation,
    plan: Plan,
) -> Value {
    use OwnerDomainWalkVerifyMutation as M;
    let mut report = json!({"kind": kind.name()});
    match plan {
        Plan::NotApplicable(reason) => {
            report["applied"] = json!(false);
            report["reason"] = json!(reason);
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
            report["applied"] = json!(true);
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
            if kind == M::RetargetedAlias {
                loaded.nodes[source].link = to;
            }
            report["applied"] = json!(true);
            report["edge"] = json!([source, from]);
            report["retargeted_to"] = json!(to);
            report["successor"] = json!(successor);
        }
        Plan::Node { id } => {
            let node = &mut loaded.nodes[id];
            let flag = &mut loaded.raw.flags[id];
            match kind {
                M::DroppedFrontierRecord => {
                    report["dropped_frontiers"] = json!(node.frontiers);
                    node.frontiers = 0;
                    *flag |= FLAG_SEALED;
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
                _ => unreachable!("node mutation kind"),
            }
            report["applied"] = json!(true);
            report["node"] = json!(id);
        }
    }
    report
}

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
        let mut nodes = vec![Node::MISSING; 5];
        let mut violations = Violations {
            limit: 50,
            ..Violations::default()
        };
        record_node(
            &row(0, "native_inspection", &domains[0], json!({})),
            &domains,
            &mut nodes,
            &mut violations,
        );
        record_node(
            &row(
                1,
                "delegated_not_inspected",
                &domains[1],
                json!({"representative_id":2}),
            ),
            &domains,
            &mut nodes,
            &mut violations,
        );
        record_node(
            &row(
                2,
                "native_inspection",
                &domains[2],
                json!({"frontiers":[{"kind":"x"}]}),
            ),
            &domains,
            &mut nodes,
            &mut violations,
        );
        record_node(
            &row(
                3,
                "native_inspection",
                &domains[3],
                json!({"error":"boom","accepted_events":4}),
            ),
            &domains,
            &mut nodes,
            &mut violations,
        );
        assert!(violations.by_class.contains_key("accepted_events"));
        let mut wrong = row(4, "native_inspection", &domains[4], json!({}));
        wrong.upper = vec![Some(5), Some(2)];
        record_node(&wrong, &domains, &mut nodes, &mut violations);
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
    fn mutation_names_round_trip_and_samples_are_deterministic() {
        for kind in OwnerDomainWalkVerifyMutation::ALL {
            assert_eq!(
                OwnerDomainWalkVerifyMutation::parse(kind.name()),
                Some(kind)
            );
        }
        let pool: Vec<usize> = (0..100).collect();
        assert_eq!(sample(&pool, 10, 3), sample(&pool, 10, 3));
        assert_eq!(sample(&pool, 10, 3).len(), 10);
        assert_eq!(sample(&pool, 1000, 3), pool);
    }
}
