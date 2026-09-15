//! Join normalized original-source weights without changing their identities.
//!
//! Only weights are restricted to the target face. The retained completed
//! ordinary batch owns every full source row, its scope and inherited guards.

use std::collections::{BTreeMap, btree_map::Entry};

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::cell::{FixedIndexRestriction, RuleCellLimits, SourceViewBatch};
use crate::identity::{
    IntegralShift, ParametricIbpGenerator, RowId, TranslatedSourceLimits, TranslatedSourceRequest,
};

use super::{OriginalRowNormalization, OriginalSourceCorpus, OriginalSourceReplay};
use super::{SourcePortAuditError, error};

/// Raw, unspecialized original identities and their fixed-face weights.
/// This is preparation only, not a rule, exact replay witness or owner.
pub(in crate::foundry::artifact::source_port) struct TranslatedOriginalSources {
    pub(in crate::foundry::artifact::source_port) sources: SourceViewBatch,
    /// (Selected-row ordinal, actual translated relation RowId, weight).
    pub(in crate::foundry::artifact::source_port) contributions:
        Vec<(usize, RowId, IndexedCoefficient)>,
    /// Obligations from every incoming weight, before specialization/cancellation.
    /// The caller must also retain replay.source_conditions and checked-rule guards.
    pub(in crate::foundry::artifact::source_port) weight_conditions: Vec<IndexedPolynomial>,
}

impl OriginalSourceCorpus {
    pub(in crate::foundry::artifact::source_port) fn translate_normalized<const N: usize>(
        &self,
        generator: &ParametricIbpGenerator<'_>,
        replay: &OriginalSourceReplay<N>,
        fixed: &[FixedIndexRestriction],
        translation_limits: TranslatedSourceLimits,
        cell_limits: RuleCellLimits,
    ) -> Result<TranslatedOriginalSources, SourcePortAuditError> {
        if replay.normalization != OriginalRowNormalization::OriginalGeneratorOrdinaryV1 {
            return Err(error(
                "selected original sources require normalized generator weights",
            ));
        }
        if self.context.index_count() != N
            || generator.context().fingerprint() != self.context.fingerprint()
        {
            return Err(error(
                "selected original sources have an incompatible index context",
            ));
        }
        if replay.contributions.len() > translation_limits.max_requested_source_translations
            || fixed.len() > cell_limits.max_fixed_restrictions
        {
            return Err(error(
                "selected original-source preparation exceeds its input budget",
            ));
        }
        let fixed: Vec<_> = fixed
            .iter()
            .map(|item| (item.position(), item.value()))
            .collect();
        let mut joined: BTreeMap<TranslatedSourceRequest, IndexedCoefficient> = BTreeMap::new();
        let mut weight_conditions = Vec::new();
        for contribution in &replay.contributions {
            let original = self
                .rows
                .get(&contribution.source_row)
                .ok_or_else(|| error("selected source request has no original generator RowId"))?;
            let request = TranslatedSourceRequest::new(
                original.ordinal,
                IntegralShift::try_new(contribution.offset).map_err(error)?,
            );
            let weight = self
                .context
                .admit_native_result_with_limits(
                    contribution.weight.clone(),
                    translation_limits.relation.arithmetic.exact_algebra,
                )
                .map_err(error)?;
            // Preserve the denominator BEFORE native normalization and joining.
            // In particular, opposite weights do not make a pole admissible.
            let (weight, denominator) = if fixed.is_empty() {
                let denominator = self
                    .context
                    .denominator_condition_from_bound(
                        self.context.bind_sealed(&weight).map_err(error)?,
                    )
                    .map_err(error)?;
                (weight, denominator)
            } else {
                self.context
                    .specialize_fixed_indices_sealed(
                        &weight,
                        &fixed,
                        translation_limits.relation.arithmetic,
                    )
                    .map_err(error)?
            };
            if !denominator.raw().is_constant() && !weight_conditions.contains(&denominator) {
                if weight_conditions.len() >= cell_limits.max_guards {
                    return Err(error(
                        "selected original-source weight guards exceed their budget",
                    ));
                }
                weight_conditions.push(denominator);
            }
            match joined.entry(request) {
                Entry::Vacant(entry) => {
                    entry.insert(weight);
                }
                Entry::Occupied(mut entry) => {
                    let sum = self
                        .context
                        .add_bound_with_limits(
                            self.context.bind_sealed(entry.get()).map_err(error)?,
                            self.context.bind_sealed(&weight).map_err(error)?,
                            translation_limits.relation.arithmetic.exact_algebra,
                        )
                        .map_err(error)?;
                    entry.insert(sum);
                }
            }
        }
        joined.retain(|_, weight| !weight.is_zero());
        if joined.is_empty() {
            return Err(error(
                "selected original-source support vanishes after exact joining",
            ));
        }
        // The existing generator validates the complete family/source scope,
        // budgets all translations and preserves offset-major native chronology.
        let selected = generator
            .translate_selected_completed_source_rows(
                &self.completed,
                joined.keys().cloned(),
                translation_limits,
            )
            .map_err(error)?;
        let weights = selected
            .requests()
            .iter()
            .map(|request| {
                joined
                    .remove(request)
                    .ok_or_else(|| error("selected original-source request mismatch"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !joined.is_empty() {
            return Err(error(
                "selected original-source translation omitted a joined request",
            ));
        }
        let ordinals: Vec<_> = (0..selected.len()).collect();
        let sources =
            SourceViewBatch::try_select(selected.into_translated_batch(), &ordinals, cell_limits)
                .map_err(error)?;
        let contributions = sources
            .relations()
            .iter()
            .zip(weights)
            .enumerate()
            .map(|(ordinal, (relation, weight))| (ordinal, relation.row_id().clone(), weight))
            .collect();
        Ok(TranslatedOriginalSources {
            sources,
            contributions,
            weight_conditions,
        })
    }
}

#[cfg(test)]
mod tests;
