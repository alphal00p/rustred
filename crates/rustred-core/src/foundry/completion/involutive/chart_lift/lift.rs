use crate::algebra::IndexedCoefficientContext;
use crate::identity::{CompletedIbpSourceRows, ParametricRelation};

use super::super::error::{check_limit, checked_add, checked_mul, try_vec};
use super::super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, OreConsequence, OreOrderingAdapter, OreRow,
};
use super::census::{
    SymbolicCensus, authenticate_input_symbolic_census, preflight_batch_symbolic_limits,
    preflight_relation_symbolic_limits,
};
use super::{
    LiftedOrdinarySource, LiftedOrdinarySourceBatch, OrdinaryChartLiftError,
    OrdinaryChartLiftLimits,
};

/// Lift a sealed complete ordinary source module into one sector-forward Ore
/// chart.  For source row
///
/// `P = sum_delta c_delta(n) E^delta`,
///
/// this computes the componentwise-minimal forward shift `lambda` for which
/// every `lambda + chart(delta)` is nonnegative, and returns
///
/// `E^lambda P = sum_delta c_delta(n + physical(lambda))
///                         E^(lambda + chart(delta))`.
///
/// Every input cardinality, symbolic payload, coordinate-cell, known degree,
/// and chart-conversion-work bound is checked before retained output is
/// allocated. Symbolica translations remain independently bounded because an
/// affine substitution can expand an output polynomial beyond what its input
/// sparse census alone can predict.
pub(crate) fn try_lift_completed_ordinary_sources(
    completed: &CompletedIbpSourceRows,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: OrdinaryChartLiftLimits,
) -> Result<LiftedOrdinarySourceBatch, OrdinaryChartLiftError> {
    if !completed.is_complete_ordinary() {
        return Err(OrdinaryChartLiftError::SourceLayout {
            actual: completed.layout_name(),
        });
    }
    if completed.context_fingerprint() != context.fingerprint() {
        return Err(OrdinaryChartLiftError::ContextMismatch);
    }
    if !ordering.owns_completed_source_module(completed) {
        return Err(OrdinaryChartLiftError::ForeignSourceOwner);
    }
    ordering
        .require_arity("completed ordinary source rows", context.index_count())
        .map_err(OrdinaryChartLiftError::from)?;
    let relations = completed.relations();
    if relations.is_empty() {
        return Err(OrdinaryChartLiftError::EmptySourceRows);
    }
    preflight_batch(relations, ordering, context, limits)?;

    let mut sources = try_vec("lifted ordinary source rows", relations.len())?;
    for (source_ordinal, relation) in relations.iter().enumerate() {
        sources.push(build_lifted_source(
            relation,
            source_ordinal,
            ordering,
            context,
            limits.involutive,
        )?);
    }
    Ok(LiftedOrdinarySourceBatch {
        completed_owner: completed.identity_owner(),
        sources: sources.into_boxed_slice(),
    })
}

fn preflight_batch(
    relations: &[ParametricRelation],
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: OrdinaryChartLiftLimits,
) -> Result<(), OrdinaryChartLiftError> {
    let arity = ordering.arity();
    preflight_nested_shape_limits(arity, limits.involutive)?;
    check_limit(
        "ordinary chart-lift source rows",
        relations.len(),
        limits.max_source_rows,
    )?;
    let mut input_terms = 0usize;
    let mut input_conditions = 0usize;
    let mut guard_census = SymbolicCensus::default();
    let mut symbolic_census = SymbolicCensus::default();
    for (source_ordinal, relation) in relations.iter().enumerate() {
        relation.validate_context(context)?;
        if relation.terms().is_empty() {
            return Err(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal });
        }
        input_terms = checked_add(
            "ordinary chart-lift input terms",
            input_terms,
            relation.terms().len(),
        )?;
        input_conditions = checked_add(
            "ordinary chart-lift input conditions",
            input_conditions,
            relation.nonzero_conditions().len(),
        )?;
        let relation_census = authenticate_input_symbolic_census(
            relation,
            context,
            limits.involutive.indexed_algebra,
        )?;
        preflight_relation_symbolic_limits(relation, relation_census, limits.involutive)?;
        guard_census = guard_census.try_add(
            relation_census.guards,
            "ordinary chart-lift input guard payload",
        )?;
        symbolic_census = symbolic_census.try_add(
            relation_census.all,
            "ordinary chart-lift input symbolic payload",
        )?;
    }
    check_limit(
        "ordinary chart-lift input terms",
        input_terms,
        limits.max_input_terms,
    )?;
    check_limit(
        "ordinary chart-lift coefficient translations",
        input_terms,
        limits.max_coefficient_translations,
    )?;
    check_limit(
        "ordinary chart-lift input conditions",
        input_conditions,
        limits.max_input_conditions,
    )?;
    preflight_batch_symbolic_limits(guard_census, symbolic_census, limits)?;

    let input_cells = checked_mul(
        "ordinary chart-lift input coordinate cells",
        input_terms,
        arity,
    )?;
    check_limit(
        "ordinary chart-lift input coordinate cells",
        input_cells,
        limits.max_input_coordinate_cells,
    )?;
    let left_shift_cells = checked_mul(
        "ordinary chart-lift retained coordinate cells",
        relations.len(),
        arity,
    )?;
    let lifted_cells = checked_add(
        "ordinary chart-lift retained coordinate cells",
        input_cells,
        left_shift_cells,
    )?;
    check_limit(
        "ordinary chart-lift retained coordinate cells",
        lifted_cells,
        limits.max_lifted_coordinate_cells,
    )?;
    let conversion_work = checked_add(
        "ordinary chart-lift conversion work",
        checked_mul("ordinary chart-lift conversion work", input_cells, 4)?,
        left_shift_cells,
    )?;
    check_limit(
        "ordinary chart-lift conversion work",
        conversion_work,
        limits.max_chart_conversion_work,
    )?;

    for (source_ordinal, relation) in relations.iter().enumerate() {
        preflight_relation_geometry(relation, source_ordinal, ordering, limits.involutive)?;
    }
    Ok(())
}

pub(super) fn preflight_relation(
    relation: &ParametricRelation,
    source_ordinal: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: OrdinaryChartLiftLimits,
) -> Result<(), OrdinaryChartLiftError> {
    preflight_nested_shape_limits(ordering.arity(), limits.involutive)?;
    relation.validate_context(context)?;
    if relation.terms().is_empty() {
        return Err(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal });
    }
    check_limit("ordinary chart-lift source rows", 1, limits.max_source_rows)?;
    check_limit(
        "ordinary chart-lift input terms",
        relation.terms().len(),
        limits.max_input_terms,
    )?;
    check_limit(
        "ordinary chart-lift coefficient translations",
        relation.terms().len(),
        limits.max_coefficient_translations,
    )?;
    check_limit(
        "ordinary chart-lift input conditions",
        relation.nonzero_conditions().len(),
        limits.max_input_conditions,
    )?;
    let relation_census =
        authenticate_input_symbolic_census(relation, context, limits.involutive.indexed_algebra)?;
    preflight_relation_symbolic_limits(relation, relation_census, limits.involutive)?;
    preflight_batch_symbolic_limits(relation_census.guards, relation_census.all, limits)?;
    let input_cells = checked_mul(
        "ordinary chart-lift input coordinate cells",
        relation.terms().len(),
        ordering.arity(),
    )?;
    check_limit(
        "ordinary chart-lift input coordinate cells",
        input_cells,
        limits.max_input_coordinate_cells,
    )?;
    let retained_cells = checked_add(
        "ordinary chart-lift retained coordinate cells",
        input_cells,
        ordering.arity(),
    )?;
    check_limit(
        "ordinary chart-lift retained coordinate cells",
        retained_cells,
        limits.max_lifted_coordinate_cells,
    )?;
    let work = checked_add(
        "ordinary chart-lift conversion work",
        checked_mul("ordinary chart-lift conversion work", input_cells, 4)?,
        ordering.arity(),
    )?;
    check_limit(
        "ordinary chart-lift conversion work",
        work,
        limits.max_chart_conversion_work,
    )?;
    preflight_relation_geometry(relation, source_ordinal, ordering, limits.involutive)
}

fn preflight_relation_geometry(
    relation: &ParametricRelation,
    source_ordinal: usize,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
) -> Result<(), OrdinaryChartLiftError> {
    if relation.terms().is_empty() {
        return Err(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal });
    }
    check_limit(
        "ordinary chart-lift row terms",
        relation.terms().len(),
        limits.max_row_terms,
    )?;
    let arity = ordering.arity();
    for shift in relation.terms().keys() {
        ordering.require_arity("ordinary source integral shift", shift.values().len())?;
    }

    let mut left_values = try_vec("ordinary chart-lift preflight left coordinates", arity)?;
    let mut left_total_degree = 0usize;
    for position in 0..arity {
        let coordinate = minimal_left_coordinate(relation, ordering, position, source_ordinal)?;
        check_forward_coordinate(position, coordinate, limits)?;
        left_total_degree = checked_add(
            "ordinary chart-lift left-shift degree",
            left_total_degree,
            usize::try_from(coordinate).map_err(|_| InvolutiveError::ResourceCountOverflow {
                resource: "ordinary chart-lift left-shift degree",
            })?,
        )?;
        check_limit(
            "ordinary chart-lift left-shift degree",
            left_total_degree,
            limits.max_total_shift_degree,
        )?;
        left_values.push(coordinate);
    }

    for shift in relation.terms().keys() {
        let mut total_degree = 0usize;
        for position in 0..arity {
            let coordinate = lifted_forward_coordinate(
                shift.values()[position],
                ordering.sector().active_bits()[position],
                left_values[position],
            )?;
            check_forward_coordinate(position, coordinate, limits)?;
            total_degree = checked_add(
                "ordinary chart-lift output degree",
                total_degree,
                usize::try_from(coordinate).map_err(|_| {
                    InvolutiveError::ResourceCountOverflow {
                        resource: "ordinary chart-lift output degree",
                    }
                })?,
            )?;
            check_limit(
                "ordinary chart-lift output degree",
                total_degree,
                limits.max_total_shift_degree,
            )?;
        }
    }
    Ok(())
}

pub(super) fn build_lifted_source(
    relation: &ParametricRelation,
    source_ordinal: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> Result<LiftedOrdinarySource, OrdinaryChartLiftError> {
    let arity = ordering.arity();
    let mut left_values = try_vec("ordinary chart-lift left-shift coordinates", arity)?;
    for position in 0..arity {
        left_values.push(minimal_left_coordinate(
            relation,
            ordering,
            position,
            source_ordinal,
        )?);
    }
    let left_shift = ForwardShift::try_new(left_values, limits)?;
    let physical_left_shift = ordering.try_physical_translation(&left_shift)?;
    let mut terms = try_vec("lifted ordinary Ore row terms", relation.terms().len())?;
    for (physical_shift, coefficient) in relation.terms() {
        let mut forward_values = try_vec("lifted ordinary term coordinates", arity)?;
        for (position, (&value, &active)) in physical_shift
            .values()
            .iter()
            .zip(ordering.sector().active_bits())
            .enumerate()
        {
            forward_values.push(lifted_forward_coordinate(
                value,
                active,
                left_shift.values()[position],
            )?);
        }
        let shift = ForwardShift::try_new(forward_values, limits)?;
        let coefficient =
            context.translate_sealed(coefficient, &physical_left_shift, limits.indexed_algebra)?;
        terms.push((shift, coefficient));
    }
    let row = OreRow::try_new(ordering, terms, context, limits)?;
    if row.is_zero() {
        return Err(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal });
    }
    let mut consequence = OreConsequence::try_from_left_shifted_source(
        source_ordinal,
        left_shift.clone(),
        row,
        ordering,
        context,
        limits,
    )?;
    let mut guards = try_vec(
        "translated ordinary localization guards",
        relation.nonzero_conditions().len(),
    )?;
    for condition in relation.nonzero_conditions() {
        guards.push(context.translate_polynomial_sealed(
            condition.polynomial(),
            &physical_left_shift,
            limits.indexed_algebra,
        )?);
    }
    consequence = consequence.try_require_nonzero_guards(guards, context, limits)?;
    Ok(LiftedOrdinarySource {
        source_ordinal,
        source_row: relation.row_id().clone(),
        left_shift,
        consequence,
    })
}

fn preflight_nested_shape_limits(
    arity: usize,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    check_limit("Ore arity", arity, limits.max_arity)?;
    check_limit("Ore provenance terms", 1, limits.max_provenance_terms)
}

fn minimal_left_coordinate(
    relation: &ParametricRelation,
    ordering: &OreOrderingAdapter,
    position: usize,
    source_ordinal: usize,
) -> Result<u64, OrdinaryChartLiftError> {
    let active = ordering.sector().active_bits()[position];
    let minimum = relation
        .terms()
        .keys()
        .map(|shift| chart_coordinate(shift.values()[position], active))
        .min()
        .ok_or(OrdinaryChartLiftError::EmptySourceRelation { source_ordinal })?;
    if minimum < 0 {
        Ok(
            u64::try_from(-minimum).map_err(|_| InvolutiveError::ResourceCountOverflow {
                resource: "ordinary chart-lift left-shift coordinate",
            })?,
        )
    } else {
        Ok(0)
    }
}

fn lifted_forward_coordinate(
    physical: i64,
    active: bool,
    left: u64,
) -> Result<u64, InvolutiveError> {
    let value = i128::from(left) + chart_coordinate(physical, active);
    if value < 0 {
        return Err(InvolutiveError::Invariant {
            detail: "minimal common chart lift retained a negative exponent",
        });
    }
    u64::try_from(value).map_err(|_| InvolutiveError::ResourceCountOverflow {
        resource: "ordinary chart-lift forward coordinate",
    })
}

fn chart_coordinate(physical: i64, active: bool) -> i128 {
    let value = i128::from(physical);
    if active { value } else { -value }
}

fn check_forward_coordinate(
    position: usize,
    coordinate: u64,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    if coordinate > limits.max_shift_coordinate {
        return Err(InvolutiveError::ShiftCoordinateLimit {
            position,
            requested: coordinate,
            limit: limits.max_shift_coordinate,
        });
    }
    if coordinate > i64::MAX as u64 {
        return Err(InvolutiveError::ShiftCoordinateNotRepresentable {
            position,
            coordinate,
        });
    }
    Ok(())
}
