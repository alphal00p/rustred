use symbolica::domains::SelfRing;

use crate::algebra::matrix::multiply_coefficient_matrices;
use crate::family::{IntegralFamily, symbolica_matrix_limits};
use crate::sector::symmetry::{self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap};

use super::super::{ProductSkipReason as Skip, TerminalAliasError as Error, products};
use super::proposal::Candidate;

pub(super) fn alias(
    family: &IntegralFamily,
    source: &Candidate,
    target: &Candidate,
) -> Result<Result<symmetry::VerifiedMap, Skip>, Error> {
    if source.pivot != target.pivot || source.slots.len() != target.slots.len() {
        return Err(Error::InvalidCircuitWitness);
    }
    let context = family.coefficient_context();
    let target_basis: Vec<_> = target
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| (i != target.pivot).then_some(row.clone()))
        .collect();
    let (matrix, _) = multiply_coefficient_matrices(
        context,
        &source.inverse,
        &target_basis,
        symbolica_matrix_limits(family.construction_limits()),
    )
    .map_err(|e| Error::ExactAlgebra(e.to_string()))?;
    for entry in matrix.iter().flatten() {
        if products::integer(entry, context)?.is_none() {
            return Ok(Err(Skip::NonIntegralMomentumMap));
        }
    }
    // Replay all signed active vectors before the more expensive generic
    // full-family verifier. Inactive coordinates need not permute.
    let (replayed, _) = multiply_coefficient_matrices(
        context,
        &source.rows,
        &matrix,
        symbolica_matrix_limits(family.construction_limits()),
    )
    .map_err(|e| Error::ExactAlgebra(e.to_string()))?;
    if replayed != target.rows {
        return Err(Error::InvalidCircuitWitness);
    }
    let checked = |rows, columns, entries| {
        CoefficientMatrix::try_new(rows, columns, entries)
            .map_err(|e| Error::MomentumVerification(e.to_string()))
    };
    let map = MomentumMap::new(
        checked(
            family.loop_count(),
            family.loop_count(),
            matrix.into_iter().flatten().collect(),
        )?,
        checked(family.loop_count(), 0, vec![])?,
        checked(0, 0, vec![])?,
    );
    let witness = symmetry::verify(family, family, map, symmetry::Limits::default())
        .map_err(|e| Error::MomentumVerification(e.to_string()))?;
    if !matches!(witness.jacobian(), Jacobian::Unit { .. }) {
        return Ok(Err(Skip::NonUnimodularMomentumMap));
    }
    for (&source_slot, &target_slot) in source.slots.iter().zip(&target.slots) {
        if !matches!(&witness.row_actions()[source_slot],
            DenominatorAction::Monomial { target, scale }
            if *target == target_slot && context.contains(scale) && scale.is_one())
            || source.key.powers()[source_slot] != target.key.powers()[target_slot]
        {
            return Err(Error::InvalidCircuitWitness);
        }
    }
    if witness
        .nonzero_conditions()
        .iter()
        .any(|c| !c.polynomial().is_constant())
    {
        return Ok(Err(Skip::ConditionalMomentumMap));
    }
    Ok(Ok(witness))
}
