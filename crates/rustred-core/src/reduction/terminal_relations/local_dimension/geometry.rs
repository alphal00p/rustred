//! Exact local integration directions and numerator-complete spectators.
//!
//! Numerica owns RREF, rank, rational arithmetic and matrix products. The only
//! kernel adapter here extracts the deterministic free-column vectors from its
//! public RREF; every vector is checked against the ORIGINAL matrix. There is
//! deliberately no topology/loop-count/rank cap and no numerical sampling.

use std::collections::BTreeSet;
use std::fmt;

use symbolica::prelude::{Matrix, Q, Rational};

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::{IntegralFamily, IntegralKey, ScalarProductCoordinate};

type NativeMatrix = Matrix<symbolica::domains::rational::RationalField>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LinearMomentum {
    pub loops: Vec<Rational>,
    pub external: Vec<Rational>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LocalGeometry {
    /// Primitive signed integer representative of the rational direction.
    pub direction: Vec<Rational>,
    /// K = kappa.k, normalized by kappa.u = 1.
    pub kappa: Vec<Rational>,
    /// Minimal independent ORIGINAL residuals, including numerator factors.
    pub spectators: Vec<LinearMomentum>,
    pub alpha: Vec<Rational>,
    pub residuals: Vec<LinearMomentum>,
    /// (H_i u, J_i^T u), the gradient contraction needed by native sources.
    pub gradient_contractions: Vec<LinearMomentum>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GeometryError {
    ParameterValuedGeometry {
        denominator: usize,
        coordinate: usize,
    },
    InvalidInput(&'static str),
    NativeRepresentation(&'static str),
    Algebra(String),
    Cancelled,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParameterValuedGeometry {
                denominator,
                coordinate,
            } => write!(
                f,
                "local dimensional geometry requires a rational momentum coefficient at denominator {denominator}, coordinate {coordinate}"
            ),
            Self::InvalidInput(message) | Self::NativeRepresentation(message) => {
                f.write_str(message)
            }
            Self::Algebra(message) => f.write_str(message),
            Self::Cancelled => f.write_str("local dimensional geometry cancelled"),
        }
    }
}

impl std::error::Error for GeometryError {}

struct Quadratics {
    h: Vec<Vec<Vec<Rational>>>,
    j: Vec<Vec<Vec<Rational>>>,
}

fn checkpoint(cancel: Option<&dyn Fn() -> bool>) -> Result<(), GeometryError> {
    if cancel.is_some_and(|c| c()) {
        Err(GeometryError::Cancelled)
    } else {
        Ok(())
    }
}

fn algebra(error: impl fmt::Display) -> GeometryError {
    GeometryError::Algebra(error.to_string())
}

fn dimension(value: usize) -> Result<u32, GeometryError> {
    u32::try_from(value).map_err(|_| {
        GeometryError::NativeRepresentation("local dimensional native matrix dimension exceeds u32")
    })
}

fn matrix(rows: &[Vec<Rational>], columns: usize) -> Result<NativeMatrix, GeometryError> {
    if rows.iter().any(|row| row.len() != columns) {
        return Err(GeometryError::InvalidInput(
            "ragged local dimensional matrix",
        ));
    }
    rows.len()
        .checked_mul(columns)
        .ok_or(GeometryError::NativeRepresentation(
            "local dimensional matrix entry count overflow",
        ))?;
    Matrix::from_linear(
        rows.iter().flatten().cloned().collect(),
        dimension(rows.len())?,
        dimension(columns)?,
        Q,
    )
    .map_err(algebra)
}

fn rational(
    coefficient: &Coefficient,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
    denominator: usize,
    coordinate: usize,
) -> Result<Rational, GeometryError> {
    context
        .validate_with_limits(coefficient, limits)
        .map_err(algebra)?;
    if !coefficient.numerator.is_constant() || !coefficient.denominator.is_constant() {
        return Err(GeometryError::ParameterValuedGeometry {
            denominator,
            coordinate,
        });
    }
    if coefficient.denominator.is_zero() {
        return Err(GeometryError::InvalidInput(
            "zero geometry coefficient denominator",
        ));
    }
    Ok(Rational::from((
        coefficient.numerator.get_constant(),
        coefficient.denominator.get_constant(),
    )))
}

pub(super) fn rational_coefficient(
    value: &Rational,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, GeometryError> {
    let numerator: Coefficient = context
        .zero()
        .numerator
        .constant(value.numerator_ref().clone())
        .into();
    let denominator: Coefficient = context
        .zero()
        .numerator
        .constant(value.denominator_ref().clone())
        .into();
    context
        .try_div(&numerator, &denominator, limits)
        .map_err(algebra)
}

fn quadratics(
    family: &IntegralFamily,
    limits: ExactAlgebraLimits,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<Quadratics, GeometryError> {
    let loops = family.loop_count();
    let external = family.external_count();
    let mut h = Vec::with_capacity(family.denominator_count());
    let mut j = Vec::with_capacity(family.denominator_count());
    for (slot, denominator) in family.denominators().iter().enumerate() {
        checkpoint(cancel)?;
        let mut hs = vec![vec![Rational::from(0); loops]; loops];
        let mut js = vec![vec![Rational::from(0); external]; loops];
        for (coordinate, (&typed, coefficient)) in family
            .coordinates()
            .iter()
            .zip(denominator.coefficients())
            .enumerate()
        {
            let original = rational(
                coefficient,
                family.coefficient_context(),
                limits,
                slot,
                coordinate,
            )?;
            match typed {
                ScalarProductCoordinate::LoopLoop { left, right } => {
                    if left == right {
                        hs[left][right] = original.clone();
                    } else {
                        let half = &original / &Rational::from(2);
                        hs[left][right] = half.clone();
                        hs[right][left] = half;
                    }
                    let reconstructed = if left == right {
                        hs[left][right].clone()
                    } else {
                        &hs[left][right] + &hs[right][left]
                    };
                    if reconstructed != original {
                        return Err(GeometryError::InvalidInput(
                            "quadratic H reconstruction failed",
                        ));
                    }
                }
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                } => {
                    js[loop_index][external_index] = &original / &Rational::from(2);
                    if &js[loop_index][external_index] * &Rational::from(2) != original {
                        return Err(GeometryError::InvalidInput(
                            "quadratic J reconstruction failed",
                        ));
                    }
                }
            }
        }
        h.push(hs);
        j.push(js);
    }
    Ok(Quadratics { h, j })
}

fn normalize_direction(values: &[Rational]) -> Result<Vec<Rational>, GeometryError> {
    if values.iter().all(|value| value.is_zero()) {
        return Err(GeometryError::InvalidInput(
            "zero local dimensional direction",
        ));
    }
    dimension(values.len())?;
    // Same native primitive-part adapter as algebra/matrix/right_kernel:
    // Numerica owns denominator clearing/content, not a local gcd/lcm kernel.
    let primitive = Matrix::new_vec(values.to_vec(), Q).primitive_part();
    let mut direction = primitive.into_vec();
    if direction.iter().any(|value| !value.is_integer()) {
        return Err(GeometryError::InvalidInput(
            "native direction primitive part is not integral",
        ));
    }
    if direction
        .iter()
        .find(|value| !value.is_zero())
        .unwrap()
        .numerator_ref()
        .is_negative()
    {
        for value in &mut direction {
            *value = -value.clone();
        }
    }
    Ok(direction)
}

fn kernel(rows: &[Vec<Rational>], columns: usize) -> Result<Vec<Vec<Rational>>, GeometryError> {
    let original = matrix(rows, columns)?;
    let mut reduced = original.clone();
    let rank = reduced.row_reduce(dimension(columns)?);
    let pivots: Vec<_> = reduced
        .row_iter()
        .take(rank)
        .map(|row| {
            row.iter()
                .position(|value| !value.is_zero())
                .ok_or(GeometryError::InvalidInput(
                    "native RREF rank has a zero pivot row",
                ))
        })
        .collect::<Result<_, _>>()?;
    let mut result = Vec::new();
    for free in (0..columns).filter(|column| !pivots.contains(column)) {
        let mut vector = vec![Rational::from(0); columns];
        vector[free] = Rational::from(1);
        for (row, &pivot) in reduced.row_iter().take(rank).zip(&pivots) {
            vector[pivot] = -row[free].clone();
        }
        let vector = normalize_direction(&vector)?;
        let product = &original
            * &matrix(
                &vector.iter().cloned().map(|x| vec![x]).collect::<Vec<_>>(),
                1,
            )?;
        if product.row_iter().flatten().any(|value| !value.is_zero()) {
            return Err(GeometryError::InvalidInput(
                "native local kernel replay failed",
            ));
        }
        result.push(vector);
    }
    Ok(result)
}

/// Nominate singleton/pair-local directions, with no spectator-rank assumption.
pub(super) fn structural_directions(
    family: &IntegralFamily,
    key: &IntegralKey,
    limits: ExactAlgebraLimits,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<Vec<Vec<Rational>>, GeometryError> {
    if key.powers().len() != family.denominator_count() {
        return Err(GeometryError::InvalidInput(
            "local dimensional source key arity mismatch",
        ));
    }
    let data = quadratics(family, limits, cancel)?;
    let context = family.coefficient_context();
    let mut positive = Vec::new();
    for (slot, &power) in key.powers().iter().enumerate() {
        let effective = context
            .try_add(
                &context.integer(power),
                &family.power_shifts()[slot],
                limits,
            )
            .map_err(algebra)?;
        if power > 0 && !effective.is_zero() {
            positive.push(slot);
        }
    }
    let mut directions = BTreeSet::new();
    // One extra empty nomination covers a scaleless/free integration direction.
    let nominees = std::iter::once(Vec::new())
        .chain(positive.iter().map(|&i| vec![i]))
        .chain(
            positive
                .iter()
                .enumerate()
                .flat_map(|(i, &a)| positive[i + 1..].iter().map(move |&b| vec![a, b])),
        );
    for nominated in nominees {
        checkpoint(cancel)?;
        let mut constraints = Vec::new();
        for &slot in &positive {
            if nominated.contains(&slot) {
                continue;
            }
            constraints.extend(data.h[slot].iter().cloned());
            for external in 0..family.external_count() {
                constraints.push(
                    (0..family.loop_count())
                        .map(|l| data.j[slot][l][external].clone())
                        .collect(),
                );
            }
        }
        directions.extend(kernel(&constraints, family.loop_count())?);
    }
    Ok(directions.into_iter().collect())
}

fn dot(left: &[Rational], right: &[Rational]) -> Rational {
    left.iter()
        .zip(right)
        .fold(Rational::from(0), |sum, (a, b)| sum + a * b)
}

fn flatten(momentum: &LinearMomentum) -> Vec<Rational> {
    momentum
        .loops
        .iter()
        .chain(&momentum.external)
        .cloned()
        .collect()
}

/// Reconstruct and verify a caller-proposed local direction exactly.
pub(super) fn prepare_geometry(
    family: &IntegralFamily,
    key: &IntegralKey,
    direction: &[Rational],
    limits: ExactAlgebraLimits,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<LocalGeometry, GeometryError> {
    if key.powers().len() != family.denominator_count() || direction.len() != family.loop_count() {
        return Err(GeometryError::InvalidInput(
            "local dimensional key/direction arity mismatch",
        ));
    }
    let direction = normalize_direction(direction)?;
    let first = direction.iter().position(|x| !x.is_zero()).unwrap();
    let mut kappa = vec![Rational::from(0); direction.len()];
    kappa[first] = &Rational::from(1) / &direction[first];
    if dot(&direction, &kappa) != Rational::from(1) {
        return Err(GeometryError::InvalidInput(
            "local dimensional kappa.u normalization failed",
        ));
    }
    let data = quadratics(family, limits, cancel)?;
    let mut alpha = Vec::new();
    let mut residuals = Vec::new();
    let mut gradient_contractions = Vec::new();
    let mut participating = Vec::new();
    let context = family.coefficient_context();
    for slot in 0..family.denominator_count() {
        checkpoint(cancel)?;
        let gradient = LinearMomentum {
            loops: data.h[slot]
                .iter()
                .map(|row| dot(row, &direction))
                .collect(),
            external: (0..family.external_count())
                .map(|e| {
                    dot(
                        &(0..family.loop_count())
                            .map(|l| data.j[slot][l][e].clone())
                            .collect::<Vec<_>>(),
                        &direction,
                    )
                })
                .collect(),
        };
        let a = dot(&direction, &gradient.loops);
        let residual = LinearMomentum {
            loops: gradient
                .loops
                .iter()
                .zip(&kappa)
                .map(|(g, k)| g - &(&a * k))
                .collect(),
            external: gradient.external.clone(),
        };
        if !dot(&direction, &residual.loops).is_zero() {
            return Err(GeometryError::InvalidInput(
                "local dimensional residual is not spectator invariant",
            ));
        }
        // Independently reconstruct D = alpha K² + 2 K.r + rho and verify
        // that the quadratic remainder rho is independent of the direction.
        let mut remainder_h = data.h[slot].clone();
        let mut remainder_j = data.j[slot].clone();
        for l in 0..family.loop_count() {
            for m in 0..family.loop_count() {
                let parallel = &(&a * &kappa[l]) * &kappa[m];
                let cross_left = &kappa[l] * &residual.loops[m];
                let cross_right = &residual.loops[l] * &kappa[m];
                let removed = &(&parallel + &cross_left) + &cross_right;
                remainder_h[l][m] = &remainder_h[l][m] - &removed;
            }
            for e in 0..family.external_count() {
                remainder_j[l][e] = &remainder_j[l][e] - &(&kappa[l] * &residual.external[e]);
            }
        }
        if remainder_h
            .iter()
            .any(|row| !dot(row, &direction).is_zero())
            || (0..family.external_count()).any(|e| {
                !dot(
                    &(0..family.loop_count())
                        .map(|l| remainder_j[l][e].clone())
                        .collect::<Vec<_>>(),
                    &direction,
                )
                .is_zero()
            })
        {
            return Err(GeometryError::InvalidInput(
                "local dimensional quadratic reconstruction failed",
            ));
        }
        let effective = context
            .try_add(
                &context.integer(key.powers()[slot]),
                &family.power_shifts()[slot],
                limits,
            )
            .map_err(algebra)?;
        if !effective.is_zero() && flatten(&residual).iter().any(|x| !x.is_zero()) {
            participating.push(flatten(&residual));
        }
        alpha.push(a);
        residuals.push(residual);
        gradient_contractions.push(gradient);
    }
    checkpoint(cancel)?;
    let width = family
        .loop_count()
        .checked_add(family.external_count())
        .ok_or(GeometryError::NativeRepresentation(
            "local spectator width overflow",
        ))?;
    let all = matrix(&participating, width)?;
    // Native RREF pivot columns of the transpose select independent ORIGINAL
    // residual rows; no numerator spectator can be silently omitted.
    let mut transposed = all.transpose();
    let rank = transposed.row_reduce(dimension(participating.len())?);
    let selected: Vec<_> = transposed
        .row_iter()
        .take(rank)
        .map(|row| {
            row.iter()
                .position(|x| !x.is_zero())
                .ok_or(GeometryError::InvalidInput(
                    "zero native spectator pivot row",
                ))
        })
        .collect::<Result<_, _>>()?;
    let basis: Vec<_> = selected.iter().map(|&i| participating[i].clone()).collect();
    let mut replay = basis.clone();
    replay.extend(participating.iter().cloned());
    if matrix(&basis, width)?.rank() != rank || matrix(&replay, width)?.rank() != rank {
        return Err(GeometryError::InvalidInput(
            "local dimensional spectator span replay failed",
        ));
    }
    let spectators = basis
        .into_iter()
        .map(|row| LinearMomentum {
            loops: row[..family.loop_count()].to_vec(),
            external: row[family.loop_count()..].to_vec(),
        })
        .collect();
    Ok(LocalGeometry {
        direction,
        kappa,
        spectators,
        alpha,
        residuals,
        gradient_contractions,
    })
}

/// Lower a complete momentum scalar product through the family's native
/// affine map, including external invariants and all mass constants.
pub(super) fn scalar_product_coefficients(
    family: &IntegralFamily,
    left: &LinearMomentum,
    right: &LinearMomentum,
    limits: ExactAlgebraLimits,
) -> Result<(Coefficient, Vec<Coefficient>), GeometryError> {
    let loops = family.loop_count();
    let external = family.external_count();
    if left.loops.len() != loops
        || right.loops.len() != loops
        || left.external.len() != external
        || right.external.len() != external
    {
        return Err(GeometryError::InvalidInput(
            "scalar product momentum arity mismatch",
        ));
    }
    let context = family.coefficient_context();
    let mut constant = context.zero();
    let mut coefficients = vec![context.zero(); family.denominator_count()];
    for (coordinate, &typed) in family.coordinates().iter().enumerate() {
        let weight = match typed {
            ScalarProductCoordinate::LoopLoop { left: l, right: r } => {
                if l == r {
                    &left.loops[l] * &right.loops[r]
                } else {
                    &left.loops[l] * &right.loops[r] + &left.loops[r] * &right.loops[l]
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index: l,
                external_index: e,
            } => &left.loops[l] * &right.external[e] + &right.loops[l] * &left.external[e],
        };
        if weight.is_zero() {
            continue;
        }
        let weight = rational_coefficient(&weight, context, limits)?;
        let expansion = family
            .scalar_product_expansion(coordinate)
            .map_err(algebra)?;
        let term = context
            .try_mul(&weight, expansion.constant(), limits)
            .map_err(algebra)?;
        constant = context.try_add(&constant, &term, limits).map_err(algebra)?;
        for (out, coefficient) in coefficients
            .iter_mut()
            .zip(expansion.denominator_coefficients())
        {
            let term = context
                .try_mul(&weight, coefficient, limits)
                .map_err(algebra)?;
            *out = context.try_add(out, &term, limits).map_err(algebra)?;
        }
    }
    for l in 0..external {
        for r in 0..external {
            let weight = &left.external[l] * &right.external[r];
            if weight.is_zero() {
                continue;
            }
            let weight = rational_coefficient(&weight, context, limits)?;
            let term = context
                .try_mul(&weight, &family.external_gram()[l][r], limits)
                .map_err(algebra)?;
            constant = context.try_add(&constant, &term, limits).map_err(algebra)?;
        }
    }
    Ok((constant, coefficients))
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
