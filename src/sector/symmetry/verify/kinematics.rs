use crate::algebra::matrix::congruence_of_coefficient_matrix;
use crate::family::{IntegralFamily, ScalarProductCoordinate};

use super::super::limits::checked_add;
use super::super::{CoefficientMatrix, Error, MomentumMap, ScalarProductMap};
use super::algebra::ReplayAlgebra;
use super::matrix::clone_matrix_rows;

pub(super) fn verify_external_gram(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), Error> {
    let externals = source.external_count();
    if externals == 0 {
        return Ok(());
    }

    let transform = clone_matrix_rows(
        &momentum.external_linear,
        algebra,
        "external Gram transform rows",
    )?;
    algebra.charge_entries(externals.checked_mul(externals).ok_or(
        Error::ResourceCountOverflow {
            resource: "mapped external Gram entries",
        },
    )?)?;
    let native_limits = algebra.remaining_symbolica_limits()?;
    let (mapped, stats) = match congruence_of_coefficient_matrix(
        algebra.context,
        &transform,
        target.external_gram(),
        native_limits,
    ) {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if mapped.len() != externals || mapped.iter().any(|row| row.len() != externals) {
        return Err(Error::InternalSymbolicaAlgebra {
            detail: "external Gram congruence returned the wrong shape".to_owned(),
        });
    }
    for mu in 0..externals {
        for nu in 0..externals {
            if !algebra.equal(&mapped[mu][nu], &source.external_gram()[mu][nu])? {
                return Err(Error::ExternalGramMismatch {
                    row: mu,
                    column: nu,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn derive_scalar_product_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<ScalarProductMap, Error> {
    let source_count = source.denominator_count();
    let target_count = target.denominator_count();
    let entries = source_count
        .checked_mul(target_count)
        .ok_or(Error::ResourceCountOverflow {
            resource: "scalar-product map entries",
        })?;
    algebra.charge_entries(checked_add(
        entries,
        source_count,
        "scalar-product map entries",
    )?)?;
    let mut constant = vec![algebra.context.zero(); source_count];
    let mut linear = vec![algebra.context.zero(); entries];

    for (source_coordinate, coordinate) in source.coordinates().iter().copied().enumerate() {
        match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                for alpha in 0..source.external_count() {
                    for beta in 0..source.external_count() {
                        let product = algebra.mul(
                            momentum.loop_external.at(left, alpha),
                            momentum.loop_external.at(right, beta),
                        )?;
                        let contribution =
                            algebra.mul(&product, &target.external_gram()[alpha][beta])?;
                        constant[source_coordinate] =
                            algebra.add(&constant[source_coordinate], &contribution)?;
                    }
                }
                for first in 0..source.loop_count() {
                    for second in first..source.loop_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopLoop {
                                left: first,
                                right: second,
                            })?;
                        let value = if first == second {
                            algebra.mul(
                                momentum.loop_linear.at(left, first),
                                momentum.loop_linear.at(right, first),
                            )?
                        } else {
                            let direct = algebra.mul(
                                momentum.loop_linear.at(left, first),
                                momentum.loop_linear.at(right, second),
                            )?;
                            let crossed = algebra.mul(
                                momentum.loop_linear.at(left, second),
                                momentum.loop_linear.at(right, first),
                            )?;
                            algebra.add(&direct, &crossed)?
                        };
                        linear[source_coordinate * target_count + target_coordinate] = value;
                    }
                }
                for target_loop in 0..source.loop_count() {
                    for external in 0..source.external_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                loop_index: target_loop,
                                external_index: external,
                            })?;
                        let direct = algebra.mul(
                            momentum.loop_linear.at(left, target_loop),
                            momentum.loop_external.at(right, external),
                        )?;
                        let crossed = algebra.mul(
                            momentum.loop_linear.at(right, target_loop),
                            momentum.loop_external.at(left, external),
                        )?;
                        linear[source_coordinate * target_count + target_coordinate] =
                            algebra.add(&direct, &crossed)?;
                    }
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                for alpha in 0..source.external_count() {
                    for beta in 0..source.external_count() {
                        let product = algebra.mul(
                            momentum.loop_external.at(loop_index, alpha),
                            momentum.external_linear.at(external_index, beta),
                        )?;
                        let contribution =
                            algebra.mul(&product, &target.external_gram()[alpha][beta])?;
                        constant[source_coordinate] =
                            algebra.add(&constant[source_coordinate], &contribution)?;
                    }
                }
                for target_loop in 0..source.loop_count() {
                    for target_external in 0..source.external_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                loop_index: target_loop,
                                external_index: target_external,
                            })?;
                        linear[source_coordinate * target_count + target_coordinate] = algebra
                            .mul(
                                momentum.loop_linear.at(loop_index, target_loop),
                                momentum.external_linear.at(external_index, target_external),
                            )?;
                    }
                }
            }
        }
    }

    Ok(ScalarProductMap {
        constant: constant.into_boxed_slice(),
        linear: CoefficientMatrix::try_new_with_max_entries(
            source_count,
            target_count,
            linear,
            algebra.limits.max_matrix_entries,
        )?,
    })
}
