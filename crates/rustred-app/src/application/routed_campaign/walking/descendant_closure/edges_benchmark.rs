//! Synthetic scale gate for the CSR edge layout (checkpoint design 3.4),
//! ignored by default and driven by the environment:
//!   RUSTRED_CLOSURE_BENCH_NODES (default 200000),
//!   RUSTRED_CLOSURE_BENCH_EDGES (default 4000000),
//!   RUSTRED_CLOSURE_BENCH_COMMITTED_PER_MILLE (default 350: the committed,
//!     sealed prefix that owns every edge; the rest is pending and unsealed,
//!     so reverse reachability blocks nearly every node, the costly case;
//!     every tenth committed node depends only on earlier ones of its own
//!     class, which therefore stays closed),
//!   RUSTRED_CLOSURE_BENCH_RECEIPT (optional path for the JSON receipt).
//! Reports bytes per edge after the fold, the forced refresh wall time against
//! the former linked-list layout on the same graph, and the restore rebuild
//! (CSR from persisted pairs plus validation) wall time. Asserted: equal
//! closure flags of both layouts and an order-independent fingerprint of the
//! edge multiset after the fold and after the restore rebuild; the numbers
//! are measurements, not gates in CI.
use super::*;
use std::time::Instant;

/// The former layout (16 B nodes, 24 B edges linked by target) and its scan,
/// kept only to time the same graph; not used by the walk.
mod linked_list {
    const NONE: usize = usize::MAX;
    #[allow(dead_code)] // `inspected` kept for layout parity with the former tracker.
    struct Node {
        sealed: bool,
        inspected: bool,
        closed: bool,
        incoming: usize,
    }
    #[allow(dead_code)] // `target` kept for layout parity with the former tracker.
    struct Edge {
        source: usize,
        target: usize,
        next: usize,
    }
    pub(super) struct Graph {
        nodes: Vec<Node>,
        edges: Vec<Edge>,
    }
    impl Graph {
        pub fn new(sealed: impl Iterator<Item = bool>, edges: usize) -> Self {
            Self {
                nodes: sealed
                    .map(|sealed| Node {
                        sealed,
                        inspected: sealed,
                        closed: false,
                        incoming: NONE,
                    })
                    .collect(),
                edges: Vec::with_capacity(edges),
            }
        }
        pub fn push(&mut self, source: usize, target: usize) {
            let index = self.edges.len();
            self.edges.push(Edge {
                source,
                target,
                next: self.nodes[target].incoming,
            });
            self.nodes[target].incoming = index;
        }
        pub fn storage_bytes(&self) -> usize {
            self.nodes.capacity() * size_of::<Node>() + self.edges.capacity() * size_of::<Edge>()
        }
        pub fn closed(&self) -> impl Iterator<Item = bool> + '_ {
            self.nodes.iter().map(|node| node.closed)
        }
        /// The pre-CSR `Tracker::refresh` scan, statement for statement.
        pub fn refresh(&mut self, cancellation: &std::sync::atomic::AtomicBool) -> usize {
            use std::sync::atomic::Ordering;
            let mut blocked = Vec::with_capacity(self.nodes.len());
            let mut stack = Vec::with_capacity(self.nodes.len());
            for (id, node) in self.nodes.iter().enumerate() {
                if id % 1024 == 0 && cancellation.load(Ordering::Relaxed) {
                    return 0;
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
                        return 0;
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
            for (id, node) in self.nodes.iter_mut().enumerate() {
                node.closed = !blocked[id];
                total += usize::from(node.closed);
            }
            total
        }
    }
}

fn env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .map_or(default, |value| value.parse().expect(name))
}

fn status_bytes(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(key))?;
    line.split_ascii_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

/// Order-independent fingerprint of an edge multiset: wrapping sum and xor
/// of a SplitMix64 finalizer over each (source, target) pair.
#[derive(Debug, Default, PartialEq)]
struct Fingerprint(u64, u64);
impl Fingerprint {
    fn add(&mut self, source: usize, target: usize) {
        let mut z = ((source as u64) << 32 | target as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        self.0 = self.0.wrapping_add(z);
        self.1 ^= z;
    }
    fn of(graph: &Tracker) -> Self {
        let mut fingerprint = Self::default();
        graph.for_each_edge(|source, target| fingerprint.add(source, target));
        fingerprint
    }
}

/// Best of three forced scans of an unchanged graph.
fn timed_refresh(graph: &mut Tracker) -> f64 {
    (0..3)
        .map(|_| {
            graph.snapshot_revision = graph.revision.wrapping_sub(1);
            let started = Instant::now();
            graph.refresh(&AtomicBool::new(false), true);
            started.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
#[ignore = "synthetic scale measurement; set RUSTRED_CLOSURE_BENCH_* and run with --ignored"]
fn csr_edges_scale_benchmark() {
    let nodes = env("RUSTRED_CLOSURE_BENCH_NODES", 200_000);
    let target_edges = env("RUSTRED_CLOSURE_BENCH_EDGES", 4_000_000);
    let committed = (nodes * env("RUSTRED_CLOSURE_BENCH_COMMITTED_PER_MILLE", 350) / 1000).max(1);
    let initial = nodes / 10;
    let rss_start = status_bytes("VmRSS:");
    // Committed sources in order, each sealed after its edges were accepted,
    // exactly as the walk records them; targets uniform over all nodes, or
    // over the earlier members of the closed class.
    let started = Instant::now();
    let mut graph = Tracker::new(initial);
    graph.discovered(nodes);
    let mut pairs: Vec<(u32, u32)> = Vec::with_capacity(target_edges);
    let mut rng = 0x5eed_u64;
    let mut draw = || {
        rng = rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    for source in 0..committed {
        let degree = target_edges / committed + usize::from(source < target_edges % committed);
        for _ in 0..degree {
            let target = match (source % 10, source / 10) {
                (0, 0) => break,
                (0, class) => (draw() % class as u64) as usize * 10,
                _ => (draw() % nodes as u64) as usize,
            };
            let before = graph.edge_count();
            graph.edge(source, target);
            if graph.edge_count() > before {
                pairs.push((source as u32, target as u32));
            }
        }
        graph.finish(source, true, true);
    }
    assert!(graph.unavailable.is_none());
    let edges = graph.edge_count();
    let mut persisted = Fingerprint::default();
    for &(source, target) in &pairs {
        persisted.add(source as usize, target as usize);
    }
    let build_seconds = started.elapsed().as_secs_f64();
    let log_bytes = graph.storage_estimate_bytes();
    let refresh_log_seconds = timed_refresh(&mut graph);
    let started = Instant::now();
    graph.persisted(edges);
    let fold_seconds = started.elapsed().as_secs_f64();
    assert_eq!(graph.folded_edge_count(), edges);
    assert_eq!(Fingerprint::of(&graph), persisted);
    let edge_bytes = graph.edges.storage_bytes();
    let tracker_bytes = graph.storage_estimate_bytes();
    let rss_folded = status_bytes("VmRSS:");
    let refresh_csr_seconds = timed_refresh(&mut graph);
    let closed = graph.total_closed;
    assert!(
        closed >= committed / 10,
        "the closed class is closed: {closed}"
    );
    // Restore rebuild: CSR from the persisted pairs plus full validation.
    let (counters, flags) = (graph.counters(), graph.node_flags().collect::<Vec<_>>());
    let started = Instant::now();
    let mut restored = Tracker::from_parts(counters, &flags, &pairs).unwrap();
    let restore_csr_seconds = started.elapsed().as_secs_f64();
    restored.restore(nodes, initial).unwrap();
    let restore_rebuild_seconds = started.elapsed().as_secs_f64();
    assert_eq!(restored.edge_count(), edges);
    assert_eq!(Fingerprint::of(&restored), persisted);
    drop(restored);
    let sealed: Vec<bool> = flags.iter().map(|flag| flag & FLAG_SEALED != 0).collect();
    drop(graph);
    let mut former = linked_list::Graph::new(sealed.into_iter(), pairs.len());
    for &(source, target) in &pairs {
        former.push(source as usize, target as usize);
    }
    let former_bytes = former.storage_bytes();
    let refresh_linked_list_seconds = (0..3)
        .map(|_| {
            let started = Instant::now();
            assert_eq!(former.refresh(&AtomicBool::new(false)), closed);
            started.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min);
    assert!(
        former
            .closed()
            .zip(&flags)
            .all(|(closed, flag)| closed == (flag & FLAG_CLOSED != 0))
    );
    let receipt = json!({"nodes":nodes,"edges":edges,"committed_sources":committed,"initial":initial,
        "total_closed":closed,"build_seconds":build_seconds,
        "log_storage_bytes_before_fold":log_bytes,"fold_seconds":fold_seconds,
        "edge_storage_bytes_after_fold":edge_bytes,"edge_bytes_per_edge_after_fold":edge_bytes as f64 / edges as f64,
        "tracker_storage_bytes_after_fold":tracker_bytes,"tracker_bytes_per_edge_after_fold":tracker_bytes as f64 / edges as f64,
        "former_linked_list_storage_bytes":former_bytes,
        "refresh_seconds_unfolded_log":refresh_log_seconds,"refresh_seconds_csr":refresh_csr_seconds,
        "refresh_seconds_former_linked_list":refresh_linked_list_seconds,
        "refresh_csr_over_linked_list":refresh_csr_seconds / refresh_linked_list_seconds,
        "restore_rebuild_seconds":restore_rebuild_seconds,"restore_csr_build_seconds":restore_csr_seconds,
        "restore_validation_seconds":restore_rebuild_seconds - restore_csr_seconds,
        "rss_bytes_at_start":rss_start,"rss_bytes_after_fold":rss_folded,"peak_rss_bytes":status_bytes("VmHWM:"),
        "targets":{"edge_bytes_per_edge":5.0,"refresh_ratio":1.0 / 3.0,"restore_rebuild_seconds":10.0},
        "scope":"synthetic closure graph only; measurements, not a campaign claim","family_closure_claim":false});
    eprintln!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    if let Ok(path) = std::env::var("RUSTRED_CLOSURE_BENCH_RECEIPT") {
        std::fs::write(path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }
}
