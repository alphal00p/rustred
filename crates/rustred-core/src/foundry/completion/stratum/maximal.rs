use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::sector::SectorMonotoneDomain;

use super::{
    DecoratedStratum, StratumRegistryError, StratumRegistryLimits, check_limit, checked_mul,
    try_reserve,
};

/// Declared initial maximal stratum for one growing campaign task.
///
/// Construction cold-verifies the complete decorated identity. Exact
/// maximality is authenticated only when this declaration is consumed by a
/// [`MaximalStratumSequence`] and its first physical frame is materialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MaximalStratumAnchor {
    initial: DecoratedStratum,
}

/// Stateful maximal-stratum proof sequence for one growing campaign.
///
/// There is deliberately no caller-selected phase. The first materialization
/// must reproduce the declared anchor exactly. Every later materialization is
/// checked against the immediately preceding authenticated domain, so a
/// tightened campaign cannot subsequently widen even while remaining inside
/// its original anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MaximalStratumSequence {
    anchor: MaximalStratumAnchor,
    current: Option<DecoratedStratum>,
}

impl MaximalStratumAnchor {
    pub(crate) fn try_new(
        initial: DecoratedStratum,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if !initial.try_verify(limits)? {
            return Err(StratumRegistryError::Invariant {
                detail: "maximal-stratum anchor failed cold verification",
            });
        }
        Ok(Self { initial })
    }

    pub(crate) const fn initial(&self) -> &DecoratedStratum {
        &self.initial
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.initial.family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.initial.context_fingerprint()
    }

    pub(crate) fn arity(&self) -> usize {
        self.initial.domain().arity()
    }

    pub(crate) fn into_sequence(self) -> MaximalStratumSequence {
        MaximalStratumSequence {
            anchor: self,
            current: None,
        }
    }
}

impl MaximalStratumSequence {
    pub(crate) const fn scope(&self) -> &DecoratedStratum {
        self.anchor.initial()
    }

    /// Recompute and authenticate the maximal decorated stratum for one frame.
    ///
    /// The first call must reproduce the complete incoming anchor. A later
    /// call may only restrict the immediately preceding authenticated domain.
    pub(crate) fn try_materialize(
        &mut self,
        frame: &PhysicalFramePlan,
        target_column: usize,
        limits: StratumRegistryLimits,
    ) -> Result<DecoratedStratum, StratumRegistryError> {
        if frame.family_fingerprint() != self.anchor.family_fingerprint() {
            return Err(StratumRegistryError::WrongFrameFamily);
        }
        if frame.context_fingerprint() != self.anchor.context_fingerprint() {
            return Err(StratumRegistryError::WrongFrameContext);
        }
        if frame.sector() != self.anchor.initial.domain().sector() {
            return Err(StratumRegistryError::WrongFrameSector);
        }

        let domain = try_maximal_frame_domain(frame, target_column, limits)?;
        let materialized = if let Some(previous) = &self.current {
            if !domain_is_contained_by(&domain, previous.domain()) {
                return Err(StratumRegistryError::NonMonotoneMaximalDomain);
            }
            DecoratedStratum::try_new(
                self.anchor.family_fingerprint(),
                self.anchor.context_fingerprint(),
                domain,
                self.anchor.initial.guards().iter().cloned(),
                limits,
            )?
        } else {
            if &domain != self.anchor.initial.domain() {
                return Err(StratumRegistryError::InitialMaximalDomainMismatch);
            }
            self.anchor.initial.clone()
        };

        self.current = Some(materialized.clone());
        Ok(materialized)
    }
}

/// Canonical maximal finite-carrier domain for one exact physical frame.
pub(crate) fn try_maximal_frame_domain(
    frame: &PhysicalFramePlan,
    target_column: usize,
    limits: StratumRegistryLimits,
) -> Result<SectorMonotoneDomain, StratumRegistryError> {
    let target =
        frame
            .columns()
            .get(target_column)
            .ok_or(StratumRegistryError::TargetColumnOutOfRange {
                target: target_column,
                columns: frame.columns().len(),
            })?;
    check_limit(
        "decorated-stratum physical columns",
        frame.columns().len(),
        limits.max_physical_columns,
    )?;
    let coordinate_cells = checked_mul(
        "decorated-stratum physical-column coordinate cells",
        frame.columns().len(),
        frame.sector().arity(),
    )?;
    check_limit(
        "decorated-stratum physical-column coordinate cells",
        coordinate_cells,
        limits.max_column_coordinate_cells,
    )?;

    let mut shifts = Vec::new();
    try_reserve(
        &mut shifts,
        frame.columns().len(),
        "maximal-stratum physical shift views",
    )?;
    shifts.extend(frame.columns().iter().map(|shift| shift.values()));
    SectorMonotoneDomain::try_maximal_for_rule(frame.sector().clone(), target.values(), &shifts)
        .map_err(StratumRegistryError::Sector)
}

fn domain_is_contained_by(inner: &SectorMonotoneDomain, outer: &SectorMonotoneDomain) -> bool {
    inner.sector() == outer.sector()
        && inner
            .bounds()
            .iter()
            .zip(outer.bounds())
            .all(|(&inner, &outer)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
}
