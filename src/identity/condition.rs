//! Deterministic provenance for identity nonzero conditions.

use std::collections::BTreeSet;
use std::fmt;

use crate::algebra::{
    ExactAlgebraLimits, IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext,
    IndexedPolynomial,
};
use crate::family::CoefficientLocation;

use super::RowId;

fn write_joined<T: fmt::Display>(
    writer: &mut impl fmt::Write,
    values: &[T],
    separator: &str,
) -> fmt::Result {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            writer.write_str(separator)?;
        }
        write!(writer, "{value}")?;
    }
    Ok(())
}

/// One atomic reason why an identity polynomial must remain nonzero.
///
/// Sources are flat and refer directly to the real identity row type. Equal
/// polynomials therefore merge deterministically without an adapter row ID or
/// recursive provenance tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IdentityConditionSource {
    FamilyInputCoefficientDenominator {
        location: CoefficientLocation,
    },
    FamilyBasisDeterminantNumerator,
    ExplicitRelationCondition,
    RelationConditionAttached {
        row: RowId,
    },
    RelationInputTermDenominator {
        row: RowId,
        shift: Box<[i64]>,
    },
    RelationCollectedTermDenominator {
        row: RowId,
        shift: Box<[i64]>,
    },
    RelationScaleFactorDenominator {
        target_row: RowId,
        source_row: RowId,
    },
    RelationTranslation {
        source_row: RowId,
        target_row: RowId,
        offset: Box<[i64]>,
    },
    IndexTranslation {
        offset: Box<[i64]>,
    },
}

impl IdentityConditionSource {
    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing identity-condition source to String cannot fail");
        output
    }

    fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::FamilyInputCoefficientDenominator { location } => {
                writer.write_str("family-input-coefficient-denominator:")?;
                location.write_stable(writer)
            }
            Self::FamilyBasisDeterminantNumerator => {
                writer.write_str("family-basis-determinant-numerator")
            }
            Self::ExplicitRelationCondition => writer.write_str("explicit-relation-condition"),
            Self::RelationConditionAttached { row } => {
                writer.write_str("relation-condition-attached:")?;
                row.write_stable(writer)
            }
            Self::RelationInputTermDenominator { row, shift } => {
                writer.write_str("relation-input-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                writer.write_str("]")
            }
            Self::RelationCollectedTermDenominator { row, shift } => {
                writer.write_str("relation-collected-term-denominator:")?;
                row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, shift, ",")?;
                writer.write_str("]")
            }
            Self::RelationScaleFactorDenominator {
                target_row,
                source_row,
            } => {
                writer.write_str("relation-scale-factor-denominator:")?;
                target_row.write_stable(writer)?;
                writer.write_str(":")?;
                source_row.write_stable(writer)
            }
            Self::RelationTranslation {
                source_row,
                target_row,
                offset,
            } => {
                writer.write_str("relation-translation:")?;
                source_row.write_stable(writer)?;
                writer.write_str(":")?;
                target_row.write_stable(writer)?;
                writer.write_str(":[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
            }
            Self::IndexTranslation { offset } => {
                writer.write_str("index-translation:[")?;
                write_joined(writer, offset, ",")?;
                writer.write_str("]")
            }
        }
    }
}

/// Cardinality policy for one identity condition's deterministic source set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityConditionLimits {
    pub max_sources: usize,
}

impl Default for IdentityConditionLimits {
    fn default() -> Self {
        Self {
            max_sources: 65_536,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityConditionError {
    MissingSource,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Coefficient(IndexedAlgebraError),
}

impl fmt::Display for IdentityConditionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource => {
                formatter.write_str("an identity nonzero condition needs at least one typed source")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for IdentityConditionError {}

impl From<IndexedAlgebraError> for IdentityConditionError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}

/// One authenticated polynomial condition over the index-extended field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroCondition {
    polynomial: IndexedPolynomial,
    sources: BTreeSet<IdentityConditionSource>,
}

impl ParametricNonZeroCondition {
    pub fn try_new(
        context: &IndexedCoefficientContext,
        polynomial: IndexedPolynomial,
        sources: impl IntoIterator<Item = IdentityConditionSource>,
    ) -> Result<Self, IdentityConditionError> {
        Self::try_new_with_limits(
            context,
            polynomial,
            sources,
            ExactAlgebraLimits::default(),
            IdentityConditionLimits::default(),
        )
    }

    pub fn try_new_with_limits(
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

    pub fn try_with_source(
        mut self,
        source: IdentityConditionSource,
        limits: IdentityConditionLimits,
    ) -> Result<Self, IdentityConditionError> {
        self.add_source(source, limits)?;
        Ok(self)
    }

    pub fn translated(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    #[test]
    fn stable_string_pins_nested_row_sources() {
        let source = IdentityConditionSource::RelationTranslation {
            source_row: RowId::Derived {
                label: "a:b".into(),
            },
            target_row: RowId::OrdinaryIbp {
                contraction_momentum: 3,
                differentiated_loop: 2,
            },
            offset: vec![-1, 2].into_boxed_slice(),
        };
        assert_eq!(
            source.stable_string(),
            "relation-translation:derived:3:a:b:ordinary-ibp:3:2:[-1,2]"
        );
    }

    #[test]
    fn translation_source_limit_precedes_polynomial_translation() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = IndexedCoefficientContext::try_new(&base, "translation-source", 1).unwrap();
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = ParametricNonZeroCondition::try_new(
            &context,
            polynomial,
            [IdentityConditionSource::ExplicitRelationCondition],
        )
        .unwrap();
        let arithmetic_limits = IndexedAlgebraLimits {
            exact_algebra: ExactAlgebraLimits {
                max_polynomial_terms: 0,
                ..ExactAlgebraLimits::default()
            },
            ..IndexedAlgebraLimits::default()
        };
        assert!(matches!(
            condition.translated(
                &context,
                &[1],
                arithmetic_limits,
                IdentityConditionLimits { max_sources: 1 },
            ),
            Err(IdentityConditionError::ResourceLimit {
                resource: "identity condition sources",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn translation_index_arity_precedes_condition_source_preflight() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = IndexedCoefficientContext::try_new(&base, "condition-arity", 1).unwrap();
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = ParametricNonZeroCondition::try_new(
            &context,
            polynomial,
            [IdentityConditionSource::ExplicitRelationCondition],
        )
        .unwrap();
        let condition_limits = IdentityConditionLimits { max_sources: 1 };

        assert!(matches!(
            condition.translated(
                &context,
                &[],
                IndexedAlgebraLimits::default(),
                condition_limits,
            ),
            Err(IdentityConditionError::Coefficient(
                IndexedAlgebraError::WrongIndexArity {
                    expected: 1,
                    actual: 0,
                }
            ))
        ));
    }
}
