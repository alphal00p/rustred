use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, ScalarProductCoordinate};

use super::super::{DenominatorMap, Error, MomentumMap};
use super::algebra::ReplayAlgebra;

fn add_exact_product(
    accumulator: &mut Coefficient,
    factors: &[&Coefficient],
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), Error> {
    debug_assert!(!factors.is_empty());
    let mut product = factors[0].clone();
    for factor in &factors[1..] {
        product = algebra.mul(&product, factor)?;
    }
    *accumulator = algebra.add(accumulator, &product)?;
    Ok(())
}

/// Replay each source denominator by a fresh bilinear expansion of `A/B/C/G`.
///
/// This route deliberately does not consume `ScalarProductMap`: ordered
/// loop-loop pairs are folded into the target upper triangle directly, so an
/// orientation or off-diagonal-factor defect in the retained map cannot
/// certify itself.
pub(super) fn replay_denominator_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    denominators: &DenominatorMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), Error> {
    let target_count = target.denominator_count();
    algebra.charge_entries(target_count)?;
    for source_denominator in 0..source.denominator_count() {
        let mut direct_constant = source.denominators()[source_denominator].constant().clone();
        let mut direct_linear = vec![algebra.context.zero(); target_count];
        for (source_coordinate, coordinate) in source.coordinates().iter().copied().enumerate() {
            let weight =
                &source.denominators()[source_denominator].coefficients()[source_coordinate];
            match coordinate {
                ScalarProductCoordinate::LoopLoop { left, right } => {
                    // Sum ordered target-loop pairs, then fold (a,b) and (b,a)
                    // into the same upper-triangular scalar coordinate.
                    for first in 0..source.loop_count() {
                        for second in 0..source.loop_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopLoop {
                                    left: first.min(second),
                                    right: first.max(second),
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(left, first),
                                    momentum.loop_linear.at(right, second),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for target_loop in 0..source.loop_count() {
                        for external in 0..source.external_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                    loop_index: target_loop,
                                    external_index: external,
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(left, target_loop),
                                    momentum.loop_external.at(right, external),
                                ],
                                algebra,
                            )?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_external.at(left, external),
                                    momentum.loop_linear.at(right, target_loop),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for alpha in 0..source.external_count() {
                        for beta in 0..source.external_count() {
                            add_exact_product(
                                &mut direct_constant,
                                &[
                                    weight,
                                    momentum.loop_external.at(left, alpha),
                                    momentum.loop_external.at(right, beta),
                                    &target.external_gram()[alpha][beta],
                                ],
                                algebra,
                            )?;
                        }
                    }
                }
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                } => {
                    for target_loop in 0..source.loop_count() {
                        for target_external in 0..source.external_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                    loop_index: target_loop,
                                    external_index: target_external,
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(loop_index, target_loop),
                                    momentum.external_linear.at(external_index, target_external),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for alpha in 0..source.external_count() {
                        for beta in 0..source.external_count() {
                            add_exact_product(
                                &mut direct_constant,
                                &[
                                    weight,
                                    momentum.loop_external.at(loop_index, alpha),
                                    momentum.external_linear.at(external_index, beta),
                                    &target.external_gram()[alpha][beta],
                                ],
                                algebra,
                            )?;
                        }
                    }
                }
            }
        }

        let mut mapped_constant = denominators.constant[source_denominator].clone();
        for target_denominator in 0..target.denominator_count() {
            mapped_constant = algebra.add_product(
                mapped_constant,
                denominators
                    .linear
                    .at(source_denominator, target_denominator),
                target.denominators()[target_denominator].constant(),
            )?;
        }
        if !algebra.equal(&direct_constant, &mapped_constant)? {
            return Err(Error::DenominatorReplayMismatch {
                denominator: source_denominator,
                coordinate: None,
            });
        }

        for target_coordinate in 0..target_count {
            let mut mapped = algebra.context.zero();
            for target_denominator in 0..target.denominator_count() {
                mapped = algebra.add_product(
                    mapped,
                    denominators
                        .linear
                        .at(source_denominator, target_denominator),
                    &target.denominators()[target_denominator].coefficients()[target_coordinate],
                )?;
            }
            if !algebra.equal(&direct_linear[target_coordinate], &mapped)? {
                return Err(Error::DenominatorReplayMismatch {
                    denominator: source_denominator,
                    coordinate: Some(target_coordinate),
                });
            }
        }
    }
    Ok(())
}
