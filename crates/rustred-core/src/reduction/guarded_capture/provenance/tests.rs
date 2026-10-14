use super::*;
use crate::algebra::CoefficientContext;

#[test]
fn diamond_guards_are_shared_exact_and_do_not_leak_between_siblings() {
    let context = CoefficientContext::new(["d"]);
    let values = ["d-1", "d-2", "d-3", "d-4"].map(|s| context.coefficient_fixture(s));
    let shared = Arc::new(SharedCacheBudget::default());
    let mut graph = GuardProvenance::new(Arc::clone(&shared), Default::default(), None);
    let root = graph.allocate_node().unwrap();
    let left = graph.allocate_node().unwrap();
    let right = graph.allocate_node().unwrap();
    let shared_leaf = graph.allocate_node().unwrap();
    for (node, value) in [
        (root, &values[0]),
        (left, &values[1]),
        (right, &values[2]),
        (shared_leaf, &values[3]),
    ] {
        graph.add_local(node, std::slice::from_ref(value)).unwrap();
    }
    graph.add_child(root, left).unwrap();
    graph.add_child(root, right).unwrap();
    graph.add_child(left, shared_leaf).unwrap();
    graph.add_child(right, shared_leaf).unwrap();
    assert!(graph.finish(root).is_err());
    graph.finish(shared_leaf).unwrap();
    graph.finish(left).unwrap();
    graph.finish(right).unwrap();
    graph.finish(root).unwrap();
    assert!(graph.prepare_export(root).unwrap());
    assert_eq!(
        graph.conditions(left).unwrap(),
        vec![values[1].clone(), values[3].clone()]
    );
    assert_eq!(
        graph.conditions(right).unwrap(),
        vec![values[2].clone(), values[3].clone()]
    );
    assert_eq!(
        graph.conditions(root).unwrap(),
        vec![
            values[0].clone(),
            values[1].clone(),
            values[3].clone(),
            values[2].clone()
        ]
    );
    assert_eq!(graph.statistics().unique_guards, 4);
    assert_eq!(graph.statistics().edges, 4);
    assert!(graph.add_local(left, &[values[0].clone()]).is_err());
    assert!(graph.add_child(root, root).is_err());
    assert!(shared.snapshot().coefficient_bytes > 0);
    drop(graph);
    assert_eq!(shared.snapshot(), Default::default());
}

#[test]
fn program_identity_qualifies_equal_integral_keys() {
    let mut graph = GuardProvenance::new(Arc::default(), Default::default(), None);
    let key = IntegralKey::try_new([1, 1]).unwrap();
    let a = graph.node_for(10, &key).unwrap();
    let b = graph.node_for(11, &key).unwrap();
    assert_ne!(a, b);
    assert_eq!(a, graph.node_for(10, &key).unwrap());
}

#[test]
fn soft_metadata_exhaustion_retains_all_root_guards_but_never_exports_incomplete_nodes() {
    let context = CoefficientContext::new(["d"]);
    let guard = context.coefficient_fixture("d-4");
    let later = context.coefficient_fixture("d-5");
    let shared = Arc::new(SharedCacheBudget::default());
    let mut graph = GuardProvenance::new(
        Arc::clone(&shared),
        Default::default(),
        Some(4 * std::mem::size_of::<Node>() + 32),
    );
    let root = graph.allocate_node().unwrap();
    graph.add_local(root, std::slice::from_ref(&guard)).unwrap();
    assert!(graph.is_active());
    for _ in 0..3 {
        graph.allocate_node().unwrap();
    }
    let child = graph.allocate_node().unwrap();
    assert!(!graph.is_active());
    graph
        .add_local(child, std::slice::from_ref(&later))
        .unwrap();
    graph.add_child(root, child).unwrap();
    graph.finish(root).unwrap();
    assert!(graph.conditions(root).is_err());
    assert_eq!(graph.all_conditions(), vec![guard, later]);
    assert!(graph.statistics().metadata_exhausted);
    assert_eq!(graph.statistics().metadata_bytes, 0);
    drop(graph);
    assert_eq!(shared.snapshot(), Default::default());
}
