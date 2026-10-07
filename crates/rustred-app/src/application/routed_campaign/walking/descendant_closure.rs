//! Observational dependency coverage, never algebraic termination authority.
//! A node is sealed only after its entire valid callback stream was accepted.
//! Reverse reachability from unsealed nodes blocks all their ancestors. The
//! complement includes sealed cycles with no unresolved outgoing dependency;
//! this is scoped finite worklist coverage, NOT a descent/family certificate.
mod bulk;
mod edges;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FLAG_SEALED: u8 = 1;
const FLAG_INSPECTED: u8 = 2;
const FLAG_CLOSED: u8 = 4;

/// Fold the append log into the CSR once it exceeds 1/16 of the folded
/// edges (amortized O(1) per edge) or this many edges.
const FOLD_LOG_FRACTION: usize = 16;
const FOLD_LOG_EDGES: usize = 64 << 20;

/// Persisted scalar state. Node flags and (source, target) edges travel in
/// their own checkpoint sections; the incoming lists are rebuilt on restore.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counters {
    pub initial: usize,
    pub unavailable: Option<String>,
    pub revision: u64,
    pub snapshot_revision: u64,
    pub initial_closed: usize,
    pub total_closed: usize,
    pub inspected: usize,
    pub refresh_count: u64,
    pub refresh_seconds: f64,
    /// Optional diagnostic history; older checkpoints did not record scan times.
    #[serde(default)]
    scan_history: ScanHistory,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanHistory {
    previous: Option<CompletedScan>,
    latest: Option<CompletedScan>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedScan {
    completed_unix_seconds: Option<f64>,
    total_domains: usize,
    total_closed: usize,
    initial_closed: usize,
    refresh_count: u64,
    scan_seconds: f64,
}

fn unix_seconds() -> Option<f64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs_f64())
}

/// Periodic refresh spacing as a multiple of the last scan's wall time: the
/// coordinator spends at most ~1/multiplier of its wall in closure scans.
pub(super) const REFRESH_DUTY_MULTIPLIER: f64 = 100.0;
pub(super) const REFRESH_MIN_INTERVAL_SECONDS: f64 = 5.0;

/// The refresh scratch (blocked bitset, u32 stack) could not be reserved.
struct ScratchUnavailable;

/// Node flags as bytes (bit0 sealed, bit1 inspected, bit2 closed) and the
/// edges as a u32 CSR-by-target plus an append log (`edges.rs`). The log is
/// folded after a checkpoint save persisted it; without a checkpoint store
/// it grows at 12 B per edge (refresh walks its per-target chains) and folds
/// only when its u32 chain links are exhausted.
pub(super) struct Tracker {
    flags: Vec<u8>,
    edges: edges::Edges,
    initial: usize,
    unavailable: Option<String>,
    revision: u64,
    snapshot_revision: u64,
    initial_closed: usize,
    total_closed: usize,
    inspected: usize,
    refresh_count: u64,
    refresh_seconds: f64,
    open_targets: HashMap<u32, HashSet<u32>>,
    last_refresh: Option<Instant>,
    last_refresh_seconds: f64,
    scan_history: ScanHistory,
}

impl Tracker {
    pub fn new(initial: usize) -> Self {
        let mut value = Self {
            flags: Vec::new(),
            edges: edges::Edges::default(),
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
            scan_history: ScanHistory::default(),
        };
        value.discovered(initial);
        value
    }

    pub fn disable(&mut self, reason: &str) {
        if self.unavailable.is_none() {
            self.unavailable = Some(reason.to_owned());
        }
        // Release optional monitoring allocations; mathematical state survives.
        self.flags = Vec::new();
        self.edges = edges::Edges::default();
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
        let Some(extra) = total.checked_sub(self.flags.len()) else {
            self.disable("dependency domain count regressed");
            return;
        };
        if extra == 0 {
            return;
        }
        if self.flags.try_reserve(extra).is_err() || self.edges.grow(total).is_err() {
            self.disable("dependency node allocation unavailable");
            return;
        }
        self.flags.resize(total, 0);
        self.changed();
    }

    pub fn edge(&mut self, source: usize, target: usize) {
        if self.unavailable.is_some() {
            return;
        }
        if source >= self.flags.len() || target >= self.flags.len() {
            self.disable("dependency endpoint outside discovered domain set");
            return;
        }
        if self.flags[source] & FLAG_SEALED != 0 {
            self.disable("dependency added after source sealed");
            return;
        }
        // Node ids fit u32: `discovered` bounds the node count below u32::MAX.
        let (source, target) = (source as u32, target as u32);
        if !self.open_targets.contains_key(&source) && self.open_targets.try_reserve(1).is_err() {
            self.disable("dependency deduplication allocation unavailable");
            return;
        }
        let targets = self.open_targets.entry(source).or_default();
        if targets.contains(&target) {
            return;
        }
        if targets.try_reserve(1).is_err() || self.edges.push(source, target).is_err() {
            self.disable("dependency edge allocation unavailable");
            return;
        }
        targets.insert(target);
        self.changed();
    }

    pub fn finish(&mut self, id: usize, inspected: bool, success: bool) {
        if self.unavailable.is_some() {
            return;
        }
        let Some(flag) = self.flags.get_mut(id) else {
            self.disable("dependency completion outside discovered domain set");
            return;
        };
        if *flag & FLAG_SEALED != 0 {
            self.disable("dependency node sealed twice");
            return;
        }
        if inspected && *flag & FLAG_INSPECTED == 0 {
            *flag |= FLAG_INSPECTED;
            self.inspected += 1; // Bounded by allocated node count.
        }
        if success {
            *flag |= FLAG_SEALED;
            self.open_targets.remove(&(id as u32));
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
        #[cfg(test)]
        super::epoch::assert_large_finalization_allowed();
        if self.scan(cancellation, force).is_err() {
            self.disable("dependency refresh allocation unavailable");
        }
    }

    /// Explicit coordinator monitoring maintenance, never a save/finalizer.
    /// Reuse the periodic scan's dirty/duty gates and real run cancellation;
    /// cached CLOSED bits remain conservative when a scan is skipped/cut.
    pub(super) fn refresh_periodic_monitor(&mut self, cancellation: &AtomicBool) {
        if cancellation.load(Ordering::Relaxed) {
            return;
        }
        if self.scan(cancellation, false).is_err() {
            self.disable("dependency refresh allocation unavailable");
        }
    }

    /// O(1) invocation-local freshness; restored snapshots have unknown age.
    /// Unlike `json`, this never walks retained open-target storage.
    pub(super) fn snapshot_age_seconds(&self) -> Option<f64> {
        self.last_refresh.map(|time| time.elapsed().as_secs_f64())
    }

    pub(super) fn last_refresh_seconds(&self) -> f64 {
        self.last_refresh_seconds
    }

    /// The forced scan before a checkpoint save. As in 102adcc3, the run's
    /// cancellation never cuts it: the CLOSED bits and closure counters a
    /// generation persists (the paused one a stop request leaves included)
    /// are those of a current snapshot. Only missing scratch memory leaves
    /// the previous snapshot in place: stale but valid (closed nodes stay
    /// closed), which restore accepts, so the save never gives up the monitor
    /// it is about to persist (102adcc3 disabled it there).
    pub fn refresh_before_save(&mut self) {
        #[cfg(test)]
        super::epoch::assert_large_finalization_allowed();
        let _ = self.scan(&AtomicBool::new(false), true);
    }

    fn scan(&mut self, cancellation: &AtomicBool, force: bool) -> Result<(), ScratchUnavailable> {
        if self.unavailable.is_some() || self.revision == self.snapshot_revision {
            return Ok(());
        }
        let interval = self.refresh_interval();
        if !force
            && self
                .last_refresh
                .is_some_and(|time| time.elapsed() < interval)
        {
            return Ok(());
        }
        let started = Instant::now();
        let nodes = self.flags.len();
        let mut blocked: Vec<u64> = Vec::new();
        let mut stack: Vec<u32> = Vec::new();
        if blocked.try_reserve_exact(nodes.div_ceil(64)).is_err()
            || stack.try_reserve_exact(nodes).is_err()
        {
            return Err(ScratchUnavailable);
        }
        blocked.resize(nodes.div_ceil(64), 0);
        for (id, &flag) in self.flags.iter().enumerate() {
            if id % 1024 == 0 && cancellation.load(Ordering::Relaxed) {
                return Ok(());
            }
            if flag & FLAG_SEALED == 0 {
                blocked[id / 64] |= 1 << (id % 64);
                stack.push(id as u32);
            }
        }
        let mut work = 0usize;
        while let Some(id) = stack.pop() {
            let visit = self.edges.incoming(id as usize).try_for_each(|source| {
                if work % 1024 == 0 && cancellation.load(Ordering::Relaxed) {
                    return ControlFlow::Break(());
                }
                work = work.wrapping_add(1);
                let (word, bit) = (source as usize / 64, 1u64 << (source % 64));
                if blocked[word] & bit == 0 {
                    blocked[word] |= bit;
                    stack.push(source);
                }
                ControlFlow::Continue(())
            });
            if visit.is_break() {
                return Ok(());
            }
        }
        let mut total = 0;
        let mut initial = 0;
        // No mutation until the cancellable scan completed. Closed nodes can
        // never acquire later edges because sources must be unsealed at edge().
        for (id, flag) in self.flags.iter_mut().enumerate() {
            let closed = blocked[id / 64] & (1 << (id % 64)) == 0;
            *flag = (*flag & !FLAG_CLOSED) | u8::from(closed) * FLAG_CLOSED;
            total += usize::from(closed);
            initial += usize::from(closed && id < self.initial);
        }
        self.total_closed = total;
        self.initial_closed = initial;
        self.snapshot_revision = self.revision;
        self.last_refresh_seconds = started.elapsed().as_secs_f64();
        self.refresh_count = self.refresh_count.saturating_add(1);
        self.refresh_seconds += self.last_refresh_seconds;
        self.last_refresh = Some(Instant::now());
        self.scan_history.previous = self.scan_history.latest.take();
        self.scan_history.latest = Some(CompletedScan {
            completed_unix_seconds: unix_seconds(),
            total_domains: nodes,
            total_closed: total,
            initial_closed: initial,
            refresh_count: self.refresh_count,
            scan_seconds: self.last_refresh_seconds,
        });
        Ok(())
    }

    /// Observational eligibility for the existing periodic refresh throttle.
    /// This does not promise a scan at a particular wall-clock time: the
    /// coordinator must first reach a cooperative monitoring boundary.
    pub fn refresh_policy_json(&self) -> Value {
        let delay = self.next_refresh_seconds();
        let status = if self.unavailable.is_some() {
            "unavailable"
        } else if self.revision == self.snapshot_revision {
            "unchanged"
        } else if delay.is_some_and(|seconds| seconds > 0.0) {
            "throttled"
        } else {
            "eligible"
        };
        let earliest = matches!(status, "eligible" | "throttled")
            .then(|| unix_seconds().map(|now| now + delay.unwrap_or(0.0)))
            .flatten();
        json!({"duty_bound":1.0 / REFRESH_DUTY_MULTIPLIER,
            "min_interval_seconds":REFRESH_MIN_INTERVAL_SECONDS,
            "next_refresh_seconds":delay,"status":status,
            "earliest_refresh_unix_seconds":earliest,
            "schedule_scope":"eligibility at a coordinator monitoring boundary, not a promised execution time",
            "scope":"periodic_refresh_spacing_max(min_interval, last_scan_wall / duty_bound); forced_refreshes_bypass"})
    }

    /// Last two *completed* scans, not repeated heartbeat observations. This
    /// O(1) diagnostic survives new checkpoints and never triggers a scan.
    pub(super) fn scan_history_json(&self) -> Value {
        json!(self.scan_history)
    }

    pub fn json(&self, total: usize, initial: usize) -> Value {
        #[cfg(test)]
        super::epoch::assert_large_finalization_allowed();
        let available =
            self.unavailable.is_none() && self.flags.len() == total && self.initial == initial;
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
            "scan_history":self.scan_history_json(),
            "retained_storage_estimate_bytes":self.storage_estimate_bytes(),
            "refresh_scratch_estimate_bytes":total.div_ceil(64).saturating_mul(size_of::<u64>())
                .saturating_add(total.saturating_mul(size_of::<u32>())),
            "storage_estimate_scope":"logical capacities; excludes allocator/hash control bytes; not RSS",
            "method":"reverse_unsealed_reachability_including_sealed_cycles",
            "scope":"discovered_dependency_coverage; not termination, descent, or family certification",
            "closed_counts_are_conservative_lower_bounds":true,"family_closure_claim":false,
            "reason":if available {None} else {Some(self.unavailable.as_deref().unwrap_or("dependency domain inventory mismatch"))}})
    }

    fn storage_estimate_bytes(&self) -> usize {
        let mut bytes = self
            .flags
            .capacity()
            .saturating_add(self.edges.storage_bytes())
            .saturating_add(
                self.open_targets
                    .capacity()
                    .saturating_mul(size_of::<(u32, HashSet<u32>)>()),
            );
        for targets in self.open_targets.values() {
            bytes = bytes.saturating_add(targets.capacity().saturating_mul(size_of::<u32>()));
        }
        bytes
    }

    /// Frontier taint for the resume-time rescue (`rescue.rs`): a bitset (one
    /// bit per node) of every node that reaches, over recorded edges, a node
    /// that finished its inspection unsealed (a native that kept frontiers;
    /// a failed inspection never reaches a checkpoint). Such a node can never
    /// close, so neither can any node in the set. Pending nodes are not
    /// tainted by themselves. None when the monitor is unavailable (the
    /// rescue then refuses: it cannot tell which nodes are blocked).
    pub fn tainted(&self) -> Option<Vec<u64>> {
        self.tainted_with(&[])
    }

    /// `tainted`, also seeded with the IDs set in `extra` (the rescue's dead
    /// pending obligations): every node reaching a frontier-bearing native
    /// or a seed.
    pub fn tainted_with(&self, extra: &[u64]) -> Option<Vec<u64>> {
        self.tainted_with_cancellable(extra, &mut || false)
    }

    /// Resume preparation may stop between bounded scan chunks. `None` also
    /// covers unavailable scratch; the caller retains its cancellation flag
    /// to distinguish interruption from allocation failure.
    pub fn tainted_with_cancellable(
        &self,
        extra: &[u64],
        stop: &mut impl FnMut() -> bool,
    ) -> Option<Vec<u64>> {
        if self.unavailable.is_some() || stop() {
            return None;
        }
        let nodes = self.flags.len();
        let mut tainted = Vec::new();
        tainted.try_reserve_exact(nodes.div_ceil(64)).ok()?;
        tainted.resize(nodes.div_ceil(64), 0u64);
        let mut stack: Vec<u32> = Vec::new();
        if stop() {
            return None;
        }
        for (id, &flag) in self.flags.iter().enumerate() {
            if id % 1024 == 0 && stop() {
                return None;
            }
            let seeded = extra.get(id / 64).is_some_and(|w| w >> (id % 64) & 1 != 0);
            if seeded || flag & FLAG_INSPECTED != 0 && flag & FLAG_SEALED == 0 {
                tainted[id / 64] |= 1 << (id % 64);
                stack.try_reserve(1).ok()?;
                stack.push(id as u32);
            }
        }
        let mut visited = 0usize;
        while let Some(id) = stack.pop() {
            visited = visited.wrapping_add(1);
            if visited % 1024 == 0 && stop() {
                return None;
            }
            for source in self.edges.incoming(id as usize) {
                visited = visited.wrapping_add(1);
                if visited % 1024 == 0 && stop() {
                    return None;
                }
                let (word, bit) = (source as usize / 64, 1u64 << (source % 64));
                if tainted[word] & bit == 0 {
                    tainted[word] |= bit;
                    stack.try_reserve(1).ok()?;
                    stack.push(source);
                }
            }
        }
        Some(tainted)
    }

    /// Liveness for the rescue: a bitset of every node reachable over
    /// recorded edges from `roots` (the untainted input roots). Builds a
    /// transient CSR by source (4 B per edge plus 8 B per node). None when the
    /// monitor is unavailable or the scratch cannot be reserved.
    pub fn reachable_from(&self, roots: impl IntoIterator<Item = usize>) -> Option<Vec<u64>> {
        self.reachable_from_cancellable(roots, &mut || false)
    }

    pub fn reachable_from_cancellable(
        &self,
        roots: impl IntoIterator<Item = usize>,
        stop: &mut impl FnMut() -> bool,
    ) -> Option<Vec<u64>> {
        if self.unavailable.is_some() || stop() {
            return None;
        }
        let nodes = self.flags.len();
        let mut offsets: Vec<usize> = Vec::new();
        offsets.try_reserve_exact(nodes + 1).ok()?;
        offsets.resize(nodes + 1, 0);
        let mut visited = 0usize;
        self.edges
            .try_for_each(|source, _| {
                visited = visited.wrapping_add(1);
                if visited % 1024 == 0 && stop() {
                    return ControlFlow::Break(());
                }
                offsets[source as usize + 1] += 1;
                ControlFlow::<()>::Continue(())
            })
            .continue_value()?;
        for index in 1..offsets.len() {
            if index % 1024 == 0 && stop() {
                return None;
            }
            offsets[index] += offsets[index - 1];
        }
        let mut targets: Vec<u32> = Vec::new();
        if stop() {
            return None;
        }
        targets.try_reserve_exact(offsets[nodes]).ok()?;
        targets.resize(offsets[nodes], 0);
        if stop() {
            return None;
        }
        let mut fill = Vec::new();
        fill.try_reserve_exact(offsets.len()).ok()?;
        fill.extend_from_slice(&offsets);
        self.edges
            .try_for_each(|source, target| {
                visited = visited.wrapping_add(1);
                if visited % 1024 == 0 && stop() {
                    return ControlFlow::Break(());
                }
                let slot = &mut fill[source as usize];
                targets[*slot] = target;
                *slot += 1;
                ControlFlow::<()>::Continue(())
            })
            .continue_value()?;
        drop(fill);
        if stop() {
            return None;
        }
        let mut live = Vec::new();
        live.try_reserve_exact(nodes.div_ceil(64)).ok()?;
        live.resize(nodes.div_ceil(64), 0u64);
        let mut stack: Vec<usize> = Vec::new();
        if stop() {
            return None;
        }
        for (index, root) in roots.into_iter().enumerate() {
            if index % 1024 == 0 && stop() {
                return None;
            }
            if root < nodes && live[root / 64] >> (root % 64) & 1 == 0 {
                live[root / 64] |= 1 << (root % 64);
                stack.try_reserve(1).ok()?;
                stack.push(root);
            }
        }
        while let Some(id) = stack.pop() {
            visited = visited.wrapping_add(1);
            if visited % 1024 == 0 && stop() {
                return None;
            }
            for &target in &targets[offsets[id]..offsets[id + 1]] {
                visited = visited.wrapping_add(1);
                if visited % 1024 == 0 && stop() {
                    return None;
                }
                let target = target as usize;
                if live[target / 64] >> (target % 64) & 1 == 0 {
                    live[target / 64] |= 1 << (target % 64);
                    stack.try_reserve(1).ok()?;
                    stack.push(target);
                }
            }
        }
        Some(live)
    }

    pub fn closed(&self, id: usize) -> Option<bool> {
        self.unavailable
            .is_none()
            .then(|| self.flags.get(id).map(|&f| f & FLAG_CLOSED != 0))
            .flatten()
    }

    pub fn local_status(&self, id: usize) -> Option<(bool, bool)> {
        self.unavailable
            .is_none()
            .then(|| {
                self.flags
                    .get(id)
                    .map(|&f| (f & FLAG_INSPECTED != 0, f & FLAG_SEALED != 0))
            })
            .flatten()
    }

    /// Every (source, target) edge exactly once, in unspecified order
    /// (currently folded edges grouped by target, then the unfolded log in
    /// insertion order). Validation must not depend on the order. Test-only:
    /// production validation iterates with `try_for_each_edge`.
    #[cfg(test)]
    pub fn dependencies(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.edges
            .iter()
            .map(|(source, target)| (source as usize, target as usize))
    }

    /// `dependencies` as internal iteration (`fold` walks the CSR slices as
    /// plain loops): every edge exactly once, order unspecified. Test-only
    /// (the closure tests and the edge benchmark).
    #[cfg(test)]
    pub fn for_each_edge(&self, mut visit: impl FnMut(usize, usize)) {
        self.dependencies()
            .for_each(|(source, target)| visit(source, target));
    }

    /// `for_each_edge` that stops at the first `Break` (plain loops over the
    /// CSR slices, then the log): every edge at most once, order unspecified.
    pub fn try_for_each_edge<B>(
        &self,
        mut visit: impl FnMut(usize, usize) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        self.edges
            .try_for_each(|source, target| visit(source as usize, target as usize))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn node_count(&self) -> usize {
        self.flags.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Edges whose insertion order a fold gave up; a checkpoint tiling that
    /// ends below this count cannot be extended and must be re-tiled.
    pub fn folded_edge_count(&self) -> usize {
        self.edges.folded()
    }

    /// Checkpoint edge segment `[first, first + count)`: the unpersisted log
    /// tail in insertion order, or (`first == 0`) a full re-tile.
    pub fn edge_segment(
        &self,
        first: usize,
        count: usize,
    ) -> Result<impl Iterator<Item = (u32, u32)> + '_, String> {
        self.edges.segment(first, count)
    }

    /// A save made the first `persisted` edges durable in insertion order.
    /// When that covers the whole log, fold it into the CSR once it outgrew
    /// `1 / FOLD_LOG_FRACTION` of the folded edges or `FOLD_LOG_EDGES`. The
    /// fold changes representation only (no revision bump); a failed scratch
    /// allocation keeps the log.
    pub fn persisted(&mut self, persisted: usize) {
        let log = self.edges.log_len();
        if self.unavailable.is_some() || persisted != self.edges.len() || log == 0 {
            return;
        }
        if log > self.edges.folded() / FOLD_LOG_FRACTION || log > FOLD_LOG_EDGES {
            let _ = self.edges.fold(self.flags.len());
        }
    }

    pub fn counters(&self) -> Counters {
        Counters {
            initial: self.initial,
            unavailable: self.unavailable.clone(),
            revision: self.revision,
            snapshot_revision: self.snapshot_revision,
            initial_closed: self.initial_closed,
            total_closed: self.total_closed,
            inspected: self.inspected,
            refresh_count: self.refresh_count,
            refresh_seconds: self.refresh_seconds,
            scan_history: self.scan_history.clone(),
        }
    }

    /// One byte per node: bit0 sealed, bit1 inspected, bit2 closed.
    pub fn node_flags(&self) -> impl Iterator<Item = u8> + '_ {
        self.flags.iter().copied()
    }

    /// Rebuild the CSR from the persisted edges (all folded; insertion order
    /// kept within a target). The caller must still run `restore`, which
    /// validates counters, deduplication and closure consistency; nothing is
    /// reconstructed from geometry.
    pub fn from_parts(
        counters: Counters,
        flags: &[u8],
        edges: &[(u32, u32)],
    ) -> Result<Self, String> {
        if counters.unavailable.is_some() && (!flags.is_empty() || !edges.is_empty()) {
            return Err(if flags.is_empty() {
                "invalid checkpoint dependency edge".into()
            } else {
                "unavailable dependency monitor retains nodes".into()
            });
        }
        if flags
            .iter()
            .any(|flag| flag & !(FLAG_SEALED | FLAG_INSPECTED | FLAG_CLOSED) != 0)
        {
            return Err("invalid checkpoint dependency node flags".into());
        }
        let mut stored = Vec::new();
        stored
            .try_reserve_exact(flags.len())
            .map_err(|_| "dependency restore allocation")?;
        stored.extend_from_slice(flags);
        Ok(Self {
            edges: edges::Edges::from_pairs(flags.len(), edges)?,
            flags: stored,
            initial: counters.initial,
            unavailable: counters.unavailable,
            revision: counters.revision,
            snapshot_revision: counters.snapshot_revision,
            initial_closed: counters.initial_closed,
            total_closed: counters.total_closed,
            inspected: counters.inspected,
            refresh_count: counters.refresh_count,
            refresh_seconds: counters.refresh_seconds,
            open_targets: HashMap::new(),
            last_refresh: None,
            last_refresh_seconds: 0.0,
            scan_history: counters.scan_history,
        })
    }

    /// Epoch restore already owns the decoded flags and authenticated run
    /// log. Move flags and build CSR in two passes over a repeatable run
    /// iterator; do not clone flags or expand all runs into temporary pairs.
    /// The same `restore` validator still runs before any state is usable.
    pub fn from_owned_parts(
        counters: Counters,
        flags: Vec<u8>,
        pairs: impl Iterator<Item = (u32, u32)> + Clone,
    ) -> Result<Self, String> {
        if counters.unavailable.is_some() && (!flags.is_empty() || pairs.clone().next().is_some()) {
            return Err("unavailable dependency monitor retains nodes or edges".into());
        }
        if flags
            .iter()
            .any(|flag| flag & !(FLAG_SEALED | FLAG_INSPECTED | FLAG_CLOSED) != 0)
        {
            return Err("invalid checkpoint dependency node flags".into());
        }
        Ok(Self {
            edges: edges::Edges::from_iter(flags.len(), pairs)?,
            flags,
            initial: counters.initial,
            unavailable: counters.unavailable,
            revision: counters.revision,
            snapshot_revision: counters.snapshot_revision,
            initial_closed: counters.initial_closed,
            total_closed: counters.total_closed,
            inspected: counters.inspected,
            refresh_count: counters.refresh_count,
            refresh_seconds: counters.refresh_seconds,
            open_targets: HashMap::new(),
            last_refresh: None,
            last_refresh_seconds: 0.0,
            scan_history: counters.scan_history,
        })
    }

    /// Validate and rebuild only ephemeral dedup state. Never reconstruct
    /// missing historical dependencies from counters or domain geometry.
    /// Endpoint ranges are checked here again (and in `from_parts`); the
    /// persisted order itself is authenticated by the segment digests.
    pub fn restore(&mut self, total: usize, initial: usize) -> Result<(), String> {
        if self.unavailable.is_some() {
            return Ok(());
        }
        if self.flags.len() != total
            || self.initial != initial
            || initial > total
            || self.snapshot_revision > self.revision
            || !self.refresh_seconds.is_finite()
            || self.refresh_seconds < 0.0
        {
            return Err("invalid checkpoint dependency inventory".into());
        }
        self.open_targets.clear();
        let (flags, open_targets) = (&self.flags, &mut self.open_targets);
        let mut closed_frontier = false;
        let invalid = self.edges.try_for_each(|source, target| {
            let (Some(&from), Some(&to)) = (flags.get(source as usize), flags.get(target as usize))
            else {
                return ControlFlow::Break("invalid checkpoint dependency edge");
            };
            closed_frontier |= from & FLAG_CLOSED != 0 && to & FLAG_CLOSED == 0;
            if from & FLAG_SEALED == 0 {
                if !open_targets.contains_key(&source) && open_targets.try_reserve(1).is_err() {
                    return ControlFlow::Break("dependency restore allocation");
                }
                let targets = open_targets.entry(source).or_default();
                if targets.try_reserve(1).is_err() {
                    return ControlFlow::Break("dependency restore allocation");
                }
                if !targets.insert(target) {
                    return ControlFlow::Break("duplicate checkpoint dependency edge");
                }
            }
            ControlFlow::Continue(())
        });
        if let ControlFlow::Break(error) = invalid {
            return Err(error.into());
        }
        let count = |flags: &[u8], bit: u8| flags.iter().filter(|&&f| f & bit != 0).count();
        if closed_frontier
            || self
                .flags
                .iter()
                .any(|&f| f & FLAG_CLOSED != 0 && f & FLAG_SEALED == 0)
            || self.inspected != count(&self.flags, FLAG_INSPECTED)
            || self.total_closed != count(&self.flags, FLAG_CLOSED)
            || self.initial_closed != count(&self.flags[..initial], FLAG_CLOSED)
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
mod edges_benchmark;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn periodic_monitor_is_throttled_cancelled_and_not_a_forced_finalizer() {
        let mut graph = Tracker::new(2);
        graph.finish(0, true, true);
        assert!(graph.snapshot_age_seconds().is_none());
        // The CP6 guard still forbids refresh/json finalizers, but explicitly
        // scheduled monitoring is allowed to use the periodic scan.
        super::super::epoch::FORBID_LARGE_FINALIZATION.with(|flag| flag.set(true));
        graph.refresh_periodic_monitor(&AtomicBool::new(false));
        super::super::epoch::FORBID_LARGE_FINALIZATION.with(|flag| flag.set(false));
        assert_eq!((graph.refresh_count, graph.total_closed), (1, 1));
        assert!(graph.snapshot_age_seconds().unwrap() >= 0.0);
        graph.finish(1, true, true);
        let before = parts(&graph);
        graph.refresh_periodic_monitor(&AtomicBool::new(false));
        assert_eq!(graph.refresh_count, 1, "second dirty cut is throttled");
        graph.last_refresh = Some(Instant::now() - graph.refresh_interval());
        graph.refresh_periodic_monitor(&AtomicBool::new(true));
        assert_eq!(
            parts(&graph),
            before,
            "cancelled scan preserves saved flags/edges"
        );
        assert_eq!(graph.refresh_count, 1);
        graph.refresh_periodic_monitor(&AtomicBool::new(false));
        assert_eq!((graph.refresh_count, graph.total_closed), (2, 2));
        assert_eq!(
            parts(&graph).1,
            before.1,
            "monitor never changes dependencies"
        );
        let (flags, edges) = parts(&graph);
        let restored = Tracker::from_parts(graph.counters(), &flags, &edges).unwrap();
        assert!(restored.snapshot_age_seconds().is_none());
        assert_eq!(restored.refresh_count, 2);
    }

    fn scan(graph: &mut Tracker) {
        graph.refresh(&AtomicBool::new(false), true);
    }
    /// The persisted parts of a tracker, as the checkpoint sections hold them.
    fn parts(graph: &Tracker) -> (Vec<u8>, Vec<(u32, u32)>) {
        let edges = graph.edge_segment(0, graph.edge_count()).unwrap().collect();
        (graph.node_flags().collect(), edges)
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
        assert_eq!(g.edge_count(), 4);
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
    fn parts_rebuild_incoming_lists_and_reject_bad_flags_or_endpoints() {
        let mut g = Tracker::new(1);
        g.discovered(3);
        g.edge(0, 1);
        g.edge(1, 2);
        g.edge(0, 2);
        g.finish(0, true, true);
        scan(&mut g);
        let (flags, edges) = parts(&g);
        assert_eq!(flags, [3, 0, 0]);
        // Unfolded: the log in insertion order.
        assert_eq!(edges, [(0, 1), (1, 2), (0, 2)]);
        let mut restored = Tracker::from_parts(g.counters(), &flags, &edges).unwrap();
        restored.restore(3, 1).unwrap();
        // Rebuilt as CSR by target, insertion order within a target.
        assert_eq!(restored.folded_edge_count(), 3);
        assert_eq!(restored.edges.incoming(2).collect::<Vec<_>>(), [1, 0]);
        assert_eq!(
            restored.dependencies().collect::<Vec<_>>(),
            [(0, 1), (1, 2), (0, 2)]
        );
        assert_eq!(restored.total_closed, 0);
        restored.edge(1, 2); // Deduplicated against the rebuilt open set.
        assert_eq!(restored.edge_count(), 3);
        assert!(Tracker::from_parts(g.counters(), &[8, 0, 0], &edges).is_err());
        assert!(Tracker::from_parts(g.counters(), &flags, &[(0, 3)]).is_err());
        let mut disabled = g.counters();
        disabled.unavailable = Some("test".into());
        assert!(Tracker::from_parts(disabled, &flags, &[]).is_err());
        let mut bad = Tracker::from_parts(g.counters(), &flags, &edges).unwrap();
        bad.total_closed = 1;
        assert!(bad.restore(3, 1).is_err());
    }
    #[test]
    fn owned_parts_move_flags_and_preserve_stale_closure_and_edge_order() {
        let mut original = Tracker::new(1);
        original.discovered(3);
        original.edge(0, 1);
        original.edge(1, 2);
        original.edge(0, 2);
        original.finish(2, true, true);
        scan(&mut original);
        original.finish(1, true, true); // A valid deliberately stale snapshot.
        let (flags, pairs) = parts(&original);
        let allocation = flags.as_ptr();
        let mut restored =
            Tracker::from_owned_parts(original.counters(), flags, pairs.iter().copied()).unwrap();
        assert_eq!(
            allocation,
            restored.flags.as_ptr(),
            "flags move without cloning"
        );
        restored.restore(3, 1).unwrap();
        assert_eq!(restored.flags, original.flags);
        assert_eq!(restored.total_closed, 1);
        assert_ne!(restored.snapshot_revision, restored.revision);
        assert_eq!(
            restored.dependencies().collect::<Vec<_>>(),
            pairs
                .iter()
                .map(|&(source, target)| (source as usize, target as usize))
                .collect::<Vec<_>>()
        );
        assert_eq!(restored.edges.incoming(2).collect::<Vec<_>>(), [1, 0]);
        restored.finish(0, true, true);
        scan(&mut restored);
        assert_eq!(restored.total_closed, 3);
        assert!(
            Tracker::from_owned_parts(original.counters(), vec![8, 0, 0], pairs.iter().copied())
                .is_err()
        );
        assert!(
            Tracker::from_owned_parts(original.counters(), vec![0; 3], [(0, 3)].into_iter())
                .is_err()
        );
    }

    #[test]
    fn owned_parts_refuse_equal_length_changed_iterator_distribution() {
        struct Changed {
            rows: std::vec::IntoIter<(u32, u32)>,
        }
        impl Iterator for Changed {
            type Item = (u32, u32);
            fn next(&mut self) -> Option<Self::Item> {
                self.rows.next()
            }
        }
        impl Clone for Changed {
            fn clone(&self) -> Self {
                Self {
                    rows: vec![(0, 1), (1, 2)].into_iter(),
                }
            }
        }
        let source = Tracker::new(3);
        // First pass sees one edge to each target, second pass sees two to
        // target 1. Both inventories have valid endpoints and equal length.
        let pairs = Changed {
            rows: vec![(0, 1), (2, 1)].into_iter(),
        };
        assert!(Tracker::from_owned_parts(source.counters(), vec![0; 3], pairs).is_err());
    }
    #[test]
    fn interrupted_prefix_roundtrip_keeps_edges_and_deduplicates_replay() {
        let mut g = Tracker::new(1);
        g.discovered(2);
        g.edge(0, 1);
        scan(&mut g);
        let (flags, edges) = parts(&g);
        let mut restored = Tracker::from_parts(g.counters(), &flags, &edges).unwrap();
        restored.restore(2, 1).unwrap();
        restored.edge(0, 1);
        assert_eq!(restored.edge_count(), 1);
        restored.finish(0, true, true);
        restored.refresh(&AtomicBool::new(true), true);
        assert_eq!(restored.total_closed, 0);
        restored.finish(1, true, true);
        scan(&mut restored);
        assert_eq!(restored.total_closed, 2);
        restored.edge(0, 1);
        assert!(!restored.json(2, 1)["available"].as_bool().unwrap());
    }
    #[test]
    fn save_path_refresh_is_never_throttled_and_keeps_the_monitor() {
        let mut g = Tracker::new(1);
        g.discovered(2);
        g.edge(0, 1);
        g.finish(1, true, true);
        g.refresh(&AtomicBool::new(false), true);
        assert_eq!((g.refresh_count, g.total_closed), (1, 1));
        g.finish(0, true, true);
        g.refresh(&AtomicBool::new(false), false);
        assert_eq!(g.refresh_count, 1, "the periodic scan is throttled");
        assert!(g.json(2, 1)["snapshot_stale"].as_bool().unwrap());
        g.refresh_before_save();
        assert_eq!(g.refresh_count, 2);
        assert_eq!(g.total_closed, 2);
        assert!(!g.json(2, 1)["snapshot_stale"].as_bool().unwrap());
        assert!(g.unavailable.is_none());
    }
    #[test]
    fn fold_only_after_the_whole_log_was_persisted() {
        let mut g = Tracker::new(4);
        g.edge(0, 1);
        g.edge(0, 2);
        g.persisted(1); // An unpersisted tail keeps its insertion order.
        assert_eq!(g.folded_edge_count(), 0);
        g.persisted(2);
        assert_eq!(g.folded_edge_count(), 2);
        let revision = g.revision();
        g.edge(1, 3);
        assert!(g.edge_segment(1, 1).is_err(), "inside the folded prefix");
        assert_eq!(g.edge_segment(2, 1).unwrap().collect::<Vec<_>>(), [(1, 3)]);
        g.persisted(3); // A log of 1 > 2 / 16 folds.
        assert_eq!(g.folded_edge_count(), 3);
        assert_eq!(g.revision(), revision + 1, "a fold is not a graph change");
        // A large CSR keeps a small log until it outgrows 1/16 of it.
        let mut big = Tracker::new(64);
        for target in 1..64 {
            big.edge(0, target);
        }
        big.persisted(63);
        big.edge(1, 0);
        big.edge(1, 2);
        big.edge(1, 3);
        big.persisted(66);
        assert_eq!(big.folded_edge_count(), 63);
        big.edge(1, 4);
        big.persisted(67);
        assert_eq!(big.folded_edge_count(), 67);
        assert!(big.edge_segment(0, 67).unwrap().count() == 67);
    }
}
