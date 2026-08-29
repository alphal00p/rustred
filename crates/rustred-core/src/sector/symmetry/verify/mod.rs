mod algebra;
mod denominator;
mod kinematics;
mod matrix;
mod replay;

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;

use self::algebra::ReplayAlgebra;
use self::denominator::{classify_rows, derive_denominator_map};
use self::kinematics::{derive_scalar_product_map, verify_external_gram};
use self::matrix::checked_determinant;
use self::replay::replay_denominator_map;
use super::condition::{Collector, collect_candidate_denominators};
use super::{
    CoefficientMatrix, ConditionSource, Error, Jacobian, Limits, MomentumMap, VerifiedMap,
};

/// Derive and independently replay the exact family map induced by an
/// explicit momentum substitution.
pub fn verify(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: MomentumMap,
    limits: Limits,
) -> Result<VerifiedMap, Error> {
    if source.loop_count() != target.loop_count() {
        return Err(Error::UnequalLoopCount {
            source: source.loop_count(),
            target: target.loop_count(),
        });
    }
    if source.external_count() != target.external_count() {
        return Err(Error::UnequalExternalCount {
            source: source.external_count(),
            target: target.external_count(),
        });
    }
    if !source
        .coefficient_context()
        .has_same_variable_map(target.coefficient_context())
    {
        return Err(Error::ForeignCoefficientContext);
    }

    let loops = source.loop_count();
    let externals = source.external_count();
    check_shape(&momentum.loop_linear, "A", loops, loops)?;
    check_shape(&momentum.loop_external, "B", loops, externals)?;
    check_shape(&momentum.external_linear, "C", externals, externals)?;

    let mut algebra = ReplayAlgebra::new(source.coefficient_context(), limits);
    algebra.retain_matrix(&momentum.loop_linear, "A")?;
    algebra.retain_matrix(&momentum.loop_external, "B")?;
    algebra.retain_matrix(&momentum.external_linear, "C")?;

    let mut conditions = Collector::new(limits);
    conditions.add_family_domain(source.domain(), true)?;
    conditions.add_family_domain(target.domain(), false)?;
    let candidate_denominator_conditions = collect_candidate_denominators(
        [
            ("A", &momentum.loop_linear),
            ("B", &momentum.loop_external),
            ("C", &momentum.external_linear),
        ],
        &mut conditions,
    )?;

    let loop_determinant = checked_determinant(&momentum.loop_linear, &mut algebra)?;
    if loop_determinant.is_zero() {
        return Err(Error::SingularLoopMap);
    }
    conditions.add(
        loop_determinant.numerator.clone(),
        ConditionSource::LoopMapDeterminantNumerator,
    )?;
    let external_determinant = checked_determinant(&momentum.external_linear, &mut algebra)?;
    if external_determinant.is_zero() {
        return Err(Error::SingularExternalMap);
    }
    conditions.add(
        external_determinant.numerator.clone(),
        ConditionSource::ExternalMapDeterminantNumerator,
    )?;

    verify_external_gram(source, target, &momentum, &mut algebra)?;
    let jacobian = classify_jacobian(&loop_determinant, &mut algebra)?;
    let scalar_products = derive_scalar_product_map(source, target, &momentum, &mut algebra)?;
    let denominators = derive_denominator_map(source, target, &scalar_products, &mut algebra)?;
    replay_denominator_map(source, target, &momentum, &denominators, &mut algebra)?;
    let row_actions = classify_rows(&denominators, &mut conditions)?;

    algebra.stats.nonzero_conditions = conditions.condition_count();
    algebra.stats.condition_sources = conditions.source_count();
    let nonzero_conditions = conditions.finish();

    Ok(VerifiedMap {
        source_family_fingerprint: source.fingerprint(),
        target_family_fingerprint: target.fingerprint(),
        momentum,
        scalar_products,
        denominators,
        row_actions: row_actions.into_boxed_slice(),
        loop_determinant,
        external_determinant,
        jacobian,
        source_domain: source.domain().clone(),
        target_domain: target.domain().clone(),
        candidate_denominator_conditions: candidate_denominator_conditions.into_boxed_slice(),
        nonzero_conditions,
        stats: algebra.stats,
    })
}

fn check_shape(
    matrix: &CoefficientMatrix,
    name: &'static str,
    rows: usize,
    columns: usize,
) -> Result<(), Error> {
    if matrix.rows == rows && matrix.columns == columns {
        Ok(())
    } else {
        Err(Error::WrongMatrixShape {
            matrix: name,
            expected_rows: rows,
            expected_columns: columns,
            actual_rows: matrix.rows,
            actual_columns: matrix.columns,
        })
    }
}

fn classify_jacobian(
    determinant: &Coefficient,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<Jacobian, Error> {
    let one = algebra.context.one();
    if algebra.equal(determinant, &one)? {
        return Ok(Jacobian::Unit {
            determinant_sign: 1,
        });
    }
    let negative_one = algebra.context.integer(-1);
    if algebra.equal(determinant, &negative_one)? {
        return Ok(Jacobian::Unit {
            determinant_sign: -1,
        });
    }
    Ok(Jacobian::FormalDeterminantPower {
        determinant: determinant.clone(),
    })
}
