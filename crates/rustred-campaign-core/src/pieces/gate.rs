//! THE SHIP GATE (TRICKS item 1, gate half): over EVERYTHING handed to the import for a node, before anything is
//! called closed: 0 non-descending edges under the FULL node order AND an explicit, comparator-free acyclicity check.
//!
//! Streaming: relations are pushed one at a time; the descent of every edge is judged on arrival (only a bounded list
//! of offenders is kept); keys are interned ([`super::graph::KeyInterner`]). Edges are held in memory (default,
//! [`GateStorage::Memory`]: 12 bytes per edge while pushing + 4 bytes per edge of CSR at the end; 10^8 edges ~ 1.6 GB)
//! or spilled to disk ([`GateStorage::Disk`]: [`super::graph::DiskGraphBuilder`], the CSR written in chunks, Kahn over
//! positioned reads; memory ~ the keys only). Both give the same report. Zero-coefficient terms are not edges; a term
//! whose non-zero-ness is unknown IS an edge.
//!
//! If no edge is non-descending but the graph has a cycle, the verdict is [`GateVerdict::Inconsistent`]: the order
//! the descent was judged with is not a strict order on the keys as given (a comparator or frame bug).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::NonZero;
use super::graph::{Adjacency, DiskGraphBuilder, KeyInterner, MemGraph, kahn, sccs, witness_cycle};
use super::order::{EdgeOrder, KeyOrder, edge_order};

pub const GATE_SCHEMA: &str = "rustred.campaign.pieces-gate.v1";

/// Where the gate keeps its edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateStorage {
    /// In memory (fast; 12 + 4 bytes per edge).
    Memory,
    /// Spill and CSR files in this directory (removed at the end; use a disk without snapshots).
    Disk(PathBuf),
}

/// Configuration of a gate run.
#[derive(Clone, Debug)]
pub struct GateConfig {
    pub storage: GateStorage,
    /// Non-descending edges listed in the report (the smallest by (key, term, provenance)); all are counted.
    pub max_listed: usize,
    /// Disk mode: bytes of targets held in memory while the CSR file is written.
    pub chunk_bytes: usize,
    /// Cyclic SCCs listed (smallest member first) in the report.
    pub max_sccs_listed: usize,
}

impl GateConfig {
    /// Edges in memory (the default mode).
    pub fn memory() -> GateConfig {
        GateConfig {
            storage: GateStorage::Memory,
            max_listed: 1000,
            chunk_bytes: 256 << 20,
            max_sccs_listed: 20,
        }
    }
    /// Edges spilled to files in `spill_dir`.
    pub fn disk(spill_dir: impl Into<PathBuf>) -> GateConfig {
        GateConfig {
            storage: GateStorage::Disk(spill_dir.into()),
            ..GateConfig::memory()
        }
    }
}

impl Default for GateConfig {
    fn default() -> Self {
        GateConfig::memory()
    }
}

/// Streams every stored edge (src, dst, tag) into the callback.
type EachEdge<'a> = dyn Fn(&mut dyn FnMut(u32, u32, u32)) -> std::io::Result<()> + 'a;

/// The edge store of a running gate.
enum Edges {
    Memory {
        pairs: Vec<(u32, u32)>,
        tags: Vec<u32>,
        nodes: usize,
    },
    Disk(DiskGraphBuilder),
}

impl Edges {
    fn ensure_nodes(&mut self, n: usize) {
        match self {
            Edges::Memory { nodes, .. } => *nodes = (*nodes).max(n),
            Edges::Disk(d) => d.ensure_nodes(n),
        }
    }
    fn push(&mut self, s: u32, d: u32, tag: u32) -> std::io::Result<()> {
        match self {
            Edges::Memory { pairs, tags, nodes } => {
                pairs.push((s, d));
                tags.push(tag);
                *nodes = (*nodes).max(s.max(d) as usize + 1);
                Ok(())
            }
            Edges::Disk(g) => g.push(s, d, tag),
        }
    }
    fn edges(&self) -> u64 {
        match self {
            Edges::Memory { pairs, .. } => pairs.len() as u64,
            Edges::Disk(g) => g.edges(),
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Edges::Memory { pairs, tags, .. } => pairs.capacity() * 8 + tags.capacity() * 4,
            Edges::Disk(g) => g.bytes(),
        }
    }
}

#[derive(Debug)]
pub enum GateError {
    /// A key of another arity than the gate's.
    Arity {
        expected: usize,
        key: Vec<i32>,
    },
    Io(std::io::Error),
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateError::Arity { expected, key } => {
                write!(
                    f,
                    "pieces gate: key {key:?} has arity {} (gate arity {expected})",
                    key.len()
                )
            }
            GateError::Io(e) => write!(f, "pieces gate: i/o: {e}"),
        }
    }
}

impl std::error::Error for GateError {}

impl From<std::io::Error> for GateError {
    fn from(e: std::io::Error) -> Self {
        GateError::Io(e)
    }
}

/// One non-descending edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NonDescendingEdge {
    pub key: Vec<i32>,
    pub term: Vec<i32>,
    pub provenance: String,
    /// `equal` or `higher`.
    pub order: EdgeOrder,
    pub nonzero: NonZero,
}

/// One edge of the witness cycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleEdge {
    pub from: Vec<i32>,
    pub to: Vec<i32>,
    /// Provenance of the first spilled edge from -> to.
    pub provenance: String,
    pub order: EdgeOrder,
}

/// The comparator-free part of the report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphCheck {
    pub method: String,
    pub nodes: u64,
    pub kahn_processed: u64,
    /// Nodes left by Kahn (on a cycle or reachable from one).
    pub residual: u64,
    pub sccs: super::graph::SccSummary,
    /// Cyclic SCCs (size, smallest member), the first `max_sccs_listed` in Tarjan order.
    pub scc_list: Vec<(u64, Vec<i32>)>,
    pub witness_cycle: Vec<CycleEdge>,
    pub csr_passes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum GateVerdict {
    Pass,
    Fail {
        non_descending: u64,
        cyclic: bool,
    },
    /// No edge non-descending, yet a cycle: the order is not a strict order on these keys. Hard failure.
    Inconsistent,
}

impl GateVerdict {
    pub fn passed(&self) -> bool {
        matches!(self, GateVerdict::Pass)
    }
}

/// The gate's report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub schema: String,
    pub order: String,
    pub verdict: GateVerdict,
    pub relations: u64,
    pub edges: u64,
    pub zero_terms_skipped: u64,
    pub keys: u64,
    pub non_descending: u64,
    pub non_descending_equal: u64,
    pub non_descending_listed: Vec<NonDescendingEdge>,
    /// provenance -> [edges, non-descending].
    pub by_provenance: BTreeMap<String, [u64; 2]>,
    pub graph: GraphCheck,
    pub seconds_intake: f64,
    pub seconds_graph: f64,
    pub edges_per_second: f64,
    /// Interner + degree arrays at the end of intake, CSR offsets during the graph check (bytes).
    pub memory_bytes: u64,
    /// audit r2-planners G1: edges whose term is a first-route zero no second route confirmed
    /// ([`NonZero::ZeroUnconfirmed`]), and those of them that do not descend.
    #[serde(default)]
    pub zero_unconfirmed_edges: u64,
    #[serde(default)]
    pub zero_unconfirmed_non_descending: u64,
    /// Typed reasons of a verdict other than `pass` (`all_zeros_unconfirmed:N` = non-descending edges kept because their
    /// zero is unconfirmed, `non_descending:N` = the other non-descending edges, `cyclic`, `inconsistent`).
    #[serde(default)]
    pub reasons: Vec<String>,
}

/// The streaming gate. See the module documentation.
pub struct ShipGate<'o, O: KeyOrder + ?Sized> {
    order: &'o O,
    arity: usize,
    cfg: GateConfig,
    keys: KeyInterner,
    graph: Edges,
    prov: BTreeMap<String, u32>,
    prov_names: Vec<String>,
    by_prov: Vec<[u64; 2]>,
    relations: u64,
    zero: u64,
    nd: u64,
    nd_equal: u64,
    zu: u64,
    zu_nd: u64,
    listed: BTreeSet<NonDescendingEdge>,
    started: Instant,
}

impl<'o, O: KeyOrder + ?Sized> ShipGate<'o, O> {
    pub fn new(order: &'o O, arity: usize, cfg: GateConfig) -> Result<Self, GateError> {
        let graph = match &cfg.storage {
            GateStorage::Memory => Edges::Memory {
                pairs: Vec::new(),
                tags: Vec::new(),
                nodes: 0,
            },
            GateStorage::Disk(dir) => Edges::Disk(DiskGraphBuilder::new(dir)?),
        };
        Ok(ShipGate {
            order,
            arity,
            cfg,
            keys: KeyInterner::new(arity),
            graph,
            prov: BTreeMap::new(),
            prov_names: Vec::new(),
            by_prov: Vec::new(),
            relations: 0,
            zero: 0,
            nd: 0,
            nd_equal: 0,
            zu: 0,
            zu_nd: 0,
            listed: BTreeSet::new(),
            started: Instant::now(),
        })
    }

    fn check_arity(&self, k: &[i32]) -> Result<(), GateError> {
        if k.len() != self.arity {
            return Err(GateError::Arity {
                expected: self.arity,
                key: k.to_vec(),
            });
        }
        Ok(())
    }

    fn prov_id(&mut self, p: &str) -> u32 {
        if let Some(&i) = self.prov.get(p) {
            return i;
        }
        let i = self.prov_names.len() as u32;
        self.prov.insert(p.to_string(), i);
        self.prov_names.push(p.to_string());
        self.by_prov.push([0, 0]);
        i
    }

    /// One shipped relation `key -> terms`. The key is a node of the graph even if every term is zero.
    pub fn push_relation<'t, I>(
        &mut self,
        key: &[i32],
        provenance: &str,
        terms: I,
    ) -> Result<(), GateError>
    where
        I: IntoIterator<Item = (&'t [i32], NonZero)>,
    {
        self.check_arity(key)?;
        let k = self.keys.intern(key);
        self.graph.ensure_nodes(self.keys.len());
        let p = self.prov_id(provenance);
        self.relations += 1;
        for (t, nz) in terms {
            self.edge_inner(k, key, p, t, nz)?;
        }
        Ok(())
    }

    /// One shipped edge (a relation given term by term).
    pub fn push_edge(
        &mut self,
        key: &[i32],
        provenance: &str,
        term: &[i32],
        nonzero: NonZero,
    ) -> Result<(), GateError> {
        self.check_arity(key)?;
        let k = self.keys.intern(key);
        self.graph.ensure_nodes(self.keys.len());
        let p = self.prov_id(provenance);
        self.edge_inner(k, key, p, term, nonzero)
    }

    fn edge_inner(
        &mut self,
        k: u32,
        key: &[i32],
        p: u32,
        term: &[i32],
        nz: NonZero,
    ) -> Result<(), GateError> {
        self.check_arity(term)?;
        if nz == NonZero::Zero {
            self.zero += 1;
            return Ok(());
        }
        let t = self.keys.intern(term);
        self.graph.push(k, t, p)?;
        self.by_prov[p as usize][0] += 1;
        if nz == NonZero::ZeroUnconfirmed {
            self.zu += 1;
        }
        let o = edge_order(self.order, key, term);
        if o != EdgeOrder::Lower {
            self.nd += 1;
            if nz == NonZero::ZeroUnconfirmed {
                self.zu_nd += 1;
            }
            if o == EdgeOrder::Equal {
                self.nd_equal += 1;
            }
            self.by_prov[p as usize][1] += 1;
            if self.cfg.max_listed > 0 {
                let e = NonDescendingEdge {
                    key: key.to_vec(),
                    term: term.to_vec(),
                    provenance: self.prov_names[p as usize].clone(),
                    order: o,
                    nonzero: nz,
                };
                let full = self.listed.len() >= self.cfg.max_listed;
                if !full || self.listed.last().is_some_and(|l| &e < l) {
                    self.listed.insert(e);
                    if self.listed.len() > self.cfg.max_listed {
                        self.listed.pop_last();
                    }
                }
            }
        }
        Ok(())
    }

    /// The comparator-free check on a built graph: Kahn, then (if a residual is left) the witness cycle with the
    /// provenance of each of its edges and the cyclic SCCs.
    fn analyse<A: Adjacency>(
        &self,
        g: &A,
        mut indeg: Vec<u32>,
        each_edge: &EachEdge<'_>,
    ) -> Result<GraphCheck, GateError> {
        let nodes = indeg.len() as u64;
        let processed = kahn(g, &mut indeg, None)?;
        let residual: Vec<bool> = indeg.iter().map(|&d| d > 0).collect();
        drop(indeg);
        let mut graph = GraphCheck {
            nodes,
            kahn_processed: processed,
            residual: nodes - processed,
            ..GraphCheck::default()
        };
        if processed == nodes {
            return Ok(graph);
        }
        let cyc = witness_cycle(g, &residual)?;
        let mut list = Vec::new();
        let cap = self.cfg.max_sccs_listed;
        graph.sccs = sccs(g, &residual, |c, cyclic| {
            if cyclic && list.len() < cap {
                list.push((c.len() as u64, self.keys.key(c[0])));
            }
        })?;
        graph.scc_list = list;
        // provenance of each cycle edge: the first stored edge with that (from, to)
        let pairs: Vec<(u32, u32)> = (0..cyc.len())
            .map(|i| (cyc[i], cyc[(i + 1) % cyc.len()]))
            .collect();
        let want: BTreeMap<(u32, u32), usize> =
            pairs.iter().enumerate().map(|(i, &p)| (p, i)).collect();
        let mut tags: Vec<Option<u32>> = vec![None; pairs.len()];
        each_edge(&mut |s, d, tag| {
            if let Some(&i) = want.get(&(s, d))
                && tags[i].is_none()
            {
                tags[i] = Some(tag);
            }
        })?;
        graph.witness_cycle = pairs
            .iter()
            .zip(&tags)
            .map(|(&(s, d), tag)| {
                let (from, to) = (self.keys.key(s), self.keys.key(d));
                let order = edge_order(self.order, &from, &to);
                CycleEdge {
                    from,
                    to,
                    provenance: tag
                        .map(|t| self.prov_names[t as usize].clone())
                        .unwrap_or_default(),
                    order,
                }
            })
            .collect();
        Ok(graph)
    }

    pub fn edges(&self) -> u64 {
        self.graph.edges()
    }

    pub fn keys(&self) -> usize {
        self.keys.len()
    }

    /// Runs the graph check and returns the report.
    pub fn finish(mut self) -> Result<GateReport, GateError> {
        let seconds_intake = self.started.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let mem_intake = (self.keys.bytes() + self.graph.bytes()) as u64;
        let edges = self.graph.edges();
        self.graph.ensure_nodes(self.keys.len());
        let store = std::mem::replace(
            &mut self.graph,
            Edges::Memory {
                pairs: vec![],
                tags: vec![],
                nodes: 0,
            },
        );
        let (graph, mem_graph) = match store {
            Edges::Memory { pairs, tags, nodes } => {
                let g = MemGraph::from_edges(nodes, &pairs);
                let mem = (self.keys.bytes() + pairs.capacity() * 8 + tags.capacity() * 4) as u64
                    + edges * 4
                    + nodes as u64 * 16;
                let indeg = g.in_degrees();
                let each = |f: &mut dyn FnMut(u32, u32, u32)| -> std::io::Result<()> {
                    for (&(s, d), &t) in pairs.iter().zip(&tags) {
                        f(s, d, t);
                    }
                    Ok(())
                };
                let mut gc = self.analyse(&g, indeg, &each)?;
                gc.method = "kahn over the edges in memory; witness by a backward walk; tarjan over the residual".into();
                (gc, mem)
            }
            Edges::Disk(b) => {
                let mut g = b.finish(self.cfg.chunk_bytes)?;
                let mem = (self.keys.bytes() + g.bytes()) as u64;
                let indeg = std::mem::take(&mut g.indeg);
                let each = |f: &mut dyn FnMut(u32, u32, u32)| -> std::io::Result<()> {
                    g.for_each_edge(&mut *f)
                };
                let mut gc = self.analyse(&g, indeg, &each)?;
                gc.method = "kahn over the edges spilled to disk; witness by a backward walk; tarjan over the residual".into();
                gc.csr_passes = g.passes;
                (gc, mem)
            }
        };
        let (nodes, processed) = (graph.nodes, graph.kahn_processed);
        let cyclic = processed < nodes;
        let verdict = if self.nd == 0 && !cyclic {
            GateVerdict::Pass
        } else if self.nd == 0 {
            GateVerdict::Inconsistent
        } else {
            GateVerdict::Fail {
                non_descending: self.nd,
                cyclic,
            }
        };
        let mut reasons = vec![];
        if self.zu_nd > 0 {
            reasons.push(format!("all_zeros_unconfirmed:{}", self.zu_nd));
        }
        if self.nd > self.zu_nd {
            reasons.push(format!("non_descending:{}", self.nd - self.zu_nd));
        }
        match verdict {
            GateVerdict::Fail { cyclic: true, .. } => reasons.push("cyclic".into()),
            GateVerdict::Inconsistent => reasons.push("inconsistent".into()),
            _ => {}
        }
        let seconds_graph = t1.elapsed().as_secs_f64();
        let total = seconds_intake + seconds_graph;
        Ok(GateReport {
            schema: GATE_SCHEMA.into(),
            order: self.order.order_label(),
            verdict,
            relations: self.relations,
            edges,
            zero_terms_skipped: self.zero,
            keys: self.keys.len() as u64,
            non_descending: self.nd,
            non_descending_equal: self.nd_equal,
            non_descending_listed: std::mem::take(&mut self.listed).into_iter().collect(),
            by_provenance: self
                .prov_names
                .iter()
                .cloned()
                .zip(self.by_prov.iter().copied())
                .collect(),
            graph,
            seconds_intake,
            seconds_graph,
            edges_per_second: if total > 0.0 {
                edges as f64 / total
            } else {
                0.0
            },
            memory_bytes: mem_intake.max(mem_graph),
            zero_unconfirmed_edges: self.zu,
            zero_unconfirmed_non_descending: self.zu_nd,
            reasons,
        })
    }
}
