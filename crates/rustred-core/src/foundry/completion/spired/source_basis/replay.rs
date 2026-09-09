use std::collections::BTreeMap;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::identity::CompletedIbpSourceRows;

use super::{SpiredSourceBasis, SpiredSourceBasisError, SpiredSourceBasisLimits};

const REPLAY_OPERATIONS: &str = "source-preconditioner translated replay exact operations";
const REPLAY_TERMS: &str = "source-preconditioner translated replay terms";

/// Replay every translated optimized row from translated ordinary sources.
///
/// Both the RREF coefficients and its provenance multipliers are translated:
/// `p(n) R(n) -> p(n+s) R(n+s)`.  This is the exact bridge a later modular
/// optimized-source hit must traverse before entering ordinary-source lifting.
pub(super) fn try_verify_translated_basis(
    basis: &SpiredSourceBasis,
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    offset: &[i64],
    limits: SpiredSourceBasisLimits,
) -> Result<(), SpiredSourceBasisError> {
    if !basis.owns_sources(sources) {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "translated replay received a different ordinary source barrier",
        });
    }
    if sources.context_fingerprint() != context.fingerprint() {
        return Err(SpiredSourceBasisError::WrongContext);
    }
    if offset.len() != context.index_count() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "translated source-basis replay offset has the wrong arity",
        });
    }
    let mut operations = 0usize;
    let mut retained_terms = 0usize;
    for (row_ordinal, row) in basis.rows().iter().enumerate() {
        let mut expected = BTreeMap::new();
        for term in row.terms() {
            retained_terms = charge_terms(retained_terms, limits)?;
            operations = charge_operation(operations, limits)?;
            let coefficient =
                context.translate_polynomial(term.coefficient(), offset, limits.translation)?;
            let coefficient = context.coefficient_from_polynomial_sealed(&coefficient)?;
            let shift = checked_translated_shift(term.shift().values(), offset)?;
            accumulate(
                context,
                &mut expected,
                shift,
                coefficient,
                limits,
                &mut operations,
            )?;
        }

        let mut replayed = BTreeMap::new();
        for provenance in row.provenance() {
            let source = sources.source_relation(provenance.source_ordinal()).ok_or(
                SpiredSourceBasisError::Invariant {
                    detail: "translated provenance names a missing ordinary source row",
                },
            )?;
            if source.row_id() != provenance.source_row() {
                return Err(SpiredSourceBasisError::Invariant {
                    detail: "translated provenance row identity differs from ordinary chronology",
                });
            }
            operations = charge_operation(operations, limits)?;
            let multiplier = context.translate_polynomial(
                provenance.coefficient(),
                offset,
                limits.translation,
            )?;
            let multiplier = context.coefficient_from_polynomial_sealed(&multiplier)?;
            for (source_shift, source_coefficient) in source.terms() {
                retained_terms = charge_terms(retained_terms, limits)?;
                operations = charge_operation(operations, limits)?;
                let source_coefficient =
                    context.translate(source_coefficient, offset, limits.translation)?;
                operations = charge_operation(operations, limits)?;
                let product = context.mul_with_limits(
                    &multiplier,
                    &source_coefficient,
                    limits.exact_algebra,
                )?;
                let shift = checked_translated_shift(source_shift.values(), offset)?;
                accumulate(
                    context,
                    &mut replayed,
                    shift,
                    product,
                    limits,
                    &mut operations,
                )?;
            }
        }
        if expected != replayed {
            return Err(SpiredSourceBasisError::ReplayMismatch {
                row_ordinal,
                detail: "translated polynomial row differs from its translated ordinary-source circuit",
            });
        }
    }
    Ok(())
}

fn checked_translated_shift(
    shift: &[i64],
    offset: &[i64],
) -> Result<Vec<i64>, SpiredSourceBasisError> {
    if shift.len() != offset.len() {
        return Err(SpiredSourceBasisError::Invariant {
            detail: "translated source-basis shift has the wrong arity",
        });
    }
    let mut result = Vec::new();
    result.try_reserve_exact(shift.len()).map_err(|_| {
        SpiredSourceBasisError::AllocationFailure {
            resource: "source-preconditioner translated shift coordinates",
            requested: shift.len(),
        }
    })?;
    for (&value, &translation) in shift.iter().zip(offset) {
        result.push(
            value
                .checked_add(translation)
                .ok_or(SpiredSourceBasisError::Invariant {
                    detail: "translated source-basis shift overflowed i64",
                })?,
        );
    }
    Ok(result)
}

fn accumulate(
    context: &IndexedCoefficientContext,
    accumulator: &mut BTreeMap<Vec<i64>, IndexedCoefficient>,
    shift: Vec<i64>,
    value: IndexedCoefficient,
    limits: SpiredSourceBasisLimits,
    operations: &mut usize,
) -> Result<(), SpiredSourceBasisError> {
    if value.is_zero() {
        return Ok(());
    }
    if let Some(previous) = accumulator.remove(&shift) {
        *operations = charge_operation(*operations, limits)?;
        let sum = context.add_with_limits(&previous, &value, limits.exact_algebra)?;
        if !sum.is_zero() {
            accumulator.insert(shift, sum);
        }
    } else {
        accumulator.insert(shift, value);
    }
    Ok(())
}

fn charge_operation(
    current: usize,
    limits: SpiredSourceBasisLimits,
) -> Result<usize, SpiredSourceBasisError> {
    let next = super::compile::checked_add(REPLAY_OPERATIONS, current, 1)?;
    super::compile::check_limit(REPLAY_OPERATIONS, next, limits.max_replay_exact_operations)?;
    Ok(next)
}

fn charge_terms(
    current: usize,
    limits: SpiredSourceBasisLimits,
) -> Result<usize, SpiredSourceBasisError> {
    let next = super::compile::checked_add(REPLAY_TERMS, current, 1)?;
    super::compile::check_limit(REPLAY_TERMS, next, limits.max_basis_term_entries)?;
    Ok(next)
}
