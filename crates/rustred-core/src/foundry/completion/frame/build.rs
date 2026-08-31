//! Rectangular one-sided chart construction over the common physical plan.

use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::Mask;

use super::assemble::{
    OrderedTranslatedSources, SOURCE_INSTANCES, assemble_physical_plan, check_limit, checked_add,
    checked_mul, try_vec,
};
use super::{OneSidedChartFrame, PhysicalFrameError, PhysicalFrameLimits};

const OFFSETS: &str = "physical-frame chart offsets";
const OFFSET_COORDINATE_CELLS: &str = "physical-frame offset coordinate cells";

impl OneSidedChartFrame {
    /// Construct the raw physical translated-source pattern for the
    /// rectangular one-sided chart frame `M_degree(sector)`.
    ///
    /// Rows are ordered by total chart degree, then chart-lexicographically,
    /// then by the completed ordinary-source chronology. The returned shell
    /// retains only chart scheduling metadata; its nested plan is the same
    /// construction-neutral type accepted by modular and exact proof layers.
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

        // The translation owner is offset-lexicographic. Build only a compact
        // permutation into total-degree/chart-lex/source chronology; no exact
        // relation is copied or regenerated.
        let mut physical_source_indices = try_vec(SOURCE_INSTANCES, expected_rows)?;
        for offset in &offsets {
            let canonical_offset_ordinal = translated_sources
                .offsets()
                .binary_search(offset)
                .map_err(|_| PhysicalFrameError::Invariant {
                    detail: "ordered chart offset is absent from the translated-source batch",
                })?;
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
                        detail: "translated-source provenance disagrees with chart row order",
                    });
                }
                physical_source_indices.push(canonical_source_index);
            }
        }

        let (family_fingerprint, context_fingerprint, sources) =
            translated_sources.into_foundry_parts();
        let plan = assemble_physical_plan(
            sector,
            OrderedTranslatedSources::new(
                family_fingerprint,
                context_fingerprint,
                sources,
                physical_source_indices,
            ),
            limits,
        )?;
        Ok(Self::from_parts(plan, degree, offsets))
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
