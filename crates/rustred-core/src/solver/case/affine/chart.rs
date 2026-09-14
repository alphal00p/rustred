//! Native coefficient restriction, separate from the integer case semantics.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{IntegerRing, Matrix, Q, Rational, Ring, Z};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::{AffineGeometryError, CoordinateCase, specialize_face};

type RationalPolynomial = MultivariatePolynomial<Q, u16>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Chart {
    Integral(Vec<(usize, CoefficientPolynomial)>),
    Rational(Vec<(usize, RationalPolynomial)>),
}

impl Chart {
    pub(super) fn new<const N: usize>(
        template: &CoefficientPolynomial,
        matrix: &Matrix<Q>,
        indices: &[usize; N],
    ) -> Self {
        if matrix.iter().all(Rational::is_integer) {
            Self::Integral(replacements(template, matrix, indices, Rational::numerator))
        } else {
            let template = template.map_coeff(|value| Rational::from(value.clone()), Q);
            Self::Rational(replacements(&template, matrix, indices, Clone::clone))
        }
    }

    pub(super) fn is_integral(&self) -> bool {
        matches!(self, Self::Integral(_))
    }

    pub(super) fn restrict_equation<const N: usize>(
        &self,
        polynomial: &CoefficientPolynomial,
        face: &CoordinateCase<N>,
        indices: &[usize; N],
    ) -> CoefficientPolynomial {
        match self {
            Self::Integral(replacements) => {
                restrict_integral(polynomial, face, indices, replacements)
            }
            Self::Rational(replacements) => {
                let restricted = restrict_rational(polynomial, face, indices, replacements);
                if restricted.is_zero() {
                    return polynomial.zero();
                }
                // Only this zero-locus operation may discard scalar content.
                // Native rational content clears all denominators together.
                let primitive = restricted.make_primitive();
                debug_assert!(primitive.coefficients.iter().all(Rational::is_integer));
                primitive.map_coeff(Rational::numerator, Z)
            }
        }
    }

    pub(super) fn restrict_polynomial_value<const N: usize>(
        &self,
        polynomial: &CoefficientPolynomial,
        face: &CoordinateCase<N>,
        indices: &[usize; N],
    ) -> Coefficient {
        match self {
            Self::Integral(replacements) => {
                restrict_integral(polynomial, face, indices, replacements).into()
            }
            Self::Rational(replacements) => {
                let numerator = restrict_rational(polynomial, face, indices, replacements);
                let denominator = numerator.one();
                // The native joint-content conversion preserves every rational
                // factor. The constant denominator needs no polynomial GCD.
                <Coefficient as FromNumeratorAndDenominator<Q, IntegerRing, u16>>::from_num_den(
                    numerator,
                    denominator,
                    &Z,
                    false,
                )
            }
        }
    }

    pub(super) fn restrict_coefficient<const N: usize>(
        &self,
        coefficient: &Coefficient,
        face: &CoordinateCase<N>,
        indices: &[usize; N],
    ) -> Result<Coefficient, AffineGeometryError> {
        match self {
            Self::Integral(replacements) => {
                let numerator =
                    restrict_integral(&coefficient.numerator, face, indices, replacements);
                let denominator =
                    restrict_integral(&coefficient.denominator, face, indices, replacements);
                if denominator.is_zero() {
                    return Err(AffineGeometryError::UndefinedCoefficient);
                }
                Ok(<Coefficient as FromNumeratorAndDenominator<
                    IntegerRing,
                    IntegerRing,
                    u16,
                >>::from_num_den(
                    numerator, denominator, &Z, true
                ))
            }
            Self::Rational(replacements) => {
                let numerator =
                    restrict_rational(&coefficient.numerator, face, indices, replacements);
                let denominator =
                    restrict_rational(&coefficient.denominator, face, indices, replacements);
                if denominator.is_zero() {
                    return Err(AffineGeometryError::UndefinedCoefficient);
                }
                // Both polynomial contents must be converted JOINTLY. Separate
                // primitive normalization would change the coefficient value.
                Ok(<Coefficient as FromNumeratorAndDenominator<
                    Q,
                    IntegerRing,
                    u16,
                >>::from_num_den(
                    numerator, denominator, &Z, true
                ))
            }
        }
    }
}

/// Read native RREF substitutions without another elimination or pivot policy.
fn replacements<F: Ring, const N: usize>(
    template: &MultivariatePolynomial<F, u16>,
    matrix: &Matrix<Q>,
    indices: &[usize; N],
    convert: impl Fn(&Rational) -> F::Element,
) -> Vec<(usize, MultivariatePolynomial<F, u16>)> {
    let mut substitutions = Vec::new();
    for row in matrix.row_iter() {
        if row[..N].iter().filter(|value| !value.is_zero()).count() <= 1 {
            continue;
        }
        let pivot = row[..N]
            .iter()
            .position(|value| !value.is_zero())
            .expect("coupled canonical row has a pivot");
        let mut replacement = template.constant(convert(&row[N]));
        let mut exponents = vec![0; template.nvars()];
        for (axis, coefficient) in row[..N].iter().enumerate() {
            if axis == pivot || coefficient.is_zero() {
                continue;
            }
            exponents[indices[axis]] = 1;
            replacement.append_monomial(convert(&-coefficient.clone()), &exponents);
            exponents[indices[axis]] = 0;
        }
        substitutions.push((indices[pivot], replacement));
    }
    substitutions
}

fn restrict_integral<const N: usize>(
    polynomial: &CoefficientPolynomial,
    face: &CoordinateCase<N>,
    indices: &[usize; N],
    replacements: &[(usize, CoefficientPolynomial)],
) -> CoefficientPolynomial {
    let mut result = specialize_face(polynomial, face, indices);
    for (position, replacement) in replacements {
        result = result.replace_with_poly(*position, replacement);
    }
    result
}

fn restrict_rational<const N: usize>(
    polynomial: &CoefficientPolynomial,
    face: &CoordinateCase<N>,
    indices: &[usize; N],
    replacements: &[(usize, RationalPolynomial)],
) -> RationalPolynomial {
    let mut result = polynomial.map_coeff(|value| Rational::from(value.clone()), Q);
    for (axis, value) in face.fixed().iter().enumerate() {
        if let Some(value) = value {
            result = result.replace(indices[axis], &Rational::from(*value));
        }
    }
    for (position, replacement) in replacements {
        result = result.replace_with_poly(*position, replacement);
    }
    result
}
