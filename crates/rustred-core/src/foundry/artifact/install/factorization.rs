//! Generic proof replay for lower-family product factorizations.

use std::collections::BTreeSet;

use crate::algebra::Coefficient;
use crate::family::{
    AffineDenominator, IntegralFamily, ScalarProductCoordinate, congruence_symbolic_matrix,
    invert_symbolic_matrix,
};

use super::super::error::ArtifactError;
use super::super::factorization::FactorizationRule;
use super::ClosingArtifactCandidate;

pub(super) fn validate(candidate: &ClosingArtifactCandidate) -> Result<(), ArtifactError> {
    for rule in &candidate.factorization_rules {
        if rule.application_domain().arity() != candidate.arity
            || rule.parent_master().powers().len() != candidate.arity
            || !candidate.masters.contains(rule.parent_master())
        {
            return Err(ArtifactError::InvalidFactorization {
                detail: "the factorization domain or parent master has a foreign shape",
            });
        }
        candidate
            .family
            .coefficient_context()
            .validate_with_limits(rule.normalization(), Default::default())
            .map_err(|_| ArtifactError::InvalidFactorization {
                detail: "the normalization belongs to a foreign coefficient context",
            })?;
        if rule.normalization().is_zero() || rule.factors().is_empty() {
            return Err(ArtifactError::InvalidFactorization {
                detail: "a factorization needs a nonzero normalization and at least one factor",
            });
        }

        let sector = rule.application_domain().sector();
        for (position, (&active, bounds)) in sector
            .active_bits()
            .iter()
            .zip(rule.application_domain().bounds())
            .enumerate()
        {
            let corner = if active { 1 } else { 0 };
            if rule.parent_master().powers()[position] != corner
                || bounds.lower() != corner
                || (!active && bounds.upper() != 0)
            {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "a factorization cell is not a sector-corner product domain",
                });
            }
        }

        for factor in rule.factors() {
            let dependency = candidate
                .dependencies
                .get(factor.dependency_ordinal())
                .ok_or(ArtifactError::InvalidFactorization {
                    detail: "a factorization references an absent dependency artifact",
                })?;
            if !dependency
                .coefficient_context()
                .has_same_variable_map(candidate.family.coefficient_context())
                || factor.parent_positions().len() != dependency.arity()
                || !dependency.masters().contains(factor.dependency_master())
            {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "a factorization projection or dependency master is incompatible",
                });
            }
            let mut seen = BTreeSet::new();
            for &position in factor.parent_positions() {
                if position >= candidate.arity || !seen.insert(position) {
                    return Err(ArtifactError::InvalidFactorization {
                        detail: "a factorization projection is out of range or repeats a coordinate",
                    });
                }
            }
        }
        validate_kinematics(candidate, rule)?;
    }
    Ok(())
}

fn validate_kinematics(
    candidate: &ClosingArtifactCandidate,
    rule: &FactorizationRule,
) -> Result<(), ArtifactError> {
    let family = &candidate.family;
    let loop_count = family.loop_count();
    let basis = rule.loop_basis();
    let expected_entries =
        loop_count
            .checked_mul(loop_count)
            .ok_or(ArtifactError::InvalidFactorization {
                detail: "the loop-basis matrix shape overflowed",
            })?;
    if family.external_count() != 0
        || basis.dimension() != loop_count
        || basis.row_major().len() != expected_entries
        || rule.normalization() != &family.coefficient_context().one()
    {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the vacuum factorization has a malformed loop basis or normalization",
        });
    }

    let context = family.coefficient_context();
    let matrix = coefficient_matrix_from_i64(context, basis.row_major(), loop_count)?;
    let (inverse, determinant) =
        invert_symbolic_matrix(context, &matrix, family.construction_limits())?;
    let minus_one = context.integer(-1);
    if determinant != context.one() && determinant != minus_one {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the factorization loop-basis change is not unimodular",
        });
    }
    let inverse_transpose = transpose_square(&inverse)?;

    let mut owned_parent_denominators = BTreeSet::new();
    let mut owned_transformed_loops = BTreeSet::new();
    for factor in rule.factors() {
        let dependency = &candidate.dependencies[factor.dependency_ordinal()];
        if dependency.family().external_count() != 0
            || factor.transformed_loop_positions().len() != dependency.family().loop_count()
        {
            return Err(ArtifactError::InvalidFactorization {
                detail: "a factor does not own exactly one dependency loop block",
            });
        }
        for &loop_position in factor.transformed_loop_positions() {
            if loop_position >= loop_count || !owned_transformed_loops.insert(loop_position) {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "transformed factor loop blocks overlap or leave their basis",
                });
            }
        }
        for (dependency_denominator, &parent_denominator) in
            factor.parent_positions().iter().enumerate()
        {
            if !owned_parent_denominators.insert(parent_denominator)
                || !rule
                    .application_domain()
                    .sector()
                    .is_active(parent_denominator)?
            {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "factor denominator blocks overlap or reference an inactive line",
                });
            }
            verify_factor_denominator(
                family,
                parent_denominator,
                dependency.family(),
                dependency_denominator,
                factor.transformed_loop_positions(),
                &inverse_transpose,
            )?;
        }
    }
    if owned_transformed_loops.len() != loop_count
        || rule
            .application_domain()
            .sector()
            .active_bits()
            .iter()
            .enumerate()
            .any(|(position, &active)| active != owned_parent_denominators.contains(&position))
    {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the certified factor blocks do not partition every loop and active denominator",
        });
    }
    Ok(())
}

fn coefficient_matrix_from_i64(
    context: &crate::algebra::CoefficientContext,
    entries: &[i64],
    dimension: usize,
) -> Result<Vec<Vec<Coefficient>>, ArtifactError> {
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(dimension)
        .map_err(|_| ArtifactError::InvalidFactorization {
            detail: "could not allocate the factorization loop-basis matrix",
        })?;
    for row in entries.chunks_exact(dimension) {
        let mut output = Vec::new();
        output
            .try_reserve_exact(dimension)
            .map_err(|_| ArtifactError::InvalidFactorization {
                detail: "could not allocate a factorization loop-basis row",
            })?;
        output.extend(row.iter().map(|&entry| context.integer(entry)));
        matrix.push(output);
    }
    Ok(matrix)
}

fn transpose_square(matrix: &[Vec<Coefficient>]) -> Result<Vec<Vec<Coefficient>>, ArtifactError> {
    let dimension = matrix.len();
    if matrix.iter().any(|row| row.len() != dimension) {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the authenticated inverse loop basis is not square",
        });
    }
    let mut transpose = Vec::new();
    transpose
        .try_reserve_exact(dimension)
        .map_err(|_| ArtifactError::InvalidFactorization {
            detail: "could not allocate the inverse-transpose loop basis",
        })?;
    for column in 0..dimension {
        let mut row = Vec::new();
        row.try_reserve_exact(dimension)
            .map_err(|_| ArtifactError::InvalidFactorization {
                detail: "could not allocate an inverse-transpose loop-basis row",
            })?;
        row.extend(matrix.iter().map(|input| input[column].clone()));
        transpose.push(row);
    }
    Ok(transpose)
}

fn verify_factor_denominator(
    parent: &IntegralFamily,
    parent_denominator: usize,
    dependency: &IntegralFamily,
    dependency_denominator: usize,
    transformed_loops: &[usize],
    inverse_transpose: &[Vec<Coefficient>],
) -> Result<(), ArtifactError> {
    let parent_denominator = parent.denominators().get(parent_denominator).ok_or(
        ArtifactError::InvalidFactorization {
            detail: "a factor parent denominator is absent",
        },
    )?;
    let dependency_denominator = dependency
        .denominators()
        .get(dependency_denominator)
        .ok_or(ArtifactError::InvalidFactorization {
            detail: "a factor dependency denominator is absent",
        })?;
    let context = parent.coefficient_context();
    if !dependency
        .coefficient_context()
        .has_same_variable_map(context)
        || !coefficients_equal(
            context,
            parent_denominator.constant(),
            dependency_denominator.constant(),
            parent.construction_limits().exact_algebra,
        )?
    {
        return Err(ArtifactError::InvalidFactorization {
            detail: "a transformed denominator has a foreign constant",
        });
    }
    let parent_quadratic = denominator_quadratic(parent, parent_denominator)?;
    let transformed = congruence_symbolic_matrix(
        context,
        inverse_transpose,
        &parent_quadratic,
        parent.construction_limits(),
    )?;
    let dependency_quadratic = denominator_quadratic(dependency, dependency_denominator)?;
    let zero = dependency.coefficient_context().zero();
    for (row, actual_row) in transformed.iter().enumerate() {
        for (column, actual) in actual_row.iter().enumerate() {
            let expected = match (
                transformed_loops.iter().position(|&index| index == row),
                transformed_loops.iter().position(|&index| index == column),
            ) {
                (Some(local_row), Some(local_column)) => {
                    &dependency_quadratic[local_row][local_column]
                }
                _ => &zero,
            };
            if !coefficients_equal(
                context,
                actual,
                expected,
                parent.construction_limits().exact_algebra,
            )? {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "the unimodular loop basis does not block-diagonalize a factor denominator",
                });
            }
        }
    }
    Ok(())
}

fn denominator_quadratic(
    family: &IntegralFamily,
    denominator: &AffineDenominator,
) -> Result<Vec<Vec<Coefficient>>, ArtifactError> {
    let context = family.coefficient_context();
    let loops = family.loop_count();
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(loops)
        .map_err(|_| ArtifactError::InvalidFactorization {
            detail: "could not allocate a denominator quadratic matrix",
        })?;
    for _ in 0..loops {
        let mut row = Vec::new();
        row.try_reserve_exact(loops)
            .map_err(|_| ArtifactError::InvalidFactorization {
                detail: "could not allocate a denominator quadratic row",
            })?;
        row.resize_with(loops, || context.zero());
        matrix.push(row);
    }
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                matrix[left][right] = coefficient.clone();
            }
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let half = context
                    .try_div(
                        coefficient,
                        &context.integer(2),
                        family.construction_limits().exact_algebra,
                    )
                    .map_err(crate::family::IntegralFamilyError::from)?;
                matrix[left][right] = half.clone();
                matrix[right][left] = half;
            }
            ScalarProductCoordinate::LoopExternal { .. } => {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "vacuum factorization received an external-momentum coordinate",
                });
            }
        }
    }
    Ok(matrix)
}

fn coefficients_equal(
    context: &crate::algebra::CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: crate::algebra::ExactAlgebraLimits,
) -> Result<bool, ArtifactError> {
    if left == right {
        Ok(true)
    } else {
        Ok(context
            .try_sub(left, right, limits)
            .map_err(crate::family::IntegralFamilyError::from)?
            .is_zero())
    }
}
