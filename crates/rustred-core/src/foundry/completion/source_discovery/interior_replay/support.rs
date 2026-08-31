use crate::foundry::completion::source_discovery::ExactSemanticExecutableOwner;
use crate::identity::IntegralShift;

use super::{
    InteriorReplayCandidateSupport, InteriorReplayRelativeResidual, InteriorReplayRelativeSource,
    InteriorReplayRunError, InteriorReplayRunLimits, InteriorReplaySupportCensus,
    InteriorReplaySupportSet,
};

const CANDIDATES: &str = "compiled support candidates";
const SOURCES: &str = "relative source supports";
const RESIDUALS: &str = "relative residual supports";
const COORDINATES: &str = "relative support coordinate cells";
const SORT_WORK: &str = "support sort work";

/// Equality of ordinal-free row/residual support shapes and guard counts.
///
/// Coefficients and guard-polynomial content are intentionally absent. A
/// match is scheduling telemetry only, not exact-rule equality, interpolation
/// evidence, admission authority, or a closure certificate.
pub(crate) fn support_shapes_match(
    left: &InteriorReplaySupportSet,
    right: &InteriorReplaySupportSet,
) -> bool {
    left == right
}

pub(super) fn try_extract_support(
    owner: &ExactSemanticExecutableOwner,
    target: &IntegralShift,
    limits: InteriorReplayRunLimits,
) -> Result<InteriorReplaySupportSet, InteriorReplayRunError> {
    let executable = owner.executable_candidates();
    check_limit(CANDIDATES, executable.len(), limits.max_support_candidates)?;
    let mut candidates = try_vec(CANDIDATES, executable.len())?;
    let mut source_count = 0usize;
    let mut residual_count = 0usize;
    let mut coordinate_cells = 0usize;
    let mut sort_work = 0usize;

    for admitted in executable {
        let circuit = admitted.circuit();
        if circuit.target_shift() != target || circuit.target_shift().len() != target.len() {
            return Err(InteriorReplayRunError::Invariant {
                detail: "compiled owner candidate target differs from the streamed task target",
            });
        }

        source_count = checked_add(SOURCES, source_count, circuit.source_combination().len())?;
        check_limit(SOURCES, source_count, limits.max_relative_source_supports)?;
        residual_count = checked_add(RESIDUALS, residual_count, circuit.residual_terms().len())?;
        check_limit(
            RESIDUALS,
            residual_count,
            limits.max_relative_residual_supports,
        )?;

        let mut sources = try_vec(SOURCES, circuit.source_combination().len())?;
        for contribution in circuit.source_combination() {
            let provenance = contribution.source_instance().provenance();
            let next_coordinate_cells = checked_add(COORDINATES, coordinate_cells, target.len())?;
            check_limit(
                COORDINATES,
                next_coordinate_cells,
                limits.max_relative_coordinate_cells,
            )?;
            let relative = relative_coordinates(
                "translated-source offset",
                provenance.offset().values(),
                target.values(),
            )?;
            coordinate_cells = next_coordinate_cells;
            sources.push(InteriorReplayRelativeSource::new(
                provenance.source_ordinal(),
                provenance.source_row().clone(),
                relative,
            ));
        }
        sort_work = checked_add(SORT_WORK, sort_work, sort_reservation(sources.len())?)?;
        check_limit(SORT_WORK, sort_work, limits.max_support_sort_work)?;
        sources.sort_unstable();
        sources.dedup();

        let mut residuals = try_vec(RESIDUALS, circuit.residual_terms().len())?;
        for term in circuit.residual_terms() {
            let next_coordinate_cells = checked_add(COORDINATES, coordinate_cells, target.len())?;
            check_limit(
                COORDINATES,
                next_coordinate_cells,
                limits.max_relative_coordinate_cells,
            )?;
            let relative = relative_coordinates(
                "exact residual shift",
                term.shift().values(),
                target.values(),
            )?;
            coordinate_cells = next_coordinate_cells;
            residuals.push(InteriorReplayRelativeResidual::new(relative));
        }
        sort_work = checked_add(SORT_WORK, sort_work, sort_reservation(residuals.len())?)?;
        check_limit(SORT_WORK, sort_work, limits.max_support_sort_work)?;
        residuals.sort_unstable();
        residuals.dedup();

        candidates.push(InteriorReplayCandidateSupport::new(
            sources,
            residuals,
            circuit.pivot_guards().len(),
            circuit.nonzero_guards().len(),
        ));
    }

    sort_work = checked_add(SORT_WORK, sort_work, sort_reservation(candidates.len())?)?;
    check_limit(SORT_WORK, sort_work, limits.max_support_sort_work)?;
    candidates.sort_unstable();
    candidates.dedup();
    Ok(InteriorReplaySupportSet::new(
        candidates,
        InteriorReplaySupportCensus::new(
            executable.len(),
            source_count,
            residual_count,
            coordinate_cells,
            sort_work,
        ),
    ))
}

fn relative_coordinates(
    object: &'static str,
    values: &[i64],
    target: &[i64],
) -> Result<Vec<i64>, InteriorReplayRunError> {
    if values.len() != target.len() {
        return Err(InteriorReplayRunError::Invariant {
            detail: "relative support and target have different arities",
        });
    }
    let mut relative = try_vec(COORDINATES, values.len())?;
    for (position, (&value, &target)) in values.iter().zip(target).enumerate() {
        relative.push(value.checked_sub(target).ok_or(
            InteriorReplayRunError::RelativeCoordinateOverflow {
                object,
                position,
                value,
                target,
            },
        )?);
    }
    Ok(relative)
}

fn sort_reservation(items: usize) -> Result<usize, InteriorReplayRunError> {
    checked_mul(SORT_WORK, items, ceil_log2(items).max(1))
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, InteriorReplayRunError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| InteriorReplayRunError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(output)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorReplayRunError> {
    left.checked_add(right)
        .ok_or(InteriorReplayRunError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorReplayRunError> {
    left.checked_mul(right)
        .ok_or(InteriorReplayRunError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), InteriorReplayRunError> {
    if requested > limit {
        Err(InteriorReplayRunError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
