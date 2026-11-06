//! Owner-class keys, classify and routes (owner: M2-T1 rr-class).
//!
//! Vacuum-style census checks on the shipped top families: class counts per
//! loop order, golden class ids, keys that do not see the routing, verified
//! routes between all class members, Whitney twists, the agreement with the
//! graph-side marked-matroid key, and the eps colours with their shift checks.
//!
//! The four-loop partition is slow:
//!
//! ```text
//! cargo test --release -p rustred --test p_library class:: -- --ignored
//! ```
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::CoefficientContext;
use rustred::propagator::PresentationId;
use rustred::propagator::census::SignedAffineLoopMap;
use rustred::propagator::class::{
    ClassLimits, ClassedSector, OwnerClassKey, classify, route, verify_route,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamilyEntry, load_p_families};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use rustred::reduction::terminal_normalization::TerminalSectorSymmetryEquations;
use rustred::sector::Mask;
use rustred::sector::symmetry::{
    DiscoveryError, SectorSymmetryCensus, SectorSymmetryCensusControl, SectorSymmetryCensusError,
    SectorSymmetryCensusOptions,
};
use rustred::sector::zero::{Analyzer, Decision};
use symbolica::graph::Graph;

use crate::support::p_integrals::{BE, BUBBLE, KITE, LA, NO, PFamily, compile};

/// The label of every 1L-3L class, sorted. A change here without a bump of
/// `P_CLASS_KEY_VERSION` is a bug: see the module docs of `propagator::class`.
const GOLDEN_CLASS_LABELS: &[&str] = &[
    "L1T02-64f4fabd2064500f",
    "L2T03-61b48e1159bdb6a2",
    "L2T04-61db0237c59ba9ce",
    "L2T04-ce0d35105dae83c5",
    "L2T05-001e279c3d892daf",
    "L3T04-df5f8c40d260746b",
    "L3T05-15bcfc1aa5546a43",
    "L3T05-2664f4a583ec888b",
    "L3T05-e3f6c3ad46ce9235",
    "L3T05-fe625a5ba26905ae",
    "L3T06-00c99db10f58faf6",
    "L3T06-5f7155fa1502296f",
    "L3T06-8372ef319969bb99",
    "L3T06-949012bf62f9cbd0",
    "L3T06-9dc5bd7ab7fc182c",
    "L3T06-9ec13b2813d25d62",
    "L3T06-a547ee63a298f5d7",
    "L3T06-c7316f3d1aaa67ba",
    "L3T07-1881c6d1a44ffe19",
    "L3T07-28806d3e18a52b2c",
    "L3T07-4f3f489a027f1572",
    "L3T07-72a95edcbcc7d714",
    "L3T07-bedbf189db8fabfc",
    "L3T07-c0a9514dc89b5320",
    "L3T08-61cd69124cf81eb3",
    "L3T08-96f2cf2d8f71f4fe",
    "L3T08-999162f8af475162",
];

fn limits() -> ClassLimits {
    ClassLimits::default()
}

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn tops(loops: u8) -> Vec<(PFamilyEntry, Arc<Presentation>)> {
    let (file, presentations) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    file.families
        .into_iter()
        .zip(presentations)
        .filter(|(entry, _)| entry.loops == loops)
        .collect()
}

fn top_presentations(loops: u8) -> Vec<Arc<Presentation>> {
    tops(loops).into_iter().map(|(_, p)| p).collect()
}

fn line(momentum: &[i64], eps_units: u8) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units,
    })
}

fn presentation(id: u32, loops: u8, slots: Vec<Slot>) -> Presentation {
    Presentation::from_slots(
        PresentationId(id),
        PresentationKind::EpsNode,
        &format!("class_test_{id}"),
        loops,
        slots,
        &context(),
        None,
    )
    .unwrap()
}

/// The same family in the routing of `tests/support/p_integrals.rs`.
fn support_routing(id: u32, family: &PFamily) -> Presentation {
    let slots = family
        .lines
        .iter()
        .zip(family.root)
        .map(|(row, &active)| {
            if active {
                line(row, 0)
            } else {
                Slot::Isp(SlotIsp {
                    left: row.to_vec(),
                    right: row.to_vec(),
                })
            }
        })
        .collect();
    presentation(id, family.loops as u8, slots)
}

/// The presentation `map` takes `p` to: `k = A k' + b q'`, `q = s q'`.
fn rerouted(id: u32, p: &Presentation, map: &SignedAffineLoopMap) -> Presentation {
    let image = |row: &[i64]| map.map_momentum(row).unwrap();
    let slots = p
        .slots
        .iter()
        .map(|slot| match slot {
            Slot::Line(l) => line(&image(&l.momentum), l.eps_units),
            Slot::Isp(isp) => Slot::Isp(SlotIsp {
                left: image(&isp.left),
                right: image(&isp.right),
            }),
        })
        .collect();
    presentation(id, p.loops, slots)
}

fn identity_map(loops: u8) -> SignedAffineLoopMap {
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

/// A product of random shears and sign flips of the loop momenta, with a
/// random shift by `q` and a random sign of `q`.
fn random_map(rng: &mut StdRng, loops: u8) -> SignedAffineLoopMap {
    let size = usize::from(loops);
    let mut map = identity_map(loops);
    for _ in 0..3 * size {
        let (row, source) = (rng.random_range(0..size), rng.random_range(0..size));
        let sign = if rng.random() { 1 } else { -1 };
        for column in 0..size {
            if row == source {
                map.loop_linear[row * size + column] *= sign;
            } else {
                map.loop_linear[row * size + column] +=
                    sign * map.loop_linear[source * size + column];
            }
        }
        map.loop_external[row] += rng.random_range(-1..=1);
    }
    map.external_sign = if rng.random() { 1 } else { -1 };
    assert!(map.is_unimodular());
    map
}

fn line_slots(p: &Presentation) -> Vec<(usize, &SlotLine)> {
    p.slots
        .iter()
        .enumerate()
        .filter_map(|(slot, entry)| match entry {
            Slot::Line(line) => Some((slot, line)),
            Slot::Isp(_) => None,
        })
        .collect()
}

fn mask(p: &Presentation, raw: u64) -> Mask {
    Mask::try_new((0..p.slots.len()).map(|slot| raw >> slot & 1 == 1)).unwrap()
}

/// The raw masks of `p` the zero analyzer does not prove zero.
fn nonzero_sectors(p: &Presentation) -> Vec<u64> {
    let analyzer = Analyzer::try_unrestricted(&p.family).unwrap();
    (1..1_u64 << line_slots(p).len())
        .filter(|&raw| {
            matches!(
                analyzer.analyze(&mask(p, raw)).unwrap(),
                Decision::Inconclusive(_)
            )
        })
        .collect()
}

fn classed(p: &Presentation, raw: u64) -> ClassedSector {
    classify(p, raw, &limits()).unwrap().unwrap()
}

/// The nonzero labelled sectors of `presentations` by class.
fn classes(presentations: &[Arc<Presentation>]) -> BTreeMap<OwnerClassKey, Vec<ClassedSector>> {
    let mut classes: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for p in presentations {
        for raw in nonzero_sectors(p) {
            let sector = classed(p, raw);
            classes.entry(sector.key.clone()).or_default().push(sector);
        }
    }
    classes
}

fn sector_count(classes: &BTreeMap<OwnerClassKey, Vec<ClassedSector>>) -> usize {
    classes.values().map(Vec::len).sum()
}

/// `map` takes the support of `source` line by line onto the support of
/// `owner`, up to sign, and keeps every eps unit.
fn assert_maps_lines(
    source_p: &Presentation,
    source: &ClassedSector,
    owner_p: &Presentation,
    owner: &ClassedSector,
    map: &SignedAffineLoopMap,
) {
    assert!(map.is_unimodular());
    let owner_lines = line_slots(owner_p);
    let mut images = BTreeSet::new();
    for (slot, line) in line_slots(source_p) {
        if source.support >> slot & 1 == 0 {
            continue;
        }
        let image = map.map_momentum(&line.momentum).unwrap();
        let negated: Vec<i64> = image.iter().map(|c| -c).collect();
        let (target, target_line) = owner_lines
            .iter()
            .find(|(target, candidate)| {
                owner.support >> target & 1 == 1
                    && (candidate.momentum == image || candidate.momentum == negated)
            })
            .unwrap_or_else(|| panic!("line {slot} of {source:?} has no image"));
        assert_eq!(target_line.eps_units, line.eps_units, "line {slot}");
        assert!(images.insert(*target));
    }
    assert_eq!(images.len() as u32, owner.support.count_ones());
}

#[test]
fn classes_of_the_one_and_two_loop_tops() {
    let bubble = classes(&top_presentations(1));
    assert_eq!((bubble.len(), sector_count(&bubble)), (1, 1));
    let kite = classes(&top_presentations(2));
    assert_eq!((kite.len(), sector_count(&kite)), (4, 8));
    let sizes: BTreeMap<_, Vec<_>> =
        kite.iter()
            .fold(BTreeMap::new(), |mut sizes, (key, members)| {
                sizes
                    .entry(key.lines)
                    .or_insert_with(Vec::new)
                    .push(members.len());
                sizes
            });
    // The top, the four triangles, then the two sunsets and the bubble product.
    assert_eq!(sizes[&5], [1]);
    assert_eq!(sizes[&4].iter().sum::<usize>(), 5);
    assert_eq!(sizes[&3], [2]);
}

#[test]
fn classes_of_the_three_loop_tops() {
    let tops = top_presentations(3);
    let per_top: Vec<_> = tops
        .iter()
        .map(|p| classes(std::slice::from_ref(p)).len())
        .collect();
    assert_eq!(per_top, [16, 18, 14]);
    let all = classes(&tops);
    assert_eq!((all.len(), sector_count(&all)), (22, 175));
}

#[test]
fn golden_class_ids_are_stable_and_distinct() {
    let mut labels = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for loops in 1..=3 {
        for key in classes(&top_presentations(loops)).keys() {
            assert!(ids.insert(key.id()), "id collision at {}", key.label());
            assert_eq!(key.loops, loops);
            assert_eq!(key.eps_lines, 0);
            labels.insert(key.label());
        }
    }
    assert_eq!(ids.len(), 27);
    assert_eq!(
        labels.iter().map(String::as_str).collect::<Vec<_>>(),
        GOLDEN_CLASS_LABELS
    );
}

#[test]
fn keys_agree_across_the_two_routings_of_a_family() {
    let pairs = [
        ("P1L", &BUBBLE, 1),
        ("P2L", &KITE, 2),
        ("P3L_LA", &LA, 3),
        ("P3L_BE", &BE, 3),
        ("P3L_NO", &NO, 3),
    ];
    for (name, family, loops) in pairs {
        let (_, top) = tops(loops)
            .into_iter()
            .find(|(entry, _)| entry.name == name)
            .unwrap();
        let other = Arc::new(support_routing(99, family));
        let mine = classes(std::slice::from_ref(&top));
        let theirs = classes(std::slice::from_ref(&other));
        assert_eq!(
            mine.keys().collect::<Vec<_>>(),
            theirs.keys().collect::<Vec<_>>(),
            "{name}"
        );
        for (key, members) in &mine {
            let map = route(&members[0], &theirs[key][0], &limits())
                .unwrap()
                .unwrap_or_else(|| panic!("{name}: {} has no route", key.label()));
            assert_maps_lines(&top, &members[0], &other, &theirs[key][0], &map);
        }
    }
}

#[test]
fn keys_do_not_see_q_reversal() {
    for loops in 1..=3 {
        for p in top_presentations(loops) {
            let map = SignedAffineLoopMap {
                external_sign: -1,
                ..identity_map(loops)
            };
            let reversed = rerouted(99, &p, &map);
            for raw in nonzero_sectors(&p) {
                let (source, owner) = (classed(&p, raw), classed(&reversed, raw));
                assert_eq!(source.key, owner.key);
                assert_eq!(
                    verify_route(&source, &owner, &map, &limits()).unwrap(),
                    Some(map.clone())
                );
                let found = route(&source, &owner, &limits()).unwrap().unwrap();
                assert_maps_lines(&p, &source, &reversed, &owner, &found);
            }
        }
    }
}

#[test]
fn keys_do_not_see_random_routings() {
    let mut rng = StdRng::seed_from_u64(0x7031);
    for loops in 1..=3 {
        for p in top_presentations(loops) {
            let sectors = nonzero_sectors(&p);
            for _ in 0..8 {
                let map = random_map(&mut rng, loops);
                let other = rerouted(99, &p, &map);
                let raw = sectors[rng.random_range(0..sectors.len())];
                let (source, owner) = (classed(&p, raw), classed(&other, raw));
                assert_eq!(source.key, owner.key);
                assert_eq!(
                    verify_route(&source, &owner, &map, &limits()).unwrap(),
                    Some(map.clone())
                );
                let found = route(&source, &owner, &limits()).unwrap().unwrap();
                assert_maps_lines(&p, &source, &other, &owner, &found);
            }
        }
    }
}

#[test]
fn every_member_routes_onto_its_class_representative() {
    for loops in 1..=3 {
        let tops = top_presentations(loops);
        let of = |sector: &ClassedSector| {
            let id = sector.presentation;
            Arc::clone(tops.iter().find(|p| p.id == id).unwrap())
        };
        for (key, members) in classes(&tops) {
            let owner = &members[0];
            for member in &members {
                let map = route(member, owner, &limits())
                    .unwrap()
                    .unwrap_or_else(|| panic!("{}: {member:?} has no route", key.label()));
                assert_maps_lines(&of(member), member, &of(owner), owner, &map);
            }
        }
    }
}

/// `p_census` may classify and route on several threads.
#[test]
fn classed_sectors_cross_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<ClassedSector>();
}

/// A sector as a graph: the absent lines are contracted.
struct SectorGraph {
    vertices: usize,
    edges: Vec<(usize, usize)>,
    external: (usize, usize),
}

fn find(parents: &mut [usize], mut vertex: usize) -> usize {
    while parents[vertex] != vertex {
        vertex = parents[vertex];
    }
    vertex
}

impl SectorGraph {
    fn new(entry: &PFamilyEntry, raw: u64) -> Self {
        let vertices = usize::from(entry.vertices);
        let mut parents: Vec<usize> = (0..vertices).collect();
        let vertex = |v: u8| usize::from(v) - 1;
        for (slot, line) in entry.lines.iter().enumerate() {
            if raw >> slot & 1 == 0 {
                let a = find(&mut parents, vertex(line.endpoints[0]));
                let b = find(&mut parents, vertex(line.endpoints[1]));
                parents[a] = b;
            }
        }
        let edges = entry
            .lines
            .iter()
            .enumerate()
            .filter(|(slot, _)| raw >> slot & 1 == 1)
            .map(|(_, line)| {
                (
                    find(&mut parents, vertex(line.endpoints[0])),
                    find(&mut parents, vertex(line.endpoints[1])),
                )
            })
            .collect();
        let external = (
            find(&mut parents, vertex(entry.external.incoming)),
            find(&mut parents, vertex(entry.external.outgoing)),
        );
        Self {
            vertices,
            edges,
            external,
        }
    }

    /// The graph key: the sector with its two external vertices coloured.
    fn graph_key(&self) -> Graph<u8, u8> {
        let mut graph = Graph::new();
        let (s, t) = self.external;
        let nodes: BTreeMap<_, _> = self
            .edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .chain([s, t])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|v| (v, graph.add_node(u8::from(v == s || v == t))))
            .collect();
        for &(a, b) in &self.edges {
            graph.add_edge(nodes[&a], nodes[&b], false, 0).unwrap();
        }
        graph.canonize().graph
    }

    /// The graph-side marked-matroid key: the element-circuit incidence graph
    /// of `Gamma + e0` with `e0` coloured (notes/forcer-tricks-graph-tools.md,
    /// `SCRATCHPAD/forcer-tricks/graph-tools/bench/src/bin/matroid.rs`).
    fn matroid_key(&self) -> Graph<u8, u8> {
        let glued: Vec<_> = self.edges.iter().copied().chain([self.external]).collect();
        let circuits = self.circuits(&glued);
        let e0 = glued.len() - 1;
        let mut graph = Graph::new();
        let elements: Vec<_> = (0..glued.len())
            .map(|element| graph.add_node(if element == e0 { 2 } else { 0 }))
            .collect();
        for circuit in circuits {
            let vertex = graph.add_node(1);
            for (element, &node) in elements.iter().enumerate() {
                if circuit >> element & 1 == 1 {
                    graph.add_edge(vertex, node, false, 0).unwrap();
                }
            }
        }
        graph.canonize().graph
    }

    /// The circuits of `edges` as element masks: the cycle-space elements in
    /// which every vertex has degree 0 or 2 and the edges are connected.
    fn circuits(&self, edges: &[(usize, usize)]) -> Vec<u64> {
        let basis = self.fundamental_cycles(edges);
        (1_u64..1 << basis.len())
            .map(|subset| {
                basis
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| subset >> position & 1 == 1)
                    .fold(0, |cycle, (_, fundamental)| cycle ^ fundamental)
            })
            .filter(|&cycle| self.is_circuit(edges, cycle))
            .collect()
    }

    fn fundamental_cycles(&self, edges: &[(usize, usize)]) -> Vec<u64> {
        let mut parents: Vec<usize> = (0..self.vertices).collect();
        let mut tree = Vec::new();
        let mut cotree = Vec::new();
        for (element, &(a, b)) in edges.iter().enumerate() {
            let (x, y) = (find(&mut parents, a), find(&mut parents, b));
            if x == y {
                cotree.push(element);
            } else {
                parents[x] = y;
                tree.push(element);
            }
        }
        cotree
            .into_iter()
            .map(|element| {
                let (a, b) = edges[element];
                1_u64 << element ^ self.tree_path(edges, &tree, a, b)
            })
            .collect()
    }

    /// The tree edges from `a` to `b` as an element mask.
    fn tree_path(&self, edges: &[(usize, usize)], tree: &[usize], a: usize, b: usize) -> u64 {
        let mut paths = vec![None; self.vertices];
        paths[a] = Some(0_u64);
        let mut frontier = vec![a];
        while let Some(vertex) = frontier.pop() {
            for &element in tree {
                let (u, v) = edges[element];
                let next = match (u == vertex, v == vertex) {
                    (true, _) => v,
                    (_, true) => u,
                    _ => continue,
                };
                if paths[next].is_none() {
                    paths[next] = paths[vertex].map(|path: u64| path | 1 << element);
                    frontier.push(next);
                }
            }
        }
        paths[b].expect("the endpoints of a cotree edge lie in one tree")
    }

    fn is_circuit(&self, edges: &[(usize, usize)], cycle: u64) -> bool {
        let chosen: Vec<_> = edges
            .iter()
            .enumerate()
            .filter(|(element, _)| cycle >> element & 1 == 1)
            .map(|(_, &edge)| edge)
            .collect();
        let mut degrees = vec![0; self.vertices];
        let mut parents: Vec<usize> = (0..self.vertices).collect();
        for &(a, b) in &chosen {
            degrees[a] += 1;
            degrees[b] += 1;
            let (x, y) = (find(&mut parents, a), find(&mut parents, b));
            parents[x] = y;
        }
        let mut roots = chosen.iter().map(|&(a, _)| find(&mut parents, a));
        let first = roots.next();
        degrees.iter().all(|&degree| degree == 0 || degree == 2) && roots.all(|r| Some(r) == first)
    }
}

/// Graph-side key and projector key induce one partition of the labelled
/// sectors; returns (sectors, classes, graph classes).
fn matroid_partition(loops: u8) -> (usize, usize, usize) {
    let mut by_projector: BTreeMap<OwnerClassKey, Graph<u8, u8>> = BTreeMap::new();
    let mut by_matroid: BTreeMap<Graph<u8, u8>, OwnerClassKey> = BTreeMap::new();
    let mut graphs = BTreeSet::new();
    let mut sectors = 0;
    for (entry, p) in tops(loops) {
        for raw in nonzero_sectors(&p) {
            let key = classed(&p, raw).key;
            let graph = SectorGraph::new(&entry, raw);
            let matroid = graph.matroid_key();
            graphs.insert(graph.graph_key());
            sectors += 1;
            assert_eq!(
                *by_projector.entry(key.clone()).or_insert(matroid.clone()),
                matroid,
                "{}: a projector class with two matroid keys",
                entry.name
            );
            assert_eq!(
                *by_matroid.entry(matroid).or_insert(key.clone()),
                key,
                "{}: a matroid class with two projector keys",
                entry.name
            );
        }
    }
    (sectors, by_projector.len(), graphs.len())
}

#[test]
fn matroid_key_partitions_the_three_loop_sectors_like_the_projector_key() {
    assert_eq!(matroid_partition(3), (175, 22, 27));
}

#[test]
#[ignore = "slow: classifies the 22,528 labelled four-loop sectors"]
fn matroid_key_partitions_the_four_loop_sectors_like_the_projector_key() {
    assert_eq!(matroid_partition(4), (4_519, 183, 280));
}

/// Sectors of one class with non-isomorphic graphs are Whitney twists.
#[test]
fn whitney_twisted_sectors_share_a_class_and_route() {
    let tops = tops(3);
    let mut pairs = 0;
    let mut by_class: BTreeMap<OwnerClassKey, BTreeMap<Graph<u8, u8>, usize>> = BTreeMap::new();
    let mut sectors = Vec::new();
    for (index, (entry, p)) in tops.iter().enumerate() {
        for raw in nonzero_sectors(p) {
            let sector = classed(p, raw);
            let graph = SectorGraph::new(entry, raw).graph_key();
            let members = by_class.entry(sector.key.clone()).or_default();
            if let Some(&first) = members.values().next()
                && !members.contains_key(&graph)
            {
                let (owner_p, owner): &(usize, ClassedSector) = &sectors[first];
                let map = route(&sector, owner, &limits()).unwrap().unwrap();
                assert_maps_lines(p, &sector, &tops[*owner_p].1, owner, &map);
                pairs += 1;
            }
            members.entry(graph).or_insert(sectors.len());
            sectors.push((index, sector));
        }
    }
    // 27 graph classes in 22 marked-matroid classes.
    assert_eq!(pairs, 5);
}

/// The kite of the T001c family; `eps` = units per line.
fn eps_kite(id: u32, eps: [u8; 5]) -> Presentation {
    let lines: [&[i64]; 5] = [
        &[1, 0, 0],
        &[-1, -1, 0],
        &[0, 1, 0],
        &[1, 1, -1],
        &[0, -1, 1],
    ];
    let slots = lines
        .iter()
        .zip(eps)
        .map(|(row, units)| line(row, units))
        .collect();
    presentation(id, 2, slots)
}

/// `k1 = -k1'`, `k2 = q' - k2'`: on the kite lines `(D2 D4)(D3 D5)`.
fn kite_mirror() -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 2,
        loops_out: 2,
        loop_linear: vec![-1, 0, 0, -1].into(),
        loop_external: vec![0, 1].into(),
        external_sign: 1,
    }
}

const TOP: u64 = 0b11111;

#[test]
fn eps_lines_colour_the_class() {
    let integer = classed(&eps_kite(0, [0; 5]), TOP);
    let t001c = classed(&eps_kite(1, [1, 0, 0, 0, 1]), TOP);
    let t013c = classed(&eps_kite(2, [1, 0, 1, 1, 0]), TOP);
    let doubled = classed(&eps_kite(3, [2, 0, 0, 0, 1]), TOP);
    assert_eq!(
        (
            integer.key.eps_lines,
            t001c.key.eps_lines,
            t013c.key.eps_lines
        ),
        (0, 2, 3)
    );
    let keys = BTreeSet::from([integer.key, t001c.key, t013c.key, doubled.key]);
    assert_eq!(keys.len(), 4);
    let ids: BTreeSet<_> = keys.iter().map(OwnerClassKey::id).collect();
    assert_eq!(ids.len(), 4);
}

/// `Gamma + e0` is `K4` and keeps a Klein four-group: the pairs of outer lines
/// fall into three orbits, told apart by the colours alone.
#[test]
fn eps_positions_colour_the_class() {
    let key = |eps: [u8; 5]| classed(&eps_kite(0, eps), TOP).key;
    let at_external_vertex = key([0, 1, 0, 1, 0]);
    let at_inner_vertex = key([0, 1, 1, 0, 0]);
    let opposite = key([0, 1, 0, 0, 1]);
    assert_eq!(at_external_vertex, key([0, 0, 1, 0, 1]));
    assert_eq!(at_inner_vertex, key([0, 0, 0, 1, 1]));
    assert_eq!(opposite, key([0, 0, 1, 1, 0]));
    let orbits = BTreeSet::from([at_external_vertex, at_inner_vertex, opposite]);
    assert_eq!(orbits.len(), 3);
}

/// Pins the bytes of the shift colours, as the integer list pins the rest.
#[test]
fn golden_eps_labels_are_stable() {
    let label = |eps: [u8; 5]| classed(&eps_kite(0, eps), TOP).key.label();
    assert_eq!(label([1, 0, 0, 0, 1]), "L2T05-bf1266160e2a5d31");
    assert_eq!(label([1, 0, 1, 1, 0]), "L2T05-ee71e46d031494d4");
}

/// The bubble top: three rows, each with a centre and two orientations.
#[test]
fn canonical_bytes_follow_the_documented_format() {
    let bubble = presentation(0, 1, vec![line(&[1, 0], 0), line(&[1, -1], 0)]);
    let key = classed(&bubble, 0b11).key;
    let bytes = &key.canonical;
    assert_eq!(bytes[..3], [1, 2, 0]);
    let word = |at: usize| usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    let (vertices, edges) = (word(3), word(5));
    assert_eq!(vertices, 9);
    let mut at = 7;
    let mut tags = Vec::new();
    let skip_text = |at: &mut usize| *at += 2 + word(*at);
    for _ in 0..vertices {
        tags.push(bytes[at]);
        at += 1;
        match tags.last().unwrap() {
            0 => (0..4).for_each(|_| skip_text(&mut at)),
            1 => {
                at += 2;
                skip_text(&mut at);
            }
            _ => {}
        }
    }
    let count = |tag| tags.iter().filter(|&&t| t == tag).count();
    assert_eq!((count(0), count(1), count(2)), (2, 1, 6));
    let mut order = Vec::new();
    for _ in 0..edges {
        order.push((word(at), word(at + 2)));
        at += 5;
        if bytes[at - 1] == 1 {
            skip_text(&mut at);
        }
    }
    assert_eq!(at, bytes.len());
    assert!(order.is_sorted());
}

#[test]
fn raw_charts_of_one_support_share_their_class() {
    let t001c = eps_kite(0, [1, 0, 0, 0, 1]);
    let top = classed(&t001c, TOP);
    assert_eq!((top.raw, top.support), (TOP, TOP));
    for chart in [0b01111, 0b11110, 0b01110] {
        let sector = classed(&t001c, chart);
        assert_eq!((sector.raw, sector.support), (chart, TOP));
        assert_eq!(sector.key, top.key);
        assert_eq!(sector.key.lines, 5);
    }
    let chart = classed(&t001c, 0b01110);
    let map = route(&chart, &top, &limits()).unwrap().unwrap();
    assert_maps_lines(&t001c, &chart, &t001c, &top, &map);
    let sub = classed(&t001c, 0b10111);
    assert_eq!(sub.support, 0b10111);
    assert_ne!(sub.key, top.key);
}

#[test]
fn routes_map_eps_lines_onto_equal_eps_lines() {
    let source_p = eps_kite(0, [1, 0, 0, 0, 1]);
    // The mirror image: D5 -> D3.
    let owner_p = eps_kite(1, [1, 0, 1, 0, 0]);
    let (source, owner) = (classed(&source_p, TOP), classed(&owner_p, TOP));
    assert_eq!(source.key, owner.key);
    let map = route(&source, &owner, &limits()).unwrap().unwrap();
    assert_maps_lines(&source_p, &source, &owner_p, &owner, &map);
    assert_eq!(
        verify_route(&source, &owner, &kite_mirror(), &limits()).unwrap(),
        Some(kite_mirror())
    );
    // A random routing of the same family keeps the colours on the lines.
    let mut rng = StdRng::seed_from_u64(0xe25);
    for _ in 0..16 {
        let reroute = random_map(&mut rng, 2);
        let other_p = rerouted(2, &source_p, &reroute);
        let other = classed(&other_p, TOP);
        assert_eq!(other.key, source.key);
        let map = route(&source, &other, &limits()).unwrap().unwrap();
        assert_maps_lines(&source_p, &source, &other_p, &other, &map);
    }
}

#[test]
fn a_shift_mismatching_candidate_is_rejected() {
    let t001c = eps_kite(0, [1, 0, 0, 0, 1]);
    let top = classed(&t001c, TOP);
    // A symmetry of the integer kite: the class has a route, but this
    // candidate takes the forced D5 onto the integer D3 and back.
    let integer = eps_kite(1, [0; 5]);
    let plain = classed(&integer, TOP);
    assert_eq!(
        verify_route(&plain, &plain, &kite_mirror(), &limits()).unwrap(),
        Some(kite_mirror())
    );
    assert_eq!(
        verify_route(&top, &top, &kite_mirror(), &limits()).unwrap(),
        None
    );
    assert!(route(&top, &top, &limits()).unwrap().is_some());
    // The same geometry with other colours has no route at all.
    assert_eq!(route(&top, &plain, &limits()).unwrap(), None);
    assert_eq!(
        verify_route(&top, &plain, &identity_map(2), &limits()).unwrap(),
        None
    );
    assert_eq!(
        verify_route(&plain, &top, &identity_map(2), &limits()).unwrap(),
        None
    );
    // Two eps lines on both sides, on different lines.
    let other = classed(&eps_kite(3, [0, 1, 0, 1, 0]), TOP);
    assert_eq!(route(&top, &other, &limits()).unwrap(), None);
    // Equal colours on the lines but one unit more on one of them.
    let doubled = classed(&eps_kite(2, [2, 0, 0, 0, 1]), TOP);
    assert_eq!(route(&top, &doubled, &limits()).unwrap(), None);
    assert_eq!(
        verify_route(&top, &doubled, &identity_map(2), &limits()).unwrap(),
        None
    );
}

#[test]
fn candidates_outside_the_class_are_rejected() {
    let kite = eps_kite(0, [0; 5]);
    let top = classed(&kite, TOP);
    // Not unimodular.
    let doubled = SignedAffineLoopMap {
        loop_linear: vec![2, 0, 0, 1].into(),
        ..identity_map(2)
    };
    assert_eq!(verify_route(&top, &top, &doubled, &limits()).unwrap(), None);
    // A loop shear: unit Jacobian, but D2 is no line of the target.
    let shear = SignedAffineLoopMap {
        loop_linear: vec![1, 1, 0, 1].into(),
        ..identity_map(2)
    };
    assert_eq!(verify_route(&top, &top, &shear, &limits()).unwrap(), None);
    // q -> 2 q is outside the class of maps.
    let scaled = SignedAffineLoopMap {
        external_sign: 2,
        ..identity_map(2)
    };
    assert_eq!(verify_route(&top, &top, &scaled, &limits()).unwrap(), None);
    // Different loop orders and line counts.
    let bubble = presentation(1, 1, vec![line(&[1, 0], 0), line(&[1, -1], 0)]);
    let one_loop = classed(&bubble, 0b11);
    assert_eq!(route(&top, &one_loop, &limits()).unwrap(), None);
    assert_eq!(
        verify_route(&top, &one_loop, &identity_map(1), &limits()).unwrap(),
        None
    );
}

#[test]
fn classify_refuses_what_is_no_sector() {
    let kite = support_routing(0, &LA);
    // An ISP slot is no line of the sector.
    let isp = 1_u64 << 8;
    assert!(matches!(
        classify(&kite, 0b1111_1111 | isp, &limits()),
        Err(PError::Contract(_))
    ));
    // Rank deficient: one line cannot span three loops.
    assert!(classify(&kite, 0b1, &limits()).unwrap().is_none());
    assert!(classify(&kite, 0, &limits()).unwrap().is_none());
}

#[test]
fn a_shifted_slot_without_a_line_momentum_has_no_class() {
    let family = compile(
        "I(name(shifted_isp),loops(k1),externals(q),dimension(d),\
         prop(D1,k1^2,1),prop(D2,sp(k1,q),0),power_shift(D2,2-d/2),gram(q,q,-1))",
    );
    let p = Presentation {
        id: PresentationId(0),
        kind: PresentationKind::EpsNode,
        loops: 1,
        slots: vec![
            line(&[1, 0], 0),
            Slot::Isp(SlotIsp {
                left: vec![1, 0],
                right: vec![0, 1],
            }),
        ],
        family: Arc::new(family),
        node: None,
    };
    assert!(classify(&p, 0b01, &limits()).unwrap().is_none());
}

/// Plan D10: eps families enter through `classify` only, the generic discovery
/// consumers refuse them as before.
#[test]
fn generic_discovery_consumers_refuse_eps_families() {
    let p = eps_kite(0, [1, 0, 0, 0, 1]);
    let census = SectorSymmetryCensus::discover(
        Arc::clone(&p.family),
        [mask(&p, TOP)],
        SectorSymmetryCensusOptions::default(),
        &SectorSymmetryCensusControl::default(),
    );
    assert!(matches!(
        census,
        Err(SectorSymmetryCensusError::Discovery(
            DiscoveryError::Invalid(_)
        ))
    ));
    let key = rustred::family::IntegralKey::try_new(vec![1; 5]).unwrap();
    let equations = TerminalSectorSymmetryEquations::prepare(
        Arc::clone(&p.family),
        &BTreeSet::from([key]),
        Default::default(),
    );
    assert!(equations.is_err());
}
