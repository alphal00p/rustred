#[cfg(test)]
use std::cell::Cell;
use std::collections::BTreeSet;

use crate::algebra::{
    ExactAlgebraLimits, IndexedAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial,
};

use super::{
    error::IdentityConditionError, limits::IdentityConditionLimits, source::IdentityConditionSource,
};

#[cfg(test)]
std::thread_local! {
    static BORROWED_CONDITION_DEEP_CLONES: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

#[cfg(test)]
pub(in crate::identity) fn borrowed_condition_deep_clone_counts() -> (usize, usize) {
    BORROWED_CONDITION_DEEP_CLONES.with(Cell::get)
}

/// One authenticated polynomial condition over the index-extended field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroCondition {
    polynomial: IndexedPolynomial,
    sources: BTreeSet<IdentityConditionSource>,
}

impl ParametricNonZeroCondition {
    pub(in crate::identity) fn try_new_with_limits(
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

    /// Attach bounded provenance to a polynomial already produced by an
    /// authenticated indexed-coefficient operation.
    ///
    /// The relation builder still checks the polynomial's exact context
    /// fingerprint before accepting this value. Keeping that cheap check at
    /// insertion prevents a condition authenticated by a different indexed
    /// context from crossing the identity boundary without rescanning every
    /// polynomial term.
    pub(crate) fn from_authenticated_with_limits(
        polynomial: IndexedPolynomial,
        sources: impl IntoIterator<Item = IdentityConditionSource>,
        condition_limits: IdentityConditionLimits,
    ) -> Result<Self, IdentityConditionError> {
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

    pub(in crate::identity) fn translated(
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
            context.translate_polynomial_sealed(self.polynomial(), shift, arithmetic_limits)?;
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

    pub(in crate::identity) fn add_source(
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

    fn clone_polynomial_after_admission(&self) -> IndexedPolynomial {
        #[cfg(test)]
        BORROWED_CONDITION_DEEP_CLONES.with(|counts| {
            let (polynomials, sources) = counts.get();
            counts.set((polynomials.saturating_add(1), sources));
        });
        self.polynomial.clone()
    }

    fn clone_sources_after_admission(&self) -> BTreeSet<IdentityConditionSource> {
        #[cfg(test)]
        BORROWED_CONDITION_DEEP_CLONES.with(|counts| {
            let (polynomials, sources) = counts.get();
            counts.set((polynomials, sources.saturating_add(1)));
        });
        self.sources.clone()
    }
}

/// Copy a borrowed, already algebraically re-admitted condition into a target
/// relation. The complete prospective provenance union is checked before any
/// polynomial, set, or individual source is cloned.
pub(in crate::identity) fn insert_borrowed_parametric_condition(
    conditions: &mut Vec<ParametricNonZeroCondition>,
    condition: &ParametricNonZeroCondition,
    additional_source: IdentityConditionSource,
    limits: IdentityConditionLimits,
) -> Result<(), IdentityConditionError> {
    // Preserve owned insertion's error precedence: first admit the borrowed
    // condition plus its attachment source on their own, then admit the full
    // union with an existing target condition if one is present.
    let attached_condition_sources = condition
        .sources
        .len()
        .checked_add(usize::from(!condition.sources.contains(&additional_source)))
        .ok_or(IdentityConditionError::ResourceCountOverflow {
            resource: "identity condition sources",
        })?;
    check_source_limit(attached_condition_sources, limits)?;

    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        let borrowed_additions = condition
            .sources
            .iter()
            .filter(|source| !existing.sources.contains(*source))
            .count();
        let source_is_new = !existing.sources.contains(&additional_source)
            && !condition.sources.contains(&additional_source);
        let requested = existing
            .sources
            .len()
            .checked_add(borrowed_additions)
            .and_then(|count| count.checked_add(usize::from(source_is_new)))
            .ok_or(IdentityConditionError::ResourceCountOverflow {
                resource: "identity condition sources",
            })?;
        check_source_limit(requested, limits)?;

        for source in &condition.sources {
            if !existing.sources.contains(source) {
                existing.sources.insert(source.clone());
            }
        }
        existing.sources.insert(additional_source);
        return Ok(());
    }

    let polynomial = condition.clone_polynomial_after_admission();
    let mut sources = condition.clone_sources_after_admission();
    sources.insert(additional_source);
    conditions.push(ParametricNonZeroCondition {
        polynomial,
        sources,
    });
    Ok(())
}

pub(in crate::identity) fn insert_parametric_condition(
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
