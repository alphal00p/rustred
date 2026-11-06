//! The p census and strong completeness (owner: M2-T2 rr-census).
//!
//! Vacuum-style census checks: sector and class counts per loop order, routes
//! checked line by line without the code that found them, zero fates against
//! the graphs, owners at the min-GraphKey node of the DAG, the eps families, a
//! census that was tampered with, determinism, and the JSON form.
//!
//! The DAG tests run on the library's own node presentations (`present_node`) and
//! on a plain spanning tree of their own: the census must not depend on how a node
//! is routed. The four-loop runs are slow:
//!
//! ```text
//! cargo test --release -p rustred --test p_library census:: -- --ignored --nocapture
//! ```
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::CoefficientContext;
use rustred::propagator::PresentationId;
use rustred::propagator::census::{
    CENSUS_NO_LEDGER, P_CENSUS_SCHEMA, PCensus, PCensusCounts, PCensusOptions, PSectorFate,
    SignedAffineLoopMap, p_census,
};
use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamilyEntry, load_p_families};
use rustred::propagator::present::{present_node, top_topology};
use rustred::propagator::presentation::{
    NodeRef, Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
};
use rustred::propagator::topology::{
    Action, ChildMode, Dag, GraphKey, NodeInfo, Normalised, PTopo, canonical, normalise,
    reduction_graph,
};
use rustred::propagator::{
    NodeIndex, OwnerClassId, P_CENSUS_VERSION, P_CLASS_KEY_VERSION, P_PRESENTATION_VERSION,
    P_TOPOLOGY_VERSION,
};
use rustred::sector::Mask;
use rustred::sector::zero::{Analyzer, Decision};

fn options() -> PCensusOptions {
    PCensusOptions {
        threads: 4,
        ..PCensusOptions::default()
    }
}

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Families and graphs
// ---------------------------------------------------------------------------------------------

/// The tops with at most `max_loops` loops and their graphs.
fn tops(max_loops: u8) -> Vec<(PFamilyEntry, Arc<Presentation>)> {
    let (file, presentations) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    file.families
        .into_iter()
        .zip(presentations)
        .filter(|(entry, _)| entry.loops <= max_loops)
        .collect()
}

fn top_presentations(max_loops: u8) -> Vec<Arc<Presentation>> {
    tops(max_loops).into_iter().map(|(_, p)| p).collect()
}

fn find(parents: &mut [usize], mut vertex: usize) -> usize {
    while parents[vertex] != vertex {
        parents[vertex] = parents[parents[vertex]];
        vertex = parents[vertex];
    }
    vertex
}

/// `topo` with every line outside `support` contracted.
fn sector_graph(topo: &PTopo, support: u64) -> PTopo {
    let mut parents: Vec<usize> = (0..usize::from(topo.n)).collect();
    for (line, &(u, v, _)) in topo.lines.iter().enumerate() {
        if support >> line & 1 == 0 {
            let (a, b) = (
                find(&mut parents, usize::from(u)),
                find(&mut parents, usize::from(v)),
            );
            parents[a.max(b)] = a.min(b);
        }
    }
    let mut vertex = |v: u8| find(&mut parents, usize::from(v)) as u8;
    let (s, t) = (vertex(topo.s), vertex(topo.t));
    let lines: Vec<_> = (topo.lines.iter().enumerate())
        .filter(|&(line, _)| support >> line & 1 == 1)
        .map(|(_, &(u, v, eps))| (vertex(u), vertex(v), eps))
        .collect();
    PTopo::new(topo.n, s, t, lines)
}

/// The DAG key of a sector, `None` when the sector is zero: its graph is, or the
/// contracted lines close a loop, whose momentum then has no line left at all. A
/// nonzero sector of a normalised graph has no lines to merge, so it is its own node.
fn sector_key(topo: &PTopo, support: u64) -> Option<GraphKey> {
    match normalise(&sector_graph(topo, support)) {
        Normalised::Topo(normal, _) if normal.loops() == topo.loops() => {
            assert_eq!(normal.lines.len() as u32, support.count_ones());
            Some(canonical(&normal).0)
        }
        Normalised::Topo(..) | Normalised::Zero | Normalised::Trivial => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Node presentations from a plain spanning tree
// ---------------------------------------------------------------------------------------------

/// Momentum rows over `(k_1, .., k_L, q)`, one per line of `topo`: the cotree
/// lines carry the loop momenta, `q` runs along the tree from `s` to `t`.
fn routed_rows(topo: &PTopo) -> Vec<Vec<i64>> {
    let mut parents: Vec<usize> = (0..usize::from(topo.n)).collect();
    let mut tree = Vec::new();
    let mut cotree = Vec::new();
    for (line, &(u, v, _)) in topo.lines.iter().enumerate() {
        let (a, b) = (
            find(&mut parents, usize::from(u)),
            find(&mut parents, usize::from(v)),
        );
        if a == b {
            cotree.push(line);
        } else {
            parents[a.max(b)] = a.min(b);
            tree.push(line);
        }
    }
    let width = cotree.len() + 1;
    let mut rows = vec![vec![0_i64; width]; topo.lines.len()];
    let path = |from: u8, to: u8| -> Vec<(usize, i64)> {
        let mut via = vec![None; usize::from(topo.n)];
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            for &line in &tree {
                let (u, v, _) = topo.lines[line];
                let step = if at == u {
                    (v, 1)
                } else if at == v {
                    (u, -1)
                } else {
                    continue;
                };
                if step.0 != from && via[usize::from(step.0)].is_none() {
                    via[usize::from(step.0)] = Some((at, line, step.1));
                    stack.push(step.0);
                }
            }
        }
        let mut steps = Vec::new();
        let mut at = to;
        while at != from {
            let (previous, line, sign) = via[usize::from(at)].expect("a spanning tree");
            steps.push((line, sign));
            at = previous;
        }
        steps
    };
    for (column, &line) in cotree.iter().enumerate() {
        let (u, v, _) = topo.lines[line];
        rows[line][column] = 1;
        for (step, sign) in path(v, u) {
            rows[step][column] += sign;
        }
    }
    for (step, sign) in path(topo.s, topo.t) {
        rows[step][width - 1] += sign;
    }
    rows
}

/// The scalar products of a momentum pair over `k_i.k_j (i <= j), k_i.q`.
fn product_row(a: &[i64], b: &[i64]) -> Vec<i128> {
    let loops = a.len() - 1;
    let mut row = Vec::new();
    for left in 0..loops {
        for right in left..loops {
            row.push(if left == right {
                i128::from(a[left] * b[left])
            } else {
                i128::from(a[left] * b[right] + a[right] * b[left])
            });
        }
    }
    row.extend((0..loops).map(|i| i128::from(a[i] * b[loops] + a[loops] * b[i])));
    row
}

/// Adds `row` to the echelon `basis` unless it depends on it.
fn extends(basis: &mut Vec<Vec<i128>>, mut row: Vec<i128>) -> bool {
    for base in basis.iter() {
        let pivot = base.iter().position(|&c| c != 0).unwrap();
        if row[pivot] != 0 {
            let (scale, factor) = (base[pivot], row[pivot]);
            for (entry, &b) in row.iter_mut().zip(base) {
                *entry = *entry * scale - b * factor;
            }
        }
    }
    let independent = row.iter().any(|&c| c != 0);
    if independent {
        basis.push(row);
    }
    independent
}

/// The ISPs `k_i.k_j`, `k_i.q` that complete the scalar products of `rows`.
fn completing_isps(rows: &[Vec<i64>]) -> Vec<Slot> {
    let width = rows[0].len();
    let unit = |position: usize| {
        let mut row = vec![0_i64; width];
        row[position] = 1;
        row
    };
    let mut basis = Vec::new();
    for row in rows {
        assert!(extends(&mut basis, product_row(row, row)));
    }
    let pairs = (0..width - 1).flat_map(|left| (left..width).map(move |right| (left, right)));
    pairs
        .filter_map(|(left, right)| {
            let (left, right) = (unit(left), unit(right));
            extends(&mut basis, product_row(&left, &right))
                .then_some(Slot::Isp(SlotIsp { left, right }))
        })
        .collect()
}

fn node_ref(node: &NodeInfo) -> Option<NodeRef> {
    Some(NodeRef {
        index: NodeIndex(node.index),
        graph_key: Arc::clone(&node.key.0),
    })
}

/// An EpsNode-kind presentation of `node`: line slot `i` is canonical line `i`.
fn routed_node(node: &NodeInfo, id: u32) -> Presentation {
    let rows = routed_rows(&node.topo);
    let isps = completing_isps(&rows);
    let lines = rows
        .into_iter()
        .zip(&node.topo.lines)
        .map(|(momentum, line)| {
            Slot::Line(SlotLine {
                momentum,
                eps_units: line.2,
            })
        });
    Presentation::from_slots(
        PresentationId(id),
        PresentationKind::EpsNode,
        &format!("census_node_{}", node.key.to_hex()),
        node.loops,
        lines.chain(isps).collect(),
        &context(),
        node_ref(node),
    )
    .unwrap()
}

/// The Induced presentation of an integer Custom node: a sector of a top.
fn induced_node(
    node: &NodeInfo,
    id: u32,
    tops: &[(PFamilyEntry, Arc<Presentation>)],
) -> (Presentation, PTopo) {
    for (entry, top) in tops.iter().filter(|(entry, _)| entry.loops == node.loops) {
        let graph = top_topology(entry);
        let mut masks =
            (1..1_u64 << graph.lines.len()).filter(|m| m.count_ones() == u32::from(node.t));
        if let Some(mask) = masks.find(|&m| sector_key(&graph, m).as_ref() == Some(&node.key)) {
            let presentation = Presentation {
                id: PresentationId(id),
                kind: PresentationKind::Induced { top: top.id, mask },
                loops: top.loops,
                slots: top.slots.clone(),
                family: Arc::clone(&top.family),
                node: node_ref(node),
            };
            return (presentation, graph);
        }
    }
    panic!("node {} is no sector of a top", node.index)
}

/// Tops and node presentations with the DAG of the tops; `graphs` holds the graph
/// each presentation's line slots are the lines of.
struct Library {
    dag: Dag,
    presentations: Vec<Arc<Presentation>>,
    graphs: BTreeMap<PresentationId, PTopo>,
}

/// The presentations M2-T8a assembles: `present_node` per node, line slot `i` =
/// canonical line `i`, and integer Custom nodes on the sector of a top.
fn production_library(max_loops: u8) -> Library {
    let tops = tops(max_loops);
    let top_presentations: Vec<_> = tops.iter().map(|(_, p)| Arc::clone(p)).collect();
    let roots: Vec<_> = tops.iter().map(|(entry, _)| top_topology(entry)).collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let mut presentations = top_presentations.clone();
    let mut graphs: BTreeMap<_, _> = (tops.iter())
        .map(|(entry, p)| (p.id, top_topology(entry)))
        .collect();
    let context = top_presentations[0].family.coefficient_context().clone();
    let mut id = presentations.iter().map(|p| p.id.0).max().unwrap();
    for node in &dag.nodes {
        let made =
            present_node(node, &top_presentations, PresentationId(id + 1), &context).unwrap();
        assert_eq!(made.fallback, None);
        let Some(presentation) = made.presentation else {
            assert_eq!(node.loops, 0);
            continue;
        };
        id += 1;
        let graph = match presentation.kind {
            PresentationKind::Induced { top, .. } => graphs[&top].clone(),
            _ => node.topo.clone(),
        };
        graphs.insert(presentation.id, graph);
        presentations.push(Arc::new(presentation));
    }
    Library {
        dag,
        presentations,
        graphs,
    }
}

/// Plain spanning trees, integer Custom nodes on the sector of a top.
fn library(max_loops: u8) -> Library {
    library_with(max_loops, |node| {
        matches!(node.action, Action::Custom) && node.eps_lines == 0
    })
}

/// `induced` picks the nodes presented as a sector of a top.
fn library_with(max_loops: u8, induced: impl Fn(&NodeInfo) -> bool) -> Library {
    let tops = tops(max_loops);
    let roots: Vec<_> = tops.iter().map(|(entry, _)| top_topology(entry)).collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let mut presentations: Vec<_> = tops.iter().map(|(_, p)| Arc::clone(p)).collect();
    let mut graphs: BTreeMap<_, _> = (tops.iter())
        .map(|(entry, p)| (p.id, top_topology(entry)))
        .collect();
    let mut id = presentations.iter().map(|p| p.id.0).max().unwrap();
    for node in dag.nodes.iter().filter(|node| node.loops > 0) {
        id += 1;
        let (presentation, graph) = if induced(node) {
            induced_node(node, id, &tops)
        } else {
            (routed_node(node, id), node.topo.clone())
        };
        graphs.insert(presentation.id, graph);
        presentations.push(Arc::new(presentation));
    }
    Library {
        dag,
        presentations,
        graphs,
    }
}

/// The 3L library and its census: several tests read them.
fn three_loop_library() -> &'static (Library, PCensus) {
    static LIBRARY: OnceLock<(Library, PCensus)> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let library = production_library(3);
        let census = p_census(&library.presentations, Some(&library.dag), &options()).unwrap();
        (library, census)
    })
}

/// The census of the 3L tops: several tests tamper with copies of it.
fn three_loop_tops() -> &'static (Vec<Arc<Presentation>>, PCensus) {
    static TOPS: OnceLock<(Vec<Arc<Presentation>>, PCensus)> = OnceLock::new();
    TOPS.get_or_init(|| {
        let tops = top_presentations(3);
        let census = p_census(&tops, None, &options()).unwrap();
        (tops, census)
    })
}

// ---------------------------------------------------------------------------------------------
// Independent checks of a census
// ---------------------------------------------------------------------------------------------

fn lines(p: &Presentation) -> Vec<(usize, &SlotLine)> {
    (p.slots.iter().enumerate())
        .filter_map(|(slot, s)| match s {
            Slot::Line(line) => Some((slot, line)),
            Slot::Isp(_) => None,
        })
        .collect()
}

fn presentation_of(ps: &[Arc<Presentation>], id: PresentationId) -> &Presentation {
    ps.iter().find(|p| p.id == id).unwrap()
}

/// `map` takes the lines of the source support onto the lines of the owner
/// support, one to one and up to sign, each with its eps units, and is a
/// unimodular loop map with `q -> +-q`. None of this calls the code under test.
fn assert_route(
    source: &Presentation,
    source_support: u64,
    owner: &Presentation,
    owner_support: u64,
    map: &SignedAffineLoopMap,
) {
    assert_eq!((map.loops_in, map.loops_out), (source.loops, owner.loops));
    assert!(map.is_unimodular(), "{map:?}");
    assert!(matches!(map.external_sign, 1 | -1));
    let mut images = 0_u64;
    for (slot, line) in lines(source) {
        if source_support >> slot & 1 == 0 {
            continue;
        }
        let image = map.map_momentum(&line.momentum).unwrap();
        let negated: Vec<_> = image.iter().map(|c| -c).collect();
        let (target, target_line) = (lines(owner).into_iter())
            .find(|(target, candidate)| {
                owner_support >> target & 1 == 1
                    && (candidate.momentum == image || candidate.momentum == negated)
            })
            .unwrap_or_else(|| panic!("line {slot} of {} has no image", source.id.0));
        assert_eq!(target_line.eps_units, line.eps_units, "line {slot}");
        assert_eq!(images & 1 << target, 0, "two lines on {target}");
        images |= 1 << target;
    }
    assert_eq!(images, owner_support);
}

/// Every route of `census`, against the owner the class record names; the owner
/// itself has the identity. Returns the number of routes.
fn assert_all_routes(ps: &[Arc<Presentation>], census: &PCensus) -> usize {
    let mut routes = 0;
    for (&(id, support), fate) in &census.sectors {
        let PSectorFate::Owner { class, map } = fate else {
            continue;
        };
        let record = &census.classes[class];
        let source = presentation_of(ps, id);
        let owner = presentation_of(ps, record.owner.0);
        assert_route(source, support, owner, record.owner.1, map);
        assert_eq!(u32::from(record.t), support.count_ones());
        if (id, support) == record.owner {
            let size = usize::from(record.key.loops);
            let unit: Vec<_> = (0..size * size)
                .map(|entry| i64::from(entry % (size + 1) == 0))
                .collect();
            assert_eq!(*map.loop_linear, *unit, "the owner keeps its loops");
            assert!(map.loop_external.iter().all(|&c| c == 0));
            assert_eq!(map.external_sign, 1);
        }
        routes += 1;
    }
    routes
}

fn zero_sectors(census: &PCensus, id: PresentationId) -> BTreeSet<u64> {
    (census.sectors.iter())
        .filter(|((p, _), fate)| *p == id && **fate == PSectorFate::Zero)
        .map(|((_, support), _)| *support)
        .collect()
}

/// `(loops, presentations, sectors, zero, nonzero, classes)`.
type CountRow = (u8, usize, usize, usize, usize, usize);

fn count_rows(counts: &[PCensusCounts]) -> Vec<CountRow> {
    counts
        .iter()
        .map(|c| {
            assert_eq!(c.sectors, c.zero_sectors + c.nonzero_sectors);
            (
                c.loops,
                c.presentations,
                c.sectors,
                c.zero_sectors,
                c.nonzero_sectors,
                c.classes,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Tops
// ---------------------------------------------------------------------------------------------

#[test]
fn counts_of_the_tops() {
    let (tops, census) = three_loop_tops();
    assert_eq!(
        count_rows(&census.counts(tops).unwrap()),
        [
            (1, 1, 4, 3, 1, 1),
            (2, 1, 32, 24, 8, 4),
            (3, 3, 768, 593, 175, 22),
        ]
    );
}

#[test]
fn the_tops_are_complete() {
    let (tops, census) = three_loop_tops();
    census.check_complete(tops, None).unwrap();
    for p in tops {
        let held = (census.sectors.range((p.id, 0)..=(p.id, u64::MAX))).count();
        assert_eq!(held, 1 << lines(p).len(), "routes + zeros == 2^t");
    }
    assert_eq!(assert_all_routes(tops, census), 1 + 8 + 175);
}

#[test]
fn one_census_per_loop_order_agrees_with_the_joint_one() {
    let (tops, census) = three_loop_tops();
    for loops in 1..=3 {
        let selected: Vec<_> = tops.iter().filter(|p| p.loops == loops).cloned().collect();
        let alone = p_census(&selected, None, &options()).unwrap();
        for (id, record) in &alone.classes {
            assert_eq!(*record, census.classes[id], "{}", record.key.label());
        }
        for (sector, fate) in &alone.sectors {
            assert_eq!(*fate, census.sectors[sector]);
        }
    }
}

#[test]
fn classes_are_the_classify_partition_of_the_nonzero_sectors() {
    let (tops, census) = three_loop_tops();
    let limits = ClassLimits::default();
    for (&(id, support), fate) in &census.sectors {
        let p = presentation_of(tops, id);
        let mask = Mask::try_new((0..p.slots.len()).map(|slot| support >> slot & 1 == 1)).unwrap();
        let decision = Analyzer::try_unrestricted(&p.family)
            .unwrap()
            .analyze(&mask)
            .unwrap();
        match fate {
            PSectorFate::Zero => assert!(matches!(decision, Decision::ProvedZero(_))),
            PSectorFate::Owner { class, .. } => {
                assert!(matches!(decision, Decision::Inconclusive(_)));
                let key = classify(p, support, &limits).unwrap().unwrap().key;
                assert_eq!(key.id(), *class);
                assert_eq!(census.classes[class].key, key);
            }
        }
    }
}

#[test]
fn zero_fates_of_the_tops_agree_with_the_graphs() {
    let (_, census) = three_loop_tops();
    for (entry, p) in tops(3) {
        let graph = top_topology(&entry);
        let zero_by_graph: BTreeSet<u64> = (0..1_u64 << graph.lines.len())
            .filter(|&support| sector_key(&graph, support).is_none())
            .collect();
        assert_eq!(zero_sectors(census, p.id), zero_by_graph, "{}", entry.name);
    }
}

#[test]
fn without_a_dag_the_owner_is_the_first_member() {
    let (_, census) = three_loop_tops();
    let mut first: BTreeMap<OwnerClassId, (PresentationId, u64)> = BTreeMap::new();
    for (&sector, fate) in &census.sectors {
        if let PSectorFate::Owner { class, .. } = fate {
            first.entry(*class).or_insert(sector);
        }
    }
    for (class, record) in &census.classes {
        assert_eq!(record.owner, first[class]);
        assert_eq!(record.node, None);
        assert!(record.nodes.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// The DAG link
// ---------------------------------------------------------------------------------------------

/// The class of each DAG node through its presentation.
fn node_classes(library: &Library, census: &PCensus) -> BTreeMap<u32, OwnerClassId> {
    let mut classes = BTreeMap::new();
    for p in &library.presentations {
        let Some(reference) = &p.node else {
            continue;
        };
        let lines_mask = lines(p)
            .iter()
            .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
        let support = match p.kind {
            PresentationKind::Induced { mask, .. } => mask,
            _ => lines_mask,
        };
        let PSectorFate::Owner { class, .. } = census.sectors[&(p.id, support)] else {
            panic!("node {} is zero", reference.index.0);
        };
        assert!(classes.insert(reference.index.0, class).is_none());
    }
    classes
}

/// Owners at min-GraphKey nodes, read from the DAG and not from the census's
/// own rule; sectors line up with the DAG node of their graph.
fn assert_dag_link(library: &Library, census: &PCensus) {
    census
        .check_complete_with(&library.presentations, Some(&library.dag), &options())
        .unwrap();
    assert_all_routes(&library.presentations, census);
    let classes = node_classes(library, census);
    for node in library.dag.nodes.iter().filter(|node| node.loops > 0) {
        assert!(classes.contains_key(&node.index), "node {}", node.index);
    }
    assert_eq!(
        classes.len() + 1,
        library.dag.nodes.len(),
        "all but trivial"
    );

    let mut by_class: BTreeMap<OwnerClassId, Vec<&NodeInfo>> = BTreeMap::new();
    for (&node, &class) in &classes {
        by_class
            .entry(class)
            .or_default()
            .push(&library.dag.nodes[node as usize]);
    }
    assert_eq!(by_class.len(), census.classes.len(), "one node at least");
    for (class, record) in &census.classes {
        let nodes = &by_class[class];
        let least = nodes.iter().min_by_key(|node| &node.key).unwrap();
        assert_eq!(record.node, Some(NodeIndex(least.index)));
        let listed: Vec<_> = nodes.iter().map(|node| NodeIndex(node.index)).collect();
        assert_eq!(record.nodes, listed);
        let owner = presentation_of(&library.presentations, record.owner.0);
        let reference = owner.node.as_ref().unwrap();
        assert_eq!(reference.index, NodeIndex(least.index));
        assert_eq!(*reference.graph_key, *least.key.0);
    }

    // Every nonzero sector of every presentation is a DAG node, of its class.
    for (&(id, support), fate) in &census.sectors {
        let key = sector_key(&library.graphs[&id], support);
        match (fate, key) {
            (PSectorFate::Zero, None) => {}
            (PSectorFate::Owner { class, .. }, Some(key)) => {
                let node = library
                    .dag
                    .find(&key)
                    .unwrap_or_else(|| panic!("sector ({}, {support:#x}) is no DAG node", id.0));
                assert_eq!(classes[&node.index], *class);
            }
            (fate, key) => panic!("sector ({}, {support:#x}): {fate:?}, key {key:?}", id.0),
        }
    }
}

#[test]
fn the_one_loop_dag_links_to_its_class() {
    let library = library(1);
    assert_eq!(library.dag.nodes.len(), 2);
    let census = p_census(&library.presentations, Some(&library.dag), &options()).unwrap();
    assert_dag_link(&library, &census);
    assert_eq!(census.classes.len(), 1);
    // The top routes onto the bubble node, and the bubble node owns the class.
    let record = census.classes.values().next().unwrap();
    assert_eq!(record.owner, (library.presentations[1].id, 0b11));
    assert_eq!(
        count_rows(&census.counts(&library.presentations).unwrap()),
        [(1, 2, 8, 6, 2, 1)]
    );
}

#[test]
fn the_two_loop_dag_links_to_its_classes() {
    let library = library(2);
    assert_eq!(library.dag.nodes.len(), 7);
    let census = p_census(&library.presentations, Some(&library.dag), &options()).unwrap();
    assert_dag_link(&library, &census);
    assert_eq!(census.classes.len(), 1 + 1 + 4);
}

const COUNTS_OF_THE_THREE_LOOP_DAG: [CountRow; 3] = [
    (1, 5, 13, 8, 5, 4),
    (2, 12, 172, 132, 40, 11),
    (3, 30, 3_344, 2_837, 507, 22),
];

#[test]
fn owners_of_the_three_loop_dag_are_its_min_graph_key_nodes() {
    let (library, census) = three_loop_library();
    assert_eq!(library.dag.nodes.len(), 43);
    assert_dag_link(library, census);
    let integer = census.classes.values().filter(|r| r.eps_lines == 0).count();
    assert_eq!(integer, 1 + 4 + 22);
    assert_eq!(census.classes.len(), integer + 10);
    assert_eq!(
        count_rows(&census.counts(&library.presentations).unwrap()),
        COUNTS_OF_THE_THREE_LOOP_DAG
    );
    // The 42 nodes with loops on 37 classes: five Whitney pairs of 3L nodes.
    let twins: Vec<_> = (census.classes.values())
        .filter(|record| record.nodes.len() > 1)
        .map(|record| (record.key.loops, record.nodes.len()))
        .collect();
    assert_eq!(twins, [(3, 2); 5]);
}

/// The routing of a node changes no class and no owner node.
#[test]
fn the_three_loop_owners_do_not_depend_on_the_routing_of_a_node() {
    let (production, census) = three_loop_library();
    let plain = library(3);
    assert_eq!(plain.dag, production.dag);
    let other = p_census(&plain.presentations, Some(&plain.dag), &options()).unwrap();
    assert_dag_link(&plain, &other);
    assert_eq!(other.classes.len(), census.classes.len());
    for (class, record) in &census.classes {
        let moved = &other.classes[class];
        assert_eq!((moved.node, &moved.nodes), (record.node, &record.nodes));
        assert_eq!(moved.owner.0, record.owner.0, "both number the nodes alike");
    }
}

/// The owner is a function of the graphs: renumbering the presentations leaves the
/// classes and nodes alone, and moves the owner only by the new number.
#[test]
fn owners_do_not_depend_on_the_presentation_list() {
    let library = library(2);
    let census = p_census(&library.presentations, Some(&library.dag), &options()).unwrap();
    let count = library.presentations.len() as u32;
    let renumbered: Vec<_> = (library.presentations.iter().rev())
        .map(|p| {
            let mut copy = (**p).clone();
            copy.id = PresentationId(count - 1 - p.id.0);
            Arc::new(copy)
        })
        .collect();
    let other = p_census(&renumbered, Some(&library.dag), &options()).unwrap();
    assert_eq!(other.classes.len(), census.classes.len());
    for (id, record) in &census.classes {
        let moved = &other.classes[id];
        assert_eq!((moved.node, &moved.nodes), (record.node, &record.nodes));
        assert_eq!(moved.owner.0.0, count - 1 - record.owner.0.0);
        assert_eq!(moved.owner.1, record.owner.1);
    }
    other
        .check_complete(&renumbered, Some(&library.dag))
        .unwrap();
}

/// A node may stand on a proper sector of a top: the routing of a node changes
/// neither its class nor its min-GraphKey node, only where the owner sits.
#[test]
fn induced_presentations_stand_for_their_sector() {
    let routed = library(2);
    let routed_census = p_census(&routed.presentations, Some(&routed.dag), &options()).unwrap();
    let induced = library_with(2, |node| node.eps_lines == 0);
    let census = p_census(&induced.presentations, Some(&induced.dag), &options()).unwrap();
    assert_dag_link(&induced, &census);
    let mut proper = 0;
    for (class, record) in &census.classes {
        let other = &routed_census.classes[class];
        assert_eq!((record.node, &record.nodes), (other.node, &other.nodes));
        let owner = presentation_of(&induced.presentations, record.owner.0);
        if let PresentationKind::Induced { mask, .. } = owner.kind {
            assert_eq!(record.owner.1, mask);
            proper += usize::from(mask != (1 << lines(owner).len()) - 1);
        }
    }
    assert!(proper >= 3, "{proper} owners on a proper sector");
    assert_eq!(routed_census.classes.len(), census.classes.len());
}

fn assert_malformed<T: std::fmt::Debug>(result: Result<T, PError>, needle: &str) {
    match result {
        Err(PError::MalformedLibrary(detail)) => {
            assert!(detail.contains(needle), "{detail:?} lacks {needle:?}");
        }
        other => panic!("{other:?} instead of the failure {needle:?}"),
    }
}

/// Both entry points refuse a list that does not fit the DAG.
fn assert_dag_refusal(dag: &Dag, ps: &[Arc<Presentation>], census: &PCensus, needle: &str) {
    assert_malformed(p_census(ps, Some(dag), &options()), needle);
    assert_malformed(census.check_complete(ps, Some(dag)), needle);
}

#[test]
fn presentations_that_do_not_fit_the_dag_are_refused() {
    let library = library(2);
    let (dag, ps) = (&library.dag, &library.presentations);
    let census = p_census(ps, Some(dag), &options()).unwrap();
    let edited = |change: &dyn Fn(&mut Presentation)| -> Vec<Arc<Presentation>> {
        let mut edited = ps.clone();
        let last = edited.last_mut().unwrap();
        let mut copy = (**last).clone();
        change(&mut copy);
        *last = Arc::new(copy);
        edited
    };
    let last_node = ps.last().unwrap().node.as_ref().unwrap().index.0;

    assert_dag_refusal(
        dag,
        &ps[..ps.len() - 1],
        &census,
        &format!("node {last_node} has no presentation"),
    );
    assert_dag_refusal(
        dag,
        &edited(&|p| p.node.as_mut().unwrap().index = NodeIndex(999)),
        &census,
        "outside the DAG",
    );
    assert_dag_refusal(
        dag,
        &edited(&|p| p.node.as_mut().unwrap().graph_key = Arc::from(&b"other"[..])),
        &census,
        "with another key",
    );
    let other = (dag.nodes.iter())
        .find(|node| node.loops > 0 && node.index != last_node)
        .unwrap();
    assert_dag_refusal(
        dag,
        &edited(&|p| p.node = node_ref(other)),
        &census,
        "two presentations",
    );

    // The right node and key on the slots of a presentation of another shape.
    let last = ps.last().unwrap();
    let wider = (ps.iter())
        .find(|p| p.node.is_some() && p.slots.len() != last.slots.len())
        .unwrap();
    assert_dag_refusal(
        dag,
        &edited(&|p| {
            p.slots = wider.slots.clone();
            p.family = Arc::clone(&wider.family);
            p.loops = wider.loops;
        }),
        &census,
        "presents (loops, lines, eps lines)",
    );
}

#[test]
fn a_sector_of_a_node_presentation_outside_its_lines_is_refused() {
    let library = library(2);
    let (dag, ps) = (&library.dag, &library.presentations);
    let census = p_census(ps, Some(dag), &options()).unwrap();
    let mut edited = ps.clone();
    let last = edited.last_mut().unwrap();
    let mut copy = (**last).clone();
    copy.kind = PresentationKind::Induced {
        top: PresentationId(0),
        mask: 1 << 40,
    };
    *last = Arc::new(copy);
    assert_dag_refusal(dag, &edited, &census, "outside its lines");
}

/// A sector of the right shape that is proved zero cannot stand for a node.
#[test]
fn a_node_on_a_zero_sector_is_refused() {
    let library = library(2);
    let (dag, ps) = (&library.dag, &library.presentations);
    let census = p_census(ps, Some(dag), &options()).unwrap();
    let kite = (ps.iter())
        .find(|p| p.node.is_none() && p.loops == 2)
        .unwrap();
    let zero_mask = (zero_sectors(&census, kite.id).into_iter())
        .find(|mask| mask.count_ones() == 3)
        .unwrap();
    let position = (ps.iter())
        .position(|p| {
            p.node.as_ref().is_some_and(|reference| {
                let node = &dag.nodes[reference.index.0 as usize];
                (node.loops, node.t, node.eps_lines) == (2, 3, 0)
            })
        })
        .unwrap();
    let mut copy = (**kite).clone();
    copy.id = ps[position].id;
    copy.kind = PresentationKind::Induced {
        top: kite.id,
        mask: zero_mask,
    };
    copy.node = ps[position].node.clone();
    let mut edited = ps.clone();
    edited[position] = Arc::new(copy);
    assert_malformed(p_census(&edited, Some(dag), &options()), "is proved zero");
}

#[test]
fn a_class_without_a_dag_node_is_refused() {
    let one_loop = library(1);
    let mut ps = one_loop.presentations.clone();
    let id = ps.iter().map(|p| p.id.0).max().unwrap() + 1;
    let (_, kite) = tops(2)
        .into_iter()
        .find(|(entry, _)| entry.loops == 2)
        .unwrap();
    let mut copy = (*kite).clone();
    copy.id = PresentationId(id);
    ps.push(Arc::new(copy));
    assert_malformed(
        p_census(&ps, Some(&one_loop.dag), &options()),
        "has no DAG node",
    );
}

// ---------------------------------------------------------------------------------------------
// Eps axes
// ---------------------------------------------------------------------------------------------

/// The kite with `eps` units per line; T001c = `[1, 0, 0, 0, 1]`, T013c =
/// `[1, 0, 1, 1, 0]` (`rules_study/knockout/families/`).
fn eps_kite(id: u32, eps: [u8; 5]) -> Arc<Presentation> {
    let rows: [&[i64]; 5] = [
        &[1, 0, 0],
        &[-1, -1, 0],
        &[0, 1, 0],
        &[1, 1, -1],
        &[0, -1, 1],
    ];
    let slots = rows
        .iter()
        .zip(eps)
        .map(|(row, eps_units)| {
            Slot::Line(SlotLine {
                momentum: row.to_vec(),
                eps_units,
            })
        })
        .collect();
    Arc::new(
        Presentation::from_slots(
            PresentationId(id),
            PresentationKind::EpsNode,
            &format!("census_eps_{id}"),
            2,
            slots,
            &context(),
            None,
        )
        .unwrap(),
    )
}

/// The graph of [`eps_kite`] with the eps units: vertices `0, 1` external, `D1`
/// the central line.
fn eps_kite_graph(eps: [u8; 5]) -> PTopo {
    let ends = [(2, 3), (0, 2), (1, 2), (0, 3), (1, 3)];
    PTopo::new(
        4,
        0,
        1,
        ends.iter().zip(eps).map(|(&(u, v), units)| (u, v, units)),
    )
}

/// All `2^forced` raw charts of every support have one fate; the census holds
/// supports only, `forced` is in each of them.
fn assert_eps_census(p: &Arc<Presentation>, census: &PCensus, forced: u64, nonzero: usize) {
    let limits = ClassLimits::default();
    let analyzer = Analyzer::try_unrestricted(&p.family).unwrap();
    let free = 0b11111 & !forced;
    let sectors: Vec<_> = (census.sectors.iter())
        .filter(|((id, _), _)| *id == p.id)
        .collect();
    assert_eq!(sectors.len(), 1 << free.count_ones());
    let mut routes = 0;
    for ((_, support), fate) in sectors {
        assert_eq!(support & forced, forced, "{support:#x}");
        for raw in (0..1_u64 << 5).filter(|raw| raw | forced == *support) {
            let mask = Mask::try_new((0..5).map(|slot| raw >> slot & 1 == 1)).unwrap();
            match (fate, analyzer.analyze(&mask).unwrap()) {
                (PSectorFate::Zero, Decision::ProvedZero(_)) => {}
                (PSectorFate::Owner { class, .. }, Decision::Inconclusive(_)) => {
                    let sector = classify(p, raw, &limits).unwrap().unwrap();
                    assert_eq!((sector.raw, sector.support), (raw, *support));
                    assert_eq!(sector.key.id(), *class);
                }
                (fate, _) => panic!("raw chart {raw:#x} disagrees with {fate:?}"),
            }
        }
        routes += usize::from(matches!(fate, PSectorFate::Owner { .. }));
    }
    assert_eq!(routes, nonzero);
}

#[test]
fn the_t001c_and_t013c_censuses_key_sectors_by_support() {
    let t001c_eps = [1, 0, 0, 0, 1];
    let t013c_eps = [1, 0, 1, 1, 0];
    let ps = vec![eps_kite(0, t001c_eps), eps_kite(1, t013c_eps)];
    let census = p_census(&ps, None, &options()).unwrap();
    census.check_complete(&ps, None).unwrap();
    assert_all_routes(&ps, &census);
    assert_eps_census(&ps[0], &census, 0b10001, 5);
    assert_eps_census(&ps[1], &census, 0b01101, 4);

    // T001c: `D1, D5` alone, or with `D3` or `D4`, pinch `s` and `t` or close `D1`.
    assert_eq!(
        zero_sectors(&census, ps[0].id),
        BTreeSet::from([0b10001, 0b10101, 0b11001])
    );
    assert_eq!(zero_sectors(&census, ps[1].id), BTreeSet::new());

    // The graphs agree, and every sector keeps its eps lines.
    for (p, eps) in [(&ps[0], t001c_eps), (&ps[1], t013c_eps)] {
        let graph = eps_kite_graph(eps);
        for support in (0..1_u64 << 5).filter(|s| s & p.forced_axes() == p.forced_axes()) {
            assert_eq!(
                sector_key(&graph, support).is_none(),
                census.sectors[&(p.id, support)] == PSectorFate::Zero,
                "support {support:#x}"
            );
        }
    }
    for record in census.classes.values() {
        assert!(matches!(record.eps_lines, 2 | 3), "{record:?}");
        assert_eq!(record.key.eps_lines, record.eps_lines);
    }
}

/// The mirror image `(D2 D4)(D3 D5)` of an eps kite is one class per sector pair;
/// a route never lands an eps line on an integer one, and the integer kite stays
/// apart.
#[test]
fn routes_between_eps_presentations_keep_the_eps_lines() {
    let ps = vec![
        eps_kite(0, [1, 0, 0, 0, 1]),
        eps_kite(1, [1, 0, 1, 0, 0]),
        eps_kite(2, [0; 5]),
    ];
    let census = p_census(&ps, None, &options()).unwrap();
    census.check_complete(&ps, None).unwrap();
    assert_all_routes(&ps, &census);
    let top = census.sectors[&(ps[0].id, 0b11111)].clone();
    let mirrored = census.sectors[&(ps[1].id, 0b11111)].clone();
    let integer = census.sectors[&(ps[2].id, 0b11111)].clone();
    let class = |fate: &PSectorFate| match fate {
        PSectorFate::Owner { class, .. } => *class,
        PSectorFate::Zero => panic!("the top is zero"),
    };
    assert_eq!(class(&top), class(&mirrored));
    assert_ne!(class(&top), class(&integer));
    assert_eq!(census.classes[&class(&top)].eps_lines, 2);
    assert_eq!(census.classes[&class(&top)].owner, (ps[0].id, 0b11111));
}

#[test]
fn the_three_loop_eps_nodes_keep_their_forced_axes() {
    let (library, census) = three_loop_library();
    let mut eps_nodes = 0;
    for p in &library.presentations {
        let forced = p.forced_axes();
        if forced == 0 {
            continue;
        }
        eps_nodes += 1;
        let free = lines(p).len() - forced.count_ones() as usize;
        let held: Vec<_> = (census.sectors.range((p.id, 0)..=(p.id, u64::MAX)))
            .map(|((_, support), _)| *support)
            .collect();
        assert_eq!(held.len(), 1 << free, "one sector per support");
        assert!(held.iter().all(|support| support & forced == forced));
    }
    assert_eq!(eps_nodes, 10);
}

// ---------------------------------------------------------------------------------------------
// A census that was tampered with
// ---------------------------------------------------------------------------------------------

/// The last sector that is not the owner of its class, with the class: tampering
/// with it leaves the first member of every class in place.
fn last_member(census: &PCensus) -> ((PresentationId, u64), OwnerClassId) {
    (census.sectors.iter().rev())
        .find_map(|(sector, fate)| match fate {
            PSectorFate::Owner { class, .. } if census.classes[class].owner != *sector => {
                Some((*sector, *class))
            }
            _ => None,
        })
        .unwrap()
}

fn route_of(census: &PCensus, sector: (PresentationId, u64)) -> SignedAffineLoopMap {
    match &census.sectors[&sector] {
        PSectorFate::Owner { map, .. } => map.clone(),
        PSectorFate::Zero => panic!("a zero sector"),
    }
}

/// The identity on `loops` loops, as an owner holds it.
fn identity(loops: u8) -> SignedAffineLoopMap {
    let size = usize::from(loops);
    SignedAffineLoopMap {
        loops_in: loops,
        loops_out: loops,
        loop_linear: (0..size * size)
            .map(|entry| i64::from(entry % (size + 1) == 0))
            .collect(),
        loop_external: vec![0; size].into(),
        external_sign: 1,
    }
}

fn tampered(change: impl FnOnce(&mut PCensus)) -> Result<(), PError> {
    let (tops, census) = three_loop_tops();
    let mut copy = census.clone();
    change(&mut copy);
    copy.check_complete(tops, None)
}

fn assert_missing_owner(result: Result<(), PError>) {
    match result {
        Err(PError::MissingOwner { ledger, .. }) => assert_eq!(ledger, CENSUS_NO_LEDGER),
        other => panic!("{other:?} instead of a missing owner"),
    }
}

#[test]
fn check_complete_refuses_missing_and_foreign_sectors() {
    let (_, census) = three_loop_tops();
    let some = *census.sectors.keys().next().unwrap();
    assert_malformed(
        tampered(|c| {
            c.sectors.remove(&some);
        }),
        "fates for",
    );
    let (id, _) = some;
    assert_malformed(
        tampered(|c| {
            c.sectors.insert((id, 1 << 40), PSectorFate::Zero);
        }),
        "sectors, its presentations have",
    );
    assert_malformed(
        tampered(|c| {
            c.sectors.insert((PresentationId(77), 1), PSectorFate::Zero);
        }),
        "sectors, its presentations have",
    );
}

#[test]
fn check_complete_repeats_the_zero_proofs() {
    let (_, census) = three_loop_tops();
    let (sector, class) = last_member(census);
    assert_malformed(
        tampered(|c| {
            c.sectors.insert(sector, PSectorFate::Zero);
        }),
        "without a proof",
    );
    let zero = *(census.sectors.iter().rev())
        .find(|(_, fate)| **fate == PSectorFate::Zero)
        .unwrap()
        .0;
    assert!(census.classes[&class].owner < zero);
    let map = route_of(census, sector);
    assert_malformed(
        tampered(|c| {
            c.sectors.insert(zero, PSectorFate::Owner { class, map });
        }),
        "proved zero",
    );
}

#[test]
fn check_complete_verifies_every_stored_route() {
    let (_, census) = three_loop_tops();
    let (sector, class) = last_member(census);
    let map = route_of(census, sector);
    let with_map = |map: SignedAffineLoopMap| {
        tampered(|c| {
            c.sectors.insert(sector, PSectorFate::Owner { class, map });
        })
    };

    let mut shifted = map.clone();
    shifted.loop_external[0] += 1;
    assert_malformed(with_map(shifted), "does not verify");

    let mut singular = map.clone();
    singular.loop_linear.fill(0);
    assert_malformed(with_map(singular), "does not verify");

    let mut scaled = map.clone();
    scaled.external_sign = 2;
    assert_malformed(with_map(scaled), "does not verify");

    let mut short = map;
    short.loops_in += 1;
    assert_malformed(with_map(short), "does not verify");
}

#[test]
fn check_complete_refuses_routes_into_the_wrong_class() {
    let (_, census) = three_loop_tops();
    let (sector, class) = last_member(census);
    let map = route_of(census, sector);
    // A class that already has an earlier member, so only the key can tell.
    let (&other, _) = (census.classes.iter())
        .find(|(id, record)| **id != class && record.owner < sector)
        .unwrap();
    assert_malformed(
        tampered(|c| {
            c.sectors.insert(
                sector,
                PSectorFate::Owner {
                    class: other,
                    map: map.clone(),
                },
            );
        }),
        "is in class",
    );
    assert_missing_owner(tampered(|c| {
        c.sectors.insert(
            sector,
            PSectorFate::Owner {
                class: OwnerClassId(0xdead),
                map,
            },
        );
    }));
}

#[test]
fn check_complete_refuses_a_class_without_its_owner() {
    let (_, census) = three_loop_tops();
    let (sector, class) = last_member(census);
    let owner = census.classes[&class].owner;
    assert_missing_owner(tampered(|c| {
        c.classes.remove(&class);
    }));
    assert_missing_owner(tampered(|c| {
        c.sectors.insert(owner, PSectorFate::Zero);
    }));

    // The owner holds the identity, and an owner elsewhere does not.
    let shifted = {
        let mut map = route_of(census, owner);
        map.loop_external[0] += 1;
        map
    };
    assert_missing_owner(tampered(|c| {
        c.sectors.insert(
            owner,
            PSectorFate::Owner {
                class,
                map: shifted,
            },
        );
    }));
    assert_missing_owner(tampered(|c| {
        c.classes.get_mut(&class).unwrap().owner = (PresentationId(77), 1);
    }));

    // Another member is no owner by the rule, even with an identity route.
    let loops = census.classes[&class].key.loops;
    assert_malformed(
        tampered(|c| {
            c.classes.get_mut(&class).unwrap().owner = sector;
            c.sectors.insert(
                sector,
                PSectorFate::Owner {
                    class,
                    map: identity(loops),
                },
            );
        }),
        "the rule gives",
    );

    assert_malformed(
        tampered(|c| c.classes.get_mut(&class).unwrap().t += 1),
        "records",
    );
    assert_malformed(
        tampered(|c| c.classes.get_mut(&class).unwrap().node = Some(NodeIndex(1))),
        "the rule gives",
    );
    let record = census.classes[&class].clone();
    assert_malformed(
        tampered(|c| {
            c.classes.remove(&class);
            c.classes.insert(OwnerClassId(class.0 ^ 1), record);
        }),
        "holds the canonical key",
    );
}

#[test]
fn check_complete_refuses_an_owner_away_from_its_min_graph_key_node() {
    let (library, census) = three_loop_library();
    let (dag, ps) = (&library.dag, &library.presentations);
    let (class, record) = (census.classes.iter())
        .find(|(_, record)| record.nodes.len() > 1)
        .expect("a class with two nodes");
    let other = *(record.nodes.iter())
        .find(|node| Some(**node) != record.node)
        .unwrap();
    let presentation = ps
        .iter()
        .find(|p| p.node.as_ref().is_some_and(|r| r.index == other))
        .unwrap();
    let support = lines(presentation)
        .iter()
        .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);

    let mut moved = census.clone();
    let entry = moved.classes.get_mut(class).unwrap();
    entry.owner = (presentation.id, support);
    entry.node = Some(other);
    moved.sectors.insert(
        (presentation.id, support),
        PSectorFate::Owner {
            class: *class,
            map: identity(record.key.loops),
        },
    );
    assert_malformed(moved.check_complete(ps, Some(dag)), "the rule gives");

    let mut short = census.clone();
    short.classes.get_mut(class).unwrap().nodes.pop();
    assert_malformed(short.check_complete(ps, Some(dag)), "the rule gives");

    // A census without node data is not the census of the DAG, and conversely.
    let (tops, tops_census) = three_loop_tops();
    assert_malformed(census.check_complete(ps, None), "the rule gives");
    assert_malformed(
        tops_census.check_complete(tops, Some(dag)),
        "no presentation",
    );
}

#[test]
fn the_census_refuses_two_presentations_of_one_id_and_foreign_contexts() {
    let tops = top_presentations(2);
    let doubled = vec![Arc::clone(&tops[0]), Arc::clone(&tops[0])];
    assert_malformed(p_census(&doubled, None, &options()), "two presentations");
    let census = PCensus::default();
    assert_malformed(census.check_complete(&doubled, None), "two presentations");

    let foreign = CoefficientContext::try_new(["d", "x"]).unwrap();
    let mut other = (*tops[0]).clone();
    other.id = PresentationId(50);
    other.family = Arc::new(rebuilt_family(&other, &foreign));
    let mixed = vec![Arc::clone(&tops[0]), Arc::new(other)];
    assert_eq!(
        p_census(&mixed, None, &options()).unwrap_err(),
        PError::ContextMismatch
    );
}

/// The family of `p` with its coefficients in `context`.
fn rebuilt_family(
    p: &Presentation,
    context: &CoefficientContext,
) -> rustred::family::IntegralFamily {
    let family = &p.family;
    let integer = |value: &rustred::algebra::Coefficient| -> rustred::algebra::Coefficient {
        let number = value.numerator.get_constant().to_i64().unwrap();
        context.integer(number)
    };
    rustred::family::IntegralFamily::new(
        family.name(),
        family.loop_momenta().to_vec(),
        family.external_momenta().to_vec(),
        context.clone(),
        context.parameter("d").unwrap(),
        (family.denominators().iter())
            .map(|denominator| {
                rustred::family::AffineDenominator::new(
                    integer(denominator.constant()),
                    denominator.coefficients().iter().map(integer).collect(),
                )
            })
            .collect(),
        vec![vec![context.integer(-1)]],
        family.power_shifts().iter().map(integer).collect(),
    )
    .unwrap()
}

#[test]
fn a_presentation_that_differs_from_its_family_is_refused() {
    let mut copy = (*top_presentations(1)[0]).clone();
    let Slot::Line(line) = &mut copy.slots[0] else {
        panic!("slot 0 is a line");
    };
    line.momentum[0] += 1;
    let error = p_census(&[Arc::new(copy)], None, &options()).unwrap_err();
    assert!(matches!(error, PError::Contract(_)), "{error:?}");
}

#[test]
fn a_family_too_wide_to_enumerate_is_refused() {
    // k_i, k_i + k_j, k_i + q: six loops, 27 lines, all of them squares.
    let width = 7;
    let unit = |position: usize| {
        let mut row = vec![0_i64; width];
        row[position] = 1;
        row
    };
    let mut rows: Vec<Vec<i64>> = (0..6).map(unit).collect();
    for left in 0..6 {
        for right in left + 1..7 {
            let mut row = unit(left);
            row[right] = 1;
            rows.push(row);
        }
    }
    let slots = rows
        .into_iter()
        .map(|momentum| {
            Slot::Line(SlotLine {
                momentum,
                eps_units: 0,
            })
        })
        .collect();
    let wide = Presentation::from_slots(
        PresentationId(0),
        PresentationKind::EpsNode,
        "census_wide",
        6,
        slots,
        &context(),
        None,
    )
    .unwrap();
    let error = p_census(&[Arc::new(wide)], None, &options()).unwrap_err();
    assert!(
        matches!(&error, PError::Unsupported(detail) if detail.contains("2^27")),
        "{error:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Determinism and JSON
// ---------------------------------------------------------------------------------------------

#[test]
fn the_census_does_not_depend_on_its_threads() {
    let (tops, census) = three_loop_tops();
    for threads in [1, 3] {
        let other = PCensusOptions {
            threads,
            ..PCensusOptions::default()
        };
        assert_eq!(p_census(tops, None, &other).unwrap(), *census);
    }
}

/// A change of fates, owners or routes must come with a bump of the version that
/// caused it, which renames the cache identity of every library program. Re-pin a
/// digest only together with the versions next to it.
#[test]
fn the_census_content_is_pinned_to_its_versions() {
    const TOPS: ((u32, u32), &str) = (
        (1, 1),
        "8b7b5e98a5386391136b95c3a8eea237fbbb336daa06be7226be6b5d3942efdd",
    );
    const LIBRARY: ((u32, u32, u32, u32), &str) = (
        (1, 1, 1, 1),
        "fa144b41a89169ae5e0bf99401414205b274015b738290489150461dbc5a2b98",
    );
    let digest = |text: String| blake3::hash(text.as_bytes()).to_hex().to_string();

    let (tops, census) = three_loop_tops();
    let versions = (P_CENSUS_VERSION, P_CLASS_KEY_VERSION);
    let found = digest(census.to_json(tops).unwrap());
    assert_eq!((versions, found.as_str()), TOPS);

    let (library, census) = three_loop_library();
    let versions = (
        P_CENSUS_VERSION,
        P_CLASS_KEY_VERSION,
        P_PRESENTATION_VERSION,
        P_TOPOLOGY_VERSION,
    );
    let found = digest(census.to_json(&library.presentations).unwrap());
    assert_eq!((versions, found.as_str()), LIBRARY);
}

#[test]
fn the_json_form_round_trips() {
    let (library, census) = three_loop_library();
    let (dag, ps) = (&library.dag, &library.presentations);
    let text = census.to_json(ps).unwrap();
    let parsed = PCensus::from_json(&text).unwrap();
    assert_eq!(parsed, *census);
    assert_eq!(parsed.to_json(ps).unwrap(), text);
    parsed.check_complete(ps, Some(dag)).unwrap();

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], P_CENSUS_SCHEMA);
    let labels: BTreeSet<_> = (value["classes"].as_array().unwrap().iter())
        .map(|class| class["label"].as_str().unwrap().to_owned())
        .collect();
    let expected: BTreeSet<_> = census.classes.values().map(|r| r.key.label()).collect();
    assert_eq!(labels, expected);
    assert_eq!(
        value["counts"].as_array().unwrap().len(),
        census.counts(ps).unwrap().len()
    );
}

type JsonEdit<'a> = &'a dyn Fn(&mut serde_json::Value);

#[test]
fn the_json_form_refuses_what_disagrees_with_itself() {
    let (tops, census) = three_loop_tops();
    let text = census.to_json(tops).unwrap();
    let edited = |change: &dyn Fn(&mut serde_json::Value)| -> String {
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        change(&mut value);
        value.to_string()
    };
    let version = |name: &str| match PCensus::from_json(&edited(&|v| {
        v[name] = serde_json::json!(999);
    })) {
        Err(PError::VersionMismatch { found, .. }) => assert_eq!(found, "999"),
        other => panic!("{other:?}"),
    };
    version("census_version");
    version("class_key_version");
    assert!(matches!(
        PCensus::from_json(&edited(&|v| v["schema"] = "other".into())),
        Err(PError::VersionMismatch { .. })
    ));

    let routed = |value: &mut serde_json::Value| -> usize {
        (value["sectors"].as_array().unwrap().iter())
            .position(|row| row.get("class").is_some())
            .unwrap()
    };
    let refusals: &[(JsonEdit, &str)] = &[
        (&|v| v["counts"][0]["classes"] = 99.into(), "counts"),
        (&|v| v["unknown"] = 1.into(), "unknown field"),
        (
            &|v| v["classes"][0]["id"] = "0000000000000000".into(),
            "holds the key",
        ),
        (
            &|v| v["classes"][0]["canonical"] = "zz".into(),
            "canonical bytes",
        ),
        (
            &|v| v["classes"][0]["canonical"] = "00".into(),
            "canonical bytes",
        ),
        (
            &|v| {
                let row = v["classes"][0].clone();
                v["classes"].as_array_mut().unwrap().push(row);
            },
            "appears twice",
        ),
        (
            &|v| {
                let row = v["sectors"][0].clone();
                v["sectors"].as_array_mut().unwrap().push(row);
            },
            "appears twice",
        ),
        (
            &|v| {
                let at = routed(v);
                v["sectors"][at].as_object_mut().unwrap().remove("map");
            },
            "a class needs a map",
        ),
        (
            &|v| {
                let at = routed(v);
                v["sectors"][at]["class"] = "not hex".into();
            },
            "class id",
        ),
    ];
    for (change, needle) in refusals {
        assert_malformed(PCensus::from_json(&edited(change)), needle);
    }
    assert_malformed(PCensus::from_json("{"), "census JSON");
}

// ---------------------------------------------------------------------------------------------
// Four loops
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "slow: 22,528 four-loop sectors and 4,519 routes"]
fn the_four_loop_tops_are_complete() {
    let tops: Vec<_> = (top_presentations(4).into_iter())
        .filter(|p| p.loops == 4)
        .collect();
    let start = Instant::now();
    let census = p_census(&tops, None, &options()).unwrap();
    let census_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    census.check_complete_with(&tops, None, &options()).unwrap();
    println!(
        "4L tops: census {census_seconds:.1} s, check {:.1} s, {:?}",
        start.elapsed().as_secs_f64(),
        census.counts(&tops).unwrap()
    );
    assert_eq!(
        count_rows(&census.counts(&tops).unwrap()),
        [(4, 11, 22_528, 18_009, 4_519, 183)]
    );
    assert_eq!(assert_all_routes(&tops, &census), 4_519);
}

#[test]
#[ignore = "slow: the 4L tops and all 437 node presentations"]
fn the_four_loop_dag_is_complete() {
    let library = production_library(4);
    assert_eq!(library.dag.nodes.len(), 438);
    let start = Instant::now();
    let census = p_census(&library.presentations, Some(&library.dag), &options()).unwrap();
    let census_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    assert_dag_link(&library, &census);
    println!(
        "4L DAG: census {census_seconds:.1} s, check {:.1} s, {:?}",
        start.elapsed().as_secs_f64(),
        census.counts(&library.presentations).unwrap()
    );
    // 437 nodes with loops on 310 classes; eps nodes end at 3L, so the 4L classes
    // are the 183 of the tops.
    assert_eq!(
        count_rows(&census.counts(&library.presentations).unwrap()),
        [
            (1, 7, 16, 9, 7, 6),
            (2, 30, 298, 211, 87, 29),
            (3, 125, 8_216, 6_794, 1_422, 92),
            (4, 291, 176_608, 156_045, 20_563, 183),
        ]
    );
}
