//! Constant-width, strictly descending auxiliary recurrence.

use std::sync::Arc;

use crate::family::IntegralKey;

use super::error::FactorizedNumeratorLiftError;
use super::model::{
    CompiledFactorizationRouting, FactorizedNumeratorLiftAction, FactorizedNumeratorLiftChild,
    FactorizedNumeratorLiftStart, FactorizedNumeratorLiftState,
};

impl CompiledFactorizationRouting {
    /// Apply a complete pure unit-image routing. This owns the star/no-affine
    /// case and also a lift action's zero-power boundary. A nonzero power in a
    /// genuinely affine row is rejected explicitly rather than treated as an
    /// identity mapping.
    pub(crate) fn try_route_key(
        &self,
        target: &IntegralKey,
    ) -> Result<IntegralKey, FactorizedNumeratorLiftError> {
        let powers = self.validate_target(target)?;
        for (position, image) in self.unit_images.iter().enumerate() {
            if image.is_none() && powers[position] != 0 {
                return Err(FactorizedNumeratorLiftError::AffineRoutingRequired {
                    position,
                    power: powers[position],
                });
            }
        }
        let routed = self.try_route_unit_images(powers)?;
        Ok(IntegralKey::try_new(routed.iter().copied())?)
    }

    fn validate_target<'target>(
        &self,
        target: &'target IntegralKey,
    ) -> Result<&'target [i64], FactorizedNumeratorLiftError> {
        let powers = target.powers();
        let arity = self.unit_images.len();
        if powers.len() != arity {
            return Err(FactorizedNumeratorLiftError::WrongTargetArity {
                expected: arity,
                actual: powers.len(),
            });
        }
        for (position, (&power, &active)) in powers
            .iter()
            .zip(self.application_domain.sector().active_bits())
            .enumerate()
        {
            if active != (power >= 1) {
                return Err(FactorizedNumeratorLiftError::OutsideApplicationDomain {
                    position,
                    power,
                    active,
                });
            }
        }
        Ok(powers)
    }

    fn try_route_unit_images(
        &self,
        powers: &[i64],
    ) -> Result<Box<[i64]>, FactorizedNumeratorLiftError> {
        let arity = self.unit_images.len();
        let mut routed_powers = Vec::new();
        routed_powers.try_reserve_exact(arity).map_err(|_| {
            FactorizedNumeratorLiftError::AllocationFailure {
                resource: "factorized numerator routed powers",
                requested: arity,
            }
        })?;
        routed_powers.resize(arity, 0);
        for (&power, image) in powers.iter().zip(&self.unit_images) {
            if let Some(image) = image {
                routed_powers[*image] = power;
            }
        }
        Ok(routed_powers.into_boxed_slice())
    }
}

impl FactorizedNumeratorLiftAction {
    /// Admit a target in the complete factorized sector and route every unit
    /// denominator image. A zero affine-source power returns the exact routed
    /// ordinary key, never an ambiguous identity/no-op marker.
    pub(crate) fn try_start(
        &self,
        target: &IntegralKey,
    ) -> Result<FactorizedNumeratorLiftStart, FactorizedNumeratorLiftError> {
        let powers = self.routing.validate_target(target)?;
        let affine_power = powers[self.affine_source];
        if affine_power == 0 {
            return Ok(FactorizedNumeratorLiftStart::Routed(
                self.routing.try_route_key(target)?,
            ));
        }
        if affine_power > 0 {
            return Err(FactorizedNumeratorLiftError::Invariant {
                detail: "the compiled affine source is active in its application domain",
            });
        }
        Ok(FactorizedNumeratorLiftStart::Auxiliary(
            FactorizedNumeratorLiftState {
                identity: self.routing.identity.clone(),
                remaining_power: affine_power.unsigned_abs(),
                routed_powers: self.routing.try_route_unit_images(powers)?,
            },
        ))
    }

    /// Expand exactly one factor of the routed affine numerator.  Every child
    /// has measure `parent.remaining_power - 1`; branch width is independent
    /// of numerator rank and was admitted at cold compilation.
    pub(crate) fn try_step(
        &self,
        state: &FactorizedNumeratorLiftState,
    ) -> Result<Box<[FactorizedNumeratorLiftChild]>, FactorizedNumeratorLiftError> {
        if !Arc::ptr_eq(&self.routing.identity, &state.identity) {
            return Err(FactorizedNumeratorLiftError::ForeignAuxiliaryState);
        }
        let remaining_power = state
            .remaining_power
            .checked_sub(1)
            .ok_or(FactorizedNumeratorLiftError::EmptyAuxiliaryState)?;
        if state.routed_powers.len() != self.routing.unit_images.len() {
            return Err(FactorizedNumeratorLiftError::Invariant {
                detail: "an action-owned auxiliary state has the wrong arity",
            });
        }

        let relation = self.affine_relation();
        let mut children = Vec::new();
        children.try_reserve_exact(self.branch_width).map_err(|_| {
            FactorizedNumeratorLiftError::AllocationFailure {
                resource: "factorized numerator recurrence children",
                requested: self.branch_width,
            }
        })?;
        if !relation.constant.is_zero() {
            children.push(FactorizedNumeratorLiftChild {
                coefficient: relation.constant.clone(),
                state: FactorizedNumeratorLiftState {
                    identity: self.routing.identity.clone(),
                    remaining_power,
                    routed_powers: try_clone_routed_powers(&state.routed_powers)?,
                },
            });
        }
        for (position, coefficient) in relation.denominator_coefficients.iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            let mut routed_powers = try_clone_routed_powers(&state.routed_powers)?;
            routed_powers[position] = routed_powers[position].checked_sub(1).ok_or(
                FactorizedNumeratorLiftError::RoutedPowerUnderflow {
                    position,
                    power: routed_powers[position],
                },
            )?;
            children.push(FactorizedNumeratorLiftChild {
                coefficient: coefficient.clone(),
                state: FactorizedNumeratorLiftState {
                    identity: self.routing.identity.clone(),
                    remaining_power,
                    routed_powers,
                },
            });
        }
        if children.len() != self.branch_width
            || children
                .iter()
                .any(|child| child.state.remaining_power >= state.remaining_power)
        {
            return Err(FactorizedNumeratorLiftError::Invariant {
                detail: "the compiled auxiliary recurrence lost width or strict descent",
            });
        }
        Ok(children.into_boxed_slice())
    }
}

fn try_clone_routed_powers(powers: &[i64]) -> Result<Box<[i64]>, FactorizedNumeratorLiftError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(powers.len()).map_err(|_| {
        FactorizedNumeratorLiftError::AllocationFailure {
            resource: "factorized numerator child routed powers",
            requested: powers.len(),
        }
    })?;
    cloned.extend_from_slice(powers);
    Ok(cloned.into_boxed_slice())
}
