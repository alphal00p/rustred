//! The SeriesParallel lane (owner: M2-T4 rr-kernel): the Insertion step along the
//! chain of a series-parallel node (plan decision D4). Every SP node has Forcer
//! action Insertion [M cascade.md section 1.4], so the lane adds no kernel;
//! [`P_INSERTION_KERNEL_VERSION`] names its meaning too.
//!
//! By default one step integrates one bubble, exactly as [`InsertionStep`]. The
//! single-step fold ([`SeriesParallelStep::with_fold`], off by default) replays
//! the chain down to the trivial node inside one step: a bubble integrates at any
//! integer power of the other slots, so no route is needed on the way, and the
//! step emits terminals only. The comparison of the two forms is an M3/M5
//! ablation (architecture doc section 3.2).
//!
//! [`P_INSERTION_KERNEL_VERSION`]: super::P_INSERTION_KERNEL_VERSION
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::reduction::ReductionLimits;

use super::PTerminalId;
use super::PresentationId;
use super::error::PError;
use super::insertion::InsertionStep;
use super::kernel::gamma::algebra;
use super::kernel::{MemoCounters, OwnerEdge, owner_edges};
use super::lane::{BubbleRoles, LaneRoles, OwnerLane};
use super::library::{PLibrary, POwnerSpec};
use super::presentation::Presentation;
use super::step::{PKeyView, PLaneStep, PStepContext, PStepOut};
use super::tag::GammaTag;

/// The Insertion step under the SeriesParallel lane.
#[derive(Debug)]
pub struct SeriesParallelStep {
    insertion: InsertionStep,
    /// The steps below this one, by presentation; `Some` turns the fold on.
    fold: Option<BTreeMap<PresentationId, Arc<InsertionStep>>>,
}

impl SeriesParallelStep {
    /// The delegating step, see [`InsertionStep::new`].
    pub fn new(
        parent: Arc<Presentation>,
        roles: BubbleRoles,
        edges: &[OwnerEdge],
    ) -> Result<Self, PError> {
        Ok(InsertionStep::new(parent, roles, edges)?.into())
    }

    /// [`Self::new`] on an owner of a library.
    pub fn from_owner(library: &PLibrary, owner: &POwnerSpec) -> Result<Self, PError> {
        let LaneRoles::Insertion(roles) = owner.roles else {
            return Err(PError::Contract(format!(
                "a series-parallel step with roles {:?}",
                owner.roles
            )));
        };
        let (parent, edges) = owner_edges(library, owner)?;
        Self::new(parent, roles, &edges)
    }

    /// Fold the chain into one step, given the step of every presentation below
    /// this one. A child without a step fails closed at run time.
    pub fn with_fold(mut self, chain: impl IntoIterator<Item = Arc<InsertionStep>>) -> Self {
        self.fold = Some(
            chain
                .into_iter()
                .map(|step| (step.presentation().id, step))
                .collect(),
        );
        self
    }

    pub fn insertion(&self) -> &InsertionStep {
        &self.insertion
    }

    pub fn memo_counters(&self) -> (MemoCounters, MemoCounters) {
        self.insertion.memo_counters()
    }

    /// The step on `key`, without a library context.
    pub fn apply(&self, key: &PKeyView<'_>, limits: &ReductionLimits) -> Result<PStepOut, PError> {
        let first = self.insertion.apply(key, limits)?;
        match &self.fold {
            None => Ok(first),
            Some(chain) => fold(
                self.insertion.presentation().family.coefficient_context(),
                chain,
                first,
                limits,
            ),
        }
    }
}

impl From<InsertionStep> for SeriesParallelStep {
    fn from(insertion: InsertionStep) -> Self {
        Self {
            insertion,
            fold: None,
        }
    }
}

type Level = BTreeMap<(PresentationId, Vec<i64>, GammaTag), Coefficient>;

fn accumulate(
    context: &CoefficientContext,
    slot: &mut Coefficient,
    value: &Coefficient,
    limits: &ReductionLimits,
) -> Result<(), PError> {
    *slot = context
        .try_add(slot, value, limits.exact_algebra)
        .map_err(algebra)?;
    Ok(())
}

/// Replay `first` down the chain, one loop per level, and collect its terminals.
fn fold(
    context: &CoefficientContext,
    chain: &BTreeMap<PresentationId, Arc<InsertionStep>>,
    first: PStepOut,
    limits: &ReductionLimits,
) -> Result<PStepOut, PError> {
    let exact = limits.exact_algebra;
    let mut terminals: BTreeMap<(PTerminalId, GammaTag), Coefficient> = BTreeMap::new();
    let mut level = Level::new();
    let mut pending = first;
    let mut factor = context.one();
    loop {
        for (id, tag, weight) in pending.terminals {
            let weight = context.try_mul(&factor, &weight, exact).map_err(algebra)?;
            let slot = terminals.entry((id, tag)).or_insert_with(|| context.zero());
            accumulate(context, slot, &weight, limits)?;
        }
        for child in pending.children {
            let weight = context
                .try_mul(&factor, &child.coefficient, exact)
                .map_err(algebra)?;
            let slot = level
                .entry((child.presentation, child.powers, child.tag))
                .or_insert_with(|| context.zero());
            accumulate(context, slot, &weight, limits)?;
        }
        let Some(((presentation, powers, tag), weight)) = level.pop_first() else {
            break;
        };
        let step = chain.get(&presentation).ok_or_else(|| {
            PError::Unsupported(format!(
                "the series-parallel fold has no step on presentation {}",
                presentation.0
            ))
        })?;
        factor = weight;
        pending = step.apply(
            &PKeyView {
                presentation,
                powers: &powers,
                tag: &tag,
            },
            limits,
        )?;
    }
    Ok(PStepOut {
        children: Vec::new(),
        terminals: terminals
            .into_iter()
            .filter(|(_, weight)| !weight.is_zero())
            .map(|((id, tag), weight)| (id, tag, weight))
            .collect(),
        conditions: Vec::new(),
    })
}

impl PLaneStep for SeriesParallelStep {
    fn lane(&self) -> OwnerLane {
        OwnerLane::SeriesParallel
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.apply(key, cx.limits)
    }
}
