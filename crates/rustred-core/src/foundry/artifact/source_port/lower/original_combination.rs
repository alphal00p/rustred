//! Compile the complete original weighted row sum once per parent rule.
//! This intermediate is arithmetic data, NOT replay or installation authority.
//! Cell-specific desired identities must still be subtracted and every full
//! residual product proved zero on their exact unbounded domain.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::SourceViewBatch;
use crate::foundry::parametric::ParametricGuardOrigin;
use crate::identity::{IndexShift, RowId};

use super::super::{SourcePortAuditError, error};

/// Joined by original RowId AND wide target-relative translation, not by the
/// preconditioned source ordinal. Ordinals here address the unchanged batch.
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
    /// Full physical columns, including zero-sector and activating columns.
    /// This map is shared across the rule's refined cells; never recompute
    /// sparse elimination or duplicate original-row multiplication per cell.
    pub columns: BTreeMap<IndexShift, IndexedCoefficient>,
    pub guards: Vec<Guard>,
    pub source_rows_multiplied: usize,
}

pub(super) fn compile(
    context: &IndexedCoefficientContext,
    sources: &SourceViewBatch,
    contributions: &[JoinedContribution],
    fixed: &[(usize, i64)],
) -> Result<OriginalCombination, SourcePortAuditError> {
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
    let mut used = BTreeSet::new();
    let mut columns = BTreeMap::new();
    let mut guards = Vec::new();
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
        source.validate_context(context).map_err(error)?;
        if source.family_fingerprint_owner().as_str() != sources.family_fingerprint() {
            return Err(error("combined original source belongs to another family"));
        }
        context
            .validate_with_limits(&contribution.weight, Default::default())
            .map_err(error)?;
        // Sources were translated by the native generator before reaching
        // this function. Only now is the target's exact fixed face applied.
        let (weight, weight_denominator) = context
            .specialize_fixed_indices_sealed(&contribution.weight, fixed, Default::default())
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
        for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
            let restricted = context
                .specialize_fixed_polynomial_sealed(
                    condition.polynomial(),
                    fixed,
                    Default::default(),
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
        }
        for (shift, coefficient) in source.terms() {
            let (coefficient, source_denominator) = context
                .specialize_fixed_indices_sealed(coefficient, fixed, Default::default())
                .map_err(error)?;
            // Keep source-denominator applicability even if multiplication
            // or later row cancellation removes that denominator entirely.
            append_guard(
                &mut guards,
                source_denominator,
                ParametricGuardOrigin::SourceCoefficientDenominator {
                    source_ordinal: ordinal,
                    row_id: source.row_id().clone(),
                    shift: shift.clone(),
                },
            )?;
            let product = context
                .mul_bound_with_limits(
                    context.bind_sealed(&weight).map_err(error)?,
                    context.bind_sealed(&coefficient).map_err(error)?,
                    Default::default(),
                )
                .map_err(error)?;
            if let Some(previous) = columns.get_mut(shift) {
                *previous = context
                    .add_bound_with_limits(
                        context.bind_sealed(previous).map_err(error)?,
                        context.bind_sealed(&product).map_err(error)?,
                        Default::default(),
                    )
                    .map_err(error)?;
            } else {
                columns.insert(shift.clone(), product);
            }
        }
    }
    columns.retain(|_, coefficient| !coefficient.is_zero());
    Ok(OriginalCombination {
        columns,
        guards,
        source_rows_multiplied: contributions.len(),
    })
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
