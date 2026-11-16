//! `PLibrary`, its builder and the reduction request (types and builder:
//! M2-I0 rr-iface; reduce, step-cache binding and onto-masters bodies:
//! M2-T6 rr-plib).
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::{
    OrderedPhaseEvent, OrderedSchedulerOptions, OrderedStepCache, OrderedStepCacheAdapter,
    OrderedStepCacheBinding, OrderedWeightArithmetic, ReductionError, ReductionLimits,
    ReductionStatistics,
};
use crate::solver::{CandidateOrderedReductionProgress, UnresolvedPolicy};

use super::census::{PCensus, PSectorFate};
use super::custom::PRuleOrder;
use super::engine::{PEngine, merge};
use super::error::{PError, PRequestContext, PRequestInput};
use super::lane::{LaneRoles, OwnerLane};
use super::payload_cache::PProgramBudget;
use super::presentation::{EdgeMap, Presentation, PresentationKind, verify_edge_map};
use super::programs::{CustomPayloadRef, lane_program};
use super::saved_orbits::PSavedOrbits;
use super::saved_routes::PSavedRoutes;
use super::step::PLaneStep;
use super::tag::GammaTag;
use super::terminal::PTerminalCodec;
use super::topology::{Dag, NodeInfo};
use super::{
    NodeIndex, OwnerClassId, P_ORBIT_CANON_DEFAULT, P_ORBIT_CANON_VERSION,
    P_ORBIT_CANON_WITH_SAVED_ORBITS, PTerminalId, PresentationId, p_versions,
};

/// Known-zero child pruning at emission (Astra AR5; ON by default since M5
/// batch 4, `RUSTRED_P_PRUNE_ZERO_CHILDREN=0`/`off`/`false` disables): a
/// routed child whose sector the census proves zero is dropped where the
/// step emits it instead of being merged into the frontier and routed to
/// nothing. One process-wide snapshot, shared by the cache identity and every
/// request: changing the environment after first use cannot change step
/// semantics under an already-created cache binding.
pub(super) fn prune_zero_children_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        !matches!(
            std::env::var("RUSTRED_P_PRUNE_ZERO_CHILDREN").as_deref(),
            Ok("0" | "off" | "false")
        )
    })
}

/// Whether a request prints its pruning counters on stderr
/// (`rustred.p-zero-pruning.v1`): only when the switch is set explicitly.
pub(super) fn prune_zero_children_reported() -> bool {
    std::env::var("RUSTRED_P_PRUNE_ZERO_CHILDREN").is_ok_and(|value| value == "1")
}

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

    /// The program if it is already built (never builds it).
    pub fn loaded_program(&self) -> Option<&OwnerProgram> {
        self.program.get().and_then(|program| program.as_ref().ok())
    }

    /// The owner's program: the seeded one, or built once through [`lane_program`].
    pub fn program(&self, library: &PLibrary) -> Result<&OwnerProgram, PError> {
        self.program
            .get_or_init(|| {
                // b7-schemeload: RUSTRED_P_PROGRAM_TIMING=1 prints every
                // program build (lane, owner, seconds) on stderr.
                static TIMING: OnceLock<bool> = OnceLock::new();
                if !*TIMING.get_or_init(|| {
                    std::env::var("RUSTRED_P_PROGRAM_TIMING").is_ok_and(|v| v == "1")
                }) {
                    return lane_program(library, &self.spec);
                }
                let started = std::time::Instant::now();
                let program = lane_program(library, &self.spec);
                eprintln!(
                    "p program {:?} owner {:016x}: {:.3} s",
                    self.spec.lane,
                    self.spec.class.0,
                    started.elapsed().as_secs_f64()
                );
                program
            })
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
    /// The opt-in path census of the request (M5 R0 WP0,
    /// [`PPathCensus`](super::path_census::PPathCensus)); `None`, the
    /// default, keeps it off. Asked once, when the request starts: a request
    /// whose inspector returns `None` then records nothing.
    fn path_census(&mut self) -> Option<&mut super::path_census::PPathCensus> {
        None
    }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PUncoveredPolicy {
    /// A key no rule covers is the request's typed error (the default).
    #[default]
    Reject,
    /// vk5l WP1 (plan VK5L §3.2, decision Q9), the vacuum path's Retain
    /// mirror: a key the library cannot reduce ([`PError::Uncovered`],
    /// [`PError::OutsideBox`], [`PError::OutsideCertifiedSet`],
    /// [`PError::NodeNotClosed`]) stays in the result as a residual terminal
    /// ([`PBasis::Residual`]; [`PLibrary::residual_terminal`] names it and
    /// gives the refusal in [`PResidualTerminal::retained`]). Never a master,
    /// never a value. A Retain request uses no step cache (a retained
    /// terminal's id is local to the process) and skips the early box check.
    Retain,
}

/// A Masters.prc name, e.g. `no`, `t105`.
pub type PMasterName = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PBasis {
    Unit,
    Master(PMasterName),
    Residual(PTerminalId),
}

/// [`PLibrary::residual_terminal`]: a residual terminal as a consumer names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PResidualTerminal {
    /// The boxed node (`T162`) or the owner presentation's family name.
    pub node: String,
    /// The key in the owner presentation's slots.
    pub key: IntegralKey,
    /// A terminal of a BOXED node: an unknown integral, never evaluated.
    pub boxed: bool,
    /// vk5l WP1: a key kept by [`PUncoveredPolicy::Retain`]: the refusal
    /// it would have been (`None` for a terminal of the library).
    pub retained: Option<PError>,
}

/// vk5l WP1: a node the package declares not closed (no table, e.g. T266):
/// a key reaching its owner is [`PError::NodeNotClosed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct POpenNode {
    pub node: String,
    pub status: String,
}

/// vk5l WP1: the keys retained by Retain requests of one library, ids after
/// the library's terminals (process-local, never persisted).
#[derive(Debug, Default)]
struct PRetained {
    ids: BTreeMap<(OwnerClassId, IntegralKey), PTerminalId>,
    records: Vec<(OwnerClassId, IntegralKey, PError)>,
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
    /// vk5l WP2: the keys of boxed nodes OUTSIDE their certified sets that
    /// this request reduced at run time (rules, then recipe families at the
    /// key); empty when every key stayed inside. Sorted.
    pub outside_certified: Vec<POutsideKey>,
    /// vk5l WP2: the relation caches during this request (process-wide
    /// counters before / after: approximate when requests run concurrently).
    pub relation_cache: super::relation_entries::RelationCacheStatistics,
}

/// vk5l WP2: one key of a boxed node outside its certified set, reduced at
/// run time.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct POutsideKey {
    pub node: String,
    pub presentation: PresentationId,
    pub powers: Vec<i64>,
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
    /// p artefact v2: explicit rule orders of Custom owners and the ordering
    /// tag (`<P_ORDERING_VERSION>:<heuristic>`) that joins the identity.
    rule_orders: BTreeMap<OwnerClassId, Arc<PRuleOrder>>,
    ordering: Option<String>,
    /// p5l-library-6: the package content revision (a plain name), the last
    /// part of the identity (`,P_PACKAGE_REVISION=<name>`).
    revision: Option<String>,
    /// p artefact v2: saved route verifications, restored on a sector's first
    /// use instead of verifying its map ([`super::saved_routes`]).
    saved_routes: Option<Arc<PSavedRoutes>>,
    /// p artefact v2 (M5 batch 2): saved verified automorphism groups,
    /// restored on a sector's first canonicalised key instead of searching
    /// and authenticating them ([`super::saved_orbits`]).
    saved_orbits: Option<Arc<PSavedOrbits>>,
    /// p5l-library session 2: the memory budget of the programs of Custom
    /// owners whose payload is not held in memory ([`super::payload_cache`]).
    program_budget: Option<Arc<PProgramBudget>>,
    /// vk5l WP1: nodes declared not closed, by owner class.
    open_nodes: BTreeMap<OwnerClassId, POpenNode>,
    /// vk5l WP1: keys retained by Retain requests.
    retained: std::sync::Mutex<PRetained>,
    /// vk5l WP1: where the library was loaded from (a package directory),
    /// for the reproduction line of a typed refusal.
    source: OnceLock<String>,
    engine: OnceLock<Result<PEngine, PError>>,
    /// Orbit canonicalisation of Apply keys (M5 R0 A2); off by default
    /// (M5 batch 2: opt-in until the coordinator switches it on).
    orbit_canonicalization: AtomicBool,
    /// With it, the Route keys of routed sectors too; on by default.
    route_canonicalization: AtomicBool,
    /// The greatest exact image represents an orbit (the default), or the least.
    orbit_prefer_high: AtomicBool,
    /// b7-schemeload: the Scheme owners' stored certificates are trusted at
    /// program build (set by the package loader only, see
    /// [`super::scheme::package_trusts_scheme_certificates`]); off by default.
    scheme_certificates_trusted: AtomicBool,
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
            rule_orders: BTreeMap::new(),
            ordering: None,
            revision: None,
            saved_routes: None,
            saved_orbits: None,
            open_nodes: BTreeMap::new(),
        }
    }

    /// The identity of this program: `"p-library:scope=<a>-<b>:"` plus every
    /// `name=value` of [`p_versions`], then `,P_FAMILIES_5L_VERSION=<v>` when
    /// the scope reaches 5 loops (M5 batch 3) and the ordering tag. No loop
    /// count, no digest. It names the
    /// library content (`library.json` records it); reduction settings such as
    /// orbit canonicalisation are not part of it, see [`Self::step_identity`].
    pub fn identity(&self) -> String {
        let versions: Vec<String> = p_versions()
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        let ordering = self
            .ordering
            .as_ref()
            .map_or_else(String::new, |tag| format!(",P_ORDERING_VERSION={tag}"));
        // G9 (M5 batch 3): the 5L families enter only the identity of a
        // library that reaches 5 loops; scope 1-4 identities are unchanged.
        let families_5l = if *self.scope.end() >= 5 {
            format!(",P_FAMILIES_5L_VERSION={}", super::P_FAMILIES_5L_VERSION)
        } else {
            String::new()
        };
        // p5l-library-6: the content revision of the package, last.
        let revision = self.revision.as_ref().map_or_else(String::new, |name| {
            format!(",{P_PACKAGE_REVISION_KEY}={name}")
        });
        format!(
            "p-library:scope={}-{}:{}{families_5l}{ordering}{revision}",
            self.scope.start(),
            self.scope.end(),
            versions.join(",")
        )
    }

    /// The explicit rule order of the Custom owner `class` (p artefact v2),
    /// if the library has an ordering.
    pub fn rule_order(&self, class: OwnerClassId) -> Option<&PRuleOrder> {
        self.rule_orders.get(&class).map(Arc::as_ref)
    }

    /// [`Self::rule_order`], shared.
    pub fn rule_order_arc(&self, class: OwnerClassId) -> Option<Arc<PRuleOrder>> {
        self.rule_orders.get(&class).cloned()
    }

    /// The budget of the programs of Custom owners whose payload is read on
    /// use (`RUSTRED_P_PROGRAM_BUDGET_GB`, [`super::payload_cache`]).
    pub fn program_budget(&self) -> Option<&Arc<PProgramBudget>> {
        self.program_budget.as_ref()
    }

    /// The ordering tag `<P_ORDERING_VERSION>:<heuristic>` of the library's
    /// ordering (p artefact v2), part of its identity.
    pub fn ordering(&self) -> Option<&str> {
        self.ordering.as_deref()
    }

    /// p5l-library-6: the package content revision, part of the identity.
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    /// The saved route verifications of this library (p artefact v2).
    pub fn saved_routes(&self) -> Option<&PSavedRoutes> {
        self.saved_routes.as_deref()
    }

    /// The saved automorphism groups of this library (p artefact v2).
    pub fn saved_orbits(&self) -> Option<&PSavedOrbits> {
        self.saved_orbits.as_deref()
    }

    /// The identity the step cache binds to: [`Self::identity`], plus
    /// `P_ORBIT_CANON_VERSION` while orbit canonicalisation is on (it changes
    /// the recorded steps; `-apply` when it stops at Apply keys), plus
    /// `P_ADJOINT_TAIL=off` while p requests run without the adjoint tail (the
    /// p profile, M5 batch 2: a cache they write has no tail table, so it is a
    /// distinct identity that a process whose p requests run the tail,
    /// `RUSTRED_ORDERED_ADJOINT_TAIL=on`, never reads), plus
    /// `P_PRUNE_ZERO_CHILDREN=off` while known-zero children are NOT pruned
    /// (pruning is the default since M5 batch 4; a cache written without it
    /// is a distinct identity). The pruning setting is captured once per
    /// process, shared with requests.
    pub fn step_identity(&self) -> String {
        let mut identity = self.identity();
        if self.orbit_canonicalization() {
            let scope = match (self.route_canonicalization(), self.orbit_prefer_high()) {
                (true, true) => "",
                (false, true) => "-apply",
                (true, false) => "-least",
                (false, false) => "-apply-least",
            };
            identity.push_str(&format!(
                ",P_ORBIT_CANON_VERSION={P_ORBIT_CANON_VERSION}{scope}"
            ));
        }
        if !crate::reduction::ordered_reduction::p_requests_run_the_adjoint_tail() {
            identity.push_str(",P_ADJOINT_TAIL=off");
        }
        if !prune_zero_children_enabled() {
            identity.push_str(",P_PRUNE_ZERO_CHILDREN=off");
        }
        // vk5l WP2: recipe steps (boxed nodes, 5L only) are steps of their
        // Custom owners; their semantics version joins the cache binding of a
        // library that reaches 5 loops (scope 1-4 bindings unchanged).
        if *self.scope.end() >= 5 {
            identity.push_str(&format!(
                ",P_RECIPE_VERSION={}",
                super::recipes::P_RECIPE_VERSION
            ));
        }
        identity
    }

    /// Switch the orbit canonicalisation of Apply keys (M5 R0 A2) on or off
    /// for the requests that start after this call; the default is
    /// [`super::P_ORBIT_CANON_WITH_SAVED_ORBITS`] for a library with saved
    /// groups (`orbits.rrbin`), else [`super::P_ORBIT_CANON_DEFAULT`].
    /// Canonicalisation replaces an Apply key by the least of its exact images
    /// under the verified automorphisms of its owner sector (one key times
    /// `+-1`, module [`super::orbit`]), so symmetric copies merge. The values
    /// do not depend on it: each replacement is an exact identity and the
    /// decomposition onto the fixed terminals is unique. The setting changes
    /// the recorded steps, so it is part of [`Self::step_identity`] and of the step
    /// cache binding: set it before the first request that uses a cache.
    pub fn set_orbit_canonicalization(&self, on: bool) {
        self.orbit_canonicalization.store(on, Ordering::Relaxed);
    }

    /// Whether requests canonicalise Apply keys under owner automorphisms.
    pub fn orbit_canonicalization(&self) -> bool {
        self.orbit_canonicalization.load(Ordering::Relaxed)
    }

    /// b7-schemeload: trust the stored certificates of the embedded Scheme
    /// owners at program build instead of re-verifying them (the trust rule is
    /// [`super::scheme::certificate_check`]). The package loader sets it for a
    /// package whose identity carries this binary's
    /// `P_SCHEME_TEMPLATE_VERSION` and a `P_PACKAGE_REVISION`; no value
    /// depends on it (the verdicts are the verified ones, gate test).
    pub fn set_scheme_certificates_trusted(&self, on: bool) {
        self.scheme_certificates_trusted
            .store(on, Ordering::Relaxed);
    }

    /// Whether [`Self::set_scheme_certificates_trusted`] is on.
    pub fn scheme_certificates_trusted(&self) -> bool {
        self.scheme_certificates_trusted.load(Ordering::Relaxed)
    }

    /// With orbit canonicalisation on, canonicalise the Route keys of routed
    /// sectors as well (under the owner's automorphisms carried over by the
    /// route); on by default. Off, only Apply keys are canonicalised.
    pub fn set_route_canonicalization(&self, on: bool) {
        self.route_canonicalization.store(on, Ordering::Relaxed);
    }

    /// Represent an orbit by its greatest exact image (the default) or by its
    /// least (M5 R0 A2). The lanes are not covariant under the automorphisms,
    /// so the representative changes the reduction path: the greatest image
    /// measured fewer keys and edges than the least on every 4L cell (NO2
    /// d2_r6: -17 % keys against -6 %). For a Custom or Triangle owner the
    /// order key still decides first.
    pub fn set_orbit_prefer_high(&self, on: bool) {
        self.orbit_prefer_high.store(on, Ordering::Relaxed);
    }

    /// Whether the greatest exact image represents an orbit.
    pub fn orbit_prefer_high(&self) -> bool {
        self.orbit_prefer_high.load(Ordering::Relaxed)
    }

    /// Whether canonicalisation, when on, reaches the Route keys.
    pub fn route_canonicalization(&self) -> bool {
        self.route_canonicalization.load(Ordering::Relaxed)
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

    /// t162-export: what a consumer prints for the residual terminal `id`
    /// (`None` for `Unit` or an unknown id): the node (the box record's node
    /// of a BOXED owner, else the family name of the owner's presentation),
    /// the key in that presentation's slots, and whether the owner is boxed
    /// (its terminal is an "unknown integral", not a master). Reads only the
    /// provenance prefix of a Custom owner's payload.
    pub fn residual_terminal(&self, id: PTerminalId) -> Result<Option<PResidualTerminal>, PError> {
        let Some(PTerminal::Residual { owner, key }) = self.terminals.get(id.0 as usize) else {
            return self.retained_terminal(id);
        };
        let owner_record = self
            .owners
            .get(owner)
            .ok_or_else(|| malformed(format!("residual {} of an unknown owner", id.0)))?;
        let spec = &owner_record.spec;
        // A built program knows its box; else read the payload's prefix (a
        // lazy payload is read once more: name terminals after a reduction).
        let boxed = match (owner_record.program.get(), &spec.payload) {
            (Some(Ok(OwnerProgram::Lane(step))), _) => {
                step.boxed_record().map(|record| record.node.clone())
            }
            (_, Some(payload)) if spec.lane == OwnerLane::Custom => {
                super::custom::custom_payload_boxed_node(&payload.bytes()?)?
                    .map(|record| record.node)
            }
            _ => None,
        };
        Ok(Some(PResidualTerminal {
            node: boxed.clone().unwrap_or_else(|| {
                self.presentation(spec.presentation)
                    .family
                    .name()
                    .to_owned()
            }),
            key: key.clone(),
            boxed: boxed.is_some(),
            retained: None,
        }))
    }

    /// vk5l WP1: [`Self::residual_terminal`] of a key retained by a Retain
    /// request (`id` after the library's terminals).
    fn retained_terminal(&self, id: PTerminalId) -> Result<Option<PResidualTerminal>, PError> {
        let Some(index) = (id.0 as usize).checked_sub(self.terminals.len()) else {
            return Ok(None);
        };
        let retained = self.retained.lock().unwrap_or_else(|e| e.into_inner());
        let Some((class, key, refusal)) = retained.records.get(index) else {
            return Ok(None);
        };
        let node = match refusal {
            PError::OutsideBox { node, .. }
            | PError::OutsideCertifiedSet { node, .. }
            | PError::NodeNotClosed { node, .. } => node.clone(),
            _ => match self.owners.get(class) {
                Some(owner) => self
                    .presentation(owner.spec.presentation)
                    .family
                    .name()
                    .to_owned(),
                None => format!("{:016x}", class.0),
            },
        };
        Ok(Some(PResidualTerminal {
            node,
            key: key.clone(),
            boxed: matches!(
                refusal,
                PError::OutsideBox { .. } | PError::OutsideCertifiedSet { .. }
            ),
            retained: Some(refusal.clone()),
        }))
    }

    /// vk5l WP1: the id of a retained key (one id per (owner, key)).
    pub(crate) fn retain(
        &self,
        class: OwnerClassId,
        key: IntegralKey,
        refusal: PError,
    ) -> Result<PTerminalId, PError> {
        let mut retained = self.retained.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&id) = retained.ids.get(&(class, key.clone())) {
            return Ok(id);
        }
        let id = u32::try_from(self.terminals.len() + retained.records.len())
            .ok()
            .filter(|&id| id < PTerminalCodec::ID_LIMIT)
            .map(PTerminalId)
            .ok_or_else(|| malformed("too many retained keys"))?;
        retained.ids.insert((class, key.clone()), id);
        retained.records.push((class, key, refusal));
        Ok(id)
    }

    /// vk5l WP1: the first terminal id without a terminal: the library's
    /// terminals, then the keys retained so far.
    pub(crate) fn terminal_limit(&self) -> usize {
        self.terminals.len()
            + self
                .retained
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .records
                .len()
    }

    /// vk5l WP1: the node `class` declares not closed.
    pub fn open_node(&self, class: OwnerClassId) -> Option<&POpenNode> {
        self.open_nodes.get(&class)
    }

    /// vk5l WP1: the nodes declared not closed.
    pub fn open_nodes(&self) -> &BTreeMap<OwnerClassId, POpenNode> {
        &self.open_nodes
    }

    /// vk5l WP1: record where the library was loaded from (the package
    /// directory of the reproduction line); the first call wins.
    pub fn set_source(&self, source: impl Into<String>) {
        let _ = self.source.set(source.into());
    }

    /// vk5l WP1: the original request of a typed refusal and its
    /// reproduction command line (`p5l_library reduce`).
    pub fn request_context(
        &self,
        inputs: &[(PresentationId, IntegralKey, Coefficient)],
    ) -> PRequestContext {
        let inputs: Vec<PRequestInput> = inputs
            .iter()
            .map(|(id, key, _)| PRequestInput {
                presentation: *id,
                family: self
                    .presentations
                    .get(id.0 as usize)
                    .map_or_else(|| "?".to_owned(), |p| p.family.name().to_owned()),
                powers: key.powers().to_vec(),
            })
            .collect();
        let package = self.source.get().map_or("<package dir>", String::as_str);
        let keys: Vec<String> = inputs
            .iter()
            .map(|input| {
                let powers: Vec<String> = input.powers.iter().map(i64::to_string).collect();
                format!("p{}:{}", input.presentation.0, powers.join(","))
            })
            .collect();
        PRequestContext {
            library: self.identity(),
            reproduce: format!(
                "cargo run --release -p rustred-app --example p5l_library -- reduce '{package}' {}",
                keys.join(" ")
            ),
            inputs,
        }
    }

    /// vk5l WP2: a recipe failure with one rule chain from a request input
    /// to the failing key, searched inside the boxed owner (its rules and the
    /// recipe relations solved so far, no new solve, at most
    /// [`RECIPE_CHAIN_BUDGET`] keys): only from inputs that lie on that owner
    /// directly; otherwise the failure says why no chain is given.
    fn with_recipe_chain(&self, engine: &PEngine, error: PError) -> PError {
        let PError::RecipeFailed(mut failure) = error else {
            return error;
        };
        if failure.rule_chain.is_some() {
            return PError::RecipeFailed(failure);
        }
        let Some(request) = failure.request.clone() else {
            return PError::RecipeFailed(failure);
        };
        let mut direct = 0;
        for input in &request.inputs {
            let Some(class) = engine.direct_owner(input.presentation, &input.powers) else {
                continue;
            };
            let Some(owner) = self.owners.get(&class) else {
                continue;
            };
            let Ok(OwnerProgram::Lane(step)) = owner.program(self) else {
                continue;
            };
            if step
                .boxed_record()
                .is_none_or(|record| record.node != failure.node)
            {
                continue;
            }
            direct += 1;
            if let Some(chain) = step.recipe_chain(&input.powers, &failure.key, RECIPE_CHAIN_BUDGET)
            {
                failure.rule_chain = Some(chain);
                return PError::RecipeFailed(failure);
            }
        }
        failure.rule_chain_note = Some(if direct == 0 {
            format!(
                "not reconstructed: no request input lies directly on node {} (it is reached through routes \
                 from other sectors)",
                failure.node
            )
        } else {
            format!(
                "not found within {RECIPE_CHAIN_BUDGET} keys from the {direct} request input(s) on node {}",
                failure.node
            )
        });
        PError::RecipeFailed(failure)
    }

    /// vk5l WP1 (plan VK5L §3.4, decision Q7): the early check of a request
    /// against the box of a boxed node: an input on a sector a boxed Custom
    /// owner owns directly must lie in its box, else [`PError::OutsideBox`]
    /// naming node, key and the violated bound (before any step). Inputs on
    /// routed sectors are left to the dispatch check (the authoritative one).
    fn check_boxed_inputs(
        &self,
        engine: &PEngine,
        inputs: &[(PresentationId, IntegralKey, Coefficient)],
    ) -> Result<(), PError> {
        for (presentation, key, _) in inputs {
            let Some(class) = engine.direct_owner(*presentation, key.powers()) else {
                continue;
            };
            let Some(owner) = self.owners.get(&class) else {
                continue;
            };
            if owner.spec.lane != OwnerLane::Custom || self.open_nodes.contains_key(&class) {
                continue;
            }
            let Ok(OwnerProgram::Lane(step)) = owner.program(self) else {
                continue;
            };
            let Some(record) = step.boxed_record() else {
                continue;
            };
            // vk5l WP2 (user ruling 2026-11-14): under the Solve policy a key
            // outside the box is reduced at run time, not refused early.
            if super::recipes::outside_policy_of(&record.node)
                == super::recipes::POutsidePolicy::Solve
            {
                continue;
            }
            if let Some(violated) = record.violation(key.powers()) {
                return Err(PError::OutsideBox {
                    class,
                    key: key.clone(),
                    node: record.node.clone(),
                    definition: record.describe(),
                    violated: Some(Box::new(violated)),
                    request: None,
                });
            }
        }
        Ok(())
    }

    /// Reduce one weighted sum of inputs of one loop count onto the terminals
    /// (`onto_masters = false`), or onto the masters in Forcer's G-scheme: the
    /// sum times `(eps G(1,1))^-L` with each terminal mapped through the
    /// library's [`PBasisMap`] (without one, `Unit` stays and a residual stays
    /// [`PBasis::Residual`]). Terms are sorted by basis and tag;
    /// [`PReduction::dimension`] is the loop count of the request. Every thread
    /// count and arithmetic returns the same terms and conditions.
    #[allow(clippy::mutable_key_type)] // PKey views cache their IntegralKey lazily; Hash/Ord read only the packed powers (rr-alloc)
    pub fn reduce(&self, request: PReductionRequest<'_>) -> Result<PReduction, PError> {
        let PReductionRequest {
            inputs,
            arithmetic,
            scheduler,
            step_cache,
            uncovered,
            onto_masters,
            limits,
            inspector,
        } = request;
        let retain = uncovered == PUncoveredPolicy::Retain;
        // vk5l WP1: a retained terminal's id is local to this process.
        let step_cache = if retain { None } else { step_cache };
        let loops = self.request_loops(&inputs)?;
        // vk5l WP1: the original request, recorded on a typed refusal.
        let context = Arc::new(self.request_context(&inputs));
        let with_request = |error: PError| error.with_request(&context);
        if let Some(cache) = &step_cache {
            let binding = self.step_cache_binding()?;
            cache.check_binding(
                binding.adapter,
                &binding.family_fingerprint,
                &self.context,
                binding.unresolved_policy,
            )?;
        }
        let engine = self
            .engine
            .get_or_init(|| PEngine::new(self))
            .as_ref()
            .map_err(Clone::clone)?;
        if !retain {
            self.check_boxed_inputs(engine, &inputs)
                .map_err(with_request)?;
        }
        let initial = engine.initial(self, inputs, &limits)?;
        let cache_before = super::relation_entries::relation_cache_statistics();
        let mut outside = BTreeSet::new();
        let output = engine
            .run_reporting(
                self,
                initial,
                arithmetic,
                scheduler,
                step_cache.as_deref(),
                limits,
                inspector,
                retain,
                &mut outside,
            )
            .map_err(with_request)
            .map_err(|error| self.with_recipe_chain(engine, error))?;
        let mut terms = PTermSum::default();
        let raw = output
            .terms
            .iter()
            .map(|(key, coefficient)| {
                let (terminal, tag) = engine.decode_terminal(key, self.terminal_limit())?;
                Ok((terminal, tag, coefficient.clone()))
            })
            .collect::<Result<Vec<_>, PError>>()?;
        if onto_masters {
            self.onto_masters(loops, raw, &limits, &mut terms)?;
        } else {
            for (terminal, tag, coefficient) in raw {
                terms.add(self, Self::plain_basis(terminal), tag, coefficient, &limits)?;
            }
        }
        Ok(PReduction {
            dimension: i64::from(loops),
            terms: terms.into_terms(),
            nonzero_conditions: output.nonzero_conditions,
            statistics: PStatistics {
                work: output.work,
                progress: output.progress,
            },
            outside_certified: outside.into_iter().collect(),
            relation_cache: super::relation_entries::relation_cache_statistics()
                .since(&cache_before),
        })
    }

    /// The one loop count of a request; `0` for no input.
    fn request_loops(
        &self,
        inputs: &[(PresentationId, IntegralKey, Coefficient)],
    ) -> Result<u8, PError> {
        let mut loops = None;
        for (id, ..) in inputs {
            let presentation = self.presentations.get(id.0 as usize).ok_or_else(|| {
                PError::Unsupported(format!(
                    "input on presentation {}, which this library does not have",
                    id.0
                ))
            })?;
            if *loops.get_or_insert(presentation.loops) != presentation.loops {
                return Err(PError::MixedDimension);
            }
        }
        Ok(loops.unwrap_or(0))
    }

    fn plain_basis(terminal: PTerminalId) -> PBasis {
        if terminal == PTerminalId(0) {
            PBasis::Unit
        } else {
            PBasis::Residual(terminal)
        }
    }

    /// Plan convention C3: `(eps G(1,1))^-1` is the tag
    /// `{(+1,-1),(-1,-2),(-2,+1)}` times `(d-3)`. Tags are absolute on both sides
    /// of [`PBasisMap::map`]: it receives the tag of the term, scheme included,
    /// and each tag it returns is taken as it stands, so a map adds its own
    /// factors to the tag it was given. Every terminal goes through the map,
    /// `Unit` (terminal 0) included. A row without terms is refused, because a
    /// terminal that drops out unnoticed is a silent zero; a terminal that
    /// really vanishes returns a term with a zero coefficient.
    fn onto_masters(
        &self,
        loops: u8,
        raw: Vec<(PTerminalId, GammaTag, Coefficient)>,
        limits: &ReductionLimits,
        terms: &mut PTermSum,
    ) -> Result<(), PError> {
        let context = &self.context;
        let exact = limits.exact_algebra;
        let algebra = |error| PError::Engine(ReductionError::ExactAlgebra(error));
        let l = i16::from(loops);
        let scheme = GammaTag::from_pairs([(1, -l), (-1, -2 * l), (-2, l)])?;
        let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
        let d_minus_3 = context
            .try_sub(&d, &context.integer(3), exact)
            .map_err(algebra)?;
        let mut factor = context.one();
        for _ in 0..loops {
            factor = context
                .try_mul(&factor, &d_minus_3, exact)
                .map_err(algebra)?;
        }
        for (terminal, tag, coefficient) in raw {
            let tag = tag.add(&scheme);
            // A saturated exponent never leaves the engine.
            tag.encode_dense(&mut Vec::new())?;
            let coefficient = context
                .try_mul(&coefficient, &factor, exact)
                .map_err(algebra)?;
            // vk5l WP1: a retained key has no basis row: it stays residual.
            let Some(map) = self
                .basis_map
                .as_ref()
                .filter(|_| (terminal.0 as usize) < self.terminals.len())
            else {
                terms.add(self, Self::plain_basis(terminal), tag, coefficient, limits)?;
                continue;
            };
            let row = map.map(terminal, &tag)?;
            if row.is_empty() {
                return Err(PError::MalformedLibrary(format!(
                    "the basis map has no row for terminal {}",
                    terminal.0
                )));
            }
            for term in row {
                if !context.contains(&term.coefficient) {
                    return Err(PError::ContextMismatch);
                }
                term.tag.encode_dense(&mut Vec::new())?;
                let value = context
                    .try_mul(&coefficient, &term.coefficient, exact)
                    .map_err(algebra)?;
                terms.add(self, term.basis, term.tag, value, limits)?;
            }
        }
        Ok(())
    }

    /// The binding of this program's step cache: one cache serves every loop
    /// count of the program.
    pub fn step_cache_binding(&self) -> Result<OrderedStepCacheBinding, PError> {
        Ok(OrderedStepCacheBinding::new(
            OrderedStepCacheAdapter::PropagatorLibrary,
            self.step_identity(),
            &self.context,
            Some(UnresolvedPolicy::Reject),
        ))
    }
}

/// Output terms collected by basis and tag.
#[derive(Default)]
struct PTermSum(BTreeMap<(PBasisOrder, GammaTag), Coefficient>);

/// `PBasis` in output order: `Unit`, masters by name, residuals by id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PBasisOrder {
    Unit,
    Master(PMasterName),
    Residual(PTerminalId),
}

impl PTermSum {
    fn add(
        &mut self,
        library: &PLibrary,
        basis: PBasis,
        tag: GammaTag,
        coefficient: Coefficient,
        limits: &ReductionLimits,
    ) -> Result<(), PError> {
        let basis = match basis {
            PBasis::Unit => PBasisOrder::Unit,
            PBasis::Master(name) => PBasisOrder::Master(name),
            PBasis::Residual(id) => PBasisOrder::Residual(id),
        };
        merge(
            library.coefficient_context(),
            limits,
            &mut self.0,
            (basis, tag),
            coefficient,
        )
    }

    fn into_terms(self) -> Vec<PTerm> {
        self.0
            .into_iter()
            .map(|((basis, tag), coefficient)| PTerm {
                basis: match basis {
                    PBasisOrder::Unit => PBasis::Unit,
                    PBasisOrder::Master(name) => PBasis::Master(name),
                    PBasisOrder::Residual(id) => PBasis::Residual(id),
                },
                tag,
                coefficient,
            })
            .collect()
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
    rule_orders: BTreeMap<OwnerClassId, Arc<PRuleOrder>>,
    ordering: Option<String>,
    revision: Option<String>,
    saved_routes: Option<Arc<PSavedRoutes>>,
    saved_orbits: Option<Arc<PSavedOrbits>>,
    open_nodes: BTreeMap<OwnerClassId, POpenNode>,
}

/// vk5l WP2: keys a rule-chain search of a recipe failure may expand.
pub const RECIPE_CHAIN_BUDGET: usize = 200_000;

/// p5l-library-6: the identity key of a package content revision.
pub const P_PACKAGE_REVISION_KEY: &str = "P_PACKAGE_REVISION";

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
        if !self.scope.contains(&p.loops) {
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
    /// - census: supports lie on the slots and contain the forced axes; a class
    ///   records the `t` and `eps_lines` of its owner sector, whose own fate is
    ///   not `Zero` and no other class; routes are unimodular and go to a known
    ///   class with the loop counts of both presentations; class nodes exist in
    ///   the topology;
    /// - owners: the node is the one of the class record, edge maps exist and
    ///   leave the owner's presentation, terminals exist and a residual is the
    ///   owner's own;
    /// - residual terminals name a known class, with a key of the arity of its
    ///   owner presentation.
    ///
    /// Lane roles are not read here: the steps check them (`check_*_contract`).
    /// p artefact v2: the explicit rule orders of the library's Custom owners
    /// and the ordering's heuristic name, which joins the identity as
    /// `,P_ORDERING_VERSION=<P_ORDERING_VERSION>:<heuristic>`. Every order must
    /// name a Custom owner; each is checked against its payload when the
    /// owner's program is built (fail closed).
    pub fn set_ordering(
        &mut self,
        heuristic: &str,
        orders: BTreeMap<OwnerClassId, PRuleOrder>,
    ) -> Result<(), PError> {
        if heuristic.is_empty()
            || !heuristic
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(malformed(format!("ordering heuristic name {heuristic:?}")));
        }
        for class in orders.keys() {
            match self.owners.get(class) {
                Some(owner) if owner.spec.lane == OwnerLane::Custom => {}
                // M5 no6: the Custom fallback payload of a Scheme owner.
                Some(owner)
                    if owner.spec.lane == OwnerLane::Scheme && owner.spec.payload.is_some() => {}
                _ => {
                    return Err(malformed(format!(
                        "a rule order for {:016x}, which is no Custom owner",
                        class.0
                    )));
                }
            }
        }
        self.rule_orders = orders.into_iter().map(|(c, o)| (c, Arc::new(o))).collect();
        self.ordering = Some(format!("{}:{heuristic}", super::P_ORDERING_VERSION));
        Ok(())
    }

    /// p5l-library-6: the package content revision, a plain name
    /// (`[A-Za-z0-9._-]+`, no digest): two packages of one textual identity but
    /// different content (a single-swap hybrid and its base) get different
    /// revisions, so different identities and step-cache bindings.
    pub fn set_revision(&mut self, name: &str) -> Result<(), PError> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(malformed(format!("package revision name {name:?}")));
        }
        self.revision = Some(name.to_owned());
        Ok(())
    }

    /// vk5l WP1 (plan VK5L §3.6, decision Q8): declare the node of owner
    /// `class` NOT CLOSED (its owner is added as usual, typically with an
    /// `OwnerProgram::Missing` program): any key reaching it is
    /// [`PError::NodeNotClosed`] naming `node` and `status`. Not part of the
    /// identity (it changes no value: such a key never reduces).
    pub fn declare_open_node(
        &mut self,
        class: OwnerClassId,
        node: &str,
        status: &str,
    ) -> Result<(), PError> {
        if node.is_empty() {
            return Err(malformed("an open node without a name"));
        }
        let open = POpenNode {
            node: node.to_owned(),
            status: status.to_owned(),
        };
        if self.open_nodes.contains_key(&class) {
            return Err(malformed(format!(
                "the open node of class {:016x} is declared twice",
                class.0
            )));
        }
        self.open_nodes.insert(class, open);
        Ok(())
    }

    /// p artefact v2: saved route verifications (`routes.rrbin`). Values do
    /// not depend on them (a restored map is the verified one), so they are
    /// not part of the identity.
    pub fn set_saved_routes(&mut self, routes: PSavedRoutes) {
        self.saved_routes = Some(Arc::new(routes));
    }

    /// p artefact v2: saved automorphism groups (`orbits.rrbin`). Values do
    /// not depend on them (they only spare the run-time search of orbit
    /// canonicalisation), so they are not part of the identity.
    pub fn set_saved_orbits(&mut self, orbits: PSavedOrbits) {
        self.saved_orbits = Some(Arc::new(orbits));
    }

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
        if let Some(class) = self
            .open_nodes
            .keys()
            .find(|class| !self.owners.contains_key(class))
        {
            return Err(malformed(format!(
                "the open node of class {:016x} has no owner",
                class.0
            )));
        }
        let mut tops = BTreeMap::new();
        for presentation in &self.presentations {
            if let PresentationKind::Top { name } = &presentation.kind
                && tops.insert(name.clone(), presentation.id).is_some()
            {
                return Err(malformed(format!("two top presentations named {name}")));
            }
        }
        // Default on when the groups are saved (P_ORBIT_CANON_WITH_SAVED_ORBITS).
        let orbit_default = if self.saved_orbits.is_some() {
            P_ORBIT_CANON_WITH_SAVED_ORBITS
        } else {
            P_ORBIT_CANON_DEFAULT
        };
        let max_loops = *self.scope.end();
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
            rule_orders: self.rule_orders,
            ordering: self.ordering,
            revision: self.revision,
            saved_routes: self.saved_routes,
            saved_orbits: self.saved_orbits,
            program_budget: PProgramBudget::from_env(max_loops),
            open_nodes: self.open_nodes,
            retained: Default::default(),
            source: OnceLock::new(),
            engine: OnceLock::new(),
            orbit_canonicalization: AtomicBool::new(orbit_default),
            route_canonicalization: AtomicBool::new(true),
            orbit_prefer_high: AtomicBool::new(true),
            scheme_certificates_trusted: AtomicBool::new(false),
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
            let sector = self.sector_presentation(presentation, support)?;
            let lines = sector
                .lines()
                .filter(|&(slot, _)| support >> slot & 1 == 1)
                .count();
            let eps_lines = sector.forced_axes().count_ones();
            if (usize::from(record.t), u32::from(record.eps_lines)) != (lines, eps_lines) {
                return Err(malformed(format!(
                    "class {label} records {} lines, {} on eps axes; its owner sector has {lines}, {eps_lines}",
                    record.t, record.eps_lines
                )));
            }
            for &node in record.node.iter().chain(&record.nodes) {
                self.node(node)?;
            }
            if owner.spec.node != record.node {
                return Err(malformed(format!(
                    "class {label} is at node {:?}, its owner record says {:?}",
                    record.node, owner.spec.node
                )));
            }
            match census.sectors.get(&record.owner) {
                None => {}
                Some(PSectorFate::Owner { class: routed, .. }) if routed == class => {}
                Some(fate) => {
                    return Err(malformed(format!(
                        "the owner sector of class {label} has the fate {fate:?}"
                    )));
                }
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
            if !map.is_unimodular() {
                return Err(malformed(format!(
                    "the route of sector ({}, {support:#x}) is not unimodular: {map:?}",
                    id.0
                )));
            }
        }
        Ok(())
    }

    /// After [`Self::check_classes`]: an owner's node is its class's, which exists.
    fn check_owners(&self, census: &PCensus) -> Result<(), PError> {
        for (class, owner) in &self.owners {
            let spec = &owner.spec;
            if !census.classes.contains_key(class) {
                return Err(malformed(format!(
                    "owner of class {:016x} outside the census",
                    class.0
                )));
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
            for terminal in &spec.terminals {
                match self.terminals.get(terminal.0 as usize) {
                    None => {
                        return Err(malformed(format!("owner names terminal {}", terminal.0)));
                    }
                    Some(PTerminal::Residual { owner, .. }) if owner != class => {
                        return Err(malformed(format!(
                            "the owner of class {:016x} names terminal {}, a residual of class {:016x}",
                            class.0, terminal.0, owner.0
                        )));
                    }
                    Some(_) => {}
                }
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
