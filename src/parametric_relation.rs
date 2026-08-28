//! Typed sparse relations on the parametric integral lattice.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::algebra::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
};
use crate::identity::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition, RowId, insert_parametric_condition,
};

/// Complete arithmetic and identity-condition policy for relation operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationLimits {
    pub arithmetic: IndexedAlgebraLimits,
    pub identity_conditions: IdentityConditionLimits,
}

/// A checked displacement in one family's integral-index lattice.
///
/// Construction fallibly allocates the component buffer before moving that
/// buffer into shared storage. Cloning a cached shift therefore only bumps an
/// `Arc` count; it neither copies nor allocates another arity-sized buffer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShift(Arc<Vec<i64>>);

impl IndexShift {
    pub(crate) fn try_new(
        values: impl IntoIterator<Item = i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        let mut retained = Vec::new();
        try_reserve_relation_entries("index-shift components", &mut retained, expected_arity)?;
        let mut values = values.into_iter();
        while retained.len() < expected_arity {
            let Some(value) = values.next() else {
                return Err(ParametricRelationError::WrongArity {
                    expected: expected_arity,
                    actual: retained.len(),
                });
            };
            retained.push(value);
        }
        if values.next().is_some() {
            let actual = expected_arity.checked_add(1).ok_or(
                ParametricRelationError::ResourceCountOverflow {
                    resource: "index-shift components",
                },
            )?;
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual,
            });
        }
        Self::try_from_preallocated(retained, expected_arity)
    }

    /// Retain an allocation which its caller already acquired fallibly.
    pub(crate) fn try_from_preallocated(
        values: Vec<i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        if values.len() != expected_arity {
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual: values.len(),
            });
        }
        Ok(Self(Arc::new(values)))
    }

    pub fn values(&self) -> &[i64] {
        self.0.as_slice()
    }

    fn arity(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn checked_add(&self, other: &Self) -> Result<Self, ParametricRelationError> {
        if self.arity() != other.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity(),
                actual: other.arity(),
            });
        }
        let mut values = Vec::new();
        try_reserve_relation_entries("summed index-shift components", &mut values, self.arity())?;
        for (position, (&left, &right)) in self.0.iter().zip(other.0.iter()).enumerate() {
            values.push(
                left.checked_add(right)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Self::try_from_preallocated(values, self.arity())
    }
}

/// Constructs arity-authenticated shifts without repeating length checks at
/// every generator call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexSpace {
    arity: usize,
}

impl IndexSpace {
    pub(crate) fn try_new(arity: usize) -> Result<Self, ParametricRelationError> {
        if arity == 0 {
            Err(ParametricRelationError::EmptyIndexSpace)
        } else {
            Ok(Self { arity })
        }
    }

    /// Fallible zero-shift construction for resource-hardened callers.
    pub(crate) fn try_zero(self) -> Result<IndexShift, ParametricRelationError> {
        let mut values = Vec::new();
        try_reserve_relation_entries("zero index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        IndexShift::try_from_preallocated(values, self.arity)
    }

    pub(crate) fn unit(
        self,
        position: usize,
        direction: i64,
    ) -> Result<IndexShift, ParametricRelationError> {
        if position >= self.arity {
            return Err(ParametricRelationError::IndexOutOfRange {
                position,
                arity: self.arity,
            });
        }
        IndexShift::try_new(
            (0..self.arity).map(
                |component| {
                    if component == position { direction } else { 0 }
                },
            ),
            self.arity,
        )
    }
}

/// A raw parametric zero equation together with every condition inherited
/// before fraction-field cancellation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricRelation {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_id: RowId,
    arity: usize,
    terms: BTreeMap<IndexShift, IndexedCoefficient>,
    nonzero_conditions: Vec<ParametricNonZeroCondition>,
}

impl ParametricRelation {
    pub(crate) fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: RowId,
        context: &IndexedCoefficientContext,
    ) -> Self {
        Self {
            family_fingerprint: family_fingerprint.into(),
            context_fingerprint: context.fingerprint().into(),
            row_id,
            arity: context.index_count(),
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        }
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, IndexedCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.nonzero_conditions
    }

    pub(crate) fn add_nonzero_condition_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        mut condition: ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        context.validate_polynomial_with_limits(
            condition.polynomial(),
            limits.arithmetic.exact_algebra,
        )?;
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_source(
            IdentityConditionSource::RelationConditionAttached {
                row: self.row_id.clone(),
            },
            limits.identity_conditions,
        )?;
        insert_parametric_condition(
            &mut self.nonzero_conditions,
            condition,
            limits.identity_conditions,
        )?;
        Ok(())
    }

    pub(crate) fn add_term_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_term_in_place(context, shift, coefficient, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply one term insertion to an isolated relation snapshot.
    ///
    /// The transactional entry point clones before calling this helper because the
    /// input-denominator condition is discovered before coefficient collection.
    /// A later exact-arithmetic failure must not leave that condition committed to
    /// an otherwise unchanged relation.
    fn add_term_in_place(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(&shift)?;
        context.validate_with_limits(&coefficient, limits.arithmetic.exact_algebra)?;

        // Inspect the incoming fraction before testing whether its numerator
        // is zero.  This preserves a deliberately unnormalized `0 / p` as a
        // domain-bearing zero term.
        let denominator = context
            .denominator_condition_with_limits(&coefficient, limits.arithmetic.exact_algebra)?;
        let condition = ParametricNonZeroCondition::try_new_with_limits(
            context,
            denominator,
            [IdentityConditionSource::RelationInputTermDenominator {
                row: self.row_id.clone(),
                shift: shift.values().to_vec().into_boxed_slice(),
            }],
            limits.arithmetic.exact_algebra,
            limits.identity_conditions,
        )?;
        self.add_nonzero_condition_with_limits(context, condition, limits)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&shift) {
            let sum =
                context.add_with_limits(current, &coefficient, limits.arithmetic.exact_algebra)?;
            if sum.is_zero() {
                self.terms.remove(&shift);
            } else {
                let denominator = context
                    .denominator_condition_with_limits(&sum, limits.arithmetic.exact_algebra)?;
                let condition = ParametricNonZeroCondition::try_new_with_limits(
                    context,
                    denominator,
                    [IdentityConditionSource::RelationCollectedTermDenominator {
                        row: self.row_id.clone(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    }],
                    limits.arithmetic.exact_algebra,
                    limits.identity_conditions,
                )?;
                self.add_nonzero_condition_with_limits(context, condition, limits)?;
                self.terms.insert(shift, sum);
            }
        } else {
            self.terms.insert(shift, coefficient);
        }
        Ok(())
    }

    pub(crate) fn add_scaled_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        other: &Self,
        factor: &IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_scaled_in_place(context, other, factor, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply a scaled addition to an isolated relation snapshot.
    fn add_scaled_in_place(
        &mut self,
        context: &IndexedCoefficientContext,
        other: &Self,
        factor: &IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_compatible(other, context)?;
        context.validate_with_limits(factor, limits.arithmetic.exact_algebra)?;
        for condition in &other.nonzero_conditions {
            self.add_nonzero_condition_with_limits(context, condition.clone(), limits)?;
        }
        let factor_denominator =
            context.denominator_condition_with_limits(factor, limits.arithmetic.exact_algebra)?;
        let factor_condition = ParametricNonZeroCondition::try_new_with_limits(
            context,
            factor_denominator,
            [IdentityConditionSource::RelationScaleFactorDenominator {
                target_row: self.row_id.clone(),
                source_row: other.row_id.clone(),
            }],
            limits.arithmetic.exact_algebra,
            limits.identity_conditions,
        )?;
        self.add_nonzero_condition_with_limits(context, factor_condition, limits)?;
        for (shift, coefficient) in &other.terms {
            let scaled =
                context.mul_with_limits(coefficient, factor, limits.arithmetic.exact_algebra)?;
            self.add_term_in_place(context, shift.clone(), scaled, limits)?;
        }
        Ok(())
    }

    pub(crate) fn translated(
        &self,
        context: &IndexedCoefficientContext,
        translation: &IndexShift,
        row_id: RowId,
        limits: RelationLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(translation)?;
        let target_row = row_id.clone();
        let source_row = self.row_id.clone();
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        for condition in &self.nonzero_conditions {
            let mut translated = condition.translated(
                context,
                translation.values(),
                limits.arithmetic,
                limits.identity_conditions,
            )?;
            translated.add_source(
                IdentityConditionSource::RelationTranslation {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    offset: translation.values().to_vec().into_boxed_slice(),
                },
                limits.identity_conditions,
            )?;
            result.add_nonzero_condition_with_limits(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate(coefficient, translation.values(), limits.arithmetic)?;
            // `result` is an isolated, not-yet-published relation.  Use the
            // transactional helper directly so translating many terms does
            // not deep-clone every previously retained condition and source on
            // each insertion; any error still drops the complete local row.
            result.add_term_in_place(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result)
    }

    fn validate_context(
        &self,
        context: &IndexedCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        if self.context_fingerprint.as_ref() == context.fingerprint()
            && self.arity == context.index_count()
        {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongContext)
        }
    }

    fn validate_shift(&self, shift: &IndexShift) -> Result<(), ParametricRelationError> {
        if shift.arity() == self.arity {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: shift.arity(),
            })
        }
    }

    fn validate_compatible(
        &self,
        other: &Self,
        context: &IndexedCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        other.validate_context(context)?;
        if self.family_fingerprint == other.family_fingerprint {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongFamily)
        }
    }
}

fn relation_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRelationError> {
    left.checked_add(right)
        .ok_or(ParametricRelationError::ResourceCountOverflow { resource })
}

fn try_reserve_relation_entries<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ParametricRelationError> {
    let requested = relation_checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ParametricRelationError::AllocationFailure {
            resource,
            requested,
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRelationError {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        position: usize,
        arity: usize,
    },
    IndexOverflow {
        position: usize,
    },
    WrongContext,
    WrongFamily,
    UnsatisfiableDomain,
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    IdentityCondition(IdentityConditionError),
    Coefficient(IndexedAlgebraError),
}

impl fmt::Display for ParametricRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("an integral index space cannot be empty"),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "index arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { position, arity } => {
                write!(
                    formatter,
                    "index position {position} is outside arity {arity}"
                )
            }
            Self::IndexOverflow { position } => {
                write!(formatter, "integer index overflow at position {position}")
            }
            Self::WrongContext => formatter.write_str("relation and coefficient contexts differ"),
            Self::WrongFamily => formatter.write_str("relations belong to different families"),
            Self::UnsatisfiableDomain => formatter
                .write_str("relation domain contains an identically zero nonzero condition"),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::IdentityCondition(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricRelationError {}

impl From<IndexedAlgebraError> for ParametricRelationError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<IdentityConditionError> for ParametricRelationError {
    fn from(value: IdentityConditionError) -> Self {
        Self::IdentityCondition(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{CoefficientContext, ExactAlgebraLimits};

    fn actual_input_denominator_condition(
        scope: &str,
    ) -> (IndexedCoefficientContext, ParametricNonZeroCondition) {
        let base = CoefficientContext::new(["x"]);
        let context = IndexedCoefficientContext::try_new(&base, scope, 1).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: scope.into(),
            },
            &context,
        );
        relation
            .add_term_with_limits(
                &context,
                IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
                context.lift(&base.coefficient_fixture("1/x")).unwrap(),
                RelationLimits::default(),
            )
            .unwrap();
        assert_eq!(relation.nonzero_conditions().len(), 1);
        (context, relation.nonzero_conditions()[0].clone())
    }

    #[test]
    fn translation_moves_keys_and_coefficient_indices_together() {
        let base = CoefficientContext::new(["d"]);
        let context = IndexedCoefficientContext::try_new(&base, "relation-translate", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        relation
            .add_term_with_limits(
                &context,
                space.try_zero().unwrap(),
                context.index(0).unwrap(),
                RelationLimits::default(),
            )
            .unwrap();
        let translation = IndexShift::try_new([2, -1], 2).unwrap();
        let translated = relation
            .translated(
                &context,
                &translation,
                RowId::Derived {
                    label: "translated".into(),
                },
                RelationLimits::default(),
            )
            .unwrap();
        let (shift, coefficient) = translated.terms().first_key_value().unwrap();
        assert_eq!(shift.values(), &[2, -1]);
        let (coefficient, denominator_nonzero) = context
            .specialize(coefficient, &[3, 7], IndexedAlgebraLimits::default())
            .unwrap();
        assert_eq!(coefficient, base.integer(5));
        assert!(denominator_nonzero.is_none());
    }

    #[test]
    fn translation_composes_exactly() {
        let base = CoefficientContext::new(["d"]);
        let context = IndexedCoefficientContext::try_new(&base, "relation-compose", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term_with_limits(
                &context,
                space.unit(1, 1).unwrap(),
                context.index(0).unwrap(),
                RelationLimits::default(),
            )
            .unwrap();
        let s = IndexShift::try_new([1, -2], 2).unwrap();
        let t = IndexShift::try_new([-4, 3], 2).unwrap();
        let st = s.checked_add(&t).unwrap();
        let sequential = source
            .translated(
                &context,
                &s,
                RowId::Derived { label: "s".into() },
                RelationLimits::default(),
            )
            .unwrap()
            .translated(
                &context,
                &t,
                RowId::Derived { label: "st".into() },
                RelationLimits::default(),
            )
            .unwrap();
        let direct = source
            .translated(
                &context,
                &st,
                RowId::Derived { label: "st".into() },
                RelationLimits::default(),
            )
            .unwrap();
        assert_eq!(sequential.terms(), direct.terms());
        assert_eq!(sequential.nonzero_conditions(), direct.nonzero_conditions());
    }

    #[test]
    fn repeated_rational_term_merges_real_denominator_sources() {
        let base = CoefficientContext::new(["x"]);
        let context = IndexedCoefficientContext::try_new(&base, "condition-merge", 1).unwrap();
        let row_id = RowId::Derived {
            label: Arc::from("condition-source"),
        };
        let mut relation = ParametricRelation::new("family", row_id.clone(), &context);
        let shift = IndexSpace::try_new(1).unwrap().try_zero().unwrap();
        let reciprocal = context.lift(&base.coefficient_fixture("1/x")).unwrap();
        let input_source = IdentityConditionSource::RelationInputTermDenominator {
            row: row_id.clone(),
            shift: vec![0].into_boxed_slice(),
        };
        let collected_source = IdentityConditionSource::RelationCollectedTermDenominator {
            row: row_id.clone(),
            shift: vec![0].into_boxed_slice(),
        };
        relation
            .add_term_with_limits(
                &context,
                shift.clone(),
                reciprocal.clone(),
                RelationLimits::default(),
            )
            .unwrap();
        relation
            .add_term_with_limits(&context, shift, reciprocal, RelationLimits::default())
            .unwrap();

        assert_eq!(relation.nonzero_conditions().len(), 1);
        assert_eq!(
            relation.nonzero_conditions()[0].sources(),
            &std::collections::BTreeSet::from([
                input_source,
                collected_source,
                IdentityConditionSource::RelationConditionAttached { row: row_id },
            ])
        );
    }

    #[test]
    fn input_denominator_source_limit_is_enforced_by_real_term_insertion() {
        let base = CoefficientContext::new(["x"]);
        let context = IndexedCoefficientContext::try_new(&base, "condition-limit", 1).unwrap();
        let row_id = RowId::Derived {
            label: Arc::from("limited"),
        };
        let mut relation = ParametricRelation::new("family", row_id, &context);
        let limits = RelationLimits {
            identity_conditions: IdentityConditionLimits { max_sources: 1 },
            ..RelationLimits::default()
        };
        assert!(matches!(
            relation.add_term_with_limits(
                &context,
                IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
                context.lift(&base.coefficient_fixture("1/x")).unwrap(),
                limits,
            ),
            Err(ParametricRelationError::IdentityCondition(
                IdentityConditionError::ResourceLimit {
                    resource: "identity condition sources",
                    requested: 2,
                    limit: 1,
                }
            ))
        ));
        assert!(relation.terms().is_empty());
        assert!(relation.nonzero_conditions().is_empty());
    }

    #[test]
    fn real_relation_condition_source_limit_precedes_polynomial_translation() {
        let (context, condition) = actual_input_denominator_condition("translation-source-order");
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
                IdentityConditionLimits { max_sources: 2 },
            ),
            Err(IdentityConditionError::ResourceLimit {
                resource: "identity condition sources",
                requested: 3,
                limit: 2,
            })
        ));
    }

    #[test]
    fn real_relation_condition_index_arity_precedes_source_preflight() {
        let (context, condition) = actual_input_denominator_condition("translation-arity-order");
        assert!(matches!(
            condition.translated(
                &context,
                &[],
                IndexedAlgebraLimits::default(),
                IdentityConditionLimits { max_sources: 2 },
            ),
            Err(IdentityConditionError::Coefficient(
                IndexedAlgebraError::WrongIndexArity {
                    expected: 1,
                    actual: 0,
                }
            ))
        ));
    }

    #[test]
    fn fallible_index_construction_preserves_checked_semantics() {
        let space = IndexSpace::try_new(2).unwrap();
        assert_eq!(space.try_zero().unwrap().values(), &[0, 0]);
        assert_eq!(space.unit(1, -1).unwrap().values(), &[0, -1]);
        assert_eq!(
            IndexShift::try_new([2, -3], 2)
                .unwrap()
                .checked_add(&IndexShift::try_new([-1, 5], 2).unwrap())
                .unwrap()
                .values(),
            &[1, 2]
        );
        assert!(matches!(
            IndexShift::try_new([i64::MAX], 1)
                .unwrap()
                .checked_add(&IndexShift::try_new([1], 1).unwrap()),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
        // This is rejected by Vec's capacity arithmetic without attempting a
        // material allocation, exercising the checked internal allocation path.
        assert!(matches!(
            IndexSpace::try_new(usize::MAX).unwrap().try_zero(),
            Err(ParametricRelationError::AllocationFailure {
                resource: "zero index-shift components",
                requested: usize::MAX,
            })
        ));
    }

    #[test]
    fn cached_shift_clones_share_fallibly_built_storage() {
        let space = IndexSpace::try_new(3).unwrap();
        for shift in [
            space.try_zero().unwrap(),
            space.unit(1, 1).unwrap(),
            space.unit(2, -1).unwrap(),
        ] {
            let cloned = shift.clone();
            assert!(Arc::ptr_eq(&shift.0, &cloned.0));
            assert_eq!(shift.values(), cloned.values());
        }
    }

    #[test]
    fn overlong_index_shift_is_rejected_without_draining_the_iterator() {
        struct PanicIfPolledAfterFirstExtra {
            polls: usize,
        }

        impl Iterator for PanicIfPolledAfterFirstExtra {
            type Item = i64;

            fn next(&mut self) -> Option<Self::Item> {
                self.polls += 1;
                match self.polls {
                    1 => Some(7),
                    2 => Some(11),
                    _ => panic!("an overlong shift iterator was drained after arity was known"),
                }
            }
        }

        assert_eq!(
            IndexShift::try_new(PanicIfPolledAfterFirstExtra { polls: 0 }, 1),
            Err(ParametricRelationError::WrongArity {
                expected: 1,
                actual: 2,
            })
        );
    }
}
