//! The interface every lane implements (owner: M2-I0 rr-iface).
use crate::algebra::Coefficient;
use crate::reduction::ReductionLimits;

use super::error::PError;
use super::lane::OwnerLane;
use super::library::{PLibrary, POwner};
use super::tag::GammaTag;
use super::{PTerminalId, PresentationId};

/// One lane program. Pure: the same key gives the same output, so a step can
/// be recorded and replayed.
pub trait PLaneStep: Send + Sync {
    fn lane(&self) -> OwnerLane;
    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError>;
}

pub struct PStepContext<'a> {
    pub library: &'a PLibrary,
    pub owner: &'a POwner,
    pub limits: &'a ReductionLimits,
}

/// The key of one step: the powers of the presentation (its arity) and the
/// absolute tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PKeyView<'a> {
    pub presentation: PresentationId,
    pub powers: &'a [i64],
    pub tag: &'a GammaTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PStage {
    Route,
    Apply,
}

/// One child key with its absolute tag (parent tag + relative tag of the step)
/// and its weight in `Q(d)`, context `{d}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PChild {
    pub stage: PStage,
    pub presentation: PresentationId,
    pub powers: Vec<i64>,
    pub tag: GammaTag,
    pub coefficient: Coefficient,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PStepOut {
    pub children: Vec<PChild>,
    pub terminals: Vec<(PTerminalId, GammaTag, Coefficient)>,
    pub conditions: Vec<Coefficient>,
}
