//! Construction, authentication, and exact field arithmetic.

use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomial, ExactAlgebraLimits,
    checked_coefficient_add_on_map, checked_coefficient_mul_on_map, checked_coefficient_neg_on_map,
    checked_coefficient_sub_on_map, validate_coefficient_on_map, validate_polynomial_on_map,
};

use super::error::IndexedAlgebraError;
use super::scope::{
    base_context_fingerprint, encode_symbol_component, indexed_context_fingerprint,
    preflight_qualified_index_symbols, qualified_index_symbol,
};
use super::value::{IndexedCoefficient, IndexedPolynomial};

/// One exact pair of authenticated fields `K` and `K(n)`.
#[derive(Clone, Debug)]
pub struct IndexedCoefficientContext {
    pub(super) base: CoefficientContext,
    // Moving the fallibly constructed String into Arc only allocates the
    // fixed-size control block; unlike String -> Arc<str>, it does not make
    // another caller-sized allocation and copy.
    pub(super) fingerprint: Arc<String>,
    pub(super) variables: Arc<Vec<PolyVariable>>,
    pub(super) index_variables: Arc<Vec<PolyVariable>>,
    pub(super) template: Coefficient,
}

impl IndexedCoefficientContext {
    /// Extend `base` by `index_count` private index variables.
    ///
    /// `scope` is persisted as part of the context identity.  Its bytes are
    /// encoded losslessly in Symbolica's namespace, so two different scopes
    /// cannot alias merely because they sanitize to the same identifier.
    pub fn try_new(
        base: &CoefficientContext,
        scope: &str,
        index_count: usize,
    ) -> Result<Self, IndexedAlgebraError> {
        if index_count == 0 {
            return Err(IndexedAlgebraError::EmptyIndexSpace);
        }
        if scope.is_empty() {
            return Err(IndexedAlgebraError::InvalidScope);
        }

        let variable_count = base.variables().len().checked_add(index_count).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "indexed coefficient variables",
            },
        )?;
        let encoded_scope = encode_symbol_component(scope.as_bytes())?;
        let mut index_variables = Vec::new();
        index_variables
            .try_reserve_exact(index_count)
            .map_err(|_| IndexedAlgebraError::AllocationFailure {
                resource: "indexed coefficient index variables",
                requested: index_count,
            })?;
        let mut variables = Vec::new();
        variables.try_reserve_exact(variable_count).map_err(|_| {
            IndexedAlgebraError::AllocationFailure {
                resource: "indexed coefficient variables",
                requested: variable_count,
            }
        })?;
        preflight_qualified_index_symbols(encoded_scope.len(), index_count)?;
        let base_fingerprint = base_context_fingerprint(base)?;
        let fingerprint = Arc::new(indexed_context_fingerprint(
            &base_fingerprint,
            scope,
            index_count,
        )?);

        // RustRed has overflow-checked the complete caller-derived name
        // workload and fallibly reserved every Rust-owned vector/string used
        // here. Symbolica's public API does not expose a capacity preflight
        // for NamespacedSymbol parsing or its global symbol interner; retain
        // its Option/Result errors, but do not claim that an unrelated probe
        // or catch_unwind can make those internal allocations fallible.
        for position in 0..index_count {
            let qualified = qualified_index_symbol(&encoded_scope, position)?;
            let namespaced = NamespacedSymbol::try_parse(&qualified)
                .ok_or(IndexedAlgebraError::IndexSymbolRegistrationFailure { position })?;
            let symbol = SymbolBuilder::new(namespaced)
                .build()
                .map_err(|_| IndexedAlgebraError::IndexSymbolRegistrationFailure { position })?;
            let variable = PolyVariable::Symbol(symbol);
            if base.variables().contains(&variable) {
                return Err(IndexedAlgebraError::IndexSymbolCollision { position });
            }
            index_variables.push(variable);
        }

        variables.extend(base.variables().iter().cloned());
        variables.extend(index_variables.iter().cloned());
        let variables = Arc::new(variables);
        // RationalPolynomial::new is likewise infallible in Symbolica's
        // public API and may initialize internal template state. All sizes
        // RustRed can truthfully preflight (the variable count and retained
        // Rust-owned containers) have already been checked and reserved.
        let template = RationalPolynomial::new(&Z, variables.clone());

        Ok(Self {
            base: base.clone(),
            fingerprint,
            variables,
            index_variables: Arc::new(index_variables),
            template,
        })
    }

    pub fn base(&self) -> &CoefficientContext {
        &self.base
    }

    pub fn fingerprint(&self) -> &str {
        self.fingerprint.as_str()
    }

    pub fn index_count(&self) -> usize {
        self.index_variables.len()
    }

    pub fn contains(&self, value: &IndexedCoefficient) -> bool {
        value.context.as_str() == self.fingerprint.as_str()
            && validate_coefficient_on_map(
                &value.raw,
                &self.variables,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn validate_polynomial_with_limits(
        &self,
        value: &IndexedPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<(), IndexedAlgebraError> {
        self.validate_polynomial_context(value)?;
        validate_polynomial_on_map(
            &value.raw,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(())
    }

    pub(crate) fn validate_polynomial_context(
        &self,
        value: &IndexedPolynomial,
    ) -> Result<(), IndexedAlgebraError> {
        if value.context.as_str() == self.fingerprint.as_str() {
            Ok(())
        } else {
            Err(IndexedAlgebraError::WrongContext)
        }
    }

    pub fn zero(&self) -> IndexedCoefficient {
        self.wrap_unchecked(self.template.numerator.zero().into())
    }

    pub fn one(&self) -> IndexedCoefficient {
        self.wrap_unchecked(self.template.numerator.one().into())
    }

    pub fn integer(&self, value: i64) -> IndexedCoefficient {
        self.wrap_unchecked(
            self.template
                .numerator
                .constant(Integer::from(value))
                .into(),
        )
    }

    pub fn index(&self, position: usize) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let variable =
            self.index_variables
                .get(position)
                .ok_or(IndexedAlgebraError::WrongIndexArity {
                    expected: self.index_count(),
                    actual: position.saturating_add(1),
                })?;
        let polynomial = self
            .template
            .numerator
            .variable(variable)
            .map_err(IndexedAlgebraError::Symbolica)?;
        Ok(self.wrap_unchecked(polynomial.into()))
    }

    pub fn lift(&self, value: &Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        if !self.base.contains(value) {
            return Err(IndexedAlgebraError::WrongContext);
        }
        let numerator = self.extend_base_polynomial(&value.numerator)?;
        let denominator = self.extend_base_polynomial(&value.denominator)?;
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let raw = <Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true);
        self.wrap_checked(raw)
    }

    pub fn lift_base_polynomial(
        &self,
        value: &CoefficientPolynomial,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let raw = self.extend_base_polynomial(value)?;
        Ok(IndexedPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn numerator_condition(
        &self,
        value: &IndexedCoefficient,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.numerator_condition_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn numerator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_with_limits(value, limits)?;
        Ok(IndexedPolynomial {
            raw: value.raw.numerator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub fn denominator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_with_limits(value, limits)?;
        Ok(IndexedPolynomial {
            raw: value.raw.denominator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub fn add(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.add_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn add_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_add_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn sub(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.sub_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn sub_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_sub_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn mul(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.mul_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn mul_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_mul_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn neg_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_with_limits(value, limits)?;
        let raw = checked_coefficient_neg_on_map(&value.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn validate_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), IndexedAlgebraError> {
        if value.context.as_str() != self.fingerprint.as_str() {
            return Err(IndexedAlgebraError::WrongContext);
        }
        validate_coefficient_on_map(&value.raw, &self.variables, limits)?;
        Ok(())
    }

    pub(crate) fn validate_index_arity(&self, shift: &[i64]) -> Result<(), IndexedAlgebraError> {
        if shift.len() == self.index_count() {
            Ok(())
        } else {
            Err(IndexedAlgebraError::WrongIndexArity {
                expected: self.index_count(),
                actual: shift.len(),
            })
        }
    }

    fn raw_uses_extended_map(&self, raw: &Coefficient) -> bool {
        validate_coefficient_on_map(raw, &self.variables, ExactAlgebraLimits::default()).is_ok()
    }

    fn wrap_unchecked(&self, raw: Coefficient) -> IndexedCoefficient {
        debug_assert!(self.raw_uses_extended_map(&raw));
        IndexedCoefficient {
            raw,
            context: self.fingerprint.clone(),
        }
    }

    fn wrap_checked(&self, raw: Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.wrap_checked_with_limits(raw, ExactAlgebraLimits::default())
    }

    pub(super) fn wrap_checked_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        validate_coefficient_on_map(&raw, &self.variables, limits)?;
        Ok(self.wrap_unchecked(raw))
    }

    fn extend_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        validate_polynomial_on_map(
            source,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        let mut result = self
            .template
            .numerator
            .zero_with_capacity(source.coefficients.len());
        let mut exponents = vec![0_u16; self.variables.len()];
        for (coefficient, source_exponents) in
            source.coefficients.iter().zip(source.exponents_iter())
        {
            exponents.fill(0);
            exponents[..self.base.variables().len()].copy_from_slice(source_exponents);
            result.append_monomial(coefficient.clone(), &exponents);
        }
        Ok(result)
    }
}
