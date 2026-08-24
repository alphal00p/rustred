//! Checked specialization between exact kinematic base fields.
//!
//! A [`BaseKinematicSpecialization`] is an authenticated, ordered map
//! `K_source -> K_target` from every named source parameter to one exact
//! rational function in the target context.  The target may have no
//! parameters, in which case it is the exact field `Q`.
//!
//! Rational coefficients are evaluated as two independent source
//! polynomials.  The mapped source denominator is checked before the final
//! fraction-field division, and its numerator is retained as a typed nonzero
//! guard even if normalization later cancels it.  Denominators of parameter
//! images are retained for the same reason.
//!
//! This module deliberately does not construct a specialized
//! [`IntegralFamily`](crate::generic_family::IntegralFamily).  Evaluating a
//! family's domain can prove that a kinematic point is applicable, conditional,
//! or inapplicable, but that alone does not replay and authenticate every
//! cached family object in a new coefficient context.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use symbolica::prelude::{IntegerRing, MultivariatePolynomial};

use crate::coefficient::{
    Coefficient, CoefficientContext, CoefficientPolynomialPart, ExactAlgebraError,
    ExactAlgebraLimits, validate_polynomial_on_map,
};
use crate::generic_family::{BaseNonZeroCondition, CoefficientLocation, IntegralFamily};
use crate::guards::GuardOrigin;

/// A polynomial over the target base-field parameters.
///
/// This raw alias carries Symbolica's variable map but not RustRed's parameter
/// labels. Do not combine values obtained from unrelated specializations.
/// [`BaseSpecializationGuard`] retains the target label manifest and can be
/// re-authenticated with [`BaseKinematicSpecialization::authenticate_guard`].
pub type SpecializedBasePolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// Bounded work policy for one base-field specialization map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseSpecializationLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_parameter_images: usize,
    pub max_source_terms: usize,
    pub max_evaluation_operations: usize,
    pub max_guards: usize,
    pub max_guard_origins: usize,
    pub max_family_domain_conditions: usize,
}

impl Default for BaseSpecializationLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_parameter_images: 4_096,
            max_source_terms: 4_000_000,
            max_evaluation_operations: 16_000_000,
            max_guards: 1_000_000,
            max_guard_origins: 1_000_000,
            max_family_domain_conditions: 1_000_000,
        }
    }
}

/// One explicitly named image in the ordered source-parameter manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseParameterImage {
    source_parameter: String,
    image: Coefficient,
}

impl BaseParameterImage {
    pub fn new(source_parameter: impl Into<String>, image: Coefficient) -> Self {
        Self {
            source_parameter: source_parameter.into(),
            image,
        }
    }

    pub fn source_parameter(&self) -> &str {
        &self.source_parameter
    }

    pub fn image(&self) -> &Coefficient {
        &self.image
    }
}

/// Caller-visible provenance for an independently specialized coefficient.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BaseCoefficientProvenance {
    Named(String),
    Family(CoefficientLocation),
}

impl BaseCoefficientProvenance {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

/// All authenticated provenance of one family-domain polynomial.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FamilyDomainConditionSource {
    location: CoefficientLocation,
    origins: BTreeSet<GuardOrigin>,
}

impl FamilyDomainConditionSource {
    pub fn location(&self) -> &CoefficientLocation {
        &self.location
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }
}

/// Exact origin of a target-field nonzero guard.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BaseSpecializationGuardProvenance {
    ParameterImageDenominator {
        source_parameter_index: usize,
        source_parameter: String,
    },
    MappedCoefficientDenominator {
        source: BaseCoefficientProvenance,
    },
    FamilyDomainOrigin {
        origin: GuardOrigin,
    },
}

/// A target polynomial that must remain nonzero for a mapped value to apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseSpecializationGuard {
    origins: BTreeSet<BaseSpecializationGuardProvenance>,
    polynomial: SpecializedBasePolynomial,
    target_parameters: Arc<[String]>,
}

impl BaseSpecializationGuard {
    pub fn origins(&self) -> &BTreeSet<BaseSpecializationGuardProvenance> {
        &self.origins
    }

    pub fn polynomial(&self) -> &SpecializedBasePolynomial {
        &self.polynomial
    }

    pub fn target_parameters(&self) -> &[String] {
        &self.target_parameters
    }
}

/// One exact coefficient in the target field plus every pre-cancellation
/// nonzero condition needed by its source-to-target evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedBaseCoefficient {
    value: Coefficient,
    guards: Vec<BaseSpecializationGuard>,
}

impl GuardedBaseCoefficient {
    pub fn value(&self) -> &Coefficient {
        &self.value
    }

    pub fn guards(&self) -> &[BaseSpecializationGuard] {
        &self.guards
    }
}

/// Exact disposition of a family domain under one base specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyDomainEvaluationStatus {
    /// Every mapped condition is a provably nonzero constant.
    Applicable,
    /// No mapped condition vanishes identically, but target-polynomial guards
    /// remain to be imposed.
    Conditional,
    /// At least one required input denominator or basis determinant maps to
    /// the zero rational function.
    Inapplicable,
}

/// A required family-domain condition that mapped identically to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InapplicableFamilyDomainCondition {
    source: FamilyDomainConditionSource,
}

impl InapplicableFamilyDomainCondition {
    pub fn source(&self) -> &FamilyDomainConditionSource {
        &self.source
    }
}

/// Checked evaluation of [`crate::generic_family::FamilyDomain`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyDomainEvaluation {
    status: FamilyDomainEvaluationStatus,
    guards: Vec<BaseSpecializationGuard>,
    zero_conditions: Vec<InapplicableFamilyDomainCondition>,
}

impl FamilyDomainEvaluation {
    pub const fn status(&self) -> FamilyDomainEvaluationStatus {
        self.status
    }

    pub fn guards(&self) -> &[BaseSpecializationGuard] {
        &self.guards
    }

    pub fn zero_conditions(&self) -> &[InapplicableFamilyDomainCondition] {
        &self.zero_conditions
    }
}

/// Typed construction, authentication, evaluation, and domain failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseSpecializationError {
    WrongImageCount {
        expected: usize,
        actual: usize,
    },
    SourceParameterMismatch {
        position: usize,
        expected: String,
        actual: String,
    },
    InvalidSourceContext(ExactAlgebraError),
    InvalidTargetContext(ExactAlgebraError),
    InvalidParameterImage {
        position: usize,
        source_parameter: String,
        error: ExactAlgebraError,
    },
    ForeignSourceCoefficient {
        source: BaseCoefficientProvenance,
        error: ExactAlgebraError,
    },
    ForeignFamilyContext,
    InvalidSourcePolynomial {
        part: CoefficientPolynomialPart,
        error: ExactAlgebraError,
    },
    InvalidFamilyCondition {
        source: FamilyDomainConditionSource,
        error: ExactAlgebraError,
    },
    FamilyDeterminantConditionMismatch,
    MappedCoefficientDenominatorZero {
        source: BaseCoefficientProvenance,
    },
    InapplicableFamilyDomain {
        zero_conditions: Vec<InapplicableFamilyDomainCondition>,
    },
    ForeignTargetGuard,
    ExactEvaluation {
        stage: &'static str,
        error: ExactAlgebraError,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for BaseSpecializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongImageCount { expected, actual } => write!(
                formatter,
                "base specialization needs {expected} parameter images, received {actual}"
            ),
            Self::SourceParameterMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "base specialization image {position} names source parameter {actual:?}, expected {expected:?}"
            ),
            Self::InvalidSourceContext(error) => {
                write!(formatter, "invalid source coefficient context: {error}")
            }
            Self::InvalidTargetContext(error) => {
                write!(formatter, "invalid target coefficient context: {error}")
            }
            Self::InvalidParameterImage {
                position,
                source_parameter,
                error,
            } => write!(
                formatter,
                "invalid image {position} of source parameter {source_parameter:?}: {error}"
            ),
            Self::ForeignSourceCoefficient { source, error } => {
                write!(
                    formatter,
                    "coefficient {source:?} is not in the source base field: {error}"
                )
            }
            Self::ForeignFamilyContext => formatter.write_str(
                "integral family and base specialization use different source coefficient contexts",
            ),
            Self::InvalidSourcePolynomial { part, error } => {
                write!(formatter, "invalid source {part} polynomial: {error}")
            }
            Self::InvalidFamilyCondition { source, error } => {
                write!(
                    formatter,
                    "invalid family-domain condition {source:?}: {error}"
                )
            }
            Self::FamilyDeterminantConditionMismatch => formatter.write_str(
                "family determinant condition is not the authenticated basis-determinant numerator",
            ),
            Self::MappedCoefficientDenominatorZero { source } => write!(
                formatter,
                "the original denominator of coefficient {source:?} maps identically to zero"
            ),
            Self::InapplicableFamilyDomain { zero_conditions } => write!(
                formatter,
                "base specialization violates {} family-domain condition(s)",
                zero_conditions.len()
            ),
            Self::ForeignTargetGuard => formatter.write_str(
                "base-specialization guard does not use this specialization's target context",
            ),
            Self::ExactEvaluation { stage, error } => {
                write!(
                    formatter,
                    "exact base specialization failed during {stage}: {error}"
                )
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
        }
    }
}

impl std::error::Error for BaseSpecializationError {}

/// Authenticated exact substitution from one kinematic base field to another.
#[derive(Clone, Debug)]
pub struct BaseKinematicSpecialization {
    source: CoefficientContext,
    target: CoefficientContext,
    images: Vec<BaseParameterImage>,
    image_denominator_guards: Vec<BaseSpecializationGuard>,
    target_parameter_manifest: Arc<[String]>,
    limits: BaseSpecializationLimits,
}

impl BaseKinematicSpecialization {
    /// Authenticate an ordered, explicitly named source-parameter map.
    pub fn new(
        source: CoefficientContext,
        target: CoefficientContext,
        images: Vec<BaseParameterImage>,
    ) -> Result<Self, BaseSpecializationError> {
        Self::new_with_limits(source, target, images, BaseSpecializationLimits::default())
    }

    pub fn new_with_limits(
        source: CoefficientContext,
        target: CoefficientContext,
        images: Vec<BaseParameterImage>,
        limits: BaseSpecializationLimits,
    ) -> Result<Self, BaseSpecializationError> {
        source
            .validate_with_limits(&source.one(), limits.exact_algebra)
            .map_err(BaseSpecializationError::InvalidSourceContext)?;
        target
            .validate_with_limits(&target.one(), limits.exact_algebra)
            .map_err(BaseSpecializationError::InvalidTargetContext)?;

        if images.len() != source.parameter_names().len() {
            return Err(BaseSpecializationError::WrongImageCount {
                expected: source.parameter_names().len(),
                actual: images.len(),
            });
        }
        check_limit(
            "base-specialization parameter images",
            images.len(),
            limits.max_parameter_images,
        )?;

        let target_parameter_manifest: Arc<[String]> = target.parameter_names().to_vec().into();
        let mut image_denominator_guards = Vec::new();
        for (position, image) in images.iter().enumerate() {
            let expected = &source.parameter_names()[position];
            if image.source_parameter != *expected {
                return Err(BaseSpecializationError::SourceParameterMismatch {
                    position,
                    expected: expected.clone(),
                    actual: image.source_parameter.clone(),
                });
            }
            target
                .validate_with_limits(&image.image, limits.exact_algebra)
                .map_err(|error| BaseSpecializationError::InvalidParameterImage {
                    position,
                    source_parameter: expected.clone(),
                    error,
                })?;
            if !image.image.denominator.is_constant() {
                merge_guard_checked(
                    &mut image_denominator_guards,
                    make_guard(
                        &target_parameter_manifest,
                        image.image.denominator.clone(),
                        [
                            BaseSpecializationGuardProvenance::ParameterImageDenominator {
                                source_parameter_index: position,
                                source_parameter: expected.clone(),
                            },
                        ],
                        limits,
                    )?,
                    limits,
                )?;
            }
        }

        Ok(Self {
            source,
            target,
            images,
            image_denominator_guards,
            target_parameter_manifest,
            limits,
        })
    }

    pub fn source_context(&self) -> &CoefficientContext {
        &self.source
    }

    pub fn target_context(&self) -> &CoefficientContext {
        &self.target
    }

    pub fn images(&self) -> &[BaseParameterImage] {
        &self.images
    }

    pub const fn limits(&self) -> BaseSpecializationLimits {
        self.limits
    }

    /// Re-authenticate a guard before composing it with another result.
    pub fn authenticate_guard(
        &self,
        guard: &BaseSpecializationGuard,
    ) -> Result<(), BaseSpecializationError> {
        if guard.target_parameters.as_ref() != self.target.parameter_names()
            || validate_polynomial_on_map(
                &guard.polynomial,
                self.target.variables(),
                CoefficientPolynomialPart::Numerator,
                self.limits.exact_algebra,
            )
            .is_err()
            || guard.polynomial.is_zero()
        {
            Err(BaseSpecializationError::ForeignTargetGuard)
        } else {
            Ok(())
        }
    }

    /// Evaluate a rational source coefficient with an explicit provenance.
    ///
    /// Its numerator and denominator are mapped independently.  A zero mapped
    /// denominator is rejected before normalization.  Otherwise the numerator
    /// of that mapped denominator is retained as a guard whenever it is not a
    /// provably nonzero constant.
    pub fn evaluate_coefficient(
        &self,
        coefficient: &Coefficient,
        source: BaseCoefficientProvenance,
    ) -> Result<GuardedBaseCoefficient, BaseSpecializationError> {
        self.source
            .validate_with_limits(coefficient, self.limits.exact_algebra)
            .map_err(|error| BaseSpecializationError::ForeignSourceCoefficient {
                source: source.clone(),
                error,
            })?;

        let mapped_numerator =
            self.evaluate_polynomial(&coefficient.numerator, CoefficientPolynomialPart::Numerator)?;
        let mapped_denominator = self.evaluate_polynomial(
            &coefficient.denominator,
            CoefficientPolynomialPart::Denominator,
        )?;
        if mapped_denominator.is_zero() {
            return Err(BaseSpecializationError::MappedCoefficientDenominatorZero { source });
        }

        let mut guards = self.image_denominator_guards.clone();
        if !mapped_denominator.numerator.is_constant() {
            merge_guard_checked(
                &mut guards,
                make_guard(
                    &self.target_parameter_manifest,
                    mapped_denominator.numerator.clone(),
                    [
                        BaseSpecializationGuardProvenance::MappedCoefficientDenominator {
                            source: source.clone(),
                        },
                    ],
                    self.limits,
                )?,
                self.limits,
            )?;
        }

        let value = self
            .target
            .try_div(
                &mapped_numerator,
                &mapped_denominator,
                self.limits.exact_algebra,
            )
            .map_err(|error| BaseSpecializationError::ExactEvaluation {
                stage: "final checked division",
                error,
            })?;
        self.target
            .validate_with_limits(&value, self.limits.exact_algebra)
            .map_err(|error| BaseSpecializationError::ExactEvaluation {
                stage: "target coefficient authentication",
                error,
            })?;
        Ok(GuardedBaseCoefficient { value, guards })
    }

    /// Evaluate the exact domain of a complete authenticated family.
    ///
    /// This method authenticates that `family` uses this map's source context.
    /// It evaluates only domain validity and intentionally does not return a
    /// specialized family object.
    pub fn evaluate_family_domain(
        &self,
        family: &IntegralFamily,
    ) -> Result<FamilyDomainEvaluation, BaseSpecializationError> {
        if !self
            .source
            .has_same_variable_map(family.coefficient_context())
        {
            return Err(BaseSpecializationError::ForeignFamilyContext);
        }
        let determinant_source = family_condition_source(family.domain().determinant_nonzero());
        self.source
            .validate_with_limits(
                family.domain().basis_determinant(),
                self.limits.exact_algebra,
            )
            .map_err(|error| BaseSpecializationError::InvalidFamilyCondition {
                source: determinant_source,
                error,
            })?;
        if family.domain().determinant_nonzero().polynomial()
            != &family.domain().basis_determinant().numerator
        {
            return Err(BaseSpecializationError::FamilyDeterminantConditionMismatch);
        }

        let mut guards = self.image_denominator_guards.clone();
        let mut zero_conditions = Vec::new();
        let condition_count = family.domain().conditions().count();
        check_limit(
            "base-specialization family-domain conditions",
            condition_count,
            self.limits.max_family_domain_conditions,
        )?;
        for condition in family.domain().conditions() {
            self.evaluate_domain_condition(
                condition,
                family_condition_source(condition),
                &mut guards,
                &mut zero_conditions,
            )?;
        }

        let status = if !zero_conditions.is_empty() {
            FamilyDomainEvaluationStatus::Inapplicable
        } else if guards.is_empty() {
            FamilyDomainEvaluationStatus::Applicable
        } else {
            FamilyDomainEvaluationStatus::Conditional
        };
        Ok(FamilyDomainEvaluation {
            status,
            guards,
            zero_conditions,
        })
    }

    /// Evaluate and explicitly reject any point at which an input denominator
    /// or the required denominator-basis determinant maps to zero.
    pub fn require_family_domain(
        &self,
        family: &IntegralFamily,
    ) -> Result<FamilyDomainEvaluation, BaseSpecializationError> {
        let evaluation = self.evaluate_family_domain(family)?;
        if evaluation.status == FamilyDomainEvaluationStatus::Inapplicable {
            Err(BaseSpecializationError::InapplicableFamilyDomain {
                zero_conditions: evaluation.zero_conditions,
            })
        } else {
            Ok(evaluation)
        }
    }

    fn evaluate_domain_condition(
        &self,
        condition: &BaseNonZeroCondition,
        source: FamilyDomainConditionSource,
        guards: &mut Vec<BaseSpecializationGuard>,
        zero_conditions: &mut Vec<InapplicableFamilyDomainCondition>,
    ) -> Result<(), BaseSpecializationError> {
        validate_polynomial_on_map(
            condition.polynomial(),
            self.source.variables(),
            CoefficientPolynomialPart::Numerator,
            self.limits.exact_algebra,
        )
        .map_err(|error| BaseSpecializationError::InvalidFamilyCondition {
            source: source.clone(),
            error,
        })?;
        let mapped =
            self.evaluate_polynomial(condition.polynomial(), CoefficientPolynomialPart::Numerator)?;
        if mapped.is_zero() {
            merge_zero_condition(zero_conditions, source, self.limits)?;
        } else if !mapped.numerator.is_constant() {
            let origins = source
                .origins
                .iter()
                .cloned()
                .map(|origin| BaseSpecializationGuardProvenance::FamilyDomainOrigin { origin });
            merge_guard_checked(
                guards,
                make_guard(
                    &self.target_parameter_manifest,
                    mapped.numerator.clone(),
                    origins,
                    self.limits,
                )?,
                self.limits,
            )?;
        }
        Ok(())
    }

    fn evaluate_polynomial(
        &self,
        polynomial: &SpecializedBasePolynomial,
        part: CoefficientPolynomialPart,
    ) -> Result<Coefficient, BaseSpecializationError> {
        validate_polynomial_on_map(
            polynomial,
            self.source.variables(),
            part,
            self.limits.exact_algebra,
        )
        .map_err(|error| BaseSpecializationError::InvalidSourcePolynomial { part, error })?;
        check_limit(
            "base-specialization source polynomial terms",
            polynomial.nterms(),
            self.limits.max_source_terms,
        )?;

        let mut operations = 0_usize;
        let mut result = self.target.zero();
        for (integer, exponents) in polynomial
            .coefficients
            .iter()
            .zip(polynomial.exponents_iter())
        {
            let integer_polynomial = self.target.template().numerator.constant(integer.clone());
            let mut term = Coefficient::from(integer_polynomial);
            for (image, &exponent) in self.images.iter().zip(exponents) {
                if exponent == 0 {
                    continue;
                }
                let power = self.checked_power(&image.image, exponent, &mut operations)?;
                charge_operation(&mut operations, self.limits)?;
                term = self
                    .target
                    .try_mul(&term, &power, self.limits.exact_algebra)
                    .map_err(|error| BaseSpecializationError::ExactEvaluation {
                        stage: "monomial image multiplication",
                        error,
                    })?;
            }
            charge_operation(&mut operations, self.limits)?;
            result = self
                .target
                .try_add(&result, &term, self.limits.exact_algebra)
                .map_err(|error| BaseSpecializationError::ExactEvaluation {
                    stage: "mapped polynomial term collection",
                    error,
                })?;
        }
        self.target
            .validate_with_limits(&result, self.limits.exact_algebra)
            .map_err(|error| BaseSpecializationError::ExactEvaluation {
                stage: "mapped polynomial authentication",
                error,
            })?;
        Ok(result)
    }

    fn checked_power(
        &self,
        base: &Coefficient,
        exponent: u16,
        operations: &mut usize,
    ) -> Result<Coefficient, BaseSpecializationError> {
        let mut result = self.target.one();
        let mut factor = base.clone();
        let mut remaining = exponent;
        while remaining != 0 {
            if remaining & 1 == 1 {
                charge_operation(operations, self.limits)?;
                result = self
                    .target
                    .try_mul(&result, &factor, self.limits.exact_algebra)
                    .map_err(|error| BaseSpecializationError::ExactEvaluation {
                        stage: "parameter-image exponentiation",
                        error,
                    })?;
            }
            remaining >>= 1;
            if remaining != 0 {
                charge_operation(operations, self.limits)?;
                factor = self
                    .target
                    .try_mul(&factor, &factor, self.limits.exact_algebra)
                    .map_err(|error| BaseSpecializationError::ExactEvaluation {
                        stage: "parameter-image exponentiation",
                        error,
                    })?;
            }
        }
        Ok(result)
    }
}

fn family_condition_source(condition: &BaseNonZeroCondition) -> FamilyDomainConditionSource {
    FamilyDomainConditionSource {
        location: condition.source().clone(),
        origins: condition.origins().clone(),
    }
}

fn merge_zero_condition(
    zero_conditions: &mut Vec<InapplicableFamilyDomainCondition>,
    source: FamilyDomainConditionSource,
    limits: BaseSpecializationLimits,
) -> Result<(), BaseSpecializationError> {
    if let Some(existing) = zero_conditions.first_mut() {
        if source.location < existing.source.location {
            existing.source.location = source.location;
        }
        for origin in source.origins {
            if !existing.source.origins.contains(&origin) {
                let requested = existing.source.origins.len().checked_add(1).ok_or(
                    BaseSpecializationError::ResourceCountOverflow {
                        resource: "inapplicable family-domain origins",
                    },
                )?;
                check_limit(
                    "inapplicable family-domain origins",
                    requested,
                    limits.max_guard_origins,
                )?;
                existing.source.origins.insert(origin);
            }
        }
    } else {
        check_limit(
            "inapplicable family-domain origins",
            source.origins.len(),
            limits.max_guard_origins,
        )?;
        zero_conditions.push(InapplicableFamilyDomainCondition { source });
    }
    Ok(())
}

fn make_guard(
    target_parameters: &Arc<[String]>,
    polynomial: SpecializedBasePolynomial,
    origins: impl IntoIterator<Item = BaseSpecializationGuardProvenance>,
    limits: BaseSpecializationLimits,
) -> Result<BaseSpecializationGuard, BaseSpecializationError> {
    let mut retained_origins = BTreeSet::new();
    for origin in origins {
        retained_origins.insert(origin);
        check_limit(
            "base-specialization guard origins",
            retained_origins.len(),
            limits.max_guard_origins,
        )?;
    }
    Ok(BaseSpecializationGuard {
        origins: retained_origins,
        polynomial,
        target_parameters: Arc::clone(target_parameters),
    })
}

fn merge_guard_checked(
    guards: &mut Vec<BaseSpecializationGuard>,
    guard: BaseSpecializationGuard,
    limits: BaseSpecializationLimits,
) -> Result<(), BaseSpecializationError> {
    if let Some(existing) = guards.iter_mut().find(|existing| {
        existing.target_parameters == guard.target_parameters
            && existing.polynomial == guard.polynomial
    }) {
        for origin in guard.origins {
            if !existing.origins.contains(&origin) {
                let requested = existing.origins.len().checked_add(1).ok_or(
                    BaseSpecializationError::ResourceCountOverflow {
                        resource: "base-specialization guard origins",
                    },
                )?;
                check_limit(
                    "base-specialization guard origins",
                    requested,
                    limits.max_guard_origins,
                )?;
                existing.origins.insert(origin);
            }
        }
        return Ok(());
    }
    let requested =
        guards
            .len()
            .checked_add(1)
            .ok_or(BaseSpecializationError::ResourceCountOverflow {
                resource: "base-specialization guards",
            })?;
    check_limit("base-specialization guards", requested, limits.max_guards)?;
    guards.push(guard);
    Ok(())
}

fn charge_operation(
    operations: &mut usize,
    limits: BaseSpecializationLimits,
) -> Result<(), BaseSpecializationError> {
    *operations =
        operations
            .checked_add(1)
            .ok_or(BaseSpecializationError::ResourceCountOverflow {
                resource: "base-specialization evaluation operations",
            })?;
    check_limit(
        "base-specialization evaluation operations",
        *operations,
        limits.max_evaluation_operations,
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), BaseSpecializationError> {
    if requested > limit {
        Err(BaseSpecializationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
