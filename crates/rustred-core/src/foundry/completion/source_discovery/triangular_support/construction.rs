use crate::foundry::completion::frame::{PhysicalFrameError, SelectedSourceFrame};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::Mask;

use super::super::{AccumulatedSourceRequests, CampaignLimits};
use super::enumeration::{count_offsets, enumerate_requests, validate_axes};
use super::resource::{
    check_limit, check_physical_frame_limit, check_selected_translation_limit, checked_add,
    checked_mul, try_vec,
};
use super::{
    ACCUMULATED_REQUESTS, ARITY, CANONICAL_REQUESTS, DEGREE, OFFSET_COORDINATES, OFFSETS,
    PHYSICAL_CSR_ROW_OFFSETS, PHYSICAL_SOURCE_INSTANCES, REQUEST_COORDINATES,
    SELECTED_TRANSLATION_OFFSETS, SELECTED_TRANSLATION_REQUESTS, SELECTED_TRANSLATION_SOURCES,
    SUBMITTED_REQUESTS, TriangularSupportError,
};

/// Materialize exactly the requested per-source triangular support.
///
/// For source `s`, selected chart axes `A`, and ceiling `d_s`, this emits
/// every pair `(s, offset)` whose offset is zero outside `A`, is oriented into
/// the supplied sector on `A`, and has total chart degree at most `d_s`.
/// Axis order controls only bounded enumeration work; the campaign boundary
/// canonicalizes and deduplicates exact requests before any symbolic
/// translation. The result is the common construction-neutral selected frame
/// and confers no evidence, rule, owner, terminal, or closure authority.
pub(crate) fn try_build_triangular_support_frame(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    sector: Mask,
    chart_axes: &[usize],
    source_degree_ceilings: &[usize],
    limits: CampaignLimits,
) -> Result<SelectedSourceFrame, TriangularSupportError> {
    if !completed.is_complete_ordinary() {
        return Err(TriangularSupportError::WrongSourceLayout {
            actual: completed.layout_name(),
        });
    }
    generator
        .validate_completed_scope(completed)
        .map_err(TriangularSupportError::SourceTranslation)?;
    let arity = generator.context().index_count();
    if sector.arity() != arity {
        return Err(TriangularSupportError::WrongSectorArity {
            expected: arity,
            actual: sector.arity(),
        });
    }
    check_limit(ARITY, arity, limits.max_request_arity)?;
    check_limit(ARITY, arity, limits.physical_frame.max_arity)?;

    let source_count = completed.source_row_count();
    if source_count == 0 {
        return Err(TriangularSupportError::Invariant {
            detail: "complete ordinary source chronology is empty",
        });
    }
    if source_degree_ceilings.len() != source_count {
        return Err(TriangularSupportError::WrongSourceCeilingCount {
            expected: source_count,
            actual: source_degree_ceilings.len(),
        });
    }
    validate_axes(chart_axes, arity)?;

    let mut request_count = 0usize;
    let mut maximum_degree = 0usize;
    for (source_ordinal, &degree) in source_degree_ceilings.iter().enumerate() {
        check_limit(DEGREE, degree, limits.physical_frame.max_degree)?;
        i64::try_from(degree).map_err(|_| TriangularSupportError::DegreeNotRepresentable {
            source_ordinal,
            degree,
        })?;
        maximum_degree = maximum_degree.max(degree);
        request_count = checked_add(
            SUBMITTED_REQUESTS,
            request_count,
            count_offsets(chart_axes.len(), degree)?,
        )?;
    }

    let distinct_offset_count = count_offsets(chart_axes.len(), maximum_degree)?;
    check_limit(
        OFFSETS,
        distinct_offset_count,
        limits.physical_frame.max_offsets,
    )?;
    let distinct_offset_coordinates =
        checked_mul(OFFSET_COORDINATES, distinct_offset_count, arity)?;
    check_limit(
        OFFSET_COORDINATES,
        distinct_offset_coordinates,
        limits.physical_frame.max_offset_coordinate_cells,
    )?;

    check_selected_translation_limit(
        SELECTED_TRANSLATION_REQUESTS,
        request_count,
        limits.translated_sources.max_requested_source_translations,
    )?;
    check_selected_translation_limit(
        SELECTED_TRANSLATION_SOURCES,
        request_count,
        limits.translated_sources.max_translated_sources,
    )?;
    check_selected_translation_limit(
        SELECTED_TRANSLATION_OFFSETS,
        distinct_offset_count,
        limits.translated_sources.max_requested_offsets,
    )?;
    check_physical_frame_limit(
        PHYSICAL_SOURCE_INSTANCES,
        request_count,
        limits.physical_frame.max_source_instances,
    )?;
    let csr_row_offsets = request_count.checked_add(1).ok_or_else(|| {
        TriangularSupportError::PhysicalFrame(PhysicalFrameError::ResourceCountOverflow {
            resource: PHYSICAL_CSR_ROW_OFFSETS,
        })
    })?;
    check_physical_frame_limit(
        PHYSICAL_CSR_ROW_OFFSETS,
        csr_row_offsets,
        limits.physical_frame.max_csr_row_offsets,
    )?;

    check_limit(
        SUBMITTED_REQUESTS,
        request_count,
        limits.max_submitted_requests,
    )?;
    check_limit(
        CANONICAL_REQUESTS,
        request_count,
        limits.max_canonical_candidate_requests,
    )?;
    check_limit(
        ACCUMULATED_REQUESTS,
        request_count,
        limits.max_accumulated_requests,
    )?;
    let request_coordinates = checked_mul(REQUEST_COORDINATES, request_count, arity)?;
    check_limit(
        REQUEST_COORDINATES,
        request_coordinates,
        limits.max_request_coordinate_cells,
    )?;

    let mut requests = try_vec(SUBMITTED_REQUESTS, request_count)?;
    enumerate_requests(
        &mut requests,
        &sector,
        chart_axes,
        source_degree_ceilings,
        arity,
    )?;
    if requests.len() != request_count {
        return Err(TriangularSupportError::Invariant {
            detail: "triangular enumeration disagrees with its exact preflight count",
        });
    }

    let accumulated = AccumulatedSourceRequests::try_new(arity, requests, limits)
        .map_err(TriangularSupportError::RequestAccumulation)?;
    if accumulated.len() != request_count {
        return Err(TriangularSupportError::Invariant {
            detail: "triangular enumeration emitted duplicate exact source requests",
        });
    }
    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            accumulated.requests().iter().cloned(),
            limits.translated_sources,
        )
        .map_err(TriangularSupportError::SourceTranslation)?;
    if selected.completed_source_row_count() != source_count
        || selected.requests() != accumulated.requests()
        || selected.sources().len() != request_count
        || selected
            .sources()
            .iter()
            .zip(selected.requests())
            .any(|(source, request)| {
                source.provenance().source_ordinal() != request.source_ordinal()
                    || source.provenance().offset() != request.offset()
            })
    {
        return Err(TriangularSupportError::Invariant {
            detail: "selected translation changed triangular request provenance",
        });
    }

    SelectedSourceFrame::try_new(selected, sector, limits.physical_frame)
        .map_err(TriangularSupportError::PhysicalFrame)
}
