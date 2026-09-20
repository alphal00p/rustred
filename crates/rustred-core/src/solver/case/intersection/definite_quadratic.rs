//! Exact consequences of parameter-free definite quadratic guards.
//!
//! A strictly positive minimum proves an AND branch empty. A zero minimum
//! describes one point in the polynomial's supported coordinates, equivalently
//! the native affine gradient equations; other coordinates remain free. Unknown,
//! singular and indefinite forms retain the ordinary unsupported-geometry
//! outcome. Symbolica owns differentiation, rational arithmetic, determinants
//! and the stationary-point linear solve.

use symbolica::prelude::{Integer, Matrix, Q, Rational};

use crate::algebra::CoefficientPolynomial;

use super::super::Case;

/// At most this many native determinant calls and one native matrix solve.
/// Over this cold-geometry limit we make no inference; it is independent of
/// topology, loop count and compact-power representation.
const MAX_PRINCIPAL_MINORS: usize = 32;

#[derive(Debug)]
pub(super) enum QuadraticProof {
    Empty,
    EquivalentAffine(Vec<CoefficientPolynomial>),
    Unknown,
}

pub(super) fn classify<const N: usize>(
    parent: &Case<N>,
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
    sector: &[bool; N],
) -> QuadraticProof {
    proof(parent, equation, indices, sector).unwrap_or(QuadraticProof::Unknown)
}

fn proof<const N: usize>(
    parent: &Case<N>,
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Option<QuadraticProof> {
    let nvars = equation.nvars();
    let mut axis_of = vec![None; nvars];
    for (axis, &variable) in indices.iter().enumerate() {
        if variable >= nvars || axis_of[variable].replace(axis).is_some() {
            return None;
        }
    }

    // A parameter coefficient can change sign or vanish. Only a polynomial
    // entirely in physical integer indices may receive this real-domain test.
    let mut supported = vec![false; N];
    let mut quadratic = false;
    for powers in equation.exponents_iter() {
        let mut degree = 0u64;
        for (variable, &power) in powers.iter().enumerate() {
            if power == 0 {
                continue;
            }
            degree += u64::from(power);
            let axis = axis_of[variable]?;
            supported[axis] = true;
        }
        if degree > 2 {
            return None;
        }
        quadratic |= degree == 2;
    }
    if !quadratic {
        return None;
    }
    let axes: Vec<_> = (0..N).filter(|&axis| supported[axis]).collect();
    let size = axes.len();
    if size == 0 || size > MAX_PRINCIPAL_MINORS {
        return None;
    }
    let size_u32 = u32::try_from(size).ok()?;

    let zero = vec![Integer::zero(); nvars];
    let gradient_polynomials = axes
        .iter()
        .map(|&axis| equation.derivative(indices[axis]))
        .collect::<Vec<_>>();
    let first_curvature = gradient_polynomials[0]
        .derivative(indices[axes[0]])
        .replace_all(&zero);
    if first_curvature.is_zero() {
        return None;
    }
    // q and -q have the same zero set. Orient the definite candidate so its
    // first Hessian diagonal is positive, then apply Sylvester's criterion.
    let reverse_sign = first_curvature.is_negative();
    let orient = |value: Integer| Rational::from(if reverse_sign { -value } else { value });
    let mut hessian_data = Vec::with_capacity(size * size);
    for gradient in &gradient_polynomials {
        for &axis in &axes {
            hessian_data.push(orient(
                gradient.derivative(indices[axis]).replace_all(&zero),
            ));
        }
    }
    let hessian = Matrix::from_linear(hessian_data, size_u32, size_u32, Q).ok()?;
    for width in 1..=size_u32 {
        let mut entries = Vec::with_capacity((width as usize) * (width as usize));
        for row in 0..width {
            for column in 0..width {
                entries.push(hessian[(row, column)].clone());
            }
        }
        let minor = Matrix::from_linear(entries, width, width, Q).ok()?;
        if minor.det().ok()? <= Rational::zero() {
            return None;
        }
    }

    let gradient = gradient_polynomials
        .iter()
        .map(|polynomial| orient(polynomial.replace_all(&zero)))
        .collect::<Vec<_>>();
    let rhs = Matrix::new_vec(gradient.iter().map(|value| -value.clone()).collect(), Q);
    let stationary = hessian.solve(&rhs).ok()?;
    // For q(x)=c+b*x+(x*H*x)/2, H*x*=-b and
    // min q = c+(b*x*)/2. All operations here are native exact rationals.
    let mut minimum = orient(equation.replace_all(&zero));
    for (row, value) in gradient.iter().enumerate() {
        minimum += (value * &stationary[(row as u32, 0)]) / Rational::from(2);
    }
    if minimum > Rational::zero() {
        return Some(QuadraticProof::Empty);
    }
    if !minimum.is_zero() {
        return None;
    }

    // A positive-definite form with zero minimum has exactly this one real
    // supported-coordinate point. If any such coordinate is not an admitted
    // integer power, the entire AND branch is empty, regardless of other free
    // variables or coupled affine equalities in the parent.
    for (row, &axis) in axes.iter().enumerate() {
        let value = &stationary[(row as u32, 0)];
        if !value.is_integer()
            || parent.fixed()[axis].is_some_and(|fixed| value != &Rational::from(fixed))
            || (value > &Rational::zero()) != sector[axis]
        {
            return Some(QuadraticProof::Empty);
        }
    }
    // At a definite quadratic's zero minimum, q=0 iff its gradient vanishes.
    // Reuse the native gradient polynomials instead of constructing or rounding
    // stationary coordinates. The ordinary affine service will intersect these
    // exact equations with every parent constraint and the whole sector.
    Some(QuadraticProof::EquivalentAffine(gradient_polynomials))
}

#[cfg(test)]
fn proves_empty<const N: usize>(
    parent: &Case<N>,
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
    sector: &[bool; N],
) -> bool {
    matches!(
        classify(parent, equation, indices, sector),
        QuadraticProof::Empty
    )
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;
    use crate::solver::{Case, CoordinateCase};

    use super::{QuadraticProof, classify, proves_empty};

    #[test]
    fn zero_minimum_returns_native_gradients_on_supported_axes_only() {
        let context = CoefficientContext::new(["d", "x", "unused", "y", "also_unused"]);
        let indices = [1, 2, 3, 4];
        let sector = [true, false, false, true];
        let parent = Case::<4>::generic();
        let equation = context
            .coefficient_fixture("2*(x-1)^2+2*(y+2)^2+(x-y-3)^2")
            .numerator;
        for equation in [equation.clone(), -equation] {
            let QuadraticProof::EquivalentAffine(gradients) =
                classify(&parent, &equation, &indices, &sector)
            else {
                panic!("an admitted definite zero minimum has an affine zero set");
            };
            assert_eq!(
                gradients,
                vec![equation.derivative(1), equation.derivative(3)]
            );
            let child = parent
                .intersect(&gradients, &indices, &sector)
                .unwrap()
                .unwrap();
            assert_eq!(child.fixed(), &[Some(1), None, Some(-2), None]);
        }
    }

    #[test]
    fn strict_positive_and_opposite_orientation_are_empty() {
        let context = CoefficientContext::new(["x", "y"]);
        let parent = Case::<2>::generic();
        let positive = context.coefficient_fixture("x^2+y^2+1").numerator;
        for equation in [positive.clone(), -positive] {
            assert!(proves_empty(&parent, &equation, &[0, 1], &[true, false]));
        }
    }

    #[test]
    fn exact_zero_minimum_needs_an_excluded_integer_coordinate() {
        let context = CoefficientContext::new(["d", "x", "y"]);
        let equation = context
            .coefficient_fixture("2*x^2+2*(y-1)^2+(x-y+1)^2")
            .numerator;
        let parent = Case::<2>::generic();
        assert!(proves_empty(&parent, &equation, &[1, 2], &[false, false]));
        assert!(!proves_empty(&parent, &equation, &[1, 2], &[false, true]));
        let fixed: Case<2> = CoordinateCase::new([Some(1), None]).unwrap().into();
        assert!(proves_empty(&fixed, &equation, &[1, 2], &[true, true]));
    }

    #[test]
    fn noninteger_unique_minimum_is_empty_but_other_shapes_remain_unknown() {
        let context = CoefficientContext::new(["d", "x", "y"]);
        let parent = Case::<2>::generic();
        let parse = |input| context.coefficient_fixture(input).numerator;
        assert!(proves_empty(
            &parent,
            &parse("(2*x-1)^2+y^2"),
            &[1, 2],
            &[true, false],
        ));
        for input in ["x^2+y^2-1", "x^2-y^2+1", "x^2+y+1", "x^2+y^2+d"] {
            assert!(!proves_empty(
                &parent,
                &parse(input),
                &[1, 2],
                &[true, false]
            ));
        }
    }
}

#[cfg(test)]
#[path = "definite_quadratic/adversarial_tests.rs"]
mod adversarial_tests;
