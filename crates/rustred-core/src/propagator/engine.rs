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
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
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
    OrderedOutput, OrderedPhaseBoundary, OrderedPhaseDescriptor, OrderedPhaseEvent,
    OrderedPosition, OrderedProfile, OrderedStep, OrderedStepper,
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
use super::lane::{LaneRoles, OwnerLane};
use super::library::{OwnerProgram, PInspector, PLibrary, POwner};
use super::orbit::{OwnerOrbit, owner_orbit, routed_orbit};
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
    #[inline(always)]
    pub(crate) fn slots(&self) -> ViewSlots {
        let mut slots = ViewSlots::new();
        self.packed.decode_into(&mut slots);
        slots
    }

    /// fwd-opt: the slots decoded into the caller's `out` (no 520-byte
    /// `ViewSlots` move per call, which the hot paths paid as a `memmove`).
    #[inline(always)]
    pub(crate) fn slots_into(&self, out: &mut ViewSlots) {
        self.packed.decode_into(out);
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
        let mut slots = ViewSlots::new();
        self.slots_into(&mut slots);
        slots.as_slice().hash(state);
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

/// fwd-opt: one once-per-process setup of the engine or the library that the
/// steps of a request will need ([`PEngine::warm_tasks`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WarmTask {
    /// The owner's lane program ([`POwner::program`]).
    Program(OwnerClassId),
    /// The automorphisms of the owner's sector.
    OwnerOrbit(OwnerClassId),
    /// The compiled route of a routed sector (and its automorphisms when
    /// Route keys are canonicalised).
    Route(PresentationId, u64),
    /// The automorphisms of a presentation's whole line set.
    PresentationOrbit(PresentationId),
}

/// `RUSTRED_P_WARM_LEVELS=n` (fwd-opt, default 2): setup tasks only for
/// sectors with at least `top - n` lines (`top`: the most lines of a
/// reached presentation). The first phases have few keys and wait for their
/// setup one step at a time; later phases have enough parallel steps to
/// hide theirs, where tasks for sectors no step needs would only compete
/// (measured at 16 threads on NO3 L4_d2_r8: every level warmed cost +220 MB
/// and slowed the scheduler; 3 levels cost +11..+25 % RSS on NO1-NO3, 2
/// levels +1..+10 % for about the same time).
fn warm_levels() -> usize {
    static LEVELS: OnceLock<usize> = OnceLock::new();
    *LEVELS.get_or_init(|| {
        std::env::var("RUSTRED_P_WARM_LEVELS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2)
    })
}

/// `RUSTRED_P_WARM_MAX=n` (fwd-opt): at most `n` setup tasks per request
/// (default 4096; the most lines first).
fn warm_max() -> usize {
    static MAX: OnceLock<usize> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("RUSTRED_P_WARM_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4096)
    })
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
/// A lazily searched and authenticated automorphism group (M5 R0 A2).
type OrbitCell = OnceLock<Result<Arc<OwnerOrbit>, PError>>;

/// fwd-opt: a fast deterministic hasher for the engine's sector and owner
/// tables (looked up several times per child of every step; the tables are
/// only read by key, never iterated in order).
#[derive(Clone, Copy, Default)]
pub(crate) struct EngineHash;

pub(crate) struct EngineHasher(u64);

impl std::hash::Hasher for EngineHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        // A final avalanche: the table uses the top and the low bits.
        let x = self.0;
        (x ^ (x >> 29)).wrapping_mul(0xbf58_476d_1ce4_e5b9) ^ (x >> 32)
    }
}

impl std::hash::BuildHasher for EngineHash {
    type Hasher = EngineHasher;
    #[inline]
    fn build_hasher(&self) -> EngineHasher {
        EngineHasher(0)
    }
}

type FastMap<K, V> = std::collections::HashMap<K, V, EngineHash>;

pub(crate) struct PEngine {
    shapes: Vec<Shape>,
    /// Slots of every view and terminal key of this library.
    width: usize,
    /// Terminals of this library: ids at or above it name nothing.
    terminals: usize,
    phases: PhasePlan,
    owners: FastMap<OwnerClassId, OwnerSlot>,
    owner_sectors: FastMap<(PresentationId, u64), OwnerClassId>,
    /// fwd-opt: the census fate of every sector, indexed (`None`: zero;
    /// `Some(i)`: routed onto `routed[i]`), so that the per-child sector
    /// tests need no ordered-map search.
    fates: FastMap<(PresentationId, u64), Option<u32>>,
    routed: Vec<(OwnerClassId, SignedAffineLoopMap)>,
    routes: FastMap<(PresentationId, u64), OnceLock<Result<Prepared, PError>>>,
    /// The verified automorphisms of each owner sector, found on the owner's
    /// first canonicalised key (M5 R0 A2).
    orbits: FastMap<OwnerClassId, OrbitCell>,
    /// The same for every routed sector, carried over from its owner.
    route_orbits: FastMap<(PresentationId, u64), OrbitCell>,
    /// The verified automorphisms of the whole line set of each presentation
    /// (indexed by presentation id): they move a Route key across the
    /// subsectors of its presentation, as the vacuum reducer's one family
    /// group does for every key.
    presentation_orbits: Vec<OrbitCell>,
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
        let mut owners = FastMap::default();
        let mut owner_sectors = FastMap::default();
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
        let orbits = owners
            .keys()
            .map(|&class| (class, OnceLock::new()))
            .collect();
        let route_orbits = census
            .sectors
            .iter()
            .filter(|(sector, fate)| {
                matches!(fate, PSectorFate::Owner { .. }) && !owner_sectors.contains_key(*sector)
            })
            .map(|(&sector, _)| (sector, OnceLock::new()))
            .collect();
        let presentation_orbits = shapes.iter().map(|_| OnceLock::new()).collect();
        let mut fates = FastMap::default();
        let mut routed = Vec::new();
        for (&sector, fate) in &census.sectors {
            let fate = match fate {
                PSectorFate::Zero => None,
                PSectorFate::Owner { class, map } => {
                    routed.push((*class, map.clone()));
                    Some(u32::try_from(routed.len() - 1).map_err(|_| {
                        PError::MalformedLibrary("too many routed sectors".to_owned())
                    })?)
                }
            };
            fates.insert(sector, fate);
        }
        Ok(Self {
            orbits,
            route_orbits,
            fates,
            routed,
            presentation_orbits,
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
        slots.extend(powers.iter().copied());
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
        &'e self,
        _library: &PLibrary,
        presentation: PresentationId,
        support: u64,
    ) -> Result<Sector<'e>, PError> {
        let sector = (presentation, support);
        if let Some(&class) = self.owner_sectors.get(&sector) {
            return Ok(Sector::Owner(class));
        }
        match self.fates.get(&sector) {
            Some(None) => Ok(Sector::Zero),
            Some(Some(index)) => {
                let (class, map) = &self.routed[*index as usize];
                Ok(Sector::Routed { class: *class, map })
            }
            None => Err(PError::MalformedLibrary(format!(
                "sector {support:#x} of presentation {} is not in the census",
                presentation.0
            ))),
        }
    }

    /// The weighted Route keys of `inputs`, checked one by one: a cancelled
    /// input is checked like every other.
    #[allow(clippy::mutable_key_type)] // PKey views cache their IntegralKey lazily; Hash/Ord read only the packed powers (rr-alloc)
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

    /// fwd-opt: the setup tasks a request on `inputs` may need, the most
    /// lines first: the owners and routed sectors of the presentations
    /// reachable from the inputs' presentations through the census routes
    /// (an over-approximation; a task no step needs only costs time on an
    /// otherwise idle worker), with orbit tasks only when keys are
    /// canonicalised.
    pub(crate) fn warm_tasks(
        &self,
        inputs: impl Iterator<Item = PresentationId>,
        canonical: bool,
        canonical_routes: bool,
    ) -> Vec<WarmTask> {
        let mut reached: std::collections::BTreeSet<PresentationId> = inputs.collect();
        loop {
            let before = reached.len();
            for (&(presentation, _), fate) in &self.fates {
                if let Some(at) = fate
                    && reached.contains(&presentation)
                    && let Some((class, _)) = self.routed.get(*at as usize)
                    && let Ok(slot) = self.owner(*class)
                {
                    reached.insert(slot.presentation);
                }
            }
            if reached.len() == before {
                break;
            }
        }
        let mut tasks: Vec<(usize, WarmTask)> = Vec::new();
        for (&(presentation, _), &class) in &self.owner_sectors {
            if !reached.contains(&presentation) {
                continue;
            }
            let Ok(slot) = self.owner(class) else {
                continue;
            };
            tasks.push((slot.t, WarmTask::Program(class)));
            if canonical {
                tasks.push((slot.t, WarmTask::OwnerOrbit(class)));
            }
        }
        for (&(presentation, support), fate) in &self.fates {
            if fate.is_some()
                && reached.contains(&presentation)
                && let Ok(shape) = self.shape(presentation)
            {
                tasks.push((
                    shape.line_count(support),
                    WarmTask::Route(presentation, support),
                ));
            }
        }
        if canonical_routes {
            for &presentation in &reached {
                if let Ok(shape) = self.shape(presentation) {
                    tasks.push((
                        shape.line_count(shape.lines),
                        WarmTask::PresentationOrbit(presentation),
                    ));
                }
            }
        }
        // The most lines first (the order the phases need them), then a fixed
        // order (hash-map iteration order is not one).
        tasks.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let top = tasks.first().map_or(0, |task| task.0);
        let floor = top.saturating_sub(warm_levels());
        tasks.retain(|task| task.0 >= floor);
        tasks.truncate(warm_max());
        tasks.into_iter().map(|(_, task)| task).collect()
    }

    /// Run one request; the terminal keys of the output are [`PTerminalCodec`]
    /// keys.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::mutable_key_type)] // PKey views cache their IntegralKey lazily; Hash/Ord read only the packed powers (rr-alloc)
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
        // p requests run the p profile (M5 batch 2) unless the caller chose one.
        let scheduler = scheduler.resolve_profile(OrderedProfile::P);
        let census = match inspector.as_deref_mut().and_then(PInspector::path_census) {
            Some(census) => PCensusMode {
                on: true,
                keep_apply_keys: census.options.keep_apply_keys,
            },
            None => PCensusMode::OFF,
        };
        let shared_memo = super::step_memo::begin(scheduler.threads);
        // The same process-wide snapshot appends the pruning marker to the
        // library's step identity, so persistent pruned/unpruned records cannot
        // be attached or reopened across modes.
        let zero_pruning =
            super::library::prune_zero_children_enabled().then(ZeroPruningCounters::default);
        let canonical = library.orbit_canonicalization();
        let canonical_routes = canonical && library.route_canonicalization();
        // fwd-opt: setup tasks for the workers of a multi-thread request.
        let warm: Vec<WarmTask> = if scheduler.threads > 1 {
            self.warm_tasks(
                initial.keys().filter_map(|key| match key {
                    PKey::Route { presentation, .. } => Some(*presentation),
                    PKey::Apply { class, .. } => {
                        self.owner(*class).ok().map(|slot| slot.presentation)
                    }
                }),
                canonical,
                canonical_routes,
            )
        } else {
            Vec::new()
        };
        let mut kernel = PKernel {
            stepper: PStepper {
                library,
                engine: self,
                limits,
                census,
                canonical: library.orbit_canonicalization(),
                canonical_routes: library.orbit_canonicalization()
                    && library.route_canonicalization(),
                prefer_high: library.orbit_prefer_high(),
                zero_pruning: zero_pruning.as_ref(),
                warm: &warm,
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
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(
                    file,
                    "{{\"insertion\":{{\"hits\":{ih},\"misses\":{im},\"entries\":{ie}}},\"route\":{{\"hits\":{rh},\"misses\":{rm},\"entries\":{re}}},\"coefficients\":{distinct}}}"
                );
            }
        }
        if let Some(counters) = zero_pruning
            .as_ref()
            .filter(|_| super::library::prune_zero_children_reported())
        {
            eprintln!(
                "rustred.p-zero-pruning.v1 canonical_candidates_before_parent_merge={} known_zero_candidates_pruned={} complete={}",
                counters.canonical_candidates.load(AtomicOrdering::Relaxed),
                counters.known_zero.load(AtomicOrdering::Relaxed),
                output.is_ok(),
            );
        }
        super::step_memo::end(shared_memo);
        canon_stats::flush();
        output
    }
}

/// fwd-opt: light steps (worker threads of a layered request only, switched
/// on per thread by `OrderedStepper::set_light`): a memoised route step hands
/// its singly met children over as (key, handle) pairs, handle = shared memo
/// coefficient index and sign, beside the materialised terms; the caller
/// takes them right after the step (`take_light`) and maps a handle to a
/// value only once per request (`light_value`). `RUSTRED_P_LIGHT_STEPS=off`
/// disables it. Value-neutral (the same terms, the same coefficients).
pub(crate) mod light {
    use std::cell::{Cell, RefCell};
    use std::sync::OnceLock;

    use super::PKey;

    thread_local! {
        static ON: Cell<bool> = const { Cell::new(false) };
        static OUT: RefCell<Vec<(PKey, u64)>> = const { RefCell::new(Vec::new()) };
    }

    fn allowed() -> bool {
        static ALLOWED: OnceLock<bool> = OnceLock::new();
        *ALLOWED.get_or_init(|| {
            !matches!(
                std::env::var("RUSTRED_P_LIGHT_STEPS").as_deref(),
                Ok("0" | "off" | "false")
            )
        })
    }

    pub(crate) fn set(on: bool) {
        ON.with(|cell| cell.set(on && allowed()));
    }

    pub(crate) fn enabled() -> bool {
        ON.with(Cell::get)
    }

    /// A handle: shared or this thread's (`local`) memo index, and sign.
    /// A local handle is valid only on the thread that made it.
    pub(crate) fn handle(index: u32, negative: bool, local: bool) -> u64 {
        (1 << 63) | (u64::from(negative) << 62) | (u64::from(local) << 61) | u64::from(index)
    }

    pub(crate) fn split(handle: u64) -> (u32, bool) {
        ((handle & 0xFFFF_FFFF) as u32, handle >> 62 & 1 == 1)
    }

    pub(crate) fn is_local(handle: u64) -> bool {
        handle >> 61 & 1 == 1
    }

    pub(super) fn put(children: Vec<(PKey, u64)>) {
        OUT.with(|out| *out.borrow_mut() = children);
    }

    pub(crate) fn clear() {
        OUT.with(|out| out.borrow_mut().clear());
    }

    pub(crate) fn take() -> Vec<(PKey, u64)> {
        OUT.with(|out| std::mem::take(&mut *out.borrow_mut()))
    }

    /// Light children of an Insertion step before canonicalisation: the
    /// child presentation, the absolute tag and (powers, handle) pairs.
    pub(crate) type Pending = (super::PresentationId, super::GammaTag, Vec<(Vec<i64>, u64)>);

    thread_local! {
        static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
    }

    /// fwd-opt: an Insertion step hands its memoised children over by handle.
    pub(crate) fn put_pending(pending: Pending) {
        PENDING.with(|cell| *cell.borrow_mut() = Some(pending));
    }

    pub(super) fn take_pending() -> Option<Pending> {
        PENDING.with(|cell| cell.borrow_mut().take())
    }

    /// The handle with its sign flipped.
    pub(crate) fn flip(handle: u64) -> u64 {
        handle ^ (1 << 62)
    }
}

/// fwd-opt: a per-thread, direct-mapped cache of canonical images. The
/// image of a key under its automorphisms (and the sign) is a pure function
/// of the engine, the presentation or owner class, the orientation and the
/// powers; a key reached from many parents (14 contributions per key on
/// NO3 L4_d2_r8) is canonicalised once per cache residency instead of once per
/// contribution. `RUSTRED_P_CANON_CACHE=off` disables it,
/// `RUSTRED_P_CANON_CACHE=<b>` sets 2^b entries per thread (default 2^13).
/// Errors are not cached (computed again). Value-neutral.
mod canon_cache {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use smallvec::SmallVec;

    use super::PError;

    pub(super) type Powers = SmallVec<[i64; 16]>;

    struct Entry {
        id: u64,
        powers: Powers,
        image: Option<(Powers, bool)>,
    }

    #[derive(Default)]
    struct Cache {
        engine: u64,
        flags: u8,
        entries: Vec<Option<Entry>>,
    }

    thread_local! {
        static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
    }

    /// The cache size of this thread: `RUSTRED_P_CANON_CACHE` when set;
    /// unset, 2^13 entries on the thread of a one-thread request and none on
    /// the workers of a multi-thread request (measured at 16 threads on NO3
    /// L4_d2_r8, batch 5: no instruction saved, +60 MB of caches; a worker
    /// sees about 1/16 of the keys).
    pub(super) fn bits() -> Option<u32> {
        static BITS: OnceLock<Option<Option<u32>>> = OnceLock::new();
        let set = *BITS.get_or_init(|| match std::env::var("RUSTRED_P_CANON_CACHE").as_deref() {
            Ok("0" | "off" | "false") => Some(None),
            Ok(value) => Some(
                value
                    .parse::<u32>()
                    .ok()
                    .filter(|b| (4..=24).contains(b))
                    .or(Some(13)),
            ),
            Err(_) => None,
        });
        match set {
            Some(bits) => bits,
            None => super::super::step_memo::single_thread().then_some(13),
        }
    }

    #[inline]
    fn hash(id: u64, powers: &[i64]) -> u64 {
        let mut h = id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for &power in powers {
            h = (h.rotate_left(5) ^ power as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        h ^ (h >> 31)
    }

    /// The canonical image of `powers` under `id` (a presentation, or an owner
    /// class with the top bit set), from the cache or `compute`.
    pub(super) fn lookup(
        engine: u64,
        id: u64,
        flags: u8,
        powers: &[i64],
        compute: impl FnOnce() -> Result<Option<(Vec<i64>, bool)>, PError>,
    ) -> Result<Option<(Powers, bool)>, PError> {
        let Some(bits) = bits() else {
            return Ok(compute()?.map(|(image, negative)| (Powers::from_vec(image), negative)));
        };
        let at = (hash(id, powers) >> (64 - bits)) as usize;
        let found = CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.engine != engine || cache.flags != flags || cache.entries.is_empty() {
                cache.engine = engine;
                cache.flags = flags;
                cache.entries.clear();
                cache.entries.resize_with(1 << bits, || None);
            }
            match &cache.entries[at] {
                Some(entry) if entry.id == id && entry.powers.as_slice() == powers => Some(
                    entry
                        .image
                        .as_ref()
                        .map(|(image, negative)| (image.clone(), *negative)),
                ),
                _ => None,
            }
        });
        if let Some(image) = found {
            return Ok(image);
        }
        let image = compute()?.map(|(image, negative)| (Powers::from_vec(image), negative));
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.engine == engine && cache.flags == flags && at < cache.entries.len() {
                cache.entries[at] = Some(Entry {
                    id,
                    powers: Powers::from_slice(powers),
                    image: image.clone(),
                });
            }
        });
        Ok(image)
    }
}

/// fwd-opt: a per-thread, direct-mapped cache of Apply key priorities (the
/// owner's order key: a pure function of the engine, the class and the
/// packed key). A key reached from many parents has its position computed
/// once per cache residency. Same switch as [`canon_cache`]
/// (`RUSTRED_P_CANON_CACHE`). Errors are not cached. Value-neutral.
mod order_cache {
    use std::cell::RefCell;
    use std::hash::BuildHasher;

    use super::{EngineHash, PError, PPriority};
    use crate::reduction::ordered_reduction::CompactPowers;

    struct Entry {
        class: u64,
        packed: CompactPowers,
        priority: PPriority,
    }

    #[derive(Default)]
    struct Cache {
        engine: u64,
        entries: Vec<Option<Entry>>,
    }

    thread_local! {
        static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
    }

    pub(super) fn lookup(
        engine: u64,
        class: u64,
        packed: &CompactPowers,
        compute: impl FnOnce() -> Result<PPriority, PError>,
    ) -> Result<PPriority, PError> {
        let Some(bits) = super::canon_cache::bits() else {
            return compute();
        };
        let hash = EngineHash.hash_one((class, packed));
        let at = (hash >> (64 - bits)) as usize;
        let found = CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.engine != engine || cache.entries.is_empty() {
                cache.engine = engine;
                cache.entries.clear();
                cache.entries.resize_with(1 << bits, || None);
            }
            match &cache.entries[at] {
                Some(entry) if entry.class == class && entry.packed == *packed => {
                    Some(entry.priority.clone())
                }
                _ => None,
            }
        });
        if let Some(priority) = found {
            return Ok(priority);
        }
        let priority = compute()?;
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.engine == engine && at < cache.entries.len() {
                cache.entries[at] = Some(Entry {
                    class,
                    packed: packed.clone(),
                    priority: priority.clone(),
                });
            }
        });
        Ok(priority)
    }
}

/// fwd-opt diagnostics: canonicalisation coverage of the stepped Apply keys
/// per owner class (`RUSTRED_P_CANON_STATS=<file>`, one JSON line per class
/// at the end of each request). Per class: keys, keys whose owner has a
/// nontrivial group, keys on which every action is exact (signed slot
/// permutation on the key: canonicalised), keys on which no action is exact
/// (escape canonicalisation entirely), keys with at least one inexact action
/// (an ISP with a nonzero power maps to a combination), keys whose line part
/// some inexact action would lower (candidate duplicates by symmetry), and
/// the number of actions.
pub(crate) mod canon_stats {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    static STATS: Mutex<BTreeMap<u64, [u64; 7]>> = Mutex::new(BTreeMap::new());

    pub(crate) fn enabled() -> bool {
        PATH.get_or_init(|| std::env::var_os("RUSTRED_P_CANON_STATS").map(Into::into))
            .is_some()
    }

    pub(crate) fn record(class: u64, actions: u64, exact: u64, inexact: u64, better: bool) {
        let mut stats = STATS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = stats.entry(class).or_default();
        entry[0] += 1;
        entry[1] += u64::from(actions > 0);
        entry[2] += u64::from(actions > 0 && inexact == 0);
        entry[3] += u64::from(actions > 0 && exact == 0);
        entry[4] += u64::from(inexact > 0);
        entry[5] += u64::from(better);
        entry[6] = actions;
    }

    pub(crate) fn flush() {
        use std::io::Write;
        let Some(Some(path)) = PATH.get() else {
            return;
        };
        let stats = std::mem::take(
            &mut *STATS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            for (
                class,
                [
                    keys,
                    nontrivial,
                    all_exact,
                    none_exact,
                    some_inexact,
                    better,
                    actions,
                ],
            ) in stats
            {
                let _ = writeln!(
                    file,
                    "{{\"class\":\"{class:016x}\",\"keys\":{keys},\"nontrivial\":{nontrivial},\"all_exact\":{all_exact},\"none_exact\":{none_exact},\"some_inexact\":{some_inexact},\"line_lowered\":{better},\"actions\":{actions}}}"
                );
            }
        }
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
    crate::prof_fine!(StepMerge);
    if coefficient.is_zero() {
        return Ok(());
    }
    match terms.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(coefficient);
        }
        Entry::Occupied(mut entry) => {
            let sum = crate::prof_wrap!(
                StepAdd,
                context.try_add(entry.get(), &coefficient, limits.exact_algebra)
            )
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

/// fwd-opt: the step-local merge of a whole child list at once: the same
/// map as `merge` applied pair by pair (exact sums are canonical; a key whose
/// sum is zero is absent), built by one sort and a bulk insert instead of a
/// B-tree insertion per child.
pub(super) fn merge_all<K: Ord>(
    context: &CoefficientContext,
    limits: &ReductionLimits,
    mut pairs: Vec<(K, Coefficient)>,
) -> Result<BTreeMap<K, Coefficient>, PError> {
    // `RUSTRED_P_SORTED_MERGE=off`: the pair-by-pair B-tree merge (control).
    static SORTED: OnceLock<bool> = OnceLock::new();
    if !*SORTED.get_or_init(|| {
        !matches!(
            std::env::var("RUSTRED_P_SORTED_MERGE").as_deref(),
            Ok("0" | "off" | "false")
        )
    }) {
        let mut terms = BTreeMap::new();
        for (key, coefficient) in pairs {
            merge(context, limits, &mut terms, key, coefficient)?;
        }
        return Ok(terms);
    }
    pairs.retain(|(_, coefficient)| !coefficient.is_zero());
    // Stable: equal keys keep their emission order (the order of the sums).
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut merged: Vec<(K, Coefficient)> = Vec::with_capacity(pairs.len());
    let mut zeros = false;
    for (key, coefficient) in pairs {
        match merged.last_mut() {
            Some((last, sum)) if (*last).cmp(&key) == std::cmp::Ordering::Equal => {
                *sum = context
                    .try_add(sum, &coefficient, limits.exact_algebra)
                    .map_err(algebra)?;
                zeros |= sum.is_zero();
            }
            _ => merged.push((key, coefficient)),
        }
    }
    if zeros {
        merged.retain(|(_, coefficient)| !coefficient.is_zero());
    }
    Ok(merged.into_iter().collect())
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

pub(crate) fn transport_of(
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
#[derive(Debug, Default)]
struct ZeroPruningCounters {
    // These are canonical candidates BEFORE per-parent merging, not the
    // coalesced emitted children counted by PCensusAgg::ch_zero.
    canonical_candidates: AtomicU64,
    known_zero: AtomicU64,
}

/// The shared, read-only part of the adapter: pure positions and steps.
#[derive(Clone, Copy)]
pub(crate) struct PStepper<'l> {
    library: &'l PLibrary,
    engine: &'l PEngine,
    limits: ReductionLimits,
    census: PCensusMode,
    /// Orbit canonicalisation of Apply keys ([`PLibrary::orbit_canonicalization`]).
    canonical: bool,
    /// And of the Route keys of routed sectors ([`PLibrary::route_canonicalization`]).
    canonical_routes: bool,
    /// The greatest exact image stands for an orbit (the default), else the
    /// least ([`PLibrary::set_orbit_prefer_high`]).
    prefer_high: bool,
    /// None keeps the historical path byte-for-byte; the opt-in counter is
    /// request-local and shared only by that request's detached steppers.
    zero_pruning: Option<&'l ZeroPruningCounters>,
    /// fwd-opt: the request's setup tasks ([`OrderedStepper::warm`]).
    warm: &'l [WarmTask],
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
        let mut slots = ViewSlots::new();
        view.slots_into(&mut slots);
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
        order_cache::lookup(self.engine.uid, class.0, &view.packed, || {
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
        })
    }

    #[allow(clippy::mutable_key_type)] // PKey views cache their IntegralKey lazily; Hash/Ord read only the packed powers (rr-alloc)
    fn route_step(
        &self,
        presentation: PresentationId,
        view: &PView,
    ) -> Result<OrderedStep<PKey>, PError> {
        crate::prof!(RouteStep);
        let engine = self.engine;
        let slots = view.slots();
        let (powers, tag_slots) = engine.split(presentation, &slots)?;
        let support = engine.shape(presentation)?.support(powers);
        let mut step = OrderedStep::default();
        let (class, map) = match engine.sector(self.library, presentation, support)? {
            Sector::Zero => return Ok(step),
            Sector::Owner(class) => {
                let one = self.library.coefficient_context().one();
                let (key, coefficient) = if self.canonical {
                    let tag = GammaTag::decode_dense(tag_slots)?;
                    self.apply_key(class, powers, &tag, one)?
                } else {
                    (
                        PKey::Apply {
                            class,
                            view: view.clone(),
                        },
                        one,
                    )
                };
                step.terms.insert(key, coefficient);
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
                crate::prof!(RouteCompile);
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
        let child =
            |powers: &[i64], coefficient: Coefficient| -> Result<(PKey, Coefficient), PError> {
                // A numerator can pinch a line of the owner: that is a subsector
                // of its presentation, routed by the census in a later phase.
                // With canonicalisation on (M5 R0 A2) the key is the least exact
                // image under the sector's automorphisms and the weight carries its
                // sign; the memoised expansion below is taken before this, so the
                // memo is independent of canonicalisation (M5 batch 3).
                if shape.support(powers) == owner.support {
                    self.apply_key(class, powers, &tag, coefficient)
                } else {
                    self.route_child(owner.presentation, powers, &tag, coefficient)
                }
            };
        let direct = |terms: &mut BTreeMap<PKey, Coefficient>| -> Result<(), PError> {
            crate::prof!(RouteTransport);
            let key = IntegralKey::try_from_preallocated(powers.to_vec())
                .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
            let mapped = prepared.transport(&key, limits)?;
            for endpoint in mapped.terms() {
                let (child, coefficient) =
                    child(endpoint.key().powers(), endpoint.coefficient().clone())?;
                if self.keep_canonical_child(&child, &coefficient)? {
                    merge(context, &self.limits, terms, child, coefficient)?;
                }
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
        #[cfg(feature = "fwd-profile")]
        let memo_scope = crate::prof::Scope::enter(crate::prof::Cat::MemoRoute);
        let expansion = super::step_memo::route(&memo_key, self.limits.exact_algebra, || {
            crate::prof!(RouteTransport);
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
        #[cfg(feature = "fwd-profile")]
        drop(memo_scope);
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
        if mode != super::step_memo::MemoMode::Check && light::enabled() {
            let local = matches!(expansion, super::step_memo::Held::Local(..));
            // fwd-opt light step: children of the shared memo carry a handle
            // (memo index, sign) instead of a materialised coefficient; only
            // children met twice are materialised and summed.
            let mut pairs: Vec<(PKey, u64)> = Vec::with_capacity(expansion.coefficients.len());
            for (relative, &index) in expansion
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
                let (child, negative) = if shape.support(&target) == owner.support {
                    self.apply_key_with(class, &target, &tag, false, |b| !b)?
                } else {
                    self.route_child_with(owner.presentation, &target, &tag, false, |b| !b)?
                };
                let keep = match self.zero_pruning {
                    Some(counters) => self.keep_canonical_key(&child, counters)?,
                    None => true,
                };
                if keep {
                    pairs.push((child, light::handle(index, negative, local)));
                }
            }
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            let mut single = Vec::with_capacity(pairs.len());
            let mut repeated: Vec<(PKey, Coefficient)> = Vec::new();
            super::step_memo::coefficients_of(&expansion, |values| {
                let mut at = 0;
                while at < pairs.len() {
                    let end = at
                        + pairs[at..]
                            .iter()
                            .take_while(|(key, _)| *key == pairs[at].0)
                            .count();
                    if end - at == 1 {
                        single.push(pairs[at].clone());
                    } else {
                        for (key, handle) in &pairs[at..end] {
                            let (index, negative) = light::split(*handle);
                            let value = values(index);
                            repeated.push((key.clone(), if negative { -value } else { value }));
                        }
                    }
                    at = end;
                }
            });
            step.terms = merge_all(context, &self.limits, repeated)?;
            light::put(single);
            return Ok(step);
        }
        let mut pairs: Vec<(PKey, Coefficient)> = Vec::with_capacity(expansion.coefficients.len());
        super::step_memo::coefficients_of(&expansion, |values| -> Result<(), PError> {
            crate::prof!(RouteChildren);
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
                let (child, coefficient) = child(&target, values(coefficient))?;
                if self.keep_canonical_child(&child, &coefficient)? {
                    pairs.push((child, coefficient));
                }
            }
            Ok(())
        })?;
        step.terms = merge_all(context, &self.limits, pairs)?;
        if mode == super::step_memo::MemoMode::Check {
            let mut reference = BTreeMap::new();
            direct(&mut reference)?;
            if reference != step.terms {
                return Err(super::step_memo::mismatch("route expansion"));
            }
        }
        Ok(step)
    }

    fn apply_step(&self, class: OwnerClassId, view: &PView) -> Result<OrderedStep<PKey>, PError> {
        crate::prof!(ApplyStep);
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
        if canon_stats::enabled() {
            self.canon_coverage(class, slot.presentation, powers);
        }
        let tag = GammaTag::decode_dense(tag_slots)?;
        let key = PKeyView {
            presentation: slot.presentation,
            powers,
            tag: &tag,
        };
        let output = {
            #[cfg(feature = "fwd-profile")]
            let _lane = {
                use crate::prof::Cat;
                crate::prof::Scope::enter(match lane.lane() {
                    OwnerLane::Insertion => Cat::LaneInsertion,
                    OwnerLane::SeriesParallel => Cat::LaneSeriesParallel,
                    OwnerLane::Triangle => Cat::LaneTriangle,
                    OwnerLane::Carpet => Cat::LaneCarpet,
                    OwnerLane::Diamond => Cat::LaneDiamond,
                    OwnerLane::Scheme => Cat::LaneScheme,
                    OwnerLane::Custom => Cat::LaneCustom,
                    _ => Cat::LaneOther,
                })
            };
            lane.step(&self.context(owner), &key)?
        };
        self.ordered_step(output)
    }

    /// fwd-opt diagnostics (`RUSTRED_P_CANON_STATS=<file>`): how the owner's
    /// automorphisms act on this stepped Apply key (see `canon_stats`).
    fn canon_coverage(&self, class: OwnerClassId, presentation: PresentationId, powers: &[i64]) {
        let Ok(orbit) = self.orbit(class) else {
            return;
        };
        let Ok(shape) = self.engine.shape(presentation) else {
            return;
        };
        let n = powers.len();
        let line = |slot: usize| shape.lines >> slot & 1 == 1;
        let (mut exact, mut inexact, mut better) = (0u64, 0u64, false);
        let own: Vec<i64> = (0..n)
            .map(|i| if line(i) { powers[i].max(0) } else { 0 })
            .collect();
        for action in &orbit.actions {
            if action.len() < n {
                continue;
            }
            let is_exact = powers
                .iter()
                .zip(action.iter())
                .all(|(&power, image)| power == 0 || image.is_some());
            if is_exact {
                exact += 1;
                continue;
            }
            inexact += 1;
            // The image of the line part (lines always map onto lines).
            let mut image = vec![0i64; n];
            for slot in (0..n).filter(|&slot| line(slot) && powers[slot] > 0) {
                if let Some((target, _)) = action[slot] {
                    image[usize::from(target)] = powers[slot];
                }
            }
            let below = if self.prefer_high {
                image > own
            } else {
                image < own
            };
            better |= below;
        }
        canon_stats::record(class.0, orbit.actions.len() as u64, exact, inexact, better);
    }

    /// The Apply key of `powers` with `tag` on the sector of `class`, and the
    /// weight `coefficient` carries onto it: with canonicalisation on, the
    /// least exact image of the key under the owner's automorphisms
    /// ([`super::orbit`]), the weight times its sign.
    fn apply_key(
        &self,
        class: OwnerClassId,
        powers: &[i64],
        tag: &GammaTag,
        coefficient: Coefficient,
    ) -> Result<(PKey, Coefficient), PError> {
        self.apply_key_with(class, powers, tag, coefficient, |c| -c)
    }

    /// `apply_key` for any weight with a sign flip (fwd-opt: a bare sign).
    fn apply_key_with<C>(
        &self,
        class: OwnerClassId,
        powers: &[i64],
        tag: &GammaTag,
        coefficient: C,
        negate: fn(C) -> C,
    ) -> Result<(PKey, C), PError> {
        crate::prof_fine!(CanonApply);
        if self.canonical
            && let Some((image, negative)) = canon_cache::lookup(
                self.engine.uid,
                (1 << 63) | (class.0 & !(1 << 63)),
                u8::from(self.prefer_high),
                powers,
                || self.canonical_powers(class, powers),
            )?
        {
            // A sign flip of a coefficient the library already sealed: same
            // variables, degrees and sizes, so no new authentication (M5 R0 A2).
            let coefficient = if negative {
                negate(coefficient)
            } else {
                coefficient
            };
            return Ok((
                PKey::Apply {
                    class,
                    view: crate::prof_wrap_fine!(CanonKey, self.engine.view(&image, tag))?,
                },
                coefficient,
            ));
        }
        Ok((
            PKey::Apply {
                class,
                view: crate::prof_wrap_fine!(CanonKey, self.engine.view(powers, tag))?,
            },
            coefficient,
        ))
    }

    /// The Route key of `powers` with `tag` on `presentation`, and the weight
    /// `coefficient` carries onto it: with canonicalisation on and on a routed
    /// sector, the least exact image of the key under the sector's
    /// automorphisms ([`super::orbit::routed_orbit`]), the weight times its
    /// sign. A Route key has no order inside its phase, and every image has
    /// the same lines, so any image may stand for the key.
    fn route_child(
        &self,
        presentation: PresentationId,
        powers: &[i64],
        tag: &GammaTag,
        coefficient: Coefficient,
    ) -> Result<(PKey, Coefficient), PError> {
        self.route_child_with(presentation, powers, tag, coefficient, |c| -c)
    }

    /// `route_child` for any weight with a sign flip (fwd-opt: a bare sign).
    fn route_child_with<C>(
        &self,
        presentation: PresentationId,
        powers: &[i64],
        tag: &GammaTag,
        coefficient: C,
        negate: fn(C) -> C,
    ) -> Result<(PKey, C), PError> {
        crate::prof_fine!(CanonRoute);
        if self.canonical_routes {
            // First across the subsectors of the presentation, then inside the
            // sector reached (its own automorphisms can be more than the
            // presentation's); each step is exact, the signs compose.
            let image = canon_cache::lookup(
                self.engine.uid,
                u64::from(presentation.0),
                u8::from(self.prefer_high),
                powers,
                || {
                    let across = self.canonical_in_presentation(presentation, powers)?;
                    let base = across
                        .as_ref()
                        .map_or(powers, |(image, _)| image.as_slice());
                    let inside = self.canonical_route(presentation, base)?;
                    Ok(match (across, inside) {
                        (None, None) => None,
                        (Some(across), None) => Some(across),
                        (None, Some(inside)) => Some(inside),
                        (Some((_, first)), Some((image, second))) => Some((image, first != second)),
                    })
                },
            )?;
            if let Some((image, negative)) = image {
                // A sign flip of a coefficient the library already sealed: same
                // variables, degrees and sizes, so no new authentication (M5 R0 A2).
                let coefficient = if negative {
                    negate(coefficient)
                } else {
                    coefficient
                };
                return Ok((
                    crate::prof_wrap_fine!(
                        CanonKey,
                        self.engine.route_key(presentation, &image, tag)
                    )?,
                    coefficient,
                ));
            }
        }
        Ok((
            crate::prof_wrap_fine!(CanonKey, self.engine.route_key(presentation, powers, tag))?,
            coefficient,
        ))
    }

    /// The least exact image of a Route key of `presentation` under the
    /// automorphisms of the presentation's whole line set, on any sector the
    /// census knows (the image of a sector is a sector with the same number
    /// of lines, so the key keeps its phase); `None` when the key is its own
    /// representative.
    fn canonical_in_presentation(
        &self,
        presentation: PresentationId,
        powers: &[i64],
    ) -> Result<Option<(Vec<i64>, bool)>, PError> {
        let shape = self.engine.shape(presentation)?;
        let orbit = self.presentation_orbit(presentation)?;
        if orbit.actions.is_empty() {
            return Ok(None);
        }
        let fates = &self.engine.fates;
        Ok(orbit.least_image(powers, self.prefer_high, |image| {
            fates.contains_key(&(presentation, shape.support(image)))
        }))
    }

    /// The verified automorphisms of the whole line set of `presentation`.
    fn presentation_orbit(&self, presentation: PresentationId) -> Result<&Arc<OwnerOrbit>, PError> {
        let shape = self.engine.shape(presentation)?;
        self.engine
            .presentation_orbits
            .get(presentation.0 as usize)
            .ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "presentation {} has no orbit cell",
                    presentation.0
                ))
            })?
            .get_or_init(|| {
                crate::prof!(OrbitInit);
                let source = self.library.presentation(presentation);
                if let Some(orbit) = self
                    .library
                    .saved_orbits()
                    .and_then(|saved| saved.restore(source, shape.lines))
                {
                    return Ok(Arc::new(orbit));
                }
                owner_orbit(source, shape.lines).map(Arc::new)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn canonical_route(
        &self,
        presentation: PresentationId,
        powers: &[i64],
    ) -> Result<Option<(Vec<i64>, bool)>, PError> {
        let engine = self.engine;
        let shape = engine.shape(presentation)?;
        let support = shape.support(powers);
        let Some(cell) = engine.route_orbits.get(&(presentation, support)) else {
            return Ok(None);
        };
        let orbit = cell
            .get_or_init(|| {
                crate::prof!(OrbitInit);
                let Ok(Sector::Routed { class, map }) =
                    engine.sector(self.library, presentation, support)
                else {
                    return Ok(Arc::new(OwnerOrbit::default()));
                };
                let owner = engine.owner(class)?;
                // The route must compile (and so be verified) before it is used.
                let source = self.library.presentation(presentation);
                let target = self.library.presentation(owner.presentation);
                let compiled = engine
                    .routes
                    .get(&(presentation, support))
                    .map(|route| {
                        route
                            .get_or_init(|| {
                                restored_route(
                                    self.library,
                                    source,
                                    target,
                                    map,
                                    support,
                                    owner.support,
                                )
                                .unwrap_or_else(|| {
                                    compile_route(source, target, map, support, owner.support)
                                })
                            })
                            .is_ok()
                    })
                    .unwrap_or(false);
                if !compiled {
                    return Ok(Arc::new(OwnerOrbit::default()));
                }
                let owner_orbit = self.orbit(class)?;
                Ok(Arc::new(routed_orbit(source, support, map, owner_orbit)))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        Ok(orbit.least_image(powers, self.prefer_high, |image| {
            shape.support(image) == support
        }))
    }

    /// The verified automorphisms of the sector of `class`.
    fn orbit(&self, class: OwnerClassId) -> Result<&Arc<OwnerOrbit>, PError> {
        let slot = self.engine.owner(class)?;
        self.engine
            .orbits
            .get(&class)
            .ok_or_else(|| {
                PError::MalformedLibrary(format!("owner class {:016x} has no orbit cell", class.0))
            })?
            .get_or_init(|| {
                crate::prof!(OrbitInit);
                // An owner on every line of its presentation has the
                // presentation's group: search and authenticate it once.
                let lines = self.engine.shape(slot.presentation)?.lines;
                if slot.support & lines == lines {
                    return self.presentation_orbit(slot.presentation).map(Arc::clone);
                }
                let source = self.library.presentation(slot.presentation);
                if let Some(orbit) = self
                    .library
                    .saved_orbits()
                    .and_then(|saved| saved.restore(source, slot.support))
                {
                    return Ok(Arc::new(orbit));
                }
                owner_orbit(source, slot.support).map(Arc::new)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// fwd-opt: the compiled route of sector `support` of `presentation` (as
    /// [`Self::route_step`] compiles it) and, with Route keys canonicalised,
    /// its automorphisms (as [`Self::canonical_route`] finds them).
    fn warm_route(&self, presentation: PresentationId, support: u64) {
        let engine = self.engine;
        let Ok(Sector::Routed { class, map }) = engine.sector(self.library, presentation, support)
        else {
            return;
        };
        let Ok(owner) = engine.owner(class) else {
            return;
        };
        if let Some(route) = engine.routes.get(&(presentation, support)) {
            let source = self.library.presentation(presentation);
            let target = self.library.presentation(owner.presentation);
            let _ = route.get_or_init(|| {
                crate::prof!(RouteCompile);
                restored_route(self.library, source, target, map, support, owner.support)
                    .unwrap_or_else(|| compile_route(source, target, map, support, owner.support))
            });
        }
        if self.canonical_routes
            && let Ok(shape) = engine.shape(presentation)
        {
            // A key of the sector: its lines at power 1 (`canonical_route`
            // reads only the support of the key to find the cell).
            let powers: Vec<i64> = (0..shape.arity)
                .map(|slot| i64::from((support >> slot) & 1 == 1))
                .collect();
            if shape.support(&powers) == support {
                let _ = self.canonical_route(presentation, &powers);
            }
        }
    }

    /// The least exact image of `powers` other than itself, in the owner's
    /// order and then by powers, with its sign; `None` when the key is its own
    /// representative. Images the owner cannot order, or off its support, are
    /// not used.
    fn canonical_powers(
        &self,
        class: OwnerClassId,
        powers: &[i64],
    ) -> Result<Option<(Vec<i64>, bool)>, PError> {
        let orbit = self.orbit(class)?;
        if orbit.actions.is_empty() {
            return Ok(None);
        }
        let slot = self.engine.owner(class)?;
        let shape = self.engine.shape(slot.presentation)?;
        // The default order key (every lane but Custom; Triangle owners weigh
        // their role slots) is a symmetric function of the powers, so every
        // image ties with the key and only the powers decide.
        let spec = &self.owner(class)?.spec;
        // Diamond and Scheme owners (M5 B1, A1) have their own order keys, not
        // symmetric under the automorphisms: they take the order-key path.
        if !matches!(
            spec.lane,
            OwnerLane::Custom | OwnerLane::Diamond | OwnerLane::Scheme | OwnerLane::Triangle
        ) && !matches!(spec.roles, LaneRoles::Triangle(_))
        {
            return Ok(orbit.least_image(powers, self.prefer_high, |image| {
                shape.support(image) == slot.support
            }));
        }
        let mut images: BTreeMap<Vec<i64>, bool> = BTreeMap::new();
        for action in &orbit.actions {
            if let Some((image, negative)) = OwnerOrbit::image(action, powers)
                && image.as_slice() != powers
            {
                images.entry(image).or_insert(negative);
            }
        }
        if images.is_empty() {
            return Ok(None);
        }
        let owner = self.owner(class)?;
        let context = self.context(owner);
        let order = |powers: &[i64]| match owner.program(self.library) {
            Ok(OwnerProgram::Lane(lane)) => lane.order_key(&context, powers).ok(),
            Ok(OwnerProgram::Missing { .. }) => default_order_key(&owner.spec, powers).ok(),
            Err(_) => None,
        };
        let Some(mut best_order) = order(powers) else {
            return Ok(None);
        };
        let mut best: Option<(Vec<i64>, bool)> = None;
        for (image, negative) in images {
            if shape.support(&image) != slot.support {
                continue;
            }
            let Some(image_order) = order(&image) else {
                continue;
            };
            let below = match image_order.cmp(&best_order) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Greater => false,
                std::cmp::Ordering::Equal => {
                    let bound = best.as_ref().map_or(powers, |(b, _)| b.as_slice());
                    if self.prefer_high {
                        image.as_slice() > bound
                    } else {
                        image.as_slice() < bound
                    }
                }
            };
            if below {
                best_order = image_order;
                best = Some((image, negative));
            }
        }
        Ok(best)
    }

    /// The Apply key of `child`: its sector must be an owner sector.
    fn apply_child(&self, child: &PChild) -> Result<(PKey, Coefficient), PError> {
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
        self.apply_key(*class, &child.powers, &child.tag, child.coefficient.clone())
    }

    fn keep_canonical_child(
        &self,
        child: &PKey,
        coefficient: &Coefficient,
    ) -> Result<bool, PError> {
        crate::prof_fine!(CanonZero);
        let Some(counters) = self.zero_pruning else {
            return Ok(true);
        };
        if coefficient.is_zero() {
            return Ok(true); // The existing merge erases literal zero weights.
        }
        self.keep_canonical_key(child, counters)
    }

    /// `keep_canonical_child` of a child with a nonzero weight (fwd-opt).
    fn keep_canonical_key(
        &self,
        child: &PKey,
        counters: &ZeroPruningCounters,
    ) -> Result<bool, PError> {
        counters
            .canonical_candidates
            .fetch_add(1, AtomicOrdering::Relaxed);
        let PKey::Route { presentation, view } = child else {
            return Ok(true);
        };
        let mut slots = ViewSlots::new();
        view.slots_into(&mut slots);
        let (powers, _) = self.engine.split(*presentation, &slots)?;
        let support = self.engine.shape(*presentation)?.support(powers);
        // Use exactly the existing census authority. In particular, a gap
        // propagates its error and is never inferred to be a zero sector.
        if matches!(
            self.engine.sector(self.library, *presentation, support)?,
            Sector::Zero
        ) {
            counters.known_zero.fetch_add(1, AtomicOrdering::Relaxed);
            return Ok(false);
        }
        Ok(true)
    }

    fn ordered_step(&self, output: PStepOut) -> Result<OrderedStep<PKey>, PError> {
        crate::prof!(ChildAssembly);
        let context = self.library.coefficient_context();
        let mut step = OrderedStep::default();
        let mut pairs: Vec<(PKey, Coefficient)> = Vec::with_capacity(output.children.len());
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
            let (key, coefficient) = match child.stage {
                PStage::Route => self.route_child(
                    child.presentation,
                    &child.powers,
                    &child.tag,
                    child.coefficient,
                )?,
                PStage::Apply => self.apply_child(&child)?,
            };
            if self.keep_canonical_child(&key, &coefficient)? {
                pairs.push((key, coefficient));
            }
            super::step_memo::put_powers(child.powers);
        }
        if let Some((presentation, tag, children)) = light::take_pending() {
            // fwd-opt light Insertion children: canonicalised by handle; a key
            // met twice (or also among the materialised children) is
            // materialised and summed, the others stay light.
            let mut light_pairs: Vec<(PKey, u64)> = Vec::with_capacity(children.len());
            for (powers, handle) in children {
                let (key, negative) =
                    self.route_child_with(presentation, &powers, &tag, false, |b| !b)?;
                super::step_memo::put_powers(powers);
                let keep = match self.zero_pruning {
                    Some(counters) => self.keep_canonical_key(&key, counters)?,
                    None => true,
                };
                if keep {
                    light_pairs.push((
                        key,
                        if negative {
                            light::flip(handle)
                        } else {
                            handle
                        },
                    ));
                }
            }
            light_pairs.sort_by(|a, b| a.0.cmp(&b.0));
            let materialised: std::collections::HashSet<&PKey> =
                pairs.iter().map(|(key, _)| key).collect();
            let mut single = Vec::with_capacity(light_pairs.len());
            let mut repeated: Vec<(PKey, Coefficient)> = Vec::new();
            let mut at = 0;
            while at < light_pairs.len() {
                let end = at
                    + light_pairs[at..]
                        .iter()
                        .take_while(|(key, _)| *key == light_pairs[at].0)
                        .count();
                if end - at == 1 && !materialised.contains(&light_pairs[at].0) {
                    single.push(light_pairs[at].clone());
                } else {
                    for (key, handle) in &light_pairs[at..end] {
                        let value =
                            OrderedStepper::light_value(self, *handle).ok_or_else(|| {
                                PError::Contract(
                                    "a light handle names no memo coefficient".to_owned(),
                                )
                            })?;
                        repeated.push((key.clone(), value));
                    }
                }
                at = end;
            }
            drop(materialised);
            pairs.extend(repeated);
            light::put(single);
        }
        step.terms = merge_all(context, &self.limits, pairs)?;
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
        light::clear();
        drop(light::take_pending());
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

    fn set_light(&self, on: bool) {
        // Light children only without the census (its step records count
        // the terms of the step).
        light::set(on && !self.census.on);
    }

    fn take_light(&self) -> Vec<(PKey, u64)> {
        if light::enabled() {
            light::take()
        } else {
            Vec::new()
        }
    }

    fn light_value(&self, handle: u64) -> Option<Coefficient> {
        let (index, negative) = light::split(handle);
        let value = if light::is_local(handle) {
            super::step_memo::local_value(index)
        } else {
            super::step_memo::shared_value(index)
        };
        value.map(|value| if negative { -value } else { value })
    }

    fn warm_count(&self) -> usize {
        self.warm.len()
    }

    fn warm(&self, index: usize) {
        // Errors are kept in the cells, exactly as a step would keep them,
        // and surface only in a step that needs the cell.
        match self.warm.get(index) {
            Some(&WarmTask::Program(class)) => {
                if let Ok(owner) = self.owner(class) {
                    let _ = owner.program(self.library);
                }
            }
            Some(&WarmTask::OwnerOrbit(class)) => {
                let _ = self.orbit(class);
            }
            Some(&WarmTask::Route(presentation, support)) => {
                self.warm_route(presentation, support);
            }
            Some(&WarmTask::PresentationOrbit(presentation)) => {
                let _ = self.presentation_orbit(presentation);
            }
            None => {}
        }
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

    fn set_light(&mut self, on: bool) {
        OrderedStepper::set_light(&self.stepper, on);
    }

    fn take_light(&mut self) -> Vec<(PKey, u64)> {
        OrderedStepper::take_light(&self.stepper)
    }

    fn light_value(&self, handle: u64) -> Option<Coefficient> {
        OrderedStepper::light_value(&self.stepper, handle)
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

impl<'l> ConcurrentOrderedKernel for PKernel<'l, '_> {
    type Account = PAccount;
    type Detached = PStepper<'l>;
    type Stepper<'s>
        = PStepper<'s>
    where
        Self: 's;

    fn stepper(&self) -> Option<PStepper<'_>> {
        Some(self.stepper)
    }

    fn detached_stepper(&self) -> Option<PStepper<'l>> {
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

    fn step_cache_refresh(&self, step: OrderedStep<PKey>) -> Result<OrderedStep<PKey>, PError> {
        self.stepper.refresh_canonical(step)
    }
}

/// Owner classes whose Apply children are canonicalised again when a step is
/// replayed from a step cache (dry census, R4a): a cache forked from another
/// program whose order of these owners differs holds representatives chosen
/// in the old order. Process-wide; empty (the default) changes nothing.
static REFRESH_CLASSES: std::sync::RwLock<BTreeSet<u64>> = std::sync::RwLock::new(BTreeSet::new());

/// Set the classes of [`REFRESH_CLASSES`] for the requests that start after
/// this call (dry census only; never set it while a cache serves programs
/// that disagree on any other owner).
pub fn set_step_cache_refresh_classes(classes: BTreeSet<u64>) {
    *REFRESH_CLASSES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = classes;
}

impl PStepper<'_> {
    /// Canonicalise again (in the current owners' orders) the Apply children
    /// of the [`REFRESH_CLASSES`] of a replayed step, merging children that
    /// meet and dropping sums that cancel, as the step itself would.
    #[allow(clippy::mutable_key_type)] // PKey views cache their IntegralKey lazily; Hash/Ord read only the packed powers (rr-alloc)
    fn refresh_canonical(&self, step: OrderedStep<PKey>) -> Result<OrderedStep<PKey>, PError> {
        if !self.canonical {
            return Ok(step);
        }
        let classes = REFRESH_CLASSES
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if classes.is_empty()
            || !step
                .terms
                .keys()
                .any(|key| matches!(key, PKey::Apply { class, .. } if classes.contains(&class.0)))
        {
            return Ok(step);
        }
        let OrderedStep {
            terms,
            terminals,
            nonzero_conditions,
        } = step;
        let context = self.library.coefficient_context();
        let mut out = BTreeMap::new();
        for (key, coefficient) in terms {
            let (key, coefficient) = match &key {
                PKey::Apply { class, view } if classes.contains(&class.0) => {
                    let slot = self.engine.owner(*class)?;
                    let slots = view.slots();
                    let (powers, tag_slots) = self.engine.split(slot.presentation, &slots)?;
                    let tag = GammaTag::decode_dense(tag_slots)?;
                    self.apply_key(*class, powers, &tag, coefficient)?
                }
                _ => (key, coefficient),
            };
            merge(context, &self.limits, &mut out, key, coefficient)?;
        }
        Ok(OrderedStep {
            terms: out,
            terminals,
            nonzero_conditions,
        })
    }
}

impl<'l> ConcurrentStepCacheKernel for PKernel<'l, '_> {
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

    fn detached_shard(stepper: &PStepper<'l>, key: &PKey) -> Option<OrderedStepShard> {
        stepper.shard(key)
    }

    fn detached_key(
        stepper: &PStepper<'l>,
        stage: NodeStage,
        view: IntegralKey,
    ) -> Result<PKey, PError> {
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
    use crate::propagator::step::PLaneStep;

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
        library_with_program(OwnerProgram::Missing { ledger: 0 })
    }

    fn library_with_program(program: OwnerProgram) -> (PLibrary, OwnerClassId) {
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
        // A one-line massless bubble is scaleless; the other one-line mask
        // remains absent so the zero-pruning tests also exercise a census gap.
        census.sectors.insert((BUBBLE, 0b01), PSectorFate::Zero);
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
                program,
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
            canonical: true,
            canonical_routes: true,
            prefer_high: false,
            zero_pruning: None,
            warm: &[],
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
            canonical: true,
            canonical_routes: true,
            prefer_high: false,
            zero_pruning: None,
            warm: &[],
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

    #[test]
    fn known_zero_pruning_preserves_default_tags_conditions_and_cancellation() {
        let (library, _) = library();
        let engine = PEngine::new(&library).unwrap();
        let counters = ZeroPruningCounters::default();
        let off = PStepper {
            library: &library,
            engine: &engine,
            limits: ReductionLimits::default(),
            census: PCensusMode::OFF,
            canonical: false,
            canonical_routes: false,
            prefer_high: false,
            zero_pruning: None,
            warm: &[],
        };
        let on = PStepper {
            zero_pruning: Some(&counters),
            ..off
        };
        let one = library.coefficient_context().one();
        let tag = GammaTag::from_pairs([(1, 2)]).unwrap();
        let other_tag = GammaTag::from_pairs([(-1, 3)]).unwrap();
        let child = |powers: Vec<i64>, tag: &GammaTag, coefficient: Coefficient| PChild {
            stage: PStage::Route,
            presentation: BUBBLE,
            powers,
            tag: tag.clone(),
            coefficient,
        };
        let output = PStepOut {
            children: vec![
                child(vec![1, 0], &tag, one.clone()),
                child(vec![1, 1], &tag, one.clone()),
                child(vec![1, 1], &tag, one.clone()),
                child(vec![1, 1], &tag, -one.clone()),
                child(vec![1, 1], &other_tag, one.clone()),
            ],
            terminals: vec![(PTerminalId(0), tag.clone(), one.clone())],
            conditions: vec![one.clone()],
        };
        let old = off.ordered_step(output.clone()).unwrap();
        let pruned = on.ordered_step(output).unwrap();
        let zero = engine.route_key(BUBBLE, &[1, 0], &tag).unwrap();
        let first = engine.route_key(BUBBLE, &[1, 1], &tag).unwrap();
        let second = engine.route_key(BUBBLE, &[1, 1], &other_tag).unwrap();
        // The default has exactly the old three coalesced tagged children.
        assert_eq!(old.terms.len(), 3);
        for key in [&zero, &first, &second] {
            assert_eq!(old.terms.get(key), Some(&one));
        }
        assert_eq!(pruned.terms.len(), 2);
        assert!(!pruned.terms.contains_key(&zero));
        for key in [&first, &second] {
            assert_eq!(pruned.terms.get(key), Some(&one));
        }
        assert_eq!(pruned.terminals, old.terminals);
        assert_eq!(pruned.nonzero_conditions, old.nonzero_conditions);
        assert_eq!(
            counters.canonical_candidates.load(AtomicOrdering::Relaxed),
            5
        );
        assert_eq!(counters.known_zero.load(AtomicOrdering::Relaxed), 1);
        assert!(
            on.route_step(BUBBLE, &engine.view(&[1, 0], &tag).unwrap())
                .unwrap()
                .terms
                .is_empty()
        );
    }

    #[test]
    fn known_zero_pruning_refuses_gaps_and_preserves_coefficient_admission() {
        let (library, _) = library();
        let engine = PEngine::new(&library).unwrap();
        let counters = ZeroPruningCounters::default();
        let stepper = PStepper {
            library: &library,
            engine: &engine,
            limits: ReductionLimits::default(),
            census: PCensusMode::OFF,
            canonical: false,
            canonical_routes: false,
            prefer_high: false,
            zero_pruning: Some(&counters),
            warm: &[],
        };
        let tag = GammaTag::one();
        let one = library.coefficient_context().one();
        let gap = engine.route_key(BUBBLE, &[0, 1], &tag).unwrap();
        assert!(matches!(
            stepper.keep_canonical_child(&gap, &one),
            Err(PError::MalformedLibrary(_))
        ));
        assert_eq!(counters.known_zero.load(AtomicOrdering::Relaxed), 0);
        let foreign = CoefficientContext::try_new(["x"])
            .unwrap()
            .parameter("x")
            .unwrap();
        let output = PStepOut {
            children: vec![PChild {
                stage: PStage::Route,
                presentation: BUBBLE,
                powers: vec![1, 0],
                tag,
                coefficient: foreign,
            }],
            ..Default::default()
        };
        assert!(matches!(
            stepper.ordered_step(output),
            Err(PError::ContextMismatch)
        ));
        assert_eq!(counters.known_zero.load(AtomicOrdering::Relaxed), 0);
    }

    /// A native fixture with one explicit zero child, two nonzero tagged
    /// terminals, and a nontrivial condition. It uses the actual Ordered cache,
    /// canonical keys, coefficient context and terminal decoding.
    struct PruningCacheLane;

    impl PLaneStep for PruningCacheLane {
        fn lane(&self) -> OwnerLane {
            OwnerLane::Custom
        }

        fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
            let context = cx.library.coefficient_context();
            let d = context.parameter("d").unwrap();
            Ok(PStepOut {
                children: vec![PChild {
                    stage: PStage::Route,
                    presentation: BUBBLE,
                    powers: vec![1, 0],
                    tag: key.tag.add(&GammaTag::from_pairs([(1, 2)])?),
                    coefficient: context.one(),
                }],
                terminals: vec![
                    (
                        PTerminalId(0),
                        key.tag.add(&GammaTag::from_pairs([(1, 2)])?),
                        d.clone(),
                    ),
                    (
                        PTerminalId(0),
                        key.tag.add(&GammaTag::from_pairs([(-1, 3)])?),
                        context.one(),
                    ),
                ],
                conditions: vec![d],
            })
        }
    }

    /// Run by the parent test in separate processes: no concurrent environment
    /// mutation, and each process captures the production flag exactly once.
    #[test]
    fn known_zero_cache_fixture_process() {
        use super::super::library::{PReductionRequest, PUncoveredPolicy};
        use crate::reduction::OrderedStepCacheBinding;
        use crate::solver::UnresolvedPolicy;

        let Ok(action) = std::env::var("RUSTRED_TEST_PRUNING_CACHE_ACTION") else {
            return;
        };
        let root =
            std::path::PathBuf::from(std::env::var_os("RUSTRED_TEST_PRUNING_CACHE_ROOT").unwrap());
        let on = super::super::library::prune_zero_children_enabled();
        let mode = if on { "on" } else { "off" };
        let (library, _) = library_with_program(OwnerProgram::Lane(Arc::new(PruningCacheLane)));
        library.set_orbit_canonicalization(false);
        let context = library.coefficient_context();
        let mut legacy = library.identity();
        assert!(!crate::reduction::ordered_reduction::p_requests_run_the_adjoint_tail());
        legacy.push_str(",P_ADJOINT_TAIL=off");
        // M5 batch 4: pruning is the default; the identity of a process that
        // does NOT prune carries the marker.
        let marker = ",P_PRUNE_ZERO_CHILDREN=off";
        for (route, high, suffix) in [
            (true, true, ""),
            (false, true, "-apply"),
            (true, false, "-least"),
            (false, false, "-apply-least"),
        ] {
            library.set_orbit_canonicalization(true);
            library.set_route_canonicalization(route);
            library.set_orbit_prefer_high(high);
            assert_eq!(
                library.step_identity(),
                format!(
                    "{},P_ORBIT_CANON_VERSION={}{suffix},P_ADJOINT_TAIL=off{}",
                    library.identity(),
                    super::super::P_ORBIT_CANON_VERSION,
                    if on { "" } else { marker }
                )
            );
        }
        library.set_orbit_canonicalization(false);
        let expected_identity = format!("{legacy}{}", if on { "" } else { marker });
        assert_eq!(library.step_identity(), expected_identity);
        let binding = library.step_cache_binding().unwrap();
        assert_eq!(binding.family_fingerprint, expected_identity);
        let request = |cache| PReductionRequest {
            inputs: vec![(BUBBLE, IntegralKey::try_new([1, 1]).unwrap(), context.one())],
            arithmetic: OrderedWeightArithmetic::Exact,
            scheduler: OrderedSchedulerOptions {
                threads: 1,
                ..Default::default()
            },
            step_cache: cache,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        };
        if action == "reject-other" {
            let other_mode = if on { "off" } else { "on" };
            assert!(matches!(
                OrderedStepCache::open(root.join(other_mode), binding, Default::default()),
                Err(ReductionError::OrderedStepCache { .. })
            ));
            let other_identity = if on {
                format!("{legacy}{marker}")
            } else {
                legacy
            };
            let wrong = OrderedStepCacheBinding::new(
                crate::reduction::OrderedStepCacheAdapter::PropagatorLibrary,
                other_identity,
                context,
                Some(UnresolvedPolicy::Reject),
            );
            assert!(matches!(
                library.reduce(request(Some(OrderedStepCache::in_memory(
                    wrong,
                    Default::default()
                )))),
                Err(PError::Engine(ReductionError::OrderedStepCache { .. }))
            ));
            return;
        }
        assert_eq!(action, "roundtrip");
        let uncached = library.reduce(request(None)).unwrap();
        assert_eq!(uncached.terms.len(), 2);
        assert_eq!(uncached.nonzero_conditions.len(), 1);
        assert_ne!(uncached.terms[0].tag, uncached.terms[1].tag);
        let directory = root.join(mode);
        let cold = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
        let first = library.reduce(request(Some(Arc::clone(&cold)))).unwrap();
        let progress = &first.statistics.progress;
        assert!(progress.processed_keys > 0);
        assert_eq!(progress.step_cache_hits, 0);
        assert_eq!(progress.step_cache_misses, progress.processed_keys);
        let saved = cold.save().unwrap();
        assert!(saved.records > 0);
        let warm = OrderedStepCache::open(&directory, binding, Default::default()).unwrap();
        let second = library.reduce(request(Some(warm))).unwrap();
        let warm_progress = &second.statistics.progress;
        assert!(warm_progress.step_cache_hits > 0);
        assert_eq!(warm_progress.step_cache_hits, warm_progress.processed_keys);
        assert_eq!(warm_progress.step_cache_misses, 0);
        for output in [&first, &second] {
            assert_eq!(output.terms, uncached.terms);
            assert_eq!(output.nonzero_conditions, uncached.nonzero_conditions);
            assert_eq!(output.dimension, uncached.dimension);
        }
        let terms: Vec<_> = second
            .terms
            .iter()
            .map(|term| {
                let mut tag = Vec::new();
                term.tag.encode_dense(&mut tag).unwrap();
                serde_json::json!({"tag": tag, "coefficient": term.coefficient.to_string()})
            })
            .collect();
        let report = serde_json::json!({
            "identity": expected_identity,
            "value": {"dimension": second.dimension, "terms": terms,
                "conditions": second.nonzero_conditions.iter().map(ToString::to_string).collect::<Vec<_>>()},
            "cold_misses": progress.step_cache_misses,
            "warm_hits": warm_progress.step_cache_hits,
            "warm_misses": warm_progress.step_cache_misses,
            "saved_records": saved.records,
        });
        std::fs::write(
            root.join(format!("{mode}.json")),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn known_zero_cache_roundtrip_and_mode_isolation() {
        let keep = std::env::var_os("RUSTRED_TEST_PRUNING_CACHE_ROOT");
        let root = keep
            .clone()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("rustred-pruning-cache-{}", std::process::id()))
            });
        std::fs::create_dir_all(&root).unwrap();
        let executable = std::env::current_exe().unwrap();
        for action in ["roundtrip", "reject-other"] {
            for on in [false, true] {
                let mut child = std::process::Command::new(&executable);
                child
                    .args([
                        "--exact",
                        "propagator::engine::tests::known_zero_cache_fixture_process",
                        "--nocapture",
                    ])
                    .env("RUSTRED_TEST_PRUNING_CACHE_ACTION", action)
                    .env("RUSTRED_TEST_PRUNING_CACHE_ROOT", &root)
                    .env("RUSTRED_ORDERED_ADJOINT_TAIL", "off");
                if on {
                    child.env_remove("RUSTRED_P_PRUNE_ZERO_CHILDREN");
                } else {
                    child.env("RUSTRED_P_PRUNE_ZERO_CHILDREN", "0");
                }
                assert!(child.status().unwrap().success(), "{action}, pruning={on}");
            }
        }
        let off: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("off.json")).unwrap()).unwrap();
        let on: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("on.json")).unwrap()).unwrap();
        assert_eq!(off["value"], on["value"]);
        assert_eq!(
            off["identity"].as_str().unwrap(),
            format!(
                "{},P_PRUNE_ZERO_CHILDREN=off",
                on["identity"].as_str().unwrap()
            )
        );
        if keep.is_none() {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
