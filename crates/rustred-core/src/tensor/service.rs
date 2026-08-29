//! Lane admission and the cohesive tensor-service owner.

use std::collections::HashMap;

use symbolica::atom::Atom;

use crate::family::IntegralKey;
use crate::family::presentation::{DenominatorRole, FamilyPresentation, SingleScaleVacuumEvidence};

use super::error::{MomentumKind, TensorError, check_limit, checked_add};
use super::heads::TensorHeads;
use super::model::{
    ResolvedTensorLane, ScalarProductLowering, TensorLane, TensorLimits, TensorMomenta,
    TensorProjection, TensorReduction,
};
use super::syntax::{census_atom, first_reserved_head};

/// One admitted tensor service bound to an authenticated family presentation.
pub struct TensorService<'presentation> {
    pub(super) evidence: SingleScaleVacuumEvidence<'presentation>,
    pub(super) heads: TensorHeads,
    pub(super) momenta: TensorMomenta,
    pub(super) limits: TensorLimits,
}

impl<'presentation> TensorService<'presentation> {
    pub fn try_new(
        presentation: &'presentation FamilyPresentation,
        lane: TensorLane,
        heads: TensorHeads,
        momenta: TensorMomenta,
        limits: TensorLimits,
    ) -> Result<Self, TensorError> {
        let evidence = match lane {
            TensorLane::Auto => presentation
                .single_scale_vacuum_evidence()
                .map_err(TensorError::AutomaticLaneUnavailable)?,
            TensorLane::SingleScaleVacuum => presentation
                .single_scale_vacuum_evidence()
                .map_err(TensorError::SingleScaleVacuumIneligible)?,
            TensorLane::Generic => return Err(TensorError::UnsupportedGenericKinematics),
        };
        validate_momenta(&evidence, &heads, &momenta, limits)?;
        Ok(Self {
            evidence,
            heads,
            momenta,
            limits,
        })
    }

    pub const fn lane(&self) -> ResolvedTensorLane {
        ResolvedTensorLane::SingleScaleVacuum
    }

    pub const fn heads(&self) -> &TensorHeads {
        &self.heads
    }

    pub const fn momenta(&self) -> &TensorMomenta {
        &self.momenta
    }

    pub const fn limits(&self) -> TensorLimits {
        self.limits
    }

    /// Project a numerator only after authenticating the scalar integral
    /// weight that makes the vacuum isotropy identities applicable.
    pub fn project(
        &self,
        numerator: &Atom,
        base_integral: &IntegralKey,
    ) -> Result<TensorProjection, TensorError> {
        self.validate_integral_eligibility(base_integral)?;
        self.project_impl(numerator)
    }

    pub fn lower_scalar_products(
        &self,
        projection: &TensorProjection,
        base_integral: &IntegralKey,
    ) -> Result<ScalarProductLowering, TensorError> {
        self.lower_impl(projection, base_integral)
    }

    pub fn reduce(
        &self,
        numerator: &Atom,
        base_integral: &IntegralKey,
    ) -> Result<TensorReduction, TensorError> {
        let projection = self.project(numerator, base_integral)?;
        self.lower_scalar_products(&projection, base_integral)
            .map(TensorReduction::from_lowering)
    }

    pub(super) fn presentation(&self) -> &'presentation FamilyPresentation {
        self.evidence.presentation()
    }

    pub(super) fn validate_integral_eligibility(
        &self,
        base_integral: &IntegralKey,
    ) -> Result<(), TensorError> {
        let family = self.presentation().family();
        if base_integral.powers().len() != family.denominator_count() {
            return Err(TensorError::WrongIntegralKeyArity {
                expected: family.denominator_count(),
                actual: base_integral.powers().len(),
            });
        }
        for (denominator, role) in self.presentation().denominator_roles().iter().enumerate() {
            if !matches!(role, DenominatorRole::Auxiliary(_)) {
                continue;
            }
            if !family.power_shifts()[denominator].is_zero() {
                return Err(TensorError::UnsupportedAuxiliaryPowerShift { denominator });
            }
            let power = base_integral.powers()[denominator];
            if power != 0 {
                return Err(TensorError::UnsupportedAuxiliaryIntegral { denominator, power });
            }
        }
        Ok(())
    }
}

fn validate_momenta(
    evidence: &SingleScaleVacuumEvidence<'_>,
    heads: &TensorHeads,
    momenta: &TensorMomenta,
    limits: TensorLimits,
) -> Result<(), TensorError> {
    let expected = evidence.presentation().family().loop_count();
    let actual = momenta.loop_momenta().len();
    if actual != expected {
        return Err(TensorError::WrongLoopMomentumCount { expected, actual });
    }
    let mut total_nodes = 0usize;
    for momentum in momenta
        .loop_momenta()
        .iter()
        .chain(momenta.external_momenta())
    {
        let nodes = census_atom(
            momentum.as_view(),
            "tensor momentum label nodes",
            limits.max_momentum_label_nodes,
            limits,
        )?;
        total_nodes = checked_add("tensor momentum label nodes", total_nodes, nodes)?;
        check_limit(
            "tensor momentum label nodes",
            total_nodes,
            limits.max_momentum_label_nodes,
        )?;
        if let Some(kind) = first_reserved_head(momentum.as_view(), heads) {
            return Err(TensorError::ReservedHeadInMomentum { kind });
        }
    }
    let momentum_count = checked_add(
        "tensor momentum labels",
        momenta.loop_momenta().len(),
        momenta.external_momenta().len(),
    )?;
    let mut seen = HashMap::<&Atom, MomentumKind>::new();
    seen.try_reserve(momentum_count)
        .map_err(|_| TensorError::AllocationFailure {
            resource: "tensor momentum label set",
            requested: momentum_count,
        })?;
    for (kind, labels) in [
        (MomentumKind::Loop, momenta.loop_momenta()),
        (MomentumKind::External, momenta.external_momenta()),
    ] {
        for label in labels {
            if let Some(previous) = seen.insert(label, kind) {
                return Err(TensorError::DuplicateMomentum {
                    first: previous,
                    second: kind,
                });
            }
        }
    }
    Ok(())
}
