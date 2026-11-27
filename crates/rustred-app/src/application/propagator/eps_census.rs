//! Coverage census of eps-shifted families (`rustred p-eps-census`; skeleton-mode audit, lane sk-census).
//!
//! For a bound `S`, every pair (two-point graph `G` with `L` loops, eps units `u` on its lines) with
//! `L + sum(u) <= S` and whether the package has a presentation for it through the public lookup
//! (`PTopo` with `(u, v, eps_units)` lines -> [`normalise`] -> [`canonical`] -> `GraphKey` -> the
//! presentation whose DAG node has that key), the same path as vakint's `eps_family::find_eps_presentation`.
//!
//! The universe is generated here, independently of the package and of RustRed's reduction DAG:
//! - `G` ranges over the NORMAL FORMS of [`normalise`] with `L` loops: two-point graphs whose glued graph
//!   (`G` + the external line `s`-`t`) is loopless, 2-connected and 3-edge-connected (no line in series
//!   with the external line, no two lines with equal momentum). Vertex degrees in the glued graph are >= 3;
//!   parallel lines (multi-edges: one-loop insertions) are kept, series lines are merged.
//! - They are taken from graphica's graph generator (two external legs, vertices of every degree
//!   `3..=2L+1`, no self-loop, no bridge), keeping the generated graphs that [`normalise`] leaves unchanged.
//! - Cross-check: the tops (normal forms with every glued vertex cubic) and every sector of them (any
//!   subset of lines contracted, normalised, nonzero); the two sets must be equal ("all sectors of the
//!   cubic tops").
//! - `u` ranges over every vector of units `0..=MAX_UNITS_PER_LINE` on the lines of `G` with
//!   `sum(u) <= S - L`, modulo the automorphisms of `G` (two assignments are one row when their
//!   eps-coloured keys are equal).
//!
//! For a missing row the census classifies the cause from its bubble preimages (each unit of a line
//! replaced by an integrated one-loop insertion, the only rule that puts eps units on lines) and says
//! whether the integral is still reachable as a sector of a presented node (integer lines contracted),
//! and the reduction DAG (`ChildMode::ForcerTree`) that adding the missing rows as roots would create.
//! Diagnostic only: nothing here changes a package or a lookup.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use rayon::prelude::*;
use rustred::propagator::topology::{
    ChildMode, Fate, GraphKey, Normalised, PTopo, TopologyService, canonical, normalise,
};
use serde_json::{Value, json};
use symbolica::graph::{GenerationSettings, Graph, HalfEdge};

/// Largest number of eps units on one line that the census enumerates (the requester's bound).
pub const MAX_UNITS_PER_LINE: u8 = 4;

// ---------------------------------------------------------------------------------------------------------------
// Universe
// ---------------------------------------------------------------------------------------------------------------

/// The generated universe of one loop count.
#[derive(Clone, Debug, Default)]
pub struct LoopUniverse {
    pub loops: u8,
    /// Graphs with exactly `loops` loops returned by graphica's generator.
    pub generated: usize,
    /// Generated graphs with both legs on one vertex (scaleless).
    pub same_vertex: usize,
    /// Generated graphs that [`normalise`] changes (merges or absorbs a line), and how many of those have a
    /// normal form that is not itself in `normal` (must be 0).
    pub not_normal: usize,
    pub not_normal_outside: usize,
    /// Generated graphs that [`normalise`] sends to zero or to the trivial topology.
    pub zero_or_trivial: usize,
    /// Normal forms: canonical key -> canonically labelled topology (eps units 0).
    pub normal: BTreeMap<GraphKey, PTopo>,
    /// Normal forms whose glued graph is cubic.
    pub tops: BTreeSet<GraphKey>,
    /// Nonzero sectors of the tops (subsets of lines contracted, normalised) with `loops` loops.
    pub top_sectors: BTreeSet<GraphKey>,
}

/// The generated graph as a two-point topology: `None` when both legs sit on one vertex.
fn as_two_point(graph: &Graph<u8, u8>) -> Option<PTopo> {
    let externals: Vec<usize> = (0..graph.nodes().len())
        .filter(|&v| graph.node(v).data == 1)
        .collect();
    assert_eq!(externals.len(), 2, "two external vertices");
    let leg = |x: usize| -> usize {
        let edge = graph.edge(graph.node(x).edges[0]);
        if edge.vertices.0 == x {
            edge.vertices.1
        } else {
            edge.vertices.0
        }
    };
    let (s, t) = (leg(externals[0]), leg(externals[1]));
    if s == t {
        return None;
    }
    let mut label = vec![u8::MAX; graph.nodes().len()];
    let mut next = 0u8;
    for (v, slot) in label.iter_mut().enumerate() {
        if !externals.contains(&v) {
            *slot = next;
            next += 1;
        }
    }
    let lines = graph
        .edges()
        .iter()
        .filter(|e| !externals.contains(&e.vertices.0) && !externals.contains(&e.vertices.1))
        .map(|e| (label[e.vertices.0], label[e.vertices.1], 0u8));
    Some(PTopo::new(next, label[s], label[t], lines))
}

/// The canonical key and labelling of `topo` when [`normalise`] leaves it unchanged (every line kept).
fn normal_form(topo: &PTopo) -> Result<(GraphKey, PTopo), Normalised> {
    match normalise(topo) {
        Normalised::Topo(p, fates)
            if p.lines.len() == topo.lines.len()
                && fates.iter().all(|f| matches!(f, Fate::Kept(_))) =>
        {
            let (key, canon) = canonical(&p);
            Ok((key, canon.apply(&p)))
        }
        other => Err(other),
    }
}

/// The key of the normal form of `topo`, `None` when it is zero or trivial.
fn normalised_key(topo: &PTopo) -> Option<(GraphKey, PTopo)> {
    match normalise(topo) {
        Normalised::Topo(p, _) => {
            let (key, canon) = canonical(&p);
            Some((key, canon.apply(&p)))
        }
        Normalised::Zero | Normalised::Trivial => None,
    }
}

/// Is every glued vertex of `topo` cubic (an internal vertex on three lines, `s` and `t` on two)?
fn is_cubic(topo: &PTopo) -> bool {
    let mut degree = vec![0usize; usize::from(topo.n)];
    for &(u, v, _) in &topo.lines {
        degree[usize::from(u)] += 1;
        degree[usize::from(v)] += 1;
    }
    degree[usize::from(topo.s)] += 1;
    degree[usize::from(topo.t)] += 1;
    degree.iter().all(|&d| d == 3)
}

/// `topo` with the lines of `mask` contracted (power 0: endpoints identified), not normalised.
fn contracted(topo: &PTopo, mask: u64) -> PTopo {
    let n = usize::from(topo.n);
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for (i, &(u, v, _)) in topo.lines.iter().enumerate() {
        if mask >> i & 1 == 1 {
            let (a, b) = (
                root(&mut parent, usize::from(u)),
                root(&mut parent, usize::from(v)),
            );
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    let mut map = vec![0u8; n];
    for (v, slot) in map.iter_mut().enumerate() {
        *slot = root(&mut parent, v) as u8;
    }
    let lines = topo
        .lines
        .iter()
        .enumerate()
        .filter(|(i, _)| mask >> i & 1 == 0)
        .map(|(_, &(u, v, e))| (map[usize::from(u)], map[usize::from(v)], e));
    PTopo::new(
        topo.n,
        map[usize::from(topo.s)],
        map[usize::from(topo.t)],
        lines,
    )
}

/// The universe of `loops` loops: graphica's generator, normal forms, tops and the sectors of the tops.
pub fn loop_universe(loops: u8) -> LoopUniverse {
    assert!((1..=6).contains(&loops), "the census generates 1..=6 loops");
    let g = HalfEdge::undirected(0u8);
    let signatures: Vec<Vec<HalfEdge<u8>>> = (3..=2 * usize::from(loops) + 1)
        .map(|d| vec![g; d])
        .collect();
    let settings = GenerationSettings::new()
        .max_loops(usize::from(loops))
        .max_vertices(2 * usize::from(loops) + 2)
        .max_bridges(0);
    let graphs = Graph::<u8, u8>::generate(&[(1, g), (1, g)], &signatures, settings)
        .unwrap_or_else(|partial| partial);
    let mut universe = LoopUniverse {
        loops,
        ..LoopUniverse::default()
    };
    let mut changed = Vec::new();
    for graph in graphs
        .keys()
        .filter(|graph| graph.num_loops() == usize::from(loops))
    {
        universe.generated += 1;
        let Some(topo) = as_two_point(graph) else {
            universe.same_vertex += 1;
            continue;
        };
        match normal_form(&topo) {
            Ok((key, canonical_topo)) => {
                universe.normal.insert(key, canonical_topo);
            }
            Err(Normalised::Topo(p, _)) => changed.push(p),
            Err(_) => universe.zero_or_trivial += 1,
        }
    }
    universe.not_normal = changed.len();
    for p in changed {
        let (key, _) = canonical(&p);
        if p.loops() != loops || !universe.normal.contains_key(&key) {
            universe.not_normal_outside += 1;
        }
    }
    universe.tops = (universe.normal.iter())
        .filter(|(_, topo)| is_cubic(topo))
        .map(|(key, _)| key.clone())
        .collect();
    let tops: Vec<&PTopo> = universe
        .tops
        .iter()
        .map(|key| &universe.normal[key])
        .collect();
    universe.top_sectors = tops
        .par_iter()
        .map(|top| {
            let mut keys = BTreeSet::new();
            for mask in 0..1u64 << top.lines.len() {
                if let Some((key, p)) = normalised_key(&contracted(top, mask))
                    && p.loops() == loops
                {
                    keys.insert(key);
                }
            }
            keys
        })
        .reduce(BTreeSet::new, |mut a, b| {
            a.extend(b);
            a
        });
    universe
}

// ---------------------------------------------------------------------------------------------------------------
// The package's lookup index
// ---------------------------------------------------------------------------------------------------------------

/// One presentation of a DAG node, as the lookup sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPresentation {
    pub id: u32,
    pub kind: String,
    pub loops: u8,
    /// `eps_units` of the line slots, in slot order.
    pub line_eps: Vec<u8>,
}

/// The lookup index of a package: graph key bytes -> the presentations of that DAG node, in id order (the
/// lookup takes the first), as vakint's `eps_family::key_index` builds it from the loaded library.
#[derive(Clone, Debug, Default)]
pub struct PackageIndex {
    pub identity: String,
    pub scope: [u8; 2],
    /// Top presentations per loop count.
    pub tops: BTreeMap<u8, usize>,
    pub by_key: HashMap<Vec<u8>, Vec<IndexedPresentation>>,
}

impl PackageIndex {
    /// The index from `library.json` alone (the loader takes every presentation's node key from it and checks it
    /// against the DAG of the topology file).
    pub fn from_manifest(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_manifest_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// [`Self::from_manifest`] of the bytes of a `library.json`.
    pub fn from_manifest_bytes(bytes: &[u8]) -> Result<Self, String> {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let text = |v: &Value| v.as_str().map(str::to_owned);
        let identity = value.get("identity").and_then(text).unwrap_or_default();
        let scope = value
            .get("scope")
            .and_then(Value::as_array)
            .and_then(|s| Some([s.first()?.as_u64()? as u8, s.get(1)?.as_u64()? as u8]))
            .ok_or("library.json: no scope")?;
        let mut index = PackageIndex {
            identity,
            scope,
            ..PackageIndex::default()
        };
        let presentations = value
            .get("presentations")
            .and_then(Value::as_array)
            .ok_or("library.json: no presentations")?;
        for record in presentations {
            let id = record
                .get("index")
                .and_then(Value::as_u64)
                .ok_or("presentation index")? as u32;
            let loops = record
                .get("loops")
                .and_then(Value::as_u64)
                .ok_or("presentation loops")? as u8;
            let kind = match record.get("kind") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Object(o)) => o.keys().next().cloned().unwrap_or_default(),
                _ => return Err(format!("presentation {id}: no kind")),
            };
            let line_eps = record
                .get("slots")
                .and_then(Value::as_array)
                .ok_or("presentation slots")?
                .iter()
                .filter_map(|slot| slot.get("line"))
                .map(|line| line.get("eps_units").and_then(Value::as_u64).unwrap_or(0) as u8)
                .collect();
            if kind == "top" {
                *index.tops.entry(loops).or_default() += 1;
            }
            let Some(key) = record
                .get("node")
                .and_then(|n| n.get("key"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let key = GraphKey::from_hex(key).ok_or_else(|| format!("presentation {id}: key"))?;
            index
                .by_key
                .entry(key.as_bytes().to_vec())
                .or_default()
                .push(IndexedPresentation {
                    id,
                    kind,
                    loops,
                    line_eps,
                });
        }
        for list in index.by_key.values_mut() {
            list.sort_by_key(|p| p.id);
        }
        Ok(index)
    }

    /// The index from a loaded library (`load_p_library`): exactly the caller's `key_index`.
    pub fn from_library(library: &rustred::propagator::library::PLibrary) -> Self {
        use rustred::propagator::presentation::{PresentationKind, Slot};
        let scope = library.scope();
        let mut index = PackageIndex {
            identity: library.identity(),
            scope: [*scope.start(), *scope.end()],
            ..PackageIndex::default()
        };
        for presentation in library.presentations() {
            if matches!(presentation.kind, PresentationKind::Top { .. }) {
                *index.tops.entry(presentation.loops).or_default() += 1;
            }
            let Some(node) = &presentation.node else {
                continue;
            };
            let kind = match &presentation.kind {
                PresentationKind::Top { .. } => "top",
                PresentationKind::Induced { .. } => "induced",
                PresentationKind::InsertionAdapted => "insertion_adapted",
                PresentationKind::CarpetAdapted => "carpet_adapted",
                PresentationKind::TriangleAdapted => "triangle_adapted",
                PresentationKind::DiamondAdapted => "diamond_adapted",
                PresentationKind::SchemeAdapted => "scheme_adapted",
                PresentationKind::EpsNode => "eps_node",
                PresentationKind::Caller => "caller",
            };
            let line_eps = presentation
                .slots
                .iter()
                .filter_map(|slot| match slot {
                    Slot::Line(line) => Some(line.eps_units),
                    Slot::Isp(_) => None,
                })
                .collect();
            index
                .by_key
                .entry(node.graph_key.to_vec())
                .or_default()
                .push(IndexedPresentation {
                    id: presentation.id.0,
                    kind: kind.to_owned(),
                    loops: presentation.loops,
                    line_eps,
                });
        }
        index
    }

    /// The node keys of the index decoded into topologies (canonical labelling of the key).
    pub fn node_topologies(&self) -> Vec<(Vec<u8>, PTopo)> {
        let mut out: Vec<(Vec<u8>, PTopo)> = self
            .by_key
            .keys()
            .filter_map(|key| decode_key(key).map(|topo| (key.clone(), topo)))
            .collect();
        out.sort();
        out
    }
}

/// The topology a [`GraphKey`] encodes (version 1): `[version, n, colour(0..n), (u, v, m, e_1..e_m)*]`; `s < t`
/// are the two vertices of colour 1. `None` for another version or a malformed key.
pub fn decode_key(bytes: &[u8]) -> Option<PTopo> {
    let (&version, rest) = bytes.split_first()?;
    if u32::from(version) != rustred::propagator::topology::P_TOPOLOGY_VERSION {
        return None;
    }
    let (&n, rest) = rest.split_first()?;
    let colours = rest.get(..usize::from(n))?;
    let mut rest = &rest[usize::from(n)..];
    let externals: Vec<u8> = (0..n).filter(|&v| colours[usize::from(v)] == 1).collect();
    let [s, t] = externals.as_slice() else {
        return None;
    };
    let mut lines = Vec::new();
    while !rest.is_empty() {
        let (&u, r) = rest.split_first()?;
        let (&v, r) = r.split_first()?;
        let (&m, r) = r.split_first()?;
        let eps = r.get(..usize::from(m))?;
        lines.extend(eps.iter().map(|&e| (u, v, e)));
        rest = &r[usize::from(m)..];
    }
    if lines.iter().any(|&(u, v, _)| u >= n || v >= n) {
        return None;
    }
    Some(PTopo::new(n, *s, *t, lines))
}

// ---------------------------------------------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------------------------------------------

/// Why a row is missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cause {
    /// `L + sum(u)` exceeds the loop count of the package's tops.
    AboveTops,
    /// A one-step bubble preimage (one unit as an explicit one-loop insertion) is a node of the package: the DAG
    /// integrated another insertion of that node first, so these units never sit on these lines together.
    ParentPresent,
    /// No one-step preimage is a node, a deeper one is (`depth` units as explicit insertions).
    AncestorPresent { depth: u8 },
    /// No bubble preimage at any depth is a node: the integer graph with explicit insertions is not reached.
    Unreached,
}

impl Cause {
    pub fn label(&self) -> String {
        match self {
            Cause::AboveTops => "above_tops".to_owned(),
            Cause::ParentPresent => "parent_present".to_owned(),
            Cause::AncestorPresent { depth } => format!("ancestor_present_d{depth}"),
            Cause::Unreached => "unreached".to_owned(),
        }
    }
}

/// One (graph, unit assignment) of the census.
#[derive(Clone, Debug)]
pub struct Row {
    pub loops: u8,
    pub lines: u8,
    /// Key of the integer graph `G` (hex).
    pub graph_key: String,
    /// Units per line of `G`, in the canonical line order of `G` (the lexicographically first assignment of its
    /// orbit).
    pub units: Vec<u8>,
    /// Key of the eps-shifted graph, the lookup key (hex).
    pub key: String,
    pub total: u8,
    pub shifted: u8,
    pub max_units: u8,
    /// The presentation the lookup returns (the first of the node) and its kind.
    pub presentation: Option<(u32, String)>,
    /// Number of presentations of the node (the lookup takes the first).
    pub presentations: usize,
    /// For a found row: the presentation's line slots carry exactly the units of the canonical lines.
    pub slots_match: Option<bool>,
    pub cause: Option<Cause>,
    /// For a missing row: a presentation of which it is a sector (integer lines contracted), if any.
    pub sector_of: Option<u32>,
    /// For a missing row: the smallest number of explicit insertions after which the graph is a sector of a
    /// presented node (0 = itself), if any within its units.
    pub nearest_sector_depth: Option<u8>,
}

/// Every assignment of `0..=max` units to `n` lines with total at most `budget`, in lexicographic order.
fn assignments(n: usize, budget: u8, max: u8) -> Vec<Vec<u8>> {
    fn go(n: usize, budget: u8, max: u8, prefix: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
        if prefix.len() == n {
            out.push(prefix.clone());
            return;
        }
        for k in 0..=budget.min(max) {
            prefix.push(k);
            go(n, budget - k, max, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    go(n, budget, max, &mut Vec::with_capacity(n), &mut out);
    out
}

/// `topo` with eps units `units` on its lines.
fn with_units(topo: &PTopo, units: &[u8]) -> PTopo {
    PTopo::new(
        topo.n,
        topo.s,
        topo.t,
        topo.lines
            .iter()
            .zip(units)
            .map(|(&(u, v, _), &e)| (u, v, e)),
    )
}

/// The one-step bubble preimages of `topo`: one unit of an eps line `p-q` with `m` units as an explicit
/// one-loop insertion whose integration (rule output `a + b + 1` units, then the series merge of `normalise`)
/// gives the line back: the line itself as two parallel lines with `a + b = m - 1`, or a line `p-x` with `c`
/// units in series with an insertion `x=q` (or mirrored) with `a + b + c = m - 1`. Normalised, keyed.
fn one_step_preimages(topo: &PTopo) -> Vec<(GraphKey, PTopo)> {
    let mut out = Vec::new();
    for (i, &(p, q, m)) in topo.lines.iter().enumerate() {
        if m == 0 {
            continue;
        }
        let rest: Vec<(u8, u8, u8)> = (topo.lines.iter().enumerate())
            .filter(|(j, _)| *j != i)
            .map(|(_, &l)| l)
            .collect();
        let mut candidates: Vec<PTopo> = Vec::new();
        for a in 0..=(m - 1) / 2 {
            let mut lines = rest.clone();
            lines.extend([(p, q, a), (p, q, m - 1 - a)]);
            candidates.push(PTopo::new(topo.n, topo.s, topo.t, lines));
        }
        let x = topo.n;
        for c in 0..m {
            let r = m - 1 - c;
            for a in 0..=r / 2 {
                for (near, far) in [(p, q), (q, p)] {
                    let mut lines = rest.clone();
                    lines.extend([(near, x, c), (x, far, a), (x, far, r - a)]);
                    candidates.push(PTopo::new(topo.n + 1, topo.s, topo.t, lines));
                }
            }
        }
        out.extend(candidates.iter().filter_map(normalised_key));
    }
    out
}

/// Every key reachable from the eps nodes of `index` by contracting integer lines: key -> the presentation.
fn sector_index(index: &PackageIndex) -> HashMap<Vec<u8>, u32> {
    let nodes: Vec<(u32, PTopo)> = index
        .node_topologies()
        .into_iter()
        .filter(|(_, topo)| topo.lines.iter().any(|l| l.2 > 0))
        .map(|(key, topo)| (index.by_key[&key][0].id, topo))
        .collect();
    let maps: Vec<Vec<(Vec<u8>, u32)>> = nodes
        .par_iter()
        .map(|(id, topo)| {
            let integer: Vec<usize> = (0..topo.lines.len())
                .filter(|&i| topo.lines[i].2 == 0)
                .collect();
            let mut out = Vec::new();
            for sub in 0..1u64 << integer.len() {
                let mask = integer
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| sub >> k & 1 == 1)
                    .fold(0u64, |m, (_, &i)| m | 1 << i);
                if let Some((key, _)) = normalised_key(&contracted(topo, mask)) {
                    out.push((key.as_bytes().to_vec(), *id));
                }
            }
            out
        })
        .collect();
    let mut sectors = HashMap::new();
    for (key, id) in maps.into_iter().flatten() {
        sectors
            .entry(key)
            .and_modify(|old: &mut u32| *old = (*old).min(id))
            .or_insert(id);
    }
    sectors
}

/// The census rows of `universes` against `index` for the bound `total` (`L + sum(u) <= total`), with the
/// classification of the missing rows.
pub fn census_rows(
    universes: &BTreeMap<u8, LoopUniverse>,
    index: &PackageIndex,
    total: u8,
) -> Vec<Row> {
    let max_top = index.tops.keys().copied().max().unwrap_or(0);
    let sectors = sector_index(index);
    let mut work: Vec<(u8, &GraphKey, &PTopo)> = Vec::new();
    for (&loops, universe) in universes.range(1..=total) {
        for (key, topo) in &universe.normal {
            work.push((loops, key, topo));
        }
    }
    let per_graph: Vec<Vec<Row>> = work
        .par_iter()
        .map(|&(loops, graph_key, topo)| {
            let mut seen = HashSet::new();
            let mut rows = Vec::new();
            for units in assignments(topo.lines.len(), total - loops, MAX_UNITS_PER_LINE) {
                let shifted = with_units(topo, &units);
                let (key, canon) = match normalise(&shifted) {
                    Normalised::Topo(p, fates)
                        if fates.iter().all(|f| matches!(f, Fate::Kept(_))) =>
                    {
                        let (key, canon) = canonical(&p);
                        (key, canon.apply(&p))
                    }
                    other => panic!("a normal form with eps units is not normal: {other:?}"),
                };
                if !seen.insert(key.clone()) {
                    continue;
                }
                let found = index.by_key.get(key.as_bytes());
                let presentation = found.map(|list| (list[0].id, list[0].kind.clone()));
                let slots_match = found.map(|list| {
                    let eps: Vec<u8> = canon.lines.iter().map(|l| l.2).collect();
                    list[0].loops == loops && list[0].line_eps == eps
                });
                let sum: u8 = units.iter().sum();
                let mut row = Row {
                    loops,
                    lines: topo.lines.len() as u8,
                    graph_key: graph_key.to_hex(),
                    units: units.clone(),
                    key: key.to_hex(),
                    total: sum,
                    shifted: units.iter().filter(|&&u| u > 0).count() as u8,
                    max_units: units.iter().copied().max().unwrap_or(0),
                    presentation,
                    presentations: found.map_or(0, Vec::len),
                    slots_match,
                    cause: None,
                    sector_of: None,
                    nearest_sector_depth: None,
                };
                if row.presentation.is_none() {
                    classify(&mut row, &canon, index, &sectors, max_top);
                }
                rows.push(row);
            }
            rows
        })
        .collect();
    let mut rows: Vec<Row> = per_graph.into_iter().flatten().collect();
    rows.sort_by(|a, b| {
        (a.loops, a.total, a.lines, &a.graph_key, &a.units).cmp(&(
            b.loops,
            b.total,
            b.lines,
            &b.graph_key,
            &b.units,
        ))
    });
    rows
}

fn classify(
    row: &mut Row,
    topo: &PTopo,
    index: &PackageIndex,
    sectors: &HashMap<Vec<u8>, u32>,
    max_top: u8,
) {
    let key = |t: &PTopo| canonical(t).0.as_bytes().to_vec();
    row.sector_of = sectors.get(&key(topo)).copied();
    if row.sector_of.is_some() {
        row.nearest_sector_depth = Some(0);
    }
    if row.loops + row.total > max_top {
        row.cause = Some(Cause::AboveTops);
        return;
    }
    let mut frontier: Vec<(GraphKey, PTopo)> = vec![(canonical(topo).0, topo.clone())];
    let mut seen: HashSet<GraphKey> = frontier.iter().map(|(k, _)| k.clone()).collect();
    let mut present_at = None;
    for depth in 1..=row.total {
        let mut next = Vec::new();
        for (_, t) in &frontier {
            for (k, p) in one_step_preimages(t) {
                if seen.insert(k.clone()) {
                    next.push((k, p));
                }
            }
        }
        if present_at.is_none()
            && next
                .iter()
                .any(|(k, _)| index.by_key.contains_key(k.as_bytes()))
        {
            present_at = Some(depth);
        }
        if row.nearest_sector_depth.is_none()
            && next.iter().any(|(k, _)| sectors.contains_key(k.as_bytes()))
        {
            row.nearest_sector_depth = Some(depth);
        }
        frontier = next;
    }
    row.cause = Some(match present_at {
        Some(1) => Cause::ParentPresent,
        Some(depth) => Cause::AncestorPresent { depth },
        None => Cause::Unreached,
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Production estimate
// ---------------------------------------------------------------------------------------------------------------

/// The ForcerTree DAG of the missing rows as roots: its nodes that the package lacks, by
/// (loops, eps lines > 0, action kind, series-parallel).
pub fn missing_closure(
    rows: &[Row],
    index: &PackageIndex,
) -> BTreeMap<(u8, bool, String, bool), usize> {
    let roots: Vec<PTopo> = rows
        .iter()
        .filter(|row| row.presentation.is_none())
        .filter_map(|row| GraphKey::from_hex(&row.key).and_then(|k| decode_key(k.as_bytes())))
        .collect();
    let dag = TopologyService::new().reduction_graph(&roots, ChildMode::ForcerTree);
    let mut out = BTreeMap::new();
    for node in &dag.nodes {
        if node.topo.lines.is_empty() || index.by_key.contains_key(node.key.as_bytes()) {
            continue;
        }
        *out.entry((
            node.loops,
            node.eps_lines > 0,
            node.action.kind().as_str().to_owned(),
            node.series_parallel,
        ))
        .or_default() += 1;
    }
    out
}

/// The ForcerTree DAG of the GENERATED tops with at most `l` loops, for each `l` of `universes`:
/// `(l, nodes, not_in_package, package_not_in_dag)` = its node count (trivial node included), its non-trivial nodes
/// that are no node of the package, and the package's nodes of at most `l` loops that are not in it (meaningful
/// against a package whose scope is `1..=l`).
pub fn generated_top_dags(
    universes: &BTreeMap<u8, LoopUniverse>,
    index: &PackageIndex,
) -> Vec<(u8, usize, usize, usize)> {
    let service = TopologyService::new();
    let mut out = Vec::new();
    let mut tops: Vec<PTopo> = Vec::new();
    for (&loops, universe) in universes {
        tops.extend(universe.tops.iter().map(|key| universe.normal[key].clone()));
        let dag = service.reduction_graph(&tops, ChildMode::ForcerTree);
        let keys: HashSet<&[u8]> = (dag.nodes.iter())
            .filter(|n| !n.topo.lines.is_empty())
            .map(|n| n.key.as_bytes())
            .collect();
        let not_in_package = keys
            .iter()
            .filter(|k| !index.by_key.contains_key(**k))
            .count();
        let package_not_in_dag = (index.by_key.iter())
            .filter(|(k, list)| list[0].loops <= loops && !keys.contains(k.as_slice()))
            .count();
        out.push((loops, dag.nodes.len(), not_in_package, package_not_in_dag));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Eps axes: forced in every sector, two-sided in every owner, keys with integer power <= 0 accepted
// ---------------------------------------------------------------------------------------------------------------

/// Integer powers put on the eps lines by the trace of [`axes_report`]: for every non-empty subset of the eps lines of
/// a presentation and each of these values, the lines of the subset at that power, the other eps lines and every
/// integer line at power 1, ISPs at 0 (2, 6, 14 inputs for 1, 2, 3 eps lines).
pub const AXIS_TRACE_POWERS: [i64; 2] = [0, -1];

/// The eps-axis checks of one presentation of a present eps row.
#[derive(Clone, Debug, Default)]
pub struct AxesRow {
    pub id: u32,
    pub kind: String,
    pub loops: u8,
    /// Line slots with eps units (`Presentation::forced_axes`).
    pub forced: u64,
    /// Census sectors `(presentation, support)` of the presentation, and those whose support lacks a forced axis.
    pub sectors: usize,
    pub pinched: usize,
    /// Traced inputs, the distinct keys their trace stepped, the refusals reached from them (`None` until traced),
    /// the refusal kinds and the first refusal's message, and the trace time.
    pub inputs: usize,
    pub keys: usize,
    pub refused: Option<usize>,
    pub refusal: String,
    pub seconds: f64,
}

/// What [`axes_report`] found.
#[derive(Clone, Debug, Default)]
pub struct AxesReport {
    pub rows: BTreeMap<u32, AxesRow>,
    /// Custom owners whose presentation has eps lines, and those whose two-sided axes are not exactly the forced
    /// axes of their sector or whose charts are not every sign pattern of them (label and reason).
    pub custom_eps_owners: usize,
    pub custom_not_two_sided: Vec<String>,
    /// Owners of other lanes whose presentation has eps lines, by lane.
    pub lane_eps_owners: BTreeMap<String, usize>,
    pub trace_inputs: usize,
    pub trace_distinct_keys: usize,
    pub trace_failures: BTreeMap<String, usize>,
    pub trace_samples: Vec<String>,
    pub trace_seconds: f64,
}

/// `bits` spread onto the set bits of `mask` (lowest first).
fn deposit(bits: u64, mask: u64) -> u64 {
    let (mut out, mut m, mut k) = (0u64, mask, 0);
    while m != 0 {
        let low = m & m.wrapping_neg();
        if bits >> k & 1 == 1 {
            out |= low;
        }
        m &= m - 1;
        k += 1;
    }
    out
}

/// The eps-axis checks on the presentations of the present eps rows of `rows` (`total > 0`), on a loaded
/// library: (1) every census sector of such a presentation keeps every eps line in its support; (2) every Custom
/// owner on an eps presentation is two-sided exactly on the forced axes of its sector, with every sign chart;
/// (3) keys with eps lines at integer power 0 and -1 ([`AXIS_TRACE_POWERS`]) are traced through the library without
/// coefficients (routes, owners, children down to terminals), one trace per presentation, and every refusal is
/// counted; each presentation's result is appended to `stream` when it is done (resumable).
pub fn axes_report(
    library: &rustred::propagator::library::PLibrary,
    rows: &[Row],
    stream: &Path,
    threads: usize,
) -> Result<AxesReport, rustred::propagator::error::PError> {
    use rustred::propagator::PresentationId;
    use rustred::propagator::descent::trace;
    use rustred::propagator::presentation::Slot;
    use rustred::reduction::ReductionLimits;

    let mut report = AxesReport::default();
    let ids: BTreeSet<u32> = rows
        .iter()
        .filter(|r| r.total > 0)
        .filter_map(|r| r.presentation.as_ref().map(|p| p.0))
        .collect();
    let census = library.census();
    let mut inputs_of: BTreeMap<u32, Vec<(PresentationId, Vec<i64>)>> = BTreeMap::new();
    for &id in &ids {
        let p = library.presentation(PresentationId(id));
        let forced = p.forced_axes();
        let mut row = AxesRow {
            id,
            kind: rows
                .iter()
                .find_map(|r| {
                    r.presentation
                        .as_ref()
                        .filter(|x| x.0 == id)
                        .map(|x| x.1.clone())
                })
                .unwrap_or_default(),
            loops: p.loops,
            forced,
            ..AxesRow::default()
        };
        for (&(_, support), _) in census
            .sectors
            .range((PresentationId(id), 0)..=(PresentationId(id), u64::MAX))
        {
            row.sectors += 1;
            if support & forced != forced {
                row.pinched += 1;
            }
        }
        let eps_slots: Vec<usize> = (0..p.slots.len())
            .filter(|&s| forced >> s & 1 == 1)
            .collect();
        let mut inputs: Vec<(PresentationId, Vec<i64>)> = Vec::new();
        for subset in 1..1u64 << eps_slots.len() {
            for &power in &AXIS_TRACE_POWERS {
                let mut powers: Vec<i64> = (p.slots.iter())
                    .map(|slot| i64::from(matches!(slot, Slot::Line(_))))
                    .collect();
                for (k, &slot) in eps_slots.iter().enumerate() {
                    if subset >> k & 1 == 1 {
                        powers[slot] = power;
                    }
                }
                inputs.push((PresentationId(id), powers));
            }
        }
        row.inputs = inputs.len();
        inputs_of.insert(id, inputs);
        report.rows.insert(id, row);
    }
    for owner in library.owners() {
        let spec = &owner.spec;
        let p = library.presentation(spec.presentation);
        let forced = p.forced_axes();
        if forced == 0 {
            continue;
        }
        let Some(meta) = &spec.custom else {
            *report
                .lane_eps_owners
                .entry(format!("{:?}", spec.lane))
                .or_default() += 1;
            continue;
        };
        report.custom_eps_owners += 1;
        let label = census
            .classes
            .get(&spec.class)
            .map(|c| c.key.label())
            .unwrap_or_else(|| format!("{:016x}", spec.class.0));
        let Some(support) = census.classes.get(&spec.class).map(|c| c.owner.1) else {
            report
                .custom_not_two_sided
                .push(format!("{label}: no census class"));
            continue;
        };
        let expected_axes = forced & support;
        let integer = support & !forced;
        let mut expected: Vec<u64> = (0..1u64 << expected_axes.count_ones())
            .map(|bits| integer | deposit(bits, expected_axes))
            .collect();
        expected.sort_unstable();
        let mut charts = meta.charts.clone();
        charts.sort_unstable();
        if meta.two_sided_axes != expected_axes || charts != expected {
            report.custom_not_two_sided.push(format!(
                "{label}: two-sided {:#x} charts {:?}, forced {expected_axes:#x} charts {expected:?}",
                meta.two_sided_axes, meta.charts
            ));
        }
    }
    // (3) one trace per presentation (its state is dropped after it), in a pool of `threads`, smallest loop count
    // first; each result is appended to `stream` at once, and a presentation already in `stream` is not traced again.
    let mut done: BTreeMap<u32, (usize, usize, String, f64)> = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(stream) {
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if let [id, _, keys, refused, refusal, seconds, ..] = f.as_slice()
                && let (Ok(id), Ok(keys), Ok(refused), Ok(seconds)) =
                    (id.parse(), keys.parse(), refused.parse(), seconds.parse())
            {
                done.insert(id, (keys, refused, (*refusal).to_owned(), seconds));
            }
        }
    }
    let fresh = std::fs::metadata(stream).map_or(true, |m| m.len() == 0);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(stream)
        .map_err(|e| {
            rustred::propagator::error::PError::Unsupported(format!("{}: {e}", stream.display()))
        })?;
    let file = std::sync::Mutex::new(std::io::BufWriter::new(file));
    if fresh {
        use std::io::Write as _;
        let mut f = file.lock().unwrap();
        writeln!(
            f,
            "presentation\tinputs\tkeys\trefused\trefusal\tseconds\tloops"
        )
        .ok();
        f.flush().ok();
    }
    let mut todo: Vec<(u8, u32)> = (report.rows.values())
        .filter(|r| !done.contains_key(&r.id))
        .map(|r| (r.loops, r.id))
        .collect();
    todo.sort_unstable();
    let total = report.rows.len();
    let counter = std::sync::atomic::AtomicUsize::new(done.len());
    let started = std::time::Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .map_err(|e| rustred::propagator::error::PError::Unsupported(e.to_string()))?;
    let results: Vec<Result<(u32, (usize, usize, String, f64)), rustred::propagator::error::PError>> =
        pool.install(|| {
            todo.par_iter()
                .with_max_len(1)
                .map(|&(loops, id)| {
                    let t = std::time::Instant::now();
                    let traced = trace(library, &inputs_of[&id], ReductionLimits::default())?;
                    let refused: usize = traced.failures.values().sum();
                    let refusal = traced
                        .samples
                        .first()
                        .map(|f| format!("{:?}: {}", f.kind, f.detail.replace(['\t', '\n'], " ")))
                        .unwrap_or_else(|| "-".to_owned());
                    let keys = traced.route_keys + traced.apply_keys;
                    let seconds = t.elapsed().as_secs_f64();
                    {
                        use std::io::Write as _;
                        let mut f = file.lock().unwrap();
                        writeln!(
                            f,
                            "{id}\t{}\t{keys}\t{refused}\t{refusal}\t{seconds:.3}\t{loops}",
                            traced.inputs
                        )
                        .ok();
                        f.flush().ok();
                    }
                    let n = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    eprintln!(
                        "axes {n}/{total} presentation {id} L{loops}: {} inputs, {keys} keys, {refused} refused, {seconds:.1} s ({:.0} s)",
                        traced.inputs,
                        started.elapsed().as_secs_f64()
                    );
                    Ok((id, (keys, refused, refusal, seconds)))
                })
                .collect()
        });
    for result in results {
        let (id, value) = result?;
        done.insert(id, value);
    }
    report.trace_seconds = started.elapsed().as_secs_f64();
    for row in report.rows.values_mut() {
        if let Some((keys, refused, refusal, seconds)) = done.get(&row.id) {
            row.keys = *keys;
            row.refused = Some(*refused);
            row.refusal = refusal.clone();
            row.seconds = *seconds;
            report.trace_inputs += row.inputs;
            report.trace_distinct_keys += keys;
            if *refused > 0 {
                *report
                    .trace_failures
                    .entry(refusal.split(':').next().unwrap_or("").to_owned())
                    .or_default() += refused;
                if report.trace_samples.len() < 32 {
                    report
                        .trace_samples
                        .push(format!("presentation {}: {refusal}", row.id));
                }
            }
        }
    }
    Ok(report)
}

/// The axes report as TSV, one row per presentation.
pub fn axes_tsv(report: &AxesReport) -> String {
    let mut out = String::from(
        "presentation\tkind\tloops\tforced_axes\tcensus_sectors\tpinched_sectors\ttrace_inputs\ttrace_keys\trefused\trefusal\tseconds\n",
    );
    for r in report.rows.values() {
        let refused = r.refused.map_or_else(|| "-".to_owned(), |x| x.to_string());
        writeln!(
            out,
            "{}\t{}\t{}\t{:#x}\t{}\t{}\t{}\t{}\t{refused}\t{}\t{:.3}",
            r.id,
            r.kind,
            r.loops,
            r.forced,
            r.sectors,
            r.pinched,
            r.inputs,
            r.keys,
            r.refusal,
            r.seconds
        )
        .unwrap();
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------------------------------------------

/// The universe cross-checks per loop count against the package: `(table, all checks passed)`.
pub fn universe_table(
    universes: &BTreeMap<u8, LoopUniverse>,
    index: &PackageIndex,
) -> (String, Value) {
    let mut integer_nodes: BTreeMap<u8, BTreeSet<Vec<u8>>> = BTreeMap::new();
    let mut eps_nodes: BTreeMap<u8, BTreeSet<Vec<u8>>> = BTreeMap::new();
    for (key, list) in &index.by_key {
        let loops = list[0].loops;
        if list[0].line_eps.iter().all(|&e| e == 0) {
            integer_nodes.entry(loops).or_default().insert(key.clone());
        } else {
            eps_nodes.entry(loops).or_default().insert(key.clone());
        }
    }
    let mut table = String::from(
        "loops\tgenerated\tsame_vertex\tzero_or_trivial\tnot_normal\tnot_normal_outside\tnormal_forms\ttops\ttop_sectors\tnormal_eq_top_sectors\tpackage_tops\tpackage_integer_nodes\tpackage_integer_nodes_not_in_universe\tuniverse_not_package_node\tpackage_eps_nodes\tpackage_eps_nodes_not_in_universe\n",
    );
    let mut json_rows = Vec::new();
    for (&loops, u) in universes {
        let normal: BTreeSet<Vec<u8>> = u.normal.keys().map(|k| k.as_bytes().to_vec()).collect();
        let top_sectors: BTreeSet<Vec<u8>> = u
            .top_sectors
            .iter()
            .map(|k| k.as_bytes().to_vec())
            .collect();
        let empty = BTreeSet::new();
        let lib_int = integer_nodes.get(&loops).unwrap_or(&empty);
        let lib_eps = eps_nodes.get(&loops).unwrap_or(&empty);
        let lib_int_outside = lib_int.difference(&normal).count();
        let universe_missing = normal.difference(lib_int).count();
        let eps_outside = lib_eps
            .iter()
            .filter(|key| {
                decode_key(key).is_none_or(|t| {
                    let integer =
                        PTopo::new(t.n, t.s, t.t, t.lines.iter().map(|&(a, b, _)| (a, b, 0)));
                    !normal.contains(canonical(&integer).0.as_bytes())
                })
            })
            .count();
        let package_tops = index.tops.get(&loops).copied().unwrap_or(0);
        writeln!(
            table,
            "{loops}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{package_tops}\t{}\t{lib_int_outside}\t{universe_missing}\t{}\t{eps_outside}",
            u.generated,
            u.same_vertex,
            u.zero_or_trivial,
            u.not_normal,
            u.not_normal_outside,
            normal.len(),
            u.tops.len(),
            top_sectors.len(),
            normal == top_sectors,
            lib_int.len(),
            lib_eps.len(),
        )
        .unwrap();
        json_rows.push(json!({
            "loops": loops, "generated": u.generated, "normal_forms": normal.len(), "tops": u.tops.len(),
            "top_sectors": top_sectors.len(), "normal_eq_top_sectors": normal == top_sectors,
            "not_normal_outside": u.not_normal_outside, "package_tops": package_tops,
            "package_integer_nodes": lib_int.len(), "package_integer_nodes_not_in_universe": lib_int_outside,
            "universe_not_package_node": universe_missing, "package_eps_nodes": lib_eps.len(),
            "package_eps_nodes_not_in_universe": eps_outside,
        }));
    }
    (table, Value::Array(json_rows))
}

/// The rows as TSV (one row per (graph key, assignment)).
pub fn rows_tsv(rows: &[Row]) -> String {
    let mut out = String::from(
        "loops\tlines\ttotal_units\tshifted_lines\tmax_units\tunits\tgraph_key\teps_key\tpresentation\tkind\tnode_presentations\tslots_match\tcause\tsector_of\tnearest_sector_depth\n",
    );
    for r in rows {
        let units = r
            .units
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let (presentation, kind) = match &r.presentation {
            Some((id, kind)) => (id.to_string(), kind.clone()),
            None => ("MISSING".to_owned(), "-".to_owned()),
        };
        let opt = |x: Option<String>| x.unwrap_or_else(|| "-".to_owned());
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{units}\t{}\t{}\t{presentation}\t{kind}\t{}\t{}\t{}\t{}\t{}",
            r.loops,
            r.lines,
            r.total,
            r.shifted,
            r.max_units,
            r.graph_key,
            r.key,
            r.presentations,
            opt(r.slots_match.map(|b| b.to_string())),
            opt(r.cause.map(|c| c.label())),
            opt(r.sector_of.map(|x| x.to_string())),
            opt(r.nearest_sector_depth.map(|x| x.to_string())),
        )
        .unwrap();
    }
    out
}

/// Counts by (loops, total units, shifted lines, max units): total, present, missing, missing but a sector of a
/// presented node.
pub fn summary_tsv(rows: &[Row]) -> String {
    let mut groups: BTreeMap<(u8, u8, u8, u8), [usize; 4]> = BTreeMap::new();
    for r in rows {
        let g = groups
            .entry((r.loops, r.total, r.shifted, r.max_units))
            .or_default();
        g[0] += 1;
        if r.presentation.is_some() {
            g[1] += 1;
        } else {
            g[2] += 1;
            if r.sector_of.is_some() {
                g[3] += 1;
            }
        }
    }
    let mut out = String::from(
        "loops\ttotal_units\tshifted_lines\tmax_units\trows\tpresent\tmissing\tmissing_but_sector\n",
    );
    let mut sums = [0usize; 4];
    for ((l, t, s, m), g) in &groups {
        writeln!(
            out,
            "{l}\t{t}\t{s}\t{m}\t{}\t{}\t{}\t{}",
            g[0], g[1], g[2], g[3]
        )
        .unwrap();
        for (sum, x) in sums.iter_mut().zip(g) {
            *sum += x;
        }
    }
    writeln!(
        out,
        "all\t-\t-\t-\t{}\t{}\t{}\t{}",
        sums[0], sums[1], sums[2], sums[3]
    )
    .unwrap();
    out
}

/// Missing rows by (cause, loops, total units, shifted lines, max units).
pub fn causes_tsv(rows: &[Row]) -> String {
    let mut groups: BTreeMap<(String, u8, u8, u8, u8), [usize; 2]> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.presentation.is_none()) {
        let cause = r.cause.map(|c| c.label()).unwrap_or_default();
        let g = groups
            .entry((cause, r.loops, r.total, r.shifted, r.max_units))
            .or_default();
        g[0] += 1;
        if r.sector_of.is_some() {
            g[1] += 1;
        }
    }
    let mut out = String::from(
        "cause\tloops\ttotal_units\tshifted_lines\tmax_units\tmissing\tmissing_but_sector\n",
    );
    for ((c, l, t, s, m), g) in &groups {
        writeln!(out, "{c}\t{l}\t{t}\t{s}\t{m}\t{}\t{}", g[0], g[1]).unwrap();
    }
    out
}

/// The production estimate as TSV.
pub fn closure_tsv(closure: &BTreeMap<(u8, bool, String, bool), usize>) -> String {
    let mut out = String::from("loops\teps\taction\tseries_parallel\tnew_nodes\n");
    let mut total = 0;
    for ((l, e, a, sp), n) in closure {
        writeln!(out, "{l}\t{e}\t{a}\t{sp}\t{n}").unwrap();
        total += n;
    }
    writeln!(out, "all\t-\t-\t-\t{total}").unwrap();
    out
}
