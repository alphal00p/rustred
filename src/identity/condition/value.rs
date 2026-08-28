use std::collections::BTreeSet;

use crate::algebra::{
    ExactAlgebraLimits, IndexedAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial,
};

use super::{
    error::IdentityConditionError, limits::IdentityConditionLimits, source::IdentityConditionSource,
};

/// One authenticated polynomial condition over the index-extended field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroCondition {
    polynomial: IndexedPolynomial,
    sources: BTreeSet<IdentityConditionSource>,
}

impl ParametricNonZeroCondition {
    pub(crate) fn try_new_with_limits(
        context: &IndexedCoefficientContext,
        polynomial: IndexedPolynomial,
        sources: impl IntoIterator<Item = IdentityConditionSource>,
        algebra_limits: ExactAlgebraLimits,
        condition_limits: IdentityConditionLimits,
    ) -> Result<Self, IdentityConditionError> {
        context.validate_polynomial_with_limits(&polynomial, algebra_limits)?;
        let sources = collect_sources(sources, condition_limits)?;
        Ok(Self {
            polynomial,
            sources,
        })
    }

    pub fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }

    pub fn sources(&self) -> &BTreeSet<IdentityConditionSource> {
        &self.sources
    }

    pub(crate) fn translated(
        &self,
        context: &IndexedCoefficientContext,
        shift: &[i64],
        arithmetic_limits: IndexedAlgebraLimits,
        condition_limits: IdentityConditionLimits,
    ) -> Result<Self, IdentityConditionError> {
        context.validate_polynomial_context(&self.polynomial)?;
        context.validate_index_arity(shift)?;
        let already_has_translation = self.sources.iter().any(|source| {
            matches!(
                source,
                IdentityConditionSource::IndexTranslation { offset }
                    if offset.as_ref() == shift
            )
        });
        let additional = usize::from(!already_has_translation);
        check_source_limit(
            self.sources.len().checked_add(additional).ok_or(
                IdentityConditionError::ResourceCountOverflow {
                    resource: "identity condition sources",
                },
            )?,
            condition_limits,
        )?;
        let polynomial =
            context.translate_polynomial(self.polynomial(), shift, arithmetic_limits)?;
        let mut sources = self.sources.clone();
        if !already_has_translation {
            sources.insert(IdentityConditionSource::IndexTranslation {
                offset: shift.to_vec().into_boxed_slice(),
            });
        }
        Ok(Self {
            polynomial,
            sources,
        })
    }

    pub(crate) fn add_source(
        &mut self,
        source: IdentityConditionSource,
        limits: IdentityConditionLimits,
    ) -> Result<(), IdentityConditionError> {
        if !self.sources.contains(&source) {
            let requested = self.sources.len().checked_add(1).ok_or(
                IdentityConditionError::ResourceCountOverflow {
                    resource: "identity condition sources",
                },
            )?;
            check_source_limit(requested, limits)?;
            self.sources.insert(source);
        }
        Ok(())
    }

    fn merge_sources_from(
        &mut self,
        other: &Self,
        limits: IdentityConditionLimits,
    ) -> Result<(), IdentityConditionError> {
        debug_assert_eq!(self.polynomial, other.polynomial);
        let additional = other
            .sources
            .iter()
            .filter(|source| !self.sources.contains(*source))
            .count();
        let requested = self.sources.len().checked_add(additional).ok_or(
            IdentityConditionError::ResourceCountOverflow {
                resource: "identity condition sources",
            },
        )?;
        check_source_limit(requested, limits)?;
        self.sources.extend(other.sources.iter().cloned());
        Ok(())
    }
}

pub(crate) fn insert_parametric_condition(
    conditions: &mut Vec<ParametricNonZeroCondition>,
    condition: ParametricNonZeroCondition,
    limits: IdentityConditionLimits,
) -> Result<(), IdentityConditionError> {
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        existing.merge_sources_from(&condition, limits)
    } else {
        check_source_limit(condition.sources.len(), limits)?;
        conditions.push(condition);
        Ok(())
    }
}

fn collect_sources(
    sources: impl IntoIterator<Item = IdentityConditionSource>,
    limits: IdentityConditionLimits,
) -> Result<BTreeSet<IdentityConditionSource>, IdentityConditionError> {
    let mut collected = BTreeSet::new();
    for (position, source) in sources.into_iter().enumerate() {
        let requested =
            position
                .checked_add(1)
                .ok_or(IdentityConditionError::ResourceCountOverflow {
                    resource: "identity condition source inputs",
                })?;
        check_source_limit(requested, limits)?;
        collected.insert(source);
    }
    if collected.is_empty() {
        return Err(IdentityConditionError::MissingSource);
    }
    check_source_limit(collected.len(), limits)?;
    Ok(collected)
}

fn check_source_limit(
    requested: usize,
    limits: IdentityConditionLimits,
) -> Result<(), IdentityConditionError> {
    if requested > limits.max_sources {
        Err(IdentityConditionError::ResourceLimit {
            resource: "identity condition sources",
            requested,
            limit: limits.max_sources,
        })
    } else {
        Ok(())
    }
}
