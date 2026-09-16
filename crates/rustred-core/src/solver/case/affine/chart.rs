//! Native coefficient restriction, separate from the integer case semantics.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{Integer, IntegerRing, Matrix, Q, Rational, Ring, Z};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::{AffineGeometryError, CoordinateCase, integer_row_possible};

type RationalPolynomial = MultivariatePolynomial<Q, u16>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Chart {
    Integral(Vec<(usize, CoefficientPolynomial)>),
    Rational(Vec<(usize, RationalPolynomial)>),
}

impl Chart {
    pub(crate) fn new(
        template: &CoefficientPolynomial,
        matrix: &Matrix<Q>,
        indices: &[usize],
    ) -> Self {
        if matrix.iter().all(Rational::is_integer) {
            Self::Integral(replacements(template, matrix, indices, Rational::numerator))
        } else {
            let template = template.map_coeff(|value| Rational::from(value.clone()), Q);
            Self::Rational(replacements(&template, matrix, indices, Clone::clone))
        }
    }

    pub(crate) fn is_integral(&self) -> bool {
        matches!(self, Self::Integral(_))
    }

    pub(super) fn restrict_equation<const N: usize>(
        &self,
        polynomial: &CoefficientPolynomial,
        face: &CoordinateCase<N>,
        indices: &[usize; N],
    ) -> CoefficientPolynomial {
        self.restrict_equation_on_face(polynomial, face.fixed(), indices)
    }

    pub(crate) fn restrict_equation_on_face(
        &self,
        polynomial: &CoefficientPolynomial,
        fixed: &[Option<i16>],
        indices: &[usize],
    ) -> CoefficientPolynomial {
        match self {
            Self::Integral(replacements) => {
                restrict_integral(polynomial, fixed, indices, replacements)
            }
            Self::Rational(replacements) => {
                let restricted = restrict_rational(polynomial, fixed, indices, replacements);
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
        self.restrict_polynomial_value_on_face(polynomial, face.fixed(), indices)
    }

    pub(crate) fn restrict_polynomial_value_on_face(
        &self,
        polynomial: &CoefficientPolynomial,
        fixed: &[Option<i16>],
        indices: &[usize],
    ) -> Coefficient {
        match self {
            Self::Integral(replacements) => {
                restrict_integral(polynomial, fixed, indices, replacements).into()
            }
            Self::Rational(replacements) => {
                let numerator = restrict_rational(polynomial, fixed, indices, replacements);
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
        self.restrict_coefficient_on_face(coefficient, face.fixed(), indices)
    }

    pub(crate) fn restrict_coefficient_on_face(
        &self,
        coefficient: &Coefficient,
        fixed: &[Option<i16>],
        indices: &[usize],
    ) -> Result<Coefficient, AffineGeometryError> {
        match self {
            Self::Integral(replacements) => {
                let numerator =
                    restrict_integral(&coefficient.numerator, fixed, indices, replacements);
                let denominator =
                    restrict_integral(&coefficient.denominator, fixed, indices, replacements);
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
                    restrict_rational(&coefficient.numerator, fixed, indices, replacements);
                let denominator =
                    restrict_rational(&coefficient.denominator, fixed, indices, replacements);
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
fn replacements<F: Ring>(
    template: &MultivariatePolynomial<F, u16>,
    matrix: &Matrix<Q>,
    indices: &[usize],
    convert: impl Fn(&Rational) -> F::Element,
) -> Vec<(usize, MultivariatePolynomial<F, u16>)> {
    let n = indices.len();
    let mut substitutions = Vec::new();
    for row in matrix.row_iter() {
        if row[..n].iter().filter(|value| !value.is_zero()).count() <= 1 {
            continue;
        }
        let pivot = row[..n]
            .iter()
            .position(|value| !value.is_zero())
            .expect("coupled canonical row has a pivot");
        let mut replacement = template.constant(convert(&row[n]));
        let mut exponents = vec![0; template.nvars()];
        for (axis, coefficient) in row[..n].iter().enumerate() {
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

fn restrict_integral(
    polynomial: &CoefficientPolynomial,
    fixed: &[Option<i16>],
    indices: &[usize],
    replacements: &[(usize, CoefficientPolynomial)],
) -> CoefficientPolynomial {
    let mut result = specialize_fixed(polynomial, fixed, indices);
    for (position, replacement) in replacements {
        result = result.replace_with_poly(*position, replacement);
    }
    result
}

fn restrict_rational(
    polynomial: &CoefficientPolynomial,
    fixed: &[Option<i16>],
    indices: &[usize],
    replacements: &[(usize, RationalPolynomial)],
) -> RationalPolynomial {
    let mut result = polynomial.map_coeff(|value| Rational::from(value.clone()), Q);
    for (axis, value) in fixed.iter().enumerate() {
        if let Some(value) = value {
            result = result.replace(indices[axis], &Rational::from(*value));
        }
    }
    for (position, replacement) in replacements {
        result = result.replace_with_poly(*position, replacement);
    }
    result
}

fn specialize_fixed(
    polynomial: &CoefficientPolynomial,
    fixed: &[Option<i16>],
    indices: &[usize],
) -> CoefficientPolynomial {
    let mut result = polynomial.clone();
    for (&index, value) in indices.iter().zip(fixed) {
        if let Some(value) = value {
            result = result.replace(index, &Integer::from(*value));
        }
    }
    result
}

/// One shared native equality reduction for search and cold affine replay.
/// `None` means an exact rational or single-row integer contradiction. A
/// returned matrix does not assert integer/sector feasibility.
pub(crate) fn canonical_equalities(
    fixed: &[Option<i16>],
    equations: &[CoefficientPolynomial],
    indices: &[usize],
) -> Result<Option<(Matrix<Q>, Matrix<IntegerRing>)>, AffineGeometryError> {
    let n = indices.len();
    let Some(template) = equations.first() else {
        return Err(AffineGeometryError::InvalidInput(
            "missing affine equations",
        ));
    };
    if n == 0 || fixed.len() != n {
        return Err(AffineGeometryError::InvalidInput("affine index/face arity"));
    }
    let mut seen = std::collections::BTreeSet::new();
    if indices
        .iter()
        .any(|&index| index >= template.nvars() || !seen.insert(index))
    {
        return Err(AffineGeometryError::InvalidInput(
            "invalid affine index-variable map",
        ));
    }
    for equation in equations {
        if equation.variables() != template.variables()
            || equation.coefficients.len().checked_mul(equation.nvars())
                != Some(equation.exponents.len())
        {
            return Err(AffineGeometryError::InvalidInput(
                "invalid affine polynomial variable map or storage",
            ));
        }
    }
    let columns = n.checked_add(1).and_then(|v| u32::try_from(v).ok()).ok_or(
        AffineGeometryError::InvalidInput("affine matrix dimensions exceed native limits"),
    )?;
    let mut rows: Vec<Vec<Rational>> = Vec::new();
    for (axis, value) in fixed.iter().enumerate() {
        if let Some(value) = value {
            let mut row = vec![Rational::zero(); n + 1];
            row[axis] = Rational::one();
            row[n] = Rational::from(*value);
            rows.push(row);
        }
    }
    for source in equations {
        let equation = specialize_fixed(source, fixed, indices);
        if equation.is_zero() {
            continue;
        }
        if equation.is_constant() {
            return Ok(None);
        }
        if (0..equation.nterms()).any(|term| {
            equation
                .exponents(term)
                .iter()
                .map(|&e| usize::from(e))
                .sum::<usize>()
                > 1
        }) {
            return Err(AffineGeometryError::UnsupportedNonlinear {
                equations: equations.to_vec(),
            });
        }
        if (0..equation.nterms()).any(|term| {
            equation
                .exponents(term)
                .iter()
                .enumerate()
                .any(|(index, &power)| power != 0 && !seen.contains(&index))
        }) {
            return Err(AffineGeometryError::InvalidInput(
                "affine equation depends on a non-index parameter",
            ));
        }
        let mut row = vec![Integer::zero(); n + 1];
        let mut exponents = vec![0; equation.nvars()];
        for (axis, &position) in indices.iter().enumerate() {
            exponents[position] = 1;
            row[axis] = equation
                .coefficient(&exponents)
                .unwrap_or_else(Integer::zero);
            exponents[position] = 0;
        }
        row[n] = -equation.get_constant();
        if !integer_row_possible(&row) {
            return Ok(None);
        }
        rows.push(row.into_iter().map(Rational::from).collect());
    }
    let count = u32::try_from(rows.len()).map_err(|_| {
        AffineGeometryError::InvalidInput("affine matrix dimensions exceed native limits")
    })?;
    count
        .checked_mul(columns)
        .ok_or(AffineGeometryError::InvalidInput(
            "affine matrix dimensions exceed native limits",
        ))?;
    if rows.is_empty() {
        return Ok(Some((
            Matrix::new(0, columns, Q),
            Matrix::new(0, columns, Z),
        )));
    }
    let mut matrix = Matrix::from_linear(rows.into_iter().flatten().collect(), count, columns, Q)
        .map_err(|_| AffineGeometryError::NativeAlgebra)?;
    let rank = matrix.row_reduce(n as u32);
    if matrix.row_iter().skip(rank).any(|row| !row[n].is_zero()) {
        return Ok(None);
    }
    let mut primitive_rows = Vec::with_capacity(rank * (n + 1));
    for row in matrix.row_iter().take(rank) {
        let primitive = Matrix::new_vec(row.to_vec(), Q).primitive_part();
        if primitive.iter().any(|entry| !entry.is_integer()) {
            return Err(AffineGeometryError::NativeAlgebra);
        }
        let integers: Vec<_> = primitive.iter().map(|entry| entry.numerator()).collect();
        if !integer_row_possible(&integers) {
            return Ok(None);
        }
        primitive_rows.extend(integers);
    }
    let primitive = Matrix::from_linear(primitive_rows, rank as u32, columns, Z)
        .map_err(|_| AffineGeometryError::NativeAlgebra)?;
    let matrix = Matrix::from_linear(
        matrix.row_iter().take(rank).flatten().cloned().collect(),
        rank as u32,
        columns,
        Q,
    )
    .map_err(|_| AffineGeometryError::NativeAlgebra)?;
    Ok(Some((matrix, primitive)))
}
