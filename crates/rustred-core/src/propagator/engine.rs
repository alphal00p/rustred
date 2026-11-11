//! The Ordered adapter of the p library (owner: M2-T6 rr-plib).
//!
//! A scheduled identity is `Route` (presentation, view) or `Apply` (owner class,
//! view), where the view holds the powers of the presentation (its own arity)
//! and the [`P_GAMMA_TAG_WIDTH`] dense slots of the absolute Gamma tag, so
//! different tags never merge. Views and terminal keys of one library are
//! padded with zeros to one width: a step-cache shard stores one identity width
//! ([`ORDERED_STEP_CACHE_SCHEMA`] layout), and a step mixes presentations and
//! terminals.
//!
//! Phases follow the number of lines `t` of the support, descending: `Route(t)`
//! then `Apply(t)`. The owners of one `t` share `Apply(t)`, ordered by node rank
//! and then by the owner's order key ([`PLaneStep::order_key`]). A Route step
//! is a zero proof, the census route onto the owner, or nothing at all on an
//! owner sector; an Apply step is the owner's lane program. The support of a
//! key always holds the forced eps axes of its presentation, so a key with an
//! eps power `<= 0` stays in its full sector.
//!
//! Steps are pure functions of the key and the library: the scheduler computes
//! them ahead of time on its workers, and the step cache records them (shards
//! `(stage, t, class)`, the class of a Route shard being the presentation).
//!
//! [`PLaneStep::order_key`]: super::step::PLaneStep::order_key
//! [`ORDERED_STEP_CACHE_SCHEMA`]: crate::reduction::ORDERED_STEP_CACHE_SCHEMA
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use smallvec::SmallVec;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::IntegralKey;
use crate::reduction::ordered_reduction::step_cache::codec::NodeStage;
use crate::reduction::ordered_reduction::step_cache::failure;
use crate::reduction::ordered_reduction::step_cache::kernel::{
    ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
};
use crate::reduction::ordered_reduction::{
    CompactPowers, ConcurrentOrderedKernel, FrontierKey, OrderedKernel, OrderedKeyStage,
    OrderedOutput, OrderedPhaseBoundary,
    OrderedPhaseDescriptor, OrderedPhaseEvent, OrderedPosition, OrderedStep, OrderedStepper,
};
use crate::reduction::{
    OrderedSchedulerOptions, OrderedStepCache, OrderedStepShard, OrderedStepStage,
    OrderedWeightArithmetic, ReductionError, ReductionLimits, ReductionRequest,
    ReductionStatistics,
};
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport::{self, ExpansionLimits, Prepared};
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap};
use crate::solver::{CandidateOrderedPhaseKind, CandidateOrderedReductionProgress};

use super::census::{PSectorFate, SignedAffineLoopMap};
use super::error::PError;
use super::library::{OwnerProgram, PInspector, PLibrary, POwner};
use super::path_census::{
    PCensusAgg, PCensusKind, PCensusOwner, PPathCensus, PRouteRecord, PStepRecord,
};
use super::presentation::Presentation;
use super::step::{PChild, PKeyView, POrderKey, PStage, PStepContext, PStepOut, default_order_key};
use super::tag::GammaTag;
use super::terminal::PTerminalCodec;
use super::{OwnerClassId, P_GAMMA_TAG_WIDTH, PTerminalId, PresentationId};

/// The slots of a view decoded on the stack (no heap for the 4L and 5L
/// libraries: 30 and 36 slots).
pub(crate) type ViewSlots = SmallVec<[i64; 64]>;

/// rr-alloc: the view of a p key (`powers ++ dense tag ++ zeros`, see the
/// module docs) stored inline as `i8` slots ([`CompactPowers`], a heap form
/// only for a slot outside `i8` or a library wider than its inline
/// capacity), so building, cloning, packing and unpacking a key allocates
/// nothing. Equality, order and hash are those of the slot sequence, exactly
/// as for the `IntegralKey` it replaces (`Vec<i64>` order and hash); the
/// `IntegralKey` itself is built only on demand ([`Self::integral`]: phase
/// inspection and the step cache) and kept.
#[derive(Clone, Debug)]
pub(crate) struct PView {
    packed: CompactPowers,
    integral: OnceLock<IntegralKey>,
}

impl PView {
    fn new(slots: &[i64]) -> Self {
        Self {
            packed: CompactPowers::new(slots, false),
            integral: OnceLock::new(),
        }
    }

    fn from_integral(key: IntegralKey) -> Self {
        Self {
            packed: CompactPowers::new(key.powers(), false),
            integral: OnceLock::from(key),
        }
    }

    /// The slots, decoded.
    pub(crate) fn slots(&self) -> ViewSlots {
        let mut slots = ViewSlots::new();
        self.packed.decode_into(&mut slots);
        slots
    }

    /// The view as an `IntegralKey`, built on first use.
    pub(crate) fn integral(&self) -> &IntegralKey {
        self.integral.get_or_init(|| {
            IntegralKey::try_from_preallocated(self.packed.to_vec()).expect("a view is nonempty")
        })
    }
}

impl PartialEq for PView {
    fn eq(&self, other: &Self) -> bool {
        self.packed == other.packed
    }
}

impl Eq for PView {}

impl PartialOrd for PView {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PView {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.packed.cmp_powers(&other.packed)
    }
}

impl std::hash::Hash for PView {
    /// The hash of the slot slice, as `IntegralKey`'s.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.slots().as_slice().hash(state);
    }
}

/// One scheduled identity of a p reduction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PKey {
    Route {
        presentation: PresentationId,
        view: PView,
    },
    Apply {
        class: OwnerClassId,
        view: PView,
    },
}

/// `PKey` packed in a `Packed` frontier (rr-frontier): the presentation or
/// class id, and the powers inline with the variant as the tag (`Apply`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PackedPKey {
    id: u64,
    view: CompactPowers,
}

impl FrontierKey for PKey {
    type Packed = PackedPKey;
    fn pack(&self) -> PackedPKey {
        match self {
            PKey::Route { presentation, view } => PackedPKey {
                id: u64::from(presentation.0),
                view: view.packed.with_tag(false),
            },
            PKey::Apply { class, view } => PackedPKey {
                id: class.0,
                view: view.packed.with_tag(true),
            },
        }
    }
    fn unpack(packed: &PackedPKey) -> PKey {
        let view = PView {
            packed: packed.view.with_tag(false),
            integral: OnceLock::new(),
        };
        if packed.view.tag() {
            PKey::Apply {
                class: OwnerClassId(packed.id),
                view,
            }
        } else {
            PKey::Route {
                presentation: PresentationId(
                    u32::try_from(packed.id).expect("a packed presentation id is a u32"),
                ),
                view,
            }
        }
    }
    /// The derived order of `PKey`: `Route` before `Apply`, then the id, then
    /// the powers.
    fn cmp_packed(a: &PackedPKey, b: &PackedPKey) -> std::cmp::Ordering {
        a.view
            .tag()
            .cmp(&b.view.tag())
            .then(a.id.cmp(&b.id))
            .then_with(|| a.view.cmp_powers(&b.view))
    }
    fn packed_heap_bytes(packed: &PackedPKey) -> usize {
        packed.view.heap_bytes()
    }
}

/// Position inside one phase, larger first: the node rank of the owner, then
/// the owner's order key. Route keys have no descent inside their phase.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PPriority {
    rank: u32,
    order: POrderKey,
}

/// `Route(t)` and `Apply(t)` of `t` lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhasePlan {
    max_lines: usize,
}

impl PhasePlan {
    fn route(self, t: usize) -> usize {
        2 * (self.max_lines - t)
    }

    fn apply(self, t: usize) -> usize {
        self.route(t) + 1
    }

    fn count(self) -> usize {
        self.apply(0) + 1
    }

    fn describe(self, phase: usize) -> OrderedPhaseDescriptor<'static> {
        let kind = if phase.is_multiple_of(2) {
            CandidateOrderedPhaseKind::Routing
        } else {
            CandidateOrderedPhaseKind::Reduction
        };
        OrderedPhaseDescriptor {
            kind,
            support: Some(self.max_lines.saturating_sub(phase / 2)),
            ..Default::default()
        }
    }
}

/// What the adapter reads of one presentation on every key.
struct Shape {
    arity: usize,
    lines: u64,
    forced: u64,
}

impl Shape {
    fn new(presentation: &Presentation) -> Self {
        Self {
            arity: presentation.slots.len(),
            lines: presentation
                .lines()
                .fold(0, |mask, (slot, _)| mask | 1 << slot),
            forced: presentation.forced_axes(),
        }
    }

    /// `raw | forced` of `powers`.
    fn support(&self, powers: &[i64]) -> u64 {
        powers
            .iter()
            .enumerate()
            .filter(|&(_, &power)| power > 0)
            .fold(self.forced, |mask, (slot, _)| mask | 1 << slot)
    }

    fn line_count(&self, support: u64) -> usize {
        (support & self.lines).count_ones() as usize
    }
}

struct OwnerSlot {
    presentation: PresentationId,
    support: u64,
    t: usize,
    rank: u32,
}

/// Where a Route key goes.
enum Sector<'e> {
    Zero,
    /// The owner sector itself.
    Owner(OwnerClassId),
    Routed {
        class: OwnerClassId,
        map: &'e SignedAffineLoopMap,
    },
}

/// Scheduling tables of one library and its lazily compiled routes; pure
/// functions of the library, built on its first reduction.
pub(crate) struct PEngine {
    shapes: Vec<Shape>,
    /// Slots of every view and terminal key of this library.
    width: usize,
    /// Terminals of this library: ids at or above it name nothing.
    terminals: usize,
    phases: PhasePlan,
    owners: BTreeMap<OwnerClassId, OwnerSlot>,
    owner_sectors: BTreeMap<(PresentationId, u64), OwnerClassId>,
    routes: BTreeMap<(PresentationId, u64), OnceLock<Result<Prepared, PError>>>,
    /// rr-alloc: the id of this engine in the per-thread route memo.
    uid: u64,
}

impl PEngine {
    pub(crate) fn new(library: &PLibrary) -> Result<Self, PError> {
        let shapes: Vec<Shape> = library
            .presentations()
            .iter()
            .map(|presentation| Shape::new(presentation))
            .collect();
        let max_lines = shapes
            .iter()
            .map(|shape| shape.lines.count_ones() as usize)
            .max()
            .unwrap_or(0);
        let max_arity = shapes.iter().map(|shape| shape.arity).max().unwrap_or(0);
        // Parents first: the DAG lists more lines and more loops first, and a
        // larger rank is reduced first. Rank ties cannot happen.
        let census = library.census();
        let mut order: Vec<_> = census.classes.iter().collect();
        order.sort_by_key(|(class, record)| (record.node.is_none(), record.node, **class));
        let count = u32::try_from(order.len()).unwrap_or(u32::MAX);
        let mut owners = BTreeMap::new();
        let mut owner_sectors = BTreeMap::new();
        for (index, (&class, record)) in order.into_iter().enumerate() {
            let (presentation, support) = record.owner;
            let rank = count.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let t = usize::from(record.t);
            owners.insert(
                class,
                OwnerSlot {
                    presentation,
                    support,
                    t,
                    rank,
                },
            );
            if owner_sectors.insert(record.owner, class).is_some() {
                return Err(PError::MalformedLibrary(format!(
                    "two classes own sector {support:#x} of presentation {}",
                    presentation.0
                )));
            }
        }
        let routes = census
            .sectors
            .iter()
            .filter(|(_, fate)| matches!(fate, PSectorFate::Owner { .. }))
            .map(|(&sector, _)| (sector, OnceLock::new()))
            .collect();
        Ok(Self {
            shapes,
            width: (max_arity + P_GAMMA_TAG_WIDTH).max(PTerminalCodec::KEY_SLOTS),
            terminals: library.terminals().len(),
            phases: PhasePlan { max_lines },
            owners,
            owner_sectors,
            routes,
            uid: super::step_memo::next_uid(),
        })
    }

    fn shape(&self, presentation: PresentationId) -> Result<&Shape, PError> {
        self.shapes.get(presentation.0 as usize).ok_or_else(|| {
            PError::MalformedLibrary(format!("unknown presentation {}", presentation.0))
        })
    }

    fn owner(&self, class: OwnerClassId) -> Result<&OwnerSlot, PError> {
        self.owners.get(&class).ok_or_else(|| {
            PError::MalformedLibrary(format!("unknown owner class {:016x}", class.0))
        })
    }

    /// The powers and tag slots of `view`, a key of `presentation`; the slots
    /// beyond the tag are zero in every key this library writes.
    fn split<'v>(
        &self,
        presentation: PresentationId,
        slots: &'v [i64],
    ) -> Result<(&'v [i64], &'v [i64]), PError> {
        let arity = self.shape(presentation)?.arity;
        if slots.len() != self.width || arity + P_GAMMA_TAG_WIDTH > self.width {
            return Err(PError::MalformedLibrary(format!(
                "a key of presentation {} has {} slots, not {}",
                presentation.0,
                slots.len(),
                self.width
            )));
        }
        let (powers, rest) = slots.split_at(arity);
        let (tag, padding) = rest.split_at(P_GAMMA_TAG_WIDTH);
        if padding.iter().any(|&slot| slot != 0) {
            return Err(PError::MalformedLibrary(format!(
                "a key of presentation {} has nonzero slots beyond its tag",
                presentation.0
            )));
        }
        Ok((powers, tag))
    }

    /// `powers ++ dense tag ++ zeros`; a tag outside the key range is
    /// [`PError::TagOutOfRange`].
    fn view(&self, powers: &[i64], tag: &GammaTag) -> Result<PView, PError> {
        let mut slots = ViewSlots::new();
        slots.extend_from_slice(powers);
        tag.encode_dense_into(&mut slots)?;
        if slots.len() > self.width {
            return Err(PError::MalformedLibrary(format!(
                "a key of {} slots in a library of width {}",
                slots.len(),
                self.width
            )));
        }
        slots.resize(self.width, 0);
        Ok(PView::new(&slots))
    }

    fn padded(&self, mut slots: Vec<i64>) -> Result<IntegralKey, PError> {
        if slots.len() > self.width {
            return Err(PError::MalformedLibrary(format!(
                "a key of {} slots in a library of width {}",
                slots.len(),
                self.width
            )));
        }
        slots.resize(self.width, 0);
        IntegralKey::try_from_preallocated(slots)
            .map_err(|error| PError::MalformedLibrary(error.to_string()))
    }

    /// The terminal key of `terminal` with `tag`.
    fn terminal_key(&self, terminal: PTerminalId, tag: &GammaTag) -> Result<IntegralKey, PError> {
        self.padded(PTerminalCodec::encode(terminal, tag)?.into_powers())
    }

    /// The terminal and tag of a terminal key of the output. A key can come from
    /// a step record, so nothing of it is taken on trust: its slots beyond the
    /// codec's are zero and its terminal exists in this library.
    pub(crate) fn decode_terminal(
        &self,
        key: &IntegralKey,
    ) -> Result<(PTerminalId, GammaTag), PError> {
        let slots = key.powers();
        if slots.len() != self.width {
            return Err(PError::MalformedLibrary(format!(
                "a terminal key has {} slots, not {}",
                slots.len(),
                self.width
            )));
        }
        let (plain, padding) = slots.split_at(PTerminalCodec::KEY_SLOTS);
        if padding.iter().any(|&slot| slot != 0) {
            return Err(PError::MalformedLibrary(
                "a terminal key has nonzero slots beyond its tag".to_owned(),
            ));
        }
        let plain = IntegralKey::try_new(plain.iter().copied())
            .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
        let (terminal, tag) = PTerminalCodec::decode(&plain)?;
        if terminal.0 as usize >= self.terminals {
            return Err(PError::MalformedLibrary(format!(
                "a terminal key names the unknown terminal {}",
                terminal.0
            )));
        }
        Ok((terminal, tag))
    }

    /// The Route key of `powers` on `presentation` with `tag`.
    fn route_key(
        &self,
        presentation: PresentationId,
        powers: &[i64],
        tag: &GammaTag,
    ) -> Result<PKey, PError> {
        Ok(PKey::Route {
            presentation,
            view: self.view(powers, tag)?,
        })
    }

    /// The fate of a sector of `presentation`. A sector without a fate is a gap
    /// of the census, never a zero.
    fn sector<'e>(
        &self,
        library: &'e PLibrary,
        presentation: PresentationId,
        support: u64,
    ) -> Result<Sector<'e>, PError> {
        let sector = (presentation, support);
        if let Some(&class) = self.owner_sectors.get(&sector) {
            return Ok(Sector::Owner(class));
        }
        match library.census().sectors.get(&sector) {
            Some(PSectorFate::Zero) => Ok(Sector::Zero),
            Some(PSectorFate::Owner { class, map }) => Ok(Sector::Routed { class: *class, map }),
            None => Err(PError::MalformedLibrary(format!(
                "sector {support:#x} of presentation {} is not in the census",
                presentation.0
            ))),
        }
    }

    /// The weighted Route keys of `inputs`, checked one by one: a cancelled
    /// input is checked like every other.
    pub(crate) fn initial(
        &self,
        library: &PLibrary,
        inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
        limits: &ReductionLimits,
    ) -> Result<BTreeMap<PKey, Coefficient>, PError> {
        let context = library.coefficient_context();
        let one = GammaTag::one();
        let mut initial = BTreeMap::new();
        for (presentation, key, coefficient) in inputs {
            let shape = self.shapes.get(presentation.0 as usize).ok_or_else(|| {
                PError::Unsupported(format!(
                    "input on presentation {}, which this library does not have",
                    presentation.0
                ))
            })?;
            if key.powers().len() != shape.arity {
                return Err(PError::Unsupported(format!(
                    "an input of presentation {} has {} powers, not {}",
                    presentation.0,
                    key.powers().len(),
                    shape.arity
                )));
            }
            if !context.contains(&coefficient) {
                return Err(PError::ContextMismatch);
            }
            context
                .validate_with_limits(&coefficient, limits.exact_algebra)
                .map_err(algebra)?;
            let support = shape.support(key.powers());
            if support & !shape.lines != 0 {
                return Err(PError::Unsupported(format!(
                    "input {:?} of presentation {} has an ISP in its denominator",
                    key.powers(),
                    presentation.0
                )));
            }
            self.sector(library, presentation, support)?;
            let key = self.route_key(presentation, key.powers(), &one)?;
            merge(context, limits, &mut initial, key, coefficient)?;
        }
        Ok(initial)
    }

    /// Run one request; the terminal keys of the output are [`PTerminalCodec`]
    /// keys.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &self,
        library: &PLibrary,
        initial: BTreeMap<PKey, Coefficient>,
        arithmetic: OrderedWeightArithmetic,
        scheduler: OrderedSchedulerOptions,
        cache: Option<&OrderedStepCache>,
        limits: ReductionLimits,
        mut inspector: Option<&mut dyn PInspector>,
    ) -> Result<OrderedOutput, PError> {
        let census = match inspector.as_deref_mut().and_then(PInspector::path_census) {
            Some(census) => PCensusMode {
                on: true,
                keep_apply_keys: census.options.keep_apply_keys,
            },
            None => PCensusMode::OFF,
        };
        let mut kernel = PKernel {
            stepper: PStepper {
                library,
                engine: self,
                limits,
                census,
            },
            inspector,
        };
        let output = run_with_step_cache_and_options(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            Vec::new(),
            arithmetic,
            scheduler,
            cache,
        );
        // rr-alloc diagnostics: the step memo counters of this thread (all
        // steps at one thread).
        if let Ok(path) = std::env::var("RUSTRED_P_STEP_MEMO_STATS") {
            use std::io::Write;
            let [ih, im, ie, rh, rm, re, distinct] = super::step_memo::counters();
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(
                    file,
                    "{{\"insertion\":{{\"hits\":{ih},\"misses\":{im},\"entries\":{ie}}},\"route\":{{\"hits\":{rh},\"misses\":{rm},\"entries\":{re}}},\"coefficients\":{distinct}}}"
                );
            }
        }
        super::step_memo::clear();
        output
    }
}

fn algebra(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

/// Add `coefficient` to `key`; a sum that cancels leaves no entry.
pub(super) fn merge<K: Ord>(
    context: &CoefficientContext,
    limits: &ReductionLimits,
    terms: &mut BTreeMap<K, Coefficient>,
    key: K,
    coefficient: Coefficient,
) -> Result<(), PError> {
    use std::collections::btree_map::Entry;
    if coefficient.is_zero() {
        return Ok(());
    }
    match terms.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(coefficient);
        }
        Entry::Occupied(mut entry) => {
            let sum = context
                .try_add(entry.get(), &coefficient, limits.exact_algebra)
                .map_err(algebra)?;
            if sum.is_zero() {
                entry.remove();
            } else {
                entry.insert(sum);
            }
        }
    }
    Ok(())
}

/// The route of a sector as an exact, verified transport.
fn compile_route(
    source: &Presentation,
    target: &Presentation,
    map: &SignedAffineLoopMap,
    source_support: u64,
    target_support: u64,
) -> Result<Prepared, PError> {
    let verified = verify_route(source, target, map, source_support)?;
    transport_of(source, target, verified, source_support, target_support)
}

/// The route of a sector from its saved verification (p artefact v2, spec
/// item 4): a trusted restore ([`symmetry::saved::SavedVerifiedMap::restore`])
/// of the map that [`verify_route`] proved when the package was written, then
/// the same transport compilation. `None` without a saved record or when the
/// record does not restore (the caller then verifies, as without saved routes).
fn restored_route(
    library: &PLibrary,
    source: &Presentation,
    target: &Presentation,
    map: &SignedAffineLoopMap,
    source_support: u64,
    target_support: u64,
) -> Option<Result<Prepared, PError>> {
    let verified =
        library
            .saved_routes()?
            .restore(source, target, source_support, target_support, map)?;
    Some(transport_of(
        source,
        target,
        verified,
        source_support,
        target_support,
    ))
}

fn transport_of(
    source: &Presentation,
    target: &Presentation,
    verified: symmetry::VerifiedMap,
    source_support: u64,
    target_support: u64,
) -> Result<Prepared, PError> {
    let root = |presentation: &Presentation, support: u64| {
        Mask::try_new((0..presentation.slots.len()).map(|slot| support >> slot & 1 == 1))
            .map_err(|error| PError::MalformedLibrary(error.to_string()))
    };
    Ok(integral_transport::compile(
        &source.family,
        Arc::clone(&target.family),
        Arc::new(verified),
        root(source, source_support)?,
        root(target, target_support)?,
        ExpansionLimits::default(),
    )?)
}

/// The exact verification of the route map of a sector.
pub(crate) fn verify_route(
    source: &Presentation,
    target: &Presentation,
    map: &SignedAffineLoopMap,
    source_support: u64,
) -> Result<symmetry::VerifiedMap, PError> {
    let context = source.family.coefficient_context();
    let (loops_in, loops_out) = (usize::from(map.loops_in), usize::from(map.loops_out));
    let malformed = |error: symmetry::Error| {
        PError::MalformedLibrary(format!(
            "the route of sector {source_support:#x} of presentation {}: {error}",
            source.id.0
        ))
    };
    let matrix = |rows, columns, values: &[i64]| {
        CoefficientMatrix::try_new(
            rows,
            columns,
            values.iter().map(|&value| context.integer(value)),
        )
        .map_err(malformed)
    };
    let momentum = MomentumMap::new(
        matrix(loops_in, loops_out, &map.loop_linear)?,
        matrix(loops_in, 1, &map.loop_external)?,
        matrix(1, 1, &[i64::from(map.external_sign)])?,
    );
    let verified = symmetry::verify(
        &source.family,
        &target.family,
        momentum,
        symmetry::Limits::default(),
    )
    .map_err(malformed)?;
    Ok(verified)
}

/// Every routed sector of `library` (census fate `Owner` on a sector that is
/// not an owner sector), in census order, with its owner's presentation and
/// support and its census map.
pub(crate) fn routed_sectors(
    library: &PLibrary,
) -> Vec<(
    PresentationId,
    u64,
    PresentationId,
    u64,
    &SignedAffineLoopMap,
)> {
    let census = library.census();
    let owners: BTreeMap<OwnerClassId, (PresentationId, u64)> = census
        .classes
        .iter()
        .map(|(class, record)| (*class, record.owner))
        .collect();
    let owner_sectors: std::collections::BTreeSet<(PresentationId, u64)> =
        owners.values().copied().collect();
    census
        .sectors
        .iter()
        .filter_map(|(&(presentation, support), fate)| match fate {
            PSectorFate::Owner { class, map }
                if !owner_sectors.contains(&(presentation, support)) =>
            {
                let &(target, target_support) = owners.get(class)?;
                Some((presentation, support, target, target_support, map))
            }
            _ => None,
        })
        .collect()
}

/// The shared, read-only part of the adapter: pure positions and steps.
#[derive(Clone, Copy)]
pub(crate) struct PStepper<'l> {
    library: &'l PLibrary,
    engine: &'l PEngine,
    limits: ReductionLimits,
    census: PCensusMode,
}

/// Whether the request runs the opt-in path census (M5 R0 WP0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PCensusMode {
    on: bool,
    keep_apply_keys: bool,
}

impl PCensusMode {
    pub(crate) const OFF: Self = Self {
        on: false,
        keep_apply_keys: false,
    };
}

/// Work of one detached step; with the path census on, its record.
#[derive(Debug, Default)]
pub(crate) struct PAccount {
    rule_applications: usize,
    census: Option<Box<PStepRecord>>,
}

impl<'l> PStepper<'l> {
    /// The path-census kind and id of a key: Route by presentation, Apply by
    /// owner class.
    fn census_id(key: &PKey) -> (PCensusKind, u64) {
        match key {
            PKey::Route { presentation, .. } => (0, u64::from(presentation.0)),
            PKey::Apply { class, .. } => (1, class.0),
        }
    }

    /// A 128-bit identity of a key (two independent SipHash-1-3 digests).
    fn census_hash(key: &PKey) -> u128 {
        use std::hash::{Hash, Hasher};
        let mut first = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut first);
        let mut second = std::collections::hash_map::DefaultHasher::new();
        0x9e37_79b9_7f4a_7c15_u64.hash(&mut second);
        key.hash(&mut second);
        (u128::from(first.finish()) << 64) | u128::from(second.finish())
    }

    /// The path-census record of one computed step (census on only).
    fn census_record(&self, key: &PKey, step: &OrderedStep<PKey>, nanos: u64) -> PStepRecord {
        let (kind, id) = Self::census_id(key);
        let (presentation, view) = match key {
            PKey::Route { presentation, view } => (Some(*presentation), view),
            PKey::Apply { class, view } => (
                self.engine.owner(*class).ok().map(|slot| slot.presentation),
                view,
            ),
        };
        let parent_phase = self.phase(key).ok();
        let (mut lines, mut dots, mut rank) = (0_u32, 0_i64, 0_i64);
        let slots = view.slots();
        if let Some(presentation) = presentation
            && let Ok((powers, _)) = self.engine.split(presentation, &slots)
            && let Ok(shape) = self.engine.shape(presentation)
        {
            lines = u32::try_from(shape.line_count(shape.support(powers))).unwrap_or(u32::MAX);
            dots = powers.iter().filter(|&&p| p > 1).map(|&p| p - 1).sum();
            rank = powers.iter().filter(|&&p| p < 0).map(|&p| -p).sum();
        }
        let mut agg = PCensusAgg {
            steps: 1,
            nanos,
            terminals: step.terminals.len() as u64,
            ch_max: step.terms.len() as u64,
            empty: u64::from(step.terms.is_empty() && step.terminals.is_empty()),
            ..PCensusAgg::default()
        };
        for (child, coefficient) in &step.terms {
            match child {
                PKey::Apply { class, .. } if kind == 1 && class.0 == id => agg.ch_same += 1,
                PKey::Apply { .. } => agg.ch_apply += 1,
                PKey::Route {
                    presentation: child_presentation,
                    view: child_view,
                } => {
                    agg.ch_route += 1;
                    let child_slots = child_view.slots();
                    let zero = self
                        .engine
                        .split(*child_presentation, &child_slots)
                        .ok()
                        .and_then(|(child_powers, _)| {
                            let support = self
                                .engine
                                .shape(*child_presentation)
                                .ok()?
                                .support(child_powers);
                            Some(matches!(
                                self.engine
                                    .sector(self.library, *child_presentation, support),
                                Ok(Sector::Zero)
                            ))
                        })
                        .unwrap_or(false);
                    agg.ch_zero += u64::from(zero);
                }
            }
            match (parent_phase, self.phase(child).ok()) {
                (Some(parent), Some(phase)) if phase < parent => agg.ch_earlier_phase += 1,
                (Some(parent), Some(phase)) if phase == parent => {
                    agg.ch_same_phase += 1;
                    let same_owner =
                        matches!(child, PKey::Apply { class, .. } if kind == 1 && class.0 == id);
                    agg.ch_foreign_same_phase += u64::from(!same_owner);
                }
                _ => {}
            }
            let terms = (coefficient.numerator.nterms() + coefficient.denominator.nterms()) as u64;
            agg.coeff_terms += terms;
            agg.coeff_terms_max = agg.coeff_terms_max.max(terms);
        }
        for coefficient in step.terminals.values() {
            let terms = (coefficient.numerator.nterms() + coefficient.denominator.nterms()) as u64;
            agg.coeff_terms += terms;
            agg.coeff_terms_max = agg.coeff_terms_max.max(terms);
        }
        let apply_view = match key {
            PKey::Apply { view, .. } if self.census.keep_apply_keys => {
                Some(view.slots().into_vec().into())
            }
            _ => None,
        };
        let route = match key {
            PKey::Route { presentation, view } => self.route_record(*presentation, view, step),
            PKey::Apply { .. } => None,
        };
        let phase = parent_phase.map_or(u32::MAX, |phase| u32::try_from(phase).unwrap_or(u32::MAX));
        PStepRecord {
            key: (phase, kind, id, lines, dots, rank),
            agg,
            apply_view,
            rule: None,
            route,
        }
    }

    /// The route record of a Route key on a routed sector (census on only).
    fn route_record(
        &self,
        presentation: PresentationId,
        view: &PView,
        step: &OrderedStep<PKey>,
    ) -> Option<PRouteRecord> {
        let slots = view.slots();
        let (powers, _) = self.engine.split(presentation, &slots).ok()?;
        let support = self.engine.shape(presentation).ok()?.support(powers);
        match self.engine.sector(self.library, presentation, support) {
            Ok(Sector::Routed { .. }) => Some(PRouteRecord {
                presentation: presentation.0,
                support,
                numerator: powers.iter().map(|&p| p.min(0)).collect(),
                terms: step.terms.len() as u64,
            }),
            _ => None,
        }
    }

    /// The inflow of a phase at its start: its pending keys per `(kind, id)`,
    /// its pending Apply keys per (lines, dots, rank), and the pending keys of
    /// later phases.
    #[allow(clippy::type_complexity)]
    fn census_inflow(
        &self,
        event: &OrderedPhaseEvent<'_>,
    ) -> (
        BTreeMap<(PCensusKind, u64), u64>,
        BTreeMap<(u32, i64, i64), u64>,
        u64,
    ) {
        let mut inflow: BTreeMap<(PCensusKind, u64), u64> = BTreeMap::new();
        let mut profile: BTreeMap<(u32, i64, i64), u64> = BTreeMap::new();
        let mut later = 0_u64;
        for entry in event.frontier() {
            if entry.phase != event.phase {
                later += 1;
                continue;
            }
            let (kind, id, presentation) = match entry.stage {
                OrderedKeyStage::ClassRoute { class } => {
                    (0, class, u32::try_from(class).ok().map(PresentationId))
                }
                OrderedKeyStage::ClassApply { class } => (
                    1,
                    class,
                    self.engine
                        .owner(OwnerClassId(class))
                        .ok()
                        .map(|slot| slot.presentation),
                ),
                _ => (u8::MAX, 0, None),
            };
            *inflow.entry((kind, id)).or_default() += 1;
            if kind == 1
                && let Some(presentation) = presentation
                && let Ok((powers, _)) = self.engine.split(presentation, entry.integral.powers())
                && let Ok(shape) = self.engine.shape(presentation)
            {
                let lines =
                    u32::try_from(shape.line_count(shape.support(powers))).unwrap_or(u32::MAX);
                let dots = powers.iter().filter(|&&p| p > 1).map(|&p| p - 1).sum();
                let rank = powers.iter().filter(|&&p| p < 0).map(|&p| -p).sum();
                *profile.entry((lines, dots, rank)).or_default() += 1;
            }
        }
        (inflow, profile, later)
    }

    /// The census entry of owner `class`.
    fn census_owner(&self, class: u64) -> Option<PCensusOwner> {
        let class = OwnerClassId(class);
        let slot = self.engine.owner(class).ok()?;
        let owner = self.library.owner(class)?;
        Some(PCensusOwner {
            lane: format!("{:?}", owner.spec.lane),
            presentation: slot.presentation.0,
            t: slot.t,
            rank: slot.rank,
        })
    }

    fn owner(&self, class: OwnerClassId) -> Result<&'l POwner, PError> {
        self.library
            .owner(class)
            .ok_or_else(|| PError::MalformedLibrary(format!("no owner of class {:016x}", class.0)))
    }

    fn context(&self, owner: &'l POwner) -> PStepContext<'_> {
        PStepContext {
            library: self.library,
            owner,
            limits: &self.limits,
        }
    }

    fn route_t(&self, presentation: PresentationId, view: &PView) -> Result<usize, PError> {
        let slots = view.slots();
        let (powers, _) = self.engine.split(presentation, &slots)?;
        let shape = self.engine.shape(presentation)?;
        Ok(shape.line_count(shape.support(powers)))
    }

    fn phase(&self, key: &PKey) -> Result<usize, PError> {
        let phases = self.engine.phases;
        match key {
            PKey::Route { presentation, view } => {
                Ok(phases.route(self.route_t(*presentation, view)?))
            }
            PKey::Apply { class, .. } => Ok(phases.apply(self.engine.owner(*class)?.t)),
        }
    }

    fn priority(&self, key: &PKey) -> Result<PPriority, PError> {
        let PKey::Apply { class, view } = key else {
            return Ok(PPriority {
                rank: 0,
                order: POrderKey::Lexicographic(Default::default()),
            });
        };
        let slot = self.engine.owner(*class)?;
        let slots = view.slots();
        let (powers, _) = self.engine.split(slot.presentation, &slots)?;
        let owner = self.owner(*class)?;
        let order = match owner.program(self.library)? {
            OwnerProgram::Lane(lane) => lane.order_key(&self.context(owner), powers)?,
            // The step of this key is the typed error, unless the key cancels.
            OwnerProgram::Missing { .. } => default_order_key(&owner.spec, powers)?,
        };
        Ok(PPriority {
            rank: slot.rank,
            order,
        })
    }

    fn route_step(
        &self,
        presentation: PresentationId,
        view: &PView,
    ) -> Result<OrderedStep<PKey>, PError> {
        let engine = self.engine;
        let slots = view.slots();
        let (powers, tag_slots) = engine.split(presentation, &slots)?;
        let support = engine.shape(presentation)?.support(powers);
        let mut step = OrderedStep::default();
        let (class, map) = match engine.sector(self.library, presentation, support)? {
            Sector::Zero => return Ok(step),
            Sector::Owner(class) => {
                let key = PKey::Apply {
                    class,
                    view: view.clone(),
                };
                step.terms
                    .insert(key, self.library.coefficient_context().one());
                return Ok(step);
            }
            Sector::Routed { class, map } => (class, map),
        };
        let owner = engine.owner(class)?;
        let source = self.library.presentation(presentation);
        let target = self.library.presentation(owner.presentation);
        let prepared = engine
            .routes
            .get(&(presentation, support))
            .ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "sector {support:#x} of presentation {} has no route cell",
                    presentation.0
                ))
            })?
            .get_or_init(|| {
                restored_route(self.library, source, target, map, support, owner.support)
                    .unwrap_or_else(|| compile_route(source, target, map, support, owner.support))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let limits = ExpansionLimits {
            diagnostics_enabled: self.limits.diagnostics_enabled,
            exact_algebra: self.limits.exact_algebra,
            ..Default::default()
        };
        let tag = GammaTag::decode_dense(tag_slots)?;
        let context = self.library.coefficient_context();
        let shape = engine.shape(owner.presentation)?;
        let child = |powers: &[i64]| -> Result<PKey, PError> {
            // A numerator can pinch a line of the owner: that is a subsector
            // of its presentation, routed by the census in a later phase.
            if shape.support(powers) == owner.support {
                Ok(PKey::Apply {
                    class,
                    view: engine.view(powers, &tag)?,
                })
            } else {
                engine.route_key(owner.presentation, powers, &tag)
            }
        };
        let direct = |terms: &mut BTreeMap<PKey, Coefficient>| -> Result<(), PError> {
            let key = IntegralKey::try_from_preallocated(powers.to_vec())
                .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
            let mapped = prepared.transport(&key, limits)?;
            for endpoint in mapped.terms() {
                let child = child(endpoint.key().powers())?;
                merge(
                    context,
                    &self.limits,
                    terms,
                    child,
                    endpoint.coefficient().clone(),
                )?;
            }
            Ok(())
        };
        let mode = super::step_memo::mode();
        if mode == super::step_memo::MemoMode::Off {
            direct(&mut step.terms)?;
            return Ok(step);
        }
        // rr-alloc memo: the transport of a key is its layered base (the
        // positive and forced powers, placed on their target axes) times the
        // expansion of the numerator factors of its non-forced negative axes
        // (`Prepared::layered_base`). The expansion is memoised under the
        // non-positive powers, relative to their own base (forced negative
        // powers only), so the base of `powers` plus a memoised endpoint is an
        // endpoint of `prepared.transport(powers)`, with its coefficient.
        let mut memo_key = super::step_memo::MemoKey::new();
        memo_key.push(engine.uid as i64);
        memo_key.push(i64::from(presentation.0));
        memo_key.push(support as i64);
        memo_key.extend(powers.iter().map(|&p| p.min(0)));
        let expansion = super::step_memo::route(&memo_key, self.limits.exact_algebra, || {
            let numerator: Vec<i64> = powers.iter().map(|&p| p.min(0)).collect();
            let mut base = Vec::new();
            prepared.layered_base_powers(&numerator, &mut base)?;
            let numerator = IntegralKey::try_from_preallocated(numerator)
                .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
            let mapped = prepared.transport(&numerator, limits)?;
            let mut offsets = Vec::with_capacity(mapped.terms().len() * base.len());
            let mut coefficients = Vec::with_capacity(mapped.terms().len());
            for endpoint in mapped.terms() {
                offsets.extend(
                    endpoint
                        .key()
                        .powers()
                        .iter()
                        .zip(&base)
                        .map(|(power, base)| power - base),
                );
                coefficients.push(endpoint.coefficient().clone());
            }
            Ok((base.len(), offsets, coefficients))
        })?;
        let Some(expansion) = expansion else {
            direct(&mut step.terms)?;
            return Ok(step);
        };
        let mut base = Vec::with_capacity(engine.width);
        prepared.layered_base_powers(powers, &mut base)?;
        if base.len() != expansion.arity {
            return Err(PError::MalformedLibrary(
                "a memoised route expansion of another arity".to_owned(),
            ));
        }
        let mut target: SmallVec<[i64; 24]> = SmallVec::new();
        super::step_memo::coefficients(|values| -> Result<(), PError> {
            for (relative, &coefficient) in expansion
                .offsets
                .chunks(expansion.arity.max(1))
                .zip(&expansion.coefficients)
            {
                target.clear();
                target.extend(
                    base.iter()
                        .zip(relative)
                        .map(|(base, &relative)| base + i64::from(relative)),
                );
                let child = child(&target)?;
                merge(
                    context,
                    &self.limits,
                    &mut step.terms,
                    child,
                    values[coefficient as usize].clone(),
                )?;
            }
            Ok(())
        })?;
        if mode == super::step_memo::MemoMode::Check {
            let mut reference = BTreeMap::new();
            direct(&mut reference)?;
            if reference != step.terms {
                return Err(super::step_memo::mismatch("route expansion"));
            }
        }
        Ok(step)
    }

    fn apply_step(
        &self,
        class: OwnerClassId,
        view: &PView,
    ) -> Result<OrderedStep<PKey>, PError> {
        let slot = self.engine.owner(class)?;
        let slots = view.slots();
        let (powers, tag_slots) = self.engine.split(slot.presentation, &slots)?;
        if self.engine.shape(slot.presentation)?.support(powers) != slot.support {
            return Err(PError::MalformedLibrary(format!(
                "Apply key {powers:?} outside the support {:#x} of its owner",
                slot.support
            )));
        }
        let owner = self.owner(class)?;
        let lane = match owner.program(self.library)? {
            OwnerProgram::Lane(lane) => lane,
            OwnerProgram::Missing { ledger } => {
                return Err(PError::MissingOwner {
                    class,
                    ledger: *ledger,
                });
            }
        };
        let tag = GammaTag::decode_dense(tag_slots)?;
        let key = PKeyView {
            presentation: slot.presentation,
            powers,
            tag: &tag,
        };
        let output = lane.step(&self.context(owner), &key)?;
        self.ordered_step(output)
    }

    /// The Apply key of `child`: its sector must be an owner sector.
    fn apply_child(&self, child: &PChild) -> Result<PKey, PError> {
        let support = self
            .engine
            .shape(child.presentation)?
            .support(&child.powers);
        let class = self
            .engine
            .owner_sectors
            .get(&(child.presentation, support))
            .ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "an Apply child on sector {support:#x} of presentation {}, no owner",
                    child.presentation.0
                ))
            })?;
        Ok(PKey::Apply {
            class: *class,
            view: self.engine.view(&child.powers, &child.tag)?,
        })
    }

    fn ordered_step(&self, output: PStepOut) -> Result<OrderedStep<PKey>, PError> {
        let context = self.library.coefficient_context();
        let mut step = OrderedStep::default();
        for child in output.children {
            if !context.contains(&child.coefficient) {
                return Err(PError::ContextMismatch);
            }
            let arity = self.engine.shape(child.presentation)?.arity;
            if child.powers.len() != arity {
                return Err(PError::MalformedLibrary(format!(
                    "a child of presentation {} has {} powers, not {arity}",
                    child.presentation.0,
                    child.powers.len()
                )));
            }
            let key = match child.stage {
                PStage::Route => {
                    self.engine
                        .route_key(child.presentation, &child.powers, &child.tag)?
                }
                PStage::Apply => self.apply_child(&child)?,
            };
            merge(
                context,
                &self.limits,
                &mut step.terms,
                key,
                child.coefficient,
            )?;
            super::step_memo::put_powers(child.powers);
        }
        for (terminal, tag, coefficient) in output.terminals {
            if !context.contains(&coefficient) {
                return Err(PError::ContextMismatch);
            }
            if terminal.0 as usize >= self.library.terminals().len() {
                return Err(PError::MalformedLibrary(format!(
                    "a step emits the unknown terminal {}",
                    terminal.0
                )));
            }
            let key = self.engine.terminal_key(terminal, &tag)?;
            merge(context, &self.limits, &mut step.terminals, key, coefficient)?;
        }
        for condition in output.conditions {
            if !context.contains(&condition) {
                return Err(PError::ContextMismatch);
            }
            if condition.is_zero() {
                return Err(PError::Contract(
                    "a step emits a nonzero condition that is identically zero".to_owned(),
                ));
            }
            if !step.nonzero_conditions.contains(&condition) {
                step.nonzero_conditions.push(condition);
            }
        }
        Ok(step)
    }

    fn shard(&self, key: &PKey) -> Option<OrderedStepShard> {
        let (stage, t, class) = match key {
            PKey::Route { presentation, view } => (
                OrderedStepStage::Route,
                self.route_t(*presentation, view).ok()?,
                u64::from(presentation.0),
            ),
            PKey::Apply { class, .. } => (
                OrderedStepStage::Apply,
                self.engine.owner(*class).ok()?.t,
                class.0,
            ),
        };
        Some(OrderedStepShard {
            stage,
            support: u16::try_from(t).ok()?,
            class,
        })
    }

    /// The key of a recorded identity of this program.
    fn recorded_key(&self, stage: NodeStage, view: IntegralKey) -> Result<PKey, PError> {
        let arity = |presentation| {
            self.engine
                .split(presentation, view.powers())
                .map_err(|_| failure("a recorded identity is not a key of this library"))
        };
        match stage {
            NodeStage::ClassRoute(class) => {
                let presentation = u32::try_from(class)
                    .ok()
                    .filter(|&id| (id as usize) < self.engine.shapes.len())
                    .map(PresentationId)
                    .ok_or_else(|| failure("a recorded Route identity names no presentation"))?;
                arity(presentation)?;
                Ok(PKey::Route {
                    presentation,
                    view: PView::from_integral(view),
                })
            }
            NodeStage::ClassApply(class) => {
                let class = OwnerClassId(class);
                let slot = self
                    .engine
                    .owners
                    .get(&class)
                    .ok_or_else(|| failure("a recorded Apply identity names no owner"))?;
                arity(slot.presentation)?;
                Ok(PKey::Apply {
                    class,
                    view: PView::from_integral(view),
                })
            }
            NodeStage::Integral | NodeStage::Route | NodeStage::Apply(_) => {
                Err(failure("a recorded identity is not a p-library identity").into())
            }
        }
    }
}

impl OrderedStepper for PStepper<'_> {
    type Key = PKey;
    type Priority = PPriority;
    type Error = PError;
    type Account = PAccount;

    fn position(&self, key: &PKey) -> Result<OrderedPosition<PPriority>, PError> {
        Ok(OrderedPosition {
            phase: self.phase(key)?,
            priority: self.priority(key)?,
        })
    }

    fn step(&self, key: &PKey) -> (Result<OrderedStep<PKey>, PError>, PAccount) {
        let started = self.census.on.then(std::time::Instant::now);
        let (result, mut account) = match key {
            PKey::Route { presentation, view } => {
                (self.route_step(*presentation, view), PAccount::default())
            }
            PKey::Apply { class, view } => {
                if self.census.on {
                    super::custom::take_rule_hit();
                }
                let result = self.apply_step(*class, view);
                let account = PAccount {
                    rule_applications: usize::from(result.is_ok()),
                    census: None,
                };
                (result, account)
            }
        };
        if let (Some(started), Ok(step)) = (started, &result) {
            let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let mut record = self.census_record(key, step, nanos);
            if matches!(key, PKey::Apply { .. }) {
                record.rule = super::custom::take_rule_hit();
            }
            account.census = Some(Box::new(record));
        }
        (result, account)
    }

    fn later_phase(&self, parent: &PKey, child: &PKey) -> bool {
        matches!(
            (self.phase(parent), self.phase(child)),
            (Ok(parent), Ok(child)) if child > parent
        )
    }
}

/// The adapter of one request.
struct PKernel<'l, 'r> {
    stepper: PStepper<'l>,
    inspector: Option<&'r mut dyn PInspector>,
}

impl PKernel<'_, '_> {
    /// The path census of the request, when it is on.
    fn census(&mut self) -> Option<&mut PPathCensus> {
        if !self.stepper.census.on {
            return None;
        }
        self.inspector
            .as_deref_mut()
            .and_then(PInspector::path_census)
    }

    fn account(
        &mut self,
        account: PAccount,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), PError> {
        if let Some(record) = account.census {
            let class = (record.key.1 == 1).then_some(record.key.2);
            let stepper = self.stepper;
            if let Some(census) = self.census() {
                if let Some(class) = class
                    && !census.owners.contains_key(&class)
                    && let Some(owner) = stepper.census_owner(class)
                {
                    census.owners.insert(class, owner);
                }
                census.stepped(*record);
            }
        }
        let limits = self.stepper.limits;
        request.record_rule_applications(
            account.rule_applications,
            limits.diagnostic_limit(limits.max_rule_applications),
        )?;
        work.record_rule_applications(account.rule_applications);
        Ok(())
    }
}

impl OrderedKernel for PKernel<'_, '_> {
    type Key = PKey;
    type Priority = PPriority;
    type Error = PError;

    fn context(&self) -> &CoefficientContext {
        self.stepper.library.coefficient_context()
    }

    fn limits(&self) -> ReductionLimits {
        self.stepper.limits
    }

    fn phase_count(&self) -> usize {
        self.stepper.engine.phases.count()
    }

    fn position(&mut self, key: &PKey) -> Result<OrderedPosition<PPriority>, PError> {
        OrderedStepper::position(&self.stepper, key)
    }

    fn step(
        &mut self,
        key: &PKey,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<PKey>, PError> {
        let (result, account) = OrderedStepper::step(&self.stepper, key);
        self.account(account, request, work)?;
        result
    }

    fn key_heap_bytes(key: &PKey) -> usize {
        match key {
            PKey::Route { view, .. } | PKey::Apply { view, .. } => view.packed.heap_bytes(),
        }
    }

    fn priority_heap_bytes(priority: &PPriority) -> [usize; 2] {
        use crate::reduction::ordered_reduction::memstats::block;
        match &priority.order {
            POrderKey::Lexicographic(entries) if entries.spilled() => {
                [block(entries.as_ptr(), entries.capacity()), 0]
            }
            POrderKey::Lexicographic(_) => [0, 0],
            POrderKey::Complexity(key) => [0, key.shared_heap_bytes()],
        }
    }

    fn key_view(key: &PKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some(match key {
            PKey::Route { presentation, view } => (
                OrderedKeyStage::ClassRoute {
                    class: u64::from(presentation.0),
                },
                view.integral(),
            ),
            PKey::Apply { class, view } => (
                OrderedKeyStage::ClassApply { class: class.0 },
                view.integral(),
            ),
        })
    }

    /// The drivers observe each popped key twice, before its step and after
    /// it (`processed_keys` then counts it); the census counts the second.
    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        current: Option<&PKey>,
    ) -> Result<(), PError> {
        if !self.stepper.census.on {
            return Ok(());
        }
        let Some(key) = current else {
            return Ok(());
        };
        let processed = progress.processed_keys as u64;
        if self
            .census()
            .is_none_or(|census| processed != census.popped + 1)
        {
            return Ok(());
        }
        let phase = self.stepper.phase(key)?;
        let (kind, id) = PStepper::census_id(key);
        let hash = PStepper::census_hash(key);
        match self.census() {
            Some(census) => census.popped(phase, kind, id, hash),
            None => Ok(()),
        }
    }

    fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), PError> {
        if self.stepper.census.on {
            let stepper = self.stepper;
            let started = event.boundary == OrderedPhaseBoundary::Started;
            let inflow = started.then(|| stepper.census_inflow(&event));
            if let Some(census) = self.census() {
                census.phase_boundary(event.phase, started, event.progress);
                if let Some((inflow, profile, later)) = inflow {
                    census.phase_inflow(event.phase, inflow, profile, later);
                }
            }
        }
        let Some(inspector) = self.inspector.as_deref_mut() else {
            return Ok(());
        };
        let descriptor = self.stepper.engine.phases.describe(event.phase);
        inspector.inspect(&event.with_descriptor(descriptor))
    }
}

impl ConcurrentOrderedKernel for PKernel<'_, '_> {
    type Account = PAccount;
    type Stepper<'s>
        = PStepper<'s>
    where
        Self: 's;

    fn stepper(&self) -> Option<PStepper<'_>> {
        Some(self.stepper)
    }

    fn absorb(
        &mut self,
        _: &PKey,
        account: PAccount,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), PError> {
        self.account(account, request, work)
    }
}

impl OrderedStepCacheKernel for PKernel<'_, '_> {
    fn step_cache_shard(&mut self, key: &PKey) -> Option<OrderedStepShard> {
        self.stepper.shard(key)
    }

    fn step_cache_hit(&mut self, _key: &PKey) -> Result<(), PError> {
        if let Some(census) = self.census() {
            census.cache_hit();
        }
        Ok(())
    }

    fn step_cache_key(&self, stage: NodeStage, view: IntegralKey) -> Result<PKey, PError> {
        self.stepper.recorded_key(stage, view)
    }
}

impl ConcurrentStepCacheKernel for PKernel<'_, '_> {
    fn stepper_shard<'s>(stepper: &PStepper<'s>, key: &PKey) -> Option<OrderedStepShard>
    where
        Self: 's,
    {
        stepper.shard(key)
    }

    fn stepper_key<'s>(
        stepper: &PStepper<'s>,
        stage: NodeStage,
        view: IntegralKey,
    ) -> Result<PKey, PError>
    where
        Self: 's,
    {
        stepper.recorded_key(stage, view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagator::census::{PCensus, PClassRecord};
    use crate::propagator::class::OwnerClassKey;
    use crate::propagator::lane::{LaneRoles, OwnerLane};
    use crate::propagator::library::POwnerSpec;
    use crate::propagator::presentation::{PresentationKind, Slot, SlotLine};

    const BUBBLE: PresentationId = PresentationId(1);

    fn presentation(
        context: &CoefficientContext,
        id: u32,
        loops: u8,
        momenta: &[&[i64]],
    ) -> Presentation {
        let slots = momenta
            .iter()
            .map(|momentum| {
                Slot::Line(SlotLine {
                    momentum: momentum.to_vec(),
                    eps_units: 0,
                })
            })
            .collect();
        Presentation::from_slots(
            PresentationId(id),
            PresentationKind::EpsNode,
            &format!("presentation {id}"),
            loops,
            slots,
            context,
            None,
        )
        .unwrap()
    }

    /// A kite and a bubble with a missing owner on its full sector, and no
    /// terminal but `Unit`. Keys have the 21 slots of the kite: a view of the
    /// bubble ends in 3 slots of padding, a terminal key in 2.
    fn library() -> (PLibrary, OwnerClassId) {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut census = PCensus::default();
        let class = census
            .insert_class(PClassRecord {
                key: OwnerClassKey {
                    loops: 1,
                    lines: 2,
                    eps_lines: 0,
                    canonical: Arc::from(&b"bubble"[..]),
                },
                owner: (BUBBLE, 0b11),
                node: None,
                nodes: Vec::new(),
                t: 2,
                eps_lines: 0,
            })
            .unwrap();
        let mut builder = PLibrary::builder(context.clone(), 1..=2);
        builder
            .add_presentation(presentation(
                &context,
                0,
                2,
                &[
                    &[1, 0, 0],
                    &[0, 1, 0],
                    &[1, 0, -1],
                    &[0, 1, -1],
                    &[1, -1, 0],
                ],
            ))
            .unwrap();
        builder
            .add_presentation(presentation(&context, 1, 1, &[&[1, 0], &[1, -1]]))
            .unwrap();
        builder.set_census(census).unwrap();
        builder
            .add_owner_with_program(
                POwnerSpec {
                    class,
                    lane: OwnerLane::Custom,
                    presentation: BUBBLE,
                    node: None,
                    roles: LaneRoles::None,
                    edge_maps: Vec::new(),
                    payload: None,
                    custom: None,
                    terminals: Vec::new(),
                },
                OwnerProgram::Missing { ledger: 0 },
            )
            .unwrap();
        (builder.build().unwrap(), class)
    }

    /// `key` one slot short, and with a nonzero last slot.
    fn damaged(key: &IntegralKey) -> [IntegralKey; 2] {
        let mut padded = key.powers().to_vec();
        *padded.last_mut().unwrap() = 1;
        [
            IntegralKey::try_new(key.powers()[1..].iter().copied()).unwrap(),
            IntegralKey::try_new(padded).unwrap(),
        ]
    }

    #[test]
    fn a_view_outside_the_layout_of_the_library_is_refused() {
        let (library, class) = library();
        let engine = PEngine::new(&library).unwrap();
        let tag = GammaTag::from_pairs([(1, 2)]).unwrap();
        let packed = engine.view(&[1, 1], &tag).unwrap();
        let view = packed.integral().clone();
        assert_eq!(view.powers().len(), 21);
        let (powers, tag_slots) = engine.split(BUBBLE, view.powers()).unwrap();
        assert_eq!(powers, [1, 1]);
        assert_eq!(GammaTag::decode_dense(tag_slots).unwrap(), tag);
        for damaged in damaged(&view) {
            assert!(matches!(
                engine.split(BUBBLE, damaged.powers()),
                Err(PError::MalformedLibrary(_))
            ));
        }
        // And so is the identity of a record, whichever stage it names.
        let stepper = PStepper {
            library: &library,
            engine: &engine,
            limits: ReductionLimits::default(),
            census: PCensusMode::OFF,
        };
        assert_eq!(
            stepper.recorded_key(NodeStage::ClassRoute(1), view.clone()),
            Ok(PKey::Route {
                presentation: BUBBLE,
                view: packed.clone()
            })
        );
        assert_eq!(
            stepper.recorded_key(NodeStage::ClassApply(class.0), view.clone()),
            Ok(PKey::Apply {
                class,
                view: packed.clone()
            })
        );
        let stale = |result: Result<PKey, PError>| {
            matches!(
                result,
                Err(PError::Engine(ReductionError::OrderedStepCache { .. }))
            )
        };
        for damaged in damaged(&view) {
            for stage in [NodeStage::ClassRoute(1), NodeStage::ClassApply(class.0)] {
                assert!(stale(stepper.recorded_key(stage, damaged.clone())));
            }
        }
        for stage in [
            NodeStage::ClassRoute(2),
            NodeStage::ClassApply(class.0 ^ 1),
            NodeStage::Integral,
            NodeStage::Route,
            NodeStage::Apply(0b11),
        ] {
            assert!(stale(stepper.recorded_key(stage, view.clone())));
        }
    }

    #[test]
    fn a_terminal_key_of_a_record_is_checked_like_a_new_one() {
        let (library, _) = library();
        let engine = PEngine::new(&library).unwrap();
        let tag = GammaTag::from_pairs([(-1, 3)]).unwrap();
        let unit = engine.terminal_key(PTerminalId(0), &tag).unwrap();
        assert_eq!(
            engine.decode_terminal(&unit),
            Ok((PTerminalId(0), tag.clone()))
        );
        let unknown = engine.terminal_key(PTerminalId(1), &tag).unwrap();
        let [short, padded] = damaged(&unit);
        for key in [unknown, short, padded] {
            assert!(
                matches!(
                    engine.decode_terminal(&key),
                    Err(PError::MalformedLibrary(_))
                ),
                "{key:?}"
            );
        }
    }

    /// Reachable only from a record: a step never writes such a key.
    #[test]
    fn an_apply_key_off_the_sector_of_its_owner_is_refused() {
        let (library, class) = library();
        let engine = PEngine::new(&library).unwrap();
        let stepper = PStepper {
            library: &library,
            engine: &engine,
            limits: ReductionLimits::default(),
            census: PCensusMode::OFF,
        };
        let inside = engine.view(&[1, 1], &GammaTag::one()).unwrap();
        assert!(matches!(
            stepper.apply_step(class, &inside),
            Err(PError::MissingOwner { .. })
        ));
        let pinched = engine.view(&[1, 0], &GammaTag::one()).unwrap();
        assert!(matches!(
            stepper.apply_step(class, &pinched),
            Err(PError::MalformedLibrary(_))
        ));
    }
}
