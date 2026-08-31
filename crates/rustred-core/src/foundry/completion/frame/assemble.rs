//! Common validated assembly of construction-neutral physical CSR plans.

use std::sync::Arc;

use crate::identity::{IntegralShift, TranslatedSource};
use crate::sector::Mask;

use super::{PhysicalFrameError, PhysicalFrameLimits, PhysicalFramePlan, SourceInstanceId};

pub(super) const SOURCE_INSTANCES: &str = "physical-frame source instances";
const PHYSICAL_COLUMNS: &str = "physical-frame physical columns";
const PHYSICAL_COLUMN_COORDINATE_CELLS: &str = "physical-frame physical-column coordinate cells";
const PHYSICAL_ENTRIES: &str = "physical-frame physical entries";
const CSR_ROW_OFFSETS: &str = "physical-frame CSR row offsets";

/// One already-sealed translated-source owner plus its requested physical row
/// chronology. Builders can select or permute rows, but cannot inject raw CSR
/// state into the plan.
pub(super) struct OrderedTranslatedSources {
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    sources: Vec<TranslatedSource>,
    physical_source_indices: Vec<usize>,
}

impl OrderedTranslatedSources {
    pub(super) fn new(
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        sources: Vec<TranslatedSource>,
        physical_source_indices: Vec<usize>,
    ) -> Self {
        Self {
            family_fingerprint,
            context_fingerprint,
            sources,
            physical_source_indices,
        }
    }
}

/// Assemble the exact physical column union and CSR pattern shared by every
/// bounded source-selection strategy.
///
/// The input is an internal sealed source owner, not arbitrary caller-provided
/// sparse data. This boundary validates the row permutation and all arities,
/// then retains complete exact source terms and conditions without creating
/// target, helper, provenance, or symmetry columns.
pub(super) fn assemble_physical_plan(
    sector: Mask,
    ordered: OrderedTranslatedSources,
    limits: PhysicalFrameLimits,
) -> Result<PhysicalFramePlan, PhysicalFrameError> {
    let arity = sector.arity();
    check_limit("physical-frame arity", arity, limits.max_arity)?;

    let source_count = ordered.sources.len();
    if source_count == 0 {
        return Err(PhysicalFrameError::Invariant {
            detail: "physical-frame assembly received no translated sources",
        });
    }
    if ordered.physical_source_indices.len() != source_count {
        return Err(PhysicalFrameError::Invariant {
            detail: "physical-frame row chronology is not source-complete",
        });
    }
    check_limit(SOURCE_INSTANCES, source_count, limits.max_source_instances)?;
    let row_offset_count = checked_add(CSR_ROW_OFFSETS, source_count, 1)?;
    check_limit(
        CSR_ROW_OFFSETS,
        row_offset_count,
        limits.max_csr_row_offsets,
    )?;

    let mut seen_sources = try_vec(SOURCE_INSTANCES, source_count)?;
    seen_sources.resize(source_count, false);
    let mut physical_entry_count = 0usize;
    for (physical_row, &source_index) in ordered.physical_source_indices.iter().enumerate() {
        let source = ordered
            .sources
            .get(source_index)
            .ok_or(PhysicalFrameError::Invariant {
                detail: "physical-frame row chronology contains an out-of-range source",
            })?;
        if std::mem::replace(&mut seen_sources[source_index], true) {
            return Err(PhysicalFrameError::Invariant {
                detail: "physical-frame row chronology contains a duplicate source",
            });
        }
        let offset_arity = source.provenance().offset().len();
        if offset_arity != arity {
            return Err(PhysicalFrameError::WrongSourceOffsetArity {
                row: physical_row,
                expected: arity,
                actual: offset_arity,
            });
        }
        for (shift, coefficient) in source.terms() {
            if shift.values().len() != arity {
                return Err(PhysicalFrameError::WrongSourceTermArity {
                    row: physical_row,
                    expected: arity,
                    actual: shift.values().len(),
                });
            }
            if coefficient.is_zero() {
                return Err(PhysicalFrameError::ZeroSourceTerm { row: physical_row });
            }
            physical_entry_count = checked_add(PHYSICAL_ENTRIES, physical_entry_count, 1)?;
        }
    }
    if seen_sources.iter().any(|&seen| !seen) {
        return Err(PhysicalFrameError::Invariant {
            detail: "physical-frame row chronology omits a translated source",
        });
    }
    check_limit(
        PHYSICAL_ENTRIES,
        physical_entry_count,
        limits.max_physical_entries,
    )?;
    checked_u32(
        "physical-frame CSR terminal row offset",
        physical_entry_count,
    )?;

    let mut column_keys = try_vec(PHYSICAL_ENTRIES, physical_entry_count)?;
    for source in &ordered.sources {
        for shift in source.terms().keys() {
            column_keys.push(shift);
        }
    }
    if column_keys.len() != physical_entry_count {
        return Err(PhysicalFrameError::Invariant {
            detail: "physical-column collection changed the preflighted entry count",
        });
    }
    column_keys.sort_unstable_by(|left, right| left.values().cmp(right.values()));
    column_keys.dedup_by(|left, right| left.values() == right.values());

    let physical_column_count = column_keys.len();
    check_limit(
        PHYSICAL_COLUMNS,
        physical_column_count,
        limits.max_physical_columns,
    )?;
    checked_u32(
        "physical-frame physical-column count",
        physical_column_count,
    )?;
    let physical_column_coordinate_cells = checked_mul(
        PHYSICAL_COLUMN_COORDINATE_CELLS,
        physical_column_count,
        arity,
    )?;
    check_limit(
        PHYSICAL_COLUMN_COORDINATE_CELLS,
        physical_column_coordinate_cells,
        limits.max_physical_column_coordinate_cells,
    )?;

    let mut columns = try_vec(PHYSICAL_COLUMNS, physical_column_count)?;
    for key in &column_keys {
        columns.push(
            IntegralShift::try_new_with_component_limit(key.values().iter().copied(), arity)
                .map_err(PhysicalFrameError::IntegralShift)?,
        );
    }
    drop(column_keys);

    let mut row_offsets = try_vec(CSR_ROW_OFFSETS, row_offset_count)?;
    let mut column_indices = try_vec(PHYSICAL_ENTRIES, physical_entry_count)?;
    let mut source_instances = try_vec(SOURCE_INSTANCES, source_count)?;
    let mut translated_source_indices = try_vec(SOURCE_INSTANCES, source_count)?;
    row_offsets.push(0);

    for (physical_row, &source_index) in ordered.physical_source_indices.iter().enumerate() {
        let source = &ordered.sources[source_index];
        source_instances.push(SourceInstanceId::new(source.provenance().clone()));
        translated_source_indices.push(checked_u32(
            "physical-frame translated-source row index",
            source_index,
        )?);

        let mut previous_column = None;
        for shift in source.terms().keys() {
            let column = columns
                .binary_search_by(|candidate| candidate.values().cmp(shift.values()))
                .map_err(|_| PhysicalFrameError::Invariant {
                    detail: "source term is absent from the physical-column registry",
                })?;
            let column = checked_u32("physical-frame CSR column index", column)?;
            if previous_column.is_some_and(|previous| previous >= column) {
                return Err(PhysicalFrameError::Invariant {
                    detail: "one physical CSR row is not strictly column-sorted",
                });
            }
            column_indices.push(column);
            previous_column = Some(column);
        }
        debug_assert_eq!(source_instances.len(), physical_row + 1);
        row_offsets.push(checked_u32(
            "physical-frame CSR row offset",
            column_indices.len(),
        )?);
    }

    if source_instances.len() != source_count
        || translated_source_indices.len() != source_count
        || row_offsets.len() != row_offset_count
        || column_indices.len() != physical_entry_count
    {
        return Err(PhysicalFrameError::Invariant {
            detail: "assembled physical CSR dimensions differ from their preflight",
        });
    }

    Ok(PhysicalFramePlan::from_parts(
        ordered.family_fingerprint,
        ordered.context_fingerprint,
        sector,
        columns,
        row_offsets,
        column_indices,
        source_instances,
        translated_source_indices,
        ordered.sources,
    ))
}

pub(super) fn total_translation_degree(
    offset: &IntegralShift,
) -> Result<usize, PhysicalFrameError> {
    offset.values().iter().try_fold(0usize, |total, value| {
        let magnitude = usize::try_from(value.unsigned_abs()).map_err(|_| {
            PhysicalFrameError::ResourceCountOverflow {
                resource: "physical-frame total translation degree",
            }
        })?;
        checked_add("physical-frame total translation degree", total, magnitude)
    })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), PhysicalFrameError> {
    if requested > limit {
        Err(PhysicalFrameError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, PhysicalFrameError> {
    left.checked_add(right)
        .ok_or(PhysicalFrameError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, PhysicalFrameError> {
    left.checked_mul(right)
        .ok_or(PhysicalFrameError::ResourceCountOverflow { resource })
}

pub(super) fn checked_u32(resource: &'static str, value: usize) -> Result<u32, PhysicalFrameError> {
    u32::try_from(value).map_err(|_| PhysicalFrameError::U32NotRepresentable { resource, value })
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, PhysicalFrameError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| PhysicalFrameError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

#[cfg(test)]
pub(super) fn checked_u32_for_test(
    resource: &'static str,
    value: usize,
) -> Result<u32, PhysicalFrameError> {
    checked_u32(resource, value)
}
