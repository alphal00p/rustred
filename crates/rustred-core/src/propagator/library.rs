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
use super::topology::Dag;
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

    /// The owner's program, built once through [`lane_program`].
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
        let class = owner.class;
        if self.owners.insert(class, POwner::new(owner)).is_some() {
            return Err(malformed(format!("two owners of class {:016x}", class.0)));
        }
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

    /// Check the program as a whole: one context `{d}`, one canonical key per
    /// class id, every census class with exactly its owner record, every
    /// reference in range.
    pub fn build(self) -> Result<PLibrary, PError> {
        if self.context.parameter_names() != ["d"] {
            return Err(PError::ContextMismatch);
        }
        let census = self.census.ok_or_else(|| malformed("no census"))?;
        census.check_class_ids()?;
        let presentations = self.presentations.len();
        let known = |id: PresentationId| (id.0 as usize) < presentations;
        for ((presentation, support), fate) in &census.sectors {
            if !known(*presentation) {
                return Err(malformed(format!(
                    "the census names presentation {}",
                    presentation.0
                )));
            }
            if let PSectorFate::Owner { class, .. } = fate
                && !census.classes.contains_key(class)
            {
                return Err(malformed(format!(
                    "sector ({}, {support:#x}) routes to the unknown class {:016x}",
                    presentation.0, class.0
                )));
            }
        }
        for (class, record) in &census.classes {
            let owner = self.owners.get(class).ok_or_else(|| {
                malformed(format!("class {} has no owner record", record.key.label()))
            })?;
            if !known(record.owner.0) || owner.spec.presentation != record.owner.0 {
                return Err(malformed(format!(
                    "class {} is owned by presentation {}, its owner record says {}",
                    record.key.label(),
                    record.owner.0.0,
                    owner.spec.presentation.0
                )));
            }
        }
        for (class, owner) in &self.owners {
            if !census.classes.contains_key(class) {
                return Err(malformed(format!(
                    "owner of class {:016x} outside the census",
                    class.0
                )));
            }
            if let Some(&edge) = owner
                .spec
                .edge_maps
                .iter()
                .find(|&&e| e >= self.edge_maps.len())
            {
                return Err(malformed(format!("owner names edge map {edge}")));
            }
            if let Some(terminal) = owner
                .spec
                .terminals
                .iter()
                .find(|terminal| terminal.0 as usize >= self.terminals.len())
            {
                return Err(malformed(format!("owner names terminal {}", terminal.0)));
            }
        }
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
}
