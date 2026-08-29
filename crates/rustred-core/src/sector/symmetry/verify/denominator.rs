use crate::algebra::matrix::multiply_three_coefficient_matrices;
use crate::family::IntegralFamily;

use super::super::condition::Collector;
use super::super::limits::checked_add;
use super::super::{
    CoefficientMatrix, ConditionSource, DenominatorAction, DenominatorMap, Error, ScalarProductMap,
};
use super::algebra::ReplayAlgebra;
use super::matrix::{
    clone_coefficient_column, clone_denominator_coefficient_rows,
    clone_denominator_constant_column, clone_matrix_rows, native_product,
};

pub(super) fn derive_denominator_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    scalar_products: &ScalarProductMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<DenominatorMap, Error> {
    let source_count = source.denominator_count();
    let target_count = target.denominator_count();
    let entries = source_count
        .checked_mul(target_count)
        .ok_or(Error::ResourceCountOverflow {
            resource: "affine denominator map entries",
        })?;
    algebra.charge_entries(checked_add(
        entries,
        source_count,
        "affine denominator map entries",
    )?)?;

    if source_count == 0 {
        return Ok(DenominatorMap {
            constant: Box::new([]),
            linear: CoefficientMatrix::try_new_with_max_entries(
                0,
                0,
                std::iter::empty(),
                algebra.limits.max_matrix_entries,
            )?,
        });
    }

    let source_rows =
        clone_denominator_coefficient_rows(source, algebra, "source denominator coefficient rows")?;
    let scalar_rows = clone_matrix_rows(
        &scalar_products.linear,
        algebra,
        "scalar-product linear rows",
    )?;

    // c_source + R_source h. The matrix-vector product is native; only the
    // affine translation by c_source remains coefficient-level bookkeeping.
    let scalar_constant_column = clone_coefficient_column(
        &scalar_products.constant,
        algebra,
        "scalar-product constant column",
    )?;
    algebra.charge_entries(source_count)?;
    let transformed_shift = native_product(algebra, &source_rows, &scalar_constant_column)?;
    let mut transformed_constant = Vec::new();
    transformed_constant
        .try_reserve_exact(source_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "transformed denominator constants",
            requested: source_count,
        })?;
    for denominator in 0..source_count {
        transformed_constant.push(algebra.add(
            source.denominators()[denominator].constant(),
            &transformed_shift[denominator][0],
        )?);
    }

    // P = R_source T R_target^-1. Both ordinary products are owned by one
    // authenticated Symbolica session; RustRed retains only family semantics.
    let native_limits = algebra.remaining_symbolica_limits()?;
    let (denominator_linear, stats) = match multiply_three_coefficient_matrices(
        algebra.context,
        &source_rows,
        &scalar_rows,
        target.inverse_basis(),
        native_limits,
    ) {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if denominator_linear.len() != source_count
        || denominator_linear
            .iter()
            .any(|row| row.len() != target_count)
    {
        return Err(Error::InternalSymbolicaAlgebra {
            detail: "denominator linear map product returned the wrong shape".to_owned(),
        });
    }

    // b = transformed_constant - P c_target. This matvec is native too.
    let target_constant_column =
        clone_denominator_constant_column(target, algebra, "target denominator constant column")?;
    algebra.charge_entries(source_count)?;
    let target_shift = native_product(algebra, &denominator_linear, &target_constant_column)?;
    let mut constant = Vec::new();
    constant
        .try_reserve_exact(source_count)
        .map_err(|_| Error::AllocationFailure {
            resource: "affine denominator constants",
            requested: source_count,
        })?;
    for source_denominator in 0..source_count {
        constant.push(algebra.sub(
            &transformed_constant[source_denominator],
            &target_shift[source_denominator][0],
        )?);
    }

    Ok(DenominatorMap {
        constant: constant.into_boxed_slice(),
        linear: CoefficientMatrix::try_new_with_max_entries(
            source_count,
            target_count,
            denominator_linear.into_iter().flatten(),
            algebra.limits.max_matrix_entries,
        )?,
    })
}

pub(super) fn classify_rows(
    map: &DenominatorMap,
    conditions: &mut Collector,
) -> Result<Vec<DenominatorAction>, Error> {
    let mut actions = Vec::with_capacity(map.linear.rows);
    for row in 0..map.linear.rows {
        if !map.constant[row].is_zero() {
            actions.push(DenominatorAction::Affine);
            continue;
        }
        let mut nonzero =
            (0..map.linear.columns).filter(|&column| !map.linear.at(row, column).is_zero());
        let Some(target) = nonzero.next() else {
            actions.push(DenominatorAction::Affine);
            continue;
        };
        if nonzero.next().is_some() {
            actions.push(DenominatorAction::Affine);
            continue;
        }
        let scale = map.linear.at(row, target).clone();
        conditions.add(
            scale.numerator.clone(),
            ConditionSource::DenominatorScaleNumerator {
                source_denominator: row,
                target_denominator: target,
            },
        )?;
        actions.push(DenominatorAction::Monomial { target, scale });
    }
    Ok(actions)
}
