use std::collections::BTreeMap;

use super::error::{add, limit, mul, reserve};
use super::{
    TangentConditionOrigin as Origin, TangentSourceCombination, TangentSourceCondition,
    TangentSourceError as Error, TangentSourcePlan,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator, TranslatedSourceLimits};

impl TangentSourcePlan {
    /// Regenerate complete original selected rows and multiply their weights.
    ///
    /// All native family power shifts are inherited from the ordinary source
    /// generator. No owner mask, fixed face, source-zero deletion, or target
    /// normalization is used here. An identity can legitimately have no useful
    /// pivot, or even an identically zero final row.
    pub fn materialize(
        &self,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        translated_limits: TranslatedSourceLimits,
    ) -> Result<TangentSourceCombination, Error> {
        let context = generator.context();
        if completed.family_fingerprint() != self.family_fingerprint()
            || completed.context_fingerprint() != context.fingerprint()
            || context.index_count() != self.arity
        {
            return Err(Error::ScopeMismatch);
        }
        if !completed.is_complete_ordinary() {
            return Err(Error::IncompleteOrdinarySources);
        }
        for contribution in &self.contributions {
            if completed.source_row_id(contribution.request.source_ordinal())
                != Some(&contribution.row_id)
            {
                return Err(Error::SourceChronologyMismatch);
            }
        }
        // Charge the full source envelope before translation or multiplication.
        let mut terms = 0;
        let mut source_conditions = 0;
        for contribution in &self.contributions {
            let source = completed
                .source_relation(contribution.request.source_ordinal())
                .ok_or(Error::SourceChronologyMismatch)?;
            terms = add("original source terms", terms, source.terms().len())?;
            source_conditions = add(
                "original source conditions",
                source_conditions,
                source.nonzero_conditions().len(),
            )?;
        }
        let conditions = add(
            "retained conditions",
            add("retained conditions", terms, source_conditions)?,
            self.contributions.len(),
        )?;
        limit(
            "retained conditions",
            conditions,
            self.limits.max_conditions,
        )?;
        limit(
            "prospective product terms",
            terms,
            self.limits.max_product_terms,
        )?;
        let operations = add(
            "exact product operations",
            mul("exact product operations", terms, 3)?,
            add(
                "exact product operations",
                source_conditions,
                self.contributions.len(),
            )?,
        )?;
        limit(
            "exact product operations",
            operations,
            self.limits.max_exact_operations,
        )?;
        limit(
            "product and guard shift cells",
            mul(
                "product and guard shift cells",
                mul("product and guard shift cells", terms, 2)?,
                self.arity,
            )?,
            self.limits.max_retained_coordinate_cells,
        )?;
        let sources = generator.translate_selected_completed_source_rows(
            completed,
            self.contributions.iter().map(|c| c.request.clone()),
            translated_limits,
        )?;
        if sources.family_fingerprint() != self.family_fingerprint()
            || sources.context_fingerprint() != context.fingerprint()
            || sources.sources().len() != self.contributions.len()
            || !sources.is_complete_ordinary()
        {
            return Err(Error::ScopeMismatch);
        }
        let mut weights = Vec::new();
        reserve(
            &mut weights,
            self.contributions.len(),
            "indexed tangent weights",
        )?;
        let mut retained = Vec::new();
        reserve(&mut retained, conditions, "tangent conditions")?;
        let mut product = BTreeMap::new();
        for (ordinal, (source, contribution)) in sources
            .sources()
            .iter()
            .zip(&self.contributions)
            .enumerate()
        {
            let provenance = source.provenance();
            if provenance.source_row() != &contribution.row_id
                || provenance.source_ordinal() != contribution.request.source_ordinal()
                || provenance.offset() != contribution.request.offset()
            {
                return Err(Error::SourceChronologyMismatch);
            }
            context
                .base()
                .validate_with_limits(&contribution.weight, self.limits.exact_algebra)?;
            let weight = context.lift(&contribution.weight)?;
            context.validate_with_limits(&weight, self.limits.exact_algebra)?;
            retained.push(TangentSourceCondition {
                polynomial: context
                    .denominator_condition_with_limits(&weight, self.limits.exact_algebra)?,
                origin: Origin::WeightDenominator { source: ordinal },
            });
            for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
                context.validate_polynomial_with_limits(
                    condition.polynomial(),
                    self.limits.exact_algebra,
                )?;
                if condition.polynomial().is_zero() {
                    return Err(Error::InvalidInput {
                        detail: "original source has an identically zero condition",
                    });
                }
                retained.push(TangentSourceCondition {
                    polynomial: condition.polynomial().clone(),
                    origin: Origin::OriginalCondition {
                        source: ordinal,
                        condition: condition_ordinal,
                    },
                });
            }
            for (shift, coefficient) in source.terms() {
                // Preserve conditions before every product and later cancellation.
                retained.push(TangentSourceCondition {
                    polynomial: context.denominator_condition_with_limits(
                        coefficient,
                        self.limits.exact_algebra,
                    )?,
                    origin: Origin::OriginalCoefficientDenominator {
                        source: ordinal,
                        shift: shift.clone(),
                    },
                });
                let value =
                    context.mul_with_limits(&weight, coefficient, self.limits.exact_algebra)?;
                let value = if let Some(previous) = product.remove(shift) {
                    context.add_with_limits(&previous, &value, self.limits.exact_algebra)?
                } else {
                    value
                };
                if !value.is_zero() {
                    product.insert(shift.clone(), value);
                }
            }
            weights.push(weight);
        }
        // Translation may combine source terms; the preflight is conservative.
        if retained.len() > conditions {
            return Err(Error::ResourceOverflow {
                resource: "translated condition envelope",
            });
        }
        Ok(TangentSourceCombination {
            sources,
            weights,
            product,
            conditions: retained,
        })
    }
}
