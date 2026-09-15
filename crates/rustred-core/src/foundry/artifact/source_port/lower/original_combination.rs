//! Compile the complete original weighted row sum once per parent rule.
//! Arithmetic data only: each cell still needs independent full residual,
//! applicability, unbounded descent and cover proofs before installation.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::SourceViewBatch;
use crate::foundry::parametric::{
    ParametricGuardOrigin, ParametricRuleLimits, condition_source_index_cells,
};
use crate::identity::{IndexShift, RowId};

use super::super::{SourcePortAuditError, error};

pub(super) struct JoinedContribution {
    pub source_ordinal: usize,
    pub original_row: RowId,
    pub offset: Vec<i64>,
    pub weight: IndexedCoefficient,
}

#[derive(Clone)]
pub(super) struct Guard {
    pub polynomial: IndexedPolynomial,
    pub origins: Vec<ParametricGuardOrigin>,
}

pub(super) struct OriginalCombination {
    pub columns: BTreeMap<IndexShift, IndexedCoefficient>,
    pub guards: Vec<Guard>,
    pub source_rows_multiplied: usize,
}

/// Return the same already-specialized weights that were actually multiplied.
/// Guard capture happens before specialization/cancellation; callers must not
/// regenerate this record by specializing a second, independently supplied
/// contribution list.
pub(super) fn compile_with_limits(
    context: &IndexedCoefficientContext,
    sources: &SourceViewBatch,
    contributions: &[JoinedContribution],
    fixed: &[(usize, i64)],
    limits: ParametricRuleLimits,
) -> Result<(OriginalCombination, Vec<(usize, RowId, IndexedCoefficient)>), SourcePortAuditError> {
    if contributions.is_empty() || sources.relations().is_empty() {
        return Err(error(
            "combined identity has no original source contribution",
        ));
    }
    if sources.context_fingerprint() != context.fingerprint()
        || sources.relations().len() != sources.provenance().len()
    {
        return Err(error(
            "combined source batch has incompatible context/provenance",
        ));
    }
    check_limit(
        "original source rows",
        sources.len(),
        limits.max_source_rows,
    )?;
    check_limit(
        "original source combination",
        contributions.len(),
        limits.max_source_combination_terms,
    )?;
    let mut input_terms = 0usize;
    let mut input_conditions = 0usize;
    let mut condition_sources = 0usize;
    let mut provenance_cells = 0usize;
    let mut used = BTreeSet::new();
    for contribution in contributions {
        let ordinal = contribution.source_ordinal;
        if !used.insert(ordinal) {
            return Err(error(
                "combined source requests were not canonically joined",
            ));
        }
        let source = sources
            .relations()
            .get(ordinal)
            .ok_or_else(|| error("combined source ordinal is out of range"))?;
        let provenance = sources
            .provenance()
            .get(ordinal)
            .ok_or_else(|| error("combined source provenance is absent"))?;
        if provenance.symmetry().is_some()
            || provenance.translated().source_row() != &contribution.original_row
            || provenance.translated().offset().values() != contribution.offset
        {
            return Err(error(
                "combined original RowId or wide source translation changed",
            ));
        }
        input_terms = input_terms
            .checked_add(source.terms().len())
            .ok_or_else(|| error("combined source term count overflow"))?;
        input_conditions = input_conditions
            .checked_add(source.nonzero_conditions().len())
            .ok_or_else(|| error("combined source condition count overflow"))?;
        for condition in source.nonzero_conditions() {
            condition_sources = condition_sources
                .checked_add(condition.sources().len())
                .ok_or_else(|| error("combined condition provenance count overflow"))?;
            for origin in condition.sources() {
                provenance_cells = provenance_cells
                    .checked_add(condition_source_index_cells(origin))
                    .ok_or_else(|| error("combined provenance coordinate count overflow"))?;
            }
        }
    }
    check_limit(
        "original source input terms",
        input_terms,
        limits.max_input_nonzero_entries,
    )?;
    // Conservative whole-parent work admission before native multiplication.
    // Every source term needs at most one restriction, product and addition;
    // conditions and weights add one restriction each. Guard normalization is
    // independently bounded by the indexed native algebra policy.
    let operations = input_terms
        .checked_mul(3)
        .and_then(|value| value.checked_add(input_conditions))
        .and_then(|value| value.checked_add(contributions.len()))
        .ok_or_else(|| error("combined replay operation count overflow"))?;
    check_limit(
        "original replay operations",
        operations,
        limits.max_replay_exact_operations,
    )?;
    check_limit(
        "original condition provenance",
        condition_sources,
        limits.max_guard_provenance_sources,
    )?;
    check_limit(
        "original condition provenance coordinate cells",
        provenance_cells,
        limits.max_guard_provenance_index_cells,
    )?;

    let mut columns = BTreeMap::new();
    let mut guards = Vec::new();
    let mut normalized = Vec::with_capacity(contributions.len());
    for contribution in contributions {
        let ordinal = contribution.source_ordinal;
        let source = &sources.relations()[ordinal];
        source.validate_context(context).map_err(error)?;
        if source.family_fingerprint_owner().as_str() != sources.family_fingerprint() {
            return Err(error("combined original source belongs to another family"));
        }
        context
            .validate_with_limits(&contribution.weight, limits.indexed_algebra.exact_algebra)
            .map_err(error)?;
        // Native generator translation has already happened. Fixed target
        // specialization must come afterwards, including every source pole.
        let (weight, weight_denominator) = context
            .specialize_fixed_indices_sealed(&contribution.weight, fixed, limits.indexed_algebra)
            .map_err(error)?;
        if weight.is_zero() {
            return Err(error(
                "combined zero weight must be removed during canonical joining",
            ));
        }
        append_guard(
            &mut guards,
            weight_denominator,
            ParametricGuardOrigin::SourceCombinationDenominator {
                source_ordinal: ordinal,
                row_id: source.row_id().clone(),
            },
        )?;
        validate_guard_limits(&guards, limits)?;
        for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
            let restricted = context
                .specialize_fixed_polynomial_sealed(
                    condition.polynomial(),
                    fixed,
                    limits.indexed_algebra,
                )
                .map_err(error)?;
            append_guard(
                &mut guards,
                restricted,
                ParametricGuardOrigin::SourceCondition {
                    source_ordinal: ordinal,
                    row_id: source.row_id().clone(),
                    condition_ordinal,
                    condition_sources: condition.sources().iter().cloned().collect(),
                },
            )?;
            validate_guard_limits(&guards, limits)?;
        }
        for (shift, coefficient) in source.terms() {
            let (coefficient, source_denominator) = context
                .specialize_fixed_indices_sealed(coefficient, fixed, limits.indexed_algebra)
                .map_err(error)?;
            append_guard(
                &mut guards,
                source_denominator,
                ParametricGuardOrigin::SourceCoefficientDenominator {
                    source_ordinal: ordinal,
                    row_id: source.row_id().clone(),
                    shift: shift.clone(),
                },
            )?;
            validate_guard_limits(&guards, limits)?;
            let product = context
                .mul_bound_with_limits(
                    context.bind_sealed(&weight).map_err(error)?,
                    context.bind_sealed(&coefficient).map_err(error)?,
                    limits.indexed_algebra.exact_algebra,
                )
                .map_err(error)?;
            if let Some(previous) = columns.get_mut(shift) {
                *previous = context
                    .add_bound_with_limits(
                        context.bind_sealed(previous).map_err(error)?,
                        context.bind_sealed(&product).map_err(error)?,
                        limits.indexed_algebra.exact_algebra,
                    )
                    .map_err(error)?;
            } else {
                check_limit(
                    "original physical columns",
                    columns.len() + 1,
                    limits.max_shift_columns,
                )?;
                let cells = (columns.len() + 1)
                    .checked_mul(context.index_count())
                    .ok_or_else(|| error("combined coordinate count overflow"))?;
                check_limit(
                    "original physical coordinates",
                    cells,
                    limits.max_index_coordinate_cells,
                )?;
                columns.insert(shift.clone(), product);
            }
            check_limit("original rule guards", guards.len(), limits.max_rule_guards)?;
        }
        check_limit("original rule guards", guards.len(), limits.max_rule_guards)?;
        normalized.push((ordinal, source.row_id().clone(), weight));
    }
    columns.retain(|_, coefficient| !coefficient.is_zero());
    validate_guard_limits(&guards, limits)?;
    Ok((
        OriginalCombination {
            columns,
            guards,
            source_rows_multiplied: contributions.len(),
        },
        normalized,
    ))
}

fn check_limit(resource: &str, requested: usize, limit: usize) -> Result<(), SourcePortAuditError> {
    if requested > limit {
        Err(error(format!(
            "{resource} budget exceeded: requested {requested}, limit {limit}"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn validate_guard_limits(
    guards: &[Guard],
    limits: ParametricRuleLimits,
) -> Result<(), SourcePortAuditError> {
    check_limit("combined rule guards", guards.len(), limits.max_rule_guards)?;
    let origins = guards.iter().try_fold(0usize, |count, guard| {
        count
            .checked_add(guard.origins.len())
            .ok_or_else(|| error("combined guard origin count overflow"))
    })?;
    check_limit("combined guard origins", origins, limits.max_guard_origins)
}

pub(super) fn append_guard(
    guards: &mut Vec<Guard>,
    polynomial: IndexedPolynomial,
    origin: ParametricGuardOrigin,
) -> Result<(), SourcePortAuditError> {
    if polynomial.is_zero() {
        return Err(error("combined source applicability is identically zero"));
    }
    if polynomial.is_nonzero_constant() {
        return Ok(());
    }
    if let Some(guard) = guards
        .iter_mut()
        .find(|guard| guard.polynomial == polynomial)
    {
        if !guard.origins.contains(&origin) {
            guard.origins.push(origin);
        }
    } else {
        guards.push(Guard {
            polynomial,
            origins: vec![origin],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
