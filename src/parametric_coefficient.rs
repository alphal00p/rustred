//! Authenticated exact coefficient fields for parametric integral identities.
//!
//! A family is defined over a base field `K = Q(theta)`.  Parametric IBP
//! coefficients live in the strictly extended field `K(n)`, whose index
//! variables are internal RustRed symbols appended after every base variable.
//! Symbolica can automatically unify variable maps; this module deliberately
//! rejects that behavior at the proof-bearing boundary.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::GuardOrigin;
use crate::algebra::{
    ExactAlgebraError, ExactAlgebraLimits, checked_coefficient_add_on_map,
    checked_coefficient_mul_on_map, checked_coefficient_neg_on_map, checked_coefficient_sub_on_map,
    validate_coefficient_on_map, validate_polynomial_on_map,
};
use crate::{algebra::Coefficient, algebra::CoefficientContext};

pub type CoefficientPolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// A canonical coefficient known to belong to one exact `K(n)` variable map.
///
/// All public constructors normalize numerator and denominator to coprime
/// factors. This invariant lets integral index translations avoid a second
/// polynomial GCD: `n -> n + a` is a polynomial-ring automorphism and thus
/// preserves coprimality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricCoefficient {
    raw: Coefficient,
    context: Arc<str>,
}

impl ParametricCoefficient {
    pub fn raw(&self) -> &Coefficient {
        &self.raw
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }
}

/// A polynomial over `K`'s integer polynomial ring, authenticated by its
/// ordered base variable map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasePolynomial {
    raw: CoefficientPolynomial,
    context: Arc<str>,
}

impl BasePolynomial {
    /// Authenticate a base-field polynomial against an exact coefficient
    /// context. This is used when a later concrete quotient introduces a new
    /// nonzero condition that did not exist in the parametric source rows.
    pub fn try_from_raw(
        raw: CoefficientPolynomial,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ParametricCoefficientError> {
        validate_polynomial_on_map(
            &raw,
            context.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(Self {
            raw,
            context: base_context_fingerprint(context).into(),
        })
    }

    pub fn raw(&self) -> &CoefficientPolynomial {
        &self.raw
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.raw.is_one()
    }

    pub fn is_nonzero_constant(&self) -> bool {
        self.raw.is_constant() && !self.raw.is_zero()
    }
}

/// A polynomial over the exact index-extended map `K(n)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricPolynomial {
    raw: CoefficientPolynomial,
    context: Arc<str>,
}

/// One authenticated polynomial nonzero condition with every atomic reason
/// it entered the exceptional-domain set.
///
/// Origins are stored in a `BTreeSet`, so merging the same polynomial is
/// deterministic and independent of relation assembly order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroCondition {
    polynomial: ParametricPolynomial,
    origins: BTreeSet<GuardOrigin>,
}

impl ParametricNonZeroCondition {
    pub fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }

    /// Attach an origin under an explicit provenance-cardinality budget.
    pub fn try_with_origin(
        mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        self.add_origin_with_limit(origin, max_guard_origins)?;
        Ok(self)
    }

    pub(crate) fn add_origin_with_limit(
        &mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        if !self.origins.contains(&origin) {
            let requested = self.origins.len().checked_add(1).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric guard origins",
                },
            )?;
            check_limit("parametric guard origins", requested, max_guard_origins)?;
            self.origins.insert(origin);
        }
        Ok(())
    }

    pub(crate) fn merge_origins_from(
        &mut self,
        other: &Self,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        debug_assert_eq!(self.polynomial, other.polynomial);
        let additional = other
            .origins
            .iter()
            .filter(|origin| !self.origins.contains(*origin))
            .count();
        let requested = self.origins.len().checked_add(additional).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric guard origins",
            },
        )?;
        check_limit("parametric guard origins", requested, max_guard_origins)?;
        self.origins.extend(other.origins.iter().cloned());
        Ok(())
    }
}

/// A specialized base-field polynomial condition with retained parametric
/// provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecializedNonZeroCondition {
    polynomial: BasePolynomial,
    origins: BTreeSet<GuardOrigin>,
}

impl SpecializedNonZeroCondition {
    pub fn from_base_polynomial(
        polynomial: BasePolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        max_guard_origins: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        if polynomial.is_zero() {
            return Err(ParametricCoefficientError::ZeroPolynomialCondition);
        }
        let origins = origins.into_iter().collect::<BTreeSet<_>>();
        if origins.is_empty() {
            return Err(ParametricCoefficientError::MissingGuardOrigin);
        }
        check_limit(
            "specialized guard origins",
            origins.len(),
            max_guard_origins,
        )?;
        Ok(Self {
            polynomial,
            origins,
        })
    }

    pub fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }

    pub(crate) fn add_origin_with_limit(
        &mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        if !self.origins.contains(&origin) {
            let requested = self.origins.len().checked_add(1).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "specialized guard origins",
                },
            )?;
            check_limit("specialized guard origins", requested, max_guard_origins)?;
            self.origins.insert(origin);
        }
        Ok(())
    }

    pub(crate) fn merge_origins_from(
        &mut self,
        other: &Self,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        debug_assert_eq!(self.polynomial, other.polynomial);
        let additional = other
            .origins
            .iter()
            .filter(|origin| !self.origins.contains(*origin))
            .count();
        let requested = self.origins.len().checked_add(additional).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "specialized guard origins",
            },
        )?;
        check_limit("specialized guard origins", requested, max_guard_origins)?;
        self.origins.extend(other.origins.iter().cloned());
        Ok(())
    }
}

impl ParametricPolynomial {
    pub fn raw(&self) -> &CoefficientPolynomial {
        &self.raw
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.raw.is_one()
    }

    pub fn is_nonzero_constant(&self) -> bool {
        self.raw.is_constant() && !self.raw.is_zero()
    }

    /// Number of sparse monomials retained by the authenticated Symbolica
    /// polynomial.  Proof-bearing layers use this to preflight the memory
    /// cost of duplicating a predicate across complementary case branches.
    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }
}

/// Explicit upper bounds around Symbolica operations whose output can expand
/// under an affine index translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricArithmeticLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_specialization_power_operations: usize,
    /// Maximum conservative magnitude bit length of an integer coefficient
    /// produced while specializing or affinely translating index variables.
    pub max_specialization_integer_bits: usize,
    pub max_guard_origins: usize,
}

impl Default for ParametricArithmeticLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_specialization_power_operations: 16_000_000,
            max_specialization_integer_bits: 16_000_000,
            max_guard_origins: 65_536,
        }
    }
}

/// Prospective mathematical bounds used immediately by one translation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ParametricPolynomialTranslationPreflight {
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    largest_output_integer_bit_bound: usize,
}

/// Prospective mathematical bounds used immediately by one specialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ParametricPolynomialSpecializationPreflight {
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    largest_output_integer_bit_bound: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricCoefficientError {
    EmptyIndexSpace,
    InvalidScope(String),
    IndexSymbolCollision {
        position: usize,
    },
    WrongContext,
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    ZeroPolynomialCondition,
    ZeroDenominator,
    MalformedPolynomial {
        terms: usize,
        exponents: usize,
        variables: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    MissingGuardOrigin,
    ExactAlgebra(ExactAlgebraError),
    Symbolica(String),
}

impl fmt::Display for ParametricCoefficientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => {
                formatter.write_str("a parametric context needs at least one index")
            }
            Self::InvalidScope(scope) => {
                write!(formatter, "invalid parametric context scope {scope:?}")
            }
            Self::IndexSymbolCollision { position } => write!(
                formatter,
                "generated parametric index symbol {position} collides with a base variable"
            ),
            Self::WrongContext => formatter.write_str(
                "coefficient or polynomial belongs to a different authenticated context",
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "index vector has arity {actual}, expected {expected}"
            ),
            Self::ZeroPolynomialCondition => {
                formatter.write_str("a required nonzero polynomial is identically zero")
            }
            Self::ZeroDenominator => {
                formatter.write_str("rational coefficient has a zero denominator")
            }
            Self::MalformedPolynomial {
                terms,
                exponents,
                variables,
            } => write!(
                formatter,
                "polynomial has {terms} terms, {exponents} exponents, and {variables} variables"
            ),
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
            Self::MissingGuardOrigin => {
                formatter.write_str("a nonzero condition needs at least one typed origin")
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Symbolica(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ParametricCoefficientError {}

impl From<ExactAlgebraError> for ParametricCoefficientError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

/// Successful specialization of one `K(n)` coefficient back into `K`.
///
/// `nonzero` retains the mapped original denominator before Symbolica can
/// cancel factors in the resulting fraction-field element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedCoefficientSpecialization {
    pub value: Coefficient,
    pub nonzero: Vec<BasePolynomial>,
    guarded_nonzero: Vec<SpecializedNonZeroCondition>,
}

impl GuardedCoefficientSpecialization {
    /// Provenance-preserving view of [`Self::nonzero`].
    pub fn guarded_nonzero_conditions(&self) -> &[SpecializedNonZeroCondition] {
        &self.guarded_nonzero
    }
}

/// One exact pair of authenticated fields `K` and `K(n)`.
#[derive(Clone, Debug)]
pub struct ParametricCoefficientContext {
    base: CoefficientContext,
    base_fingerprint: Arc<str>,
    fingerprint: Arc<str>,
    variables: Arc<Vec<PolyVariable>>,
    index_variables: Arc<Vec<PolyVariable>>,
    template: Coefficient,
}

impl ParametricCoefficientContext {
    /// Extend `base` by `index_count` private index variables.
    ///
    /// `scope` is persisted as part of the context identity.  Its bytes are
    /// encoded losslessly in Symbolica's namespace, so two different scopes
    /// cannot alias merely because they sanitize to the same identifier.
    pub fn try_new(
        base: &CoefficientContext,
        scope: &str,
        index_count: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        if index_count == 0 {
            return Err(ParametricCoefficientError::EmptyIndexSpace);
        }
        if scope.is_empty() {
            return Err(ParametricCoefficientError::InvalidScope(scope.to_owned()));
        }

        let encoded_scope = encode_symbol_component(scope.as_bytes());
        let mut index_variables = Vec::with_capacity(index_count);
        for position in 0..index_count {
            let qualified = format!("rustred::parametric_s{encoded_scope}::n{position}");
            let namespaced = NamespacedSymbol::try_parse(&qualified)
                .ok_or_else(|| ParametricCoefficientError::InvalidScope(scope.to_owned()))?;
            let symbol = SymbolBuilder::new(namespaced)
                .build()
                .map_err(|error| ParametricCoefficientError::Symbolica(error.to_string()))?;
            let variable = PolyVariable::Symbol(symbol);
            if base.variables().contains(&variable) {
                return Err(ParametricCoefficientError::IndexSymbolCollision { position });
            }
            index_variables.push(variable);
        }

        let mut variables = Vec::with_capacity(base.variables().len() + index_count);
        variables.extend(base.variables().iter().cloned());
        variables.extend(index_variables.iter().cloned());
        let variables = Arc::new(variables);
        let template = RationalPolynomial::new(&Z, variables.clone());
        let base_fingerprint: Arc<str> = base_context_fingerprint(base).into();
        let fingerprint: Arc<str> = format!(
            "rustred-parametric-context-v1|base={}|scope={}:{}|indices={index_count}",
            base_fingerprint,
            scope.len(),
            scope
        )
        .into();

        Ok(Self {
            base: base.clone(),
            base_fingerprint,
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
        &self.fingerprint
    }

    pub fn index_count(&self) -> usize {
        self.index_variables.len()
    }

    pub fn contains(&self, value: &ParametricCoefficient) -> bool {
        value.context.as_ref() == self.fingerprint.as_ref()
            && validate_coefficient_on_map(
                &value.raw,
                &self.variables,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn contains_polynomial(&self, value: &ParametricPolynomial) -> bool {
        value.context.as_ref() == self.fingerprint.as_ref()
            && validate_polynomial_on_map(
                &value.raw,
                &self.variables,
                crate::algebra::CoefficientPolynomialPart::Numerator,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn contains_nonzero_condition(&self, value: &ParametricNonZeroCondition) -> bool {
        !value.origins.is_empty() && self.contains_polynomial(&value.polynomial)
    }

    /// Authenticate one polynomial condition and attach one atomic origin.
    pub fn nonzero_condition(
        &self,
        polynomial: ParametricPolynomial,
        origin: GuardOrigin,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.nonzero_condition_with_origins_and_limits(
            polynomial,
            [origin],
            ExactAlgebraLimits::default(),
        )
    }

    /// Authenticate one polynomial condition with an already collected
    /// deterministic origin set.
    ///
    /// The iterator is consumed under the default provenance budget, so an
    /// untrusted or unbounded iterator cannot allocate an unbounded set.  Use
    /// [`Self::nonzero_condition_with_origins_and_origin_limit`] when a caller
    /// needs a stricter budget.
    pub fn nonzero_condition_with_origins_and_limits(
        &self,
        polynomial: ParametricPolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            origins,
            limits,
            ParametricArithmeticLimits::default().max_guard_origins,
        )
    }

    /// Authenticate a condition under independent exact-algebra and
    /// provenance-cardinality budgets.
    pub fn nonzero_condition_with_origins_and_origin_limit(
        &self,
        polynomial: ParametricPolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        limits: ExactAlgebraLimits,
        max_guard_origins: usize,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(&polynomial, limits)?;
        let origins = collect_guard_origins_with_limit(origins, max_guard_origins)?;
        if origins.is_empty() {
            return Err(ParametricCoefficientError::MissingGuardOrigin);
        }
        Ok(ParametricNonZeroCondition {
            polynomial,
            origins,
        })
    }

    pub fn validate_polynomial_with_limits(
        &self,
        value: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        validate_polynomial_on_map(
            &value.raw,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(())
    }

    pub fn contains_base_polynomial(&self, value: &BasePolynomial) -> bool {
        value.context.as_ref() == self.base_fingerprint.as_ref()
            && validate_polynomial_on_map(
                &value.raw,
                self.base.variables(),
                crate::algebra::CoefficientPolynomialPart::Numerator,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn zero(&self) -> ParametricCoefficient {
        self.wrap_unchecked(self.template.numerator.zero().into())
    }

    pub fn one(&self) -> ParametricCoefficient {
        self.wrap_unchecked(self.template.numerator.one().into())
    }

    pub fn integer(&self, value: i64) -> ParametricCoefficient {
        self.wrap_unchecked(
            self.template
                .numerator
                .constant(Integer::from(value))
                .into(),
        )
    }

    pub fn index(
        &self,
        position: usize,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        let variable = self.index_variables.get(position).ok_or(
            ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: position.saturating_add(1),
            },
        )?;
        let polynomial = self
            .template
            .numerator
            .variable(variable)
            .map_err(ParametricCoefficientError::Symbolica)?;
        Ok(self.wrap_unchecked(polynomial.into()))
    }

    pub fn lift(
        &self,
        value: &Coefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        if !self.base.contains(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        let numerator = self.extend_base_polynomial(&value.numerator)?;
        let denominator = self.extend_base_polynomial(&value.denominator)?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
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
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        let raw = self.extend_base_polynomial(value)?;
        Ok(ParametricPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn base_polynomial(
        &self,
        value: CoefficientPolynomial,
    ) -> Result<BasePolynomial, ParametricCoefficientError> {
        validate_polynomial_on_map(
            &value,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        Ok(BasePolynomial {
            raw: value,
            context: self.base_fingerprint.clone(),
        })
    }

    pub fn numerator_condition(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.numerator_condition_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn numerator_condition_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        Ok(ParametricPolynomial {
            raw: value.raw.numerator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub fn denominator_condition(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.denominator_condition_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn denominator_condition_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        Ok(ParametricPolynomial {
            raw: value.raw.denominator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub fn add(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.add_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn add_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_add_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn sub(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.sub_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn sub_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_sub_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn mul(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.mul_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn mul_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_mul_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn neg(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.neg_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn neg_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        let raw = checked_coefficient_neg_on_map(&value.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    /// Apply `n -> n + shift` to a complete coefficient.
    pub fn translate(
        &self,
        value: &ParametricCoefficient,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift)?;
        self.translate_coefficient_validated(value, shift, limits)
    }

    fn translate_coefficient_validated(
        &self,
        value: &ParametricCoefficient,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        let numerator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.numerator, shift, limits)?;
        let denominator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.denominator, shift, limits)?;
        let numerator = self.execute_translate_polynomial_raw(
            &value.raw.numerator,
            shift,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_translate_polynomial_raw(
            &value.raw.denominator,
            shift,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Translation is a polynomial-ring automorphism. The validated
            // source numerator and denominator are coprime, hence their
            // translated images are coprime too. Avoid a redundant native
            // GCD and its otherwise unbounded transient workspace.
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                false,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked parametric translation".to_owned(),
            )
        })?;
        self.wrap_checked_with_limits(raw, limits.exact_algebra)
    }

    pub fn translate_polynomial(
        &self,
        value: &ParametricPolynomial,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift)?;
        Ok(ParametricPolynomial {
            raw: self.translate_polynomial_raw(&value.raw, shift, limits)?,
            context: self.fingerprint.clone(),
        })
    }

    /// Translate a guard polynomial while preserving its source origins and
    /// recording the affine index map that changed its locus.
    pub fn translate_nonzero_condition(
        &self,
        value: &ParametricNonZeroCondition,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_shift(shift)?;
        let already_has_translation = value.origins.iter().any(|origin| {
            matches!(
                origin,
                GuardOrigin::IndexTranslation { offset } if offset.as_ref() == shift
            )
        });
        let final_origin_count = value
            .origins
            .len()
            .checked_add(usize::from(!already_has_translation))
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric guard origins",
            })?;
        check_limit(
            "parametric guard origins",
            final_origin_count,
            limits.max_guard_origins,
        )?;
        let polynomial = self.translate_polynomial(&value.polynomial, shift, limits)?;
        let mut origins = value.origins.clone();
        if !already_has_translation {
            let mut offset = Vec::new();
            offset.try_reserve_exact(shift.len()).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "index translation origin components",
                }
            })?;
            offset.extend_from_slice(shift);
            origins.insert(GuardOrigin::IndexTranslation {
                offset: offset.into_boxed_slice(),
            });
        }
        self.nonzero_condition_with_origins_and_limits(polynomial, origins, limits.exact_algebra)
    }

    /// Simultaneously specialize every index and project the result to the
    /// exact base variable map.  The original mapped denominator is retained
    /// as a nonzero condition even when normalization cancels it.
    pub fn specialize(
        &self,
        value: &ParametricCoefficient,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<GuardedCoefficientSpecialization, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(assignment)?;
        let numerator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.numerator, assignment, limits)?;
        let denominator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.denominator, assignment, limits)?;
        check_coefficient_specialization_normalization_limits(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator_preflight,
            denominator_preflight,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.base.variables().len(),
            limits,
        )?;
        let numerator = self.execute_specialize_polynomial_raw(
            &value.raw.numerator,
            assignment,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_specialize_polynomial_raw(
            &value.raw.denominator,
            assignment,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let mut nonzero = Vec::new();
        let mut guarded_nonzero = Vec::new();
        if !denominator.is_constant() {
            let polynomial = BasePolynomial {
                raw: denominator.clone(),
                context: self.base_fingerprint.clone(),
            };
            let origins = BTreeSet::from([
                GuardOrigin::CoefficientSpecializationDenominator,
                GuardOrigin::IndexSpecialization {
                    assignment: assignment.to_vec().into_boxed_slice(),
                },
            ]);
            check_limit(
                "specialized guard origins",
                origins.len(),
                limits.max_guard_origins,
            )?;
            nonzero.push(polynomial.clone());
            guarded_nonzero.push(SpecializedNonZeroCondition {
                polynomial,
                origins,
            });
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                true,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked coefficient specialization"
                    .to_owned(),
            )
        })?;
        validate_coefficient_on_map(&result, self.base.variables(), limits.exact_algebra)?;
        Ok(GuardedCoefficientSpecialization {
            value: result,
            nonzero,
            guarded_nonzero,
        })
    }

    pub fn specialize_polynomial(
        &self,
        value: &ParametricPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<BasePolynomial, ParametricCoefficientError> {
        self.validate_parametric_polynomial(value)?;
        self.validate_shift(assignment)?;
        Ok(BasePolynomial {
            raw: self.specialize_polynomial_raw(&value.raw, assignment, limits)?,
            context: self.base_fingerprint.clone(),
        })
    }

    /// Specialize one existing parametric condition and retain all source
    /// provenance alongside the exact assignment.
    pub fn specialize_nonzero_condition(
        &self,
        value: &ParametricNonZeroCondition,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<SpecializedNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_shift(assignment)?;
        let polynomial = self.specialize_polynomial(&value.polynomial, assignment, limits)?;
        let mut origins = value.origins.clone();
        origins.insert(GuardOrigin::IndexSpecialization {
            assignment: assignment.to_vec().into_boxed_slice(),
        });
        check_limit(
            "specialized guard origins",
            origins.len(),
            limits.max_guard_origins,
        )?;
        Ok(SpecializedNonZeroCondition {
            polynomial,
            origins,
        })
    }

    pub fn validate_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        validate_coefficient_on_map(&value.raw, &self.variables, limits)?;
        Ok(())
    }

    fn validate_parametric_polynomial(
        &self,
        value: &ParametricPolynomial,
    ) -> Result<(), ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, ExactAlgebraLimits::default())
    }

    fn validate_shift(&self, shift: &[i64]) -> Result<(), ParametricCoefficientError> {
        if shift.len() == self.index_count() {
            Ok(())
        } else {
            Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: shift.len(),
            })
        }
    }

    fn raw_uses_extended_map(&self, raw: &Coefficient) -> bool {
        validate_coefficient_on_map(raw, &self.variables, ExactAlgebraLimits::default()).is_ok()
    }

    fn wrap_unchecked(&self, raw: Coefficient) -> ParametricCoefficient {
        debug_assert!(self.raw_uses_extended_map(&raw));
        ParametricCoefficient {
            raw,
            context: self.fingerprint.clone(),
        }
    }

    fn wrap_checked(
        &self,
        raw: Coefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.wrap_checked_with_limits(raw, ExactAlgebraLimits::default())
    }

    fn wrap_checked_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        validate_coefficient_on_map(&raw, &self.variables, limits)?;
        Ok(self.wrap_unchecked(raw))
    }

    fn extend_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
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

    fn translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let preflight = self.preflight_translate_polynomial_raw(source, shift, limits)?;
        self.execute_translate_polynomial_raw(source, shift, limits, preflight)
    }

    fn preflight_translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialTranslationPreflight, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        let base_count = self.base.variables().len();
        let mut output_term_bound = 0_usize;
        let mut power_operation_bound = 0_usize;
        let mut largest_contribution_bits = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut term_bound = 1_usize;
            for (position, offset) in shift.iter().enumerate() {
                if *offset == 0 {
                    continue;
                }
                let exponent = usize::from(exponents[base_count + position]);
                if exponent != 0 {
                    power_operation_bound = checked_parametric_add(
                        "parametric translation power operations",
                        power_operation_bound,
                        term_bound,
                    )?;
                }
                term_bound = checked_parametric_mul(
                    "parametric translation output terms",
                    term_bound,
                    exponent + 1,
                )?;
            }
            output_term_bound = checked_parametric_add(
                "parametric translation output terms",
                output_term_bound,
                term_bound,
            )?;
            let mut requested = integer_magnitude_bits(coefficient);
            for (position, offset) in shift.iter().enumerate() {
                if *offset == 0 {
                    continue;
                }
                let exponent = u128::from(exponents[base_count + position]);
                if exponent == 0 {
                    continue;
                }
                requested = requested.checked_add(exponent).ok_or(
                    ParametricCoefficientError::ResourceCountOverflow {
                        resource: "parametric translation integer bits",
                    },
                )?;
                let offset_bits = u128::from(i64::BITS - offset.unsigned_abs().leading_zeros());
                if offset_bits > 1 {
                    requested = requested
                        .checked_add(offset_bits.checked_mul(exponent).ok_or(
                            ParametricCoefficientError::ResourceCountOverflow {
                                resource: "parametric translation integer bits",
                            },
                        )?)
                        .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                            resource: "parametric translation integer bits",
                        })?;
                }
            }
            let requested = usize::try_from(requested).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric translation integer bits",
                }
            })?;
            check_limit(
                "parametric translation integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_contribution_bits = largest_contribution_bits.max(requested);
        }
        check_limit(
            "parametric translation output terms",
            output_term_bound,
            limits.exact_algebra.max_polynomial_terms,
        )?;
        check_limit(
            "parametric translation power operations",
            power_operation_bound,
            limits.max_specialization_power_operations,
        )?;

        // Expanding (n+a)^e produces coefficients containing binomial(e,k)
        // and powers of `a`. For each contribution use binomial(e,k) <= 2^e,
        // then charge ceil(log2(output_term_bound)) for worst-case collection
        // of equal monomials.
        let collision_bits = parametric_ceil_log2(output_term_bound);
        let collected_bits = largest_contribution_bits
            .checked_add(collision_bits)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric translation integer bits",
            })?;
        check_limit(
            "parametric translation integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        let output_exponent_entry_bound = checked_parametric_mul(
            "parametric translation output exponent entries",
            output_term_bound,
            self.variables.len(),
        )?;
        Ok(ParametricPolynomialTranslationPreflight {
            output_term_bound,
            output_exponent_entry_bound,
            largest_output_integer_bit_bound: collected_bits,
        })
    }

    fn execute_translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
        preflight: ParametricPolynomialTranslationPreflight,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let mut result = source.clone();
        let base_count = self.base.variables().len();
        for (position, offset) in shift.iter().enumerate() {
            if *offset == 0 {
                continue;
            }
            let variable_position = base_count + position;
            if !source
                .exponents_iter()
                .any(|exponents| exponents[variable_position] != 0)
            {
                // The preflight correctly charges no offset bits when this
                // index is absent.
                continue;
            }
            let variable = self
                .template
                .numerator
                .variable(&self.index_variables[position])
                .map_err(ParametricCoefficientError::Symbolica)?;
            result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let replacement =
                    &variable + &self.template.numerator.constant(Integer::from(*offset));
                result.replace_with_poly(variable_position, &replacement)
            }))
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "Symbolica panicked during checked parametric translation".to_owned(),
                )
            })?;
        }
        if result.variables.as_ref() != self.variables.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "parametric translation",
        )?;
        validate_polynomial_on_map(
            &result,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }

    fn specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let preflight = self.preflight_specialize_polynomial_raw(source, assignment, limits)?;
        self.execute_specialize_polynomial_raw(source, assignment, limits, preflight)
    }

    fn preflight_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialSpecializationPreflight, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        check_limit(
            "coefficient specialization output terms",
            source.nterms(),
            limits.exact_algebra.max_polynomial_terms,
        )?;
        let operations = source.nterms().checked_mul(self.index_count()).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "coefficient specialization power operations",
            },
        )?;
        check_limit(
            "coefficient specialization power operations",
            operations,
            limits.max_specialization_power_operations,
        )?;

        let base_count = self.base.variables().len();
        // Preflight every arbitrary-precision power before constructing any
        // output coefficient.  Counting calls alone is insufficient:
        // `value^exponent` can allocate an integer linear in `exponent` bits
        // even when the source polynomial contains only one term.
        let mut largest_term_bits = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let requested =
                specialization_integer_bit_bound(coefficient, exponents, base_count, assignment)?;
            check_limit(
                "coefficient specialization integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_term_bits = largest_term_bits.max(requested);
        }
        let collision_bits = parametric_ceil_log2(source.nterms());
        let collected_bits = largest_term_bits.checked_add(collision_bits).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "coefficient specialization integer bits",
            },
        )?;
        check_limit(
            "coefficient specialization integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        let output_exponent_entry_bound = checked_parametric_mul(
            "coefficient specialization output exponent entries",
            source.nterms(),
            base_count,
        )?;
        Ok(ParametricPolynomialSpecializationPreflight {
            output_term_bound: source.nterms(),
            output_exponent_entry_bound,
            largest_output_integer_bit_bound: collected_bits,
        })
    }

    fn execute_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
        preflight: ParametricPolynomialSpecializationPreflight,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let base_count = self.base.variables().len();
        let mut result = self
            .base
            .template()
            .numerator
            .zero_with_capacity(source.nterms());
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut specialized = coefficient.clone();
            for (position, value) in assignment.iter().copied().enumerate() {
                let exponent = exponents[base_count + position];
                if exponent != 0 {
                    specialized = specialized * Integer::from(value).pow(u64::from(exponent));
                }
            }
            result.append_monomial(specialized, &exponents[..base_count]);
        }
        if result.variables.as_ref() != self.base.variables().as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "coefficient specialization",
        )?;
        validate_polynomial_on_map(
            &result,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }
}

fn check_coefficient_specialization_normalization_limits(
    numerator_source: &CoefficientPolynomial,
    denominator_source: &CoefficientPolynomial,
    numerator: ParametricPolynomialSpecializationPreflight,
    denominator: ParametricPolynomialSpecializationPreflight,
    numerator_is_zero: bool,
    denominator_is_one: bool,
    variable_count: usize,
    limits: ParametricArithmeticLimits,
) -> Result<(), ParametricCoefficientError> {
    let normalization_input_term_pairs = checked_parametric_mul(
        "coefficient specialization normalization input term pairs",
        numerator.output_term_bound.max(1),
        denominator.output_term_bound,
    )?;
    check_limit(
        "coefficient specialization normalization input term pairs",
        normalization_input_term_pairs,
        limits.exact_algebra.max_term_operations,
    )?;

    let (numerator_bits, denominator_bits) = if numerator_is_zero || denominator_is_one {
        (
            numerator.largest_output_integer_bit_bound,
            denominator.largest_output_integer_bit_bound,
        )
    } else {
        let term_cap = limits.exact_algebra.max_polynomial_terms;
        (
            normalized_factor_envelope_from_source(
                numerator_source,
                0,
                variable_count,
                numerator.output_term_bound,
                numerator.largest_output_integer_bit_bound,
                term_cap,
                "coefficient specialization normalized numerator support",
            )?
            .1,
            normalized_factor_envelope_from_source(
                denominator_source,
                0,
                variable_count,
                denominator.output_term_bound,
                denominator.largest_output_integer_bit_bound,
                term_cap,
                "coefficient specialization normalized denominator support",
            )?
            .1,
        )
    };
    check_limit(
        "coefficient specialization normalized integer bits",
        numerator_bits.max(denominator_bits),
        limits.max_specialization_integer_bits,
    )
}

fn normalized_factor_envelope_from_source(
    source: &CoefficientPolynomial,
    first_variable: usize,
    variable_count: usize,
    mapped_term_bound: usize,
    mapped_integer_bit_bound: usize,
    successful_term_cap: usize,
    resource: &'static str,
) -> Result<(usize, usize), ParametricCoefficientError> {
    if source.is_zero() {
        return Ok((0, 0));
    }
    // A mixed-radix Kronecker image with radices degree_i+1 is injective on
    // every possible factor. Its degree is support_size-1. The univariate
    // Landau-Mignotte factor-height bound then gives
    //   bits(factor) <= bits(input) + degree + ceil(log2(input terms)).
    // This is intentionally coarse, but it remains finite, allocation-free,
    // and sound even when exact GCD division turns a sparse input into a dense
    // quotient such as (x^n-1)/(x-1).
    let mut support_size = 1usize;
    let variable_end = first_variable
        .checked_add(variable_count)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })?;
    if variable_end > source.variables.len() {
        return Err(ParametricCoefficientError::WrongContext);
    }
    for variable in first_variable..variable_end {
        let mut degree = 0usize;
        for exponents in source.exponents_iter() {
            degree = degree.max(usize::from(exponents[variable]));
        }
        support_size = checked_parametric_mul(resource, support_size, degree + 1)?;
    }
    // Exact division may materialize every monomial in this support before
    // the post-normalization authenticator sees the result. Reject the dense
    // support prospectively; `min(successful_term_cap)` would only turn the
    // configured term cap into a post-allocation publication gate.
    check_limit(resource, support_size, successful_term_cap)?;
    let term_bound = support_size;
    let integer_bit_bound = checked_parametric_add(
        resource,
        mapped_integer_bit_bound.max(1),
        checked_parametric_add(
            resource,
            support_size.saturating_sub(1),
            parametric_ceil_log2(mapped_term_bound),
        )?,
    )?;
    Ok((term_bound, integer_bit_bound))
}

fn verify_polynomial_execution_envelope(
    polynomial: &CoefficientPolynomial,
    term_bound: usize,
    exponent_entry_bound: usize,
    integer_bit_bound: usize,
    operation: &'static str,
) -> Result<(), ParametricCoefficientError> {
    if polynomial.nterms() > term_bound
        || polynomial.exponents.len() > exponent_entry_bound
        || polynomial
            .coefficients
            .iter()
            .any(|coefficient| integer_magnitude_bits(coefficient) > integer_bit_bound as u128)
    {
        return Err(ParametricCoefficientError::Symbolica(format!(
            "{operation} escaped its allocation-free preflight envelope"
        )));
    }
    Ok(())
}

fn checked_parametric_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricCoefficientError> {
    left.checked_add(right)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn checked_parametric_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricCoefficientError> {
    left.checked_mul(right)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn parametric_ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn specialization_integer_bit_bound(
    coefficient: &Integer,
    exponents: &[u16],
    base_count: usize,
    assignment: &[i64],
) -> Result<usize, ParametricCoefficientError> {
    let mut requested = integer_magnitude_bits(coefficient);
    if requested == 0 {
        return Ok(0);
    }
    for (position, value) in assignment.iter().copied().enumerate() {
        let exponent = exponents[base_count + position];
        if exponent == 0 {
            continue;
        }
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Ok(0);
        }
        // Multiplication by (+/-1)^e does not grow the coefficient.  For all
        // other bases, e*bit_length(base) is a conservative bit bound for the
        // power and hence for its contribution to the product.
        if magnitude != 1 {
            let value_bits = u128::from(u64::BITS - magnitude.leading_zeros());
            let power_bits = value_bits.checked_mul(u128::from(exponent)).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
            requested = requested.checked_add(power_bits).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
        }
    }
    usize::try_from(requested).map_err(|_| ParametricCoefficientError::ResourceCountOverflow {
        resource: "coefficient specialization integer bits",
    })
}

fn integer_magnitude_bits(value: &Integer) -> u128 {
    match value {
        Integer::Single(value) => {
            let magnitude = value.unsigned_abs();
            u128::from(i64::BITS - magnitude.leading_zeros())
        }
        Integer::Double(value) => {
            let magnitude = value.unsigned_abs();
            u128::from(i128::BITS - magnitude.leading_zeros())
        }
        Integer::Large(value) => u128::from(value.significant_bits()),
    }
}

pub(crate) fn insert_parametric_condition(
    conditions: &mut Vec<ParametricNonZeroCondition>,
    condition: ParametricNonZeroCondition,
    max_guard_origins: usize,
) -> Result<(), ParametricCoefficientError> {
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        existing.merge_origins_from(&condition, max_guard_origins)
    } else {
        check_limit(
            "parametric guard origins",
            condition.origins.len(),
            max_guard_origins,
        )?;
        conditions.push(condition);
        Ok(())
    }
}

pub(crate) fn insert_specialized_condition(
    conditions: &mut Vec<SpecializedNonZeroCondition>,
    condition: SpecializedNonZeroCondition,
    max_guard_origins: usize,
) -> Result<(), ParametricCoefficientError> {
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        existing.merge_origins_from(&condition, max_guard_origins)
    } else {
        check_limit(
            "specialized guard origins",
            condition.origins.len(),
            max_guard_origins,
        )?;
        conditions.push(condition);
        Ok(())
    }
}

fn collect_guard_origins_with_limit(
    origins: impl IntoIterator<Item = GuardOrigin>,
    max_guard_origins: usize,
) -> Result<BTreeSet<GuardOrigin>, ParametricCoefficientError> {
    let mut collected = BTreeSet::new();
    for (position, origin) in origins.into_iter().enumerate() {
        let requested =
            position
                .checked_add(1)
                .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric guard origin inputs",
                })?;
        check_limit(
            "parametric guard origin inputs",
            requested,
            max_guard_origins,
        )?;
        collected.insert(origin);
    }
    check_limit(
        "parametric guard origins",
        collected.len(),
        max_guard_origins,
    )?;
    Ok(collected)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricCoefficientError> {
    if requested > limit {
        Err(ParametricCoefficientError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn encode_symbol_component(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn base_context_fingerprint(base: &CoefficientContext) -> String {
    let mut result = format!(
        "rustred-base-context-v1|parameters={}",
        base.parameter_names().len()
    );
    for name in base.parameter_names() {
        result.push('|');
        result.push_str(&name.len().to_string());
        result.push(':');
        result.push_str(name);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_field_may_be_q_and_indices_remain_distinct() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "empty-base", 2).unwrap();
        assert_eq!(base.parameter_names(), &[] as &[String]);
        assert_eq!(context.index_count(), 2);
        assert!(context.contains(&context.index(0).unwrap()));
    }

    #[test]
    fn specialized_nonzero_condition_rejects_empty_provenance() {
        let base = CoefficientContext::new(["x"]);
        let polynomial = BasePolynomial::try_from_raw(
            base.parameter("x").unwrap().numerator.clone(),
            &base,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            SpecializedNonZeroCondition::from_base_polynomial(
                polynomial,
                Vec::<GuardOrigin>::new(),
                1,
            ),
            Err(ParametricCoefficientError::MissingGuardOrigin)
        ));
    }

    #[test]
    fn translation_guard_origin_limit_precedes_polynomial_translation() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "translation-origin-preflight", 1)
                .unwrap();
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = context
            .nonzero_condition_with_origins_and_limits(
                polynomial,
                [GuardOrigin::ExplicitRelationCondition],
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let limits = ParametricArithmeticLimits {
            exact_algebra: ExactAlgebraLimits {
                max_polynomial_terms: 0,
                ..ExactAlgebraLimits::default()
            },
            max_guard_origins: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            context.translate_nonzero_condition(&condition, &[1], limits),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "parametric guard origins",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn lift_translate_and_specialize_preserve_authenticated_maps() {
        let base = CoefficientContext::new(["d", "m2"]);
        let context = ParametricCoefficientContext::try_new(&base, "translation", 2).unwrap();
        let d = base.parameter("d").unwrap();
        let m2 = base.parameter("m2").unwrap();
        let family_value = &(&d + &base.integer(1)) / &m2;
        let lifted = context.lift(&family_value).unwrap();
        let n0 = context.index(0).unwrap();
        let value = context.mul(&n0, &lifted).unwrap();
        let translated = context
            .translate(&value, &[2, -3], ParametricArithmeticLimits::default())
            .unwrap();
        let specialized = context
            .specialize(
                &translated,
                &[5, 100],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let expected = &base.integer(7) * &family_value;
        assert_eq!(specialized.value, expected);
        assert_eq!(specialized.nonzero.len(), 1);
        assert_eq!(specialized.nonzero[0].to_expression(), m2.to_expression());
    }

    #[test]
    fn specialization_retains_a_cancelled_index_dependent_pole() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "cancelled-pole", 1).unwrap();
        let n = context.index(0).unwrap();
        let one = context.one();
        let n_minus_one = context.sub(&n, &one).unwrap();
        let fabricated = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: n_minus_one.raw.numerator.clone(),
                denominator: n_minus_one.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let generic = context
            .specialize(&fabricated, &[2], ParametricArithmeticLimits::default())
            .unwrap();
        assert_eq!(generic.value, base.one());
        assert!(
            generic.nonzero.is_empty(),
            "constant nonzero guards are tautologies"
        );
        assert!(matches!(
            context.specialize(&fabricated, &[1], ParametricArithmeticLimits::default(),),
            Err(ParametricCoefficientError::ZeroDenominator)
        ));
    }

    #[test]
    fn rejects_foreign_maps_before_symbolica_can_unify_them() {
        let base = CoefficientContext::new(["d"]);
        let foreign = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "strict-map", 1).unwrap();
        assert!(matches!(
            context.lift(&foreign.one()),
            Err(ParametricCoefficientError::WrongContext)
        ));
        assert!(matches!(
            context.translate(&context.one(), &[], ParametricArithmeticLimits::default()),
            Err(ParametricCoefficientError::WrongIndexArity { .. })
        ));
    }

    #[test]
    fn parametric_authentication_rejects_malformed_layout_before_arithmetic() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "malformed", 1).unwrap();
        let mut malformed = context.one();
        malformed.raw.numerator.exponents.push(0);

        assert!(!context.contains(&malformed));
        assert!(matches!(
            context.add(&malformed, &context.one()),
            Err(ParametricCoefficientError::ExactAlgebra(
                ExactAlgebraError::MalformedExponentLayout { .. }
            ))
        ));
    }
}
