use std::sync::Arc;

use super::super::super::condition::IdentityConditionSource;
use super::super::super::relation::IndexShift;
use super::super::model::{CompletedIbpSourceRows, ParametricIbpGenerator};
use super::error::TranslatedSourceError;
use super::limits::TranslatedSourceLimits;
use super::model::{
    IntegralShift, TranslatedSource, TranslatedSourceBatch, TranslatedSourceProvenance,
};

const DEFAULT_MAX_INTEGRAL_SHIFT_COMPONENTS: usize = 4_096;

impl IntegralShift {
    /// Construct a shift under the default family-sized component ceiling.
    pub fn try_new(values: impl IntoIterator<Item = i64>) -> Result<Self, TranslatedSourceError> {
        Self::try_new_with_component_limit(values, DEFAULT_MAX_INTEGRAL_SHIFT_COMPONENTS)
    }

    /// Construct a shift under an explicit component ceiling.
    pub fn try_new_with_component_limit(
        values: impl IntoIterator<Item = i64>,
        max_components: usize,
    ) -> Result<Self, TranslatedSourceError> {
        let mut retained = Vec::new();
        for value in values {
            let requested = checked_add("integral-shift components", retained.len(), 1)?;
            check_limit("integral-shift components", requested, max_components)?;
            retained
                .try_reserve(1)
                .map_err(|_| TranslatedSourceError::AllocationFailure {
                    resource: "integral-shift components",
                    requested,
                })?;
            retained.push(value);
        }
        if retained.is_empty() {
            return Err(TranslatedSourceError::EmptyIntegralShift);
        }
        Ok(Self(IndexShift::from_nonempty_preallocated(retained)))
    }
}

impl ParametricIbpGenerator<'_> {
    /// Translate every row of one complete sealed source batch at arbitrary
    /// integral-lattice offsets.
    ///
    /// Offsets are canonicalized before any Symbolica work: they are sorted
    /// lexicographically and exact duplicates are removed. Results are then
    /// ordered offset-major and source-chronology-minor. Scope and aggregate
    /// resource checks precede output construction.
    pub fn translate_completed_source_rows(
        &self,
        completed: &CompletedIbpSourceRows,
        offsets: impl IntoIterator<Item = IntegralShift>,
        limits: TranslatedSourceLimits,
    ) -> Result<TranslatedSourceBatch, TranslatedSourceError> {
        self.validate_completed_scope(completed)?;
        if completed.relations.is_empty() {
            return Err(TranslatedSourceError::EmptySourceRows);
        }
        let arity = self.context.index_count();
        let mut canonical_offsets = Vec::new();
        let mut requested_offsets = 0usize;
        for offset in offsets {
            requested_offsets =
                checked_add("requested translated-source offsets", requested_offsets, 1)?;
            check_limit(
                "requested translated-source offsets",
                requested_offsets,
                limits.max_requested_offsets,
            )?;
            if offset.len() != arity {
                return Err(TranslatedSourceError::WrongOffsetArity {
                    offset_ordinal: requested_offsets - 1,
                    expected: arity,
                    actual: offset.len(),
                });
            }
            canonical_offsets.try_reserve(1).map_err(|_| {
                TranslatedSourceError::AllocationFailure {
                    resource: "canonical translated-source offsets",
                    requested: requested_offsets,
                }
            })?;
            canonical_offsets.push(offset);
        }
        if canonical_offsets.is_empty() {
            return Err(TranslatedSourceError::EmptyOffsets);
        }
        canonical_offsets.sort_unstable();
        canonical_offsets.dedup();

        let source_rows = completed.relations.len();
        let offset_count = canonical_offsets.len();
        let nonzero_offset_count = canonical_offsets
            .iter()
            .filter(|offset| offset.values().iter().any(|value| *value != 0))
            .count();
        let translated_sources = checked_mul("translated source rows", source_rows, offset_count)?;
        check_limit(
            "translated source rows",
            translated_sources,
            limits.max_translated_sources,
        )?;

        let terms_per_offset = checked_sum(
            "translated source term entries",
            completed
                .relations
                .iter()
                .map(|relation| relation.terms().len()),
        )?;
        let translated_terms = checked_mul(
            "translated source term entries",
            terms_per_offset,
            offset_count,
        )?;
        check_limit(
            "translated source term entries",
            translated_terms,
            limits.max_translated_term_entries,
        )?;

        let conditions_per_offset = checked_sum(
            "translated source condition entries",
            completed
                .relations
                .iter()
                .map(|relation| relation.nonzero_conditions().len()),
        )?;
        let translated_conditions = checked_mul(
            "translated source condition entries",
            conditions_per_offset,
            offset_count,
        )?;
        check_limit(
            "translated source condition entries",
            translated_conditions,
            limits.max_translated_condition_entries,
        )?;

        let retained_condition_sources =
            retained_condition_source_entry_bound(completed, &canonical_offsets)?;
        check_limit(
            "translated-source retained condition-source entries",
            retained_condition_sources,
            limits.max_retained_condition_source_entries,
        )?;

        let coordinate_cells = retained_coordinate_cell_bound(
            completed,
            arity,
            offset_count,
            nonzero_offset_count,
            terms_per_offset,
            conditions_per_offset,
        )?;
        check_limit(
            "translated-source retained index-coordinate cells",
            coordinate_cells,
            limits.max_retained_index_coordinate_cells,
        )?;

        let mut translated = Vec::new();
        translated
            .try_reserve_exact(translated_sources)
            .map_err(|_| TranslatedSourceError::AllocationFailure {
                resource: "translated source rows",
                requested: translated_sources,
            })?;
        for (offset_ordinal, offset) in canonical_offsets.iter().enumerate() {
            for (source_ordinal, source) in completed.relations.iter().enumerate() {
                let source_row = source.row_id().clone();
                let relation = if offset.values().iter().all(|value| *value == 0) {
                    source
                        .cloned_with_limits(&self.context, limits.relation)
                        .map_err(|error| TranslatedSourceError::RelationTranslation {
                            offset_ordinal,
                            source_ordinal,
                            error,
                        })?
                } else {
                    source
                        .translated(
                            &self.context,
                            &offset.0,
                            source_row.clone(),
                            limits.relation,
                        )
                        .map_err(|error| TranslatedSourceError::RelationTranslation {
                            offset_ordinal,
                            source_ordinal,
                            error,
                        })?
                };
                translated.push(TranslatedSource {
                    relation,
                    provenance: TranslatedSourceProvenance {
                        source_ordinal,
                        source_row,
                        offset: offset.clone(),
                    },
                });
            }
        }

        Ok(TranslatedSourceBatch {
            family_fingerprint: self.source_scope.family_fingerprint.clone(),
            context_fingerprint: self.source_scope.context_fingerprint.clone(),
            source_row_count: source_rows,
            offsets: canonical_offsets,
            sources: translated,
        })
    }

    fn validate_completed_scope(
        &self,
        completed: &CompletedIbpSourceRows,
    ) -> Result<(), TranslatedSourceError> {
        if !same_owner_or_value(
            &self.source_scope.family_fingerprint,
            &completed.scope.family_fingerprint,
        ) {
            return Err(TranslatedSourceError::CompletedSourceFamilyMismatch);
        }
        if !same_owner_or_value(
            &self.source_scope.context_fingerprint,
            &completed.scope.context_fingerprint,
        ) {
            return Err(TranslatedSourceError::CompletedSourceContextMismatch);
        }
        Ok(())
    }
}

fn same_owner_or_value(left: &Arc<String>, right: &Arc<String>) -> bool {
    Arc::ptr_eq(left, right) || left == right
}

pub(super) fn retained_condition_source_entry_bound(
    completed: &CompletedIbpSourceRows,
    offsets: &[IntegralShift],
) -> Result<usize, TranslatedSourceError> {
    let mut total = 0usize;
    for offset in offsets {
        let is_zero = offset.values().iter().all(|value| *value == 0);
        for relation in &completed.relations {
            for condition in relation.nonzero_conditions() {
                total = add_condition_source_entries(total, condition.sources().len())?;
                if is_zero {
                    continue;
                }
                if !condition.sources().iter().any(|source| {
                    matches!(
                        source,
                        IdentityConditionSource::IndexTranslation { offset: existing }
                            if existing.as_ref() == offset.values()
                    )
                }) {
                    total = add_condition_source_entries(total, 1)?;
                }
                if !condition.sources().iter().any(|source| {
                    matches!(
                        source,
                        IdentityConditionSource::RelationTranslation {
                            source_row,
                            target_row,
                            offset: existing,
                        } if source_row == relation.row_id()
                            && target_row == relation.row_id()
                            && existing.as_ref() == offset.values()
                    )
                }) {
                    total = add_condition_source_entries(total, 1)?;
                }
                if !condition.sources().iter().any(|source| {
                    matches!(
                        source,
                        IdentityConditionSource::RelationConditionAttached { row }
                            if row == relation.row_id()
                    )
                }) {
                    total = add_condition_source_entries(total, 1)?;
                }
            }
            if !is_zero {
                let denominator_conditions = relation
                    .terms()
                    .values()
                    .filter(|coefficient| !coefficient.raw().denominator.is_constant())
                    .count();
                // Translation is an automorphism, so each nonconstant source
                // denominator stays nonconstant. Sealed relation construction
                // already retained its guard; rebuilding the translated term
                // can add at most its translated input-term provenance source.
                total = add_condition_source_entries(total, denominator_conditions)?;
            }
        }
    }
    Ok(total)
}

pub(super) fn add_condition_source_entries(
    total: usize,
    additional: usize,
) -> Result<usize, TranslatedSourceError> {
    checked_add(
        "translated-source retained condition-source entries",
        total,
        additional,
    )
}

fn retained_coordinate_cell_bound(
    completed: &CompletedIbpSourceRows,
    arity: usize,
    offset_count: usize,
    nonzero_offset_count: usize,
    terms_per_offset: usize,
    conditions_per_offset: usize,
) -> Result<usize, TranslatedSourceError> {
    let resource = "translated-source retained index-coordinate cells";
    // One canonical coordinate buffer per unique offset.
    let canonical_offsets = checked_mul(resource, arity, offset_count)?;
    // Every nonzero translation can retain both one newly summed sparse-key
    // buffer and one term-denominator provenance shift. Zero translations
    // share key buffers and add no provenance, so only nonzero offsets are
    // charged here.
    let translated_nonzero_terms = checked_mul(resource, terms_per_offset, nonzero_offset_count)?;
    let term_shift_cells = checked_mul(resource, arity, translated_nonzero_terms)?;
    let term_shifts_and_provenance = checked_mul(resource, term_shift_cells, 2)?;

    // Existing condition provenance is deep-cloned into every translated
    // result. Each condition can additionally retain both IndexTranslation
    // and RelationTranslation offsets; using two is a conservative bound when
    // one of those exact sources was already present and gets deduplicated.
    let existing_per_offset = checked_sum(
        resource,
        completed.relations.iter().flat_map(|relation| {
            relation
                .nonzero_conditions()
                .iter()
                .flat_map(|condition| condition.sources().iter())
                .map(identity_source_coordinate_cells)
        }),
    )?;
    let existing = checked_mul(resource, existing_per_offset, offset_count)?;
    let translated_nonzero_conditions =
        checked_mul(resource, conditions_per_offset, nonzero_offset_count)?;
    let added_per_condition = checked_mul(resource, arity, 2)?;
    let added = checked_mul(resource, translated_nonzero_conditions, added_per_condition)?;

    checked_add(
        resource,
        checked_add(resource, canonical_offsets, term_shifts_and_provenance)?,
        checked_add(resource, existing, added)?,
    )
}

fn identity_source_coordinate_cells(source: &IdentityConditionSource) -> usize {
    match source {
        IdentityConditionSource::RelationInputTermDenominator { shift, .. }
        | IdentityConditionSource::RelationCollectedTermDenominator { shift, .. } => shift.len(),
        IdentityConditionSource::RelationTranslation { offset, .. }
        | IdentityConditionSource::IndexTranslation { offset } => offset.len(),
        IdentityConditionSource::FamilyInputCoefficientDenominator { .. }
        | IdentityConditionSource::FamilyBasisDeterminantNumerator
        | IdentityConditionSource::RelationConditionAttached { .. }
        | IdentityConditionSource::RelationScaleFactorDenominator { .. } => 0,
    }
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, TranslatedSourceError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TranslatedSourceError> {
    left.checked_add(right)
        .ok_or(TranslatedSourceError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TranslatedSourceError> {
    left.checked_mul(right)
        .ok_or(TranslatedSourceError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TranslatedSourceError> {
    if requested > limit {
        Err(TranslatedSourceError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
