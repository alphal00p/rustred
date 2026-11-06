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

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::IntegralKey;
use crate::reduction::ordered_reduction::step_cache::codec::NodeStage;
use crate::reduction::ordered_reduction::step_cache::failure;
use crate::reduction::ordered_reduction::step_cache::kernel::{
    ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
};
use crate::reduction::ordered_reduction::{
    ConcurrentOrderedKernel, OrderedKernel, OrderedKeyStage, OrderedOutput, OrderedPhaseDescriptor,
    OrderedPhaseEvent, OrderedPosition, OrderedStep, OrderedStepper,
};
use crate::reduction::{
    OrderedSchedulerOptions, OrderedStepCache, OrderedStepShard, OrderedStepStage,
    OrderedWeightArithmetic, ReductionError, ReductionLimits, ReductionRequest,
    ReductionStatistics,
};
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport::{self, ExpansionLimits, Prepared};
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap};
use crate::solver::CandidateOrderedPhaseKind;

use super::census::{PSectorFate, SignedAffineLoopMap};
use super::error::PError;
use super::library::{OwnerProgram, PInspector, PLibrary, POwner};
use super::presentation::Presentation;
use super::step::{PChild, PKeyView, POrderKey, PStage, PStepContext, PStepOut, default_order_key};
use super::tag::GammaTag;
use super::terminal::PTerminalCodec;
use super::{OwnerClassId, P_GAMMA_TAG_WIDTH, PTerminalId, PresentationId};

/// One scheduled identity of a p reduction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PKey {
    Route {
        presentation: PresentationId,
        view: IntegralKey,
    },
    Apply {
        class: OwnerClassId,
        view: IntegralKey,
    },
}

/// Position inside one phase, larger first: the node rank of the owner, then
/// the owner's order key. Route keys have no descent inside their phase.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    phases: PhasePlan,
    owners: BTreeMap<OwnerClassId, OwnerSlot>,
    owner_sectors: BTreeMap<(PresentationId, u64), OwnerClassId>,
    routes: BTreeMap<(PresentationId, u64), OnceLock<Result<Prepared, PError>>>,
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
            phases: PhasePlan { max_lines },
            owners,
            owner_sectors,
            routes,
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

    /// The powers and tag slots of `view`, a key of `presentation`.
    fn split<'v>(
        &self,
        presentation: PresentationId,
        view: &'v IntegralKey,
    ) -> Result<(&'v [i64], &'v [i64]), PError> {
        let arity = self.shape(presentation)?.arity;
        let slots = view.powers();
        if slots.len() != self.width || arity + P_GAMMA_TAG_WIDTH > self.width {
            return Err(PError::MalformedLibrary(format!(
                "a key of presentation {} has {} slots, not {}",
                presentation.0,
                slots.len(),
                self.width
            )));
        }
        let (powers, rest) = slots.split_at(arity);
        Ok((powers, &rest[..P_GAMMA_TAG_WIDTH]))
    }

    /// `powers ++ dense tag ++ zeros`; a tag outside the key range is
    /// [`PError::TagOutOfRange`].
    fn view(&self, powers: &[i64], tag: &GammaTag) -> Result<IntegralKey, PError> {
        let mut slots = Vec::with_capacity(self.width);
        slots.extend_from_slice(powers);
        tag.encode_dense(&mut slots)?;
        self.padded(slots)
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

    /// The terminal and tag of a terminal key of the output.
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
        let plain = IntegralKey::try_new(slots[..PTerminalCodec::KEY_SLOTS].iter().copied())
            .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
        PTerminalCodec::decode(&plain)
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
        inspector: Option<&mut dyn PInspector>,
    ) -> Result<OrderedOutput, PError> {
        let mut kernel = PKernel {
            stepper: PStepper {
                library,
                engine: self,
                limits,
            },
            inspector,
        };
        run_with_step_cache_and_options(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            Vec::new(),
            arithmetic,
            scheduler,
            cache,
        )
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

/// The shared, read-only part of the adapter: pure positions and steps.
#[derive(Clone, Copy)]
pub(crate) struct PStepper<'l> {
    library: &'l PLibrary,
    engine: &'l PEngine,
    limits: ReductionLimits,
}

/// Work of one detached step.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PAccount {
    rule_applications: usize,
}

impl<'l> PStepper<'l> {
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

    fn route_t(&self, presentation: PresentationId, view: &IntegralKey) -> Result<usize, PError> {
        let (powers, _) = self.engine.split(presentation, view)?;
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
                order: POrderKey::Lexicographic(Vec::new()),
            });
        };
        let slot = self.engine.owner(*class)?;
        let (powers, _) = self.engine.split(slot.presentation, view)?;
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
        view: &IntegralKey,
    ) -> Result<OrderedStep<PKey>, PError> {
        let engine = self.engine;
        let (powers, tag_slots) = engine.split(presentation, view)?;
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
            .get_or_init(|| compile_route(source, target, map, support, owner.support))
            .as_ref()
            .map_err(Clone::clone)?;
        let key = IntegralKey::try_from_preallocated(powers.to_vec())
            .map_err(|error| PError::MalformedLibrary(error.to_string()))?;
        let limits = ExpansionLimits {
            diagnostics_enabled: self.limits.diagnostics_enabled,
            exact_algebra: self.limits.exact_algebra,
            ..Default::default()
        };
        let mapped = prepared.transport(&key, limits)?;
        let tag = GammaTag::decode_dense(tag_slots)?;
        let context = self.library.coefficient_context();
        let shape = engine.shape(owner.presentation)?;
        for endpoint in mapped.terms() {
            let powers = endpoint.key().powers();
            // A numerator can pinch a line of the owner: that is a subsector
            // of its presentation, routed by the census in a later phase.
            let child = if shape.support(powers) == owner.support {
                PKey::Apply {
                    class,
                    view: engine.view(powers, &tag)?,
                }
            } else {
                engine.route_key(owner.presentation, powers, &tag)?
            };
            merge(
                context,
                &self.limits,
                &mut step.terms,
                child,
                endpoint.coefficient().clone(),
            )?;
        }
        Ok(step)
    }

    fn apply_step(
        &self,
        class: OwnerClassId,
        view: &IntegralKey,
    ) -> Result<OrderedStep<PKey>, PError> {
        let slot = self.engine.owner(class)?;
        let (powers, tag_slots) = self.engine.split(slot.presentation, view)?;
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
                .split(presentation, &view)
                .map_err(|_| failure("a recorded identity has the wrong arity"))
        };
        match stage {
            NodeStage::ClassRoute(class) => {
                let presentation = u32::try_from(class)
                    .ok()
                    .filter(|&id| (id as usize) < self.engine.shapes.len())
                    .map(PresentationId)
                    .ok_or_else(|| failure("a recorded Route identity names no presentation"))?;
                arity(presentation)?;
                Ok(PKey::Route { presentation, view })
            }
            NodeStage::ClassApply(class) => {
                let class = OwnerClassId(class);
                let slot = self
                    .engine
                    .owners
                    .get(&class)
                    .ok_or_else(|| failure("a recorded Apply identity names no owner"))?;
                arity(slot.presentation)?;
                Ok(PKey::Apply { class, view })
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
        match key {
            PKey::Route { presentation, view } => {
                (self.route_step(*presentation, view), PAccount::default())
            }
            PKey::Apply { class, view } => {
                let result = self.apply_step(*class, view);
                let account = PAccount {
                    rule_applications: usize::from(result.is_ok()),
                };
                (result, account)
            }
        }
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
    fn account(
        &self,
        account: PAccount,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), PError> {
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

    fn key_view(key: &PKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some(match key {
            PKey::Route { presentation, view } => (
                OrderedKeyStage::ClassRoute {
                    class: u64::from(presentation.0),
                },
                view,
            ),
            PKey::Apply { class, view } => (OrderedKeyStage::ClassApply { class: class.0 }, view),
        })
    }

    fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), PError> {
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
