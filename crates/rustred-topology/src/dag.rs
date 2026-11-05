//! The reduction DAG (Forcer's createTree, generation/pathforcer.py:1543-1667, thesis Alg. 2) and the cached topology
//! service that builds it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use serde_json::{Value, json};

use crate::matchers::{action_kind, action_with};
use crate::{
    Action, ActionKind, Fate, GraphKey, Normalised, P_TOPOLOGIES_SCHEMA, P_TOPOLOGY_VERSION, PTopo,
    canonical, is_series_parallel, normalise,
};

/// Which children a DAG node gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChildMode {
    /// Forcer's createTree (production and golden): the rule output of an insertion or a carpet, plus the contraction
    /// of every integer line that is not in a multi-edge (pathforcer.py:1576-1592). Closed under the single integer
    /// line contractions that run-time keys reach.
    ForcerTree,
    /// Diagnostic: only what the action needs. Insertion / carpet: the rule output; triangle: contractions of `b`,
    /// `c1`, `c2`; diamond: of every `b_i` and `c_i`; custom: of every integer line.
    Demand,
}

/// How a DAG edge arises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DagEdgeKind {
    /// The output of the parent's insertion or carpet rule.
    Rule,
    /// Contraction of the parent's canonical line (power 0).
    Contract(u8),
}

/// One DAG edge; `fates[i]` = what became of the parent's canonical line `i` in the child's canonical labelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DagEdge {
    pub parent: u32,
    pub child: u32,
    pub kind: DagEdgeKind,
    pub fates: Vec<Fate>,
}

/// One DAG node. `topo` is normalised and canonically labelled; `t` is the number of lines (the level: every child
/// has fewer lines); `children` are the distinct child indices (ascending).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeInfo {
    pub index: u32,
    pub key: GraphKey,
    pub topo: PTopo,
    pub loops: u8,
    pub t: u8,
    pub eps_lines: u8,
    pub action: Action,
    pub series_parallel: bool,
    pub children: Vec<u32>,
}

/// A reduction DAG: nodes ordered by level (`t` descending, then loops descending, then key bytes ascending), so
/// every edge points to a larger index; edges ordered by parent, then rule edge first, then contracted line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dag {
    pub nodes: Vec<NodeInfo>,
    pub edges: Vec<DagEdge>,
}

impl Dag {
    /// The node with this key (binary search is not possible on the level order: linear).
    pub fn find(&self, key: &GraphKey) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| &n.key == key)
    }

    /// Edges leaving node `index`.
    pub fn edges_from(&self, index: u32) -> impl Iterator<Item = &DagEdge> {
        self.edges.iter().filter(move |e| e.parent == index)
    }
}

/// Everything about one canonical topology that does not depend on a particular DAG; cached by key.
#[derive(Debug)]
struct Analysis {
    key: GraphKey,
    topo: PTopo,
    action: Action,
    series_parallel: bool,
    /// ForcerTree edges: the rule edge first, then contractions by line; zero children are absent.
    edges: Vec<ChildEdge>,
}

#[derive(Debug)]
struct ChildEdge {
    kind: DagEdgeKind,
    key: GraphKey,
    topo: PTopo,
    fates: Vec<Fate>,
}

impl Analysis {
    fn selected(&self, mode: ChildMode) -> impl Iterator<Item = &ChildEdge> {
        let demand_lines: Option<Vec<u8>> = match (&self.action, mode) {
            (_, ChildMode::ForcerTree) => None,
            (Action::Triangle(m), ChildMode::Demand) => Some(vec![m.b, m.c1, m.c2]),
            (Action::Diamond(m), ChildMode::Demand) => {
                Some(m.b.iter().chain(&m.c).copied().collect())
            }
            (Action::Custom, ChildMode::Demand) => Some((0..self.topo.lines.len() as u8).collect()),
            (Action::Trivial | Action::Insertion(_) | Action::Carpet(_), ChildMode::Demand) => {
                Some(Vec::new())
            }
        };
        let rule_only = mode == ChildMode::Demand
            && matches!(self.action, Action::Insertion(_) | Action::Carpet(_));
        self.edges
            .iter()
            .filter(move |e| match (&demand_lines, e.kind) {
                (None, _) => true,
                (Some(_), DagEdgeKind::Rule) => rule_only,
                (Some(lines), DagEdgeKind::Contract(l)) => lines.contains(&l),
            })
    }
}

/// Where a parent line went before normalisation.
#[derive(Clone, Copy)]
enum Pre {
    Line(usize),
    Absorbed,
    Contracted,
}

/// The child produced by an operation on a canonical parent, with fates in the child's canonical labelling.
fn child_after(lines: Vec<(u8, u8, u8)>, n: u8, s: u8, t: u8, pre: Vec<Pre>) -> Option<ChildEdge> {
    let inter = PTopo {
        n,
        s,
        t,
        lines: lines.into_iter().collect(),
    };
    let (key, topo, image): (GraphKey, PTopo, Vec<Option<u8>>) = match normalise(&inter) {
        Normalised::Zero => return None,
        Normalised::Trivial => {
            let trivial = PTopo::trivial();
            (canonical(&trivial).0, trivial, vec![None; pre.len()])
        }
        Normalised::Topo(p, nf) => {
            let (key, canon) = canonical(&p);
            let topo = canon.apply(&p);
            let image = pre
                .iter()
                .map(|x| match *x {
                    Pre::Line(j) => nf[j].line().map(|k| canon.line_map[usize::from(k)]),
                    Pre::Absorbed | Pre::Contracted => None,
                })
                .collect();
            (key, topo, image)
        }
    };
    let mut count = vec![0usize; topo.lines.len()];
    for c in image.iter().flatten() {
        count[usize::from(*c)] += 1;
    }
    let fates = pre
        .iter()
        .zip(&image)
        .map(|(p, img)| match (p, img) {
            (Pre::Contracted, _) => Fate::Contracted,
            (_, Some(c)) if count[usize::from(*c)] == 1 => Fate::Kept(*c),
            (_, Some(c)) => Fate::SeriesMergedInto(*c),
            (_, None) => Fate::AbsorbedIntoQ,
        })
        .collect();
    Some(ChildEdge {
        kind: DagEdgeKind::Rule,
        key,
        topo,
        fates,
    })
}

fn contraction_edge(t: &PTopo, line: usize) -> Option<ChildEdge> {
    let (a, b, _) = t.lines[line];
    let relabel = |x: u8| if x == b { a } else { x };
    let mut lines = Vec::with_capacity(t.lines.len() - 1);
    let mut pre = Vec::with_capacity(t.lines.len());
    for (i, &(u, v, e)) in t.lines.iter().enumerate() {
        if i == line {
            pre.push(Pre::Contracted);
        } else {
            pre.push(Pre::Line(lines.len()));
            lines.push((relabel(u), relabel(v), e));
        }
    }
    let mut edge = child_after(lines, t.n, relabel(t.s), relabel(t.t), pre)?;
    edge.kind = DagEdgeKind::Contract(line as u8);
    Some(edge)
}

fn rule_edge(t: &PTopo, action: &Action) -> Option<ChildEdge> {
    let mut lines = Vec::with_capacity(t.lines.len());
    let mut pre = Vec::with_capacity(t.lines.len());
    match action {
        Action::Insertion(ins) => {
            let (keep, drop) = (usize::from(ins.keep), usize::from(ins.drop));
            let mut keep_at = 0;
            for (i, &(u, v, e)) in t.lines.iter().enumerate() {
                if i == drop {
                    pre.push(Pre::Line(usize::MAX)); // patched below
                    continue;
                }
                if i == keep {
                    keep_at = lines.len();
                    let eps = e
                        .checked_add(t.lines[drop].2)
                        .and_then(|x| x.checked_add(1))
                        .expect("rustred-topology: eps units overflow u8");
                    lines.push((u, v, eps));
                } else {
                    lines.push((u, v, e));
                }
                pre.push(Pre::Line(lines.len() - 1));
            }
            pre[drop] = Pre::Line(keep_at);
        }
        Action::Carpet(c) => {
            for (i, &(u, v, e)) in t.lines.iter().enumerate() {
                if i == usize::from(c.r) {
                    pre.push(Pre::Absorbed);
                } else {
                    pre.push(Pre::Line(lines.len()));
                    lines.push((u, v, e));
                }
            }
        }
        _ => return None,
    }
    child_after(lines, t.n, t.s, t.t, pre)
}

fn analyse(key: GraphKey, topo: PTopo) -> Analysis {
    let mut multiplicity: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for &(u, v, _) in &topo.lines {
        *multiplicity.entry((u.min(v), u.max(v))).or_default() += 1;
    }
    let contractions: Vec<Option<ChildEdge>> = topo
        .lines
        .iter()
        .enumerate()
        .map(|(i, &(u, v, e))| {
            if e == 0 && multiplicity[&(u.min(v), u.max(v))] == 1 {
                contraction_edge(&topo, i)
            } else {
                None
            }
        })
        .collect();
    let action = action_with(&topo, &mut |line| {
        contractions[line]
            .as_ref()
            .map(|c| (c.key.clone(), action_kind(&c.topo) == ActionKind::Custom))
    });
    let mut edges: Vec<ChildEdge> = rule_edge(&topo, &action).into_iter().collect();
    edges.extend(contractions.into_iter().flatten());
    let series_parallel = is_series_parallel(&topo);
    Analysis {
        key,
        topo,
        action,
        series_parallel,
        edges,
    }
}

const SHARDS: usize = 16;

fn shard_of(key: &GraphKey) -> usize {
    // FNV-1a: deterministic and cheap; only selects a shard
    let h = key
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
    (h % SHARDS as u64) as usize
}

/// Cached topology analysis (canonical form, action, children with fates) keyed by [`GraphKey`], shared by every
/// [`TopologyService::reduction_graph`] call on the same service. Sharded `RwLock`s; hit/miss counters.
#[derive(Debug)]
pub struct TopologyService {
    shards: Vec<RwLock<HashMap<GraphKey, Arc<Analysis>>>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for TopologyService {
    fn default() -> Self {
        Self::new()
    }
}

impl TopologyService {
    pub fn new() -> Self {
        TopologyService {
            shards: (0..SHARDS).map(|_| RwLock::new(HashMap::new())).collect(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// `(hits, misses)` of the analysis cache since creation.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Number of cached analyses.
    pub fn cached(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.read().expect("topology cache poisoned").len())
            .sum()
    }

    fn analysis(&self, key: &GraphKey, topo: &PTopo) -> Arc<Analysis> {
        let shard = &self.shards[shard_of(key)];
        if let Some(a) = shard.read().expect("topology cache poisoned").get(key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Arc::clone(a);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let fresh = Arc::new(analyse(key.clone(), topo.clone()));
        let mut w = shard.write().expect("topology cache poisoned");
        Arc::clone(w.entry(key.clone()).or_insert(fresh))
    }

    /// The reduction DAG of `roots` (any labelling, normalised here; zero roots are skipped) in the given child mode.
    pub fn reduction_graph(&self, roots: &[PTopo], mode: ChildMode) -> Dag {
        let mut stack: Vec<(GraphKey, PTopo)> = Vec::new();
        for root in roots {
            match normalise(root) {
                Normalised::Zero => {}
                Normalised::Trivial => {
                    let trivial = PTopo::trivial();
                    stack.push((canonical(&trivial).0, trivial));
                }
                Normalised::Topo(p, _) => {
                    let (key, canon) = canonical(&p);
                    stack.push((key, canon.apply(&p)));
                }
            }
        }
        stack.reverse(); // roots in input order first
        let mut seen: HashMap<GraphKey, Arc<Analysis>> = HashMap::new();
        while let Some((key, topo)) = stack.pop() {
            if seen.contains_key(&key) {
                continue;
            }
            let a = self.analysis(&key, &topo);
            for e in a.selected(mode) {
                if !seen.contains_key(&e.key) {
                    stack.push((e.key.clone(), e.topo.clone()));
                }
            }
            seen.insert(key, a);
        }
        let mut order: Vec<Arc<Analysis>> = seen.into_values().collect();
        order.sort_by(|x, y| {
            (y.topo.lines.len(), y.topo.loops(), &x.key).cmp(&(
                x.topo.lines.len(),
                x.topo.loops(),
                &y.key,
            ))
        });
        let index: HashMap<&GraphKey, u32> = order
            .iter()
            .enumerate()
            .map(|(i, a)| (&a.key, i as u32))
            .collect();
        let mut dag = Dag::default();
        for (i, a) in order.iter().enumerate() {
            let i = i as u32;
            let mut children: Vec<u32> = Vec::new();
            for e in a.selected(mode) {
                let child = index[&e.key];
                children.push(child);
                dag.edges.push(DagEdge {
                    parent: i,
                    child,
                    kind: e.kind,
                    fates: e.fates.clone(),
                });
            }
            children.sort_unstable();
            children.dedup();
            dag.nodes.push(NodeInfo {
                index: i,
                key: a.key.clone(),
                topo: a.topo.clone(),
                loops: a.topo.loops(),
                t: a.topo.line_count(),
                eps_lines: a.topo.eps_lines(),
                action: a.action.clone(),
                series_parallel: a.series_parallel,
                children,
            });
        }
        debug_assert!(dag.edges.iter().all(|e| e.child > e.parent));
        debug_assert_eq!(
            dag.nodes
                .iter()
                .map(|n| &n.key)
                .collect::<HashSet<_>>()
                .len(),
            dag.nodes.len()
        );
        dag
    }
}

/// [`TopologyService::reduction_graph`] on a fresh service.
pub fn reduction_graph(roots: &[PTopo], mode: ChildMode) -> Dag {
    TopologyService::new().reduction_graph(roots, mode)
}

fn action_json(a: &Action) -> Value {
    let kind = a.kind().as_str();
    match a {
        Action::Trivial | Action::Custom => json!({ "kind": kind }),
        Action::Insertion(m) => json!({ "kind": kind, "keep": m.keep, "drop": m.drop }),
        Action::Carpet(m) => json!({
            "kind": kind, "r": m.r, "third": m.third, "blob": m.blob.as_slice(),
            "blob_loops": m.blob_loops, "blob_eps": m.blob_eps,
        }),
        Action::Triangle(m) => json!({
            "kind": kind, "a1": m.a1, "a2": m.a2, "b": m.b, "c1": m.c1, "c2": m.c2,
            "apex": m.apex, "v_a": m.v_a, "v_b": m.v_b,
        }),
        Action::Diamond(m) => json!({
            "kind": kind, "a": m.a.as_slice(), "b": m.b.as_slice(), "c": m.c.as_slice(),
            "spect": m.spect.as_slice(), "top": m.top, "bottom": m.bottom,
        }),
    }
}

/// The DAG as JSON, schema `rustred.p-topologies.v1`: `{schema, p_topology_version, nodes: [{index, key (hex),
/// topology: {n, s, t, lines: [[u, v, eps]]}, loops, t, eps_lines, action: {kind, ..match data}, series_parallel,
/// children}], edges: [{parent, child, kind: "rule" | "contract", line (contract only), fates}]}`.
pub fn topologies_json(d: &Dag) -> Value {
    let nodes: Vec<Value> = d
        .nodes
        .iter()
        .map(|n| {
            json!({
                "index": n.index,
                "key": n.key.to_hex(),
                "topology": {
                    "n": n.topo.n, "s": n.topo.s, "t": n.topo.t,
                    "lines": n.topo.lines.iter().map(|&(u, v, e)| [u, v, e]).collect::<Vec<_>>(),
                },
                "loops": n.loops,
                "t": n.t,
                "eps_lines": n.eps_lines,
                "action": action_json(&n.action),
                "series_parallel": n.series_parallel,
                "children": n.children,
            })
        })
        .collect();
    let edges: Vec<Value> = d
        .edges
        .iter()
        .map(|e| {
            let mut v = json!({
                "parent": e.parent,
                "child": e.child,
                "kind": match e.kind { DagEdgeKind::Rule => "rule", DagEdgeKind::Contract(_) => "contract" },
                "fates": e.fates,
            });
            if let DagEdgeKind::Contract(l) = e.kind {
                v["line"] = json!(l);
            }
            v
        })
        .collect();
    json!({
        "schema": P_TOPOLOGIES_SCHEMA,
        "p_topology_version": P_TOPOLOGY_VERSION,
        "nodes": nodes,
        "edges": edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topo(n: u8, s: u8, t: u8, lines: &[(u8, u8, u8)]) -> PTopo {
        PTopo {
            n,
            s,
            t,
            lines: lines.iter().copied().collect(),
        }
    }

    #[test]
    fn one_loop_bubble_reduces_to_the_trivial_node() {
        let dag = reduction_graph(
            &[topo(2, 0, 1, &[(0, 1, 0), (0, 1, 0)])],
            ChildMode::ForcerTree,
        );
        assert_eq!(dag.nodes.len(), 2);
        assert_eq!(
            dag.nodes[0].action,
            Action::Insertion(crate::Ins { keep: 0, drop: 1 })
        );
        assert_eq!(dag.nodes[1].action, Action::Trivial);
        assert_eq!(dag.edges.len(), 1);
        assert_eq!(dag.edges[0].kind, DagEdgeKind::Rule);
        assert_eq!(
            dag.edges[0].fates,
            vec![Fate::AbsorbedIntoQ, Fate::AbsorbedIntoQ]
        );
    }

    #[test]
    fn kite_dag_and_fates() {
        let kite = topo(
            4,
            0,
            3,
            &[(0, 1, 0), (0, 2, 0), (1, 2, 0), (1, 3, 0), (2, 3, 0)],
        );
        let service = TopologyService::new();
        let dag = service.reduction_graph(&[kite], ChildMode::ForcerTree);
        // kite -> {2L sunset+bubble (contract a side), bubble chain (contract rung)} -> ...
        assert_eq!(dag.nodes[0].action.kind(), ActionKind::Triangle);
        for e in dag.edges_from(0) {
            let DagEdgeKind::Contract(l) = e.kind else {
                panic!("triangle has no rule edge")
            };
            assert_eq!(e.fates[usize::from(l)], Fate::Contracted);
            let child = &dag.nodes[e.child as usize];
            let kept: Vec<u8> = e.fates.iter().filter_map(|f| f.line()).collect();
            assert_eq!(kept.len(), child.topo.lines.len());
        }
        let (hits, misses) = service.stats();
        assert_eq!(misses as usize, dag.nodes.len());
        let again = service.reduction_graph(
            &[topo(
                4,
                3,
                0,
                &[(3, 1, 0), (3, 2, 0), (1, 2, 0), (1, 0, 0), (2, 0, 0)],
            )],
            ChildMode::ForcerTree,
        );
        assert_eq!(again, dag);
        assert_eq!(service.stats(), (hits + dag.nodes.len() as u64, misses));
        let json = topologies_json(&dag);
        assert_eq!(json["schema"], P_TOPOLOGIES_SCHEMA);
        assert_eq!(
            json["nodes"].as_array().map(Vec::len),
            Some(dag.nodes.len())
        );
    }
}
