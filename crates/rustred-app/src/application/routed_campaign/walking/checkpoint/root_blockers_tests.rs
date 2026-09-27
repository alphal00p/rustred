//! Root-closure blockers: a read-only diagnostic on a copied production CP5
//! checkpoint. The checkpoint is restored exactly as
//! `restore_copied_production_checkpoint` restores it (request and owner
//! binding, every section digest, decode and validation; no walk, no native
//! owner import, no save), then the dependency graph is walked forward from
//! every initial domain ("root").
//!
//! Semantics (`descendant_closure.rs`): an edge `(source, target)` means the
//! source waits on the target (a successor admission or reuse, an alias to
//! its containing representative, or a partial inspection's initial anchor).
//! A node is sealed once its whole callback stream was accepted with no
//! frontier. The monitor marks every ancestor of an unsealed node blocked and
//! calls the rest closed, so a root is closed iff no unsealed node is
//! forward-reachable from it. The analysis checks this against the persisted
//! CLOSED flag of every root before reporting anything else.
//!
//! Three forward walks per root:
//! - `full`: every reachable node; its unsealed nodes are the root's blockers.
//! - `private`: stops at other roots (recorded as root dependencies).
//! - `projected`: also stops at every non-root node contained in the
//!   comparison input's initial domain of the same phase and owner (the node
//!   a run with those inputs would have aliased into that initial domain),
//!   recorded as a dependency on that comparison root.
//!
//! Blockers are histogrammed by (phase, owner, finite/unbounded A, rank), BFS
//! depth, ledger status, Ready queue position and containment class: whether
//! the checkpoint's or the comparison input's initial domain of the same
//! phase and owner contains the node, by `DomainPowerSummary` inclusion (the
//! unlimited admission lane's authority), with the syntactic
//! `Domain::contains` alongside.
//!
//! The comparison initial domains come from simulating the walk's fresh-start
//! initial admission (`walking::run`) over another query document with this
//! request's selection and policy; the same simulation over the checkpoint's
//! own queries must reproduce the restored initial domains and input map.
//!
//! ```text
//! RUSTRED_CHECKPOINT_RESTORE_DIRECTORY=<fresh copy of checkpoints/main> \
//! RUSTRED_CHECKPOINT_RESTORE_REQUEST=<campaign run>/request.json \
//! RUSTRED_CHECKPOINT_RESTORE_RECEIPT=<new file> \
//! [RUSTRED_CHECKPOINT_RESTORE_{MANIFEST,OWNER_BASE,QUERIES}=<identical copies>] \
//! RUSTRED_ROOT_BLOCKERS_COMPARISON_QUERIES=<other campaign's queries.json> \
//! [RUSTRED_ROOT_BLOCKERS_DETAIL_ROOTS=25,35,42,43] \
//! [RUSTRED_ROOT_BLOCKERS_COMPARISON_CLOSED=<root ids closed in that run>] \
//! [RUSTRED_ROOT_BLOCKERS_THREADS=16] \
//! cargo test --release -p rustred-app --lib root_closure_blockers -- --ignored --nocapture
//! ```
//!
//! Scope: one snapshot of discovered-dependency coverage; no closure,
//! descent, family or ETA claim. The projection is a counterfactual on this
//! graph, not a replay of the comparison run.
use super::super::{
    super::{input, matching},
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy,
    delegation::Ledger,
    descendant_closure::Tracker,
    mask, power_bounds_json,
    queue::{CompactDomain, Domain, Phase, Queue},
};
use super::restore::Restored;
use super::scale_tests::{RestoredAnalysis, memory, restore_copied_checkpoint};
use rustred::solver::DomainPowerSummary;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const COMPARISON_QUERIES: &str = "RUSTRED_ROOT_BLOCKERS_COMPARISON_QUERIES";
const DETAIL_ROOTS: &str = "RUSTRED_ROOT_BLOCKERS_DETAIL_ROOTS";
const COMPARISON_CLOSED: &str = "RUSTRED_ROOT_BLOCKERS_COMPARISON_CLOSED";
const THREADS: &str = "RUSTRED_ROOT_BLOCKERS_THREADS";

/// Persisted node flags (`descendant_closure.rs`).
const SEALED: u8 = 1;
const INSPECTED: u8 = 2;
const CLOSED: u8 = 4;

/// Per-node containment bits.
const IN_CHECKPOINT: u8 = 1;
const IN_COMPARISON: u8 = 2;
const IN_CHECKPOINT_SYNTACTIC: u8 = 4;
const IN_COMPARISON_SYNTACTIC: u8 = 8;
const A_FINITE: u8 = 16;
const ROUTE: u8 = 32;
const SUMMARY_ERROR: u8 = 64;
/// No containing initial domain (initial ids stay below it).
const NO_CONTAINER: u8 = u8::MAX;

/// Containment class of a node, as reported.
const CLASS_NAMES: [&str; 7] = [
    "initial_domain",
    "in_checkpoint_and_comparison_initial",
    "in_comparison_initial_only",
    "in_checkpoint_initial_only",
    "in_neither_initial",
    "route_phase",
    "summary_error",
];

/// Ledger status of a node, as reported.
const STATUS_NAMES: [&str; 5] = [
    "unreserved_or_started",
    "reserved",
    "delegated_unpublished",
    "published",
    "no_ledger",
];

/// Histogram entries kept per key family (the rest is summed as `other`).
const TOP: usize = 64;
/// First-hop successors listed individually per detail root.
const FIRST_HOP_LISTED: usize = 256;

fn seconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}

fn summary<const N: usize>(domain: &Domain<N>) -> Result<DomainPowerSummary<N>, String> {
    DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        domain.powers,
    )
    .map_err(|e| e.to_string())
}

fn owner_bits<const N: usize>(owner: &[bool; N]) -> u32 {
    owner
        .iter()
        .enumerate()
        .fold(0, |bits, (axis, &active)| bits | u32::from(active) << axis)
}

fn owner_mask<const N: usize>(bits: u32) -> String {
    mask::<N>(&std::array::from_fn(|axis| bits >> axis & 1 == 1))
}

fn wide(value: u128) -> Value {
    u64::try_from(value).map_or_else(|_| json!(value.to_string()), |v| json!(v))
}

/// A domain with its tight A/R/D extrema (None upper = unbounded).
fn domain_json<const N: usize>(domain: &Domain<N>) -> Value {
    let mut value = json!({"phase":format!("{:?}", domain.phase),"owner":mask(&domain.owner),
        "lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
        "power_bounds":power_bounds_json(domain.powers)});
    match summary(domain) {
        Ok(s) => match s.extrema() {
            Some(e) => {
                let (a, a_hi) = e.positive_power();
                let (r, r_hi) = e.numerator_rank();
                let (d, d_hi) = e.power_difference();
                value["a_extrema"] = json!([wide(a), a_hi.map(wide)]);
                value["r_extrema"] = json!([wide(r), r_hi.map(wide)]);
                value["d_extrema"] = json!([d.map(|v| v.to_string()), d_hi.map(|v| v.to_string())]);
            }
            None => value["empty"] = json!(true),
        },
        Err(error) => value["summary_error"] = json!(error),
    }
    value
}

/// The initial domains of one input, bucketed by (phase, owner) as the queue's
/// candidate index buckets them.
struct Boxes<const N: usize> {
    by_key: HashMap<(Phase, [bool; N]), Vec<(usize, Domain<N>, DomainPowerSummary<N>)>>,
}

impl<const N: usize> Boxes<N> {
    fn new(domains: &[Domain<N>]) -> Result<Self, String> {
        let mut by_key: HashMap<_, Vec<_>> = HashMap::new();
        for (id, domain) in domains.iter().enumerate() {
            let s = summary(domain).map_err(|e| format!("initial domain {id}: {e}"))?;
            by_key
                .entry((domain.phase, domain.owner))
                .or_default()
                .push((id, domain.clone(), s));
        }
        Ok(Self { by_key })
    }

    /// The first initial domain of the same phase and owner that contains
    /// `domain` by summary inclusion, and whether one contains it
    /// syntactically (`Domain::contains`).
    fn container(&self, domain: &Domain<N>, s: &DomainPowerSummary<N>) -> (Option<usize>, bool) {
        let Some(bucket) = self.by_key.get(&(domain.phase, domain.owner)) else {
            return (None, false);
        };
        let semantic = bucket
            .iter()
            .find(|(_, _, container)| container.contains(s))
            .map(|(id, _, _)| *id);
        let syntactic = bucket
            .iter()
            .any(|(_, container, _)| container.contains(domain));
        (semantic, syntactic)
    }
}

/// The walk's fresh-start initial admission (`walking::run`) over `text`:
/// its initial domains and input map.
fn simulate_initial<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &input::Selection,
    text: &str,
    max_bytes: usize,
) -> Result<(Vec<Domain<N>>, Vec<Value>), String> {
    let queries = matching::input::parse(text, N, request.matching.max_queries, max_bytes)
        .map_err(|e| e.to_string())?;
    let mut owners = Vec::with_capacity(selection.owners.len());
    for owner in &selection.owners {
        input::mask(&owner.mask, N).map_err(|e| e.to_string())?;
        let bytes = owner.mask.as_bytes();
        owners.push(std::array::from_fn::<bool, N, _>(|axis| {
            bytes[axis] == b'1'
        }));
    }
    let mut queue = Queue::<N>::with_policy(
        request.max_domains,
        request.max_containment_checks,
        request.scheduling_policy,
    )?;
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready {
        let OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead } =
            request.scheduling_policy
        else {
            return Err("Ready without transfer scheduling".into());
        };
        queue.delegation =
            Some(Ledger::new_ready(lookahead, request.max_domains).map_err(|e| e.to_string())?);
    }
    if request.reuse_initial_d_bands {
        queue
            .delegation
            .as_mut()
            .ok_or("initial D-band reuse without a ledger")?
            .begin_initial_admission()
            .map_err(|e| e.to_string())?;
    }
    let mut inputs = Vec::with_capacity(queries.len());
    for query in &queries {
        let owner: [bool; N] = query
            .owner
            .as_slice()
            .try_into()
            .map_err(|_| "query arity")?;
        if request.route_domain_overcover && !owners.contains(&owner) {
            return Err(format!(
                "query {} targets an owner without a program (initial routing is not simulated)",
                query.id
            ));
        }
        let domain = Domain {
            phase: Phase::Apply,
            owner,
            lower: query.lower.clone(),
            upper: query.upper.clone(),
            rank: query.rank,
            powers: query.powers,
        };
        let (id, _) = queue.admit(domain).map_err(str::to_owned)?;
        inputs.push(json!({"id":query.id,"domain":id}));
    }
    if request.reuse_initial_d_bands {
        queue
            .delegation
            .as_mut()
            .ok_or("initial D-band reuse without a ledger")?
            .finish_initial_admission()
            .map_err(|e| e.to_string())?;
    }
    Ok((
        queue.domains.iter().map(CompactDomain::expand).collect(),
        inputs,
    ))
}

/// Per-node containment bits and containing initial ids of both inputs.
struct Classes {
    bits: Vec<u8>,
    checkpoint_container: Vec<u8>,
    comparison_container: Vec<u8>,
}

impl Classes {
    fn build<const N: usize>(
        domains: &[CompactDomain<N>],
        checkpoint: &Boxes<N>,
        comparison: &Boxes<N>,
        threads: usize,
    ) -> Self {
        let n = domains.len();
        let mut bits = vec![0u8; n];
        let mut checkpoint_container = vec![NO_CONTAINER; n];
        let mut comparison_container = vec![NO_CONTAINER; n];
        let chunk = n.div_ceil(threads.max(1)).max(1);
        std::thread::scope(|scope| {
            for (((bits, own), other), start) in bits
                .chunks_mut(chunk)
                .zip(checkpoint_container.chunks_mut(chunk))
                .zip(comparison_container.chunks_mut(chunk))
                .zip((0..n).step_by(chunk))
            {
                scope.spawn(move || {
                    for offset in 0..bits.len() {
                        let domain = domains[start + offset].expand();
                        let mut class = if domain.phase == Phase::Route {
                            ROUTE
                        } else {
                            0
                        };
                        match summary(&domain) {
                            Ok(s) => {
                                if s.extrema().is_none_or(|e| e.positive_power().1.is_some()) {
                                    class |= A_FINITE;
                                }
                                let (id, syntactic) = checkpoint.container(&domain, &s);
                                if let Some(id) = id {
                                    class |= IN_CHECKPOINT;
                                    own[offset] = id as u8;
                                }
                                class |= u8::from(syntactic) * IN_CHECKPOINT_SYNTACTIC;
                                let (id, syntactic) = comparison.container(&domain, &s);
                                if let Some(id) = id {
                                    class |= IN_COMPARISON;
                                    other[offset] = id as u8;
                                }
                                class |= u8::from(syntactic) * IN_COMPARISON_SYNTACTIC;
                            }
                            Err(_) => class |= SUMMARY_ERROR,
                        }
                        bits[offset] = class;
                    }
                });
            }
        });
        Self {
            bits,
            checkpoint_container,
            comparison_container,
        }
    }

    fn code(&self, id: usize, initial: usize) -> usize {
        let class = self.bits[id];
        if id < initial {
            0
        } else if class & SUMMARY_ERROR != 0 {
            6
        } else if class & ROUTE != 0 {
            5
        } else {
            match (class & IN_CHECKPOINT != 0, class & IN_COMPARISON != 0) {
                (true, true) => 1,
                (false, true) => 2,
                (true, false) => 3,
                (false, false) => 4,
            }
        }
    }
}

/// Outgoing CSR of the tracker's (source, target) edges.
struct Outgoing {
    offsets: Vec<u64>,
    targets: Vec<u32>,
}

impl Outgoing {
    fn build(closure: &Tracker, nodes: usize) -> Self {
        let mut offsets = vec![0u64; nodes + 1];
        let _ = closure.try_for_each_edge(|source, _| {
            offsets[source + 1] += 1;
            ControlFlow::<()>::Continue(())
        });
        for id in 1..=nodes {
            offsets[id] += offsets[id - 1];
        }
        let mut cursor = offsets[..nodes].to_vec();
        let mut targets = vec![0u32; offsets[nodes] as usize];
        let _ = closure.try_for_each_edge(|source, target| {
            let at = &mut cursor[source];
            targets[*at as usize] = target as u32;
            *at += 1;
            ControlFlow::<()>::Continue(())
        });
        Self { offsets, targets }
    }

    fn of(&self, id: usize) -> &[u32] {
        &self.targets[self.offsets[id] as usize..self.offsets[id + 1] as usize]
    }
}

/// Everything a walk reads; shared read-only by the walker threads.
struct Graph<'a, const N: usize> {
    out: &'a Outgoing,
    flags: &'a [u8],
    classes: &'a Classes,
    status: &'a [u8],
    /// `unreserved_before[id]`: unreserved local obligations with a smaller
    /// id, i.e. the Ready reservation scan's FIFO distance to `id`.
    unreserved_before: &'a [u32],
    domains: &'a [CompactDomain<N>],
    initial: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    Full,
    Private,
    Projected,
}

impl Walk {
    fn name(self) -> &'static str {
        match self {
            Walk::Full => "full",
            Walk::Private => "private",
            Walk::Projected => "projected",
        }
    }
}

#[derive(Default)]
struct Blockers {
    count: u64,
    inspected: u64,
    with_outgoing: u64,
    status: [u64; STATUS_NAMES.len()],
    classes: [u64; CLASS_NAMES.len()],
    keys: HashMap<(Phase, u32, bool, Option<u32>), u64>,
    owner_class: HashMap<(u32, usize), u64>,
    depth: BTreeMap<u32, u64>,
    ids: Vec<u32>,
}

fn top<K, V: Ord + Copy + Into<u64>>(
    map: impl IntoIterator<Item = (K, V)>,
    entry: impl Fn(&K, V) -> Value,
) -> Value {
    let mut rows: Vec<(K, V)> = map.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let other: u64 = rows.iter().skip(TOP).map(|(_, v)| (*v).into()).sum();
    let kept: Vec<Value> = rows.iter().take(TOP).map(|(k, v)| entry(k, *v)).collect();
    json!({"distinct":rows.len(),"top":kept,"other_count":other})
}

impl Blockers {
    fn add<const N: usize>(&mut self, g: &Graph<N>, id: usize, depth: u32) {
        let flag = g.flags[id];
        let domain = &g.domains[id];
        let owner = owner_bits(&domain.owner());
        let code = g.classes.code(id, g.initial);
        self.count += 1;
        self.inspected += u64::from(flag & INSPECTED != 0);
        self.with_outgoing += u64::from(!g.out.of(id).is_empty());
        self.status[usize::from(g.status[id])] += 1;
        self.classes[code] += 1;
        *self
            .keys
            .entry((
                domain.phase(),
                owner,
                g.classes.bits[id] & A_FINITE != 0,
                domain.rank(),
            ))
            .or_default() += 1;
        *self.owner_class.entry((owner, code)).or_default() += 1;
        *self.depth.entry(depth).or_default() += 1;
        self.ids.push(id as u32);
    }

    fn json<const N: usize>(mut self, g: &Graph<N>) -> Value {
        self.ids.sort_unstable();
        let at = |q: f64| {
            (!self.ids.is_empty()).then(|| {
                let id = self.ids[((self.ids.len() - 1) as f64 * q).round() as usize] as usize;
                json!({"id":id,"unreserved_obligations_before":g.unreserved_before[id]})
            })
        };
        let quantiles =
            json!({"min":at(0.0),"p10":at(0.1),"p50":at(0.5),"p90":at(0.9),"max":at(1.0)});
        let mut owners: HashMap<u32, u64> = HashMap::new();
        for (&(_, owner, _, _), &count) in &self.keys {
            *owners.entry(owner).or_default() += count;
        }
        json!({"count":self.count,"inspected_unsealed":self.inspected,
            "with_outgoing_edges":self.with_outgoing,
            "status":STATUS_NAMES.iter().zip(self.status).map(|(k, v)| (k.to_string(), json!(v)))
                .collect::<serde_json::Map<_, _>>(),
            "classes":CLASS_NAMES.iter().zip(self.classes).map(|(k, v)| (k.to_string(), json!(v)))
                .collect::<serde_json::Map<_, _>>(),
            "id_quantiles":quantiles,
            "depth":self.depth.iter().map(|(d, c)| (d.to_string(), json!(c)))
                .collect::<serde_json::Map<_, _>>(),
            "by_phase_owner_a_rank":top(self.keys, |&(phase, owner, finite, rank), count| json!({
                "phase":format!("{phase:?}"),"owner":owner_mask::<N>(owner),
                "a":if finite {"finite"} else {"unbounded"},"rank":rank,"count":count})),
            "by_owner":top(owners, |&owner, count| json!({"owner":owner_mask::<N>(owner),"count":count})),
            "by_owner_class":top(self.owner_class, |&(owner, code), count| json!({
                "owner":owner_mask::<N>(owner),"class":CLASS_NAMES[code],"count":count}))})
    }
}

/// Per BFS level of a full walk: node count, unsealed count and id
/// quantiles. Ready reserves in id order, so ids growing with depth show one
/// level admitted per pass of the reservation scan.
fn levels<const N: usize>(g: &Graph<N>, order: &[u32], starts: &[usize]) -> Vec<Value> {
    starts
        .iter()
        .enumerate()
        .map(|(depth, &start)| {
            let end = starts.get(depth + 1).copied().unwrap_or(order.len());
            let mut ids = order[start..end].to_vec();
            let unsealed = ids
                .iter()
                .filter(|&&id| g.flags[id as usize] & SEALED == 0)
                .count();
            let middle = ids.len() / 2;
            let p50 = *ids.select_nth_unstable(middle).1;
            json!({"depth":depth,"nodes":ids.len(),"unsealed":unsealed,
                "id_min":ids.iter().min(),"id_p50":p50,"id_max":ids.iter().max()})
        })
        .collect()
}

/// One forward BFS from `root` (see module docs for the three modes). A
/// projected walk also returns the bitset of the nodes it expanded.
fn walk<const N: usize>(g: &Graph<N>, root: usize, mode: Walk) -> (Value, Option<Vec<u64>>) {
    let started = Instant::now();
    let n = g.flags.len();
    let mut seen = vec![0u64; n.div_ceil(64)];
    let mut expanded = (mode == Walk::Projected).then(|| vec![0u64; n.div_ceil(64)]);
    let mut order: Vec<u32> = vec![root as u32];
    let mut starts = vec![0usize];
    seen[root / 64] |= 1 << (root % 64);
    let (mut head, mut depth, mut level_end) = (0usize, 0u32, 1usize);
    let mut blockers = Blockers::default();
    let mut visited_classes = [0u64; CLASS_NAMES.len()];
    let mut roots: BTreeMap<usize, bool> = BTreeMap::new();
    let mut absorbed: BTreeMap<usize, [u64; 2]> = BTreeMap::new();
    let (mut stopped, mut stopped_unsealed, mut edges) = (0u64, 0u64, 0u64);
    while head < order.len() {
        if head == level_end {
            depth += 1;
            level_end = order.len();
            starts.push(head);
        }
        let id = order[head] as usize;
        head += 1;
        let unsealed = g.flags[id] & SEALED == 0;
        visited_classes[g.classes.code(id, g.initial)] += 1;
        let stop = if id == root {
            false
        } else if id < g.initial {
            roots.insert(id, g.flags[id] & CLOSED != 0);
            mode != Walk::Full
        } else if mode == Walk::Projected && g.classes.bits[id] & IN_COMPARISON != 0 {
            let entry = absorbed
                .entry(usize::from(g.classes.comparison_container[id]))
                .or_default();
            entry[0] += 1;
            entry[1] += u64::from(unsealed);
            true
        } else {
            false
        };
        if stop {
            stopped += 1;
            stopped_unsealed += u64::from(unsealed);
            continue;
        }
        if unsealed {
            blockers.add(g, id, depth);
        }
        if let Some(expanded) = expanded.as_mut() {
            expanded[id / 64] |= 1 << (id % 64);
        }
        let targets = g.out.of(id);
        edges += targets.len() as u64;
        for &target in targets {
            let target = target as usize;
            let (word, bit) = (target / 64, 1u64 << (target % 64));
            if seen[word] & bit == 0 {
                seen[word] |= bit;
                order.push(target as u32);
            }
        }
    }
    let levels = (mode == Walk::Full).then(|| levels(g, &order, &starts));
    let value = json!({"mode":mode.name(),"visited":order.len(),"expanded_edges":edges,"max_depth":depth,
        "visited_classes":CLASS_NAMES.iter().zip(visited_classes).map(|(k, v)| (k.to_string(), json!(v)))
            .collect::<serde_json::Map<_, _>>(),
        "stopped":stopped,"stopped_unsealed":stopped_unsealed,
        "roots_reached":roots.iter().map(|(id, closed)| json!({"id":id,"closed":closed})).collect::<Vec<_>>(),
        "absorbed_by_comparison_root":(mode == Walk::Projected).then(|| absorbed.iter()
            .map(|(id, [nodes, unsealed])| json!({"comparison_root":id,"nodes":nodes,"unsealed":unsealed}))
            .collect::<Vec<_>>()),
        "levels":levels,"blockers":blockers.json(g),"seconds":seconds(started)});
    (value, expanded)
}

/// Roots whose every transitively required root has no blocker of its own.
fn predicted_closed(own_zero: &[bool], deps: &[BTreeSet<usize>]) -> Vec<bool> {
    (0..own_zero.len())
        .map(|root| {
            let mut seen = BTreeSet::from([root]);
            let mut stack = vec![root];
            while let Some(r) = stack.pop() {
                if !own_zero[r] {
                    return false;
                }
                for &d in &deps[r] {
                    if seen.insert(d) {
                        stack.push(d);
                    }
                }
            }
            true
        })
        .collect()
}

fn ids(list: &str) -> Vec<usize> {
    list.split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse().expect("root id list"))
        .collect()
}

struct RootBlockers {
    comparison_queries: PathBuf,
    detail: Vec<usize>,
    comparison_closed: Option<Vec<usize>>,
    threads: usize,
}

impl RestoredAnalysis for RootBlockers {
    fn analyze<const N: usize>(
        &mut self,
        request: &OwnerDomainWalkRequest,
        selection: &input::Selection,
        restored: &Restored<N>,
        receipt: &mut Value,
    ) -> Result<(), String> {
        let started = Instant::now();
        let mut timings = serde_json::Map::new();
        let state = &restored.state;
        let initial = state.initial_domain_count;
        let domains: &[CompactDomain<N>] = &state.queue.domains;
        let n = domains.len();
        if initial >= usize::from(NO_CONTAINER) || n >= u32::MAX as usize {
            return Err("graph too large for the diagnostic's compact ids".into());
        }

        // Initial admission of both inputs; the checkpoint's must replay exactly.
        let phase = Instant::now();
        let (own_initial, own_inputs) = simulate_initial::<N>(
            request,
            selection,
            &request.matching.queries_json,
            request.matching.max_query_bytes,
        )?;
        let reproduces_domains = own_initial.len() == initial
            && own_initial
                .iter()
                .zip(domains)
                .all(|(simulated, restored)| *simulated == restored.expand());
        let reproduces_inputs = own_inputs == restored.inputs;
        if !reproduces_domains || !reproduces_inputs {
            return Err(format!(
                "simulated initial admission does not reproduce the checkpoint (domains {reproduces_domains}, inputs {reproduces_inputs})"
            ));
        }
        let text = std::fs::read_to_string(&self.comparison_queries)
            .map_err(|e| format!("{}: {e}", self.comparison_queries.display()))?;
        let (other_initial, other_inputs) = simulate_initial::<N>(
            request,
            selection,
            &text,
            text.len().max(request.matching.max_query_bytes),
        )?;
        let same_owner_sequence = other_initial.len() == own_initial.len()
            && other_initial
                .iter()
                .zip(&own_initial)
                .all(|(a, b)| a.phase == b.phase && a.owner == b.owner);
        let own_boxes = Boxes::new(&own_initial)?;
        let other_boxes = Boxes::new(&other_initial)?;
        timings.insert("simulate_initial_seconds".into(), json!(seconds(phase)));

        // Flags, outgoing CSR, classes, ledger status.
        let phase = Instant::now();
        let closure = state.closure.borrow();
        let flags: Vec<u8> = closure.node_flags().collect();
        if flags.len() != n {
            return Err("dependency monitor does not cover the domain inventory".into());
        }
        let out = Outgoing::build(&closure, n);
        let edge_count = closure.edge_count();
        drop(closure);
        if out.targets.len() != edge_count {
            return Err("outgoing CSR lost edges".into());
        }
        timings.insert("outgoing_csr_seconds".into(), json!(seconds(phase)));
        receipt["memory"]["after_outgoing_csr"] = memory();

        let phase = Instant::now();
        let classes = Classes::build(domains, &own_boxes, &other_boxes, self.threads);
        timings.insert("classify_seconds".into(), json!(seconds(phase)));

        let phase = Instant::now();
        let ledger = state.queue.delegation.as_ref();
        let mut status = vec![0u8; n];
        let mut unreserved_before = vec![0u32; n + 1];
        for id in 0..n {
            status[id] = match ledger {
                None => 4,
                Some(l) if l.is_published(id) => 3,
                Some(l) if l.delegated_to(id).is_some() => 2,
                Some(l) if l.can_dispatch(id) => 1,
                Some(_) => 0,
            };
            unreserved_before[id + 1] = unreserved_before[id] + u32::from(status[id] == 0);
        }
        timings.insert("ledger_status_seconds".into(), json!(seconds(phase)));

        // Global inventory.
        let mut all_classes = [0u64; CLASS_NAMES.len()];
        let mut unsealed_classes = [0u64; CLASS_NAMES.len()];
        let mut unsealed_status = [0u64; STATUS_NAMES.len()];
        let (mut unsealed, mut inspected_unsealed, mut unsealed_outgoing, mut syntactic_gaps) =
            (0u64, 0u64, 0u64, [0u64; 2]);
        for id in 0..n {
            let code = classes.code(id, initial);
            let bits = classes.bits[id];
            all_classes[code] += 1;
            syntactic_gaps[0] +=
                u64::from(bits & IN_CHECKPOINT_SYNTACTIC != 0 && bits & IN_CHECKPOINT == 0);
            syntactic_gaps[1] +=
                u64::from(bits & IN_COMPARISON_SYNTACTIC != 0 && bits & IN_COMPARISON == 0);
            if flags[id] & SEALED == 0 {
                unsealed += 1;
                inspected_unsealed += u64::from(flags[id] & INSPECTED != 0);
                unsealed_outgoing += u64::from(!out.of(id).is_empty());
                unsealed_classes[code] += 1;
                unsealed_status[usize::from(status[id])] += 1;
            }
        }
        let named = |names: &[&str], values: &[u64]| {
            names
                .iter()
                .zip(values)
                .map(|(k, v)| (k.to_string(), json!(v)))
                .collect::<serde_json::Map<_, _>>()
        };
        let initial_table: Vec<Value> = (0..initial)
            .map(|id| {
                let (own, other) = (&own_initial[id], other_initial.get(id));
                let other_contains_own = other.is_some_and(|o| {
                    o.phase == own.phase
                        && o.owner == own.owner
                        && summary(o)
                            .and_then(|so| summary(own).map(|ss| so.contains(&ss)))
                            .unwrap_or(false)
                });
                json!({"id":id,"owner":mask(&own.owner),"sealed":flags[id] & SEALED != 0,
                    "inspected":flags[id] & INSPECTED != 0,"closed":flags[id] & CLOSED != 0,
                    "out_degree":out.of(id).len(),
                    "checkpoint_initial":domain_json(own),"comparison_initial":other.map(domain_json),
                    "comparison_initial_contains_checkpoint_initial":other_contains_own,
                    "identical_initial_domains":other == Some(own)})
            })
            .collect();

        // Three walks per root, in parallel.
        let graph = Graph {
            out: &out,
            flags: &flags,
            classes: &classes,
            status: &status,
            unreserved_before: &unreserved_before,
            domains,
            initial,
        };
        let phase = Instant::now();
        let tasks: Vec<(usize, Walk)> = (0..initial)
            .flat_map(|root| [Walk::Full, Walk::Private, Walk::Projected].map(|m| (root, m)))
            .collect();
        let next = AtomicUsize::new(0);
        let results = Mutex::new(vec![Value::Null; tasks.len()]);
        let union = Mutex::new(vec![0u64; n.div_ceil(64)]);
        std::thread::scope(|scope| {
            for _ in 0..self.threads.max(1) {
                scope.spawn(|| {
                    loop {
                        let task = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&(root, mode)) = tasks.get(task) else {
                            break;
                        };
                        let (value, expanded) = walk(&graph, root, mode);
                        if let Some(expanded) = expanded {
                            let mut union = union.lock().expect("projection union");
                            for (word, bits) in union.iter_mut().zip(expanded) {
                                *word |= bits;
                            }
                        }
                        results.lock().expect("walk results")[task] = value;
                    }
                });
            }
        });
        let mut results = results.into_inner().expect("walk results").into_iter();
        let union = union.into_inner().expect("projection union");
        let (mut kept, mut kept_unsealed, mut kept_below_scan) = (0u64, 0u64, 0u64);
        let mut kept_unsealed_classes = [0u64; CLASS_NAMES.len()];
        let scan = ledger.map_or(n, |l| l.reservation_scan().min(n));
        for id in 0..n {
            if union[id / 64] >> (id % 64) & 1 == 1 {
                kept += 1;
                kept_below_scan += u64::from(id < scan);
                if flags[id] & SEALED == 0 {
                    kept_unsealed += 1;
                    kept_unsealed_classes[classes.code(id, initial)] += 1;
                }
            }
        }
        // Composition of the Ready FIFO ahead of an id: unreserved local
        // obligations outside the projection union, and those a comparison
        // initial domain contains.
        let mut outside_before = vec![0u32; n + 1];
        let mut comparison_before = vec![0u32; n + 1];
        for id in 0..n {
            let unreserved = status[id] == 0;
            let outside = union[id / 64] >> (id % 64) & 1 == 0;
            outside_before[id + 1] = outside_before[id] + u32::from(unreserved && outside);
            comparison_before[id + 1] =
                comparison_before[id] + u32::from(unreserved && classes.code(id, initial) == 2);
        }
        let unreserved_total = unreserved_before[n];
        let annotate = |walk: &mut Value| {
            if let Some(quantiles) = walk["blockers"]["id_quantiles"].as_object_mut() {
                for entry in quantiles.values_mut() {
                    if let Some(id) = entry["id"].as_u64().map(|id| id as usize) {
                        entry["unreserved_before_outside_projection"] = json!(outside_before[id]);
                        entry["unreserved_before_in_comparison_initial"] =
                            json!(comparison_before[id]);
                    }
                }
            }
        };
        timings.insert("walks_seconds".into(), json!(seconds(phase)));
        receipt["memory"]["after_walks"] = memory();

        // Per root, then the decompositions and their predictions.
        let mut roots = Vec::with_capacity(initial);
        let (mut private_zero, mut projected_zero) = (vec![false; initial], vec![false; initial]);
        let (mut private_deps, mut projected_deps) = (
            vec![BTreeSet::new(); initial],
            vec![BTreeSet::new(); initial],
        );
        let mut flag_disagreements = Vec::new();
        for root in 0..initial {
            let (mut full, mut private, mut projected) = (
                results.next().expect("full"),
                results.next().expect("private"),
                results.next().expect("projected"),
            );
            for walk in [&mut full, &mut private, &mut projected] {
                annotate(walk);
            }
            let closed = flags[root] & CLOSED != 0;
            if (full["blockers"]["count"] == 0) != closed {
                flag_disagreements.push(root);
            }
            private_zero[root] = private["blockers"]["count"] == 0;
            projected_zero[root] = projected["blockers"]["count"] == 0;
            for reached in private["roots_reached"].as_array().expect("roots") {
                private_deps[root].insert(reached["id"].as_u64().expect("id") as usize);
            }
            for reached in projected["roots_reached"].as_array().expect("roots") {
                projected_deps[root].insert(reached["id"].as_u64().expect("id") as usize);
            }
            for absorbed in projected["absorbed_by_comparison_root"]
                .as_array()
                .expect("absorbed")
            {
                let id = absorbed["comparison_root"].as_u64().expect("id") as usize;
                if id != root {
                    projected_deps[root].insert(id);
                }
            }
            roots.push(
                json!({"id":root,"owner":mask(&own_initial[root].owner),"closed_flag":closed,
                "full":full,"private":private,"projected":projected}),
            );
        }
        if !flag_disagreements.is_empty() {
            return Err(format!(
                "forward reachability disagrees with the persisted CLOSED flag of roots {flag_disagreements:?}"
            ));
        }
        let closed: Vec<bool> = (0..initial).map(|r| flags[r] & CLOSED != 0).collect();
        let private_prediction = predicted_closed(&private_zero, &private_deps);
        let projected_prediction = predicted_closed(&projected_zero, &projected_deps);
        let listed = |values: &[bool]| {
            values
                .iter()
                .enumerate()
                .filter_map(|(id, &v)| v.then_some(id))
                .collect::<Vec<_>>()
        };
        let closed_ids = listed(&closed);
        let comparison_closed = self.comparison_closed.clone().unwrap_or_else(|| {
            let mut ids: BTreeSet<usize> = closed_ids.iter().copied().collect();
            ids.extend(&self.detail);
            ids.into_iter().collect()
        });
        for (root, value) in roots.iter_mut().enumerate() {
            let deps = |set: &BTreeSet<usize>| {
                set.iter()
                    .map(|&d| {
                        json!({"id":d,"closed":closed[d],
                        "closed_in_comparison_run":comparison_closed.contains(&d)})
                    })
                    .collect::<Vec<_>>()
            };
            value["private_root_dependencies"] = json!(deps(&private_deps[root]));
            value["projected_root_dependencies"] = json!(deps(&projected_deps[root]));
            value["private_decomposition_predicts_closed"] = json!(private_prediction[root]);
            value["projection_predicts_closed"] = json!(projected_prediction[root]);
        }

        // First hop of the detail roots.
        let mut detail = serde_json::Map::new();
        for &root in &self.detail {
            if root >= initial {
                return Err(format!("detail root {root} is not an initial domain"));
            }
            let targets = out.of(root);
            let mut hop_classes = [0u64; CLASS_NAMES.len()];
            let (mut sealed, mut closed_targets) = (0u64, 0u64);
            let mut listed_targets = Vec::new();
            let mut initial_targets = BTreeSet::new();
            for &target in targets {
                let target = target as usize;
                let code = classes.code(target, initial);
                hop_classes[code] += 1;
                sealed += u64::from(flags[target] & SEALED != 0);
                closed_targets += u64::from(flags[target] & CLOSED != 0);
                if target < initial {
                    initial_targets.insert(target);
                }
                if listed_targets.len() < FIRST_HOP_LISTED {
                    let bits = classes.bits[target];
                    let container = |c: u8| (c != NO_CONTAINER).then_some(c);
                    listed_targets.push(json!({"id":target,"class":CLASS_NAMES[code],
                        "sealed":flags[target] & SEALED != 0,"inspected":flags[target] & INSPECTED != 0,
                        "closed":flags[target] & CLOSED != 0,"status":STATUS_NAMES[usize::from(status[target])],
                        "checkpoint_container":container(classes.checkpoint_container[target]),
                        "comparison_container":container(classes.comparison_container[target]),
                        "in_checkpoint_initial_syntactic":bits & IN_CHECKPOINT_SYNTACTIC != 0,
                        "in_comparison_initial_syntactic":bits & IN_COMPARISON_SYNTACTIC != 0,
                        "domain":domain_json(&domains[target].expand())}));
                }
            }
            detail.insert(
                root.to_string(),
                json!({"first_hop":{"successors":targets.len(),
                "classes":named(&CLASS_NAMES, &hop_classes),"sealed":sealed,"closed":closed_targets,
                "initial_targets":initial_targets,"listed":listed_targets}}),
            );
        }

        // Class counts over the unclosed roots other than the detail roots.
        let mut context = serde_json::Map::new();
        for mode in ["full", "private", "projected"] {
            let mut roots_with = [0u64; CLASS_NAMES.len()];
            let mut blockers = [0u64; CLASS_NAMES.len()];
            let (mut considered, mut zero) = (0u64, 0u64);
            for (root, value) in roots.iter().enumerate() {
                if closed[root] || self.detail.contains(&root) {
                    continue;
                }
                considered += 1;
                zero += u64::from(value[mode]["blockers"]["count"] == 0);
                for (code, name) in CLASS_NAMES.iter().enumerate() {
                    let count = value[mode]["blockers"]["classes"][name]
                        .as_u64()
                        .unwrap_or(0);
                    roots_with[code] += u64::from(count > 0);
                    blockers[code] += count;
                }
            }
            context.insert(
                mode.into(),
                json!({"roots":considered,"roots_without_blockers":zero,
                "roots_with_a_blocker_of_class":named(&CLASS_NAMES, &roots_with),
                "blockers_by_class":named(&CLASS_NAMES, &blockers)}),
            );
        }

        let summary_rows: Vec<Value> = roots
            .iter()
            .map(|r| {
                json!([
                    r["id"],
                    r["closed_flag"],
                    r["full"]["visited"],
                    r["full"]["blockers"]["count"],
                    r["private"]["visited"],
                    r["private"]["blockers"]["count"],
                    r["projected"]["visited"],
                    r["projected"]["blockers"]["count"],
                    r["private_root_dependencies"].as_array().map(Vec::len),
                    r["projected_root_dependencies"].as_array().map(Vec::len),
                    r["projection_predicts_closed"]
                ])
            })
            .collect();
        timings.insert("analysis_seconds".into(), json!(seconds(started)));
        let hash = |bytes: &[u8]| blake3::hash(bytes).to_hex().to_string();
        receipt["analysis_summary"] = json!({
            "columns":["root","closed","full_visited","full_blockers","private_visited",
                "private_blockers","projected_visited","projected_blockers","private_root_deps",
                "projected_root_deps","projection_predicts_closed"],
            "rows":summary_rows,"closed_roots":closed_ids,
            "private_decomposition_predicts_closed":listed(&private_prediction),
            "projection_predicts_closed":listed(&projected_prediction),
            "comparison_closed":comparison_closed,"context_other_unclosed_roots":context,
            "projection_union":{"expanded_nodes":kept,"unsealed":kept_unsealed,
                "nodes":n,"unsealed_total":unsealed,"unreserved_obligations":unreserved_total,
                "unreserved_outside":outside_before[n]},
            "timings":timings.clone()});
        receipt["root_blockers"] = json!({
            "schema":"rustred.root-closure-blockers.v1",
            "semantics":{"edge":"(source, target): source waits on target (successor admission or reuse, alias to its containing representative, partial-inspection initial anchor)",
                "sealed":"node flag bit0: whole callback stream accepted with no frontier",
                "closed":"no unsealed node forward-reachable (descendant_closure.rs: reverse unsealed reachability); checked against the persisted CLOSED flag of every root",
                "blocker":"an unsealed node reachable from the root in the given walk",
                "walks":{"full":"all reachable nodes","private":"stops at other initial domains (root dependencies)",
                    "projected":"also stops at non-initial nodes contained (summary inclusion, same phase and owner) in the comparison input's initial domain: the alias a run with those inputs would have made"},
                "containment":"DomainPowerSummary inclusion within the initial domain of the same phase and owner, as the unlimited admission lane decides; the syntactic Domain::contains is reported alongside",
                "a":"finite iff the tight A upper extremum of the node's domain is finite",
                "status":"ledger responsibility; unreserved_before counts unreserved local obligations with a smaller id (Ready reserves in id order)",
                "scope":"one checkpoint snapshot of discovered-dependency coverage; the projection is a counterfactual on this graph, not a replay of the comparison run; no closure, descent or ETA claim"},
            "inputs":{"checkpoint_queries_blake3":hash(request.matching.queries_json.as_bytes()),
                "comparison_queries":self.comparison_queries,"comparison_queries_blake3":hash(text.as_bytes()),
                "detail_roots":self.detail,"comparison_closed":comparison_closed,
                "comparison_closed_source":if self.comparison_closed.is_some() {"RUSTRED_ROOT_BLOCKERS_COMPARISON_CLOSED"}
                    else {"checkpoint closed roots plus the detail roots (given, not measured here)"},
                "threads":self.threads},
            "initial_admission":{"initial_domains":initial,"checkpoint_simulation_reproduces_domains":reproduces_domains,
                "checkpoint_simulation_reproduces_inputs":reproduces_inputs,
                "comparison_initial_domains":other_initial.len(),"comparison_inputs":other_inputs,
                "same_owner_sequence":same_owner_sequence,"initial":initial_table},
            "graph":{"nodes":n,"edges":edge_count,"unsealed":unsealed,"inspected_unsealed":inspected_unsealed,
                "unsealed_with_outgoing_edges":unsealed_outgoing,
                "classes_all_nodes":named(&CLASS_NAMES, &all_classes),
                "classes_unsealed":named(&CLASS_NAMES, &unsealed_classes),
                "status_unsealed":named(&STATUS_NAMES, &unsealed_status),
                "syntactic_only_containment":{"checkpoint":syntactic_gaps[0],"comparison":syntactic_gaps[1]},
                "ledger":ledger.map(|l| json!({"cursor":l.cursor(),"reservation_scan":l.reservation_scan(),
                    "outstanding_native":l.outstanding_native(),"published":l.published_count()}))},
            "projection_union":{"expanded_nodes":kept,"unsealed":kept_unsealed,
                "unsealed_by_class":named(&CLASS_NAMES, &kept_unsealed_classes),
                "nodes_outside":n as u64 - kept,"unsealed_outside":unsealed - kept_unsealed,
                "unreserved_obligations":unreserved_total,"unreserved_outside":outside_before[n],
                "unreserved_in_comparison_initial":comparison_before[n],
                "reservation_scan":scan,"expanded_below_reservation_scan":kept_below_scan,
                "admitted_per_scanned":{"graph":n as f64 / scan as f64,
                    "projection":kept as f64 / kept_below_scan as f64},
                "scope":"union over roots of the nodes a projected walk expanded: what remains of this graph when every node contained in a comparison initial domain is replaced by that domain; the comparison run's own inspection of its larger initial domains is not modelled"},
            "closed_roots":closed_ids,
            "private_decomposition_predicts_closed":listed(&private_prediction),
            "private_decomposition_reproduces_closed_flags":private_prediction == closed,
            "projection_predicts_closed":listed(&projected_prediction),
            "context_other_unclosed_roots":context,
            "detail_roots":detail,
            "roots":roots,
            "timings":timings});
        Ok(())
    }
}

#[test]
#[ignore = "needs a copied production CP5 checkpoint, its campaign request and a comparison query document (see module docs)"]
fn root_closure_blockers() {
    let mut analysis = RootBlockers {
        comparison_queries: PathBuf::from(
            std::env::var_os(COMPARISON_QUERIES).expect(COMPARISON_QUERIES),
        ),
        detail: std::env::var(DETAIL_ROOTS).map_or_else(|_| Vec::new(), |list| ids(&list)),
        comparison_closed: std::env::var(COMPARISON_CLOSED).ok().map(|list| ids(&list)),
        threads: std::env::var(THREADS).map_or(16, |t| t.parse().expect(THREADS)),
    };
    restore_copied_checkpoint(
        "root_closure_blockers",
        "rustred.checkpoint-root-closure-blockers.v1",
        "root-closure-blockers",
        &mut analysis,
    );
}

#[test]
fn predicted_closure_needs_every_transitively_required_root() {
    let own_zero = [true, true, false, true];
    let deps = [
        BTreeSet::from([1]),
        BTreeSet::from([0]),
        BTreeSet::new(),
        BTreeSet::from([2]),
    ];
    assert_eq!(
        predicted_closed(&own_zero, &deps),
        [true, true, false, false]
    );
}
