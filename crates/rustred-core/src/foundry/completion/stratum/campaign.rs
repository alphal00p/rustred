use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::sector::{InteriorBounds, SectorMonotoneDomain};

use super::{
    DecoratedStratum, MaximalStratumAnchor, MaximalStratumSequence, StratumRegistryError,
    StratumRegistryLimits, try_reserve,
};

/// Verified semantic domain declared for one source-discovery task.
///
/// The maximal lane preserves the stronger first-frame maximality proof used
/// by ordinary interior campaigns.  The restricted lane represents an exact
/// face, ray, or cylinder selected from an uncovered lattice box.  Growing
/// source sets may tighten either lane for i64 representability, but can never
/// widen the declared domain or release a singleton coordinate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CampaignStratumAnchor {
    Maximal(MaximalStratumAnchor),
    Restricted(RestrictedStratumAnchor),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestrictedStratumAnchor {
    initial: DecoratedStratum,
}

/// Monotone materialization state for one growing source-discovery task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CampaignStratumSequence {
    Maximal(MaximalStratumSequence),
    Restricted(RestrictedStratumSequence),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestrictedStratumSequence {
    anchor: RestrictedStratumAnchor,
    current: Option<DecoratedStratum>,
}

impl CampaignStratumAnchor {
    pub(crate) fn try_restricted(
        initial: DecoratedStratum,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if !initial.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "restricted campaign-stratum anchor failed cold verification",
            });
        }
        Ok(Self::Restricted(RestrictedStratumAnchor { initial }))
    }

    pub(crate) const fn initial(&self) -> &DecoratedStratum {
        match self {
            Self::Maximal(anchor) => anchor.initial(),
            Self::Restricted(anchor) => &anchor.initial,
        }
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.initial().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.initial().context_fingerprint()
    }

    pub(crate) fn arity(&self) -> usize {
        self.initial().domain().arity()
    }

    pub(crate) fn into_sequence(self) -> CampaignStratumSequence {
        match self {
            Self::Maximal(anchor) => CampaignStratumSequence::Maximal(anchor.into_sequence()),
            Self::Restricted(anchor) => {
                CampaignStratumSequence::Restricted(RestrictedStratumSequence {
                    anchor,
                    current: None,
                })
            }
        }
    }
}

impl From<MaximalStratumAnchor> for CampaignStratumAnchor {
    fn from(value: MaximalStratumAnchor) -> Self {
        Self::Maximal(value)
    }
}

impl CampaignStratumSequence {
    pub(crate) const fn scope(&self) -> &DecoratedStratum {
        match self {
            Self::Maximal(sequence) => sequence.scope(),
            Self::Restricted(sequence) => &sequence.anchor.initial,
        }
    }

    pub(crate) fn try_materialize(
        &mut self,
        frame: &PhysicalFramePlan,
        target_column: usize,
        limits: StratumRegistryLimits,
    ) -> Result<DecoratedStratum, StratumRegistryError> {
        match self {
            Self::Maximal(sequence) => sequence.try_materialize(frame, target_column, limits),
            Self::Restricted(sequence) => sequence.try_materialize(frame, target_column, limits),
        }
    }
}

impl RestrictedStratumSequence {
    fn try_materialize(
        &mut self,
        frame: &PhysicalFramePlan,
        target_column: usize,
        limits: StratumRegistryLimits,
    ) -> Result<DecoratedStratum, StratumRegistryError> {
        let initial = &self.anchor.initial;
        if frame.family_fingerprint() != initial.family_fingerprint() {
            return Err(StratumRegistryError::WrongFrameFamily);
        }
        if frame.context_fingerprint() != initial.context_fingerprint() {
            return Err(StratumRegistryError::WrongFrameContext);
        }
        if frame.sector() != initial.domain().sector() {
            return Err(StratumRegistryError::WrongFrameSector);
        }
        let pivot = frame.columns().get(target_column).ok_or(
            StratumRegistryError::TargetColumnOutOfRange {
                target: target_column,
                columns: frame.columns().len(),
            },
        )?;
        let previous = self
            .current
            .as_ref()
            .map_or(initial.domain(), DecoratedStratum::domain);
        let maximal = super::maximal::try_maximal_frame_domain(frame, target_column, limits)?;
        let mut bounds = Vec::new();
        try_reserve(
            &mut bounds,
            previous.arity(),
            "restricted campaign-stratum intersection bounds",
        )?;
        for (&previous, &maximal) in previous.bounds().iter().zip(maximal.bounds()) {
            bounds.push(InteriorBounds::new(
                previous.lower().max(maximal.lower()),
                previous.upper().min(maximal.upper()),
            ));
        }
        let mut shifts = Vec::new();
        try_reserve(
            &mut shifts,
            frame.columns().len(),
            "restricted campaign-stratum physical shift views",
        )?;
        shifts.extend(frame.columns().iter().map(|shift| shift.values()));
        let domain = SectorMonotoneDomain::try_new_for_rule(
            frame.sector().clone(),
            bounds,
            pivot.values(),
            &shifts,
        )?;
        let materialized = if &domain == previous {
            self.current.as_ref().unwrap_or(initial).clone()
        } else {
            DecoratedStratum::try_new(
                initial.family_fingerprint(),
                initial.context_fingerprint(),
                domain,
                initial.guards().iter().cloned(),
                limits,
            )?
        };
        self.current = Some(materialized.clone());
        Ok(materialized)
    }
}
