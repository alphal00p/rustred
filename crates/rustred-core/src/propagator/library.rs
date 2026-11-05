//! `PLibrary`, its builder and the reduction request (types and builder:
//! M2-I0 rr-iface; reduce, step-cache binding and onto-masters bodies:
//! M2-T6 rr-plib).
use std::collections::BTreeMap;
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::{Arc, OnceLock};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::{
    OrderedPhaseEvent, OrderedSchedulerOptions, OrderedStepCache, OrderedStepCacheBinding,
    OrderedWeightArithmetic, ReductionLimits, ReductionStatistics,
};
use crate::solver::CandidateOrderedReductionProgress;

use super::census::{PCensus, PSectorFate};
use super::error::PError;
use super::lane::{LaneRoles, OwnerLane};
use super::presentation::{EdgeMap, Presentation, PresentationKind, verify_edge_map};
use super::programs::{CustomPayloadRef, lane_program};
use super::step::PLaneStep;
use super::tag::GammaTag;
use super::terminal::PTerminalCodec;
use super::topology::{Dag, NodeInfo};
use super::{NodeIndex, OwnerClassId, PTerminalId, PresentationId, p_versions};

#[derive(Clone)]
pub enum OwnerProgram {
    Lane(Arc<dyn PLaneStep>),
    Missing { ledger: u32 },
}

impl fmt::Debug for OwnerProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lane(step) => write!(formatter, "Lane({:?})", step.lane()),
            Self::Missing { ledger } => write!(formatter, "Missing {{ ledger: {ledger} }}"),
        }
    }
}

/// What a Custom owner records: the LP master count, its residuals, the
/// two-sided axes and the raw charts of the payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomOwnerMeta {
    pub lp_count: u32,
    pub residuals: u32,
    pub two_sided_axes: u64,
    pub charts: Vec<u64>,
}

/// One owner: everything needed to rebuild its program.
#[derive(Clone, Debug)]
pub struct POwnerSpec {
    pub class: OwnerClassId,
    pub lane: OwnerLane,
    pub presentation: PresentationId,
    pub node: Option<NodeIndex>,
    pub roles: LaneRoles,
    /// Indices into the library's edge maps.
    pub edge_maps: Vec<usize>,
    pub payload: Option<CustomPayloadRef>,
    pub custom: Option<CustomOwnerMeta>,
    pub terminals: Vec<PTerminalId>,
}

/// An owner with its program, built on first use.
#[derive(Debug)]
pub struct POwner {
    pub spec: POwnerSpec,
    program: OnceLock<Result<OwnerProgram, PError>>,
}

impl POwner {
    fn new(spec: POwnerSpec) -> Self {
        Self {
            spec,
            program: OnceLock::new(),
        }
    }

    /// An owner whose program is `program`, for tests and fake lanes.
    fn seeded(spec: POwnerSpec, program: OwnerProgram) -> Self {
        Self {
            spec,
            program: OnceLock::from(Ok(program)),
        }
    }

    /// The owner's program: the seeded one, or built once through [`lane_program`].
    pub fn program(&self, library: &PLibrary) -> Result<&OwnerProgram, PError> {
        self.program
            .get_or_init(|| lane_program(library, &self.spec))
            .as_ref()
            .map_err(Clone::clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PTerminal {
    /// The trivial node, id 0.
    Unit,
    Residual {
        owner: OwnerClassId,
        key: IntegralKey,
    },
}

/// Maps a terminal onto the basis (M2-T9 implements, M2-T6 calls it in
/// `onto_masters`).
pub trait PBasisMap: Send + Sync {
    fn map(&self, terminal: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError>;
}

/// Caller-thread phase inspection; an error stops the request with that error.
pub trait PInspector {
    fn inspect(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<(), PError>;
}

impl<F> PInspector for F
where
    F: FnMut(&OrderedPhaseEvent<'_>) -> Result<(), PError>,
{
    fn inspect(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<(), PError> {
        self(event)
    }
}

pub struct PReductionRequest<'a> {
    pub inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
    pub arithmetic: OrderedWeightArithmetic,
    pub scheduler: OrderedSchedulerOptions,
    pub step_cache: Option<Arc<OrderedStepCache>>,
    pub uncovered: PUncoveredPolicy,
    pub onto_masters: bool,
    pub limits: ReductionLimits,
    pub inspector: Option<&'a mut dyn PInspector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PUncoveredPolicy {
    Reject,
}

/// A Masters.prc name, e.g. `no`, `t105`.
pub type PMasterName = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PBasis {
    Unit,
    Master(PMasterName),
    Residual(PTerminalId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PTerm {
    pub basis: PBasis,
    pub tag: GammaTag,
    pub coefficient: Coefficient,
}

/// Work counters of one reduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PStatistics {
    pub work: ReductionStatistics,
    pub progress: CandidateOrderedReductionProgress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PReduction {
    pub dimension: i64,
    pub terms: Vec<PTerm>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub statistics: PStatistics,
}

/// One library program: presentations, census, owners, edge maps and
/// terminals of one release scope. Non-generic: keys have the presentation's
/// own arity (plan M2 section 3.6).
pub struct PLibrary {
    context: CoefficientContext,
    scope: RangeInclusive<u8>,
    presentations: Vec<Arc<Presentation>>,
    tops: BTreeMap<String, PresentationId>,
    owners: BTreeMap<OwnerClassId, POwner>,
    census: PCensus,
    topology: Option<Arc<Dag>>,
    edge_maps: Vec<EdgeMap>,
    terminals: Vec<PTerminal>,
    basis_map: Option<Arc<dyn PBasisMap>>,
}

impl fmt::Debug for PLibrary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PLibrary")
            .field("scope", &self.scope)
            .field("presentations", &self.presentations.len())
            .field("owners", &self.owners.len())
            .field("terminals", &self.terminals.len())
            .finish_non_exhaustive()
    }
}

impl PLibrary {
    pub fn builder(context: CoefficientContext, scope: RangeInclusive<u8>) -> PLibraryBuilder {
        PLibraryBuilder {
            context,
            scope,
            presentations: Vec::new(),
            topology: None,
            census: None,
            owners: BTreeMap::new(),
            edge_maps: Vec::new(),
            terminals: vec![PTerminal::Unit],
            residuals: BTreeMap::new(),
            basis_map: None,
        }
    }

    /// The cache identity of this program: `"p-library:scope=<a>-<b>:"` plus
    /// every `name=value` of [`p_versions`]. No loop count, no digest.
    pub fn identity(&self) -> String {
        let versions: Vec<String> = p_versions()
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        format!(
            "p-library:scope={}-{}:{}",
            self.scope.start(),
            self.scope.end(),
            versions.join(",")
        )
    }

    pub fn scope(&self) -> RangeInclusive<u8> {
        self.scope.clone()
    }

    pub fn coefficient_context(&self) -> &CoefficientContext {
        &self.context
    }

    pub fn top_presentation(&self, name: &str) -> Option<PresentationId> {
        self.tops.get(name).copied()
    }

    /// The presentation `id` of this library; ids of another library panic.
    pub fn presentation(&self, id: PresentationId) -> &Arc<Presentation> {
        &self.presentations[id.0 as usize]
    }

    pub fn presentations(&self) -> &[Arc<Presentation>] {
        &self.presentations
    }

    pub fn owner(&self, class: OwnerClassId) -> Option<&POwner> {
        self.owners.get(&class)
    }

    pub fn owners(&self) -> impl ExactSizeIterator<Item = &POwner> {
        self.owners.values()
    }

    pub fn census(&self) -> &PCensus {
        &self.census
    }

    pub fn topology(&self) -> Option<&Arc<Dag>> {
        self.topology.as_ref()
    }

    pub fn edge_maps(&self) -> &[EdgeMap] {
        &self.edge_maps
    }

    pub fn terminals(&self) -> &[PTerminal] {
        &self.terminals
    }

    pub fn basis_map(&self) -> Option<&Arc<dyn PBasisMap>> {
        self.basis_map.as_ref()
    }

    pub fn reduce(&self, _request: PReductionRequest<'_>) -> Result<PReduction, PError> {
        Err(PError::NotImplemented("M2-T6"))
    }

    pub fn step_cache_binding(&self) -> Result<OrderedStepCacheBinding, PError> {
        Err(PError::NotImplemented("M2-T6"))
    }
}

/// Collects one program; `build` checks it as a whole. The terminal id 0 is
/// [`PTerminal::Unit`] from the start.
pub struct PLibraryBuilder {
    context: CoefficientContext,
    scope: RangeInclusive<u8>,
    presentations: Vec<Arc<Presentation>>,
    topology: Option<Arc<Dag>>,
    census: Option<PCensus>,
    owners: BTreeMap<OwnerClassId, POwner>,
    edge_maps: Vec<EdgeMap>,
    terminals: Vec<PTerminal>,
    residuals: BTreeMap<(OwnerClassId, IntegralKey), PTerminalId>,
    basis_map: Option<Arc<dyn PBasisMap>>,
}

fn malformed(detail: impl Into<String>) -> PError {
    PError::MalformedLibrary(detail.into())
}

/// `mask` has no bit beyond the first `slots <= 64` axes.
fn on_slots(mask: u64, slots: usize) -> bool {
    mask.checked_shr(slots as u32).is_none_or(|rest| rest == 0)
}

impl PLibraryBuilder {
    /// Add the next presentation: `p.id` must be its index, and `p` a valid
    /// presentation in the library's context and within its scope.
    pub fn add_presentation(&mut self, p: Presentation) -> Result<PresentationId, PError> {
        let index = u32::try_from(self.presentations.len())
            .map_err(|_| malformed("too many presentations"))?;
        if p.id != PresentationId(index) {
            return Err(malformed(format!(
                "presentation id {} added at index {index}",
                p.id.0
            )));
        }
        if !p
            .family
            .coefficient_context()
            .has_same_variable_map(&self.context)
        {
            return Err(PError::ContextMismatch);
        }
        if p.loops > *self.scope.end() {
            return Err(malformed(format!(
                "a {}-loop presentation in scope {:?}",
                p.loops, self.scope
            )));
        }
        p.validate()?;
        let id = p.id;
        self.presentations.push(Arc::new(p));
        Ok(id)
    }

    pub fn set_topology(&mut self, dag: Arc<Dag>) -> Result<(), PError> {
        if self.topology.replace(dag).is_some() {
            return Err(malformed("the topology is set twice"));
        }
        Ok(())
    }

    pub fn set_census(&mut self, census: PCensus) -> Result<(), PError> {
        if self.census.replace(census).is_some() {
            return Err(malformed("the census is set twice"));
        }
        Ok(())
    }

    pub fn add_owner(&mut self, owner: POwnerSpec) -> Result<(), PError> {
        self.insert_owner(POwner::new(owner))
    }

    /// [`Self::add_owner`] with `program` in place of the one `lane_program`
    /// would build: fake lanes and `OwnerProgram::Missing` of hand-built test
    /// libraries; assembly and the loader use `add_owner`.
    #[doc(hidden)]
    pub fn add_owner_with_program(
        &mut self,
        owner: POwnerSpec,
        program: OwnerProgram,
    ) -> Result<(), PError> {
        self.insert_owner(POwner::seeded(owner, program))
    }

    fn insert_owner(&mut self, owner: POwner) -> Result<(), PError> {
        let class = owner.spec.class;
        if self.owners.contains_key(&class) {
            return Err(malformed(format!("two owners of class {:016x}", class.0)));
        }
        self.owners.insert(class, owner);
        Ok(())
    }

    /// Verify `map` against its presentations, already added, and add it.
    pub fn add_edge_map(&mut self, map: EdgeMap) -> Result<usize, PError> {
        let presentation = |id: PresentationId| {
            self.presentations
                .get(id.0 as usize)
                .ok_or_else(|| malformed(format!("edge map names presentation {}", id.0)))
        };
        verify_edge_map(presentation(map.parent)?, presentation(map.child)?, &map)?;
        self.edge_maps.push(map);
        Ok(self.edge_maps.len() - 1)
    }

    /// The id of `terminal`; an equal terminal keeps its id.
    pub fn add_terminal(&mut self, terminal: PTerminal) -> Result<PTerminalId, PError> {
        let PTerminal::Residual { owner, key } = &terminal else {
            return Ok(PTerminalId(0));
        };
        let residual = (*owner, key.clone());
        if let Some(&id) = self.residuals.get(&residual) {
            return Ok(id);
        }
        let id = u32::try_from(self.terminals.len())
            .ok()
            .filter(|&id| id < PTerminalCodec::ID_LIMIT)
            .map(PTerminalId)
            .ok_or_else(|| malformed("too many terminals"))?;
        self.terminals.push(terminal);
        self.residuals.insert(residual, id);
        Ok(id)
    }

    pub fn set_basis_map(&mut self, map: Arc<dyn PBasisMap>) -> Result<(), PError> {
        if self.basis_map.replace(map).is_some() {
            return Err(malformed("the basis map is set twice"));
        }
        Ok(())
    }

    /// Check the program as a whole: one context `{d}`, a non-empty scope of
    /// `L >= 1`, one canonical key per class id, every census class with exactly
    /// its owner record, and every reference resolved:
    /// - presentations: `Induced` names a top and a mask on its slots; a
    ///   `NodeRef` names a topology node with its graph key;
    /// - census: supports lie on the slots and contain the forced axes, routes
    ///   go to a known class with the arities of both presentations, class
    ///   nodes exist in the topology;
    /// - owners: the node exists, edge maps exist and leave the owner's
    ///   presentation, terminals exist;
    /// - residual terminals name a known class, with a key of the arity of its
    ///   owner presentation.
    ///
    /// Lane roles are not read here: the steps check them (`check_*_contract`).
    pub fn build(mut self) -> Result<PLibrary, PError> {
        if self.context.parameter_names() != ["d"] {
            return Err(PError::ContextMismatch);
        }
        if self.scope.is_empty() || *self.scope.start() == 0 {
            return Err(malformed(format!(
                "scope {:?}, a non-empty scope of L >= 1 is required",
                self.scope
            )));
        }
        let census = self.census.take().ok_or_else(|| malformed("no census"))?;
        census.check_class_ids()?;
        self.check_presentations()?;
        self.check_classes(&census)?;
        self.check_sectors(&census)?;
        self.check_owners(&census)?;
        self.check_terminals(&census)?;
        let mut tops = BTreeMap::new();
        for presentation in &self.presentations {
            if let PresentationKind::Top { name } = &presentation.kind
                && tops.insert(name.clone(), presentation.id).is_some()
            {
                return Err(malformed(format!("two top presentations named {name}")));
            }
        }
        Ok(PLibrary {
            context: self.context,
            scope: self.scope,
            presentations: self.presentations,
            tops,
            owners: self.owners,
            census,
            topology: self.topology,
            edge_maps: self.edge_maps,
            terminals: self.terminals,
            basis_map: self.basis_map,
        })
    }

    fn presentation(&self, id: PresentationId) -> Result<&Presentation, PError> {
        self.presentations
            .get(id.0 as usize)
            .map(Arc::as_ref)
            .ok_or_else(|| malformed(format!("unknown presentation {}", id.0)))
    }

    fn node(&self, index: NodeIndex) -> Result<&NodeInfo, PError> {
        let dag = self
            .topology
            .as_ref()
            .ok_or_else(|| malformed(format!("node {} without a topology", index.0)))?;
        dag.nodes
            .get(index.0 as usize)
            .ok_or_else(|| malformed(format!("node {} is not in the topology", index.0)))
    }

    /// `support` of presentation `id`, whose forced axes it must contain.
    fn sector_presentation(
        &self,
        id: PresentationId,
        support: u64,
    ) -> Result<&Presentation, PError> {
        let presentation = self.presentation(id)?;
        let forced = presentation.forced_axes();
        if !on_slots(support, presentation.slots.len()) || support & forced != forced {
            return Err(malformed(format!(
                "support {support:#x} of presentation {}, forced axes {forced:#x}",
                id.0
            )));
        }
        Ok(presentation)
    }

    fn check_presentations(&self) -> Result<(), PError> {
        for presentation in &self.presentations {
            if let Some(node) = &presentation.node
                && self.node(node.index)?.key.0 != node.graph_key
            {
                return Err(malformed(format!(
                    "presentation {} has another graph key than node {}",
                    presentation.id.0, node.index.0
                )));
            }
            if let PresentationKind::Induced { top, mask } = presentation.kind {
                let source = self.presentation(top)?;
                if !matches!(source.kind, PresentationKind::Top { .. })
                    || !on_slots(mask, source.slots.len())
                {
                    return Err(malformed(format!(
                        "presentation {} is induced by mask {mask:#x} of presentation {}",
                        presentation.id.0, top.0
                    )));
                }
            }
        }
        Ok(())
    }

    fn check_classes(&self, census: &PCensus) -> Result<(), PError> {
        for (class, record) in &census.classes {
            let label = record.key.label();
            let owner = self
                .owners
                .get(class)
                .ok_or_else(|| malformed(format!("class {label} has no owner record")))?;
            let (presentation, support) = record.owner;
            if owner.spec.presentation != presentation {
                return Err(malformed(format!(
                    "class {label} is owned by presentation {}, its owner record says {}",
                    presentation.0, owner.spec.presentation.0
                )));
            }
            self.sector_presentation(presentation, support)?;
            for &node in record.node.iter().chain(&record.nodes) {
                self.node(node)?;
            }
        }
        Ok(())
    }

    /// After [`Self::check_classes`]: class owners exist.
    fn check_sectors(&self, census: &PCensus) -> Result<(), PError> {
        for (&(id, support), fate) in &census.sectors {
            let source = self.sector_presentation(id, support)?;
            let PSectorFate::Owner { class, map } = fate else {
                continue;
            };
            let record = census.classes.get(class).ok_or_else(|| {
                malformed(format!(
                    "sector ({}, {support:#x}) routes to the unknown class {:016x}",
                    id.0, class.0
                ))
            })?;
            let target = self.presentation(record.owner.0)?;
            if (map.loops_in, map.loops_out) != (source.loops, target.loops) {
                return Err(malformed(format!(
                    "sector ({}, {support:#x}) routes {} -> {} loops, not {} -> {}",
                    id.0, map.loops_in, map.loops_out, source.loops, target.loops
                )));
            }
        }
        Ok(())
    }

    fn check_owners(&self, census: &PCensus) -> Result<(), PError> {
        for (class, owner) in &self.owners {
            let spec = &owner.spec;
            if !census.classes.contains_key(class) {
                return Err(malformed(format!(
                    "owner of class {:016x} outside the census",
                    class.0
                )));
            }
            if let Some(node) = spec.node {
                self.node(node)?;
            }
            for &edge in &spec.edge_maps {
                let map = self
                    .edge_maps
                    .get(edge)
                    .ok_or_else(|| malformed(format!("owner names edge map {edge}")))?;
                if map.parent != spec.presentation {
                    return Err(malformed(format!(
                        "an owner on presentation {} names edge map {edge} of presentation {}",
                        spec.presentation.0, map.parent.0
                    )));
                }
            }
            if let Some(terminal) = spec
                .terminals
                .iter()
                .find(|terminal| terminal.0 as usize >= self.terminals.len())
            {
                return Err(malformed(format!("owner names terminal {}", terminal.0)));
            }
        }
        Ok(())
    }

    /// After [`Self::check_classes`]: class owners exist.
    fn check_terminals(&self, census: &PCensus) -> Result<(), PError> {
        for terminal in &self.terminals {
            let PTerminal::Residual { owner, key } = terminal else {
                continue;
            };
            let record = census.classes.get(owner).ok_or_else(|| {
                malformed(format!(
                    "a residual terminal of the unknown class {:016x}",
                    owner.0
                ))
            })?;
            let arity = self.presentation(record.owner.0)?.slots.len();
            if key.powers().len() != arity {
                return Err(malformed(format!(
                    "a residual key of class {} has {} slots, not {arity}",
                    record.key.label(),
                    key.powers().len()
                )));
            }
        }
        Ok(())
    }
}
