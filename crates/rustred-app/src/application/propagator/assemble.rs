//! Library assembly for scope 1..max_loops (owner: M2-T8a rr-assemble).
//!
//! [`assemble_library`] builds the complete in-memory library program of plan
//! M2 section 3.2 from a families file:
//!
//! 1. tops: every family with `1 <= L <= max_loops`, ids in file order;
//! 2. the reduction DAG of their graphs, `ChildMode::ForcerTree`, through one
//!    [`TopologyService`];
//! 3. per node `present_node` (lane, roles, lane-adapted presentation, or the
//!    Custom fallback with its reason); a Triangle node whose IBP row does not
//!    have Forcer's shape (`TemplateError::Mismatch`) falls back to Custom too;
//! 4. the census over the tops and every node presentation, with strong
//!    completeness;
//! 5. one owner per class at its min-GraphKey node: lane and roles of that node,
//!    the verified edge map of every rule edge into a node with a presentation
//!    (a rule edge into the trivial node ends in the terminal `Unit`), the Custom
//!    payload and meta;
//! 6. the [`PLibraryBuilder`](rustred::propagator::library::PLibraryBuilder),
//!    every owner program built once (a class without a lane program fails),
//!    and the descent certificate.
//!
//! Every step is deterministic: ids follow the families file, the DAG order and
//! the class ids. Any error stops the assembly; nothing changes a lane except a
//! lane contract the presentation cannot meet, which is recorded.
//!
//! [`validate_library`] is `rustred p-validate`: random keys of the tops traced
//! through the library without coefficients ([`trace`]).
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::CoefficientContext;
use rustred::propagator::census::{PCensusOptions, p_census};
use rustred::propagator::descent::{DescentReport, TraceReport, check, trace};
use rustred::propagator::error::PError;
use rustred::propagator::families::{PFamiliesFile, PFamilyEntry};
use rustred::propagator::lane::{LaneDecision, LaneRoles, OwnerLane};
use rustred::propagator::library::{PLibrary, POwnerSpec};
use rustred::propagator::present::{present, present_node, top_topology};
use rustred::propagator::presentation::{Presentation, Slot};
use rustred::propagator::topology::{ChildMode, Dag, DagEdgeKind, TopologyService};
use rustred::propagator::triangle::{TemplateError, TriangleTemplate};
use rustred::propagator::{NodeIndex, OwnerClassId, PTerminalId, PresentationId};
use rustred::reduction::ReductionLimits;

use super::CustomPayloads;
use super::generate::{CustomOwnerPayload, generate_custom_owner};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembleOptions {
    /// Census worker threads; `0` means one. The library does not depend on it.
    pub threads: usize,
    /// Run the descent certificate on the assembled library.
    pub verify_descent: bool,
}

impl Default for AssembleOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            verify_descent: true,
        }
    }
}

/// What assembly made of one DAG node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledNode {
    pub index: NodeIndex,
    /// The decision the node is presented under (Custom after a fallback).
    pub decision: LaneDecision,
    /// `None` on the trivial node.
    pub presentation: Option<PresentationId>,
    /// Why the node left its lane for Custom.
    pub fallback: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AssembledLibrary {
    pub library: Arc<PLibrary>,
    /// The families of the scope, in file order: the tops are presentations
    /// `0..families.families.len()`.
    pub families: PFamiliesFile,
    pub max_loops: u8,
    pub dag: Arc<Dag>,
    /// One entry per DAG node, in DAG order.
    pub nodes: Vec<AssembledNode>,
    /// The Custom payloads by class label.
    pub custom: BTreeMap<String, CustomOwnerPayload>,
    /// `None` when `verify_descent` was off.
    pub descent: Option<DescentReport>,
    /// Wall seconds per assembly phase.
    pub timings: Vec<(&'static str, f64)>,
}

impl AssembledLibrary {
    /// The top presentations of `loops` loops.
    pub fn tops_of(&self, loops: u8) -> Vec<PresentationId> {
        (self.families.families.iter().enumerate())
            .filter(|(_, entry)| entry.loops == loops)
            .map(|(index, _)| PresentationId(index as u32))
            .collect()
    }
}

const CUSTOM: LaneDecision = LaneDecision {
    lane: OwnerLane::Custom,
    roles: LaneRoles::None,
    trivial: false,
};

struct Clock(Instant, Vec<(&'static str, f64)>);

impl Clock {
    fn lap(&mut self, phase: &'static str) {
        let now = Instant::now();
        self.1.push((phase, (now - self.0).as_secs_f64()));
        self.0 = now;
    }
}

/// The library program of scope `1..=max_loops` of `families`, see the module
/// docs. `Provided` payloads are used verbatim, by class label.
pub fn assemble_library(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
) -> Result<AssembledLibrary, PError> {
    let mut clock = Clock(Instant::now(), Vec::new());
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let entries: Vec<PFamilyEntry> = (families.families.iter())
        .filter(|entry| entry.loops <= max_loops)
        .cloned()
        .collect();
    if max_loops == 0 || entries.is_empty() {
        return Err(PError::Unsupported(format!(
            "no family with 1 to {max_loops} loops to assemble"
        )));
    }
    let scope = PFamiliesFile {
        families: entries,
        ..families.clone()
    };

    // 1. Tops, ids in file order.
    let mut presentations = Vec::new();
    for (index, entry) in scope.families.iter().enumerate() {
        let id = PresentationId(u32::try_from(index).map_err(|_| too_many())?);
        presentations.push(Arc::new(entry.presentation(id, &context)?));
    }
    let tops = presentations.clone();

    // 2. The DAG.
    let roots: Vec<_> = scope.families.iter().map(top_topology).collect();
    let dag = Arc::new(TopologyService::new().reduction_graph(&roots, ChildMode::ForcerTree));
    clock.lap("dag");

    // 3. Node presentations.
    let mut nodes = Vec::with_capacity(dag.nodes.len());
    for node in &dag.nodes {
        let id = PresentationId(u32::try_from(presentations.len()).map_err(|_| too_many())?);
        let made = present_node(node, &tops, id, &context)?;
        let (mut decision, mut presentation, mut fallback) =
            (made.decision, made.presentation, made.fallback);
        if let (LaneRoles::Triangle(roles), Some(adapted)) = (decision.roles, &presentation) {
            match TriangleTemplate::extract(adapted, &roles) {
                Ok(_) => {}
                Err(TemplateError::Mismatch(mismatch)) => {
                    decision = CUSTOM;
                    presentation = Some(present(node, &CUSTOM, &tops, id, &context)?);
                    fallback = Some(mismatch.to_string());
                }
                Err(TemplateError::Failure(error)) => return Err(error),
            }
        }
        let index = NodeIndex(node.index);
        let presentation = presentation.map(|p| {
            presentations.push(Arc::new(p));
            id
        });
        nodes.push(AssembledNode {
            index,
            decision,
            presentation,
            fallback,
        });
    }
    clock.lap("present");

    // 4. The census, strongly complete.
    let census_options = PCensusOptions {
        threads: options.threads,
        ..PCensusOptions::default()
    };
    let census = p_census(&presentations, Some(&dag), &census_options)?;
    clock.lap("census");
    census.check_complete_with(&presentations, Some(&dag), &census_options)?;
    clock.lap("census-check");

    // 5. Owners, edge maps and terminals.
    let mut builder = PLibrary::builder(context, 1..=max_loops);
    for presentation in &presentations {
        builder.add_presentation(Presentation::clone(presentation))?;
    }
    builder.set_topology(Arc::clone(&dag))?;
    let mut payloads = BTreeMap::new();
    let mut owners = Vec::with_capacity(census.classes.len());
    for (&class, record) in &census.classes {
        let label = record.key.label();
        let node_index = record
            .node
            .ok_or_else(|| PError::MalformedLibrary(format!("class {label} has no DAG node")))?;
        let node = nodes.get(node_index.0 as usize).ok_or_else(|| {
            PError::MalformedLibrary(format!("class {label} names node {}", node_index.0))
        })?;
        let (owner_presentation, _) = record.owner;
        if node.presentation != Some(owner_presentation) {
            return Err(PError::MalformedLibrary(format!(
                "class {label} is owned by presentation {}, its node {} has {:?}",
                owner_presentation.0, node_index.0, node.presentation
            )));
        }
        let mut spec = POwnerSpec {
            class,
            lane: node.decision.lane,
            presentation: owner_presentation,
            node: Some(node_index),
            roles: node.decision.roles,
            edge_maps: Vec::new(),
            payload: None,
            custom: None,
            terminals: Vec::new(),
        };
        if spec.lane == OwnerLane::Custom {
            let payload = custom_payload(
                &custom,
                &label,
                &presentations[owner_presentation.0 as usize],
            )?;
            custom_owner(&mut spec, &payload)?;
            payloads.insert(label, payload);
        } else {
            for edge in dag.edges_from(node_index.0) {
                if edge.kind != DagEdgeKind::Rule {
                    continue;
                }
                let child = nodes.get(edge.child as usize).ok_or_else(|| {
                    PError::MalformedLibrary(format!("edge to the unknown node {}", edge.child))
                })?;
                match child.presentation {
                    Some(child) => {
                        let map = rustred::propagator::present::edge_map(
                            &presentations[owner_presentation.0 as usize],
                            &presentations[child.0 as usize],
                            edge,
                        )?;
                        spec.edge_maps.push(builder.add_edge_map(map)?);
                    }
                    None => spec.terminals.push(PTerminalId(0)),
                }
            }
        }
        owners.push(spec);
    }
    builder.set_census(census)?;
    for spec in owners {
        builder.add_owner(spec)?;
    }
    let library = Arc::new(builder.build()?);
    clock.lap("build");

    // 6. Every program once, then the certificate.
    for owner in library.owners() {
        owner.program(&library)?;
    }
    clock.lap("programs");
    let descent = if options.verify_descent {
        let report = check(&library)?;
        clock.lap("descent");
        Some(report)
    } else {
        None
    };
    Ok(AssembledLibrary {
        library,
        families: scope,
        max_loops,
        dag,
        nodes,
        custom: payloads,
        descent,
        timings: clock.1,
    })
}

fn too_many() -> PError {
    PError::MalformedLibrary("too many presentations".to_owned())
}

/// The payload of the Custom owner `label`: generated, or provided verbatim.
fn custom_payload(
    custom: &CustomPayloads,
    label: &str,
    presentation: &Presentation,
) -> Result<CustomOwnerPayload, PError> {
    match custom {
        CustomPayloads::Generate(options) => generate_custom_owner(presentation, options),
        CustomPayloads::Provided(payloads) => payloads.get(label).cloned().ok_or_else(|| {
            PError::MalformedLibrary(format!(
                "no payload is provided for the Custom owner {label}"
            ))
        }),
    }
}

/// The payload, meta and residual terminals of a Custom owner (M2-T7c).
fn custom_owner(_spec: &mut POwnerSpec, _payload: &CustomOwnerPayload) -> Result<(), PError> {
    Err(PError::NotImplemented("M2-T7c"))
}

// ---------------------------------------------------------------------------
// p-validate
// ---------------------------------------------------------------------------

/// The random keys of `rustred p-validate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidateOptions {
    /// Distinct keys per loop count: fewer when the box holds fewer.
    pub points: usize,
    pub seed: u64,
    /// Total dots over the lines of a key (excess of positive powers over 1).
    pub max_dots: u32,
    /// Total numerator rank: negative line powers and ISP powers.
    pub max_rank: u32,
    pub limits: ReductionLimits,
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            points: 10_000,
            seed: 0x7a11_da7e,
            max_dots: 3,
            max_rank: 8,
            limits: ReductionLimits::default(),
        }
    }
}

/// One loop count of [`validate_library`].
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateRow {
    pub loops: u8,
    /// Distinct keys drawn (the trace's `distinct_inputs`).
    pub keys: usize,
    /// All keys of the box when it is smaller than the request.
    pub box_exhausted: bool,
    pub report: TraceReport,
    pub seconds: f64,
}

/// A small deterministic generator (SplitMix64).
#[derive(Clone, Debug)]
pub struct SplitMix(u64);

impl SplitMix {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..bound`, `bound > 0`.
    pub fn below(&mut self, bound: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(bound)) >> 64) as u64
    }
}

/// A random key of `p`: a random subset of its lines at positive powers with
/// `dots` extra units, `rank` numerator units on the other lines and the ISPs.
pub fn random_key(p: &Presentation, dots: u32, rank: u32, rng: &mut SplitMix) -> Vec<i64> {
    let lines: Vec<usize> = (p.slots.iter().enumerate())
        .filter(|(_, slot)| matches!(slot, Slot::Line(_)))
        .map(|(slot, _)| slot)
        .collect();
    let mut powers = vec![0_i64; p.slots.len()];
    let mut positive = Vec::new();
    for &slot in &lines {
        if rng.below(4) != 0 {
            powers[slot] = 1;
            positive.push(slot);
        }
    }
    if !positive.is_empty() {
        for _ in 0..dots {
            powers[positive[rng.below(positive.len() as u64) as usize]] += 1;
        }
    }
    let numerators: Vec<usize> = (0..p.slots.len()).filter(|&s| powers[s] == 0).collect();
    if !numerators.is_empty() {
        for _ in 0..rank {
            powers[numerators[rng.below(numerators.len() as u64) as usize]] -= 1;
        }
    }
    powers
}

/// Every key of `p` within the bounds, if there are at most `limit`.
fn enumerate_box(
    p: &Presentation,
    options: &ValidateOptions,
    limit: usize,
) -> Option<Vec<Vec<i64>>> {
    let is_line: Vec<bool> = p.slots.iter().map(|s| matches!(s, Slot::Line(_))).collect();
    let mut out = Vec::new();
    let mut powers = vec![0_i64; p.slots.len()];
    fn walk(
        slot: usize,
        dots: u32,
        rank: u32,
        is_line: &[bool],
        powers: &mut Vec<i64>,
        out: &mut Vec<Vec<i64>>,
        limit: usize,
    ) -> bool {
        if out.len() > limit {
            return false;
        }
        if slot == powers.len() {
            out.push(powers.clone());
            return true;
        }
        for r in 0..=rank {
            powers[slot] = -i64::from(r);
            if !walk(slot + 1, dots, rank - r, is_line, powers, out, limit) {
                return false;
            }
        }
        if is_line[slot] {
            for e in 0..=dots {
                powers[slot] = 1 + i64::from(e);
                if !walk(slot + 1, dots - e, rank, is_line, powers, out, limit) {
                    return false;
                }
            }
        }
        powers[slot] = 0;
        true
    }
    let complete = walk(
        0,
        options.max_dots,
        options.max_rank,
        &is_line,
        &mut powers,
        &mut out,
        limit,
    );
    (complete && out.len() <= limit).then_some(out)
}

/// `points` distinct random keys over the tops of `loops` loops; the whole box
/// when it holds at most `points` keys.
pub fn validation_keys(
    assembled: &AssembledLibrary,
    loops: u8,
    options: &ValidateOptions,
) -> (Vec<(PresentationId, Vec<i64>)>, bool) {
    let tops = assembled.tops_of(loops);
    let library = &assembled.library;
    let mut whole = Vec::new();
    for &top in &tops {
        match enumerate_box(library.presentation(top), options, options.points) {
            Some(keys) if whole.len() + keys.len() <= options.points => {
                whole.extend(keys.into_iter().map(|powers| (top, powers)));
            }
            _ => {
                whole.clear();
                break;
            }
        }
    }
    if !whole.is_empty() {
        return (whole, true);
    }
    let mut rng = SplitMix::new(options.seed ^ u64::from(loops).wrapping_mul(0x9e37_79b9));
    let mut keys = BTreeSet::new();
    let mut attempts = 0_usize;
    while keys.len() < options.points
        && attempts < options.points.saturating_mul(50)
        && !tops.is_empty()
    {
        attempts += 1;
        let top = tops[rng.below(tops.len() as u64) as usize];
        let dots = rng.below(u64::from(options.max_dots) + 1) as u32;
        let rank = rng.below(u64::from(options.max_rank) + 1) as u32;
        keys.insert((
            top,
            random_key(library.presentation(top), dots, rank, &mut rng),
        ));
    }
    (keys.into_iter().collect(), false)
}

/// `rustred p-validate`: per loop count of the scope, distinct random keys of the
/// tops traced through the library without coefficients, with
/// `PUncoveredPolicy::Reject` semantics.
pub fn validate_library(
    assembled: &AssembledLibrary,
    options: &ValidateOptions,
) -> Result<Vec<ValidateRow>, PError> {
    let mut rows = Vec::new();
    for loops in 1..=assembled.max_loops {
        let start = Instant::now();
        let (keys, box_exhausted) = validation_keys(assembled, loops, options);
        let report = trace(&assembled.library, &keys, options.limits)?;
        rows.push(ValidateRow {
            loops,
            keys: report.distinct_inputs,
            box_exhausted,
            report,
            seconds: start.elapsed().as_secs_f64(),
        });
    }
    Ok(rows)
}

/// Owner classes of `assembled` that a validation run never applied.
pub fn unapplied_owners(assembled: &AssembledLibrary, rows: &[ValidateRow]) -> Vec<OwnerClassId> {
    let applied: BTreeSet<OwnerClassId> = rows
        .iter()
        .flat_map(|row| row.report.applied.keys().copied())
        .collect();
    (assembled.library.owners())
        .map(|owner| owner.spec.class)
        .filter(|class| !applied.contains(class))
        .collect()
}
