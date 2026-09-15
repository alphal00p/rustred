//! Bind adapter-row weights to the generator's original ordinary identities.
//!
//! Symbolica supplies every coefficient ratio, product and translation. The
//! adapter's LCM implementation is not duplicated: a proposed scalar is checked
//! against every physical coefficient of the complete original row.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{
    Coefficient, CoefficientPolynomial, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::family::IntegralFamily;
use crate::identity::{
    CompletedIbpSourceRows, IndexShift, ParametricIbpGenerator, ParametricRelation, RowId,
};
use crate::solver::{Case, SourceSystem, Term};

use super::certificate::{OriginalRowNormalization, OriginalSourceReplay};
use super::{error, SourcePortAuditError};

mod translation;

struct OriginalRow {
    ordinal: usize,
    scale: IndexedPolynomial,
}

/// Prepared once, immutable, and shared by all cold sector checks. Original
/// relation ownership retains its family/context identity and source conditions.
pub(super) struct OriginalSourceCorpus {
    family_fingerprint: std::sync::Arc<String>,
    context: IndexedCoefficientContext,
    completed: CompletedIbpSourceRows,
    rows: BTreeMap<RowId, OriginalRow>,
}

impl OriginalSourceCorpus {
    pub(super) fn try_new<const N: usize>(
        family: &IntegralFamily,
        system: &SourceSystem<N>,
        row_ids: &[RowId],
    ) -> Result<Self, SourcePortAuditError> {
        if family.external_count() != 0 || system.rows().len() != row_ids.len() {
            return Err(error(
                "original-source normalization requires a complete vacuum row map",
            ));
        }
        let generator = ParametricIbpGenerator::try_new(family).map_err(error)?;
        let context = generator.context().clone();
        let first_index = context.base().parameter_names().len();
        if context.index_count() != N
            || system.index_variables() != &std::array::from_fn(|axis| first_index + axis)
            || system.fixed().iter().any(Option::is_some)
        {
            return Err(error(
                "adapter index coordinates differ from the original generator context",
            ));
        }
        let batch = generator.prepare_ordinary_ibp().map_err(error)?;
        let generated = (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect();
        let completed = batch.complete(generated).map_err(error)?;
        let mut available: BTreeMap<_, _> = completed
            .relations()
            .iter()
            .enumerate()
            .map(|(ordinal, relation)| (relation.row_id().clone(), ordinal))
            .collect();
        if available.len() != row_ids.len() {
            return Err(error(
                "original-source normalization row count differs from the adapter",
            ));
        }
        let mut rows = BTreeMap::new();
        for (row_id, adapter) in row_ids.iter().zip(system.rows()) {
            let ordinal = available
                .remove(row_id)
                .ok_or_else(|| error("unknown or duplicate original ordinary RowId"))?;
            let scale = checked_scale(&context, &completed.relations()[ordinal], adapter)?;
            rows.insert(row_id.clone(), OriginalRow { ordinal, scale });
        }
        Ok(Self {
            family_fingerprint: family.fingerprint_owner(),
            context,
            completed,
            rows,
        })
    }

    pub(super) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(in crate::foundry::artifact::source_port) fn context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    pub(in crate::foundry::artifact::source_port) fn into_relations(
        self,
    ) -> Vec<ParametricRelation> {
        self.completed.into_relations()
    }

    /// Convert checked adapter weights to weights of original generator rows.
    /// Translate the normalization and source conditions BEFORE fixing target
    /// coordinates, exactly as for the original source request itself.
    pub(super) fn normalize<const N: usize>(
        &self,
        mut replay: OriginalSourceReplay<N>,
        case: &Case<N>,
    ) -> Result<OriginalSourceReplay<N>, SourcePortAuditError> {
        if replay.normalization != OriginalRowNormalization::NativeDenominatorClearedOrdinaryV1 {
            return Err(error(
                "ordinary replay normalization has already been converted",
            ));
        }
        if let Some(affine) = case.affine() {
            let first = self.context.base().parameter_names().len();
            if affine.index_variables() != &std::array::from_fn(|axis| first + axis) {
                return Err(error(
                    "original-source normalization uses a different affine index map",
                ));
            }
        }
        let fixed: Vec<_> = case
            .fixed()
            .iter()
            .enumerate()
            .filter_map(|(axis, value)| value.map(|value| (axis, i64::from(value))))
            .collect();
        let mut conditions = Vec::new();
        for contribution in &mut replay.contributions {
            let original = self
                .rows
                .get(&contribution.source_row)
                .ok_or_else(|| error("retained source request has no original generator RowId"))?;
            let scale = self
                .context
                .translate_polynomial_sealed(
                    &original.scale,
                    &contribution.offset,
                    Default::default(),
                )
                .map_err(error)?;
            let scale = self
                .context
                .specialize_fixed_polynomial_sealed(&scale, &fixed, Default::default())
                .map_err(error)?;
            let scale = self
                .context
                .coefficient_from_polynomial_sealed(&scale)
                .map_err(error)?;
            let weight = self
                .context
                .admit_native_result_with_limits(contribution.weight.clone(), Default::default())
                .map_err(error)?;
            let (weight, scale) = if let Some(affine) = case.affine() {
                // Preserve the incoming pole BEFORE chart restriction and
                // scale multiplication can cancel it. The chart is a native
                // coefficient service, never a replacement for integral keys.
                let denominator = affine
                    .restrict_equation(&weight.raw().denominator)
                    .map_err(error)?;
                if denominator.is_zero() {
                    return Err(error(
                        "original source weight denominator vanishes on the target case",
                    ));
                }
                if !denominator.is_constant() && !conditions.contains(&denominator) {
                    conditions.push(denominator);
                }
                let restrict = |value: &crate::algebra::IndexedCoefficient| {
                    let restricted = affine.restrict_coefficient(value.raw()).map_err(error)?;
                    self.context
                        .admit_native_result_with_limits(restricted, Default::default())
                        .map_err(error)
                };
                (restrict(&weight)?, restrict(&scale)?)
            } else {
                (weight, scale)
            };
            contribution.weight = self
                .context
                .mul(&weight, &scale)
                .map_err(error)?
                .raw()
                .clone();
            for condition in self.completed.relations()[original.ordinal].nonzero_conditions() {
                let restricted = self.condition_for_target(
                    condition.polynomial(),
                    &contribution.offset,
                    &fixed,
                )?;
                let restricted = match case.affine() {
                    Some(affine) => affine.restrict_equation(restricted.raw()).map_err(error)?,
                    None => restricted.raw().clone(),
                };
                if restricted.is_zero() {
                    return Err(error(
                        "original source condition vanishes on the target case",
                    ));
                }
                if !restricted.is_constant() && !conditions.contains(&restricted) {
                    conditions.push(restricted);
                }
            }
        }
        replay
            .contributions
            .retain(|contribution| !contribution.weight.is_zero());
        replay.source_conditions = conditions;
        replay.normalization = OriginalRowNormalization::OriginalGeneratorOrdinaryV1;
        Ok(replay)
    }

    fn condition_for_target(
        &self,
        condition: &IndexedPolynomial,
        offset: &[i64],
        fixed: &[(usize, i64)],
    ) -> Result<IndexedPolynomial, SourcePortAuditError> {
        let translated = self
            .context
            .translate_polynomial_sealed(condition, offset, Default::default())
            .map_err(error)?;
        let restricted = self
            .context
            .specialize_fixed_polynomial_sealed(&translated, fixed, Default::default())
            .map_err(error)?;
        if restricted.raw().is_zero() {
            return Err(error(
                "original source condition vanishes on the target case",
            ));
        }
        Ok(restricted)
    }
}

fn checked_scale<const N: usize>(
    context: &IndexedCoefficientContext,
    original: &ParametricRelation,
    adapter: &[Term<N, CoefficientPolynomial>],
) -> Result<IndexedPolynomial, SourcePortAuditError> {
    original.validate_context(context).map_err(error)?;
    if original.terms().len() != adapter.len() {
        return Err(error(
            "adapter normalization changed the original source support",
        ));
    }
    let template = context.one();
    let mut scale: Option<Coefficient> = None;
    let mut seen = BTreeSet::new();
    for term in adapter {
        if term
            .integral
            .powers()
            .iter()
            .any(|power| !power.is_symbolic())
            || term.coefficient.variables() != template.raw().numerator.variables()
        {
            return Err(error(
                "adapter normalization has incompatible powers or variable map",
            ));
        }
        let shift = IndexShift::try_new(
            term.integral
                .powers()
                .iter()
                .map(|power| i64::from(power.value())),
            N,
        )
        .map_err(error)?;
        if !seen.insert(shift.clone()) {
            return Err(error("adapter normalization repeats a source shift"));
        }
        let coefficient = original
            .terms()
            .get(&shift)
            .ok_or_else(|| error("adapter normalization changes a physical source shift"))?;
        if coefficient.is_zero() || term.coefficient.is_zero() {
            return Err(error("adapter normalization contains a zero source term"));
        }
        let adapted: Coefficient = term.coefficient.clone().into();
        let factor = scale.get_or_insert_with(|| &adapted / coefficient.raw());
        if !factor.denominator.is_one() || factor.is_zero() {
            return Err(error(
                "adapter normalization is not a nonzero polynomial multiplier",
            ));
        }
        if !(&(coefficient.raw() * &*factor) - &adapted).is_zero() {
            return Err(error(
                "adapter normalization fails complete original-row replay",
            ));
        }
    }
    let scale = scale.unwrap_or_else(|| template.raw().clone());
    context
        .admit_native_polynomial_result_with_limits(scale.numerator, Default::default())
        .map_err(error)
}

#[cfg(test)]
mod tests;
