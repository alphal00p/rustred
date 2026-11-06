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
//! [`PCensus::check_complete`] trusts nothing it is given: it repeats the zero
//! proofs and verifies every stored route again, so it costs about what the census
//! does. A loader that wants to skip it has to say so.
//!
//! # Versioning
//!
//! Bump [`P_CENSUS_VERSION`] when a fate, an owner or a stored route can change
//! for the same input: it is part of the identity of every library program. The
//! JSON form ([`PCensus::to_json`], [`P_CENSUS_SCHEMA`]) records it and the class
//! key version, and [`PCensus::from_json`] refuses any other value.
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
use super::topology::{Dag, GraphKey};
use super::{NodeIndex, OwnerClassId, P_CENSUS_VERSION, P_CLASS_KEY_VERSION, PresentationId};

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
        let map = map.ok_or_else(|| {
            PError::Generation(format!(
                "sector ({}, {support:#x}) has no verified route onto its class {}",
                id.0,
                source.key.label()
            ))
        })?;
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
    /// class of every DAG node has an owner. The zero proofs are repeated and
    /// every route is verified again. What an owner runs is checked by M2-T8a.
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

    /// The census as pretty JSON, schema [`P_CENSUS_SCHEMA`].
    pub fn to_json(&self, presentations: &[Arc<Presentation>]) -> Result<String, PError> {
        let ordered = by_id(presentations)?;
        let file = CensusFile {
            schema: P_CENSUS_SCHEMA.to_owned(),
            census_version: P_CENSUS_VERSION,
            class_key_version: P_CLASS_KEY_VERSION,
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
        let mut text = serde_json::to_string_pretty(&file)
            .map_err(|error| PError::Generation(format!("census JSON: {error}")))?;
        text.push('\n');
        Ok(text)
    }

    /// The census of [`Self::to_json`]. Refuses another schema or version and
    /// everything that disagrees with itself; [`Self::check_complete`] remains
    /// the check against the presentations.
    pub fn from_json(json: &str) -> Result<Self, PError> {
        let malformed = |detail: String| PError::MalformedLibrary(format!("census JSON: {detail}"));
        let file: CensusFile =
            serde_json::from_str(json).map_err(|error| malformed(error.to_string()))?;
        if file.schema != P_CENSUS_SCHEMA {
            return Err(PError::VersionMismatch {
                name: "P_CENSUS_SCHEMA".to_owned(),
                found: file.schema,
                expected: P_CENSUS_SCHEMA.to_owned(),
            });
        }
        for (name, found, expected) in [
            ("P_CENSUS_VERSION", file.census_version, P_CENSUS_VERSION),
            (
                "P_CLASS_KEY_VERSION",
                file.class_key_version,
                P_CLASS_KEY_VERSION,
            ),
        ] {
            if found != expected {
                return Err(PError::VersionMismatch {
                    name: name.to_owned(),
                    found: found.to_string(),
                    expected: expected.to_string(),
                });
            }
        }
        let mut census = Self::default();
        for row in file.classes {
            let bytes = hex_bytes(&row.canonical)
                .ok_or_else(|| malformed(format!("class {}: canonical bytes", row.id)))?;
            let &[loops, lines, eps_lines, ..] = bytes.as_slice() else {
                return Err(malformed(format!("class {}: canonical bytes", row.id)));
            };
            let key = OwnerClassKey {
                loops,
                lines,
                eps_lines,
                canonical: bytes.into(),
            };
            if id_text(key.id()) != row.id || key.label() != row.label {
                return Err(malformed(format!(
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
                return Err(malformed(format!("class {} appears twice", row.id)));
            }
        }
        for row in file.sectors {
            let fate = match (row.class, row.map) {
                (None, None) => PSectorFate::Zero,
                (Some(class), Some(map)) => PSectorFate::Owner {
                    class: id_value(&class)
                        .ok_or_else(|| malformed(format!("class id {class}")))?,
                    map: SignedAffineLoopMap {
                        loops_in: map.loops_in,
                        loops_out: map.loops_out,
                        loop_linear: map.loop_linear.into(),
                        loop_external: map.loop_external.into(),
                        external_sign: map.external_sign,
                    },
                },
                _ => {
                    return Err(malformed(format!(
                        "sector ({}, {:#x}): a class needs a map and a map a class",
                        row.presentation, row.support
                    )));
                }
            };
            let sector = (PresentationId(row.presentation), row.support);
            if census.sectors.insert(sector, fate).is_some() {
                return Err(malformed(format!(
                    "sector ({}, {:#x}) appears twice",
                    row.presentation, row.support
                )));
            }
        }
        let rows: Vec<_> = file
            .presentations
            .iter()
            .map(|row| (PresentationId(row.id), row.loops))
            .collect();
        if census.counts_over(&rows)? != file.counts {
            return Err(malformed("the counts disagree with the rows".to_owned()));
        }
        Ok(census)
    }
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
    census_version: u32,
    class_key_version: u32,
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
