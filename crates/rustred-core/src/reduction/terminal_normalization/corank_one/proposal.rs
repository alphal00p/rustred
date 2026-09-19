use std::sync::Arc;

use symbolica::prelude::{Integer, Matrix, PolyVariable, Q, Rational, Z};

use crate::algebra::Coefficient;
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, invert_and_verify_coefficient_matrix,
    multiply_coefficient_matrices,
};
use crate::family::{IntegralFamily, IntegralKey, symbolica_matrix_limits};
use crate::sector::{ComplexityKey, OrderingPolicy};

use super::super::{
    ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasStatistics, products,
};

pub(super) type Signature = Vec<(Integer, i64)>;

pub(super) struct Support {
    slots: Vec<usize>,
    rows: Vec<Vec<Coefficient>>,
    circuit: Vec<Integer>,
}

pub(super) struct Candidate {
    pub key: IntegralKey,
    pub complexity: ComplexityKey,
    pub slots: Vec<usize>,
    pub rows: Vec<Vec<Coefficient>>,
    pub inverse: Vec<Vec<Coefficient>>,
    pub pivot: usize,
}

pub(super) fn support(
    family: &IntegralFamily,
    slots: &[usize],
    variables: &Arc<Vec<PolyVariable>>,
    momenta: &mut [Option<Result<Vec<Coefficient>, Skip>>],
    statistics: &mut TerminalAliasStatistics,
) -> Result<Result<Support, Skip>, Error> {
    let mut rows = Vec::with_capacity(slots.len());
    for &slot in slots {
        if momenta[slot].is_none() {
            statistics.analyzed_denominators += 1;
            momenta[slot] = Some(products::momentum(family, slot, variables)?);
        }
        match momenta[slot].as_ref().expect("momentum initialized") {
            Ok(row) => rows.push(row.clone()),
            Err(reason) => return Ok(Err(*reason)),
        }
    }
    let context = family.coefficient_context();
    // At most L+1 native inversions; no row-reduction or rational kernel
    // arithmetic is reimplemented here. An invertible basis proves rank L.
    for omitted in 0..rows.len() {
        let basis: Vec<_> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| (i != omitted).then_some(row.clone()))
            .collect();
        let inverse = match invert_and_verify_coefficient_matrix(
            context,
            &basis,
            symbolica_matrix_limits(family.construction_limits()),
        ) {
            Ok(inverse) => inverse.into_parts().0,
            Err(SymbolicaCoefficientMatrixError::Singular) => continue,
            Err(error) => return Err(Error::ExactAlgebra(error.to_string())),
        };
        let (coordinates, _) = multiply_coefficient_matrices(
            context,
            &[rows[omitted].clone()],
            &inverse,
            symbolica_matrix_limits(family.construction_limits()),
        )
        .map_err(|e| Error::ExactAlgebra(e.to_string()))?;
        let mut values = Vec::with_capacity(rows.len());
        let mut coordinates = coordinates[0].iter();
        for i in 0..rows.len() {
            if i == omitted {
                values.push(Rational::one());
            } else {
                let value = coordinates.next().ok_or(Error::InvalidCircuitWitness)?;
                if !context.contains(value)
                    || !value.numerator.is_constant()
                    || !value.denominator.is_constant()
                    || value.denominator.is_zero()
                {
                    return Err(Error::InvalidCircuitWitness);
                }
                values.push(-Rational::from((
                    value.numerator.get_constant(),
                    value.denominator.get_constant(),
                )));
            }
        }
        // Symbolica performs joint rational-content/denominator removal.
        let primitive = Matrix::new_vec(values, Q).primitive_part();
        let mut circuit = Vec::with_capacity(rows.len());
        for value in primitive.into_vec() {
            if !value.is_integer() {
                return Err(Error::InvalidCircuitWitness);
            }
            circuit.push(value.numerator_ref().clone());
        }
        if circuit[omitted].is_zero() {
            return Err(Error::InvalidCircuitWitness);
        }
        replay(family, &rows, &circuit)?;
        return Ok(Ok(Support {
            slots: slots.to_vec(),
            rows,
            circuit,
        }));
    }
    Ok(Err(Skip::SingularMomentumBasis))
}

fn replay(
    family: &IntegralFamily,
    rows: &[Vec<Coefficient>],
    circuit: &[Integer],
) -> Result<(), Error> {
    let mut transpose = Vec::with_capacity(rows.len() * family.loop_count());
    for column in 0..family.loop_count() {
        for row in rows {
            transpose.push(
                products::integer(&row[column], family.coefficient_context())?
                    .ok_or(Error::InvalidCircuitWitness)?,
            );
        }
    }
    let nrows = u32::try_from(family.loop_count()).map_err(|_| Error::InvalidCircuitWitness)?;
    let ncols = u32::try_from(rows.len()).map_err(|_| Error::InvalidCircuitWitness)?;
    let matrix = Matrix::from_linear(transpose, nrows, ncols, Z)
        .map_err(|_| Error::InvalidCircuitWitness)?;
    if !(&matrix * &Matrix::new_vec(circuit.to_vec(), Z)).is_zero() {
        return Err(Error::InvalidCircuitWitness);
    }
    Ok(())
}

pub(super) fn candidate(
    family: &IntegralFamily,
    key: &IntegralKey,
    support: &Support,
    ordering: OrderingPolicy,
) -> Result<(Signature, Candidate), Error> {
    let context = family.coefficient_context();
    let mut indices: Vec<_> = (0..support.slots.len()).collect();
    indices.sort_by_key(|&i| {
        (
            support.circuit[i].abs(),
            key.powers()[support.slots[i]],
            support.slots[i],
        )
    });
    let signature: Signature = indices
        .iter()
        .map(|&i| (support.circuit[i].abs(), key.powers()[support.slots[i]]))
        .collect();
    let pivot = signature
        .iter()
        .position(|(c, _)| !c.is_zero())
        .ok_or(Error::InvalidCircuitWitness)?;
    let mut slots = Vec::with_capacity(indices.len());
    let mut rows = Vec::with_capacity(indices.len());
    for i in indices {
        slots.push(support.slots[i]);
        rows.push(if support.circuit[i].is_negative() {
            support.rows[i]
                .iter()
                .map(|value| {
                    context
                        .try_neg(value, Default::default())
                        .map_err(|e| Error::ExactAlgebra(e.to_string()))
                })
                .collect::<Result<_, _>>()?
        } else {
            support.rows[i].clone()
        });
    }
    // Removing a nonzero circuit entry gives a basis, even with coloops.
    let basis: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| (i != pivot).then_some(row.clone()))
        .collect();
    let inverse = invert_and_verify_coefficient_matrix(
        context,
        &basis,
        symbolica_matrix_limits(family.construction_limits()),
    )
    .map_err(|e| Error::ExactAlgebra(e.to_string()))?
    .into_parts()
    .0;
    Ok((
        signature,
        Candidate {
            key: key.clone(),
            complexity: ordering
                .complexity_key(key.powers())
                .map_err(|e| Error::Ordering(e.to_string()))?,
            slots,
            rows,
            inverse,
            pivot,
        },
    ))
}
