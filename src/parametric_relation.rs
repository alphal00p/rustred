//! Typed sparse relations on the parametric integral lattice.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::parametric_coefficient::{insert_parametric_condition, insert_specialized_condition};
use crate::{
    BasePolynomial, GuardOrigin, GuardRowId, GuardedParametricCoefficient,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    SpecializedNonZeroCondition,
};
use crate::{algebra::Coefficient, algebra::CoefficientContext};

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

/// Stable source identity of a generated relation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParametricRowId {
    OrdinaryIbp {
        /// Loops first, then external momenta.
        contraction_momentum: usize,
        differentiated_loop: usize,
    },
    LorentzInvariance {
        first_external: usize,
        second_external: usize,
    },
    Derived {
        label: Arc<str>,
    },
}

impl ParametricRowId {
    pub fn guard_identity(&self) -> GuardRowId {
        match self {
            Self::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            } => GuardRowId::OrdinaryIbp {
                contraction_momentum: *contraction_momentum,
                differentiated_loop: *differentiated_loop,
            },
            Self::LorentzInvariance {
                first_external,
                second_external,
            } => GuardRowId::LorentzInvariance {
                first_external: *first_external,
                second_external: *second_external,
            },
            Self::Derived { label } => GuardRowId::Derived {
                label: label.clone(),
            },
        }
    }

    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        self.guard_identity().stable_string()
    }
}

/// A raw parametric zero equation together with every condition inherited
/// before fraction-field cancellation.
#[derive(Clone, Debug)]
pub struct ParametricRelation {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_id: ParametricRowId,
    arity: usize,
    terms: BTreeMap<IndexShift, ParametricCoefficient>,
    // Compatibility polynomial-only view.  `guarded_nonzero` is canonical.
    nonzero: Vec<ParametricPolynomial>,
    guarded_nonzero: Vec<ParametricNonZeroCondition>,
}

// Preserve the pre-provenance meaning of relation equality.  Provenance is
// auditable through `guarded_nonzero_conditions`, but adapter history does not
// change the mathematical sparse relation or its exceptional polynomial set.
impl PartialEq for ParametricRelation {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.row_id == other.row_id
            && self.arity == other.arity
            && self.terms == other.terms
            && self.nonzero == other.nonzero
    }
}

impl Eq for ParametricRelation {}

impl ParametricRelation {
    pub fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: ParametricRowId,
        context: &ParametricCoefficientContext,
    ) -> Self {
        Self {
            family_fingerprint: family_fingerprint.into(),
            context_fingerprint: context.fingerprint().into(),
            row_id,
            arity: context.index_count(),
            terms: BTreeMap::new(),
            nonzero: Vec::new(),
            guarded_nonzero: Vec::new(),
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

    pub fn row_id(&self) -> &ParametricRowId {
        &self.row_id
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, ParametricCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricPolynomial] {
        &self.nonzero
    }

    pub fn guarded_nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.guarded_nonzero
    }

    /// Compare both mathematical relation content and complete guard history.
    /// Ordinary `PartialEq` intentionally preserves the legacy mathematical
    /// equality semantics and does not compare adapter-history atoms.
    pub fn has_identical_guard_provenance(&self, other: &Self) -> bool {
        self == other && self.guarded_nonzero == other.guarded_nonzero
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn add_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
    ) -> Result<(), ParametricRelationError> {
        self.add_nonzero_condition_with_limits(
            context,
            condition,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let condition = context.nonzero_condition_with_origins_and_limits(
            condition,
            [GuardOrigin::ExplicitRelationCondition],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, condition, limits)
    }

    pub fn add_guarded_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricNonZeroCondition,
    ) -> Result<(), ParametricRelationError> {
        self.add_guarded_nonzero_condition_with_limits(
            context,
            condition,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_guarded_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        mut condition: ParametricNonZeroCondition,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        if !context.contains_nonzero_condition(&condition) {
            return Err(ParametricRelationError::WrongContext);
        }
        context.validate_polynomial_with_limits(condition.polynomial(), limits.exact_algebra)?;
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_origin_with_limit(
            GuardOrigin::RelationConditionAttached {
                row: self.row_id.guard_identity(),
            },
            limits.max_guard_origins,
        )?;
        let is_new = !self
            .guarded_nonzero
            .iter()
            .any(|existing| existing.polynomial() == condition.polynomial());
        let polynomial = condition.polynomial().clone();
        insert_parametric_condition(
            &mut self.guarded_nonzero,
            condition,
            limits.max_guard_origins,
        )?;
        if is_new {
            self.nonzero.push(polynomial);
        }
        Ok(())
    }

    pub fn add_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_term_with_limits(
            context,
            shift,
            coefficient,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_term_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_term_in_place(context, shift, coefficient, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply one term insertion to an isolated relation snapshot.
    ///
    /// The public entry point clones before calling this helper because the
    /// input-denominator guard is discovered before coefficient collection.
    /// A later exact-arithmetic failure must not leave that guard committed to
    /// an otherwise unchanged relation.
    fn add_term_in_place(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(&shift)?;
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;

        // Inspect the incoming fraction before testing whether its numerator
        // is zero.  This preserves a deliberately unnormalized `0 / p` as a
        // domain-bearing zero term.
        let denominator =
            context.denominator_condition_with_limits(&coefficient, limits.exact_algebra)?;
        let condition = context.nonzero_condition_with_origins_and_limits(
            denominator,
            [GuardOrigin::RelationInputTermDenominator {
                row: self.row_id.guard_identity(),
                shift: shift.values().to_vec().into_boxed_slice(),
            }],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&shift) {
            let sum = context.add_with_limits(current, &coefficient, limits.exact_algebra)?;
            if sum.is_zero() {
                self.terms.remove(&shift);
            } else {
                let denominator =
                    context.denominator_condition_with_limits(&sum, limits.exact_algebra)?;
                let condition = context.nonzero_condition_with_origins_and_limits(
                    denominator,
                    [GuardOrigin::RelationCollectedTermDenominator {
                        row: self.row_id.guard_identity(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    }],
                    limits.exact_algebra,
                )?;
                self.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
                self.terms.insert(shift, sum);
            }
        } else {
            self.terms.insert(shift, coefficient);
        }
        Ok(())
    }

    /// Insert a coefficient returned by guarded division.  Its exceptional
    /// domain is retained even when the normalized value is zero.
    pub fn add_guarded_term_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: GuardedParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        for condition in coefficient.nonzero {
            staged.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        }
        staged.add_term_in_place(context, shift, coefficient.value, limits)?;
        *self = staged;
        Ok(())
    }

    pub fn add_guarded_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: GuardedParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_guarded_term_with_limits(
            context,
            shift,
            coefficient,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_scaled(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_scaled_with_limits(
            context,
            other,
            factor,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_scaled_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
        limits: ParametricArithmeticLimits,
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
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_compatible(other, context)?;
        context.validate_with_limits(factor, limits.exact_algebra)?;
        for condition in &other.guarded_nonzero {
            self.add_guarded_nonzero_condition_with_limits(context, condition.clone(), limits)?;
        }
        let factor_denominator =
            context.denominator_condition_with_limits(factor, limits.exact_algebra)?;
        let factor_condition = context.nonzero_condition_with_origins_and_limits(
            factor_denominator,
            [GuardOrigin::RelationScaleFactorDenominator {
                target_row: self.row_id.guard_identity(),
                source_row: other.row_id.guard_identity(),
            }],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, factor_condition, limits)?;
        for (shift, coefficient) in &other.terms {
            let scaled = context.mul_with_limits(coefficient, factor, limits.exact_algebra)?;
            self.add_term_in_place(context, shift.clone(), scaled, limits)?;
        }
        Ok(())
    }

    /// Add a relation with a scalar returned by guarded division.  This is
    /// the elimination-facing counterpart of [`Self::add_guarded_term`].
    pub fn add_scaled_guarded_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: GuardedParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.validate_compatible(other, context)?;
        context.validate_with_limits(&factor.value, limits.exact_algebra)?;
        for condition in factor.nonzero {
            staged.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        }
        staged.add_scaled_in_place(context, other, &factor.value, limits)?;
        *self = staged;
        Ok(())
    }

    pub fn add_scaled_guarded(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: GuardedParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_scaled_guarded_with_limits(
            context,
            other,
            factor,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn translated(
        &self,
        context: &ParametricCoefficientContext,
        translation: &IndexShift,
        row_id: ParametricRowId,
        limits: ParametricArithmeticLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(translation)?;
        let target_row = row_id.guard_identity();
        let source_row = self.row_id.guard_identity();
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        for condition in &self.guarded_nonzero {
            let mut translated =
                context.translate_nonzero_condition(condition, translation.values(), limits)?;
            translated.add_origin_with_limit(
                GuardOrigin::RelationTranslation {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    offset: translation.values().to_vec().into_boxed_slice(),
                },
                limits.max_guard_origins,
            )?;
            result.add_guarded_nonzero_condition_with_limits(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate(coefficient, translation.values(), limits)?;
            // `result` is an isolated, not-yet-published relation.  Use the
            // transactional helper directly so translating many terms does
            // not deep-clone every previously retained guard and origin on
            // each insertion; any error still drops the complete local row.
            result.add_term_in_place(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result)
    }

    pub fn specialize(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
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
            nonzero: Vec::new(),
            guarded_nonzero: Vec::new(),
        };
        for condition in &self.guarded_nonzero {
            specialize_borrowed_nonzero_condition(
                &mut result,
                context,
                condition,
                assignment,
                limits,
            )?;
        }
        for (shift, coefficient) in &self.terms {
            let specialized = context.specialize(coefficient, assignment, limits)?;
            for condition in specialized.guarded_nonzero_conditions() {
                result.add_guarded_nonzero(context.base(), condition.clone(), limits)?;
            }
            // A symbolic term may vanish at this assignment. In that case its
            // integral key is absent, so an overflowing assignment-plus-shift
            // cannot reject the specialization.
            if specialized.value.is_zero() {
                continue;
            }
            let key = ConcreteIntegralKey::checked_from_assignment(assignment, shift)?;
            result.add_term(context.base(), key, specialized.value, limits)?;
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

fn specialize_borrowed_nonzero_condition(
    result: &mut ConcreteRelation,
    context: &ParametricCoefficientContext,
    condition: &ParametricNonZeroCondition,
    assignment: &[i64],
    limits: ParametricArithmeticLimits,
) -> Result<(), ParametricRelationError> {
    let specialized = context.specialize_nonzero_condition(condition, assignment, limits)?;
    if specialized.polynomial().is_zero() {
        return Err(ParametricRelationError::UnsatisfiableDomain);
    }
    result.add_guarded_nonzero(context.base(), specialized, limits)
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

#[derive(Clone, Debug)]
pub struct ConcreteRelation {
    family_fingerprint: Arc<str>,
    row_id: ParametricRowId,
    arity: usize,
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    // Compatibility polynomial-only view. `guarded_nonzero` is canonical.
    nonzero: Vec<BasePolynomial>,
    guarded_nonzero: Vec<SpecializedNonZeroCondition>,
}

impl PartialEq for ConcreteRelation {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.row_id == other.row_id
            && self.arity == other.arity
            && self.terms == other.terms
            && self.nonzero == other.nonzero
    }
}

impl Eq for ConcreteRelation {}

impl ConcreteRelation {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn row_id(&self) -> &ParametricRowId {
        &self.row_id
    }

    pub fn terms(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[BasePolynomial] {
        &self.nonzero
    }

    pub fn guarded_nonzero_conditions(&self) -> &[SpecializedNonZeroCondition] {
        &self.guarded_nonzero
    }

    pub fn has_identical_guard_provenance(&self, other: &Self) -> bool {
        self == other && self.guarded_nonzero == other.guarded_nonzero
    }

    fn add_guarded_nonzero(
        &mut self,
        context: &CoefficientContext,
        mut condition: SpecializedNonZeroCondition,
        limits: ParametricArithmeticLimits,
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
        condition.add_origin_with_limit(
            GuardOrigin::RelationConditionAttached {
                row: self.row_id.guard_identity(),
            },
            limits.max_guard_origins,
        )?;
        let is_new = !self
            .guarded_nonzero
            .iter()
            .any(|existing| existing.polynomial() == condition.polynomial());
        let polynomial = condition.polynomial().clone();
        insert_specialized_condition(
            &mut self.guarded_nonzero,
            condition,
            limits.max_guard_origins,
        )?;
        if is_new {
            self.nonzero.push(polynomial);
        }
        Ok(())
    }

    fn add_term(
        &mut self,
        context: &CoefficientContext,
        key: ConcreteIntegralKey,
        coefficient: Coefficient,
        limits: ParametricArithmeticLimits,
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
                .try_add(current, &coefficient, limits.exact_algebra)
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
            Self::UnsatisfiableDomain => {
                formatter.write_str("relation domain contains an identically zero nonzero guard")
            }
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
            ParametricRowId::Derived {
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
                ParametricRowId::Derived {
                    label: "translated".into(),
                },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let concrete = translated
            .specialize(&context, &[3, 7], ParametricArithmeticLimits::default())
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
            ParametricRowId::Derived {
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
                ParametricRowId::Derived { label: "s".into() },
                ParametricArithmeticLimits::default(),
            )
            .unwrap()
            .translated(
                &context,
                &t,
                ParametricRowId::Derived { label: "st".into() },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let direct = source
            .translated(
                &context,
                &st,
                ParametricRowId::Derived { label: "st".into() },
                ParametricArithmeticLimits::default(),
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
            ParametricRowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term(&context, space.unit(0, 1).unwrap(), context.one())
            .unwrap();
        assert!(matches!(
            source.specialize(&context, &[i64::MAX], ParametricArithmeticLimits::default()),
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
            ParametricRowId::Derived {
                label: Arc::from("zero-before-key-overflow"),
            },
            &context,
        );
        relation
            .add_term(&context, space.unit(0, 1).unwrap(), coefficient)
            .unwrap();

        let specialized = relation
            .specialize(&context, &[i64::MAX], ParametricArithmeticLimits::default())
            .unwrap();
        assert!(specialized.terms().is_empty());
    }

    #[test]
    fn equal_guard_polynomials_merge_deterministic_origin_sets() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guard-merge", 1).unwrap();
        let row_id = ParametricRowId::Derived {
            label: Arc::from("guard-source"),
        };
        let mut relation = ParametricRelation::new("family", row_id.clone(), &context);
        let n = context.index(0).unwrap();
        let polynomial = context.numerator_condition(&n).unwrap();
        let first = context
            .nonzero_condition(
                polynomial.clone(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        let second = context
            .nonzero_condition(polynomial, GuardOrigin::GuardedDivisionDividendDenominator)
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, first)
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, second)
            .unwrap();

        assert_eq!(relation.nonzero_conditions().len(), 1);
        assert_eq!(relation.guarded_nonzero_conditions().len(), 1);
        assert_eq!(
            relation.guarded_nonzero_conditions()[0].origins(),
            &std::collections::BTreeSet::from([
                GuardOrigin::GuardedDivisionDividendDenominator,
                GuardOrigin::GuardedDivisionDivisorNumerator,
                GuardOrigin::RelationConditionAttached {
                    row: row_id.guard_identity(),
                },
            ])
        );
    }

    #[test]
    fn guarded_zero_division_term_keeps_its_domain() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "zero-div-term", 1).unwrap();
        let n = context.index(0).unwrap();
        let divided = context.checked_div_guarded(&context.zero(), &n).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("guarded-zero"),
            },
            &context,
        );
        relation
            .add_guarded_term(&context, IndexSpace::try_new(1).unwrap().zero(), divided)
            .unwrap();
        assert!(relation.terms().is_empty());
        assert_eq!(relation.guarded_nonzero_conditions().len(), 1);
        assert!(matches!(
            relation.specialize(&context, &[0], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));
        assert!(
            relation
                .specialize(&context, &[1], ParametricArithmeticLimits::default())
                .unwrap()
                .terms()
                .is_empty()
        );
    }

    #[test]
    fn custom_guard_origin_limit_is_enforced_at_relation_boundary() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guard-limit", 1).unwrap();
        let n = context.index(0).unwrap();
        let condition = context
            .nonzero_condition(
                context.numerator_condition(&n).unwrap(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("limited"),
            },
            &context,
        );
        let limits = ParametricArithmeticLimits {
            max_guard_origins: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            relation.add_guarded_nonzero_condition_with_limits(&context, condition, limits),
            Err(ParametricRelationError::Coefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "parametric guard origins",
                    requested: 2,
                    limit: 1,
                }
            ))
        ));
    }

    #[test]
    fn guarded_scaled_addition_retains_a_zero_factor_domain() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guarded-scale", 1).unwrap();
        let row_id = ParametricRowId::Derived {
            label: Arc::from("scale-row"),
        };
        let mut source = ParametricRelation::new("family", row_id.clone(), &context);
        source
            .add_term(
                &context,
                IndexSpace::try_new(1).unwrap().zero(),
                context.one(),
            )
            .unwrap();
        let factor = context
            .checked_div_guarded(&context.zero(), &context.index(0).unwrap())
            .unwrap();
        let mut target = ParametricRelation::new("family", row_id, &context);
        target
            .add_scaled_guarded(&context, &source, factor)
            .unwrap();

        assert!(target.terms().is_empty());
        assert_eq!(target.guarded_nonzero_conditions().len(), 1);
        assert!(matches!(
            target.specialize(&context, &[0], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
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
