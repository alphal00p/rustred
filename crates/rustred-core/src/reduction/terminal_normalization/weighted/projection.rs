use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::domains::SelfRing;
use symbolica::prelude::{Matrix, Q, Rational};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::symmetry::{self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap};

use super::super::{ProductSkipReason, corank_one::proposal::Support};
use super::{
    TerminalNormalizationError as Error, TerminalNormalizationLimits,
    TerminalNormalizationSkipReason as Skip, TerminalProjectionWitness, VerifiedNumeratorSupport,
    algebra, check,
};

type NativeMatrix = Matrix<symbolica::domains::rational::RationalField>;

pub(super) fn rational(
    value: &Coefficient,
    context: &CoefficientContext,
) -> Result<Rational, Error> {
    context
        .validate_with_limits(value, Default::default())
        .map_err(algebra)?;
    if !value.numerator.is_constant()
        || !value.denominator.is_constant()
        || value.denominator.is_zero()
    {
        return Err(Error::InvalidWitness(
            "coefficient is not a constant rational",
        ));
    }
    Ok(Rational::from((
        value.numerator.get_constant(),
        value.denominator.get_constant(),
    )))
}

fn coefficient(value: &Rational, context: &CoefficientContext) -> Result<Coefficient, Error> {
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
        .try_div(&numerator, &denominator, Default::default())
        .map_err(algebra)
}

fn matrix(rows: &[Vec<Rational>]) -> Result<NativeMatrix, Error> {
    let columns = rows
        .first()
        .ok_or(Error::InvalidWitness("empty projection matrix"))?
        .len();
    if rows.iter().any(|row| row.len() != columns) {
        return Err(Error::InvalidWitness("ragged projection matrix"));
    }
    Matrix::from_linear(
        rows.iter().flatten().cloned().collect(),
        u32::try_from(rows.len()).map_err(algebra)?,
        u32::try_from(columns).map_err(algebra)?,
        Q,
    )
    .map_err(algebra)
}

fn denominator(family: &IntegralFamily, slot: usize) -> Result<Vec<Rational>, Error> {
    std::iter::once(family.denominators()[slot].constant())
        .chain(family.denominators()[slot].coefficients())
        .map(|c| rational(c, family.coefficient_context()))
        .collect()
}

pub(super) fn prepare(
    family: &IntegralFamily,
    support: &Support,
    limits: TerminalNormalizationLimits,
) -> Result<Result<Arc<VerifiedNumeratorSupport>, Skip>, Error> {
    let context = family.coefficient_context();
    let loops = family.loop_count();
    let coordinates = family.coordinates().len();
    let max_columns = loops
        .checked_mul(coordinates)
        .and_then(|n| n.checked_add(support.slots.len()))
        .and_then(|n| n.checked_add(1))
        .ok_or(Error::InvalidWitness("projection dimensions overflow"))?;
    let cells = coordinates
        .checked_add(1)
        .and_then(|n| n.checked_mul(max_columns))
        .ok_or(Error::InvalidWitness("projection cell count overflow"))?;
    check("projection matrix cells", cells, limits.max_matrix_cells)?;
    if support.circuit.iter().any(|c| !c.is_zero() && c.abs() != 1) {
        return Ok(Err(Skip::NonUnitCircuit));
    }
    let circuit: Vec<_> = support
        .circuit
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (!c.is_zero()).then_some(i))
        .collect();
    let bridges: Vec<_> = support
        .circuit
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.is_zero().then_some(i))
        .collect();
    let omitted = *circuit
        .first()
        .ok_or(Error::InvalidWitness("zero primitive circuit"))?;
    let normalized: Vec<Vec<_>> = support
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .map(|value| {
                    let value = rational(value, context)?;
                    Ok(if support.circuit[i].is_negative() {
                        -value
                    } else {
                        value
                    })
                })
                .collect::<Result<_, Error>>()
        })
        .collect::<Result<_, _>>()?;
    let basis = matrix(
        &normalized
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != omitted)
            .map(|(_, r)| r.clone())
            .collect::<Vec<_>>(),
    )?;
    let inverse = basis.inv().map_err(algebra)?;
    let mut proposals = Vec::new();
    for pair in circuit.windows(2) {
        let mut permutation: Vec<_> = (0..support.slots.len()).collect();
        permutation.swap(pair[0], pair[1]);
        proposals.push((permutation, None));
    }
    for &bridge in &bridges {
        proposals.push(((0..support.slots.len()).collect::<Vec<_>>(), Some(bridge)));
    }
    if proposals.len() != loops {
        return Err(Error::InvalidWitness("corank-one generator count"));
    }
    let mut columns = Vec::new();
    let mut constant = vec![Rational::zero(); coordinates + 1];
    constant[0] = Rational::one();
    columns.push(constant);
    for &slot in &support.slots {
        columns.push(denominator(family, slot)?);
    }
    let mut generators = Vec::new();
    for (permutation, flip) in proposals {
        let targets: Vec<Vec<_>> = (0..support.slots.len())
            .filter(|i| *i != omitted)
            .map(|i| {
                normalized[permutation[i]]
                    .iter()
                    .map(|c| {
                        if flip == Some(i) {
                            -c.clone()
                        } else {
                            c.clone()
                        }
                    })
                    .collect()
            })
            .collect();
        let a = &inverse * &matrix(&targets)?;
        if a.clone().into_vec().iter().any(|c| !c.is_integer()) {
            return Ok(Err(Skip::UnsupportedGeometry(
                ProductSkipReason::NonIntegralMomentumMap,
            )));
        }
        let entries = a
            .into_vec()
            .iter()
            .map(|c| coefficient(c, context))
            .collect::<Result<Vec<_>, _>>()?;
        let map = MomentumMap::new(
            CoefficientMatrix::try_new(loops, loops, entries).map_err(algebra)?,
            CoefficientMatrix::try_new(loops, 0, []).map_err(algebra)?,
            CoefficientMatrix::try_new(0, 0, []).map_err(algebra)?,
        );
        let verified =
            symmetry::verify(family, family, map, Default::default()).map_err(algebra)?;
        if !matches!(verified.jacobian(), Jacobian::Unit { .. }) {
            return Ok(Err(Skip::UnsupportedGeometry(
                ProductSkipReason::NonUnimodularMomentumMap,
            )));
        }
        if verified
            .nonzero_conditions()
            .iter()
            .any(|c| !c.polynomial().is_constant())
        {
            return Ok(Err(Skip::UnsupportedGeometry(
                ProductSkipReason::ConditionalMomentumMap,
            )));
        }
        for (i, &slot) in support.slots.iter().enumerate() {
            if !matches!(&verified.row_actions()[slot],DenominatorAction::Monomial {target,scale} if *target == support.slots[permutation[i]] && context.contains(scale) && scale.is_one())
            {
                return Err(Error::InvalidWitness(
                    "active denominator is not an exact unit permutation",
                ));
            }
        }
        for i in 0..coordinates {
            let mut difference = vec![-rational(
                &verified.scalar_products().constant()[i],
                context,
            )?];
            for j in 0..coordinates {
                difference.push(
                    Rational::from(i32::from(i == j))
                        - rational(
                            verified
                                .scalar_products()
                                .linear()
                                .get(i, j)
                                .ok_or(Error::InvalidWitness("scalar-product shape"))?,
                            context,
                        )?,
                );
            }
            if difference.iter().any(|c| !c.is_zero()) {
                columns.push(difference);
            }
        }
        generators.push(Arc::new(verified));
    }
    Ok(Ok(Arc::new(VerifiedNumeratorSupport {
        slots: support.slots.clone(),
        generators,
        columns,
    })))
}

pub(super) fn project(
    family: &IntegralFamily,
    key: &IntegralKey,
    support: Arc<VerifiedNumeratorSupport>,
) -> Result<Option<TerminalProjectionWitness>, Error> {
    let numerator = key
        .powers()
        .iter()
        .position(|&n| n == -1)
        .ok_or(Error::InvalidWitness("missing quadratic numerator"))?;
    for c in std::iter::once(family.denominators()[numerator].constant())
        .chain(family.denominators()[numerator].coefficients())
    {
        family
            .coefficient_context()
            .validate_with_limits(c, Default::default())
            .map_err(algebra)?;
        if !c.numerator.is_constant() || !c.denominator.is_constant() {
            return Ok(None);
        }
    }
    let target = Matrix::new_vec(denominator(family, numerator)?, Q);
    let system = matrix(&support.columns)?.transpose();
    let solution = match system.solve_any(&target) {
        Ok(value) => value,
        Err(symbolica::tensors::matrix::MatrixError::Inconsistent) => return Ok(None),
        Err(error) => return Err(algebra(error)),
    };
    if &system * &solution != target {
        return Err(Error::InvalidWitness("native projection replay"));
    }
    let combination = solution.into_vec();
    let mut scalar = key.powers().to_vec();
    scalar[numerator] = 0;
    let mut terms = BTreeMap::new();
    if !combination[0].is_zero() {
        terms.insert(
            IntegralKey::try_new(scalar.clone()).map_err(algebra)?,
            coefficient(&combination[0], family.coefficient_context())?,
        );
    }
    for (j, &slot) in support.slots.iter().enumerate() {
        if combination[j + 1].is_zero() {
            continue;
        }
        let mut pinch = scalar.clone();
        pinch[slot] = 0;
        terms.insert(
            IntegralKey::try_new(pinch).map_err(algebra)?,
            coefficient(&combination[j + 1], family.coefficient_context())?,
        );
    }
    Ok(Some(TerminalProjectionWitness {
        target: key.clone(),
        support,
        combination,
        unbound_terms: terms,
    }))
}
