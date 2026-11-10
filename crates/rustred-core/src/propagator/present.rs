//! Lane-adapted node presentations and edge maps (owner: M2-T3 rr-lanes).
//!
//! A presentation is a pure function of the node's `GraphKey`, its lane, the
//! tops and [`P_PRESENTATION_VERSION`]; any change of the routing, the slot
//! order, the ISP choice or an edge map bumps that constant and the golden
//! dumps of `tests/p_library/golden/`.
//!
//! Every presentation but the Induced one has line slot `i` = canonical line
//! `i`, oriented from `u` to `v` of the canonical line `(u, v, eps)`, then the
//! ISPs of [`IspCompletion`]: unit rows, so an ISP is `k_i.k_j` or `k_i.q`. A
//! spanning tree routes the momenta: the loop momenta are the cotree lines, the
//! adapted loop ([`ADAPTED_LOOP`]) first and the others by ascending line, and
//! `q` runs along the tree from `s` to `t`.
//! - Insertion: the tree holds the second bubble line and not the first, so
//!   `k_0` runs through the pair alone.
//! - Triangle: the tree holds `a1`, `a2` and not `b`, so `k_0 = mom(b)` runs
//!   through `b`, `a1`, `a2` alone.
//! - Carpet: the tree spans the graph without `r` and `p = k_0` runs along it
//!   from `s` to `t`, so `q` is routed through `r = q - p` alone and `third`,
//!   the bridge of that graph, is `p`. The scan of [`IspCompletion`] takes every
//!   `k_i.k_j` before any `k_i.q`, and `p^2` and `r` then span `p.q`.
//! - Diamond: the tree holds every `a` and the last `b`, not the other `b` nor
//!   the spectator, so loop `i < L` runs through `b[i]`, loop `L` through the
//!   spectator, and each closes through `a[i]` and the last path alone.
//! - Custom with eps lines: the default tree, by ascending line.
//! - Custom without eps lines: the Induced presentation, a sector of a vakint
//!   top with the top's slots and family.
//!
//! [`P_PRESENTATION_VERSION`]: super::P_PRESENTATION_VERSION
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::algebra::CoefficientContext;
use crate::family::isp::{IspCompletion, IspCompletionError};
use crate::family::{AffineDenominator, ScalarProductCoordinate};
use crate::reduction::ReductionError;

use super::census::SignedAffineLoopMap;
use super::decide::{ADAPTED_LOOP, CUSTOM, decide};
use super::diamond::check_diamond_contract;
use super::error::PError;
use super::families::{P_FAMILIES_V1_JSON, PFamiliesFile, PFamilyEntry};
use super::lane::{DiamondRoles, LaneDecision, LaneRoles, OwnerLane};
use super::presentation::{
    EdgeKind, EdgeMap, NodeRef, P_EXTERNAL_SQUARE, Presentation, PresentationKind, Slot, SlotIsp,
    SlotLine, check_carpet_contract, check_insertion_contract, check_triangle_contract,
    eps_power_shift, verify_edge_map,
};
use super::topology::{
    DagEdge, DagEdgeKind, GraphKey, NodeInfo, Normalised, PTopo, canonical, normalise,
};
use super::{NodeIndex, PresentationId};

fn broken(detail: impl fmt::Display) -> PError {
    PError::Contract(detail.to_string())
}

/// A spanning forest of `topo`, as lines.
struct Tree<'a> {
    topo: &'a PTopo,
    lines: Vec<usize>,
}

impl<'a> Tree<'a> {
    /// Kruskal over the lines of `first`, then every line by ascending index,
    /// without the lines of `excluded`.
    fn grow(topo: &'a PTopo, first: &[usize], excluded: &[usize]) -> Self {
        let mut parent: Vec<usize> = (0..usize::from(topo.n)).collect();
        let mut lines = Vec::new();
        let order = first.iter().copied().chain(0..topo.lines.len());
        for line in order.filter(|line| !excluded.contains(line)) {
            let (u, v, _) = topo.lines[line];
            let (a, b) = (
                find(&mut parent, usize::from(u)),
                find(&mut parent, usize::from(v)),
            );
            if a != b {
                parent[a.max(b)] = a.min(b);
                lines.push(line);
            }
        }
        Self { topo, lines }
    }

    /// The tree path from `from` to `to`: lines with the sign of a momentum
    /// flowing along the path, `+1` from `u` to `v`.
    fn path(&self, from: u8, to: u8) -> Result<Vec<(usize, i64)>, PError> {
        let mut via = vec![None; usize::from(self.topo.n)];
        let mut stack = vec![from];
        while let Some(vertex) = stack.pop() {
            for &line in &self.lines {
                let (u, v, _) = self.topo.lines[line];
                let (next, sign) = match vertex {
                    x if x == u => (v, 1),
                    x if x == v => (u, -1),
                    _ => continue,
                };
                if next != from && via[usize::from(next)].is_none() {
                    via[usize::from(next)] = Some((vertex, line, sign));
                    stack.push(next);
                }
            }
        }
        let mut steps = Vec::new();
        let mut at = to;
        while at != from {
            let (previous, line, sign) = via[usize::from(at)]
                .ok_or_else(|| broken(format!("no tree path from {from} to {to}")))?;
            steps.push((line, sign));
            at = previous;
        }
        Ok(steps)
    }

    /// Lines outside the tree and outside `excluded`, the line `first` in front.
    fn cotree(&self, first: Option<usize>, excluded: &[usize]) -> Vec<usize> {
        let free = (0..self.topo.lines.len())
            .filter(|line| !self.lines.contains(line) && !excluded.contains(line));
        first
            .into_iter()
            .chain(free.filter(|line| Some(*line) != first))
            .collect()
    }

    /// Column `column + i` runs through `cotree[i]` and back along the tree.
    fn add_cycles(
        &self,
        rows: &mut [Vec<i64>],
        cotree: &[usize],
        column: usize,
    ) -> Result<(), PError> {
        for (offset, &line) in cotree.iter().enumerate() {
            let (u, v, _) = self.topo.lines[line];
            rows[line][column + offset] = 1;
            for (step, sign) in self.path(v, u)? {
                rows[step][column + offset] += sign;
            }
        }
        Ok(())
    }

    /// Column `column` runs along the tree from `s` to `t`.
    fn add_external(&self, rows: &mut [Vec<i64>], column: usize) -> Result<(), PError> {
        for (step, sign) in self.path(self.topo.s, self.topo.t)? {
            rows[step][column] += sign;
        }
        Ok(())
    }
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// The line momenta over `(k_1, .., k_L, q)` with `first` the line of loop 0.
fn routed(
    node: &NodeInfo,
    first: Option<usize>,
    tree_first: &[usize],
) -> Result<Vec<Vec<i64>>, PError> {
    let topo = &node.topo;
    let loops = usize::from(node.loops);
    let excluded: Vec<usize> = first.into_iter().collect();
    let tree = Tree::grow(topo, tree_first, &excluded);
    let cotree = tree.cotree(first, &[]);
    if cotree.len() != loops {
        return Err(broken(format!(
            "{} cotree lines for {loops} loops",
            cotree.len()
        )));
    }
    let mut rows = vec![vec![0; loops + 1]; topo.lines.len()];
    tree.add_cycles(&mut rows, &cotree, 0)?;
    tree.add_external(&mut rows, loops)?;
    Ok(rows)
}

/// The carpet routing: `p = k_0` along the tree of the graph without `r`, the
/// blob loops after it, `r` carries `q - p` from `s` to `t`.
fn routed_carpet(node: &NodeInfo, r: usize) -> Result<Vec<Vec<i64>>, PError> {
    let topo = &node.topo;
    let loops = usize::from(node.loops);
    let tree = Tree::grow(topo, &[], &[r]);
    let cotree = tree.cotree(None, &[r]);
    if cotree.len() + 1 != loops {
        return Err(broken(format!(
            "{} blob loops for {loops} loops",
            cotree.len()
        )));
    }
    let mut rows = vec![vec![0; loops + 1]; topo.lines.len()];
    tree.add_cycles(&mut rows, &cotree, 1)?;
    tree.add_external(&mut rows, 0)?;
    let (u, v, _) = topo.lines[r];
    let sign = match (u, v) {
        (a, b) if (a, b) == (topo.s, topo.t) => 1,
        (a, b) if (b, a) == (topo.s, topo.t) => -1,
        _ => return Err(broken(format!("line {r} does not join s and t"))),
    };
    rows[r][0] = -sign;
    rows[r][loops] = sign;
    Ok(rows)
}

/// The diamond routing of the module docs: the loops `0..L + S` are the
/// cotree lines `b[0..L]` and the spectator, in this order.
fn routed_diamond(node: &NodeInfo, roles: &DiamondRoles) -> Result<Vec<Vec<i64>>, PError> {
    let topo = &node.topo;
    let loops = usize::from(node.loops);
    let last = roles.paths() - 1;
    let index = |slot: &u16| usize::from(*slot);
    let first: Vec<usize> = (roles.b()[..last].iter().chain(&roles.spectator))
        .map(index)
        .collect();
    let tree_first: Vec<usize> = (roles.a().iter().chain(&roles.b()[last..]))
        .map(index)
        .collect();
    let tree = Tree::grow(topo, &tree_first, &first);
    let mut cotree = first.clone();
    cotree.extend(tree.cotree(None, &first));
    if cotree.len() != loops {
        return Err(broken(format!(
            "{} cotree lines for {loops} loops",
            cotree.len()
        )));
    }
    let mut rows = vec![vec![0; loops + 1]; topo.lines.len()];
    tree.add_cycles(&mut rows, &cotree, 0)?;
    tree.add_external(&mut rows, loops)?;
    Ok(rows)
}

/// `mom^2 = constant + row . S` over the coordinates of the family.
fn square_row(momentum: &[i64]) -> (i64, Vec<i64>) {
    let (&q, loop_part) = momentum.split_last().expect("loops >= 1");
    let products = (0..loop_part.len()).flat_map(|left| {
        (left..loop_part.len()).map(move |right| {
            let factor = if left == right { 1 } else { 2 };
            factor * loop_part[left] * loop_part[right]
        })
    });
    let externals = loop_part.iter().map(|&k| 2 * k * q);
    (
        q * q * P_EXTERNAL_SQUARE,
        products.chain(externals).collect(),
    )
}

fn unit_row(width: usize, position: usize) -> Vec<i64> {
    let mut row = vec![0; width];
    row[position] = 1;
    row
}

/// Family name of a node presentation: a function of the key alone.
fn node_family_name(node: &NodeInfo) -> String {
    format!("p_node_{}", node.key.to_hex())
}

fn node_reference(node: &NodeInfo) -> NodeRef {
    NodeRef {
        index: NodeIndex(node.index),
        graph_key: Arc::clone(&node.key.0),
    }
}

/// Dependent denominators are a routing that cannot exist, so the lane is at
/// fault. Nothing else says anything about the lane: a resource limit or an
/// exhausted budget must not move a node to another lane.
fn completion_error(node: &NodeInfo, error: IspCompletionError) -> PError {
    let detail = format!("node {}: {error}", node.index);
    match error {
        IspCompletionError::DependentInputDenominators { .. } => PError::Contract(detail),
        IspCompletionError::ExactAlgebra(error) => ReductionError::ExactAlgebra(error).into(),
        _ => PError::Generation(detail),
    }
}

/// Environment variable of the experimental generation-time ISP choice (M5
/// maps): a JSON file `{"<node family name>": [coordinate ordinals], ..}`.
/// The listed scalar-product coordinates (LoopLoop by `(left, right)`, then
/// LoopExternal) are scanned first by [`IspCompletion`]. Unset, every
/// presentation is the default one. A package generated with it has other
/// presentations for the same [`P_PRESENTATION_VERSION`]: experimental only;
/// adopting it means a version bump.
///
/// [`P_PRESENTATION_VERSION`]: super::P_PRESENTATION_VERSION
pub const P_ISP_PREFERENCE_ENV: &str = "RUSTRED_P_ISP_PREFERENCE";

fn isp_preference(name: &str) -> Result<Vec<usize>, PError> {
    static TABLE: OnceLock<Result<BTreeMap<String, Vec<usize>>, String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| match std::env::var(P_ISP_PREFERENCE_ENV) {
        Err(_) => Ok(BTreeMap::new()),
        Ok(path) => std::fs::read_to_string(&path)
            .map_err(|error| format!("{path}: {error}"))
            .and_then(|text| serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))),
    });
    let table = table
        .as_ref()
        .map_err(|error| PError::Generation(format!("{P_ISP_PREFERENCE_ENV}: {error}")))?;
    Ok(table.get(name).cloned().unwrap_or_default())
}

/// The presentation of `rows`: line slots, then the ISPs of [`IspCompletion`].
fn with_isps(
    node: &NodeInfo,
    rows: Vec<Vec<i64>>,
    kind: PresentationKind,
    id: PresentationId,
    context: &CoefficientContext,
) -> Result<Presentation, PError> {
    let name = node_family_name(node);
    let loops = usize::from(node.loops);
    let dimension = context.parameter("d").ok_or(PError::ContextMismatch)?;
    let denominators = rows
        .iter()
        .map(|momentum| {
            let (constant, row) = square_row(momentum);
            AffineDenominator::new(
                context.integer(constant),
                row.into_iter().map(|c| context.integer(c)).collect(),
            )
        })
        .collect();
    let shifts = node
        .topo
        .lines
        .iter()
        .map(|&(_, _, eps_units)| eps_power_shift(context, eps_units))
        .collect::<Result<_, _>>()?;
    let preferred = isp_preference(name.as_str())?;
    let completion = IspCompletion::try_new_with_preference(
        name.as_str(),
        (1..=loops).map(|l| format!("k{l}")).collect(),
        vec!["q".to_owned()],
        context.clone(),
        dimension,
        denominators,
        vec![vec![context.integer(P_EXTERNAL_SQUARE)]],
        shifts,
        Default::default(),
        &preferred,
    )
    .map_err(|error| completion_error(node, error))?;
    let width = loops + 1;
    let isps = completion
        .appended_coordinate_ordinals()
        .iter()
        .map(
            |&ordinal| match completion.family().coordinates()[ordinal] {
                ScalarProductCoordinate::LoopLoop { left, right } => Slot::Isp(SlotIsp {
                    left: unit_row(width, left),
                    right: unit_row(width, right),
                }),
                ScalarProductCoordinate::LoopExternal { loop_index, .. } => Slot::Isp(SlotIsp {
                    left: unit_row(width, loop_index),
                    right: unit_row(width, loops),
                }),
            },
        )
        .collect::<Vec<_>>();
    let lines = rows
        .into_iter()
        .zip(&node.topo.lines)
        .map(|(momentum, &(_, _, eps_units))| {
            Slot::Line(SlotLine {
                momentum,
                eps_units,
            })
        });
    let presentation = Presentation {
        id,
        kind,
        loops: node.loops,
        slots: lines.chain(isps).collect(),
        family: Arc::new(completion.into_family()),
        node: Some(node_reference(node)),
    };
    presentation.validate()?;
    Ok(presentation)
}

/// The lines a decision names.
fn role_lines(roles: LaneRoles) -> Vec<u16> {
    match roles {
        LaneRoles::None => Vec::new(),
        LaneRoles::Insertion(roles) => roles.lines.to_vec(),
        LaneRoles::Carpet(roles) => [roles.r].into_iter().chain(roles.third).collect(),
        LaneRoles::Triangle(roles) => {
            vec![roles.b, roles.a[0], roles.a[1], roles.c[0], roles.c[1]]
        }
        LaneRoles::Diamond(roles) => (roles.a().iter().chain(roles.b()).chain(roles.c()))
            .chain(&roles.spectator)
            .copied()
            .collect(),
    }
}

/// The presentation of a rule lane, contract-checked.
fn adapted(
    node: &NodeInfo,
    decision: &LaneDecision,
    id: PresentationId,
    context: &CoefficientContext,
) -> Result<Presentation, PError> {
    let lines = node.topo.lines.len();
    let outside = role_lines(decision.roles)
        .into_iter()
        .find(|&line| usize::from(line) >= lines);
    if let Some(line) = outside {
        return Err(broken(format!(
            "node {}: role line {line} of {lines} lines",
            node.index
        )));
    }
    let index = |slot: u16| usize::from(slot);
    match (decision.lane, decision.roles) {
        (OwnerLane::SeriesParallel | OwnerLane::Insertion, LaneRoles::Insertion(roles)) => {
            let rows = routed(node, Some(index(roles.lines[0])), &[index(roles.lines[1])])?;
            let kind = PresentationKind::InsertionAdapted;
            let presentation = with_isps(node, rows, kind, id, context)?;
            check_insertion_contract(&presentation, &roles)?;
            Ok(presentation)
        }
        (OwnerLane::Carpet, LaneRoles::Carpet(roles)) => {
            let rows = routed_carpet(node, index(roles.r))?;
            let kind = PresentationKind::CarpetAdapted;
            let presentation = with_isps(node, rows, kind, id, context)?;
            check_carpet_contract(&presentation, &roles)?;
            Ok(presentation)
        }
        (OwnerLane::Triangle, LaneRoles::Triangle(roles)) => {
            let tree = [index(roles.a[0]), index(roles.a[1])];
            let rows = routed(node, Some(index(roles.b)), &tree)?;
            let kind = PresentationKind::TriangleAdapted;
            let presentation = with_isps(node, rows, kind, id, context)?;
            check_triangle_contract(&presentation, &roles)?;
            Ok(presentation)
        }
        (OwnerLane::Diamond, LaneRoles::Diamond(roles)) => {
            let rows = routed_diamond(node, &roles)?;
            let kind = PresentationKind::DiamondAdapted;
            let presentation = with_isps(node, rows, kind, id, context)?;
            check_diamond_contract(&presentation, &roles)?;
            Ok(presentation)
        }
        (lane, roles) => Err(broken(format!(
            "node {}: lane {lane:?} with roles {roles:?}",
            node.index
        ))),
    }
}

/// The graph of a vakint top: vertices `0..`, `q` from `external.in` to
/// `external.out`, lines in slot order.
pub fn top_topology(entry: &PFamilyEntry) -> PTopo {
    let vertex = |v: u8| v - 1;
    PTopo::new(
        entry.vertices,
        vertex(entry.external.incoming),
        vertex(entry.external.outgoing),
        entry.lines.iter().map(|line| {
            let [tail, head] = line.endpoints;
            (vertex(tail), vertex(head), 0)
        }),
    )
}

/// The embedded file holds the graphs: a [`Presentation`] has momenta only.
fn embedded_family(name: &str) -> Result<&'static PFamilyEntry, PError> {
    static FAMILIES: OnceLock<Result<BTreeMap<String, PFamilyEntry>, String>> = OnceLock::new();
    let families = FAMILIES.get_or_init(|| {
        let file: PFamiliesFile =
            serde_json::from_str(P_FAMILIES_V1_JSON).map_err(|error| error.to_string())?;
        let entries = file.families.into_iter();
        Ok(entries.map(|entry| (entry.name.clone(), entry)).collect())
    });
    let families = families
        .as_ref()
        .map_err(|error| PError::MalformedLibrary(format!("p families: {error}")))?;
    families
        .get(name)
        .ok_or_else(|| broken(format!("{name} is not a family of the embedded file")))
}

/// The topology of the sector `mask` of `top`: the other lines are contracted.
fn sector_topology(top: &PTopo, mask: u64) -> PTopo {
    let in_mask = |line: usize| mask >> line & 1 == 1;
    let mut parent: Vec<usize> = (0..usize::from(top.n)).collect();
    for (line, &(u, v, _)) in top.lines.iter().enumerate() {
        if !in_mask(line) {
            let (a, b) = (
                find(&mut parent, usize::from(u)),
                find(&mut parent, usize::from(v)),
            );
            parent[a.max(b)] = a.min(b);
        }
    }
    let mut vertex = |v: u8| u8::try_from(find(&mut parent, usize::from(v))).expect("a vertex");
    let (s, t) = (vertex(top.s), vertex(top.t));
    let lines: Vec<_> = (top.lines.iter().enumerate())
        .filter(|&(line, _)| in_mask(line))
        .map(|(_, &(u, v, eps_units))| (vertex(u), vertex(v), eps_units))
        .collect();
    PTopo::new(top.n, s, t, lines)
}

/// Canonical line to top slot, when the sector `mask` of `top` is the node `key`
/// line by line: a sector whose lines merge has no slot per node line.
fn sector_slots(top: &PTopo, mask: u64, key: &GraphKey) -> Option<Vec<u16>> {
    let Normalised::Topo(normal, fates) = normalise(&sector_topology(top, mask)) else {
        return None;
    };
    let (sector_key, canon) = canonical(&normal);
    if sector_key != *key {
        return None;
    }
    let slots = (0..u16::try_from(top.lines.len()).ok()?).filter(|&slot| mask >> slot & 1 == 1);
    let mut canonical_slots = vec![None; normal.lines.len()];
    for (slot, fate) in slots.zip(fates) {
        let line = fate.line()?;
        canonical_slots[usize::from(canon.line_map[usize::from(line)])] = Some(slot);
    }
    canonical_slots.into_iter().collect()
}

/// The vakint top of a presentation and its graph; the rows must be the file's.
fn top_graph(top: &Presentation) -> Result<PTopo, PError> {
    let PresentationKind::Top { name } = &top.kind else {
        return Err(broken(format!("presentation {} is no top", top.id.0)));
    };
    let entry = embedded_family(name)?;
    let file_rows = entry.lines.iter().map(|line| &line.momentum);
    if !file_rows.eq(top.lines().map(|(_, line)| &line.momentum)) {
        return Err(broken(format!("the top {name} differs from the file")));
    }
    Ok(top_topology(entry))
}

/// The first sector, by tops and ascending mask, that is the integer node.
fn induced(
    node: &NodeInfo,
    tops: &[Arc<Presentation>],
    id: PresentationId,
    context: &CoefficientContext,
) -> Result<Presentation, PError> {
    for top in tops.iter().filter(|top| top.loops == node.loops) {
        let graph = top_graph(top)?;
        let mut masks =
            (0..1_u64 << graph.lines.len()).filter(|m| m.count_ones() == u32::from(node.t));
        let Some(mask) = masks.find(|&m| sector_slots(&graph, m, &node.key).is_some()) else {
            continue;
        };
        if !(top.family.coefficient_context()).has_same_variable_map(context) {
            return Err(PError::ContextMismatch);
        }
        let presentation = Presentation {
            id,
            kind: PresentationKind::Induced { top: top.id, mask },
            loops: top.loops,
            slots: top.slots.clone(),
            family: Arc::clone(&top.family),
            node: Some(node_reference(node)),
        };
        presentation.validate()?;
        return Ok(presentation);
    }
    Err(broken(format!(
        "the integer Custom node {} is no sector of the {} tops",
        node.index,
        tops.len()
    )))
}

/// The presentation of `node` under `decision`; the trivial node has none. A
/// decision that contradicts itself is refused, it is never partly read.
pub fn present(
    node: &NodeInfo,
    decision: &LaneDecision,
    tops: &[Arc<Presentation>],
    id: PresentationId,
    context: &CoefficientContext,
) -> Result<Presentation, PError> {
    if decision.trivial {
        return Err(broken(format!(
            "node {}: a trivial node has no presentation",
            node.index
        )));
    }
    if decision.lane == OwnerLane::Custom && decision.roles != LaneRoles::None {
        return Err(broken(format!(
            "node {}: the lane Custom has no roles, not {:?}",
            node.index, decision.roles
        )));
    }
    match decision.lane {
        OwnerLane::Custom if node.eps_lines == 0 => induced(node, tops, id, context),
        OwnerLane::Custom => {
            let rows = routed(node, None, &[])?;
            with_isps(node, rows, PresentationKind::EpsNode, id, context)
        }
        OwnerLane::SeriesParallel
        | OwnerLane::Insertion
        | OwnerLane::Carpet
        | OwnerLane::Triangle
        | OwnerLane::Diamond => adapted(node, decision, id, context),
        lane @ (OwnerLane::Zero | OwnerLane::Product) => Err(broken(format!(
            "node {}: the lane {lane:?} has no presentation",
            node.index
        ))),
    }
}

/// What [`present_node`] makes of a node.
#[derive(Clone, Debug)]
pub struct NodePresentation {
    pub decision: LaneDecision,
    /// `None` on the trivial node.
    pub presentation: Option<Presentation>,
    /// Why the node left its lane for Custom.
    pub fallback: Option<String>,
}

/// [`decide`] and [`present`] with the fallback: a node whose presentation
/// cannot meet its lane contract is Custom, with the reason. None is expected
/// at `L <= 4`. Any other failure is returned: it says nothing about the lane.
pub fn present_node(
    node: &NodeInfo,
    tops: &[Arc<Presentation>],
    id: PresentationId,
    context: &CoefficientContext,
) -> Result<NodePresentation, PError> {
    let decision = decide(node)?;
    if decision.lane == OwnerLane::Zero {
        return Ok(NodePresentation {
            decision,
            presentation: None,
            fallback: None,
        });
    }
    match present(node, &decision, tops, id, context) {
        Ok(presentation) => Ok(NodePresentation {
            decision,
            presentation: Some(presentation),
            fallback: None,
        }),
        Err(PError::Contract(reason)) if decision.lane != OwnerLane::Custom => {
            Ok(NodePresentation {
                decision: CUSTOM,
                presentation: Some(present(node, &CUSTOM, tops, id, context)?),
                fallback: Some(reason),
            })
        }
        Err(error) => Err(error),
    }
}

/// Child slot of each canonical line of the child.
fn child_slots(child: &Presentation) -> Result<Vec<u16>, PError> {
    let node = child
        .node
        .as_ref()
        .ok_or_else(|| broken(format!("presentation {} has no node", child.id.0)))?;
    let PresentationKind::Induced { mask, .. } = child.kind else {
        return Ok((0..).take(child.lines().count()).collect());
    };
    let graph = top_topology(embedded_family(child.family.name())?);
    let key = GraphKey(Arc::clone(&node.graph_key));
    sector_slots(&graph, mask, &key).ok_or_else(|| {
        broken(format!(
            "the sector {mask:#x} of {} is not its node",
            child.family.name()
        ))
    })
}

fn up_to_sign(a: &[i64], b: &[i64]) -> bool {
    a == b || a.iter().zip(b).all(|(x, y)| *x == -*y)
}

/// A parent row over the input of an edge's `outer_map`.
fn outer_input(kind: EdgeKind, row: &[i64]) -> Option<Vec<i64>> {
    let integrated = usize::from(ADAPTED_LOOP);
    let external = row.len() - 1;
    let (dropped, kept) = match kind {
        EdgeKind::Insertion => (integrated, external),
        EdgeKind::Carpet => (external, integrated),
    };
    if row[dropped] != 0 {
        return None;
    }
    let outer = (0..external).filter(|&column| column != integrated);
    Some(outer.chain([kept]).map(|column| row[column]).collect())
}

/// The bubble momentum of two lines `s (k + R1)`, `t (k + R2)`: `R1 - R2`.
fn bubble_momentum(first: &SlotLine, second: &SlotLine) -> Vec<i64> {
    let integrated = usize::from(ADAPTED_LOOP);
    let (s, t) = (first.momentum[integrated], second.momentum[integrated]);
    let oriented = first.momentum.iter().zip(&second.momentum);
    oriented.map(|(a, b)| a * s - b * t).collect()
}

/// The signs of the outer loops and of the external column that carry every
/// row of `landing` onto its child row, in ascending sign pattern.
fn solved_outer_map(
    child_loops: u8,
    loop_images: &[Vec<i64>],
    landing: &[(Vec<i64>, Vec<i64>)],
) -> Result<Option<SignedAffineLoopMap>, PError> {
    let loops = loop_images.len();
    for pattern in 0_u32..1 << (loops + 1) {
        let sign = |bit: usize| if pattern >> bit & 1 == 1 { -1_i64 } else { 1 };
        let mut linear = Vec::with_capacity(loops * loops);
        let mut external = Vec::with_capacity(loops);
        for (row, image) in loop_images.iter().enumerate() {
            linear.extend(image[..loops].iter().map(|c| c * sign(row)));
            external.push(image[loops] * sign(row));
        }
        let map = SignedAffineLoopMap {
            loops_in: child_loops,
            loops_out: child_loops,
            loop_linear: linear.into_boxed_slice(),
            loop_external: external.into_boxed_slice(),
            external_sign: if sign(loops) == 1 { 1 } else { -1 },
        };
        let mut holds = true;
        for (input, target) in landing {
            holds &= up_to_sign(&map.map_momentum(input)?, target);
        }
        if holds && map.is_unimodular() {
            return Ok(Some(map));
        }
    }
    Ok(None)
}

/// The verified map of a rule edge between two presentations of [`present`].
/// A rule edge into the trivial node has none, and neither has a contraction.
pub fn edge_map(
    parent: &Presentation,
    child: &Presentation,
    edge: &DagEdge,
) -> Result<EdgeMap, PError> {
    parent.validate()?;
    child.validate()?;
    let fail = |detail: String| broken(format!("edge {} -> {}: {detail}", edge.parent, edge.child));
    if edge.kind != DagEdgeKind::Rule {
        return Err(fail("only a rule edge has an edge map".to_owned()));
    }
    let is_node = |presentation: &Presentation, index: u32| {
        (presentation.node.as_ref()).is_some_and(|node| node.index == NodeIndex(index))
    };
    if !is_node(parent, edge.parent) || !is_node(child, edge.child) {
        return Err(fail("the presentations are not its nodes".to_owned()));
    }
    let kind = match parent.kind {
        PresentationKind::InsertionAdapted => EdgeKind::Insertion,
        PresentationKind::CarpetAdapted => EdgeKind::Carpet,
        _ => return Err(fail("the parent has no rule".to_owned())),
    };
    if child.loops.checked_add(1) != Some(parent.loops) {
        return Err(fail(format!("{} -> {} loops", parent.loops, child.loops)));
    }
    let lines = parent.lines().count();
    if edge.fates.len() != lines {
        return Err(fail(format!(
            "{} fates for {lines} lines",
            edge.fates.len()
        )));
    }

    let targets = child_slots(child)?;
    let target = |line: u8| {
        targets
            .get(usize::from(line))
            .copied()
            .ok_or_else(|| fail(format!("the child has no line {line}")))
    };
    let child_row = |slot: u16| match &child.slots[usize::from(slot)] {
        Slot::Line(line) => Ok(line.momentum.clone()),
        Slot::Isp(_) => Err(fail(format!("child slot {slot} is an ISP"))),
    };

    let integrated = usize::from(ADAPTED_LOOP);
    let mut line_slots = vec![None; parent.slots.len()];
    let mut bubble = Vec::new();
    let mut landing = Vec::new();
    let mut merged_slot = None;
    for (index, line) in parent.lines() {
        let slot = edge.fates[index].line().map(&target).transpose()?;
        if kind == EdgeKind::Insertion && line.momentum[integrated] != 0 {
            if !bubble.is_empty() && slot != merged_slot {
                return Err(fail("the bubble lines part ways".to_owned()));
            }
            bubble.push(line);
            merged_slot = slot;
        } else if let Some(slot) = slot {
            line_slots[index] = Some(slot);
            let input = outer_input(kind, &line.momentum)
                .ok_or_else(|| fail(format!("line {index} depends on the removed momentum")))?;
            landing.push((input, child_row(slot)?));
        }
    }

    let width = usize::from(child.loops) + 1;
    if let [first, second] = bubble.as_slice() {
        let momentum = bubble_momentum(first, second);
        let input = outer_input(kind, &momentum)
            .ok_or_else(|| fail("the bubble momentum keeps the loop".to_owned()))?;
        let output = match merged_slot {
            Some(slot) => child_row(slot)?,
            None => unit_row(width, width - 1),
        };
        landing.push((input, output));
    }

    // Outer loop `i` is the momentum of its cotree line, so its image is that
    // line's child momentum up to a sign.
    let parent_width = usize::from(parent.loops) + 1;
    let mut loop_images = Vec::with_capacity(usize::from(child.loops));
    for column in (0..usize::from(parent.loops)).filter(|&column| column != integrated) {
        let unit = unit_row(parent_width, column);
        let slot = parent
            .lines()
            .find(|(_, line)| line.momentum == unit)
            .and_then(|(index, _)| line_slots[index])
            .ok_or_else(|| fail(format!("no kept line carries loop {column}")))?;
        loop_images.push(child_row(slot)?);
    }

    let outer_map = solved_outer_map(child.loops, &loop_images, &landing)?
        .ok_or_else(|| fail("no outer map carries the parent onto the child".to_owned()))?;
    let map = EdgeMap {
        parent: parent.id,
        child: child.id,
        kind,
        integrated_loop: ADAPTED_LOOP,
        outer_map,
        line_slots,
        merged_slot,
    };
    verify_edge_map(parent, child, &map)?;
    Ok(map)
}
