//! Bounded sparse translation of explicitly selected completed source rows.

use super::super::model::{CompletedIbpSourceRows, ParametricIbpGenerator};
use super::construction::{
    check_limit, checked_add, checked_sum, retained_condition_source_entry_bound_for,
    retained_coordinate_cell_bound_for, translate_source,
};
use super::error::TranslatedSourceError;
use super::limits::TranslatedSourceLimits;
use super::model::{SelectedTranslatedSourceBatch, TranslatedSourceRequest};

const REQUESTED_TRANSLATIONS: &str = "requested selected source translations";
const CANONICAL_REQUESTS: &str = "canonical selected source translations";
const CANONICAL_OFFSETS: &str = "canonical selected translation offsets";
const TRANSLATED_SOURCES: &str = "translated source rows";
const TRANSLATED_TERMS: &str = "translated source term entries";
const TRANSLATED_CONDITIONS: &str = "translated source condition entries";
const RETAINED_CONDITION_SOURCES: &str = "translated-source retained condition-source entries";
const RETAINED_COORDINATE_CELLS: &str = "translated-source retained index-coordinate cells";

impl ParametricIbpGenerator<'_> {
    /// Translate only explicitly requested rows from one completed source
    /// batch.
    ///
    /// Requests may arrive in any order. Before symbolic work they are sorted
    /// offset-major and source-chronology-minor, then exact duplicate pairs
    /// are removed. The returned requests and sources are one-to-one in that
    /// canonical order. Every aggregate allocation and retained exact payload
    /// is bounded before the first relation is translated.
    pub fn translate_selected_completed_source_rows(
        &self,
        completed: &CompletedIbpSourceRows,
        requests: impl IntoIterator<Item = TranslatedSourceRequest>,
        limits: TranslatedSourceLimits,
    ) -> Result<SelectedTranslatedSourceBatch, TranslatedSourceError> {
        self.validate_completed_scope(completed)?;
        if completed.relations.is_empty() {
            return Err(TranslatedSourceError::EmptySourceRows);
        }

        let arity = self.context.index_count();
        let source_count = completed.relations.len();
        let mut canonical_requests = Vec::new();
        let mut requested_count = 0usize;
        for request in requests {
            let request_ordinal = requested_count;
            requested_count = checked_add(REQUESTED_TRANSLATIONS, requested_count, 1)?;
            check_limit(
                REQUESTED_TRANSLATIONS,
                requested_count,
                limits.max_requested_source_translations,
            )?;
            if request.offset().len() != arity {
                return Err(TranslatedSourceError::WrongRequestOffsetArity {
                    request_ordinal,
                    expected: arity,
                    actual: request.offset().len(),
                });
            }
            if request.source_ordinal() >= source_count {
                return Err(TranslatedSourceError::SourceOrdinalOutOfRange {
                    request_ordinal,
                    source_ordinal: request.source_ordinal(),
                    source_count,
                });
            }
            canonical_requests.try_reserve(1).map_err(|_| {
                TranslatedSourceError::AllocationFailure {
                    resource: CANONICAL_REQUESTS,
                    requested: requested_count,
                }
            })?;
            canonical_requests.push(request);
        }
        if canonical_requests.is_empty() {
            return Err(TranslatedSourceError::EmptySourceRequests);
        }
        canonical_requests.sort_unstable();
        canonical_requests.dedup();

        let translated_source_count = canonical_requests.len();
        check_limit(
            TRANSLATED_SOURCES,
            translated_source_count,
            limits.max_translated_sources,
        )?;
        let canonical_offset_count = count_canonical_offsets(&canonical_requests)?;
        check_limit(
            CANONICAL_OFFSETS,
            canonical_offset_count,
            limits.max_requested_offsets,
        )?;

        let translated_terms = checked_sum(
            TRANSLATED_TERMS,
            canonical_requests
                .iter()
                .map(|request| completed.relations[request.source_ordinal()].terms().len()),
        )?;
        check_limit(
            TRANSLATED_TERMS,
            translated_terms,
            limits.max_translated_term_entries,
        )?;
        let translated_conditions = checked_sum(
            TRANSLATED_CONDITIONS,
            canonical_requests.iter().map(|request| {
                completed.relations[request.source_ordinal()]
                    .nonzero_conditions()
                    .len()
            }),
        )?;
        check_limit(
            TRANSLATED_CONDITIONS,
            translated_conditions,
            limits.max_translated_condition_entries,
        )?;

        let retained_condition_sources =
            retained_condition_source_entry_bound_for(canonical_requests.iter().map(|request| {
                (
                    &completed.relations[request.source_ordinal()],
                    request.offset(),
                )
            }))?;
        check_limit(
            RETAINED_CONDITION_SOURCES,
            retained_condition_sources,
            limits.max_retained_condition_source_entries,
        )?;
        // Each retained request may own its incoming shift buffer even when
        // another source uses the same equal offset. Charging every request is
        // conservative and bounds the actual sparse-batch ownership model.
        let retained_coordinate_cells = retained_coordinate_cell_bound_for(
            arity,
            canonical_requests.len(),
            canonical_requests.iter().map(|request| {
                (
                    &completed.relations[request.source_ordinal()],
                    request.offset(),
                )
            }),
        )?;
        check_limit(
            RETAINED_COORDINATE_CELLS,
            retained_coordinate_cells,
            limits.max_retained_index_coordinate_cells,
        )?;

        let mut translated = Vec::new();
        translated
            .try_reserve_exact(translated_source_count)
            .map_err(|_| TranslatedSourceError::AllocationFailure {
                resource: TRANSLATED_SOURCES,
                requested: translated_source_count,
            })?;
        for (canonical_request_ordinal, request) in canonical_requests.iter().enumerate() {
            let source_ordinal = request.source_ordinal();
            translated.push(
                translate_source(
                    self,
                    &completed.relations[source_ordinal],
                    source_ordinal,
                    request.offset(),
                    limits.relation,
                )
                .map_err(|error| TranslatedSourceError::RequestTranslation {
                    canonical_request_ordinal,
                    source_ordinal,
                    error,
                })?,
            );
        }

        Ok(SelectedTranslatedSourceBatch {
            family_fingerprint: self.source_scope.family_fingerprint.clone(),
            context_fingerprint: self.source_scope.context_fingerprint.clone(),
            completed_source_row_count: source_count,
            requests: canonical_requests,
            sources: translated,
        })
    }
}

fn count_canonical_offsets(
    requests: &[TranslatedSourceRequest],
) -> Result<usize, TranslatedSourceError> {
    let mut count = 0usize;
    let mut previous = None;
    for request in requests {
        if previous.is_none_or(|offset| offset != request.offset()) {
            count = checked_add(CANONICAL_OFFSETS, count, 1)?;
            previous = Some(request.offset());
        }
    }
    Ok(count)
}
