//! Bounded canonical momentum charts and complete lower-family construction.
use std::collections::BTreeMap;
use std::sync::Arc;

use super::{
    VacuumProductError as Error, VacuumProductLimits, VacuumProductStatistics, algebra, charge,
};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, invert_and_verify_coefficient_matrix,
    multiply_coefficient_matrices, rank_of_coefficient_matrix,
};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, symbolica_matrix_limits};
use crate::reduction::terminal_normalization::products;
use symbolica::prelude::Integer;

pub(super) struct CanonicalFactor {
    pub family: Arc<IntegralFamily>,
    /// Original parent slots in canonical lower-family active-slot order.
    /// Each alignment retains its exact old-block to canonical-loop map.
    pub alignments: BTreeMap<Vec<usize>, Vec<Vec<Coefficient>>>,
}

pub(super) struct MomentumChart {
    pub rows: Vec<Vec<Coefficient>>,
    /// Old loop coordinates in the new momentum basis.
    pub inverse: Vec<Vec<Coefficient>>,
}

/// Enumerate combinations without retaining the combinatorial search space.
/// Returning false from the callback stops at the first accepted witness.
pub(super) fn combinations(
    count: usize,
    width: usize,
    callback: &mut impl FnMut(&[usize]) -> Result<bool, Error>,
) -> Result<(), Error> {
    if width == 0 || width > count {
        return Ok(());
    }
    let mut selection = (0..width).collect::<Vec<_>>();
    loop {
        if !callback(&selection)? {
            return Ok(());
        }
        let Some(axis) = (0..width)
            .rev()
            .find(|&axis| selection[axis] < count - width + axis)
        else {
            return Ok(());
        };
        selection[axis] += 1;
        for next in axis + 1..width {
            selection[next] = selection[next - 1] + 1;
        }
    }
}

/// Symbolica owns inversion/determinant and both multiplications. Replay of
/// every active momentum row in the original basis authenticates the chart.
pub(super) fn unimodular_transform(
    family: &IntegralFamily,
    rows: &[Vec<Coefficient>],
    selection: &[usize],
) -> Result<Option<MomentumChart>, Error> {
    let context = family.coefficient_context();
    let limits = symbolica_matrix_limits(family.construction_limits());
    let basis = selection
        .iter()
        .map(|&i| rows[i].clone())
        .collect::<Vec<_>>();
    let verified = match invert_and_verify_coefficient_matrix(context, &basis, limits) {
        Ok(verified) => verified,
        Err(SymbolicaCoefficientMatrixError::Singular) => return Ok(None),
        Err(error) => return Err(algebra(error)),
    };
    let (inverse, determinant, _) = verified.into_parts();
    if determinant != context.one() && determinant != context.integer(-1) {
        return Ok(None);
    }
    let transformed = multiply_coefficient_matrices(context, rows, &inverse, limits)
        .map_err(algebra)?
        .0;
    let replay = multiply_coefficient_matrices(context, &transformed, &basis, limits)
        .map_err(algebra)?
        .0;
    if replay != rows {
        return Err(Error::InvalidInput(
            "momentum transformation failed exact replay".into(),
        ));
    }
    Ok(Some(MomentumChart {
        rows: transformed,
        inverse,
    }))
}

pub(super) fn canonical_factor(
    family: &IntegralFamily,
    slots: &[usize],
    rows: &[Vec<Coefficient>],
    limits: VacuumProductLimits,
    stats: &mut VacuumProductStatistics,
) -> Result<CanonicalFactor, Error> {
    let loops = rows.first().map_or(0, Vec::len);
    if loops == 0 {
        return Err(Error::InvalidInput("empty momentum block".into()));
    }
    let mut best: Option<Vec<Vec<Integer>>> = None;
    let mut alignments = BTreeMap::new();
    combinations(rows.len(), loops, &mut |selection| {
        charge(
            &mut stats.basis_candidates,
            limits.diagnostic_limit(limits.max_basis_candidates),
            "basis candidates",
        )?;
        let Some(transformed) = unimodular_transform(family, rows, selection)? else {
            return Ok(true);
        };
        let integral = transformed
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| {
                        products::integer(value, family.coefficient_context())
                            .map_err(algebra)?
                            .ok_or_else(|| {
                                Error::InvalidInput(
                                    "unimodular chart produced noninteger momentum".into(),
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, Error>>()
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut permutation = (0..loops).collect::<Vec<_>>();
        loop {
            let mut signs = vec![false; loops];
            loop {
                charge(
                    &mut stats.canonical_candidates,
                    limits.diagnostic_limit(limits.max_canonical_candidates),
                    "canonical candidates",
                )?;
                let mut proposed = integral
                    .iter()
                    .zip(slots)
                    .map(|(row, &slot)| {
                        let mut row = permutation
                            .iter()
                            .zip(&signs)
                            .map(|(&axis, &negative)| {
                                if negative {
                                    -row[axis].clone()
                                } else {
                                    row[axis].clone()
                                }
                            })
                            .collect::<Vec<_>>();
                        // Each propagator momentum may be reversed independently.
                        if row
                            .iter()
                            .find(|value| **value != Integer::from(0))
                            .is_some_and(|value| *value < Integer::from(0))
                        {
                            for value in &mut row {
                                *value = -value.clone();
                            }
                        }
                        (row, slot)
                    })
                    .collect::<Vec<_>>();
                proposed.sort();
                let momenta = proposed
                    .iter()
                    .map(|(row, _)| row.clone())
                    .collect::<Vec<_>>();
                let alignment = proposed.iter().map(|(_, slot)| *slot).collect::<Vec<_>>();
                let keep = best.as_ref().is_none_or(|best| momenta <= *best);
                let transport = if keep {
                    Some(
                        transformed
                            .inverse
                            .iter()
                            .map(|row| {
                                permutation
                                    .iter()
                                    .zip(&signs)
                                    .map(|(&axis, &negative)| {
                                        if negative {
                                            family
                                                .coefficient_context()
                                                .try_neg(
                                                    &row[axis],
                                                    family.construction_limits().exact_algebra,
                                                )
                                                .map_err(algebra)
                                        } else {
                                            Ok(row[axis].clone())
                                        }
                                    })
                                    .collect::<Result<Vec<_>, Error>>()
                            })
                            .collect::<Result<Vec<_>, Error>>()?,
                    )
                } else {
                    None
                };
                match best.as_ref().map(|best| momenta.cmp(best)) {
                    None | Some(std::cmp::Ordering::Less) => {
                        best = Some(momenta);
                        alignments.clear();
                        alignments.insert(alignment, transport.expect("retained minimum"));
                    }
                    Some(std::cmp::Ordering::Equal) => {
                        // First witness wins: the bounded enumeration order is
                        // deterministic even when momentum reversals coincide.
                        alignments
                            .entry(alignment)
                            .or_insert_with(|| transport.expect("retained equal chart"));
                    }
                    Some(std::cmp::Ordering::Greater) => {}
                }
                let Some(axis) = signs.iter().position(|&negative| !negative) else {
                    break;
                };
                signs[..axis].fill(false);
                signs[axis] = true;
            }
            if !next_permutation(&mut permutation) {
                break;
            }
        }
        Ok(true)
    })?;
    let momenta =
        best.ok_or_else(|| Error::InvalidInput("momentum block has no unimodular basis".into()))?;
    let lower = lower_family(family, &momenta)?;
    Ok(CanonicalFactor {
        family: Arc::new(lower),
        alignments,
    })
}

fn next_permutation(values: &mut [usize]) -> bool {
    let Some(left) = (0..values.len().saturating_sub(1))
        .rev()
        .find(|&i| values[i] < values[i + 1])
    else {
        return false;
    };
    let right = (left + 1..values.len())
        .rev()
        .find(|&i| values[left] < values[i])
        .unwrap();
    values.swap(left, right);
    values[left + 1..].reverse();
    true
}

fn coefficient(context: &CoefficientContext, value: &Integer) -> Coefficient {
    context.zero().numerator.constant(value.clone()).into()
}

fn square_row(parent: &IntegralFamily, momentum: &[Integer]) -> Result<Vec<Coefficient>, Error> {
    let context = parent.coefficient_context();
    let limits = parent.construction_limits().exact_algebra;
    let momentum = momentum
        .iter()
        .map(|value| coefficient(context, value))
        .collect::<Vec<_>>();
    let mut row = Vec::new();
    for left in 0..momentum.len() {
        for right in left..momentum.len() {
            let value = context
                .try_mul(&momentum[left], &momentum[right], limits)
                .map_err(algebra)?;
            row.push(if left == right {
                value
            } else {
                context
                    .try_mul(&value, &context.integer(2), limits)
                    .map_err(algebra)?
            });
        }
    }
    Ok(row)
}

/// The active propagators are followed by exact zero-power ISP coordinates.
/// Native rank, never custom elimination, extends them to a complete family.
fn lower_family(
    parent: &IntegralFamily,
    momenta: &[Vec<Integer>],
) -> Result<IntegralFamily, Error> {
    let loops = momenta[0].len();
    let arity = loops
        .checked_mul(loops + 1)
        .and_then(|n| n.checked_div(2))
        .ok_or_else(|| Error::InvalidInput("lower-family arity overflow".into()))?;
    let context = parent.coefficient_context();
    let matrix_limits = symbolica_matrix_limits(parent.construction_limits());
    let mut rows = momenta
        .iter()
        .map(|row| square_row(parent, row))
        .collect::<Result<Vec<_>, _>>()?;
    if rank_of_coefficient_matrix(context, &rows, matrix_limits)
        .map_err(algebra)?
        .0
        != rows.len()
    {
        return Err(Error::InvalidInput(
            "factor active quadratic coordinates are dependent".into(),
        ));
    }
    let mut candidates = Vec::new();
    for left in 0..loops {
        let mut row = vec![Integer::from(0); loops];
        row[left] = Integer::from(1);
        candidates.push(row);
    }
    for left in 0..loops {
        for right in left + 1..loops {
            let mut row = vec![Integer::from(0); loops];
            row[left] = Integer::from(1);
            row[right] = Integer::from(-1);
            candidates.push(row);
        }
    }
    for momentum in candidates {
        if rows.len() == arity {
            break;
        }
        rows.push(square_row(parent, &momentum)?);
        if rank_of_coefficient_matrix(context, &rows, matrix_limits)
            .map_err(algebra)?
            .0
            < rows.len()
        {
            rows.pop();
        }
    }
    if rows.len() != arity {
        return Err(Error::InvalidInput(
            "could not complete the lower-family ISP basis".into(),
        ));
    }
    IntegralFamily::new_with_limits(
        "unit_mass_vacuum_product",
        (0..loops).map(|i| format!("q{i}")).collect(),
        vec![],
        context.clone(),
        parent.dimension().clone(),
        rows.into_iter()
            .map(|row| AffineDenominator::new(context.integer(-1), row))
            .collect(),
        vec![],
        vec![context.zero(); arity],
        parent.construction_limits(),
    )
    .map_err(algebra)
}
