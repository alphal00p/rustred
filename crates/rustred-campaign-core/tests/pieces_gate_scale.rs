//! Lane pieces: the ship gate on 10^6 synthetic edges within memory (its own test binary, so that the process peak
//! is the gate's). Keys of arity 20 (a test instance, not a constant of the code).

use std::path::PathBuf;
use std::time::Instant;

use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::pieces::{GateConfig, GateReport, GateVerdict, KeyOrder, NonZero, ShipGate};

const ARITY: usize = 20;
const KEYS: usize = 200_000;
const EDGES: usize = 1_000_000;

fn spill(name: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(format!("pieces-scale-{}-{name}", std::process::id()))
}

fn status_kb(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self, m: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % m
    }
}

fn pool(r: &mut Rng) -> Vec<i32> {
    // flat pool of KEYS keys: 10 line slots in 1..=3, 10 numerator slots in -2..=0
    let mut v = Vec::with_capacity(KEYS * ARITY);
    for _ in 0..KEYS {
        for s in 0..ARITY {
            v.push(if s < ARITY / 2 {
                1 + r.next(3) as i32
            } else {
                -(r.next(3) as i32)
            });
        }
    }
    v
}

/// Pushes EDGES descending edges (each random pair oriented by the comparator, equal pairs skipped); `plant` adds one
/// reversed (non-descending) copy of the first edge.
fn run(c: &NodeComparator, keys: &[i32], chunk: usize, tag: &str, plant: bool) -> GateReport {
    let mut cfg = if chunk == 0 { GateConfig::memory() } else { GateConfig::disk(spill(tag)) };
    cfg.chunk_bytes = chunk;
    let mut g = ShipGate::new(c, ARITY, cfg).unwrap();
    let mut r = Rng(99);
    let k = |i: usize| &keys[i * ARITY..(i + 1) * ARITY];
    let mut n = 0;
    let mut first: Option<(usize, usize)> = None;
    while n < EDGES {
        let (a, b) = (r.next(KEYS as u64) as usize, r.next(KEYS as u64) as usize);
        let (hi, lo) = match c.cmp_keys(k(a), k(b)) {
            std::cmp::Ordering::Greater => (a, b),
            std::cmp::Ordering::Less => (b, a),
            std::cmp::Ordering::Equal => continue,
        };
        g.push_edge(k(hi), "synthetic", k(lo), NonZero::Unknown).unwrap();
        first.get_or_insert((hi, lo));
        n += 1;
    }
    if plant {
        let (hi, lo) = first.unwrap();
        g.push_edge(k(lo), "planted", k(hi), NonZero::Yes).unwrap();
    }
    g.finish().unwrap()
}

fn summary(r: &GateReport, secs: f64) -> serde_json::Value {
    serde_json::json!({"edges": r.edges, "keys": r.keys, "verdict": r.verdict, "seconds": secs,
        "edges_per_second": r.edges_per_second, "intake_s": r.seconds_intake, "graph_s": r.seconds_graph,
        "gate_memory_bytes": r.memory_bytes, "csr_passes": r.graph.csr_passes, "method": r.graph.method})
}

#[test]
fn a_million_edges_within_memory_and_a_planted_cycle() {
    let c = NodeComparator::legacy(ARITY, None).unwrap();
    let mut r = Rng(7);
    let keys = pool(&mut r);
    let rss0 = status_kb("VmRSS:");
    // in memory (the default mode)
    let t = Instant::now();
    let a = run(&c, &keys, 0, "a", false);
    let sa = summary(&a, t.elapsed().as_secs_f64());
    // on disk, one CSR pass and many small ones
    let t = Instant::now();
    let d = run(&c, &keys, 256 << 20, "d", false);
    let sd = summary(&d, t.elapsed().as_secs_f64());
    let b = run(&c, &keys, 1 << 20, "b", false);
    let hwm = status_kb("VmHWM:");
    println!(
        "{}",
        serde_json::json!({"memory": sa, "disk": sd, "rss_before_kb": rss0, "process_peak_kb": hwm})
    );
    assert_eq!(a.verdict, GateVerdict::Pass);
    assert_eq!(a.edges, EDGES as u64);
    assert_eq!(a.graph.kahn_processed, a.keys);
    assert!(hwm < 2 * 1024 * 1024, "process peak {hwm} kB");
    assert!(a.memory_bytes < 64 << 20, "gate memory {} bytes", a.memory_bytes);
    assert!(d.memory_bytes < a.memory_bytes);
    assert!(b.graph.csr_passes > 1);
    for x in [&d, &b] {
        assert_eq!(
            (x.verdict.clone(), x.edges, x.keys, x.graph.kahn_processed),
            (a.verdict.clone(), a.edges, a.keys, a.graph.kahn_processed)
        );
    }
    // one planted non-descending edge closing a cycle: found by both checks, the witness goes through it; both modes
    for chunk in [0, 4 << 20] {
        let p = run(&c, &keys, chunk, "p", true);
        assert_eq!(
            p.verdict,
            GateVerdict::Fail {
                non_descending: 1,
                cyclic: true
            }
        );
        assert_eq!(p.non_descending_listed[0].provenance, "planted");
        assert!(p.graph.witness_cycle.iter().any(|e| e.provenance == "planted"));
        assert!(p.graph.witness_cycle.len() >= 2);
        assert_eq!(p.graph.sccs.cyclic, 1);
        println!(
            "{}",
            serde_json::json!({"planted": {"chunk": chunk, "residual": p.graph.residual, "scc": p.graph.sccs,
                "witness_len": p.graph.witness_cycle.len()}})
        );
    }
}
