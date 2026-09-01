//! Structural compilation of one authenticated `K_1^N` product chart.

use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey, ScalarProductCoordinate};

use super::super::ClosedTerminalAuthority;
use super::super::CommonMassHomogeneityProof;
use super::super::factorized_numerator_lift::{
    FactorizedNumeratorLiftLimits, compile_factorized_numerator_lift,
};
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::FactorizedProductMomentChart;
use super::resources::{admit_output_key_payload, constant_integer_magnitude_bits};

pub(crate) fn compile_factorized_product_moment_chart(
    authority: &ClosedTerminalAuthority,
    factorization_ordinal: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<FactorizedProductMomentChart<'_>, FactorizedProductMomentError> {
    let family = authority.family();
    admit_limit(
        "product chart arity",
        family.denominator_count(),
        limits.max_arity,
    )?;
    validate_parent_power_shifts(family, limits)?;
    let rule = authority
        .factorization_rules()
        .get(factorization_ordinal)
        .ok_or(FactorizedProductMomentError::MissingFactorizationRule {
            ordinal: factorization_ordinal,
        })?;
    let compiled = compile_factorized_numerator_lift(
        family,
        rule,
        FactorizedNumeratorLiftLimits {
            max_arity: limits.max_arity,
            ..FactorizedNumeratorLiftLimits::default()
        },
    )?;
    let routing = compiled.routing();
    if routing.family_fingerprint() != authority.family_fingerprint() {
        return Err(FactorizedProductMomentError::WrongFamily);
    }

    let loop_count = family.loop_count();
    if rule.factors().len() != loop_count {
        return Err(FactorizedProductMomentError::UnsupportedFactorCount {
            expected: loop_count,
            actual: rule.factors().len(),
        });
    }
    prove_signed_singleton_block_equivalence(
        rule.loop_basis().row_major(),
        routing.signed_loop_basis(),
        loop_count,
    )?;

    let active = rule.application_domain().sector().active_bits();
    if active.len() != family.denominator_count()
        || active.iter().filter(|&&entry| entry).count() != loop_count
    {
        return Err(FactorizedProductMomentError::IncompleteFactorCover);
    }
    let mut parent_by_vector =
        fallible_filled(loop_count, usize::MAX, "product parent-by-vector entries")?;
    let mut dependency_by_vector = fallible_filled(
        loop_count,
        usize::MAX,
        "product dependency-by-vector entries",
    )?;
    let mut seen_parent = fallible_filled(
        family.denominator_count(),
        false,
        "product parent occupancy",
    )?;
    for (factor_ordinal, factor) in rule.factors().iter().enumerate() {
        if factor.parent_positions().len() != 1 {
            return Err(FactorizedProductMomentError::UnsupportedFactorShape {
                factor: factor_ordinal,
                detail: "expected exactly one parent denominator",
            });
        }
        if factor.transformed_loop_positions().len() != 1 {
            return Err(FactorizedProductMomentError::UnsupportedFactorShape {
                factor: factor_ordinal,
                detail: "expected exactly one transformed loop row",
            });
        }
        let parent = factor.parent_positions()[0];
        let vector = factor.transformed_loop_positions()[0];
        if parent >= active.len() || !active[parent] || vector >= loop_count {
            return Err(FactorizedProductMomentError::UnsupportedFactorShape {
                factor: factor_ordinal,
                detail: "the singleton parent or transformed row is outside its admitted cover",
            });
        }
        if seen_parent[parent] || parent_by_vector[vector] != usize::MAX {
            return Err(FactorizedProductMomentError::IncompleteFactorCover);
        }
        let dependency_ordinal = factor.dependency_ordinal();
        let dependency = authority.dependencies().get(dependency_ordinal).ok_or(
            FactorizedProductMomentError::MissingDependency {
                ordinal: dependency_ordinal,
            },
        )?;
        if dependency.arity() != 1 {
            return Err(FactorizedProductMomentError::DependencyNotOneCoordinate {
                ordinal: dependency_ordinal,
                arity: dependency.arity(),
            });
        }
        if dependency.masters().len() != 1 {
            return Err(FactorizedProductMomentError::DependencyMasterCount {
                ordinal: dependency_ordinal,
                count: dependency.masters().len(),
            });
        }
        validate_tadpole_dependency(
            family.coefficient_context(),
            family.dimension(),
            dependency,
            dependency_ordinal,
            limits,
        )?;
        seen_parent[parent] = true;
        parent_by_vector[vector] = parent;
        dependency_by_vector[vector] = dependency_ordinal;
    }
    if parent_by_vector.contains(&usize::MAX)
        || dependency_by_vector.contains(&usize::MAX)
        || active
            .iter()
            .enumerate()
            .any(|(position, &is_active)| is_active != seen_parent[position])
    {
        return Err(FactorizedProductMomentError::IncompleteFactorCover);
    }

    let edges = complete_cross_edges(loop_count)?;
    let variable_count = loop_count.checked_add(edges.len()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product polynomial variables",
        },
    )?;
    admit_limit(
        "product polynomial variables",
        variable_count,
        limits.max_polynomial_variables,
    )?;
    let coordinate_positions = coordinate_positions(family, &edges)?;
    validate_routed_affine_coefficients(
        family.coefficient_context(),
        routing.transformed_denominators(),
        limits,
    )?;
    validate_unit_radial_denominators(
        family.coefficient_context(),
        routing.transformed_denominators(),
        &coordinate_positions,
        &parent_by_vector,
        limits,
    )?;

    if rule.master_embeddings().len() != 1 {
        return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
    }
    let embedding = &rule.master_embeddings()[0];
    let mut expected_raw_powers = fallible_filled(
        family.denominator_count(),
        0_i64,
        "product raw-master powers",
    )?;
    for factor in rule.factors() {
        let dependency = &authority.dependencies()[factor.dependency_ordinal()];
        expected_raw_powers[factor.parent_positions()[0]] = dependency
            .masters()
            .first()
            .ok_or(FactorizedProductMomentError::InvalidMasterEmbedding)?
            .powers()[0];
    }
    let expected_raw = IntegralKey::try_new(expected_raw_powers)?;
    if embedding.raw_parent_master() != &expected_raw {
        return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
    }
    admit_output_key_payload(2, family.denominator_count(), limits)?;
    let raw_master = IntegralKey::try_new(embedding.raw_parent_master().powers().iter().copied())?;
    let terminal = IntegralKey::try_new(embedding.parent_terminal().powers().iter().copied())?;

    Ok(FactorizedProductMomentChart {
        authority,
        factorization_ordinal,
        identity: Arc::new(()),
        routing: compiled,
        parent_by_vector: parent_by_vector.into_boxed_slice(),
        dependency_by_vector: dependency_by_vector.into_boxed_slice(),
        edges,
        radial_coordinate_positions: coordinate_positions.radial,
        cross_coordinate_positions: coordinate_positions.cross,
        normalization: rule.normalization().clone(),
        raw_master,
        terminal,
    })
}

fn validate_parent_power_shifts(
    family: &IntegralFamily,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for (position, shift) in family.power_shifts().iter().enumerate() {
        family
            .coefficient_context()
            .validate_with_limits(shift, limits.exact_algebra)?;
        if !shift.is_zero() {
            return Err(FactorizedProductMomentError::UnsupportedParentPowerShift { position });
        }
    }
    Ok(())
}

fn validate_routed_affine_coefficients(
    context: &CoefficientContext,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for (denominator, form) in forms.iter().enumerate() {
        for coefficient in std::iter::once(form.constant()).chain(form.scalar_coefficients()) {
            context.validate_with_limits(coefficient, limits.exact_algebra)?;
            if !coefficient.is_constant() {
                return Err(FactorizedProductMomentError::NonconstantAffineCoefficient {
                    denominator,
                });
            }
            if constant_integer_magnitude_bits(coefficient).is_none() {
                return Err(FactorizedProductMomentError::NonintegerAffineCoefficient {
                    denominator,
                });
            }
        }
    }
    Ok(())
}

fn validate_tadpole_dependency(
    parent_context: &CoefficientContext,
    parent_dimension: &Coefficient,
    dependency: &super::super::ClosedArtifact,
    ordinal: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let family = dependency.family();
    let semantic_shape = dependency.algorithm_id() == super::super::one_loop::ALGORITHM_ID
        && dependency.common_mass_homogeneity()
            == Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        && family.loop_count() == 1
        && family.external_count() == 0
        && family.denominator_count() == 1
        && family.coordinates() == [ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }]
        && parent_context.has_same_variable_map(family.coefficient_context())
        && family.power_shifts().len() == 1
        && family.power_shifts()[0].is_zero();
    if !semantic_shape {
        return Err(FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal });
    }
    if !coefficients_equal(parent_context, parent_dimension, family.dimension(), limits)? {
        return Err(FactorizedProductMomentError::DependencyDimensionMismatch { ordinal });
    }
    let denominator = &family.denominators()[0];
    if denominator.coefficients().len() != 1
        || !coefficients_equal(
            family.coefficient_context(),
            denominator.constant(),
            &family.coefficient_context().integer(-1),
            limits,
        )?
        || !coefficients_equal(
            family.coefficient_context(),
            &denominator.coefficients()[0],
            &family.coefficient_context().one(),
            limits,
        )?
        || dependency
            .masters()
            .first()
            .is_none_or(|master| master.powers() != [1])
    {
        return Err(FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal });
    }
    Ok(())
}

fn prove_signed_singleton_block_equivalence(
    original: &[i64],
    signed: &[i64],
    dimension: usize,
) -> Result<(), FactorizedProductMomentError> {
    let expected = dimension.checked_mul(dimension).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product signed-basis entries",
        },
    )?;
    if original.len() != expected || signed.len() != expected {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the compiled and installed loop bases have incompatible dimensions",
        });
    }
    for row in 0..dimension {
        let range = row * dimension..(row + 1) * dimension;
        let original_row = &original[range.clone()];
        let signed_row = &signed[range];
        let same = original_row == signed_row;
        let opposite = original_row
            .iter()
            .zip(signed_row)
            .all(|(&left, &right)| left.checked_neg().is_some_and(|negated| negated == right));
        if !same && !opposite {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a compiled singleton block is not the installed row up to sign",
            });
        }
    }
    Ok(())
}

fn complete_cross_edges(
    loop_count: usize,
) -> Result<Box<[(usize, usize)]>, FactorizedProductMomentError> {
    let count = loop_count
        .checked_mul(loop_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product cross coordinates",
        })?;
    let mut edges = Vec::new();
    edges.try_reserve_exact(count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product cross coordinates",
            requested: count,
        }
    })?;
    for left in 0..loop_count {
        for right in left + 1..loop_count {
            edges.push((left, right));
        }
    }
    Ok(edges.into_boxed_slice())
}

struct CoordinatePositions {
    radial: Box<[usize]>,
    cross: Box<[usize]>,
}

fn coordinate_positions(
    family: &IntegralFamily,
    edges: &[(usize, usize)],
) -> Result<CoordinatePositions, FactorizedProductMomentError> {
    let loop_count = family.loop_count();
    let mut radial = fallible_filled(
        loop_count,
        usize::MAX,
        "product radial coordinate positions",
    )?;
    let mut cross = fallible_filled(
        edges.len(),
        usize::MAX,
        "product cross coordinate positions",
    )?;
    for (coordinate, product) in family.coordinates().iter().enumerate() {
        let ScalarProductCoordinate::LoopLoop { left, right } = *product else {
            return Err(FactorizedProductMomentError::UnsupportedCoordinate { coordinate });
        };
        if left >= loop_count || right >= loop_count || left > right {
            return Err(FactorizedProductMomentError::UnsupportedCoordinate { coordinate });
        }
        let destination = if left == right {
            &mut radial[left]
        } else {
            let edge = edges
                .iter()
                .position(|&candidate| candidate == (left, right))
                .ok_or(FactorizedProductMomentError::UnsupportedCoordinate { coordinate })?;
            &mut cross[edge]
        };
        if *destination != usize::MAX {
            return Err(
                FactorizedProductMomentError::DuplicateScalarProductCoordinate { left, right },
            );
        }
        *destination = coordinate;
    }
    for (vector, &position) in radial.iter().enumerate() {
        if position == usize::MAX {
            return Err(
                FactorizedProductMomentError::MissingScalarProductCoordinate {
                    left: vector,
                    right: vector,
                },
            );
        }
    }
    for (edge, &position) in cross.iter().enumerate() {
        if position == usize::MAX {
            return Err(
                FactorizedProductMomentError::MissingScalarProductCoordinate {
                    left: edges[edge].0,
                    right: edges[edge].1,
                },
            );
        }
    }
    Ok(CoordinatePositions {
        radial: radial.into_boxed_slice(),
        cross: cross.into_boxed_slice(),
    })
}

fn validate_unit_radial_denominators(
    context: &CoefficientContext,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    positions: &CoordinatePositions,
    parent_by_vector: &[usize],
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let minus_one = context.integer(-1);
    let one = context.one();
    for (vector, &parent) in parent_by_vector.iter().enumerate() {
        let form =
            forms
                .get(parent)
                .ok_or(FactorizedProductMomentError::NonUnitRadialDenominator {
                    vector,
                    parent_position: parent,
                })?;
        if !coefficients_equal(context, form.constant(), &minus_one, limits)? {
            return Err(FactorizedProductMomentError::NonUnitRadialDenominator {
                vector,
                parent_position: parent,
            });
        }
        for (candidate, &position) in positions.radial.iter().enumerate() {
            let expected = if candidate == vector {
                one.clone()
            } else {
                context.zero()
            };
            if !coefficients_equal(
                context,
                &form.scalar_coefficients()[position],
                &expected,
                limits,
            )? {
                return Err(FactorizedProductMomentError::NonUnitRadialDenominator {
                    vector,
                    parent_position: parent,
                });
            }
        }
        for &position in &positions.cross {
            if !form.scalar_coefficients()[position].is_zero() {
                return Err(FactorizedProductMomentError::NonUnitRadialDenominator {
                    vector,
                    parent_position: parent,
                });
            }
        }
    }
    Ok(())
}

fn coefficients_equal(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: FactorizedProductMomentLimits,
) -> Result<bool, FactorizedProductMomentError> {
    if left == right {
        Ok(true)
    } else {
        Ok(context
            .try_sub(left, right, limits.exact_algebra)?
            .is_zero())
    }
}

fn fallible_filled<T: Clone>(
    count: usize,
    value: T,
    resource: &'static str,
) -> Result<Vec<T>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: count,
        }
    })?;
    output.resize(count, value);
    Ok(output)
}

pub(super) fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FactorizedProductMomentError> {
    if requested > limit {
        Err(FactorizedProductMomentError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;
    use crate::family::{AffineDenominator, IntegralFamily};
    use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;

    use super::{
        FactorizedProductMomentError, FactorizedProductMomentLimits, validate_parent_power_shifts,
        validate_tadpole_dependency,
    };

    #[test]
    fn semantic_admission_rejects_parent_dimension_mismatch() {
        let dependency = derive_one_loop_unit_mass_tadpole().unwrap();
        let context = dependency.family().coefficient_context();
        assert_eq!(
            validate_tadpole_dependency(
                context,
                &context.integer(4),
                &dependency,
                7,
                FactorizedProductMomentLimits::default(),
            ),
            Err(FactorizedProductMomentError::DependencyDimensionMismatch { ordinal: 7 })
        );
    }

    #[test]
    fn semantic_admission_rejects_nonzero_parent_power_shift() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let family = IntegralFamily::new(
            "factorized-product-shift-rejection-fixture",
            vec!["q".to_owned()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.one()],
        )
        .unwrap();
        assert_eq!(
            validate_parent_power_shifts(&family, FactorizedProductMomentLimits::default()),
            Err(FactorizedProductMomentError::UnsupportedParentPowerShift { position: 0 })
        );
    }
}
