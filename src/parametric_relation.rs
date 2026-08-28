//! Typed sparse relations on the parametric integral lattice.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::identity::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition, RowId, SpecializedNonZeroCondition, insert_parametric_condition,
    insert_specialized_condition, specialize_coefficient_with_condition,
};
use crate::parametric_coefficient::{
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricPolynomial,
};
use crate::{algebra::Coefficient, algebra::CoefficientContext};

/// Complete arithmetic and identity-condition policy for relation operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub identity_conditions: IdentityConditionLimits,
}

/// A checked displacement in one family's integral-index lattice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShift(Vec<i64>);

impl IndexShift {
    pub fn try_new(
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
    /// This is crate-private so public construction semantics remain intact.
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
        Ok(Self(values))
    }

    pub fn values(&self) -> &[i64] {
        &self.0
    }

    pub fn arity(&self) -> usize {
        self.0.len()
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, ParametricRelationError> {
        if self.arity() != other.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity(),
                actual: other.arity(),
            });
        }
        let mut values = Vec::new();
        try_reserve_relation_entries("summed index-shift components", &mut values, self.arity())?;
        for (position, (&left, &right)) in self.0.iter().zip(&other.0).enumerate() {
            values.push(
                left.checked_add(right)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Ok(Self(values))
    }
}

/// Constructs arity-authenticated shifts without repeating length checks at
/// every generator call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexSpace {
    arity: usize,
}

impl IndexSpace {
    pub fn try_new(arity: usize) -> Result<Self, ParametricRelationError> {
        if arity == 0 {
            Err(ParametricRelationError::EmptyIndexSpace)
        } else {
            Ok(Self { arity })
        }
    }

    pub fn arity(self) -> usize {
        self.arity
    }

    /// Fallible zero-shift construction for resource-hardened callers.
    pub fn try_zero(self) -> Result<IndexShift, ParametricRelationError> {
        let mut values = Vec::new();
        try_reserve_relation_entries("zero index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        Ok(IndexShift(values))
    }

    /// Compatibility wrapper. New bounded code should use [`Self::try_zero`].
    pub fn zero(self) -> IndexShift {
        self.try_zero()
            .expect("allocating an IndexSpace zero shift failed")
    }

    pub fn unit(
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
        let mut values = Vec::new();
        try_reserve_relation_entries("unit index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        values[position] = direction;
        Ok(IndexShift(values))
    }

    pub fn shift(
        self,
        values: impl IntoIterator<Item = i64>,
    ) -> Result<IndexShift, ParametricRelationError> {
        IndexShift::try_new(values, self.arity)
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
    terms: BTreeMap<IndexShift, ParametricCoefficient>,
    nonzero_conditions: Vec<ParametricNonZeroCondition>,
}

impl ParametricRelation {
    pub fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: RowId,
        context: &ParametricCoefficientContext,
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

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Exact authenticated `K(n)` identity used by this relation.
    ///
    /// Operator adapters use this to reject even empty relations from a
    /// foreign index scope; inspecting term coefficients alone would not be
    /// sufficient for a zero row.
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, ParametricCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.nonzero_conditions
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn add_explicit_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
    ) -> Result<(), ParametricRelationError> {
        self.add_explicit_nonzero_condition_with_limits(
            context,
            condition,
            RelationLimits::default(),
        )
    }

    pub fn add_explicit_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let condition = ParametricNonZeroCondition::try_new_with_limits(
            context,
            condition,
            [IdentityConditionSource::ExplicitRelationCondition],
            limits.arithmetic.exact_algebra,
            limits.identity_conditions,
        )?;
        self.add_nonzero_condition_with_limits(context, condition, limits)
    }

    pub fn add_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricNonZeroCondition,
    ) -> Result<(), ParametricRelationError> {
        self.add_nonzero_condition_with_limits(context, condition, RelationLimits::default())
    }

    pub fn add_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
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

    pub fn add_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_term_with_limits(context, shift, coefficient, RelationLimits::default())
    }

    pub fn add_term_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_term_in_place(context, shift, coefficient, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply one term insertion to an isolated relation snapshot.
    ///
    /// The public entry point clones before calling this helper because the
    /// input-denominator condition is discovered before coefficient collection.
    /// A later exact-arithmetic failure must not leave that condition committed to
    /// an otherwise unchanged relation.
    fn add_term_in_place(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
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

    pub fn add_scaled(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_scaled_with_limits(context, other, factor, RelationLimits::default())
    }

    pub fn add_scaled_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
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
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
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

    pub fn translated(
        &self,
        context: &ParametricCoefficientContext,
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

    pub fn specialize(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: RelationLimits,
    ) -> Result<ConcreteRelation, ParametricRelationError> {
        self.validate_context(context)?;
        if assignment.len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: assignment.len(),
            });
        }
        let mut result = ConcreteRelation {
            family_fingerprint: self.family_fingerprint.clone(),
            row_id: self.row_id.clone(),
            arity: self.arity,
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        };
        for condition in &self.nonzero_conditions {
            let specialized = match condition.specialized(
                context,
                assignment,
                limits.arithmetic,
                limits.identity_conditions,
            ) {
                Ok(specialized) => specialized,
                Err(IdentityConditionError::ZeroPolynomial) => {
                    return Err(ParametricRelationError::UnsatisfiableDomain);
                }
                Err(error) => return Err(error.into()),
            };
            result.add_nonzero_condition(context.base(), specialized, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let (value, denominator_condition) = specialize_coefficient_with_condition(
                context,
                coefficient,
                assignment,
                limits.arithmetic,
                limits.identity_conditions,
            )?;
            if let Some(condition) = denominator_condition {
                result.add_nonzero_condition(context.base(), condition, limits)?;
            }
            // A symbolic term may vanish at this assignment. In that case its
            // integral key is absent, so an overflowing assignment-plus-shift
            // cannot reject the specialization.
            if value.is_zero() {
                continue;
            }
            let key = ConcreteIntegralKey::checked_from_assignment(assignment, shift)?;
            result.add_term(context.base(), key, value, limits)?;
        }
        Ok(result)
    }

    fn validate_context(
        &self,
        context: &ParametricCoefficientContext,
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
        context: &ParametricCoefficientContext,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConcreteIntegralKey(Vec<i64>);

impl ConcreteIntegralKey {
    /// Construct an exact concrete integral key from unshifted integer powers.
    pub fn try_new(powers: impl IntoIterator<Item = i64>) -> Result<Self, ParametricRelationError> {
        let mut retained = Vec::new();
        for power in powers {
            try_reserve_relation_entries("concrete integral powers", &mut retained, 1)?;
            retained.push(power);
        }
        if retained.is_empty() {
            return Err(ParametricRelationError::EmptyIndexSpace);
        }
        Ok(Self(retained))
    }

    fn checked_from_assignment(
        assignment: &[i64],
        shift: &IndexShift,
    ) -> Result<Self, ParametricRelationError> {
        if assignment.len() != shift.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: shift.arity(),
                actual: assignment.len(),
            });
        }
        let mut powers = Vec::new();
        try_reserve_relation_entries("concrete integral powers", &mut powers, assignment.len())?;
        for (position, (&power, &offset)) in assignment.iter().zip(shift.values()).enumerate() {
            powers.push(
                power
                    .checked_add(offset)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Ok(Self(powers))
    }

    pub fn powers(&self) -> &[i64] {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcreteRelation {
    family_fingerprint: Arc<str>,
    row_id: RowId,
    arity: usize,
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    nonzero_conditions: Vec<SpecializedNonZeroCondition>,
}

impl ConcreteRelation {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn terms(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[SpecializedNonZeroCondition] {
        &self.nonzero_conditions
    }

    fn add_nonzero_condition(
        &mut self,
        context: &CoefficientContext,
        mut condition: SpecializedNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        if !context.contains(&condition.polynomial().raw().clone().into()) {
            return Err(ParametricRelationError::WrongContext);
        }
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
        insert_specialized_condition(
            &mut self.nonzero_conditions,
            condition,
            limits.identity_conditions,
        )?;
        Ok(())
    }

    fn add_term(
        &mut self,
        context: &CoefficientContext,
        key: ConcreteIntegralKey,
        coefficient: Coefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        if key.powers().len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: key.powers().len(),
            });
        }
        if !context.contains(&coefficient) {
            return Err(ParametricRelationError::WrongContext);
        }
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&key) {
            let sum = context
                .try_add(current, &coefficient, limits.arithmetic.exact_algebra)
                .map_err(crate::ParametricCoefficientError::from)?;
            if sum.is_zero() {
                self.terms.remove(&key);
            } else {
                self.terms.insert(key, sum);
            }
        } else {
            self.terms.insert(key, coefficient);
        }
        Ok(())
    }
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
    Coefficient(ParametricCoefficientError),
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

impl From<ParametricCoefficientError> for ParametricRelationError {
    fn from(value: ParametricCoefficientError) -> Self {
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

    #[test]
    fn translation_moves_keys_and_coefficient_indices_together() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "relation-translate", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        relation
            .add_term(&context, space.zero(), context.index(0).unwrap())
            .unwrap();
        let translation = space.shift([2, -1]).unwrap();
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
        let concrete = translated
            .specialize(&context, &[3, 7], RelationLimits::default())
            .unwrap();
        let (key, coefficient) = concrete.terms().first_key_value().unwrap();
        assert_eq!(key.powers(), &[5, 6]);
        assert_eq!(coefficient, &base.integer(5));
    }

    #[test]
    fn translation_composes_exactly() {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(&base, "relation-compose", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term(
                &context,
                space.unit(1, 1).unwrap(),
                context.index(0).unwrap(),
            )
            .unwrap();
        let s = space.shift([1, -2]).unwrap();
        let t = space.shift([-4, 3]).unwrap();
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
    fn specialization_checks_key_overflow() {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(&base, "relation-overflow", 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term(&context, space.unit(0, 1).unwrap(), context.one())
            .unwrap();
        assert!(matches!(
            source.specialize(&context, &[i64::MAX], RelationLimits::default()),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
    }

    #[test]
    fn specialization_discards_zero_coefficient_before_key_arithmetic() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "zero-before-key-overflow", 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.integer(i64::MAX))
            .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: Arc::from("zero-before-key-overflow"),
            },
            &context,
        );
        relation
            .add_term(&context, space.unit(0, 1).unwrap(), coefficient)
            .unwrap();

        let specialized = relation
            .specialize(&context, &[i64::MAX], RelationLimits::default())
            .unwrap();
        assert!(specialized.terms().is_empty());
    }

    #[test]
    fn specialization_reports_an_unsatisfiable_zero_condition() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "unsatisfiable-specialization", 1)
                .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: Arc::from("unsatisfiable-specialization"),
            },
            &context,
        );
        let index = context.index(0).unwrap();
        relation
            .add_explicit_nonzero_condition(&context, context.numerator_condition(&index).unwrap())
            .unwrap();

        assert!(matches!(
            relation.specialize(&context, &[0], RelationLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));
    }

    #[test]
    fn equal_condition_polynomials_merge_deterministic_source_sets() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "condition-merge", 1).unwrap();
        let row_id = RowId::Derived {
            label: Arc::from("condition-source"),
        };
        let mut relation = ParametricRelation::new("family", row_id.clone(), &context);
        let n = context.index(0).unwrap();
        let polynomial = context.numerator_condition(&n).unwrap();
        let first = ParametricNonZeroCondition::try_new(
            &context,
            polynomial.clone(),
            [IdentityConditionSource::ExplicitRelationCondition],
        )
        .unwrap();
        let second = ParametricNonZeroCondition::try_new(
            &context,
            polynomial,
            [IdentityConditionSource::IndexTranslation {
                offset: vec![1].into_boxed_slice(),
            }],
        )
        .unwrap();
        relation.add_nonzero_condition(&context, first).unwrap();
        relation.add_nonzero_condition(&context, second).unwrap();

        assert_eq!(relation.nonzero_conditions().len(), 1);
        assert_eq!(
            relation.nonzero_conditions()[0].sources(),
            &std::collections::BTreeSet::from([
                IdentityConditionSource::ExplicitRelationCondition,
                IdentityConditionSource::RelationConditionAttached { row: row_id },
                IdentityConditionSource::IndexTranslation {
                    offset: vec![1].into_boxed_slice(),
                },
            ])
        );
    }

    #[test]
    fn custom_condition_source_limit_is_enforced_at_relation_boundary() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "condition-limit", 1).unwrap();
        let n = context.index(0).unwrap();
        let condition = ParametricNonZeroCondition::try_new(
            &context,
            context.numerator_condition(&n).unwrap(),
            [IdentityConditionSource::ExplicitRelationCondition],
        )
        .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            RowId::Derived {
                label: Arc::from("limited"),
            },
            &context,
        );
        let limits = RelationLimits {
            identity_conditions: IdentityConditionLimits { max_sources: 1 },
            ..RelationLimits::default()
        };
        assert!(matches!(
            relation.add_nonzero_condition_with_limits(&context, condition, limits),
            Err(ParametricRelationError::IdentityCondition(
                IdentityConditionError::ResourceLimit {
                    resource: "identity condition sources",
                    requested: 2,
                    limit: 1,
                }
            ))
        ));
    }

    #[test]
    fn fallible_index_and_integral_key_construction_preserves_checked_semantics() {
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
        assert_eq!(
            ConcreteIntegralKey::try_new([3, -2]).unwrap().powers(),
            &[3, -2]
        );

        // This is rejected by Vec's capacity arithmetic without attempting a
        // material allocation, exercising the public allocation error path.
        assert!(matches!(
            IndexSpace::try_new(usize::MAX).unwrap().try_zero(),
            Err(ParametricRelationError::AllocationFailure {
                resource: "zero index-shift components",
                requested: usize::MAX,
            })
        ));
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
