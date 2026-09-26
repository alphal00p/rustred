//! Observational dependency coverage, never algebraic termination authority.
//! A node is sealed only after its entire valid callback stream was accepted.
//! Reverse reachability from unsealed nodes blocks all their ancestors. The
//! complement includes sealed cycles with no unresolved outgoing dependency;
//! this is scoped finite worklist coverage, NOT a descent/family certificate.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const NONE: usize = usize::MAX;

/// Periodic refresh spacing as a multiple of the last scan's wall time: the
/// coordinator spends at most ~1/multiplier of its wall in closure scans.
pub(super) const REFRESH_DUTY_MULTIPLIER: f64 = 100.0;
pub(super) const REFRESH_MIN_INTERVAL_SECONDS: f64 = 5.0;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    sealed: bool,
    inspected: bool,
    closed: bool,
    incoming: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge {
    source: usize,
    target: usize,
    next: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tracker {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    initial: usize,
    unavailable: Option<String>,
    revision: u64,
    snapshot_revision: u64,
    initial_closed: usize,
    total_closed: usize,
    inspected: usize,
    refresh_count: u64,
    refresh_seconds: f64,
    #[serde(skip)]
    open_targets: HashMap<usize, HashSet<usize>>,
    #[serde(skip)]
    last_refresh: Option<Instant>,
    #[serde(skip)]
    last_refresh_seconds: f64,
}

impl Tracker {
    pub fn new(initial: usize) -> Self {
        let mut value = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            initial,
            unavailable: None,
            revision: 0,
            snapshot_revision: 0,
            initial_closed: 0,
            total_closed: 0,
            inspected: 0,
            refresh_count: 0,
            refresh_seconds: 0.0,
            open_targets: HashMap::new(),
            last_refresh: None,
            last_refresh_seconds: 0.0,
        };
        value.discovered(initial);
        value
    }

    pub fn disable(&mut self, reason: &str) {
        if self.unavailable.is_none() {
            self.unavailable = Some(reason.to_owned());
        }
        // Release optional monitoring allocations; mathematical state survives.
        self.nodes = Vec::new();
        self.edges = Vec::new();
        self.open_targets = HashMap::new();
    }

    fn changed(&mut self) {
        if let Some(next) = self.revision.checked_add(1) {
            self.revision = next;
        } else {
            self.disable("dependency graph revision overflow");
        }
    }

    pub fn discovered(&mut self, total: usize) {
        if self.unavailable.is_some() {
            return;
        }
        let Some(extra) = total.checked_sub(self.nodes.len()) else {
            self.disable("dependency domain count regressed");
            return;
        };
        if extra == 0 {
            return;
        }
        if self.nodes.try_reserve(extra).is_err() {
            self.disable("dependency node allocation unavailable");
            return;
        }
        self.nodes.resize_with(total, || Node {
            sealed: false,
            inspected: false,
            closed: false,
            incoming: NONE,
        });
        self.changed();
    }

    pub fn edge(&mut self, source: usize, target: usize) {
        if self.unavailable.is_some() {
            return;
        }
        if source >= self.nodes.len() || target >= self.nodes.len() {
            self.disable("dependency endpoint outside discovered domain set");
            return;
        }
        if self.nodes[source].sealed {
            self.disable("dependency added after source sealed");
            return;
        }
        if !self.open_targets.contains_key(&source) && self.open_targets.try_reserve(1).is_err() {
            self.disable("dependency deduplication allocation unavailable");
            return;
        }
        let targets = self.open_targets.entry(source).or_default();
        if targets.contains(&target) {
            return;
        }
        if targets.try_reserve(1).is_err() || self.edges.try_reserve(1).is_err() {
            self.disable("dependency edge allocation unavailable");
            return;
        }
        targets.insert(target);
        let index = self.edges.len();
        self.edges.push(Edge {
            source,
            target,
            next: self.nodes[target].incoming,
        });
        self.nodes[target].incoming = index;
        self.changed();
    }

    pub fn finish(&mut self, id: usize, inspected: bool, success: bool) {
        if self.unavailable.is_some() {
            return;
        }
        let Some(node) = self.nodes.get_mut(id) else {
            self.disable("dependency completion outside discovered domain set");
            return;
        };
        if node.sealed {
            self.disable("dependency node sealed twice");
            return;
        }
        if inspected && !node.inspected {
            node.inspected = true;
            self.inspected += 1; // Bounded by allocated node count.
        }
        node.sealed = success;
        if success {
            self.open_targets.remove(&id);
        }
        self.changed();
    }

    /// Minimum spacing between two periodic scans: at least five seconds and
    /// at least `REFRESH_DUTY_MULTIPLIER` times the last scan's own wall time,
    /// so a costly scan bounds the periodic refresh duty to about 1%.
    pub(super) fn refresh_interval(&self) -> Duration {
        Duration::from_secs_f64(
            REFRESH_MIN_INTERVAL_SECONDS.max(self.last_refresh_seconds * REFRESH_DUTY_MULTIPLIER),
        )
    }

    /// Seconds until the periodic throttle admits another scan; None before
    /// the first scan. A forced refresh ignores it.
    fn next_refresh_seconds(&self) -> Option<f64> {
        self.last_refresh.map(|time| {
            self.refresh_interval()
                .saturating_sub(time.elapsed())
                .as_secs_f64()
        })
    }

    /// At most one periodic scan per `refresh_interval`: <= ~1% refresh duty
    /// after a costly scan. Dirty older counts remain a monotone conservative
    /// bound. `force` bypasses the throttle, never the cancellation checks.
    pub fn refresh(&mut self, cancellation: &AtomicBool, force: bool) {
        if self.unavailable.is_some() || self.revision == self.snapshot_revision {
            return;
        }
        let interval = self.refresh_interval();
        if !force
            && self
                .last_refresh
                .is_some_and(|time| time.elapsed() < interval)
        {
            return;
        }
        let started = Instant::now();
        let mut blocked = Vec::new();
        let mut stack = Vec::new();
        if blocked.try_reserve_exact(self.nodes.len()).is_err()
            || stack.try_reserve_exact(self.nodes.len()).is_err()
        {
            self.disable("dependency refresh allocation unavailable");
            return;
        }
        for (id, node) in self.nodes.iter().enumerate() {
            if id % 1024 == 0 && cancellation.load(Ordering::Relaxed) {
                return;
            }
            blocked.push(!node.sealed);
            if !node.sealed {
                stack.push(id);
            }
        }
        let mut work = 0usize;
        while let Some(id) = stack.pop() {
            let mut edge = self.nodes[id].incoming;
            while edge != NONE {
                if work % 1024 == 0 && cancellation.load(Ordering::Relaxed) {
                    return;
                }
                work = work.wrapping_add(1);
                let link = &self.edges[edge];
                if !blocked[link.source] {
                    blocked[link.source] = true;
                    stack.push(link.source);
                }
                edge = link.next;
            }
        }
        let mut total = 0;
        let mut initial = 0;
        // No mutation until the cancellable scan completed. Closed nodes can
        // never acquire later edges because sources must be unsealed at edge().
        for (id, node) in self.nodes.iter_mut().enumerate() {
            node.closed = !blocked[id];
            total += usize::from(node.closed);
            initial += usize::from(node.closed && id < self.initial);
        }
        self.total_closed = total;
        self.initial_closed = initial;
        self.snapshot_revision = self.revision;
        self.last_refresh_seconds = started.elapsed().as_secs_f64();
        self.refresh_count = self.refresh_count.saturating_add(1);
        self.refresh_seconds += self.last_refresh_seconds;
        self.last_refresh = Some(Instant::now());
    }

    pub fn json(&self, total: usize, initial: usize) -> Value {
        let available =
            self.unavailable.is_none() && self.nodes.len() == total && self.initial == initial;
        json!({"available":available,"initial_total":initial,
            "initial_closed":available.then_some(self.initial_closed),
            "total_domains":total,"total_closed":available.then_some(self.total_closed),
            "unresolved_domains":available.then_some(total.saturating_sub(self.total_closed)),
            "locally_inspected":available.then_some(self.inspected),
            "dependency_edges":available.then_some(self.edges.len()),
            "graph_revision":self.revision,"snapshot_revision":self.snapshot_revision,
            "snapshot_stale":self.revision != self.snapshot_revision,
            "snapshot_age_seconds":self.last_refresh.map(|t|t.elapsed().as_secs_f64()),
            "last_refresh_seconds":self.last_refresh_seconds,
            "refresh_count":self.refresh_count,"refresh_seconds":self.refresh_seconds,
            "refresh_duty_bound":1.0 / REFRESH_DUTY_MULTIPLIER,
            "refresh_min_interval_seconds":REFRESH_MIN_INTERVAL_SECONDS,
            "next_refresh_seconds":self.next_refresh_seconds(),
            "retained_storage_estimate_bytes":self.storage_estimate_bytes(),
            "refresh_scratch_estimate_bytes":total.saturating_mul(size_of::<bool>() + size_of::<usize>()),
            "storage_estimate_scope":"logical capacities; excludes allocator/hash control bytes; not RSS",
            "method":"reverse_unsealed_reachability_including_sealed_cycles",
            "scope":"discovered_dependency_coverage; not termination, descent, or family certification",
            "closed_counts_are_conservative_lower_bounds":true,"family_closure_claim":false,
            "reason":if available {None} else {Some(self.unavailable.as_deref().unwrap_or("dependency domain inventory mismatch"))}})
    }

    fn storage_estimate_bytes(&self) -> usize {
        let mut bytes = self
            .nodes
            .capacity()
            .saturating_mul(size_of::<Node>())
            .saturating_add(self.edges.capacity().saturating_mul(size_of::<Edge>()))
            .saturating_add(
                self.open_targets
                    .capacity()
                    .saturating_mul(size_of::<(usize, HashSet<usize>)>()),
            );
        for targets in self.open_targets.values() {
            bytes = bytes.saturating_add(targets.capacity().saturating_mul(size_of::<usize>()));
        }
        bytes
    }

    pub fn closed(&self, id: usize) -> Option<bool> {
        self.unavailable
            .is_none()
            .then(|| self.nodes.get(id).map(|n| n.closed))
            .flatten()
    }

    pub fn local_status(&self, id: usize) -> Option<(bool, bool)> {
        self.unavailable
            .is_none()
            .then(|| self.nodes.get(id).map(|n| (n.inspected, n.sealed)))
            .flatten()
    }

    pub fn dependencies(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.edges.iter().map(|edge| (edge.source, edge.target))
    }

    /// Validate and rebuild only ephemeral dedup state. Never reconstruct
    /// missing historical dependencies from counters or domain geometry.
    pub fn restore(&mut self, total: usize, initial: usize) -> Result<(), String> {
        if self.unavailable.is_some() {
            return Ok(());
        }
        if self.nodes.len() != total
            || self.initial != initial
            || initial > total
            || self.snapshot_revision > self.revision
            || !self.refresh_seconds.is_finite()
            || self.refresh_seconds < 0.0
        {
            return Err("invalid checkpoint dependency inventory".into());
        }
        self.open_targets.clear();
        let mut heads = Vec::new();
        heads
            .try_reserve_exact(total)
            .map_err(|_| "dependency restore allocation")?;
        heads.resize(total, NONE);
        for (index, edge) in self.edges.iter().enumerate() {
            if edge.source >= total || edge.target >= total || edge.next != heads[edge.target] {
                return Err("invalid checkpoint dependency edge".into());
            }
            heads[edge.target] = index;
            if !self.nodes[edge.source].sealed {
                if !self.open_targets.contains_key(&edge.source) {
                    self.open_targets
                        .try_reserve(1)
                        .map_err(|_| "dependency restore allocation")?;
                }
                let targets = self.open_targets.entry(edge.source).or_default();
                targets
                    .try_reserve(1)
                    .map_err(|_| "dependency restore allocation")?;
                if !targets.insert(edge.target) {
                    return Err("duplicate checkpoint dependency edge".into());
                }
            }
        }
        if self
            .nodes
            .iter()
            .enumerate()
            .any(|(id, n)| n.incoming != heads[id] || n.closed && !n.sealed)
            || self.inspected != self.nodes.iter().filter(|n| n.inspected).count()
            || self.total_closed != self.nodes.iter().filter(|n| n.closed).count()
            || self.initial_closed != self.nodes[..initial].iter().filter(|n| n.closed).count()
            || self
                .edges
                .iter()
                .any(|e| self.nodes[e.source].closed && !self.nodes[e.target].closed)
        {
            return Err("invalid checkpoint dependency closure counters".into());
        }
        self.last_refresh = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "descendant_closure_tests.rs"]
mod reference_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn scan(graph: &mut Tracker) {
        graph.refresh(&AtomicBool::new(false), true);
    }
    #[test]
    fn chain_branch_shared_and_unrelated_roots() {
        let mut g = Tracker::new(3);
        g.discovered(5);
        g.edge(0, 3);
        g.edge(1, 3);
        g.edge(3, 4);
        g.edge(0, 4);
        g.edge(0, 3);
        for id in 0..4 {
            g.finish(id, true, true);
        }
        scan(&mut g);
        assert_eq!(g.initial_closed, 1);
        assert_eq!(g.total_closed, 1);
        assert_eq!(g.edges.len(), 4);
        g.finish(4, true, true);
        scan(&mut g);
        assert_eq!(g.initial_closed, 3);
        assert_eq!(g.total_closed, 5);
    }
    #[test]
    fn sealed_cycles_need_every_outgoing_dependency_and_no_frontier() {
        let mut g = Tracker::new(2);
        g.discovered(3);
        g.edge(0, 1);
        g.edge(1, 0);
        g.edge(1, 2);
        g.finish(0, true, true);
        g.finish(1, true, true);
        scan(&mut g);
        assert_eq!(g.total_closed, 0);
        g.finish(2, true, false);
        scan(&mut g);
        assert_eq!(g.total_closed, 0);
        let mut own = Tracker::new(1);
        own.edge(0, 0);
        own.finish(0, true, true);
        scan(&mut own);
        assert_eq!(own.total_closed, 1);
        g.finish(2, true, true);
        scan(&mut g);
        assert_eq!(g.total_closed, 3);
    }
    #[test]
    fn alias_and_partial_anchor_block_until_target_closed() {
        let mut g = Tracker::new(1);
        g.discovered(3);
        g.edge(0, 1);
        g.edge(1, 2);
        g.finish(0, true, true);
        g.finish(1, false, true);
        scan(&mut g);
        assert_eq!(g.total_closed, 0);
        g.finish(2, true, true);
        scan(&mut g);
        assert_eq!(g.total_closed, 3);
        assert_eq!(g.inspected, 2);
    }
    #[test]
    fn interrupted_prefix_roundtrip_keeps_edges_and_deduplicates_replay() {
        let mut g = Tracker::new(1);
        g.discovered(2);
        g.edge(0, 1);
        scan(&mut g);
        let mut restored: Tracker =
            serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
        restored.restore(2, 1).unwrap();
        restored.edge(0, 1);
        assert_eq!(restored.edges.len(), 1);
        restored.finish(0, true, true);
        restored.refresh(&AtomicBool::new(true), true);
        assert_eq!(restored.total_closed, 0);
        restored.finish(1, true, true);
        scan(&mut restored);
        assert_eq!(restored.total_closed, 2);
        restored.edge(0, 1);
        assert!(!restored.json(2, 1)["available"].as_bool().unwrap());
    }
}
