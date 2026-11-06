//! Gamma functions split into a Gamma tag and an exact rational in `d` (owner:
//! M2-T4 rr-kernel).
//!
//! With `eps = (4 - d) / 2`,
//! `Gamma(c + s eps) = Gamma(1 + s eps) (1 + s eps)_{c - 1}`, where the
//! Pochhammer symbol `(x)_k = Gamma(x + k) / Gamma(x)` of any integer `k` is a
//! product of linear factors `a + b eps`. The tag keeps the transcendental part,
//! a [`FactoredRational`] the rest, with its linear factors in lowest terms, so
//! every kernel weight has factored linear denominators.
//!
//! At `s = 0`: `Gamma(c) = (c - 1)!`, `1 / Gamma(c <= 0) = 0`, and `Gamma(c <= 0)`
//! in a numerator is [`PError::Unsupported`], never a silent zero or infinity.
use std::collections::BTreeMap;

use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::propagator::error::PError;
use crate::propagator::presentation::eps_power_shift;
use crate::propagator::tag::GammaTag;
use crate::reduction::ReductionError;

pub(crate) fn algebra(error: ExactAlgebraError) -> PError {
    ReductionError::ExactAlgebra(error).into()
}

fn greatest_common_divisor(a: i64, b: i64) -> u64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `constant + slope eps` with coprime entries and `slope > 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LinearFactor {
    pub constant: i64,
    pub slope: i64,
}

/// `r Prod_i (a_i + b_i eps)^{e_i}`, exact; distinct factors are coprime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactoredRational {
    prefactor: Rational,
    factors: BTreeMap<LinearFactor, i32>,
}

impl FactoredRational {
    pub fn one() -> Self {
        Self::from_rational(Rational::one())
    }

    pub fn zero() -> Self {
        Self::from_rational(Rational::zero())
    }

    pub fn from_rational(prefactor: Rational) -> Self {
        Self {
            prefactor,
            factors: BTreeMap::new(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.prefactor.is_zero()
    }

    pub fn prefactor(&self) -> &Rational {
        &self.prefactor
    }

    /// The linear factors with their nonzero exponents.
    pub fn factors(&self) -> impl Iterator<Item = (LinearFactor, i32)> + '_ {
        self.factors.iter().map(|(&factor, &power)| (factor, power))
    }

    /// Multiply by `(constant + slope eps)^power`. `(0)^power` with `power < 0`
    /// is [`PError::Unsupported`].
    pub fn mul_linear(&mut self, constant: i64, slope: i64, power: i32) -> Result<(), PError> {
        if power == 0 || self.is_zero() {
            return Ok(());
        }
        if constant == 0 && slope == 0 {
            if power < 0 {
                return Err(PError::Unsupported(
                    "a Gamma ratio with a vanishing exact denominator".to_owned(),
                ));
            }
            *self = Self::zero();
            return Ok(());
        }
        if slope == 0 {
            let scale = Rational::from(constant);
            for _ in 0..power.unsigned_abs() {
                self.prefactor = if power > 0 {
                    &self.prefactor * &scale
                } else {
                    &self.prefactor / &scale
                };
            }
            return Ok(());
        }
        let divisor = greatest_common_divisor(constant, slope);
        let divisor = i64::try_from(divisor)
            .map_err(|_| PError::Unsupported("a linear factor out of range".to_owned()))?;
        let unit = if slope < 0 { -divisor } else { divisor };
        let scale = Rational::from(unit);
        for _ in 0..power.unsigned_abs() {
            self.prefactor = if power > 0 {
                &self.prefactor * &scale
            } else {
                &self.prefactor / &scale
            };
        }
        let factor = LinearFactor {
            constant: constant / unit,
            slope: slope / unit,
        };
        let exponent = self.factors.entry(factor).or_insert(0);
        *exponent += power;
        if *exponent == 0 {
            self.factors.remove(&factor);
        }
        Ok(())
    }

    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut product = self.clone();
        product.prefactor = &product.prefactor * &other.prefactor;
        for (&factor, &power) in &other.factors {
            let exponent = product.factors.entry(factor).or_insert(0);
            *exponent += power;
            if *exponent == 0 {
                product.factors.remove(&factor);
            }
        }
        product
    }

    /// `Gamma(x + k) / Gamma(x)`, `x = constant + slope eps`.
    ///
    /// An exact zero among the factors (`slope = 0`) gives zero for `k >= 0` and
    /// [`PError::Unsupported`] for `k < 0`.
    pub fn pochhammer(constant: i64, slope: i64, k: i64) -> Result<Self, PError> {
        let mut value = Self::one();
        for i in 0..k {
            value.mul_linear(constant + i, slope, 1)?;
        }
        for i in 1..=-k {
            value.mul_linear(constant - i, slope, -1)?;
        }
        Ok(value)
    }

    /// The value in `Q(d)`.
    pub fn to_coefficient(
        &self,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, PError> {
        let prefactor = rational_coefficient(context, &self.prefactor, limits)?;
        if self.is_zero() || self.factors.is_empty() {
            return Ok(prefactor);
        }
        let eps = eps_power_shift(context, 1)?;
        let mut numerator = prefactor;
        let mut denominator = context.one();
        for (factor, &power) in &self.factors {
            let linear = context
                .try_mul(&context.integer(factor.slope), &eps, limits)
                .and_then(|slope| {
                    context.try_add(&context.integer(factor.constant), &slope, limits)
                })
                .map_err(algebra)?;
            let target = if power > 0 {
                &mut numerator
            } else {
                &mut denominator
            };
            for _ in 0..power.unsigned_abs() {
                *target = context.try_mul(target, &linear, limits).map_err(algebra)?;
            }
        }
        context
            .try_div(&numerator, &denominator, limits)
            .map_err(algebra)
    }
}

/// `value` in `Q(d)`.
pub fn rational_coefficient(
    context: &CoefficientContext,
    value: &Rational,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    let template = context.zero();
    let numerator: Coefficient = template
        .numerator
        .constant(value.numerator_ref().clone())
        .into();
    if value.is_integer() {
        return Ok(numerator);
    }
    let denominator: Coefficient = template
        .numerator
        .constant(value.denominator_ref().clone())
        .into();
    context
        .try_div(&numerator, &denominator, limits)
        .map_err(algebra)
}

pub(crate) fn factorial(n: i64) -> Rational {
    (1..=n).fold(Rational::one(), |product, value| {
        &product * &Rational::from(value)
    })
}

/// A product of `Gamma(c + s eps)^{+-1}`: a tag and an exact rational.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GammaProduct {
    tag: BTreeMap<i64, i64>,
    rational: FactoredRational,
}

impl Default for GammaProduct {
    fn default() -> Self {
        Self::one()
    }
}

impl GammaProduct {
    pub fn one() -> Self {
        Self {
            tag: BTreeMap::new(),
            rational: FactoredRational::one(),
        }
    }

    fn bump(&mut self, slope: i64, power: i64) {
        let exponent = self.tag.entry(slope).or_insert(0);
        *exponent += power;
        if *exponent == 0 {
            self.tag.remove(&slope);
        }
    }

    /// Multiply by `Gamma(c + s eps)`.
    pub fn gamma(&mut self, c: i64, s: i64) -> Result<(), PError> {
        if s != 0 {
            self.bump(s, 1);
            let ratio = FactoredRational::pochhammer(1, s, c - 1)?;
            self.rational = self.rational.mul(&ratio);
            return Ok(());
        }
        if c <= 0 {
            return Err(PError::Unsupported(format!(
                "Gamma({c}) in a kernel numerator"
            )));
        }
        self.rational = self
            .rational
            .mul(&FactoredRational::from_rational(factorial(c - 1)));
        Ok(())
    }

    /// Multiply by `1 / Gamma(c + s eps)`; zero at `s = 0`, `c <= 0`.
    pub fn inverse_gamma(&mut self, c: i64, s: i64) -> Result<(), PError> {
        if s != 0 {
            self.bump(s, -1);
            let mut ratio = FactoredRational::one();
            for i in 0..c - 1 {
                ratio.mul_linear(1 + i, s, -1)?;
            }
            for i in 1..=1 - c {
                ratio.mul_linear(1 - i, s, 1)?;
            }
            self.rational = self.rational.mul(&ratio);
            return Ok(());
        }
        self.rational = if c <= 0 {
            FactoredRational::zero()
        } else {
            self.rational
                .mul(&FactoredRational::from_rational(factorial(c - 1).inv()))
        };
        Ok(())
    }

    /// The tag `Prod Gamma(1 + m eps)^e`, [`PError::TagOutOfRange`] beyond its range.
    pub fn tag(&self) -> Result<GammaTag, PError> {
        let mut pairs = Vec::with_capacity(self.tag.len());
        for (&slope, &power) in &self.tag {
            pairs.push((
                i8::try_from(slope).map_err(|_| PError::TagOutOfRange)?,
                i16::try_from(power).map_err(|_| PError::TagOutOfRange)?,
            ));
        }
        GammaTag::from_pairs(pairs)
    }

    pub fn rational(&self) -> &FactoredRational {
        &self.rational
    }
}
