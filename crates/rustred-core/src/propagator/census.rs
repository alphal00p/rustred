//! The p census: sector fates and owner classes (types: M2-I0 rr-iface;
//! p_census and completeness: M2-T2 rr-census).
//!
//! [`p_census`] gives every `(presentation, support)` a fate. A sector is
//! keyed by its support `raw | forced`, so the raw charts of one support share
//! one fate, and [`p_census`] checks that they do. The zero analyzer proves the
//! zero sectors; every other sector has an [`OwnerClassKey`] and a verified
//! route onto the owner of its class, which is the identity for the owner itself.
//!
//! # The owner of a class
//!
//! With a DAG, the owner is the presentation and support of the min-GraphKey node
//! of the class (critique-phys MF3), so it does not depend on the order of the
//! presentation list. That needs every node with `L >= 1` to have a presentation
//! and every class to have a node: anything else is an error, not another owner.
//! Without a DAG, as in unit tests, the owner is the first member by
//! `(presentation, support)`; all members of a class have the same `t`.
//!
//! [`PCensus::check_complete`] repeats the zero proofs and verifies every stored
//! route again, so it costs about what the census does. A loader that wants to skip
//! it has to say so.
//!
//! # What a census takes from its input
//!
//! The owner rule needs each node presentation to stand for the node it names, so
//! both entry points compare the two. A presentation that is not Induced must carry
//! the graph of its node: line slot `i` is canonical line `i` with its eps units, and
//! momentum is conserved on that graph. An Induced presentation has the lines of its
//! top, in the top's order, and a [`Presentation`] has momenta only, so only its
//! loops, lines, eps lines and key bytes are compared. That its sector is the graph
//! of its node remains the word of `present`, which chooses the mask on the graph of
//! the top, and of `edge_map`, which checks it for a child.
//!
//! # Route maps
//!
//! [`p_census`] stores the first verified route of [`route`]
//! ([`PRouteMaps::First`], the default) or, with [`PRouteMaps::StaticScore`]
//! (what a package assembly uses; M5 maps, p5l-maps-rs), the map `T M` with
//! the fewest static-score terms over the owner sector's automorphisms `M`
//! ([`route_choice::choose_static`]: the choices of `choose_maps.py static`),
//! each verified exactly by [`verify_route`] before it is stored.
//!
//! [`route_choice::choose_static`]: super::route_choice::choose_static
//!
//! # Versioning
//!
//! Bump [`P_CENSUS_VERSION`] when a fate, an owner or a stored route can change
//! for the same input: it is part of the identity of every library program. The
//! JSON form ([`PCensus::to_json`], [`P_CENSUS_SCHEMA`]) records all of
//! [`p_versions`], and [`PCensus::from_json`] refuses any other value.
//!
//! [`P_CENSUS_VERSION`]: super::P_CENSUS_VERSION
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::sector::Mask;
use crate::sector::zero::{Analyzer, Decision};

use super::class::{ClassLimits, ClassedSector, OwnerClassKey, classify, route, verify_route};
use super::error::PError;
use super::presentation::{Presentation, PresentationKind, determinant};
use super::route_choice::{FormBasis, RouteDecision, choose_static, sector_automorphisms};
use super::topology::{Dag, GraphKey, PTopo};
use super::{NodeIndex, OwnerClassId, PresentationId, p_versions};

/// Schema name of [`PCensus::to_json`].
pub const P_CENSUS_SCHEMA: &str = "rustred.p-census.v1";

/// A census enumerates `2^t` supports per presentation.
const MAX_CENSUS_LINES: u32 = 24;

/// The ledger entry of a [`PError::MissingOwner`] raised by the census: a class
/// without an owner is not on any ledger.
pub const CENSUS_NO_LEDGER: u32 = u32::MAX;

/// Stack of one census worker: the exact algebra recurses as in the test harness,
/// which runs with `RUST_MIN_STACK` = 32 MiB.
const WORKER_STACK_BYTES: usize = 32 * 1024 * 1024;

/// The substitution `k_in[i] = Sum_j A[i][j] k_out[j] + b[i] q_out`,
/// `q_in = s q_out`, from a source to a target momentum basis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignedAffineLoopMap {
    pub loops_in: u8,
    pub loops_out: u8,
    /// `A`, row major, `loops_in * loops_out`.
    pub loop_linear: Box<[i64]>,
    /// `b`, `loops_in` entries.
    pub loop_external: Box<[i64]>,
    /// `s`, `+1` or `-1`.
    pub external_sign: i8,
}

impl SignedAffineLoopMap {
    /// The target row of a source momentum over `(k_in.., q)`.
    pub fn map_momentum(&self, momentum: &[i64]) -> Result<Vec<i64>, PError> {
        let (rows, columns) = (usize::from(self.loops_in), usize::from(self.loops_out));
        if momentum.len() != rows + 1
            || self.loop_linear.len() != rows * columns
            || self.loop_external.len() != rows
        {
            return Err(PError::Contract(format!(
                "a {rows}->{columns} loop map cannot act on a momentum of {} entries",
                momentum.len()
            )));
        }
        let overflow = || PError::Contract("momentum coefficient overflow".to_owned());
        let mut out = vec![0_i64; columns + 1];
        for (row, &value) in momentum[..rows].iter().enumerate() {
            for (column, target) in out[..columns].iter_mut().enumerate() {
                let term = value
                    .checked_mul(self.loop_linear[row * columns + column])
                    .ok_or_else(overflow)?;
                *target = target.checked_add(term).ok_or_else(overflow)?;
            }
            let term = value
                .checked_mul(self.loop_external[row])
                .ok_or_else(overflow)?;
            out[columns] = out[columns].checked_add(term).ok_or_else(overflow)?;
        }
        let term = momentum[rows]
            .checked_mul(i64::from(self.external_sign))
            .ok_or_else(overflow)?;
        out[columns] = out[columns].checked_add(term).ok_or_else(overflow)?;
        Ok(out)
    }

    /// `A` square with determinant `+-1` and `s = +-1`.
    pub fn is_unimodular(&self) -> bool {
        let size = usize::from(self.loops_in);
        if self.loops_in != self.loops_out
            || !matches!(self.external_sign, 1 | -1)
            || self.loop_linear.len() != size * size
            || self.loop_external.len() != size
        {
            return false;
        }
        let det = determinant(size, &self.loop_linear);
        det == 1 || det == -1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PSectorFate {
    Zero,
    Owner {
        class: OwnerClassId,
        map: SignedAffineLoopMap,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PClassRecord {
    pub key: OwnerClassKey,
    /// Owner presentation and support.
    pub owner: (PresentationId, u64),
    /// The min-GraphKey DAG node.
    pub node: Option<NodeIndex>,
    pub nodes: Vec<NodeIndex>,
    pub t: u8,
    pub eps_lines: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PCensus {
    /// `(presentation, support)` to its fate; `support = raw | forced`.
    pub sectors: BTreeMap<(PresentationId, u64), PSectorFate>,
    pub classes: BTreeMap<OwnerClassId, PClassRecord>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PCensusOptions {
    pub limits: ClassLimits,
    /// Worker threads; `0` means one. The census does not depend on it.
    pub threads: usize,
    /// Which verified route a routed sector stores.
    pub route_maps: PRouteMaps,
}

/// Which verified route [`p_census`] stores for a routed (non-owner) sector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PRouteMaps {
    /// The route [`route`] finds first.
    #[default]
    First,
    /// The first route composed with the owner-sector automorphism of the
    /// fewest static-score terms (`route_choice::choose_static`; the stored
    /// route is kept unless strictly beaten, and an owner whose slots are no
    /// basis of the quadratic forms keeps every first route). Each changed
    /// route is verified by [`verify_route`].
    StaticScore,
}

impl PCensusOptions {
    fn threads(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.threads).unwrap_or(NonZeroUsize::MIN)
    }
}

/// The line and forced axes of one presentation, and so its sectors.
#[derive(Clone, Copy)]
struct Axes {
    lines: u64,
    forced: u64,
}

impl Axes {
    fn of(p: &Presentation) -> Result<Self, PError> {
        let lines = p.lines().fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
        if lines.count_ones() > MAX_CENSUS_LINES {
            return Err(PError::Unsupported(format!(
                "a census of presentation {} would enumerate 2^{} supports",
                p.id.0,
                lines.count_ones()
            )));
        }
        Ok(Self {
            lines,
            forced: p.forced_axes(),
        })
    }

    /// Every support: the forced axes plus any set of the other lines.
    fn supports(self) -> impl Iterator<Item = u64> {
        let forced = self.forced;
        submasks(self.lines & !forced).map(move |free| free | forced)
    }

    /// The number of [`Self::supports`].
    fn support_count(self) -> usize {
        1 << (self.lines & !self.forced).count_ones()
    }

    /// The raw charts of one support: a forced axis may be on either side of
    /// `n = 0`, a line outside the forced axes is positive.
    fn charts(self, support: u64) -> impl Iterator<Item = u64> {
        let free = support & !self.forced;
        submasks(self.forced).map(move |chart| free | chart)
    }

    fn holds(self, support: u64) -> bool {
        support & !self.lines == 0 && support & self.forced == self.forced
    }
}

/// The submasks of `mask`, ascending.
fn submasks(mask: u64) -> impl Iterator<Item = u64> {
    let mut next = Some(0_u64);
    std::iter::from_fn(move || {
        let current = next?;
        next = (current != mask).then(|| current.wrapping_sub(mask) & mask);
        Some(current)
    })
}

fn zero_error(p: &Presentation, error: impl std::fmt::Display) -> PError {
    PError::Unsupported(format!("zero analysis of presentation {}: {error}", p.id.0))
}

/// The class key of one raw chart, `None` for a proved zero. A sector the zero
/// analyzer leaves open needs a class: there is no third fate.
fn chart_fate(
    p: &Presentation,
    analyzer: &Analyzer,
    raw: u64,
    limits: &ClassLimits,
) -> Result<Option<OwnerClassKey>, PError> {
    let mask = Mask::try_new((0..p.slots.len()).map(|slot| raw >> slot & 1 == 1))
        .map_err(|error| zero_error(p, error))?;
    match analyzer.analyze(&mask).map_err(|e| zero_error(p, e))? {
        Decision::ProvedZero(_) => Ok(None),
        Decision::Excluded(exclusion) => Err(zero_error(p, format!("{exclusion:?}"))),
        Decision::Inconclusive(_) => classify(p, raw, limits)?
            .map(|s| s.key)
            .map(Some)
            .ok_or_else(|| {
                PError::Generation(format!(
                    "raw sector {raw:#x} of presentation {} is not proved zero and has no class",
                    p.id.0
                ))
            }),
    }
}

/// The class key of one support, `None` for a proved zero: all its raw charts
/// must agree.
fn support_fate(
    p: &Presentation,
    analyzer: &Analyzer,
    axes: Axes,
    support: u64,
    limits: &ClassLimits,
) -> Result<Option<OwnerClassKey>, PError> {
    let mut charts = axes.charts(support);
    let first = charts.next().expect("a support has a raw chart");
    let fate = chart_fate(p, analyzer, first, limits)?;
    for raw in charts {
        if chart_fate(p, analyzer, raw, limits)? != fate {
            return Err(PError::Generation(format!(
                "raw charts {first:#x} and {raw:#x} of presentation {} differ in their fate",
                p.id.0
            )));
        }
    }
    Ok(fate)
}

/// `f` on every item with `threads` workers, in the order of `items`. All items
/// run, so the error returned does not depend on the threads either.
fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    threads: NonZeroUsize,
    f: impl Fn(&T) -> Result<R, PError> + Sync,
) -> Result<Vec<R>, PError> {
    let workers = threads.get().min(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(index) else {
                return done;
            };
            done.push((index, f(item)));
        }
    };
    let mut outcomes = std::thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| {
                std::thread::Builder::new()
                    .stack_size(WORKER_STACK_BYTES)
                    .spawn_scoped(scope, work)
            })
            .collect::<Vec<_>>();
        let mut outcomes = Vec::with_capacity(items.len());
        let mut failure = None;
        for handle in handles {
            match handle.map(|handle| handle.join()) {
                Ok(Ok(done)) => outcomes.extend(done),
                Ok(Err(_)) => failure = Some("a census worker panicked".to_owned()),
                Err(error) => failure = Some(format!("cannot start a census worker: {error}")),
            }
        }
        match failure {
            Some(detail) => Err(PError::Generation(detail)),
            None => Ok(outcomes),
        }
    })?;
    outcomes.sort_unstable_by_key(|(index, _)| *index);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

/// The valid presentations by id; two of one id or two contexts fail.
fn by_id(presentations: &[Arc<Presentation>]) -> Result<Vec<&Arc<Presentation>>, PError> {
    let mut ordered: Vec<_> = presentations.iter().collect();
    ordered.sort_by_key(|p| p.id);
    for p in &ordered {
        p.validate()?;
    }
    if let Some(pair) = ordered.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(PError::MalformedLibrary(format!(
            "two presentations with the id {}",
            pair[0].id.0
        )));
    }
    if let Some(first) = ordered.first() {
        let context = first.family.coefficient_context();
        if ordered.iter().any(|p| {
            !p.family
                .coefficient_context()
                .has_same_variable_map(context)
        }) {
            return Err(PError::ContextMismatch);
        }
    }
    Ok(ordered)
}

/// One DAG node with the sector that presents it.
struct NodeLink<'a> {
    node: u32,
    key: &'a GraphKey,
    sector: (PresentationId, u64),
    class: OwnerClassId,
}

/// The support of `p` that its node stands for: all lines, on an Induced
/// presentation the lines of the node's sector.
fn node_support(p: &Presentation, axes: Axes) -> Result<u64, PError> {
    let support = match p.kind {
        PresentationKind::Induced { mask, .. } => mask | axes.forced,
        _ => axes.lines,
    };
    if axes.holds(support) {
        Ok(support)
    } else {
        Err(PError::MalformedLibrary(format!(
            "presentation {} presents its node on {support:#x}, outside its lines {:#x}",
            p.id.0, axes.lines
        )))
    }
}

/// `p`, which is not Induced, on the graph of its node: line slot `i` is canonical
/// line `i` from `u` to `v` with its eps units, and `q` enters at `s` and leaves at
/// `t` (the form of `PFamilyEntry::check`). The rows of a valid presentation are
/// small, so the balance cannot overflow.
fn check_on_graph(p: &Presentation, graph: &PTopo) -> Result<(), String> {
    let width = usize::from(p.loops) + 1;
    let mut balance = vec![vec![0_i64; width]; usize::from(graph.n)];
    for ((slot, line), &(tail, head, eps_units)) in p.lines().zip(&graph.lines) {
        if line.eps_units != eps_units {
            return Err(format!(
                "line {slot} has {} eps units, the graph {eps_units}",
                line.eps_units
            ));
        }
        for (column, &value) in line.momentum.iter().enumerate() {
            balance[usize::from(tail)][column] -= value;
            balance[usize::from(head)][column] += value;
        }
    }
    balance[usize::from(graph.s)][width - 1] += 1;
    balance[usize::from(graph.t)][width - 1] -= 1;
    match balance.iter().position(|row| row.iter().any(|&c| c != 0)) {
        Some(vertex) => Err(format!(
            "momentum is not conserved at vertex {vertex} of the graph"
        )),
        None => Ok(()),
    }
}

/// The DAG nodes by class. `class_of` gives the class of a sector, `None` for a
/// proved zero. Every node with loops needs exactly one presentation, which has
/// to fit the node: the owner rule depends on seeing all of them.
fn node_links<'a>(
    ordered: &[&Arc<Presentation>],
    dag: &'a Dag,
    class_of: impl Fn(PresentationId, u64) -> Result<Option<OwnerClassId>, PError>,
) -> Result<BTreeMap<OwnerClassId, Vec<NodeLink<'a>>>, PError> {
    let malformed = |detail: String| PError::MalformedLibrary(format!("census DAG link: {detail}"));
    let mut links: BTreeMap<OwnerClassId, Vec<NodeLink<'a>>> = BTreeMap::new();
    let mut presented = BTreeSet::new();
    for p in ordered {
        let Some(reference) = &p.node else {
            continue;
        };
        let node = usize::try_from(reference.index.0)
            .ok()
            .and_then(|index| dag.nodes.get(index))
            .filter(|node| node.index == reference.index.0)
            .ok_or_else(|| {
                malformed(format!(
                    "presentation {} names node {}, outside the DAG",
                    p.id.0, reference.index.0
                ))
            })?;
        if node.key.as_bytes() != &*reference.graph_key {
            return Err(malformed(format!(
                "presentation {} names node {} with another key",
                p.id.0, node.index
            )));
        }
        if !presented.insert(node.index) {
            return Err(malformed(format!(
                "node {} has two presentations",
                node.index
            )));
        }
        let axes = Axes::of(p)?;
        let support = node_support(p, axes)?;
        let shape = (
            p.loops,
            support.count_ones(),
            (support & axes.forced).count_ones(),
        );
        if shape != (node.loops, u32::from(node.t), u32::from(node.eps_lines)) {
            return Err(malformed(format!(
                "presentation {} presents (loops, lines, eps lines) = {shape:?} for node {}",
                p.id.0, node.index
            )));
        }
        if !matches!(p.kind, PresentationKind::Induced { .. }) {
            check_on_graph(p, &node.topo).map_err(|detail| {
                malformed(format!(
                    "presentation {} is not node {}: {detail}",
                    p.id.0, node.index
                ))
            })?;
        }
        let class = class_of(p.id, support)?.ok_or_else(|| {
            malformed(format!(
                "sector ({}, {support:#x}) of node {} is proved zero",
                p.id.0, node.index
            ))
        })?;
        links.entry(class).or_default().push(NodeLink {
            node: node.index,
            key: &node.key,
            sector: (p.id, support),
            class,
        });
    }
    if let Some(node) = dag
        .nodes
        .iter()
        .find(|node| node.loops > 0 && !presented.contains(&node.index))
    {
        return Err(malformed(format!(
            "node {} has no presentation",
            node.index
        )));
    }
    Ok(links)
}

/// What the rule of the module docs says of one class.
#[derive(Debug, PartialEq, Eq)]
struct CanonicalOwner {
    sector: (PresentationId, u64),
    node: Option<NodeIndex>,
    nodes: Vec<NodeIndex>,
}

fn owner_of(
    label: &str,
    first_member: (PresentationId, u64),
    dag_links: Option<&[NodeLink<'_>]>,
) -> Result<CanonicalOwner, PError> {
    let Some(links) = dag_links else {
        return Ok(CanonicalOwner {
            sector: first_member,
            node: None,
            nodes: Vec::new(),
        });
    };
    let owner = links
        .iter()
        .min_by_key(|link| link.key)
        .ok_or_else(|| PError::MalformedLibrary(format!("class {label} has no DAG node")))?;
    let mut nodes: Vec<_> = links.iter().map(|link| NodeIndex(link.node)).collect();
    nodes.sort_unstable();
    Ok(CanonicalOwner {
        sector: owner.sector,
        node: Some(NodeIndex(owner.node)),
        nodes,
    })
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

/// `classify` of a sector the census holds as nonzero.
fn classed_support(
    p: &Presentation,
    support: u64,
    limits: &ClassLimits,
) -> Result<ClassedSector, PError> {
    classify(p, support, limits)?.ok_or_else(|| {
        PError::MalformedLibrary(format!(
            "sector ({}, {support:#x}) holds a class and has none",
            p.id.0
        ))
    })
}

/// The census of `presentations` (`dag`: owners at min-GraphKey nodes).
pub fn p_census(
    presentations: &[Arc<Presentation>],
    dag: Option<&Dag>,
    options: &PCensusOptions,
) -> Result<PCensus, PError> {
    let ordered = by_id(presentations)?;
    let threads = options.threads();
    let limits = &options.limits;

    // Fates first: an owner can be chosen only once every class is known.
    let scanned = parallel_map(&ordered, threads, |p| {
        let axes = Axes::of(p)?;
        let analyzer = Analyzer::try_unrestricted(&p.family).map_err(|e| zero_error(p, e))?;
        axes.supports()
            .map(|support| {
                support_fate(p, &analyzer, axes, support, limits).map(|fate| (support, fate))
            })
            .collect::<Result<Vec<_>, PError>>()
    })?;

    let mut keys: BTreeMap<OwnerClassId, OwnerClassKey> = BTreeMap::new();
    let mut first_members: BTreeMap<OwnerClassId, (PresentationId, u64)> = BTreeMap::new();
    let mut fates: BTreeMap<(PresentationId, u64), Option<OwnerClassId>> = BTreeMap::new();
    for (p, sectors) in ordered.iter().zip(scanned) {
        for (support, fate) in sectors {
            let class = fate.map(|key| {
                let id = key.id();
                match keys.get(&id) {
                    Some(known) if *known != key => Err(PError::MalformedLibrary(format!(
                        "owner class id {:016x} names two canonical keys",
                        id.0
                    ))),
                    Some(_) => Ok(id),
                    None => {
                        keys.insert(id, key);
                        first_members.insert(id, (p.id, support));
                        Ok(id)
                    }
                }
            });
            fates.insert((p.id, support), class.transpose()?);
        }
    }

    let links = dag
        .map(|dag| {
            node_links(&ordered, dag, |id, support| {
                fates.get(&(id, support)).copied().ok_or_else(|| {
                    PError::MalformedLibrary(format!(
                        "sector ({}, {support:#x}) is outside the census",
                        id.0
                    ))
                })
            })
        })
        .transpose()?;

    let mut census = PCensus::default();
    let empty = Vec::new();
    for (&id, key) in &keys {
        let node_links = links
            .as_ref()
            .map(|links| links.get(&id).unwrap_or(&empty).as_slice());
        let owner = owner_of(&key.label(), first_members[&id], node_links)?;
        census.insert_class(PClassRecord {
            key: key.clone(),
            owner: owner.sector,
            node: owner.node,
            nodes: owner.nodes,
            t: key.lines,
            eps_lines: key.eps_lines,
        })?;
    }

    let presentation = |id: PresentationId| {
        ordered
            .binary_search_by_key(&id, |p| p.id)
            .map(|position| ordered[position])
            .map_err(|_| PError::MalformedLibrary(format!("no presentation {}", id.0)))
    };
    let owners = census
        .classes
        .iter()
        .map(|(&id, record)| {
            let (owner, support) = record.owner;
            let classed = classed_support(presentation(owner)?, support, limits)?;
            if classed.key != record.key {
                return Err(PError::MalformedLibrary(format!(
                    "the owner sector of class {} has another key",
                    record.key.label()
                )));
            }
            Ok((id, classed))
        })
        .collect::<Result<BTreeMap<_, _>, PError>>()?;

    // Static-score route maps: the automorphisms of every owner sector and the
    // form basis of every owner presentation, once.
    let choice = match options.route_maps {
        PRouteMaps::First => None,
        PRouteMaps::StaticScore => {
            let owner_sectors: Vec<(OwnerClassId, (PresentationId, u64))> = census
                .classes
                .iter()
                .map(|(&id, record)| (id, record.owner))
                .collect();
            let groups = parallel_map(&owner_sectors, threads, |&(id, (owner, support))| {
                Ok((id, sector_automorphisms(presentation(owner)?, support)?))
            })?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
            let mut owner_ids: Vec<PresentationId> =
                owner_sectors.iter().map(|(_, (owner, _))| *owner).collect();
            owner_ids.sort();
            owner_ids.dedup();
            let bases = parallel_map(&owner_ids, threads, |&owner| {
                Ok((owner, FormBasis::of(presentation(owner)?)?))
            })?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
            Some((groups, bases))
        }
    };

    // Routes last, each from a fresh classification, so that no prepared
    // geometry is held per sector.
    let sectors: Vec<_> = fates
        .iter()
        .map(|(&sector, &class)| (sector, class))
        .collect();
    let routed = parallel_map(&sectors, threads, |&((id, support), class)| {
        let Some(class) = class else {
            return Ok(((id, support), PSectorFate::Zero));
        };
        let source = classed_support(presentation(id)?, support, limits)?;
        let owner = &owners[&class];
        let map = if (id, support) == census.classes[&class].owner {
            let identity = identity_map(source.key.loops);
            verify_route(&source, owner, &identity, limits)?.filter(|map| *map == identity)
        } else {
            route(&source, owner, limits)?
        };
        let mut map = map.ok_or_else(|| {
            PError::Generation(format!(
                "sector ({}, {support:#x}) has no verified route onto its class {}",
                id.0,
                source.key.label()
            ))
        })?;
        let (owner_id, _) = census.classes[&class].owner;
        if let Some((groups, bases)) = &choice
            && (id, support) != census.classes[&class].owner
            && let Some(basis) = &bases[&owner_id]
            && let RouteDecision::Changed(chosen) =
                choose_static(presentation(id)?, support, &map, &groups[&class], basis)?
        {
            if verify_route(&source, owner, &chosen.map, limits)?.as_ref() != Some(&chosen.map) {
                return Err(PError::Generation(format!(
                    "sector ({}, {support:#x}): the static-score route is no verified route",
                    id.0
                )));
            }
            map = chosen.map;
        }
        Ok(((id, support), PSectorFate::Owner { class, map }))
    })?;
    census.sectors = routed.into_iter().collect();
    Ok(census)
}

/// Counts of one loop order; see [`PCensus::counts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PCensusCounts {
    pub loops: u8,
    pub presentations: usize,
    /// All sectors: one per support.
    pub sectors: usize,
    pub zero_sectors: usize,
    pub nonzero_sectors: usize,
    pub classes: usize,
}

impl PCensus {
    /// Register a class under its id. An equal record is a no-op; anything else
    /// under a taken id fails closed, so build a record completely first (a
    /// plain `classes.insert` would overwrite silently).
    pub fn insert_class(&mut self, record: PClassRecord) -> Result<OwnerClassId, PError> {
        self.insert_class_as(record.key.id(), record)
    }

    /// [`Self::insert_class`] under an explicit id: the entry point of the
    /// forced-collision test; every other caller uses `insert_class`.
    #[doc(hidden)]
    pub fn insert_class_as(
        &mut self,
        id: OwnerClassId,
        record: PClassRecord,
    ) -> Result<OwnerClassId, PError> {
        match self.classes.get(&id) {
            Some(present) if *present == record => {}
            Some(present) if present.key.canonical != record.key.canonical => {
                return Err(PError::MalformedLibrary(format!(
                    "owner class id {:016x} names two canonical keys ({} and {})",
                    id.0,
                    present.key.label(),
                    record.key.label()
                )));
            }
            Some(present) => {
                return Err(PError::MalformedLibrary(format!(
                    "owner class {} has two records",
                    present.key.label()
                )));
            }
            None => {
                self.classes.insert(id, record);
            }
        }
        Ok(id)
    }

    /// Every class is stored under the id of its own canonical bytes.
    pub fn check_class_ids(&self) -> Result<(), PError> {
        for (id, record) in &self.classes {
            if record.key.id() != *id {
                return Err(PError::MalformedLibrary(format!(
                    "owner class id {:016x} holds the canonical key {}",
                    id.0,
                    record.key.label()
                )));
            }
        }
        Ok(())
    }

    /// Strong completeness over `presentations` and `dag`, on one thread: see
    /// [`Self::check_complete_with`].
    pub fn check_complete(
        &self,
        presentations: &[Arc<Presentation>],
        dag: Option<&Dag>,
    ) -> Result<(), PError> {
        self.check_complete_with(presentations, dag, &PCensusOptions::default())
    }

    /// Strong completeness: `routes + zeros == 2^t` per presentation (`t`
    /// without the forced axes, each support is one sector) and nothing else;
    /// every class has its canonical owner; every class has a DAG node and the
    /// class of every DAG node has an owner; a node presentation fits its node as
    /// far as the module docs say. The zero proofs are repeated and every route is
    /// verified again. What an owner runs is checked by M2-T8a.
    pub fn check_complete_with(
        &self,
        presentations: &[Arc<Presentation>],
        dag: Option<&Dag>,
        options: &PCensusOptions,
    ) -> Result<(), PError> {
        let ordered = by_id(presentations)?;
        self.check_class_ids()?;
        let links = dag
            .map(|dag| node_links(&ordered, dag, |id, support| self.class_of(id, support)))
            .transpose()?;
        self.check_shape(&ordered)?;
        self.check_owners(links.as_ref())?;

        let limits = &options.limits;
        let owners = self
            .classes
            .iter()
            .map(|(&id, record)| {
                let (owner, support) = record.owner;
                let position = ordered
                    .binary_search_by_key(&owner, |p| p.id)
                    .map_err(|_| missing_owner(id))?;
                Ok((id, classed_support(ordered[position], support, limits)?))
            })
            .collect::<Result<BTreeMap<_, _>, PError>>()?;
        parallel_map(&ordered, options.threads(), |p| {
            self.check_presentation(p, &owners, limits)
        })?;
        Ok(())
    }

    /// One fate per support of every presentation, and no other sector.
    fn check_shape(&self, ordered: &[&Arc<Presentation>]) -> Result<(), PError> {
        let mut sectors = 0_usize;
        for p in ordered {
            let axes = Axes::of(p)?;
            sectors += axes.support_count();
            let held = self
                .sectors
                .range((p.id, 0)..=(p.id, u64::MAX))
                .filter(|&(&(_, support), _)| axes.holds(support))
                .count();
            if held != axes.support_count() {
                return Err(PError::MalformedLibrary(format!(
                    "presentation {} holds {held} fates for {} supports",
                    p.id.0,
                    axes.support_count()
                )));
            }
        }
        if self.sectors.len() != sectors {
            return Err(PError::MalformedLibrary(format!(
                "the census holds {} sectors, its presentations have {sectors}",
                self.sectors.len()
            )));
        }
        Ok(())
    }

    /// The fate of a sector as the class it belongs to, `None` for a zero.
    fn class_of(&self, id: PresentationId, support: u64) -> Result<Option<OwnerClassId>, PError> {
        match self.sectors.get(&(id, support)) {
            None => Err(PError::MalformedLibrary(format!(
                "sector ({}, {support:#x}) is outside the census",
                id.0
            ))),
            Some(PSectorFate::Zero) => Ok(None),
            Some(PSectorFate::Owner { class, .. }) => Ok(Some(*class)),
        }
    }

    /// Every record and route belongs to its class, and every class has the
    /// canonical owner, whose route is the identity.
    fn check_owners(
        &self,
        links: Option<&BTreeMap<OwnerClassId, Vec<NodeLink<'_>>>>,
    ) -> Result<(), PError> {
        let mut first_members: BTreeMap<OwnerClassId, (PresentationId, u64)> = BTreeMap::new();
        for (&sector, fate) in &self.sectors {
            if let PSectorFate::Owner { class, .. } = fate {
                if !self.classes.contains_key(class) {
                    return Err(missing_owner(*class));
                }
                first_members.entry(*class).or_insert(sector);
            }
        }
        if let Some(link) = links
            .into_iter()
            .flat_map(BTreeMap::values)
            .flatten()
            .find(|link| !self.classes.contains_key(&link.class))
        {
            return Err(missing_owner(link.class));
        }
        let empty = Vec::new();
        for (&id, record) in &self.classes {
            let label = record.key.label();
            if (record.t, record.eps_lines) != (record.key.lines, record.key.eps_lines) {
                return Err(PError::MalformedLibrary(format!(
                    "class {label} records {} lines, {} eps lines",
                    record.t, record.eps_lines
                )));
            }
            match self.sectors.get(&record.owner) {
                Some(PSectorFate::Owner { class, map })
                    if *class == id && *map == identity_map(record.key.loops) => {}
                _ => return Err(missing_owner(id)),
            }
            let node_links = links.map(|links| links.get(&id).unwrap_or(&empty).as_slice());
            let canonical = owner_of(&label, first_members[&id], node_links)?;
            let held = CanonicalOwner {
                sector: record.owner,
                node: record.node,
                nodes: record.nodes.clone(),
            };
            if held != canonical {
                return Err(PError::MalformedLibrary(format!(
                    "class {label} has {held:?}, the rule gives {canonical:?}"
                )));
            }
        }
        Ok(())
    }

    /// The fates of one presentation again: proofs, keys and routes.
    fn check_presentation(
        &self,
        p: &Presentation,
        owners: &BTreeMap<OwnerClassId, ClassedSector>,
        limits: &ClassLimits,
    ) -> Result<(), PError> {
        let axes = Axes::of(p)?;
        let analyzer = Analyzer::try_unrestricted(&p.family).map_err(|e| zero_error(p, e))?;
        for support in axes.supports() {
            let sector = |detail: String| {
                PError::MalformedLibrary(format!("sector ({}, {support:#x}): {detail}", p.id.0))
            };
            let actual = support_fate(p, &analyzer, axes, support, limits)?;
            match (self.sectors.get(&(p.id, support)), actual) {
                (None, _) => return Err(sector("no fate".to_owned())),
                (Some(PSectorFate::Zero), None) => {}
                (Some(PSectorFate::Zero), Some(key)) => {
                    return Err(sector(format!(
                        "held as zero, without a proof, in class {}",
                        key.label()
                    )));
                }
                (Some(PSectorFate::Owner { .. }), None) => {
                    return Err(sector("routed, yet it is proved zero".to_owned()));
                }
                (Some(PSectorFate::Owner { class, map }), Some(key)) => {
                    if key.id() != *class {
                        return Err(sector(format!("is in class {}", key.label())));
                    }
                    let owner = owners.get(class).ok_or_else(|| missing_owner(*class))?;
                    if owner.key != key {
                        return Err(sector(format!("its owner is not in class {}", key.label())));
                    }
                    let source = classed_support(p, support, limits)?;
                    if verify_route(&source, owner, map, limits)?.as_ref() != Some(map) {
                        return Err(sector(format!("the route {map:?} does not verify")));
                    }
                }
            }
        }
        Ok(())
    }

    /// Counts per loop order, ascending; a class counts at its own loop order.
    pub fn counts(
        &self,
        presentations: &[Arc<Presentation>],
    ) -> Result<Vec<PCensusCounts>, PError> {
        let ordered = by_id(presentations)?;
        let rows: Vec<_> = ordered.iter().map(|p| (p.id, p.loops)).collect();
        self.counts_over(&rows)
    }

    fn counts_over(
        &self,
        presentations: &[(PresentationId, u8)],
    ) -> Result<Vec<PCensusCounts>, PError> {
        let mut counts: BTreeMap<u8, PCensusCounts> = BTreeMap::new();
        let blank = |loops| PCensusCounts {
            loops,
            presentations: 0,
            sectors: 0,
            zero_sectors: 0,
            nonzero_sectors: 0,
            classes: 0,
        };
        let loops_of: BTreeMap<_, _> = presentations.iter().copied().collect();
        for &(_, loops) in presentations {
            counts
                .entry(loops)
                .or_insert_with(|| blank(loops))
                .presentations += 1;
        }
        for (&(id, support), fate) in &self.sectors {
            let &loops = loops_of.get(&id).ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "sector ({}, {support:#x}) of an unknown presentation",
                    id.0
                ))
            })?;
            let entry = counts.entry(loops).or_insert_with(|| blank(loops));
            entry.sectors += 1;
            match fate {
                PSectorFate::Zero => entry.zero_sectors += 1,
                PSectorFate::Owner { .. } => entry.nonzero_sectors += 1,
            }
        }
        for record in self.classes.values() {
            let loops = record.key.loops;
            counts.entry(loops).or_insert_with(|| blank(loops)).classes += 1;
        }
        Ok(counts.into_values().collect())
    }

    /// The census as pretty JSON, schema [`P_CENSUS_SCHEMA`]. Refuses presentations
    /// that are not those of the census, by [`Self::from_json`]'s check of the rows.
    pub fn to_json(&self, presentations: &[Arc<Presentation>]) -> Result<String, PError> {
        let ordered = by_id(presentations)?;
        let file = CensusFile {
            schema: P_CENSUS_SCHEMA.to_owned(),
            versions: versions_of_this_build(),
            counts: self.counts(presentations)?,
            presentations: ordered
                .iter()
                .map(|p| {
                    let axes = Axes::of(p)?;
                    Ok(PresentationRow {
                        id: p.id.0,
                        name: p.family.name().to_owned(),
                        loops: p.loops,
                        lines: axes.lines.count_ones(),
                        forced: axes.forced,
                        node: p.node.as_ref().map(|node| node.index.0),
                        node_support: p.node.as_ref().map(|_| node_support(p, axes)).transpose()?,
                    })
                })
                .collect::<Result<_, PError>>()?,
            classes: self
                .classes
                .iter()
                .map(|(id, record)| ClassRow {
                    id: id_text(*id),
                    label: record.key.label(),
                    canonical: hex_text(&record.key.canonical),
                    owner: SectorRef {
                        presentation: record.owner.0.0,
                        support: record.owner.1,
                    },
                    node: record.node.map(|node| node.0),
                    nodes: record.nodes.iter().map(|node| node.0).collect(),
                    t: record.t,
                    eps_lines: record.eps_lines,
                })
                .collect(),
            sectors: self
                .sectors
                .iter()
                .map(|(&(id, support), fate)| {
                    let (class, map) = match fate {
                        PSectorFate::Zero => (None, None),
                        PSectorFate::Owner { class, map } => (
                            Some(id_text(*class)),
                            Some(MapRow {
                                loops_in: map.loops_in,
                                loops_out: map.loops_out,
                                loop_linear: map.loop_linear.to_vec(),
                                loop_external: map.loop_external.to_vec(),
                                external_sign: map.external_sign,
                            }),
                        ),
                    };
                    SectorRow {
                        presentation: id.0,
                        support,
                        class,
                        map,
                    }
                })
                .collect(),
        };
        self.check_rows(&file.presentations)?;
        let mut text = serde_json::to_string_pretty(&file)
            .map_err(|error| PError::Generation(format!("census JSON: {error}")))?;
        text.push('\n');
        Ok(text)
    }

    /// The census of [`Self::to_json`]. Refuses another schema, another value of any
    /// of [`p_versions`], and rows that disagree with each other:
    /// - a class is stored under the id and label of its canonical bytes, with their
    ///   lines and eps lines;
    /// - a presentation row holds one sector per support and no other sector;
    /// - a routed sector names a class that has a record and the same loops, lines and
    ///   eps lines, and holds a unimodular map on those loops;
    /// - the owner of a class holds the identity, on the sector of the node the record
    ///   names, or on the first member of the class if the census has no nodes;
    /// - the nodes of a class are those whose sector is in it, ascending;
    /// - the counts are those of the rows.
    ///
    /// The file holds neither graphs nor families, so nothing here repeats a zero
    /// proof, verifies a route, compares a key or applies the min-GraphKey rule:
    /// [`Self::check_complete`] remains the check against the presentations.
    pub fn from_json(json: &str) -> Result<Self, PError> {
        let file: CensusFile =
            serde_json::from_str(json).map_err(|error| malformed_file(error.to_string()))?;
        if file.schema != P_CENSUS_SCHEMA {
            return Err(PError::VersionMismatch {
                name: "P_CENSUS_SCHEMA".to_owned(),
                found: file.schema,
                expected: P_CENSUS_SCHEMA.to_owned(),
            });
        }
        check_versions(&file.versions)?;
        let mut census = Self::default();
        for row in file.classes {
            let bytes = hex_bytes(&row.canonical)
                .ok_or_else(|| malformed_file(format!("class {}: canonical bytes", row.id)))?;
            let &[loops, lines, eps_lines, ..] = bytes.as_slice() else {
                return Err(malformed_file(format!("class {}: canonical bytes", row.id)));
            };
            let key = OwnerClassKey {
                loops,
                lines,
                eps_lines,
                canonical: bytes.into(),
            };
            if id_text(key.id()) != row.id || key.label() != row.label {
                return Err(malformed_file(format!(
                    "class {} ({}) holds the key {}",
                    row.id,
                    row.label,
                    key.label()
                )));
            }
            let id = key.id();
            let record = PClassRecord {
                key,
                owner: (PresentationId(row.owner.presentation), row.owner.support),
                node: row.node.map(NodeIndex),
                nodes: row.nodes.into_iter().map(NodeIndex).collect(),
                t: row.t,
                eps_lines: row.eps_lines,
            };
            if census.classes.insert(id, record).is_some() {
                return Err(malformed_file(format!("class {} appears twice", row.id)));
            }
        }
        for row in file.sectors {
            let fate = match (row.class, row.map) {
                (None, None) => PSectorFate::Zero,
                (Some(class), Some(map)) => PSectorFate::Owner {
                    class: id_value(&class)
                        .ok_or_else(|| malformed_file(format!("class id {class}")))?,
                    map: SignedAffineLoopMap {
                        loops_in: map.loops_in,
                        loops_out: map.loops_out,
                        loop_linear: map.loop_linear.into(),
                        loop_external: map.loop_external.into(),
                        external_sign: map.external_sign,
                    },
                },
                _ => {
                    return Err(malformed_file(format!(
                        "sector ({}, {:#x}): a class needs a map and a map a class",
                        row.presentation, row.support
                    )));
                }
            };
            let sector = (PresentationId(row.presentation), row.support);
            if census.sectors.insert(sector, fate).is_some() {
                return Err(malformed_file(format!(
                    "sector ({}, {:#x}) appears twice",
                    row.presentation, row.support
                )));
            }
        }
        census.check_rows(&file.presentations)?;
        let rows: Vec<_> = file
            .presentations
            .iter()
            .map(|row| (PresentationId(row.id), row.loops))
            .collect();
        if census.counts_over(&rows)? != file.counts {
            return Err(malformed_file(
                "the counts disagree with the rows".to_owned(),
            ));
        }
        Ok(census)
    }

    /// The rows of a census file against each other: see [`Self::from_json`].
    fn check_rows(&self, rows: &[PresentationRow]) -> Result<(), PError> {
        let axes = row_axes(rows)?;
        let first_members = self.check_sector_rows(rows, &axes)?;
        self.check_class_rows(rows, &first_members)
    }

    /// Every sector lies on its presentation row and fits its class, and every row
    /// holds one sector per support. Returns the first member of each class.
    fn check_sector_rows(
        &self,
        rows: &[PresentationRow],
        axes: &BTreeMap<u32, Axes>,
    ) -> Result<BTreeMap<OwnerClassId, (PresentationId, u64)>, PError> {
        let loops_of: BTreeMap<_, _> = rows.iter().map(|row| (row.id, row.loops)).collect();
        let mut held: BTreeMap<u32, usize> = BTreeMap::new();
        let mut first_members = BTreeMap::new();
        for (&(id, support), fate) in &self.sectors {
            let sector = |detail: String| {
                malformed_file(format!("sector ({}, {support:#x}) {detail}", id.0))
            };
            let row = axes
                .get(&id.0)
                .ok_or_else(|| sector("has no presentation row".to_owned()))?;
            if !row.holds(support) {
                return Err(sector(format!(
                    "leaves its row: lines {:#x}, forced {:#x}",
                    row.lines, row.forced
                )));
            }
            *held.entry(id.0).or_default() += 1;
            let PSectorFate::Owner { class, map } = fate else {
                continue;
            };
            let record = self.classes.get(class).ok_or_else(|| {
                sector(format!(
                    "names the class {} without a record",
                    id_text(*class)
                ))
            })?;
            let key = &record.key;
            let shape = (
                loops_of[&id.0],
                support.count_ones(),
                (support & row.forced).count_ones(),
            );
            if shape != (key.loops, u32::from(key.lines), u32::from(key.eps_lines)) {
                return Err(sector(format!(
                    "has (loops, lines, eps lines) = {shape:?}, unlike its class {}",
                    key.label()
                )));
            }
            if (map.loops_in, map.loops_out) != (key.loops, key.loops) || !map.is_unimodular() {
                return Err(sector(format!(
                    "holds no unimodular map on {} loops: {map:?}",
                    key.loops
                )));
            }
            first_members.entry(*class).or_insert((id, support));
        }
        for (id, row) in axes {
            let count = held.get(id).copied().unwrap_or(0);
            if count != row.support_count() {
                return Err(malformed_file(format!(
                    "presentation {id} holds {count} sectors for {} supports",
                    row.support_count()
                )));
            }
        }
        Ok(first_members)
    }

    /// Every record fits its canonical bytes, holds the identity at its owner, and
    /// names the owner and the nodes that the rows give.
    fn check_class_rows(
        &self,
        rows: &[PresentationRow],
        first_members: &BTreeMap<OwnerClassId, (PresentationId, u64)>,
    ) -> Result<(), PError> {
        let with_node = self.classes.values().filter(|r| r.node.is_some()).count();
        if with_node != 0 && with_node != self.classes.len() {
            return Err(malformed_file(format!(
                "{with_node} of {} classes have a node",
                self.classes.len()
            )));
        }
        let mut node_sectors: BTreeMap<OwnerClassId, BTreeMap<NodeIndex, _>> = BTreeMap::new();
        if with_node != 0 {
            for row in rows {
                let (Some(node), Some(support)) = (row.node, row.node_support) else {
                    continue;
                };
                let sector = (PresentationId(row.id), support);
                let Some(PSectorFate::Owner { class, .. }) = self.sectors.get(&sector) else {
                    return Err(malformed_file(format!(
                        "node {node} stands on ({}, {support:#x}), which has no class",
                        row.id
                    )));
                };
                let nodes = node_sectors.entry(*class).or_default();
                nodes.insert(NodeIndex(node), sector);
            }
        }
        let none = BTreeMap::new();
        for (id, record) in &self.classes {
            let class =
                |detail: String| malformed_file(format!("class {} {detail}", record.key.label()));
            if (record.t, record.eps_lines) != (record.key.lines, record.key.eps_lines) {
                return Err(class(format!(
                    "records {} lines, {} eps lines",
                    record.t, record.eps_lines
                )));
            }
            match self.sectors.get(&record.owner) {
                Some(PSectorFate::Owner { class: routed, map })
                    if routed == id && *map == identity_map(record.key.loops) => {}
                _ => {
                    return Err(class(format!(
                        "has no identity route at its owner ({}, {:#x})",
                        record.owner.0.0, record.owner.1
                    )));
                }
            }
            let nodes = node_sectors.get(id).unwrap_or(&none);
            let owner = match record.node {
                Some(node) => nodes.get(&node),
                None => first_members.get(id),
            };
            if owner != Some(&record.owner) || !record.nodes.iter().eq(nodes.keys()) {
                return Err(class(format!(
                    "has the owner {:?} at node {:?} with the nodes {:?}; the rows give {nodes:?}",
                    record.owner, record.node, record.nodes
                )));
            }
        }
        Ok(())
    }
}

fn malformed_file(detail: String) -> PError {
    PError::MalformedLibrary(format!("census JSON: {detail}"))
}

fn versions_of_this_build() -> BTreeMap<String, String> {
    (p_versions().into_iter())
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// A file holds [`p_versions`] exactly: every constant, with the value of this build.
fn check_versions(file: &BTreeMap<String, String>) -> Result<(), PError> {
    let build = versions_of_this_build();
    let absent = || "absent".to_owned();
    for name in file.keys().chain(build.keys()) {
        let (found, expected) = (file.get(name), build.get(name));
        if found != expected {
            return Err(PError::VersionMismatch {
                name: name.clone(),
                found: found.cloned().unwrap_or_else(absent),
                expected: expected.cloned().unwrap_or_else(absent),
            });
        }
    }
    Ok(())
}

/// The axes of the presentation rows by id: lines are the first slots of a valid
/// presentation. Two rows of one presentation or of one node fail.
fn row_axes(rows: &[PresentationRow]) -> Result<BTreeMap<u32, Axes>, PError> {
    let mut axes = BTreeMap::new();
    let mut nodes = BTreeSet::new();
    for row in rows {
        let refusal = |detail: String| malformed_file(format!("presentation {} {detail}", row.id));
        if row.lines > MAX_CENSUS_LINES {
            return Err(refusal(format!("has {} lines", row.lines)));
        }
        let lines = (1_u64 << row.lines) - 1;
        if row.forced & !lines != 0 {
            return Err(refusal(format!(
                "forces {:#x} outside its lines {lines:#x}",
                row.forced
            )));
        }
        if row.node.is_some() != row.node_support.is_some() {
            return Err(refusal("needs a node and its support together".to_owned()));
        }
        if let Some(node) = row.node
            && !nodes.insert(node)
        {
            return Err(refusal(format!("is a second presentation of node {node}")));
        }
        let held = Axes {
            lines,
            forced: row.forced,
        };
        if axes.insert(row.id, held).is_some() {
            return Err(refusal("appears twice".to_owned()));
        }
    }
    Ok(axes)
}

fn missing_owner(class: OwnerClassId) -> PError {
    PError::MissingOwner {
        class,
        ledger: CENSUS_NO_LEDGER,
    }
}

fn id_text(id: OwnerClassId) -> String {
    format!("{:016x}", id.0)
}

fn id_value(text: &str) -> Option<OwnerClassId> {
    (text.len() == 16)
        .then(|| u64::from_str_radix(text, 16).ok().map(OwnerClassId))
        .flatten()
}

fn hex_text(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        write!(text, "{byte:02x}").expect("writing to a string");
    }
    text
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.is_ascii() || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CensusFile {
    schema: String,
    /// [`p_versions`] by name.
    versions: BTreeMap<String, String>,
    counts: Vec<PCensusCounts>,
    presentations: Vec<PresentationRow>,
    classes: Vec<ClassRow>,
    sectors: Vec<SectorRow>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationRow {
    id: u32,
    name: String,
    loops: u8,
    lines: u32,
    forced: u64,
    node: Option<u32>,
    /// The sector that stands for `node`: all lines, on an Induced presentation its
    /// mask.
    node_support: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectorRef {
    presentation: u32,
    support: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassRow {
    id: String,
    label: String,
    canonical: String,
    owner: SectorRef,
    node: Option<u32>,
    nodes: Vec<u32>,
    t: u8,
    eps_lines: u8,
}

/// A zero sector has neither `class` nor `map`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectorRow {
    presentation: u32,
    support: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    map: Option<MapRow>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapRow {
    loops_in: u8,
    loops_out: u8,
    loop_linear: Vec<i64>,
    loop_external: Vec<i64>,
    external_sign: i8,
}
