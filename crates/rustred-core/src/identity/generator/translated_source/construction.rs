use std::sync::Arc;
use symbolica::utils::resources;

use super::super::super::condition::IdentityConditionSource;
use super::super::super::relation::{
    IndexShift, ParametricRelation, ParametricRelationError, RelationLimits,
};
use super::super::model::{CompletedIbpSourceRows, ParametricIbpGenerator};
use super::error::TranslatedSourceError;
use super::limits::TranslatedSourceLimits;
use super::model::{
    IntegralShift, TranslatedSource, TranslatedSourceBatch, TranslatedSourceProvenance,
};

impl IntegralShift {
    /// Construct a shift; concrete family arity is authenticated at translation.
    pub fn try_new(values: impl IntoIterator<Item = i64>) -> Result<Self, TranslatedSourceError> {
        Self::collect(values, None)
    }

    /// Construct a shift under an explicit component ceiling.
    pub fn try_new_with_component_limit(
        values: impl IntoIterator<Item = i64>,
        max_components: usize,
    ) -> Result<Self, TranslatedSourceError> {
        Self::collect(values, Some(max_components))
    }

    fn collect(
        values: impl IntoIterator<Item = i64>,
        max_components: Option<usize>,
    ) -> Result<Self, TranslatedSourceError> {
        resources::checkpoint()?;
        let mut retained = Vec::new();
        for value in values {
            let requested = checked_add("integral-shift components", retained.len(), 1)?;
            if let Some(max_components) = max_components {
                check_limit("integral-shift components", requested, max_components)?;
            }
            allocation_for::<i64>(1, "integral-shift components")?;
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
        allocation_for::<i64>(retained.len(), "integral-shift components")?;
        let shift = Self(IndexShift::from_nonempty_preallocated(retained));
        resources::checkpoint()?;
        Ok(shift)
    }
}

impl ParametricIbpGenerator<'_> {
    /// Translate every row of one complete sealed source batch at arbitrary
    /// integral-lattice offsets.
    ///
    /// Offsets are canonicalized before any Symbolica work: they are sorted
    /// lexicographically and exact duplicates are removed. Results are then
    /// ordered offset-major and source-chronology-minor. Scope, representation,
    /// and any explicitly enabled aggregate diagnostics precede construction.
    pub fn translate_completed_source_rows(
        &self,
        completed: &CompletedIbpSourceRows,
        offsets: impl IntoIterator<Item = IntegralShift>,
        limits: TranslatedSourceLimits,
    ) -> Result<TranslatedSourceBatch, TranslatedSourceError> {
        resources::checkpoint()?;
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
            limits.check_limit(
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
            allocation_for::<IntegralShift>(1, "canonical translated-source offsets")?;
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
        resources::checkpoint()?;
        canonical_offsets.sort_unstable();
        canonical_offsets.dedup();
        resources::checkpoint()?;

        let source_rows = completed.relations.len();
        let offset_count = canonical_offsets.len();
        let translated_sources = checked_mul("translated source rows", source_rows, offset_count)?;
        limits.check_limit(
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
        limits.check_limit(
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
        limits.check_limit(
            "translated source condition entries",
            translated_conditions,
            limits.max_translated_condition_entries,
        )?;

        let retained_condition_sources =
            retained_condition_source_entry_bound(completed, &canonical_offsets)?;
        limits.check_limit(
            "translated-source retained condition-source entries",
            retained_condition_sources,
            limits.max_retained_condition_source_entries,
        )?;

        let coordinate_cells = retained_coordinate_cell_bound_for(
            arity,
            offset_count,
            canonical_offsets.iter().flat_map(|offset| {
                completed
                    .relations
                    .iter()
                    .map(move |relation| (relation, offset))
            }),
        )?;
        limits.check_limit(
            "translated-source retained index-coordinate cells",
            coordinate_cells,
            limits.max_retained_index_coordinate_cells,
        )?;

        let mut translated = Vec::new();
        allocation_for::<TranslatedSource>(translated_sources, "translated source rows")?;
        translated
            .try_reserve_exact(translated_sources)
            .map_err(|_| TranslatedSourceError::AllocationFailure {
                resource: "translated source rows",
                requested: translated_sources,
            })?;
        for (offset_ordinal, offset) in canonical_offsets.iter().enumerate() {
            for (source_ordinal, source) in completed.relations.iter().enumerate() {
                resources::checkpoint()?;
                translated.push(
                    translate_source(self, source, source_ordinal, offset, limits.relation)
                        .map_err(|error| TranslatedSourceError::RelationTranslation {
                            offset_ordinal,
                            source_ordinal,
                            error,
                        })?,
                );
                resources::checkpoint()?;
            }
        }

        resources::checkpoint()?;
        Ok(TranslatedSourceBatch {
            family_fingerprint: self.source_scope.family_fingerprint.clone(),
            context_fingerprint: self.source_scope.context_fingerprint.clone(),
            source_layout: completed.layout,
            source_row_count: source_rows,
            offsets: canonical_offsets,
            sources: translated,
        })
    }

    pub(crate) fn validate_completed_scope(
        &self,
        completed: &CompletedIbpSourceRows,
    ) -> Result<(), TranslatedSourceError> {
        resources::checkpoint()?;
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

pub(super) fn translate_source(
    generator: &ParametricIbpGenerator<'_>,
    source: &ParametricRelation,
    source_ordinal: usize,
    offset: &IntegralShift,
    limits: RelationLimits,
) -> Result<TranslatedSource, ParametricRelationError> {
    let source_row = source.row_id().clone();
    let relation = if offset.values().iter().all(|value| *value == 0) {
        source.cloned_with_limits(&generator.context, limits)?
    } else {
        source.translated(&generator.context, &offset.0, source_row.clone(), limits)?
    };
    Ok(TranslatedSource {
        relation,
        provenance: TranslatedSourceProvenance {
            source_ordinal,
            source_row,
            offset: offset.clone(),
        },
    })
}

fn same_owner_or_value(left: &Arc<String>, right: &Arc<String>) -> bool {
    Arc::ptr_eq(left, right) || left == right
}

pub(super) fn retained_condition_source_entry_bound(
    completed: &CompletedIbpSourceRows,
    offsets: &[IntegralShift],
) -> Result<usize, TranslatedSourceError> {
    retained_condition_source_entry_bound_for(offsets.iter().flat_map(|offset| {
        completed
            .relations
            .iter()
            .map(move |relation| (relation, offset))
    }))
}

pub(super) fn retained_condition_source_entry_bound_for<'a>(
    translations: impl IntoIterator<Item = (&'a ParametricRelation, &'a IntegralShift)>,
) -> Result<usize, TranslatedSourceError> {
    let mut total = 0usize;
    for (relation, offset) in translations {
        resources::checkpoint()?;
        let is_zero = offset.values().iter().all(|value| *value == 0);
        for condition in relation.nonzero_conditions() {
            resources::checkpoint()?;
            total = add_condition_source_entries(total, condition.sources().len())?;
            if is_zero {
                continue;
            }
            let mut has_index_translation = false;
            let mut has_relation_translation = false;
            let mut has_attachment = false;
            for source in condition.sources() {
                resources::checkpoint()?;
                match source {
                    IdentityConditionSource::IndexTranslation { offset: existing }
                        if existing.as_ref() == offset.values() =>
                    {
                        has_index_translation = true
                    }
                    IdentityConditionSource::RelationTranslation {
                        source_row,
                        target_row,
                        offset: existing,
                    } if source_row == relation.row_id()
                        && target_row == relation.row_id()
                        && existing.as_ref() == offset.values() =>
                    {
                        has_relation_translation = true
                    }
                    IdentityConditionSource::RelationConditionAttached { row }
                        if row == relation.row_id() =>
                    {
                        has_attachment = true
                    }
                    _ => {}
                }
            }
            if !has_index_translation {
                total = add_condition_source_entries(total, 1)?;
            }
            if !has_relation_translation {
                total = add_condition_source_entries(total, 1)?;
            }
            if !has_attachment {
                total = add_condition_source_entries(total, 1)?;
            }
        }
        if !is_zero {
            let denominator_conditions = relation.terms().values().try_fold(
                0usize,
                |count, coefficient| -> Result<usize, TranslatedSourceError> {
                    resources::checkpoint()?;
                    if coefficient.raw().denominator.is_constant() {
                        Ok(count)
                    } else {
                        checked_add("translated denominator conditions", count, 1)
                    }
                },
            )?;
            // Translation is an automorphism, so each nonconstant source
            // denominator stays nonconstant. Sealed relation construction
            // already retained its guard; rebuilding the translated term can
            // add at most its translated input-term provenance source.
            total = add_condition_source_entries(total, denominator_conditions)?;
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

pub(super) fn retained_coordinate_cell_bound_for<'a>(
    arity: usize,
    retained_offset_count: usize,
    translations: impl IntoIterator<Item = (&'a ParametricRelation, &'a IntegralShift)>,
) -> Result<usize, TranslatedSourceError> {
    let resource = "translated-source retained index-coordinate cells";
    let mut total = checked_mul(resource, arity, retained_offset_count)?;
    for (relation, offset) in translations {
        resources::checkpoint()?;
        total = checked_add(
            resource,
            total,
            checked_sum(
                resource,
                relation
                    .nonzero_conditions()
                    .iter()
                    .flat_map(|condition| condition.sources().iter())
                    .map(identity_source_coordinate_cells),
            )?,
        )?;
        if offset.values().iter().all(|value| *value == 0) {
            continue;
        }
        let term_shift_cells = checked_mul(resource, arity, relation.terms().len())?;
        total = checked_add(resource, total, checked_mul(resource, term_shift_cells, 2)?)?;
        let added_per_condition = checked_mul(resource, arity, 2)?;
        total = checked_add(
            resource,
            total,
            checked_mul(
                resource,
                relation.nonzero_conditions().len(),
                added_per_condition,
            )?,
        )?;
    }
    Ok(total)
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

pub(super) fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, TranslatedSourceError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TranslatedSourceError> {
    resources::checkpoint()?;
    left.checked_add(right)
        .ok_or(TranslatedSourceError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TranslatedSourceError> {
    resources::checkpoint()?;
    left.checked_mul(right)
        .ok_or(TranslatedSourceError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TranslatedSourceError> {
    resources::checkpoint()?;
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

pub(super) fn allocation_for<T>(
    count: usize,
    resource: &'static str,
) -> Result<(), TranslatedSourceError> {
    let bytes = checked_mul(resource, count, size_of::<T>())?;
    let bytes = u64::try_from(bytes)
        .map_err(|_| TranslatedSourceError::ResourceCountOverflow { resource })?;
    resources::check_allocation(bytes)?;
    Ok(())
}
