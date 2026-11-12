//! Library assembly for scope 1..max_loops (owner: M2-T8a rr-assemble).
//!
//! [`assemble_library`] builds the complete in-memory library program of plan
//! M2 section 3.2 from a families file:
//!
//! 1. tops: every family with `1 <= L <= max_loops`, ids in file order; a
//!    loop count of the scope without a family is refused;
//! 2. the reduction DAG of their graphs, `ChildMode::ForcerTree`, through one
//!    [`TopologyService`];
//! 3. per node `present_node` (lane, roles, lane-adapted presentation, or the
//!    Custom fallback with its reason); a Triangle node whose IBP row does not
//!    have Forcer's shape (`TemplateError::Mismatch`) falls back to Custom too,
//!    and so does a Diamond node whose summed rows do not
//!    (`DiamondError::Mismatch`, M3-T5b);
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
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensusOptions, PRouteMaps, p_census};
use rustred::propagator::custom::inspect_custom_payload;
use rustred::propagator::descent::{DescentReport, TraceReport, check, trace};
use rustred::propagator::diamond::{DiamondError, DiamondTemplate};
use rustred::propagator::error::PError;
use rustred::propagator::families::{PFamiliesFile, PFamilyEntry};
use rustred::propagator::lane::{DiamondRoles, LaneDecision, LaneRoles, OwnerLane, TriangleRoles};
use rustred::propagator::library::{PLibrary, PLibraryBuilder, POwnerSpec, PTerminal};
use rustred::propagator::present::{present, present_node, top_topology};
use rustred::propagator::presentation::{Presentation, Slot};
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::topology::{ChildMode, Dag, DagEdgeKind, TopologyService};
use rustred::propagator::triangle::{TemplateError, TriangleTemplate};
use rustred::propagator::{NodeIndex, OwnerClassId, PTerminalId, PresentationId};
use rustred::reduction::ReductionLimits;

use super::generate::{CustomOwnerPayload, custom_owner_lp_count, generate_custom_owner};
use super::{CustomPayloads, ImportedCustomNode};

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
    /// Wall seconds per assembly phase (`owners` includes generating the
    /// Custom payloads).
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

/// The Triangle template check of assembly: `Ok` keeps the node on its lane,
/// a [`TemplateError::Mismatch`] presents it as Custom, a
/// [`TemplateError::Failure`] stops the assembly.
pub type TriangleCheck<'a> = &'a dyn Fn(&Presentation, &TriangleRoles) -> Result<(), TemplateError>;

/// The Diamond template check of assembly, as [`TriangleCheck`] (M3-T5b).
pub type DiamondCheck<'a> = &'a dyn Fn(&Presentation, &DiamondRoles) -> Result<(), DiamondError>;

/// The Diamond check of [`assemble_library`]: the template extracts.
pub fn diamond_template_check(
    presentation: &Presentation,
    roles: &DiamondRoles,
) -> Result<(), DiamondError> {
    DiamondTemplate::extract(presentation, roles).map(|_| ())
}

/// The library program of scope `1..=max_loops` of `families`, see the module
/// docs. `Provided` payloads are used verbatim, by class label, with the LP
/// count their record states (see [`assemble_library_recounted`]).
pub fn assemble_library(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
) -> Result<AssembledLibrary, PError> {
    assemble_library_with(families, max_loops, custom, options, &triangle_template)
}

fn triangle_template(
    presentation: &Presentation,
    roles: &TriangleRoles,
) -> Result<(), TemplateError> {
    TriangleTemplate::extract(presentation, roles).map(|_| ())
}

/// Which `Provided` Custom payloads [`assemble_library_recounted`] recounts
/// (M3-I1, I-2; the follow-up of departure PSF6): an owner whose support has
/// at most `max_lines` lines, or every owner. The default filter is
/// [`Self::from_env`]: the owners of at most [`Self::DEFAULT_MAX_LINES`]
/// lines (seconds each), every owner only when `P_LP_RECOUNT_ALL=1` (a 10 or
/// 11 line count takes about 24 min and 3.4 GB, and M3-T15's count is the
/// one full count, plan M3 D-M3-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecountProvidedLp {
    max_lines: Option<u32>,
}

impl RecountProvidedLp {
    pub const DEFAULT_MAX_LINES: u32 = 9;
    pub const ALL_VARIABLE: &'static str = "P_LP_RECOUNT_ALL";

    /// The owners of at most `max_lines` lines.
    pub fn up_to_lines(max_lines: u32) -> Self {
        Self {
            max_lines: Some(max_lines),
        }
    }

    /// The filter of this process: [`Self::for_variable`] of `P_LP_RECOUNT_ALL`.
    pub fn from_env() -> Self {
        Self::for_variable(std::env::var(Self::ALL_VARIABLE).ok().as_deref())
    }

    /// Every owner for the value `"1"`, else the owners of at most
    /// [`Self::DEFAULT_MAX_LINES`] lines.
    pub fn for_variable(value: Option<&str>) -> Self {
        match value {
            Some("1") => Self { max_lines: None },
            _ => Self::up_to_lines(Self::DEFAULT_MAX_LINES),
        }
    }

    /// `None`: every owner.
    pub fn max_lines(&self) -> Option<u32> {
        self.max_lines
    }

    pub fn recounts(&self, lines: u32) -> bool {
        self.max_lines.is_none_or(|max| lines <= max)
    }
}

/// The recount of one `Provided` Custom payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LpRecount {
    pub label: String,
    /// Lines of the owner support (eps axes included).
    pub lines: u32,
    /// The count of the payload's record.
    pub stored: u32,
    /// `None`: not recounted (more lines than the filter), the stored count is
    /// trusted as the loader trusts it.
    pub recounted: Option<u32>,
}

/// Recount the LP masters of the support of a `Provided` payload of the owner
/// `label` on `presentation` with [`custom_owner_lp_count`], as generation
/// counts them, when `recount` takes an owner of its line count. A count other
/// than the record's is refused ([`PError::MalformedLibrary`]): the record is
/// not the payload's, and the false-knock-out guard would check a count nobody
/// derived.
pub fn recount_provided_lp(
    label: &str,
    presentation: &Presentation,
    payload: &CustomOwnerPayload,
    recount: &RecountProvidedLp,
) -> Result<LpRecount, PError> {
    let support = inspect_custom_payload(&payload.bytes)?.support;
    let lines = support.count_ones();
    let stored = payload.meta.lp_count;
    let mut outcome = LpRecount {
        label: label.to_owned(),
        lines,
        stored,
        recounted: None,
    };
    if recount.recounts(lines) {
        let counted = custom_owner_lp_count(presentation, support)?;
        if counted != stored {
            return Err(PError::MalformedLibrary(format!(
                "the Provided Custom owner {label} records LP {stored}, its support {support:#x} \
                 ({lines} lines) counts LP {counted}"
            )));
        }
        outcome.recounted = Some(counted);
    }
    Ok(outcome)
}

/// [`assemble_library`] that recounts the LP of every `Provided` payload that
/// `recount` takes ([`recount_provided_lp`]) before it builds the owner, and
/// refuses a count other than the record's; with `Generate` payloads it is
/// [`assemble_library`] (generation counts). Returns the recounts in class
/// order. The loader never recounts: LP is not re-derived at load.
pub fn assemble_library_recounted(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
    recount: &RecountProvidedLp,
) -> Result<(AssembledLibrary, Vec<LpRecount>), PError> {
    let mut recounts = Vec::new();
    let assembled = assemble(
        families,
        max_loops,
        custom,
        options,
        &triangle_template,
        &diamond_template_check,
        Some((recount, &mut recounts)),
    )?;
    Ok((assembled, recounts))
}

/// [`assemble_library`] with another Triangle template check: tests of the
/// fallback branches use it.
#[doc(hidden)]
pub fn assemble_library_with(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
    triangle: TriangleCheck<'_>,
) -> Result<AssembledLibrary, PError> {
    let diamond: DiamondCheck<'_> = &diamond_template_check;
    assemble(
        families, max_loops, custom, options, triangle, diamond, None,
    )
}

/// [`assemble_library_with`] with another Diamond template check as well: the
/// Diamond tests use it to present the diamond nodes as Custom owners.
#[doc(hidden)]
pub fn assemble_library_checked(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
    triangle: TriangleCheck<'_>,
    diamond: DiamondCheck<'_>,
) -> Result<AssembledLibrary, PError> {
    assemble(
        families, max_loops, custom, options, triangle, diamond, None,
    )
}

fn assemble(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
    triangle: TriangleCheck<'_>,
    diamond: DiamondCheck<'_>,
    mut recount: Option<(&RecountProvidedLp, &mut Vec<LpRecount>)>,
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
    // Scope 1..=max_loops means every loop count of it: a scope with a gap
    // would be labelled and validated for loop counts it has no top of.
    if let Some(missing) = (1..=max_loops).find(|&loops| entries.iter().all(|e| e.loops != loops)) {
        return Err(PError::Unsupported(format!(
            "scope 1-{max_loops}: the families file has no family with {missing} loops"
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
            match triangle(adapted, &roles) {
                Ok(()) => {}
                Err(TemplateError::Mismatch(mismatch)) => {
                    decision = CUSTOM;
                    presentation = Some(present(node, &CUSTOM, &tops, id, &context)?);
                    fallback = Some(mismatch.to_string());
                }
                Err(TemplateError::Failure(error)) => return Err(error),
            }
        }
        if let (LaneRoles::Diamond(roles), Some(adapted)) = (decision.roles, &presentation) {
            match diamond(adapted, &roles) {
                Ok(()) => {}
                Err(mismatch @ DiamondError::Mismatch { .. }) => {
                    decision = CUSTOM;
                    presentation = Some(present(node, &CUSTOM, &tops, id, &context)?);
                    fallback = Some(mismatch.to_string());
                }
                Err(DiamondError::Failure(error)) => return Err(error),
            }
        }
        if let (LaneRoles::Scheme(_), Some(adapted)) = (decision.roles, &presentation) {
            match rustred::propagator::scheme::SchemeTemplate::extract(adapted, node) {
                Ok(_) => {}
                Err(rustred::propagator::scheme::SchemeError::Mismatch(mismatch)) => {
                    decision = CUSTOM;
                    presentation = Some(present(node, &CUSTOM, &tops, id, &context)?);
                    fallback = Some(mismatch);
                }
                Err(rustred::propagator::scheme::SchemeError::Failure(error)) => return Err(error),
            }
        }
        let index = NodeIndex(node.index);
        if decision.lane == OwnerLane::Custom
            && let Some(imported) = imported_node(&custom, node.key.as_bytes())
        {
            // p5l-library: the import's presentation (its table family) replaces
            // the node presentation; the payload is bound to that family.
            let mut adapted = imported.presentation.clone();
            adapted.id = id;
            adapted.node = Some(rustred::propagator::presentation::NodeRef {
                index,
                graph_key: Arc::clone(&node.key.0),
            });
            adapted.validate()?;
            presentation = Some(adapted);
        }
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
    // Routed sectors store the static-score route maps (P_CENSUS_VERSION 2).
    let census_options = PCensusOptions {
        threads: options.threads,
        route_maps: PRouteMaps::StaticScore,
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
        if spec.lane == OwnerLane::Scheme {
            // M5 A1 schemes: a scheme with a master rule has the node's corner as
            // its one residual terminal, as the Custom owner it replaces has it
            // (same class, same powers); a scheme without one (bubu) has none.
            let info = dag.nodes.get(node_index.0 as usize).ok_or_else(|| {
                PError::MalformedLibrary(format!("class {label} names node {}", node_index.0))
            })?;
            if rustred::propagator::scheme::scheme_has_corner(info)? {
                let presentation = &presentations[owner_presentation.0 as usize];
                let corner = rustred::propagator::basis::node_corner(presentation);
                let key = IntegralKey::try_new(corner).map_err(|error| {
                    PError::MalformedLibrary(format!(
                        "the corner of the Scheme owner {label}: {error}"
                    ))
                })?;
                spec.terminals
                    .push(builder.add_terminal(PTerminal::Residual { owner: class, key })?);
            }
            // M5 no6: a scheme with `"fallback": "custom"` carries the node's
            // Custom payload (generated on its induced Custom presentation);
            // the Scheme program closes the keys its rules do not take with it.
            if rustred::propagator::scheme::scheme_fallback(info)? {
                let induced = present(
                    info,
                    &CUSTOM,
                    &tops,
                    PresentationId(u32::MAX),
                    tops[0].family.coefficient_context(),
                )?;
                let payload = custom_payload(
                    &custom,
                    &label,
                    &induced,
                    recount
                        .as_mut()
                        .map(|(filter, recounts)| (*filter, &mut **recounts)),
                )?;
                spec.payload = Some(CustomPayloadRef::Bytes(Arc::clone(&payload.bytes)));
                spec.custom = Some(payload.meta.clone());
                payloads.insert(label, payload);
            }
        } else if spec.lane == OwnerLane::Custom {
            let owner = &presentations[owner_presentation.0 as usize];
            let imported = (owner.node.as_ref())
                .and_then(|reference| imported_node(&custom, &reference.graph_key));
            let payload = if let Some(imported) = imported {
                imported.payload.clone()
            } else {
                custom_payload(
                    &custom,
                    &label,
                    owner,
                    recount
                        .as_mut()
                        .map(|(filter, recounts)| (*filter, &mut **recounts)),
                )?
            };
            custom_owner(&mut builder, &mut spec, &payload)?;
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
    clock.lap("owners");
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

/// The imported Custom node of `graph_key`, if `custom` has one.
fn imported_node<'a>(
    custom: &'a CustomPayloads,
    graph_key: &[u8],
) -> Option<&'a ImportedCustomNode> {
    match custom {
        CustomPayloads::Imported { imported, .. } => imported.get(graph_key),
        _ => None,
    }
}

fn too_many() -> PError {
    PError::MalformedLibrary("too many presentations".to_owned())
}

/// The payload of the Custom owner `label`: generated, or provided verbatim
/// (its LP recounted under `recount`, [`recount_provided_lp`]).
fn custom_payload(
    custom: &CustomPayloads,
    label: &str,
    presentation: &Presentation,
    recount: Option<(&RecountProvidedLp, &mut Vec<LpRecount>)>,
) -> Result<CustomOwnerPayload, PError> {
    match custom {
        CustomPayloads::Generate(options) => generate_custom_owner(presentation, options),
        CustomPayloads::Provided(payloads)
        | CustomPayloads::Imported {
            provided: payloads, ..
        } => {
            let payload = payloads.get(label).cloned().ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "no payload is provided for the Custom owner {label}"
                ))
            })?;
            if let Some((filter, recounts)) = recount {
                recounts.push(recount_provided_lp(label, presentation, &payload, filter)?);
            }
            Ok(payload)
        }
    }
}

/// The payload, meta and residual terminals of a Custom owner (M2-T7c): one
/// [`PTerminal::Residual`] per residual key of the payload, chart by chart, in
/// the owner's terminals. The payload is kept as bytes; `lane_program` checks
/// that the meta describes it and binds it to the owner presentation's family.
fn custom_owner(
    builder: &mut PLibraryBuilder,
    spec: &mut POwnerSpec,
    payload: &CustomOwnerPayload,
) -> Result<(), PError> {
    let summary = inspect_custom_payload(&payload.bytes)?;
    for residual in summary.residual_keys() {
        let key = IntegralKey::try_new(residual).map_err(|error| {
            PError::MalformedLibrary(format!(
                "a residual of the Custom owner {:016x}: {error}",
                spec.class.0
            ))
        })?;
        spec.terminals
            .push(builder.add_terminal(PTerminal::Residual {
                owner: spec.class,
                key,
            })?);
    }
    spec.payload = Some(CustomPayloadRef::Bytes(Arc::clone(&payload.bytes)));
    spec.custom = Some(payload.meta.clone());
    Ok(())
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

impl ValidateRow {
    /// Random keys drawn short of the request (the generator gives up after
    /// `50 * points` draws): `requested - keys`, zero when the box was
    /// exhausted or the request met.
    pub fn shortfall(&self, requested: usize) -> usize {
        if self.box_exhausted {
            0
        } else {
            requested.saturating_sub(self.keys)
        }
    }
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
/// `PUncoveredPolicy::Reject` semantics. A loop count without any key is
/// refused (`PError::Unsupported`), never reported as clean.
pub fn validate_library(
    assembled: &AssembledLibrary,
    options: &ValidateOptions,
) -> Result<Vec<ValidateRow>, PError> {
    if options.points == 0 {
        return Err(PError::Unsupported(
            "p-validate: 0 keys per loop count requested".to_owned(),
        ));
    }
    let mut rows = Vec::new();
    for loops in 1..=assembled.max_loops {
        let start = Instant::now();
        let (keys, box_exhausted) = validation_keys(assembled, loops, options);
        if keys.is_empty() {
            return Err(PError::Unsupported(format!(
                "p-validate: no key of {loops} loops to trace (the scope has no top of {loops} loops)"
            )));
        }
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
