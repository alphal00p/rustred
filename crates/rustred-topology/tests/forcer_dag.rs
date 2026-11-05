//! Golden tests of the reduction DAG against Forcer's own 4L reduction tree (benruijl/forcer 68a30bf,
//! forcer/pathforcer.h): node by node keys and actions, child sets of the rule-less nodes, the 2L/3L sub-DAGs, the
//! fixed topologies, the eps-coloured key, Demand counts and the pinned key bytes.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::time::Instant;

use rustred_topology::{
    Action, ActionKind, ChildMode, Dag, DagEdgeKind, Fate, GraphKey, Normalised,
    P_TOPOLOGY_VERSION, PTopo, TopologyService, canonical, forcer_action, normalise,
    reduction_graph, topologies_json,
};
use support::forcer::{
    ForcerNode, GRAPH_KEYS_TSV_PATH, forcer_dag, forcer_fixed, forcer_names_by_key, forcer_tops,
    key_of_normal_form, sector_classes,
};

/// One service for the whole binary: the tests share its cache.
fn service() -> &'static TopologyService {
    static SERVICE: OnceLock<TopologyService> = OnceLock::new();
    SERVICE.get_or_init(TopologyService::new)
}

fn fixture() -> &'static (Vec<ForcerNode>, BTreeMap<GraphKey, String>) {
    static FIXTURE: OnceLock<(Vec<ForcerNode>, BTreeMap<GraphKey, String>)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let nodes = forcer_dag();
        let names = forcer_names_by_key(&nodes);
        (nodes, names)
    })
}

fn forcer_node(name: &str) -> &'static ForcerNode {
    fixture()
        .0
        .iter()
        .find(|n| n.name == name)
        .expect("fixture name")
}

fn tops(loops: u8) -> Vec<PTopo> {
    forcer_tops(loops).into_iter().map(|(_, t)| t).collect()
}

fn forcer_tree(loops: u8) -> Dag {
    service().reduction_graph(&tops(loops), ChildMode::ForcerTree)
}

/// Every nonzero integer sector class of the `loops`-loop tops (census mode `eall`, the eager Demand roots): the
/// nodes reached from the tops through contraction edges of the ForcerTree DAG (every line of an integer top is
/// integer, and contracting a line of a multi-edge gives zero). `sector_classes` computes the same set independently
/// (`sector_classes_are_the_contraction_closure`).
fn eall_roots(loops: u8) -> Vec<PTopo> {
    let dag = forcer_tree(loops);
    let mut contract: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for e in dag
        .edges
        .iter()
        .filter(|e| matches!(e.kind, DagEdgeKind::Contract(_)))
    {
        contract.entry(e.parent).or_default().push(e.child);
    }
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut work: Vec<u32> = tops(loops)
        .iter()
        .map(|t| {
            let key = canonical(&normal_form(t)).0;
            dag.find(&key).expect("top is a DAG node").index
        })
        .collect();
    while let Some(i) = work.pop() {
        if seen.insert(i) {
            work.extend(contract.get(&i).into_iter().flatten().copied());
        }
    }
    seen.into_iter()
        .map(|i| dag.nodes[i as usize].topo.clone())
        .collect()
}

fn normal_form(t: &PTopo) -> PTopo {
    match normalise(t) {
        Normalised::Topo(p, _) => p,
        Normalised::Trivial => PTopo::trivial(),
        Normalised::Zero => panic!("zero topology {t:?}"),
    }
}

/// Forcer name of every DAG node (panics on a node Forcer does not have).
fn names(dag: &Dag) -> Vec<&'static str> {
    let by_key = &fixture().1;
    dag.nodes
        .iter()
        .map(|n| {
            by_key
                .get(&n.key)
                .unwrap_or_else(|| panic!("DAG node {:?} is not a Forcer topology", n.topo))
                .as_str()
        })
        .collect()
}

fn sorted(names: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

/// (loops, action) -> count
fn action_table(dag: &Dag) -> BTreeMap<(u8, &'static str), usize> {
    let mut table = BTreeMap::new();
    for n in &dag.nodes {
        *table
            .entry((n.loops, n.action.kind().as_str()))
            .or_default() += 1;
    }
    table
}

const CUSTOM_21: [&str; 21] = [
    "haha", "no1", "no2", "no6", "bubu", "nono", "lala", "bebe", "cross", // 4L
    "nostar0", "nostar5", "nostar6", "lastar5", "bustar5", "fastar2", "fastar3", // 3L
    "t1star05", "t1star24", "t1star34", "t1star45", "t1star55", // 2L
];

const DAG_2L: [&str; 7] = ["d22", "d26", "d9", "d43", "d3", "d4", "d1"];

const DAG_3L: [&str; 43] = [
    "d121", "d60", "nostar0", "d106", "d107", "d148", "d149", "d190", "d191", "d28", "d32", "d33",
    "d37", "d50", "d54", "d55", "d77", "d78", "d79", "d80", "d81", "d87", "d92", "d118", "d137",
    "d192", "d197", "t1star05", "d11", "d13", "d14", "d15", "d17", "d22", "d26", "d9", "d30",
    "d43", "d3", "d4", "d6", "d7", "d1",
];

#[test]
fn forcer_tree_4l_equals_forcer() {
    let dag = forcer_tree(4);
    let (nodes, by_key) = fixture();
    assert_eq!(nodes.len(), 438);
    assert_eq!(
        by_key.len(),
        438,
        "the 438 Forcer topologies have distinct keys"
    );
    assert_eq!(dag.nodes.len(), 438);
    let ours: BTreeSet<&GraphKey> = dag.nodes.iter().map(|n| &n.key).collect();
    let forcer: BTreeSet<&GraphKey> = by_key.keys().collect();
    assert_eq!(
        ours, forcer,
        "ForcerTree from the 11 4L tops = Forcer's 438 topologies"
    );
    for (node, name) in dag.nodes.iter().zip(names(&dag)) {
        let f = forcer_node(name);
        assert_eq!(node.action.kind().as_str(), f.action, "action of {name}");
        assert_eq!(node.loops, f.loops, "loops of {name}");
    }
    let expected: BTreeMap<(u8, &str), usize> = [
        ((0, "trivial"), 1),
        ((1, "insertion"), 6),
        ((2, "insertion"), 20),
        ((2, "triangle"), 4),
        ((2, "custom"), 5),
        ((3, "insertion"), 89),
        ((3, "carpet"), 10),
        ((3, "triangle"), 16),
        ((3, "custom"), 7),
        ((4, "insertion"), 220),
        ((4, "carpet"), 14),
        ((4, "triangle"), 33),
        ((4, "diamond"), 4),
        ((4, "custom"), 9),
    ]
    .into_iter()
    .collect();
    assert_eq!(action_table(&dag), expected);
    let custom: Vec<&str> = dag
        .nodes
        .iter()
        .zip(names(&dag))
        .filter(|(n, _)| n.action.kind() == ActionKind::Custom)
        .map(|(_, name)| name)
        .collect();
    assert_eq!(sorted(&custom), sorted(&CUSTOM_21));
}

#[test]
fn forcer_tree_3l_is_43_nodes() {
    let dag = forcer_tree(3);
    assert_eq!(sorted(&names(&dag)), sorted(&DAG_3L));
    for (node, name) in dag.nodes.iter().zip(names(&dag)) {
        assert_eq!(
            node.action.kind().as_str(),
            forcer_node(name).action,
            "action of {name}"
        );
    }
    // the eps nodes of the 3L DAG (needed in M2): 1L d4, d6, d7; 2L d11, d13, d14, d15, d17, d30, t1star05
    let eps: Vec<&str> = dag
        .nodes
        .iter()
        .zip(names(&dag))
        .filter(|(n, _)| n.eps_lines > 0)
        .map(|(_, m)| m)
        .collect();
    assert_eq!(
        sorted(&eps),
        sorted(&[
            "d4", "d6", "d7", "d11", "d13", "d14", "d15", "d17", "d30", "t1star05"
        ])
    );
}

#[test]
fn forcer_tree_2l_is_7_nodes() {
    let dag = forcer_tree(2);
    assert_eq!(sorted(&names(&dag)), sorted(&DAG_2L));
    for (node, name) in dag.nodes.iter().zip(names(&dag)) {
        assert_eq!(
            node.action.kind().as_str(),
            forcer_node(name).action,
            "action of {name}"
        );
    }
}

#[test]
fn rule_children_equal_forcer() {
    let dag = forcer_tree(4);
    let all = names(&dag);
    let mut checked = 0;
    for (node, name) in dag.nodes.iter().zip(&all) {
        if matches!(
            node.action.kind(),
            ActionKind::Insertion | ActionKind::Carpet
        ) {
            continue;
        }
        let ours: BTreeSet<&str> = node.children.iter().map(|&c| all[c as usize]).collect();
        let forcer: BTreeSet<&str> = forcer_node(name)
            .children
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(ours, forcer, "children of {name}");
        checked += 1;
    }
    assert_eq!(checked, 79);
}

#[test]
fn dag_edge_fates_are_consistent() {
    // every edge of the 4L DAGs: each child line has a preimage, Kept iff exactly one, eps units are conserved (an
    // integrated bubble adds one unit), loops drop by one on a rule edge and stay on a contraction
    for mode in [ChildMode::ForcerTree, ChildMode::Demand] {
        let dag = match mode {
            ChildMode::ForcerTree => forcer_tree(4),
            ChildMode::Demand => service().reduction_graph(&eall_roots(4), ChildMode::Demand),
        };
        for e in &dag.edges {
            let (parent, child) = (&dag.nodes[e.parent as usize], &dag.nodes[e.child as usize]);
            assert_eq!(e.fates.len(), parent.topo.lines.len());
            let mut preimages = vec![Vec::new(); child.topo.lines.len()];
            for (i, f) in e.fates.iter().enumerate() {
                if let Some(c) = f.line() {
                    preimages[usize::from(c)].push(i);
                }
            }
            for (c, pre) in preimages.iter().enumerate() {
                assert!(!pre.is_empty(), "child line {c} has no parent line");
                for &i in pre {
                    let expected = if pre.len() == 1 {
                        Fate::Kept(c as u8)
                    } else {
                        Fate::SeriesMergedInto(c as u8)
                    };
                    assert_eq!(e.fates[i], expected);
                }
                let eps: u8 = pre.iter().map(|&i| parent.topo.lines[i].2).sum();
                let bubble = matches!((&parent.action, e.kind), (Action::Insertion(ins), DagEdgeKind::Rule)
                    if pre.contains(&usize::from(ins.keep)));
                assert_eq!(
                    child.topo.lines[c].2,
                    eps + u8::from(bubble),
                    "eps units of child line {c}"
                );
            }
            match (e.kind, &parent.action) {
                (DagEdgeKind::Contract(l), _) => {
                    assert_eq!(e.fates[usize::from(l)], Fate::Contracted);
                    assert_eq!(
                        parent.topo.lines[usize::from(l)].2,
                        0,
                        "only integer lines are contracted"
                    );
                    assert!(child.loops == parent.loops || child.loops == 0);
                }
                (DagEdgeKind::Rule, Action::Insertion(ins)) => {
                    assert_eq!(
                        e.fates[usize::from(ins.keep)],
                        e.fates[usize::from(ins.drop)]
                    );
                    assert_eq!(child.loops + 1, parent.loops);
                }
                (DagEdgeKind::Rule, Action::Carpet(c)) => {
                    assert_eq!(e.fates[usize::from(c.r)], Fate::AbsorbedIntoQ);
                    if let Some(third) = c.third {
                        assert_eq!(e.fates[usize::from(third)], Fate::AbsorbedIntoQ);
                    }
                    assert_eq!(child.loops + 1, parent.loops);
                }
                (DagEdgeKind::Rule, other) => panic!("rule edge from a {other:?} node"),
            }
        }
    }
}

#[test]
fn line_orbits_match_brute_force() {
    // two lines are in one orbit iff marking either of them (a unique eps colour) gives the same key; every generator
    // keeps eps units and the line incidence structure
    for node in forcer_tree(4).nodes {
        let (_, canon) = canonical(&node.topo);
        assert_eq!(
            canon.apply(&node.topo),
            node.topo,
            "a node topology is canonically labelled"
        );
        let marked: Vec<GraphKey> = (0..node.topo.lines.len())
            .map(|c| {
                let mut t = node.topo.clone();
                t.lines[c].2 = 200;
                canonical(&t).0
            })
            .collect();
        for c in 0..marked.len() {
            for d in 0..marked.len() {
                let same_orbit = canon.line_orbits[c] == canon.line_orbits[d];
                assert_eq!(
                    same_orbit,
                    marked[c] == marked[d],
                    "lines {c}, {d} of {:?}",
                    node.topo
                );
            }
        }
        let l = &node.topo.lines;
        let meet = |a: usize, b: usize| {
            l[a].0 == l[b].0 || l[a].0 == l[b].1 || l[a].1 == l[b].0 || l[a].1 == l[b].1
        };
        for g in &canon.aut_gens {
            for (a, &ga) in g.iter().enumerate() {
                let ga = usize::from(ga);
                assert_eq!(l[a].2, l[ga].2);
                for (b, &gb) in g.iter().enumerate() {
                    assert_eq!(meet(a, b), meet(ga, usize::from(gb)));
                }
            }
        }
    }
}

#[test]
fn fixed_topologies_are_custom() {
    let fixed = forcer_fixed();
    assert_eq!(fixed.len(), 18);
    let by_key = &fixture().1;
    for (name, loops, topo) in &fixed {
        assert_eq!(forcer_action(topo).kind(), ActionKind::Custom, "{name}");
        let key = key_of_normal_form(topo);
        assert_eq!(
            by_key.get(&key),
            Some(name),
            "{name} is the Forcer node of that name"
        );
        assert_eq!(topo.loops(), *loops);
    }
}

#[test]
fn eps_on_q_is_not_in_the_key() {
    let (nodes, by_key) = fixture();
    // the eps power of q never distinguishes two Forcer topologies: 438 distinct keys without it, 438 with it
    let with_q: BTreeSet<(GraphKey, u8)> = nodes
        .iter()
        .map(|n| (key_of_normal_form(&n.topology.topo), n.topology.eps_on_q))
        .collect();
    assert_eq!(by_key.len(), 438);
    assert_eq!(with_q.len(), 438);
    // d1 carries four eps units on q, d3 three: neither shows up in the key
    let d1 = forcer_node("d1");
    assert_eq!(d1.topology.eps_on_q, 4);
    assert_eq!(
        key_of_normal_form(&d1.topology.topo),
        canonical(&PTopo::trivial()).0
    );
    let d3 = forcer_node("d3");
    assert_eq!(d3.topology.eps_on_q, 3);
    let bubble = PTopo::new(2, 0, 1, [(0, 1, 0), (0, 1, 0)]);
    assert_eq!(key_of_normal_form(&d3.topology.topo), canonical(&bubble).0);
}

#[test]
fn eps_colours_are_in_the_key() {
    // the 2L kite family: integer (d43) and the five eps variants Forcer treats as fixed topologies
    let names = [
        "d43", "t1star05", "t1star24", "t1star34", "t1star45", "t1star55", "d30",
    ];
    let keys: BTreeSet<GraphKey> = names
        .iter()
        .map(|n| key_of_normal_form(&forcer_node(n).topology.topo))
        .collect();
    assert_eq!(keys.len(), names.len());
    // an eps unit on the central line vs on a side line: different keys; on two automorphic side lines: equal
    let kite = |eps: [u8; 5]| {
        PTopo::new(
            4,
            0,
            3,
            [
                (0, 1, eps[0]),
                (0, 2, eps[1]),
                (1, 2, eps[2]),
                (1, 3, eps[3]),
                (2, 3, eps[4]),
            ],
        )
    };
    let k = |t: PTopo| canonical(&t).0;
    assert_ne!(k(kite([0, 0, 1, 0, 0])), k(kite([1, 0, 0, 0, 0])));
    assert_eq!(k(kite([1, 0, 0, 0, 0])), k(kite([0, 0, 0, 0, 1])));
    assert_ne!(k(kite([2, 0, 0, 0, 0])), k(kite([1, 0, 0, 0, 0])));
    assert_eq!(
        key_of_normal_form(&forcer_node("t1star05").topology.topo),
        k(kite([0, 0, 1, 0, 0]))
    );
}

#[test]
fn demand_dag_counts() {
    // eager Demand roots = every nonzero integer sector class of the tops (census mode eall)
    let mut got = Vec::new();
    for loops in [2u8, 3, 4] {
        let roots = eall_roots(loops);
        let dag = service().reduction_graph(&roots, ChildMode::Demand);
        let custom = dag
            .nodes
            .iter()
            .filter(|n| n.action.kind() == ActionKind::Custom)
            .count();
        got.push((loops, roots.len(), dag.nodes.len(), custom));
        if loops < 4 {
            // at 2L and 3L the Demand DAG is the ForcerTree DAG
            let tree: BTreeSet<GraphKey> = forcer_tree(loops)
                .nodes
                .into_iter()
                .map(|n| n.key)
                .collect();
            assert_eq!(
                dag.nodes
                    .iter()
                    .map(|n| n.key.clone())
                    .collect::<BTreeSet<_>>(),
                tree
            );
        }
        let all: BTreeSet<&str> = names(&dag).into_iter().collect();
        assert_eq!(all.len(), dag.nodes.len());
    }
    // (L, integer sector classes, Demand nodes, custom nodes); Forcer's own triangle choice gives 427 at 4L
    assert_eq!(
        got,
        vec![(2, 4, 7, 0), (3, 27, 43, 2), (4, 280, DEMAND_4L, 21)]
    );
}

/// Demand nodes at 4L with the frozen triangle choice rule (Forcer's min-edge-id choice: 427, others 426).
const DEMAND_4L: usize = 426;

#[test]
fn sector_classes_are_the_contraction_closure() {
    for loops in [2u8, 3, 4] {
        let keys = |ts: Vec<PTopo>| ts.iter().map(|t| canonical(t).0).collect::<BTreeSet<_>>();
        assert_eq!(keys(sector_classes(&tops(loops))), keys(eall_roots(loops)));
    }
}

#[test]
fn series_parallel_counts() {
    // nodes whose glued graph has no K4 minor, per L (the trivial node counts at L = 0)
    let sp = |dag: &Dag| {
        let mut per_l: BTreeMap<u8, usize> = BTreeMap::new();
        for n in dag.nodes.iter().filter(|n| n.series_parallel) {
            *per_l.entry(n.loops).or_default() += 1;
        }
        per_l.into_iter().collect::<Vec<_>>()
    };
    assert_eq!(sp(&forcer_tree(2)), SP_2L);
    assert_eq!(sp(&forcer_tree(3)), SP_3L);
    assert_eq!(sp(&forcer_tree(4)), SP_4L);
    // every series-parallel node is an insertion node (or the trivial one)
    for n in forcer_tree(4).nodes.iter().filter(|n| n.series_parallel) {
        assert!(matches!(
            n.action.kind(),
            ActionKind::Insertion | ActionKind::Trivial
        ));
    }
}

const SP_2L: &[(u8, usize)] = &[(0, 1), (1, 2), (2, 3)];
const SP_3L: &[(u8, usize)] = &[(0, 1), (1, 4), (2, 8), (3, 14)];
const SP_4L: &[(u8, usize)] = &[(0, 1), (1, 6), (2, 20), (3, 56), (4, 79)];

#[test]
fn graph_keys_match_golden() {
    // the 438 key bytes pinned in hex: detects any change of encoding, normalise or graphica's canonical labelling
    assert_eq!(
        P_TOPOLOGY_VERSION, 1,
        "a version bump needs a new graph_keys_p_topology_v<N>.tsv"
    );
    let (nodes, _) = fixture();
    let mut text = String::from(
        "# rustred-topology GraphKey (P_TOPOLOGY_VERSION 1) of the 438 Forcer topologies; columns: name hex\n",
    );
    for n in nodes {
        text.push_str(&format!(
            "{}\t{}\n",
            n.name,
            key_of_normal_form(&n.topology.topo).to_hex()
        ));
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(GRAPH_KEYS_TSV_PATH);
    if std::env::var_os("RUSTRED_TOPOLOGY_BLESS").is_some() {
        std::fs::write(&path, &text).expect("write golden");
    }
    let golden = std::fs::read_to_string(&path).expect("golden key file");
    assert!(
        golden == text,
        "GraphKey bytes differ from {GRAPH_KEYS_TSV_PATH}"
    );
    for line in golden.lines().filter(|l| !l.starts_with('#')) {
        let hex = line.split('\t').nth(1).expect("hex column");
        assert_eq!(
            GraphKey::from_hex(hex).map(|k| k.to_hex()),
            Some(hex.to_string())
        );
    }
}

#[test]
fn reduction_graph_runs_through_the_service() {
    let fresh = TopologyService::new();
    let t0 = Instant::now();
    let dag4 = fresh.reduction_graph(&tops(4), ChildMode::ForcerTree);
    let cold = t0.elapsed();
    assert_eq!(fresh.stats(), (0, 438));
    assert_eq!(fresh.cached(), 438);
    // 3L and 2L reuse the 4L analyses: every 3L / 2L node is a hit
    let t1 = Instant::now();
    let dag3 = fresh.reduction_graph(&tops(3), ChildMode::ForcerTree);
    let dag2 = fresh.reduction_graph(&tops(2), ChildMode::ForcerTree);
    let warm = t1.elapsed();
    assert_eq!(fresh.stats(), (43 + 7, 438));
    assert_eq!(dag4, reduction_graph(&tops(4), ChildMode::ForcerTree));
    assert_eq!(dag3.nodes.len(), 43);
    assert_eq!(dag2.nodes.len(), 7);
    let json = topologies_json(&dag4);
    assert_eq!(json["schema"], "rustred.p-topologies.v1");
    assert_eq!(json["nodes"].as_array().map(Vec::len), Some(438));
    // timings are recorded (notes/rr-topo.md), not asserted: wall clock depends on the load of the test CPUs
    eprintln!("4L ForcerTree cold {cold:?}; 3L + 2L warm {warm:?}");
}

#[test]
#[ignore = "slow: 5L DAGs from Forcer's 64 tops (6,571 nodes) and all 5L integer sector classes"]
fn forcer_tree_5l_counts() {
    let fresh = TopologyService::new();
    let t0 = Instant::now();
    let dag = fresh.reduction_graph(&tops(5), ChildMode::ForcerTree);
    let tree = t0.elapsed();
    let custom = dag
        .nodes
        .iter()
        .filter(|n| n.action.kind() == ActionKind::Custom)
        .count();
    assert_eq!((dag.nodes.len(), custom), (6571, 283));
    let t1 = Instant::now();
    let roots = sector_classes(&tops(5));
    let closure: BTreeSet<GraphKey> = roots.iter().map(|t| canonical(t).0).collect();
    let from_dag: BTreeSet<GraphKey> = {
        let mut contract: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for e in dag
            .edges
            .iter()
            .filter(|e| matches!(e.kind, DagEdgeKind::Contract(_)))
        {
            contract.entry(e.parent).or_default().push(e.child);
        }
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut work: Vec<u32> = tops(5)
            .iter()
            .map(|t| dag.find(&canonical(&normal_form(t)).0).expect("top").index)
            .collect();
        while let Some(i) = work.pop() {
            if seen.insert(i) {
                work.extend(contract.get(&i).into_iter().flatten().copied());
            }
        }
        seen.into_iter()
            .map(|i| dag.nodes[i as usize].key.clone())
            .collect()
    };
    assert_eq!(closure, from_dag);
    let demand = fresh.reduction_graph(&roots, ChildMode::Demand);
    let demand_t = t1.elapsed();
    let demand_custom = demand
        .nodes
        .iter()
        .filter(|n| n.action.kind() == ActionKind::Custom)
        .count();
    eprintln!(
        "5L ForcerTree {tree:?}; 5L Demand (eall, {} roots) {demand_t:?}; cache {:?}",
        roots.len(),
        fresh.stats()
    );
    assert_eq!(
        (roots.len(), demand.nodes.len(), demand_custom),
        (4142, DEMAND_5L, 283)
    );
    // the 4L tree is a sub-DAG of the 5L one, and so is the Demand DAG (every Demand node was a cache hit)
    let keys5: BTreeSet<GraphKey> = dag.nodes.iter().map(|n| n.key.clone()).collect();
    assert!(forcer_tree(4).nodes.iter().all(|n| keys5.contains(&n.key)));
    assert!(demand.nodes.iter().all(|n| keys5.contains(&n.key)));
    assert_eq!(fresh.stats(), (6260, 6571));
    eprintln!(
        "non-local merges (events, ambiguous) so far: {:?}",
        rustred_topology::nonlocal_merge_events()
    );
}

/// 5L Demand nodes from every integer sector class with the frozen triangle choice rule. The census with Forcer's
/// min-edge-id choice had 6,197; a pure canonical-id choice gives 6,256 [M notes/rr-topo.md]. The custom set (283) and
/// the ForcerTree DAG do not depend on the choice.
const DEMAND_5L: usize = 6260;
