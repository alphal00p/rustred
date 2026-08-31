use crate::algebra::IndexedCoefficientContext;
use crate::identity::{IndexShift, ParametricRelation};

use super::super::ExactTargetCircuit;
use super::resource::{check_limit, checked_add, checked_mul};
use super::{ExactCircuitLoweringError, ExactCircuitLoweringLimits};

pub(super) fn preflight_circuit_payload(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    limits: ExactCircuitLoweringLimits,
) -> Result<(), ExactCircuitLoweringError> {
    let policy = limits.parametric;
    check_limit(
        "source combination terms",
        circuit.source_combination().len(),
        policy.max_source_combination_terms,
    )?;
    check_limit(
        "elimination pivots",
        circuit.pivot_guards().len(),
        policy.max_elimination_pivots,
    )?;
    check_limit(
        "sector mask cells",
        context.index_count(),
        policy.max_sector_mask_cells,
    )?;

    let domain_count = checked_add(
        "sector-monotone domain containers",
        circuit.residual_terms().len(),
        2,
    )?;
    let endpoints_per_domain = checked_mul(
        "sector-monotone domain bound endpoint cells",
        context.index_count(),
        2,
    )?;
    let domain_endpoints = checked_mul(
        "sector-monotone domain bound endpoint cells",
        domain_count,
        endpoints_per_domain,
    )?;
    check_limit(
        "sector-monotone domain bound endpoint cells",
        domain_endpoints,
        policy.max_domain_bound_endpoint_cells,
    )?;

    let monotone_domain = circuit.residual_terms()[0].descent().domain();
    let mut threshold_count = 0usize;
    for term in circuit.residual_terms() {
        threshold_count = checked_add(
            "sector-monotone active pinch thresholds",
            threshold_count,
            monotone_domain.retained_pinch_threshold_count(term.shift().values())?,
        )?;
    }
    check_limit(
        "sector-monotone active pinch thresholds",
        threshold_count,
        policy.max_sector_monotone_thresholds,
    )?;

    let algebra = policy.indexed_algebra.exact_algebra;
    for term in circuit.residual_terms() {
        context.validate_with_limits(term.coefficient(), algebra)?;
    }
    for contribution in circuit.source_combination() {
        context.validate_with_limits(contribution.coefficient(), algebra)?;
    }
    for pivot in circuit.pivot_guards() {
        context.validate_with_limits(pivot.coefficient(), algebra)?;
        context.validate_polynomial_with_limits(pivot.nonzero_polynomial(), algebra)?;
    }
    for guard in circuit.nonzero_guards() {
        context.validate_polynomial_with_limits(guard.polynomial(), algebra)?;
    }
    Ok(())
}

pub(super) fn preflight_source_view_and_rule(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    selected_rows: &[usize],
    relations: &[ParametricRelation],
    source_shift_columns: &[IndexShift],
    limits: ExactCircuitLoweringLimits,
) -> Result<(), ExactCircuitLoweringError> {
    let policy = limits.parametric;
    check_limit(
        "parametric source rows",
        selected_rows.len(),
        policy.max_source_rows,
    )?;

    let source_terms = relations.iter().try_fold(0usize, |total, relation| {
        checked_add(
            "parametric source nonzero entries",
            total,
            relation.terms().len(),
        )
    })?;
    let input_entries = checked_add(
        "parametric source nonzero entries",
        source_terms,
        relations.len(),
    )?;
    check_limit(
        "parametric source nonzero entries",
        input_entries,
        policy.max_input_nonzero_entries,
    )?;
    check_limit(
        "parametric shift columns",
        source_shift_columns.len(),
        policy.max_shift_columns,
    )?;

    let index_buffers = checked_add(
        "live parametric index-coordinate buffers",
        source_shift_columns.len(),
        1,
    )?;
    let index_cells = checked_mul(
        "live parametric index-coordinate cells",
        index_buffers,
        context.index_count(),
    )?;
    check_limit(
        "live parametric index-coordinate cells",
        index_cells,
        policy.max_index_coordinate_cells,
    )?;

    let witness_keys = checked_mul(
        "live parametric ordering keys",
        circuit.residual_terms().len(),
        4,
    )?;
    let ordering_keys = checked_add(
        "live parametric ordering keys",
        source_shift_columns.len(),
        witness_keys,
    )?;
    let ordering_cells = checked_mul(
        "live parametric ordering-key coordinate cells",
        ordering_keys,
        context.index_count(),
    )?;
    check_limit(
        "live parametric ordering-key coordinate cells",
        ordering_cells,
        policy.max_ordering_key_coordinate_cells,
    )?;
    Ok(())
}
