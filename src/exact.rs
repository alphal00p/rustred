use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::{Ring, RingOps};

/// A canonical exact rational backed by Symbolica's GMP-enabled rational field.
///
/// This nominal wrapper keeps RustRed's public kinematic-coordinate type
/// separate from its rational-polynomial coefficients. All normalization and
/// arithmetic are delegated to Symbolica; RustRed does not implement a second
/// rational field.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExactRational(Rational);

/// Checked-construction and checked-division failures for ExactRational.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactRationalError {
    ZeroDenominator,
    DivisionByZero,
}

impl fmt::Display for ExactRationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => formatter.write_str("a rational denominator cannot be zero"),
            Self::DivisionByZero => formatter.write_str("cannot divide by zero"),
        }
    }
}

impl std::error::Error for ExactRationalError {}

impl ExactRational {
    /// Construct and canonically normalize an exact rational.
    ///
    /// This compatibility constructor retains the historical panic on a zero
    /// denominator. Input-dependent code should use try_new.
    pub fn new<N: Into<Integer>, D: Into<Integer>>(numerator: N, denominator: D) -> Self {
        Self::try_new(numerator, denominator).expect("a rational denominator cannot be zero")
    }

    /// Construct and canonically normalize an exact rational without a panic
    /// on a zero denominator.
    pub fn try_new<N: Into<Integer>, D: Into<Integer>>(
        numerator: N,
        denominator: D,
    ) -> Result<Self, ExactRationalError> {
        let numerator = numerator.into();
        let denominator = denominator.into();
        if denominator.is_zero() {
            return Err(ExactRationalError::ZeroDenominator);
        }

        // Rational::new delegates gcd reduction and denominator-sign
        // normalization to Symbolica's exact rational field. The zero case was
        // checked above because Symbolica's constructor is intentionally
        // panicking for that malformed input.
        Ok(Self(Rational::new(numerator, denominator)))
    }

    pub fn zero() -> Self {
        Self(Rational::zero())
    }

    pub fn one() -> Self {
        Self(Rational::one())
    }

    /// Return the arbitrary-precision canonical numerator.
    pub fn numerator(&self) -> &Integer {
        self.0.numerator_ref()
    }

    /// Return the arbitrary-precision positive canonical denominator.
    pub fn denominator(&self) -> &Integer {
        self.0.denominator_ref()
    }

    /// Narrow the numerator only when it is exactly representable as i64.
    pub fn numerator_i64(&self) -> Option<i64> {
        self.numerator().to_i64()
    }

    /// Narrow the denominator only when it is exactly representable as i64.
    pub fn denominator_i64(&self) -> Option<i64> {
        self.denominator().to_i64()
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.0.is_one()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// Invert this value using Symbolica's checked rational-field operation.
    pub fn try_reciprocal(&self) -> Result<Self, ExactRationalError> {
        Q.try_inv(&self.0)
            .map(Self)
            .ok_or(ExactRationalError::DivisionByZero)
    }

    /// Historical panicking reciprocal. Input-dependent code should use
    /// try_reciprocal.
    pub fn reciprocal(&self) -> Self {
        self.try_reciprocal().expect("cannot invert zero")
    }

    /// Divide through Symbolica's checked rational-field operation.
    pub fn try_div(&self, rhs: &Self) -> Result<Self, ExactRationalError> {
        Q.try_div(&self.0, &rhs.0)
            .map(Self)
            .ok_or(ExactRationalError::DivisionByZero)
    }

    /// Borrow the underlying canonical Symbolica value.
    pub fn as_rational(&self) -> &Rational {
        &self.0
    }

    /// Consume this wrapper and return its canonical Symbolica value.
    pub fn into_rational(self) -> Rational {
        self.0
    }
}

impl From<i64> for ExactRational {
    fn from(value: i64) -> Self {
        Self(Rational::from(value))
    }
}

impl From<Integer> for ExactRational {
    fn from(value: Integer) -> Self {
        Self(Rational::from(value))
    }
}

macro_rules! forward_binary_operator {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for ExactRational {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                Self(self.0 $operator rhs.0)
            }
        }

        impl $trait<&ExactRational> for ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: &ExactRational) -> Self::Output {
                ExactRational(self.0 $operator &rhs.0)
            }
        }

        impl $trait<ExactRational> for &ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: ExactRational) -> Self::Output {
                ExactRational(&self.0 $operator &rhs.0)
            }
        }

        impl $trait<&ExactRational> for &ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: &ExactRational) -> Self::Output {
                ExactRational(&self.0 $operator &rhs.0)
            }
        }
    };
}

forward_binary_operator!(Add, add, +);
forward_binary_operator!(Sub, sub, -);
forward_binary_operator!(Mul, mul, *);
forward_binary_operator!(Div, div, /);

impl Neg for ExactRational {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Neg for &ExactRational {
    type Output = ExactRational;

    fn neg(self) -> Self::Output {
        ExactRational(Q.neg(&self.0))
    }
}

impl fmt::Display for ExactRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_rational_is_gmp_backed_and_canonical() {
        let huge = Integer::from(2).pow(257);
        let value = ExactRational::try_new(-&huge * 6, Integer::from(-18)).unwrap();
        assert_eq!(value.numerator(), &huge);
        assert_eq!(value.denominator(), &Integer::from(3));
        assert_eq!(value.numerator_i64(), None);
        assert_eq!(value.denominator_i64(), Some(3));

        let squared = &value * &value;
        assert_eq!(squared.numerator(), &huge.pow(2));
        assert_eq!(squared.denominator(), &Integer::from(9));
    }

    #[test]
    fn exact_rational_checked_boundaries_reject_zero() {
        assert_eq!(
            ExactRational::try_new(1, 0),
            Err(ExactRationalError::ZeroDenominator)
        );
        assert_eq!(
            ExactRational::zero().try_reciprocal(),
            Err(ExactRationalError::DivisionByZero)
        );
        assert_eq!(
            ExactRational::one().try_div(&ExactRational::zero()),
            Err(ExactRationalError::DivisionByZero)
        );
    }
}
