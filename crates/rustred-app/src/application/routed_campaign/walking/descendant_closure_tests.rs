//! Independent small-graph reference checks for observational coverage only.
use super::super::checkpoint::{
    SaveKind,
    manifest::Section,
    test_support::{Fixture, OWNER},
};
use super::super::execution::State;
use super::super::queue::{Domain, Phase, Queue};
use super::*;

fn reference_closed(start: usize, edges: &[Vec<usize>], sealed: &[bool]) -> bool {
    let mut seen = vec![false; sealed.len()];
    let mut pending = vec![start];
    while let Some(id) = pending.pop() {
        if seen[id] {
            continue;
        }
        seen[id] = true;
        if !sealed[id] {
            return false;
        }
        pending.extend(edges[id].iter().copied());
    }
    true
}

#[test]
fn reference_reachability_matches_every_root_through_incremental_publication() {
    // Deterministic many-shape graph corpus: no RNG/dependency and no topology.
    for seed in 0..80usize {
        let count = 1 + seed % 23;
        let initial = 1 + seed % count;
        let mut graph = Tracker::new(initial);
        graph.discovered(count);
        let mut edges = vec![Vec::new(); count];
        let mut sealed = vec![false; count];
        for step in 0..count {
            // Permutations are not needed: vary edge fanout, cycles and sharing.
            let source = step;
            for target in 0..count {
                if (source * 31 + target * 17 + seed * 7) % 11 < 3 {
                    graph.edge(source, target);
                    graph.edge(source, target);
                    edges[source].push(target);
                }
            }
            sealed[source] = (source + seed) % 7 != 0;
            graph.finish(source, source % 3 != 0, sealed[source]);
            graph.refresh(&AtomicBool::new(false), true);
            for root in 0..count {
                assert_eq!(
                    graph.closed(root),
                    Some(reference_closed(root, &edges, &sealed)),
                    "seed={seed}, step={step}, root={root}"
                );
            }
            assert_eq!(
                graph.total_closed,
                (0..count)
                    .filter(|&id| reference_closed(id, &edges, &sealed))
                    .count()
            );
            assert_eq!(
                graph.initial_closed,
                (0..initial)
                    .filter(|&id| reference_closed(id, &edges, &sealed))
                    .count()
            );
        }
    }
}

#[test]
fn stale_snapshot_is_a_lower_bound_after_discovery_and_shared_edge_changes() {
    let mut graph = Tracker::new(2);
    graph.finish(0, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    assert_eq!(graph.initial_closed, 1);
    graph.discovered(4);
    graph.edge(1, 3);
    graph.edge(2, 3);
    graph.finish(1, true, true);
    graph.finish(2, true, true);
    let stale = graph.json(4, 2);
    assert_eq!(stale["snapshot_stale"], true);
    assert_eq!(stale["total_closed"], 1);
    assert_eq!(stale["unresolved_domains"], 3);
    graph.refresh(&AtomicBool::new(false), false); // Periodic throttle preserves bound.
    assert_eq!(graph.total_closed, 1);
    graph.finish(3, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    assert_eq!(graph.initial_closed, 2);
    assert_eq!(graph.total_closed, 4);
    assert_eq!(graph.json(4, 2)["snapshot_stale"], false);
}

#[test]
fn refresh_interval_bounds_duty_to_one_percent_and_force_bypasses_it() {
    let mut graph = Tracker::new(2);
    assert_eq!(graph.refresh_interval(), Duration::from_secs(5));
    assert_eq!(
        graph.refresh_policy_json()["next_refresh_seconds"],
        Value::Null
    );
    assert_eq!(graph.refresh_policy_json()["duty_bound"], 0.01);
    assert_eq!(graph.refresh_policy_json()["min_interval_seconds"], 5.0);
    // The closure report itself keeps its historical key set.
    assert!(graph.json(2, 2).get("refresh_duty_bound").is_none());
    assert!(graph.json(2, 2).get("next_refresh_seconds").is_none());
    graph.finish(0, true, true);
    graph.refresh(&AtomicBool::new(false), false); // First scan is never throttled.
    assert_eq!(graph.refresh_count, 1);
    // A costly scan spaces the next periodic scan to 100x its wall time.
    graph.last_refresh_seconds = 0.5;
    assert_eq!(graph.refresh_interval(), Duration::from_secs(50));
    let next = graph.refresh_policy_json()["next_refresh_seconds"]
        .as_f64()
        .unwrap();
    assert!(next > 49.0 && next <= 50.0, "{next}");
    graph.finish(1, true, true);
    assert_eq!(graph.json(2, 2)["snapshot_stale"], true);
    graph.refresh(&AtomicBool::new(false), false);
    assert_eq!(graph.refresh_count, 1, "periodic refresh is throttled");
    assert_eq!(graph.total_closed, 1);
    // Cheap scans still keep the five-second floor.
    graph.last_refresh_seconds = 0.001;
    assert_eq!(graph.refresh_interval(), Duration::from_secs(5));
    graph.refresh(&AtomicBool::new(false), false);
    assert_eq!(graph.refresh_count, 1);
    // An elapsed interval admits the periodic scan again.
    graph.last_refresh = Some(Instant::now() - Duration::from_secs(6));
    assert_eq!(graph.refresh_policy_json()["next_refresh_seconds"], 0.0);
    graph.refresh(&AtomicBool::new(false), false);
    assert_eq!(graph.refresh_count, 2);
    assert_eq!(graph.total_closed, 2);
    // Force bypasses any throttle, but never a cancellation.
    graph.discovered(3);
    graph.finish(2, true, true);
    graph.last_refresh_seconds = 10.0;
    graph.refresh(&AtomicBool::new(false), false);
    assert_eq!(graph.refresh_count, 2);
    graph.refresh(&AtomicBool::new(true), true);
    assert_eq!(graph.refresh_count, 2);
    graph.refresh(&AtomicBool::new(false), true);
    assert_eq!(graph.refresh_count, 3);
    assert_eq!(graph.total_closed, 3);
}

#[test]
fn allocation_or_bad_edges_disable_monitoring_without_fake_completion() {
    let mut graph = Tracker::new(2);
    graph.edge(0, 2);
    let report = graph.json(2, 2);
    assert_eq!(report["available"], false);
    assert_eq!(report["total_closed"], Value::Null);
    assert_eq!(report["initial_closed"], Value::Null);
    graph.discovered(3);
    graph.finish(0, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    assert_eq!(graph.json(3, 2)["available"], false);
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    let mut restored = Tracker::from_parts(graph.counters(), &[], &[]).unwrap();
    restored.restore(3, 2).unwrap();
    assert_eq!(restored.json(3, 2)["available"], false);
}

/// Checkpoint-section image of a tracker: flag bytes, edge pairs, counters.
fn image(graph: &Tracker) -> (Vec<u8>, Vec<(u32, u32)>, Value) {
    let edges = graph.edge_segment(0, graph.edge_count()).unwrap().collect();
    let counters = serde_json::to_value(graph.counters()).unwrap();
    (graph.node_flags().collect(), edges, counters)
}

#[test]
fn scan_history_records_completed_scans_not_heartbeats_and_survives_restore() {
    let mut graph = Tracker::new(2);
    assert_eq!(graph.refresh_policy_json()["status"], "eligible");
    assert_eq!(
        graph.scan_history_json(),
        json!({"previous":null,"latest":null})
    );
    graph.finish(0, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    let first = graph.scan_history_json()["latest"].clone();
    assert_eq!(first["total_domains"], 2);
    assert_eq!(first["total_closed"], 1);
    assert_eq!(first["refresh_count"], 1);
    assert!(first["completed_unix_seconds"].as_f64().unwrap() > 0.0);
    assert_eq!(graph.refresh_policy_json()["status"], "unchanged");

    graph.discovered(3);
    assert_eq!(graph.refresh_policy_json()["status"], "throttled");
    graph.refresh(&AtomicBool::new(false), false);
    graph.refresh(&AtomicBool::new(true), true);
    assert_eq!(graph.scan_history_json()["latest"], first);
    assert_eq!(graph.scan_history_json()["previous"], Value::Null);
    graph.finish(1, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    let history = graph.scan_history_json();
    assert_eq!(history["previous"], first);
    assert_eq!(history["latest"]["total_domains"], 3);
    assert_eq!(history["latest"]["total_closed"], 2);
    assert_eq!(history["latest"]["refresh_count"], 2);

    let (flags, edges, counters) = image(&graph);
    let mut restored = rebuild(&flags, &edges, &counters).unwrap();
    restored.restore(3, 2).unwrap();
    assert_eq!(restored.scan_history_json(), history);
    assert_eq!(restored.refresh_policy_json()["status"], "unchanged");
    restored.finish(2, true, true);
    assert_eq!(restored.refresh_policy_json()["status"], "eligible");
    restored.refresh(&AtomicBool::new(false), true);
    assert_eq!(restored.scan_history_json()["previous"], history["latest"]);
    assert_eq!(restored.scan_history_json()["latest"]["total_closed"], 3);

    // Old checkpoints remain readable, but cannot invent scan timestamps.
    let mut old_counters = counters;
    old_counters.as_object_mut().unwrap().remove("scan_history");
    let mut old = rebuild(&flags, &edges, &old_counters).unwrap();
    old.restore(3, 2).unwrap();
    assert_eq!(
        old.scan_history_json(),
        json!({"previous":null,"latest":null})
    );
    old.disable("test unavailable");
    assert_eq!(old.refresh_policy_json()["status"], "unavailable");
    assert_eq!(
        old.refresh_policy_json()["earliest_refresh_unix_seconds"],
        Value::Null
    );
}

fn rebuild(flags: &[u8], edges: &[(u32, u32)], counters: &Value) -> Result<Tracker, String> {
    let counters: Counters = serde_json::from_value(counters.clone()).unwrap();
    Tracker::from_parts(counters, flags, edges)
}

#[test]
fn checkpoint_rejects_bad_endpoints_duplicates_closed_frontier_and_counts() {
    let mut graph = Tracker::new(1);
    graph.discovered(4);
    graph.edge(0, 1);
    graph.edge(1, 2);
    graph.edge(3, 2);
    graph.finish(0, true, true);
    graph.finish(2, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    let (flags, edges, counters) = image(&graph);
    assert_eq!(flags, [3, 0, 7, 0]);
    for case in 0..8 {
        let (mut flags, mut edges, mut counters) = (flags.clone(), edges.clone(), counters.clone());
        let expected = match case {
            0 => {
                edges[0].0 = 4;
                "invalid checkpoint dependency edge"
            }
            1 => {
                edges.push((3, 2)); // Node 3 is unsealed: its edges are deduplicated.
                "duplicate checkpoint dependency edge"
            }
            2 => {
                flags[1] |= FLAG_CLOSED; // Closed but unsealed.
                "closure counters"
            }
            3 => {
                flags[0] |= FLAG_CLOSED; // Sealed and closed, yet 0 -> 1 is open.
                counters["total_closed"] = json!(2);
                counters["initial_closed"] = json!(1);
                "closure counters"
            }
            4 => {
                counters["total_closed"] = json!(2);
                "closure counters"
            }
            5 => {
                counters["initial_closed"] = json!(1);
                "closure counters"
            }
            6 => {
                counters["inspected"] = json!(1);
                "closure counters"
            }
            _ => {
                counters["snapshot_revision"] = json!(u64::MAX);
                "dependency inventory"
            }
        };
        let error = rebuild(&flags, &edges, &counters)
            .and_then(|mut restored| restored.restore(4, 1))
            .unwrap_err();
        assert!(error.contains(expected), "corruption={case}: {error}");
    }
    let mut restored = rebuild(&flags, &edges, &counters).unwrap();
    restored.restore(4, 1).unwrap();
    restored.restore(4, 1).unwrap(); // Revalidation must rebuild, not duplicate edges.
    restored.edge(1, 2);
    restored.edge(3, 2);
    assert_eq!(restored.edge_count(), 3);
    restored.finish(1, true, true);
    restored.finish(3, true, true);
    restored.refresh(&AtomicBool::new(false), true);
    assert_eq!(restored.initial_closed, 1);
    assert_eq!(restored.total_closed, 4);
}

#[test]
fn zero_inventory_and_shared_failed_leaf_do_not_report_a_fake_full_bar() {
    let graph = Tracker::new(0);
    assert_eq!(graph.json(0, 0)["initial_closed"], 0);
    assert_eq!(graph.json(0, 0)["family_closure_claim"], false);
    let mut graph = Tracker::new(3);
    graph.discovered(4);
    graph.edge(0, 3);
    graph.edge(1, 3);
    for id in 0..3 {
        graph.finish(id, true, true);
    }
    graph.finish(3, true, false);
    graph.refresh(&AtomicBool::new(false), true);
    assert_eq!(graph.total_closed, 1);
    assert_eq!(graph.initial_closed, 1);
    assert_eq!(graph.closed(2), Some(true));
    assert_eq!(graph.closed(0), Some(false));
}

/// Deterministic SplitMix64: random graphs without an RNG dependency.
struct Mix(u64);
impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[test]
fn csr_refresh_matches_linked_list_reference_after_folds() {
    // Random graphs with incremental discovery (the CSR then covers fewer
    // targets than the graph), late edges from failed nodes, late seals, and
    // at random points a save-path fold, a forced fold or a restore round trip.
    let mut folds = 0;
    for seed in 0..300u64 {
        let mut rng = Mix(seed);
        let count = 1 + rng.below(48);
        let initial = 1 + rng.below(count);
        let mut graph = Tracker::new(initial);
        let mut discovered = initial;
        let mut edges = vec![Vec::new(); count];
        let mut sealed = vec![false; count];
        for step in 0..count {
            discovered = (discovered + rng.below(3)).max(step + 1).min(count);
            graph.discovered(discovered);
            let mut sources = vec![step];
            if step > 0 {
                sources.push(rng.below(step)); // Kept only while unsealed.
            }
            for source in sources.into_iter().filter(|&source| !sealed[source]) {
                for _ in 0..rng.below(6) {
                    let target = rng.below(discovered);
                    graph.edge(source, target);
                    if !edges[source].contains(&target) {
                        edges[source].push(target);
                    }
                }
            }
            sealed[step] = rng.below(5) != 0;
            graph.finish(step, rng.below(3) != 0, sealed[step]);
            if step > 0 && rng.below(4) == 0 {
                let other = rng.below(step);
                if !sealed[other] {
                    sealed[other] = true;
                    graph.finish(other, false, true);
                }
            }
            match rng.below(4) {
                0 => graph.persisted(graph.edge_count()),
                1 => {
                    let nodes = graph.node_count();
                    graph.edges.fold(nodes).unwrap();
                }
                2 => {
                    let (flags, pairs, counters) = image(&graph);
                    graph = rebuild(&flags, &pairs, &counters).unwrap();
                    graph.restore(discovered, initial).unwrap();
                }
                _ => {}
            }
            folds += usize::from(graph.edges.log_len() == 0 && graph.edge_count() > 0);
            graph.refresh(&AtomicBool::new(false), true);
            assert!(graph.unavailable.is_none(), "seed={seed}, step={step}");
            for root in 0..discovered {
                assert_eq!(
                    graph.closed(root),
                    Some(reference_closed(root, &edges, &sealed)),
                    "seed={seed}, step={step}, root={root}"
                );
            }
            let closed = |range: std::ops::Range<usize>| {
                range
                    .filter(|&id| reference_closed(id, &edges, &sealed))
                    .count()
            };
            assert_eq!(graph.total_closed, closed(0..discovered));
            assert_eq!(graph.initial_closed, closed(0..initial));
            assert_eq!(
                graph.edge_count(),
                edges.iter().map(Vec::len).sum::<usize>()
            );
        }
    }
    assert!(folds > 1000, "folded states exercised: {folds}");
}

/// `nodes` distinct point domains, nothing published: the closure is the only
/// populated walk state, so resume validates it without records.
fn point_state(nodes: u64) -> State<1> {
    let mut queue = Queue::<1>::new(64, None);
    for i in 0..nodes {
        queue
            .admit(Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: vec![i],
                upper: vec![Some(i)],
                rank: None,
                powers: Default::default(),
            })
            .unwrap();
    }
    State::new(queue, 0, None)
}

#[test]
fn edge_segments_are_disjoint_prefix_partition_of_the_log() {
    let state = point_state(8);
    let batches: [&[(usize, usize)]; 3] = [
        &[(0, 1), (0, 2), (3, 1)],
        &[(4, 5), (0, 5)],
        &[(6, 7), (7, 6), (5, 0)],
    ];
    let add = |batch: &[(usize, usize)]| {
        for &(source, target) in batch {
            state.closure.borrow_mut().edge(source, target);
        }
    };
    add(batches[0]);
    let fixture = Fixture::save(&state);
    // The save persisted the whole log, so it was folded (the CSR was empty).
    assert_eq!(state.closure.borrow().folded_edge_count(), 3);
    for batch in &batches[1..] {
        add(batch);
        fixture.save_again(&state).unwrap().unwrap();
        assert_eq!(state.closure.borrow().edges.log_len(), 0);
    }
    let manifest = fixture.manifest();
    assert_eq!(manifest["sections"]["edges"]["total"], 8);
    let segments = manifest["sections"]["edges"]["segments"]
        .as_array()
        .unwrap();
    assert_eq!(segments.len(), 3);
    let (mut next, mut persisted) = (0, Vec::new());
    for (segment, batch) in segments.iter().zip(batches) {
        assert_eq!(segment["first"], next);
        assert_eq!(segment["count"], batch.len());
        next += batch.len();
        let bytes = std::fs::read(fixture.dir.join(segment["file"].as_str().unwrap())).unwrap();
        // 32-byte section header, then (u32 source, u32 target) little-endian.
        persisted.extend(bytes[32..].chunks_exact(8).map(|pair| {
            let word = |at: usize| u32::from_le_bytes(pair[at..at + 4].try_into().unwrap());
            (word(0) as usize, word(4) as usize)
        }));
    }
    assert_eq!(persisted, batches.concat(), "insertion order, no overlap");
    let restored = fixture.resume::<1>().unwrap();
    let mut rebuilt: Vec<_> = restored.closure.borrow().dependencies().collect();
    let mut expected = batches.concat();
    rebuilt.sort_unstable();
    expected.sort_unstable();
    assert_eq!(rebuilt, expected);
    assert_eq!(restored.closure.borrow().folded_edge_count(), 8);
}

#[test]
fn binary_closure_sections_reject_endpoint_duplicate_closed_unsealed_and_counts() {
    let state = point_state(3);
    state.closure.borrow_mut().edge(0, 1);
    state.closure.borrow_mut().edge(1, 2);
    let fixture = Fixture::save(&state);
    let fails = |expected: &str| {
        let error = fixture.resume::<1>().err().unwrap();
        assert!(error.contains(expected), "{expected}: {error}");
    };
    fixture.rewrite_section::<1>(Section::Edges, |edges| edges[0] = json!([3, 1]));
    fails("invalid checkpoint dependency edge");
    fixture.rewrite_section::<1>(Section::Edges, |edges| edges[0] = json!([0, 1]));
    fixture.rewrite_section::<1>(Section::Edges, |edges| {
        edges.as_array_mut().unwrap().push(json!([1, 2])); // Node 1 is unsealed.
    });
    fails("duplicate checkpoint dependency edge");
    fixture.rewrite_section::<1>(Section::Edges, |edges| {
        edges.as_array_mut().unwrap().pop();
    });
    fixture.rewrite_section::<1>(Section::Nodes, |flags| flags[1] = json!(FLAG_CLOSED));
    fails("dependency closure counters");
    fixture.rewrite_section::<1>(Section::Nodes, |flags| flags[1] = json!(0));
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["closure"]["total_closed"] = json!(1)
    });
    fails("dependency closure counters");
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["closure"]["total_closed"] = json!(0)
    });
    let restored = fixture.resume::<1>().unwrap();
    assert_eq!(restored.closure.borrow().edge_count(), 2);
}

#[test]
fn final_and_cancelled_saves_persist_the_log_without_folding_it() {
    let state = point_state(40);
    let add = |source: usize, targets: std::ops::Range<usize>| {
        for target in targets {
            state.closure.borrow_mut().edge(source, target);
        }
    };
    add(0, 1..40);
    let fixture = Fixture::save(&state);
    let layout = || {
        let closure = state.closure.borrow();
        (closure.folded_edge_count(), closure.edges.log_len())
    };
    assert_eq!(layout(), (39, 0));
    let save = |kind: SaveKind, cancelled: bool| {
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store
            .save_cancellable(&state, &[], &[], kind, &AtomicBool::new(cancelled), &|_| {})
            .unwrap()
            .unwrap();
    };
    add(1, 2..6); // A log of 4 > 39 / 16 would fold after a continuing save.
    save(SaveKind::Final, false);
    assert_eq!(layout(), (39, 4), "the final save persists without folding");
    add(1, 6..7);
    save(SaveKind::Forced, true);
    assert_eq!(
        layout(),
        (39, 5),
        "nor does a save while the run is cancelled"
    );
    let edges = &fixture.manifest()["sections"]["edges"];
    assert_eq!(edges["total"], 44);
    assert_eq!(edges["segments"].as_array().unwrap().len(), 3);
    let sorted = |state: &State<1>| {
        let mut edges: Vec<_> = state.closure.borrow().dependencies().collect();
        edges.sort_unstable();
        edges
    };
    assert_eq!(sorted(&fixture.resume::<1>().unwrap()), sorted(&state));
    add(1, 7..8);
    save(SaveKind::Forced, false);
    assert_eq!(layout(), (45, 0), "a save the walk continues after folds");
}

/// Rescue (`rescue.rs`): the frontier taint is reverse reachability from
/// inspected-unsealed nodes (plus extra seeds); liveness is forward
/// reachability from roots. Checked against a brute-force reference.
#[test]
fn rescue_taint_and_liveness_match_reference_reachability() {
    for seed in 0..60usize {
        let count = 2 + seed % 17;
        let mut graph = Tracker::new(1);
        graph.discovered(count);
        let mut edges = vec![Vec::new(); count];
        for source in 0..count {
            for target in 0..count {
                if (source * 7 + target * 3 + seed) % 5 == 0 && source != target {
                    graph.edge(source, target);
                    edges[source].push(target);
                }
            }
        }
        // Node kinds: 0 pending, 1 sealed native, 2 frontier-bearing native.
        let kind = |id: usize| (id * 11 + seed) % 3;
        for id in 0..count {
            match kind(id) {
                1 => graph.finish(id, true, true),
                2 => graph.finish(id, true, false),
                _ => {}
            }
        }
        let reaches = |from: usize, target: &dyn Fn(usize) -> bool| {
            let mut seen = vec![false; count];
            let mut stack = vec![from];
            while let Some(id) = stack.pop() {
                if std::mem::replace(&mut seen[id], true) {
                    continue;
                }
                if target(id) {
                    return true;
                }
                stack.extend(edges[id].iter().copied());
            }
            false
        };
        let bit = |bits: &[u64], id: usize| bits[id / 64] >> (id % 64) & 1 != 0;
        let tainted = graph.tainted().unwrap();
        let extra_seed = seed % count;
        let mut extra = vec![0u64; count.div_ceil(64)];
        extra[extra_seed / 64] |= 1 << (extra_seed % 64);
        let seeded = graph.tainted_with(&extra).unwrap();
        let roots = [0, count / 2];
        let live = graph.reachable_from(roots).unwrap();
        for id in 0..count {
            assert_eq!(
                bit(&tainted, id),
                reaches(id, &|t| kind(t) == 2),
                "seed {seed} id {id}"
            );
            assert_eq!(
                bit(&seeded, id),
                reaches(id, &|t| kind(t) == 2 || t == extra_seed),
                "seed {seed} id {id}"
            );
            assert_eq!(
                bit(&live, id),
                roots.iter().any(|&r| reaches(r, &|t| t == id)),
                "seed {seed} id {id}"
            );
        }
    }
    let mut unavailable = Tracker::new(1);
    unavailable.disable("test");
    assert!(unavailable.tainted().is_none());
    assert!(unavailable.reachable_from([0]).is_none());
}
