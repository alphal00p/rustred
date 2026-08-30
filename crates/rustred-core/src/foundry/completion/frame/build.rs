use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::Mask;

use super::{PhysicalFrameError, PhysicalFrameLimits, PhysicalFramePlan, SourceInstanceId};

const OFFSETS: &str = "physical-frame chart offsets";
const OFFSET_COORDINATE_CELLS: &str = "physical-frame offset coordinate cells";
const SOURCE_INSTANCES: &str = "physical-frame source instances";
const PHYSICAL_COLUMNS: &str = "physical-frame physical columns";
const PHYSICAL_COLUMN_COORDINATE_CELLS: &str = "physical-frame physical-column coordinate cells";
const PHYSICAL_ENTRIES: &str = "physical-frame physical entries";
const CSR_ROW_OFFSETS: &str = "physical-frame CSR row offsets";

impl PhysicalFramePlan {
    /// Construct the raw physical translated-source pattern for the
    /// one-sided chart frame `M_degree(sector)`.
    ///
    /// Rows are ordered by total chart degree, then chart-lexicographically,
    /// then by the completed ordinary-source chronology. Columns are sorted
    /// raw [`IntegralShift`] keys. No symmetry or provenance column enters the
    /// physical pattern.
    pub(crate) fn try_new(
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        sector: Mask,
        degree: usize,
        limits: PhysicalFrameLimits,
    ) -> Result<Self, PhysicalFrameError> {
        if !completed.is_complete_ordinary() {
            return Err(PhysicalFrameError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        let arity = generator.context().index_count();
        // ParametricIbpGenerator and Mask construction independently reject
        // an empty index space, so an equal zero-arity pair is unreachable.
        if sector.arity() != arity {
            return Err(PhysicalFrameError::WrongSectorArity {
                expected: arity,
                actual: sector.arity(),
            });
        }
        check_limit("physical-frame arity", arity, limits.max_arity)?;
        check_limit("physical-frame degree", degree, limits.max_degree)?;
        i64::try_from(degree).map_err(|_| PhysicalFrameError::DegreeNotRepresentable { degree })?;

        let offset_count = count_offsets(arity, degree)?;
        check_limit(OFFSETS, offset_count, limits.max_offsets)?;
        let offset_coordinate_cells = checked_mul(OFFSET_COORDINATE_CELLS, offset_count, arity)?;
        check_limit(
            OFFSET_COORDINATE_CELLS,
            offset_coordinate_cells,
            limits.max_offset_coordinate_cells,
        )?;
        let offsets = enumerate_offsets(&sector, degree, offset_count)?;

        let translated_sources = generator
            .translate_completed_source_rows(
                completed,
                offsets.iter().cloned(),
                limits.translated_sources,
            )
            .map_err(PhysicalFrameError::TranslatedSources)?;
        if translated_sources.offsets().len() != offset_count {
            return Err(PhysicalFrameError::Invariant {
                detail: "translated-source canonicalization changed the unique offset count",
            });
        }

        let source_row_count = translated_sources.source_row_count();
        let expected_rows = checked_mul(SOURCE_INSTANCES, offset_count, source_row_count)?;
        if translated_sources.len() != expected_rows {
            return Err(PhysicalFrameError::Invariant {
                detail: "translated-source batch has the wrong row count",
            });
        }
        check_limit(SOURCE_INSTANCES, expected_rows, limits.max_source_instances)?;
        let row_offset_count = checked_add(CSR_ROW_OFFSETS, expected_rows, 1)?;
        check_limit(
            CSR_ROW_OFFSETS,
            row_offset_count,
            limits.max_csr_row_offsets,
        )?;

        let physical_entry_count = count_nonzero_entries(&translated_sources)?;
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
        for source in translated_sources.sources() {
            column_keys.extend(source.terms().keys());
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
        let mut source_instances = try_vec(SOURCE_INSTANCES, expected_rows)?;
        let mut translated_source_indices = try_vec(SOURCE_INSTANCES, expected_rows)?;
        row_offsets.push(0);

        for offset in &offsets {
            let canonical_offset_ordinal = translated_sources
                .offsets()
                .binary_search(offset)
                .map_err(|_| PhysicalFrameError::Invariant {
                    detail: "ordered chart offset is absent from the translated-source batch",
                })?;
            let total_translation_degree = total_translation_degree(offset)?;
            for source_ordinal in 0..source_row_count {
                let canonical_source_index = checked_add(
                    SOURCE_INSTANCES,
                    checked_mul(SOURCE_INSTANCES, canonical_offset_ordinal, source_row_count)?,
                    source_ordinal,
                )?;
                let source = translated_sources
                    .sources()
                    .get(canonical_source_index)
                    .ok_or(PhysicalFrameError::Invariant {
                        detail: "translated-source row permutation is outside the batch",
                    })?;
                if source.provenance().source_ordinal() != source_ordinal
                    || source.provenance().offset() != offset
                {
                    return Err(PhysicalFrameError::Invariant {
                        detail: "translated-source provenance disagrees with frame row order",
                    });
                }
                source_instances.push(SourceInstanceId::new(
                    total_translation_degree,
                    source.provenance().clone(),
                ));
                translated_source_indices.push(checked_u32(
                    "physical-frame translated-source row index",
                    canonical_source_index,
                )?);

                let physical_row = source_instances.len() - 1;
                let mut previous_column = None;
                for (shift, coefficient) in source.terms() {
                    if coefficient.is_zero() {
                        return Err(PhysicalFrameError::ZeroSourceTerm { row: physical_row });
                    }
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
                row_offsets.push(checked_u32(
                    "physical-frame CSR row offset",
                    column_indices.len(),
                )?);
            }
        }

        if source_instances.len() != expected_rows
            || translated_source_indices.len() != expected_rows
            || row_offsets.len() != row_offset_count
            || column_indices.len() != physical_entry_count
        {
            return Err(PhysicalFrameError::Invariant {
                detail: "assembled physical CSR dimensions differ from their preflight",
            });
        }

        Ok(Self::from_parts(
            sector,
            degree,
            offsets,
            columns,
            row_offsets,
            column_indices,
            source_instances,
            translated_source_indices,
            translated_sources,
        ))
    }
}

fn count_offsets(arity: usize, degree: usize) -> Result<usize, PhysicalFrameError> {
    let count_cells = checked_add("physical-frame offset-count work cells", degree, 1)?;
    let mut exact_counts = try_vec("physical-frame offset-count work cells", count_cells)?;
    exact_counts.resize(count_cells, 0usize);
    exact_counts[0] = 1;
    for _ in 0..arity {
        for total_degree in 1..=degree {
            exact_counts[total_degree] = checked_add(
                OFFSETS,
                exact_counts[total_degree],
                exact_counts[total_degree - 1],
            )?;
        }
    }
    exact_counts
        .into_iter()
        .try_fold(0usize, |total, exact| checked_add(OFFSETS, total, exact))
}

fn enumerate_offsets(
    sector: &Mask,
    degree: usize,
    expected: usize,
) -> Result<Vec<IntegralShift>, PhysicalFrameError> {
    let arity = sector.arity();
    let mut offsets = try_vec(OFFSETS, expected)?;
    let mut chart = try_vec("physical-frame chart enumeration coordinates", arity)?;
    chart.resize(arity, 0usize);
    let mut remaining = try_vec("physical-frame chart enumeration budgets", arity)?;
    remaining.resize(arity, 0usize);
    let mut next = try_vec("physical-frame chart enumeration cursors", arity)?;
    next.resize(arity, None::<usize>);

    for total_degree in 0..=degree {
        if arity == 1 {
            chart[0] = total_degree;
            push_chart_offset(&mut offsets, sector, &chart)?;
            continue;
        }

        remaining[0] = total_degree;
        next.fill(None);
        next[0] = Some(0);
        let mut position = 0usize;
        loop {
            if position == arity - 1 {
                chart[position] = remaining[position];
                push_chart_offset(&mut offsets, sector, &chart)?;
                position -= 1;
                continue;
            }

            let Some(coordinate) = next[position] else {
                if position == 0 {
                    break;
                }
                position -= 1;
                continue;
            };
            next[position] = if coordinate == remaining[position] {
                None
            } else {
                Some(checked_add(
                    "physical-frame chart enumeration cursor",
                    coordinate,
                    1,
                )?)
            };
            chart[position] = coordinate;
            remaining[position + 1] = remaining[position] - coordinate;
            position += 1;
            if position < arity - 1 {
                next[position] = Some(0);
            }
        }
    }

    if offsets.len() != expected {
        return Err(PhysicalFrameError::Invariant {
            detail: "chart enumeration did not reproduce the preflighted offset count",
        });
    }
    Ok(offsets)
}

fn push_chart_offset(
    offsets: &mut Vec<IntegralShift>,
    sector: &Mask,
    chart: &[usize],
) -> Result<(), PhysicalFrameError> {
    let mut values = try_vec("physical-frame integral-shift coordinates", chart.len())?;
    for (&coordinate, &active) in chart.iter().zip(sector.active_bits()) {
        let coordinate = i64::try_from(coordinate)
            .map_err(|_| PhysicalFrameError::DegreeNotRepresentable { degree: coordinate })?;
        values.push(if active { coordinate } else { -coordinate });
    }
    offsets.push(
        IntegralShift::try_new_with_component_limit(values, chart.len())
            .map_err(PhysicalFrameError::IntegralShift)?,
    );
    Ok(())
}

fn count_nonzero_entries(
    translated_sources: &crate::identity::TranslatedSourceBatch,
) -> Result<usize, PhysicalFrameError> {
    let mut entries = 0usize;
    for (row, source) in translated_sources.sources().iter().enumerate() {
        for coefficient in source.terms().values() {
            if coefficient.is_zero() {
                return Err(PhysicalFrameError::ZeroSourceTerm { row });
            }
            entries = checked_add(PHYSICAL_ENTRIES, entries, 1)?;
        }
    }
    Ok(entries)
}

fn total_translation_degree(offset: &IntegralShift) -> Result<usize, PhysicalFrameError> {
    offset.values().iter().try_fold(0usize, |total, value| {
        let magnitude = usize::try_from(value.unsigned_abs()).map_err(|_| {
            PhysicalFrameError::ResourceCountOverflow {
                resource: "physical-frame total translation degree",
            }
        })?;
        checked_add("physical-frame total translation degree", total, magnitude)
    })
}

fn check_limit(
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

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, PhysicalFrameError> {
    left.checked_add(right)
        .ok_or(PhysicalFrameError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, PhysicalFrameError> {
    left.checked_mul(right)
        .ok_or(PhysicalFrameError::ResourceCountOverflow { resource })
}

fn checked_u32(resource: &'static str, value: usize) -> Result<u32, PhysicalFrameError> {
    u32::try_from(value).map_err(|_| PhysicalFrameError::U32NotRepresentable { resource, value })
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, PhysicalFrameError> {
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
