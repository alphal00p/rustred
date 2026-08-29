//! Exact affine scalar-product lowering onto integral keys.

use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralKey;

use super::error::{TensorError, check_limit, checked_add, checked_mul};
use super::model::{ScalarProductLowering, TensorProjection, TensorReductionTerm};
use super::service::TensorService;
use super::syntax::census_atom;

struct Contribution {
    powers: Vec<i64>,
    coefficient: Coefficient,
}

impl TensorService<'_> {
    pub(super) fn lower_impl(
        &self,
        projection: &TensorProjection,
        base_integral: &IntegralKey,
    ) -> Result<ScalarProductLowering, TensorError> {
        let family = self.presentation().family();
        let family_identity = family.fingerprint_owner();
        if !Arc::ptr_eq(&family_identity, &projection.family_identity) {
            return Err(TensorError::ProjectionFamilyMismatch);
        }
        self.validate_integral_eligibility(base_integral)?;
        check_limit(
            "projected tensor terms admitted for lowering",
            projection.terms.len(),
            self.limits.max_projected_terms,
        )?;

        let mut lowered = Vec::new();
        let mut output_atom_nodes = 0usize;
        for projected in &projection.terms {
            family
                .coefficient_context()
                .validate_with_limits(&projected.coefficient, self.limits.exact_algebra)?;
            let mut initial_powers = Vec::new();
            initial_powers
                .try_reserve_exact(base_integral.powers().len())
                .map_err(|_| TensorError::AllocationFailure {
                    resource: "scalar-lowering integral powers",
                    requested: base_integral.powers().len(),
                })?;
            initial_powers.extend_from_slice(base_integral.powers());
            let mut contributions = Vec::new();
            contributions
                .try_reserve_exact(1)
                .map_err(|_| TensorError::AllocationFailure {
                    resource: "initial scalar-product contribution",
                    requested: 1,
                })?;
            contributions.push(Contribution {
                powers: initial_powers,
                coefficient: projected.coefficient.clone(),
            });

            for coordinate in &projected.scalar_products {
                let coordinate_index = family.coordinate_index(*coordinate)?;
                let expansion = family.scalar_product_expansion(coordinate_index)?;
                let branch_count = checked_add(
                    "affine scalar-product branches",
                    family.denominator_count(),
                    1,
                )?;
                let upper_bound = checked_mul(
                    "affine scalar-product contributions",
                    contributions.len(),
                    branch_count,
                )?;
                check_limit(
                    "affine scalar-product contributions",
                    upper_bound,
                    self.limits.max_lowered_terms,
                )?;
                let mut next = Vec::new();
                next.try_reserve_exact(upper_bound).map_err(|_| {
                    TensorError::AllocationFailure {
                        resource: "affine scalar-product contributions",
                        requested: upper_bound,
                    }
                })?;
                for contribution in contributions {
                    if !expansion.constant().is_zero() {
                        let coefficient = family.coefficient_context().try_mul(
                            &contribution.coefficient,
                            expansion.constant(),
                            self.limits.exact_algebra,
                        )?;
                        if !coefficient.is_zero() {
                            next.push(Contribution {
                                powers: clone_powers(&contribution.powers)?,
                                coefficient,
                            });
                        }
                    }
                    for (denominator, multiplier) in
                        expansion.denominator_coefficients().iter().enumerate()
                    {
                        if multiplier.is_zero() {
                            continue;
                        }
                        let coefficient = family.coefficient_context().try_mul(
                            &contribution.coefficient,
                            multiplier,
                            self.limits.exact_algebra,
                        )?;
                        if coefficient.is_zero() {
                            continue;
                        }
                        let mut powers = clone_powers(&contribution.powers)?;
                        let power = powers[denominator];
                        powers[denominator] = power
                            .checked_sub(1)
                            .ok_or(TensorError::IntegralPowerUnderflow { denominator, power })?;
                        next.push(Contribution {
                            powers,
                            coefficient,
                        });
                    }
                }
                contributions = next;
            }
            contributions.sort_by(|left, right| left.powers.cmp(&right.powers));

            let requested =
                checked_add("lowered tensor terms", lowered.len(), contributions.len())?;
            check_limit(
                "lowered tensor terms",
                requested,
                self.limits.max_lowered_terms,
            )?;
            lowered.try_reserve(contributions.len()).map_err(|_| {
                TensorError::AllocationFailure {
                    resource: "lowered tensor terms",
                    requested,
                }
            })?;

            let spectator_nodes = census_atom(
                projected.scalar_spectator.as_view(),
                "lowered output Atom nodes",
                self.limits.max_output_atom_nodes,
                self.limits,
            )?;
            let tensor_nodes = census_atom(
                projected.outside_tensor.as_view(),
                "lowered output Atom nodes",
                self.limits.max_output_atom_nodes,
                self.limits,
            )?;
            let nodes_per_term =
                checked_add("lowered output Atom nodes", spectator_nodes, tensor_nodes)?;
            let new_nodes = checked_mul(
                "lowered output Atom nodes",
                nodes_per_term,
                contributions.len(),
            )?;
            output_atom_nodes =
                checked_add("lowered output Atom nodes", output_atom_nodes, new_nodes)?;
            check_limit(
                "lowered output Atom nodes",
                output_atom_nodes,
                self.limits.max_output_atom_nodes,
            )?;

            for contribution in contributions {
                lowered.push(TensorReductionTerm {
                    coefficient: contribution.coefficient,
                    scalar_spectator: projected.scalar_spectator.clone(),
                    outside_tensor: projected.outside_tensor.clone(),
                    integral: IntegralKey::try_from_preallocated(contribution.powers)?,
                });
            }
        }
        Ok(ScalarProductLowering {
            family_identity,
            lane: projection.lane,
            terms: lowered,
            guards: projection.guards.clone(),
        })
    }
}

fn clone_powers(source: &[i64]) -> Result<Vec<i64>, TensorError> {
    let mut target = Vec::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| TensorError::AllocationFailure {
            resource: "scalar-lowering integral powers",
            requested: source.len(),
        })?;
    target.extend_from_slice(source);
    Ok(target)
}
