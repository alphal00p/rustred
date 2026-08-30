//! Generic proof replay for lower-family product factorizations.

use std::collections::BTreeSet;

use crate::algebra::Coefficient;
use crate::family::{
    AffineDenominator, IntegralFamily, ScalarProductCoordinate, congruence_symbolic_matrix,
    invert_symbolic_matrix,
};

use super::super::error::ArtifactError;
use super::super::factorization::{FactorizationMasterEmbedding, FactorizationRule};
use super::ClosingArtifactCandidate;

const MAX_MASTER_EMBEDDINGS_PER_RULE: usize = 1_000_000;

pub(super) fn validate_and_compile(
    candidate: &mut ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    for rule_ordinal in 0..candidate.factorization_rules.len() {
        let rule = &candidate.factorization_rules[rule_ordinal];
        if rule.application_domain().arity() != candidate.arity {
            return Err(ArtifactError::InvalidFactorization {
                detail: "the factorization domain has a foreign shape",
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
        for (&active, bounds) in sector
            .active_bits()
            .iter()
            .zip(rule.application_domain().bounds())
        {
            let corner = if active { 1 } else { 0 };
            if bounds.lower() != corner || (!active && bounds.upper() != 0) {
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
            {
                return Err(ArtifactError::InvalidFactorization {
                    detail: "a factorization projection or dependency artifact is incompatible",
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
        let embeddings = compile_embedded_master_products(candidate, rule)?;
        candidate.factorization_rules[rule_ordinal].install_master_embeddings(embeddings);
    }
    Ok(())
}

/// Prove once at installation that every Cartesian product of typed
/// dependency masters embeds into an explicit parent-family terminal. The
/// reducer can then perform the same projection without any hot-path artifact
/// authentication.
fn compile_embedded_master_products(
    candidate: &ClosingArtifactCandidate,
    rule: &FactorizationRule,
) -> Result<Vec<FactorizationMasterEmbedding>, ArtifactError> {
    let embedding_count = rule.factors().iter().try_fold(1_usize, |count, factor| {
        count.checked_mul(
            candidate.dependencies[factor.dependency_ordinal()]
                .masters()
                .len(),
        )
    });
    let Some(embedding_count) = embedding_count else {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the dependency-master Cartesian product cardinality overflowed",
        });
    };
    if embedding_count > MAX_MASTER_EMBEDDINGS_PER_RULE {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the dependency-master Cartesian product exceeds the installation limit",
        });
    }
    let mut embeddings = Vec::new();
    embeddings.try_reserve_exact(embedding_count).map_err(|_| {
        ArtifactError::InvalidFactorization {
            detail: "could not allocate the dependency-master embedding table",
        }
    })?;
    let mut parent_powers = vec![0_i64; candidate.arity];
    compile_embedded_master_product_at(candidate, rule, 0, &mut parent_powers, &mut embeddings)?;
    embeddings
        .sort_unstable_by(|left, right| left.raw_parent_master().cmp(right.raw_parent_master()));
    if embeddings.len() != embedding_count
        || embeddings
            .windows(2)
            .any(|pair| pair[0].raw_parent_master() == pair[1].raw_parent_master())
    {
        return Err(ArtifactError::InvalidFactorization {
            detail: "the dependency-master embedding table is incomplete or non-injective",
        });
    }
    Ok(embeddings)
}

fn compile_embedded_master_product_at(
    candidate: &ClosingArtifactCandidate,
    rule: &FactorizationRule,
    factor_ordinal: usize,
    parent_powers: &mut [i64],
    embeddings: &mut Vec<FactorizationMasterEmbedding>,
) -> Result<(), ArtifactError> {
    let Some(factor) = rule.factors().get(factor_ordinal) else {
        let raw = crate::family::IntegralKey::try_new(parent_powers.iter().copied())?;
        let terminal = match &candidate.canonicalizer {
            Some(canonicalizer) => canonicalizer.canonicalize(&raw)?.canonical().clone(),
            None => raw.clone(),
        };
        if !candidate.masters.contains(&terminal) {
            return Err(ArtifactError::InvalidFactorization {
                detail: "a dependency-master product does not embed into a parent master terminal",
            });
        }
        embeddings.push(FactorizationMasterEmbedding::new(raw, terminal));
        return Ok(());
    };
    let dependency = &candidate.dependencies[factor.dependency_ordinal()];
    for master in dependency.masters() {
        for (&parent_position, &power) in factor.parent_positions().iter().zip(master.powers()) {
            parent_powers[parent_position] = power;
        }
        compile_embedded_master_product_at(
            candidate,
            rule,
            factor_ordinal + 1,
            parent_powers,
            embeddings,
        )?;
    }
    for &parent_position in factor.parent_positions() {
        parent_powers[parent_position] = 0;
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

#[cfg(test)]
#[path = "factorization/tests.rs"]
mod tests;
