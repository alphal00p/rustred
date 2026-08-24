//! Exact noncommutative index-shift operators with LiteRed `A`/`B` action.
//!
//! LiteRed's `SmallCircle` words act from left to right.  At lattice index
//! `n`, `A_i` contributes the current index and then raises it, whereas `B_i`
//! lowers it without a prefactor:
//!
//! ```text
//! A_i J(n) = n_i J(n + e_i)
//! B_i J(n) =     J(n - e_i)
//! ```
//!
//! Consequently `A_i B_i` acts as `n_i`, while `B_i A_i` acts as
//! `n_i - 1`.  Words are therefore retained explicitly.  A second primitive
//! monomial, `Shift(delta)`, acts as `J(n) -> J(n+delta)` with unit prefactor.
//! Sparse relations convert bijectively to these primitive shifts, avoiding
//! the spurious `n_i != 0` chart exclusions that division by an `A_i`
//! prefactor would introduce.  Arbitrary ordered words normalize through
//! their exact action to primitive shifts.
//!
//! This is deliberately named an **index-shift operator** layer, not
//! LiteRed's completed `ToAB` polynomial form.  LiteRed's `toab` additionally
//! substitutes every free `n_i` in scalar coefficients by the composite
//! `A_i ∘ B_i`, chooses a common shift, and factors lowering operators on
//! the left.  Coefficients here remain authenticated elements of `K(n)`.
//! A future s-basis polynomial layer must perform that additional elimination
//! of explicit index variables; callers must not serialize this intermediate
//! as LiteRed `ABIBP`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::parametric_coefficient::insert_parametric_condition;
use crate::{
    ConcreteRelation, GuardOrigin, GuardedParametricCoefficient, IndexShift,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    ParametricRelation, ParametricRelationError, ParametricRowId,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

/// One LiteRed shift-operator kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IndexShiftOperatorKind {
    /// `A_i J(n) = n_i J(n+e_i)`.
    A,
    /// `B_i J(n) = J(n-e_i)`.
    B,
}

/// One typed operator with a zero-based integral-index position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShiftOperator {
    kind: IndexShiftOperatorKind,
    index: usize,
}

impl IndexShiftOperator {
    pub const fn new(kind: IndexShiftOperatorKind, index: usize) -> Self {
        Self { kind, index }
    }

    pub const fn a(index: usize) -> Self {
        Self::new(IndexShiftOperatorKind::A, index)
    }

    pub const fn b(index: usize) -> Self {
        Self::new(IndexShiftOperatorKind::B, index)
    }

    pub const fn kind(self) -> IndexShiftOperatorKind {
        self.kind
    }

    pub const fn index(self) -> usize {
        self.index
    }
}

/// Resource policy for word conversion and coefficient specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexShiftOperatorLimits {
    pub max_word_length: usize,
    pub arithmetic_limits: ParametricArithmeticLimits,
}

impl Default for IndexShiftOperatorLimits {
    fn default() -> Self {
        Self {
            max_word_length: 1_000_000,
            arithmetic_limits: ParametricArithmeticLimits::default(),
        }
    }
}

/// A noncommutative word in exact LiteRed execution order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShiftOperatorWord {
    arity: usize,
    factors: Box<[IndexShiftOperator]>,
}

impl IndexShiftOperatorWord {
    pub fn try_new(
        arity: usize,
        factors: impl IntoIterator<Item = IndexShiftOperator>,
    ) -> Result<Self, IndexShiftOperatorError> {
        if arity == 0 {
            return Err(IndexShiftOperatorError::EmptyIndexSpace);
        }
        let factors: Box<[IndexShiftOperator]> = factors.into_iter().collect();
        for factor in &factors {
            if factor.index >= arity {
                return Err(IndexShiftOperatorError::IndexOutOfRange {
                    index: factor.index,
                    arity,
                });
            }
        }
        Ok(Self { arity, factors })
    }

    pub fn identity(arity: usize) -> Result<Self, IndexShiftOperatorError> {
        Self::try_new(arity, std::iter::empty())
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn factors(&self) -> &[IndexShiftOperator] {
        &self.factors
    }

    pub fn len(&self) -> usize {
        self.factors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factors.is_empty()
    }

    /// Net displacement after executing the complete word.
    pub fn net_shift(&self) -> Result<IndexShift, IndexShiftOperatorError> {
        let mut shift = vec![0_i64; self.arity];
        for factor in &self.factors {
            shift[factor.index] = match factor.kind {
                IndexShiftOperatorKind::A => shift[factor.index].checked_add(1),
                IndexShiftOperatorKind::B => shift[factor.index].checked_sub(1),
            }
            .ok_or(IndexShiftOperatorError::IndexOverflow {
                index: factor.index,
            })?;
        }
        Ok(IndexShift::try_new(shift, self.arity)?)
    }

    /// Return the exact prefactor and net shift produced by this word.
    ///
    /// Factors execute left-to-right, matching LiteRed's `ablisttoexpr`.
    pub fn action_signature(
        &self,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Result<(ParametricCoefficient, IndexShift), IndexShiftOperatorError> {
        self.validate_context_and_limits(context, limits)?;
        let mut offsets = vec![0_i64; self.arity];
        let mut a_counts = vec![0_u128; self.arity];
        for factor in &self.factors {
            if factor.kind == IndexShiftOperatorKind::A {
                a_counts[factor.index] = a_counts[factor.index].checked_add(1).ok_or(
                    IndexShiftOperatorError::ResourceCountOverflow {
                        resource: "A-operator degree",
                    },
                )?;
                if a_counts[factor.index] > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
                    return Err(IndexShiftOperatorError::ExponentLimit {
                        index: factor.index,
                        requested: a_counts[factor.index],
                        limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                    });
                }
            }
        }

        let mut factor_value = context.one();
        for operator in &self.factors {
            match operator.kind {
                IndexShiftOperatorKind::A => {
                    let index = context.index(operator.index)?;
                    let offset = context.integer(offsets[operator.index]);
                    let current_index = context.add_with_limits(
                        &index,
                        &offset,
                        limits.arithmetic_limits.exact_algebra,
                    )?;
                    factor_value = context.mul_with_limits(
                        &factor_value,
                        &current_index,
                        limits.arithmetic_limits.exact_algebra,
                    )?;
                    offsets[operator.index] = offsets[operator.index].checked_add(1).ok_or(
                        IndexShiftOperatorError::IndexOverflow {
                            index: operator.index,
                        },
                    )?;
                }
                IndexShiftOperatorKind::B => {
                    offsets[operator.index] = offsets[operator.index].checked_sub(1).ok_or(
                        IndexShiftOperatorError::IndexOverflow {
                            index: operator.index,
                        },
                    )?;
                }
            }
        }
        Ok((factor_value, IndexShift::try_new(offsets, self.arity)?))
    }

    /// Build an index-major pure word with the requested net displacement.
    ///
    /// This is not the unit-prefactor primitive `Shift(delta)`: every positive
    /// component contributes its usual rising-factor `A` action.
    pub fn pure_word_with_net_shift(
        shift: &IndexShift,
        limits: IndexShiftOperatorLimits,
    ) -> Result<Self, IndexShiftOperatorError> {
        if shift.arity() == 0 {
            return Err(IndexShiftOperatorError::EmptyIndexSpace);
        }
        let mut requested = 0_usize;
        for &value in shift.values() {
            let magnitude = usize::try_from(value.unsigned_abs()).map_err(|_| {
                IndexShiftOperatorError::ResourceCountOverflow {
                    resource: "operator word length",
                }
            })?;
            if magnitude > limits.max_word_length {
                return Err(IndexShiftOperatorError::WordLengthLimit {
                    requested: magnitude,
                    limit: limits.max_word_length,
                });
            }
            requested = requested.checked_add(magnitude).ok_or(
                IndexShiftOperatorError::ResourceCountOverflow {
                    resource: "operator word length",
                },
            )?;
            if requested > limits.max_word_length {
                return Err(IndexShiftOperatorError::WordLengthLimit {
                    requested,
                    limit: limits.max_word_length,
                });
            }
        }

        let mut factors = Vec::with_capacity(requested);
        for (index, &value) in shift.values().iter().enumerate() {
            let operator = if value >= 0 {
                IndexShiftOperator::a(index)
            } else {
                IndexShiftOperator::b(index)
            };
            let count = usize::try_from(value.unsigned_abs()).map_err(|_| {
                IndexShiftOperatorError::ResourceCountOverflow {
                    resource: "operator word length",
                }
            })?;
            factors.extend(std::iter::repeat_n(operator, count));
        }
        Self::try_new(shift.arity(), factors)
    }

    /// Whether factors are index-major and use at most one kind per index.
    pub fn is_index_major_pure(&self) -> bool {
        let mut last_index = None;
        let mut kind_at_index = vec![None; self.arity];
        for factor in &self.factors {
            if last_index.is_some_and(|last| factor.index < last) {
                return false;
            }
            if let Some(kind) = kind_at_index[factor.index] {
                if kind != factor.kind {
                    return false;
                }
            } else {
                kind_at_index[factor.index] = Some(factor.kind);
            }
            last_index = Some(factor.index);
        }
        true
    }

    fn validate_context_and_limits(
        &self,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Result<(), IndexShiftOperatorError> {
        if self.arity != context.index_count() {
            return Err(IndexShiftOperatorError::WrongArity {
                expected: context.index_count(),
                actual: self.arity,
            });
        }
        if self.factors.len() > limits.max_word_length {
            return Err(IndexShiftOperatorError::WordLengthLimit {
                requested: self.factors.len(),
                limit: limits.max_word_length,
            });
        }
        Ok(())
    }
}

/// One action monomial: either a lossless relation shift or an ordered word.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IndexShiftOperatorMonomial {
    /// Primitive unit-prefactor translation `J(n) -> J(n+delta)`.
    Shift(IndexShift),
    /// Ordered noncommutative LiteRed `A`/`B` word.
    Word(IndexShiftOperatorWord),
}

impl IndexShiftOperatorMonomial {
    pub fn arity(&self) -> usize {
        match self {
            Self::Shift(shift) => shift.arity(),
            Self::Word(word) => word.arity(),
        }
    }

    pub fn is_primitive_shift(&self) -> bool {
        matches!(self, Self::Shift(_))
    }

    pub fn action_signature(
        &self,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Result<(ParametricCoefficient, IndexShift), IndexShiftOperatorError> {
        match self {
            Self::Shift(shift) => {
                if shift.arity() != context.index_count() {
                    return Err(IndexShiftOperatorError::WrongArity {
                        expected: context.index_count(),
                        actual: shift.arity(),
                    });
                }
                Ok((context.one(), shift.clone()))
            }
            Self::Word(word) => word.action_signature(context, limits),
        }
    }
}

/// Sparse exact linear combination of primitive shifts and ordered words.
#[derive(Clone, Debug)]
pub struct IndexShiftOperatorExpression {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_id: ParametricRowId,
    arity: usize,
    terms: BTreeMap<IndexShiftOperatorMonomial, ParametricCoefficient>,
    // Compatibility polynomial-only view. `guarded_nonzero` is canonical.
    nonzero: Vec<ParametricPolynomial>,
    guarded_nonzero: Vec<ParametricNonZeroCondition>,
    limits: IndexShiftOperatorLimits,
}

impl PartialEq for IndexShiftOperatorExpression {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.row_id == other.row_id
            && self.arity == other.arity
            && self.terms == other.terms
            && self.nonzero == other.nonzero
            && self.limits == other.limits
    }
}

impl Eq for IndexShiftOperatorExpression {}

impl IndexShiftOperatorExpression {
    pub fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: ParametricRowId,
        context: &ParametricCoefficientContext,
    ) -> Self {
        Self::new_with_limits(
            family_fingerprint,
            row_id,
            context,
            IndexShiftOperatorLimits::default(),
        )
    }

    pub fn new_with_limits(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: ParametricRowId,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Self {
        Self {
            family_fingerprint: family_fingerprint.into(),
            context_fingerprint: context.fingerprint().into(),
            row_id,
            arity: context.index_count(),
            terms: BTreeMap::new(),
            nonzero: Vec::new(),
            guarded_nonzero: Vec::new(),
            limits,
        }
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn row_id(&self) -> &ParametricRowId {
        &self.row_id
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn terms(&self) -> &BTreeMap<IndexShiftOperatorMonomial, ParametricCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricPolynomial] {
        &self.nonzero
    }

    pub fn guarded_nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.guarded_nonzero
    }

    pub fn has_identical_guard_provenance(&self, other: &Self) -> bool {
        self == other && self.guarded_nonzero == other.guarded_nonzero
    }

    pub fn limits(&self) -> IndexShiftOperatorLimits {
        self.limits
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether every monomial is in the lossless sparse-relation normal form.
    pub fn is_relation_normal_form(&self) -> bool {
        self.terms
            .keys()
            .all(IndexShiftOperatorMonomial::is_primitive_shift)
    }

    pub fn add_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
    ) -> Result<(), IndexShiftOperatorError> {
        let condition = context.nonzero_condition_with_origins_and_limits(
            condition,
            [GuardOrigin::ExplicitShiftOperatorCondition],
            self.limits.arithmetic_limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition(context, condition)
    }

    pub fn add_guarded_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        mut condition: ParametricNonZeroCondition,
    ) -> Result<(), IndexShiftOperatorError> {
        self.validate_context(context)?;
        if !context.contains_nonzero_condition(&condition) {
            return Err(IndexShiftOperatorError::WrongContext);
        }
        context.validate_polynomial_with_limits(
            condition.polynomial(),
            self.limits.arithmetic_limits.exact_algebra,
        )?;
        if condition.polynomial().is_zero() {
            return Err(IndexShiftOperatorError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_origin_with_limit(
            GuardOrigin::ShiftOperatorConditionAttached {
                row: self.row_id.guard_identity(),
            },
            self.limits.arithmetic_limits.max_guard_origins,
        )?;
        let is_new = !self
            .guarded_nonzero
            .iter()
            .any(|existing| existing.polynomial() == condition.polynomial());
        let polynomial = condition.polynomial().clone();
        insert_parametric_condition(
            &mut self.guarded_nonzero,
            condition,
            self.limits.arithmetic_limits.max_guard_origins,
        )?;
        if is_new {
            self.nonzero.push(polynomial);
        }
        Ok(())
    }

    /// Add one action monomial without commuting its coefficient through it.
    pub fn add_monomial(
        &mut self,
        context: &ParametricCoefficientContext,
        monomial: IndexShiftOperatorMonomial,
        coefficient: ParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        let mut staged = self.clone();
        staged.add_monomial_in_place(context, monomial, coefficient)?;
        *self = staged;
        Ok(())
    }

    /// Apply an insertion to an isolated expression snapshot.
    ///
    /// The incoming denominator guard is attached before equal monomials are
    /// collected.  Keeping this helper private ensures a collection failure
    /// cannot commit only the guard through a public mutator.
    fn add_monomial_in_place(
        &mut self,
        context: &ParametricCoefficientContext,
        monomial: IndexShiftOperatorMonomial,
        coefficient: ParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        self.validate_context(context)?;
        if monomial.arity() != self.arity {
            return Err(IndexShiftOperatorError::WrongArity {
                expected: self.arity,
                actual: monomial.arity(),
            });
        }
        if let IndexShiftOperatorMonomial::Word(word) = &monomial {
            word.validate_context_and_limits(context, self.limits)?;
        }
        if !context.contains(&coefficient) {
            return Err(IndexShiftOperatorError::WrongContext);
        }
        context.validate_with_limits(&coefficient, self.limits.arithmetic_limits.exact_algebra)?;

        let denominator = context.denominator_condition_with_limits(
            &coefficient,
            self.limits.arithmetic_limits.exact_algebra,
        )?;
        let condition = context.nonzero_condition_with_origins_and_limits(
            denominator,
            [GuardOrigin::ShiftOperatorInputTermDenominator {
                row: self.row_id.guard_identity(),
            }],
            self.limits.arithmetic_limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition(context, condition)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        // A rational scalar coefficient is defined only on its original
        // denominator locus, even when later word action could cancel it.
        // Retain that provenance before collecting equal monomials.
        if let Some(current) = self.terms.get(&monomial) {
            let sum = context.add_with_limits(
                current,
                &coefficient,
                self.limits.arithmetic_limits.exact_algebra,
            )?;
            if sum.is_zero() {
                self.terms.remove(&monomial);
            } else {
                let denominator = context.denominator_condition_with_limits(
                    &sum,
                    self.limits.arithmetic_limits.exact_algebra,
                )?;
                let condition = context.nonzero_condition_with_origins_and_limits(
                    denominator,
                    [GuardOrigin::ShiftOperatorCollectedTermDenominator {
                        row: self.row_id.guard_identity(),
                    }],
                    self.limits.arithmetic_limits.exact_algebra,
                )?;
                self.add_guarded_nonzero_condition(context, condition)?;
                self.terms.insert(monomial, sum);
            }
        } else {
            self.terms.insert(monomial, coefficient);
        }
        Ok(())
    }

    /// Insert a scalar returned by guarded division without losing its domain
    /// when normalization makes the scalar (or a collected term) vanish.
    pub fn add_guarded_monomial(
        &mut self,
        context: &ParametricCoefficientContext,
        monomial: IndexShiftOperatorMonomial,
        coefficient: GuardedParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        let mut staged = self.clone();
        for condition in coefficient.nonzero {
            staged.add_guarded_nonzero_condition(context, condition)?;
        }
        staged.add_monomial_in_place(context, monomial, coefficient.value)?;
        *self = staged;
        Ok(())
    }

    pub fn add_word_term(
        &mut self,
        context: &ParametricCoefficientContext,
        word: IndexShiftOperatorWord,
        coefficient: ParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        self.add_monomial(context, IndexShiftOperatorMonomial::Word(word), coefficient)
    }

    pub fn add_guarded_word_term(
        &mut self,
        context: &ParametricCoefficientContext,
        word: IndexShiftOperatorWord,
        coefficient: GuardedParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        self.add_guarded_monomial(context, IndexShiftOperatorMonomial::Word(word), coefficient)
    }

    pub fn add_shift_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        self.add_monomial(
            context,
            IndexShiftOperatorMonomial::Shift(shift),
            coefficient,
        )
    }

    pub fn add_guarded_shift_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: GuardedParametricCoefficient,
    ) -> Result<(), IndexShiftOperatorError> {
        self.add_guarded_monomial(
            context,
            IndexShiftOperatorMonomial::Shift(shift),
            coefficient,
        )
    }

    /// Convert a relation bijectively to primitive unit-prefactor shifts.
    ///
    /// Unlike LiteRed's completed `ToAB`, this does not replace free `n_i` in
    /// coefficients by the composite operator `A_i ∘ B_i`.
    pub fn from_relation(
        relation: &ParametricRelation,
        context: &ParametricCoefficientContext,
    ) -> Result<Self, IndexShiftOperatorError> {
        Self::from_relation_with_limits(relation, context, IndexShiftOperatorLimits::default())
    }

    pub fn from_relation_with_limits(
        relation: &ParametricRelation,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Result<Self, IndexShiftOperatorError> {
        if relation.context_fingerprint() != context.fingerprint() {
            return Err(IndexShiftOperatorError::WrongContext);
        }
        if relation.arity() != context.index_count() {
            return Err(IndexShiftOperatorError::WrongArity {
                expected: context.index_count(),
                actual: relation.arity(),
            });
        }
        let mut result = Self::new_with_limits(
            relation.family_fingerprint(),
            relation.row_id().clone(),
            context,
            limits,
        );
        for condition in relation.guarded_nonzero_conditions() {
            let mut condition = condition.clone();
            condition.add_origin_with_limit(
                GuardOrigin::ShiftOperatorFromRelationAdapter {
                    row: relation.row_id().guard_identity(),
                },
                limits.arithmetic_limits.max_guard_origins,
            )?;
            result.add_guarded_nonzero_condition(context, condition)?;
        }
        for (shift, coefficient) in relation.terms() {
            result.add_shift_term(context, shift.clone(), coefficient.clone())?;
        }
        Ok(result)
    }

    /// Convert the intermediate back to a relation by executing every word
    /// left-to-right and collecting equal integral shifts only after
    /// multiplying its coefficient by the complete word prefactor.
    pub fn to_relation(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<ParametricRelation, IndexShiftOperatorError> {
        self.to_relation_with_limits(context, self.limits)
    }

    /// Convert the intermediate back to a relation under an explicit resource
    /// policy.  The supplied limits govern word execution, guard
    /// authentication, and every exact coefficient operation in the
    /// conversion.
    pub fn to_relation_with_limits(
        &self,
        context: &ParametricCoefficientContext,
        limits: IndexShiftOperatorLimits,
    ) -> Result<ParametricRelation, IndexShiftOperatorError> {
        self.validate_context(context)?;
        let mut relation = ParametricRelation::new(
            self.family_fingerprint.clone(),
            self.row_id.clone(),
            context,
        );
        for condition in &self.guarded_nonzero {
            let mut condition = condition.clone();
            condition.add_origin_with_limit(
                GuardOrigin::ShiftOperatorToRelationAdapter {
                    row: self.row_id.guard_identity(),
                },
                limits.arithmetic_limits.max_guard_origins,
            )?;
            relation.add_guarded_nonzero_condition_with_limits(
                context,
                condition,
                limits.arithmetic_limits,
            )?;
        }
        for (monomial, coefficient) in &self.terms {
            let (action_factor, shift) = monomial.action_signature(context, limits)?;
            let effective_coefficient = context.mul_with_limits(
                coefficient,
                &action_factor,
                limits.arithmetic_limits.exact_algebra,
            )?;
            relation.add_term_with_limits(
                context,
                shift,
                effective_coefficient,
                limits.arithmetic_limits,
            )?;
        }
        Ok(relation)
    }

    /// Normalize arbitrary mixed words to primitive relation shifts through
    /// their exact action.  This preserves action, not word structure.
    pub fn normalized(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<Self, IndexShiftOperatorError> {
        let relation = self.to_relation(context)?;
        Self::from_relation_with_limits(&relation, context, self.limits)
    }

    /// Apply this operator expression to `J(assignment)`.
    ///
    /// Conversion to a relation occurs before specialization, so word
    /// prefactors and scalar coefficients are multiplied symbolically first.
    pub fn specialize_action(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
    ) -> Result<ConcreteRelation, IndexShiftOperatorError> {
        let relation = self.to_relation(context)?;
        Ok(relation.specialize(context, assignment, self.limits.arithmetic_limits)?)
    }

    fn validate_context(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), IndexShiftOperatorError> {
        if self.context_fingerprint.as_ref() == context.fingerprint()
            && self.arity == context.index_count()
        {
            Ok(())
        } else {
            Err(IndexShiftOperatorError::WrongContext)
        }
    }
}

/// Checked failures from shift-operator construction or conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexShiftOperatorError {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        index: usize,
        arity: usize,
    },
    IndexOverflow {
        index: usize,
    },
    WordLengthLimit {
        requested: usize,
        limit: usize,
    },
    ExponentLimit {
        index: usize,
        requested: u128,
        limit: u128,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    WrongContext,
    UnsatisfiableDomain,
    Coefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
}

impl fmt::Display for IndexShiftOperatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => {
                formatter.write_str("an operator word needs at least one index")
            }
            Self::WrongArity { expected, actual } => {
                write!(formatter, "operator arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { index, arity } => write!(
                formatter,
                "operator index {index} is outside an index space of arity {arity}"
            ),
            Self::IndexOverflow { index } => {
                write!(
                    formatter,
                    "operator displacement overflowed at index {index}"
                )
            }
            Self::WordLengthLimit { requested, limit } => write!(
                formatter,
                "operator word length {requested} exceeds the configured limit {limit}"
            ),
            Self::ExponentLimit {
                index,
                requested,
                limit,
            } => write!(
                formatter,
                "A-operator degree {requested} at index {index} exceeds Symbolica's exponent limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed")
            }
            Self::WrongContext => formatter.write_str(
                "operator expression belongs to a different authenticated parametric context",
            ),
            Self::UnsatisfiableDomain => {
                formatter.write_str("operator expression has an identically zero domain guard")
            }
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for IndexShiftOperatorError {}

impl From<ParametricCoefficientError> for IndexShiftOperatorError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<ParametricRelationError> for IndexShiftOperatorError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits,
        ExactAlgebraOperation, IntegralFamily, ParametricIbpGenerator,
    };

    fn context(arity: usize) -> (CoefficientContext, ParametricCoefficientContext) {
        let base = CoefficientContext::new(["x", "nu", "d", "m"]);
        let parametric =
            ParametricCoefficientContext::try_new(&base, "shift-operator-tests", arity).unwrap();
        (base, parametric)
    }

    fn assert_parametric_eq(
        context: &ParametricCoefficientContext,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) {
        assert!(context.sub(left, right).unwrap().is_zero());
    }

    #[test]
    fn words_follow_litered_left_to_right_noncommutative_semantics() {
        let (_, context) = context(2);
        let limits = IndexShiftOperatorLimits::default();
        let n0 = context.index(0).unwrap();

        let ab = IndexShiftOperatorWord::try_new(
            2,
            [IndexShiftOperator::a(0), IndexShiftOperator::b(0)],
        )
        .unwrap();
        let ba = IndexShiftOperatorWord::try_new(
            2,
            [IndexShiftOperator::b(0), IndexShiftOperator::a(0)],
        )
        .unwrap();
        let (ab_factor, ab_shift) = ab.action_signature(&context, limits).unwrap();
        let (ba_factor, ba_shift) = ba.action_signature(&context, limits).unwrap();
        assert_parametric_eq(&context, &ab_factor, &n0);
        assert_parametric_eq(
            &context,
            &ba_factor,
            &context.sub(&n0, &context.one()).unwrap(),
        );
        assert_eq!(ab_shift.values(), &[0, 0]);
        assert_eq!(ba_shift.values(), &[0, 0]);
        assert_parametric_eq(
            &context,
            &context.sub(&ab_factor, &ba_factor).unwrap(),
            &context.one(),
        );

        let repeated = IndexShiftOperatorWord::try_new(
            2,
            [
                IndexShiftOperator::a(0),
                IndexShiftOperator::a(0),
                IndexShiftOperator::b(0),
                IndexShiftOperator::a(0),
                IndexShiftOperator::b(0),
                IndexShiftOperator::b(1),
                IndexShiftOperator::a(1),
            ],
        )
        .unwrap();
        let (factor, shift) = repeated.action_signature(&context, limits).unwrap();
        let n0_plus_one = context.add(&n0, &context.one()).unwrap();
        let n1_minus_one = context
            .sub(&context.index(1).unwrap(), &context.one())
            .unwrap();
        let expected = context
            .mul(
                &context.mul(&n0, &n0_plus_one).unwrap(),
                &context.mul(&n0_plus_one, &n1_minus_one).unwrap(),
            )
            .unwrap();
        assert_parametric_eq(&context, &factor, &expected);
        assert_eq!(shift.values(), &[1, 0]);
        assert!(!repeated.is_index_major_pure());

        let requested_shift = IndexShift::try_new([2, -1], 2).unwrap();
        let pure = IndexShiftOperatorWord::pure_word_with_net_shift(
            &requested_shift,
            IndexShiftOperatorLimits::default(),
        )
        .unwrap();
        let (pure_factor, pure_shift) = pure.action_signature(&context, limits).unwrap();
        assert!(pure.is_index_major_pure());
        assert_eq!(pure_shift, requested_shift);
        assert_parametric_eq(
            &context,
            &pure_factor,
            &context.mul(&n0, &n0_plus_one).unwrap(),
        );

        let tiny_limits = IndexShiftOperatorLimits {
            max_word_length: 2,
            ..IndexShiftOperatorLimits::default()
        };
        assert_eq!(
            IndexShiftOperatorWord::pure_word_with_net_shift(&requested_shift, tiny_limits),
            Err(IndexShiftOperatorError::WordLengthLimit {
                requested: 3,
                limit: 2,
            })
        );

        let single_oversized_component = IndexShift::try_new([3, 0], 2).unwrap();
        assert_eq!(
            IndexShiftOperatorWord::pure_word_with_net_shift(
                &single_oversized_component,
                tiny_limits,
            ),
            Err(IndexShiftOperatorError::WordLengthLimit {
                requested: 3,
                limit: 2,
            })
        );
    }

    #[test]
    fn custom_exact_limits_authenticate_insertions_and_both_relation_adapters() {
        let (_, context) = context(1);
        let strict_limits = IndexShiftOperatorLimits {
            arithmetic_limits: ParametricArithmeticLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_exponent: 1,
                    ..ExactAlgebraLimits::default()
                },
                ..ParametricArithmeticLimits::default()
            },
            ..IndexShiftOperatorLimits::default()
        };
        let n0 = context.index(0).unwrap();
        let n0_squared = context.mul(&n0, &n0).unwrap();
        let rational = context.checked_div(&context.one(), &n0_squared).unwrap();
        let guard = context.denominator_condition(&rational).unwrap();
        let shift = IndexShift::try_new([0], 1).unwrap();

        let mut strict_expression = IndexShiftOperatorExpression::new_with_limits(
            "strict-operator-family",
            ParametricRowId::Derived {
                label: "strict-insertion".into(),
            },
            &context,
            strict_limits,
        );
        assert!(matches!(
            strict_expression.add_shift_term(&context, shift.clone(), n0_squared.clone()),
            Err(IndexShiftOperatorError::Coefficient(
                ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    requested: 2,
                    limit: 1,
                    ..
                })
            ))
        ));
        assert!(strict_expression.terms().is_empty());
        assert!(matches!(
            strict_expression.add_nonzero_condition(&context, guard.clone()),
            Err(IndexShiftOperatorError::Coefficient(
                ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    requested: 2,
                    limit: 1,
                    ..
                })
            ))
        ));
        assert!(strict_expression.nonzero_conditions().is_empty());

        let mut relation = ParametricRelation::new(
            "strict-operator-family",
            ParametricRowId::Derived {
                label: "strict-from-relation".into(),
            },
            &context,
        );
        relation
            .add_term(&context, shift, rational.clone())
            .unwrap();
        assert!(matches!(
            IndexShiftOperatorExpression::from_relation_with_limits(
                &relation,
                &context,
                strict_limits,
            ),
            Err(IndexShiftOperatorError::Coefficient(
                ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    requested: 2,
                    limit: 1,
                    ..
                })
            ))
        ));

        let mut permissive_expression = IndexShiftOperatorExpression::new(
            "strict-operator-family",
            ParametricRowId::Derived {
                label: "strict-to-relation".into(),
            },
            &context,
        );
        permissive_expression
            .add_nonzero_condition(&context, guard)
            .unwrap();
        assert!(permissive_expression.to_relation(&context).is_ok());
        assert!(matches!(
            permissive_expression.to_relation_with_limits(&context, strict_limits),
            Err(IndexShiftOperatorError::Relation(
                ParametricRelationError::Coefficient(ParametricCoefficientError::ExactAlgebra(
                    ExactAlgebraError::ExponentLimit {
                        operation: ExactAlgebraOperation::Authenticate,
                        requested: 2,
                        limit: 1,
                        ..
                    }
                ))
            ))
        ));
    }

    #[test]
    fn relation_to_primitive_shifts_to_relation_is_structurally_exact_on_shift_grid() {
        let (base, context) = context(2);
        let mut relation = ParametricRelation::new(
            "shift-grid-family",
            ParametricRowId::Derived {
                label: "grid".into(),
            },
            &context,
        );
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();
        let one = context.one();
        for first in -3_i64..=3 {
            for second in -2_i64..=2 {
                let shift = IndexShift::try_new([first, second], 2).unwrap();
                let coefficient = context
                    .add(
                        &context.add(&n0, &context.mul(&n1, &x).unwrap()).unwrap(),
                        &context.integer(17 + 3 * first - second),
                    )
                    .unwrap();
                let denominator = context.add(&n1, &one).unwrap();
                let coefficient = if first == 3 && second == -2 {
                    context.checked_div(&coefficient, &denominator).unwrap()
                } else {
                    coefficient
                };
                relation.add_term(&context, shift, coefficient).unwrap();
            }
        }
        let operators = IndexShiftOperatorExpression::from_relation(&relation, &context).unwrap();
        assert!(operators.is_relation_normal_form());
        let reconstructed = operators.to_relation(&context).unwrap();
        assert_eq!(reconstructed, relation);
    }

    #[test]
    fn primitive_shift_to_relation_round_trip_is_structurally_exact() {
        let (base, context) = context(2);
        let row_id = ParametricRowId::Derived {
            label: "canonical-ab".into(),
        };
        let mut expression =
            IndexShiftOperatorExpression::new("canonical-family", row_id, &context);
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();
        let n1_plus_x = context.add(&context.index(1).unwrap(), &x).unwrap();
        for (shift, coefficient) in [
            ([2, -1], n1_plus_x),
            (
                [-2, 0],
                context.add(&context.index(0).unwrap(), &x).unwrap(),
            ),
            ([0, 0], context.integer(7)),
        ] {
            let shift = IndexShift::try_new(shift, 2).unwrap();
            expression
                .add_shift_term(&context, shift, coefficient)
                .unwrap();
        }
        assert!(expression.is_relation_normal_form());
        let relation = expression.to_relation(&context).unwrap();
        let reconstructed = IndexShiftOperatorExpression::from_relation_with_limits(
            &relation,
            &context,
            expression.limits(),
        )
        .unwrap();
        assert_eq!(reconstructed, expression);
    }

    #[test]
    fn primitive_round_trip_preserves_pre_cancellation_denominator_guards() {
        let (_, context) = context(1);
        let n0 = context.index(0).unwrap();
        let n0_plus_one = context.add(&n0, &context.one()).unwrap();
        let coefficient = context.checked_div(&n0, &n0_plus_one).unwrap();
        let mut relation = ParametricRelation::new(
            "guard-family",
            ParametricRowId::Derived {
                label: "guard".into(),
            },
            &context,
        );
        relation
            .add_term(&context, IndexShift::try_new([1], 1).unwrap(), coefficient)
            .unwrap();
        assert_eq!(relation.nonzero_conditions().len(), 1);

        let expression = IndexShiftOperatorExpression::from_relation(&relation, &context).unwrap();
        assert_eq!(
            expression.nonzero_conditions(),
            relation.nonzero_conditions()
        );
        let row = relation.row_id().guard_identity();
        let operator_origins = expression.guarded_nonzero_conditions()[0].origins();
        assert!(
            operator_origins.contains(&GuardOrigin::RelationInputTermDenominator {
                row: row.clone(),
                shift: vec![1].into_boxed_slice(),
            })
        );
        assert!(
            operator_origins
                .contains(&GuardOrigin::ShiftOperatorFromRelationAdapter { row: row.clone() })
        );
        let reconstructed = expression.to_relation(&context).unwrap();
        assert_eq!(reconstructed, relation);
        assert!(!reconstructed.has_identical_guard_provenance(&relation));
        assert!(
            reconstructed.guarded_nonzero_conditions()[0]
                .origins()
                .contains(&GuardOrigin::ShiftOperatorToRelationAdapter { row })
        );
        assert!(matches!(
            expression.specialize_action(&context, &[-1]),
            Err(IndexShiftOperatorError::Relation(
                ParametricRelationError::UnsatisfiableDomain
            ))
        ));
    }

    #[test]
    fn guarded_zero_operator_term_survives_both_adapters() {
        let (_, context) = context(1);
        let n0 = context.index(0).unwrap();
        let divided = context.checked_div_guarded(&context.zero(), &n0).unwrap();
        let mut expression = IndexShiftOperatorExpression::new(
            "guarded-zero-operator-family",
            ParametricRowId::Derived {
                label: "guarded-zero-operator".into(),
            },
            &context,
        );
        expression
            .add_guarded_shift_term(&context, IndexShift::try_new([0], 1).unwrap(), divided)
            .unwrap();
        assert!(expression.terms().is_empty());
        assert_eq!(expression.guarded_nonzero_conditions().len(), 1);

        let relation = expression.to_relation(&context).unwrap();
        assert!(relation.terms().is_empty());
        assert_eq!(relation.guarded_nonzero_conditions().len(), 1);
        assert!(matches!(
            relation.specialize(&context, &[0], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));
    }

    fn direct_action(
        expression: &IndexShiftOperatorExpression,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
    ) -> BTreeMap<Vec<i64>, Coefficient> {
        let base = context.base();
        let mut result = BTreeMap::new();
        for (monomial, coefficient) in expression.terms() {
            let coefficient = context
                .specialize(
                    coefficient,
                    assignment,
                    expression.limits().arithmetic_limits,
                )
                .unwrap()
                .value;
            let mut position = assignment.to_vec();
            let mut integer_factor = 1_i64;
            match monomial {
                IndexShiftOperatorMonomial::Shift(shift) => {
                    for (position, &delta) in position.iter_mut().zip(shift.values()) {
                        *position = position.checked_add(delta).unwrap();
                    }
                }
                IndexShiftOperatorMonomial::Word(word) => {
                    for operator in word.factors() {
                        match operator.kind() {
                            IndexShiftOperatorKind::A => {
                                integer_factor = integer_factor
                                    .checked_mul(position[operator.index()])
                                    .unwrap();
                                position[operator.index()] += 1;
                            }
                            IndexShiftOperatorKind::B => position[operator.index()] -= 1,
                        }
                    }
                }
            }
            let value = &coefficient * &base.integer(integer_factor);
            if value.is_zero() {
                continue;
            }
            if let Some(current) = result.get(&position) {
                let sum: Coefficient = current + &value;
                if sum.is_zero() {
                    result.remove(&position);
                } else {
                    result.insert(position, sum);
                }
            } else {
                result.insert(position, value);
            }
        }
        result
    }

    #[test]
    fn mixed_words_normalize_without_changing_concrete_action() {
        let (base, context) = context(2);
        let mut expression = IndexShiftOperatorExpression::new(
            "mixed-family",
            ParametricRowId::Derived {
                label: "mixed".into(),
            },
            &context,
        );
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();
        let n0_plus_x = context.add(&context.index(0).unwrap(), &x).unwrap();
        for (factors, coefficient) in [
            (
                vec![IndexShiftOperator::a(0), IndexShiftOperator::b(0)],
                n0_plus_x,
            ),
            (
                vec![IndexShiftOperator::b(0), IndexShiftOperator::a(0)],
                context.integer(3),
            ),
            (
                vec![
                    IndexShiftOperator::a(1),
                    IndexShiftOperator::a(1),
                    IndexShiftOperator::b(1),
                    IndexShiftOperator::b(0),
                ],
                context.integer(-2),
            ),
        ] {
            expression
                .add_word_term(
                    &context,
                    IndexShiftOperatorWord::try_new(2, factors).unwrap(),
                    coefficient,
                )
                .unwrap();
        }
        assert!(!expression.is_relation_normal_form());
        let normalized = expression.normalized(&context).unwrap();
        assert!(normalized.is_relation_normal_form());

        for assignment in [[2, 3], [4, -2], [-3, 5]] {
            let direct = direct_action(&expression, &context, &assignment);
            let specialized = expression.specialize_action(&context, &assignment).unwrap();
            let actual = specialized
                .terms()
                .iter()
                .map(|(key, coefficient)| (key.powers().to_vec(), coefficient.clone()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual, direct);
            assert_eq!(
                normalized.specialize_action(&context, &assignment).unwrap(),
                specialized
            );
        }
    }

    #[test]
    fn power_shifted_generated_row_round_trips_and_acts_at_zero() {
        let base = CoefficientContext::new(["d", "m", "nu"]);
        let family = IntegralFamily::new(
            "operator-power-shift-family",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            base.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                base.parameter("m").unwrap(),
                vec![base.one()],
            )],
            Vec::new(),
            vec![base.parameter("nu").unwrap()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let relation = &generated.ordinary_ibp()[0];
        let expression =
            IndexShiftOperatorExpression::from_relation(relation, generated.context()).unwrap();
        assert!(expression.is_relation_normal_form());
        assert_eq!(
            expression.to_relation(generated.context()).unwrap(),
            *relation
        );

        // Primitive shifts introduce no `n0 != 0` chart.  In particular, a
        // nonzero PowerShift keeps the raised contribution at n0=0.
        let operator_at_zero = expression
            .specialize_action(generated.context(), &[0])
            .unwrap();
        let relation_at_zero = relation
            .specialize(
                generated.context(),
                &[0],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(operator_at_zero, relation_at_zero);
        assert_eq!(operator_at_zero.terms().len(), 2);
    }

    #[test]
    fn words_and_relations_reject_foreign_arity_or_context() {
        assert_eq!(
            IndexShiftOperatorWord::try_new(1, [IndexShiftOperator::a(1)]),
            Err(IndexShiftOperatorError::IndexOutOfRange { index: 1, arity: 1 })
        );
        let (_, one_index) = context(1);
        let (_, two_indices) = context(2);
        let relation = ParametricRelation::new(
            "foreign",
            ParametricRowId::Derived {
                label: "foreign".into(),
            },
            &one_index,
        );
        assert_eq!(
            IndexShiftOperatorExpression::from_relation(&relation, &two_indices),
            Err(IndexShiftOperatorError::WrongContext)
        );
    }
}
