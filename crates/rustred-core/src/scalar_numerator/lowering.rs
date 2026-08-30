use std::collections::BTreeMap;

use symbolica::atom::{Atom, AtomCore};

use crate::algebra::Coefficient;
use crate::family::IntegralKey;

use super::error::{ScalarNumeratorError, check_limit, checked_add, checked_mul};
use super::model::{LoweredScalarNumeratorTerm, ScalarNumeratorLowering};
use super::service::ScalarNumeratorService;
use super::syntax::{
    ExactComparisonBudget, census_atom, preflight_polynomial_shape, validate_scalar_syntax,
};

type ContributionKey = (Vec<i64>, u32);

impl ScalarNumeratorService<'_> {
    pub(super) fn lower_impl(
        &self,
        numerator: &Atom,
        base_integral: &IntegralKey,
    ) -> Result<ScalarNumeratorLowering, ScalarNumeratorError> {
        let family = self.artifact.family();
        validate_root_key(self.artifact, base_integral)?;
        census_atom(
            numerator.as_view(),
            "scalar-numerator input nodes",
            self.limits.max_input_nodes,
            self.limits,
        )?;
        let mut comparisons =
            ExactComparisonBudget::new(self.limits.max_loop_momentum_label_checks);
        validate_scalar_syntax(
            numerator.as_view(),
            self.dot_head,
            &self.loop_momenta,
            &mut comparisons,
        )?;
        preflight_polynomial_shape(
            numerator.as_view(),
            &self.scalar_products,
            self.limits,
            &mut comparisons,
        )?;

        let polynomial = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            numerator.to_polynomial_in_vars::<u32>(self.scalar_product_variables.clone())
        }))
        .map_err(|_| ScalarNumeratorError::SymbolicaPanic {
            operation: "scalar-product polynomial conversion",
        })?;
        check_limit(
            "scalar-numerator polynomial terms",
            polynomial.coefficients.len(),
            self.limits.max_polynomial_terms,
        )?;

        let mut output = Vec::new();
        let mut output_atom_nodes = 0usize;
        for monomial in &polynomial {
            let degree = monomial
                .exponents
                .iter()
                .try_fold(0usize, |sum, exponent| {
                    checked_add(
                        "scalar-product degree",
                        sum,
                        usize::try_from(*exponent)
                            .map_err(|_| ScalarNumeratorError::ScalarProductExponentOverflow)?,
                    )
                })?;
            check_limit(
                "scalar-product degree",
                degree,
                self.limits.max_scalar_product_degree,
            )?;

            let mut contributions = BTreeMap::new();
            contributions.insert(
                (clone_powers(base_integral.powers())?, 0),
                family.coefficient_context().one(),
            );
            for (coordinate, exponent) in monomial.exponents.iter().enumerate() {
                for _ in 0..*exponent {
                    contributions = self.expand_coordinate(contributions, coordinate)?;
                }
            }

            let requested = checked_add(
                "lowered scalar-numerator terms",
                output.len(),
                contributions.len(),
            )?;
            check_limit(
                "lowered scalar-numerator terms",
                requested,
                self.limits.max_lowered_terms,
            )?;
            output.try_reserve(contributions.len()).map_err(|_| {
                ScalarNumeratorError::AllocationFailure {
                    resource: "lowered scalar-numerator terms",
                    requested,
                }
            })?;
            let spectator_nodes = census_atom(
                monomial.coefficient.as_view(),
                "lowered scalar-numerator output nodes",
                self.limits.max_output_atom_nodes,
                self.limits,
            )?;
            let new_nodes = checked_mul(
                "lowered scalar-numerator output nodes",
                spectator_nodes,
                contributions.len(),
            )?;
            output_atom_nodes = checked_add(
                "lowered scalar-numerator output nodes",
                output_atom_nodes,
                new_nodes,
            )?;
            check_limit(
                "lowered scalar-numerator output nodes",
                output_atom_nodes,
                self.limits.max_output_atom_nodes,
            )?;

            for ((powers, mass_power), coefficient) in contributions {
                if coefficient.is_zero() {
                    continue;
                }
                let integral = IntegralKey::try_from_preallocated(powers)?;
                validate_root_key(self.artifact, &integral)?;
                output.push(LoweredScalarNumeratorTerm {
                    coefficient,
                    scalar_spectator: monomial.coefficient.clone(),
                    integral,
                    common_mass_squared_power: mass_power,
                });
            }
        }

        Ok(ScalarNumeratorLowering {
            family_identity: self.artifact.family_fingerprint_owner(),
            terms: output,
        })
    }

    fn expand_coordinate(
        &self,
        contributions: BTreeMap<ContributionKey, Coefficient>,
        coordinate: usize,
    ) -> Result<BTreeMap<ContributionKey, Coefficient>, ScalarNumeratorError> {
        let family = self.artifact.family();
        let expansion = family.scalar_product_expansion(coordinate)?;
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

        let mut next = BTreeMap::new();
        for ((powers, mass_power), coefficient) in contributions {
            if !expansion.constant().is_zero() {
                let mass_power = mass_power
                    .checked_add(1)
                    .ok_or(ScalarNumeratorError::CommonMassPowerOverflow)?;
                let coefficient = family.coefficient_context().try_mul(
                    &coefficient,
                    expansion.constant(),
                    self.limits.exact_algebra,
                )?;
                accumulate(
                    family.coefficient_context(),
                    &mut next,
                    (clone_powers(&powers)?, mass_power),
                    coefficient,
                    self.limits.exact_algebra,
                )?;
            }
            for (denominator, multiplier) in expansion.denominator_coefficients().iter().enumerate()
            {
                if multiplier.is_zero() {
                    continue;
                }
                let coefficient = family.coefficient_context().try_mul(
                    &coefficient,
                    multiplier,
                    self.limits.exact_algebra,
                )?;
                let mut shifted = clone_powers(&powers)?;
                let power = shifted[denominator];
                shifted[denominator] = power
                    .checked_sub(1)
                    .ok_or(ScalarNumeratorError::IntegralPowerUnderflow { denominator, power })?;
                accumulate(
                    family.coefficient_context(),
                    &mut next,
                    (shifted, mass_power),
                    coefficient,
                    self.limits.exact_algebra,
                )?;
            }
        }
        Ok(next)
    }
}

fn clone_powers(source: &[i64]) -> Result<Vec<i64>, ScalarNumeratorError> {
    let mut target = Vec::new();
    target.try_reserve_exact(source.len()).map_err(|_| {
        ScalarNumeratorError::AllocationFailure {
            resource: "scalar-numerator integral powers",
            requested: source.len(),
        }
    })?;
    target.extend_from_slice(source);
    Ok(target)
}

fn validate_root_key(
    artifact: &crate::foundry::artifact::ClosedArtifact,
    integral: &IntegralKey,
) -> Result<(), ScalarNumeratorError> {
    if integral.powers().len() != artifact.arity() {
        return Err(ScalarNumeratorError::WrongIntegralKeyArity {
            expected: artifact.arity(),
            actual: integral.powers().len(),
        });
    }
    for (position, (&value, bounds)) in integral
        .powers()
        .iter()
        .zip(artifact.supported_root_power_bounds())
        .enumerate()
    {
        if !bounds.contains(value) {
            return Err(ScalarNumeratorError::OutsideCertifiedRootDomain {
                position,
                value,
                lower: bounds.lower(),
                upper: bounds.upper(),
            });
        }
    }
    Ok(())
}

fn accumulate(
    context: &crate::algebra::CoefficientContext,
    target: &mut BTreeMap<ContributionKey, Coefficient>,
    key: ContributionKey,
    contribution: Coefficient,
    limits: crate::algebra::ExactAlgebraLimits,
) -> Result<(), ScalarNumeratorError> {
    if contribution.is_zero() {
        return Ok(());
    }
    if let Some(existing) = target.remove(&key) {
        let combined = context.try_add(&existing, &contribution, limits)?;
        if !combined.is_zero() {
            target.insert(key, combined);
        }
    } else {
        target.insert(key, contribution);
    }
    Ok(())
}
