//! Independent small-graph reference checks for observational coverage only.
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
    let mut restored: Tracker =
        serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
    restored.restore(3, 2).unwrap();
    assert_eq!(restored.json(3, 2)["available"], false);
}

#[test]
fn checkpoint_rejects_corrupt_links_closed_frontier_and_counts() {
    let mut graph = Tracker::new(1);
    graph.discovered(3);
    graph.edge(0, 1);
    graph.edge(1, 2);
    graph.finish(0, true, true);
    graph.refresh(&AtomicBool::new(false), true);
    let image = serde_json::to_value(&graph).unwrap();
    for case in 0..6 {
        let mut changed = image.clone();
        match case {
            0 => changed["edges"][0]["source"] = json!(3),
            1 => changed["edges"][0]["next"] = json!(0),
            2 => changed["nodes"][1]["incoming"] = json!(1),
            3 => changed["nodes"][1]["closed"] = json!(true),
            4 => changed["initial_closed"] = json!(1),
            _ => changed["snapshot_revision"] = json!(u64::MAX),
        }
        let mut restored: Tracker = serde_json::from_value(changed).unwrap();
        assert!(restored.restore(3, 1).is_err(), "corruption={case}");
    }
    let mut restored: Tracker = serde_json::from_value(image).unwrap();
    restored.restore(3, 1).unwrap();
    restored.restore(3, 1).unwrap(); // Revalidation must rebuild, not duplicate edges.
    restored.edge(1, 2);
    assert_eq!(restored.edges.len(), 2);
    restored.finish(1, true, true);
    restored.finish(2, true, true);
    restored.refresh(&AtomicBool::new(false), true);
    assert_eq!(restored.initial_closed, 1);
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
